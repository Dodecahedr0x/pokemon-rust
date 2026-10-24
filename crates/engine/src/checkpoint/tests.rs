//! Codec, schema policy and build identity. Every value here is synthetic.

use super::*;
use crate::data::{Dirs, Floor, PLAYER};
use crate::mon::{NAME_LEN, Status};
use crate::{data::MoveId, data::SpeciesId};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../../build_support.rs"]
mod build_support;

mod worst_case;

const TOWN: MapId = MapId(3);

fn object(local_id: u8, pos: (i16, i16), action: Option<ActionSnap>) -> ObjectSnap {
    ObjectSnap {
        local_id,
        map: TOWN,
        gfx: if local_id == PLAYER {
            None
        } else {
            Some(GfxId(7))
        },
        pos,
        prev: pos,
        elevation: 3,
        facing: Dir::Down,
        movement: MovementType::Wander,
        home: pos,
        range: (1, 2),
        visible: true,
        frozen: false,
        facing_locked: false,
        action,
        idle: IdleSnap::Wait(17),
        anim: AnimSnap {
            table: AnimTableSnap::Face,
            cmd: 0,
            timer: 0,
        },
    }
}

fn mon() -> Pokemon {
    // A synthetic nickname (Gen3 charmap "ZZTOP" + EOS padding), not a game name.
    let mut nickname = [0xFF; NAME_LEN];
    nickname[..5].copy_from_slice(&[0xD4, 0xD4, 0xCE, 0xC9, 0xCA]);
    Pokemon {
        species: SpeciesId(258),
        personality: 0x1234_5678,
        ot_id: 0x0BAD_F00D,
        nickname,
        level: 5,
        exp: 135,
        ivs: [1, 2, 3, 4, 5, 6],
        evs: [0; 6],
        moves: [Some(MoveId(33)), Some(MoveId(45)), None, None],
        pp: [35, 40, 0, 0],
        pp_bonuses: 0,
        friendship: 70,
        ability_slot: 0,
        status: Status::Sleep(2),
        hp: 19,
        stats: [20, 12, 11, 10, 9, 8],
        held: None,
    }
}

/// 2 objects (the player mid-walk), 3 flags, 2 vars, 1 override, 1 template delta, 1 party mon.
fn sample() -> Checkpoint {
    let mut player = object(
        PLAYER,
        (5, 6),
        Some(ActionSnap {
            kind: ActionKindSnap::Walk(Speed::Normal),
            dir: Dir::Up,
            t: 4,
            off: (0, -4),
            jump_y: 0,
        }),
    );
    player.prev = (5, 7);
    player.movement = MovementType::Face(Dir::Down);
    player.anim.table = AnimTableSnap::Go(1);
    player.anim.cmd = 1;
    player.anim.timer = 3;
    player.idle = IdleSnap::Done;
    let world = WorldSnapshot {
        frame: 1234,
        rng: 0xDEAD_BEEF,
        input: InputSnap {
            held: Buttons(0x40),
            pressed: Buttons(0),
        },
        runner_issued: 9,
        player_name: [0xBB, 0xBC, 0xBD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        player_id: 0x0BAD_F00D,
        field: FieldSnap {
            map: TOWN,
            camera: (-16, 32),
            drawn: Some([-1, -2, 15, 11]),
            ring: (3, 4),
            overrides: vec![((4, 5), 0x0201)],
            stale: Some(StaleSnap {
                x0: -1,
                y0: -2,
                x1: 15,
                y1: -1,
            }),
        },
        objects: ObjectsSnap {
            map: TOWN,
            run_state: RunStateSnap::Moving,
            objs: vec![player, object(2, (8, 3), None)],
        },
        templates: vec![TemplateDelta {
            local_id: 2,
            pos: Some((8, 3)),
            movement: None,
        }],
        vars: VarsSnap {
            flags: vec![FlagId(0x20), FlagId(0x200), FlagId(0x4001)],
            vars: vec![(VarId(0x4050), 3), (VarId(0x8004), 0xFFFF)],
        },
        fader: FaderSnap {
            level: 0,
            target: 0,
        },
        transition: TransitionSnap {
            map: TOWN,
            x: 5,
            y: 7,
            facing: Dir::Up,
            exit_walk: false,
            player_hidden: false,
        },
        selected: Some(1),
        last_mover: 0,
        delay: 0,
        door: 0,
        party: vec![mon()],
        respawn: 2,
        prev_behavior: Behavior::Ledge(Dir::Down),
        encounter_immunity: 0,
        bag: BagSnap {
            slots: vec![
                BagSlotSnap {
                    pocket: Pocket::Items,
                    index: 0,
                    item: ItemId(13),
                    qty: 3,
                },
                BagSlotSnap {
                    pocket: Pocket::KeyItems,
                    index: 2,
                    item: ItemId(259),
                    qty: 1,
                },
            ],
        },
        money: 3000,
        str_vars: [vec![0xBB, 0xBC], vec![], vec![0xA1; 32]],
        warp_slots: WarpSlotsSnap {
            dynamic: Some(WarpDestSnap {
                map: TOWN,
                warp: None,
                x: 4,
                y: -1,
            }),
            escape: None,
        },
        party_held: vec![Some(ItemId(139))],
        options: Some(crate::options::Options {
            text_speed: crate::options::TextSpeed::Fast,
            ..Default::default()
        }),
        pokedex: PokedexSnap {
            seen: vec![0x02, 0x00, 0x80],
            caught: vec![0x02],
        },
        pc: PcSnap {
            items: vec![PcItemSnap {
                index: 0,
                item: ItemId(13),
                qty: 999,
            }],
            mons: vec![PcMonSnap {
                pc_box: 1,
                slot: 29,
                mon: mon().to_boxed(),
            }],
            current_box: 1,
        },
        coins: 9999,
        registered_item: Some(ItemId(259)),
        berry_trees: vec![BerryTreeSnap {
            id: 3,
            tree: crate::berry::BerryTree {
                berry: 7,
                stage: 5,
                stop_growth: true,
                minutes_until_next_stage: 720,
                berry_yield: 2,
                regrowth_count: 0,
                watered: 0b0101,
            },
        }],
        map_layout: Some(291),
        sky: Some(SkySnap {
            weather: crate::weather::Weather {
                saved: crate::weather::RAIN,
                current: crate::weather::RAIN,
                next: crate::weather::NONE,
                color_map: 3,
                target: 0,
                ..crate::weather::Weather::default()
            },
            palette: Some(crate::sky::PaletteFx {
                mode: crate::sky::PaletteMode::Blend,
                color: 0x1234,
                amount: 8,
                current: 4,
                frames: 30,
                t: 15,
                pulse: 0,
                keep: true,
            }),
        }),
        audio: Some(crate::audio::Audio {
            current: 405,
            state: crate::audio::map_state::PLAYING,
            saved: 431,
            ..crate::audio::Audio::default()
        }),
    };
    Checkpoint {
        header: Header {
            schema: SCHEMA,
            build: BuildHash {
                engine: 0x0123_4567_89AB_CDEF,
                content: 42,
            },
            frame: 1234,
            world_hash: 0xFEDC_BA98_7654_3210,
        },
        world,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The schema of record. A postcard failure means a DTO changed: bump `SCHEMA` (and decide on a
/// migration, see `decode`), then re-pin both strings. A RON-only failure (a serde rename, a
/// `ron` format change) leaves the flash bytes alone and needs no `SCHEMA` bump, but stored RON
/// files (snapshots, nodes) may need a look.
const GOLDEN_POSTCARD: &str = concat!(
    "08ef9bafcdf8acd191012ad20990e4d0b287d3aeeefe01d209effdb6f50d4000",
    "09bbbcbdffffffffff8de0b75d031f400101031e16060801080a81040101031e",
    "01030202ff03000a0c0a0e030000000a0c010201000001000101040007000201",
    "0101030203010710061006030002100601020100000001110000000102011006",
    "000320800481800102d0800103848002ffff030000030a0e0100000101000000",
    "018202f8acd191018de0b75dd4d4cec9caffffffffffff058701010203040506",
    "0000000000000121012d000023280000004600010213140c0b0a090802020000",
    "0200000d030102830201b81702bbbc0020a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
    "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a101030008010001018b010102010000",
    "000003020080010201000de70701011d8202f8acd191018de0b75dd4d4cec9ca",
    "ffffffffffff8701c198c862000000000000212d00002328000000460000018f",
    "4e0183020103070501d00502000501a302010300030003000000000000000000",
    "000000000d0100b42408041e0f00010100000000000000000000000000000000",
    "00009503000200af0300000000",
);
const GOLDEN_RON: &str = concat!(
    "(header:(schema:8,build:(engine:81985529216486895,content:42),frame:1234,world_hash:1836",
    "4758544493064720),world:(frame:1234,rng:3735928559,input:(held:(64),pressed:(0)),runner_",
    "issued:9,player_name:(187,188,189,255,255,255,255,255),player_id:195948557,field:(map:(3",
    "),camera:(-16,32),drawn:Some((-1,-2,15,11)),ring:(3,4),overrides:[((4,5),513)],stale:Som",
    "e((x0:-1,y0:-2,x1:15,y1:-1))),objects:(map:(3),run_state:Moving,objs:[(local_id:255,map:",
    "(3),gfx:None,pos:(5,6),prev:(5,7),elevation:3,facing:Down,movement:Face(Down),home:(5,6)",
    ",range:(1,2),visible:true,frozen:false,facing_locked:false,action:Some((kind:Walk(Normal",
    "),dir:Up,t:4,off:(0,-4),jump_y:0)),idle:Done,anim:(table:Go(1),cmd:1,timer:3)),(local_id",
    ":2,map:(3),gfx:Some((7)),pos:(8,3),prev:(8,3),elevation:3,facing:Down,movement:Wander,ho",
    "me:(8,3),range:(1,2),visible:true,frozen:false,facing_locked:false,action:None,idle:Wait",
    "(17),anim:(table:Face,cmd:0,timer:0))]),templates:[(local_id:2,pos:Some((8,3)),movement:",
    "None)],vars:(flags:[(32),(512),(16385)],vars:[((16464),3),((32772),65535)]),fader:(level",
    ":0,target:0),transition:(map:(3),x:5,y:7,facing:Up,exit_walk:false,player_hidden:false),",
    "selected:Some(1),last_mover:0,delay:0,door:0,party:[(species:(258),personality:305419896",
    ",ot_id:195948557,nickname:(212,212,206,201,202,255,255,255,255,255,255),level:5,exp:135,",
    "ivs:(1,2,3,4,5,6),evs:(0,0,0,0,0,0),moves:(Some((33)),Some((45)),None,None),pp:(35,40,0,",
    "0),pp_bonuses:0,friendship:70,ability_slot:0,status:Sleep(2),hp:19,stats:(20,12,11,10,9,",
    "8))],respawn:2,prev_behavior:Ledge(Down),encounter_immunity:0,bag:(slots:[(pocket:Items,",
    "index:0,item:(13),qty:3),(pocket:KeyItems,index:2,item:(259),qty:1)]),money:3000,str_var",
    "s:([187,188],[],[161,161,161,161,161,161,161,161,161,161,161,161,161,161,161,161,161,161",
    ",161,161,161,161,161,161,161,161,161,161,161,161,161,161]),warp_slots:(dynamic:Some((map",
    ":(3),warp:None,x:4,y:-1)),escape:None),party_held:[Some((139))],options:Some((text_speed",
    ":Fast,battle_scene:true,battle_style:Shift,sound:Mono,button_mode:Normal,frame:0)),poked",
    "ex:(seen:[2,0,128],caught:[2]),pc:(items:[(index:0,item:(13),qty:999)],mons:[(pc_box:1,s",
    "lot:29,mon:(species:(258),personality:305419896,ot_id:195948557,nickname:(212,212,206,20",
    "1,202,255,255,255,255,255,255),exp:135,ivs:206703681,evs:(0,0,0,0,0,0),moves:(33,45,0,0)",
    ",pp:(35,40,0,0),pp_bonuses:0,friendship:70,ability_slot:0,held:None))],current_box:1),co",
    "ins:9999,registered_item:Some((259)),berry_trees:[(id:3,tree:(berry:7,stage:5,stop_growt",
    "h:true,minutes_until_next_stage:720,berry_yield:2,regrowth_count:0,watered:5))],map_layo",
    "ut:Some(291),sky:Some((weather:(saved:3,cycle_stage:0,current:3,next:0,color_map:3,targe",
    "t:0,step_delay:0,step_counter:0,init:0,finish:0,thunder:(state:0,timer:0,long_bolt:false",
    ",short_bolts:0,allow_end:false,enqueued:false,se_timer:0),abnormal:None,abnormal_current",
    ":13),palette:Some((mode:Blend,color:4660,amount:8,current:4,frames:30,t:15,pulse:0,keep:",
    "true)))),audio:Some((players:((song:0,left:0,paused:false,priority:0),(song:0,left:0,pau",
    "sed:false,priority:0),(song:0,left:0,paused:false,priority:0),(song:0,left:0,paused:fals",
    "e,priority:0)),fade:0,fade_temporary:false,current:405,next:0,state:2,fade_in:0,saved:43",
    "1,fanfare:None,duck:None,cry:0,low_hp:false))))",
);

#[test]
fn golden_postcard_bytes() {
    let bytes = encode(&sample());
    std::println!("synthetic checkpoint: {} postcard bytes", bytes.len());
    assert_eq!(hex(&bytes), GOLDEN_POSTCARD, "DTO change: bump SCHEMA");
    assert_eq!(encoded_len(&sample()), bytes.len());
    assert_eq!(SCHEMA, 8);
}

/// The CONTINUE window's data: the name, the species caught, the badges among the set flags.
#[test]
fn save_meta_counts_caught_and_badges() {
    let cp = sample();
    let m = SaveMeta::of(&cp);
    assert_eq!(
        (m.name, m.dex_caught, m.badges),
        (cp.world.player_name, 1, 0)
    );
    let mut ids = crate::data::SpecialIds::default();
    ids.flag_badges[0] = Some(FlagId(0x200));
    ids.flag_badges[1] = Some(FlagId(0x201));
    let m = cp.save_meta(&ids);
    assert_eq!((m.dex_caught, m.badges, m.frame), (1, 1, 1234));
}

#[test]
fn golden_ron() {
    let ron = ron::to_string(&sample()).unwrap();
    assert_eq!(
        ron, GOLDEN_RON,
        "DTO, serde-name or ron-format change (no SCHEMA bump if the postcard golden holds)"
    );
}

/// A schema 3 RON snapshot (a store's cache) has none of the Milestone 9 fields: it reads as a
/// new game's.
#[test]
fn schema_3_ron_reads_without_m9_fields() {
    let mut cp = sample_m9_default();
    let ron = ron::to_string(&cp).unwrap();
    let start = ron.find(",options:").unwrap();
    let v3 = format!("{}))", &ron[..start]).replacen("schema:8", "schema:3", 1);
    let back: Checkpoint = ron::from_str(&v3).unwrap();
    cp.header.schema = 3;
    assert_eq!(back, cp);
}

#[test]
fn postcard_and_ron_round_trip() {
    let cp = sample();
    assert_eq!(decode(&encode(&cp)), Ok(cp.clone()));
    let ron = ron::ser::to_string_pretty(&cp, ron::ser::PrettyConfig::default()).unwrap();
    assert_eq!(ron::from_str::<Checkpoint>(&ron).unwrap(), cp);
}

#[test]
fn encoding_starts_with_the_schema() {
    let bytes = encode(&sample());
    assert_eq!(postcard::take_from_bytes::<u16>(&bytes).unwrap().0, SCHEMA);
}

#[test]
fn another_schema_is_refused_before_decoding() {
    let mut cp = sample();
    for old_or_new in [1, 9] {
        cp.header.schema = old_or_new;
        assert_eq!(
            decode(&encode(&cp)),
            Err(CodecError::Schema { found: old_or_new })
        );
    }
    // Only the schema is read: a newer layout behind it does not matter.
    let mut bytes = postcard::to_allocvec(&9u16).unwrap();
    bytes.extend_from_slice(&[0xFF; 3]);
    assert_eq!(decode(&bytes), Err(CodecError::Schema { found: 9 }));
    // A multi-byte varint schema.
    let bytes = postcard::to_allocvec(&300u16).unwrap();
    assert_eq!(decode(&bytes), Err(CodecError::Schema { found: 300 }));
}

/// `sample()` with the schema 4 fields at a new game's values, no berry trees and no layout
/// override.
fn sample_m9_default() -> Checkpoint {
    let mut cp = sample();
    cp.world.audio = None;
    cp.world.sky = None;
    cp.world.map_layout = None;
    cp.world.berry_trees.clear();
    cp.world.options = None;
    cp.world.pokedex = PokedexSnap::default();
    cp.world.pc = PcSnap::default();
    cp.world.coins = 0;
    cp.world.registered_item = None;
    assert!(cp.world.m9_is_default());
    cp
}

/// A schema-2 checkpoint is a schema-6 one without held items, Milestone 9 state, berry trees
/// and layout override, minus the trailing empty `party_held`, `M9_DEFAULT_TAIL`, the empty
/// berry trees and the `None` layout: it decodes (as schema 6), and its digest is the schema-2
/// bytes' digest.
#[test]
fn schema_2_reads_as_6_without_held_items() {
    let mut cp = sample_m9_default();
    cp.world.party_held.clear();
    let v5 = encode(&cp);
    let tail = 1 + M9_DEFAULT_TAIL.len() + 4;
    assert!(v5[v5.len() - tail..].iter().all(|&b| b == 0));
    let mut v2 = v5[..v5.len() - tail].to_vec();
    assert_eq!(v2[0], 8);
    v2[0] = 2;
    assert_eq!(decode(&v2), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - tail]);
    assert_eq!(state_digest(&cp.world), h.finish());
    assert!((2..=8).all(schema_readable));
    assert!(!schema_readable(1) && !schema_readable(9));
}

/// A schema-3 checkpoint is a schema-6 one minus `M9_DEFAULT_TAIL`, the empty berry trees and the
/// `None` layout: it decodes (as schema 6) and keeps its schema-3 digest; any Milestone 9 state
/// makes the whole encoding count.
#[test]
fn schema_3_reads_as_6_without_m9_state() {
    let cp = sample_m9_default();
    let v5 = encode(&cp);
    let tail = M9_DEFAULT_TAIL.len() + 4;
    assert_eq!(&v5[v5.len() - tail..v5.len() - 4], &M9_DEFAULT_TAIL);
    let mut v3 = v5[..v5.len() - tail].to_vec();
    v3[0] = 3;
    assert_eq!(decode(&v3), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - tail]);
    assert_eq!(state_digest(&cp.world), h.finish());
    // With held items or Milestone 9 state the whole encoding counts.
    for w in [sample().world, {
        let mut w = sample_m9_default().world;
        w.coins = 1;
        w
    }] {
        if w.berry_trees.is_empty() {
            // Only the empty berry trees', the layout's, the sky's and the sound's bytes are
            // left out.
            let bytes = postcard::to_allocvec(&w).unwrap();
            let mut h = Fnv64::new();
            h.write(&bytes[..bytes.len() - 4]);
            assert_eq!(state_digest(&w), h.finish());
            continue;
        }
        let mut h = Fnv64::new();
        h.write(&postcard::to_allocvec(&w).unwrap());
        assert_eq!(state_digest(&w), h.finish());
    }
}

/// A schema-4 checkpoint is a schema-6 one without berry trees and layout override, minus their
/// 0 bytes: it decodes (as schema 6) and keeps its schema-4 digest; a berry tree makes the whole
/// encoding count.
#[test]
fn schema_4_reads_as_6_without_berry_trees() {
    let mut cp = sample();
    cp.world.berry_trees.clear();
    cp.world.map_layout = None;
    cp.world.sky = None;
    cp.world.audio = None;
    let v6 = encode(&cp);
    assert_eq!(v6[v6.len() - 4..], [0, 0, 0, 0]);
    let mut v4 = v6[..v6.len() - 4].to_vec();
    v4[0] = 4;
    assert_eq!(decode(&v4), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - 4]);
    assert_eq!(state_digest(&cp.world), h.finish());
    let w = sample().world;
    let mut h = Fnv64::new();
    h.write(&postcard::to_allocvec(&w).unwrap());
    assert_eq!(state_digest(&w), h.finish());
}

/// A schema-5 checkpoint is a schema-6 one on its map's own layout, minus the `None` layout's 0
/// byte: it decodes (as schema 6) and keeps its schema-5 digest; a layout override makes the
/// whole encoding count.
#[test]
fn schema_5_reads_as_6_without_layout() {
    let mut cp = sample();
    cp.world.map_layout = None;
    cp.world.sky = None;
    cp.world.audio = None;
    let v6 = encode(&cp);
    assert_eq!(v6[v6.len() - 3..], [0, 0, 0]);
    let mut v5 = v6[..v6.len() - 3].to_vec();
    v5[0] = 5;
    assert_eq!(decode(&v5), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - 3]);
    assert_eq!(state_digest(&cp.world), h.finish());
    let w = sample().world;
    assert!(w.map_layout.is_some());
    let mut h = Fnv64::new();
    h.write(&postcard::to_allocvec(&w).unwrap());
    assert_eq!(state_digest(&w), h.finish());
}

/// A schema-6 checkpoint is a schema-7 one in its map's settled weather without a palette
/// override, minus the `None` sky's 0 byte: it decodes (as schema 7) and keeps its schema-6
/// digest; a sky makes the whole encoding count.
#[test]
fn schema_6_reads_as_7_without_sky() {
    let mut cp = sample();
    cp.world.sky = None;
    cp.world.audio = None;
    let v7 = encode(&cp);
    assert_eq!(v7[v7.len() - 2..], [0, 0]);
    let mut v6 = v7[..v7.len() - 2].to_vec();
    v6[0] = 6;
    assert_eq!(decode(&v6), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - 2]);
    assert_eq!(state_digest(&cp.world), h.finish());
    let mut w = sample().world;
    w.audio = None;
    assert!(w.sky.is_some());
    let bytes = postcard::to_allocvec(&w).unwrap();
    let mut h = Fnv64::new();
    h.write(&bytes[..bytes.len() - 1]);
    assert_eq!(state_digest(&w), h.finish());
}

/// A schema-7 checkpoint is a schema-8 one with the sound a restore rebuilds, minus the `None`
/// sound's 0 byte: it decodes (as schema 8) and keeps its schema-7 digest; a sound state makes
/// the whole encoding count.
#[test]
fn schema_7_reads_as_8_without_sound() {
    let mut cp = sample();
    cp.world.audio = None;
    let v8 = encode(&cp);
    assert_eq!(v8[v8.len() - 1], 0);
    let mut v7 = v8[..v8.len() - 1].to_vec();
    v7[0] = 7;
    assert_eq!(decode(&v7), Ok(cp.clone()));
    let world = postcard::to_allocvec(&cp.world).unwrap();
    let mut h = Fnv64::new();
    h.write(&world[..world.len() - 1]);
    assert_eq!(state_digest(&cp.world), h.finish());
    let w = sample().world;
    assert!(w.audio.is_some());
    let mut h = Fnv64::new();
    h.write(&postcard::to_allocvec(&w).unwrap());
    assert_eq!(state_digest(&w), h.finish());
}

#[test]
fn bad_bytes_are_postcard_errors() {
    let bytes = encode(&sample());
    for len in [0, 1, bytes.len() / 2, bytes.len() - 1] {
        assert_eq!(
            decode(&bytes[..len]),
            Err(CodecError::Postcard),
            "truncated to {len}"
        );
    }
    let mut long = bytes.clone();
    long.push(0);
    assert_eq!(decode(&long), Err(CodecError::Postcard), "trailing byte");
}

#[test]
fn encode_checked_rejects_past_the_flash_budget() {
    let cp = sample();
    assert_eq!(encode_checked(&cp), Ok(encode(&cp)));
    let mut big = cp;
    big.world.vars.flags = (0..20000).map(FlagId).collect();
    let len = encode(&big).len();
    assert_eq!(encoded_len(&big), len);
    assert!(len > MAX_ENCODED);
    assert_eq!(encode_checked(&big), Err(CodecError::TooLarge(len)));
}

#[test]
fn state_digest_is_stable_and_semantic() {
    let a = sample().world;
    assert_eq!(state_digest(&a), state_digest(&a.clone()));
    let mut fnv = Fnv64::new();
    fnv.write(&postcard::to_allocvec(&a).unwrap());
    assert_eq!(state_digest(&a), fnv.finish());
    let mut b = a.clone();
    b.vars.flags.push(FlagId(0x4002));
    assert_ne!(state_digest(&a), state_digest(&b));
    // The header (build, frame, world hash) is not part of the identity.
    let mut cp = sample();
    cp.header.build = BuildHash::default();
    assert_eq!(state_digest(&cp.world), state_digest(&a));
    // The streaming digest hashes exactly the encoded world: the checkpoint bytes minus the
    // header's.
    let header = postcard::to_allocvec(&cp.header).unwrap();
    let mut fnv = Fnv64::new();
    fnv.write(&encode(&cp)[header.len()..]);
    assert_eq!(state_digest(&cp.world), fnv.finish());
}

#[test]
fn decode_refuses_more_than_the_flash_budget() {
    let bytes = vec![0; MAX_ENCODED + 1];
    assert_eq!(decode(&bytes), Err(CodecError::TooLarge(MAX_ENCODED + 1)));
    // Even a valid checkpoint (20000 flags) is refused unread.
    let mut big = sample();
    big.world.vars.flags = (0..20000).map(FlagId).collect();
    let bytes = encode(&big);
    assert_eq!(decode(&bytes), Err(CodecError::TooLarge(bytes.len())));
}

/// Peak heap use of the current thread, for the forged-length test.
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    std::thread_local! {
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static PEAK: Cell<usize> = const { Cell::new(0) };
    }

    fn add(n: usize) {
        let _ = LIVE.try_with(|live| {
            let v = live.get() + n;
            live.set(v);
            let _ = PEAK.try_with(|p| p.set(p.get().max(v)));
        });
    }

    fn sub(n: usize) {
        let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(n)));
    }

    pub struct Counting;

    // SAFETY: forwards every call to `System` unchanged; the counters are const thread-locals,
    // which never allocate.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            add(l.size());
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            add(l.size());
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            sub(l.size());
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
            sub(l.size());
            add(new);
            unsafe { System.realloc(p, l, new) }
        }
    }

    /// `f`'s result and the most heap it held at once above what was live before.
    pub fn peak<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let base = LIVE.with(Cell::get);
        PEAK.with(|p| p.set(base));
        let r = f();
        (r, PEAK.with(Cell::get) - base)
    }
}

#[global_allocator]
static ALLOC: counting::Counting = counting::Counting;

#[test]
fn a_forged_length_cannot_preallocate() {
    let mut cp = sample();
    cp.world.party.clear();
    let bytes = encode(&cp);
    // The party length (0), then the fields after the party.
    let w = &cp.world;
    let after = postcard::to_allocvec(&(
        w.respawn,
        w.prev_behavior,
        w.encounter_immunity,
        &w.bag,
        w.money,
        &w.str_vars,
        w.warp_slots,
        &w.party_held,
        (&w.options, &w.pokedex, &w.pc, w.coins, w.registered_item),
        &w.berry_trees,
        w.map_layout,
        w.sky,
        w.audio,
    ))
    .unwrap();
    let party_at = bytes.len() - after.len() - 1;
    assert_eq!(bytes[party_at], 0x00);
    assert_eq!(bytes[party_at + 1..], after[..]);
    // A party of 4000 (varint a0 1f), then 4000 bytes that are no `Pokemon` (an overlong
    // varint). Postcard hints the length whenever that many bytes remain, and serde's `Vec`
    // would preallocate 4000 `Pokemon`s from the hint.
    let mut forged = bytes[..party_at].to_vec();
    forged.extend_from_slice(&[0xA0, 0x1F]);
    forged.extend_from_slice(&[0xFF; 4000]);
    assert!(forged.len() <= MAX_ENCODED);
    let (res, peak) = counting::peak(|| decode(&forged));
    assert_eq!(res, Err(CodecError::Postcard));
    // 4000 `Pokemon`s would be ~270 KB; the decode holds about one snapshot.
    std::println!("forged party decode: peak {peak} bytes");
    assert!(peak < 4096, "peak {peak}");
    // Every forged length of every snapshot `Vec` fails the same way.
    let (_, peak) = counting::peak(|| {
        for cut in 0..bytes.len() {
            let mut f = bytes[..cut].to_vec();
            f.extend_from_slice(&[0xFF, 0xFF, 0x03]);
            f.extend_from_slice(&[0xFF; 4000]);
            let _ = decode(&f);
        }
    });
    assert!(peak < 16384, "peak {peak}");
}

/// The postcard variant index of every variant of every enum in the snapshot. A reorder fails
/// here, an addition fails to compile (the matches are exhaustive): both change the schema.
#[test]
fn enum_variant_indices_are_pinned() {
    fn tag<T: Serialize>(v: &T) -> u8 {
        postcard::to_allocvec(v).unwrap()[0]
    }
    fn check<T: Serialize + core::fmt::Debug>(all: &[T], idx: fn(&T) -> u8) {
        for (i, v) in all.iter().enumerate() {
            assert_eq!(idx(v) as usize, i, "{v:?}: list out of order");
            assert_eq!(tag(v), idx(v), "{v:?}: variant index changed: bump SCHEMA");
        }
    }
    check(&[Dir::Down, Dir::Up, Dir::Left, Dir::Right], |v| match v {
        Dir::Down => 0,
        Dir::Up => 1,
        Dir::Left => 2,
        Dir::Right => 3,
    });
    check(
        &[Speed::Slow, Speed::Normal, Speed::Fast, Speed::Faster],
        |v| match v {
            Speed::Slow => 0,
            Speed::Normal => 1,
            Speed::Fast => 2,
            Speed::Faster => 3,
        },
    );
    check(
        &[
            RunStateSnap::NotMoving,
            RunStateSnap::TurnDirection,
            RunStateSnap::Moving,
            RunStateSnap::NotMovingHeld,
            RunStateSnap::TurnDirectionHeld,
            RunStateSnap::NotMovingCentered,
            RunStateSnap::TurnDirectionCentered,
            RunStateSnap::MovingCentered,
        ],
        |v| match v {
            RunStateSnap::NotMoving => 0,
            RunStateSnap::TurnDirection => 1,
            RunStateSnap::Moving => 2,
            RunStateSnap::NotMovingHeld => 3,
            RunStateSnap::TurnDirectionHeld => 4,
            RunStateSnap::NotMovingCentered => 5,
            RunStateSnap::TurnDirectionCentered => 6,
            RunStateSnap::MovingCentered => 7,
        },
    );
    check(
        &[
            ActionKindSnap::Walk(Speed::Slow),
            ActionKindSnap::Run,
            ActionKindSnap::InPlace(0),
            ActionKindSnap::Jump(0),
            ActionKindSnap::Delay(0),
        ],
        |v| match v {
            ActionKindSnap::Walk(_) => 0,
            ActionKindSnap::Run => 1,
            ActionKindSnap::InPlace(_) => 2,
            ActionKindSnap::Jump(_) => 3,
            ActionKindSnap::Delay(_) => 4,
        },
    );
    check(&Pocket::ALL, |v| match v {
        Pocket::Items => 0,
        Pocket::KeyItems => 1,
        Pocket::PokeBalls => 2,
        Pocket::TmHm => 3,
        Pocket::Berries => 4,
    });
    check(
        &[IdleSnap::Start, IdleSnap::Wait(0), IdleSnap::Done],
        |v| match v {
            IdleSnap::Start => 0,
            IdleSnap::Wait(_) => 1,
            IdleSnap::Done => 2,
        },
    );
    check(
        &[
            AnimTableSnap::Face,
            AnimTableSnap::Go(0),
            AnimTableSnap::Run,
        ],
        |v| match v {
            AnimTableSnap::Face => 0,
            AnimTableSnap::Go(_) => 1,
            AnimTableSnap::Run => 2,
        },
    );
    check(
        &[
            MovementType::Face(Dir::Down),
            MovementType::LookAround,
            MovementType::Wander,
            MovementType::WanderVertical,
            MovementType::WanderHorizontal,
            MovementType::WalkInPlace(Dir::Down),
            MovementType::Invisible,
            MovementType::FaceTwo(Dirs::two(Dir::Down, Dir::Up)),
            MovementType::FaceAllBut(Dir::Down),
            MovementType::Rotate { clockwise: true },
            MovementType::WalkBackAndForth(Dir::Down),
            MovementType::WalkSequence(Dirs::new([Dir::Down; 4])),
            MovementType::JogInPlace(Dir::Down),
            MovementType::RunInPlace(Dir::Down),
            MovementType::WalkSlowlyInPlace(Dir::Down),
            MovementType::WanderBackwards,
        ],
        |v| match v {
            MovementType::Face(_) => 0,
            MovementType::LookAround => 1,
            MovementType::Wander => 2,
            MovementType::WanderVertical => 3,
            MovementType::WanderHorizontal => 4,
            MovementType::WalkInPlace(_) => 5,
            MovementType::Invisible => 6,
            MovementType::FaceTwo(..) => 7,
            MovementType::FaceAllBut(_) => 8,
            MovementType::Rotate { .. } => 9,
            MovementType::WalkBackAndForth(_) => 10,
            MovementType::WalkSequence(_) => 11,
            MovementType::JogInPlace(_) => 12,
            MovementType::RunInPlace(_) => 13,
            MovementType::WalkSlowlyInPlace(_) => 14,
            MovementType::WanderBackwards => 15,
        },
    );
    check(
        &[
            Behavior::Normal,
            Behavior::Blocked,
            Behavior::Ledge(Dir::Down),
            Behavior::Door,
            Behavior::Warp,
            Behavior::ArrowWarp(Dir::Down),
            Behavior::Counter,
            Behavior::TallGrass,
            Behavior::Impassable(crate::data::Sides(1)),
            Behavior::NonAnimDoor(Dir::Up),
            Behavior::Ladder,
            Behavior::Escalator(Dir::Right),
            Behavior::Ice,
            Behavior::ForcedWalk(Dir::Left),
            Behavior::Slide(Dir::Up),
            Behavior::MuddySlope,
            Behavior::ThinIce,
            Behavior::CrackedIce,
            Behavior::CrackedFloor,
            Behavior::CrackedFloorHole,
            Behavior::Floor(Floor::Cave),
            Behavior::Water(crate::data::Water::Ocean),
            Behavior::PadWarp,
            Behavior::PopOutWarp(Dir::Right),
        ],
        |v| match v {
            Behavior::Normal => 0,
            Behavior::Blocked => 1,
            Behavior::Ledge(_) => 2,
            Behavior::Door => 3,
            Behavior::Warp => 4,
            Behavior::ArrowWarp(_) => 5,
            Behavior::Counter => 6,
            Behavior::TallGrass => 7,
            Behavior::Impassable(_) => 8,
            Behavior::NonAnimDoor(_) => 9,
            Behavior::Ladder => 10,
            Behavior::Escalator(_) => 11,
            Behavior::Ice => 12,
            Behavior::ForcedWalk(_) => 13,
            Behavior::Slide(_) => 14,
            Behavior::MuddySlope => 15,
            Behavior::ThinIce => 16,
            Behavior::CrackedIce => 17,
            Behavior::CrackedFloor => 18,
            Behavior::CrackedFloorHole => 19,
            Behavior::Floor(_) => 20,
            Behavior::Water(_) => 21,
            Behavior::PadWarp => 22,
            Behavior::PopOutWarp(_) => 23,
        },
    );
    check(
        &[
            Floor::LongGrass,
            Floor::Unused05,
            Floor::DeepSand,
            Floor::Cave,
            Floor::IndoorEncounter,
            Floor::AshGrass,
            Floor::Footprints,
            Floor::Sand,
            Floor::MountainTop,
        ],
        |v| match v {
            Floor::LongGrass => 0,
            Floor::Unused05 => 1,
            Floor::DeepSand => 2,
            Floor::Cave => 3,
            Floor::IndoorEncounter => 4,
            Floor::AshGrass => 5,
            Floor::Footprints => 6,
            Floor::Sand => 7,
            Floor::MountainTop => 8,
        },
    );
    check(
        &[
            Status::None,
            Status::Sleep(0),
            Status::Poison,
            Status::Burn,
            Status::Freeze,
            Status::Paralysis,
            Status::Toxic(0),
        ],
        |v| match v {
            Status::None => 0,
            Status::Sleep(_) => 1,
            Status::Poison => 2,
            Status::Burn => 3,
            Status::Freeze => 4,
            Status::Paralysis => 5,
            Status::Toxic(_) => 6,
        },
    );
}

// --- build identity ---

/// A fresh, empty temp dir for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pkr-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(root: &Path, rel: &str, s: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

const TREE: [(&str, &str); 8] = [
    ("lib.rs", "pub mod a;\n"),
    ("a.rs", "pub fn a() {}\n"),
    ("a/b.rs", "pub fn b() {}\n"),
    ("z.rs", "// z\n"),
    ("notes.txt", "not a source"),
    ("tests.rs", "#[test] fn t() {}\n"),
    ("a/test_data.rs", "// fixture\n"),
    ("world/tests/case.rs", "// case\n"),
];

#[test]
fn tree_hash_is_fnv64() {
    let mut a = build_support::TreeHash::new();
    let mut b = Fnv64::new();
    a.write(b"pokemon");
    b.write(b"pokemon");
    assert_eq!(a.finish(), b.finish());
}

#[test]
fn engine_hash_is_deterministic_and_ignores_tests() {
    let one = temp_dir("hash-one");
    let two = temp_dir("hash-two");
    // Same files, other absolute path, other creation order.
    for (rel, s) in TREE {
        put(&one, rel, s);
    }
    for (rel, s) in TREE.iter().rev() {
        put(&two, rel, s);
    }
    let h = build_support::engine_src_hash(&one).unwrap();
    assert_eq!(build_support::engine_src_hash(&two).unwrap(), h);
    assert_eq!(build_support::engine_src_hash(&one).unwrap(), h);

    // Editing tests or non-sources changes nothing.
    put(&two, "tests.rs", "#[test] fn other() {}\n");
    put(&two, "a/test_data.rs", "// other fixture\n");
    put(&two, "world/tests/new.rs", "// new case\n");
    put(&two, "x_tests.rs", "// tests\n");
    put(&two, "notes.txt", "edited");
    assert_eq!(build_support::engine_src_hash(&two).unwrap(), h);

    // CRLF checkouts hash like LF ones.
    put(&two, "a.rs", "pub fn a() {}\r\n");
    assert_eq!(build_support::engine_src_hash(&two).unwrap(), h);

    // A source edit, a new source and a rename all change it.
    put(&two, "a.rs", "pub fn a() { }\n");
    assert_ne!(build_support::engine_src_hash(&two).unwrap(), h);
    put(&two, "a.rs", "pub fn a() {}\n");
    put(&two, "new.rs", "");
    assert_ne!(build_support::engine_src_hash(&two).unwrap(), h);
    std::fs::remove_file(two.join("new.rs")).unwrap();
    std::fs::rename(two.join("z.rs"), two.join("y.rs")).unwrap();
    assert_ne!(build_support::engine_src_hash(&two).unwrap(), h);

    let _ = std::fs::remove_dir_all(&one);
    let _ = std::fs::remove_dir_all(&two);
}

#[test]
fn file_order_is_by_relative_path_bytes() {
    let dir = temp_dir("order");
    for (rel, s) in TREE {
        put(&dir, rel, s);
    }
    let files = build_support::files(&dir, &build_support::is_build_source).unwrap();
    let rels: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(rels, ["a.rs", "a/b.rs", "lib.rs", "z.rs"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hidden_files_and_backups_are_not_inputs() {
    let dir = temp_dir("hidden");
    for (rel, s) in TREE {
        put(&dir, rel, s);
    }
    let all = |_: &str| true;
    let before = build_support::files(&dir, &all).unwrap();
    let h = build_support::engine_src_hash(&dir).unwrap();
    put(&dir, ".DS_Store", "finder");
    put(&dir, "a/.DS_Store", "finder");
    put(&dir, ".git/HEAD", "ref");
    put(&dir, ".hidden/c.rs", "pub fn c() {}\n");
    put(&dir, "a.rs~", "backup");
    put(&dir, "a/.b.rs.swp", "swap");
    assert_eq!(build_support::files(&dir, &all).unwrap(), before);
    assert_eq!(build_support::engine_src_hash(&dir).unwrap(), h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn symlinks_are_followed() {
    let dir = temp_dir("links");
    let real = temp_dir("links-real");
    for (rel, s) in TREE {
        put(&dir, rel, s);
    }
    let h = build_support::engine_src_hash(&dir).unwrap();
    // `a/` moved out and linked back: same files, same hash, no `IsADirectory`.
    std::fs::rename(dir.join("a"), real.join("a")).unwrap();
    std::os::unix::fs::symlink(real.join("a"), dir.join("a")).unwrap();
    // A linked file counts as its target.
    std::fs::rename(dir.join("z.rs"), real.join("z.rs")).unwrap();
    std::os::unix::fs::symlink(real.join("z.rs"), dir.join("z.rs")).unwrap();
    assert_eq!(build_support::engine_src_hash(&dir).unwrap(), h);
    // A cycle is an error, not a hang.
    std::os::unix::fs::symlink(&dir, real.join("a/up")).unwrap();
    assert!(build_support::engine_src_hash(&dir).is_err());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&real);
}

#[test]
fn engine_src_hash_is_this_build() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert_eq!(
        build_support::engine_src_hash(&src).unwrap(),
        ENGINE_SRC_HASH
    );
    assert_ne!(ENGINE_SRC_HASH, 0);
}

/// Test modules live in their own files (`tests.rs`, `*_tests.rs`), which the build hash
/// skips: an inline `#[cfg(test)] mod x { .. }` in a hashed source would make a test-only edit
/// change `ENGINE_SRC_HASH` (and so every checkpoint's build).
#[test]
fn no_hashed_engine_source_holds_an_inline_test_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut inline = Vec::new();
    for (rel, path) in build_support::files(&src, &build_support::is_build_source).unwrap() {
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (i, w) in lines.windows(2).enumerate() {
            if w[0] == "#[cfg(test)]" && w[1].contains("mod ") && w[1].ends_with('{') {
                inline.push(format!("{rel}:{}", i + 2));
            }
        }
    }
    assert!(inline.is_empty(), "move these into test files: {inline:?}");
}
