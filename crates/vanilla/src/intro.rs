//! Past the truck: BPEE driven through the Littleroot intro (Mom, the clock, May's house,
//! meeting May) to the point our new game starts at (`content::new_game`,
//! `Start::BeforeRescue`: outside the houses, May met), then aligned on our new game's cell and
//! facing. Our new game skips vanilla's intro, so the spines start from here.

use crate::drive::Walk;
use crate::state::Vanilla;
use anyhow::{Result, anyhow, ensure};
use engine::data::{Dir, GameData, MapId};
use explore::semantic::map_name;

/// Frames one leg of the intro may take.
pub const LEG: u64 = 6_000;

impl Vanilla {
    /// Walks to `to`, going on after triggers (a coord event interrupts a walk).
    pub fn go<G: GameData>(&mut self, game: &G, to: (i16, i16)) -> Result<Walk> {
        for _ in 0..8 {
            match self.walk_to(game, to, LEG)? {
                Walk::Interrupted(_) => {}
                w => {
                    self.settle(LEG)?;
                    return Ok(w);
                }
            }
        }
        Err(anyhow!("walk to {to:?}: interrupted 8 times"))
    }

    /// Checks the current map's decomp name.
    pub fn on(&mut self, map: &str) -> Result<()> {
        let at = self.current_map()?;
        ensure!(
            map_name(at) == map,
            "on {} at {:?}, not {map}",
            map_name(at),
            self.player_object().map(|o| o.cell)
        );
        Ok(())
    }

    /// From the first control in the truck ([`Vanilla::boot`]) to our new game's start:
    /// out of the truck, the clock upstairs, the TV, May's house and May, out again.
    pub fn intro<G: GameData>(&mut self, game: &G) -> Result<()> {
        // Out of the truck (its door is the warp column x = 4); the coord event at x = 3 sets
        // the male intro flags. Mom then walks the player into the house.
        self.go(game, (4, 2))?;
        self.push(Dir::Right, LEG)?;
        self.on("MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F")?;
        // Upstairs to set the clock.
        self.go(game, (8, 2))?;
        self.on("MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F")?;
        ensure!(self.go(game, (5, 2))? == Walk::Reached, "the clock");
        self.interact(Dir::Up, LEG)?;
        // Downstairs: the TV.
        self.go(game, (7, 1))?;
        self.on("MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F")?;
        // Out, then to May's house.
        self.go(game, (8, 8))?;
        self.push(Dir::Down, LEG)?; // the door mat is an arrow warp
        self.on("MAP_LITTLEROOT_TOWN")?;
        self.go(game, (14, 8))?;
        self.on("MAP_LITTLEROOT_TOWN_MAYS_HOUSE_1F")?;
        self.go(game, (2, 2))?;
        self.on("MAP_LITTLEROOT_TOWN_MAYS_HOUSE_2F")?;
        // May's Poké Ball (5, 4): May comes in.
        ensure!(self.go(game, (5, 3))? == Walk::Reached, "May's ball");
        self.interact(Dir::Down, LEG)?;
        self.go(game, (1, 1))?;
        self.go(game, (2, 8))?;
        self.push(Dir::Down, LEG)?; // the door mat is an arrow warp
        self.on("MAP_LITTLEROOT_TOWN")
    }

    /// Walks to `cell` on `map` and turns to `facing`: our new game's spot.
    pub fn align<G: GameData>(
        &mut self,
        game: &G,
        map: MapId,
        cell: (i16, i16),
        facing: Dir,
    ) -> Result<()> {
        let on = self.current_map()?;
        ensure!(
            on == map,
            "align: on {}, not {}",
            map_name(on),
            map_name(map)
        );
        ensure!(
            self.go(game, cell)? == Walk::Reached,
            "align: never reached {cell:?}"
        );
        self.face(facing, LEG)
    }
}
