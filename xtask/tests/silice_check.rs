//! `cargo xtask silice check`: the clean fixture import passes, and each rule fires on a planted
//! error.

mod silice_common;
use silice_common::*;
use std::path::Path;

fn fails_with(games: &Path, tag: &str) -> String {
    let r = check(games, &[]);
    assert!(!r.ok, "expected a failure with {tag}:\n{}", r.out);
    assert!(r.out.contains(tag), "expected {tag}:\n{}", r.out);
    r.out
}

fn warns_with(games: &Path, tag: &str) -> String {
    let r = check(games, &[]);
    assert!(r.ok, "expected success with {tag}:\n{}", r.out);
    assert!(r.out.contains(tag), "expected {tag}:\n{}", r.out);
    r.out
}

#[test]
fn clean_import_passes() {
    let (_tmp, g) = imported();
    let out = warns_with(
        &g,
        "warning[item-incomplete] ITEM_SIL_LUCKY_CHARM: no pocket",
    );
    assert!(out.contains("| total | 0 | 1 | 0 |"), "{out}");
    assert!(out.contains("note: PKR_DECOMP not set"), "{out}");
}

#[test]
fn works_without_the_extract() {
    let (_tmp, g) = imported();
    let r = xtask(&[
        "check",
        "--games",
        g.to_str().unwrap(),
        "--extracted",
        "/nonexistent",
    ]);
    assert!(r.ok, "{}", r.out);
    assert!(
        r.out.contains("note: no extract at /nonexistent"),
        "{}",
        r.out
    );
    // The width rule is skipped: a too-wide line passes without the font.
    plant(&g, "text/fr.ron", "Sign number two.", &"W".repeat(40));
    let r = xtask(&[
        "check",
        "--games",
        g.to_str().unwrap(),
        "--extracted",
        "/nonexistent",
    ]);
    assert!(r.ok, "{}", r.out);
    fails_with(
        &g,
        "error[text-width] Alpha_Sign_2: line 1 is 240 px (max 216)",
    );
}

#[test]
fn ids_rules() {
    let (_tmp, g) = imported();
    plant(&g, "ids/flags.ron", "num: 33,", "num: 32,");
    fails_with(
        &g,
        "error[id-alloc] FLAG_SIL_ALPHA_LAMP_BLUE: ids/flags.ron: number 32 is also",
    );

    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/flags.ron",
        "num: 32,",
        "num: 32,\n            removed: true,",
    );
    fails_with(&g, "error[id-removed] FLAG_SIL_ALPHA_DONE");

    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/refs.ron",
        "\"FLAG_SIL_ALPHA_DONE\": [",
        "\"FLAG_SIL_UNLISTED\": [",
    );
    fails_with(
        &g,
        "error[id-undefined] FLAG_SIL_UNLISTED: missing from ids/flags.ron",
    );

    let (_tmp, g) = imported();
    plant(
        &g,
        "battle/trainers.ron",
        "\"ITEM_ORAN_BERRY\"",
        "\"ITEM_MADE_UP\"",
    );
    fails_with(&g, "error[sil-prefix] ITEM_MADE_UP");

    let (_tmp, g) = imported();
    plant(
        &g,
        "battle/trainers.ron",
        "\"SPECIES_POOCHYENA\"",
        "\"SPECIES_NOT_A_MON\"",
    );
    fails_with(&g, "error[id-unknown] SPECIES_NOT_A_MON");

    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/refs.ron",
        "\"MAP_BETA_CITY\": [",
        "\"MAP_GAMMA_CITY\": [",
    );
    fails_with(&g, "error[map-undefined] MAP_GAMMA_CITY");

    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/refs.ron",
        "\"TRAINER_SIL_ALPHA_ONE\": [",
        "\"TRAINER_SIL_GHOST\": [",
    );
    fails_with(&g, "error[trainer-undefined] TRAINER_SIL_GHOST");

    let (_tmp, g) = imported();
    let base = read(&g, "ids/moves.ron");
    let base = base
        .split("base: ")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .to_string();
    plant(
        &g,
        "ids/moves.ron",
        "ids: [],",
        &format!("ids: [(name: \"MOVE_SIL_NEW\", num: {base})],"),
    );
    warns_with(&g, "warning[invented-move] MOVE_SIL_NEW");
}

#[test]
fn text_rules() {
    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "\n}",
        "\n    \"Alpha_Sign_1\": \"Again.\",\n}",
    );
    fails_with(&g, "error[text-duplicate] Alpha_Sign_1");

    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "\"Beta_Sc01_Hello\":",
        "\"Beta_Sc01_Renamed\":",
    );
    fails_with(&g, "error[text-missing] Beta_Sc01_Hello");

    let (_tmp, g) = imported();
    plant(&g, "text/fr.ron", "Sign number one.", "Sign \u{2014} one.");
    let out = fails_with(
        &g,
        "error[text-charmap] Alpha_Sign_1: no glyph (FR charmap or font/): '\u{2014}'",
    );
    assert!(
        out.contains("glyphs in neither the FR charmap nor font/"),
        "{out}"
    );

    let (_tmp, g) = imported();
    // `{PLAYER}` counts as 7 of the widest capital (6 px here): 30 + 42 + 150 = 222 px.
    plant(
        &g,
        "text/fr.ron",
        "Hello, {PLAYER}!",
        &format!("Hello{{PLAYER}}{}", "W".repeat(25)),
    );
    fails_with(
        &g,
        "error[text-width] Beta_Sc01_Hello: line 1 is 222 px (max 216)",
    );

    // An unencodable character still counts (as a widest capital): 37 x 6 = 222 px.
    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "Sign number two.",
        &"\u{2014}".repeat(37),
    );
    let out = fails_with(
        &g,
        "error[text-width] Alpha_Sign_2: line 1 is 222 px (max 216)",
    );
    assert!(out.contains("error[text-charmap] Alpha_Sign_2"), "{out}");

    // `{STR_VAR_1}` counts as 13 widest capitals: 78 + 25 x 6 = 228 px.
    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "Sign number two.",
        &format!("{{STR_VAR_1}}{}", "W".repeat(25)),
    );
    fails_with(
        &g,
        "error[text-width] Alpha_Sign_2: line 1 is 228 px (max 216)",
    );

    // A defeat text (shown in the battle box) gets 208 px: 35 x 6 = 210 fails there only.
    let (_tmp, g) = imported();
    plant(&g, "text/fr.ron", "You beat me.", &"W".repeat(35));
    plant(&g, "text/fr.ron", "Sign number two.", &"W".repeat(35));
    let out = fails_with(
        &g,
        "error[text-width] Beta_Sc02_Defeat: line 1 is 210 px (max 208)",
    );
    assert!(!out.contains("Alpha_Sign_2"), "{out}");

    // Two lines per box; `\l` scrolls, `\p` starts a new box.
    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "Sign number one.",
        r"One\nTwo\lThree\pFour\nFive",
    );
    plant(&g, "text/fr.ron", "Sign number two.", r"One\nTwo\nThree");
    let out = fails_with(
        &g,
        "error[text-lines] Alpha_Sign_2: box 1 has 3 lines (max 2; use \\l to scroll)",
    );
    assert!(!out.contains("[text-lines] Alpha_Sign_1"), "{out}");

    // `text/speakers.ron` and `text/fr.ron` must list the same ids.
    let (_tmp, g) = imported();
    plant(
        &g,
        "text/fr.ron",
        "\n}",
        "\n    \"Alpha_Extra\": r\"Extra.\",\n}",
    );
    fails_with(&g, "error[speakers-mismatch] Alpha_Extra");

    // An unresolved placeholder in the source.
    let tmp = tempfile::tempdir().unwrap();
    let src2 = tmp.path().join("src");
    copy_dir(&src(), &src2);
    let ch = src2.join("chapitres/02-beta.md");
    let s = std::fs::read_to_string(&ch).unwrap();
    std::fs::write(
        &ch,
        s.replace(
            "msgbox(Beta_Sc01_Hello)\n",
            "msgbox(Beta_Sc01_Hello)\nmsgbox(Beta_Q<n>_Pick)\n",
        ),
    )
    .unwrap();
    let g = tmp.path().join("games");
    assert!(import(&src2, &g, false).ok);
    fails_with(&g, "error[dynamic-unresolved] Beta_Q<n>_Pick");
}

/// The authored glyphs (`font/*.ron`, SIL1 Task G): the committed files, on the fixture extract
/// (every glyph 6 px wide).
fn with_glyphs(g: &Path) {
    let font = g.join("font");
    std::fs::create_dir_all(&font).unwrap();
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../games/silice/font");
    for e in std::fs::read_dir(&committed).unwrap().flatten() {
        std::fs::copy(e.path(), font.join(e.file_name())).unwrap();
    }
}

/// The glyph file `name` redrawn at `width` (rows padded or cut on the right).
fn set_width(g: &Path, name: &str, width: usize) {
    let p = g.join("font").join(format!("{name}.ron"));
    let mut f: extract::games::font::GlyphFile =
        ron::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    f.width = width as u8;
    for r in &mut f.rows {
        *r = format!("{r:.<width$}").chars().take(width).collect();
    }
    std::fs::write(&p, ron::to_string(&f).unwrap()).unwrap();
}

#[test]
fn authored_glyph_rules() {
    let heart = char::from_u32(0x2665).unwrap();
    let dot = char::from_u32(0xB7).unwrap();
    // The redrawn middle dot keeps the extract's width (6 in the fixture, 3 in the FR font).
    let (_tmp, g) = imported();
    with_glyphs(&g);
    fails_with(
        &g,
        "error[font-glyph] font: glyph middle_dot: width 3 but the extract draws 0xAF 6 wide",
    );

    // Both encode; the heart is measured at its authored width (8): 27 fit in 216 px.
    let (_tmp, g) = imported();
    with_glyphs(&g);
    set_width(&g, "middle_dot", 6);
    plant(
        &g,
        "text/fr.ron",
        "Sign number one.",
        &format!("Sign {heart} one {dot} two."),
    );
    plant(
        &g,
        "text/fr.ron",
        "Sign number two.",
        &heart.to_string().repeat(27),
    );
    let r = check(&g, &[]);
    assert!(r.ok, "{}", r.out);
    assert!(!r.out.contains("text-charmap"), "{}", r.out);
    assert!(!r.out.contains("font-glyph"), "{}", r.out);
    let row = |n: usize| heart.to_string().repeat(n);
    plant(&g, "text/fr.ron", &row(27), &row(28));
    fails_with(
        &g,
        "error[text-width] Alpha_Sign_2: line 1 is 224 px (max 216)",
    );

    // The other missing glyphs still need a ruling.
    let (_tmp, g) = imported();
    with_glyphs(&g);
    set_width(&g, "middle_dot", 6);
    plant(
        &g,
        "text/fr.ron",
        "Sign number one.",
        &format!("{heart} \u{2014} \u{B0}"),
    );
    fails_with(
        &g,
        "error[text-charmap] Alpha_Sign_1: no glyph (FR charmap or font/): '\u{B0}' '\u{2014}'",
    );

    // A code without its glyph, and a glyph without a code.
    let (_tmp, g) = imported();
    with_glyphs(&g);
    set_width(&g, "middle_dot", 6);
    std::fs::remove_file(g.join("font/heart.ron")).unwrap();
    fails_with(
        &g,
        "error[font-glyph] font: no glyph file draws '\u{2665}' (code 0x37)",
    );
    let (_tmp, g) = imported();
    with_glyphs(&g);
    plant(&g, "font/heart.ron", &format!("'{heart}'"), "'\u{2014}'");
    fails_with(&g, "has no code in extract::games::font::CODES");

    // Without `font/` the check runs and says so.
    let (_tmp, g) = imported();
    let r = check(&g, &[]);
    assert!(r.ok, "{}", r.out);
    assert!(
        r.out
            .contains("the authored glyphs encode but nothing draws them"),
        "{}",
        r.out
    );
}

#[test]
fn trainer_rules() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "name: \"ONE\",",
            "name: \"ELEVENCHARS\",",
            "error[trainer-name] TRAINER_SIL_ALPHA_ONE",
        ),
        (
            "class_name: \"HIKER\",",
            "class_name: \"THIRTEENCHARS\",",
            "error[trainer-class-name] TRAINER_SIL_ALPHA_ONE",
        ),
        (
            "\"ITEM_POTION\",\n",
            "\"ITEM_POTION\", \"ITEM_POTION\", \"ITEM_POTION\", \"ITEM_POTION\", \"ITEM_POTION\",\n",
            "error[trainer-items] TRAINER_SIL_ALPHA_ONE: 5 items",
        ),
        (
            "\"MOVE_GROWL\",",
            "\"MOVE_GROWL\", \"MOVE_GROWL\",",
            "error[party-moves] TRAINER_SIL_ALPHA_ONE: mon 2: 5 moves",
        ),
        (
            "level: 5,",
            "level: 0,",
            "error[party-level] TRAINER_SIL_ALPHA_ONE: mon 1: level 0",
        ),
    ];
    for (from, to, tag) in cases {
        let (_tmp, g) = imported();
        plant(&g, "battle/trainers.ron", from, to);
        fails_with(&g, tag);
    }

    let (_tmp, g) = imported();
    let t = read(&g, "battle/trainers.ron");
    let a = t.find("party: [").unwrap();
    let b = t.find("items: [").unwrap();
    std::fs::write(
        g.join("battle/trainers.ron"),
        format!("{}party: [],\n            {}", &t[..a], &t[b..]),
    )
    .unwrap();
    fails_with(&g, "error[party-size] TRAINER_SIL_ALPHA_ONE: 0 mons");

    let (_tmp, g) = imported();
    let t = read(&g, "battle/trainers.ron");
    let a = t.find("        (\n            id:").unwrap();
    let b = t.rfind("    ],\n)").unwrap();
    std::fs::write(
        g.join("battle/trainers.ron"),
        format!("{}{}{}", &t[..b], &t[a..b], &t[b..]),
    )
    .unwrap();
    fails_with(&g, "error[trainer-duplicate] TRAINER_SIL_ALPHA_ONE");
}

#[test]
fn encounter_rules() {
    let (_tmp, g) = imported();
    plant(
        &g,
        "battle/encounters.ron",
        "max_level: 3,",
        "max_level: 1,",
    );
    fails_with(
        &g,
        "error[encounter-level] AlphaRoute: header 0 land slot 0: levels 2-1",
    );

    let (_tmp, g) = imported();
    let slot = "                        (\n                            species: \"SPECIES_ZIGZAGOON\",\n                            min_level: 2,\n                            max_level: 3,\n                        ),\n";
    plant(&g, "battle/encounters.ron", slot, "");
    fails_with(
        &g,
        "error[encounter-slots] AlphaRoute: header 0 land: 11 slots (Gen3 has 12)",
    );

    let (_tmp, g) = imported();
    plant(
        &g,
        "battle/encounters.ron",
        "\"AlphaRoute\":",
        "\"NowhereRoute\":",
    );
    fails_with(&g, "error[map-undefined] NowhereRoute");

    let (_tmp, g) = imported();
    plant(
        &g,
        "items.ron",
        "item: Some(\"ITEM_OLD_ROD\"),",
        "item: Some(\"ITEM_POKE_BALL\"),",
    );
    warns_with(
        &g,
        "warning[fishing-unreachable] AlphaRoute: old rod slots 0-1",
    );
}

#[test]
fn item_rules() {
    let (_tmp, g) = imported();
    plant(
        &g,
        "items.ron",
        "name: Some(\"Lucky Charm\"),",
        "name: Some(\"A Very Long Charm\"),",
    );
    warns_with(&g, "warning[item-name-length] ITEM_SIL_LUCKY_CHARM");

    let (_tmp, g) = imported();
    // The fixture extract names ITEM_POTION "Tonic".
    plant(
        &g,
        "items.ron",
        "name: Some(\"Lucky Charm\"),",
        "name: Some(\"to nic\"),",
    );
    warns_with(
        &g,
        "warning[item-name-clash] ITEM_SIL_LUCKY_CHARM: \"to nic\" is also the name of ITEM_POTION",
    );

    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/items.ron",
        "ITEM_SIL_LUCKY_CHARM",
        "ITEM_SIL_OTHER",
    );
    fails_with(&g, "error[id-undefined] ITEM_SIL_LUCKY_CHARM");
}

#[test]
fn exceptions_and_allow() {
    let (_tmp, g) = imported();
    plant(
        &g,
        "battle/trainers.ron",
        "name: \"ONE\",",
        "name: \"ELEVENCHARS\",",
    );
    fails_with(&g, "error[trainer-name]");

    // `--allow RULE` lists the finding as excepted.
    let r = check(&g, &["--allow", "trainer-name"]);
    assert!(r.ok, "{}", r.out);
    assert!(r.out.contains("| trainer-name | 0 | 0 | 1 |"), "{}", r.out);

    // An exceptions file: a glob subject, a reason, a ruling flag; an unused entry warns.
    std::fs::write(
        g.join("check-exceptions.ron"),
        r#"(
    exceptions: [
        (
            rule: "trainer-name",
            subject: "TRAINER_SIL_ALPHA_*",
            reason: "a test name",
            ruling_needed: true,
        ),
        (
            rule: "text-width",
            subject: "Nothing_*",
            reason: "matches nothing",
        ),
    ],
)
"#,
    )
    .unwrap();
    let r = check(&g, &[]);
    assert!(r.ok, "{}", r.out);
    assert!(r.out.contains("| trainer-name | 0 | 0 | 1 |"), "{}", r.out);
    assert!(
        r.out
            .contains("ruling needed [trainer-name] TRAINER_SIL_ALPHA_*: a test name"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .contains("warning[exception-unused] text-width Nothing_*"),
        "{}",
        r.out
    );
}

/// A map file in the authored maps' layout (one field per line).
fn map(body: &str) -> String {
    format!("(\n    music: \"MUS_SIL_ALPHA_THEME\",\n{body})\n")
}

#[test]
fn map_rules() {
    // A ball placement whose map exists but lacks its flag.
    let (_tmp, g) = imported();
    std::fs::create_dir_all(g.join("maps")).unwrap();
    let rods = "    objects: [\n        (\n            flag: Some(\"FLAG_SIL_ITEM_ALPHA_RODS\"),\n        ),\n    ],\n";
    std::fs::write(g.join("maps/AlphaTown.ron"), map(rods)).unwrap();
    let out = fails_with(
        &g,
        "error[placement-map] FLAG_SIL_ITEM_ALPHA_POTION: Potion (ITEM_POTION) is placed in MAP_ALPHA_TOWN",
    );
    assert!(
        !out.contains("placement-map] FLAG_SIL_ITEM_ALPHA_RODS"),
        "{out}"
    );

    // The flag on a hidden item of another item.
    let hidden = "    bg_events: [\n        (\n            kind: HiddenItem(\n                item: \"ITEM_ORAN_BERRY\",\n                flag: \"FLAG_SIL_ITEM_ALPHA_POTION\",\n            ),\n        ),\n    ],\n";
    std::fs::write(
        g.join("maps/AlphaTown.ron"),
        map(&format!("{rods}{hidden}")),
    )
    .unwrap();
    fails_with(
        &g,
        "error[placement-map] FLAG_SIL_ITEM_ALPHA_POTION: maps/AlphaTown.ron hides ITEM_ORAN_BERRY under this flag",
    );

    // Names in maps: undefined Silice ids, Gen3-looking names without _SIL_, unknown warp maps.
    let (_tmp, g) = imported();
    std::fs::create_dir_all(g.join("maps")).unwrap();
    for (body, tag) in [
        (
            "    objects: [\n        (\n            flag: Some(\"FLAG_SIL_NOT_ALLOCATED\"),\n        ),\n    ],\n",
            "error[id-undefined] FLAG_SIL_NOT_ALLOCATED",
        ),
        (
            "    coord_events: [\n        (\n            var: \"VAR_MADE_UP\",\n        ),\n    ],\n",
            "error[sil-prefix] VAR_MADE_UP",
        ),
        (
            "    warps: [\n        (\n            dest_map: \"NowhereTown\",\n        ),\n    ],\n",
            "error[map-undefined] NowhereTown",
        ),
    ] {
        std::fs::write(g.join("maps/BetaCity.ron"), map(body)).unwrap();
        fails_with(&g, tag);
    }

    // A placement whose map cell is not a MAP_ constant.
    let (_tmp, g) = imported();
    plant(
        &g,
        "items.ron",
        "map: \"MAP_ALPHA_GYM\",",
        "map: \"the gym\",",
    );
    fails_with(&g, "error[map-undefined] the gym");
}

#[test]
fn id_file_rules() {
    // A Gen3 story flag the data uses, numbered inside the Silice flag block.
    let (_tmp, g) = imported();
    plant(
        &g,
        "ids/refs.ron",
        "\"FLAG_SIL_ALPHA_DONE\": [",
        "\"FLAG_UNUSED_0x021\": [\n            \"01-alpha\",\n        ],\n        \"FLAG_SIL_ALPHA_DONE\": [",
    );
    fails_with(
        &g,
        "error[gen3-collision] FLAG_UNUSED_0x021: Gen3 number 0x21",
    );

    // The pre-merge music and sounds files are refused; songs share one number space.
    let (_tmp, g) = imported();
    std::fs::write(g.join("ids/music.ron"), read(&g, "ids/songs.ron")).unwrap();
    fails_with(&g, "error[id-alloc] ids/music.ron");
}
