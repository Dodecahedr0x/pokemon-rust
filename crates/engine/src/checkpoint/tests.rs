//! Codec, schema policy and build identity. Every value here is synthetic.

use super::*;
use crate::data::PLAYER;
use crate::mon::{NAME_LEN, Status};
use crate::{data::MoveId, data::SpeciesId};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../../build_support.rs"]
mod build_support;

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
    "01ef9bafcdf8acd191012ad20990e4d0b287d3aeeefe01d209effdb6f50d4000",
    "09bbbcbdffffffffff8de0b75d031f400101031e16060801080a8104030202ff",
    "03000a0c0a0e030000000a0c0102010000010001010400070002010101030203",
    "0107100610060300021006010201000000011100000001020110060003208004",
    "81800102d0800103848002ffff030000030a0e0100000101000000018202f8ac",
    "d191018de0b75dd4d4cec9caffffffffffff0587010102030405060000000000",
    "000121012d000023280000004600010213140c0b0a090802020000",
);
const GOLDEN_RON: &str = concat!(
    "(header:(schema:1,build:(engine:81985529216486895,content:42),frame:1234,",
    "world_hash:18364758544493064720),world:(frame:1234,rng:3735928559,",
    "input:(held:(64),pressed:(0)),runner_issued:9,",
    "player_name:(187,188,189,255,255,255,255,255),player_id:195948557,",
    "field:(map:(3),camera:(-16,32),drawn:Some((-1,-2,15,11)),ring:(3,4),",
    "overrides:[((4,5),513)]),",
    "objects:(map:(3),run_state:Moving,objs:[",
    "(local_id:255,map:(3),gfx:None,pos:(5,6),prev:(5,7),elevation:3,facing:Down,",
    "movement:Face(Down),home:(5,6),range:(1,2),visible:true,frozen:false,",
    "facing_locked:false,action:Some((kind:Walk(Normal),dir:Up,t:4,off:(0,-4),jump_y:0)),",
    "idle:Done,anim:(table:Go(1),cmd:1,timer:3)),",
    "(local_id:2,map:(3),gfx:Some((7)),pos:(8,3),prev:(8,3),elevation:3,facing:Down,",
    "movement:Wander,home:(8,3),range:(1,2),visible:true,frozen:false,",
    "facing_locked:false,action:None,idle:Wait(17),anim:(table:Face,cmd:0,timer:0))]),",
    "templates:[(local_id:2,pos:Some((8,3)),movement:None)],",
    "vars:(flags:[(32),(512),(16385)],vars:[((16464),3),((32772),65535)]),",
    "fader:(level:0,target:0),",
    "transition:(map:(3),x:5,y:7,facing:Up,exit_walk:false,player_hidden:false),",
    "selected:Some(1),last_mover:0,delay:0,door:0,",
    "party:[(species:(258),personality:305419896,ot_id:195948557,",
    "nickname:(212,212,206,201,202,255,255,255,255,255,255),level:5,exp:135,",
    "ivs:(1,2,3,4,5,6),evs:(0,0,0,0,0,0),moves:(Some((33)),Some((45)),None,None),",
    "pp:(35,40,0,0),pp_bonuses:0,friendship:70,ability_slot:0,status:Sleep(2),hp:19,",
    "stats:(20,12,11,10,9,8))],",
    "respawn:2,prev_behavior:Ledge(Down),encounter_immunity:0))",
);

#[test]
fn golden_postcard_bytes() {
    let bytes = encode(&sample());
    std::println!("synthetic checkpoint: {} postcard bytes", bytes.len());
    assert_eq!(hex(&bytes), GOLDEN_POSTCARD, "DTO change: bump SCHEMA");
    assert_eq!(encoded_len(&sample()), bytes.len());
    assert_eq!(SCHEMA, 1);
}

#[test]
fn golden_ron() {
    let ron = ron::to_string(&sample()).unwrap();
    assert_eq!(
        ron, GOLDEN_RON,
        "DTO, serde-name or ron-format change (no SCHEMA bump if the postcard golden holds)"
    );
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
    cp.header.schema = 2;
    assert_eq!(decode(&encode(&cp)), Err(CodecError::Schema { found: 2 }));
    // Only the schema is read: a newer layout behind it does not matter.
    let mut bytes = postcard::to_allocvec(&2u16).unwrap();
    bytes.extend_from_slice(&[0xFF; 3]);
    assert_eq!(decode(&bytes), Err(CodecError::Schema { found: 2 }));
    // A multi-byte varint schema.
    let bytes = postcard::to_allocvec(&300u16).unwrap();
    assert_eq!(decode(&bytes), Err(CodecError::Schema { found: 300 }));
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
    big.world.vars.flags = (0..5000).map(FlagId).collect();
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
    // Even a valid checkpoint (5000 flags) is refused unread.
    let mut big = sample();
    big.world.vars.flags = (0..5000).map(FlagId).collect();
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
    // The world ends with the party length, respawn, prev_behavior (Ledge, Down) and
    // encounter_immunity.
    let tail = [0x00, 0x02, 0x02, 0x00, 0x00];
    assert_eq!(bytes[bytes.len() - 5..], tail);
    // A party of 4000 (varint a0 1f), then 4000 bytes that are no `Pokemon` (an overlong
    // varint). Postcard hints the length whenever that many bytes remain, and serde's `Vec`
    // would preallocate 4000 `Pokemon`s from the hint.
    let mut forged = bytes[..bytes.len() - 5].to_vec();
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
        ],
        |v| match v {
            RunStateSnap::NotMoving => 0,
            RunStateSnap::TurnDirection => 1,
            RunStateSnap::Moving => 2,
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
        ],
        |v| match v {
            MovementType::Face(_) => 0,
            MovementType::LookAround => 1,
            MovementType::Wander => 2,
            MovementType::WanderVertical => 3,
            MovementType::WanderHorizontal => 4,
            MovementType::WalkInPlace(_) => 5,
            MovementType::Invisible => 6,
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
