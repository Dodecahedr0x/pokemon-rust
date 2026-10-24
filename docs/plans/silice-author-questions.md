# Silice: open questions for the author

Compiled 2026-10-24 from master `08136606`. This page gathers every open author question in one place. Each question appears once.

Sources:
- `2026-10-23-silice-content.md`: §4, §5, and the "Questions for the author" and "Author questions" block of every chapter and "as built" section;
- `2026-10-23-status-handoff.md`: "Author rulings waiting";
- `games/silice/check-exceptions.ron`;
- `games/silice/IMPORT-REPORT.md`;
- the "author question" comments in `crates/content/src/silice/` and `crates/content/tests/silice_spine/`.

The code builds and plays with the provisional choice that each item gives. To answer, give the item's id and the chosen option. Items already answered or superseded are listed at the end, so they are not asked again.

**Author rulings (2026-10-24):** G-01: the difficulty jump is fine (keep the Ch10–11 rosters; the player trains up). G-48: the Tower hall is only reachable with Dive after Ch9 (intended). G-49: Reverse is not in his gym while Thomas is at the Breach. G-50: there must be a Dojo map; the Sc21 tournament happens there. Audio: needed; use the extracted Emerald music and sound effects for now, the author will provide MIDI tracks later.

Ids: **G** = gameplay or blocking, **T** = text the agents invented or borrowed, **C** = cosmetic or naming, **X** = check-exception groups.

---

## 1. Blocking or gameplay-shaping questions

### 1a. Difficulty curve and party (read first)

**G-01 · Ch9 → Ch10–11 · The difficulty wall at the Tower**
- **Question:** The Tower bosses need a party of about 89, but Ch9 ends far below that. Should Ch9 train the player up, or should the Ch10–11 rosters come down?
- **The numbers:**
  - The Tower leaders and the Apex bosses have six 240-IV Pokémon each, with held items, Megas, Primals and Revenge's prism. Kimiko's HO-OH (Calm Mind, Recover, Leftovers) beat a KADABRA / TYPHLOSION at 88 / 92.
  - Ch9 ends with KADABRA 69, TYPHLOSION 76, LAIRON 35, MAREEP 14, PORYGON 25 and DUGTRIO 26.
  - The bench is weak, and three evolutions are still missing.
  - The Tower has no grass and only one heal (Mathieu's meal). The cabin climbs as soon as the story reaches 99.
- **Provisional:** the Ch10 and Ch11 spines start from a constructed party at 89: ALAKAZAM, TYPHLOSION 91, AMPHAROS with TM24, AGGRON, DUGTRIO and PORYGON2. They mend from the bag between leaders. Measured: 87–91 all win; 85 loses to Revenge. The chapters are not chained (`silice_spine/ch10.rs`).
- **Options:**
  - (a) Keep the rosters, and give Ch9 or the late game a real training ground and the evolutions: higher-level grass on Route S9, or trainers to rematch. Training on Route S9's level 45 grass would cost "millions of frames".
  - (b) Lower the Ch10–11 rosters: fewer Pokémon, lower levels or IVs, fewer items.
  - (c) Both: a moderate cut plus a late training spot.
  - (d) Add heals inside the Tower.

**G-02 · Ch4 → Ch8 · The intended party chain**
- **Question:** Which party should each chapter assume?
- **The gaps:**
  - The MAREEP line is never trained by a spine (MAREEP 12 in Ch4, FLAAFFY 21 in Ch5), yet Ch6 leads with an AMPHAROS 44.
  - Ch5 ends with a LANTURN, but Ch6 starts with a WAILORD from Route S5's sea, which needs Surf.
- **Provisional:** each chapter's start state is tuned by hand. These are Ch6 questions 1–2 and Ch7 question 12.
- **Options:**
  - Carry a FLAAFFY near 35 from Ch5, or lead Ch6 with another member (an ALAKAZAM needs about 57 for Tak).
  - Keep the WAILORD, or carry the LANTURN on.

**G-03 · Ch1 · The Chimiste's Koffing knows Self-Destruct**
- **Question:** Against a one-Pokémon party, the Self-Destruct can end in a double KO, which Gen3 scores as a whiteout. Keep the move or swap it?
- **Provisional:** kept. The spine adds a MAREEP before this battle (Ch1 question 12).
- **Options:** keep it, or swap the move.

**G-04 · Ch1 · No heal after Loïc**
- **Question:** Sc06 (Gogo and Romain) fires as soon as the player leaves the gym, with no chance to heal. Add a heal, or a Centre visit first?
- **Provisional:** no heal (Ch1 question 11).

**G-05 · Ch0 · Route S1 trainers at level 6**
- **Question:** The Route S1 trainers reach level 6 against a level 5 starter. Is that intended?
- **Provisional:** the spine trains Cyndaquil to 8 first (Ch0 question 7, from the source's §10).

**G-06 · Ch3 · The Raikou wall**
- **Question:** The leaders' Raikou (Rain Dance, Calm Mind, Thunder) is hard without a Ground type, and none is wild in Ch3. Add a Ground Pokémon to Route S3 or the garden?
- **Provisional:** the spine adds a DUGTRIO 26 (Ch3 question 12).
- **Options:** add a wild Ground type, or soften the Raikou.

**G-07 · Ch7 · The Gogo–Romain fight**
- **Question:** It is two singles with no heal between them, and it is the chapter's hardest fight. Should a nurse or a heal follow Gogo?
- **Provisional:** no heal (Ch7 question 10).

**G-08 · Ch10 · Kimiko's "émotion"**
- **Question:** The source says −1 Sp. Atk but does not say on whose side. Which side?
- **Provisional:** not built; reported `kimiko_emotion`.

**G-09 · Ch10 · The Cœur d'Évalon**
- **Question:** Does the Cœur work in every battle once received, or only on the Tower's 1F?
- **Provisional:** 1F trainers only, using Ch10's version (half the confusions shaken off, the rest last 2 turns at most), not Ch8's full immunity.

**G-10 · Ch3, Ch7, Ch5 · Held items from the source's fallbacks**
- **Question:** The battle rules now run for real. Should the fallback items stay or go?
  - **Ch3 strobe:** the fallback gives Bright Powder (Sableye, Dusclops) and a Quick Claw (Magneton). `trainers.ron` gives Sitrus Berry, Spell Tag and Magnet.
  - **Ch7 injectors:** the fallback items are Liechi Berry, Petaya Berry and Double-Edge.
  - **Ch5 Absol:** the fallback gives a Scope Lens, but the Absolite now works.
- **Provisional:** Ch3 and Ch5 use `trainers.ron`'s items; Ch7 keeps the fallback items alongside the rule.

**G-11 · Ch6 · The samurai's hasty variants**
- **Question:** A blink now means the normal samurai with the lead's Speed −1. Should `Gym_Sc01_Repli_Q` and the `_HATIF` trainers (+2 levels) be dropped, or kept as well?
- **Provisional:** both unused (field question 6).

**G-12 · Ch4 · Readings of the battle rules**
- **Questions:**
  - After Samuel's R-7 loss, the toll plays on the quarterdeck and the player stays there for the rematch. Is that the intended place?
  - Gwada's rum: the source says "ses Pokémon commencent avec". Does that mean each Pokémon as it enters?
- **Provisional:** yes to both. The same reading applies to Ch8's overdose and Ch7's injector split (+2/+2, −1/−1).

### 1b. Double battles, trainer sight and softlock layout changes

**G-13 · Ch3, Ch8 · `double: true` entries are now real double battles**
- **Question:** Is a double battle intended for each of these entries? This matters most for the two that are a single named trainer, Morgane and Élise.
  - Tauber & Hocseat (Ch3)
  - Léa & Zoé (Route S8)
  - MAGE NOIRE MORGANE and CLERC ÉLISE (Ch8 gym)
  - Loulpix & Doris
- **Impact:** the doubles raise the required levels. The spine now trains two members:
  - Ch3: TYPHLOSION 44 and LAIRON 35.
  - Ch8: both leads, to 68 and 76.
- **Provisional:** all are double battles since Wave G1 (merge `4048f83c`).
- **Options:** keep all; make Morgane and Élise singles; or turn the pairs into two-trainer doubles. That last option also fixes their names (C-01).

**G-14 · Ch1, Ch3, Ch7, Ch5, Ch6, Ch8 · Two-trainer and multi battles still fought as singles**
- **Question:** The engine now has two-trainer doubles and in-game-partner multis. Should these scenes switch to them?
  - **Played as two singles in a row** (`battle_two_trainers` in the prelude): Gogo & Romain in Ch1 Sc06, Ch3 Depot Sc02 and Ch7. The Ch7 entries `GOGO_3` and `ROMAIN_3` are themselves flagged `double: true`.
  - **Still the source's fallback** (`battle_multi`): the Ch5 East Cave with Praugect, the Ch6 Route S6 grunts with Adrien, and Ch8's Gogo and Romain under the overdose.
- **Provisional:** the source's own fallbacks.

**G-15 · Ch8 · Morgane and Élise sight lowered to 1**
- **Question:** Is sight 1 acceptable for both, or should the room be redrawn?
- **Why:**
  - **Morgane** (5, 21): with sight 3, she walked down to the 1-wide cell (5, 23) to fight and stayed there, shutting the runes' only way.
  - **Élise** (6, 3): with sight 2, she was drawn onto the stairs' only approach (5, 3).
  - Every route to the stairs still crosses their sight.
- **Provisional:** sight 1 for both (commit `ff68499f`).
- **Options:** keep it; redraw the rooms so they can keep sight 2–3; or have them step aside after the battle, as Gérard does (G-16).

**G-16 · Ch9 · Gérard steps aside after his battle**
- **Question:** Is this added step-aside acceptable?
- **Why:** after his battle, Gérard moves one or two cells east and his template is updated. Without the update, he respawned on (11, 11) and blocked pump B's console: a softlock. The step-aside is the agent's addition (Ch9 question 10).
- **Options:** keep it, or move his start cell.

**G-17 · Ch8 · Route S8's cracked rock gets a temporary flag**
- **Question:** Is Gen3 rock behaviour right here?
- **Why:** the rock at (31, 10) guards the braises nook. Without a flag it respawned on the next step and shut the player inside the nook: a softlock.
- **Provisional:** `FLAG_TEMP_11`. The rock stays gone during a visit and returns on the next load, as Gen3 rocks do.
- **Also:** the source says this rock blocks Q3's pigeon, but Q3 is in Évalon's Pré des Joutes, behind the brambles.
- **Options:** keep it, or give it a permanent flag.

**G-18 · Ch1 · Gym sight changes and an escape door**
- **Question:** Are these changes acceptable, or should the room be redrawn?
  - **Sight:** Iris went from 3 to 2 and Jules from 3 to 1. At sight 3, walking up to the player put each of them on the safe path. The workbench and a sbire were also moved.
  - **Sas2:** it re-opens the back door, so the player cannot be shut in.
- **Provisional:** as built (Ch1 questions 4–5).

**G-19 · Ch6 · Seiji can be walked round**
- **Question:** Should Seiji block the alley, as the samurai do?
- **Provisional:** he can be passed by (11, 5–7). The source does not make him mandatory, nor Ayame (Ch6 question 3).

**G-20 · Ch5 · Mathieu on the walkway**
- **Question:** Should Mathieu walk down instead, or leave before the log?
- **Why:** the walkway is the platform's only way down. Left there, he would wall the player in.
- **Provisional:** he fades out after the log (Ch5 question 1).

**G-21 · Ch3 · The head croupier steps aside**
- **Question:** Is the head croupier's fade to (6, 10) acceptable?
- **Why:** on the built B1F the capsule (8, 10) is solid, so he steps down, then fades to (6, 10) (Ch3 question 9).

**G-22 · Ch7 · Reverse's exit and Benou's walks**
- **Questions:**
  - Reverse moonwalks up into the great mirror. Should he leave another way, or should the map give him a corridor?
  - Benou's walks in Gym Sc02 and Sc07 are shortened. Are the new paths acceptable?
- **Provisional:** Reverse is hidden behind a fade (Ch7 questions 4–5).

**G-23 · Ch10 · The Studio's set**
- **Question:** Should the set open segment by segment, with Kimiko walled in some other way?
- **Why:** the three cut segments stay solid until all three are cut, then open at once. Otherwise one console opened a way to Kimiko.
- **Provisional:** all three at once (Ch10–11 scripts question 4).

**G-24 · Ch10–11 · Shortened choreography**
- **Question:** Are the shortened walks acceptable?
  - Poussin walks up 1 (not 3) and east 7 (not 10).
  - Gogo and Romain come up 2 cells (not 3).
  - Mathieu comes down 1 cell in the cabin, and the 2F drone 1 cell.
  - Sc10 walks the player 1 cell south (not 3).
- **Provisional:** as built (Ch10–11 scripts question 5).

**G-25 · Ch11 · Where the canonical save is made**
- **Question:** Is the save on the house's 2F, three days later, acceptable?
- **Provisional:** the save is made on the house's 2F, not on Nébula right after the credits. Continue then resumes at home with the epilogue ahead (question 3).

**G-26 · Ch11 · The Apex camera cell**
- **Question:** For the alternate ending's camera cell (8, 18), keep row 18 closed, or reopen a cell there?
- **Provisional:** row 18 closed (Ch10–11 maps question 2).

**G-27 · Ch12 · The steel beam**
- **Question:** The engine cannot fill a broken bridge. Is "the beam blocks the bridge and is pushed off it" acceptable, or should a script repaint the gap?
- **Provisional:** the beam is pushed off (Ch12 maps question 2).

**G-28 · Ch12 · Rayquaza after a failed battle**
- **Question:** Sc32 hides Rayquaza after a lost or fled battle, and it comes back on the next visit. Keep that, or leave it standing like the other legendaries?
- **Provisional:** hidden (Ch12 maps question 1).

**G-29 · Ch12 · The antennas' two floors**
- **Question:** Should each antenna be one 9 × 9 map holding both rooms, or a second map each (three new ids)?
- **Provisional:** one map each (both rooms on the 9 × 9 map; the receptionist warps to the top room (4, 6)).

**G-30 · Ch4 · The Port-Ressac canal is surfable**
- **Question:** Should the canal be surfable after the badge?
- **Why:** the source wants it fishable but not surfable, and Gen3 water cannot be both. Surf-proof water would need an engine rule.
- **Provisional:** fishable and surfable. Surfing it is harmless: it is a dead-end strip (Ch4 maps; Ch4 question 8).

**G-31 · Ch4 · Other Port-Ressac points**
- **Questions:**
  - The cargo's berth becomes sea once the ship sails. Should it stay sea, or become a usable Surf spot (question 7)?
  - The duel pirates appear directly at their cells, because the built quays have no room for their walk. Acceptable (question 4)?
  - The gangway pirate is hidden for good from 46. Should he come back from 47 to give `Rappel_Badge` after a loss?
  - The respawn is set by talking to the nurse, not by entering the Taverne. Is that right (question 9)?
- **Provisional:** as built.

**G-32 · Ch4, Ch10 · Strength**
- **Question:** Is pushing with A acceptable until Strength exists, or should these pushes wait for it?
- **Provisional:** the Champ Magnétique (Ch4 question 2) and the Tower's barrel are pushed with A.

**G-33 · Ch5 · Lokhlass after a failed battle**
- **Question:** The object can reappear as soon as its cell scrolls back into view, not only on the next load as the source says. Acceptable, or should the coral cave get an `OnTransition`?
- **Provisional:** as is (Ch5 question 10).

**G-34 · Ch6 · Rock Smash encounters**
- **Question:** Should breaking a rock start a `rock_smash` encounter? §6 lists none for Route S6.
- **Provisional:** no encounter.

**G-35 · Ch7 · The controls invert from story 72**
- **Question:** The controls invert from the welcome (story 72), when the player meets the distortion. Should they instead invert on the first entry?
- **Provisional:** from 72 (Ch7 question 9).

**G-36 · Ch7 · Reverse-City's residents after Reverse falls**
- **Question:** The residents walk backwards on every visit, also after Reverse falls. Should they stop with the inversion?
- **Provisional:** they keep walking backwards (field question 9).

**G-37 · Ch7 · The emitters**
- **Question:** Move to the full design?
- **The full design:** 300 still frames in a 3-cell zone show `Emetteur_T1` and confuse the lead, and the Scanner destroys the emitters without a battle. It needs non-trainer objects (a map edit) and a battle-start confusion.
- **Provisional:** the trainer-object fallback (field question 8).
- **Related:** should `Emetteur_T2` (switch it off without a battle) be offered before the fight, as a yes/no (Ch7 question 6)?

**G-38 · Ch7 · The vault**
- **Question:** Is one yes/no that dials the code, once all four plaques are read, acceptable? Or should there be four `Coffre_Code_n` texts and a real choice?
- **Provisional:** one yes/no (Ch7 question 3).

**G-39 · Ch6 · The gym quiz**
- **Question:** Will you write `Gym_Sc02_Q1_Choix` to `_Q5_Choix` (three options each), so a real choice can be built?
- **Why:** the source's multichoice has no option texts.
- **Provisional:** the quiz is degenerate. Reading each question's source sets a flag, and that flag decides the answer.

**G-40 · Ch3 · Missing seller and casino menus**
- **Questions:**
  - **Mathieu's stand:** nothing sells the Neon Skewer. Add the stand seller's object, or let Mathieu sell (question 13)?
  - **The roulette and the prize counter** wait for multichoice menus (question 8).
  - **TM34** is given twice, by Ch2's Technicien and by Tauber & Hocseat. Should the gym give another TM (question 5)?
  - **Mathieu heals once per visit.** Is a repeat heal wanted (question 3)?
  - **Casino state 3:** should Gogo stay upstairs (question 16)?
- **Provisional:** as built.

**G-41 · Ch9 and Ch10 · Heals and the Tower entrance**
- **Questions:**
  - `Tour_Hall_Sc02` heals the party, as the source's comfort note asks, but the source wants Ch10 to confirm (Ch9 question 4).
  - The guards in Sc03 are moved under a fade. Which way do they run (Ch9 question 7)?
  - Pump B switches off silently, because the source gives it no question. Is that intended (Ch9 question 2)?
- **Provisional:** heal kept; fade; silent.

**G-42 · Mallette · Field modules in the party menu**
- **Question:** The party menu lists SURF, VOL, CASCADE and PLONGÉE on every Pokémon, and only VOL works there. Should Silice hide them from the party menu (the bible says "les modules s'utilisent depuis la Mallette") and add a Mallette FLY entry instead?
- **Provisional:** shown in the party menu (field question 10).

**G-43 · Mallette · What APPELS and CARTE show**
- **Question:** What should APPELS list (calls to replay?) and CARTE show (the RDV signal per story step)?
- **Provisional:** placeholder lines: "Aucun appel en attente." and "Cette fonction n'est pas encore disponible." (field question 2).

**G-44 · Ch12 · Placeholders that block the Ch12 scripts**
- **Question:** The source leaves these undefined. What should each be?
  - `<Champion>_PG_Revanche_Deja`: the per-champion texts are missing.
  - `FLAG_SIL_ITEM_PG_GEM_LOIC … _LOULPIX`: list the flags, and give each Mega gem its own row and gym map (the source says only "arènes").
  - `SPECIES_<NOM>`: the legendary table.
  - `FLAG_DAILY`, `FLAG_GEMME`, `FLAG_PREMIERE`: real flag names, and a daily-reset mechanism for the rematches.
  - `MAP_ROUTE_SXX_ANTENNE`: the route number is undecided.
- **Provisional (SIL6 scripts):** one shared `PG_Revanche_Deja` line; the gem flags `FLAG_SIL_ITEM_PG_GEM_<LEADER>`, one Mega Stone per leader given in the leader's gym; the legendaries' species and levels from the §1 table; `FLAG_SIL_DAILY_*` reset by the engine's clock (`VAR_DAYS`); the antennas on Routes S2, S3 and S7.

**G-45 · Ch12 · `MOVE_SIL_CODE_AQUA`**
- **Question:** The move (Sc28) contradicts D-00 and D-02b, which allow Gen3 moves only. Is it new move data, or the documented tutor fallback? Due by SIL6.
- **Provisional (SIL6):** the source's Gen3 fallback per starter (Meganium Ancient Power, Typhlosion Crush Claw, Feraligatr Crunch) through Gen3's tutor screen: the player picks the mon, and any mon can learn it (reported as a fallback).

**G-48 · Ch12 · The Tower hall after Ch9**
- **Question:** Nébula has no door into the Tower hall after Ch9, so the post-game reaches the hall (and its elevator hub) only by Dive, as Ch9 does. Intended, or should a door open?
- **Provisional:** Dive only.

**G-49 · Ch12 · Reverse in his gym from 123**
- **Question:** Reverse's rematch is in his gym from 123, while Thomas is also at the Breach. Is that the intended timing?
- **Provisional:** in the gym from 123 (`ch07`).

**G-50 · Ch12 · Sc21's tournament in the Library**
- **Question:** The source sends the player to a Dojo that has no map; a warp also ends a script. Sc21's three battles are fought in place in the Library. Acceptable, or should a Dojo map be built?
- **Provisional:** in the Library.

**G-51 · Ch12 · The Antennas' battles and rule**
- **Question:** The three battles run from the top room's on-frame script after the receptionist's warp. The rule is checked at the door only; "ni du Sac" is not enforced in battle. Acceptable?
- **Provisional:** as described.

**G-46 · All · The Old Rod**
- **Question:** 11 maps fill old-rod slots, but no Old Rod is ever given. Give one somewhere, or drop the slots?
- **Provisional:** the slots are unreachable (the author's own note in ch04 §10; see X-03).

**G-47 · All · Original art and music**
- **Question:** Will you supply original art and music, or do the Emerald placeholders stay? This is the one §5 decision still open.
- **What is needed:** about 102 trainer pics, 36 Mega and Primal pics, tilesets, the 13 `MUS_SIL_*` tracks, and back-facing pics (Reverse's are mirrored for now).
- **Provisional:** Emerald assets referenced by name.

---

## 2. Text the agents invented or borrowed

### 2a. New text ids, not in `contenu-jeu`

There are 65 ids, found by checking `text/speakers.ron` against the source markdown. The 2 `_Ch09` renames are left out; they are under C-12. Each item asks whether to keep the wording or rewrite it.

**T-01 · The Mallette (41 ids, chapter `00-prologue`, `scripts/mallette.rs`)**
- **Menu:** `Mallette_Label`, `Mallette_Menu_Analyse`, `_Carte`, `_Appels`, `_Modules`, `_Rapports`, `_Synchro`, `_Retour`.
- **Panels:** `Mallette_Analyse`, `_Appels`, `_Carte`, `_Sync`, `_Verrouille`, `_Modules_Aucun`, `_Module_Pret`.
- **Module names:** `Mallette_Module_{Coupe,EclateRoc,Flash,Force,Surf,Vol,Cascade,Plongee}`.
- **Module prompts:** `Mallette_Surf_Ask/_Used`, `_Cascade_Ask/_Bloque/_Used`, `_Plongee_Ask/_Remonter/_Used`.
- **Logs:** `Mallette_Log_01`…`_08`, `_Log_Chiffre`, and `Mallette_Log08_Texte` ("BZZT… Fichier illisible.").
- Gen3's Surf prompt names a Pokémon, and a module has none, so the prompts had to be written.

**T-02 · Battle engine strings (13 ids, chapter `moteur`, `build/battle.rs` `gen_strings`)**
- `Battle_MegaReacting`, `_MegaEvolved`, `_PrimalReversion`, `_HarshSunBegan`, `_HarshSunFaded`, `_HeavyRainBegan`, `_HeavyRainLifted`, `_FizzledInRain`, `_EvaporatedInSun`, `_BreaksTheMold`, `_BouncedBack`, `_MegaArmed`, `_SolarPowerHurt`.
- **Related:** should Tak's `Gym_Mega_T2` and `_T3` replace `Battle_MegaReacting` and `_MegaEvolved` in his battle? They are unused today.

**T-03 · Ch3 (`ch03_gym.rs`, `ch03_casino.rs`)**
- `Nocturne_Strobe_Blanche`, `_UV`, `_Neon`: from §4.5's table, as messages.
- `Casino_B1F_Porygon_DO`: the PORYGON gift.

**T-04 · Ch4:** `Port_Ressac_Gym_Roulis`, the roll's message. It uses the source's wording « Le navire tangue!… ».

**T-05 · Ch8:** `Evalon_Surdose_Rage` and `_Fatigue`, the overdose messages taken from §3.

**T-06 · Ch10:** `Tour_4F_Prisme_On` and `_Off` (the prism), `Tour_Coeur_Pulse` (the Cœur) and `Tour_Ascenseur_Panneau` (the hall's elevator sign).

### 2b. Texts the source never wrote

**T-11 · Ch12 · Silent, reused or invented lines (SIL6 scripts)**
- **Silent (no text in the source):** Benou's door sign, painting and door; the Zone 404's slabs; the Braille wall; the Apex's socle; the Crater's boulder.
- **Turn only:** Gwada outside Sc30; Adrien before 122; the citizens; Julien at 120.
- **Absent:** the Sc19 actors; Arthur's Port-Ressac NPC.
- **Invented from the source's list:** the elevator labels (1F, 2F, 3F, 4F, SOMMET, TOIT, ANNULER).
- **Reused:** Mathieu repeats `Sc02_T1`; Gwada's reminders reuse her lines; the workbench carries Silas's line; each defeat text is the source's after-battle line.
- **Sound:** the fragment's bip is `SE_PIN`.

These still need lines from you. Until then the scripts borrow a nearby line or say nothing.

**T-07 · Lines to write.** Each wants either a real text or confirmation that silence or a borrowed line is fine.
- **Battle lines:**
  - Kimiko's six send-out lines.
  - Poussin's and Cocorico's mid-battle lines (first KO, Perish Song, weather, under 25 % HP) and their Primal staging.
  - A Mega line for Loulpix's Mawile.
- **The deaf room (Ch2):** it has no message of its own (a sound move prints "But it failed!"). Should it open with `Gym2_Sourde_Panneau`?
- **The Ch6 groomer:** no line for a player short of 500 ₽; `Toiletteur_T3` stands in (field question 5).

**T-08 · Trainers with no intro, defeat or post-battle lines**
- **Ch0:** no `_PostBattle` texts, so the defeat line repeats (question 2).
- **Ch6:** Luna, Hoshi, Yumi, Gaston, Kotaro and Armand borrow Ch6 lines.
- **Ch7:** Route S7's trainers borrow lines from their zone.
- **Post-badge lines:** Yohan repeats his defeat line (Ch2); Samuel says `Sc02_T3b` (Ch4).

**T-09 · NPCs and objects that borrow a line or say nothing**, by chapter:
- **Ch0:** Crow, the bar friends, Maman before story 3 and the town friends borrow lines; the story-1 guard says Mathieu's "take it". The desk and bed are silent outside story 2 (questions 1, 3, 5).
- **Ch1:**
  - Milo and Rose give their pieces silently.
  - The red crate and the workbench are silent.
  - The Conseiller and the Gardien repeat their lines (questions 1–3, 6).
- **Ch2:**
  - The Technicien and the Agent only turn to the player after their gifts.
  - Skitty, Lainergie and the Tour Top consoles borrow lines.
  - The Usine crates and the Route server reuse `Depot_Bloc_T1`/`_T2` (questions 1, 2, 4).
- **Ch3:**
  - `Viber_City_Neon_T2` is missing, so the sign is silent after the first reading.
  - The refugees under blankets borrow lines.
  - The slot machines have no win text, the coin counter no "not enough money" text, the 20 hidden coins no text.
  - The manager repeats the rules after the Fresh Water.
  - `Depot_Caisse_T1` has no event on the built depot (questions 1, 2, 6, 7, 14).
- **Ch4:**
  - The client sailor, the gangway pirate, the duel pirates, the decor grunts and the closed grille borrow lines, as do Gogo and Romain after Sc03, Crow outside 42, the terminal and the quay gate.
  - `Npc_Sbire` and `Gym_Guide` only half fit (question 1).
- **Ch5:** the Centre's pilgrim, the lift Watchers, the orphanage's drawings and the fisherman after his gift (question 4).
- **Ch6:** Kimiko on the Dojo stage borrows the stream line; the Skitty and the disappointed fan before 63 are silent.
- **Ch7:** the bins are silent. Should they say a line, or hide an item as bins 2 and 4 do (question 2)?
- **Ch8:**
  - The dark gallery reuses Ch4's `Route_S04_Mur_Sombre`; room 4's sign uses the Sphinge's challenge; the crypt guard reads room 3's sign.
  - The bard borrows the role-player's line below the Paladin rank.
  - The taverne customer, Hilde after Q2, the fairy after Q1, the swords on the wall, Old Bohort, the crackle's line (`Evalon_Sc01_T1`), the fountain, the four lecterns and the stepladder all borrow lines.
- **Ch9:** the crowd, the guards, the barricade and the drone; the Comptoir's client; the Grande Porte and the maintenance door (question 3).
- **Ch10–11:** Poussin, Julien and the diffuser have no idle lines.

**T-10 · Unused source texts: keep for later, or drop?**
- **Ch5:** `Route_S05_Npc_Marin_Ilot`, `Owlys_Gym_Trappe`, `Owlys_Gym_Sc01_Mega`.
- **Ch7:** `Injecteur_*`, `Gym07_Inverse_Debut` (now used as Reverse's intro) and `Reverse_Mart_Caissier_Bonus`.
- **Ch6:** `Gym_Sc01_Repli_Q` (see G-11).

---

## 3. Cosmetic and naming questions

**C-01 · Names over the length limits**
- **Question:** Shorten these names, or pick other classes?
- **Trainer names**, over 10 bytes (8 entries):
  - `TAUBER&HOCSEAT` and `TAUBER&HOCS` (×2), `LOULPIX & DORIS` (×2) and `LOULPIX&DOR`: these pairs could become two-trainer doubles instead (G-13).
  - `CHEF DE SILO`, `CONTREMAÎTRE`.
- **Class names**, over 12 bytes (10 entries): `MAÎTRES DU DONJON` (×2), `TOP DRESSEUSE` (×3), `DRESSEUR DRAGON`, `CHEF SÉCURITÉ`, `PLONGEUR COCKET`, `PLONGEUSE COCKET`, `TECHNICIEN COCKET`.
- **Provisional:** the names are truncated.

**C-02 · Item names over 13 bytes (16 entries)**
- **Question:** What should each one be shortened to? Among them: "Chronique de la Clôture", "Plan de la Prise d'Eau", "Composant de Filtrage", "Brochette Néon", "Carte Perforée", "Pain de Lembas", and "Mewtwoïte X et Y", which is one name for both stones.

**C-03 · Item names that clash with Gen3 items**
- **Question:** These share a bag name with a Gen3 item. Rename them?
  - "Poudre Claire" (`ITEM_SIL_POUDRE_CLAIRE`) and Gen3's Bright Powder;
  - "Orbe Bleu" and "Orbe Rouge" and Gen3's orbs;
  - the two Mewtwo stones, which share one name.

**C-04 · Mega form names**
- **Question:** Mega forms keep the base name ("ALAKAZAM"), because "MÉGA-ALAKAZAM" is over Gen3's 10 characters. Fine?

**C-05 · Sounds and music**
- **Question:** Confirm these mappings or name Silice sounds.
  - `SE_EXPLOSION` → `SE_M_EXPLOSION`, `SE_RING` → `SE_POKENAV_CALL` (Ch0 question 6).
  - The Ch4 mappings (Ch4 question 6).
  - Ch6: hum → `SE_PC_ON`, gong → `SE_ARENA_TIMEUP1`, shoji → `SE_SLIDING_DOOR`, rock → `SE_M_ROCK_THROW`.
- **Invented `SE_*` without the `SE_SIL_` prefix:** `SE_APPEL`, `_SIFFLET`, `_GONG`, `_SERVEURS`, `_SHOJI`, `_COFFRE`, `_HACK`, `_CLOCHE_BRONZE`, `_GRESILLEMENT`, `_FILTRE`, `_MALLETTE_APPEL`, `_MALLETTE_BIP`. Each needs a Silice name or a Gen3 sound.
- **Music:** `MUS_MAUVILLE`, `MUS_MOSSDEEP`, `MUS_ROUTE111`, `MUS_ENCOUNTER_MYSTERIOUS` and `MUS_KIMIKO_STREAM` are not decomp tracks. Pick a Gen3 track or a `MUS_SIL_*` name.

**C-06 · Missing glyphs**
- **Question:** For each glyph, add a new glyph or reword the text?
  - `—` (68 texts), `_` (25, the `LOG_xx` names), `°` (15, plus the trainer names `N°7` and `N°12`), `[ ]` (12), `₽` (3), `○ ●` (2), and `♪`, `#`, `*`, `←→` (1 each).
  - Gen3 `EMOJI_*` symbols could stand in for some of them: underscore, note, circle, sphere.
  - Today the Mallette's log titles read "LOG 01", with no underscore.

**C-07 · Screen colours and effects**
- **Questions:**
  - Should GOLD (the flashes) and GOLD_ON_WHITE (the Elysium end text) get their own colours? Today they are YELLOW and white on black (Ch10–11 question 1).
  - `screen_flash(NEGATIF)` is a white flash, because the engine has no palette inversion. Is that enough (field question 3)?
  - The default lengths: a flash is 16 frames, a shake 20 (field question 4).
  - The red security-light metatile: the sas use 537. Which tile should it be (Ch9 question 8)?
  - Is a switched-off jammer graphic wanted (Ch2 question 5)?
  - The petals have no animation.

**C-08 · Battle backgrounds and pics**
- **Questions:**
  - Skycker's night background has no Gen3 environment. Which should stand in: cave or building (Ch5)?
  - Reverse's pic is mirrored, and the palette glitch and the menu-mirroring bonus are not built (Ch7).

**C-09 · Weather**
- **Question:** Route S9 uses `WEATHER_SHADE` for the source's `WEATHER_CLOUDS` ("ciel couvert numérique"). Keep it (Ch9 maps)?
- The source's `WEATHER_PETALS` is now a battle rule (8 turns: Grass and Bug heal 1/16), not a weather.

**C-10 · Heal-location cells**
- **Question:** Confirm the cell each chapter uses:

  | Chapter | Map | Cell |
  |---|---|---|
  | Ch1 | Bois-Éther | (27, 13) |
  | Ch2 | Gris-Échec | (5, 9) |
  | Ch3 | Viber-City | (6, 15) |
  | Ch4 | the Taverne | (4, 8) |
  | Ch5 | Owlys | (15, 30) |
  | Ch6 | Manga-Square | (15, 24) |
  | Ch7 | Reverse-City | (4, 21) |

- The source's heal locations also need the `SIL_` prefix: `HEAL_LOCATION_{PORT_RESSAC,OWLYS,BRICO_BOURG}`.

**C-11 · Paths re-derived from the built maps**
- **Question:** Are these guesses right?
  - Ch1: Loïc's and Adrien's exits.
  - Ch2: Gwada's paths and the robot.
  - Ch3: the scene paths.
  - Ch5: the Umbreon, Milo and Mathieu paths, which the source gives without direction or length.
  - Ch6: Adrien stops one cell short, at (37, 3).

**C-12 · Duplicate text ids**
- **Question:** Keep the provisional names, pick others, or merge the texts?
- `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are defined in both ch02 and ch09, with different bodies.
- **Provisional:** the ch09 pair is renamed `_Ch09` (§5.6). The rename is reversible.

**C-13 · Small naming and data points**
- Ch1: the mart and the herb shop have no shop names (question 7).
- Ch1: Vieux says his `_Nuit` line at stories 1–2 and his day line at 0. Is that split right (question 4)?
- Ch3: the PORYGON's OT is "COCKET", but the source gives no OT id (0 is used) (field question 7).
- Ch5: the Silo Chief's new hide flag, `FLAG_SIL_HIDE_OWLYS_SILO_CHEF` (1060). Should the source name it (question 2)?
- Ch9: the source mentions a PC in the Centre. No Silice Centre has one yet.
- Undefined maps:
  - `MAP_MANGA_SQUARE_HOUSE_1` and `_2`: named, but with no content.
  - `MAP_BRICO_BOURG_NIGHT` and `MAP_TRONE_ZENITH_NUIT`: fallback only.
- The `FLAG_SIL_HIDE_BAR_SBIRES` tombstone: the source should name `_SBIRE_A` and `_B`.

---

## 4. Summary of `check-exceptions.ron`

There are **250 entries**:
- **244** carry `ruling_needed: true`. The plan counted 249 at import time; some have been resolved since.
- The **10** `false` entries (the post-game gifts' `placement-map`) were removed by the SIL6 scripts, which now place those gifts.
- **6** carry no flag: 5 mechanical `sil-prefix` renames (the three heal locations, `MUS_RG_SEKAIKAN` → `MUS_RG_NEW_GAME_INTRO`, `MUS_TEAM_AQUA_HIDEOUT` → `MUS_AQUA_MAGMA_HIDEOUT`) and one `item-name-clash` (the Mewtwo stones).

### 4a. By category

| Id | Rule | Count | Ruling | What it is | Covered by |
|---|---|---|---|---|---|
| X-01 | `text-charmap` | 104 | yes | glyphs: `—` 68, `_` 25, `°` 15, `[ ]` 12, `₽` 3, `○●` 2, `♪ # * ←→` 1 each; 2 trainer names | C-06 |
| X-02 | `item-incomplete` | 55 | yes | items missing a name, pocket or base item (below) | X-07 |
| X-03 | `fishing-unreachable` | 11 | yes | old-rod slots with no Old Rod | G-46 |
| X-04 | `sil-prefix` | 29 | 24 yes, 5 none | 14 invented `SE_*`, 7 `MUS_*`, 2 `WEATHER_*`, 3 pseudo-flags, 3 heal locations | C-05, C-09, C-10, G-44 |
| X-05 | `item-name-length` | 16 | yes | item names over 13 bytes | C-02 |
| X-06 | `trainer-class-name` | 10 | yes | class names over 12 bytes | C-01 |
| — | `trainer-name` | 8 | yes | trainer names over 10 bytes | C-01, G-13 |
| — | `map-undefined` | 5 | yes | `MAP_ROUTE_SXX_ANTENNE`, the Manga-Square houses ×2, `MAP_BRICO_BOURG_NIGHT`, "arènes" | G-44, C-13 |
| — | `item-name-clash` | 4 | 3 yes | Poudre Claire, Orbe Bleu and Rouge, the Mewtwo stones | C-03 |
| — | `dynamic-unresolved` | 4 | yes | Revanche_Deja, the gem flags, `Gym_Sc02_Q<n>_Choix`, `SPECIES_<NOM>` | G-39, G-44 |
| — | `text-duplicate` | 2 | yes | `Tour_Sbire{1,2}_Intro_Ch09` | C-12 |
| — | `invented-move` | 1 | yes | `MOVE_SIL_CODE_AQUA` | G-45 |
| — | `id-removed` | 1 | yes | `FLAG_SIL_HIDE_BAR_SBIRES` | C-13 |

### 4b. Entries that bend gameplay

- **The Old Rod** (G-46). These 11 maps have old-rod slots that cannot be reached:
  - No Man's Land, Port-Ressac, Reverse-City and Viber-City;
  - Route S4; Route S5 with its Banc and Grotte Corail; the Bois des Soupirs; Route S8; Route S9.
- **`Gym_Sc02_Q<n>_Choix`**: the Ch6 quiz cannot offer a choice (G-39).
- **`MOVE_SIL_CODE_AQUA`** (G-45).
- **The Ch12 placeholders** (G-44): `FLAG_DAILY` needs a daily reset; `FLAG_GEMME`, `FLAG_PREMIERE` and the gem flags need real names; `MAP_ROUTE_SXX_ANTENNE` and "arènes" decide where the antennas and the Mega gems go; `SPECIES_<NOM>` and `<Champion>_PG_Revanche_Deja` are the legendary table and the rematch texts.
- **`WEATHER_PETALS` and `WEATHER_CLOUDS`**: the petals are now a battle rule; the clouds became `WEATHER_SHADE` (C-09).
- **The trainer-name entries for the pairs** (`TAUBER&HOCS…`, `LOULPIX…`): the check's own suggested fix is "a double battle with two trainers", which would change the battle format (G-13).
- **Items that affect play** and still lack a pocket or base item:
  - `EVOLUROC` (held, Eviolite-like);
  - `POUDRE_SILICE` (a battle item: Sp. Atk +2 and poison);
  - `PIERRE_LIEN` (an evolution stone);
  - `BROCHETTE_NEON` (heals 60 HP);
  - `RHUM_PIRATE` and `BOUTEILLE_PUNCH`;
  - the two orbs (Primal Reversion).

  The build falls back on the base item's pocket. A wrong guess puts an item in the wrong pocket, or with the wrong hold effect.

### 4c. `item-incomplete` (X-07)

- **Mega stones with no name, pocket or base item (11):** Absol, Ampharos, Banette, Blaziken, Gardevoir, Gengar, Gyarados, Mawile, Metagross, Slowbro and Venusaur. With no name, they appear in the bag nameless.
- **Mega stones with no pocket or base item (23):** among them Charizardite X and Y, Alakazite, Aggronite, Steelixite, the Mewtwonites and Latiosite. The two orbs (Orbe Bleu, Orbe Rouge) are also in this group.
- **Other items with no pocket:** Évoluroc, Poudre Silice, Poudre Claire, Pierre Lien, Brochette Néon, Bouteille de Punch, Rhum Pirate, Épée de Légende, Larme de Fée, Millésime Sam, Orbe d'Éveil, Pain de Lembas and Plume d'Or.
- **Other items with no base item:** the Mallette, Carte Perforée, Accès Bêta and Gamma, Plume Ombrée and Vieux Patch.

### 4d. Leftovers from `IMPORT-REPORT.md`

- **§7 tables not imported:**
  - the Ch3 prize counter's Pokémon table (species, level, coins);
  - the Ch8 reputation-rank rewards;
  - the Ch9 objects table.

  Confirm that the scripts' versions match your intent.
- **Elided references in the source:** `..._Repli` (05-owlys:602) and `…` (06-manga-square:1018). Which ids were meant?
- **Placement cells that are not one flag:** `FLAG_SIL_ECHANTILLON_1/2/3` (12-post-game:4736), left unset.
- **A name conflict:** "Accès Alpha" was kept for `ITEM_SIL_ACCES_ALPHA`, but the source's row reads "Accès Alpha / Bêta / Gamma".

---

## Answered or superseded (not asked again)

- **§5.1–5.5:** answered by the 2026-10-23 decisions (source of truth, map editor, canonical Megas, roadmap). Only the art and music question stays open (G-47).
- **§5.7 and Ch9 question 5 (the elevator):** resolved at the SIL-9 merge. `Tour_Hall_Sc02` heals, sets 99 and warps to the cabin (3, 6). The colour question that remained is C-07.
- **Ch1 question 10, Ch5 question 3, Ch7 question 11 and Ch9 question 1:** answered in the plan.
- **Ch2 question 6:** Yohan's Mega line is now said in battle.
- **Ch4 question 3:** superseded by R-7. There is no whiteout now; see G-12.
- **Ch5 question 8 (no grass to train on):** cave floors now draw wild battles (off-grass encounters, M9), and Ch5 runs without its hook. Reopen it if the caves are not meant as training grounds.
- **Ch5 question 9 (Absol's Scope Lens):** see G-10. The Absolite now works.
