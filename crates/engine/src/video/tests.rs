use super::*;

#[test]
fn obj_dimensions_round_trip() {
    for shape in [ObjShape::Square, ObjShape::Wide, ObjShape::Tall] {
        for size in 0..4 {
            let (w, h) = Obj::dimensions(shape, size);
            assert_eq!(Obj::shape_for(w, h), Some((shape, size)));
        }
    }
    assert_eq!(Obj::dimensions(ObjShape::Tall, 2), (16, 32));
    assert_eq!(Obj::shape_for(16, 32), Some((ObjShape::Tall, 2)));
    assert_eq!(Obj::shape_for(24, 24), None);
}

#[test]
fn validate_checks_ranges() {
    static T: [Tile; 2] = [[0; 8]; 2];
    let pal = |bank| Cmd::BgPalette {
        bank,
        colors: [0; 16],
    };
    assert!(pal(15).validate().is_ok());
    assert!(pal(16).validate().is_err());
    let bg = |char_block, first| Cmd::BgTiles {
        char_block,
        first,
        tiles: &T,
    };
    assert!(bg(0, 510).validate().is_ok()); // runs on into char block 1
    assert!(bg(3, 510).validate().is_ok());
    assert!(bg(3, 511).validate().is_err());
    assert!(bg(4, 0).validate().is_err());
    let obj = |first| Cmd::ObjTiles { first, tiles: &T };
    assert!(obj(1022).validate().is_ok());
    assert!(obj(1023).validate().is_err());
    let one = |index| Cmd::ObjTile {
        index,
        tile: [0; 8],
    };
    assert!(one(1023).validate().is_ok());
    assert!(one(1024).validate().is_err());
    let map = |screen_block, start, n| Cmd::MapEntries {
        screen_block,
        start,
        entries: alloc::vec![0; n],
    };
    assert!(map(31, 0, 1024).validate().is_ok());
    assert!(map(31, 1, 1024).validate().is_err());
    assert!(map(32, 0, 1).validate().is_err());
    let ctl = |layer, priority| Cmd::Bg {
        layer,
        control: BgControl {
            enabled: true,
            priority,
            char_block: 0,
            screen_block: 0,
        },
    };
    assert!(ctl(3, 3).validate().is_ok());
    assert!(ctl(4, 0).validate().is_err());
    assert!(ctl(0, 4).validate().is_err());
    let mut f = Frame::default();
    f.objs.push(Obj::default());
    assert!(f.validate().is_ok());
    f.objs[0].palette = 16;
    assert!(f.validate().is_err());
    f.objs = alloc::vec![Obj::default(); MAX_OBJS + 1];
    assert!(f.validate().is_err());
}

#[test]
fn only_full_darkening_is_black() {
    let mut f = Frame::default();
    assert!(!f.is_black());
    f.brightness = BRIGHTNESS_BLACK;
    assert!(f.is_black());
    f.lighten = true;
    assert!(!f.is_black());
}

#[test]
fn battle_layout_fits_vram() {
    use layout::battle::*;
    let bgs = [TEXT, WINDOW, ANIM, ENV];
    for (i, &(layer, cb, sb, prio)) in bgs.iter().enumerate() {
        assert_eq!(layer as usize, i);
        assert!(cb < 4 && sb < 32 && prio < 4);
        // Screen blocks sit above every char block in use (each char block holds at most
        // 512 tiles = 8 screen blocks).
        for &(_, other_cb, ..) in &bgs {
            assert!(
                sb / 8 != other_cb,
                "screen block {sb} inside char block {other_cb}"
            );
        }
    }
}
