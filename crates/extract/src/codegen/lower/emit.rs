//! Rust source of functions, blocks, commands, movements and the dispatch.

use super::*;

/// Per-function emission state.
pub(super) struct Fx<'a> {
    module: &'a str,
    /// Block -> arm index (entry is 0).
    local: BTreeMap<usize, usize>,
    looped: bool,
}

impl<'a> Low<'a> {
    /// `name` of `module` as seen from `from_module`: bare in the same module, else absolute.
    fn item_path(from_module: &str, module: &str, name: &str) -> String {
        if module == from_module {
            name.to_string()
        } else {
            format!("{SCRIPTS}::{module}::{name}")
        }
    }

    pub(super) fn fn_path(&self, from_module: &str, f: usize) -> String {
        let l = &self.labs[f];
        Self::item_path(from_module, l.module, l.name)
    }

    pub(super) fn movement_path(&self, from_module: &str, m: &str) -> Option<String> {
        let (module, mv) = self.movements.get(m)?;
        Some(Self::item_path(from_module, module, &mv.name))
    }

    /// The blocks of a group's loop: its members' regions, in member order.
    fn group_blocks(&self, g: usize) -> Vec<usize> {
        self.groups[&g]
            .iter()
            .flat_map(|m| self.regions[m].iter().copied())
            .collect()
    }

    /// A function: its own body, or for a tail-call group member, an entry into the group's
    /// loop at its first block.
    pub(super) fn emit_fn(&mut self, f: usize) -> Result<String> {
        let name = self.labs[f].name;
        if let Some(&g) = self.group_of.get(&f) {
            let pc = self
                .group_blocks(g)
                .iter()
                .position(|&b| b == f)
                .unwrap_or(0);
            let l = &self.labs[g];
            let path = Self::item_path(self.labs[f].module, l.module, &group_name(l.name));
            return Ok(format!(
                "pub async fn {name}(ctx: Ctx) -> Flow {{\n    {path}(ctx, {pc}).await\n}}\n"
            ));
        }
        let blocks = self.regions[&f].clone();
        let jumps_home = blocks.iter().any(|&b| {
            self.labs[b]
                .edges
                .iter()
                .any(|&(k, t)| k == Edge::Jump && t == f)
        });
        let looped = blocks.len() > 1 || jumps_home;
        let head = format!("pub async fn {name}(ctx: Ctx) -> Flow {{\n");
        self.emit_body(head, self.labs[f].module, &blocks, looped, true)
    }

    /// The loop shared by the members of the tail-call group led by `g`.
    pub(super) fn emit_group(&mut self, g: usize) -> Result<String> {
        let blocks = self.group_blocks(g);
        let name = group_name(self.labs[g].name);
        let members: Vec<&str> = self.groups[&g].iter().map(|&m| self.labs[m].name).collect();
        let head = format!(
            "/// The tail-call group of {}.\npub async fn {name}(ctx: Ctx, mut pc: u16) -> Flow {{\n",
            members.join(", ")
        );
        self.emit_body(head, self.labs[g].module, &blocks, true, false)
    }

    /// `head`, then `blocks` (entry first) as straight code or as `loop { match pc }`, then `}`.
    fn emit_body(
        &mut self,
        head: String,
        module: &'a str,
        blocks: &[usize],
        looped: bool,
        declare_pc: bool,
    ) -> Result<String> {
        let local: BTreeMap<usize, usize> =
            blocks.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        let wild = blocks
            .iter()
            .any(|&b| self.labs[b].insns.iter().any(|i| i.op == "setwildbattle"));
        let fx = Fx {
            module,
            local,
            looped,
        };
        let mut s = head;
        if wild {
            s.push_str("    let mut wild: Option<(SpeciesId, u8)> = None;\n");
            // Gen3 `gBattleOutcome` of the function's `dowildbattle`, for `GetBattleOutcome`.
            s.push_str("    let mut outcome: u16 = 0;\n");
        }
        if looped {
            if declare_pc {
                s.push_str("    let mut pc: u16 = 0;\n");
            }
            s.push_str("    loop {\n        match pc {\n");
            for (i, &b) in blocks.iter().enumerate() {
                writeln!(s, "            {i} => {{")?;
                writeln!(s, "                // {}", self.labs[b].name)?;
                for line in self.emit_block(b, &fx, wild)? {
                    writeln!(s, "                {line}")?;
                }
                s.push_str("            }\n");
            }
            s.push_str("            _ => return Flow::End,\n        }\n    }\n");
        } else {
            for line in self.emit_block(blocks[0], &fx, wild)? {
                writeln!(s, "    {line}")?;
            }
        }
        s.push_str("}\n");
        Ok(s)
    }

    pub(super) fn emit_block(&mut self, b: usize, fx: &Fx, wild: bool) -> Result<Vec<String>> {
        let insns = self.labs[b].insns;
        let end = self.labs[b].end;
        let mut out = Vec::new();
        let mut st = BlockState {
            moved: false,
            trainer: None,
            word0: None,
            wild,
        };
        for (i, insn) in insns.iter().enumerate() {
            // `loadword 0, <text>` feeding a later `callstd MSGBOX_*` of the block is the
            // `msgbox` macro; it emits nothing itself.
            if insn.op == "loadword"
                && self.class("loadword") == Class::Native
                && let [a, IrArg::Text(t)] = &insn.args[..]
                && u8_lit(a) == Some(0)
                && is_ident(t)
            {
                self.count_class(Class::Native);
                st.word0 = Some(t.clone());
                let used = insns[i + 1..].iter().any(|n| {
                    matches!(
                        n.op.as_str(),
                        "callstd" | "gotostd" | "callstd_if" | "gotostd_if"
                    ) && n.args.last().and_then(std_index).is_some_and(is_msgbox)
                });
                if !used {
                    out.push(format!("// loadword 0, {t}"));
                }
                continue;
            }
            self.emit_insn(insn, fx, &mut st, &mut out)?;
        }
        match end {
            End::Terminated => {
                if !out
                    .last()
                    .is_some_and(|l| l.starts_with("return ") || l == "continue;")
                {
                    out.push("return Flow::End;".to_string());
                }
            }
            End::Fallthrough(t) => out.extend(self.jump(t, fx)),
            End::Unknown => {
                self.count_report("unknown opcode");
                out.push("ctx.report(Report::Unsupported(\"unknown opcode\")).await;".to_string());
                out.push("return Flow::End;".to_string());
            }
        }
        Ok(out)
    }

    /// Lines that continue at `t`: a local block or a tail call.
    pub(super) fn jump(&self, t: usize, fx: &Fx) -> Vec<String> {
        match fx.local.get(&t) {
            Some(i) if fx.looped => vec![format!("pc = {i};"), "continue;".to_string()],
            _ => vec![format!(
                "return Box::pin({}(ctx.clone())).await;",
                self.fn_path(fx.module, t)
            )],
        }
    }

    pub(super) fn call(&mut self, t: usize, fx: &Fx) -> Vec<String> {
        let call = format!(
            "Box::pin({}(ctx.clone())).await",
            self.fn_path(fx.module, t)
        );
        if self.can_end.contains(&t) {
            self.stats.end_in_callee += 1;
            vec![
                format!("if {call} == Flow::End {{"),
                "    return Flow::End;".to_string(),
                "}".to_string(),
            ]
        } else {
            vec![format!("{call};")]
        }
    }

    /// `ctx.report(Report::Unsupported(what))`, and `return Flow::End` after a terminator.
    /// A prelude helper that answers in `VAR_RESULT` (`multichoice`, `money_check`): it writes
    /// it itself, so the call is bare.
    fn result_call(call: String) -> String {
        format!("{call};")
    }

    pub(super) fn report(&mut self, insn: &IrInsn, what: &str, out: &mut Vec<String>) {
        self.count_report(what);
        out.push(format!(
            "ctx.report(Report::Unsupported(\"{what}\")).await;{}",
            args_comment(&insn.args)
        ));
        if TERMINATORS.contains(&insn.op.as_str()) {
            out.push("return Flow::End;".to_string());
        }
    }

    pub(super) fn emit_insn(
        &mut self,
        insn: &IrInsn,
        fx: &Fx,
        st: &mut BlockState,
        out: &mut Vec<String>,
    ) -> Result<()> {
        let class = self.class(&insn.op);
        if class != Class::Native {
            self.count_class(class);
        }
        match class {
            Class::Native => {
                let before = out.len();
                if self.native(insn, fx, st, out)? {
                    self.count_class(Class::Native);
                } else {
                    // Operands that do not fit the arm: reported like an unsupported command.
                    out.truncate(before);
                    self.count_class(Class::Unsupported);
                    self.report(insn, &insn.op, out);
                }
            }
            Class::Ignore => {
                let args: Vec<String> = insn.args.iter().map(arg_str).collect();
                out.push(
                    format!("// {} {}", insn.op, args.join(", "))
                        .trim_end()
                        .to_string(),
                );
            }
            // No report: the default is the documented behaviour (`commands.ron`).
            Class::Default(v) => {
                let dest = match (insn.op.as_str(), insn.args.first()) {
                    ("checkcoins", Some(a)) => var_target(a),
                    _ => Some(var(VAR_RESULT)),
                };
                let op = &insn.op;
                match dest {
                    Some(d) => out.push(format!("ctx.set_var({d}, {v}).await; // {op} (default)")),
                    None => out.push(format!("// {op}: default {v}, but no result var")),
                }
                if TERMINATORS.contains(&op.as_str()) {
                    out.push("return Flow::End;".to_string());
                }
            }
            Class::Unsupported => self.report(insn, &insn.op, out),
        }
        Ok(())
    }

    /// The lowering of a `Native` command; false when its operands do not fit.
    pub(super) fn native(
        &mut self,
        insn: &IrInsn,
        fx: &Fx,
        st: &mut BlockState,
        out: &mut Vec<String>,
    ) -> Result<bool> {
        use IrArg::*;
        let args = &insn.args[..];
        let op = insn.op.as_str();
        let r = var(VAR_RESULT);
        match (op, args) {
            ("nop" | "nop1", _) => out.push(format!("// {op}")),
            ("end" | "endram", _) => out.push("return Flow::End;".to_string()),
            ("return" | "returnram", _) => out.push("return Flow::Continue;".to_string()),
            ("call", [Script(t)]) => {
                let t = self.at[t.as_str()];
                out.extend(self.call(t, fx));
            }
            ("goto", [Script(t)]) => out.extend(self.jump(self.at[t.as_str()], fx)),
            ("goto_if" | "call_if", [c, Script(t)]) => {
                let Some(c) = condition(c) else {
                    return Ok(false);
                };
                let t = self.at[t.as_str()];
                let body = if op == "goto_if" {
                    self.jump(t, fx)
                } else {
                    self.call(t, fx)
                };
                out.push(format!("if {c} {{"));
                out.extend(body.into_iter().map(|l| format!("    {l}")));
                out.push("}".to_string());
            }
            ("callstd" | "gotostd", [n]) => {
                let Some(lines) = self.callstd(n, st) else {
                    return Ok(false);
                };
                out.extend(lines);
                if op == "gotostd" {
                    out.push("return Flow::Continue;".to_string());
                }
            }
            ("callstd_if" | "gotostd_if", [c, n]) => {
                let (Some(c), Some(lines)) = (condition(c), self.callstd(n, st)) else {
                    return Ok(false);
                };
                out.push(format!("if {c} {{"));
                out.extend(lines.into_iter().map(|l| format!("    {l}")));
                if op == "gotostd_if" {
                    out.push("    return Flow::Continue;".to_string());
                }
                out.push("}".to_string());
            }
            // `loadword` other than the msgbox text (see `emit_block`): only a register.
            ("loadword", _) => return Ok(false),
            ("setvar", [v, x]) => {
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, false)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({v}, {x}).await;"));
            }
            ("addvar" | "subvar", [v, x]) => {
                // Gen3 `addvar` adds the raw operand; `subvar` subtracts its `VarGet`.
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, op == "subvar")) else {
                    return Ok(false);
                };
                let f = if op == "addvar" {
                    "wrapping_add"
                } else {
                    "wrapping_sub"
                };
                out.push(format!(
                    "ctx.set_var({v}, ctx.var({v}).await.{f}({x})).await;"
                ));
            }
            ("copyvar", [a, c]) => {
                // A source below VARS_START has no var: Gen3 dereferences NULL there. The BIOS
                // is read-protected, so the load returns BIOS open bus (the last opcode the
                // BIOS fetched, after an interrupt or a SWI: non-zero in practice, and not a
                // value the engine models). The decomp's UBFIX, and this lowering, copy the id
                // itself (Granite Cave's `copyvar VAR_ICE_STEP_COUNT, 1`, which must leave the
                // count non-zero; both give that, though not the same number).
                if let Some(id) = literal_var(c) {
                    let Some(a) = var_target(a) else {
                        return Ok(false);
                    };
                    out.push(format!(
                        "ctx.set_var({a}, {id}).await; // copyvar of a literal"
                    ));
                    return Ok(true);
                }
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({a}, ctx.var({c}).await).await;"));
            }
            ("setorcopyvar", [a, x]) => {
                let (Some(a), Some(x)) = (var_target(a), u16_expr(x, true)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({a}, {x}).await;"));
            }
            ("compare_var_to_value", [v, x]) => {
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, false)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_cond(ctx.var({v}).await.cmp(&{x}));"));
            }
            ("compare_var_to_var", [a, c]) => {
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_cond(ctx.var({a}).await.cmp(&ctx.var({c}).await));"
                ));
            }
            ("special", [s]) => return Ok(self.special(s, None, st, out)),
            ("specialvar", [v, s]) => {
                let Some(v) = var_target(v) else {
                    return Ok(false);
                };
                return Ok(self.special(s, Some(v), st, out));
            }
            ("waitstate", _) => out.push("ctx.wait_state().await;".to_string()),
            ("delay", [n]) => {
                let Some(n) = u16_expr(n, false) else {
                    return Ok(false);
                };
                out.push(format!("ctx.delay({n}).await;"));
            }
            ("setflag" | "clearflag", [f]) => {
                let Some(f) = flag_arg(f) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_flag({f}, {}).await;", op == "setflag"));
            }
            ("checkflag", [f]) => {
                let Some(f) = flag_arg(f) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_cond({});",
                    flag_cond(&format!("ctx.flag({f}).await"))
                ));
            }
            (
                "warp" | "warpsilent" | "warpdoor" | "warpteleport" | "warpspinenter"
                | "warpmossdeepgym" | "warpwhitefade",
                [m, w, x, y],
            ) => {
                let (Some(m), Some(w), Some(x), Some(y)) =
                    (map_arg(m), warp_id(w), coord(x), coord(y))
                else {
                    return Ok(false);
                };
                let note = if op == "warp" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                out.push(format!("ctx.warp({m}, {w}, {x}, {y}).await;{note}"));
            }
            ("warphole", [m]) => {
                // Gen3 `ScrCmd_warphole`: MAP_UNDEFINED falls to the map's `setholewarp` map
                // (`GameData::hole_warp`).
                let m = if u16_lit(m) == Some(0xFFFF)
                    || matches!(m, IrArg::Const { name, .. } if name == "MAP_UNDEFINED")
                {
                    "None".to_string()
                } else {
                    let Some(m) = map_arg(m) else {
                        return Ok(false);
                    };
                    format!("Some({m})")
                };
                out.push(format!("ctx.warp_hole({m}).await;"));
            }
            ("setdynamicwarp" | "setescapewarp", [m, w, x, y]) => {
                let (Some(m), Some(w), Some(x), Some(y)) =
                    (map_arg(m), warp_id(w), coord(x), coord(y))
                else {
                    return Ok(false);
                };
                let slot = if op == "setdynamicwarp" {
                    "Dynamic"
                } else {
                    "Escape"
                };
                out.push(format!(
                    "set_warp(&ctx, WarpSlot::{slot}, {m}.0, {w}, {x}, {y}).await;"
                ));
            }
            ("getplayerxy", [a, c]) => {
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!(
                    "{{ let (x, y) = ctx.player_pos().await; ctx.set_var({a}, x as u16).await; ctx.set_var({c}, y as u16).await; }}"
                ));
            }
            ("getpartysize", _) => {
                out.push(format!("ctx.set_var({r}, ctx.party_size().await).await;"))
            }
            ("additem" | "removeitem" | "checkitemspace" | "checkitem", [i, q]) => {
                let (Some(i), Some(q)) = (u16_expr(i, true), u16_expr(q, true)) else {
                    return Ok(false);
                };
                // The helpers set VAR_RESULT and cast the quantity to u8 themselves.
                let f = match op {
                    "additem" => "add_item",
                    "removeitem" => "remove_item",
                    "checkitemspace" => "check_item_space",
                    _ => "check_item",
                };
                out.push(format!("{f}(&ctx, {i}, {q}).await;"));
            }
            ("checkitemtype", [i]) => {
                let Some(i) = u16_expr(i, true) else {
                    return Ok(false);
                };
                out.push(format!("check_item_type(&ctx, {i}).await;"));
            }
            ("applymovement", [id, Movement(m)]) => {
                let (Some(id), Some(m)) = (local_id(id), self.movement_path(fx.module, m)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.move_obj({id}, &{m}).await;"));
                st.moved = true;
            }
            ("applymovementat", [id, Movement(m), at]) => {
                let (Some(lid), Some(m)) = (local_id(id), self.movement_path(fx.module, m)) else {
                    return Ok(false);
                };
                // The player, on any map; another object, of the named map (the previous one
                // after a crossing mid-script, Gen3 `ScriptMovement_StartObjectMovementScript`).
                if is_player(id) {
                    out.push(format!(
                        "ctx.move_obj({lid}, &{m}).await; // applymovementat"
                    ));
                } else {
                    let Some(at) = map_arg(at) else {
                        return Ok(false);
                    };
                    out.push(format!("ctx.move_obj_at({lid}, {at}, &{m}).await;"));
                }
                st.moved = true;
            }
            ("waitmovement", [id]) => {
                let Some(line) = self.wait_move(id, st) else {
                    return Ok(false);
                };
                out.push(line);
            }
            ("waitmovementat", [id, at]) => {
                if is_player(id) {
                    let Some(line) = self.wait_move(id, st) else {
                        return Ok(false);
                    };
                    out.push(format!("{line} // waitmovementat"));
                } else {
                    let (Some(lid), Some(at)) = (local_id(id), map_arg(at)) else {
                        return Ok(false);
                    };
                    if lid == "0" && !st.moved {
                        self.stats.unresolved_waitmovement += 1;
                    }
                    out.push(format!("ctx.wait_move_at({lid}, {at}).await;"));
                }
            }
            ("addobject" | "removeobject", [id]) => {
                let Some(id) = local_id(id) else {
                    return Ok(false);
                };
                let f = if op == "addobject" { "add" } else { "remove" };
                out.push(format!("ctx.{f}_object({id}).await;"));
            }
            ("showobjectat" | "hideobjectat", [id, at]) if !is_player(id) => {
                // Gen3 `SetObjectInvisibility`: the named map's object (the previous map's after
                // a crossing mid-script).
                let (Some(lid), Some(at)) = (local_id(id), map_arg(at)) else {
                    return Ok(false);
                };
                let f = &op[..op.len() - "objectat".len()];
                out.push(format!("ctx.{f}_object_at({lid}, {at}).await;"));
            }
            ("addobjectat" | "removeobjectat" | "showobjectat" | "hideobjectat", [id, at]) => {
                let Some(lid) = local_id(id) else {
                    return Ok(false);
                };
                if !at_here(id, at, fx.module) {
                    return Ok(false);
                }
                let f = &op[..op.len() - "objectat".len()];
                out.push(format!("ctx.{f}_object({lid}).await; // {op}"));
            }
            ("setobjectxy" | "setobjectxyperm", [id, x, y]) => {
                let (Some(id), Some(x), Some(y)) = (local_id(id), coord(x), coord(y)) else {
                    return Ok(false);
                };
                let f = if op == "setobjectxy" {
                    "set_object_pos"
                } else {
                    "set_object_template_pos"
                };
                out.push(format!("ctx.{f}({id}, {x}, {y}).await;"));
            }
            ("faceplayer", _) => out.push("ctx.face_player().await;".to_string()),
            ("turnobject", [id, d]) => {
                let (Some(id), Some(d)) = (local_id(id), u8_lit(d).and_then(dir)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.turn({id}, {d}).await;"));
            }
            ("setobjectmovementtype", [id, t]) => {
                let (Some(id), Some(t)) = (local_id(id), movement_type(t)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_movement_type({id}, {t}).await;"));
            }
            ("trainerbattle", [k, tr, local, rest @ ..]) => {
                let ok = self.trainerbattle(k, tr, local, rest, out)?;
                if ok {
                    st.trainer = trainer_arg(tr, false);
                }
                return Ok(ok);
            }
            ("checktrainerflag" | "settrainerflag" | "cleartrainerflag", [t]) => {
                let Some(t) = trainer_arg(t, true) else {
                    return Ok(false);
                };
                let f = format!("trainer_flag({t})");
                out.push(match op {
                    "checktrainerflag" => {
                        format!(
                            "ctx.set_cond({});",
                            flag_cond(&format!("ctx.flag({f}).await"))
                        )
                    }
                    "settrainerflag" => format!("ctx.set_flag({f}, true).await;"),
                    _ => format!("ctx.set_flag({f}, false).await;"),
                });
            }
            ("waitmessage", _) => out.push("ctx.wait_message().await;".to_string()),
            ("message" | "messageautoscroll" | "messageinstant", [Text(t)]) if is_ident(t) => {
                let note = if op == "message" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                out.push(format!("ctx.message({t}).await;{note}"));
            }
            ("closemessage", _) => out.push("ctx.close_message().await;".to_string()),
            // Gen3 `ShowPokenavFieldMessage`: the call in the field message box (no wait).
            ("pokenavcall", [Text(t)]) if is_ident(t) => {
                out.push(format!("ctx.message({t}).await; // pokenavcall"));
            }
            ("checkpartymove", [m]) => {
                let Some(m) = move_arg(m) else {
                    return Ok(false);
                };
                out.push(format!("check_party_move(&ctx, {m}).await;"));
            }
            ("lock", _) => out.push("ctx.lock().await;".to_string()),
            ("lockall", _) => out.push("ctx.lock_all().await;".to_string()),
            ("release", _) => out.push("ctx.release().await;".to_string()),
            ("releaseall", _) => out.push("ctx.release_all().await;".to_string()),
            ("waitbuttonpress", _) => out.push("ctx.wait_button().await;".to_string()),
            ("yesnobox", _) => out.push(format!(
                "ctx.set_var({r}, ctx.yes_no().await as u16).await;"
            )),
            ("multichoice", [x, y, list, ignore_b]) => {
                let (Some(x), Some(y), Some(list), Some(ignore_b)) =
                    (u8_lit(x), u8_lit(y), u8_lit(list), u8_lit(ignore_b))
                else {
                    return Ok(false);
                };
                let call = format!(
                    "multichoice(&ctx, {list}, {x}, {y}, 0, {}).await",
                    ignore_b == 0
                );
                out.push(Self::result_call(call));
            }
            ("multichoicedefault", [x, y, list, default, ignore_b]) => {
                let (Some(x), Some(y), Some(list), Some(default), Some(ignore_b)) = (
                    u8_lit(x),
                    u8_lit(y),
                    u8_lit(list),
                    u8_lit(default),
                    u8_lit(ignore_b),
                ) else {
                    return Ok(false);
                };
                let call = format!(
                    "multichoice(&ctx, {list}, {x}, {y}, {default}, {}).await",
                    ignore_b == 0
                );
                out.push(Self::result_call(call));
            }
            ("givemon", [s, level, item, ..]) => {
                let (Some(s), Some(level)) = (species_arg(s, true), u8_lit(level)) else {
                    return Ok(false);
                };
                let note = match u16_expr(item, true) {
                    Some(i) if i == "0" => String::new(),
                    Some(i) => format!(" // held item {i} not given"),
                    None => " // held item not given".to_string(),
                };
                out.push(format!(
                    "ctx.set_var({r}, ctx.give_mon({s}, {level}).await).await;{note}"
                ));
            }
            (
                "bufferspeciesname"
                | "bufferpartymonnick"
                | "bufferitemname"
                | "buffermovename"
                | "buffernumberstring"
                | "bufferstdstring"
                | "bufferstring"
                | "bufferitemnameplural",
                [slot, v, ..],
            ) => {
                let Some(slot) = u8_lit(slot) else {
                    return Ok(false);
                };
                let src = match op {
                    "bufferspeciesname" => species_arg(v, true).map(|s| format!("Species({s})")),
                    "bufferpartymonnick" => {
                        u16_expr(v, true).map(|n| format!("PartyNick({})", as_u8(&n)))
                    }
                    "bufferitemname" | "bufferitemnameplural" => {
                        u16_expr(v, true).map(|i| format!("Item(ItemId({i}))"))
                    }
                    "buffermovename" => move_arg(v).map(|m| format!("Move({m})")),
                    "buffernumberstring" => {
                        // A literal is untyped (`u32::from(10)` would infer `i32`).
                        u16_expr(v, true).map(|n| match n.parse::<u32>() {
                            Ok(n) => format!("Number({n})"),
                            Err(_) => format!("Number(u32::from({n}))"),
                        })
                    }
                    "bufferstdstring" => u16_expr(v, true).map(|n| format!("Std({})", as_u8(&n))),
                    _ => match v {
                        Text(t) if is_ident(t) => Some(format!("Text({t})")),
                        _ => None,
                    },
                };
                let Some(src) = src else {
                    return Ok(false);
                };
                let note = if op == "bufferitemnameplural" {
                    " // bufferitemnameplural: the singular name"
                } else {
                    ""
                };
                out.push(format!(
                    "buffer(&ctx, {slot}, BufferSrc::{src}).await;{note}"
                ));
            }
            ("bufferleadmonspeciesname", [slot]) => {
                let Some(slot) = u8_lit(slot) else {
                    return Ok(false);
                };
                out.push(format!(
                    "buffer(&ctx, {slot}, BufferSrc::LeadSpecies).await;"
                ));
            }
            // The list by its absolute path (a shared shape compiles in another module).
            ("pokemart", [Items(l)]) => {
                let Some(m) = self.lists.get(l.as_str()) else {
                    return Ok(false);
                };
                out.push(format!("mart(&ctx, &{SCRIPTS}::{m}::{l}).await;"));
            }
            ("random", [n]) => {
                let Some(n) = u16_expr(n, true) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({r}, ctx.random({n}).await).await;"));
            }
            ("addmoney" | "removemoney" | "checkmoney", [n, ignore]) => {
                let (Some(n), Some(ignore)) = (u32_lit(n), u8_lit(ignore)) else {
                    return Ok(false);
                };
                if ignore != 0 {
                    out.push(format!("// {op} {n}, ignored"));
                    return Ok(true);
                }
                out.push(match op {
                    "addmoney" => format!("money_add(&ctx, {n}).await;"),
                    "removemoney" => format!("money_remove(&ctx, {n}).await;"),
                    _ => Self::result_call(format!("money_check(&ctx, {n}).await")),
                });
            }
            ("fadescreen" | "fadescreenspeed" | "fadescreenswapbuffers", [mode, ..]) => {
                let Some(mode) = u8_lit(mode) else {
                    return Ok(false);
                };
                let note = if op == "fadescreen" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                match mode {
                    0 => out.push(format!("ctx.fade(Fade::FromBlack).await;{note}")),
                    1 => out.push(format!("ctx.fade(Fade::ToBlack).await;{note}")),
                    // FADE_FROM_WHITE / FADE_TO_WHITE: black fades of the same length.
                    2 | 3 => {
                        let f = if mode == 2 { "FromBlack" } else { "ToBlack" };
                        out.push(format!("ctx.report(Report::Degraded(\"{op}\")).await;"));
                        out.push(format!("ctx.fade(Fade::{f}).await; // {op}, white"));
                    }
                    _ => return Ok(false),
                }
            }
            ("setrespawn", [h]) => {
                let h = match h {
                    Const { name, .. } if name.starts_with("HEAL_LOCATION_") && is_ident(name) => {
                        Some(name.clone())
                    }
                    Half(h) if *h < VARS_START => {
                        named(&names::HEAL_LOCATIONS, (*h).into()).map(str::to_string)
                    }
                    _ => u16_expr(h, true).map(|e| as_u8(&e)),
                };
                let Some(h) = h else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_respawn({h}).await;"));
            }
            ("checkplayergender", _) => out.push(format!(
                "ctx.set_var({r}, check_player_gender(&ctx).await).await;"
            )),
            ("setmetatile", [x, y, m, imp]) => {
                let (Some(x), Some(y), Some(m), Some(imp)) =
                    (coord(x), coord(y), u16_expr(m, true), u16_expr(imp, true))
                else {
                    return Ok(false);
                };
                let blocked = match imp.as_str() {
                    "0" => "false".to_string(),
                    "1" => "true".to_string(),
                    _ => format!("{imp} != 0"),
                };
                out.push(format!("ctx.set_metatile({x}, {y}, {m}, {blocked}).await;"));
            }
            ("opendoor" | "closedoor", [x, y]) => {
                let (Some(x), Some(y)) = (coord(x), coord(y)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.door({x}, {y}, {}).await;", op == "opendoor"));
            }
            ("waitdooranim", _) => out.push("ctx.wait_door().await;".to_string()),
            // Wave C: field effects (`prelude::field`) and `copyobjectxytoperm`.
            ("dofieldeffect" | "waitfieldeffect", [id]) => {
                let Some(id) = u16_expr(id, true) else {
                    return Ok(false);
                };
                out.push(if op == "dofieldeffect" {
                    format!("field_effect(&ctx, {id}).await;")
                } else {
                    format!("ctx.wait_field_effect({id}).await;")
                });
            }
            ("setfieldeffectargument", [n, v]) => {
                let (Some(n), Some(v)) = (u8_lit(n), u16_expr(v, true)) else {
                    return Ok(false);
                };
                out.push(format!("field_effect_arg(&ctx, {n}, {v}).await;"));
            }
            ("createvobject", [gfx, id, x, y, elevation, d]) => {
                let (Some(gfx), Some(id), Some(x), Some(y), Some(elevation), Some(d)) = (
                    u16_expr(gfx, false),
                    u8_lit(id),
                    coord(x),
                    coord(y),
                    u8_lit(elevation),
                    u8_lit(d).and_then(dir),
                ) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.create_virtual_object({gfx}, {id}, ({x}, {y}), {elevation}, {d}).await;"
                ));
            }
            ("turnvobject", [id, d]) => {
                let (Some(id), Some(d)) = (u8_lit(id), u8_lit(d).and_then(dir)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.turn_virtual_object({id}, {d}).await;"));
            }
            ("setmaplayoutindex", [l]) => {
                let Some(e) = u16_expr(l, true) else {
                    return Ok(false);
                };
                let note = match l {
                    Const { name, .. } => format!(" // {name}"),
                    _ => String::new(),
                };
                out.push(format!("ctx.set_map_layout({e}).await;{note}"));
            }
            // The field weather (`engine::weather`).
            ("setweather", [w]) => {
                let Some(e) = u16_expr(w, true) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_weather({e}).await;"));
            }
            ("doweather", []) => out.push("ctx.do_weather().await;".into()),
            ("resetweather", []) => out.push("ctx.reset_weather().await;".into()),
            // Wave F: the rotating tile puzzle (`prelude::puzzle` picks the puzzle's metatiles).
            ("initrotatingtilepuzzle", [t]) => {
                let Some(t) = u16_expr(t, true) else {
                    return Ok(false);
                };
                out.push(format!("rotating_tile_puzzle(&ctx, {t}).await;"));
            }
            ("moverotatingtileobjects", [n]) => {
                let Some(n) = u16_expr(n, true) else {
                    return Ok(false);
                };
                // `sMovingNpcId`: the last object it moved is `waitmovement 0`'s.
                st.moved = true;
                out.push(format!("ctx.rotating_tiles_move({n}).await;"));
            }
            ("turnrotatingtileobjects", _) => {
                out.push("ctx.rotating_tiles_turn().await;".to_string())
            }
            ("freerotatingtilepuzzle", _) => {
                out.push("ctx.rotating_tiles_free().await;".to_string())
            }
            ("copyobjectxytoperm", [id]) => {
                let Some(id) = local_id(id) else {
                    return Ok(false);
                };
                out.push(format!("ctx.copy_object_pos_to_template({id}).await;"));
            }
            ("setwildbattle", [s, level, _item]) => {
                let (Some(s), Some(level)) = (species_arg(s, false), u8_lit(level)) else {
                    return Ok(false);
                };
                out.push(format!("wild = Some(({s}, {level}));"));
            }
            ("dowildbattle", _) => {
                if !st.wild {
                    return Ok(false);
                }
                out.push(
                    "if let Some((s, l)) = wild { outcome = ctx.battle_wild(s, l, false).await as u16; }"
                        .to_string(),
                );
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// `callstd`/`gotostd` of `n` (without the `goto`'s return); `None` when `n` is no index.
    pub(super) fn callstd(&mut self, n: &IrArg, st: &BlockState) -> Option<Vec<String>> {
        let n = std_index(n)?;
        let std_name = names::std_name(n as u8);
        self.stats.stds.insert(std_name.clone());
        if is_msgbox(n)
            && let Some(t) = &st.word0
            && let Some(k) = msgbox_kind(n)
        {
            return Some(vec![format!("msgbox(&ctx, {t}, MsgBox::{k}).await;")]);
        }
        let item = format!(
            "ctx.var({}).await, ctx.var({}).await",
            var(ITEM_VAR),
            var(ITEM_AMOUNT_VAR)
        );
        Some(vec![match n {
            // `give_item` writes VAR_RESULT itself.
            STD_OBTAIN_ITEM => format!("give_item(&ctx, {item}).await;"),
            // Gen3 `Std_FindItem` leaves VAR_RESULT as its own commands do (0 after a pick-up,
            // `pyramid_getlocation`): the helper writes it, never the call site.
            STD_FIND_ITEM => format!("find_item(&ctx, {item}).await;"),
            _ => {
                let c = std_const(n);
                format!("callstd(&ctx, {c}).await;")
            }
        }])
    }

    /// `special`/`specialvar` by the special's class; false when the operand is no special.
    pub(super) fn special(
        &mut self,
        s: &IrArg,
        dest: Option<String>,
        st: &BlockState,
        out: &mut Vec<String>,
    ) -> bool {
        let Some((name, value)) = special_name(s) else {
            return false;
        };
        self.stats.specials.insert(name.clone());
        let class = self.support.specials.get(&name).copied();
        let Some(class) = class else {
            return false;
        };
        self.count_class(class);
        let c = if is_ident(&name) && !name.contains("0x") {
            name.clone()
        } else {
            format!("{value:#x}")
        };
        // Specials that read script state Gen3 keeps outside vars: the battle outcome of the
        // function's `dowildbattle` (`gBattleOutcome`), the trainer of the block's
        // `trainerbattle` (`gTrainerBattleOpponent_A`).
        let call = match (name.as_str(), &st.trainer) {
            ("SPECIAL_GetBattleOutcome", _) if st.wild => "outcome".to_string(),
            (n, Some(t)) if OPPONENT_SPECIALS.contains(&n) => {
                format!("special_vs(&ctx, {c}, {t}).await")
            }
            _ => format!("special(&ctx, {c}).await"),
        };
        match class {
            Class::Native => out.push(match &dest {
                Some(v) => format!("ctx.set_var({v}, {call}).await;"),
                None => format!("{call};"),
            }),
            Class::Ignore => out.push(format!("// special {name}")),
            Class::Default(v) => {
                let d = dest.unwrap_or_else(|| var(VAR_RESULT));
                out.push(format!("ctx.set_var({d}, {v}).await; // {name} (default)"));
            }
            Class::Unsupported => {
                self.count_report(&name);
                if dest.is_some() {
                    *self
                        .stats
                        .stale_specialvars
                        .entry(name.clone())
                        .or_default() += 1;
                }
                out.push(format!(
                    "ctx.report(Report::Unsupported(\"{name}\")).await;"
                ));
            }
        }
        true
    }

    pub(super) fn wait_move(&mut self, id: &IrArg, st: &mut BlockState) -> Option<String> {
        if u16_lit(id) == Some(0) {
            if !st.moved {
                self.stats.unresolved_waitmovement += 1;
            }
            return Some("ctx.wait_move(0).await; // the last mover".to_string());
        }
        let id = local_id(id)?;
        Some(format!("ctx.wait_move({id}).await;"))
    }

    /// `trainerbattle` (`data/scripts/trainer_battle.inc`): see the module doc.
    pub(super) fn trainerbattle(
        &mut self,
        k: &IrArg,
        tr: &IrArg,
        local: &IrArg,
        rest: &[IrArg],
        out: &mut Vec<String>,
    ) -> Result<bool> {
        let kind = match k {
            IrArg::Const { name, .. } => TRAINER_BATTLE_KINDS
                .iter()
                .position(|n| n == name)
                .map(|p| p as u8),
            a => u8_lit(a),
        };
        let (Some(kind), Some(trainer), Some(lid)) =
            (kind, trainer_arg(tr, false), local_id(local))
        else {
            return Ok(false);
        };
        let text = |a: &IrArg| match a {
            IrArg::Text(t) if is_ident(t) => Some(Some(t.clone())),
            IrArg::Raw(0) | IrArg::Word(0) => Some(None),
            _ => None,
        };
        let script = |a: &IrArg| match a {
            IrArg::Script(s) => Some(s.clone()),
            _ => None,
        };
        // (intro, lose, not_enough, event_script) by kind (`scripts::trainerbattle_args`).
        let parts = match (kind, rest) {
            (0 | 5 | 9..=12, [i, l]) => (text(i), text(l), Some(None), None),
            (1 | 2, [i, l, s]) => (text(i), text(l), Some(None), Some(script(s))),
            (3, [l]) => (Some(None), text(l), Some(None), None),
            (4 | 7, [i, l, n]) => (text(i), text(l), text(n), None),
            (6 | 8, [i, l, n, s]) => (text(i), text(l), text(n), Some(script(s))),
            _ => return Ok(false),
        };
        let (Some(intro), Some(Some(lose)), Some(not_enough)) = (parts.0, parts.1, parts.2) else {
            return Ok(false);
        };
        let event_script = match parts.3 {
            None => None,
            Some(Some(s)) => Some(s),
            Some(None) => return Ok(false),
        };
        let opt = |o: &Option<String>| {
            o.as_ref()
                .map_or("None".to_string(), |t| format!("Some({t})"))
        };
        let ev = event_script.as_ref().map_or("None".to_string(), |s| {
            format!("Some({DATA}::script_ids::{s})")
        });
        let music = !matches!(kind, 1 | 8);
        out.push(format!(
            "if !trainer_battle(&ctx, TrainerBattle {{ kind: {kind}, trainer: {trainer}.0, local_id: {lid}, intro: {}, lose: {lose}, not_enough: {}, event_script: {ev}, music: {music} }}).await {{",
            opt(&intro),
            opt(&not_enough)
        ));
        out.push("    return Flow::End;".to_string());
        out.push("}".to_string());
        Ok(true)
    }

    pub(super) fn movement_static(&mut self, m: &IrMovement) -> Result<String> {
        let mut s = String::from("[\n");
        let mut n = 0;
        for a in &m.actions {
            let short = a.strip_prefix("MOVEMENT_ACTION_").unwrap_or(a);
            if let Some(e) = step_of(a) {
                writeln!(s, "    {e}, // {short}")?;
                n += 1;
                continue;
            }
            match self.support.movements.get(a) {
                Some(MoveFallback::As(list)) => {
                    for t in list {
                        let e = step_of(t).with_context(|| {
                            format!("movements.ron: {a} -> {t}, which has no Step")
                        })?;
                        writeln!(
                            s,
                            "    {e}, // {short} (as {})",
                            t.strip_prefix("MOVEMENT_ACTION_").unwrap_or(t)
                        )?;
                        n += 1;
                    }
                }
                Some(MoveFallback::Delay(d)) => {
                    writeln!(s, "    Step::Delay({d}), // {short} (as a delay)")?;
                    n += 1;
                }
                Some(MoveFallback::Skip) => writeln!(s, "    // {short} (no effect here)")?,
                None => {
                    writeln!(s, "    Step::Delay(0), // {short} (unsupported)")?;
                    n += 1;
                    *self
                        .stats
                        .unknown_movement_actions
                        .entry(a.clone())
                        .or_default() += 1;
                }
            }
        }
        s.push(']');
        Ok(format!("pub static {}: [Step; {n}] = {s};\n", m.name))
    }

    pub(super) fn dispatch(&self) -> String {
        let mut names: Vec<(&str, &str)> = self
            .fns
            .iter()
            .map(|&f| (self.labs[f].name, self.labs[f].module))
            .collect();
        names.sort();
        let n = names.len();
        let mut o = String::from(
            "// @generated by content/build.rs (extract::codegen::lower)\n\
             /// How a script function finished: Gen3 `return` (`Continue`) or `end` (`End`, which\n\
             /// also stops every caller).\n\
             #[derive(Clone, Copy, PartialEq, Eq, Debug)]\n\
             pub enum Flow {\n    Continue,\n    End,\n}\n",
        );
        let _ = writeln!(o, "pub const SCRIPT_COUNT: usize = {n};");
        let _ = write!(o, "pub static SCRIPT_NAMES: [&str; {n}] = [");
        for (name, _) in &names {
            let _ = write!(o, "\"{name}\",");
        }
        o.push_str("];\n");
        o.push_str(
            "/// `ScriptId` of every script, by name.\n#[allow(non_upper_case_globals, unused_imports)]\npub mod script_ids {\n    use engine::data::ScriptId;\n",
        );
        for (i, (name, _)) in names.iter().enumerate() {
            let _ = writeln!(o, "    pub const {name}: ScriptId = ScriptId({i});");
        }
        o.push_str("}\n");
        // One adapter, not an async body per script (Task L: a generic `async fn run(f: impl
        // Future)` was one more async body per script for the type checker). Generic over the
        // script's future and boxed once (Task M; it held a second box before).
        o.push_str(
            "/// A script's future with its `Flow` dropped (the engine's `Script` has no output).\n\
             struct Run<F>(F);\n\
             impl<F: core::future::Future<Output = Flow>> core::future::Future for Run<F> {\n    type Output = ();\n    \
             fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<()> {\n        \
             // SAFETY: structural pinning of the only field, which `Run` never moves out.\n        \
             unsafe { self.map_unchecked_mut(|r| &mut r.0) }.poll(cx).map(|_| ())\n    }\n}\n",
        );
        let _ = write!(
            o,
            "/// One thunk per `ScriptId`.\npub static SCRIPTS: [fn(engine::script::Ctx) -> engine::script::Script; {n}] = [\n"
        );
        for (name, module) in &names {
            let _ = writeln!(
                o,
                "    |ctx| alloc::boxed::Box::pin(Run({SCRIPTS}::{module}::{name}(ctx))),"
            );
        }
        o.push_str("];\n");
        o.push_str(
            "/// Instantiates script `id`.\npub fn script(id: u16, ctx: engine::script::Ctx) -> engine::script::Script {\n    match SCRIPTS.get(usize::from(id)) {\n        Some(f) => f(ctx),\n        None => alloc::boxed::Box::pin(async move { ctx.log(\"unknown script id\").await }),\n    }\n}\n",
        );
        o
    }
}

pub(super) struct BlockState {
    /// An `applymovement` ran earlier in the block.
    moved: bool,
    /// The trainer of the block's last `trainerbattle` (Gen3 `gTrainerBattleOpponent_A`).
    trainer: Option<String>,
    /// The text of the block's last `loadword 0, <text>`.
    word0: Option<String>,
    /// The function has the `wild` local.
    wild: bool,
}

/// The specials that read Gen3 `gTrainerBattleOpponent_A`: after a `trainerbattle` in the same
/// block they lower to `special_vs(&ctx, id, trainer)` (the prelude's `special` answers them
/// without a trainer).
pub const OPPONENT_SPECIALS: &[&str] = &[
    "SPECIAL_ShouldTryRematchBattle",
    "SPECIAL_IsTrainerReadyForRematch",
];

/// `// a, b, c` of an unsupported command's operands, or nothing.
pub(super) fn args_comment(args: &[IrArg]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let v: Vec<String> = args.iter().map(arg_str).collect();
    format!(" // {}", v.join(", "))
}

pub(super) fn arg_str(a: &IrArg) -> String {
    match a {
        IrArg::Byte(b) => b.to_string(),
        IrArg::Half(h) => format!("{h:#x}"),
        IrArg::Word(w) | IrArg::Raw(w) => format!("{w:#x}"),
        IrArg::Flag(s)
        | IrArg::Var(s)
        | IrArg::Script(s)
        | IrArg::Text(s)
        | IrArg::Movement(s)
        | IrArg::Braille(s)
        | IrArg::Items(s) => s.clone(),
        IrArg::Const { name, .. } => name.clone(),
    }
}

pub(super) fn is_msgbox(n: u32) -> bool {
    msgbox_kind(n).is_some()
}

pub(super) fn msgbox_kind(n: u32) -> Option<&'static str> {
    Some(match names::MSGBOX_TYPES.name(n)? {
        "MSGBOX_NPC" => "Npc",
        "MSGBOX_SIGN" => "Sign",
        "MSGBOX_DEFAULT" => "Default",
        "MSGBOX_YESNO" => "YesNo",
        "MSGBOX_AUTOCLOSE" => "Autoclose",
        "MSGBOX_GETPOINTS" => "GetPoints",
        "MSGBOX_POKENAV" => "Pokenav",
        _ => return None,
    })
}

/// The prelude constant (`u8`) of std index `n`, or the number.
pub(super) fn std_const(n: u32) -> String {
    match names::STD_SCRIPTS.name(n) {
        Some(s) if s.starts_with("STD_") && is_ident(s) => s.to_string(),
        Some(s) if is_ident(s) => format!("STD_{s}"),
        _ => n.to_string(),
    }
}

/// `sScriptConditionTable` row as a test of the comparison register.
pub(super) fn condition(c: &IrArg) -> Option<&'static str> {
    Some(match u8_lit(c)? {
        0 => "ctx.cond() == Ordering::Less",
        1 => "ctx.cond() == Ordering::Equal",
        2 => "ctx.cond() == Ordering::Greater",
        3 => "ctx.cond() != Ordering::Greater",
        4 => "ctx.cond() != Ordering::Less",
        5 => "ctx.cond() != Ordering::Equal",
        _ => return None,
    })
}

pub(super) fn flag_cond(e: &str) -> String {
    format!("if {e} {{ Ordering::Equal }} else {{ Ordering::Less }}")
}
