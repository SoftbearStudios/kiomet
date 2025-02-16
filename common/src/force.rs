// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ticks::Ticks;
use crate::tower::{TowerId, TowerType};
use crate::unit::{Speed, Unit};
use crate::units::Units;
use crate::world::{World, WorldChunks};
use kodiak_common::bitcode::{self, *};
use kodiak_common::glam::Vec2;
use kodiak_common::PlayerId;

/// Represents a path that a force can take.
/// TODO optimize to inline 8 bytes (3 bits per segment (19 segments) + 7 bits control).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Path {
    path: Vec<TowerId>, // In reverse order of input.
}

impl Path {
    pub fn new(mut path: Vec<TowerId>) -> Self {
        assert!(path.len() >= 2);
        // Reverse path so we can pop front efficiently.
        path.reverse();
        Self { path }
    }

    /// Validates a path and returns Ok with a valid [`Path`] or Err with a [`str`] error.
    pub fn validate(
        self,
        towers: &WorldChunks,
        source_tower_id: TowerId,
        max_edge_distance: Option<u32>,
    ) -> Result<Self, &'static str> {
        if self.path.len() < 2 {
            return Err("path too short");
        }

        // Direct paths Some(max_edge_distance) must be 2, path must be < max path roads.
        if (max_edge_distance.is_some() && self.path.len() != 2)
            || (max_edge_distance.is_none() && self.path.len() > World::MAX_PATH_ROADS)
        {
            return Err("path too long");
        }

        // Path reversed in constructor.
        let mut iter = self.path.iter().rev();
        if iter.next() != Some(&source_tower_id) {
            return Err("source mismatch");
        }

        let max_distance_squared = max_edge_distance.map(|d| (d as u64 + 1).pow(2) - 1);

        let mut prev = source_tower_id;
        for &next in iter {
            if next == prev {
                return Err("duplicate tower in path");
            }
            if !WorldChunks::RECTANGLE.contains(next) {
                return Err("outside world");
            }

            if let Some(max_distance_squared) = max_distance_squared {
                if prev.distance_squared(next) > max_distance_squared {
                    return Err("edge too long");
                }
            } else if !prev.is_neighbor(next) {
                return Err("not neighbor");
            }

            if !towers.contains(next) {
                return Err("not generated");
            }
            prev = next;
        }

        Ok(self)
    }

    /// Returns where the force is coming from.
    /// TODO will require current tower id as input once optimized to 8 bytes.
    pub fn coming_from(&self) -> TowerId {
        *self.path.last().unwrap()
    }

    /// Returns where the force is going to.
    /// TODO will require current tower id as input once optimized to 8 bytes.
    pub fn going_to(&self) -> TowerId {
        *self.path.iter().nth_back(1).unwrap()
    }

    /// Iterates the towers in order of first to last.
    /// TODO will require current tower id as input once optimized to 8 bytes.
    pub fn iter(&self) -> impl Iterator<Item = TowerId> + '_ {
        self.path.iter().copied().rev()
    }

    /// Where the path starts.
    pub fn source(&self) -> TowerId {
        self.iter().next().unwrap()
    }

    /// Where the path ends. Not necessarily efficient.
    pub fn destination(&self) -> TowerId {
        self.iter().last().unwrap()
    }

    /// Pops a TowerId off the path signifying that it was reached.
    fn pop(&mut self) {
        self.path.pop().unwrap();
    }

    /// Returns if the path is empty (contains no segments).
    fn is_empty(&self) -> bool {
        self.path.len() < 2
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Encode, Decode)]
pub struct Force {
    /// Invariant: Always has at least two items, most likely source and destination.
    path: Path,
    #[doc(hidden)]
    pub path_progress: u8,
    pub fuel: u8,
    /// If [`None`], they can kill but not capture (e.g. for shrinking world).
    pub player_id: Option<PlayerId>,
    pub units: Units,
}

impl Force {
    #[doc(hidden)]
    pub fn new_inner(player_id: Option<PlayerId>, units: Units, path: Path) -> Self {
        debug_assert!(!units.is_empty());
        // If no player id, must not have ruler.
        debug_assert!(player_id.is_some() || units.available(Unit::Ruler) == 0);

        Self {
            path,
            path_progress: 0,
            fuel: 150,
            player_id,
            units,
        }
    }

    pub fn new(player_id: PlayerId, units: Units, path: Path) -> Self {
        Self::new_inner(Some(player_id), units, path)
    }

    /// Returns where the force is coming from.
    pub fn current_source(&self) -> TowerId {
        self.path.coming_from()
    }

    /// Returns where the force is going to.
    pub fn current_destination(&self) -> TowerId {
        self.path.going_to()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn interpolated_position(&self, time_since_tick: f32) -> Vec2 {
        let source = self.current_source().as_vec2();
        let destination = self.current_destination().as_vec2();
        source.lerp(
            destination,
            ((self.path_progress as f32
                + time_since_tick * (1.0 / Ticks::PERIOD_SECS) * self.progress_per_tick() as f32)
                / self.progress_required() as f32)
                .min(1.0),
        )
    }

    /// Force will arrive at current destination but not continue.
    pub fn halt(&mut self) {
        self.path = Path::new(self.path.iter().take(2).collect());
    }

    /// Equivalent to `force.clone().halt()` but more efficient.
    pub fn halted(&self) -> Self {
        let &Self {
            path_progress,
            fuel,
            player_id,
            ..
        } = self;
        let path = Path::new(self.path.iter().take(2).collect());
        let units = self.units.clone();
        Self {
            path,
            path_progress,
            fuel,
            player_id,
            units,
        }
    }

    /// Returns true if the force moved on (still has more path to follow).
    pub fn try_move_on(
        &mut self,
        tower_type: TowerType,
        tower_units: &mut Units,
        ally: Option<PlayerId>,
        supply_line: Option<&Path>,
    ) -> bool {
        if self.path.is_empty() || ally.is_some() {
            if let Some(supply_line) = supply_line
                && tower_type.ranged_distance().is_none()
                && self.units.is_many()
            {
                self.path = supply_line.clone();
                if let Some(ally) = ally {
                    self.player_id = Some(ally);
                } else {
                    if tower_type == TowerType::Projector {
                        let max_shield = TowerType::Projector.raw_unit_capacity(Unit::Shield);
                        let existing_shield = self.units.available(Unit::Shield);
                        if let Some(max_transfer) = max_shield.checked_sub(existing_shield) {
                            let transferred = tower_units.subtract(Unit::Shield, max_transfer);
                            self.units.add(Unit::Shield, transferred);
                        }
                    }
                    if self.units.contains(Unit::Chopper) {
                        let initial_speed = self.speed();
                        for transfer in [Unit::Soldier, Unit::Tank] {
                            if transfer.speed(None) >= initial_speed {
                                // Don't balloon the force.
                                break;
                            }
                            loop {
                                if !tower_units.contains(transfer) {
                                    // Ran out.
                                    break;
                                }
                                let t = self.units.add(transfer, 1);
                                if t == 0 {
                                    // Didn't have room in force?
                                    debug_assert!(false);
                                    break;
                                }
                                if self.speed() < initial_speed {
                                    // Over capacity (undo).
                                    let t = self.units.subtract(transfer, 1);
                                    debug_assert_eq!(t, 1);
                                    break;
                                }
                                let t = tower_units.subtract(transfer, 1);
                                debug_assert_eq!(t, 1);
                            }
                        }
                    }
                    // TODO: bring units along.
                    let _ = tower_units;
                }
            } else {
                return false;
            }
        }

        // Force moving on.
        self.path_progress = 0;
        if self.units.is_many() {
            self.fuel -= 1;
        }
        true
    }

    pub(crate) fn progress_per_tick(&self) -> u8 {
        match self.speed() {
            Speed::Immobile => {
                debug_assert!(false, "will never make progress");
                0
            }
            Speed::Slow => 1,
            Speed::Normal => 2,
            Speed::Fast => 3,
        }
    }

    pub fn progress_required(&self) -> u8 {
        let distance = self.current_source().distance(self.current_destination());
        // The constant controls the speed. 255 was the original value, and 180 is about 40% faster.
        (distance * 180 / World::MAX_ROAD_LENGTH / 2).min(u8::MAX as u32) as u8
    }

    fn speed(&self) -> Speed {
        let choppers = self.units.available(Unit::Chopper) as u8;
        if choppers != 0 {
            let weight: u32 = self
                .units
                .iter()
                .map(|(u, c)| u.weight() as u32 * c as u32)
                .sum();
            let max_weight = (choppers as u32) * 4;

            return if weight <= max_weight {
                Speed::Fast
            } else {
                // Choppers can't carry everything so carry the slowest things.
                let slow_weight: u32 = self
                    .units
                    .iter()
                    .filter_map(|(u, c)| {
                        (u.speed(None) < Speed::Normal).then(|| u.weight() as u32 * c as u32)
                    })
                    .sum();
                if slow_weight <= max_weight {
                    Speed::Normal
                } else {
                    Speed::Slow
                }
            };
        }

        if let Some(speed) = self
            .units
            .iter()
            .map(|(u, _)| {
                debug_assert!(u.is_mobile(None));
                u.speed(None)
            })
            .min()
        {
            speed
        } else {
            debug_assert!(false, "no units {:?}", self);

            // Hide the evidence.
            Speed::Fast
        }
    }

    pub(crate) fn raw_tick(&mut self, assert_current_source_equals: Option<TowerId>) -> bool {
        // Arriving if progress per tick reaches progress required.
        self.path_progress = self.path_progress.saturating_add(self.progress_per_tick());

        if self.path_progress >= self.progress_required() {
            // Mark arrived so next tick is leaving.
            self.path.pop();
            if let Some(inbound_tower_id) = assert_current_source_equals {
                assert_eq!(self.current_source(), inbound_tower_id);
            }
            true
        } else {
            false
        }
    }

    /// Advances a force by one tick and returns true if the force is arriving or is leaving.
    pub(crate) fn tick(&mut self, inbound_tower_id: TowerId) -> bool {
        debug_assert_ne!(self.current_source(), inbound_tower_id);
        self.raw_tick(Some(inbound_tower_id))
    }
}

#[cfg(test)]
mod tests {
    use crate::force::{Force, Path};
    use crate::tower::TowerId;
    use crate::unit::{Speed, Unit};
    use crate::units::Units;
    use kodiak_common::PlayerId;

    #[test]
    fn size_of() {
        size_of!(Force);
    }

    #[test]
    fn chopper_carry() {
        let path = Path::new(vec![TowerId::new(0, 0), TowerId::new(0, 1)]);
        let mut units = Units::default();
        units.add(Unit::Chopper, 2);
        let mut force = Box::new(Force::new(PlayerId::SOLO_OFFLINE, units, path));

        // 2 choppers can carry 4 tanks fast.
        force.units.add(Unit::Tank, 4);
        assert_eq!(force.speed(), Speed::Fast);

        // 2 choppers can carry 4 tanks fast but force is limited to normal because soldiers.
        force.units.add(Unit::Soldier, 4);
        assert_eq!(force.speed(), Speed::Normal);

        // 2 choppers can't carry 5 tanks so force is slow.
        force.units.add(Unit::Tank, 1);
        assert_eq!(force.speed(), Speed::Slow);

        // 2 choppers can carry 2 tanks and 4 soldiers fast.
        force.units.subtract(Unit::Tank, 3);
        assert_eq!(force.speed(), Speed::Fast);
    }
}
