use super::*;

#[test]
fn block_fields() {
    let b = 0x3000 | 0x0400 | 0x01D5;
    assert_eq!(block_metatile(b), 0x1D5);
    assert_eq!(block_collision(b), 1);
    assert_eq!(block_elevation(b), 3);
}

#[test]
fn attr_fields() {
    assert_eq!(attr_behavior(0x1023), 0x23);
    assert_eq!(attr_layer(0x0000), LayerType::Normal);
    assert_eq!(attr_layer(0x1000), LayerType::Covered);
    assert_eq!(attr_layer(0x2000), LayerType::Split);
}

#[test]
fn dir_delta_and_opposite() {
    for d in Dir::ALL {
        let (a, b) = (d.delta(), d.opposite().delta());
        assert_eq!((a.0 + b.0, a.1 + b.1), (0, 0));
    }
}
