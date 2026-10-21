# Milestone 2 — Extract v0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** A host-side `extract` crate and `cargo xtask extract` that read the user's French ROM (BPEF) and write editable, git-ignored files under `content/extracted/`: map data for Littleroot, Route 101 and Oldale; their tilesets; player and NPC overworld sprites; the Latin font; their French text; a manifest of every ROM offset used, with checksums; and PNG previews of the three maps.

**Architecture:** Nothing is hard-coded per ROM version. Each table is found with an *anchor*: a short byte signature taken from the US decomp (`../pokeemerald`) that appears unchanged in every Western ROM. From the anchor we follow GBA pointers back to the structure that owns it (e.g. map.bin → `MapLayout` → `MapHeader` → `gMapGroups`). Struct layouts come from the decomp headers.

Every module is tested against **two ROMs**:
- The US ROM (`../emerald.gba`, BPEE) has decomp ground truth. Our output must equal the decomp's files.
- The French ROM is the real input. It must give structurally identical results, with French text.

**Tech Stack:** Rust (std, host only), anyhow, serde + ron, png 0.17, sha1_smol. Root workspace member `crates/extract`.

**Ground truth already verified (2026-10-21):**

| | US (BPEE) | FR (BPEF) |
|---|---|---|
| sha1 | f3ae0881…c8de7 (decomp `rom.sha1`) | ca666651374d89ca439007bed54d839eb7bd14d0 |
| Header 0xA0 | `POKEMON EMERBPEE` | `POKEMON EMERBPEF` |
| Littleroot map.bin (raw u16 blocks) | 0x3e9f64 | 0x3eee50 |
| Littleroot `MapLayout` (20×20) | 0x3ea284 | 0x3ef170 |
| Littleroot `MapHeader` (music 405, layoutId 10) | 0x4825b4 | 0x4874a0 |
| Primary tileset `general` struct | 0x3df704 | 0x3e45f0 |
| Secondary tileset (Littleroot's, `petalburg`) struct | 0x3df71c | 0x3e4608 |
| `general/metatiles.bin` | 0x3960f0 | 0x39afdc |

Note that a second pointer to each layout comes from the `gMapLayouts` table. It is told apart by the MapHeader validation below.

Map IDs (from `data/maps/map_groups.json`): Littleroot = group 0 / num 9, Oldale = 0/10, Route 101 = 0/16. There are 34 groups.

**Decomp reference paths** (`D = ../pokeemerald`):
- Structs: `D/include/global.fieldmap.h` (Tileset, MapLayout, MapHeader, MapEvents, ObjectEventTemplate, WarpEvent, CoordEvent, BgEvent, MapConnection(s), ObjectEventGraphicsInfo).
- Constants: `D/include/fieldmap.h`.
- Map data: `D/data/maps/map_groups.json`, `D/data/maps/<Map>/map.json`, `D/data/maps/<Map>/scripts.inc`, `D/data/layouts/<Map>/{map,border}.bin`.
- Tilesets: `D/data/tilesets/{primary,secondary}/<name>/{metatiles.bin,metatile_attributes.bin,tiles.png,palettes/*.pal}`.
- Text: `D/charmap.txt`, the script commands in `D/data/script_cmd_table.inc` and `D/asm/macros/event.inc`.
- Graphics: `D/graphics/fonts/latin_normal.png` (format: `D/tools/gbagfx/font.c`), `D/graphics/object_events/pics/people/brendan/*.png`, `D/graphics/object_events/palettes/*.pal`, `D/src/data/object_events/*.h`.

**Rules for every task:**
- Use TDD. Unit tests go in the module. ROM-backed tests go in `crates/extract/tests/<module>.rs` and use `extract::fixtures::{fr, us, decomp}`, which panic with setup instructions if a file is missing.
- Each ROM-backed test that has a decomp oracle runs on **both** US and FR.
- Return `anyhow::Result` and never panic on ROM data. A bad pointer is an `Err` with the offset in its message.
- Only touch your own module files, your own `tests/` file, and anything else your task lists.
- Before committing: run `cargo fmt -p extract`; `cargo clippy -p extract --all-targets -- -D warnings` must be clean.
- Commit messages are `feat(extract): …` with no attribution lines.
- Never commit ROM-derived output. `content/extracted/` and `rom/` are git-ignored.

---

## Wave 0 — Scaffold (controller)

### Task 0: `extract` crate with `Rom` core and fixtures

**Files:** `crates/extract/{Cargo.toml,src/lib.rs,src/rom.rs,src/fixtures.rs}`, empty module files for Wave 1/2/3, root `Cargo.toml` members, `.gitignore` (`content/extracted/`).

`src/lib.rs` declares the modules `rom, fixtures, lz77, gfx, text, maps, sprites, font, scripts, render, export`.

`Rom` API (the contract for everyone):
```rust
pub const ROM_BASE: u32 = 0x0800_0000;
pub enum GameCode { Bpee, Bpef }
pub struct Rom { /* bytes, code */ }
impl Rom {
    pub fn open(path: &Path) -> Result<Rom>;          // checks 16 MiB max, header code at 0xAC
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Rom>;
    pub fn code(&self) -> GameCode;
    pub fn bytes(&self) -> &[u8];
    pub fn sha1_hex(&self) -> String;
    pub fn slice(&self, off: u32, len: usize) -> Result<&[u8]>;
    pub fn u8(&self, off: u32) -> Result<u8>;
    pub fn u16(&self, off: u32) -> Result<u16>;      // little-endian
    pub fn u32(&self, off: u32) -> Result<u32>;
    pub fn i16(&self, off: u32) -> Result<i16>;
    pub fn i32(&self, off: u32) -> Result<i32>;
    /// u32 at `off` read as a GBA ROM pointer: 0 => None, ROM_BASE..ROM_BASE+len => Some(file offset), else Err.
    pub fn ptr(&self, off: u32) -> Result<Option<u32>>;
    pub fn find_all(&self, needle: &[u8]) -> Vec<u32>;
    /// 4-byte-aligned offsets whose u32 equals ROM_BASE + target.
    pub fn pointers_to(&self, target: u32) -> Vec<u32>;
}
```
`fixtures`: `fr() -> Rom` (`<root>/rom/emeraude-fr.gba`), `us() -> Rom` (`$PKR_ROM_US` or `<root>/../emerald.gba`), `decomp() -> PathBuf` (`$PKR_DECOMP` or `<root>/../pokeemerald`).

---

## Wave 1 — independent readers (5 parallel agents)

### Task A: `lz77` + `gfx`

**Files:** `src/lz77.rs`, `src/gfx.rs`, `tests/gfx.rs`

```rust
// lz77.rs
pub fn decompress(src: &[u8]) -> Result<Vec<u8>>;           // GBA BIOS LZ77 (type 0x10)
impl Rom { pub fn lz77_at(&self, off: u32) -> Result<Vec<u8>>; }
// gfx.rs
pub type Rgb555 = u16;
pub struct Palette(pub [Rgb555; 16]);
impl Palette { pub fn from_bytes(b: &[u8]) -> Result<Palette>; pub fn rgba(&self, i: u8) -> [u8; 4]; }
pub fn read_jasc_pal(path: &Path) -> Result<Palette>;        // decomp .pal (JASC-PAL, 8-bit RGB → 5-bit)
pub struct IndexedImage { pub width: u32, pub height: u32, pub pixels: Vec<u8> } // one u8 index per pixel
/// Lay out 4bpp tiles (32 bytes each) left-to-right, `tiles_per_row` per row.
pub fn tiles_4bpp_to_image(data: &[u8], tiles_per_row: u32) -> Result<IndexedImage>;
pub fn image_to_tiles_4bpp(img: &IndexedImage) -> Result<Vec<u8>>;
pub fn read_png_indexed(path: &Path) -> Result<IndexedImage>; // decomp 4bpp/8bpp indexed PNGs; also returns nothing about palette
pub fn write_png_indexed(path: &Path, img: &IndexedImage, palette: &Palette) -> Result<()>;
pub fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()>;
```

Acceptance:
- **Unit:**
  - lz77 handles a literal-only stream, an overlapping back-reference, and a truncated stream (which must return `Err`).
  - 4bpp ↔ image round-trips.
  - A JASC parse of a tiny inline sample works.
- **ROM, US and FR:**
  - Read the `general` tileset struct (offsets in the table above): tiles ptr at +4, compressed flag at +0.
  - `lz77_at` the tiles, then `tiles_4bpp_to_image(…, 16)`.
  - Pixel indices must equal `D/data/tilesets/primary/general/tiles.png`. Compare over the min of both heights and document any trailing difference.
  - Palette 0 (16 u16 at the palettes ptr, +8) must equal `palettes/00.pal`.

### Task B: `text` (Gen 3 Western charset)

**Files:** `src/text.rs` (and `src/charmap.rs` if large), `tests/text.rs`

```rust
pub fn decode(bytes: &[u8]) -> Result<(String, usize)>;  // up to and including 0xFF; returns consumed len
pub fn encode(s: &str) -> Result<Vec<u8>>;               // inverse, appends 0xFF
impl Rom { pub fn string_at(&self, off: u32) -> Result<String>; }
```

Output syntax must match the decomp's `.string` syntax exactly:
- `\n`, `\l` and `\p` as two-character escapes.
- Placeholders and control codes in braces, e.g. `{PLAYER}`, `{STR_VAR_1}`, `{COLOR RED}`, `{PAUSE 15}`, using the names in `D/charmap.txt`.
- Encode the charmap as a Rust table. Do not read `charmap.txt` at runtime.
- Where several names map to the same byte, pick the one the decomp uses most often in `data/maps/*/scripts.inc`.

Acceptance:
- **Unit:** `encode`/`decode` round-trip on ASCII, accented French (`é è ê à ç ô É`), `…`, `♂ ♀`, and every escape and control-code form.
- **US:**
  - Parse every `.string "…"` in `D/data/maps/LittlerootTown/scripts.inc`. Join multi-line strings as the assembler does.
  - Encode each one. Each must be found in the US ROM (`find_all`), and `string_at` there must give back the identical string.
- **FR:**
  - `encode("BOURG-EN-VOL")` is found in the FR ROM.
  - Decoding at that hit round-trips.

### Task C: `maps` (headers, layouts, events, connections, tileset structs)

**Files:** `src/maps.rs`, `tests/maps.rs`

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct MapId { pub group: u8, pub num: u8 }
pub const LITTLEROOT: MapId; pub const OLDALE: MapId; pub const ROUTE101: MapId;
pub struct MapTable { pub offset: u32, pub groups: Vec<Vec<u32>> }   // header file offsets
pub fn locate_map_table(rom: &Rom) -> Result<MapTable>;
pub struct MapHeader { pub offset: u32, pub layout: MapLayout, pub events: MapEvents,
    pub map_scripts: Option<u32>, pub connections: Vec<Connection>, pub music: u16, pub layout_id: u16,
    pub region_map_section: u8, pub cave: u8, pub weather: u8, pub map_type: u8,
    pub allow_cycling: bool, pub allow_escaping: bool, pub allow_running: bool, pub show_map_name: bool,
    pub battle_type: u8 }
pub struct MapLayout { pub offset: u32, pub width: u16, pub height: u16, pub border: Vec<u16>, /* 2x2 */
    pub blocks: Vec<u16>, pub primary_tileset: u32, pub secondary_tileset: u32 }
pub struct TilesetRef { pub offset: u32, pub compressed: bool, pub secondary: bool, pub tiles: u32,
    pub palettes: u32, pub metatiles: u32, pub metatile_attributes: u32, pub metatile_count: u16 }
pub struct ObjectEvent { pub local_id: u8, pub graphics_id: u8, pub x: i16, pub y: i16, pub elevation: u8,
    pub movement_type: u8, pub movement_range_x: u8, pub movement_range_y: u8, pub trainer_type: u16,
    pub trainer_range_berry_tree_id: u16, pub script: Option<u32>, pub flag_id: u16 }
pub struct Warp { pub x: i16, pub y: i16, pub elevation: u8, pub warp_id: u8, pub dest: MapId }
pub struct CoordEvent { pub x: i16, pub y: i16, pub elevation: u8, pub trigger_var: u16, pub index: u16, pub script: Option<u32> }
pub enum BgEventKind { Script(Option<u32>), HiddenItem { item: u16, hidden_item_id: u16 }, SecretBase(u32) }
pub struct BgEvent { pub x: u16, pub y: u16, pub elevation: u8, pub kind_raw: u8, pub kind: BgEventKind }
pub struct MapEvents { pub objects: Vec<ObjectEvent>, pub warps: Vec<Warp>, pub coords: Vec<CoordEvent>, pub bgs: Vec<BgEvent> }
pub enum Direction { South, North, West, East, Dive, Emerge } // decomp CONNECTION_* order
pub struct Connection { pub direction: Direction, pub offset: i32, pub map: MapId }
impl MapTable { pub fn header(&self, rom: &Rom, id: MapId) -> Result<MapHeader>; }
pub fn read_tileset(rom: &Rom, off: u32, all: &MapTable) -> Result<TilesetRef>;
```

Locating the map table:
- Embed a 64-byte anchor taken from the start of Littleroot's `map.bin` and `find_all` it. The hit must be unique; then find the layout (pointer to it at layout+0x0C).
- `pointers_to(layout)` gives candidate headers. Validate each one: events ptr valid, layout_id == 10.
- `pointers_to(header)` gives the group-0 table entry at index 9, and `pointers_to(group0_table)` gives `gMapGroups`.
- Group sizes are a const `[u8; 34]` taken from `map_groups.json`.

Metatile count for secondary tilesets:
- Primary is always 512.
- For secondary, collect all tilesets referenced by all layouts and sort their `metatiles` pointers. The count is (next pointer − this pointer) / 16, capped at 512.
- If the decomp oracle disagrees, research how the decomp lays out `data/tilesets/metatiles.inc` and fix it.

Acceptance (US and FR):
- `locate_map_table` succeeds. The header of `LITTLEROOT` is at the offset in the ground-truth table.
- For each of the 3 maps:
  - Width, height, blocks and border equal `D/data/layouts/<Map>/{map,border}.bin`.
  - Event counts equal `map.json` (`object_events`, `warp_events`, `coord_events`, `bg_events`).
  - The x/y of each object event equals `map.json`.
  - Each warp destination equals `map.json`'s `dest_map`, resolved to a group/num through `map_groups.json` in the test.
  - Connection directions and maps equal `map.json`.
- Every map in the table parses without error (iterate all groups).
- For every tileset reachable from all layouts, `metatile_count * 16` equals the size of the decomp `metatiles.bin`. Match tilesets to decomp folders by comparing the metatiles bytes.
- **Interface for later waves:** `Rom::slice(tileset.metatiles, count*16)` and the attributes (`count*2` bytes) are raw u16 data.

### Task E: `sprites` (object event graphics)

**Files:** `src/sprites.rs`, `tests/sprites.rs`

```rust
pub struct ObjectGraphics { pub id: u8, pub offset: u32, pub width: u16, pub height: u16, pub palette_tag: u16,
    pub frames: Vec<Vec<u8>> /* raw 4bpp per frame, size = w*h/2 */, pub palette: [u16; 16] }
pub struct ObjectGraphicsTable { pub offset: u32, pub count: u16, pub palettes_offset: u32 }
pub fn locate_object_graphics(rom: &Rom) -> Result<ObjectGraphicsTable>;
impl ObjectGraphicsTable { pub fn get(&self, rom: &Rom, id: u8) -> Result<ObjectGraphics>; }
```

Steps:
1. Research `struct ObjectEventGraphicsInfo` and `SpriteFrameImage` (`{data ptr, u16 size}` in `include/`), the info and pic tables in `D/src/data/object_events/` (`object_event_graphics_info_pointers.h`, `object_event_pic_tables.h`, `object_event_graphics.h`), and the sprite-palette table (`{ptr, tag, pad}`, terminated) in `D/src/event_object_movement.c`.
2. Anchor: convert `D/graphics/object_events/pics/people/brendan/walking.png` to 4bpp with a test helper, using the `png` crate and the GBA sprite tile order (8×8 tiles, row-major within each 16×32 frame). Take its first 128 bytes and `find_all`.
3. From the anchor, back-trace pic table → graphics info → pointer table, then validate.

Acceptance (US and FR):
- `get(0)` (`OBJ_EVENT_GFX_BRENDAN_NORMAL`) is 16×32 and has the decomp's frame count.
- The concatenated frames equal the converted `walking.png`.
- The palette equals `brendan.pal`.
- Every id from 0 to `count`−1 decodes without error.
- `count` equals the number of entries in `object_event_graphics_info_pointers.h`.

### Task F: `font` (Latin normal font)

**Files:** `src/font.rs`, `tests/font.rs`

```rust
pub struct Font { pub glyph_width_max: u8, pub glyph_height: u8, pub glyphs: Vec<Vec<u8>> /* per glyph: row-major 2-bit or index pixels */, pub widths: Vec<u8> }
pub fn locate_latin_normal(rom: &Rom) -> Result<(u32, u32)>;   // (glyphs offset, widths offset)
pub fn read_latin_normal(rom: &Rom) -> Result<Font>;
```

Steps:
1. Research the `.latfont` encoding in `D/tools/gbagfx/font.c` and how `D/src/fonts.c` declares `gFontNormalLatinGlyphs` and `gFontNormalLatinGlyphWidths`.
2. Convert `D/graphics/fonts/latin_normal.png` to the ROM encoding in a test helper and anchor on its first 64 bytes.
3. Widths: find the widths table by searching near the glyphs. Validate it: 256 or more entries, each 0 to 16.

Acceptance (US and FR):
- The glyph pixels equal `latin_normal.png`.
- The widths for `A`, `a`, space and `é` (byte values via `text::encode` are not available in this wave, so hard-code them from `charmap.txt`) are plausible: non-zero, at most 16.
- FR glyph data equals US glyph data, or document any difference.

---

## Wave 2 — consumers (2 parallel agents)

### Task D: `scripts` (event script walk → text references)

**Files:** `src/scripts.rs`, `tests/scripts.rs`

```rust
pub fn map_script_roots(rom: &Rom, header: &MapHeader) -> Result<Vec<u32>>; // object/coord/bg event scripts + map script tables
pub struct ScriptScan { pub visited: BTreeSet<u32>, pub texts: BTreeSet<u32>, pub unknown: BTreeMap<u32, u8> }
pub fn walk(rom: &Rom, roots: &[u32]) -> Result<ScriptScan>;
```

Steps:
1. Build a const table of operand layouts for all opcodes in `D/data/script_cmd_table.inc`, using the macros in `D/asm/macros/event.inc`.
2. Follow control flow: goto, call, goto_if and call_if; `end`/`return` stop a branch.
3. Text pointers come from `loadword 0, ptr` (msgbox) and from `message`. Extend this to any other command whose operand is a text pointer.
4. Movement-data pointers (applymovement) are recorded but not walked as scripts.
5. Map script tables (type byte + ptr, terminated by 0): types 2 and 4 point to `(var u16, value u16, ptr)` lists terminated by a 0 u16.

Acceptance:
- **US, Littleroot:** every `<Label>` with a `.string` in `D/data/maps/LittlerootTown/scripts.inc` that is referenced by a `msgbox`/`message` in that file is reached. Its decoded text equals the decomp.
- **FR, Littleroot:** the walk reaches the same number of texts as in US. All decode without error, and contain at least one accented character overall.
- **All 3 maps, both ROMs:** `unknown` is empty.

### Task G: `render` (map preview PNGs)

**Files:** `src/render.rs`, `tests/render.rs`

```rust
pub struct TilesetData { pub tiles: Vec<u8> /* 4bpp */, pub palettes: Vec<[u16; 16]>, pub metatiles: Vec<[u16; 8]>, pub attributes: Vec<u16> }
pub fn load_tileset(rom: &Rom, t: &TilesetRef) -> Result<TilesetData>;
/// RGBA, (width*16) x (height*16): metatile = 2x2 tiles bottom layer then 2x2 top layer;
/// tile entry bits: 0-9 index (>=512 => secondary[idx-512]), 10 hflip, 11 vflip, 12-15 palette
/// (0-5 primary, 6-12 secondary); color 0 transparent over backdrop = primary palette 0 color 0.
pub fn render_map(layout: &MapLayout, primary: &TilesetData, secondary: &TilesetData) -> Result<Vec<u8>>;
```

Acceptance:
- **Unit:** flip and palette-bank handling on a synthetic 1-metatile map.
- **ROM:** for each of the 3 maps, the FR render equals the US render, except for pixels drawn from the FR-localized tiles. The `general` tileset has 10 translated sign tiles in FR: indices 222, 223, 238, 239, 327–330, 343 and 358 (the ARN gym and POKé SHOP signs). Pin the exact set of differing 8×8 tile *cells* per map: Littleroot and Route 101 must be identical; Oldale (which has a mart) may differ only in cells that use those tile indices. French graphics always come from the ROM, never from the decomp PNGs.
- **Write previews:** write the FR previews to `target/previews/<Map>.png` so the controller can inspect them.
- **Sanity check:** the Littleroot preview is 320×320.

---

## Wave 3 — export and CLI (1 agent)

### Task H: `export` + `cargo xtask extract`

**Files:** `src/export.rs`, `tests/export.rs`, `xtask/src/main.rs`, `xtask/Cargo.toml`, `README.md`

`pub fn export(rom: &Rom, out: &Path) -> Result<Manifest>` writes:

```
content/extracted/
  manifest.ron            game code, sha1, every located offset (map table, object gfx table, font, tilesets…)
  maps/<Map>/map.ron      header fields, layout (w,h,border,blocks), events, connections; scripts by offset,
                          text refs as keys "<Map>.<hex offset>"
  tilesets/<hex>/tiles.png (indexed, 16 tiles/row, palette 0), palettes.ron, metatiles.ron, attributes.ron
  sprites/<id>.png        frames side by side, own palette; sprites/<id>.ron (w,h,frames,palette_tag)
  font/latin_normal.png + latin_normal.ron (widths)
  text/fr.ron             key -> string, for every text reached by the 3 maps' scripts
  previews/<Map>.png
```
- Map names: a small const table `MapId → "LittlerootTown" | "OldaleTown" | "Route101"`.
- Sprites: export the ids used by the 3 maps' object events, plus Brendan (0) and May (`OBJ_EVENT_GFX_MAY_NORMAL`).

`cargo xtask extract [--rom <path>]` defaults to `rom/emeraude-fr.gba` and prints a summary.

Acceptance:
- **Determinism:** two runs give byte-identical trees. Test: export into two temp dirs and compare.
- **Round-trip:** `map.ron` round-trips through `ron` into the same struct.
- **README:** a section "Extract" describing the steps: place the ROM, run the command, see the output.
- **Gitignore:** `content/extracted/` is git-ignored. `git status` is clean after extraction.

---

## Done when
- `cargo test -p extract` passes, with all ROM-backed tests running on both US and FR.
- `cargo xtask extract` produces `content/extracted/` with the previews visibly correct (checked by eye), and `text/fr.ron` contains French dialogue for the 3 maps.
- No ROM-derived bytes are tracked by git.
