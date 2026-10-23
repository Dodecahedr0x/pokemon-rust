//! The Map tab's map picker (M8 Task S): every exported map (518 on the full import), grouped
//! by map group (`gMapGroups`, named after the decomp's `map_groups.json`), filtered by name or
//! constant. Pure: the UI (`ui::map::MapList`) renders a [`PickerView`].
//!
//! - Without a filter, every group is a header; a group's maps are listed when it is expanded
//!   ([`Toggles::is_open`]): the shown map's group unless the user closed it, and any group
//!   the user opened. Opening a map reveals its group ([`Toggles::reveal`]).
//! - With a filter, only the groups with a match are listed, every one expanded, with only the
//!   matching maps. A query matches a map when its alphanumerics, lowercased, are in the map's
//!   name or constant (`route 101`, `Route101` and `MAP_ROUTE101` all find `Route101`).

use crate::map_view::map_list;
use engine::data::MapId;
use std::collections::BTreeSet;

/// The decomp's map group names (`gMapGroup_<name>`, `map_groups.json` `group_order`), by group.
pub const GROUP_NAMES: [&str; 34] = [
    "TownsAndRoutes",
    "IndoorLittleroot",
    "IndoorOldale",
    "IndoorDewford",
    "IndoorLavaridge",
    "IndoorFallarbor",
    "IndoorVerdanturf",
    "IndoorPacifidlog",
    "IndoorPetalburg",
    "IndoorSlateport",
    "IndoorMauville",
    "IndoorRustboro",
    "IndoorFortree",
    "IndoorLilycove",
    "IndoorMossdeep",
    "IndoorSootopolis",
    "IndoorEverGrande",
    "IndoorRoute104",
    "IndoorRoute111",
    "IndoorRoute112",
    "IndoorRoute114",
    "IndoorRoute116",
    "IndoorRoute117",
    "IndoorRoute121",
    "Dungeons",
    "IndoorDynamic",
    "SpecialArea",
    "IndoorRoute104Prototype",
    "IndoorRoute109",
    "IndoorRoute110",
    "IndoorRoute113",
    "IndoorRoute123",
    "IndoorRoute119",
    "IndoorRoute124",
];

/// The map group of `map` (its high byte).
pub fn group_of(map: MapId) -> u8 {
    (map.0 >> 8) as u8
}

/// Group `group`'s name ([`GROUP_NAMES`]), `Group <n>` past the table.
pub fn group_label(group: u8) -> String {
    GROUP_NAMES
        .get(group as usize)
        .map_or_else(|| format!("Group {group}"), |n| (*n).to_string())
}

/// One map of the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerMap {
    pub id: MapId,
    /// The decomp map name (`MapData::name`).
    pub name: &'static str,
    /// `MAP_…` (`extract::names`), `None` for a map the decomp does not name.
    pub constant: Option<&'static str>,
    /// The name and the constant [`normalize`]d, `|`-separated (computed once, so a filter
    /// never re-normalizes 518 maps per keystroke; `|` is never in a normalized query).
    key: String,
}

impl PickerMap {
    pub fn new(id: MapId, name: &'static str, constant: Option<&'static str>) -> PickerMap {
        let key = format!(
            "{}|{}",
            normalize(name),
            constant.map_or(String::new(), normalize)
        );
        PickerMap {
            id,
            name,
            constant,
            key,
        }
    }

    fn of(id: MapId, name: &'static str) -> PickerMap {
        let constant = extract::names::map(group_of(id), id.0 as u8).map(|m| m.constant);
        PickerMap::new(id, name, constant)
    }

    /// Whether the normalized query `q` ([`normalize`]) finds this map.
    fn matches(&self, q: &str) -> bool {
        self.key.contains(q)
    }
}

/// A map group and its maps, in map order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerGroup {
    pub group: u8,
    pub label: String,
    pub maps: Vec<PickerMap>,
}

/// Every exported map ([`map_list`]) by group, groups and maps in id order.
pub fn picker_groups() -> Vec<PickerGroup> {
    let mut out: Vec<PickerGroup> = Vec::new();
    let mut list = map_list();
    list.sort_by_key(|&(id, _)| id.0);
    for (id, name) in list {
        let g = group_of(id);
        if out.last().is_none_or(|l| l.group != g) {
            out.push(PickerGroup {
                group: g,
                label: group_label(g),
                maps: Vec::new(),
            });
        }
        out.last_mut()
            .expect("pushed")
            .maps
            .push(PickerMap::of(id, name));
    }
    out
}

/// A query's alphanumerics, lowercased.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The groups the user opened and closed by their headers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toggles {
    opened: BTreeSet<u8>,
    closed: BTreeSet<u8>,
}

impl Toggles {
    /// Whether `group` lists its maps without a filter: not closed, and the shown map's group
    /// or opened.
    pub fn is_open(&self, group: u8, shown: MapId) -> bool {
        !self.closed.contains(&group) && (group == group_of(shown) || self.opened.contains(&group))
    }

    /// A click on `group`'s header: closes it if open, else opens it.
    pub fn toggle(&mut self, group: u8, shown: MapId) {
        if self.is_open(group, shown) {
            self.opened.remove(&group);
            self.closed.insert(group);
        } else {
            self.closed.remove(&group);
            self.opened.insert(group);
        }
    }

    /// `map` is opened from the picker: its group lists it.
    pub fn reveal(&mut self, map: MapId) {
        self.closed.remove(&group_of(map));
    }
}

/// Whether group `group` lists its maps: always under a filter, else [`Toggles::is_open`].
pub fn expanded(group: u8, shown: MapId, toggles: &Toggles, query: &str) -> bool {
    !normalize(query).is_empty() || toggles.is_open(group, shown)
}

/// One group as the picker draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewGroup {
    pub group: u8,
    pub label: String,
    /// Maps in the group.
    pub total: usize,
    /// Maps matching the filter (`total` without one).
    pub matching: usize,
    pub expanded: bool,
    /// The maps listed: the matching ones of an expanded group, none of a collapsed one.
    pub maps: Vec<PickerMap>,
}

/// What the picker draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerView {
    pub groups: Vec<ViewGroup>,
    /// Maps matching the filter, over every group.
    pub matching: usize,
    /// The first matching map under a filter (the filter's Enter opens it); `None` without a
    /// filter.
    pub first: Option<MapId>,
}

/// The picker for `query`, groups expanded by [`expanded`]. Groups without a match are left out
/// under a filter.
pub fn picker_view(
    groups: &[PickerGroup],
    query: &str,
    shown: MapId,
    toggles: &Toggles,
) -> PickerView {
    let q = normalize(query);
    let mut view = PickerView {
        groups: Vec::new(),
        matching: 0,
        first: None,
    };
    for g in groups {
        let hits: Vec<PickerMap> = (g.maps.iter()).filter(|m| m.matches(&q)).cloned().collect();
        if hits.is_empty() && !q.is_empty() {
            continue;
        }
        view.matching += hits.len();
        if view.first.is_none() && !q.is_empty() {
            view.first = hits.first().map(|m| m.id);
        }
        let open = expanded(g.group, shown, toggles, query);
        view.groups.push(ViewGroup {
            group: g.group,
            label: g.label.clone(),
            total: g.maps.len(),
            matching: hits.len(),
            expanded: open,
            maps: if open { hits } else { Vec::new() },
        });
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(id: u16, name: &'static str, constant: &'static str) -> PickerMap {
        PickerMap::new(MapId(id), name, Some(constant))
    }

    /// Toggles after clicking `groups`' headers in order, with `shown` shown.
    fn clicked(groups: &[u8], shown: MapId) -> Toggles {
        let mut t = Toggles::default();
        for &g in groups {
            t.toggle(g, shown);
        }
        t
    }

    fn fixture() -> Vec<PickerGroup> {
        vec![
            PickerGroup {
                group: 0,
                label: group_label(0),
                maps: vec![
                    map(0x0009, "LittlerootTown", "MAP_LITTLEROOT_TOWN"),
                    map(0x0010, "Route101", "MAP_ROUTE101"),
                    map(0x0011, "Route102", "MAP_ROUTE102"),
                ],
            },
            PickerGroup {
                group: 1,
                label: group_label(1),
                maps: vec![map(
                    0x0100,
                    "LittlerootTown_BrendansHouse_1F",
                    "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F",
                )],
            },
            PickerGroup {
                group: 24,
                label: group_label(24),
                maps: vec![map(0x1800, "MeteorFalls_1F_1R", "MAP_METEOR_FALLS_1F_1R")],
            },
        ]
    }

    fn ids(v: &PickerView) -> Vec<u16> {
        v.groups
            .iter()
            .flat_map(|g| g.maps.iter().map(|m| m.id.0))
            .collect()
    }

    #[test]
    fn without_a_filter_only_the_shown_group_is_listed() {
        let g = fixture();
        let none = Toggles::default();
        let v = picker_view(&g, "", MapId(0x0100), &none);
        assert_eq!(v.groups.len(), 3);
        assert_eq!(v.matching, 5);
        assert_eq!(v.first, None, "Enter opens nothing without a filter");
        assert_eq!(ids(&v), [0x0100]);
        assert_eq!(
            v.groups.iter().map(|g| g.expanded).collect::<Vec<_>>(),
            [false, true, false]
        );
        assert_eq!(v.groups[0].label, "TownsAndRoutes");
        assert_eq!(v.groups[2].label, "Dungeons");
        assert_eq!((v.groups[0].total, v.groups[0].matching), (3, 3));
        // Toggling opens another group and closes the shown one.
        let t = clicked(&[0, 1], MapId(0x0100));
        let v = picker_view(&g, "", MapId(0x0100), &t);
        assert_eq!(ids(&v), [0x0009, 0x0010, 0x0011]);
    }

    /// The review's case: picking a map in a group the user opened keeps that group open (and
    /// the pick listed); the group shown before stays as the user left it.
    #[test]
    fn picking_a_map_in_an_opened_group_keeps_it_open() {
        let g = fixture();
        let shown = MapId(0x0100);
        let mut t = clicked(&[0], shown);
        let picked = MapId(0x0010);
        t.reveal(picked);
        let v = picker_view(&g, "", picked, &t);
        assert_eq!(ids(&v), [0x0009, 0x0010, 0x0011]);
        assert!(v.groups[0].expanded);
        // The shown group closed, then a map of it picked again (e.g. through the filter): it
        // is revealed.
        let mut t = clicked(&[1], shown);
        assert!(!t.is_open(1, shown));
        t.reveal(shown);
        assert!(t.is_open(1, shown));
        // A header click on an open group closes it, a second click reopens it.
        let mut t = Toggles::default();
        t.toggle(0, picked);
        assert!(!t.is_open(0, picked));
        t.toggle(0, picked);
        assert!(t.is_open(0, picked));
    }

    #[test]
    fn a_filter_matches_names_and_constants_ignoring_case_and_punctuation() {
        let g = fixture();
        let none = Toggles::default();
        for q in ["route 10", "ROUTE10", "map_route10", "Route10"] {
            let v = picker_view(&g, q, MapId(0x1800), &none);
            assert_eq!(ids(&v), [0x0010, 0x0011], "{q}");
            assert_eq!((v.groups.len(), v.matching), (1, 2), "{q}");
            assert_eq!(v.first, Some(MapId(0x0010)));
            assert!(v.groups[0].expanded);
        }
        // Across underscores, and in the constant only (`MAP_` is not in the name).
        let v = picker_view(&g, "town_brendans", MapId(0x0009), &none);
        assert_eq!(ids(&v), [0x0100]);
        let v = picker_view(&g, "map_meteor", MapId(0x0009), &none);
        assert_eq!(ids(&v), [0x1800]);
        let v = picker_view(&g, "littleroot", MapId(0x0009), &none);
        assert_eq!(ids(&v), [0x0009, 0x0100]);
        assert_eq!(v.groups.len(), 2);
        // A filter overrides toggles; no match lists nothing.
        let v = picker_view(&g, "meteor", MapId(0x0009), &clicked(&[24], MapId(0x0009)));
        assert_eq!(ids(&v), [0x1800]);
        let v = picker_view(&g, "nowhere", MapId(0x0009), &none);
        assert_eq!((v.groups.len(), v.matching, v.first), (0, 0, None));
        // Punctuation alone is no filter.
        assert_eq!(picker_view(&g, " _ ", MapId(0x0009), &none).matching, 5);
    }

    #[test]
    fn every_exported_map_is_in_one_group_in_id_order() {
        let groups = picker_groups();
        let all: Vec<MapId> = groups
            .iter()
            .flat_map(|g| g.maps.iter().map(|m| m.id))
            .collect();
        let mut want: Vec<MapId> = map_list().into_iter().map(|m| m.0).collect();
        want.sort_by_key(|m| m.0);
        assert_eq!(all, want);
        for g in &groups {
            assert!(!g.maps.is_empty());
            assert!(g.maps.iter().all(|m| group_of(m.id) == g.group));
        }
        assert!(groups.windows(2).all(|w| w[0].group < w[1].group));
        if content::FLAVOR == content::Flavor::Emerald && all.len() == extract::names::MAPS.len() {
            // The full import: every group of `gMapGroups`, every map named.
            assert_eq!(groups.len(), GROUP_NAMES.len());
            assert!(
                groups
                    .iter()
                    .flat_map(|g| &g.maps)
                    .all(|m| m.constant.is_some())
            );
        }
        assert_eq!(group_label(200), "Group 200");
    }
}
