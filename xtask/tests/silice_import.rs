//! `cargo xtask silice import` on the synthetic fixture chapters (`tests/fixtures/silice/src`).

mod silice_common;
use silice_common::*;

#[test]
fn text_blocks_in_every_format() {
    let (_tmp, g) = imported();
    let fr = read(&g, "text/fr.ron");
    // Several texts in one fence; `\n` and `\p` lines joined raw.
    assert!(
        fr.contains(r#""Alpha_Sc01_T1": "First line of the text.\\nSecond line here.","#),
        "{fr}"
    );
    assert!(
        fr.contains(r#""Alpha_Sc01_T2": "Short one.\\pA new box.","#),
        "{fr}"
    );
    // A line without an escape is joined with a space (and reported).
    assert!(
        fr.contains(r#""Alpha_Sc01_T3": "Joined without escape.","#),
        "{fr}"
    );
    // An indented fence.
    assert!(
        fr.contains(r#""Alpha_Sign_1": "Sign number one.","#),
        "{fr}"
    );
    assert!(
        fr.contains(r#""Beta_Sc01_Hello": "Hello, {PLAYER}!","#),
        "{fr}"
    );
    // Pseudo-scripts are not texts.
    assert!(!fr.contains("msgbox"), "{fr}");
    let sp = read(&g, "text/speakers.ron");
    assert!(sp.contains(r#"speaker: Some("HERO"),"#), "{sp}");
    assert!(
        sp.contains(r#"note: Some("thinking, first visit"),"#),
        "{sp}"
    );
    // A note with nested parentheses and no speaker.
    assert!(
        sp.contains(r#"note: Some("an indented note (with parens)"),"#),
        "{sp}"
    );
    let report = std::fs::read_to_string(g.with_extension("report.md")).unwrap();
    assert!(report.contains("Alpha_Sc01_T3: a line without"), "{report}");
}

#[test]
fn trainers_encounters_and_items() {
    let (_tmp, g) = imported();
    let t = read(&g, "battle/trainers.ron");
    assert!(t.contains(r#"id: "TRAINER_SIL_ALPHA_ONE","#), "{t}");
    assert!(t.contains(r#"item: Some("ITEM_ORAN_BERRY"),"#), "{t}");
    // The item bag and the `//` comments are kept.
    assert!(
        t.contains("items: [\n                \"ITEM_POTION\",\n            ],"),
        "{t}"
    );
    assert!(t.contains(r#""a placeholder class","#), "{t}");
    let e = read(&g, "battle/encounters.ron");
    // Keyed by map name, a Vec of headers.
    assert!(
        e.contains("\"AlphaRoute\": [\n            (\n                land: Some(("),
        "{e}"
    );
    assert!(e.contains("fishing: Some(("), "{e}");
    let it = read(&g, "items.ron");
    assert!(it.contains(r#"id: "ITEM_SIL_LUCKY_CHARM","#), "{it}");
    assert!(it.contains(r#"clone_of: Some("ITEM_SOOT_SACK"),"#), "{it}");
    assert!(it.contains("pos: Some((4, 7)),"), "{it}");
    assert!(it.contains("quantity: 2,"), "{it}");
    // `A + B + C` with three ids: three placements.
    for rod in ["ITEM_OLD_ROD", "ITEM_GOOD_ROD", "ITEM_SUPER_ROD"] {
        assert!(it.contains(&format!("item: Some(\"{rod}\"),")), "{it}");
    }
    // A mart price with trailing words and a thousands space.
    assert!(it.contains("price: 1000,"), "{it}");
}

#[test]
fn ids_names_and_families() {
    let (_tmp, g) = imported();
    let f = read(&g, "ids/flags.ron");
    // `X_1…3` and `` `X_RED` / `_BLUE` ``, in name order from 0x20.
    for n in ["STEP_1", "STEP_2", "STEP_3", "LAMP_RED", "LAMP_BLUE"] {
        assert!(f.contains(&format!("FLAG_SIL_ALPHA_{n}")), "{f}");
    }
    assert!(
        f.contains("name: \"FLAG_SIL_ALPHA_DONE\",\n            num: 32,"),
        "{f}"
    );
    let m = read(&g, "ids/maps.ron");
    // `### MAP_X_1F / _2F` in a heading.
    assert!(m.contains(r#"map: Some("AlphaTown_House_2F"),"#), "{m}");
    assert!(m.contains(r#"map: Some("AlphaRoute"),"#), "{m}");
    for (file, name) in [
        ("vars", "VAR_SIL_ALPHA_STATE"),
        ("music", "MUS_SIL_ALPHA_THEME"),
        ("sounds", "SE_SIL_BELL"),
        ("trainers", "TRAINER_SIL_ALPHA_ONE"),
        ("items", "ITEM_SIL_LUCKY_CHARM"),
    ] {
        assert!(
            read(&g, &format!("ids/{file}.ron")).contains(name),
            "{file}"
        );
    }
    let refs = read(&g, "ids/refs.ron");
    // `msgbox(X_<n>)` and `msgbox(X_A | B)` expanded against the defined texts.
    assert!(refs.contains(r#"pattern: "Alpha_Sign_<n>","#), "{refs}");
    assert!(
        refs.contains("\"Alpha_Sign_1\",\n                \"Alpha_Sign_2\","),
        "{refs}"
    );
    assert!(
        refs.contains("\"Beta_Sc01_Yes\",\n                \"Beta_Sc01_No\","),
        "{refs}"
    );
}

#[test]
fn deterministic_and_rerunnable() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    assert!(import(&src(), &a, false).ok);
    assert!(import(&src(), &b, false).ok);
    let ta = tree(&a);
    assert!(ta.len() >= 15, "{:?}", ta.keys());
    assert_eq!(ta, tree(&b));
    // Re-running over unchanged output needs no --force and changes nothing.
    let r = import(&src(), &a, false);
    assert!(r.ok, "{}", r.out);
    assert!(r.out.contains("0 files written"), "{}", r.out);
    assert_eq!(tree(&a), ta);
}

#[test]
fn refuses_to_overwrite_without_force() {
    let (_tmp, g) = imported();
    let before = tree(&g);
    plant(&g, "text/fr.ron", "Sign number one.", "Edited by hand.");
    let edited = tree(&g);
    let r = import(&src(), &g, false);
    assert!(!r.ok);
    assert!(
        r.out.contains("refusing to overwrite") && r.out.contains("text/fr.ron"),
        "{}",
        r.out
    );
    assert_eq!(tree(&g), edited, "nothing is written on refusal");
    let r = import(&src(), &g, true);
    assert!(r.ok, "{}", r.out);
    assert_eq!(tree(&g), before);
}

#[test]
fn ids_are_append_only() {
    let (_tmp, g) = imported();
    // A tombstone and a stale live name from an earlier import keep their numbers.
    plant(
        &g,
        "ids/flags.ron",
        "    ],\n)",
        "        (\n            name: \"FLAG_SIL_GONE\",\n            num: 200,\n            removed: true,\n        ),\n        (\n            name: \"FLAG_SIL_STALE\",\n            num: 201,\n        ),\n    ],\n)",
    );
    // A new name in the source is appended after the highest number.
    let tmp = tempfile::tempdir().unwrap();
    let src2 = tmp.path().join("src");
    copy_dir(&src(), &src2);
    let ch = src2.join("chapitres/02-beta.md");
    let mut s = std::fs::read_to_string(&ch).unwrap();
    s.push_str("\nAlso `FLAG_SIL_AAA_NEW`.\n");
    std::fs::write(&ch, s).unwrap();
    let r = import(&src2, &g, true);
    assert!(r.ok, "{}", r.out);
    let f = read(&g, "ids/flags.ron");
    assert!(
        f.contains("name: \"FLAG_SIL_ALPHA_DONE\",\n            num: 32,"),
        "{f}"
    );
    assert!(
        f.contains("name: \"FLAG_SIL_GONE\",\n            num: 200,\n            removed: true,"),
        "{f}"
    );
    assert!(
        f.contains("name: \"FLAG_SIL_STALE\",\n            num: 201,"),
        "{f}"
    );
    assert!(
        f.contains("name: \"FLAG_SIL_AAA_NEW\",\n            num: 202,"),
        "{f}"
    );
    let report = std::fs::read_to_string(g.with_extension("report.md")).unwrap();
    assert!(report.contains("FLAG_SIL_STALE"), "{report}");
}
