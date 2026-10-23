//! SIL1 Task D: the explorer is per flavour. Each build explores its own acts (an act of another
//! flavour is refused and has no goals), and a db made by one flavour is refused by the others.
//! Runs on every flavour.

use content::Flavor;
use engine::checkpoint::BuildHash;
use explore::config::Config;
use explore::db::{Db, DbError};
use explore::goals::{self, Act};

const B: BuildHash = BuildHash {
    engine: 1,
    content: 2,
};

#[test]
fn each_build_explores_its_own_acts() {
    let own = Act::of(content::FLAVOR);
    assert!(!own.is_empty());
    assert!(
        own.contains(&Act::default()),
        "the default act is the build's"
    );
    assert_eq!(
        Act::default(),
        match content::FLAVOR {
            Flavor::Stub | Flavor::Emerald => Act::Act1,
            Flavor::Silice => Act::SilCh0,
        }
    );
    // Every act is one flavour's: Emerald's and the stub's are the same list, Silice's another.
    for act in Act::ALL {
        assert_ne!(
            Act::EMERALD.contains(&act),
            Act::SILICE.contains(&act),
            "{act:?}"
        );
        assert_eq!(
            Act::of(Flavor::Stub).contains(&act),
            Act::EMERALD.contains(&act)
        );
        assert_eq!(
            Act::of(Flavor::Emerald).contains(&act),
            Act::EMERALD.contains(&act)
        );
        assert_eq!(
            Act::of(Flavor::Silice).contains(&act),
            Act::SILICE.contains(&act)
        );
        assert_eq!(act.in_build(), own.contains(&act));
        if act.in_build() {
            assert_eq!(Act::parse_in_build(act.name()), Ok(act));
        } else {
            let e = Act::parse_in_build(act.name()).expect_err("another flavour's act");
            assert!(
                e.contains(act.name()) && e.contains(content::FLAVOR.name()),
                "{e}"
            );
            assert!(goals::goals_of(act).is_empty(), "{act:?} has no goals here");
        }
    }
    assert!(Act::parse_in_build("act2").is_err());
    // The Prologue's goals (the Silice spine's `Expect`s) end at the Bois-Éther edge.
    if content::FLAVOR == Flavor::Silice {
        let ch0 = goals::goals_of(Act::SilCh0);
        assert_eq!(ch0.first().map(|g| g.name), Some("intro_done"));
        assert_eq!(ch0.last().map(|g| g.name), Some("bois_ether_edge"));
    }
}

#[test]
fn explore_refuses_an_act_of_another_flavour() {
    let foreign = Act::ALL
        .into_iter()
        .find(|a| !a.in_build())
        .expect("another flavour has an act");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    // The config's act is checked: the spine's goals are the build's act.
    let config = dir.path().join("explore.ron");
    let cfg = Config {
        act: foreign,
        ..Config::default()
    };
    std::fs::write(&config, cfg.to_ron()).unwrap();
    for cmd in ["spine", "branch"] {
        let args = [
            cmd,
            "--config",
            config.to_str().unwrap(),
            "--db",
            path.to_str().unwrap(),
        ];
        let e = explore::cli::main(args.map(String::from).to_vec()).expect_err("refused");
        assert!(format!("{e:#}").contains(foreign.name()), "{e:#}");
        assert!(!path.exists(), "no db is made for a refused act");
    }
    // `--act` is gone: the spine is the build's.
    let e = explore::cli::main(
        ["spine", "--act", "act1", "--db", path.to_str().unwrap()]
            .map(String::from)
            .to_vec(),
    )
    .expect_err("no --act");
    assert!(format!("{e:#}").contains("--act"), "{e:#}");
    assert!(!cfg.act.in_build());
}

#[test]
fn a_db_opens_only_under_its_own_flavour() {
    let dir = tempfile::tempdir().unwrap();
    for made in Flavor::ALL {
        let path = dir.path().join(format!("{}.db", made.name()));
        let (db, st) = Db::open(&path, B, made).unwrap();
        assert!(st.fresh);
        assert_eq!(db.meta("flavor").unwrap().as_deref(), Some(made.name()));
        drop(db);
        for build in Flavor::ALL {
            match Db::open(&path, B, build) {
                Ok((_, st)) => {
                    assert_eq!(build, made);
                    assert!(!st.fresh);
                }
                Err(e) => {
                    assert_ne!(build, made);
                    assert!(
                        matches!(&e, DbError::Flavor { db, build: b }
                            if db == made.name() && b == build.name()),
                        "{e}"
                    );
                    let msg = e.to_string();
                    assert!(
                        msg.contains(made.name()) && msg.contains(build.name()),
                        "{msg}"
                    );
                }
            }
        }
    }
}

/// The dbs made before Task D (`meta.flavor` written as `stub` or `emerald` by the old
/// `flavor_name`) still open under their flavour; a db that records none is refused with the
/// reset hint.
#[test]
fn pre_task_d_dbs_still_open() {
    let dir = tempfile::tempdir().unwrap();
    for (old, flavor) in [("stub", Flavor::Stub), ("emerald", Flavor::Emerald)] {
        let path = dir.path().join(format!("{old}.db"));
        let (mut db, _) = Db::open(&path, B, Flavor::Silice).unwrap();
        db.set_meta("flavor", old).unwrap();
        drop(db);
        let (_, st) = Db::open(&path, B, flavor).unwrap();
        assert!(!st.fresh);
    }
    let path = dir.path().join("none.db");
    let (mut db, _) = Db::open(&path, B, Flavor::Emerald).unwrap();
    db.set_meta("flavor", "").unwrap();
    drop(db);
    let e = Db::open(&path, B, Flavor::Emerald)
        .err()
        .expect("no flavour");
    assert!(e.to_string().contains("explore reset --yes"), "{e}");
}
