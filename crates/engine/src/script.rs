//! Async scripts. A script is a plain `async fn(Ctx)` in `content`. It never touches `World`:
//! every effect is a [`Request`] posted through its [`Ctx`] mailbox. Each frame the [`Runner`]
//! polls the script and hands requests to a [`ScriptHost`] (the world). Instant requests (flag
//! reads, var writes…) are answered within the same frame; the others (printing text, walking,
//! fading…) suspend the script until the host reports completion on a later frame.

use crate::battle::BattleOutcome;
use crate::data::{Dir, FlagId, MapId, MovementType, SpeciesId, TextId, TrainerId, VarId};
use crate::hash::Fnv64;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

/// A running script instance.
pub type Script = Pin<Box<dyn Future<Output = ()>>>;

/// Movement speed of one step (Gen3 `walk_slow` … `walk_faster`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Speed {
    Slow,
    Normal,
    Fast,
    Faster,
}

/// Emote bubble shown above an object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Emote {
    Exclamation,
    Question,
    Heart,
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
}

impl Request {
    /// Feeds the request's kind and arguments into `h` (O(1): a movement script counts by its
    /// length, a log line not at all).
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
            Request::Move { obj, steps } => (18, obj as u16, steps.len() as u16, 0, 0),
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

#[derive(Default)]
struct Io {
    request: Option<Request>,
    response: Option<Response>,
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

impl Ctx {
    pub fn request(&self, request: Request) -> Call {
        Call {
            io: self.0.clone(),
            request: Some(request),
        }
    }

    async fn unit(&self, r: Request) {
        self.request(r).await;
    }

    pub async fn flag(&self, f: FlagId) -> bool {
        self.request(Request::GetFlag(f)).await.bool()
    }

    pub async fn set_flag(&self, f: FlagId, on: bool) {
        self.unit(Request::SetFlag(f, on)).await
    }

    pub async fn var(&self, v: VarId) -> u16 {
        self.request(Request::GetVar(v)).await.value()
    }

    pub async fn set_var(&self, v: VarId, value: u16) {
        self.unit(Request::SetVar(v, value)).await
    }

    pub async fn random(&self, n: u16) -> u16 {
        self.request(Request::Random(n)).await.value()
    }

    pub async fn player_pos(&self) -> (i16, i16) {
        match self.request(Request::PlayerPos).await {
            Response::Pos(x, y) => (x, y),
            r => panic!("expected Pos, got {r:?}"),
        }
    }

    pub async fn player_facing(&self) -> Dir {
        Dir::ALL[self.request(Request::PlayerFacing).await.value() as usize]
    }

    pub async fn log(&self, what: &'static str) {
        self.unit(Request::Log(what)).await
    }

    pub async fn message(&self, t: TextId) {
        self.unit(Request::Message(t)).await
    }

    pub async fn wait_message(&self) {
        self.unit(Request::WaitMessage).await
    }

    pub async fn wait_button(&self) {
        self.unit(Request::WaitButton).await
    }

    pub async fn close_message(&self) {
        self.unit(Request::CloseMessage).await
    }

    /// `message` + `wait_message` + `wait_button`.
    pub async fn say(&self, t: TextId) {
        self.message(t).await;
        self.wait_message().await;
        self.wait_button().await;
    }

    pub async fn yes_no(&self) -> bool {
        self.request(Request::YesNo).await.bool()
    }

    pub async fn lock(&self) {
        self.unit(Request::Lock).await
    }

    pub async fn lock_all(&self) {
        self.unit(Request::LockAll).await
    }

    pub async fn release(&self) {
        self.unit(Request::Release).await
    }

    pub async fn release_all(&self) {
        self.unit(Request::ReleaseAll).await
    }

    pub async fn face_player(&self) {
        self.unit(Request::FacePlayer).await
    }

    pub async fn move_obj(&self, obj: u8, steps: &'static [Step]) {
        self.unit(Request::Move { obj, steps }).await
    }

    pub async fn wait_move(&self, obj: u8) {
        self.unit(Request::WaitMove(obj)).await
    }

    pub async fn set_object_pos(&self, obj: u8, x: i16, y: i16) {
        self.unit(Request::SetObjectPos { obj, x, y }).await
    }

    pub async fn show_object(&self, obj: u8) {
        self.unit(Request::ShowObject(obj)).await
    }

    pub async fn hide_object(&self, obj: u8) {
        self.unit(Request::HideObject(obj)).await
    }

    pub async fn add_object(&self, obj: u8) {
        self.unit(Request::AddObject(obj)).await
    }

    pub async fn remove_object(&self, obj: u8) {
        self.unit(Request::RemoveObject(obj)).await
    }

    pub async fn turn(&self, obj: u8, dir: Dir) {
        self.unit(Request::Turn { obj, dir }).await
    }

    pub async fn set_movement_type(&self, obj: u8, movement: MovementType) {
        self.unit(Request::SetMovementType { obj, movement }).await
    }

    pub async fn delay(&self, frames: u16) {
        self.unit(Request::Delay(frames)).await
    }

    pub async fn fade(&self, f: Fade) {
        self.unit(Request::Fade(f)).await
    }

    pub async fn warp(&self, map: MapId, warp: Option<u8>, x: i16, y: i16) {
        self.unit(Request::Warp { map, warp, x, y }).await
    }

    pub async fn set_metatile(&self, x: i16, y: i16, metatile: u16, blocked: bool) {
        self.unit(Request::SetMetatile {
            x,
            y,
            metatile,
            blocked,
        })
        .await
    }

    pub async fn redraw_map(&self) {
        self.unit(Request::RedrawMap).await
    }

    pub async fn set_object_template_pos(&self, obj: u8, x: i16, y: i16) {
        self.unit(Request::SetObjectTemplatePos { obj, x, y }).await
    }

    pub async fn door(&self, x: i16, y: i16, open: bool) {
        self.unit(Request::Door { x, y, open }).await
    }

    pub async fn wait_door(&self) {
        self.unit(Request::WaitDoor).await
    }

    pub async fn wait_state(&self) {
        self.unit(Request::WaitState).await
    }

    /// Gen3 `givemon`: `MON_GIVEN_TO_PARTY` (0), `MON_GIVEN_TO_PC` (1) or `MON_CANT_GIVE` (2).
    pub async fn give_mon(&self, species: SpeciesId, level: u8) -> u16 {
        self.request(Request::GiveMon { species, level })
            .await
            .value()
    }

    pub async fn party_size(&self) -> u16 {
        self.request(Request::PartySize).await.value()
    }

    pub async fn heal_party(&self) {
        self.unit(Request::HealParty).await
    }

    pub async fn battle_wild(
        &self,
        species: SpeciesId,
        level: u8,
        first_battle: bool,
    ) -> BattleOutcome {
        outcome(
            self.request(Request::BattleWild {
                species,
                level,
                first_battle,
            })
            .await,
        )
    }

    pub async fn battle_trainer(&self, trainer: TrainerId, defeat: TextId) -> BattleOutcome {
        outcome(
            self.request(Request::BattleTrainer { trainer, defeat })
                .await,
        )
    }

    /// The index (0..3) of the chosen species.
    pub async fn choose_starter(&self, choices: [SpeciesId; 3]) -> u8 {
        self.request(Request::ChooseStarter(choices)).await.value() as u8
    }

    pub async fn set_respawn(&self, heal_location: u8) {
        self.unit(Request::SetRespawn(heal_location)).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Answers flag/var requests instantly; `Delay(n)` takes n frames.
    #[derive(Default)]
    struct Host {
        flags: [bool; 8],
        vars: [u16; 8],
        log: Vec<Request>,
        frames_left: u16,
    }

    impl ScriptHost for Host {
        fn begin(&mut self, r: &Request) -> Option<Response> {
            self.log.push(r.clone());
            match *r {
                Request::GetFlag(f) => Some(Response::Bool(self.flags[f.0 as usize])),
                Request::SetFlag(f, on) => {
                    self.flags[f.0 as usize] = on;
                    Some(Response::Unit)
                }
                Request::GetVar(v) => Some(Response::Value(self.vars[v.0 as usize])),
                Request::SetVar(v, x) => {
                    self.vars[v.0 as usize] = x;
                    Some(Response::Unit)
                }
                Request::Delay(n) => {
                    self.frames_left = n;
                    None
                }
                _ => Some(Response::Unit),
            }
        }

        fn resume(&mut self, r: &Request) -> Option<Response> {
            assert!(matches!(r, Request::Delay(_)));
            self.frames_left -= 1;
            (self.frames_left == 0).then_some(Response::Unit)
        }
    }

    async fn sample(ctx: Ctx) {
        if !ctx.flag(FlagId(1)).await {
            ctx.set_var(VarId(2), 7).await;
        }
        ctx.delay(3).await;
        ctx.set_flag(FlagId(1), true).await;
    }

    #[test]
    fn instant_requests_run_within_one_frame_and_delays_span_frames() {
        let mut r = Runner::new();
        let mut h = Host::default();
        r.start(Box::pin(sample(r.ctx())));
        assert!(r.step(&mut h)); // GetFlag, SetVar, then Delay(3) begins
        assert_eq!(h.vars[2], 7);
        assert_eq!(r.waiting_on(), Some(&Request::Delay(3)));
        assert!(r.step(&mut h)); // 2 left
        assert!(r.step(&mut h)); // 1 left
        assert!(!h.flags[1]);
        assert!(!r.step(&mut h)); // delay done, SetFlag, script ends
        assert!(h.flags[1]);
        assert!(!r.is_running());
        assert_eq!(h.log.len(), 4);
    }

    async fn busy(ctx: Ctx) {
        loop {
            ctx.flag(FlagId(0)).await;
        }
    }

    #[test]
    fn a_script_that_never_waits_still_yields_each_frame() {
        let mut r = Runner::new();
        let mut h = Host::default();
        r.start(Box::pin(busy(r.ctx())));
        assert!(r.step(&mut h));
        assert_eq!(h.log.len(), INSTANT_LIMIT as usize);
        assert!(r.step(&mut h));
    }

    #[test]
    fn abort_drops_the_script_and_its_pending_request() {
        let mut r = Runner::new();
        let mut h = Host::default();
        r.start(Box::pin(sample(r.ctx())));
        assert!(r.step(&mut h));
        assert_eq!(r.waiting_on(), Some(&Request::Delay(3)));
        r.abort();
        assert!(!r.is_running());
        assert_eq!(r.waiting_on(), None);
        assert!(!r.step(&mut h));
        assert!(!h.flags[1], "the rest of the script never ran");
        r.start(Box::pin(sample(r.ctx())));
        assert!(r.step(&mut h));
    }

    async fn battles(ctx: Ctx) {
        let i = ctx
            .choose_starter([SpeciesId(1), SpeciesId(2), SpeciesId(3)])
            .await;
        ctx.set_var(VarId(0), i as u16).await;
        let o = ctx.battle_wild(SpeciesId(4), 2, true).await;
        ctx.set_var(VarId(1), o as u16).await;
        let o = ctx.battle_trainer(TrainerId(5), TextId(6)).await;
        ctx.set_var(VarId(2), o as u16).await;
        let n = ctx.give_mon(SpeciesId(1), 5).await;
        ctx.set_var(VarId(3), n).await;
        ctx.set_var(VarId(4), ctx.party_size().await).await;
        ctx.heal_party().await;
        ctx.set_respawn(7).await;
    }

    /// Answers the M4 requests with fixed values.
    struct BattleHost(Vec<Request>, [u16; 8]);

    impl ScriptHost for BattleHost {
        fn begin(&mut self, r: &Request) -> Option<Response> {
            self.0.push(r.clone());
            Some(match *r {
                Request::ChooseStarter(_) => Response::Value(2),
                Request::BattleWild { .. } => Response::Value(BattleOutcome::MonFled as u16),
                Request::BattleTrainer { .. } => Response::Value(BattleOutcome::Won as u16),
                Request::GiveMon { .. } => Response::Value(0),
                Request::PartySize => Response::Value(1),
                Request::SetVar(v, x) => {
                    self.1[v.0 as usize] = x;
                    Response::Unit
                }
                _ => Response::Unit,
            })
        }

        fn resume(&mut self, _: &Request) -> Option<Response> {
            unreachable!()
        }
    }

    #[test]
    fn battle_requests_round_trip_through_ctx() {
        let mut r = Runner::new();
        let mut h = BattleHost(Vec::new(), [0; 8]);
        r.start(Box::pin(battles(r.ctx())));
        assert!(!r.step(&mut h));
        assert_eq!(h.1[..5], [2, 6, 1, 0, 1]);
        assert!(h.0.contains(&Request::SetRespawn(7)));
        assert!(h.0.contains(&Request::HealParty));
    }

    #[test]
    fn battle_request_hashes_differ() {
        let hash = |r: Request| {
            let mut h = Fnv64::new();
            r.hash_into(&mut h);
            h.finish()
        };
        let all = [
            Request::GiveMon {
                species: SpeciesId(1),
                level: 5,
            },
            Request::PartySize,
            Request::HealParty,
            Request::BattleWild {
                species: SpeciesId(1),
                level: 5,
                first_battle: false,
            },
            Request::BattleWild {
                species: SpeciesId(1),
                level: 5,
                first_battle: true,
            },
            Request::BattleTrainer {
                trainer: TrainerId(1),
                defeat: TextId(5),
            },
            Request::ChooseStarter([SpeciesId(1), SpeciesId(5), SpeciesId(0)]),
            Request::SetRespawn(1),
        ];
        let hashes: Vec<u64> = all.iter().cloned().map(hash).collect();
        for (i, a) in hashes.iter().enumerate() {
            for b in &hashes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_finished_runner_accepts_a_new_script() {
        let mut r = Runner::new();
        let mut h = Host::default();
        h.flags[1] = true;
        r.start(Box::pin(async {}));
        assert!(!r.step(&mut h));
        r.start(Box::pin(sample(r.ctx())));
        assert!(r.step(&mut h));
        assert_eq!(h.vars[2], 0, "flag was set, var untouched");
    }
}
