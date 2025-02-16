// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::field::Field;
use common::force::Force;
use common::tower::{Tower, TowerType};
use common::unit::Unit;
use common::units::Units;
use kodiak_client::glam::Vec2;
use std::f32::consts::{PI, TAU};
use std::iter;

/// Maximum number of columns, unless the number of unit types is even larger, becoming the new
/// max columns, and reducing the icon scale.
const MAX_COLUMNS: usize = 6;

/// Rust discord:
/// https://discord.com/channels/273534239310479360/993374519081312268/993375510732218379
#[repr(align(8))]
struct Align8<T: ?Sized>(pub T);

const UNIT_FORMATION_BYTES: &Align8<[u8]> = &Align8(*include_bytes!(concat!(
    env!("OUT_DIR"),
    "/unit_formation.bin"
)));

pub fn tower_layout(tower: &Tower, time: f32) -> impl Iterator<Item = UnitLayout> + '_ {
    use TowerType::*;
    let vertical_offset = match tower.tower_type {
        // Short towers need to offset their units higher.
        Runway | Silo => -0.2,
        Barracks | Factory | Airfield | Village | Town => -0.3,
        // Tall towers need to offset their units lower.
        City | Launcher | Reactor | Rocket => -0.5,
        // Large towers need a large offset.
        //Capitol => -0.6,
        //Icbm => -0.7,
        //Metropolis => -0.8,
        //Laser => -0.9,
        _ => -0.4,
    };

    let offset = Vec2::new(0.0, vertical_offset);

    let mut grid_units = Units::default();
    let mut orbit_units = Units::default();
    for (unit, count) in tower.units.iter() {
        if is_special(unit) {
            continue;
        }
        let cap = tower.units.capacity(unit, Some(tower.tower_type));
        let grid = count.min(cap);
        let orbit = count - grid;
        grid_units.add(unit, grid);
        orbit_units.add(unit, orbit);
    }

    grid_layout(grid_units)
        .map(move |mut layout| {
            layout.relative_position += Vec2::new(0.0, -0.5 * layout.scale) + offset;
            layout
        })
        .chain(orbit_layout(orbit_units, time))
}

pub fn force_layout(force: &Force) -> impl Iterator<Item = UnitLayout> + '_ {
    let delta = force.current_destination().as_vec2() - force.current_source().as_vec2();
    swarm_layout(&force.units, delta.y.atan2(delta.x))
}

fn swarm_layout(units: &Units, direction: f32) -> impl Iterator<Item = UnitLayout> + '_ {
    let mut i = 0;
    let unit_formations: &'static [Vec2] = bytemuck::cast_slice(&UNIT_FORMATION_BYTES.0);

    let shield_only = units.iter().all(|(u, _)| u == Unit::Shield);
    let len = units
        .iter()
        .filter_map(|(u, c)| (u != Unit::Shield).then_some(c))
        .sum::<usize>()
        + shield_only as usize;
    let mut center = Vec2::ZERO;
    for &pos in unit_formations.iter().take(len) {
        center += pos;
    }
    center = center * (len as f32).recip();

    units
        .iter_with_zeros()
        .filter(|(unit, _)| !is_special(*unit))
        .chain(shield_only.then_some((Unit::Shield, 1)))
        .rev() // Units that fight last should be in center.
        .flat_map(|(unit, count)| iter::repeat(unit).take(count))
        .map(move |unit| {
            let pos = unit_formations[i % unit_formations.len()] - center;
            i += 1;

            let angle = unit_angle(unit, direction - std::f32::consts::FRAC_PI_2);
            UnitLayout {
                unit,
                relative_position: pos,
                angle,
                scale: unit_scale(unit),
                active: true,
            }
        })
}

fn orbit_layout(units: Units, time: f32) -> impl Iterator<Item = UnitLayout> + 'static {
    let factor = TAU / units.len() as f32;

    units
        .into_iter()
        .filter(|(unit, count)| *count > 0 && !is_special(*unit))
        .flat_map(|(unit, count)| iter::repeat(unit).take(count))
        .enumerate()
        .map(move |(i, unit)| {
            let angle = i as f32 * factor + time * 0.1;
            let (sin, cos) = angle.sin_cos();
            let relative_position = Vec2::new(cos, sin) * 0.8;
            UnitLayout {
                unit,
                relative_position,
                angle: unit_angle(unit, angle),
                scale: unit_scale(unit),
                active: true,
            }
        })
}

fn grid_layout(units: Units) -> impl Iterator<Item = UnitLayout> + 'static {
    let mut total_types = 0;
    let mut total_units = 0;
    for (unit, count) in units.iter() {
        if is_special(unit) {
            continue;
        }
        total_types += 1;
        total_units += count;
    }

    let max_columns = MAX_COLUMNS.max(total_types).max(total_units / 5);
    let scale = MAX_COLUMNS as f32 / max_columns as f32;

    let mut total_columns = 0;
    for (unit, count) in units.iter() {
        if is_special(unit) {
            continue;
        }
        total_columns += count.min((max_columns * count).div_ceil(total_units));
    }

    let mut start_column = 0;
    units
        .into_iter()
        .filter_map(move |(unit, count)| {
            if count == 0 || is_special(unit) {
                None
            } else {
                let columns = count.min((max_columns * count).div_ceil(total_units));
                let ret = Some(UnitTypeGridLayout {
                    unit,
                    start_column,
                    columns,
                    total_columns,
                    count,
                    scale,
                    index: 0,
                });

                start_column += columns;

                ret
            }
        })
        .flatten()
}

fn is_special(unit: Unit) -> bool {
    matches!(unit, Unit::Shield)
}

/// Gets the real angle of a unit from an input angle.
/// Ground units will flip instead of rotating.
fn unit_angle(unit: Unit, mut angle: f32) -> f32 {
    fn real_fract(v: f32) -> f32 {
        v - v.floor()
    }

    angle = real_fract(angle * (1.0 / TAU)) * TAU;
    if unit.field(false, true, false) == Field::Surface {
        if angle >= PI {
            0.0001
        } else {
            -0.0001
        }
    } else {
        angle
    }
}

const fn unit_scale(unit: Unit) -> f32 {
    match unit {
        Unit::Ruler => 0.5,
        _ => 0.25,
    }
}

struct UnitTypeGridLayout {
    unit: Unit,
    start_column: usize,
    columns: usize,
    total_columns: usize,
    count: usize,
    scale: f32,
    index: usize,
}

pub struct UnitLayout {
    pub unit: Unit,
    pub relative_position: Vec2,
    pub angle: f32,
    pub scale: f32,
    pub active: bool,
}

impl Iterator for UnitTypeGridLayout {
    type Item = UnitLayout;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            None
        } else {
            let scale = self.scale * unit_scale(self.unit);

            let column = self.start_column + self.index % self.columns;
            let horizontal = scale * 0.8 * (column as f32 - (self.total_columns - 1) as f32 * 0.5);
            let vertical = (self.index / self.columns) as f32 * -scale;

            self.index += 1;

            Some(UnitLayout {
                unit: self.unit,
                relative_position: Vec2::new(horizontal, vertical),
                angle: 0.0,
                scale,
                active: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::unit_angle;
    use common::unit::Unit;
    use std::f32::consts::TAU;

    #[test]
    fn test_unit_angle() {
        assert!(unit_angle(Unit::Tank, -0.1) > 0.0);
        assert!(unit_angle(Unit::Tank, -0.1 + TAU) > 0.0);
        assert!(unit_angle(Unit::Tank, 0.1) < 0.0);
        assert!(unit_angle(Unit::Tank, 0.1 + TAU) < 0.0);
    }
}
