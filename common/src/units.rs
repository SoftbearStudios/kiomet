// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::{Tower, TowerType};
use crate::unit::{Unit, UnitCategory};
use kodiak_common::bitcode::{self, *};
use std::fmt::Formatter;

#[derive(Clone, Default, Hash, PartialEq, Eq, Encode, Decode)]
pub struct Units {
    always: [u8; UnitCategory::ALWAYS_COUNT],
    either: UnitsEither,
}

#[derive(Clone, Hash, PartialEq, Eq, Encode, Decode)]
enum UnitsEither {
    Many([u8; UnitCategory::MANY_COUNT]),
    Single(Unit, u8), // The u8 has to be non-zero.
}

impl Default for UnitsEither {
    fn default() -> Self {
        Self::Many(Default::default())
    }
}

impl UnitsEither {
    fn many(&self) -> Option<[u8; UnitCategory::MANY_COUNT]> {
        if let Self::Many(many) = *self {
            Some(many)
        } else {
            None
        }
    }

    fn single(&self) -> Option<(Unit, u8)> {
        if let Self::Single(unit, count) = *self {
            Some((unit, count))
        } else {
            None
        }
    }
}

// TODO serde.

impl Units {
    pub const CAPACITY: usize = u8::MAX as usize;

    pub(crate) fn is_many(&self) -> bool {
        matches!(self.either, UnitsEither::Many(_))
    }

    /// Returns amount added.
    fn add_inner(
        &mut self,
        unit: Unit,
        count: usize,
        tower_type: Option<TowerType>,
        overflow: bool,
    ) -> usize {
        // debug_assert_ne!(count, 0);

        let added = count.min(self.space_remaining(unit, tower_type, overflow));
        if unit == Unit::Ruler && count != 0 {
            debug_assert_eq!(added, 1, "could not add ruler to {:?}", tower_type);
        }
        if added == 0 {
            return 0;
        }

        let unit_u8 = unit as u8;
        let added_u8 = added as u8;

        match unit.category() {
            UnitCategory::Always => {
                self.always[(unit_u8 - Unit::FIRST_ALWAYS as u8) as usize] += added_u8;
            }
            UnitCategory::Many => {
                if let UnitsEither::Many(many) = &mut self.either {
                    many[(unit_u8 - Unit::FIRST_MANY as u8) as usize] += added_u8;
                } else {
                    unreachable!()
                }
            }
            UnitCategory::Single => {
                if matches!(self.either, UnitsEither::Many(_)) {
                    // If added is 0 this fails because it deletes many units to replace with 0 single.
                    // 0 single is an invalid state because it should be 0 many.
                    debug_assert_ne!(added, 0);
                    self.either = UnitsEither::Single(unit, added_u8)
                } else {
                    if let UnitsEither::Single(u, c) = &mut self.either {
                        debug_assert!(unit >= *u);
                        debug_assert!(*u != Unit::Ruler);
                        if *u == unit {
                            // Add to same unit.
                            *c += added_u8;
                        } else {
                            // Set new unit.
                            *u = unit;
                            *c = added_u8
                        }
                    }
                }
            }
        }
        added
    }

    fn index(&self, unit: Unit) -> u8 {
        match unit.category() {
            UnitCategory::Always => self.always[(unit as u8 - Unit::FIRST_ALWAYS as u8) as usize],
            UnitCategory::Many => self.either.many().map_or(0, |many| {
                many[(unit as u8 - Unit::FIRST_MANY as u8) as usize]
            }),
            UnitCategory::Single => self.either.single().map_or(0, |(u, count)| {
                (unit == u).then_some(count).unwrap_or_default()
            }),
        }
    }

    /// Returns the amount of a unit in the units.
    pub fn available(&self, unit: Unit) -> usize {
        self.index(unit) as usize
    }

    /// Removes all units.
    pub fn clear(&mut self) {
        *self = Self::default();
        debug_assert!(self.is_empty());
    }

    /// Returns true if [`Self::len`] returns zero.
    pub fn is_empty(&self) -> bool {
        // Subtracting down to 0 single units or clearing must always set to default.
        self == &Self::default()
    }

    /// Iterates all unit types, WHILE including the ones with zero counts.
    /// You can use this when you need a DoubleEndedIterator.
    pub fn iter_with_zeros(&self) -> impl Iterator<Item = (Unit, usize)> + DoubleEndedIterator {
        let cloned = self.clone();
        Unit::iter().map(move |unit| (unit, cloned.available(unit)))
    }

    /// Iterates all unit types, NOT including the ones with zero counts.
    pub fn iter(&self) -> impl Iterator<Item = (Unit, usize)> {
        self.iter_with_zeros().filter(|(_, c)| *c != 0)
    }

    /// Takes into account the ruler boost.
    pub fn capacity(&self, unit: Unit, tower_type: Option<TowerType>) -> usize {
        Units::CAPACITY.min(
            tower_type
                .map(|t| {
                    t.raw_unit_capacity(unit).saturating_add(
                        if unit == Unit::Shield && self.has_ruler() {
                            Tower::RULER_SHIELD_BOOST
                        } else {
                            0
                        },
                    )
                })
                .unwrap_or(usize::MAX),
        )
    }

    /// How much space is remaining for this unit type.
    fn space_remaining(&self, unit: Unit, tower_type: Option<TowerType>, overflow: bool) -> usize {
        match unit.category() {
            UnitCategory::Always => (),
            UnitCategory::Many => {
                // Many can't override single.
                // Single can never be zero.
                if self.either.single().is_some() {
                    return 0;
                }
            }
            UnitCategory::Single => {
                // Singles can't override other singles of higher priority.
                if self.either.single().is_some_and(|(u, _)| u > unit) {
                    return 0;
                }
            }
        }

        self.capacity(unit, tower_type)
            .saturating_add(if overflow { unit.max_overflow() } else { 0 })
            .saturating_sub(self.available(unit))
    }

    /// Subtracts up to `count` of `unit` and returns amount subtracted.
    pub fn subtract(&mut self, unit: Unit, count: usize) -> usize {
        // debug_assert_ne!(count, 0);

        let available = self.available(unit);
        let subtracted = count.min(available);
        if subtracted == 0 {
            return 0;
        }
        let unit_u8 = unit as u8;
        let subtracted_u8 = subtracted as u8;

        match unit.category() {
            UnitCategory::Always => {
                self.always[(unit_u8 - Unit::FIRST_ALWAYS as u8) as usize] -= subtracted_u8;
            }
            UnitCategory::Many => {
                if let UnitsEither::Many(many) = &mut self.either {
                    many[(unit_u8 - Unit::FIRST_MANY as u8) as usize] -= subtracted_u8;
                } else {
                    // Has to be many if it's available.
                    unreachable!();
                }
            }
            UnitCategory::Single => {
                let is_zero = if let UnitsEither::Single(u, c) = &mut self.either {
                    debug_assert_eq!(unit, *u);
                    *c -= subtracted_u8;
                    *c == 0
                } else {
                    // Has to be single if it's available.
                    unreachable!();
                };

                // Zero single is an invalid state so set to default (zero many).
                if is_zero {
                    self.either = UnitsEither::default();
                }
            }
        }
        debug_assert_eq!(available - subtracted, self.available(unit));
        subtracted
    }
}

// The impl below only calls the impl above so it doesn't care about the impl details of units.
impl Units {
    /// Adds a unit to a non-tower and returns amount added.
    pub fn add(&mut self, unit: Unit, count: usize) -> usize {
        self.add_inner(unit, count, None, true)
    }

    /// Adds a unit to a tower and returns amount added.
    /// Overflow means added units are allowed to circle tower if tower is already full.
    pub fn add_to_tower(
        &mut self,
        unit: Unit,
        count: usize,
        tower_type: TowerType,
        overflow: bool,
    ) -> usize {
        self.add_inner(unit, count, Some(tower_type), overflow)
    }

    pub fn contains(&self, unit: Unit) -> bool {
        self.available(unit) > 0
    }

    /// Returns true if the units contain a ruler.
    pub fn has_ruler(&self) -> bool {
        self.contains(Unit::Ruler)
    }

    /// Adds units to a tower's units that aren't single use.
    pub fn add_units_to_tower(&mut self, other: Self, tower_type: TowerType, has_player: bool) {
        for (unit, removed) in other {
            if unit.is_single_use() {
                continue;
            }

            let added = self.add_to_tower(unit, removed, tower_type, has_player);
            debug_assert!(added <= removed);
        }
    }

    /// Reconciles the units with a new tower type.
    pub fn reconcile(&mut self, tower_type: TowerType, has_player: bool) {
        let old_units = std::mem::take(self);
        self.add_units_to_tower(old_units, tower_type, has_player);
    }

    /// Returns the total number of units.
    pub fn len(&self) -> usize {
        self.iter().map(|(_, i)| i).sum()
    }

    /// Can claim towers i.e. not [`Unit::Shell`] or [`Unit::Nuke`].
    pub fn is_alive(&self) -> bool {
        self.iter()
            .any(|(u, _)| !(u.is_single_use() || u == Unit::Shield))
    }

    /// Returns the max edge distance of mobile units.
    /// Don't call on any non mobile units.
    pub fn max_edge_distance(&self) -> Option<u32> {
        self.iter()
            .inspect(|(unit, _)| debug_assert!(unit.is_mobile(None), "don't use on non mobile"))
            .map(|(unit, _)| unit.ranged_distance())
            .fold(None, |mut acc, item| {
                let prev = acc.get_or_insert(item);
                debug_assert_eq!(*prev, item); // Units must all have the same max_edge_distance.
                acc
            })
            .flatten()
    }

    /// Returns random units of a specified damage with a given seed.
    pub fn random_units(mut damage: u32, allow_nuke: bool, mut seed: u16) -> Self {
        let mut units = Units::default();

        for unit in [Unit::Soldier, Unit::Tank, Unit::Bomber, Unit::Nuke] {
            if !(allow_nuke || unit != Unit::Nuke || seed & 0b11 == 0) {
                // Sending (too many) zombie nukes is problematic since they don't capture, and now
                // prevent other zombie units. Only send them 1/4 of the time even if allowed.
                continue;
            }
            let mut governor = 3 + (seed & 0b111);
            seed >>= 3;
            while damage > 0 && governor > 0 {
                units.add(unit, 1);
                damage = damage.saturating_sub(Unit::damage_to_finite(unit.force_ground_damage()));
                governor -= 1;
            }
        }

        units
    }
}

impl std::fmt::Debug for Units {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl IntoIterator for Units {
    type Item = (Unit, usize);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::tower::TowerType;
    use crate::unit::Unit;
    use crate::units::Units;
    use kodiak_common::rand::prelude::IteratorRandom;
    use kodiak_common::rand::{thread_rng, Rng};

    #[test]
    fn size_of() {
        size_of!(Units)
    }

    #[test]
    fn serialized_size() {
        serialized_size_value!("Units(default)", Units::default());

        let mut units = Units::default();
        assert_eq!(units.add(Unit::Soldier, 1), 1);
        serialized_size_value!("Units(many)", units);

        let mut units = Units::default();
        assert_eq!(units.add(Unit::Nuke, 1), 1);
        serialized_size_value!("Units(single)", units);
    }

    #[test]
    fn ruler() {
        let mut units = Units::default();

        assert_eq!(units.add(Unit::Soldier, 5), 5);
        assert_eq!(units.add(Unit::Ruler, 1), 1);
        assert_eq!(units.available(Unit::Soldier), 0);
    }

    #[test]
    fn artillery() {
        let mut units = Units::default();

        // Artillery can't hold soldiers without overflowing.
        assert_eq!(
            units.add_to_tower(Unit::Soldier, 3, TowerType::Artillery, false),
            0
        );

        // Artillery can hold soldiers while overflowing.
        assert_eq!(
            units.add_to_tower(Unit::Soldier, 3, TowerType::Artillery, true),
            3
        );

        // Adding any shells with delete the soldiers.
        assert_eq!(
            units.add_to_tower(Unit::Shell, 2, TowerType::Artillery, false),
            2
        );
        assert_eq!(units.available(Unit::Soldier), 0);
    }

    #[test]
    fn fuzz() {
        let mut rng = thread_rng();
        for _ in 0..1000 {
            let mut units = Units::default();
            for _ in 0..20 {
                let unit = Unit::iter().choose(&mut rng).unwrap();
                let count = if unit == Unit::Ruler {
                    1
                } else {
                    rng.gen_range(0..24)
                };
                match rng.gen_range(0..=6) {
                    0 if unit != Unit::Ruler || !units.has_ruler() => {
                        if !(unit == Unit::Shield
                            && (units.either.single().is_some() && !units.has_ruler()))
                        {
                            units.add(unit, count);
                        }
                    }
                    1 if unit != Unit::Ruler || !units.has_ruler() => {
                        units.add_to_tower(unit, count, rng.gen(), rng.gen());
                    }
                    2 => {
                        units.subtract(unit, count);
                    }
                    3 if units
                        .iter()
                        .all(|(u, _)| u.is_mobile(None) && u != Unit::Shield) =>
                    {
                        units.max_edge_distance();
                    }
                    4 => {
                        units.available(unit);
                    }
                    5 => {
                        units.contains(unit);
                    }
                    _ => {
                        units.iter().count();
                    }
                }
            }
        }
    }
}
