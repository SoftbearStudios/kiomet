// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::enum_array::EnumArray;
use crate::field::Field;
use crate::tower::TowerType;
use crate::world::World;
use kodiak_common::bitcode::{self, *};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::{EnumIter, IntoEnumIterator};

/// In priority order.
/// Divided into unit categories.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Encode,
    Decode,
    EnumIter,
    IntoPrimitive,
    TryFromPrimitive,
)]
#[repr(u8)]
pub enum Unit {
    /// Shield is least flexible, so consume it first.
    /// Is [`Unit::FIRST_ALWAYS`] because it's the first unit that's always present.
    Shield,
    /// Fighters need to take out bombers and other fighters before they cause damage.
    /// Is [`Unit::FIRST_MANY`] because it's the first unit that's present in many units.
    Fighter,
    /// Chopper should defend bombers if necessary.
    Chopper,
    /// Bombers need to take out as many ground forces as possible.
    Bomber,
    /// Tanks defend soldiers.
    Tank,
    /// Soldiers are weakest so they fight last.
    Soldier,
    /// Order special units after regular ones.
    /// Is [`Unit::FIRST_SINGLE`] because it's the first unit that's present in single units.
    Shell,
    Emp,
    /// Nuke is a last resort.
    Nuke,
    /// Ruler is a last resort (note: never in the same combatants as nuke).
    /// Is [`Unit::LAST`] because it's the last unit in the enum.
    Ruler,
}

/// [`Unit`]s are into several categories:
///
/// [`UnitCategory::Always`] - always present in units.
///
/// [`UnitCategory::Many`] - present in many units.
///
/// [`UnitCategory::Single`] - present in single units.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UnitCategory {
    Always,
    Many,
    Single,
}

impl UnitCategory {
    // Sizes of categories.
    pub const ALWAYS_COUNT: usize = (Unit::FIRST_MANY as u8 - Unit::FIRST_ALWAYS as u8) as usize;
    pub const MANY_COUNT: usize = (Unit::FIRST_SINGLE as u8 - Unit::FIRST_MANY as u8) as usize;
    // pub const SINGLE_COUNT: usize = (Unit::LAST as u8 + 1 - Unit::FIRST_SINGLE as u8) as usize;
}

pub type UnitArray<V> = EnumArray<Unit, V, { std::mem::variant_count::<Unit>() }>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Speed {
    Immobile,
    Slow,
    Normal,
    Fast,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Range {
    Short,
    Medium,
    Long,
}

impl Range {
    /// Converts this range to a maximum world distance.
    fn to_distance(self) -> u32 {
        (match self {
            Self::Short => 5,
            Self::Medium => 8,
            Self::Long => 11,
        }) * World::MAX_ROAD_LENGTH
    }
}

impl Unit {
    // Categories of units.
    pub const FIRST_ALWAYS: Unit = Unit::Shield;
    pub const FIRST_MANY: Unit = Unit::Fighter;
    pub const FIRST_SINGLE: Unit = Unit::Shell;
    pub const LAST: Unit = Unit::Ruler;
    pub const INFINITE_DAMAGE: u8 = 31; // Units can do 1..=30 damage or infinite.
    pub const EMP_SECONDS: u8 = 60;

    pub(crate) fn category(self) -> UnitCategory {
        if (Unit::FIRST_ALWAYS..Unit::FIRST_MANY).contains(&self) {
            UnitCategory::Always
        } else if (Unit::FIRST_MANY..Unit::FIRST_SINGLE).contains(&self) {
            UnitCategory::Many
        } else {
            // Allows for better codegen than unreachable!() in release mode.
            debug_assert!((Unit::FIRST_SINGLE..=Unit::LAST).contains(&self));
            UnitCategory::Single
        }
    }

    /// Can this unit overflow a tower's capacity (temporarily).
    pub fn max_overflow(self) -> usize {
        match self {
            // For after upgrades and shield generator.
            Self::Shield => 15,
            Self::Soldier => 10,
            Self::Tank => 5,
            Self::Fighter => 4,
            Self::Bomber => 2,
            Self::Chopper => 2,
            _ => 0,
        }
    }

    /// Must be in the range 1..=30 or [`Unit::INFINITE_DAMAGE`].
    /// If equal to [`Unit::INFINITE_DAMAGE`] it signifies infinite damage.
    /// TODO maybe make a custom damage type.
    pub fn damage(self, field: Field, enemy_field: Field) -> u8 {
        match self {
            Self::Tank => 3,
            Self::Fighter if field == Field::Air => 3,
            Self::Bomber if field == Field::Air && enemy_field == Field::Surface => 5,
            // TODO: Should only do 2 air damage
            // (https://discord.com/channels/847143438939717663/933850279537967204/1018971807979688078)
            Self::Chopper if field == Field::Air => 3,
            Self::Nuke => Self::INFINITE_DAMAGE,
            Self::Shell => 3, // TODO shell shouldn't hit regular units.
            _ => 1,
        }
    }

    /// Returns how much damage a unit would do in a force to ground targets.
    /// If equal to [`Unit::INFINITE_DAMAGE`] it signifies infinite damage.
    pub fn force_ground_damage(self) -> u8 {
        self.damage(self.field(false, true, false), Field::Surface)
    }

    /// Converts a damage [`u8`] to an [`u32`] where infinity is represented as [`i32::MAX`].
    pub fn damage_to_finite(damage: u8) -> u32 {
        (damage == Unit::INFINITE_DAMAGE)
            .then_some(i32::MAX as u32)
            .unwrap_or(damage as u32)
    }

    /// Units that can skip over roads have to be single use to prevent territory acquisition.
    pub fn is_single_use(self) -> bool {
        self.ranged_distance().is_some()
    }

    /// Returns the field of a unit.
    /// `any_air` need only be set if `self` can be a [`Unit::Shield`].
    pub fn field(self, overflow: bool, in_force: bool, any_air: bool) -> Field {
        match self {
            Self::Shield if any_air => Field::Air, // Shield's field is max field in group.
            Self::Bomber | Self::Chopper | Self::Fighter | Self::Shell | Self::Emp | Self::Nuke
                if overflow || in_force =>
            {
                Field::Air
            }
            _ => Field::Surface,
        }
    }

    /// Returns true iff this can possibly exist in the given field.
    pub fn is_field_possible(self, field: Field) -> bool {
        for bits in 0u8..8u8 {
            // Test all the cases.
            if self.field((bits >> 2) & 1 == 1, (bits >> 1) & 1 == 1, bits & 1 == 1) == field {
                return true;
            }
        }
        false
    }

    /// Returns [`Some(range)`] of how far this unit can travel between towers, or [`None`] if it
    /// only travels on roads.
    pub fn range(self) -> Option<Range> {
        Some(match self {
            Self::Nuke | Self::Shell => Range::Short,
            Self::Emp => Range::Medium,
            _ => return None,
        })
    }

    /// Returns true if the unit is ranged, i.e. doesn't have to follow roads.
    pub fn is_ranged(self) -> bool {
        self.range().is_some()
    }

    /// Returns an [`Some(u32)`] of how far this unit can travel or [`None`] if only on roads.
    pub fn ranged_distance(self) -> Option<u32> {
        Some(self.range()?.to_distance())
    }

    /// 0 means immobile.
    pub fn speed(self, tower_type: Option<TowerType>) -> Speed {
        match self {
            Self::Bomber | Self::Fighter | Self::Chopper | Self::Shell => Speed::Fast,
            Self::Nuke | Self::Tank => Speed::Slow,
            Self::Shield => {
                if matches!(tower_type, None | Some(TowerType::Projector)) {
                    Speed::Fast
                } else {
                    Speed::Immobile
                }
            }
            _ => Speed::Normal,
        }
    }

    pub fn weight(self) -> u8 {
        match self {
            Self::Tank => 2,
            Self::Soldier => 1,
            _ => 0,
        }
    }

    pub fn is_mobile(self, tower_type: Option<TowerType>) -> bool {
        self.speed(tower_type) != Speed::Immobile
    }

    pub fn can_capture(self) -> bool {
        self.is_mobile(None) && self != Self::Shield && !self.is_single_use()
    }

    pub fn iter() -> impl Iterator<Item = Self> + DoubleEndedIterator + Clone + 'static {
        <Self as IntoEnumIterator>::iter()
    }

    /// Ideally would use range exclusive for this but it's complicated to implement Step.
    pub fn iter_to(self, end: Self) -> impl Iterator<Item = Self> + DoubleEndedIterator {
        ((self as u8)..(end as u8)).map(|i| i.try_into().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::unit::{Unit, UnitCategory};

    #[test]
    fn serialized_size() {
        serialized_size_enum!(Unit);
    }

    #[test]
    fn category() {
        assert_eq!(Unit::Shield.category(), UnitCategory::Always);
        assert_eq!(Unit::Fighter.category(), UnitCategory::Many);
        assert_eq!(Unit::Chopper.category(), UnitCategory::Many);
        assert_eq!(Unit::Bomber.category(), UnitCategory::Many);
        assert_eq!(Unit::Tank.category(), UnitCategory::Many);
        assert_eq!(Unit::Soldier.category(), UnitCategory::Many);
        assert_eq!(Unit::Shell.category(), UnitCategory::Single);
        assert_eq!(Unit::Emp.category(), UnitCategory::Single);
        assert_eq!(Unit::Nuke.category(), UnitCategory::Single);
        assert_eq!(Unit::Ruler.category(), UnitCategory::Single);
    }
}
