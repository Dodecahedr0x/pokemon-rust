//! The Art tab's catalog and the `cargo xtask silice gfx` command it runs.
//!
//! Tileset specs are `games/<game>/gfx/<Tileset>.ron` (not `sprites.ron`); sprites are
//! `gfx/sprites.ron`. Generation itself stays in xtask (the model, the reference images, the
//! prep): this module lists the specs and builds the command, so a retry from the studio is
//! the same run as the CLI.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `--model` values. The same names as `xtask`'s `image::Model`.
pub const MODELS: &[(&str, &str)] = &[
    ("codex", "Codex"),
    ("ark", "Ark Seedream"),
    ("nano", "Nano Banana"),
];

/// How many reference images `model` accepts. Kept in step with xtask's `Model::ref_limit`.
pub fn ref_limit(model: &str) -> usize {
    match model {
        "ark" => 10,
        "nano" => 3,
        _ => 8,
    }
}

/// What the model picker says about keys and defaults (the defaults match xtask).
pub fn model_hint(model: &str) -> &'static str {
    match model {
        "ark" => {
            "Needs ARK_API_KEY. Optional ARK_MODEL (default doubao-seedream-4-5-251128), \
             ARK_SIZE (default 2K) and ARK_BASE_URL. Up to 10 reference images."
        }
        "nano" => {
            "Needs GEMINI_API_KEY. Optional GEMINI_IMAGE_MODEL (default gemini-2.5-flash-image). \
             Up to 3 reference images."
        }
        _ => "Codex CLI image_gen. Up to 8 reference images, attached to the prompt.",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tileset,
    Sprite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSpec {
    pub name: String,
    pub prompt: Option<String>,
    pub mirror: Option<String>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilesetSpec {
    pub name: String,
    pub style: String,
    pub features: Vec<FeatureSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSpec {
    pub name: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub tilesets: Vec<TilesetSpec>,
    pub sprite_style: String,
    pub sprites: Vec<SpriteSpec>,
    pub errors: Vec<String>,
}

impl Catalog {
    /// Reads `games/gfx`. A spec that does not parse is an error; the others are still listed.
    pub fn load(games: &Path) -> Catalog {
        let mut cat = Catalog::default();
        let dir = games.join("gfx");
        if !dir.is_dir() {
            return cat;
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .collect();
        paths.sort();
        for p in paths {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            if stem == "sprites" {
                match load_sprites(&p) {
                    Ok((style, sprites)) => {
                        cat.sprite_style = style;
                        cat.sprites = sprites;
                    }
                    Err(e) => cat.errors.push(format!("sprites.ron: {e:#}")),
                }
                continue;
            }
            match load_tileset(&p, &stem) {
                Ok(t) => cat.tilesets.push(t),
                Err(e) => cat.errors.push(format!("{stem}.ron: {e:#}")),
            }
        }
        cat
    }

    pub fn tileset(&self, name: &str) -> Option<&TilesetSpec> {
        self.tilesets.iter().find(|t| t.name == name)
    }

    fn sprite(&self, name: &str) -> Option<&SpriteSpec> {
        self.sprites.iter().find(|s| s.name == name)
    }
}

#[derive(Deserialize)]
struct TilesetFile {
    #[serde(default)]
    style: String,
    features: Vec<FeatureFile>,
}

#[derive(Deserialize)]
struct FeatureFile {
    name: String,
    cells: Vec<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    mirror: Option<String>,
}

#[derive(Deserialize)]
struct SpriteFile {
    #[serde(default)]
    style: String,
    sprites: Vec<SpriteRow>,
}

#[derive(Deserialize)]
struct SpriteRow {
    name: String,
    prompt: String,
}

fn load_tileset(path: &Path, stem: &str) -> Result<TilesetSpec> {
    check_token(stem)?;
    let file: TilesetFile = ron::from_str(&std::fs::read_to_string(path)?)
        .with_context(|| path.display().to_string())?;
    let mut features = Vec::new();
    for f in file.features {
        check_token(&f.name)?;
        let width = f.cells.first().context("no cells")?.chars().count() as u32;
        ensure!(width > 0, "feature {} has an empty row", f.name);
        features.push(FeatureSpec {
            name: f.name,
            prompt: f.prompt,
            mirror: f.mirror,
            width,
            height: f.cells.len() as u32,
        });
    }
    Ok(TilesetSpec {
        name: stem.to_string(),
        style: file.style,
        features,
    })
}

fn load_sprites(path: &Path) -> Result<(String, Vec<SpriteSpec>)> {
    let file: SpriteFile = ron::from_str(&std::fs::read_to_string(path)?)
        .with_context(|| path.display().to_string())?;
    let mut sprites = Vec::new();
    for s in file.sprites {
        check_token(&s.name)?;
        sprites.push(SpriteSpec {
            name: s.name,
            prompt: s.prompt,
        });
    }
    Ok((file.style, sprites))
}

fn check_token(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "{name:?} is not a name"
    );
    Ok(())
}

/// What one Generate press asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Feature { tileset: String, feature: String },
    Sprite { name: String },
}

/// The pictures a selection can show: the previous raw, the latest raw, the prepped art.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Previews {
    pub prev: PathBuf,
    pub raw: PathBuf,
    pub art: PathBuf,
}

/// Form state for the Art tab. The prompt is this run's subject; it is not written to the spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtState {
    pub catalog: Catalog,
    pub kind: Kind,
    pub tileset: String,
    pub feature: String,
    pub sprite: String,
    pub model: String,
    pub prompt: String,
    pub refs: Vec<PathBuf>,
    pub note: String,
    /// Bumped when a generation finishes, so the previews reload.
    pub generation: u64,
}

impl Default for ArtState {
    fn default() -> Self {
        Self {
            catalog: Catalog::default(),
            kind: Kind::Tileset,
            tileset: String::new(),
            feature: String::new(),
            sprite: String::new(),
            model: "codex".into(),
            prompt: String::new(),
            refs: Vec::new(),
            note: String::new(),
            generation: 0,
        }
    }
}

impl ArtState {
    pub fn load(games: &Path) -> ArtState {
        let mut s = ArtState {
            catalog: Catalog::load(games),
            ..ArtState::default()
        };
        if let Some(name) = s.catalog.tilesets.first().map(|t| t.name.clone()) {
            s.select_tileset(&name);
        }
        if let Some(name) = s.catalog.sprites.first().map(|sp| sp.name.clone()) {
            s.sprite = name;
        }
        s
    }

    pub fn select_tileset(&mut self, name: &str) {
        let Some(t) = self.catalog.tileset(name).cloned() else {
            return;
        };
        self.tileset = t.name;
        let f = t
            .features
            .iter()
            .find(|f| f.mirror.is_none() && f.prompt.is_some())
            .or(t.features.first());
        if let Some(f) = f.cloned() {
            self.apply_feature(&f);
        }
    }

    pub fn select_feature(&mut self, name: &str) {
        let Some(t) = self.catalog.tileset(&self.tileset).cloned() else {
            return;
        };
        if let Some(f) = t.features.iter().find(|f| f.name == name).cloned() {
            self.apply_feature(&f);
        }
    }

    fn apply_feature(&mut self, f: &FeatureSpec) {
        self.feature = f.name.clone();
        self.kind = Kind::Tileset;
        self.prompt = f.prompt.clone().unwrap_or_default();
        self.note = match &f.mirror {
            Some(src) => format!("{} is a mirror of {src}", f.name),
            None => String::new(),
        };
    }

    pub fn select_sprite(&mut self, name: &str) {
        let Some(s) = self.catalog.sprite(name).cloned() else {
            return;
        };
        self.sprite = s.name;
        if self.kind == Kind::Sprite {
            self.prompt = s.prompt;
            self.note.clear();
        }
    }

    pub fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
        self.note.clear();
        match kind {
            Kind::Tileset => self.select_feature(&self.feature.clone()),
            Kind::Sprite => {
                let name = self.sprite.clone();
                self.select_sprite(&name);
            }
        }
    }

    pub fn set_model(&mut self, model: &str) {
        if MODELS.iter().all(|(id, _)| *id != model) {
            return;
        }
        self.model = model.to_string();
        let n = ref_limit(model);
        if self.refs.len() > n {
            self.refs.truncate(n);
            self.note = format!("kept the first {n} reference images");
        }
    }

    /// Appends reference images that exist, up to the model's limit. Duplicates are skipped.
    pub fn add_refs(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let n = ref_limit(&self.model);
        for p in paths {
            if !p.is_file() {
                self.note = format!("{} is not a file", p.display());
                continue;
            }
            if self.refs.iter().any(|r| r == &p) {
                continue;
            }
            if self.refs.len() >= n {
                self.note = format!("{n} reference images is the limit for this model");
                return;
            }
            self.refs.push(p);
        }
    }

    pub fn target(&self) -> Result<Target> {
        match self.kind {
            Kind::Tileset => {
                let t = self
                    .catalog
                    .tileset(&self.tileset)
                    .context("pick a tileset")?;
                let f = t
                    .features
                    .iter()
                    .find(|f| f.name == self.feature)
                    .context("pick a feature")?;
                if let Some(src) = &f.mirror {
                    bail!("{} is a mirror of {src}", f.name);
                }
                Ok(Target::Feature {
                    tileset: t.name.clone(),
                    feature: f.name.clone(),
                })
            }
            Kind::Sprite => {
                let s = self.catalog.sprite(&self.sprite).context("pick a sprite")?;
                Ok(Target::Sprite {
                    name: s.name.clone(),
                })
            }
        }
    }

    pub fn style(&self) -> &str {
        match self.kind {
            Kind::Tileset => self
                .catalog
                .tileset(&self.tileset)
                .map(|t| t.style.as_str())
                .unwrap_or(""),
            Kind::Sprite => self.catalog.sprite_style.as_str(),
        }
    }
}

/// Changes when the selection or a finished generation should reload the previews.
pub fn preview_key(s: &ArtState) -> String {
    match s.kind {
        Kind::Tileset => format!("t:{}:{}:{}", s.tileset, s.feature, s.generation),
        Kind::Sprite => format!("s:{}:{}", s.sprite, s.generation),
    }
}

pub fn preview_paths(workspace: &Path, games: &Path, s: &ArtState) -> Option<Previews> {
    match s.kind {
        Kind::Tileset => {
            check_token(&s.tileset).ok()?;
            check_token(&s.feature).ok()?;
            let raw_dir = workspace.join(format!("target/silice-gfx/raw/{}", s.tileset));
            Some(Previews {
                prev: raw_dir.join(format!("{}.prev.png", s.feature)),
                raw: raw_dir.join(format!("{}.png", s.feature)),
                art: games.join(format!("gfx/{}/{}.png", s.tileset, s.feature)),
            })
        }
        Kind::Sprite => {
            check_token(&s.sprite).ok()?;
            let raw_dir = workspace.join("target/silice-gfx/raw/sprites");
            Some(Previews {
                prev: raw_dir.join(format!("{}.prev.png", s.sprite)),
                raw: raw_dir.join(format!("{}.png", s.sprite)),
                art: games.join(format!("sprites/{}.png", s.sprite)),
            })
        }
    }
}

/// `cargo xtask silice gfx gen|sprites …` in `workspace`. The prompt is the subject only;
/// xtask adds the spec's style and the size rules.
pub fn gen_command(
    workspace: &Path,
    games: &Path,
    model: &str,
    target: &Target,
    refs: &[PathBuf],
    prompt: &str,
) -> Result<Command> {
    ensure!(MODELS.iter().any(|(id, _)| *id == model), "model {model:?}");
    ensure!(!prompt.trim().is_empty(), "the prompt is empty");
    ensure!(
        refs.len() <= ref_limit(model),
        "{} reference images; this model accepts at most {}",
        refs.len(),
        ref_limit(model)
    );
    for r in refs {
        ensure!(r.is_file(), "reference image {} is not a file", r.display());
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut c = Command::new(cargo);
    c.current_dir(workspace);
    c.args(["xtask", "silice", "gfx"]);
    match target {
        Target::Feature { tileset, feature } => {
            check_token(tileset)?;
            check_token(feature)?;
            c.args(["gen", tileset, feature]);
        }
        Target::Sprite { name } => {
            check_token(name)?;
            c.args(["sprites", name]);
        }
    }
    c.args(["--model", model, "--jobs", "1", "--games"])
        .arg(games);
    for r in refs {
        c.arg("--ref").arg(r);
    }
    c.arg("--prompt").arg(prompt);
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pkr-art-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let gfx = dir.join("gfx");
        std::fs::create_dir_all(&gfx).unwrap();
        std::fs::write(
            gfx.join("SilTest.ron"),
            r#"(
                name: "SilTest",
                primary: "General",
                style: "flat",
                features: [
                    (name: "house", cells: ["AA", "AD"], prompt: Some("a house")),
                    (name: "house_r", cells: ["AA", "AD"], mirror: Some("house")),
                ],
            )"#,
        )
        .unwrap();
        std::fs::write(
            gfx.join("sprites.ron"),
            r#"(
                style: "chibi",
                sprites: [(name: "SIL_A", prompt: "a poet")],
            )"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn the_catalog_lists_features_and_sprites_and_skips_mirrors_for_the_first_prompt() {
        let dir = fixture();
        let mut s = ArtState::load(&dir);
        assert!(s.catalog.errors.is_empty(), "{:?}", s.catalog.errors);
        assert_eq!(s.tileset, "SilTest");
        assert_eq!(s.feature, "house");
        assert_eq!(s.prompt, "a house");
        assert_eq!(s.sprite, "SIL_A");
        s.select_feature("house_r");
        assert!(s.target().is_err());
        assert!(s.note.contains("mirror"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_command_names_the_model_the_refs_and_the_prompt() {
        let dir = fixture();
        let png = dir.join("ref.png");
        std::fs::write(&png, b"png").unwrap();
        let cmd = gen_command(
            &dir,
            &dir,
            "nano",
            &Target::Feature {
                tileset: "SilTest".into(),
                feature: "house".into(),
            },
            &[png.clone()],
            "a tin house",
        )
        .unwrap();
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w == ["gen", "SilTest"]), "{args:?}");
        assert!(
            args.windows(2).any(|w| w == ["--model", "nano"]),
            "{args:?}"
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--ref", png.to_str().unwrap()]),
            "{args:?}"
        );
        assert!(
            args.windows(2).any(|w| w == ["--prompt", "a tin house"]),
            "{args:?}"
        );
        let sprite = gen_command(
            &dir,
            &dir,
            "ark",
            &Target::Sprite {
                name: "SIL_A".into(),
            },
            &[],
            "a poet",
        )
        .unwrap();
        let args: Vec<String> = sprite
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2).any(|w| w == ["sprites", "SIL_A"]),
            "{args:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_to_nano_drops_refs_past_three() {
        let mut s = ArtState::default();
        let dir = std::env::temp_dir().join(format!("pkr-art-refs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for i in 0..4 {
            let p = dir.join(format!("{i}.png"));
            std::fs::write(&p, b"x").unwrap();
            paths.push(p);
        }
        s.add_refs(paths);
        assert_eq!(s.refs.len(), 4);
        s.set_model("nano");
        assert_eq!(s.refs.len(), 3);
        assert!(s.note.contains('3'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
