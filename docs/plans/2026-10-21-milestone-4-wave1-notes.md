# M4 Wave 1 hand-off notes (for Wave 2 tasks I, K, E2 and Task J)

## From C (Pokémon model)
- J: wild encounter caller keeps 4-step immunity (`encounter_immunity: u8` in World, hashed; reset after battle and on map load, as RestartWildEncounterImmunitySteps) and updates prev behavior every completed step.
- J: call create_wild right after try_land_encounter Some, no RNG draws in between. Pass lead = party[0] even if fainted.
- J/I: GiveMon/starter: random32() personality first, then Pokemon::new(.., Ivs::Random, personality, player_id, rng).
- I: TrainerData.name and SpeciesInfo.name must be the exact FR ROM bytes (name hash drives trainer personalities).
- D/E2: exp loop: add_exp_step → LevelUp → moves_at_level → try_learn; Learn::Full → Need::LearnMove → replace_move. Use Pokemon::max_pp. gain_evs after KO is caller's job.
- mon::test_tables duplicates battle/test_data.rs — consider merging in Wave 2.

## From G (transition / starter)
- J: BattleTransition::update(f) each frame after field frame, objects frozen; owns brightness/lighten/scroll[0]. After battle re-init field TextBox + BG0 (tiles 510/511, bank 15, SB 31 overwritten).
- J: StarterChoice: load_step while black until true, then update. Overwrites BG CB0, SB 6/7, OBJ tiles from 512, OBJ pal 0..4, BG palettes → full field restore afterwards. update rebuilds f.objs and all scrolls.
- I/A: add Str::BirchInTrouble (gText_BirchInTrouble); StarterChoice prints it in Stage::Enter and Stage::Decline (one line each).
- F: starter label renderer duplicates glyph drawing; switch to text::Printer.
- B/I: BattleGfx starter shapes: starter_bag.tilemap = birch_bag.bin padded to 1024 + birch_grass.bin (1024); starter_ball = 64-tile pokeball_selection sheet (still 0, tilts 16/32, hand 48); starter_circle 64 tiles; palettes[0] each.
- Agents: use unique scratchpad log filenames.

## From F (battle text / windows)
- B/I: BattleGfx::textbox: tilemap = full 32x64 textbox_map.bin (2048 entries); palettes[0..2] = gBattleTextboxPalette, [2] = gBattleWindowTextPalette (BG bank 5), [3] = gPPTextPalette.
- I: battle strings encoded with battle charmap ({B_BUFF1} = FD 00); never pass through field text::expand.
- FR wild/foe tags are suffixes " sauvage"/" ennemi" (leading space = suffix).
- D/E2: B_EFF_NAME_WITH_PREFIX reads msg.target; ACTIVE/SCR_ACTIVE/B_TXT_26 read msg.active; Buff::Mon{side} = battler on that side.
- K: BattleWindows::load bulk (black only). MsgWait::Frames(n) keeps is_printing n updates; Button waits new A/B; strings ending \p already wait, don't add Button. print switches to message view. MoveChoice::Back in learn view = don't learn; K runs "stop learning?" yes/no. Cursors persist between turns.
- No FONT_NARROW in GameData: move names clipped at 64px (possible later addition).

## From E (battle visuals)
- OBJ VRAM: pics 0/64/128/192, player box 256..384, enemy box 384..448, HP bars 448/456, ball 464, free 468..512; opp box palette bank 7. Update video::layout::battle comment.
- B/I BattleGfx: hpbar = hpbar.png + hpbar_anim.png (30 tiles), palettes[0]; status = status.png + status2.png (30); misc = misc.png + misc_frameend.png (12); healthbox_player 128 tiles, opponent 64 (Gen3 decompressed order) palettes[0]; ball frame 0 first 4 tiles; small_font normal-font glyph layout, 0x105 = "Lv".
- K: update every component once/frame; HpBar::set after Healthbox::load; during drain healthbox.set_hp(bar.value()); after Healthbox::load → ExpBar::invalidate then set; ExpBar::start with |l| exp_for_level(growth,l); on level() change healthbox.set_level; reload pic after faint before show; busy() includes tint fades; MonSprite::load needs shiny from Pokemon::is_shiny.
- Healthbox::load bulk: black frame or hidden box only.

## From B (battle graphics extract)
- BattleGfx: textbox 256 tiles, 2 palettes (BG 0-1), 32x64 map; environments[10] tiles + 32x64 map + 3 palettes (BG bank 2..4); healthbox_player 128 tiles (OBJs at 0 and 64); healthbox_opponent 64 (OBJs at 0 and 32); hpbar 30 (0 black end, 1-2 "PV", 3-11 green, 12-20 yellow, 21-29 red) uses HP bar palette gBattleInterface_BallDisplayPal; expbar 9; status 30 (0-14 player, 15-29 opp; PSN,PAR,SLP,FRZ,BRN x3 tiles); misc 12 (GFX_36..46 + 65 frameend; idx 3 blank); ball 12 tiles (3 frames 16x16); new field starter_grass (BG2 map only, uses bag tiles/palettes); numbers EMPTY. expbar/status/misc/healthboxes use gBattleInterface_BallStatusBarPal.
- "Lv" = small font glyph 0x100 + CHAR_LV_2 (E assumed 0x105 — check).
- F assumed textbox palettes[2] = window text palette, [3] = PP palette — B exports only 2 → I must add from ROM, or F adapts.
- Trainer back pngs: frames side by side, split by 64px columns. Trainer pic placement by .size.
- Merge conflict expected in crates/extract/tests/export.rs and export hook with Task A.

## From A (battle data extract)
- strings.ron: 437 keys (369 STRINGID_*, 53 sText_* code strings, 13 gText_* menu, gText_BirchInTrouble, gText_ConfirmStarterChoice). Stat names: sText_HP2, sText_Attack2, sText_Defense2, sText_Speed, sText_SpAtk2, sText_SpDef2, sText_Accuracy, sText_Evasiveness.
- Str mapping suggestion: Intro1→sText_WildPkmnAppeared, IntroTrainer→sText_Trainer1WantsToBattle, GoPkmn/Intro2→sText_GoPkmn, PkmnSentOut→sText_Trainer1SentOutPkmn, UsedMove→sText_AttackerUsedX + sText_ExclamationMark, menu→gText_*, ConfirmStarterChoice→gText_ConfirmStarterChoice, TypeName→types.ron names, StatName→above keys, else STRINGID_*.
- Heal location name HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F (value 1).
- TrainerMon.item Some("ITEM_NONE") for item layouts; MOVE_TARGET_BOTH→BothFoes; Struggle effect EFFECT_RECOIL; 412 species incl NONE; 355 moves. Exp tables not exported (engine computes).
- Encounters: Route101/102/103 land only.
- G: gText_BirchInTrouble available → Str::BirchInTrouble.

## From F review
- B/I: BattleWindows needs textbox palettes [2]=text.pal (gBattleWindowTextPalette), [3]=text_pp.pal and full 64-row tilemap (action view rows 34..40, move view 54..60). Add ROM-backed test once I lands. B currently exports only 2 palettes → fix in B or I.
- B/I: extract FONT_NARROW → GameData; pass to draw_text for move names (FR "Vive-Attaque" 70px > 64).
- D: BattleMon::moves must be packed from slot 0.
- video::layout::battle comment: yes/no uses palettes 5 and 1 (not 15) — update at merge along with E's OBJ layout.
- Merge order: F before G (G's label renderer can then switch to text::Canvas).

## From B review (decisions)
- TrainerPic.y = Gen3 .size; I fills from size; E (or K) computes (8 - y)*4 + 40/80 (E currently hardcodes 8 → switch in Wave 2).
- I builds starter_bag.tilemap = bag map padded with 0 to 1024 + grass map (1024). No starter_grass field.
- Healthbox nickname/gender strings gText_HealthboxNickname, gText_HealthboxGender_{None,Male,Female} — add to strings if E/K need them (I).
- "Lv" = small font glyph 0x105 (confirmed; E correct).

## From A review
- I: FR StatSharplyRose → STRINGID_STATSHARPLY alone (complete phrase); StatHarshlyFell → STRINGID_STATHARSHLY alone; US concatenates with STATROSE/STATFELL. Use A's helper.
- I: STAT strings exported under STRINGID_STAT* keys (not gText_/sText_ names).
- F: menu strings BattleYesNoChoice/BattleSwitchWhich/MoveInterfacePpType begin with {PALETTE 5}{COLOR_HIGHLIGHT_SHADOW DYNAMIC_COLOR4..6} (FC 05/FC 04); draw_text must honour or skip — check in K/I.
- I: B writes pokemon/<SPECIES> incl Unown forms; species.ron has 412 entries — not 1:1.
- Some FR strings untranslated in ROM (e.g. gText_WhichMoveToForget4) — expected.
