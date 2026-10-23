//! Async scripts. A script is a plain `async fn(Ctx)` in `content`. It never touches `World`:
//! every effect is a [`Request`] posted through its [`Ctx`] mailbox. Each frame the [`Runner`]
//! polls the script and hands requests to a [`ScriptHost`] (the world). Instant requests (flag
//! reads, var writes…) are answered within the same frame; the others (printing text, walking,
//! fading…) suspend the script until the host reports completion on a later frame.

use crate::battle::BattleOutcome;
pub use crate::data::Emote;
use crate::data::{
    Dir, FlagId, ItemId, MapId, MoveId, MovementType, SpeciesId, TextId, TrainerId, VarId,
};
use crate::hash::Fnv64;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::cmp::Ordering;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

/// A running script instance.
pub type Script = Pin<Box<dyn Future<Output = ()>>>;

/// Movement speed of one step (Gen3 `walk_slow` … `walk_faster`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Speed {
    Slow,
    Normal,
    Fast,
    Faster,
}

/// One action of a movement script (Gen3 `applymovement` data).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Face(Dir),
    Walk(Dir, Speed),
    WalkInPlace(Dir, Speed),
    /// Jump `cells` (0 = in place, 1, or 2) towards `Dir`.
    Jump(Dir, u8),
    Delay(u8),
    FacePlayer,
    FaceAwayFromPlayer,
    LockFacing,
    UnlockFacing,
    Hide,
    Show,
    Emote(Emote),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fade {
    ToBlack,
    FromBlack,
}

/// What a `buffer*` command writes into a string var (Gen3 `gStringVar1..3`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BufferSrc {
    /// `bufferspeciesname`.
    Species(SpeciesId),
    /// `bufferitemname`.
    Item(ItemId),
    /// `buffermovename`.
    Move(MoveId),
    /// `buffernumberstring`.
    Number(u32),
    /// `bufferstdstring` (`STDSTRING_*`).
    Std(u8),
    /// `bufferpartymonnick`: the nickname of party slot `n`.
    PartyNick(u8),
    /// `bufferleadmonspeciesname`.
    LeadSpecies,
    /// `bufferstring` of a script text.
    Text(TextId),
}

/// Which warp a `setdynamicwarp` / `setescapewarp` sets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarpSlot {
    /// Gen3 `gSaveBlock1Ptr->dynamicWarp` (warps whose destination is `WARP_ID_DYNAMIC`).
    Dynamic,
    /// Gen3 `escapeWarp` (Escape Rope, Dig).
    Escape,
}

/// Something a script could not do as Gen3 would (logged, and an [`crate::world::Event`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Report {
    /// A command or special the engine does not implement (its decomp name).
    Unsupported(&'static str),
    /// A command run in a simplified way (its decomp name).
    Degraded(&'static str),
}

/// Gen3 `MULTI_B_PRESSED`: the multichoice answer when B cancels it.
pub const MULTI_B_PRESSED: u16 = 127;

/// Everything a script can ask of the world.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Request {
    // ---- instant: answered the same frame ----
    GetFlag(FlagId),
    SetFlag(FlagId, bool),
    GetVar(VarId),
    SetVar(VarId, u16),
    /// Uniform value in `0..n`; answers `Value`.
    Random(u16),
    /// Answers `Pos` of the player's cell.
    PlayerPos,
    /// Answers `Value(dir as u16)` in `Dir` order.
    PlayerFacing,
    Log(&'static str),
    // ---- text ----
    /// Starts printing a text in the dialogue window (opening it if needed); answers at once.
    Message(TextId),
    /// Until the current message has finished printing.
    WaitMessage,
    /// Until A or B is pressed.
    WaitButton,
    CloseMessage,
    /// Shows a yes/no menu; answers `Bool(true)` for yes, `false` for no or B.
    YesNo,
    // ---- objects (local ids; `data::PLAYER` is the player) ----
    /// Freezes the player and the object being talked to.
    Lock,
    /// Freezes every object.
    LockAll,
    /// Undoes `Lock` and closes the dialogue window.
    Release,
    ReleaseAll,
    /// The object that started the script turns to face the player.
    FacePlayer,
    Move {
        obj: u8,
        steps: &'static [Step],
    },
    /// Until `obj` has finished its movement script. `0` means the object of the last `Move`
    /// (Gen3 `sMovingNpcId`).
    WaitMove(u8),
    SetObjectPos {
        obj: u8,
        x: i16,
        y: i16,
    },
    /// Make a spawned object visible again (Gen3 `showobjectat`). No flag changes.
    ShowObject(u8),
    /// Make an object invisible but keep it spawned (Gen3 `hideobjectat`). No flag changes.
    HideObject(u8),
    /// Spawn an object of the current map from its template, whatever its hide flag
    /// (Gen3 `addobject`).
    AddObject(u8),
    /// Despawn an object of the current map and set its hide flag, so it stays gone on
    /// re-entry (Gen3 `removeobject`).
    RemoveObject(u8),
    Turn {
        obj: u8,
        dir: Dir,
    },
    /// Changes the object's template (and the live object if spawned), as Gen3
    /// `setobjectmovementtype`.
    SetMovementType {
        obj: u8,
        movement: MovementType,
    },
    /// Moves the object's template position for the rest of the map visit (Gen3
    /// `setobjectxyperm`); a spawned object is not moved.
    SetObjectTemplatePos {
        obj: u8,
        x: i16,
        y: i16,
    },
    // ---- world ----
    Delay(u16),
    Fade(Fade),
    /// Moves the player to `map`: to warp `warp` if given, else to cell (x, y).
    Warp {
        map: MapId,
        warp: Option<u8>,
        x: i16,
        y: i16,
    },
    SetMetatile {
        x: i16,
        y: i16,
        metatile: u16,
        blocked: bool,
    },
    RedrawMap,
    /// Starts the door animation at map cell (x, y) (Gen3 `opendoor`/`closedoor`).
    Door {
        x: i16,
        y: i16,
        open: bool,
    },
    /// Until every door animation has finished (Gen3 `waitdooranim`).
    WaitDoor,
    /// Until a pending warp, fade or special-started state is done (Gen3 `waitstate`).
    WaitState,
    // ---- party and battle (Milestone 4) ----
    /// Gen3 `givemon`: answers `Value` = `MON_GIVEN_TO_PARTY` (0), `_PC` (1) or
    /// `MON_CANT_GIVE` (2).
    GiveMon {
        species: SpeciesId,
        level: u8,
    },
    /// Gen3 `getpartysize`: answers `Value`.
    PartySize,
    /// Gen3 `special HealPlayerParty`.
    HealParty,
    /// A wild battle; answers `Value(BattleOutcome as u16)` once the field is back.
    BattleWild {
        species: SpeciesId,
        level: u8,
        first_battle: bool,
    },
    /// A trainer battle; `defeat` is shown when the player wins. Answers
    /// `Value(BattleOutcome as u16)` once the field is back. A battle that whites out never
    /// answers: the World aborts the runner.
    BattleTrainer {
        trainer: TrainerId,
        defeat: TextId,
    },
    /// Gen3 `special ChooseStarter`: answers `Value(index 0..3)`.
    ChooseStarter([SpeciesId; 3]),
    /// Gen3 `setrespawn` (heal location number).
    SetRespawn(u8),
    // ---- Milestone 8 ----
    /// Gen3 `additem`/`giveitem`: answers `Bool` (whether it fit in the bag).
    ItemAdd {
        item: ItemId,
        qty: u16,
    },
    /// Gen3 `removeitem`: answers `Bool` (whether the bag held that many).
    ItemRemove {
        item: ItemId,
        qty: u16,
    },
    /// Gen3 `checkitem`: answers `Bool`.
    ItemCheck {
        item: ItemId,
        qty: u16,
    },
    /// Gen3 `checkitemspace`: answers `Bool`.
    ItemSpace {
        item: ItemId,
        qty: u16,
    },
    /// Gen3 `addmoney`.
    MoneyAdd(u32),
    /// Gen3 `removemoney`.
    MoneyRemove(u32),
    /// Gen3 `checkmoney`: answers `Bool` (whether the player has at least that much).
    MoneyCheck(u32),
    /// Gen3 `buffer*`: writes string var `slot` (0..3).
    Buffer {
        slot: u8,
        src: BufferSrc,
    },
    /// Gen3 `multichoice`/`multichoicedefault`: answers `Value` (the index, or
    /// [`MULTI_B_PRESSED`]).
    Multichoice {
        list: u8,
        x: u8,
        y: u8,
        default: u8,
        cancel: bool,
    },
    /// Gen3 `setdynamicwarp`/`setescapewarp`.
    SetWarp {
        slot: WarpSlot,
        map: MapId,
        warp: Option<u8>,
        x: i16,
        y: i16,
    },
    /// Logged and reported as an event; answers at once.
    Report(Report),
    /// Until trainer `obj` has walked up to the player after seeing them (Gen3
    /// `TrainerSeeFunc` steps); the sight check issues it before the trainer's script.
    TrainerApproach {
        obj: u8,
    },
}

impl Request {
    /// Feeds the request's kind and arguments into `h` (a movement script by its length and
    /// [`steps_digest`], a log line not at all).
    pub fn hash_into(&self, h: &mut Fnv64) {
        let i = |v: i16| v as u16;
        let (tag, a, b, c, d): (u8, u16, u16, u16, u16) = match *self {
            Request::GetFlag(f) => (0, f.0, 0, 0, 0),
            Request::SetFlag(f, on) => (1, f.0, on as u16, 0, 0),
            Request::GetVar(v) => (2, v.0, 0, 0, 0),
            Request::SetVar(v, x) => (3, v.0, x, 0, 0),
            Request::Random(n) => (4, n, 0, 0, 0),
            Request::PlayerPos => (5, 0, 0, 0, 0),
            Request::PlayerFacing => (6, 0, 0, 0, 0),
            Request::Log(_) => (7, 0, 0, 0, 0),
            Request::Message(t) => (8, t.0, 0, 0, 0),
            Request::WaitMessage => (9, 0, 0, 0, 0),
            Request::WaitButton => (10, 0, 0, 0, 0),
            Request::CloseMessage => (11, 0, 0, 0, 0),
            Request::YesNo => (12, 0, 0, 0, 0),
            Request::Lock => (13, 0, 0, 0, 0),
            Request::LockAll => (14, 0, 0, 0, 0),
            Request::Release => (15, 0, 0, 0, 0),
            Request::ReleaseAll => (16, 0, 0, 0, 0),
            Request::FacePlayer => (17, 0, 0, 0, 0),
            Request::Move { obj, steps } => {
                let d = steps_digest(steps);
                (
                    18,
                    obj as u16,
                    steps.len() as u16,
                    d as u16,
                    (d >> 16) as u16,
                )
            }
            Request::WaitMove(obj) => (19, obj as u16, 0, 0, 0),
            Request::SetObjectPos { obj, x, y } => (20, obj as u16, i(x), i(y), 0),
            Request::ShowObject(obj) => (21, obj as u16, 0, 0, 0),
            Request::HideObject(obj) => (22, obj as u16, 0, 0, 0),
            Request::AddObject(obj) => (23, obj as u16, 0, 0, 0),
            Request::RemoveObject(obj) => (24, obj as u16, 0, 0, 0),
            Request::Turn { obj, dir } => (25, obj as u16, dir as u16, 0, 0),
            Request::SetMovementType { obj, movement } => {
                let m = crate::objects::movement_code(movement);
                (26, obj as u16, m[0] as u16 | (m[1] as u16) << 8, 0, 0)
            }
            Request::SetObjectTemplatePos { obj, x, y } => (27, obj as u16, i(x), i(y), 0),
            Request::Delay(n) => (28, n, 0, 0, 0),
            Request::Fade(f) => (29, f as u16, 0, 0, 0),
            Request::Warp { map, warp, x, y } => {
                (30, map.0, warp.map_or(0xFFFF, |w| w as u16), i(x), i(y))
            }
            Request::SetMetatile {
                x,
                y,
                metatile,
                blocked,
            } => (31, i(x), i(y), metatile, blocked as u16),
            Request::RedrawMap => (32, 0, 0, 0, 0),
            Request::Door { x, y, open } => (33, i(x), i(y), open as u16, 0),
            Request::WaitDoor => (34, 0, 0, 0, 0),
            Request::WaitState => (35, 0, 0, 0, 0),
            Request::GiveMon { species, level } => (36, species.0, level as u16, 0, 0),
            Request::PartySize => (37, 0, 0, 0, 0),
            Request::HealParty => (38, 0, 0, 0, 0),
            Request::BattleWild {
                species,
                level,
                first_battle,
            } => (39, species.0, level as u16, first_battle as u16, 0),
            Request::BattleTrainer { trainer, defeat } => (40, trainer.0, defeat.0, 0, 0),
            Request::ChooseStarter([a, b, c]) => (41, a.0, b.0, c.0, 0),
            Request::SetRespawn(id) => (42, id as u16, 0, 0, 0),
            Request::ItemAdd { item, qty } => (43, item.0, qty, 0, 0),
            Request::ItemRemove { item, qty } => (44, item.0, qty, 0, 0),
            Request::ItemCheck { item, qty } => (45, item.0, qty, 0, 0),
            Request::ItemSpace { item, qty } => (46, item.0, qty, 0, 0),
            Request::MoneyAdd(n) => (47, n as u16, (n >> 16) as u16, 0, 0),
            Request::MoneyRemove(n) => (48, n as u16, (n >> 16) as u16, 0, 0),
            Request::MoneyCheck(n) => (49, n as u16, (n >> 16) as u16, 0, 0),
            Request::Buffer { slot, src } => {
                let (kind, v): (u16, u32) = match src {
                    BufferSrc::Species(s) => (0, s.0 as u32),
                    BufferSrc::Item(i) => (1, i.0 as u32),
                    BufferSrc::Move(m) => (2, m.0 as u32),
                    BufferSrc::Number(n) => (3, n),
                    BufferSrc::Std(s) => (4, s as u32),
                    BufferSrc::PartyNick(n) => (5, n as u32),
                    BufferSrc::LeadSpecies => (6, 0),
                    BufferSrc::Text(t) => (7, t.0 as u32),
                };
                (50, slot as u16, kind, v as u16, (v >> 16) as u16)
            }
            Request::Multichoice {
                list,
                x,
                y,
                default,
                cancel,
            } => (
                51,
                list as u16,
                x as u16 | (y as u16) << 8,
                default as u16,
                cancel as u16,
            ),
            Request::SetWarp {
                slot,
                map,
                warp,
                x,
                y,
            } => (
                52,
                map.0,
                (slot as u16) << 8 | warp.map_or(0xFF, |w| w as u16),
                i(x),
                i(y),
            ),
            // The report's text is not hashed (as a log line).
            Request::Report(r) => (53, matches!(r, Report::Degraded(_)) as u16, 0, 0, 0),
            Request::TrainerApproach { obj } => (54, obj as u16, 0, 0, 0),
        };
        h.write(&[tag]);
        for v in [a, b, c, d] {
            h.write_u16(v);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Response {
    Unit,
    Bool(bool),
    Value(u16),
    Pos(i16, i16),
}

impl Response {
    fn bool(self) -> bool {
        match self {
            Response::Bool(b) => b,
            r => panic!("expected Bool, got {r:?}"),
        }
    }

    fn value(self) -> u16 {
        match self {
            Response::Value(v) => v,
            r => panic!("expected Value, got {r:?}"),
        }
    }
}

struct Io {
    request: Option<Request>,
    response: Option<Response>,
    /// The comparison register (Gen3 `comparisonResult`): plain script state, shared by every
    /// clone of the script's [`Ctx`], so it flows across calls. Not hashed (capture needs an
    /// idle runner, which resets it).
    cond: Ordering,
}

impl Default for Io {
    fn default() -> Self {
        Io {
            request: None,
            response: None,
            // Gen3 `comparisonResult` starts at 0 (`<`).
            cond: Ordering::Less,
        }
    }
}

/// A script's handle on the world. Cheap to clone; all clones share one mailbox.
#[derive(Clone)]
pub struct Ctx(Rc<RefCell<Io>>);

/// Future of one [`Request`]: posts it on first poll, resolves when the host answers.
pub struct Call {
    io: Rc<RefCell<Io>>,
    request: Option<Request>,
}

impl Future for Call {
    type Output = Response;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Response> {
        let this = self.get_mut();
        let mut io = this.io.borrow_mut();
        if let Some(r) = this.request.take() {
            debug_assert!(io.request.is_none(), "two requests in flight");
            io.request = Some(r);
            return Poll::Pending;
        }
        match io.response.take() {
            Some(r) => Poll::Ready(r),
            None => Poll::Pending,
        }
    }
}

/// The future of a one-request [`Ctx`] method: a named type, not an `async fn`'s opaque one, so
/// the thousands of generated script bodies that await them type-check faster (Milestone 8
/// Task L). It issues the same request on its first poll as the `async fn` did.
#[must_use = "a request does nothing until awaited"]
pub struct Req<T> {
    call: Call,
    map: fn(Response) -> T,
}

impl<T> Future for Req<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let this = self.get_mut();
        Pin::new(&mut this.call).poll(cx).map(this.map)
    }
}

fn unit(_: Response) {}

impl Ctx {
    pub fn request(&self, request: Request) -> Call {
        Call {
            io: self.0.clone(),
            request: Some(request),
        }
    }

    /// The named future of a one-request method: `r`, its answer mapped by `map`.
    fn req<T>(&self, r: Request, map: fn(Response) -> T) -> Req<T> {
        Req {
            call: self.request(r),
            map,
        }
    }

    pub fn flag(&self, f: FlagId) -> Req<bool> {
        self.req(Request::GetFlag(f), Response::bool)
    }

    pub fn set_flag(&self, f: FlagId, on: bool) -> Req<()> {
        self.req(Request::SetFlag(f, on), unit)
    }

    pub fn var(&self, v: VarId) -> Req<u16> {
        self.req(Request::GetVar(v), Response::value)
    }

    pub fn set_var(&self, v: VarId, value: u16) -> Req<()> {
        self.req(Request::SetVar(v, value), unit)
    }

    pub fn random(&self, n: u16) -> Req<u16> {
        self.req(Request::Random(n), Response::value)
    }

    pub fn player_pos(&self) -> Req<(i16, i16)> {
        self.req(Request::PlayerPos, |r| match r {
            Response::Pos(x, y) => (x, y),
            r => panic!("expected Pos, got {r:?}"),
        })
    }

    pub fn player_facing(&self) -> Req<Dir> {
        self.req(Request::PlayerFacing, |r| Dir::ALL[r.value() as usize])
    }

    pub fn log(&self, what: &'static str) -> Req<()> {
        self.req(Request::Log(what), unit)
    }

    pub fn message(&self, t: TextId) -> Req<()> {
        self.req(Request::Message(t), unit)
    }

    pub fn wait_message(&self) -> Req<()> {
        self.req(Request::WaitMessage, unit)
    }

    pub fn wait_button(&self) -> Req<()> {
        self.req(Request::WaitButton, unit)
    }

    pub fn close_message(&self) -> Req<()> {
        self.req(Request::CloseMessage, unit)
    }

    /// `message` + `wait_message` + `wait_button`.
    pub async fn say(&self, t: TextId) {
        self.message(t).await;
        self.wait_message().await;
        self.wait_button().await;
    }

    pub fn yes_no(&self) -> Req<bool> {
        self.req(Request::YesNo, Response::bool)
    }

    pub fn lock(&self) -> Req<()> {
        self.req(Request::Lock, unit)
    }

    pub fn lock_all(&self) -> Req<()> {
        self.req(Request::LockAll, unit)
    }

    pub fn release(&self) -> Req<()> {
        self.req(Request::Release, unit)
    }

    pub fn release_all(&self) -> Req<()> {
        self.req(Request::ReleaseAll, unit)
    }

    pub fn face_player(&self) -> Req<()> {
        self.req(Request::FacePlayer, unit)
    }

    pub fn move_obj(&self, obj: u8, steps: &'static [Step]) -> Req<()> {
        self.req(Request::Move { obj, steps }, unit)
    }

    pub fn wait_move(&self, obj: u8) -> Req<()> {
        self.req(Request::WaitMove(obj), unit)
    }

    pub fn set_object_pos(&self, obj: u8, x: i16, y: i16) -> Req<()> {
        self.req(Request::SetObjectPos { obj, x, y }, unit)
    }

    pub fn show_object(&self, obj: u8) -> Req<()> {
        self.req(Request::ShowObject(obj), unit)
    }

    pub fn hide_object(&self, obj: u8) -> Req<()> {
        self.req(Request::HideObject(obj), unit)
    }

    pub fn add_object(&self, obj: u8) -> Req<()> {
        self.req(Request::AddObject(obj), unit)
    }

    pub fn remove_object(&self, obj: u8) -> Req<()> {
        self.req(Request::RemoveObject(obj), unit)
    }

    pub fn turn(&self, obj: u8, dir: Dir) -> Req<()> {
        self.req(Request::Turn { obj, dir }, unit)
    }

    pub fn set_movement_type(&self, obj: u8, movement: MovementType) -> Req<()> {
        self.req(Request::SetMovementType { obj, movement }, unit)
    }

    pub fn delay(&self, frames: u16) -> Req<()> {
        self.req(Request::Delay(frames), unit)
    }

    pub fn fade(&self, f: Fade) -> Req<()> {
        self.req(Request::Fade(f), unit)
    }

    pub fn warp(&self, map: MapId, warp: Option<u8>, x: i16, y: i16) -> Req<()> {
        self.req(Request::Warp { map, warp, x, y }, unit)
    }

    pub fn set_metatile(&self, x: i16, y: i16, metatile: u16, blocked: bool) -> Req<()> {
        self.req(
            Request::SetMetatile {
                x,
                y,
                metatile,
                blocked,
            },
            unit,
        )
    }

    pub fn redraw_map(&self) -> Req<()> {
        self.req(Request::RedrawMap, unit)
    }

    pub fn set_object_template_pos(&self, obj: u8, x: i16, y: i16) -> Req<()> {
        self.req(Request::SetObjectTemplatePos { obj, x, y }, unit)
    }

    pub fn door(&self, x: i16, y: i16, open: bool) -> Req<()> {
        self.req(Request::Door { x, y, open }, unit)
    }

    pub fn wait_door(&self) -> Req<()> {
        self.req(Request::WaitDoor, unit)
    }

    pub fn wait_state(&self) -> Req<()> {
        self.req(Request::WaitState, unit)
    }

    /// Gen3 `givemon`: `MON_GIVEN_TO_PARTY` (0), `MON_GIVEN_TO_PC` (1) or `MON_CANT_GIVE` (2).
    pub fn give_mon(&self, species: SpeciesId, level: u8) -> Req<u16> {
        self.req(Request::GiveMon { species, level }, Response::value)
    }

    pub fn party_size(&self) -> Req<u16> {
        self.req(Request::PartySize, Response::value)
    }

    pub fn heal_party(&self) -> Req<()> {
        self.req(Request::HealParty, unit)
    }

    pub fn battle_wild(
        &self,
        species: SpeciesId,
        level: u8,
        first_battle: bool,
    ) -> Req<BattleOutcome> {
        self.req(
            Request::BattleWild {
                species,
                level,
                first_battle,
            },
            outcome,
        )
    }

    pub fn battle_trainer(&self, trainer: TrainerId, defeat: TextId) -> Req<BattleOutcome> {
        self.req(Request::BattleTrainer { trainer, defeat }, outcome)
    }

    /// The index (0..3) of the chosen species.
    pub fn choose_starter(&self, choices: [SpeciesId; 3]) -> Req<u8> {
        self.req(Request::ChooseStarter(choices), |r| r.value() as u8)
    }

    pub fn set_respawn(&self, heal_location: u8) -> Req<()> {
        self.req(Request::SetRespawn(heal_location), unit)
    }

    // ---- Milestone 8 ----

    /// The comparison register (Gen3 `comparisonResult`).
    pub fn cond(&self) -> Ordering {
        self.0.borrow().cond
    }

    pub fn set_cond(&self, o: Ordering) {
        self.0.borrow_mut().cond = o;
    }

    pub fn item_add(&self, item: ItemId, qty: u16) -> Req<bool> {
        self.req(Request::ItemAdd { item, qty }, Response::bool)
    }

    pub fn item_remove(&self, item: ItemId, qty: u16) -> Req<bool> {
        self.req(Request::ItemRemove { item, qty }, Response::bool)
    }

    pub fn item_check(&self, item: ItemId, qty: u16) -> Req<bool> {
        self.req(Request::ItemCheck { item, qty }, Response::bool)
    }

    pub fn item_space(&self, item: ItemId, qty: u16) -> Req<bool> {
        self.req(Request::ItemSpace { item, qty }, Response::bool)
    }

    pub fn money_add(&self, n: u32) -> Req<()> {
        self.req(Request::MoneyAdd(n), unit)
    }

    pub fn money_remove(&self, n: u32) -> Req<()> {
        self.req(Request::MoneyRemove(n), unit)
    }

    pub fn money_check(&self, n: u32) -> Req<bool> {
        self.req(Request::MoneyCheck(n), Response::bool)
    }

    pub fn buffer(&self, slot: u8, src: BufferSrc) -> Req<()> {
        self.req(Request::Buffer { slot, src }, unit)
    }

    /// The chosen index, or [`MULTI_B_PRESSED`].
    pub fn multichoice(&self, list: u8, x: u8, y: u8, default: u8, cancel: bool) -> Req<u16> {
        self.req(
            Request::Multichoice {
                list,
                x,
                y,
                default,
                cancel,
            },
            Response::value,
        )
    }

    pub fn set_warp(
        &self,
        slot: WarpSlot,
        map: MapId,
        warp: Option<u8>,
        x: i16,
        y: i16,
    ) -> Req<()> {
        self.req(
            Request::SetWarp {
                slot,
                map,
                warp,
                x,
                y,
            },
            unit,
        )
    }

    pub fn report(&self, r: Report) -> Req<()> {
        self.req(Request::Report(r), unit)
    }

    pub fn trainer_approach(&self, obj: u8) -> Req<()> {
        self.req(Request::TrainerApproach { obj }, unit)
    }
}

fn outcome(r: Response) -> BattleOutcome {
    let v = r.value();
    BattleOutcome::from_u16(v).unwrap_or_else(|| panic!("expected a B_OUTCOME_* value, got {v}"))
}

/// The world side of scripts.
pub trait ScriptHost {
    /// A new request. Return `Some` to answer it now (the script continues this frame), or
    /// `None` to start it; it is then passed to `resume` once per later frame until answered.
    fn begin(&mut self, request: &Request) -> Option<Response>;
    fn resume(&mut self, request: &Request) -> Option<Response>;
}

/// Instant requests answered in one frame before the runner yields anyway, so a script that
/// loops without waiting cannot hang the frame.
pub const INSTANT_LIMIT: u32 = 1024;

/// Runs at most one script at a time.
pub struct Runner {
    task: Option<Script>,
    io: Rc<RefCell<Io>>,
    active: Option<Request>,
    /// Requests the current script has issued (its progress, for the parity hash).
    issued: u32,
}

impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner {
    pub fn new() -> Self {
        Self {
            task: None,
            io: Rc::new(RefCell::new(Io::default())),
            active: None,
            issued: 0,
        }
    }

    /// An idle runner whose last script issued `issued` requests (checkpoint restore: only
    /// `issued` survives the end of a script and is hashed).
    pub fn restore_idle(issued: u32) -> Runner {
        Self {
            issued,
            ..Self::new()
        }
    }

    /// The handle to pass to the script being started.
    pub fn ctx(&self) -> Ctx {
        Ctx(self.io.clone())
    }

    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }

    /// The request the script is waiting on, if any.
    pub fn waiting_on(&self) -> Option<&Request> {
        self.active.as_ref()
    }

    /// Same as [`Runner::waiting_on`].
    pub fn active(&self) -> Option<&Request> {
        self.active.as_ref()
    }

    /// Requests the current script has issued so far.
    pub const fn issued(&self) -> u32 {
        self.issued
    }

    /// Feeds whether a script runs, its progress and the request it waits on into `h` (O(1)).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.is_running() as u8]);
        h.write_u32(self.issued);
        match &self.active {
            None => h.write(&[0xFF]),
            Some(r) => r.hash_into(h),
        }
    }

    /// Drops the running script and any pending request (Gen3 whiteout resets the script
    /// context). `issued` is kept, as when a script ends.
    pub fn abort(&mut self) {
        self.task = None;
        self.active = None;
        *self.io.borrow_mut() = Io::default();
    }

    /// Starts `script` (built with `self.ctx()`); it first runs on the next `step`.
    pub fn start(&mut self, script: Script) {
        assert!(self.task.is_none(), "a script is already running");
        *self.io.borrow_mut() = Io::default();
        self.issued = 0;
        self.task = Some(script);
    }

    /// Runs the script for one frame. Returns whether it is still running afterwards.
    pub fn step(&mut self, host: &mut impl ScriptHost) -> bool {
        if let Some(req) = &self.active {
            match host.resume(req) {
                Some(resp) => {
                    self.active = None;
                    self.io.borrow_mut().response = Some(resp);
                }
                None => return true,
            }
        }
        let mut cx = Context::from_waker(Waker::noop());
        for _ in 0..INSTANT_LIMIT {
            let Some(task) = self.task.as_mut() else {
                return false;
            };
            if task.as_mut().poll(&mut cx).is_ready() {
                self.task = None;
                return false;
            }
            let Some(req) = self.io.borrow_mut().request.take() else {
                // Pending on something that is not a request: just wait for the next frame.
                return true;
            };
            self.issued = self.issued.wrapping_add(1);
            match host.begin(&req) {
                Some(resp) => self.io.borrow_mut().response = Some(resp),
                None => {
                    self.active = Some(req);
                    return true;
                }
            }
        }
        true
    }
}

/// FNV-1a-32 over a stable encoding of each step (tag byte + args); the identity of a step
/// table (Milestone 5 Task A).
///
/// Encoding (stable: snapshots and hashes depend on it): one tag byte per variant in
/// declaration order (`Face` 0 … `Emote` 11), then its arguments as bytes: `Dir as u8`,
/// [`Speed`] as `Slow` 0 … `Faster` 3, the jump `cells`, the `Delay` frames, [`Emote`] as
/// `Exclamation` 0 … `Heart` 2.
pub const fn steps_digest(steps: &[Step]) -> u32 {
    const fn mix(h: u32, b: u8) -> u32 {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    }
    let mut h: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < steps.len() {
        let (tag, args, n): (u8, [u8; 2], usize) = match steps[i] {
            Step::Face(d) => (0, [d as u8, 0], 1),
            Step::Walk(d, s) => (1, [d as u8, s as u8], 2),
            Step::WalkInPlace(d, s) => (2, [d as u8, s as u8], 2),
            Step::Jump(d, cells) => (3, [d as u8, cells], 2),
            Step::Delay(n) => (4, [n, 0], 1),
            Step::FacePlayer => (5, [0; 2], 0),
            Step::FaceAwayFromPlayer => (6, [0; 2], 0),
            Step::LockFacing => (7, [0; 2], 0),
            Step::UnlockFacing => (8, [0; 2], 0),
            Step::Hide => (9, [0; 2], 0),
            Step::Show => (10, [0; 2], 0),
            Step::Emote(e) => (11, [e as u8, 0], 1),
        };
        h = mix(h, tag);
        let mut j = 0;
        while j < n {
            h = mix(h, args[j]);
            j += 1;
        }
        i += 1;
    }
    h
}

#[cfg(test)]
mod tests;
