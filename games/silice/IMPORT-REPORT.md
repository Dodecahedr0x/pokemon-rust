# Silice import report

## Counts

- chapters: 13
- texts: 1923
- texts defined twice with different bodies: 2
- trainers: 238
- encounter maps: 39
- encounter tables: 56
- ITEM_SIL_ definitions: 69
- item placements: 260
- marts: 13
- referenced constants: 1053
- referenced text ids: 1075
- dynamic references: 37
- unresolved dynamic references: 4
- ids/flags: 544
- ids/vars: 21
- ids/trainers: 238
- ids/items: 69
- ids/species: 5
- ids/moves: 1
- ids/songs: 13
- ids/maps: 115

## Bold paragraphs naming items without a mart

- 00-prologue:1269: Objet inventé : `ITEM_SIL_MALLETTE` — objet clé, poche Objets rares. Utiliser : ouvre le menu Mallette (Analyse / Carte / Appels / Modules / Logs / Synchronisation). Base Gen3 à cloner : le PokéNav d'Emerald (menu à onglets, `FLAG_SYS_POKENAV_GET`) + le Pokédex (`FLAG_SYS_POKEDEX_GET`). Voir §9.
- 01-bois-ether:1598: Objets inventés - `ITEM_SIL_EVOLUROC` — Évoluroc : objet tenu ; Défense et Défense Spéciale ×1,5 si le Pokémon peut encore évoluer (effet de l'Évoluroc de 5G, absent de Gen3). Base Gen3 à cloner : `ITEM_METAL_POWDER` (objet tenu qui modifie une stat en combat), en remplaçant le test « est Métamorph » par « a une évolution ». Voir §9. - `ITEM_SIL_POUSSIERE_SILICE` — Poussière de Silice : objet clé, aucun effet. Description : « Poussière grise scintillante extraite des herbes de Bois-Éther. Ne pas respirer. » Base : `ITEM_SOOT_SACK` (objet clé sans effet). Pourra servir de preuve dans une quête plus tard (voir §10).
- 02-gris-echec:2002: Objet inventé : `ITEM_SIL_CARTE_PERFOREE` — Carte Perforée d'Authentification, objet clé. « Utiliser » affiche `Carte_Perforee_Lire` (le code ● ○ ● ● ○ ○ ● ○). Base Gen3 : un objet clé lisible sans usage terrain, `ITEM_LETTER` (ou `ITEM_DEVON_SCOPE`). Voir §9.
- 03-viber-city:2676: Objets inventés : - Pierre Lien (`ITEM_SIL_PIERRE_LIEN`, D-30) : pierre d'évolution consommée ; fait évoluer tout POKéMON qui évolue par échange (Kadabra, Machopeur, Gravalanch, Spectrum ; avec l'objet tenu requis, consommé aussi : Porygon + Améliorator, etc.). « Une pierre où dorment deux fréquences. » Base à cloner : `ITEM_MOON_STONE`. Utile tout de suite : le KADABRA du Jardin des Néons et le PORYGON du Casino (avec son Améliorator). - Brochette Néon (`ITEM_SIL_BROCHETTE_NEON`) : soigne 60 PV d'un POKéMON. « Une brochette qui brille dans le noir. Personne ne sait pourquoi. » Base à cloner : `ITEM_SODA_POP` (soigne 60 PV). Prix : 300 ₽ (vendue au stand de MATHIEU, voir dessous).
- 06-manga-square:2389: Objets inventés : - DRACAUFITE X (`ITEM_SIL_CHARIZARDITE_X`) — Méga-Gemme, objet tenu (Dracaufeu → Méga-Dracaufeu X, Feu/Dragon). Tenue par le DRACAUFEU de TAK, non obtenable par le joueur ici. Base à cloner : un objet tenu sans effet (ex. `ITEM_SOFT_SAND` sans bonus), plus le déclencheur de Méga. - DRACAUFITE Y (`ITEM_SIL_CHARIZARDITE_Y`) — Méga-Gemme (Dracaufeu → Méga-Dracaufeu Y, Feu/Vol, talent Sécheresse). Donnée par TAK. Dracaufeu est disponible : Salamèche dans le Bois des Soupirs (10 %), Dracaufeu sauvage (1 %). Description : « Une gemme qui permet à DRACAUFEU de méga-évoluer. »
- 07-reverse-city:1860: Objets inventés : - POUDRE SILICE (`ITEM_SIL_POUDRE_SILICE`) — objet de combat (à utiliser depuis le Sac, comme un « X »). Effet : Attaque Spéciale +2 crans, mais le POKéMON est empoisonné (s'il ne l'est pas déjà ; pas d'effet de poison sur les types Poison et Acier). Description : « Résidu de silice raffiné. Booste l'ATQ. SPÉ., mais empoisonne. ». Base à cloner : `ITEM_X_SPECIAL` (+ un effet de poison). Prix de revente : 300. Ce n'est pas la Poudre Claire (la drogue de Gogo & Romain) : c'est sa version « dopage de POKéMON ». - MULTI EXP V2 (`ITEM_SIL_MULTI_EXP_V2`) — objet clé, activable ou non depuis le Sac. Activé : tous les POKéMON de l'équipe (non K.O.) qui n'ont pas combattu reçoivent 50 % de l'expérience gagnée ; ceux qui ont combattu la gagnent en entier. Base à cloner : `ITEM_EXP_SHARE` (mais en objet clé, appliqué à toute l'équipe). Repli : donner un `ITEM_EXP_SHARE` normal (objet tenu) nommé « MULTI EXP V2 ». - ALAKAZAMITE (`ITEM_SIL_ALAKAZITE`) — Méga-Gemme (Alakazam → Méga-Alakazam, talent Calque), utilisable grâce au Module Méga-Liaison (Ch6). Cachée là où Reverse se tenait, dans les pixels noirs : « le reflet a laissé quelque chose ». Kadabra sauvage sur la Route S7 (4 %), Alakazam dans le Bois des Soupirs (Ch6). Base à cloner : comme la Dracaufite (objet tenu + déclencheur de Méga). - (Rappel) MASTER BALL : objet Gen3 standard ; ici la seule du jeu (pas d'autre exemplaire à placer ailleurs).
- 09-nebula:1419: Distributeur (Centre Pokémon) : ITEM_FRESH_WATER, ITEM_SODA_POP, ITEM_LEMONADE (« saveurs certifiées »).
- 10-tour-piliers:2438: Objets inventés (aussi listés en §9) : - Accès Alpha / Bêta / Gamma (`ITEM_SIL_ACCES_ALPHA`, `_BETA`, `_GAMMA`) : objets clés sans effet d'utilisation (« Une carte de silice gravée. Elle ouvre un étage de la TOUR. »). Base Gen3 à cloner : `ITEM_STORAGE_KEY`. - Boussole d'Arthur (`ITEM_SIL_BOUSSOLE_ARTHUR`) : objet clé, retiré quand on la rend. Base : `ITEM_STORAGE_KEY`. - Méga-Gemmes des Piliers (`ITEM_SIL_STEELIXITE`, `ITEM_SIL_GYARADOSITE`) : tenues par Steelix (Kimiko) et Léviator (Arthur), non obtenables ici. Définies par D-02 ; leur distribution au joueur relève des autres chapitres. - Bateau en origami (`ITEM_SIL_BATEAU_ORIGAMI`) : objet clé souvenir ; à rendre à Arthur en post-game quand il prend la mer (piste pour le Ch.12). Base : `ITEM_STORAGE_KEY`.
- 12-post-game:4785: Boutique de Nébula (MAP_NEBULA_MART), à partir de 120 : Hyper Ball (`ITEM_ULTRA_BALL`), Chrono Ball (`ITEM_TIMER_BALL`), Filet Ball (`ITEM_NET_BALL`), Guérison (`ITEM_FULL_RESTORE`), Potion Max (`ITEM_MAX_POTION`), Total Soin (`ITEM_FULL_HEAL`), Rappel (`ITEM_REVIVE`), Max Repousse (`ITEM_MAX_REPEL`), Antidote (`ITEM_ANTIDOTE`). À partir de 130, s'ajoute la Pierre Lien (`ITEM_SIL_PIERRE_LIEN`, D-30).
- 12-post-game:4790: Ravitaillement de Mathieu (tente de la Brèche), à partir de 127 : Antidote, Total Soin, Hyper Potion (`ITEM_HYPER_POTION`), Hyper Ball, Max Repousse, Pierre Lien (`ITEM_SIL_PIERRE_LIEN`, D-30). Le vendeur est un GARDE du Conseil :

## Duplicate text ids

- Tour_Sbire1_Intro: a different text in 09-nebula:1223; both are written (rule text-duplicate)
- Tour_Sbire2_Intro: a different text in 09-nebula:1235; both are written (rule text-duplicate)

## Dynamic references

- `<Champion>_PG_Revanche_Defaite` -> Loic_PG_Revanche_Defaite, Reverse_PG_Revanche_Defaite, Samuel_PG_Revanche_Defaite, Skycker_PG_Revanche_Defaite, Tak_PG_Revanche_Defaite, Yohan_PG_Revanche_Defaite
- `<Champion>_PG_Revanche_Deja` (12-post-game): unresolved
- `<Champion>_PG_Revanche_Intro` -> Loic_PG_Revanche_Intro, Reverse_PG_Revanche_Intro, Samuel_PG_Revanche_Intro, Skycker_PG_Revanche_Intro, Tak_PG_Revanche_Intro, Yohan_PG_Revanche_Intro
- `Bar_Sc02_T1 | T2 | T3` -> Bar_Sc02_T1, Bar_Sc02_T2, Bar_Sc02_T3
- `Benou_Lab_PG_Sc24_L<n>` -> Benou_Lab_PG_Sc24_L1, Benou_Lab_PG_Sc24_L2, Benou_Lab_PG_Sc24_L3, Benou_Lab_PG_Sc24_L4, Benou_Lab_PG_Sc24_L5, Benou_Lab_PG_Sc24_L6, Benou_Lab_PG_Sc24_L7, Benou_Lab_PG_Sc24_L8, Benou_Lab_PG_Sc24_L9
- `Evalon_Sc07_Others_T[choice]` -> Evalon_Sc07_Others_T0, Evalon_Sc07_Others_T1, Evalon_Sc07_Others_T2
- `Evalon_Sc07_Partner_T[choice]` -> Evalon_Sc07_Partner_T0, Evalon_Sc07_Partner_T1, Evalon_Sc07_Partner_T2
- `FLAG_SIL_ECHANTILLON_<n>` -> FLAG_SIL_ECHANTILLON_1, FLAG_SIL_ECHANTILLON_2, FLAG_SIL_ECHANTILLON_3
- `FLAG_SIL_FRAGMENT_<n>` -> FLAG_SIL_FRAGMENT_01, FLAG_SIL_FRAGMENT_02, FLAG_SIL_FRAGMENT_03, FLAG_SIL_FRAGMENT_04, FLAG_SIL_FRAGMENT_05, FLAG_SIL_FRAGMENT_06, FLAG_SIL_FRAGMENT_07, FLAG_SIL_FRAGMENT_08
- `FLAG_SIL_GRIS_JAMMER_n` -> FLAG_SIL_GRIS_JAMMER_1, FLAG_SIL_GRIS_JAMMER_2, FLAG_SIL_GRIS_JAMMER_3
- `FLAG_SIL_GYM06_PORTE_<n>` -> FLAG_SIL_GYM06_PORTE_1, FLAG_SIL_GYM06_PORTE_2, FLAG_SIL_GYM06_PORTE_3, FLAG_SIL_GYM06_PORTE_4, FLAG_SIL_GYM06_PORTE_5
- `FLAG_SIL_HIDE_<ESPÈCE>` -> FLAG_SIL_HIDE_ARTICUNO, FLAG_SIL_HIDE_CELEBI, FLAG_SIL_HIDE_DEOXYS, FLAG_SIL_HIDE_ENTEI, FLAG_SIL_HIDE_GROUDON, FLAG_SIL_HIDE_JIRACHI, FLAG_SIL_HIDE_KYOGRE, FLAG_SIL_HIDE_LATIAS, FLAG_SIL_HIDE_LATIOS, FLAG_SIL_HIDE_LUGIA, FLAG_SIL_HIDE_MEW, FLAG_SIL_HIDE_MEWTWO, FLAG_SIL_HIDE_MOLTRES, FLAG_SIL_HIDE_RAIKOU, FLAG_SIL_HIDE_RAYQUAZA, FLAG_SIL_HIDE_REGICE, FLAG_SIL_HIDE_REGIROCK, FLAG_SIL_HIDE_REGISTEEL, FLAG_SIL_HIDE_SUICUNE, FLAG_SIL_HIDE_ZAPDOS
- `FLAG_SIL_HIDE_<NOM>` -> FLAG_SIL_HIDE_ARTICUNO, FLAG_SIL_HIDE_CELEBI, FLAG_SIL_HIDE_DEOXYS, FLAG_SIL_HIDE_ENTEI, FLAG_SIL_HIDE_GROUDON, FLAG_SIL_HIDE_JIRACHI, FLAG_SIL_HIDE_KYOGRE, FLAG_SIL_HIDE_LATIAS, FLAG_SIL_HIDE_LATIOS, FLAG_SIL_HIDE_LUGIA, FLAG_SIL_HIDE_MEW, FLAG_SIL_HIDE_MEWTWO, FLAG_SIL_HIDE_MOLTRES, FLAG_SIL_HIDE_RAIKOU, FLAG_SIL_HIDE_RAYQUAZA, FLAG_SIL_HIDE_REGICE, FLAG_SIL_HIDE_REGIROCK, FLAG_SIL_HIDE_REGISTEEL, FLAG_SIL_HIDE_SUICUNE, FLAG_SIL_HIDE_ZAPDOS
- `FLAG_SIL_HIDE_GAZ_<n>` -> FLAG_SIL_HIDE_GAZ_01, FLAG_SIL_HIDE_GAZ_02, FLAG_SIL_HIDE_GAZ_03, FLAG_SIL_HIDE_GAZ_04, FLAG_SIL_HIDE_GAZ_05, FLAG_SIL_HIDE_GAZ_06, FLAG_SIL_HIDE_GAZ_07, FLAG_SIL_HIDE_GAZ_08
- `FLAG_SIL_ITEM_PG_GEM_LOIC … _LOULPIX` (12-post-game): unresolved
- `FLAG_SIL_OWLYS_SILO_NOCTALI_n` -> FLAG_SIL_OWLYS_SILO_NOCTALI_1, FLAG_SIL_OWLYS_SILO_NOCTALI_2, FLAG_SIL_OWLYS_SILO_NOCTALI_3
- `FLAG_SIL_OWLYS_VEILLEUR_n` -> FLAG_SIL_OWLYS_VEILLEUR_1, FLAG_SIL_OWLYS_VEILLEUR_2, FLAG_SIL_OWLYS_VEILLEUR_3, FLAG_SIL_OWLYS_VEILLEUR_4
- `FLAG_SIL_REVERSE_EMETTEUR_<n>` -> FLAG_SIL_REVERSE_EMETTEUR_1, FLAG_SIL_REVERSE_EMETTEUR_2, FLAG_SIL_REVERSE_EMETTEUR_3
- `Gym2_<P>_Defeat` -> Gym2_Cavalier_Defeat, Gym2_Fou_Defeat, Gym2_Melomane_Defeat, Gym2_Pion1_Defeat, Gym2_Pion2_Defeat, Gym2_Robot_Defeat, Gym2_Sc10_Defeat, Gym2_Tour_Defeat
- `Gym2_<P>_Intro` -> Gym2_Cavalier_Intro, Gym2_Fou_Intro, Gym2_Melomane_Intro, Gym2_Pion1_Intro, Gym2_Pion2_Intro, Gym2_Robot_Intro, Gym2_Tour_Intro
- `Gym_Sc01_D_<NOM>` -> Gym_Sc01_D_KOJIRO, Gym_Sc01_D_MUSASHI
- `Gym_Sc01_T1_<NOM>` -> Gym_Sc01_T1_KOJIRO, Gym_Sc01_T1_MUSASHI
- `Gym_Sc02_Q<n>` -> Gym_Sc02_Q1, Gym_Sc02_Q2, Gym_Sc02_Q3, Gym_Sc02_Q4, Gym_Sc02_Q5
- `Gym_Sc02_Q<n>_Choix` (06-manga-square): unresolved
- `ITEM_SIL_<NOM>ITE` -> ITEM_SIL_ABSOLITE, ITEM_SIL_AERODACTYLITE, ITEM_SIL_AGGRONITE, ITEM_SIL_ALAKAZITE, ITEM_SIL_ALTARIANITE, ITEM_SIL_AMPHAROSITE, ITEM_SIL_BANETTITE, ITEM_SIL_BLAZIKENITE, ITEM_SIL_GARDEVOIRITE, ITEM_SIL_GENGARITE, ITEM_SIL_GLALITITE, ITEM_SIL_GYARADOSITE, ITEM_SIL_HERACRONITE, ITEM_SIL_HOUNDOOMINITE, ITEM_SIL_KANGASKHANITE, ITEM_SIL_LATIOSITE, ITEM_SIL_MANECTITE, ITEM_SIL_MAWILITE, ITEM_SIL_MEDICHAMITE, ITEM_SIL_METAGROSSITE, ITEM_SIL_PINSIRITE, ITEM_SIL_SABLENITE, ITEM_SIL_SALAMENCITE, ITEM_SIL_SCEPTILITE, ITEM_SIL_SCIZORITE, ITEM_SIL_STEELIXITE, ITEM_SIL_SWAMPERTITE, ITEM_SIL_TYRANITARITE, ITEM_SIL_VENUSAURITE
- `MAP_ROUTE_S0x_ANTENNE` -> MAP_ROUTE_S02_ANTENNE, MAP_ROUTE_S03_ANTENNE, MAP_ROUTE_S07_ANTENNE
- `No_Mans_Land_PG_Sc17_T<n>` -> No_Mans_Land_PG_Sc17_T1, No_Mans_Land_PG_Sc17_T2, No_Mans_Land_PG_Sc17_T3
- `Nocturne_Sc01_T[piste]` -> Nocturne_Sc01_T0, Nocturne_Sc01_T1, Nocturne_Sc01_T2
- `PG_Antenne_T1_<règle>` -> PG_Antenne_T1_MonoAcier, PG_Antenne_T1_SansObjet, PG_Antenne_T1_Trio
- `PG_Legendaire_<nom>` -> PG_Legendaire_Artikodin, PG_Legendaire_Celebi, PG_Legendaire_Electhor, PG_Legendaire_Entei, PG_Legendaire_Groudon, PG_Legendaire_Kyogre, PG_Legendaire_Latias, PG_Legendaire_Latios, PG_Legendaire_Lugia, PG_Legendaire_Raikou, PG_Legendaire_Regice, PG_Legendaire_Regirock, PG_Legendaire_Registeel, PG_Legendaire_Suicune, PG_Legendaire_Sulfura
- `SPECIES_<NOM>` (12-post-game): unresolved
- `TRAINER_SIL_ECHEC_<P>` -> TRAINER_SIL_ECHEC_CAVALIER, TRAINER_SIL_ECHEC_FOU, TRAINER_SIL_ECHEC_TOUR
- `TRAINER_SIL_EMETTEUR_<n>` -> TRAINER_SIL_EMETTEUR_1, TRAINER_SIL_EMETTEUR_2, TRAINER_SIL_EMETTEUR_3
- `TRAINER_SIL_SAMOURAI_<NOM>` -> TRAINER_SIL_SAMOURAI_KOJIRO, TRAINER_SIL_SAMOURAI_MUSASHI
- `TRAINER_SIL_SAMOURAI_<NOM>_HATIF` -> TRAINER_SIL_SAMOURAI_KOJIRO_HATIF, TRAINER_SIL_SAMOURAI_MUSASHI_HATIF
- `Tour_2F_Chat_NN` -> Tour_2F_Chat_01, Tour_2F_Chat_02, Tour_2F_Chat_03, Tour_2F_Chat_04, Tour_2F_Chat_05, Tour_2F_Chat_06, Tour_2F_Chat_07, Tour_2F_Chat_08, Tour_2F_Chat_09, Tour_2F_Chat_10, Tour_2F_Chat_11, Tour_2F_Chat_12, Tour_2F_Chat_Off
- `VAR_SIL_TOUR_PRISME_N` -> VAR_SIL_TOUR_PRISME_1, VAR_SIL_TOUR_PRISME_2, VAR_SIL_TOUR_PRISME_3

## Elided references

- 05-owlys:602: `..._Repli`
- 06-manga-square:1018: `…`

## Id bases

- ids/flags.ron starts at 0x113, above the Gen3 numbers the data reuses

## Ids no longer in the source (kept)

- ids/flags.ron: FLAG_SIL_HIDE_BAR_SBIRES is a tombstone but the source uses it

## Item definitions

- 10-tour-piliers:2442: a definition paragraph naming 2 items, not used
- name conflict: ITEM_SIL_ACCES_ALPHA: "Accès Alpha" kept, "Accès Alpha / Bêta / Gamma" at 10-tour-piliers:2439

## Item placements

- 12-post-game:4736: flag cell "FLAG_SIL_ECHANTILLON_1/2/3" is not one flag; left unset
- 12-post-game:4744: 2 items for the name "Mewtwoïte X et Y": one placement each, same name
- 12-post-game:4745: map cell "arènes" is not a MAP_ constant
- 12-post-game:4745: flag cell "FLAG_SIL_ITEM_PG_GEM_LOIC … _LOULPIX" is not one flag; left unset

## Items left open

- ITEM_SIL_ABSOLITE: no name, no pocket, no base item
- ITEM_SIL_ACCES_BETA: no base item
- ITEM_SIL_ACCES_GAMMA: no base item
- ITEM_SIL_AERODACTYLITE: no pocket, no base item
- ITEM_SIL_AGGRONITE: no pocket, no base item
- ITEM_SIL_ALAKAZITE: no pocket, no base item
- ITEM_SIL_ALTARIANITE: no pocket, no base item
- ITEM_SIL_AMPHAROSITE: no name, no pocket, no base item
- ITEM_SIL_BANETTITE: no name, no pocket, no base item
- ITEM_SIL_BLAZIKENITE: no name, no pocket, no base item
- ITEM_SIL_BOUTEILLE_PUNCH: no pocket
- ITEM_SIL_BROCHETTE_NEON: no pocket
- ITEM_SIL_CARTE_PERFOREE: no base item
- ITEM_SIL_CHARIZARDITE_X: no pocket, no base item
- ITEM_SIL_CHARIZARDITE_Y: no pocket, no base item
- ITEM_SIL_EPEE_LEGENDE: no pocket
- ITEM_SIL_EVOLUROC: no pocket
- ITEM_SIL_GARDEVOIRITE: no name, no pocket, no base item
- ITEM_SIL_GENGARITE: no name, no pocket, no base item
- ITEM_SIL_GLALITITE: no pocket, no base item
- ITEM_SIL_GYARADOSITE: no name, no pocket, no base item
- ITEM_SIL_HERACRONITE: no pocket, no base item
- ITEM_SIL_HOUNDOOMINITE: no pocket, no base item
- ITEM_SIL_KANGASKHANITE: no pocket, no base item
- ITEM_SIL_LARME_FEE: no pocket
- ITEM_SIL_LATIOSITE: no pocket, no base item
- ITEM_SIL_MALLETTE: no base item
- ITEM_SIL_MANECTITE: no pocket, no base item
- ITEM_SIL_MAWILITE: no name, no pocket, no base item
- ITEM_SIL_MEDICHAMITE: no pocket, no base item
- ITEM_SIL_METAGROSSITE: no name, no pocket, no base item
- ITEM_SIL_MEWTWONITE_X: no pocket, no base item
- ITEM_SIL_MEWTWONITE_Y: no pocket, no base item
- ITEM_SIL_MILLESIME_SAM: no pocket
- ITEM_SIL_ORBE_BLEU: no pocket, no base item
- ITEM_SIL_ORBE_EVEIL: no pocket
- ITEM_SIL_ORBE_ROUGE: no pocket, no base item
- ITEM_SIL_PAIN_LEMBAS: no pocket
- ITEM_SIL_PIERRE_LIEN: no pocket
- ITEM_SIL_PINSIRITE: no pocket, no base item
- ITEM_SIL_PLUME_OMBREE: no base item
- ITEM_SIL_PLUME_OR: no pocket
- ITEM_SIL_POUDRE_CLAIRE: no pocket
- ITEM_SIL_POUDRE_SILICE: no pocket
- ITEM_SIL_RHUM_PIRATE: no pocket
- ITEM_SIL_SABLENITE: no pocket, no base item
- ITEM_SIL_SALAMENCITE: no pocket, no base item
- ITEM_SIL_SCEPTILITE: no pocket, no base item
- ITEM_SIL_SCIZORITE: no pocket, no base item
- ITEM_SIL_SLOWBRONITE: no name, no pocket, no base item
- ITEM_SIL_STEELIXITE: no pocket, no base item
- ITEM_SIL_SWAMPERTITE: no pocket, no base item
- ITEM_SIL_TYRANITARITE: no pocket, no base item
- ITEM_SIL_VENUSAURITE: no name, no pocket, no base item
- ITEM_SIL_VIEUX_PATCH: no base item

## Map families expanded

- 00-prologue:229: MAP_BRICO_BOURG_PLAYER_HOUSE_2F
- 02-gris-echec:291: MAP_GRIS_ECHEC_MART
- 02-gris-echec:312: MAP_GRIS_ECHEC_TOUR_OBSERVATION_TOP
- 08-evalon:359: MAP_EVALON_TOUR_SCRIBE_2F
- 08-evalon:359: MAP_EVALON_TOUR_SCRIBE_3F
- 08-evalon:389: MAP_EVALON_CITADELLE_2F
- 08-evalon:1450: MAP_EVALON_CITADELLE_2F

## Mart rows not imported

- 01-bois-ether:1610: MAP_BOIS_ETHER_MART | Poké Ball (200), Potion (300), Antidote (100), Anti-Para (200), Réveil (250), Repousse (350) ; + Super Potion (700) après `FLAG_SIL_BADGE_01` (stock in prose, enter by hand)
- 01-bois-ether:1611: MAP_BOIS_ETHER_HERBORISTE (`MART_HERBORISTE`) | Poudre Soin `ITEM_HEAL_POWDER` (450), Poudre Énergie `ITEM_ENERGY_POWDER` (500), Racine Énergie `ITEM_ENERGY_ROOT` (800) ; + Herbe Rappel `ITEM_REVIVAL_HERB` (2800) après le badge (stock in prose, enter by hand)
- 02-gris-echec:2010: MAP_GRIS_ECHEC_MART | Poké Ball (200), Super Ball (600), Potion (300), Super Potion (700), Antidote (100), Anti-Para (200), Réveil (250), Repousse (350), Super Repousse (500) (stock in prose, enter by hand)
- 02-gris-echec:2011: Distributeur (MAP_GRIS_ECHEC_MART, au fond) | Eau Fraîche `ITEM_FRESH_WATER` (200), Soda Cool `ITEM_SODA_POP` (300), Limonade `ITEM_LEMONADE` (350) (stock in prose, enter by hand)

## Names only the maps use (allocated)

- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_TECHNICIEN
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_GARDE
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_ADRIEN
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_GOGO
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_ROMAIN
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_1
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_2
- maps/BoisEther.ron: FLAG_SIL_HIDE_BOIS_ETHER_MATHIEU
- maps/BoisEther_Gym.ron: FLAG_SIL_HIDE_GYM1_LOIC
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_LINGE
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_GAMIN
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_BRICOLEUSE
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_SBIRE
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_MATHIEU
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_GWADA
- maps/BricoBourg.ron: FLAG_SIL_HIDE_BRICO_ADRIEN
- maps/BricoBourg.ron: VAR_SIL_CH0_GATE
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_MATHIEU
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_GWADA
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_ADRIEN
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_JULIEN
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_SBIRE_A
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_MALLETTE
- maps/BricoBourg_Bar.ron: FLAG_SIL_HIDE_BAR_SBIRE_B
- maps/BricoBourg_Labo.ron: FLAG_SIL_HIDE_LABO_CHIMISTE
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_ADRIEN
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_GWADA
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_SCRIBE
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_ROMAIN
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_SBIRE_1
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_SBIRE_2
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_SBIRE_3
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_PIGEON
- maps/Evalon.ron: FLAG_SIL_HIDE_EVALON_GARDE_SE
- maps/Evalon_Citadelle.ron: FLAG_SIL_HIDE_GYM8_COFFRE_GAUCHE
- maps/Evalon_Galeries.ron: FLAG_SIL_ITEM_EVALON_MINERAI
- maps/Evalon_Taverne.ron: FLAG_SIL_HIDE_TAVERNE_MATHIEU
- maps/Evalon_Taverne.ron: FLAG_SIL_HIDE_TAVERNE_ADRIEN
- maps/Evalon_Taverne.ron: FLAG_SIL_HIDE_TAVERNE_GWADA
- maps/GrisEchec.ron: FLAG_SIL_HIDE_GRIS_GARDE
- maps/GrisEchec.ron: FLAG_SIL_HIDE_GRIS_GWADA
- maps/GrisEchec.ron: FLAG_SIL_HIDE_GRIS_GARDE_S3
- maps/GrisEchec.ron: FLAG_SIL_HIDE_GRIS_LAINERGIE
- maps/GrisEchec.ron: FLAG_SIL_HIDE_GRIS_GARDE_ECARTE
- maps/GrisEchec_Gym.ron: FLAG_SIL_HIDE_GYM2_ROBOT
- maps/GrisEchec_Gym.ron: FLAG_SIL_HIDE_GYM2_GWADA
- maps/GrisEchec_Gym.ron: FLAG_SIL_HIDE_GYM2_WATTOUAT
- maps/GrisEchec_Gym.ron: FLAG_SIL_HIDE_GYM2_LAINERGIE
- maps/RouteS02.ron: FLAG_SIL_HIDE_ROUTE_S02_ADRIEN
- maps/RouteS02.ron: FLAG_SIL_HIDE_ROUTE_S02_GWADA
- maps/RouteS03_Depot.ron: FLAG_SIL_HIDE_DEPOT_ROMAIN
- maps/RouteS03_Depot.ron: FLAG_SIL_HIDE_DEPOT_SBIRE
- maps/RouteS08.ron: FLAG_SIL_HIDE_S08_PIGEON
- maps/ViberCity.ron: FLAG_SIL_HIDE_VIBER_MATHIEU
- maps/ViberCity.ron: FLAG_SIL_HIDE_NOCTURNE_DANSEUR_2
- maps/ViberCity.ron: FLAG_SIL_HIDE_NOCTURNE_DANSEUR_3
- maps/ViberCity.ron: FLAG_SIL_HIDE_NOCTURNE_DANSEUR_4
- maps/ViberCity.ron: FLAG_SIL_HIDE_VIBER_TAUBER
- maps/ViberCity.ron: FLAG_SIL_HIDE_VIBER_MATHIEU_CHARIOT
- maps/ViberCity_Casino.ron: FLAG_SIL_HIDE_CASINO_GOGO
- maps/ViberCity_Casino.ron: FLAG_SIL_HIDE_CASINO_ROMAIN
- maps/ViberCity_Casino_B1F.ron: FLAG_SIL_HIDE_CASINO_B1F_ROMAIN
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ01
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ02
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ03
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ04
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ05
- maps/ViberCity_PokemonCenter.ron: FLAG_SIL_HIDE_VIBER_PC_PNJ06
- maps/ViberCity_StationLumiere.ron: FLAG_SIL_HIDE_STATION_SBIRE_2

## §7 tables not imported

- 03-viber-city:2737: POKéMON | `SPECIES_*` | Niveau | Jetons | Argument de vente
- 08-evalon:2380: Rang | Objets
- 09-nebula:1415: Objets
