//! A synthetic `GameData` for the scene tests: Task D's battle tables (every species with a
//! test pic), Task E's synthetic battle gfx with Task F's textbox, font and placeholder strings,
//! one trainer and a field text for its defeat. No game text: placeholders are ASCII.

use crate::battle::gfx::test_gfx;
use crate::battle::test_data::{SPECIES, TABLES};
use crate::battle::ui::test_game::{self as ui_game, enc, leak, t};
use crate::battle::{BattleGfx, Str};
use crate::data::{
    Behavior, Dir, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId,
    TrainerId, UiText, WindowGfx,
};
use crate::mon::{BattleTables, SpeciesInfo, TrainerData};
use crate::script::{Ctx, Script};
use alloc::boxed::Box;
use alloc::vec::Vec;

pub const TRAINER: TrainerId = TrainerId(1);
pub const DEFEAT: TextId = TextId(7);

pub struct Game {
    ui: ui_game::Game,
    tables: &'static BattleTables,
    gfx: &'static BattleGfx,
    trainer: &'static TrainerData,
    /// `Str`s ending with a `\p` prompt.
    pub prompt_strs: &'static [Str],
}

pub fn game() -> Game {
    let ui = ui_game::game();
    let pic = test_gfx::mon_gfx(8, 0, 0);
    let species: Vec<SpeciesInfo> = SPECIES
        .iter()
        .enumerate()
        .map(|(i, s)| SpeciesInfo {
            gfx: (i > 0).then_some(pic),
            ..*s
        })
        .collect();
    let tables = Box::leak(Box::new(BattleTables {
        species: Box::leak(species.into_boxed_slice()),
        ..TABLES
    }));
    let gfx = Box::leak(Box::new(BattleGfx {
        textbox: ui.gfx.textbox,
        small_font: ui.gfx.small_font,
        ..*test_gfx::gfx()
    }));
    let trainer = Box::leak(Box::new(TrainerData {
        class_name: leak(enc("YOUNGSTER")),
        name: leak(enc("JOE")),
        pic: test_gfx::trainer_pic(),
        female: false,
        ai: 0,
        double: false,
        party: &[],
    }));
    Game {
        ui,
        tables,
        gfx,
        trainer,
        prompt_strs: &[Str::PlayerWhiteout2],
    }
}

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        self.ui.font
    }
    fn window(&self) -> &'static WindowGfx {
        self.ui.window
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        if id == DEFEAT {
            // "I lost to {PLAYER}"
            let mut v = t("I lost to ");
            v.extend([0xFD, 0x01, 0xFF]);
            leak(v)
        } else {
            &[0xFF]
        }
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: leak(enc("ASH")),
        }
    }
    fn battle(&self) -> &'static BattleTables {
        self.tables
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        (id == TRAINER).then_some(self.trainer)
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        if self.prompt_strs.contains(&s) {
            // "BYE\p"
            let mut v = t("BYE");
            v.extend([0xFB, 0xFF]);
            return leak(v);
        }
        match s {
            // "{B_DEF_NAME_WITH_PREFIX} fainted"
            Str::PkmnFainted => {
                let mut v = alloc::vec![0xFD, 0x10];
                v.extend(t(" fainted"));
                v.push(0xFF);
                leak(v)
            }
            Str::UsedMove => leak(enc("USED")),
            Str::Intro1 => leak(enc("A WILD MON")),
            Str::GoPkmn => leak(enc("GO")),
            _ => self.ui.battle_text(s),
        }
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.gfx
    }
}
