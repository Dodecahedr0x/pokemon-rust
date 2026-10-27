//! Image models for `silice gfx gen` and `silice gfx sprites`.
//!
//! `codex` is the Codex CLI's `image_gen` (the default). `ark` is Volcengine Ark Seedream
//! (`ARK_API_KEY`; `ARK_MODEL`, default [`ARK_MODEL_DEFAULT`]; `ARK_SIZE`, default
//! [`ARK_SIZE_DEFAULT`]; `ARK_BASE_URL`). `nano` is Gemini's Nano Banana
//! (`GEMINI_API_KEY` or `GOOGLE_API_KEY`; `GEMINI_IMAGE_MODEL`, default [`NANO_MODEL_DEFAULT`]).
//! `--ref` pictures are attached as references (Codex `--image`, Ark `image`, Gemini inline data).

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const ARK_MODEL_DEFAULT: &str = "doubao-seedream-4-5-251128";
pub const ARK_SIZE_DEFAULT: &str = "2K";
pub const ARK_BASE_DEFAULT: &str = "https://ark.cn-beijing.volces.com/api/v3";
pub const NANO_MODEL_DEFAULT: &str = "gemini-2.5-flash-image";

const ARK_REF_BYTES: u64 = 30 * 1024 * 1024;
const NANO_REF_BYTES: u64 = 7 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Codex,
    Ark,
    Nano,
}

impl Model {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "codex" => Self::Codex,
            "ark" | "seedream" => Self::Ark,
            "nano" | "banana" | "gemini" => Self::Nano,
            _ => bail!("model {s:?} is not codex, ark or nano"),
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Ark => "Ark Seedream",
            Self::Nano => "Nano Banana",
        }
    }

    /// How many reference images this model accepts.
    pub fn ref_limit(self) -> usize {
        match self {
            Self::Codex => 8,
            Self::Ark => 10,
            Self::Nano => 3,
        }
    }

    /// What `gen` prints, including the model id the HTTP backends will call.
    pub fn detail(self) -> String {
        match self {
            Self::Codex => "codex".into(),
            Self::Ark => format!("ark {}", env_or("ARK_MODEL", ARK_MODEL_DEFAULT)),
            Self::Nano => format!("nano {}", env_or("GEMINI_IMAGE_MODEL", NANO_MODEL_DEFAULT)),
        }
    }
}

/// Flags shared by `gfx gen` and `gfx sprites`.
#[derive(Debug)]
pub struct Flags {
    pub games: PathBuf,
    pub prep_only: bool,
    pub jobs: usize,
    pub model: Model,
    pub refs: Vec<PathBuf>,
    pub prompt: Option<String>,
    pub rest: Vec<String>,
}

pub fn parse_flags(args: &[String], usage: &'static str, mut games: PathBuf) -> Result<Flags> {
    let mut prep_only = false;
    let mut jobs = 4usize;
    let mut model = Model::Codex;
    let mut refs = Vec::new();
    let mut prompt = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--games" => games = PathBuf::from(it.next().context(usage)?),
            "--prep-only" => prep_only = true,
            "--jobs" => {
                jobs = it.next().context(usage)?.parse().context("--jobs")?;
            }
            "--model" => model = Model::parse(it.next().context(usage)?)?,
            "--ref" => refs.push(PathBuf::from(it.next().context(usage)?)),
            "--prompt" => {
                let p = it.next().context(usage)?;
                ensure!(!p.trim().is_empty(), "--prompt is empty");
                prompt = Some(p.clone());
            }
            s if !s.starts_with('-') => rest.push(s.to_string()),
            _ => bail!("{usage}"),
        }
    }
    for r in &refs {
        ensure!(r.is_file(), "reference image {} is not a file", r.display());
        mime(r)?;
    }
    ensure!(
        refs.len() <= model.ref_limit(),
        "{} reference images; {} accepts at most {}",
        refs.len(),
        model.label(),
        model.ref_limit()
    );
    Ok(Flags {
        games,
        prep_only,
        jobs,
        model,
        refs,
        prompt,
        rest,
    })
}

/// One picture. JPEG replies are re-encoded as PNG (Seedream 4.5 returns JPEG).
pub fn generate(model: Model, prompt: &str, refs: &[PathBuf]) -> Result<PathBuf> {
    ensure!(
        refs.len() <= model.ref_limit(),
        "{} reference images; {} accepts at most {}",
        refs.len(),
        model.label(),
        model.ref_limit()
    );
    let prompt = instructed(prompt, refs);
    let bytes = match model {
        Model::Codex => return codex(&prompt, refs),
        Model::Ark => ark(&prompt, refs)?,
        Model::Nano => nano(&prompt, refs)?,
    };
    let png = to_png(&bytes).context("the model reply was not a png or jpeg")?;
    let path = scratch()?.join("out.png");
    std::fs::write(&path, png).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn instructed(prompt: &str, refs: &[PathBuf]) -> String {
    if refs.is_empty() {
        prompt.to_string()
    } else {
        format!(
            "{prompt}\n\nReference images are attached, in order. Follow their style, colours and \
             subject. Do not copy their background or any text in them."
        )
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn scratch() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "pkr-gfx-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn mime(path: &Path) -> Result<&'static str> {
    Ok(
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            _ => bail!("reference {} must be png, jpeg or webp", path.display()),
        },
    )
}

struct RefImage {
    mime: &'static str,
    b64: String,
}

fn load_refs(paths: &[PathBuf], max: u64) -> Result<Vec<RefImage>> {
    paths
        .iter()
        .map(|p| {
            let mime = mime(p)?;
            let meta = std::fs::metadata(p)?;
            ensure!(
                meta.len() <= max,
                "reference {} is {} bytes; the limit is {max}",
                p.display(),
                meta.len()
            );
            let bytes = std::fs::read(p)?;
            Ok(RefImage {
                mime,
                b64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes),
            })
        })
        .collect()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(180))
        .timeout_write(Duration::from_secs(60))
        .build()
}

fn error_text(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body: String = resp
                .into_string()
                .unwrap_or_default()
                .chars()
                .take(2000)
                .collect();
            format!("HTTP {code}: {body}")
        }
        other => other.to_string(),
    }
}

fn post_json(url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Value> {
    let mut req = agent().post(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.send_json(body) {
        Ok(resp) => resp.into_json().context("reading the image reply"),
        Err(e) => bail!("{}", error_text(e)),
    }
}

fn ark(prompt: &str, refs: &[PathBuf]) -> Result<Vec<u8>> {
    let key = std::env::var("ARK_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .context("ARK_API_KEY is not set (Ark Seedream)")?;
    let model = env_or("ARK_MODEL", ARK_MODEL_DEFAULT);
    let size = env_or("ARK_SIZE", ARK_SIZE_DEFAULT);
    let base = env_or("ARK_BASE_URL", ARK_BASE_DEFAULT);
    let images = load_refs(refs, ARK_REF_BYTES)?;
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "size": size,
        "response_format": "b64_json",
        "watermark": false,
    });
    if !images.is_empty() {
        body["image"] = Value::Array(
            images
                .iter()
                .map(|r| Value::String(format!("data:{};base64,{}", r.mime, r.b64)))
                .collect(),
        );
    }
    let url = format!("{}/images/generations", base.trim_end_matches('/'));
    let reply = post_json(&url, &[("Authorization", &format!("Bearer {key}"))], &body)
        .with_context(|| format!("ark {model}"))?;
    image_from_ark(&reply).with_context(|| format!("ark {model}"))
}

fn image_from_ark(reply: &Value) -> Result<Vec<u8>> {
    if let Some(msg) = reply.pointer("/error/message").and_then(Value::as_str) {
        bail!("{msg}");
    }
    let item = reply
        .pointer("/data/0")
        .context("the reply has no data[0]")?;
    if let Some(b64) = item.get("b64_json").and_then(Value::as_str) {
        return decode_b64(b64);
    }
    if let Some(url) = item.get("url").and_then(Value::as_str) {
        ensure!(url.starts_with("https://"), "image url {url}");
        return http_bytes(url, ARK_REF_BYTES);
    }
    bail!("the reply has no b64_json or url: {reply}")
}

fn nano(prompt: &str, refs: &[PathBuf]) -> Result<Vec<u8>> {
    let key = ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|s| !s.trim().is_empty()))
        .context("GEMINI_API_KEY is not set (Nano Banana)")?;
    let model = env_or("GEMINI_IMAGE_MODEL", NANO_MODEL_DEFAULT);
    let images = load_refs(refs, NANO_REF_BYTES)?;
    let mut parts = vec![json!({"text": prompt})];
    for r in &images {
        parts.push(json!({
            "inlineData": {"mimeType": r.mime, "data": r.b64}
        }));
    }
    let body = json!({
        "contents": [{"role": "user", "parts": parts}],
        "generationConfig": {"responseModalities": ["TEXT", "IMAGE"]}
    });
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let reply = post_json(&url, &[("x-goog-api-key", &key)], &body)
        .with_context(|| format!("nano {model}"))?;
    image_from_nano(&reply).with_context(|| format!("nano {model}"))
}

fn image_from_nano(reply: &Value) -> Result<Vec<u8>> {
    if let Some(msg) = reply.pointer("/error/message").and_then(Value::as_str) {
        bail!("{msg}");
    }
    let parts = reply
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .context("the reply has no image")?;
    let mut text = Vec::new();
    let mut image = None;
    for part in parts {
        if let Some(t) = part.get("text").and_then(Value::as_str) {
            text.push(t.to_string());
        }
        let inline = part.get("inlineData").or(part.get("inline_data"));
        if let Some(b64) = inline.and_then(|v| v.get("data")).and_then(Value::as_str) {
            image = Some(decode_b64(b64)?);
        }
    }
    image.with_context(|| {
        let why = if text.is_empty() {
            "the reply has no image".to_string()
        } else {
            format!("the reply has no image: {}", text.join(" "))
        };
        why
    })
}

fn decode_b64(s: &str) -> Result<Vec<u8>> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s.trim()).context("base64")
}

fn http_bytes(url: &str, max: u64) -> Result<Vec<u8>> {
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("{}", error_text(e)))?;
    let mut buf = Vec::new();
    resp.into_reader().take(max + 1).read_to_end(&mut buf)?;
    ensure!(
        buf.len() as u64 <= max,
        "response is larger than {max} bytes"
    );
    Ok(buf)
}

/// PNG bytes unchanged; JPEG decoded and re-encoded as RGBA PNG.
fn to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(bytes.to_vec());
    }
    ensure!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "not a png or jpeg");
    let mut dec = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    let pixels = dec.decode().map_err(|e| anyhow::anyhow!("jpeg: {e}"))?;
    let info = dec.info().context("jpeg has no header")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            ensure!(pixels.len() == (w * h * 3) as usize, "jpeg rgb size");
            pixels
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect::<Vec<u8>>()
        }
        jpeg_decoder::PixelFormat::L8 => {
            ensure!(pixels.len() == (w * h) as usize, "jpeg grey size");
            pixels
                .iter()
                .flat_map(|&c| [c, c, c, 255])
                .collect::<Vec<u8>>()
        }
        jpeg_decoder::PixelFormat::CMYK32 | jpeg_decoder::PixelFormat::L16 => {
            bail!("unsupported jpeg ({:?})", info.pixel_format)
        }
    };
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    Ok(out)
}

fn codex(prompt: &str, refs: &[PathBuf]) -> Result<PathBuf> {
    let dir = scratch()?;
    let mut cmd = std::process::Command::new("codex");
    cmd.args(["exec", "--skip-git-repo-check", "-s", "read-only"]);
    for r in refs {
        cmd.arg("--image").arg(r);
    }
    cmd.arg(format!(
        "Use your built-in image_gen tool exactly once to generate the image described below, \
         then stop. Do not post-process, resize or save any file. Image: {prompt}"
    ))
    .current_dir(&dir);
    let out = cmd.output().context("running codex")?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let session = log
        .lines()
        .find_map(|l| l.strip_prefix("session id: "))
        .map(str::trim)
        .with_context(|| format!("no session id in codex's output\n{}", tail(&log)))?;
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex")
        });
    let gen_dir = home.join("generated_images").join(session);
    let mut imgs: Vec<PathBuf> = std::fs::read_dir(&gen_dir)
        .with_context(|| {
            format!(
                "codex made no image ({})\n{}",
                gen_dir.display(),
                tail(&log)
            )
        })?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    imgs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    imgs.pop().with_context(|| {
        format!(
            "codex made no image ({})\n{}",
            gen_dir.display(),
            tail(&log)
        )
    })
}

fn tail(log: &str) -> String {
    let n = log.len().saturating_sub(1500);
    log[n..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_and_limits() {
        assert_eq!(Model::parse("seedream").unwrap(), Model::Ark);
        assert_eq!(Model::parse("banana").unwrap(), Model::Nano);
        assert!(Model::parse("dalle").is_err());
        assert_eq!(Model::Nano.ref_limit(), 3);
        assert_eq!(Model::Ark.ref_limit(), 10);
    }

    #[test]
    fn flags_keep_the_model_the_refs_and_one_prompt() {
        let dir = std::env::temp_dir().join(format!("pkr-gfx-flags-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("ref.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        let games = dir.join("games");
        let args = vec![
            "--model".into(),
            "nano".into(),
            "--ref".into(),
            png.display().to_string(),
            "--prompt".into(),
            "a house".into(),
            "--games".into(),
            games.display().to_string(),
            "SilTest".into(),
            "house".into(),
        ];
        let f = parse_flags(&args, "usage", PathBuf::from("unused")).unwrap();
        assert_eq!(f.model, Model::Nano);
        assert_eq!(f.refs, vec![png]);
        assert_eq!(f.prompt.as_deref(), Some("a house"));
        assert_eq!(f.rest, vec!["SilTest".to_string(), "house".to_string()]);
        assert_eq!(f.games, games);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn four_refs_are_too_many_for_nano() {
        let dir = std::env::temp_dir().join(format!("pkr-gfx-refs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut args = vec!["--model".into(), "nano".into()];
        for i in 0..4 {
            let p = dir.join(format!("{i}.png"));
            std::fs::write(&p, b"\x89PNG\r\n\x1a\n").unwrap();
            args.push("--ref".into());
            args.push(p.display().to_string());
        }
        let err = parse_flags(&args, "usage", PathBuf::from(".")).unwrap_err();
        assert!(err.to_string().contains("at most 3"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ark_and_nano_replies_decode_to_the_picture() {
        let png = tiny_png();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
        let ark = json!({"data": [{"b64_json": b64}]});
        assert_eq!(image_from_ark(&ark).unwrap(), png);
        let nano = json!({
            "candidates": [{"content": {"parts": [
                {"text": "ok"},
                {"inlineData": {"mimeType": "image/png", "data": b64}}
            ]}}]
        });
        assert_eq!(image_from_nano(&nano).unwrap(), png);
        let refused = json!({
            "candidates": [{"content": {"parts": [{"text": "no"}]}}]
        });
        assert!(
            image_from_nano(&refused)
                .unwrap_err()
                .to_string()
                .contains("no")
        );
    }

    #[test]
    fn a_jpeg_reply_becomes_a_png() {
        let png = to_png(include_bytes!("one.jpg")).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(to_png(&tiny_png()).unwrap(), tiny_png());
    }

    fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 1, 1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255])
            .unwrap();
        out
    }
}
