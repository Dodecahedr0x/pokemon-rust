//! Milestone 8 Task H: the item, money, buffer and multichoice prelude helpers, as the requests
//! they issue (Gen3 `obtain_item.inc` and the item script commands), on whatever the build has.

use content::consts::*;
use content::prelude::{self, text_named};
use engine::data::{ItemId, Pocket, TextId};
use engine::script::{BufferSrc, Ctx, Report, Request, Response, Runner, Script, ScriptHost};
use std::collections::BTreeMap;

/// Answers every request at once: bag requests with `fits`, multichoice with `choice`; records
/// them and keeps the vars.
struct Host {
    log: Vec<Request>,
    vars: BTreeMap<u16, u16>,
    fits: bool,
    choice: u16,
}

impl Host {
    fn new(fits: bool) -> Host {
        Host {
            log: Vec::new(),
            vars: BTreeMap::new(),
            fits,
            choice: 0,
        }
    }

    fn var_result(&self) -> u16 {
        self.vars[&VAR_RESULT.0]
    }

    fn has(&self, r: &Request) -> bool {
        self.log.contains(r)
    }

    fn reported(&self, what: &'static str) -> bool {
        self.has(&Request::Report(Report::Degraded(what)))
    }
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match *r {
            Request::GetVar(v) => Response::Value(self.vars.get(&v.0).copied().unwrap_or(0)),
            Request::SetVar(v, x) => {
                self.vars.insert(v.0, x);
                Response::Unit
            }
            Request::ItemAdd { .. }
            | Request::ItemRemove { .. }
            | Request::ItemCheck { .. }
            | Request::ItemSpace { .. }
            | Request::MoneyCheck(_) => Response::Bool(self.fits),
            Request::Multichoice { .. } => Response::Value(self.choice),
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        Some(Response::Unit)
    }
}

fn run(host: &mut Host, script: impl FnOnce(Ctx) -> Script) {
    let mut r = Runner::new();
    r.start(script(r.ctx()));
    for _ in 0..100 {
        if !r.step(host) {
            return;
        }
    }
    panic!("script still running");
}

/// The message request of `gText_*` `name`, or (without the text) its report.
fn says(h: &Host, name: &str) -> bool {
    match text_named(name) {
        Some(t) => h.has(&Request::Message(t)),
        None => h.reported("obtain_item.inc text"),
    }
}

#[test]
fn text_names_are_sorted_for_text_named() {
    let names = &content::data::TEXT_NAMES;
    assert!(names.windows(2).all(|w| w[0] < w[1]));
    for (i, n) in names.iter().enumerate() {
        assert_eq!(text_named(n), Some(TextId(i as u16)));
    }
    assert_eq!(text_named("no such text"), None);
}

#[test]
fn pocket_numbers_and_std_strings_are_gen3s() {
    // `include/constants/item.h` `POCKET_*`.
    let order = [
        (None, 0),
        (Some(Pocket::Items), 1),
        (Some(Pocket::PokeBalls), 2),
        (Some(Pocket::TmHm), 3),
        (Some(Pocket::Berries), 4),
        (Some(Pocket::KeyItems), 5),
    ];
    for (p, n) in order {
        assert_eq!(prelude::pocket_number(p), n);
    }
    let std = |name: &str| {
        let t = &extract::names::STD_STRINGS;
        let v = t.entries.iter().find(|(_, n)| *n == name).unwrap().0;
        u8::try_from(v).unwrap()
    };
    for (p, name) in [
        (Pocket::Items, "STDSTRING_ITEMS"),
        (Pocket::KeyItems, "STDSTRING_KEYITEMS"),
        (Pocket::PokeBalls, "STDSTRING_POKEBALLS"),
        (Pocket::TmHm, "STDSTRING_TMHMS"),
        (Pocket::Berries, "STDSTRING_BERRIES"),
    ] {
        assert_eq!(prelude::pocket_std_string(p), std(name), "{name}");
    }
}

#[test]
fn give_item_adds_says_so_and_sets_var_result() {
    let mut h = Host::new(true);
    run(&mut h, |ctx| {
        Box::pin(async move {
            // 300 passes Gen3's `(u8)` cast as 44.
            assert!(prelude::give_item(&ctx, ITEM_POTION, 300).await);
        })
    });
    let potion = ItemId(ITEM_POTION);
    assert_eq!(
        h.log[0],
        Request::ItemAdd {
            item: potion,
            qty: 44
        }
    );
    assert!(h.has(&Request::Buffer {
        slot: 1,
        src: BufferSrc::Item(potion)
    }));
    assert!(says(&h, "gText_ObtainedTheItem"));
    assert!(says(&h, "gText_PutItemInPocket"));
    assert_eq!(h.var_result(), 1);
}

#[test]
fn give_item_without_room_says_nothing() {
    let mut h = Host::new(false);
    run(&mut h, |ctx| {
        Box::pin(async move {
            assert!(!prelude::give_item(&ctx, ITEM_POKE_BALL, 5).await);
        })
    });
    assert!(!h.log.iter().any(|r| matches!(r, Request::Message(_))));
    assert!(
        h.reported("bufferitemnameplural"),
        "five balls have a plural"
    );
    assert_eq!(h.var_result(), 0);
}

#[test]
fn find_item_removes_the_ball_and_adds_the_item() {
    let mut h = Host::new(true);
    h.vars.insert(VAR_LAST_TALKED.0, 3);
    run(&mut h, |ctx| {
        Box::pin(async move {
            assert!(prelude::find_item(&ctx, ITEM_POTION, 1).await);
        })
    });
    assert_eq!(&h.log[..2], [Request::Lock, Request::FacePlayer]);
    let potion = ItemId(ITEM_POTION);
    let at = |r: &Request| h.log.iter().position(|x| x == r).unwrap();
    let space = at(&Request::ItemSpace {
        item: potion,
        qty: 1,
    });
    let remove = at(&Request::RemoveObject(3));
    let add = at(&Request::ItemAdd {
        item: potion,
        qty: 1,
    });
    assert!(space < remove && remove < add);
    assert!(says(&h, "gText_PlayerFoundOneItem"));
    assert!(says(&h, "gText_PutItemInPocket"));
    assert_eq!(h.log.last(), Some(&Request::Release));
    assert_eq!(h.var_result(), 0, "Gen3 leaves PYRAMID_LOCATION_NONE");
}

#[test]
fn find_item_with_a_full_bag_leaves_the_ball() {
    let mut h = Host::new(false);
    run(&mut h, |ctx| {
        Box::pin(async move {
            assert!(!prelude::find_item(&ctx, ITEM_POTION, 1).await);
        })
    });
    assert!(!h.log.iter().any(|r| matches!(r, Request::RemoveObject(_))));
    assert!(!h.log.iter().any(|r| matches!(r, Request::ItemAdd { .. })));
    assert!(says(&h, "gText_TooBadBagIsFull"));
    assert_eq!(h.log.last(), Some(&Request::Release));
    assert_eq!(h.var_result(), 0);
}

#[test]
fn item_and_money_commands_set_var_result() {
    for fits in [true, false] {
        let mut h = Host::new(fits);
        run(&mut h, |ctx| {
            Box::pin(async move {
                assert_eq!(prelude::check_item(&ctx, ITEM_POTION, 1).await, fits);
            })
        });
        assert_eq!(h.var_result(), fits as u16);
        let mut h = Host::new(fits);
        run(&mut h, |ctx| {
            Box::pin(async move {
                assert_eq!(prelude::money_check(&ctx, 500).await, fits);
            })
        });
        assert_eq!(h.log[0], Request::MoneyCheck(500));
        assert_eq!(h.var_result(), fits as u16);
    }
}

#[test]
fn multichoice_stores_the_answer_in_var_result() {
    let mut h = Host::new(true);
    h.choice = 127;
    run(&mut h, |ctx| {
        Box::pin(async move {
            assert_eq!(prelude::multichoice(&ctx, 4, 1, 2, 3, true).await, 127);
        })
    });
    assert_eq!(
        h.log[0],
        Request::Multichoice {
            list: 4,
            x: 1,
            y: 2,
            default: 3,
            cancel: true
        }
    );
    assert_eq!(h.var_result(), 127);
}
