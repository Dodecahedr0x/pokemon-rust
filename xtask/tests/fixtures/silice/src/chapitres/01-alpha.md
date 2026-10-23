# Chapter 1 - Alpha (synthetic fixture)

## 1. Intent

A synthetic chapter in the shape of the Silice chapters: numbered sections, map headings,
text blocks, pseudo-scripts, RON trainers and encounters, item tables.

## 2. Maps

### MAP_ALPHA_TOWN - the town

### MAP_ALPHA_TOWN_HOUSE_1F / _2F - a house on two floors

### MAP_ALPHA_ROUTE

## 3. Scenes

### Alpha_Sc01 - grouped texts

```text
[Alpha_Sc01_T1] HERO (thinking, first visit)
First line of the text.\n
Second line here.

[Alpha_Sc01_T2]
Short one.\p
A new box.

[Alpha_Sc01_T3] RIVAL
Joined
without escape.
```

    ```text
    [Alpha_Sign_1]
    Sign number one.
    ```

```text
[Alpha_Sign_2] (an indented note (with parens))
Sign number two.
```

```text
lock
msgbox(Alpha_Sc01_T1)
if !flag(FLAG_SIL_ALPHA_DONE):
    msgbox(Alpha_Sc01_T2)
    setflag(FLAG_SIL_ALPHA_STEP_1…3)
    setvar(VAR_SIL_ALPHA_STATE, 1)
    playse(SE_SIL_BELL)
    playbgm(MUS_SIL_ALPHA_THEME)
msgbox(Alpha_Sign_<n>)
msgbox(Alpha_Sc01_T3)
battle_trainer(TRAINER_SIL_ALPHA_ONE)
warp(MAP_ALPHA_TOWN_HOUSE_2F)
release
```

The house uses `FLAG_SIL_ALPHA_LAMP_RED` / `_BLUE` for its lamps.

## 4. Gym

### MAP_ALPHA_GYM

## 5. Trainers

```ron
(
    id: "TRAINER_SIL_ALPHA_ONE",
    class: "TRAINER_CLASS_HIKER",
    class_name: "HIKER",                    // a placeholder class
    name: "ONE",
    pic: "TRAINER_PIC_HIKER",
    female: false,
    ai: ["AI_SCRIPT_CHECK_BAD_MOVE"],
    double: false,
    party: [
        (iv: 0, level: 5, species: "SPECIES_ZIGZAGOON", item: Some("ITEM_ORAN_BERRY"), moves: None),
        (iv: 0, level: 6, species: "SPECIES_POOCHYENA", item: None, moves: Some(["MOVE_TACKLE", "MOVE_GROWL", "MOVE_NONE", "MOVE_NONE"])),
    ],
    items: ["ITEM_POTION"],
),
```

## 6. Wild

```ron
"MAP_ALPHA_ROUTE": (
    land: Some((rate: 20, mons: [
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),   // slot comment
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),
        (species: "SPECIES_ZIGZAGOON", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
        (species: "SPECIES_POOCHYENA", min_level: 2, max_level: 3),
    ])),
    water: None,
    rock_smash: None,
    fishing: Some((rate: 30, mons: [
        (species: "SPECIES_MAGIKARP", min_level: 5, max_level: 10),
        (species: "SPECIES_TENTACOOL", min_level: 5, max_level: 10),
        (species: "SPECIES_MAGIKARP", min_level: 10, max_level: 30),
        (species: "SPECIES_WAILMER", min_level: 10, max_level: 30),
        (species: "SPECIES_WAILMER", min_level: 10, max_level: 30),
        (species: "SPECIES_WAILMER", min_level: 20, max_level: 45),
        (species: "SPECIES_WAILMER", min_level: 20, max_level: 45),
        (species: "SPECIES_WAILMER", min_level: 20, max_level: 45),
        (species: "SPECIES_WAILMER", min_level: 20, max_level: 45),
        (species: "SPECIES_WAILMER", min_level: 20, max_level: 45),
    ])),
),
```

## 7. Items

| Flag | Carte | Où | Objet (FR) | `ITEM_*` | Méthode | Note |
|---|---|---|---|---|---|---|
| FLAG_SIL_ITEM_ALPHA_POTION | MAP_ALPHA_TOWN | Behind the house (4,7) | Potion x2 | ITEM_POTION | Ball | |
| FLAG_SIL_ITEM_ALPHA_RODS | MAP_ALPHA_TOWN | A fisher | Rod A + Rod B + Rod C | ITEM_OLD_ROD, ITEM_GOOD_ROD, ITEM_SUPER_ROD | Ball | |
| FLAG_SIL_ITEM_ALPHA_CHARM | MAP_ALPHA_GYM | Gym leader | Lucky Charm | ITEM_SIL_LUCKY_CHARM | Ball | |

| `ITEM_*` | Nom | Type | Effet | Objet Gen3 à cloner |
|---|---|---|---|---|
| `ITEM_SIL_LUCKY_CHARM` | Lucky Charm | Key | No effect | `ITEM_SOOT_SACK` |

**Alpha shop**

| `ITEM_*` | Prix |
|---|---|
| `ITEM_POTION` | 300 |
| `ITEM_SIL_LUCKY_CHARM` | 1 000 (late) |
