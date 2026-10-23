//! Field effects (`dofieldeffect`, Wave C): each `FLDEFF_*` a script starts, as the length of
//! its Gen3 animation in frames, which `waitfieldeffect` waits out. The sprites are not drawn
//! (the engine models the timing only); effects that change more than the screen (the field
//! moves', the secret bases', Deoxys's rock) report `Unsupported` with their decomp name and
//! start nothing, so a `waitfieldeffect` on them answers at once.

use super::*;
use engine::data::FieldMove;

/// `include/constants/field_effects.h` ids of the effects scripts start.
pub const FLDEFF_USE_CUT_ON_TREE: u16 = 2;
pub const FLDEFF_USE_SURF: u16 = 9;
pub const FLDEFF_USE_SECRET_POWER_CAVE: u16 = 11;
pub const FLDEFF_POKECENTER_HEAL: u16 = 25;
pub const FLDEFF_USE_SECRET_POWER_TREE: u16 = 26;
pub const FLDEFF_USE_SECRET_POWER_SHRUB: u16 = 27;
pub const FLDEFF_NPCFLY_OUT: u16 = 30;
pub const FLDEFF_USE_ROCK_SMASH: u16 = 37;
pub const FLDEFF_USE_STRENGTH: u16 = 40;
pub const FLDEFF_USE_WATERFALL: u16 = 43;
pub const FLDEFF_USE_DIVE: u16 = 44;
pub const FLDEFF_SAND_PILLAR: u16 = 52;
pub const FLDEFF_SPARKLE: u16 = 54;
pub const FLDEFF_PCTURN_ON: u16 = 61;
pub const FLDEFF_HALL_OF_FAME_RECORD: u16 = 62;
pub const FLDEFF_DESTROY_DEOXYS_ROCK: u16 = 65;

/// The effects reported, by id: Waterfall's and Dive's are M10's, the secret bases M13's,
/// and Deoxys's rock (an event island) moves and removes an object.
const UNSUPPORTED: [(u16, &str); 8] = [
    (FLDEFF_USE_SECRET_POWER_CAVE, "FLDEFF_USE_SECRET_POWER_CAVE"),
    (FLDEFF_USE_SECRET_POWER_TREE, "FLDEFF_USE_SECRET_POWER_TREE"),
    (
        FLDEFF_USE_SECRET_POWER_SHRUB,
        "FLDEFF_USE_SECRET_POWER_SHRUB",
    ),
    (FLDEFF_USE_WATERFALL, "FLDEFF_USE_WATERFALL"),
    (FLDEFF_USE_DIVE, "FLDEFF_USE_DIVE"),
    (FLDEFF_SAND_PILLAR, "FLDEFF_SAND_PILLAR"),
    (FLDEFF_PCTURN_ON, "FLDEFF_PCTURN_ON"),
    (FLDEFF_DESTROY_DEOXYS_ROCK, "FLDEFF_DESTROY_DEOXYS_ROCK"),
];

/// Gen3 `PokeballGlowEffect_PlaceBalls`: one ball every 25 frames.
const BALL_FRAMES: u16 = 25;
/// `PokeballGlowEffect_TryPlaySe`: 32 frames after the last ball.
const BALLS_SETTLE: u16 = 32;
/// `PokeballGlowEffect_Flash1` (3 rounds of 4 phases of 8 frames), `_Flash2` (3 phases of 8),
/// `_WaitAfterFlash` (30) and `_Dummy` (1).
const FLASHES: u16 = 96 + 24 + 30 + 1;
/// `sFanfares[FANFARE_HEAL]`: `MUS_HEAL` lasts 160 frames, longer than [`FLASHES`], so the
/// Center's effect ends with it (`PokeballGlowEffect_WaitForSound`).
const HEAL_FANFARE: u16 = 160;
/// The frame the effect starts on and the frame its task sees the end
/// (`PokecenterHealEffect_WaitForSoundAndEnd` removes it from the active list).
const START_END: u16 = 2;

/// The Gen3 length of field effect `id` with `mons` party mons, `None` for the reported ones and
/// the ids scripts never start.
///
/// - `FLDEFF_POKECENTER_HEAL` (`FldEff_PokecenterHeal`): a ball per mon, the flashes and the
///   heal fanfare.
/// - `FLDEFF_HALL_OF_FAME_RECORD` (`FldEff_HallOfFameRecord`): the same balls and flashes, no
///   fanfare.
/// - `FLDEFF_SPARKLE` (`FldEff_Sparkle`, `UpdateSparkleFieldEffect`): the small sparkle's
///   animation (3 + 5 + 5 frames), then 35 frames invisible.
/// - `FLDEFF_NPCFLY_OUT` (`SpriteCB_NPCFlyOut`): the bird's half circle, 4 steps of 256 a
///   frame up to 0x80: 32 frames.
pub fn field_effect_frames(id: u16, mons: u16) -> Option<u16> {
    let balls = BALL_FRAMES * mons.max(1).saturating_sub(1);
    Some(match id {
        FLDEFF_POKECENTER_HEAL => START_END + balls + BALLS_SETTLE + HEAL_FANFARE,
        FLDEFF_HALL_OF_FAME_RECORD => START_END + balls + BALLS_SETTLE + FLASHES,
        FLDEFF_SPARKLE => 3 + 5 + 5 + 35,
        FLDEFF_NPCFLY_OUT => 0x80 / 4,
        _ => return None,
    })
}

/// Gen3 `dofieldeffect <id>`: starts the effect for its Gen3 length; an effect this engine does
/// not model is reported with its decomp name (or `dofieldeffect` for an id no script names).
pub async fn field_effect(ctx: &Ctx, id: u16) {
    // The field moves the engine runs (Wave C, `Request::FieldMoveEffect`).
    let field_move = match id {
        FLDEFF_USE_SURF => Some(FieldMove::Surf),
        FLDEFF_USE_CUT_ON_TREE => Some(FieldMove::Cut),
        FLDEFF_USE_ROCK_SMASH => Some(FieldMove::RockSmash),
        FLDEFF_USE_STRENGTH => Some(FieldMove::Strength),
        _ => None,
    };
    if let Some(m) = field_move {
        return ctx.field_move_effect(m).await;
    }
    let mons = match id {
        FLDEFF_POKECENTER_HEAL | FLDEFF_HALL_OF_FAME_RECORD => ctx.party_size().await,
        _ => 0,
    };
    match field_effect_frames(id, mons) {
        Some(frames) => ctx.field_effect(id, frames).await,
        None => {
            let name = UNSUPPORTED
                .iter()
                .find(|&&(i, _)| i == id)
                .map_or("dofieldeffect", |&(_, n)| n);
            ctx.report(Report::Unsupported(name)).await;
        }
    }
}

/// Gen3 `setfieldeffectargument <n>, <value>`: `gFieldEffectArguments[n] = (s16)value`.
pub async fn field_effect_arg(ctx: &Ctx, n: u8, value: u16) {
    ctx.field_effect_arg(n, value as i16).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heal_lengths_grow_by_a_ball_per_mon() {
        let one = field_effect_frames(FLDEFF_POKECENTER_HEAL, 1).unwrap();
        assert_eq!(one, 194);
        assert_eq!(
            field_effect_frames(FLDEFF_POKECENTER_HEAL, 6),
            Some(one + 5 * 25)
        );
        // An empty party never reaches the nurse; it counts as one ball.
        assert_eq!(field_effect_frames(FLDEFF_POKECENTER_HEAL, 0), Some(one));
        assert_eq!(
            field_effect_frames(FLDEFF_HALL_OF_FAME_RECORD, 1),
            Some(185)
        );
    }

    #[test]
    fn visual_effects_have_lengths_and_the_others_none() {
        assert_eq!(field_effect_frames(FLDEFF_SPARKLE, 0), Some(48));
        assert_eq!(field_effect_frames(FLDEFF_NPCFLY_OUT, 0), Some(32));
        for (id, _) in UNSUPPORTED {
            assert_eq!(field_effect_frames(id, 1), None);
        }
    }
}
