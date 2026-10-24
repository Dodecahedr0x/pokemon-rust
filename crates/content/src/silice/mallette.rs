//! The Mallette's tables (Silice Task C, `contenu-jeu/bible/mallette.md`): the START menu, the
//! menu lists and the modules as field moves. The menu itself is the script
//! `scripts/mallette.rs` (`Mallette_Menu`), run from the START menu's first entry.
//!
//! - **START menu** ([`START_MENU`]): MALLETTE (shown once `FLAG_SIL_MALLETTE_OPEN` is set,
//!   Ch0 `BricoBourg_Sc02`) in place of Gen3's POKéDEX, then Gen3's POKéMON, BAG, the player,
//!   SAVE, OPTION and EXIT (`data::START_MENU[1..]`, the extract's labels). The bible: "Elle
//!   remplace le Pokédex, la Carte et les CS".
//! - **Lists** ([`multichoice`]): the Mallette's menu ([`LIST_MENU`]) and its logs
//!   ([`LIST_LOGS`]), the Tower's post-game elevator ([`LIST_ASCENSEUR`] and its variants),
//!   Silice texts; any other list is Gen3's (`data::multichoice`).
//! - **Modules** ([`MODULES`], [`field_move_gate`]): a module replaces an HM, usable "sans
//!   Pokémon qui connaît la capacité" once its `FLAG_SIL_MODULE_*` is set. The engine drives
//!   Surf, Waterfall and Dive from the field (`GameData::field_move_script`, the
//!   `Mallette_Use*` scripts), Fly from the party menu (the fly map) and the Champ Magnétique
//!   as Gen3's Strength (`silice::push`: an A press on the object, then walking into it); the
//!   other modules' obstacles are map scripts (`prelude::module_obstacle`). Their gates, the
//!   Champ Magnétique's included, stay `Never` (a
//!   flag gate would list them in the party menu, where the engine cannot use them).

use crate::data;
use crate::silice::prelude::*;
use engine::data::{FieldMove, FieldMoveGate, StartAction, StartItem};

/// The Mallette's menu: ANALYSE, CARTE, APPELS, MODULES, RAPPORTS, SYNCHRO, RETOUR (past
/// Gen3's `MULTI_*` ids).
pub const LIST_MENU: u8 = 0xE0;
/// The logs: `LOG_01` to `LOG_08`, RETOUR.
pub const LIST_LOGS: u8 = 0xE1;

const fn text(id: TextId) -> &'static [u8] {
    data::TEXTS[id.0 as usize]
}

static MENU: [&[u8]; 7] = [
    text(Mallette_Menu_Analyse),
    text(Mallette_Menu_Carte),
    text(Mallette_Menu_Appels),
    text(Mallette_Menu_Modules),
    text(Mallette_Menu_Rapports),
    text(Mallette_Menu_Synchro),
    text(Mallette_Menu_Retour),
];

static LOGS: [&[u8]; 9] = [
    text(Mallette_Log_01),
    text(Mallette_Log_02),
    text(Mallette_Log_03),
    text(Mallette_Log_04),
    text(Mallette_Log_05),
    text(Mallette_Log_06),
    text(Mallette_Log_07),
    text(Mallette_Log_08),
    text(Mallette_Menu_Retour),
];

/// The Tower's post-game elevator before 127 (`Tour_Nebula_PG_Ascenseur`): 2F, SOMMET, ANNULER.
pub const LIST_ASCENSEUR_FERME: u8 = 0xE2;
/// The Tower's post-game elevator from 127: 1F, 2F, 3F, 4F, SOMMET, ANNULER.
pub const LIST_ASCENSEUR: u8 = 0xE3;
/// [`LIST_ASCENSEUR`] with the Space Ticket: 1F, 2F, 3F, 4F, SOMMET, TOIT, ANNULER.
pub const LIST_ASCENSEUR_TOIT: u8 = 0xE4;

static ASCENSEUR_FERME: [&[u8]; 3] = [
    text(Tour_Nebula_PG_Ascenseur_2F),
    text(Tour_Nebula_PG_Ascenseur_Sommet),
    text(Tour_Nebula_PG_Ascenseur_Annuler),
];

static ASCENSEUR: [&[u8]; 6] = [
    text(Tour_Nebula_PG_Ascenseur_1F),
    text(Tour_Nebula_PG_Ascenseur_2F),
    text(Tour_Nebula_PG_Ascenseur_3F),
    text(Tour_Nebula_PG_Ascenseur_4F),
    text(Tour_Nebula_PG_Ascenseur_Sommet),
    text(Tour_Nebula_PG_Ascenseur_Annuler),
];

static ASCENSEUR_TOIT: [&[u8]; 7] = [
    text(Tour_Nebula_PG_Ascenseur_1F),
    text(Tour_Nebula_PG_Ascenseur_2F),
    text(Tour_Nebula_PG_Ascenseur_3F),
    text(Tour_Nebula_PG_Ascenseur_4F),
    text(Tour_Nebula_PG_Ascenseur_Sommet),
    text(Tour_Nebula_PG_Ascenseur_Toit),
    text(Tour_Nebula_PG_Ascenseur_Annuler),
];

/// Silice's lists, then Gen3's.
pub fn multichoice(list: u8) -> Option<&'static [&'static [u8]]> {
    match list {
        LIST_MENU => Some(&MENU),
        LIST_LOGS => Some(&LOGS),
        LIST_ASCENSEUR_FERME => Some(&ASCENSEUR_FERME),
        LIST_ASCENSEUR => Some(&ASCENSEUR),
        LIST_ASCENSEUR_TOIT => Some(&ASCENSEUR_TOIT),
        _ => data::multichoice(list),
    }
}

/// The START menu: the Mallette in place of the POKéDEX, then Gen3's entries.
pub static START_MENU: [StartItem; 7] = [
    StartItem {
        label: Mallette_Label,
        action: StartAction::Script(data::script_ids::Mallette_Menu),
        shown_if: Some(FLAG_SIL_MALLETTE_OPEN),
        description: None,
    },
    data::START_MENU[1],
    data::START_MENU[2],
    data::START_MENU[3],
    data::START_MENU[4],
    data::START_MENU[5],
    data::START_MENU[6],
];

/// The modules in badge order (`bible/mallette.md`): the flag, the module's name, the field
/// move it stands for.
pub const MODULES: [(FlagId, TextId, FieldMove); 8] = [
    (FLAG_SIL_MODULE_COUPE, Mallette_Module_Coupe, FieldMove::Cut),
    (
        FLAG_SIL_MODULE_FORCE,
        Mallette_Module_Force,
        FieldMove::Strength,
    ),
    (
        FLAG_SIL_MODULE_FLASH,
        Mallette_Module_Flash,
        FieldMove::Flash,
    ),
    (FLAG_SIL_MODULE_SURF, Mallette_Module_Surf, FieldMove::Surf),
    (FLAG_SIL_MODULE_VOL, Mallette_Module_Vol, FieldMove::Fly),
    (
        FLAG_SIL_MODULE_ECLATE_ROC,
        Mallette_Module_EclateRoc,
        FieldMove::RockSmash,
    ),
    (
        FLAG_SIL_MODULE_CASCADE,
        Mallette_Module_Cascade,
        FieldMove::Waterfall,
    ),
    (
        FLAG_SIL_MODULE_PLONGEE,
        Mallette_Module_Plongee,
        FieldMove::Dive,
    ),
];

/// The field moves the engine runs, gated by their module's flag (see the module docs).
pub fn field_move_gate(m: FieldMove) -> FieldMoveGate {
    match m {
        FieldMove::Surf | FieldMove::Fly | FieldMove::Waterfall | FieldMove::Dive => MODULES
            .iter()
            .find(|&&(_, _, fm)| fm == m)
            .map_or(FieldMoveGate::Never, |&(f, _, _)| FieldMoveGate::Flag(f)),
        _ => FieldMoveGate::Never,
    }
}

/// The scripts the engine runs for the modules it drives from the field.
pub fn field_move_script(m: FieldMove) -> Option<ScriptId> {
    use data::script_ids as s;
    match m {
        FieldMove::Surf => Some(s::Mallette_UseSurf),
        FieldMove::Waterfall => Some(s::Mallette_UseWaterfall),
        FieldMove::Dive => Some(s::Mallette_UseDive),
        _ => None,
    }
}

/// The party menu's label of a module (Fly's, the only one used from there; the others are
/// listed too): the Gen3 move's name, short enough for the action window (the module's own
/// name is in [`MODULES`] for the Mallette's screens).
pub fn field_move_label(
    battle: &'static engine::mon::BattleTables,
    m: FieldMove,
) -> Option<&'static [u8]> {
    let mv = match m {
        FieldMove::Surf => MOVE_SURF,
        FieldMove::Fly => MOVE_FLY,
        FieldMove::Waterfall => MOVE_WATERFALL,
        FieldMove::Dive => MOVE_DIVE,
        _ => return None,
    };
    battle.moves.get(mv.0 as usize).map(|i| i.name)
}
