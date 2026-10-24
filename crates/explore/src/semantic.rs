//! A normalised view of the story state, shared by our engine ([`Semantic::from_world`]) and
//! the vanilla oracle (`vanilla::State::semantic`, BPEE in mGBA), and a [`diff`] that names
//! each field that differs (Emerald parity plan, Task O1).
//!
//! What is kept is what both games can be asked and should agree on at a step boundary: the
//! map, the player's cell and facing, the story flags and vars (the key's exclusions:
//! [`key::story_flag`], [`key::story_var`]), the party, the bag, the money, the badges, the
//! respawn map and the visible objects of the current map. Engine-only state (the camera, the
//! fader, the hash) and Gen3-only state (the trainer id, the RNG) are left out.
//!
//! **Tolerances** ([`Tolerance`]). Some fields depend on the RNG, which the two games do not
//! share (Gen3 draws a personality and IVs per mon, and moves wanderers at random):
//! - max HP and HP depend on the IVs: compared as a [`HpBucket`] (fainted, hurt, full) unless
//!   [`Tolerance::exact_hp`];
//! - level and exp are equal until the first random battle (a wild battle's species and level
//!   are rolled); the caller clears [`Tolerance::levels`] past one;
//! - wandering objects move at random: objects are compared by presence (local id), their
//!   cells only with [`Tolerance::object_cells`].
//!
//! Everything else must be equal.

use crate::config::KeyConfig;
use crate::key;
use engine::World;
use engine::data::{Dir, FlagId, GameData, MapId, Pocket, VarId};
use engine::flow::{VARS_END, VARS_START};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Gen3's saved flags (`FLAGS_COUNT`: `gSaveBlock1Ptr->flags` is 300 bytes).
pub const GEN3_FLAG_COUNT: u16 = 300 * 8;

/// `FLAG_BADGE01_GET`; the eight badge flags follow it (checked against the decomp names).
pub const FLAG_BADGE01_GET: u16 = 0x867;

/// The story state of one game at one point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Semantic {
    pub map: MapId,
    /// The player's map cell (no Gen3 `MAP_OFFSET`).
    pub cell: (i16, i16),
    pub facing: Dir,
    /// Set story flags below [`GEN3_FLAG_COUNT`].
    pub flags: BTreeSet<u16>,
    /// Non-zero story vars.
    pub vars: BTreeMap<u16, u16>,
    pub party: Vec<Mon>,
    /// `(pocket, item) -> quantity`, slot order ignored.
    pub bag: BTreeMap<(Pocket, u16), u16>,
    pub money: u32,
    /// Badge flags set, as a bit set (bit 0 = badge 1).
    pub badges: u8,
    /// The map of the last heal location (`setrespawn`).
    pub respawn: Option<MapId>,
    /// The visible objects of the current map but the player: local id -> cell.
    pub objects: BTreeMap<u8, (i16, i16)>,
    /// The same objects' spawn cells (Gen3 `initialCoords`, our `Object::home`): what decides
    /// whether Gen3 has an object spawned around the camera ([`in_spawn_view`]).
    pub homes: BTreeMap<u8, (i16, i16)>,
    /// The current map's layout override (Gen3 `setmaplayoutindex`, `mapLayoutId`), only
    /// when the game has that variant of the map ([`layout_override`]): `None` is the map's own.
    pub layout: Option<u16>,
}

/// `layout` as an override of `map`: `Some` only when `game` has a variant of `map` with that
/// layout (the map's own layout id, or one it has no variant of, is no override).
pub fn layout_override<G: GameData>(game: &G, map: MapId, layout: Option<u16>) -> Option<u16> {
    layout.filter(|&l| game.has_map(map) && game.map_layout(map, l).is_some())
}

/// Whether Gen3 keeps an object whose spawn cell is `home` spawned with the player on `player`,
/// with a cell of margin: `TrySpawnObjectEvents` spawns the templates within x in
/// [px - 9, px + 10] and y in [py - 7, py + 9] (`gSaveBlock1Ptr->pos` is the player's cell,
/// the box is `pos - 2 ..= pos + MAP_OFFSET_W + 2` against `template + MAP_OFFSET`), and
/// `RemoveObjectEventIfOutsideView` removes an object only once both its cell and its spawn
/// cell are out of the same box. An object whose spawn cell is in the box is therefore present
/// in Gen3 whatever path the camera took; one outside may or may not be.
pub fn in_spawn_view(home: (i16, i16), player: (i16, i16)) -> bool {
    let (dx, dy) = (home.0 - player.0, home.1 - player.1);
    (-8..=9).contains(&dx) && (-6..=8).contains(&dy)
}

/// One party mon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mon {
    pub species: u16,
    pub level: u8,
    pub exp: u32,
    pub hp: u16,
    pub max_hp: u16,
    /// Move ids, 0 = none.
    pub moves: [u16; 4],
}

/// HP as compared under the default [`Tolerance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HpBucket {
    Fainted,
    Hurt,
    Full,
}

impl Mon {
    pub fn hp_bucket(&self) -> HpBucket {
        match self.hp {
            0 => HpBucket::Fainted,
            hp if hp >= self.max_hp => HpBucket::Full,
            _ => HpBucket::Hurt,
        }
    }
}

/// Which RNG-dependent fields [`diff`] compares exactly (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tolerance {
    /// HP and max HP exactly (default: as [`HpBucket`]s).
    pub exact_hp: bool,
    /// Levels and exp (default: yes; clear past the first random battle).
    pub levels: bool,
    /// Object cells (default: presence only).
    pub object_cells: bool,
    /// HP at all (as buckets or exact; default: yes). Cleared, like `levels`, past random
    /// battles: the damage taken depends on the RNG.
    pub hp: bool,
    /// Objects only where Gen3's camera decides presence the same way for both games: the ids
    /// whose spawn cell is [`in_spawn_view`] of both players (default: every object).
    pub in_view: bool,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            exact_hp: false,
            levels: true,
            object_cells: false,
            hp: true,
            in_view: false,
        }
    }
}

/// One differing field: its name, our value and vanilla's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delta {
    pub field: String,
    pub ours: String,
    pub vanilla: String,
}

impl fmt::Display for Delta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: ours {} / vanilla {}",
            self.field, self.ours, self.vanilla
        )
    }
}

/// A flag's decomp name (`FLAG_0x123` when unnamed).
pub fn flag_name(f: u16) -> String {
    extract::names::FLAGS.name_or_hex(u32::from(f))
}

/// A var's decomp name.
pub fn var_name(v: u16) -> String {
    extract::names::VARS.name_or_hex(u32::from(v))
}

/// A map's decomp constant (`MAP_0x1234` when unknown).
pub fn map_name(m: MapId) -> String {
    let (group, num) = ((m.0 >> 8) as u8, m.0 as u8);
    extract::names::MAPS
        .iter()
        .find(|n| n.group == group && n.num == num)
        .map_or_else(|| format!("MAP_0x{:04X}", m.0), |n| n.constant.to_string())
}

impl Semantic {
    /// Our engine's state. `game` resolves the respawn heal location to its map.
    pub fn from_world<G: GameData>(w: &World, game: &G) -> Semantic {
        let cfg = KeyConfig::default();
        let player = w.objects().player();
        let flags = (0..GEN3_FLAG_COUNT)
            .filter(|&f| w.flag(FlagId(f)) && key::story_flag(FlagId(f), &cfg))
            .collect();
        let vars = (VARS_START..VARS_END)
            .map(|v| (v, w.var(VarId(v))))
            .filter(|&(v, x)| x != 0 && key::story_var(v, &cfg))
            .collect();
        let party = (w.party().iter())
            .map(|m| Mon {
                species: m.species.0,
                level: m.level,
                exp: m.exp,
                hp: m.hp,
                max_hp: m.stats[0],
                moves: m.moves.map(|x| x.map_or(0, |x| x.0)),
            })
            .collect();
        let mut bag = BTreeMap::new();
        for p in Pocket::ALL {
            for s in w.bag().pocket(p).iter().filter(|s| s.qty > 0) {
                *bag.entry((p, s.item.0)).or_insert(0) += s.qty;
            }
        }
        let map = w.map();
        let visible = || (w.objects().iter().skip(1)).filter(|o| o.map == map && o.visible);
        let objects = visible().map(|o| (o.local_id, (o.x, o.y))).collect();
        let homes = visible().map(|o| (o.local_id, o.home)).collect();
        Semantic {
            map,
            cell: (player.x, player.y),
            facing: player.facing,
            badges: badges(|f| w.flag(FlagId(f))),
            flags,
            vars,
            party,
            bag,
            money: w.money(),
            respawn: game.heal_location(w.respawn()).map(|(m, _, _)| m),
            objects,
            homes,
            layout: layout_override(game, map, w.map_layout()),
        }
    }

    /// Keeps the story flags and vars of raw Gen3 lists (the same exclusions as
    /// [`Semantic::from_world`]); for the oracle's side.
    pub fn story_flags(set: impl IntoIterator<Item = u16>) -> BTreeSet<u16> {
        let cfg = KeyConfig::default();
        set.into_iter()
            .filter(|&f| f < GEN3_FLAG_COUNT && key::story_flag(FlagId(f), &cfg))
            .collect()
    }

    /// See [`Semantic::story_flags`].
    pub fn story_vars(vars: impl IntoIterator<Item = (u16, u16)>) -> BTreeMap<u16, u16> {
        let cfg = KeyConfig::default();
        vars.into_iter()
            .filter(|&(v, x)| x != 0 && key::story_var(v, &cfg))
            .collect()
    }
}

/// The badge bits from a flag reader.
pub fn badges(flag: impl Fn(u16) -> bool) -> u8 {
    (0..8u16).fold(0, |b, i| b | u8::from(flag(FLAG_BADGE01_GET + i)) << i)
}

fn show<T: fmt::Debug>(x: T) -> String {
    format!("{x:?}")
}

/// What differs between `ours` and `vanilla` under `tol`, field by field (empty when equal).
pub fn diff(ours: &Semantic, vanilla: &Semantic, tol: Tolerance) -> Vec<Delta> {
    let mut out = Vec::new();
    let mut put = |field: String, a: String, b: String| {
        if a != b {
            out.push(Delta {
                field,
                ours: a,
                vanilla: b,
            });
        }
    };
    put("map".into(), map_name(ours.map), map_name(vanilla.map));
    put("cell".into(), show(ours.cell), show(vanilla.cell));
    put("facing".into(), show(ours.facing), show(vanilla.facing));
    put("layout".into(), show(ours.layout), show(vanilla.layout));
    for f in ours.flags.symmetric_difference(&vanilla.flags) {
        let set = |s: &BTreeSet<u16>| if s.contains(f) { "set" } else { "clear" };
        put(
            format!("flag {}", flag_name(*f)),
            set(&ours.flags).into(),
            set(&vanilla.flags).into(),
        );
    }
    let vars: BTreeSet<u16> = ours
        .vars
        .keys()
        .chain(vanilla.vars.keys())
        .copied()
        .collect();
    for v in vars {
        let get = |m: &BTreeMap<u16, u16>| m.get(&v).copied().unwrap_or(0).to_string();
        put(
            format!("var {}", var_name(v)),
            get(&ours.vars),
            get(&vanilla.vars),
        );
    }
    put(
        "party.len".into(),
        ours.party.len().to_string(),
        vanilla.party.len().to_string(),
    );
    for (i, (a, b)) in ours.party.iter().zip(&vanilla.party).enumerate() {
        put(
            format!("party[{i}].species"),
            show(a.species),
            show(b.species),
        );
        put(format!("party[{i}].moves"), show(a.moves), show(b.moves));
        if tol.levels {
            put(format!("party[{i}].level"), show(a.level), show(b.level));
            put(format!("party[{i}].exp"), show(a.exp), show(b.exp));
        }
        if tol.hp && tol.exact_hp {
            put(format!("party[{i}].hp"), show(a.hp), show(b.hp));
            put(format!("party[{i}].max_hp"), show(a.max_hp), show(b.max_hp));
        } else if tol.hp {
            put(
                format!("party[{i}].hp"),
                show(a.hp_bucket()),
                show(b.hp_bucket()),
            );
        }
    }
    let items: BTreeSet<_> = ours.bag.keys().chain(vanilla.bag.keys()).copied().collect();
    for k in items {
        let get = |m: &BTreeMap<(Pocket, u16), u16>| m.get(&k).copied().unwrap_or(0).to_string();
        put(
            format!(
                "bag {:?} {}",
                k.0,
                extract::names::ITEMS.name_or_hex(u32::from(k.1))
            ),
            get(&ours.bag),
            get(&vanilla.bag),
        );
    }
    put("money".into(), show(ours.money), show(vanilla.money));
    put(
        "badges".into(),
        format!("{:08b}", ours.badges),
        format!("{:08b}", vanilla.badges),
    );
    put(
        "respawn".into(),
        ours.respawn.map_or("none".into(), map_name),
        vanilla.respawn.map_or("none".into(), map_name),
    );
    let mut ids: BTreeSet<u8> = (ours.objects.keys().chain(vanilla.objects.keys()))
        .copied()
        .collect();
    if tol.in_view {
        if ours.map != vanilla.map {
            ids.clear();
        }
        ids.retain(|id| {
            let home = (ours.homes.get(id).or(vanilla.homes.get(id)))
                .or(ours.objects.get(id))
                .or(vanilla.objects.get(id));
            home.is_some_and(|&h| in_spawn_view(h, ours.cell) && in_spawn_view(h, vanilla.cell))
        });
    }
    for id in ids {
        let get = |m: &BTreeMap<u8, (i16, i16)>| match m.get(&id) {
            None => "absent".to_string(),
            Some(c) if tol.object_cells => show(c),
            Some(_) => "present".to_string(),
        };
        put(
            format!("object {id}"),
            get(&ours.objects),
            get(&vanilla.objects),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Semantic {
        Semantic {
            map: MapId(9),
            cell: (5, 8),
            facing: Dir::Down,
            flags: BTreeSet::new(),
            vars: BTreeMap::new(),
            party: vec![Mon {
                species: 258,
                level: 5,
                exp: 135,
                hp: 20,
                max_hp: 20,
                moves: [33, 45, 0, 0],
            }],
            bag: BTreeMap::new(),
            money: 3000,
            badges: 0,
            respawn: Some(MapId(9)),
            objects: BTreeMap::from([(1, (3, 3))]),
            homes: BTreeMap::from([(1, (3, 3))]),
            layout: None,
        }
    }

    #[test]
    fn a_layout_override_is_a_delta() {
        let mut v = base();
        v.layout = Some(0x1A3);
        let d = diff(&base(), &v, Tolerance::default());
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].field, "layout");
        assert_eq!(
            (d[0].ours.as_str(), d[0].vanilla.as_str()),
            ("None", "Some(419)")
        );
    }

    #[test]
    fn equal_states_have_no_delta() {
        assert!(diff(&base(), &base(), Tolerance::default()).is_empty());
    }

    #[test]
    fn deltas_name_their_fields() {
        let mut v = base();
        v.flags.insert(FLAG_BADGE01_GET);
        v.vars.insert(0x4050, 3);
        v.money = 2900;
        v.bag.insert((Pocket::Items, 13), 1);
        let d = diff(&base(), &v, Tolerance::default());
        let fields: Vec<_> = d.iter().map(|d| d.field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "flag FLAG_BADGE01_GET",
                "var VAR_LITTLEROOT_TOWN_STATE",
                "bag Items ITEM_POTION",
                "money"
            ]
        );
        assert_eq!(d[1].vanilla, "3");
        assert_eq!(d[1].ours, "0");
    }

    #[test]
    fn rng_fields_follow_the_tolerance() {
        let mut v = base();
        v.party[0].max_hp = 21;
        v.party[0].hp = 21;
        v.objects.insert(1, (4, 3));
        assert!(diff(&base(), &v, Tolerance::default()).is_empty());
        let strict = Tolerance {
            exact_hp: true,
            object_cells: true,
            ..Tolerance::default()
        };
        assert_eq!(diff(&base(), &v, strict).len(), 3);
        v.party[0].hp = 0;
        assert_eq!(
            diff(&base(), &v, Tolerance::default())[0].field,
            "party[0].hp"
        );
        let mut lv = base();
        lv.party[0].level = 6;
        let loose = Tolerance {
            levels: false,
            ..Tolerance::default()
        };
        assert!(diff(&base(), &lv, loose).is_empty());
    }

    #[test]
    fn objects_compare_in_the_shared_view() {
        let mut v = base();
        v.objects.clear();
        v.objects.insert(2, (40, 40));
        v.homes = BTreeMap::from([(2, (40, 40))]);
        let view = Tolerance {
            in_view: true,
            ..Tolerance::default()
        };
        // Object 2 spawns far from both players; object 1 near both: only 1 counts.
        let d = diff(&base(), &v, view);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].field, "object 1");
        assert_eq!(diff(&base(), &v, Tolerance::default()).len(), 2);
        // The box: [px - 8, px + 9] x [py - 6, py + 8] (one cell inside Gen3's).
        assert!(in_spawn_view((-3, 2), (5, 8)) && !in_spawn_view((-4, 8), (5, 8)));
        assert!(in_spawn_view((14, 16), (5, 8)) && !in_spawn_view((15, 8), (5, 8)));
        assert!(!in_spawn_view((5, 1), (5, 8)) && !in_spawn_view((5, 17), (5, 8)));
        // No HP compare past random battles.
        let mut h = base();
        h.party[0].hp = 3;
        let no_hp = Tolerance {
            hp: false,
            ..Tolerance::default()
        };
        assert!(diff(&base(), &h, no_hp).is_empty());
    }

    #[test]
    fn constants_match_the_decomp_names() {
        let flag = |n| extract::names::FLAGS.value(n).map(|v| v as u16);
        assert_eq!(flag("FLAG_BADGE01_GET"), Some(FLAG_BADGE01_GET));
        assert_eq!(flag("FLAG_BADGE08_GET"), Some(FLAG_BADGE01_GET + 7));
        assert_eq!(map_name(MapId(25 << 8 | 40)), "MAP_INSIDE_OF_TRUCK");
        assert_eq!(badges(|f| f == FLAG_BADGE01_GET + 2), 0b100);
    }

    #[test]
    fn new_game_reads_from_the_world() {
        let game = content::Emerald::default();
        let s = game_native::Session::new(game);
        let sem = Semantic::from_world(s.world(), &content::Emerald::default());
        assert_eq!(sem.map, s.world().map());
        assert_eq!(sem.badges, 0);
        assert!(sem.flags.iter().all(|&f| f < GEN3_FLAG_COUNT));
    }
}
