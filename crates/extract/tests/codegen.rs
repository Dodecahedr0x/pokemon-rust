//! Script lowering (`codegen::lower`, Milestone 8 Task D1) on hand-written IR, with a compile
//! check of the lowered code against the stub prelude in `tests/stub_prelude/`, wrapped exactly
//! as `content` wraps it. The legacy ROM-to-Rust generator is gone (Task D2).

mod lowering {
    use IrArg::*;
    use extract::codegen::{self, Class, Lowered, Support};
    use extract::names;
    use extract::schema::{
        Hex, IrArg, IrInsn, IrLabel, IrMovement, NameOrigin, RootKind, ScriptEntry, ScriptFile,
        ScriptIndex,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;
    use std::process::Command;

    fn i(op: &str, args: Vec<IrArg>) -> IrInsn {
        IrInsn {
            op: op.to_string(),
            args,
        }
    }

    fn label(
        name: &str,
        root: Option<RootKind>,
        insns: Vec<IrInsn>,
        next: Option<&str>,
    ) -> IrLabel {
        IrLabel {
            name: name.to_string(),
            origin: NameOrigin::Decomp,
            offset: Hex(0),
            root,
            insns,
            next: next.map(str::to_string),
        }
    }

    fn file(module: &str, labels: Vec<IrLabel>, movements: Vec<IrMovement>) -> ScriptFile {
        ScriptFile {
            module: module.to_string(),
            source: None,
            labels,
            movements,
        }
    }

    fn s(name: &str) -> IrArg {
        Script(name.to_string())
    }
    fn t(name: &str) -> IrArg {
        Text(name.to_string())
    }
    fn v(name: &str) -> IrArg {
        Var(name.to_string())
    }
    fn f(name: &str) -> IrArg {
        Flag(name.to_string())
    }
    fn c(name: &str) -> IrArg {
        let table = name.rsplit_once('_').map_or(name, |p| p.0).to_string();
        Const {
            table,
            name: name.to_string(),
        }
    }

    /// The index a Task B export would write for `files` (texts from the IR's text operands).
    fn index(files: &[ScriptFile]) -> ScriptIndex {
        let mut ix = ScriptIndex::default();
        for f in files {
            for l in &f.labels {
                ix.scripts.insert(
                    l.name.clone(),
                    ScriptEntry {
                        module: f.module.clone(),
                        root: l.root.is_some(),
                        origin: l.origin,
                        offset: l.offset,
                    },
                );
                for insn in &l.insns {
                    for a in &insn.args {
                        if let Text(t) = a {
                            ix.texts.insert(t.clone(), f.module.clone());
                        }
                    }
                }
            }
        }
        ix
    }

    fn support_file(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../content/build/support")
                .join(name),
        )
        .unwrap()
    }

    /// The committed `commands.ron` and `movements.ron`, plus a few specials of each class.
    fn support() -> Support {
        let mut s = Support::parse(
            &support_file("commands.ron"),
            None,
            Some(&support_file("movements.ron")),
        )
        .unwrap();
        s.specials = BTreeMap::from([
            ("SPECIAL_HealPlayerParty".to_string(), Class::Native),
            ("SPECIAL_ShowMapNamePopup".to_string(), Class::Ignore),
            (
                "SPECIAL_ShouldTryRematchBattle".to_string(),
                Class::Default(0),
            ),
            ("SPECIAL_DoPCTurnOnEffect".to_string(), Class::Unsupported),
        ]);
        s
    }

    fn lower(files: &[ScriptFile]) -> Lowered {
        lower_with(files, &BTreeMap::new())
    }

    fn lower_with(files: &[ScriptFile], overrides: &BTreeMap<String, String>) -> Lowered {
        codegen::lower(files, &index(files), &support(), overrides).unwrap()
    }

    /// The source of `pub async fn <name>` in `src`.
    fn fn_src<'a>(src: &'a str, name: &str) -> &'a str {
        let start = src
            .find(&format!("pub async fn {name}("))
            .unwrap_or_else(|| panic!("no fn {name} in:\n{src}"));
        let end = start + src[start..].find("\n}\n").unwrap() + 3;
        &src[start..end]
    }

    fn kid() -> Vec<ScriptFile> {
        vec![file(
            "Town",
            vec![
                label(
                    "Town_EventScript_Kid",
                    Some(RootKind::Object(1)),
                    vec![
                        i("lock", vec![]),
                        i("faceplayer", vec![]),
                        i("checkflag", vec![f("FLAG_SYS_POKEMON_GET")]),
                        i("goto_if", vec![Byte(1), s("Town_EventScript_KidHasMon")]),
                        i("setvar", vec![v("VAR_0x8004"), Half(0)]),
                    ],
                    Some("Town_EventScript_KidLoop"),
                ),
                label(
                    "Town_EventScript_KidLoop",
                    None,
                    vec![
                        i("addvar", vec![v("VAR_0x8004"), Half(1)]),
                        i("compare_var_to_value", vec![v("VAR_0x8004"), Half(3)]),
                        i("goto_if", vec![Byte(0), s("Town_EventScript_KidLoop")]),
                        i("loadword", vec![Byte(0), t("Town_Text_Kid")]),
                        i("callstd", vec![c("MSGBOX_NPC")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "Town_EventScript_KidHasMon",
                    None,
                    vec![
                        i("loadword", vec![Byte(0), t("Town_Text_HasMon")]),
                        i("callstd", vec![Byte(4)]),
                        i("release", vec![]),
                        i("end", vec![]),
                    ],
                    None,
                ),
            ],
            vec![],
        )]
    }

    const KID: &str = r#"pub async fn Town_EventScript_Kid(ctx: Ctx) -> Flow {
    let mut pc: u16 = 0;
    loop {
        match pc {
            0 => {
                // Town_EventScript_Kid
                ctx.lock().await;
                ctx.face_player().await;
                ctx.set_cond(if ctx.flag(FLAG_SYS_POKEMON_GET).await { Ordering::Equal } else { Ordering::Less });
                if ctx.cond() == Ordering::Equal {
                    pc = 2;
                    continue;
                }
                ctx.set_var(VAR_0x8004, 0).await;
                pc = 1;
                continue;
            }
            1 => {
                // Town_EventScript_KidLoop
                ctx.set_var(VAR_0x8004, ctx.var(VAR_0x8004).await.wrapping_add(1)).await;
                ctx.set_cond(ctx.var(VAR_0x8004).await.cmp(&3));
                if ctx.cond() == Ordering::Less {
                    pc = 1;
                    continue;
                }
                msgbox(&ctx, Town_Text_Kid, MsgBox::Npc).await;
                return Flow::End;
            }
            2 => {
                // Town_EventScript_KidHasMon
                msgbox(&ctx, Town_Text_HasMon, MsgBox::Default).await;
                ctx.release().await;
                return Flow::End;
            }
            _ => return Flow::End,
        }
    }
}
"#;

    #[test]
    fn lower_structured_flow_golden() {
        let out = lower(&kid());
        let src = &out.modules["Town"];
        assert!(src.starts_with("use crate::data::Flow;\n"), "{src}");
        assert_eq!(fn_src(src, "Town_EventScript_Kid"), KID, "{src}");
        // Blocks owned by one function are arms, not functions.
        assert_eq!(out.stats.fns, 1);
        assert_eq!(out.stats.labels, 3);
        assert_eq!(out.stats.texts, 2);
        assert!(out.stats.reports.is_empty(), "{:?}", out.stats.reports);
        assert_eq!(out.stats.named_decomp_pct["functions"], 100);
    }

    #[test]
    fn lower_is_deterministic() {
        assert_eq!(lower(&kid()), lower(&kid()));
    }

    /// Two roots jump into one block: it becomes a function, entered by tail calls.
    #[test]
    fn lower_tail_calls_shared_blocks() {
        let files = vec![
            file(
                "A",
                vec![
                    label(
                        "A_One",
                        Some(RootKind::Bg(0)),
                        vec![i("lock", vec![])],
                        Some("A_Shared"),
                    ),
                    label(
                        "A_Two",
                        Some(RootKind::Bg(1)),
                        vec![i("goto", vec![s("A_Shared")])],
                        None,
                    ),
                    label(
                        "A_Shared",
                        None,
                        vec![i("release", vec![]), i("return", vec![])],
                        None,
                    ),
                ],
                vec![],
            ),
            file(
                "B",
                vec![label(
                    "B_Root",
                    Some(RootKind::Coord(0)),
                    vec![i("goto", vec![s("A_Two")])],
                    None,
                )],
                vec![],
            ),
        ];
        let out = lower(&files);
        let a = &out.modules["A"];
        assert_eq!(
            fn_src(a, "A_One"),
            "pub async fn A_One(ctx: Ctx) -> Flow {\n    ctx.lock().await;\n    \
             return Box::pin(A_Shared(ctx.clone())).await;\n}\n"
        );
        assert_eq!(
            fn_src(a, "A_Shared"),
            "pub async fn A_Shared(ctx: Ctx) -> Flow {\n    ctx.release().await;\n    \
             return Flow::Continue;\n}\n"
        );
        assert!(
            fn_src(&out.modules["B"], "B_Root")
                .contains("return Box::pin(crate::scripts::A::A_Two(ctx.clone())).await;")
        );
        assert_eq!(out.stats.fns, 4);
        assert_eq!(out.stats.tail_call_groups, 0);
    }

    /// Functions that tail-call each other share one loop: a `goto` cycle between them runs in
    /// constant memory. Each member enters the loop at its own block.
    #[test]
    fn lower_tail_call_groups_share_one_loop() {
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_A",
                    Some(RootKind::Object(1)),
                    vec![
                        i("lock", vec![]),
                        i("goto_if", vec![Byte(1), s("M_B")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_B",
                    Some(RootKind::Object(2)),
                    vec![i("release", vec![]), i("goto", vec![s("M_A")])],
                    None,
                ),
                label(
                    "M_C",
                    Some(RootKind::Object(3)),
                    vec![i("goto", vec![s("M_B")])],
                    None,
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        let m = &out.modules["M"];
        assert_eq!(out.stats.tail_call_groups, 1);
        assert_eq!(
            fn_src(m, "M_A"),
            "pub async fn M_A(ctx: Ctx) -> Flow {\n    M_A__tail(ctx, 0).await\n}\n"
        );
        assert_eq!(
            fn_src(m, "M_B"),
            "pub async fn M_B(ctx: Ctx) -> Flow {\n    M_A__tail(ctx, 1).await\n}\n"
        );
        assert_eq!(
            fn_src(m, "M_A__tail"),
            "pub async fn M_A__tail(ctx: Ctx, mut pc: u16) -> Flow {\n    loop {\n        match pc {\n            \
             0 => {\n                // M_A\n                ctx.lock().await;\n                \
             if ctx.cond() == Ordering::Equal {\n                    pc = 1;\n                    continue;\n                }\n                \
             return Flow::End;\n            }\n            \
             1 => {\n                // M_B\n                ctx.release().await;\n                \
             pc = 0;\n                continue;\n            }\n            \
             _ => return Flow::End,\n        }\n    }\n}\n"
        );
        assert!(m.contains("/// The tail-call group of M_A, M_B.\n"), "{m}");
        // Outside the group, still a tail call.
        assert!(fn_src(m, "M_C").contains("return Box::pin(M_B(ctx.clone())).await;"));
        // The group is not a script.
        assert!(!out.dispatch.contains("M_A__tail"));
        // A member's digest covers the shared loop.
        let mut g = files.clone();
        g[0].labels[1].insns[0] = i("lock", vec![]);
        assert_ne!(out.script_digests["M_A"], lower(&g).script_digests["M_A"]);
    }

    /// The order of the input files changes nothing: not the sources, not the digests.
    #[test]
    fn lower_ignores_file_order() {
        let mut files = graph("FLAG_TEMP_2");
        files.extend(kid());
        files.extend(every_native());
        let a = lower(&files);
        for k in 1..files.len() {
            let mut shuffled = files.clone();
            shuffled.rotate_left(k);
            assert_eq!(
                lower(&shuffled).script_digests,
                a.script_digests,
                "rotation {k}"
            );
        }
        files.reverse();
        assert_eq!(lower(&files), a);
    }

    /// Gen3: an `end` in a callee ends the whole script, so its callers stop too; a callee that
    /// only `return`s is a plain call.
    #[test]
    fn lower_end_in_callee_stops_the_caller() {
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Root",
                    Some(RootKind::Object(1)),
                    vec![
                        i("call", vec![s("M_Ends")]),
                        i("call", vec![s("M_Returns")]),
                        i("call", vec![s("M_EndsViaTail")]),
                        i("release", vec![]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Ends",
                    None,
                    vec![i("lock", vec![]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_Returns",
                    None,
                    vec![i("lock", vec![]), i("return", vec![])],
                    None,
                ),
                label(
                    "M_EndsViaTail",
                    None,
                    vec![i("goto", vec![s("M_Ends")])],
                    None,
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Root"),
            "pub async fn M_Root(ctx: Ctx) -> Flow {\n    \
             if Box::pin(M_Ends(ctx.clone())).await == Flow::End {\n        return Flow::End;\n    }\n    \
             Box::pin(M_Returns(ctx.clone())).await;\n    \
             if Box::pin(M_EndsViaTail(ctx.clone())).await == Flow::End {\n        return Flow::End;\n    }\n    \
             ctx.release().await;\n    return Flow::End;\n}\n"
        );
        assert_eq!(out.stats.end_in_callee, 2);
    }

    /// The comparison register is the script's: a callee's `compare` is read by its caller.
    #[test]
    fn lower_cond_flows_across_calls() {
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Root",
                    Some(RootKind::Object(1)),
                    vec![
                        i("call", vec![s("M_Check")]),
                        i("goto_if", vec![Byte(1), s("M_Yes")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Check",
                    None,
                    vec![
                        i("compare_var_to_value", vec![v("VAR_RESULT"), Half(1)]),
                        i("return", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Yes",
                    None,
                    vec![i("release", vec![]), i("end", vec![])],
                    None,
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        let m = &out.modules["M"];
        assert!(!m.contains("let mut cond"), "{m}");
        assert!(fn_src(m, "M_Root").contains("if ctx.cond() == Ordering::Equal {"));
        assert!(fn_src(m, "M_Check").contains("ctx.set_cond(ctx.var(VAR_RESULT).await.cmp(&1));"));
    }

    #[test]
    fn lower_waitmovement_zero_is_the_last_mover() {
        let mv = IrMovement {
            name: "M_Movement_Walk".into(),
            origin: NameOrigin::Decomp,
            actions: vec!["MOVEMENT_ACTION_WALK_NORMAL_UP".into()],
        };
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("waitmovement", vec![Half(0)]),
                    i(
                        "applymovement",
                        vec![Half(2), Movement("M_Movement_Walk".into())],
                    ),
                    i("waitmovement", vec![Half(0)]),
                    i("waitmovement", vec![Half(3)]),
                    i("end", vec![]),
                ],
                None,
            )],
            vec![mv],
        )];
        let out = lower(&files);
        let src = fn_src(&out.modules["M"], "M_Root");
        assert_eq!(
            src.matches("ctx.wait_move(0).await; // the last mover")
                .count(),
            2
        );
        assert!(src.contains("ctx.move_obj(2, &M_Movement_Walk).await;"));
        assert!(src.contains("ctx.wait_move(3).await;"));
        assert_eq!(out.stats.unresolved_waitmovement, 1);
    }

    /// Unknown movement actions follow `commands.ron`'s `movements`.
    #[test]
    fn lower_movements_use_the_fallback_table() {
        let mv = IrMovement {
            name: "M_Movement_Odd".into(),
            origin: NameOrigin::Positional,
            actions: [
                "MOVEMENT_ACTION_SLIDE_UP",
                "MOVEMENT_ACTION_WALK_NORMAL_DIAGONAL_UP_LEFT",
                "MOVEMENT_ACTION_FIGURE_8",
                "MOVEMENT_ACTION_DISABLE_ANIMATION",
                "MOVEMENT_ACTION_0xA0",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        };
        let out = lower(&[file("M", vec![], vec![mv])]);
        let m = &out.modules["M"];
        assert_eq!(
            m,
            "\npub static M_Movement_Odd: [Step; 5] = [\n    \
             Step::Walk(Dir::Up, Speed::Fast), // SLIDE_UP (as WALK_FAST_UP)\n    \
             Step::Walk(Dir::Up, Speed::Fast), // WALK_NORMAL_DIAGONAL_UP_LEFT (as WALK_FAST_UP)\n    \
             Step::Walk(Dir::Left, Speed::Fast), // WALK_NORMAL_DIAGONAL_UP_LEFT (as WALK_FAST_LEFT)\n    \
             Step::Delay(64), // FIGURE_8 (as a delay)\n    \
             // DISABLE_ANIMATION (no effect here)\n    \
             Step::Delay(0), // 0xA0 (unsupported)\n];\n",
            "no fns, so no Flow import"
        );
        assert_eq!(
            out.stats.unknown_movement_actions,
            BTreeMap::from([("MOVEMENT_ACTION_0xA0".to_string(), 1)])
        );
        assert_eq!(out.stats.named_decomp_pct["movements"], 0);
    }

    /// Every `movements` fallback of `commands.ron` lowers to actions that have a `Step`.
    #[test]
    fn lower_every_movement_fallback() {
        let mv = IrMovement {
            name: "M_All".into(),
            origin: NameOrigin::Decomp,
            actions: support().movements.keys().cloned().collect(),
        };
        let out = lower(&[file("M", vec![], vec![mv])]);
        assert!(
            out.stats.unknown_movement_actions.is_empty(),
            "{:?}",
            out.stats
        );
    }

    #[test]
    fn lower_classes_ignore_default_unsupported() {
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("playse", vec![c("SE_PIN")]),
                    i("checkpartymove", vec![c("MOVE_CUT")]),
                    i("checkcoins", vec![v("VAR_0x8005")]),
                    i("pokemart", vec![Raw(0x1234)]),
                    i("special", vec![c("SPECIAL_HealPlayerParty")]),
                    i("special", vec![c("SPECIAL_ShowMapNamePopup")]),
                    i(
                        "specialvar",
                        vec![v("VAR_RESULT"), c("SPECIAL_ShouldTryRematchBattle")],
                    ),
                    i("special", vec![c("SPECIAL_DoPCTurnOnEffect")]),
                    i("gotobeatenscript", vec![]),
                ],
                None,
            )],
            vec![],
        )];
        let out = lower(&files);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Root"),
            "pub async fn M_Root(ctx: Ctx) -> Flow {\n    \
             // playse SE_PIN\n    \
             ctx.set_var(VAR_RESULT, 6).await; // checkpartymove (default)\n    \
             ctx.set_var(VAR_0x8005, 0).await; // checkcoins (default)\n    \
             ctx.report(Report::Unsupported(\"pokemart\")).await; // 0x1234\n    \
             special(&ctx, SPECIAL_HealPlayerParty).await;\n    \
             // special SPECIAL_ShowMapNamePopup\n    \
             ctx.set_var(VAR_RESULT, 0).await; // SPECIAL_ShouldTryRematchBattle (default)\n    \
             ctx.report(Report::Unsupported(\"SPECIAL_DoPCTurnOnEffect\")).await;\n    \
             ctx.report(Report::Unsupported(\"gotobeatenscript\")).await;\n    \
             return Flow::End;\n}\n"
        );
        let st = &out.stats;
        assert_eq!(st.by_class["Ignore"], 2, "{:?}", st.by_class);
        assert_eq!(st.by_class["Default"], 3, "{:?}", st.by_class);
        assert_eq!(st.by_class["Unsupported"], 3, "{:?}", st.by_class);
        // Only `Unsupported` reports; `Default` is the documented behaviour.
        assert_eq!(
            st.reports.keys().collect::<Vec<_>>(),
            ["SPECIAL_DoPCTurnOnEffect", "gotobeatenscript", "pokemart"],
            "{:?}",
            st.reports
        );
    }

    #[test]
    fn lower_fails_on_unclassified_commands_and_specials() {
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("frobnicate", vec![]),
                    i("special", vec![c("SPECIAL_ChooseStarter")]),
                    i("end", vec![]),
                ],
                None,
            )],
            vec![],
        )];
        let err = codegen::lower(&files, &index(&files), &support(), &BTreeMap::new())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("frobnicate") && err.contains("SPECIAL_ChooseStarter"),
            "{err}"
        );
    }

    #[test]
    fn lower_dispatch_is_a_thunk_table() {
        let out = lower(&kid());
        let d = &out.dispatch;
        assert!(!d.contains("match id"), "{d}");
        assert!(d.contains("pub static SCRIPT_NAMES: [&str; 1] = [\"Town_EventScript_Kid\",];"));
        assert!(d.contains("pub const Town_EventScript_Kid: ScriptId = ScriptId(0);"));
        assert!(d.contains(
            "pub static SCRIPTS: [fn(engine::script::Ctx) -> engine::script::Script; 1] = [\n    \
             |ctx| alloc::boxed::Box::pin(run(crate::scripts::Town::Town_EventScript_Kid(ctx))),\n];"
        ));
        assert!(d.contains("pub fn script(id: u16, ctx: engine::script::Ctx)"));
        // Absolute paths only: the content crate `include!`s it in `crate::data`.
        assert!(!d.contains("\nuse "), "{d}");
        assert!(!d.contains("super::"), "{d}");
    }

    fn graph(callee_flag: &str) -> Vec<ScriptFile> {
        vec![
            file(
                "M",
                vec![
                    label(
                        "M_Root",
                        Some(RootKind::Object(1)),
                        vec![i("call", vec![s("M_Callee")]), i("end", vec![])],
                        None,
                    ),
                    label(
                        "M_Callee",
                        None,
                        vec![i("setflag", vec![f(callee_flag)]), i("return", vec![])],
                        None,
                    ),
                    label(
                        "M_Other",
                        Some(RootKind::Object(2)),
                        vec![i("callstd", vec![c("STD_OBTAIN_ITEM")]), i("end", vec![])],
                        None,
                    ),
                ],
                vec![],
            ),
            file(
                "Common_obtain_item",
                vec![label(
                    "Std_ObtainItem",
                    None,
                    vec![i("setflag", vec![f("FLAG_TEMP_1")]), i("return", vec![])],
                    None,
                )],
                vec![],
            ),
        ]
    }

    #[test]
    fn lower_digests_cover_the_call_graph_and_overrides() {
        let a = lower(&graph("FLAG_TEMP_2")).script_digests;
        let b = lower(&graph("FLAG_TEMP_3")).script_digests;
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            ["M_Callee", "M_Other", "M_Root", "Std_ObtainItem"]
        );
        assert_ne!(a["M_Callee"], b["M_Callee"]);
        assert_ne!(
            a["M_Root"], b["M_Root"],
            "a callee's change changes its callers"
        );
        assert_eq!(a["M_Other"], b["M_Other"]);
        // `callstd STD_OBTAIN_ITEM` depends on `Std_ObtainItem` when the IR has it.
        let mut g = graph("FLAG_TEMP_2");
        g[1].labels[0].insns[0] = i("setflag", vec![f("FLAG_TEMP_4")]);
        let c = lower(&g).script_digests;
        assert_ne!(a["M_Other"], c["M_Other"]);
        assert_eq!(a["M_Root"], c["M_Root"]);
        // An override's source is digested, and so is what it names.
        let ov = |src: &str| BTreeMap::from([("M_Other".to_string(), src.to_string())]);
        let d1 = lower_with(&graph("FLAG_TEMP_2"), &ov("M_Callee(ctx).await; // 1")).script_digests;
        let d2 = lower_with(&graph("FLAG_TEMP_2"), &ov("M_Callee(ctx).await; // 2")).script_digests;
        assert_ne!(d1["M_Other"], d2["M_Other"]);
        assert_ne!(d1["M_Other"], a["M_Other"]);
        let d3 = lower_with(&graph("FLAG_TEMP_3"), &ov("M_Callee(ctx).await; // 1")).script_digests;
        assert_ne!(d1["M_Other"], d3["M_Other"], "the override names M_Callee");
    }

    #[test]
    fn lower_overrides_replace_a_script() {
        let o = BTreeMap::from([(
            "M_Callee".to_string(),
            "pub async fn M_Callee(ctx: Ctx) {}".to_string(),
        )]);
        let out = lower_with(&graph("FLAG_TEMP_2"), &o);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Callee"),
            "pub async fn M_Callee(ctx: Ctx) -> Flow {\n    \
             crate::overrides::M_Callee(ctx).await;\n    Flow::Continue\n}\n"
        );
        let bad = BTreeMap::from([("M_Nope".to_string(), String::new())]);
        let g = graph("FLAG_TEMP_2");
        let err = codegen::lower(&g, &index(&g), &support(), &bad).unwrap_err();
        assert!(err.to_string().contains("M_Nope"), "{err}");
    }

    #[test]
    fn lower_trainerbattle_kinds() {
        let tb = |kind: u8, rest: Vec<IrArg>| {
            let mut a = vec![Byte(kind), c("TRAINER_CALVIN_1"), Half(1)];
            a.extend(rest);
            i("trainerbattle", a)
        };
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Single",
                    Some(RootKind::Object(1)),
                    vec![
                        tb(0, vec![t("M_Text_Intro"), t("M_Text_Lose")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Continue",
                    Some(RootKind::Object(2)),
                    vec![
                        tb(1, vec![t("M_Text_Intro"), t("M_Text_Lose"), s("M_After")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_After",
                    None,
                    vec![i("release", vec![]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_NoIntro",
                    Some(RootKind::Object(3)),
                    vec![tb(3, vec![t("M_Text_Lose")]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_Double",
                    Some(RootKind::Object(4)),
                    vec![
                        tb(
                            4,
                            vec![t("M_Text_Intro"), t("M_Text_Lose"), t("M_Text_NotEnough")],
                        ),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Rematch",
                    Some(RootKind::Object(5)),
                    vec![
                        tb(5, vec![t("M_Text_Intro"), t("M_Text_Lose")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Hill",
                    Some(RootKind::Object(6)),
                    vec![tb(12, vec![Raw(0), Raw(0)])],
                    Some("M_After"),
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        let m = &out.modules["M"];
        assert_eq!(
            fn_src(m, "M_Single"),
            "pub async fn M_Single(ctx: Ctx) -> Flow {\n    \
             if !trainer_battle(&ctx, TrainerBattle { kind: 0, trainer: TRAINER_CALVIN_1.0, local_id: 1, \
             intro: Some(M_Text_Intro), lose: M_Text_Lose, not_enough: None, event_script: None, \
             music: true }).await {\n        return Flow::End;\n    }\n    return Flow::End;\n}\n"
        );
        assert!(fn_src(m, "M_Continue").contains(
            "event_script: Some(crate::data::script_ids::M_After), music: false }).await {"
        ));
        let no_intro = fn_src(m, "M_NoIntro");
        assert!(no_intro.contains("kind: 3,") && no_intro.contains("intro: None"));
        assert!(fn_src(m, "M_Double").contains("not_enough: Some(M_Text_NotEnough)"));
        // Every kind has the same shape: the helper owns flags, rematches and facilities.
        for name in [
            "M_Single",
            "M_Continue",
            "M_NoIntro",
            "M_Double",
            "M_Rematch",
        ] {
            let src = fn_src(m, name);
            assert!(src.contains("    if !trainer_battle(&ctx, "), "{src}");
            assert!(!src.contains("trainer_flag"), "{src}");
            assert_eq!(src.matches("return Flow::End;").count(), 2, "{src}");
        }
        // A battle with no lose text cannot be described: reported.
        assert!(fn_src(m, "M_Hill").contains("Report::Unsupported(\"trainerbattle\")"));
        assert_eq!(
            out.trainer_scripts,
            [
                "M_Continue",
                "M_Double",
                "M_NoIntro",
                "M_Rematch",
                "M_Single"
            ]
            .iter()
            .map(|s| (s.to_string(), "TRAINER_CALVIN_1".to_string()))
            .collect::<Vec<_>>()
        );
    }

    /// One site of every command `commands.ron` marks `Native`, in Task B's operand forms.
    fn every_native() -> Vec<ScriptFile> {
        let mv = |name: &str| Movement(name.to_string());
        let map = || c("MAP_LITTLEROOT_TOWN");
        let body = vec![
            i("nop", vec![]),
            i("nop1", vec![]),
            i("loadword", vec![Byte(0), t("N_Text_Hello")]),
            i("callstd", vec![c("MSGBOX_SIGN")]),
            i("loadword", vec![Byte(0), t("N_Text_Hello")]),
            i("callstd_if", vec![Byte(1), Byte(5)]),
            i("callstd", vec![c("STD_OBTAIN_ITEM")]),
            i("callstd", vec![c("STD_FIND_ITEM")]),
            i("callstd", vec![c("STD_REGISTER_MATCH_CALL")]),
            i(
                "setvar",
                vec![v("VAR_OBJ_GFX_ID_0"), c("OBJ_EVENT_GFX_RIVAL_MAY_NORMAL")],
            ),
            i("addvar", vec![v("VAR_0x8004"), Half(1)]),
            i("subvar", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("copyvar", vec![v("VAR_0x8004"), v("VAR_RESULT")]),
            i("setorcopyvar", vec![v("VAR_0x8000"), c("ITEM_POTION")]),
            i("setorcopyvar", vec![v("VAR_0x8001"), v("VAR_0x8006")]),
            i(
                "compare_var_to_value",
                vec![v("VAR_RESULT"), c("SPECIES_TORCHIC")],
            ),
            i("compare_var_to_var", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("call_if", vec![Byte(1), s("N_Sub")]),
            i("special", vec![c("SPECIAL_HealPlayerParty")]),
            i(
                "specialvar",
                vec![v("VAR_RESULT"), c("SPECIAL_HealPlayerParty")],
            ),
            i("waitstate", vec![]),
            i("delay", vec![Half(30)]),
            i("setflag", vec![f("FLAG_SYS_POKEMON_GET")]),
            i("clearflag", vec![f("FLAG_0x500")]),
            i("checkflag", vec![Half(0x20)]),
            i("warp", vec![map(), Byte(1), Half(0xFFFF), Half(0xFFFF)]),
            i(
                "warpsilent",
                vec![map(), Byte(0xFF), Half(5), v("VAR_0x8004")],
            ),
            i("warpdoor", vec![Half(0x0100), Byte(1), Half(0), Half(0)]),
            i("warpteleport", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpspinenter", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpmossdeepgym", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpwhitefade", vec![map(), Byte(0), Half(1), Half(2)]),
            i("setdynamicwarp", vec![map(), Byte(0xFF), Half(3), Half(4)]),
            i(
                "setescapewarp",
                vec![Half(0x7F7F), Byte(1), Half(3), Half(4)],
            ),
            i("getplayerxy", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("getpartysize", vec![]),
            i("additem", vec![c("ITEM_POTION"), Half(1)]),
            i("removeitem", vec![v("VAR_0x8004"), Half(1)]),
            i("checkitemspace", vec![c("ITEM_POTION"), Half(2)]),
            i("checkitem", vec![c("ITEM_POTION"), v("VAR_0x8005")]),
            i("checkitemtype", vec![c("ITEM_POTION")]),
            i(
                "applymovement",
                vec![c("LOCALID_PLAYER"), mv("N_Movement_Walk")],
            ),
            i(
                "applymovementat",
                vec![Half(2), mv("N_Movement_Walk"), map()],
            ),
            i("waitmovement", vec![Half(0)]),
            i("waitmovementat", vec![Half(2), map()]),
            i("removeobject", vec![v("VAR_LAST_TALKED")]),
            i("removeobjectat", vec![Half(2), map()]),
            i("addobject", vec![Half(2)]),
            i("addobjectat", vec![Half(2), map()]),
            i("showobjectat", vec![Half(0xFF), c("MAP_ROUTE101")]),
            i("hideobjectat", vec![Half(2), map()]),
            i("setobjectxy", vec![Half(2), Half(3), Half(4)]),
            i("setobjectxyperm", vec![Half(2), Half(3), Half(4)]),
            i("faceplayer", vec![]),
            i("turnobject", vec![Half(2), Byte(3)]),
            i(
                "setobjectmovementtype",
                vec![Half(2), c("MOVEMENT_TYPE_FACE_LEFT")],
            ),
            i(
                "setobjectmovementtype",
                vec![Half(2), c("MOVEMENT_TYPE_WALK_SEQUENCE_UP_RIGHT_LEFT_DOWN")],
            ),
            i("setobjectmovementtype", vec![Half(2), Byte(0x4c)]),
            i(
                "trainerbattle",
                vec![
                    Byte(0),
                    c("TRAINER_CALVIN_1"),
                    Half(1),
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                ],
            ),
            i("checktrainerflag", vec![c("TRAINER_CALVIN_1")]),
            i("settrainerflag", vec![v("VAR_0x8004")]),
            i("cleartrainerflag", vec![Half(5)]),
            i("message", vec![t("N_Text_Hello")]),
            i("messageautoscroll", vec![t("N_Text_Hello")]),
            i("messageinstant", vec![t("N_Text_Hello")]),
            i("waitmessage", vec![]),
            i("closemessage", vec![]),
            i("lockall", vec![]),
            i("lock", vec![]),
            i("releaseall", vec![]),
            i("release", vec![]),
            i("waitbuttonpress", vec![]),
            i("yesnobox", vec![Byte(20), Byte(8)]),
            i(
                "multichoice",
                vec![Byte(0), Byte(0), c("MULTI_YESNOINFO"), Byte(0)],
            ),
            i(
                "multichoicedefault",
                vec![Byte(20), Byte(6), Byte(1), Byte(1), Byte(1)],
            ),
            i(
                "givemon",
                vec![
                    c("SPECIES_TORCHIC"),
                    Byte(5),
                    c("ITEM_NONE"),
                    Word(0),
                    Word(0),
                    Byte(0),
                ],
            ),
            i("bufferspeciesname", vec![Byte(0), v("VAR_0x8004")]),
            i("bufferleadmonspeciesname", vec![Byte(0)]),
            i("bufferpartymonnick", vec![Byte(1), v("VAR_0x8004")]),
            i("bufferitemname", vec![Byte(0), c("ITEM_POTION")]),
            i(
                "bufferitemnameplural",
                vec![Byte(0), c("ITEM_POTION"), Half(2)],
            ),
            i("buffermovename", vec![Byte(1), c("MOVE_CUT")]),
            i("buffernumberstring", vec![Byte(2), v("VAR_0x8005")]),
            i("bufferstdstring", vec![Byte(0), c("STDSTRING_COOL")]),
            i("bufferstring", vec![Byte(0), t("N_Text_Hello")]),
            i("random", vec![Half(3)]),
            i("addmoney", vec![Word(300), Byte(0)]),
            i("removemoney", vec![Word(300), Byte(0)]),
            i("checkmoney", vec![Raw(300), Byte(0)]),
            i("fadescreen", vec![Byte(1)]),
            i("fadescreenspeed", vec![Byte(0), Byte(4)]),
            i("fadescreenswapbuffers", vec![Byte(3)]),
            i(
                "setrespawn",
                vec![c("HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F")],
            ),
            i("checkplayergender", vec![]),
            i("setmetatile", vec![Half(3), Half(4), Half(0x201), Half(1)]),
            i("opendoor", vec![Half(3), Half(4)]),
            i("waitdooranim", vec![]),
            i("closedoor", vec![v("VAR_0x8004"), Half(4)]),
            i(
                "setwildbattle",
                vec![c("SPECIES_ZIGZAGOON"), Byte(2), c("ITEM_NONE")],
            ),
            i("dowildbattle", vec![]),
            i("goto_if", vec![Byte(5), s("N_Tail")]),
            i("gotostd_if", vec![Byte(0), c("STD_OBTAIN_ITEM")]),
            i("call", vec![s("N_Sub")]),
            i("goto", vec![s("N_Tail")]),
        ];
        vec![file(
            "LittlerootTown",
            vec![
                label("N_Root", Some(RootKind::Object(1)), body, None),
                label("N_Sub", None, vec![i("returnram", vec![])], None),
                label(
                    "N_Tail",
                    None,
                    vec![i("callstd", vec![Byte(2)])],
                    Some("N_Tail2"),
                ),
                label("N_Tail2", None, vec![i("gotostd", vec![Byte(0)])], None),
                label(
                    "N_Ends",
                    Some(RootKind::Bg(0)),
                    vec![i("return", vec![])],
                    None,
                ),
                label(
                    "N_EndRam",
                    Some(RootKind::Bg(1)),
                    vec![i("lock", vec![])],
                    Some("N_EndRam2"),
                ),
                label("N_EndRam2", None, vec![i("endram", vec![])], None),
                label("N_End", Some(RootKind::Bg(2)), vec![i("end", vec![])], None),
            ],
            vec![IrMovement {
                name: "N_Movement_Walk".into(),
                origin: NameOrigin::Decomp,
                actions: vec!["MOVEMENT_ACTION_WALK_NORMAL_UP".into()],
            }],
        )]
    }

    #[test]
    fn lower_every_native_command_without_reports() {
        let files = every_native();
        let used: BTreeSet<&str> = files
            .iter()
            .flat_map(|f| &f.labels)
            .flat_map(|l| &l.insns)
            .map(|i| i.op.as_str())
            .collect();
        let sup = support();
        let native: BTreeSet<&str> = sup
            .commands
            .iter()
            .filter(|(_, c)| **c == Class::Native)
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            used, native,
            "the sample must cover exactly the Native commands"
        );
        let out = lower(&files);
        assert!(out.stats.reports.is_empty(), "{:?}", out.stats.reports);
        assert_eq!(out.stats.by_class.get("Unsupported"), None);
        let src = &out.modules["LittlerootTown"];
        assert!(!src.contains("Report::Unsupported"), "{src}");
        for needle in [
            "msgbox(&ctx, N_Text_Hello, MsgBox::Sign).await;",
            "    msgbox(&ctx, N_Text_Hello, MsgBox::YesNo).await;",
            "ctx.set_var(VAR_RESULT, give_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await as u16).await;",
            // `Std_FindItem` leaves VAR_RESULT to its helper (Task D2).
            "    find_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await;",
            "callstd(&ctx, STD_REGISTER_MATCH_CALL).await;",
            "callstd(&ctx, STD_MSGBOX_NPC).await;",
            "ctx.set_var(VAR_OBJ_GFX_ID_0, OBJ_EVENT_GFX_RIVAL_MAY_NORMAL).await;",
            "ctx.set_var(VAR_0x8004, ctx.var(VAR_0x8004).await.wrapping_sub(ctx.var(VAR_0x8005).await)).await;",
            "ctx.set_cond(ctx.var(VAR_RESULT).await.cmp(&SPECIES_TORCHIC.0));",
            "ctx.set_flag(FlagId(0x500), false).await;",
            "ctx.warp(MAP_LITTLEROOT_TOWN, Some(1), -1, -1).await;",
            "ctx.warp(MAP_LITTLEROOT_TOWN, None, 5, ctx.var(VAR_0x8004).await as i16).await; // warpsilent",
            "set_warp(&ctx, WarpSlot::Dynamic, MAP_LITTLEROOT_TOWN.0, None, 3, 4).await;",
            "set_warp(&ctx, WarpSlot::Escape, MAP_DYNAMIC.0, Some(1), 3, 4).await;",
            "ctx.set_var(VAR_RESULT, ctx.item_add(ItemId(ITEM_POTION), 1).await as u16).await;",
            "ctx.set_var(VAR_RESULT, check_item(&ctx, ITEM_POTION, ctx.var(VAR_0x8005).await).await as u16).await;",
            "ctx.set_var(VAR_RESULT, 0).await; // checkitemtype ITEM_POTION: no pockets before Task H",
            "ctx.move_obj(PLAYER, &N_Movement_Walk).await;",
            "ctx.move_obj(2, &N_Movement_Walk).await; // applymovementat",
            "ctx.remove_object(ctx.var(VAR_LAST_TALKED).await as u8).await;",
            "ctx.show_object(PLAYER).await; // showobjectat",
            "ctx.set_movement_type(2, MovementType::Face(Dir::Left)).await;",
            "ctx.set_movement_type(2, MovementType::Wander).await;",
            "ctx.set_cond(if ctx.flag(trainer_flag(TRAINER_CALVIN_1)).await { Ordering::Equal } else { Ordering::Less });",
            "ctx.set_flag(trainer_flag(TrainerId(ctx.var(VAR_0x8004).await)), true).await;",
            "ctx.message(N_Text_Hello).await; // messageinstant",
            "multichoice(&ctx, 17, 0, 0, 0, true).await",
            "multichoice(&ctx, 1, 20, 6, 1, false).await",
            "ctx.set_var(VAR_RESULT, ctx.give_mon(SPECIES_TORCHIC, 5).await).await;",
            "buffer(&ctx, 0, BufferSrc::Species(SpeciesId(ctx.var(VAR_0x8004).await))).await;",
            "buffer(&ctx, 1, BufferSrc::PartyNick(ctx.var(VAR_0x8004).await as u8)).await;",
            "buffer(&ctx, 1, BufferSrc::Move(MOVE_CUT)).await;",
            "buffer(&ctx, 2, BufferSrc::Number(u32::from(ctx.var(VAR_0x8005).await))).await;",
            "buffer(&ctx, 0, BufferSrc::Std(0)).await;",
            "buffer(&ctx, 0, BufferSrc::Text(N_Text_Hello)).await;",
            "money_add(&ctx, 300).await;",
            "ctx.set_var(VAR_RESULT, money_check(&ctx, 300).await as u16).await;",
            "ctx.report(Report::Degraded(\"fadescreenswapbuffers\")).await;",
            "ctx.set_respawn(HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F).await;",
            "ctx.set_metatile(3, 4, 513, true).await;",
            "ctx.door(ctx.var(VAR_0x8004).await as i16, 4, false).await;",
            "wild = Some((SPECIES_ZIGZAGOON, 2));",
            "if let Some((s, l)) = wild { ctx.battle_wild(s, l, false).await; }",
            "return Flow::Continue;",
        ] {
            assert!(src.contains(needle), "missing {needle:?} in:\n{src}");
        }
    }

    /// With Task H's item helpers (`Support::item_helpers`, which the content build reads from
    /// the prelude), the item commands and `STD_OBTAIN_ITEM` call helpers that write VAR_RESULT
    /// themselves: no `set_var` around them.
    #[test]
    fn lower_item_commands_to_the_item_helpers() {
        let files = every_native();
        let mut sup = support();
        sup.item_helpers = true;
        let out = codegen::lower(&files, &index(&files), &sup, &BTreeMap::new()).unwrap();
        let src = &out.modules["LittlerootTown"];
        for needle in [
            "    give_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await;",
            "    find_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await;",
            "    add_item(&ctx, ITEM_POTION, 1).await;",
            "    remove_item(&ctx, ctx.var(VAR_0x8004).await, 1).await;",
            "    check_item_space(&ctx, ITEM_POTION, 2).await;",
            "    check_item(&ctx, ITEM_POTION, ctx.var(VAR_0x8005).await).await;",
            "    check_item_type(&ctx, ITEM_POTION).await;",
        ] {
            assert!(src.contains(needle), "missing {needle:?} in:\n{src}");
        }
        for gone in [
            "ctx.item_add(",
            "ctx.item_remove(",
            "ctx.item_space(",
            "// checkitemtype",
        ] {
            assert!(!src.contains(gone), "{gone:?} in:\n{src}");
        }
    }

    /// `trainerbattle` types keep the decomp numbering (`include/constants/battle_setup.h`):
    /// 1 `CONTINUE_SCRIPT_NO_MUSIC`, 2 `CONTINUE_SCRIPT`, 7 `REMATCH_DOUBLE`,
    /// 8 `CONTINUE_SCRIPT_DOUBLE_NO_MUSIC`.
    #[test]
    fn trainer_battle_kinds_use_the_decomp_numbering() {
        for (k, name) in [
            (1, "TRAINER_BATTLE_CONTINUE_SCRIPT_NO_MUSIC"),
            (2, "TRAINER_BATTLE_CONTINUE_SCRIPT"),
            (7, "TRAINER_BATTLE_REMATCH_DOUBLE"),
            (8, "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE_NO_MUSIC"),
        ] {
            assert_eq!(codegen::lower::TRAINER_BATTLE_KINDS[k], name);
        }
    }

    #[test]
    fn lower_output_compiles() {
        let mut files = every_native();
        files.extend(kid());
        files.extend(graph("FLAG_TEMP_2"));
        let mut tb = vec![];
        for (k, rest) in [
            (1u8, vec![t("N_Text_Hello"), t("N_Text_Hello"), s("N_Sub")]),
            (3, vec![t("N_Text_Hello")]),
            (
                4,
                vec![t("N_Text_Hello"), t("N_Text_Hello"), t("N_Text_Hello")],
            ),
            (5, vec![t("N_Text_Hello"), t("N_Text_Hello")]),
            (
                8,
                vec![
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                    s("N_Sub"),
                ],
            ),
            (10, vec![Raw(0), t("N_Text_Hello")]),
        ] {
            let mut a = vec![Byte(k), Half(0x10), Half(1)];
            a.extend(rest);
            tb.push(i("trainerbattle", a));
        }
        tb.push(i("pokemart", vec![Raw(0x1234)]));
        tb.push(i("checkcoins", vec![v("VAR_0x8004")]));
        tb.push(i("playse", vec![Half(5)]));
        tb.push(i("special", vec![c("SPECIAL_ShouldTryRematchBattle")]));
        tb.push(i("dowildbattle", vec![]));
        files.push(file(
            "Other",
            vec![
                label("O_Root", Some(RootKind::Coord(0)), tb, Some("O_Unknown")),
                label("O_Unknown", None, vec![i("lock", vec![])], None),
                label(
                    "O_Cross",
                    Some(RootKind::Coord(1)),
                    vec![i("call", vec![s("N_Sub")]), i("goto", vec![s("M_Callee")])],
                    None,
                ),
                // A tail-call group across modules, led by `G_Loop`.
                label(
                    "O_Loop",
                    Some(RootKind::Coord(2)),
                    vec![
                        i(
                            "setwildbattle",
                            vec![c("SPECIES_ZIGZAGOON"), Byte(2), Byte(0)],
                        ),
                        i("goto", vec![s("G_Loop")]),
                    ],
                    None,
                ),
            ],
            vec![],
        ));
        files.push(file(
            "G",
            vec![label(
                "G_Loop",
                Some(RootKind::Coord(0)),
                vec![
                    i("random", vec![Half(2)]),
                    i("goto_if", vec![Byte(1), s("O_Loop")]),
                    i("end", vec![]),
                ],
                None,
            )],
            vec![],
        ));
        let o = BTreeMap::from([("M_Other".to_string(), String::new())]);
        let out = lower_with(&files, &o);
        assert!(out.stats.reports.contains_key("unknown opcode"));
        assert_eq!(out.stats.tail_call_groups, 1);
        assert!(
            out.modules["Other"].contains("crate::scripts::G::G_Loop__tail(ctx, 1).await"),
            "{}",
            out.modules["Other"]
        );
        compile_lowered("m8", &out, &index(&files));
    }

    /// Builds a scratch crate laid out as `content` is after Wave 0 and Task D2 (`extern crate
    /// alloc`, `crate::prelude`, `crate::overrides`, `crate::data` holding the dispatch and
    /// `crate::scripts` one module per IR module) against the real `engine`, and runs
    /// `cargo check` with warnings denied.
    fn compile_lowered(tag: &str, out: &Lowered, ix: &ScriptIndex) {
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = tmp.join(format!("lowered-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let engine = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../engine")
            .canonicalize()
            .unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"stub_lowered\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
                 publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n\
                 engine = {{ path = {:?} }}\n\n[workspace]\n",
                engine.to_str().unwrap()
            ),
        )
        .unwrap();
        let mut prelude = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stub_prelude/prelude.rs"),
        )
        .unwrap();
        prelude += STUB_PRELUDE_M8;
        // Wave 0's `set_warp` body and helper modules, as `content::prelude` has them.
        let content_prelude = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/src/prelude");
        let mod_rs = std::fs::read_to_string(content_prelude.join("mod.rs")).unwrap();
        let start = mod_rs.find("pub async fn set_warp(").unwrap();
        let end = start + mod_rs[start..].find("\n}\n").unwrap() + 3;
        prelude += &mod_rs[start..end];
        prelude += &consts(ix.texts.keys());
        std::fs::create_dir_all(dir.join("prelude")).unwrap();
        std::fs::write(dir.join("prelude/mod.rs"), prelude).unwrap();
        for m in M8_HELPER_MODULES {
            std::fs::copy(
                content_prelude.join(format!("{m}.rs")),
                dir.join(format!("prelude/{m}.rs")),
            )
            .unwrap();
        }
        std::fs::write(dir.join("dispatch.rs"), &out.dispatch).unwrap();
        let mut overrides = String::new();
        for (m, src) in &out.modules {
            std::fs::write(dir.join(format!("{m}.rs")), src).unwrap();
            for line in src.lines() {
                if let Some(rest) = line.trim().strip_prefix("crate::overrides::") {
                    let name = rest.split('(').next().unwrap();
                    overrides += &format!(
                        "pub async fn {name}(ctx: crate::prelude::Ctx) {{ ctx.log(\"override\").await; }}\n"
                    );
                }
            }
        }
        let mut lib = String::from(
            "extern crate alloc;\n\
             #[allow(non_upper_case_globals)]\npub mod prelude;\n\
             #[allow(non_snake_case)]\npub mod overrides {\n",
        );
        lib += &overrides;
        lib += "}\npub mod data {\n    include!(\"dispatch.rs\");\n}\npub mod scripts {\n";
        for m in out.modules.keys() {
            lib += &format!(
                "    #[allow(non_snake_case, non_upper_case_globals, unused_mut, unused_variables, \
                 unused_assignments, unreachable_code, clippy::all)] pub mod {m} {{ use crate::prelude::*; \
                 include!(\"{m}.rs\"); }}\n"
            );
        }
        lib += "}\n";
        std::fs::write(dir.join("lib.rs"), lib).unwrap();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let res = Command::new(cargo)
            .args(["check", "--offline", "--quiet", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", tmp.join("stub-target"))
            .env("RUSTFLAGS", "-D warnings")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&res.stderr);
        assert!(
            res.status.success(),
            "{tag}: lowered code does not compile:\n{stderr}"
        );
    }

    /// The prelude's constants (as `content`'s `consts.rs` types them) and the text ids.
    fn consts<'a>(texts: impl Iterator<Item = &'a String>) -> String {
        let mut s = String::new();
        let mut seen = BTreeSet::new();
        let mut table = |s: &mut String, t: &names::Table, ty: &str, wrap: bool| {
            for (v, n) in t.iter() {
                if !codegen::is_ident(n) {
                    continue;
                }
                let n = if n.starts_with(t.prefix) {
                    n.to_string()
                } else {
                    format!("{}{n}", t.prefix)
                };
                if seen.insert(n.clone()) {
                    let v = if wrap {
                        format!("{ty}({v:#x})")
                    } else {
                        format!("{v:#x}")
                    };
                    *s += &format!("pub const {n}: {ty} = {v};\n");
                }
            }
        };
        table(&mut s, &names::FLAGS, "FlagId", true);
        table(&mut s, &names::VARS, "VarId", true);
        table(&mut s, &names::SPECIALS, "u16", false);
        table(&mut s, &names::STD_SCRIPTS, "u8", false);
        table(&mut s, &names::OBJ_EVENT_GFX, "u16", false);
        table(&mut s, &names::ITEMS, "u16", false);
        table(&mut s, &names::SPECIES, "SpeciesId", true);
        table(&mut s, &names::MOVES, "MoveId", true);
        table(&mut s, &names::TRAINERS, "TrainerId", true);
        table(&mut s, &names::HEAL_LOCATIONS, "u8", false);
        for m in names::MAPS {
            let id = u16::from(m.num) | u16::from(m.group) << 8;
            s += &format!("pub const {}: MapId = MapId({id:#x});\n", m.constant);
        }
        s += "pub const MAP_DYNAMIC: MapId = MapId(0x7f7f);\n";
        for (i, k) in texts.enumerate() {
            s += &format!("pub const {k}: TextId = TextId({i});\n");
        }
        s
    }

    /// The `content::prelude` modules holding Wave 0's Milestone 8 helpers, copied as they are.
    const M8_HELPER_MODULES: [&str; 4] = ["items", "menu", "text", "trainer"];

    /// The Milestone 8 prelude over the engine: its types, and Wave 0's helper modules
    /// ([`M8_HELPER_MODULES`]) and `set_warp` (appended by `compile_lowered`).
    const STUB_PRELUDE_M8: &str = r#"
pub use engine::battle::BattleOutcome;
pub use engine::data::{ItemId, MoveId, ScriptId};
pub use engine::script::{BufferSrc, Report, WarpSlot};

mod items;
mod menu;
mod text;
mod trainer;

pub use items::*;
pub use menu::*;
pub use text::*;
pub use trainer::*;


"#;
}
