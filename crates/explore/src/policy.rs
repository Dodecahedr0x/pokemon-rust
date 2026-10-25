//! Battle and menu policy (Task F). Its keys are intents: the driver applies the tap rule
//! (a release frame between two presses of the same keys, nothing while a battle or a warp has
//! the screen black).
//!
//! The battle cursor walk is the parity scenario's (`game_native::scenario`, `battle_key`),
//! adapted to [`Opts`]: v0 has a one-mon party, no bag and no switching (decision 18).

use crate::model::Opts;
use engine::battle::scene::OpenMenu;
use engine::battle::{BattleKind, BattleMon, Side};
use engine::data::{MoveId, SpeciesId};
use engine::mon::{Ability, BattleTables, Type};
use engine::ui::party::{Action as PartyAction, CONFIRM, PartyMenu, Prompt as PartyPrompt};
use engine::ui::{BagMode, PartyMode, ScreenKind, ScreenState};
use engine::{Buttons, World};

pub struct Policy {
    /// The last intent given (diagnostics; the tap rule is the driver's).
    last: Buttons,
    /// The yes/no prompts seen so far (counted on the frame each opens).
    yes_nos: u32,
    /// Whether a yes/no was open on the previous script frame.
    yes_no_open: bool,
}

/// [`Opts::choice`] value: yes to the first yes/no of the macro, no to every later one (a
/// script that offers again after a yes, the Route 111 rest stop's old lady).
pub const YES_ONCE: u8 = 3;

impl Default for Policy {
    fn default() -> Self {
        Policy::new()
    }
}

/// RUN in the 2x2 action menu (FIGHT 0, BAG 1, POKéMON 2, RUN 3).
const RUN: u8 = 3;
const FIGHT: u8 = 0;
const BAG: u8 = 1;

/// The key that moves a 2x2 menu cursor from `c` toward `want` (column first, then row), or A
/// once it is there.
pub fn toward(c: u8, want: u8) -> Buttons {
    if c & 1 != want & 1 {
        if want & 1 == 0 {
            Buttons::LEFT
        } else {
            Buttons::RIGHT
        }
    } else if c & 2 != want & 2 {
        if want & 2 == 0 {
            Buttons::UP
        } else {
            Buttons::DOWN
        }
    } else {
        Buttons::A
    }
}

impl Policy {
    pub fn new() -> Policy {
        Policy {
            last: Buttons::NONE,
            yes_nos: 0,
            yes_no_open: false,
        }
    }

    /// The last intent returned by [`Policy::battle`] or [`Policy::script`].
    pub fn last(&self) -> Buttons {
        self.last
    }

    /// Next intent while a battle owns the frame: first damaging usable move (power > 0, pp > 0;
    /// else the first move with pp > 0; else slot 0, where the engine uses Struggle), RUN from wild battles when `opts.run_wild` (never the first battle or a
    /// trainer), keep known moves (B on learn prompts), A through messages.
    pub fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons {
        self.battle_with(w, t, opts, &|_| false)
    }

    /// [`Policy::battle`], never forgetting a move `is_hm` names (Gen3 refuses: `IsHMMove2`).
    pub fn battle_with(
        &mut self,
        w: &World,
        t: &BattleTables,
        opts: Opts,
        is_hm: &dyn Fn(MoveId) -> bool,
    ) -> Buttons {
        let k = match w.battle() {
            None => Buttons::A,
            Some((_, battle, scene)) => {
                let wild = matches!(
                    battle.setup().kind,
                    BattleKind::Wild {
                        first_battle: false
                    }
                );
                // `opts.catch`: balls at that species while there are any, else run.
                let target =
                    opts.catch != 0 && battle.mon(Side::Opponent).species == SpeciesId(opts.catch);
                let throw = wild && target && scene.can_throw();
                let run = wild && !throw && (opts.run_wild || opts.catch != 0);
                match scene.open_menu() {
                    Some(OpenMenu::Action(c)) if throw => toward(c, BAG),
                    Some(OpenMenu::Move(_)) if throw => Buttons::B,
                    Some(OpenMenu::Action(c)) => toward(c, if run { RUN } else { FIGHT }),
                    // Never fight when running: back out to the action menu.
                    Some(OpenMenu::Move(_)) if run => Buttons::B,
                    Some(OpenMenu::Move(c)) if opts.best_move => {
                        toward(c, best_selectable_slot(battle, t))
                    }
                    Some(OpenMenu::Move(c)) => toward(c, first_selectable_slot(battle, t)),
                    // Doubles: the cursor (RIGHT cycles) onto the foe the move was picked for.
                    Some(OpenMenu::Target(c)) => target_key(battle, t, c, opts.best_move),
                    // Learn a stronger level-up move over the weakest known one.
                    Some(OpenMenu::LearnAsk(c))
                        if opts.learn
                            && forget_slot(battle.mon(Side::Player), t, is_hm).is_some() =>
                    {
                        if c == 0 { Buttons::A } else { Buttons::UP }
                    }
                    Some(OpenMenu::LearnPick(c)) if opts.learn => {
                        match forget_slot(battle.mon(Side::Player), t, is_hm) {
                            Some(_) if c > 3 => Buttons::LEFT,
                            Some(s) => toward(c, s),
                            None => Buttons::B,
                        }
                    }
                    // Keep the known moves (Gen3: "Stop learning X?" yes).
                    Some(OpenMenu::LearnAsk(_) | OpenMenu::LearnPick(_)) => Buttons::B,
                    Some(OpenMenu::StopAsk(0)) => Buttons::A,
                    Some(OpenMenu::StopAsk(_)) => Buttons::UP,
                    None => Buttons::A,
                }
            }
        };
        self.last = k;
        k
    }

    /// Next intent while a script or the starter screen runs: A through text; yes/no -> A for
    /// choice 0 (and the first one for [`YES_ONCE`]), B otherwise; a multichoice -> move to choice `opts.choice`, then A; starter ->
    /// move to ball `opts.choice % 3`, then A.
    ///
    /// B on every yes/no when `choice != 0` can cycle on a script that re-asks until it gets a
    /// yes: the driver's `settle_frames`/`macro_frames` limits end such a macro (a `Timeout`).
    pub fn script(&mut self, w: &World, opts: Opts) -> Buttons {
        let yes_no = w.text().is_yes_no_active();
        if yes_no && !self.yes_no_open {
            self.yes_nos += 1;
        }
        self.yes_no_open = yes_no;
        let shop = (w.screens().and_then(|s| s.top())).is_some_and(|(k, _)| {
            matches!(k, ScreenKind::Mart { .. } | ScreenKind::Bag(BagMode::Sell))
        });
        let half = match w.screens().and_then(|s| s.top()) {
            Some((ScreenKind::Party(PartyMode::ChooseHalf { max, .. }), ScreenState::Party(p))) => {
                Some(choose_half(w, p, *max))
            }
            _ => None,
        };
        // The Easy Chat screen (Wave G): B to its quit prompt, then YES (it opens on NO).
        let easy_chat = match w.screens().and_then(|s| s.top()) {
            Some((_, ScreenState::EasyChat(e))) => {
                use engine::ui::easy_chat::Prompt;
                Some(match e.prompt() {
                    Prompt::Quit { cursor: 0 }
                    | Prompt::Confirm { cursor: 0 }
                    | Prompt::Delete { cursor: 0 } => Buttons::A,
                    Prompt::Quit { .. } | Prompt::Confirm { .. } | Prompt::Delete { .. } => {
                        Buttons::UP
                    }
                    Prompt::CannotQuit { .. } => Buttons::A,
                    Prompt::Keyboard { .. } | Prompt::Words { .. } => Buttons::B,
                    Prompt::Phrase { .. } | Prompt::Buttons { .. } => Buttons::B,
                })
            }
            _ => None,
        };
        // The credits pages ignore START (Gen3 has no skip). A is harmless there; THE END
        // takes any key, so the driver waits the pages out.
        let k = if let Some(k) = half.or(easy_chat) {
            k
        } else if shop {
            // A clerk's mart (or its SELL bag): B leaves without buying (A would buy the
            // first item until the money runs out); `Step::Buy` drives a purchase.
            Buttons::B
        } else if let Some(starter) = w.starter() {
            // Its own "Do you choose this?" yes/no starts on yes: A confirms the ball.
            let want = opts.choice % 3;
            match starter.selection().cmp(&want) {
                std::cmp::Ordering::Less => Buttons::RIGHT,
                std::cmp::Ordering::Greater => Buttons::LEFT,
                std::cmp::Ordering::Equal => Buttons::A,
            }
        } else if let Some(m) = w.text().multichoice() {
            // A script multichoice: move to choice `opts.choice`, then A.
            match m.cursor().cmp(&opts.choice) {
                std::cmp::Ordering::Less => Buttons::DOWN,
                std::cmp::Ordering::Greater => Buttons::UP,
                std::cmp::Ordering::Equal => Buttons::A,
            }
        } else if yes_no {
            // The cursor starts on yes; B answers no wherever it is.
            if opts.choice == 0 || (opts.choice == YES_ONCE && self.yes_nos == 1) {
                Buttons::A
            } else {
                Buttons::B
            }
        } else {
            Buttons::A
        };
        self.last = k;
        k
    }
}

/// The choose-half party menu (Steven's multi battle): ENTER the party's mons with HP in party
/// order, up to `max`, then CONFIRM (START moves there); "Cancel the battle?" is answered no.
fn choose_half(w: &World, p: &PartyMenu, max: u8) -> Buttons {
    let order = p.order();
    let entered = order.iter().filter(|&&o| o != 0).count();
    match p.prompt() {
        PartyPrompt::Slot { cursor } => {
            let next = (entered < max as usize)
                .then(|| {
                    (w.party().iter().enumerate())
                        .find(|&(i, m)| m.hp > 0 && !order.contains(&(i as u8 + 1)))
                        .map(|(i, _)| i as u8)
                })
                .flatten();
            match next {
                Some(i) if cursor == i => Buttons::A,
                // CONFIRM and CANCEL sit below the slots: UP walks back to them.
                Some(i) if cursor > i => Buttons::UP,
                Some(_) => Buttons::DOWN,
                None if cursor == CONFIRM => Buttons::A,
                None => Buttons::START,
            }
        }
        PartyPrompt::Actions { .. } => {
            let at = p.action_cursor().unwrap_or(0) as usize;
            match p.actions().iter().position(|&a| a == PartyAction::Enter) {
                Some(i) if i == at => Buttons::A,
                Some(i) if i < at => Buttons::UP,
                Some(_) => Buttons::DOWN,
                None => Buttons::B,
            }
        }
        PartyPrompt::Message => Buttons::A,
        _ => Buttons::B,
    }
}

/// The move slot to use: the first with power > 0 and pp > 0, else the first with a move and
/// pp > 0 (a status move beats selecting a slot without PP, which the engine refuses forever),
/// else slot 0 (no PP anywhere: the engine uses Struggle).
pub fn first_damaging_slot(m: &BattleMon, t: &BattleTables) -> u8 {
    first_damaging_slot_of(&m.moves, &m.pp, t)
}

/// [`first_damaging_slot`] on raw move slots (the vanilla oracle reads them from RAM).
pub fn first_damaging_slot_of(moves: &[Option<MoveId>; 4], pp: &[u8; 4], t: &BattleTables) -> u8 {
    let usable = |i: usize| moves[i].is_some() && pp[i] > 0;
    let damaging = |i: usize| {
        usable(i)
            && moves[i]
                .and_then(|id| t.moves.get(id.0 as usize))
                .is_some_and(|mi| mi.power > 0)
    };
    (0..4)
        .find(|&i| damaging(i))
        .or_else(|| (0..4).find(|&i| usable(i)))
        .unwrap_or(0) as u8
}

/// The move slot of the best expected damage on `foe`: power x STAB (1.5) x the type chart's
/// multipliers on the foe's types, over the usable moves with power; ties go to the lower slot.
/// Without a damaging move, [`first_damaging_slot`].
/// A move's worth for [`forget_slot`]: power, x1.5 with STAB; 0 for a status move.
fn move_worth(me: SpeciesId, mov: MoveId, t: &BattleTables) -> u32 {
    let Some(mi) = t.moves.get(mov.0 as usize) else {
        return 0;
    };
    let stab = t.species_info(me).types.contains(&mi.ty);
    u32::from(mi.power) * if stab { 15 } else { 10 }
}

/// The slot to forget for the level-up move `me` is learning (its learnset's first move at its
/// level it does not know): the weakest known move ([`move_worth`]) but HM moves (`is_hm`: Gen3's
/// learn menu refuses them, `IsHMMove2`), when the new one is worth more; `None` keeps the known
/// moves.
pub fn forget_slot(me: &BattleMon, t: &BattleTables, is_hm: &dyn Fn(MoveId) -> bool) -> Option<u8> {
    let new = (t.species_info(me.species).learnset.iter())
        .find(|l| l.level == me.level && !me.moves.contains(&Some(l.mov)))?
        .mov;
    forget_slot_of(me.species, &me.moves, new, t, is_hm)
}

/// [`forget_slot`] on raw moves, for learning `new` (the vanilla oracle's `gMoveToLearn`).
pub fn forget_slot_of(
    me: SpeciesId,
    moves: &[Option<MoveId>; 4],
    new: MoveId,
    t: &BattleTables,
    is_hm: &dyn Fn(MoveId) -> bool,
) -> Option<u8> {
    let (worth, slot) = (0..4)
        .filter_map(|i| {
            moves[i]
                .filter(|&m| !is_hm(m))
                .map(|m| (move_worth(me, m, t), i as u8))
        })
        .min()?;
    (move_worth(me, new, t) > worth).then_some(slot)
}

/// Whether every ability `foe`'s species can have makes it immune to a `ty` move (Gen3's
/// Levitate, Water Absorb, Volt Absorb, Flash Fire): the policy cannot see which one it has.
fn immune_by_ability(abilities: [Ability; 2], ty: Type) -> bool {
    let immune = |a: Ability| match a {
        Ability::Levitate => ty == Type::Ground,
        Ability::WaterAbsorb => ty == Type::Water,
        Ability::VoltAbsorb => ty == Type::Electric,
        Ability::FlashFire => ty == Type::Fire,
        _ => false,
    };
    immune(abilities[0]) && (abilities[1] == Ability::None || immune(abilities[1]))
}

/// [`best_damaging_slot`] over the moves the player may choose now
/// ([`engine::battle::Battle::move_selectable`]): Torment, Disable or Taunt refuse a move and
/// reopen the move menu, so asking for it again would loop.
fn best_selectable_slot(battle: &engine::battle::Battle, t: &BattleTables) -> u8 {
    let side = battle.choosing();
    let (me, foe) = (battle.mon(side), battle.mon(target_foe(battle)));
    let pp = selectable_pp(me.pp, |s| selectable(battle, t, side, s));
    best_damaging_slot_of(me.species, &me.moves, &pp, foe.species, t)
}

/// [`first_damaging_slot`] over the moves the player may choose now (as
/// [`best_selectable_slot`]): a Disabled, Tormented or Taunted first slot would loop the menu.
fn first_selectable_slot(battle: &engine::battle::Battle, t: &BattleTables) -> u8 {
    let side = battle.choosing();
    let me = battle.mon(side);
    let pp = selectable_pp(me.pp, |s| selectable(battle, t, side, s));
    first_damaging_slot_of(&me.moves, &pp, t)
}

/// Doubles: the foe to aim at, the standing opponent with the least HP (finish it off).
fn target_foe(battle: &engine::battle::Battle) -> Side {
    let foes = [Side::Opponent, Side::OpponentRight]
        .into_iter()
        .take(battle.count() / 2);
    foes.filter(|&s| battle.is_present(s) && battle.mon(s).hp > 0)
        .min_by_key(|&s| battle.mon(s).hp)
        .unwrap_or(Side::Opponent)
}

/// Whether `side` may choose `slot` now; in doubles not a move that also hits its standing
/// ally (Earthquake: `MOVE_TARGET_FOES_AND_ALLY`) while another move is usable.
fn selectable(battle: &engine::battle::Battle, t: &BattleTables, side: Side, slot: u8) -> bool {
    if !battle.move_selectable(t, side, slot) {
        return false;
    }
    let ally = side.partner();
    if !battle.is_double() || !battle.is_present(ally) || battle.mon(ally).hp == 0 {
        return true;
    }
    let hits_ally = |s: u8| {
        battle.mon(side).moves[s as usize]
            .is_some_and(|m| t.move_info(m).target == engine::mon::MoveTarget::FoesAndAlly)
    };
    !hits_ally(slot)
        || !(0..4).any(|s| s != slot && !hits_ally(s) && battle.move_selectable(t, side, s))
}

/// The target cursor key: RIGHT until the cursor is on [`target_foe`], then A.
fn target_key(battle: &engine::battle::Battle, t: &BattleTables, c: u8, best: bool) -> Buttons {
    let side = battle.choosing();
    let slot = if best {
        best_selectable_slot(battle, t)
    } else {
        first_selectable_slot(battle, t)
    };
    let want = battle
        .target_choices(t, side, slot)
        .and_then(|v| v.iter().position(|&s| s == target_foe(battle)));
    match want {
        Some(k) if k as u8 != c => Buttons::RIGHT,
        _ => Buttons::A,
    }
}

/// `pp` with the slots `selectable` refuses at 0 (as if out of PP: never picked).
fn selectable_pp(mut pp: [u8; 4], selectable: impl Fn(u8) -> bool) -> [u8; 4] {
    for (s, p) in pp.iter_mut().enumerate() {
        if !selectable(s as u8) {
            *p = 0;
        }
    }
    pp
}

pub fn best_damaging_slot(me: &BattleMon, foe: &BattleMon, t: &BattleTables) -> u8 {
    best_damaging_slot_of(me.species, &me.moves, &me.pp, foe.species, t)
}

/// [`best_damaging_slot`] on raw species and move slots (the vanilla oracle's).
pub fn best_damaging_slot_of(
    me: SpeciesId,
    moves: &[Option<MoveId>; 4],
    pp: &[u8; 4],
    foe: SpeciesId,
    t: &BattleTables,
) -> u8 {
    let mine = t.species_info(me).types;
    let theirs = t.species_info(foe).types;
    let abilities = t.species_info(foe).abilities;
    let score = |i: usize| -> Option<u32> {
        let id = moves[i]?;
        if pp[i] == 0 {
            return None;
        }
        let mi = t.moves.get(id.0 as usize)?;
        if mi.power == 0 {
            return None;
        }
        if immune_by_ability(abilities, mi.ty) {
            return Some(0);
        }
        let mut s = u32::from(mi.power) * if mine.contains(&mi.ty) { 15 } else { 10 };
        let mut seen = None;
        for d in theirs {
            if seen == Some(d) {
                continue;
            }
            seen = Some(d);
            let m = (t.type_chart.iter())
                .find(|e| e.attack == mi.ty && e.defend == d)
                .map_or(10, |e| u32::from(e.multiplier));
            s = s * m / 10;
        }
        Some(s)
    };
    let mut best: Option<(u32, usize)> = None;
    for i in 0..4 {
        if let Some(s) = score(i)
            && best.is_none_or(|(b, _)| s > b)
        {
            best = Some((s, i));
        }
    }
    best.map_or_else(|| first_damaging_slot_of(moves, pp, t), |(_, i)| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_immunity_needs_every_ability() {
        let lev = [Ability::Levitate, Ability::None];
        assert!(immune_by_ability(lev, Type::Ground));
        assert!(!immune_by_ability(lev, Type::Water));
        assert!(immune_by_ability(
            [Ability::WaterAbsorb, Ability::WaterAbsorb],
            Type::Water
        ));
        // Lanturn: Volt Absorb or Illuminate, unknown which.
        assert!(!immune_by_ability(
            [Ability::VoltAbsorb, Ability::Illuminate],
            Type::Electric
        ));
    }

    #[test]
    fn first_slot_skips_a_refused_move() {
        use engine::data::GameData;
        let t = content::Emerald::default().battle();
        let power = |p: bool| {
            (1..t.moves.len())
                .find(|&i| (t.moves[i].power > 0) == p)
                .map(|i| MoveId(i as u16))
        };
        let (Some(hit), Some(status)) = (power(true), power(false)) else {
            return; // no move table in this build
        };
        let moves = [Some(hit), Some(hit), Some(status), None];
        let pp = [10, 10, 10, 0];
        assert_eq!(first_damaging_slot_of(&moves, &pp, t), 0);
        // Disabled slot 0: the other damaging slot, not a loop on the refused one.
        let masked = selectable_pp(pp, |s| s != 0);
        assert_eq!(masked, [0, 10, 10, 0]);
        assert_eq!(first_damaging_slot_of(&moves, &masked, t), 1);
        // Both damaging slots refused: the status move.
        let masked = selectable_pp(pp, |s| s >= 2);
        assert_eq!(first_damaging_slot_of(&moves, &masked, t), 2);
    }

    #[test]
    fn toward_walks_the_2x2_grid() {
        assert_eq!(toward(0, 0), Buttons::A);
        assert_eq!(toward(0, 3), Buttons::RIGHT);
        assert_eq!(toward(1, 3), Buttons::DOWN);
        assert_eq!(toward(3, 3), Buttons::A);
        assert_eq!(toward(3, 0), Buttons::LEFT);
        assert_eq!(toward(2, 0), Buttons::UP);
        assert_eq!(toward(0, 2), Buttons::DOWN);
    }
}
