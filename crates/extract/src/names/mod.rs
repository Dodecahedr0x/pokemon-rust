//! Decomp identifiers (constant names) for Gen3 numbers, so exported files can say
//! `FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN` instead of `0x2c`.
//!
//! The tables live in `generated.rs`, written from the decomp sources and guarded by the drift
//! test `tests/names.rs` (`PKR_BLESS=1` regenerates them). Each table can be read by number
//! ([`Table::name`]) and by name ([`Table::value`]). When several names share a number (aliases
//! such as `VAR_TEMP_CHALLENGE_STATUS` = `VAR_TEMP_0`), lookup by number gives the primary one:
//! the first in source order, skipping range bases like `FLAG_HIDDEN_ITEMS_START`.

#[rustfmt::skip]
mod battle;
#[rustfmt::skip]
mod generated;

pub use battle::*;
pub use generated::*;

/// A `name <-> number` table.
pub struct Table {
    /// Common prefix of the names, used for the `<prefix>0x<hex>` fallback of unnamed numbers.
    pub prefix: &'static str,
    /// Sorted by value; within one value the primary name comes first.
    pub entries: &'static [(u32, &'static str)],
}

impl Table {
    /// The primary name of `value`.
    pub fn name(&self, value: u32) -> Option<&'static str> {
        let i = self.entries.partition_point(|&(v, _)| v < value);
        self.entries
            .get(i)
            .filter(|&&(v, _)| v == value)
            .map(|&(_, n)| n)
    }

    /// The primary name of `value`, or `<prefix>0x<HEX>` (`FLAG_0x123`) if it has none.
    pub fn name_or_hex(&self, value: u32) -> String {
        match self.name(value) {
            Some(n) => n.to_string(),
            None => format!("{}0x{value:X}", self.prefix),
        }
    }

    /// The value of any name in the table (primary or alias). Also parses the
    /// `<prefix>0x<HEX>` fallback form.
    pub fn value(&self, name: &str) -> Option<u32> {
        if let Some(&(v, _)) = self.entries.iter().find(|&&(_, n)| n == name) {
            return Some(v);
        }
        let hex = name.strip_prefix(self.prefix)?.strip_prefix("0x")?;
        u32::from_str_radix(hex, 16).ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &'static str)> {
        self.entries.iter().copied()
    }
}

/// One map of `gMapGroups`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapName {
    pub group: u8,
    pub num: u8,
    /// Decomp map directory name (`LittlerootTown`): the `<Map>` of exported file names.
    pub dir: &'static str,
    /// `MAP_LITTLEROOT_TOWN`.
    pub constant: &'static str,
}

/// `MAP_DYNAMIC` as `(group, num)`: warps whose destination is set at run time.
pub const MAP_DYNAMIC: (u8, u8) = (0x7F, 0x7F);

pub fn map(group: u8, num: u8) -> Option<&'static MapName> {
    MAPS.iter().find(|m| m.group == group && m.num == num)
}

pub fn map_by_dir(dir: &str) -> Option<&'static MapName> {
    MAPS.iter().find(|m| m.dir == dir)
}

pub fn map_by_constant(constant: &str) -> Option<&'static MapName> {
    MAPS.iter().find(|m| m.constant == constant)
}

/// Number of maps in each group of `gMapGroups`.
pub fn group_sizes() -> Vec<u8> {
    let mut sizes = Vec::new();
    for m in MAPS {
        if sizes.len() <= m.group as usize {
            sizes.resize(m.group as usize + 1, 0);
        }
        sizes[m.group as usize] += 1;
    }
    sizes
}

pub fn flag_name(id: u16) -> String {
    FLAGS.name_or_hex(id.into())
}

pub fn var_name(id: u16) -> String {
    VARS.name_or_hex(id.into())
}

pub fn gfx_name(id: u8) -> String {
    OBJ_EVENT_GFX.name_or_hex(id.into())
}

pub fn movement_type_name(id: u8) -> String {
    MOVEMENT_TYPES.name_or_hex(id.into())
}

pub fn movement_action_name(id: u8) -> String {
    MOVEMENT_ACTIONS.name_or_hex(id.into())
}

pub fn trainer_type_name(id: u16) -> String {
    TRAINER_TYPES.name_or_hex(id.into())
}

pub fn item_name(id: u16) -> String {
    ITEMS.name_or_hex(id.into())
}

pub fn behavior_name(id: u8) -> String {
    METATILE_BEHAVIORS.name_or_hex(id.into())
}

pub fn special_name(id: u16) -> String {
    SPECIALS.name_or_hex(id.into())
}

/// `callstd` index: `STD_*` or `MSGBOX_*`.
pub fn std_name(id: u8) -> String {
    STD_SCRIPTS.name_or_hex(id.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    static T: Table = Table {
        prefix: "X_",
        entries: &[(1, "X_ONE"), (1, "X_UNO"), (5, "X_FIVE")],
    };

    #[test]
    fn table_lookups() {
        assert_eq!(T.name(1), Some("X_ONE"));
        assert_eq!(T.name(2), None);
        assert_eq!(T.name(5), Some("X_FIVE"));
        assert_eq!(T.name(6), None);
        assert_eq!(T.name_or_hex(0x1AB), "X_0x1AB");
        assert_eq!(T.value("X_UNO"), Some(1));
        assert_eq!(T.value("X_0x1AB"), Some(0x1AB));
        assert_eq!(T.value("Y_0x1"), None);
    }

    #[test]
    fn tables_are_sorted() {
        for t in [
            &FLAGS,
            &VARS,
            &OBJ_EVENT_GFX,
            &MOVEMENT_TYPES,
            &MOVEMENT_ACTIONS,
            &TRAINER_TYPES,
            &ITEMS,
            &METATILE_BEHAVIORS,
            &SPECIALS,
            &STD_SCRIPTS,
            &MSGBOX_TYPES,
            &TILESETS,
            &SPECIES,
            &TRAINER_PICS,
            &TRAINER_BACK_PICS,
        ] {
            assert!(
                t.entries.windows(2).all(|w| w[0].0 <= w[1].0),
                "{}",
                t.prefix
            );
        }
        assert_eq!(group_sizes().iter().map(|&s| s as u32).sum::<u32>(), 518);
    }
}
