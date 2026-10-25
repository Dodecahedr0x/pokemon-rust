use super::*;
use crate::bag::Bag;
use crate::checkpoint::PLAYER_NAME_SIZE;
use crate::data::{
    Behavior, Dir, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId, UiText,
};
use crate::flow::Vars;
use crate::mon::Party;
use crate::options::Options;
use crate::pc::PcStorage;
use crate::pokedex::Pokedex;
use crate::rng::Rng;
use crate::script::{Ctx, Report, Script};
use crate::ui::SaveQuestion;
use crate::video::{Frame, Tile};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `EC_WORD_BAG`: the first Trainer word Gen3's keyboard offers
/// (`gEasyChatGroup_Trainer[0].alphabeticalOrder`).
const BAG: u16 = (1 << 9) | 21;
/// `EC_WORD_CENTER`: the second Trainer word.
const CENTER: u16 = (1 << 9) | 7;
/// `EC_WORD_RED`: disabled (`numEnabledWords` drops the six version names).
const RED: u16 = (1 << 9) | 19;

struct Game {
    font: &'static crate::data::Font,
    window: &'static crate::data::WindowGfx,
}

fn game() -> Game {
    let glyphs = vec![[0u8; 64]; 1];
    let widths = vec![6u8; 1];
    let tiles: Vec<Tile> = (0..14u32).map(|i| [i; 8]).collect();
    Game {
        font: Box::leak(Box::new(crate::data::Font {
            glyphs: Box::leak(glyphs.into_boxed_slice()),
            widths: Box::leak(widths.into_boxed_slice()),
        })),
        window: Box::leak(Box::new(crate::data::WindowGfx {
            tiles: Box::leak(tiles.into_boxed_slice()),
            palette: Box::leak(Box::new(core::array::from_fn(|i| i as u16))),
            text_fg: 2,
            text_shadow: 3,
            text_bg: 1,
        })),
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
    fn font(&self) -> &'static crate::data::Font {
        self.font
    }
    fn window(&self) -> &'static crate::data::WindowGfx {
        self.window
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => &[1, 0xFF],
            UiText::No => &[2, 0xFF],
            _ => &[0xFF],
        }
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
            player_name: &[0xFF],
        }
    }
}

fn tap(chat: &mut EasyChat, g: &Game, pressed: Buttons) -> (ScreenOut, Vec<Report>) {
    let mut f = Frame::default();
    let mut rng = Rng::new(1);
    let mut party = Party::default();
    let mut bag = Bag::new([1, 1, 1, 1, 1]);
    let mut money = 0u32;
    let mut pc = PcStorage::new(1, 1);
    let mut decor = crate::decor::DecorPc::new();
    let mut str_vars = crate::str_vars::StrVars::new();
    let mut options = Options::default();
    let mut coins = 0u16;
    let mut pokedex = Pokedex::default();
    let mut registered = None;
    let mut vars = Vars::new();
    let mut name = [0xFF; PLAYER_NAME_SIZE];
    let mut rival_name = None;
    let mut reports = Vec::new();
    let mut notes = Vec::new();
    let mut cx = ScreenCx {
        game: g,
        f: &mut f,
        rng: &mut rng,
        party: &mut party,
        bag: &mut bag,
        money: &mut money,
        pc: &mut pc,
        decor: &mut decor,
        str_vars: &mut str_vars,
        options: &mut options,
        coins: &mut coins,
        pokedex: &mut pokedex,
        registered_item: &mut registered,
        vars: &mut vars,
        player_name: &mut name,
        rival_name: &mut rival_name,
        map: MapId(0),
        frame: 0,
        saved: None,
        save_question: SaveQuestion::default(),
        child: None,
        reports: &mut reports,
        notes: &mut notes,
    };
    let out = chat.step(
        &mut cx,
        ScreenInput {
            pressed,
            held: pressed,
        },
    );
    (out, reports)
}

fn running(chat: &mut EasyChat, g: &Game, pressed: Buttons) {
    let (out, reports) = tap(chat, g, pressed);
    assert_eq!(out, ScreenOut::Running);
    assert!(reports.is_empty(), "{reports:?}");
}

#[test]
fn the_trainer_list_is_gen3s_enabled_words() {
    let trainer = words::group(1);
    assert_eq!(trainer.first().copied(), Some(BAG));
    assert_eq!(trainer.get(1).copied(), Some(CENTER));
    assert!(!trainer.contains(&RED));
    assert!(trainer.contains(&((1 << 9) | 0)));
}

#[test]
fn builds_a_phrase_and_reads_it_back() {
    let g = game();
    let mut chat = EasyChat::new(0);
    assert_eq!(chat.phrase(), &[words::EMPTY; 4]);

    running(&mut chat, &g, Buttons::A);
    assert_eq!(
        chat.prompt(),
        Prompt::Keyboard {
            row: 0,
            col: 0,
            scroll: 0
        }
    );
    running(&mut chat, &g, Buttons::A);
    assert_eq!(
        chat.prompt(),
        Prompt::Words {
            group: 1,
            row: 0,
            col: 0,
            scroll: 0
        }
    );
    running(&mut chat, &g, Buttons::A);
    assert_eq!(chat.phrase()[0], BAG);
    assert_eq!(chat.prompt(), Prompt::Phrase { row: 0, col: 0 });

    running(&mut chat, &g, Buttons::RIGHT);
    running(&mut chat, &g, Buttons::A);
    running(&mut chat, &g, Buttons::A);
    running(&mut chat, &g, Buttons::RIGHT);
    running(&mut chat, &g, Buttons::A);
    assert_eq!(chat.phrase(), &[BAG, CENTER, words::EMPTY, words::EMPTY]);

    running(&mut chat, &g, Buttons::START);
    assert_eq!(chat.prompt(), Prompt::Confirm { cursor: 0 });
    let (out, reports) = tap(&mut chat, &g, Buttons::A);
    assert!(reports.is_empty(), "{reports:?}");
    assert_eq!(out, ScreenOut::Done(ScreenResult::Bool(true)));
    assert_eq!(chat.phrase(), &[BAG, CENTER, words::EMPTY, words::EMPTY]);
}

#[test]
fn unchanged_phrases_quit_and_the_questionnaire_confirms() {
    let g = game();
    let mut profile = EasyChat::new(0);
    running(&mut profile, &g, Buttons::START);
    assert_eq!(profile.prompt(), Prompt::Quit { cursor: 1 });

    let mut form = EasyChat::new(ASKS_CONFIRM);
    running(&mut form, &g, Buttons::START);
    assert_eq!(form.prompt(), Prompt::Confirm { cursor: 0 });
}

#[test]
fn contest_interview_and_apprentice_cannot_quit() {
    let g = game();
    for kind in [CONTEST_INTERVIEW, APPRENTICE] {
        let mut chat = EasyChat::new(kind);
        running(&mut chat, &g, Buttons::B);
        assert!(
            matches!(chat.prompt(), Prompt::CannotQuit { .. }),
            "kind {kind}: {:?}",
            chat.prompt()
        );
        running(&mut chat, &g, Buttons::A);
        assert!(matches!(chat.prompt(), Prompt::Phrase { .. }));
        running(&mut chat, &g, Buttons::START);
        assert!(
            matches!(chat.prompt(), Prompt::CannotQuit { .. }),
            "kind {kind} empty confirm: {:?}",
            chat.prompt()
        );
        let (out, _) = tap(&mut chat, &g, Buttons::B);
        assert_eq!(out, ScreenOut::Running);
    }

    let mut interview = EasyChat::new(INTERVIEW);
    running(&mut interview, &g, Buttons::B);
    assert_eq!(interview.prompt(), Prompt::Quit { cursor: 1 });
}
