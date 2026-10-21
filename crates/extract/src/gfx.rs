//! 4bpp tiles, GBA palettes, JASC .pal files and indexed PNG I/O.
//!
//! Conventions follow `tools/gbagfx`: in a 4bpp tile the low nibble is the left pixel;
//! 5-bit to 8-bit colour is `x * 255 / 31`, 8-bit to 5-bit is `x / 8`.

use anyhow::{Context, Result, bail, ensure};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub type Rgb555 = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette(pub [Rgb555; 16]);

impl Palette {
    /// Reads 16 little-endian BGR555 colours from the first 32 bytes of `b`.
    pub fn from_bytes(b: &[u8]) -> Result<Palette> {
        ensure!(b.len() >= 32, "palette needs 32 bytes, got {}", b.len());
        let mut c = [0; 16];
        for (i, v) in c.iter_mut().enumerate() {
            *v = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        }
        Ok(Palette(c))
    }

    /// Colour `i` (only the low 4 bits are used) as opaque 8-bit RGBA.
    pub fn rgba(&self, i: u8) -> [u8; 4] {
        let c = self.0[(i & 0xF) as usize];
        let up = |x: u16| ((x & 0x1F) as u32 * 255 / 31) as u8;
        [up(c), up(c >> 5), up(c >> 10), 255]
    }
}

/// Reads a decomp JASC-PAL file (up to 16 colours; missing ones are 0).
pub fn read_jasc_pal(path: &Path) -> Result<Palette> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_jasc(&text).with_context(|| format!("parsing {}", path.display()))
}

fn parse_jasc(text: &str) -> Result<Palette> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    ensure!(lines.next() == Some("JASC-PAL"), "missing JASC-PAL magic");
    ensure!(lines.next() == Some("0100"), "unsupported JASC version");
    let count: usize = lines.next().context("missing colour count")?.parse()?;
    ensure!(count <= 16, "{count} colours; at most 16 supported");
    let mut pal = [0u16; 16];
    for (i, slot) in pal.iter_mut().enumerate().take(count) {
        let line = lines
            .next()
            .with_context(|| format!("missing colour {i}"))?;
        let rgb: Vec<u8> = line
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .with_context(|| format!("colour {i}: {line:?}"))?;
        ensure!(
            rgb.len() == 3,
            "colour {i}: expected 3 components in {line:?}"
        );
        let down = |x: u8| (x / 8) as u16;
        *slot = down(rgb[0]) | down(rgb[1]) << 5 | down(rgb[2]) << 10;
    }
    Ok(Palette(pal))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Lay out 4bpp tiles (32 bytes each) left-to-right, `tiles_per_row` per row.
/// A partial last row is padded with index 0.
pub fn tiles_4bpp_to_image(data: &[u8], tiles_per_row: u32) -> Result<IndexedImage> {
    ensure!(tiles_per_row > 0, "tiles_per_row must be > 0");
    ensure!(
        data.len().is_multiple_of(32),
        "4bpp data length {} is not a multiple of 32",
        data.len()
    );
    let tiles = data.len() / 32;
    let tpr = tiles_per_row as usize;
    let rows = tiles.div_ceil(tpr);
    let width = tpr * 8;
    let mut pixels = vec![0u8; width * rows * 8];
    for (t, tile) in data.chunks_exact(32).enumerate() {
        let (tx, ty) = (t % tpr * 8, t / tpr * 8);
        for (k, &b) in tile.iter().enumerate() {
            let (x, y) = (tx + k % 4 * 2, ty + k / 4);
            pixels[y * width + x] = b & 0xF;
            pixels[y * width + x + 1] = b >> 4;
        }
    }
    Ok(IndexedImage {
        width: width as u32,
        height: (rows * 8) as u32,
        pixels,
    })
}

/// Inverse of [`tiles_4bpp_to_image`]: tiles in row-major order, including any padding tiles.
pub fn image_to_tiles_4bpp(img: &IndexedImage) -> Result<Vec<u8>> {
    let (w, h) = (img.width as usize, img.height as usize);
    ensure!(
        w.is_multiple_of(8) && h.is_multiple_of(8),
        "image {w}x{h} is not a multiple of 8"
    );
    ensure!(img.pixels.len() == w * h, "pixel buffer size mismatch");
    if let Some(p) = img.pixels.iter().find(|&&p| p > 15) {
        bail!("pixel index {p} does not fit in 4bpp");
    }
    let mut out = Vec::with_capacity(w * h / 2);
    for ty in (0..h).step_by(8) {
        for tx in (0..w).step_by(8) {
            for y in ty..ty + 8 {
                for x in (tx..tx + 8).step_by(2) {
                    out.push(img.pixels[y * w + x] | img.pixels[y * w + x + 1] << 4);
                }
            }
        }
    }
    Ok(out)
}

/// Reads an indexed (or greyscale) PNG of bit depth 1/2/4/8 into one index per pixel.
/// The PNG palette itself is ignored.
pub fn read_png_indexed(path: &Path) -> Result<IndexedImage> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("reading {}", path.display()))?;
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let (ct, depth) = (info.color_type, info.bit_depth as u8);
    ensure!(
        matches!(ct, png::ColorType::Indexed | png::ColorType::Grayscale) && depth <= 8,
        "{}: not an indexed PNG ({ct:?}, {depth}-bit)",
        path.display()
    );
    let mut buf = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf)?;
    let stride = frame.line_size;
    let per_byte = 8 / depth as usize;
    let mask = ((1u16 << depth) - 1) as u8;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in buf.chunks(stride).take(height as usize) {
        for x in 0..width as usize {
            let shift = 8 - depth as usize * (x % per_byte + 1);
            pixels.push(row[x / per_byte] >> shift & mask);
        }
    }
    Ok(IndexedImage {
        width,
        height,
        pixels,
    })
}

/// Writes a 4-bit indexed PNG with `palette` (indices must be < 16).
pub fn write_png_indexed(path: &Path, img: &IndexedImage, palette: &Palette) -> Result<()> {
    let (w, h) = (img.width as usize, img.height as usize);
    ensure!(img.pixels.len() == w * h, "pixel buffer size mismatch");
    if let Some(p) = img.pixels.iter().find(|&&p| p > 15) {
        bail!("pixel index {p} does not fit a 16-colour palette");
    }
    let plte: Vec<u8> = (0..16)
        .flat_map(|i| palette.rgba(i)[..3].to_vec())
        .collect();
    let stride = w.div_ceil(2);
    let mut data = vec![0u8; stride * h];
    for y in 0..h {
        for x in 0..w {
            let shift = if x % 2 == 0 { 4 } else { 0 };
            data[y * stride + x / 2] |= img.pixels[y * w + x] << shift;
        }
    }
    write_png(path, img.width, img.height, &data, |enc| {
        enc.set_color(png::ColorType::Indexed);
        enc.set_depth(png::BitDepth::Four);
        enc.set_palette(plte);
    })
}

/// Writes an 8-bit RGBA PNG.
pub fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    ensure!(
        rgba.len() == width as usize * height as usize * 4,
        "rgba buffer is {} bytes, expected {width}x{height}x4",
        rgba.len()
    );
    write_png(path, width, height, rgba, |enc| {
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
    })
}

/// Encodes one image to `path`, finishing the PNG stream and flushing the file so that every
/// write error is returned rather than lost on drop.
fn write_png(
    path: &Path,
    width: u32,
    height: u32,
    data: &[u8],
    setup: impl FnOnce(&mut png::Encoder<&mut BufWriter<File>>),
) -> Result<()> {
    let ctx = || format!("writing {}", path.display());
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut out = BufWriter::new(file);
    let mut enc = png::Encoder::new(&mut out, width, height);
    setup(&mut enc);
    let mut writer = enc.write_header().with_context(ctx)?;
    writer.write_image_data(data).with_context(ctx)?;
    writer.finish().with_context(ctx)?;
    out.flush().with_context(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("extract-gfx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn tile_low_nibble_is_left_pixel() {
        let mut tile = [0u8; 32];
        tile[0] = 0x21; // pixels (0,0)=1, (1,0)=2
        tile[31] = 0xF0; // pixel (7,7)=15
        let img = tiles_4bpp_to_image(&tile, 1).unwrap();
        assert_eq!((img.width, img.height), (8, 8));
        assert_eq!(&img.pixels[..3], &[1, 2, 0]);
        assert_eq!(img.pixels[63], 15);
    }

    #[test]
    fn tiles_4bpp_round_trip() {
        // 5 tiles at 2 per row => 16x24 image, the last tile slot padded with zeros.
        let data: Vec<u8> = (0..5 * 32).map(|i| (i * 37 % 251) as u8).collect();
        let img = tiles_4bpp_to_image(&data, 2).unwrap();
        assert_eq!((img.width, img.height), (16, 24));
        let back = image_to_tiles_4bpp(&img).unwrap();
        assert_eq!(&back[..data.len()], &data[..]);
        assert!(back[data.len()..].iter().all(|&b| b == 0));
        assert_eq!(back.len(), 6 * 32);
        // second tile sits right of the first
        assert_eq!(img.pixels[8], data[32] & 0xF);
    }

    #[test]
    fn tiles_reject_bad_input() {
        assert!(tiles_4bpp_to_image(&[0; 31], 1).is_err());
        assert!(tiles_4bpp_to_image(&[0; 32], 0).is_err());
        let img = IndexedImage {
            width: 8,
            height: 4,
            pixels: vec![0; 32],
        };
        assert!(image_to_tiles_4bpp(&img).is_err());
        let img = IndexedImage {
            width: 8,
            height: 8,
            pixels: vec![16; 64],
        };
        assert!(image_to_tiles_4bpp(&img).is_err());
    }

    #[test]
    fn jasc_parse() {
        let text = "JASC-PAL\r\n0100\r\n3\r\n0 0 0\r\n255 255 255\r\n8 16 248\r\n";
        let pal = parse_jasc(text).unwrap();
        assert_eq!(pal.0[0], 0);
        assert_eq!(pal.0[1], 0x7FFF);
        assert_eq!(pal.0[2], 1 | (2 << 5) | (31 << 10));
        assert_eq!(pal.0[3], 0);
        assert!(parse_jasc("RIFF\n0100\n1\n0 0 0\n").is_err());
        assert!(parse_jasc("JASC-PAL\n0100\n2\n0 0 0\n").is_err());
        assert!(parse_jasc("JASC-PAL\n0100\n1\n0 0 256\n").is_err());
    }

    #[test]
    fn palette_bytes_and_rgba() {
        let mut b = [0u8; 32];
        b[2..4].copy_from_slice(&0x7FFFu16.to_le_bytes());
        b[4..6].copy_from_slice(&0x001Fu16.to_le_bytes());
        let pal = Palette::from_bytes(&b).unwrap();
        assert_eq!(pal.rgba(1), [255, 255, 255, 255]);
        assert_eq!(pal.rgba(2), [255, 0, 0, 255]);
        assert_eq!(pal.rgba(0), [0, 0, 0, 255]);
        assert!(Palette::from_bytes(&b[..31]).is_err());
    }

    #[test]
    fn png_indexed_round_trip() {
        let img = IndexedImage {
            width: 3,
            height: 2,
            pixels: vec![0, 1, 15, 7, 8, 2],
        };
        let mut pal = Palette([0; 16]);
        pal.0[1] = 0x7FFF;
        let path = tmp("rt.png");
        write_png_indexed(&path, &img, &pal).unwrap();
        assert_eq!(read_png_indexed(&path).unwrap(), img);
        let bad = IndexedImage {
            pixels: vec![16; 6],
            ..img
        };
        assert!(write_png_indexed(&path, &bad, &pal).is_err());
    }

    #[test]
    fn png_rgba_writes() {
        let path = tmp("rgba.png");
        write_png_rgba(&path, 2, 1, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
        assert!(write_png_rgba(&path, 2, 2, &[0; 8]).is_err());
    }
}
