//! Berry trees (Gen3 `berry.c`; Wave E): the `ObjectEventInteraction*BerryTree*` specials over
//! the engine's saved trees ([`engine::berry`]), and the new game's trees
//! (`EventScript_ResetAllBerries`, see [`plant_initial_berries`]).
//!
//! The tree of the object talked to is its template's `trainerRange_berryTreeId`
//! ([`Query::ObjectBerryTree`] of `gSpecialVar_LastTalked`; Gen3 reads `gSelectedObjectEvent`,
//! the same object). Berry data (`gBerries`: yields, stage duration) is `data::BERRIES`.
//!
//! Degraded (documented, not reported per call):
//! - no growth over time (`BerryTreeTimeUpdate` needs the RTC): a tree stays at its stage, so a
//!   planted berry never grows and a picked tree stays empty soil;
//! - no sparkle after growth (`IsBerryTreeSparkling`), no tree sprite per stage
//!   (`MovementType_BerryTreeGrowth`) and no watering animation (`DoWateringBerryTreeAnim`);
//! - `gStringVar1` gets the berry's item name for both `GetBerryNameByBerryType` and
//!   `GetBerryCountString` (no " BERRY"/" BERRIES" plural).

use super::*;
use engine::berry::BerryTree;

/// Gen3 `BERRY_STAGE_*`.
pub const BERRY_STAGE_NO_BERRY: u8 = 0;
pub const BERRY_STAGE_PLANTED: u8 = 1;
pub const BERRY_STAGE_SPROUTED: u8 = 2;
pub const BERRY_STAGE_TALLER: u8 = 3;
pub const BERRY_STAGE_FLOWERING: u8 = 4;
pub const BERRY_STAGE_BERRIES: u8 = 5;
/// Gen3 `NUM_WATER_STAGES`.
const NUM_WATER_STAGES: u32 = 4;

/// `GetBerryInfo(berry)`: `(item, name, maxYield, minYield, stageDuration)`; an unknown berry
/// is the first (Gen3 answers Cheri). `None` without berry data.
fn berry_info(berry: u8) -> Option<(u16, &'static [u8], u8, u8, u8)> {
    let b = crate::data::BERRIES;
    let i = match berry as usize {
        0 => 0,
        n if n > b.len() => 0,
        n => n - 1,
    };
    b.get(i).copied()
}

/// `ITEM_TO_BERRY(item)` with `ItemIdToBerryType`'s fallback to the first berry.
pub fn berry_of_item(item: u16) -> u8 {
    let b = crate::data::BERRIES;
    b.iter()
        .position(|&(it, ..)| it == item)
        .map_or(1, |i| i as u8 + 1)
}

/// `BerryTypeToItemId`.
fn item_of_berry(berry: u8) -> u16 {
    berry_info(berry).map_or(0, |(item, ..)| item)
}

/// `GetStageDurationByBerryType`: minutes per stage.
fn stage_duration(berry: u8) -> u16 {
    berry_info(berry).map_or(0, |(.., d)| u16::from(d) * 60)
}

/// `BerryTreeGetNumStagesWatered`.
fn watered_count(t: &BerryTree) -> u8 {
    (t.watered & 0xF).count_ones() as u8
}

/// `CalcBerryYield` (`CalcBerryYieldInternal`): the minimum unwatered, else a draw within the
/// watered share of the range, rounded half up.
async fn calc_yield(ctx: &Ctx, t: &BerryTree) -> u8 {
    let Some((_, _, max, min, _)) = berry_info(t.berry) else {
        return 0;
    };
    let water = u32::from(watered_count(t));
    if water == 0 {
        return min;
    }
    let span = u32::from(max - min);
    let (lo, hi) = (span * (water - 1), span * water);
    let rand = lo + u32::from(ctx.random((hi - lo + 1) as u16).await);
    let extra = if rand % NUM_WATER_STAGES >= NUM_WATER_STAGES / 2 {
        rand / NUM_WATER_STAGES + 1
    } else {
        rand / NUM_WATER_STAGES
    };
    (extra + u32::from(min)) as u8
}

/// `PlantBerryTree(id, berry, stage, allowGrowth)`.
pub async fn plant(ctx: &Ctx, id: u8, berry: u8, stage: u8, allow_growth: bool) {
    let mut t = BerryTree {
        berry,
        stage,
        minutes_until_next_stage: stage_duration(berry),
        stop_growth: !allow_growth,
        ..BerryTree::default()
    };
    if stage == BERRY_STAGE_BERRIES {
        t.berry_yield = calc_yield(ctx, &t).await;
        t.minutes_until_next_stage = t.minutes_until_next_stage.wrapping_mul(4);
    }
    ctx.set_berry_tree(id, t).await;
}

/// `EventScript_ResetAllBerries` (`NewGameInitData`): every listed tree fully grown, its growth
/// stopped until seen (`setberrytree` plants with `allowGrowth` FALSE).
pub async fn plant_initial_berries(ctx: &Ctx) {
    for &(tree, item) in RESET_ALL_BERRIES {
        plant(ctx, tree, berry_of_item(item), BERRY_STAGE_BERRIES, false).await;
    }
}

/// The berry tree of the object talked to (`GetObjectEventBerryTreeId(gSelectedObjectEvent)`).
async fn talked_tree(ctx: &Ctx) -> u8 {
    let local = ctx.var(VAR_LAST_TALKED).await;
    ctx.query(Query::ObjectBerryTree(local as u8)).await as u8
}

/// `gStringVar1` = the berry's name (see the module doc).
async fn buffer_berry(ctx: &Ctx, berry: u8) {
    ctx.buffer(0, BufferSrc::Item(ItemId(item_of_berry(berry))))
        .await;
}

/// `ObjectEventInteractionGetBerryTreeData`: lets the tree grow (`AllowBerryTreeGrowth`), then
/// `VAR_0x8004` = its stage, `VAR_0x8005` = the stages watered, `VAR_0x8006` = the yield, and
/// `gStringVar1` its berry.
pub async fn get_tree_data(ctx: &Ctx) {
    let id = talked_tree(ctx).await;
    let mut t = ctx.berry_tree(id).await;
    if t.stop_growth {
        t.stop_growth = false;
        ctx.set_berry_tree(id, t).await;
    }
    ctx.set_var(VAR_0x8004, t.stage.into()).await;
    ctx.set_var(VAR_0x8005, watered_count(&t).into()).await;
    ctx.set_var(VAR_0x8006, t.berry_yield.into()).await;
    buffer_berry(ctx, t.berry).await;
}

/// `ObjectEventInteractionGetBerryName` / `GetBerryCountString`.
pub async fn get_name(ctx: &Ctx) {
    let id = talked_tree(ctx).await;
    let t = ctx.berry_tree(id).await;
    buffer_berry(ctx, t.berry).await;
}

/// `ObjectEventInteractionPlantBerryTree`: the berry of `VAR_ITEM_ID`, planted, then
/// `ObjectEventInteractionGetBerryTreeData`.
pub async fn plant_talked(ctx: &Ctx) {
    let berry = berry_of_item(ctx.var(VAR_ITEM_ID).await);
    let id = talked_tree(ctx).await;
    plant(ctx, id, berry, BERRY_STAGE_PLANTED, true).await;
    get_tree_data(ctx).await;
}

/// `ObjectEventInteractionPickBerryTree`: `VAR_0x8004` = whether the yield fit the bag
/// (`AddBagItem`).
pub async fn pick(ctx: &Ctx) {
    let id = talked_tree(ctx).await;
    let t = ctx.berry_tree(id).await;
    let item = ItemId(item_of_berry(t.berry));
    let added = ctx.item_add(item, t.berry_yield.into()).await;
    ctx.set_var(VAR_0x8004, added.into()).await;
}

/// `ObjectEventInteractionRemoveBerryTree` (`RemoveBerryTree`; the just-picked sparkle flag is
/// not kept).
pub async fn remove(ctx: &Ctx) {
    let id = talked_tree(ctx).await;
    ctx.set_berry_tree(id, BerryTree::default()).await;
}

/// `ObjectEventInteractionWaterBerryTree`: the current stage's watered bit; returns whether the
/// stage can be watered.
pub async fn water(ctx: &Ctx) -> u16 {
    let id = talked_tree(ctx).await;
    let mut t = ctx.berry_tree(id).await;
    let bit = match t.stage {
        BERRY_STAGE_PLANTED => 0,
        BERRY_STAGE_SPROUTED => 1,
        BERRY_STAGE_TALLER => 2,
        BERRY_STAGE_FLOWERING => 3,
        _ => return 0,
    };
    t.watered |= 1 << bit;
    ctx.set_berry_tree(id, t).await;
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_berries_fall_back_to_the_first() {
        if crate::data::BERRIES.is_empty() {
            return;
        }
        assert_eq!(berry_of_item(0), 1);
        let (cheri, ..) = crate::data::BERRIES[0];
        assert_eq!(berry_of_item(cheri), 1);
        assert_eq!(item_of_berry(0), cheri);
        assert_eq!(item_of_berry(200), cheri);
        assert_eq!(
            watered_count(&BerryTree {
                watered: 0b1011,
                ..BerryTree::default()
            }),
            3
        );
    }
}
