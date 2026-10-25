//! `$OUT_DIR/ui.rs`: the Milestone 9 UI tables (`content::data::{START_MENU, ITEM_USE, …}`),
//! answered by `game_data_tables!` for every flavour. Task C owns the file: it fills the tables
//! from the extracted `ui/`, `icons/` and `tables/` files (blobs for pictures and icons).
//!
//! Mon icons, item icons and the bag sprite are loaded when `icons/` and `ui/BagSprite*.ron`
//! are in the extract. Without them the tables stay empty and the screens draw frames and text.

use crate::input::World;
use extract::names;

/// `TextId`s from here on name `START_LABELS` entries (`content::data::text`).
pub const START_LABEL_BASE: u16 = 0xFF00;

/// The Rust source of `ui.rs`, `include!`d into `content::data`. `gen3_menu`: the flavour
/// has Gen3's start menu (Emerald's; Silice's too until Task C writes its own from
/// `game.ron` with the Mallette: without one a Silice player never opens the party or bag).
pub fn generate(world: &World, gen3_menu: bool) -> (String, Vec<u8>) {
    let (tm_moves, tm_compat) = tmhm(world);
    let (start_menu, start_labels) = if gen3_menu {
        start_menu(world)
    } else {
        (String::new(), String::new())
    };
    let item_use = item_uses(world);
    let mut s = format!(
        "\
/// `TextId(START_LABEL_BASE + i)` is `START_LABELS[i]` (the start menu's labels).
pub const START_LABEL_BASE: u16 = {START_LABEL_BASE:#X};
/// The start menu's labels (Gen3 `sStartMenuItems[].text`, from `MULTI_FORCED_START_MENU`).
pub static START_LABELS: &[&[u8]] = &[{start_labels}];
/// The START menu (Gen3 `BuildNormalStartMenu`; Emerald's, and Silice's until Task C).
pub static START_MENU: &[::engine::data::StartItem] = &[{start_menu}];
/// `ItemUse` by item id.
pub static ITEM_USE: &[::engine::data::ItemUse] = &[{item_use}];
"
    );
    let (pictures, bin) = menu_pictures(world);
    s.push_str(
        "\
/// Item descriptions by item id (`gItems[].description`).
pub static ITEM_DESCRIPTIONS: &[Option<&[u8]>] = &[];
/// Move descriptions by move id (`gMoveDescriptionPointers`).
pub static MOVE_DESCRIPTIONS: &[Option<&[u8]>] = &[];
",
    );
    s.push_str(&pictures);
    s.push_str(
        "\
/// The user window frames (`gUserFrames`).
pub static WINDOW_FRAMES: &[::engine::data::WindowGfx] = &[];
/// `sIngameTrades`.
pub static IN_GAME_TRADES: &[::engine::data::InGameTrade] = &[];
/// Nature names by nature (`gNatureNamePointers`).
pub static NATURE_NAMES: &[&[u8]] = &[];
",
    );
    s.push_str(&format!(
        "/// `(TM/HM item, move)`, sorted by item id (`sTMHMMoves`).\n\
         pub static TM_MOVES: &[(u16, u16)] = &[{tm_moves}];\n\
         /// `(species, bitset over TM_MOVES)`, sorted by species id (`gTMHMLearnsets`).\n\
         pub static TM_COMPAT: &[(u16, u64)] = &[{tm_compat}];\n"
    ));
    (s, bin)
}

/// `TM_MOVES` and `TM_COMPAT`'s entries from `tables/tmhm.ron` (decomp names to ids); a name
/// without an id drops its entry (and, for an item, every later bit: the bitsets index
/// `TM_MOVES`).
fn tmhm(world: &World) -> (String, String) {
    use extract::names;
    let t = &world.tables.tmhm;
    let mut moves = Vec::new();
    for (item, mv) in &t.moves {
        match (names::ITEMS.value(item), names::MOVES.value(mv)) {
            (Some(i), Some(m)) => moves.push((i, m)),
            _ => break,
        }
    }
    let sorted = moves.windows(2).all(|w| w[0].0 < w[1].0);
    if !sorted || moves.len() > 64 {
        moves.clear();
    }
    let mut compat: Vec<(u32, u64)> = t
        .learnsets
        .iter()
        .filter_map(|(s, &bits)| Some((names::SPECIES.value(s)?, bits)))
        .collect();
    compat.sort_unstable();
    if moves.is_empty() {
        compat.clear();
    }
    let mask = if moves.len() == 64 {
        u64::MAX
    } else {
        (1u64 << moves.len()) - 1
    };
    (
        moves
            .iter()
            .map(|(i, m)| format!("({i}, {m})"))
            .collect::<Vec<_>>()
            .join(", "),
        compat
            .iter()
            .map(|(s, b)| format!("({s}, {:#x})", b & mask))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Emerald's start menu (Gen3 `BuildNormalStartMenu`): POKéDEX (`FLAG_SYS_POKEDEX_GET`),
/// POKéMON (`FLAG_SYS_POKEMON_GET`), BAG, the player, SAVE, OPTION, EXIT. The PokéNav entry is
/// left out (no engine action yet). The labels are `MULTI_FORCED_START_MENU`'s (the player's
/// is empty there: Gen3 prints the player's name). Without that list: no start menu.
fn start_menu(world: &World) -> (String, String) {
    let Some(list) = world
        .tables
        .multichoice
        .lists
        .get("MULTI_FORCED_START_MENU")
    else {
        return (String::new(), String::new());
    };
    let flag = |n: &str| {
        names::FLAGS.value(n).map_or("None".to_string(), |v| {
            format!("Some(::engine::data::FlagId({v:#x}))")
        })
    };
    // (label index in the list, action, shown_if)
    let rows: [(usize, &str, String); 7] = [
        (0, "Pokedex", flag("FLAG_SYS_POKEDEX_GET")),
        (1, "Party", flag("FLAG_SYS_POKEMON_GET")),
        (2, "Bag", "None".into()),
        (4, "PlayerCard", "None".into()),
        (5, "Save", "None".into()),
        (6, "Options", "None".into()),
        (7, "Exit", "None".into()),
    ];
    let mut labels = Vec::new();
    let mut items = Vec::new();
    for (k, (i, action, shown_if)) in rows.iter().enumerate() {
        let Some(text) = list.get(*i) else {
            return (String::new(), String::new());
        };
        let bytes = extract::text::encode_lang(text, world.lang)
            .unwrap_or_else(|e| panic!("start menu label {text:?}: {e}"));
        labels.push(format!("&{bytes:?}"));
        items.push(format!(
            "::engine::data::StartItem {{ label: ::engine::data::TextId({}), action: \
             ::engine::data::StartAction::{action}, shown_if: {shown_if}, description: None }}",
            START_LABEL_BASE as usize + k
        ));
    }
    (items.join(", "), labels.join(", "))
}

/// `ItemUse` per item id, from the decomp's item effects (`gItemEffect_*`, `src/data/items.h`
/// field and battle functions) by `ITEM_*` name; an item named nowhere here is `None` (no field
/// use: `ItemUseOutOfBattle_CannotUse`). The cure bits are `engine::item_use::CURE_*`, the stat
/// indices `engine::mon::Stat`'s.
fn item_uses(world: &World) -> String {
    let tms: std::collections::BTreeMap<&str, u32> = (world.tables.tmhm.moves.iter())
        .filter_map(|(i, m)| Some((i.as_str(), names::MOVES.value(m)?)))
        .collect();
    (world.tables.items.items.iter())
        .map(|it| item_use(&it.id, &tms))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `engine::item_use::CURE_*`.
const SLEEP: u8 = 0x01;
const POISON: u8 = 0x02;
const BURN: u8 = 0x04;
const FREEZE: u8 = 0x08;
const PARALYSIS: u8 = 0x10;
const CONFUSION: u8 = 0x20;
const INFATUATION: u8 = 0x40;
const ALL: u8 = SLEEP | POISON | BURN | FREEZE | PARALYSIS | CONFUSION;

fn item_use(id: &str, tms: &std::collections::BTreeMap<&str, u32>) -> String {
    const P: &str = "::engine::data::";
    let med = |m: &str| format!("{P}ItemUse::Medicine({P}MedicineEffect::{m})");
    let heal = |n: u16| med(&format!("Heal({n})"));
    let cure = |b: u8| med(&format!("Cure({b})"));
    let key = |k: &str| format!("{P}ItemUse::Key({P}KeyUse::{k})");
    let other = |n: &str| format!("{P}ItemUse::Other({n:?})");
    let plain = |v: &str| format!("{P}ItemUse::{v}");
    if let Some(m) = tms.get(id) {
        return format!("{P}ItemUse::Tm({P}MoveId({m}))");
    }
    match id {
        "ITEM_POTION" | "ITEM_BERRY_JUICE" => heal(20),
        "ITEM_SUPER_POTION" | "ITEM_FRESH_WATER" | "ITEM_ENERGY_POWDER" => heal(50),
        "ITEM_SODA_POP" => heal(60),
        "ITEM_LEMONADE" => heal(80),
        "ITEM_MOOMOO_MILK" => heal(100),
        "ITEM_HYPER_POTION" | "ITEM_ENERGY_ROOT" => heal(200),
        "ITEM_MAX_POTION" => heal(u16::MAX),
        "ITEM_ORAN_BERRY" => heal(10),
        "ITEM_SITRUS_BERRY" => heal(30),
        "ITEM_FULL_RESTORE" => med(&format!("HealCure({}, {})", u16::MAX, ALL | INFATUATION)),
        "ITEM_ANTIDOTE" | "ITEM_PECHA_BERRY" => cure(POISON),
        "ITEM_BURN_HEAL" | "ITEM_RAWST_BERRY" => cure(BURN),
        "ITEM_ICE_HEAL" | "ITEM_ASPEAR_BERRY" => cure(FREEZE),
        "ITEM_AWAKENING" | "ITEM_CHESTO_BERRY" | "ITEM_BLUE_FLUTE" => cure(SLEEP),
        "ITEM_PARALYZE_HEAL" | "ITEM_CHERI_BERRY" => cure(PARALYSIS),
        "ITEM_PERSIM_BERRY" | "ITEM_YELLOW_FLUTE" => cure(CONFUSION),
        "ITEM_RED_FLUTE" => cure(INFATUATION),
        "ITEM_FULL_HEAL" | "ITEM_HEAL_POWDER" | "ITEM_LAVA_COOKIE" | "ITEM_LUM_BERRY" => cure(ALL),
        "ITEM_REVIVE" => med("Revive(128)"),
        "ITEM_MAX_REVIVE" | "ITEM_REVIVAL_HERB" => med("Revive(256)"),
        "ITEM_ETHER" | "ITEM_LEPPA_BERRY" => med("Pp(10, false)"),
        "ITEM_MAX_ETHER" => med("Pp(255, false)"),
        "ITEM_ELIXIR" => med("Pp(10, true)"),
        "ITEM_MAX_ELIXIR" => med("Pp(255, true)"),
        "ITEM_RARE_CANDY" => med("LevelUp"),
        "ITEM_PP_UP" => med("PpUp(false)"),
        "ITEM_PP_MAX" => med("PpUp(true)"),
        "ITEM_HP_UP" => med("Ev(0, 10)"),
        "ITEM_PROTEIN" => med("Ev(1, 10)"),
        "ITEM_IRON" => med("Ev(2, 10)"),
        "ITEM_CARBOS" => med("Ev(3, 10)"),
        "ITEM_CALCIUM" => med("Ev(4, 10)"),
        "ITEM_ZINC" => med("Ev(5, 10)"),
        "ITEM_MASTER_BALL" => plain("Ball(255)"),
        "ITEM_ULTRA_BALL" => plain("Ball(20)"),
        "ITEM_GREAT_BALL" | "ITEM_SAFARI_BALL" => plain("Ball(15)"),
        "ITEM_POKE_BALL" | "ITEM_NET_BALL" | "ITEM_DIVE_BALL" | "ITEM_NEST_BALL"
        | "ITEM_REPEAT_BALL" | "ITEM_TIMER_BALL" | "ITEM_LUXURY_BALL" | "ITEM_PREMIER_BALL" => {
            plain("Ball(10)")
        }
        "ITEM_X_ATTACK" => plain("StatStage(1, 1)"),
        "ITEM_X_DEFEND" => plain("StatStage(2, 1)"),
        "ITEM_X_SPEED" => plain("StatStage(3, 1)"),
        "ITEM_X_SPECIAL" => plain("StatStage(4, 1)"),
        "ITEM_X_ACCURACY" => plain("StatStage(6, 1)"),
        "ITEM_DIRE_HIT" | "ITEM_GUARD_SPEC" | "ITEM_POKE_DOLL" | "ITEM_FLUFFY_TAIL"
        | "ITEM_SACRED_ASH" | "ITEM_WHITE_FLUTE" | "ITEM_BLACK_FLUTE" | "ITEM_ENIGMA_BERRY" => {
            other(id)
        }
        "ITEM_REPEL" => plain("Repel(100)"),
        "ITEM_SUPER_REPEL" => plain("Repel(200)"),
        "ITEM_MAX_REPEL" => plain("Repel(250)"),
        "ITEM_ESCAPE_ROPE" => plain("EscapeRope"),
        "ITEM_FIRE_STONE" | "ITEM_THUNDER_STONE" | "ITEM_WATER_STONE" | "ITEM_LEAF_STONE"
        | "ITEM_MOON_STONE" | "ITEM_SUN_STONE" => plain("Evolution"),
        // Silice's D-30 (`bible/decisions.md`): used like a stone, it evolves a trade evolver
        // (and uses up the item a trade-with-item evolution holds).
        "ITEM_SIL_PIERRE_LIEN" => plain("TradeEvolution"),
        "ITEM_MACH_BIKE" | "ITEM_ACRO_BIKE" => key("Bike"),
        "ITEM_OLD_ROD" => key("Rod(0)"),
        "ITEM_GOOD_ROD" => key("Rod(1)"),
        "ITEM_SUPER_ROD" => key("Rod(2)"),
        "ITEM_COIN_CASE" | "ITEM_POKEBLOCK_CASE" | "ITEM_WAILMER_PAIL" | "ITEM_ITEMFINDER" => {
            key(&format!("Other({id:?})"))
        }
        _ if id.ends_with("_MAIL") => other(id),
        _ if id.ends_with("_BERRY") => plain("Berry"),
        _ => plain("None"),
    }
}

/// Mon icons, item icons and bag-sprite frames from the extract, or empty tables when those
/// files are not there (a stub, or an extract from before this graphics was written).
fn menu_pictures(world: &World) -> (String, Vec<u8>) {
    let empty = "\
/// `(UiGfx tag, picture)`.
pub static UI_GFX: &[(::engine::data::UiGfx, ::engine::data::UiPicture)] = &[];
/// `(item id, icon)`, sorted by item id.
pub static ITEM_ICONS: &[(u16, ::engine::data::Icon)] = &[];
/// `(species id, icon)`, sorted by species id.
pub static MON_ICONS: &[(u16, ::engine::data::Icon)] = &[];
";
    let Some(dir) = &world.dir else {
        return (empty.to_string(), Vec::new());
    };
    let have_icons = dir.join("icons").is_dir();
    let have_bag = dir.join("ui/BagSprite0.ron").is_file();
    if !have_icons && !have_bag {
        return (empty.to_string(), Vec::new());
    }
    let mut blob = crate::blob_data::BlobOut::new("UI");
    let mut gfx = String::new();
    let mut items = String::new();
    let mut mons = String::new();
    if have_bag {
        for i in 0..5u8 {
            let stem = dir.join(format!("ui/BagSprite{i}"));
            let ron = stem.with_extension("ron");
            if !ron.is_file() {
                continue;
            }
            let file: extract::schema::UiPictureFile = extract::export::read_ron(&ron)
                .unwrap_or_else(|e| panic!("reading {}: {e:#}", ron.display()));
            let png = stem.with_extension("png");
            let tiles = png_4bpp(&png);
            let pal = file.palettes.first().copied().unwrap_or([0; 16]);
            let tiles_e = blob.bytes(&tiles);
            let pal_e = blob.u16s(&pal);
            gfx.push_str(&format!(
                "(::engine::data::UiGfx::BagSprite({i}), ::engine::data::UiPicture {{ tiles: {tiles_e}, map: UI.bytes(0, 0), palette: {pal_e} }}),"
            ));
        }
    }
    if have_icons {
        let items_dir = dir.join("icons/items");
        if items_dir.is_dir() {
            let mut rows: Vec<(u16, String)> = Vec::new();
            for ent in std::fs::read_dir(&items_dir).unwrap_or_else(|e| panic!("{e}")) {
                let ent = ent.unwrap_or_else(|e| panic!("{e}"));
                let path = ent.path();
                if path.extension().and_then(|s| s.to_str()) != Some("ron") {
                    continue;
                }
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let Some(id) = extract::names::ITEMS.value(name) else {
                    continue;
                };
                let file: extract::schema::ItemIconFile = extract::export::read_ron(&path)
                    .unwrap_or_else(|e| panic!("reading {}: {e:#}", path.display()));
                let tiles = png_4bpp(&path.with_extension("png"));
                let tiles_e = blob.bytes(&tiles);
                let pal_e = blob.u16s(&file.palette);
                rows.push((
                    id as u16,
                    format!(
                        "({id}, ::engine::data::Icon {{ tiles: {tiles_e}, palette: {pal_e}, frames: 1 }}),"
                    ),
                ));
            }
            rows.sort_by_key(|(id, _)| *id);
            rows.dedup_by_key(|(id, _)| *id);
            for (_, row) in rows {
                items.push_str(&row);
            }
        }
        let pal_path = dir.join("icons/mons/palettes.ron");
        if pal_path.is_file() {
            let file: extract::schema::MonIconPalettesFile = extract::export::read_ron(&pal_path)
                .unwrap_or_else(|e| panic!("reading {}: {e:#}", pal_path.display()));
            let mut rows: Vec<(u16, String)> = Vec::new();
            for (name, index) in &file.species {
                let Some(id) = extract::names::SPECIES.value(name) else {
                    continue;
                };
                let Some(pal) = file.palettes.get(*index as usize) else {
                    continue;
                };
                let png = dir.join(format!("icons/mons/{name}.png"));
                if !png.is_file() {
                    continue;
                }
                let tiles = png_4bpp(&png);
                let frames = (tiles.len() / 512).clamp(1, 2) as u8;
                let tiles_e = blob.bytes(&tiles);
                let pal_e = blob.u16s(pal);
                rows.push((
                    id as u16,
                    format!(
                        "({id}, ::engine::data::Icon {{ tiles: {tiles_e}, palette: {pal_e}, frames: {frames} }}),"
                    ),
                ));
            }
            rows.sort_by_key(|(id, _)| *id);
            rows.dedup_by_key(|(id, _)| *id);
            for (_, row) in rows {
                mons.push_str(&row);
            }
        }
    }
    let decl = if blob.bytes.is_empty() {
        String::new()
    } else {
        blob.static_decl("ui.bin")
    };
    let src = format!(
        "{decl}\
/// `(UiGfx tag, picture)`.
pub static UI_GFX: &[(::engine::data::UiGfx, ::engine::data::UiPicture)] = &[{gfx}];
/// `(item id, icon)`, sorted by item id.
pub static ITEM_ICONS: &[(u16, ::engine::data::Icon)] = &[{items}];
/// `(species id, icon)`, sorted by species id.
pub static MON_ICONS: &[(u16, ::engine::data::Icon)] = &[{mons}];
"
    );
    (src, blob.bytes)
}

fn png_4bpp(path: &std::path::Path) -> Vec<u8> {
    let img = extract::gfx::read_png_indexed(path)
        .unwrap_or_else(|e| panic!("reading {}: {e:#}", path.display()));
    extract::gfx::image_to_tiles_4bpp(&img).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()))
}
