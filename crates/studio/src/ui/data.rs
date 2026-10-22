//! Data tab (M6 Task H): species, moves, trainers, encounters and texts, read-only. The model
//! (rows, lists, links, text layout) is `data_view`'s; this file holds the components.
//!
//! The rows are built once per process on a background thread ([`data_view::rows`]); the tab
//! shows "Loading…" until they are ready. Each sub-tab has a filter box, a row list and a
//! detail panel; links jump between them (species to trainers and maps, move to learners, and
//! back).

use crate::data_view::{
    self, DataRows, EncounterRow, MoveRow, Part, Pick, SpeciesRow, Sub, TextRow, TrainerRow,
    ability_or_other, gender, or_dash, text_parts, trainer_label, type_name,
};
use crate::map_view::map_name;
use content::Emerald;
use dioxus::prelude::*;
use engine::data::SpeciesId;

/// The game, for map names.
const GAME: Emerald = Emerald::for_shells();

const SUBS: [(Sub, &str); 5] = [
    (Sub::Species, "Species"),
    (Sub::Moves, "Moves"),
    (Sub::Trainers, "Trainers"),
    (Sub::Encounters, "Encounters"),
    (Sub::Texts, "Texts"),
];

const CSS: &str = r#"
.data-tab { display: flex; flex-direction: column; gap: 8px; height: calc(100vh - 70px); }
.data-subs { display: flex; gap: 4px; }
.data-subs button { background: var(--panel); color: var(--muted); border: 1px solid #333; padding: 3px 10px; font: inherit; cursor: pointer; }
.data-subs button.active { color: var(--fg); border-color: var(--accent); }
.data-tab input { background: var(--panel); color: var(--fg); border: 1px solid #333; padding: 4px 6px; font: inherit; }
.data-main { display: flex; gap: 12px; flex: 1; min-height: 0; }
.data-tab .rows { width: 260px; overflow-y: auto; list-style: none; margin: 0; padding: 0; }
.data-tab .rows button { width: 100%; text-align: left; background: none; border: 0; color: var(--fg); padding: 2px 6px; font: inherit; cursor: pointer; }
.data-tab .rows button.active { background: var(--accent); color: #000; }
.data-tab .detail { flex: 1; overflow-y: auto; }
.data-tab table { border-collapse: collapse; }
.data-tab td, .data-tab th { padding: 1px 8px; text-align: left; }
.data-tab th { color: var(--muted); font-weight: normal; }
.data-tab button.link { background: none; border: 0; color: var(--accent); padding: 0; font: inherit; cursor: pointer; }
.data-tab .tag { color: #ffd84a; }
.data-tab .box { border-top: 1px dashed #444; margin: 4px 0; }
"#;

/// Resolves once the rows are built, building them on a thread (never on the UI thread).
fn load_rows() -> futures_channel::oneshot::Receiver<()> {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        data_view::rows();
        let _ = tx.send(());
    });
    rx
}

/// The Data tab. `tab` is the sub-tab it opens on.
#[component]
pub fn Data(#[props(default)] tab: Sub) -> Element {
    let mut ready = use_signal(|| data_view::rows_if_ready().is_some());
    use_hook(|| {
        if !ready() {
            let rx = load_rows();
            spawn(async move {
                let _ = rx.await;
                ready.set(true);
            });
        }
    });
    let sub = use_signal(|| tab);
    rsx! {
        style { {CSS} }
        div { class: "data-tab",
            match data_view::rows_if_ready().filter(|_| ready()) {
                None => rsx! { p { "data-loading": "1", "Loading…" } },
                Some(r) => rsx! { DataView { rows: RowsRef(r), sub } },
            }
        }
    }
}

/// The rows as a prop (compared by address).
#[derive(Clone, Copy)]
pub struct RowsRef(&'static DataRows);

impl PartialEq for RowsRef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

#[component]
fn DataView(rows: RowsRef, sub: Signal<Sub>) -> Element {
    let r = rows.0;
    let mut sub = sub;
    let mut query = use_signal(String::new);
    let mut pick = use_signal(|| None::<Pick>);
    let go = use_callback(move |p: Pick| {
        query.set(String::new());
        sub.set(p.sub());
        pick.set(Some(p));
    });
    let list = r.list(sub(), &query.read());
    let shown = pick().filter(|p| p.sub() == sub());
    rsx! {
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

fn detail(r: &DataRows, p: Pick, go: Callback<Pick>) -> Element {
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
        ability_or_other(&s.ability_names[0]),
        ability_or_other(&s.ability_names[1])
    );
    let stats = ["HP", "Atk", "Def", "Spe", "SpA", "SpD"];
    let total: u32 = s.base.iter().map(|&b| b as u32).sum();
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
            div { {link(map_name(&GAME, m).to_string(), Pick::Map(m), go)} }
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
    use crate::data_view::rows;

    fn render(tab: Sub) -> String {
        rows(); // built: the tab renders at once
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
        let all: Vec<Pick> = r
            .species
            .iter()
            .map(|s| Pick::Species(s.id))
            .chain(r.moves.iter().map(|m| Pick::Move(m.id)))
            .chain(r.trainers.iter().map(|t| Pick::Trainer(t.id)))
            .chain(r.encounters.iter().map(|e| Pick::Map(e.map)))
            .chain(r.texts.iter().map(|t| Pick::Text(t.id)))
            .collect();
        #[component]
        fn One(p: Pick) -> Element {
            let go = use_callback(|_: Pick| {});
            detail(rows(), p, go)
        }
        let mut links = 0;
        for &p in &all {
            let mut dom = VirtualDom::new_with_props(One, OneProps { p });
            dom.rebuild_in_place();
            let html = dioxus_ssr::render(&dom);
            assert!(!html.contains("Not found"), "{p:?}");
            for l in html.split("data-link=\"").skip(1) {
                let target = &l[..l.find('"').unwrap()];
                let to = all.iter().find(|q| format!("{q:?}") == target);
                assert!(to.is_some_and(|&q| r.has(q)), "{p:?} links to {target}");
                links += 1;
            }
        }
        assert!(links > 0);
    }
}
