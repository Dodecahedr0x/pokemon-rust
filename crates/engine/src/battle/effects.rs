//! Move effects beyond `Hit` / `StatUp` / `StatDown` / `Recoil`, statuses and abilities.
//!
//! Task D ships the `effects::apply(...)` seam as a stub that falls back to `Unsupported`
//! handling; **Task E2** (Wave 2) fills it in, following each `BattleScript_Effect*` and
//! `Cmd_seteffect*` in Gen3 order.
