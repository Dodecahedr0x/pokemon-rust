//! SIL6: Chapter 12 (the post-game) end to end, native, in the Silice flavour
//! (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 12 segments (`silice_spine::ch12`), each from its documented
//! start state: the main quest from Chapter 11's canonical end (119, at home) to the last ceiling
//! (131), then the optional goals the post-game opens (Benou, the Zone 404's Core, Crow's Lounge,
//! the Friends' Tournament in the Dojo, a gym leader's rematch, the Stele's « Dominant », an Antenna, a Code Fragment).

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, Start, Step, ch12};

const FRAME_LIMIT: u64 = 60_000;

fn play(start: Start, steps: Vec<Step>) -> Run {
    let mut r = Run::starting_at(FRAME_LIMIT, start);
    r.play(&steps);
    ch12::check_clean(&r);
    r
}

#[test]
fn act_1_the_first_morning() {
    play(ch12::reveil_start(), ch12::reveil());
}

#[test]
fn act_1_the_council_of_three() {
    play(ch12::conseil_start(), ch12::conseil());
}

#[test]
fn act_1_julien_and_his_daily_rematch() {
    play(ch12::julien_start(), ch12::julien());
}

#[test]
fn act_1_thomas_at_the_lighthouse() {
    play(ch12::phare_start(), ch12::phare());
}

#[test]
fn act_2_kimiko() {
    play(ch12::kimiko_start(), ch12::kimiko());
}

#[test]
fn act_2_arthur() {
    play(ch12::arthur_start(), ch12::arthur());
}

#[test]
fn act_2_silas_and_the_breach_alarm() {
    play(ch12::silas_start(), ch12::silas());
}

#[test]
fn act_2_silas_mail() {
    play(ch12::mail_start(), ch12::mail());
}

#[test]
fn act_3_the_horde_and_the_camp() {
    play(ch12::horde_start(), ch12::horde());
}

#[test]
fn act_3_the_tower_and_the_filter() {
    play(ch12::tour_start(), ch12::tour());
}

#[test]
fn act_3_the_outside() {
    play(ch12::dehors_start(), ch12::dehors());
}

#[test]
fn act_3_the_centrale() {
    play(ch12::centrale_start(), ch12::centrale());
}

#[test]
fn act_3_the_beacon() {
    play(ch12::balise_start(), ch12::balise());
}

#[test]
fn act_3_the_last_ceiling() {
    play(ch12::fin_start(), ch12::fin());
}

#[test]
fn optional_benou() {
    play(ch12::benou_start(), ch12::benou());
}

#[test]
fn optional_the_clone_and_mewtwo() {
    play(ch12::noyau_start(), ch12::noyau());
}

#[test]
fn optional_crows_lounge() {
    play(ch12::lounge_start(), ch12::lounge());
}

#[test]
fn optional_the_friends_tournament_in_the_dojo() {
    play(ch12::tournoi_start(), ch12::tournoi());
}

#[test]
fn optional_a_gym_rematch() {
    play(ch12::revanche_start(), ch12::revanche());
}

#[test]
fn optional_the_stele() {
    play(ch12::stele_start(), ch12::stele());
}

#[test]
fn optional_an_antenna() {
    play(ch12::antenne_start(), ch12::antenne());
}

#[test]
fn optional_a_code_fragment() {
    play(ch12::fragment_start(), ch12::fragment());
}
