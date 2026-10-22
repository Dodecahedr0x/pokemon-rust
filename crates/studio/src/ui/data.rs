//! Data tab (M6 Task H): species, moves, trainers, encounters and texts, read-only.
//!
//! Rows come from `data_view`, built once per process ([`rows`]) and kept in a signal. Each
//! sub-tab has a filter box, a row list and a detail panel; links jump between them (species
//! to trainers and maps, move to learners, and back).

use crate::data_view::{self, EncounterRow, MoveRow, SpeciesRow, TextRow, TrainerRow, type_name};
use crate::map_view::map_name;
use content::Emerald;
use dioxus::prelude::*;
use engine::data::{MapId, MoveId, SpeciesId, TextId, TrainerId};
use std::sync::OnceLock;

/// Every row of every sub-tab.
pub struct DataRows {
    pub species: Vec<SpeciesRow>,
    pub moves: Vec<MoveRow>,
    pub trainers: Vec<TrainerRow>,
    pub encounters: Vec<EncounterRow>,
    pub texts: Vec<TextRow>,
}

impl DataRows {
    pub fn build() -> DataRows {
        DataRows {
            species: data_view::species(),
            moves: data_view::moves(),
            trainers: data_view::trainers(),
            encounters: data_view::encounters(),
            texts: data_view::texts(),
        }
    }

    /// Species and move ids index their rows.
    pub fn species(&self, id: SpeciesId) -> Option<&SpeciesRow> {
        self.species.get(id.0 as usize)
    }

    pub fn mov(&self, id: MoveId) -> Option<&MoveRow> {
        self.moves.get(id.0 as usize)
    }

    pub fn trainer(&self, id: TrainerId) -> Option<&TrainerRow> {
        self.trainers.iter().find(|t| t.id == id)
    }

    pub fn encounter(&self, map: MapId) -> Option<&EncounterRow> {
        self.encounters.iter().find(|e| e.map == map)
    }

    pub fn text(&self, id: TextId) -> Option<&TextRow> {
        self.texts.get(id.0 as usize)
    }

    /// The list of sub-tab `sub` filtered by `q` (case-insensitive, on what the list shows
    /// plus, for texts, the text): what to pick and its label.
    pub fn list(&self, sub: Sub, q: &str) -> Vec<(Pick, String)> {
        match sub {
            Sub::Species => data_view::filter(&self.species, q, |r| r.name.clone())
                .into_iter()
                .map(|r| (Pick::Species(r.id), format!("#{:03} {}", r.id.0, r.name)))
                .collect(),
            Sub::Moves => data_view::filter(&self.moves, q, |r| r.name.clone())
                .into_iter()
                .map(|r| (Pick::Move(r.id), format!("#{:03} {}", r.id.0, r.name)))
                .collect(),
            Sub::Trainers => data_view::filter(&self.trainers, q, trainer_label)
                .into_iter()
                .map(|r| (Pick::Trainer(r.id), trainer_label(r)))
                .collect(),
            Sub::Encounters => data_view::filter(&self.encounters, q, |r| r.map_name.into())
                .into_iter()
                .map(|r| (Pick::Map(r.map), r.map_name.to_string()))
                .collect(),
            Sub::Texts => data_view::filter(&self.texts, q, |r| format!("{} {}", r.key, r.text))
                .into_iter()
                .map(|r| (Pick::Text(r.id), r.key.to_string()))
                .collect(),
        }
    }
}

fn trainer_label(t: &TrainerRow) -> String {
    format!("{} {}", t.class, t.name)
}

/// The rows, built on first use and shared by every mount of the tab.
pub fn rows() -> &'static DataRows {
    static ROWS: OnceLock<DataRows> = OnceLock::new();
    ROWS.get_or_init(DataRows::build)
}

/// The sub-tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sub {
    #[default]
    Species,
    Moves,
    Trainers,
    Encounters,
    Texts,
}

const SUBS: [(Sub, &str); 5] = [
    (Sub::Species, "Species"),
    (Sub::Moves, "Moves"),
    (Sub::Trainers, "Trainers"),
    (Sub::Encounters, "Encounters"),
    (Sub::Texts, "Texts"),
];

/// A row to show in the detail panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Species(SpeciesId),
    Move(MoveId),
    Trainer(TrainerId),
    /// An encounter table, by map.
    Map(MapId),
    Text(TextId),
}

impl Pick {
    /// The sub-tab that shows it.
    pub fn sub(self) -> Sub {
        match self {
            Pick::Species(_) => Sub::Species,
            Pick::Move(_) => Sub::Moves,
            Pick::Trainer(_) => Sub::Trainers,
            Pick::Map(_) => Sub::Encounters,
            Pick::Text(_) => Sub::Texts,
        }
    }
}

/// A piece of decomp text: escapes become breaks, placeholders tags.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Part {
    Text(String),
    /// `\n` (new line), `\l` (scroll a line) or `\p` (new box).
    Break(char),
    /// `{PLAYER}`, `{STRING 3}`, … without the braces.
    Tag(String),
}

/// Splits a `text::decode` string into [`Part`]s. Other backslashes and an unclosed `{` stay
/// text.
pub fn text_parts(s: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, parts: &mut Vec<Part>| {
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(text)));
        }
    };
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        match c {
            '\\' if matches!(rest[1..].chars().next(), Some('n' | 'l' | 'p')) => {
                flush(&mut text, &mut parts);
                parts.push(Part::Break(rest.as_bytes()[1] as char));
                rest = &rest[2..];
            }
            '{' if rest.contains('}') => {
                flush(&mut text, &mut parts);
                let end = rest.find('}').unwrap_or(0);
                parts.push(Part::Tag(rest[1..end].to_string()));
                rest = &rest[end + 1..];
            }
            c => {
                text.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    flush(&mut text, &mut parts);
    parts
}

/// The gender split of a Gen3 gender ratio (female when `personality & 0xFF < ratio`).
pub fn gender(ratio: u8) -> String {
    match ratio {
        255 => "genderless".into(),
        254 => "always female".into(),
        0 => "always male".into(),
        r => {
            let tenths = r as u32 * 1000 / 256;
            format!("{}.{}% female", tenths / 10, tenths % 10)
        }
    }
}

/// An ability's name, "(other)" when it has none.
pub fn ability(name: &str) -> &str {
    if name.is_empty() { "(other)" } else { name }
}

/// `0` as a dash (power and accuracy of status moves).
fn or_dash(v: u8) -> String {
    if v == 0 { "—".into() } else { v.to_string() }
}

const CSS: &str = r#"
.data-tab { display: flex; flex-direction: column; gap: 8px; height: calc(100vh - 70px); }
.data-subs { display: flex; gap: 4px; }
.data-subs button { background: var(--panel); color: var(--muted); border: 1px solid #333; padding: 3px 10px; font: inherit; cursor: pointer; }
.data-subs button.active { color: var(--fg); border-color: var(--accent); }
.data-main { display: flex; gap: 12px; flex: 1; min-height: 0; }
.data-tab .rows { width: 260px; overflow-y: auto; list-style: none; margin: 0; padding: 0; }
.data-tab .detail { flex: 1; overflow-y: auto; }
.data-tab table { border-collapse: collapse; }
.data-tab td, .data-tab th { padding: 1px 8px; text-align: left; }
.data-tab th { color: var(--muted); font-weight: normal; }
.data-tab button.link { background: none; border: 0; color: var(--accent); padding: 0; font: inherit; cursor: pointer; }
.data-tab .tag { color: #ffd84a; }
.data-tab .box { border-top: 1px dashed #444; margin: 4px 0; }
"#;

/// The Data tab. `tab` is the sub-tab it opens on.
#[component]
pub fn Data(#[props(default)] tab: Sub) -> Element {
    let rows = use_signal(rows);
    let mut sub = use_signal(|| tab);
    let mut query = use_signal(String::new);
    let mut pick = use_signal(|| None::<Pick>);
    let go = use_callback(move |p: Pick| {
        query.set(String::new());
        sub.set(p.sub());
        pick.set(Some(p));
    });
    let r: &'static DataRows = rows();
    let list = r.list(sub(), &query.read());
    let shown = pick().filter(|p| p.sub() == sub());
    rsx! {
        style { {CSS} }
        div { class: "data-tab",
            nav { class: "data-subs",
                for (s, label) in SUBS {
                    button {
                        class: if sub() == s { "active" } else { "" },
                        onclick: move |_| {
                            sub.set(s);
                            query.set(String::new());
                        },
                        "{label}"
                    }
                }
                input {
                    placeholder: "Filter",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                span { "{list.len()} rows" }
            }
            div { class: "data-main",
                ul { class: "rows",
                    for (p, label) in list {
                        li { key: "{p:?}",
                            button {
                                "data-row": "1",
                                class: if shown == Some(p) { "active" } else { "" },
                                onclick: move |_| pick.set(Some(p)),
                                "{label}"
                            }
                        }
                    }
                }
                div { class: "detail",
                    match shown {
                        None => rsx! { p { "Pick a row." } },
                        Some(p) => detail(r, p, go),
                    }
                }
            }
        }
    }
}

/// A link to another row.
fn link(label: String, to: Pick, go: Callback<Pick>) -> Element {
    rsx! {
        button { class: "link", "data-link": "{to:?}", onclick: move |_| go.call(to), "{label}" }
    }
}

fn species_link(r: &DataRows, id: SpeciesId, go: Callback<Pick>) -> Element {
    let name = r
        .species(id)
        .map_or_else(|| format!("#{}", id.0), |s| s.name.clone());
    link(name, Pick::Species(id), go)
}

fn detail(r: &'static DataRows, p: Pick, go: Callback<Pick>) -> Element {
    match p {
        Pick::Species(id) => r.species(id).map(|s| species_detail(r, s, go)),
        Pick::Move(id) => r.mov(id).map(|m| move_detail(r, m, go)),
        Pick::Trainer(id) => r.trainer(id).map(|t| trainer_detail(r, t, go)),
        Pick::Map(id) => r.encounter(id).map(|e| encounter_detail(r, e, go)),
        Pick::Text(id) => r.text(id).map(text_detail),
    }
    .unwrap_or_else(|| rsx! { p { "Not found." } })
}

fn species_detail(r: &DataRows, s: &SpeciesRow, go: Callback<Pick>) -> Element {
    let types = if s.types[0] == s.types[1] {
        type_name(s.types[0])
    } else {
        format!("{} / {}", type_name(s.types[0]), type_name(s.types[1]))
    };
    let abilities = format!(
        "{} / {}",
        ability(&s.ability_names[0]),
        ability(&s.ability_names[1])
    );
    let stats = ["HP", "Atk", "Def", "Spe", "SpA", "SpD"];
    let total: u32 = s.base.iter().map(|&b| b as u32).sum();
    let game = Emerald::for_shells();
    rsx! {
        h3 { "#{s.id.0:03} {s.name}" }
        table {
            tr { th { "Types" } td { "{types}" } }
            tr { th { "Abilities" } td { "{abilities}" } }
            tr { th { "Base stats" }
                td {
                    for (n, v) in stats.iter().zip(s.base) { "{n} {v}  " }
                    "(total {total})"
                }
            }
            tr { th { "Catch rate" } td { "{s.catch_rate}" } }
            tr { th { "Exp yield" } td { "{s.exp_yield}" } }
            tr { th { "Growth" } td { "{s.growth:?}" } }
            tr { th { "Gender" } td { {gender(s.gender_ratio)} } }
            tr { th { "Graphics" } td { if s.has_gfx { "yes" } else { "no" } } }
        }
        h4 { "Learnset" }
        table {
            for (lv, mv, name) in s.learnset.iter().cloned() {
                tr { td { "Lv {lv}" } td { {link(name, Pick::Move(mv), go)} } }
            }
        }
        h4 { "Trainers ({s.trainers.len()})" }
        for t in s.trainers.iter().copied() {
            div {
                {link(r.trainer(t).map_or_else(|| format!("#{}", t.0), trainer_label), Pick::Trainer(t), go)}
            }
        }
        h4 { "Wild on ({s.maps.len()})" }
        for m in s.maps.iter().copied() {
            div { {link(map_name(&game, m).to_string(), Pick::Map(m), go)} }
        }
    }
}

fn move_detail(r: &DataRows, m: &MoveRow, go: Callback<Pick>) -> Element {
    rsx! {
        h3 { "#{m.id.0:03} {m.name}" }
        table {
            tr { th { "Type" } td { {type_name(m.ty)} } }
            tr { th { "Power" } td { {or_dash(m.power)} } }
            tr { th { "Accuracy" } td { {or_dash(m.accuracy)} } }
            tr { th { "PP" } td { "{m.pp}" } }
            tr { th { "Effect" } td { "{m.effect:?} ({m.effect_chance}%)" } }
            tr { th { "Target" } td { "{m.target:?}" } }
            tr { th { "Priority" } td { "{m.priority}" } }
            tr { th { "Flags" } td { "{m.flags.0:#04x}" } }
        }
        h4 { "Learned by ({m.learners.len()})" }
        for s in m.learners.iter().copied() {
            div { {species_link(r, s, go)} }
        }
    }
}

fn trainer_detail(r: &DataRows, t: &TrainerRow, go: Callback<Pick>) -> Element {
    rsx! {
        h3 { {trainer_label(t)} }
        p {
            "#{t.id.0}"
            if t.female { ", female" }
            if t.double { ", double battle" }
            ", AI {t.ai:#x}"
        }
        table {
            tr { th { "Species" } th { "Level" } th { "IV" } th { "Moves" } }
            for m in t.party.iter() {
                tr {
                    td { {species_link(r, m.species, go)} }
                    td { "{m.level}" }
                    td { "{m.iv}" }
                    td {
                        match &m.moves {
                            None => rsx! { "level-up" },
                            Some(ms) => rsx! {
                                for (mv, name) in ms.iter().cloned() {
                                    {link(name, Pick::Move(mv), go)}
                                    " "
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

fn encounter_detail(r: &DataRows, e: &EncounterRow, go: Callback<Pick>) -> Element {
    rsx! {
        h3 { "{e.map_name}" }
        p { "Land encounter rate {e.rate}" }
        table {
            tr { th { "Slot" } th { "Species" } th { "Levels" } th { "%" } }
            for (i, (sp, _, lo, hi, pct)) in e.mons.iter().cloned().enumerate() {
                tr {
                    td { "{i}" }
                    td { {species_link(r, sp, go)} }
                    td { if lo == hi { "{lo}" } else { "{lo}–{hi}" } }
                    td { "{pct}" }
                }
            }
        }
    }
}

fn text_detail(t: &TextRow) -> Element {
    rsx! {
        h3 { "{t.key}" }
        p {
            for part in text_parts(&t.text) {
                match part {
                    Part::Text(s) => rsx! { span { "{s}" } },
                    Part::Tag(s) => rsx! { span { class: "tag", "{{{s}}}" } },
                    Part::Break('p') => rsx! { div { class: "box" } },
                    Part::Break(_) => rsx! { br {} },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_parts_split_escapes_and_placeholders() {
        use Part::*;
        let t = |s: &str| Text(s.into());
        assert_eq!(
            text_parts(r"ab\ncd{PLAYER}e\l\pf{STRING 3}"),
            [
                t("ab"),
                Break('n'),
                t("cd"),
                Tag("PLAYER".into()),
                t("e"),
                Break('l'),
                Break('p'),
                t("f"),
                Tag("STRING 3".into())
            ]
        );
        assert_eq!(text_parts(r"a\xb{c"), [t(r"a\xb{c")]);
        assert_eq!(text_parts(""), []);
        assert_eq!(text_parts("é{X}"), [t("é"), Tag("X".into())]);
    }

    #[test]
    fn genders_and_abilities() {
        assert_eq!(gender(255), "genderless");
        assert_eq!(gender(254), "always female");
        assert_eq!(gender(0), "always male");
        assert_eq!(gender(127), "49.6% female");
        assert_eq!(gender(31), "12.1% female");
        assert_eq!(ability(""), "(other)");
        assert_eq!(ability("X"), "X");
    }

    #[test]
    fn picks_open_their_sub_tab_and_ids_index_rows() {
        let r = rows();
        for s in &r.species {
            assert_eq!(r.species(s.id).map(|x| x.id), Some(s.id));
        }
        for m in &r.moves {
            assert_eq!(r.mov(m.id).map(|x| x.id), Some(m.id));
        }
        for t in &r.texts {
            assert_eq!(r.text(t.id).map(|x| x.id), Some(t.id));
        }
        assert_eq!(Pick::Map(MapId(1)).sub(), Sub::Encounters);
        assert_eq!(Pick::Text(TextId(1)).sub(), Sub::Texts);
        // A filter keeps the rows whose label contains it.
        let some = &r.species[1].name;
        let hits = r.list(Sub::Species, &some.to_uppercase());
        assert!(
            hits.iter()
                .any(|(p, _)| *p == Pick::Species(r.species[1].id))
        );
        assert!(hits.len() < r.species.len());
    }

    fn render(tab: Sub) -> String {
        let mut dom = VirtualDom::new_with_props(Data, DataProps { tab });
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    /// The ssr smoke: each sub-tab lists every row of its table (stub mode: 0 trainers and
    /// 0 texts).
    #[test]
    fn each_sub_tab_lists_its_rows() {
        let r = rows();
        let counts = [
            (Sub::Species, data_view::species().len()),
            (Sub::Moves, data_view::moves().len()),
            (Sub::Trainers, data_view::trainers().len()),
            (Sub::Encounters, data_view::encounters().len()),
            (Sub::Texts, data_view::texts().len()),
        ];
        for (sub, n) in counts {
            let html = render(sub);
            assert_eq!(html.matches("data-row=").count(), n, "{sub:?}");
            assert_eq!(r.list(sub, "").len(), n);
            assert!(html.contains(&format!("{n} rows")), "{sub:?}");
            for (_, label) in SUBS {
                assert!(html.contains(&format!(">{label}<")), "{label}");
            }
        }
        if content::STUB {
            assert_eq!((r.trainers.len(), r.texts.len()), (0, 0));
        }
    }

    /// Every detail panel renders, and its links point at rows that exist.
    #[test]
    fn details_render_and_links_resolve() {
        let r = rows();
        let picks = r
            .species
            .iter()
            .map(|s| Pick::Species(s.id))
            .chain(r.moves.iter().map(|m| Pick::Move(m.id)))
            .chain(r.trainers.iter().map(|t| Pick::Trainer(t.id)))
            .chain(r.encounters.iter().map(|e| Pick::Map(e.map)))
            .chain(r.texts.iter().map(|t| Pick::Text(t.id)));
        let exists = |p: Pick| match p {
            Pick::Species(id) => r.species(id).is_some(),
            Pick::Move(id) => r.mov(id).is_some(),
            Pick::Trainer(id) => r.trainer(id).is_some(),
            Pick::Map(id) => r.encounter(id).is_some(),
            Pick::Text(id) => r.text(id).is_some(),
        };
        let mut links = 0;
        for p in picks {
            #[component]
            fn One(p: Pick) -> Element {
                let go = use_callback(|_: Pick| {});
                detail(rows(), p, go)
            }
            let mut dom = VirtualDom::new_with_props(One, OneProps { p });
            dom.rebuild_in_place();
            let html = dioxus_ssr::render(&dom);
            assert!(!html.contains("Not found"), "{p:?}");
            for l in html.split("data-link=\"").skip(1) {
                let target = &l[..l.find('"').unwrap()];
                let all = r
                    .species
                    .iter()
                    .map(|s| Pick::Species(s.id))
                    .chain(r.moves.iter().map(|m| Pick::Move(m.id)))
                    .chain(r.trainers.iter().map(|t| Pick::Trainer(t.id)))
                    .chain(r.encounters.iter().map(|e| Pick::Map(e.map)));
                let to = all.into_iter().find(|q| format!("{q:?}") == target);
                assert!(to.is_some_and(exists), "{p:?} links to {target}");
                links += 1;
            }
        }
        assert!(links > 0);
    }
}
