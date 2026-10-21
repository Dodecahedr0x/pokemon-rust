//! Battle visuals drawn with `video::layout::battle`. **Task E.**
//!
//! Components, each with `hash` and a per-frame `update(&mut self, f: &mut Frame)`:
//! `Background` (`DrawMainBattleBackground`, chunked `load_step`), `MonSprite` /
//! `TrainerSprite` (slides, ball pop, faint, hit blink, lunge, tint), `Healthbox`
//! (`CreateBattlerHealthboxSprites`, `Cmd::ObjTile` diffs), `HpBar` (`MoveBattleBar`, 48 px)
//! and `ExpBar` (64 px).
