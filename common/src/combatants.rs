// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::field::Field;
use crate::info::{Info, InfoEvent, LostRulerReason};
use crate::tower::TowerType;
use crate::unit::Unit;
use crate::units::Units;
use kodiak_common::glam::Vec2;
use kodiak_common::PlayerId;
use std::cmp::Ordering;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CombatInfo {
    AttackerLostRuler(Unit),
    DefenderLostRuler(Unit),
    Emp(CombatSide),
    NuclearExplosion,
    ShellExplosion,
}

impl CombatInfo {
    pub fn into_info_event(
        self,
        position: Vec2,
        attacker: Option<PlayerId>,
        defender: Option<PlayerId>,
    ) -> InfoEvent {
        InfoEvent {
            position,
            info: match self {
                Self::AttackerLostRuler(unit) => Info::LostRuler {
                    player_id: attacker.unwrap(), // Ruler must have a player id.
                    reason: LostRulerReason::KilledBy(defender, unit),
                },
                Self::DefenderLostRuler(unit) => Info::LostRuler {
                    player_id: defender.unwrap(),
                    reason: LostRulerReason::KilledBy(attacker, unit),
                },
                Self::Emp(side) => match side {
                    CombatSide::Attacker => Info::Emp(attacker),
                    CombatSide::Defender => Info::Emp(defender),
                },
                Self::NuclearExplosion => Info::NuclearExplosion,
                Self::ShellExplosion => Info::ShellExplosion,
            },
        }
    }
}

/// Sides of a fight between two [`Combatants`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CombatSide {
    Attacker,
    Defender,
}

impl CombatSide {
    fn from_attacker(is_attacker: bool) -> Self {
        if is_attacker {
            Self::Attacker
        } else {
            Self::Defender
        }
    }
}

#[derive(Debug)]
pub struct Combatants<'a> {
    units: &'a mut Units,
    tower_type: Option<TowerType>,
}

impl<'a> Combatants<'a> {
    /// Creates a combatants from a tower's type and units.
    pub fn tower(tower_type: TowerType, units: &'a mut Units) -> Self {
        Self {
            tower_type: Some(tower_type),
            units,
        }
    }

    /// Creates a combatants from a force's units.
    pub fn force(units: &'a mut Units) -> Self {
        Self {
            tower_type: None,
            units,
        }
    }

    /// Returns true amount added and makes the force alive (if applicable).
    /// Only for testing.
    #[cfg(test)]
    fn add(&mut self, unit: Unit, count: usize) -> usize {
        if let Some(tower_type) = self.tower_type {
            self.units.add_to_tower(unit, count, tower_type, false)
        } else {
            self.units.add(unit, count)
        }
    }

    /// Fights two combatants mutating them in place while providing any [`CombatInfo`] events.
    /// Only one of the combatants can be a tower. The tower wins stalemates without MAD.
    /// Returns who won or `None` if stalemate. By convention tower should be the defender.
    #[must_use]
    pub fn fight(
        attacker: &mut Self,
        defender: &mut Self,
        mut on_info: impl FnMut(CombatInfo),
    ) -> Option<CombatSide> {
        const DEBUG_FIGHT: bool = false; // cfg!(test);
        debug_assert!(
            attacker.tower_type.is_none() || defender.tower_type.is_none(),
            "two towers"
        );

        let hack = |a: &mut Combatants, b: &mut Combatants| {
            // Shield should not be used offensively against towers.
            if a.tower_type.is_some() {
                b.units.subtract(Unit::Shield, usize::MAX);
            }
        };
        hack(attacker, defender);
        hack(defender, attacker);

        let mut damage = 0;
        let mut last_attacker = None;
        let mut last_defender = None;
        let attacker_ptr = attacker as *const Self;

        let mut emped = false;
        let mut nuked = false;
        let mut shelled = false;

        let mut replace_unit = |me: &mut Self,
                                my_last: &mut Option<Unit>,
                                enemy_last: Option<Unit>,
                                unit: Option<Unit>| {
            let is_attacker = std::ptr::eq(me, attacker_ptr);
            let side = CombatSide::from_attacker(is_attacker);

            // Consume nukes right away.
            if DEBUG_FIGHT {
                println!("{side:?} used {unit:?}");
            }
            if let Some(unit) = unit
                && unit.is_single_use()
            {
                match unit {
                    Unit::Emp => {
                        // Don't do 2 emps if emps collide.
                        if !std::mem::replace(&mut emped, true) {
                            on_info(CombatInfo::Emp(side));
                        }
                    }
                    Unit::Nuke => {
                        // Don't do 2 explosions if nukes collide.
                        if !std::mem::replace(&mut nuked, true) {
                            on_info(CombatInfo::NuclearExplosion);
                        }
                    }
                    Unit::Shell => {
                        // TODO 1 event per shell with relative position.
                        if !std::mem::replace(&mut shelled, true) {
                            on_info(CombatInfo::ShellExplosion);
                        }
                    }
                    _ => {}
                }

                let subtracted = me.units.subtract(unit, 1);
                debug_assert_eq!(subtracted, 1);
            }

            // Needed another unit so kill previous.
            if let Some(last) = *my_last {
                if DEBUG_FIGHT {
                    println!("{side:?} consumed {last:?}");
                }

                // Nukes are consumed right away.
                if !last.is_single_use() {
                    if last == Unit::Ruler {
                        let cause = enemy_last.unwrap();

                        on_info(if is_attacker {
                            CombatInfo::AttackerLostRuler(cause)
                        } else {
                            CombatInfo::DefenderLostRuler(cause)
                        });
                    }

                    let subtracted = me.units.subtract(last, 1);
                    debug_assert_eq!(subtracted, 1);
                }
            }

            // Don't remove killer of ruler in stalemate.
            if unit.is_some() {
                *my_last = unit;
            }
        };

        fn unused_units<'a>(
            me: &'a Combatants,
            last: Option<Unit>,
        ) -> impl Iterator<Item = (Unit, usize)> + 'a {
            me.units.iter().filter_map(move |(u, mut c)| {
                // Don't count units already in use.
                if last == Some(u) && !u.is_single_use() {
                    c -= 1;
                }
                if c == 0 {
                    return None;
                }
                Some((u, c))
            })
        }

        let damage_against = |unit: Unit,
                              unit_field: Field,
                              enemy: &Combatants,
                              enemy_field: Field,
                              prev_dmg: i32| {
            let mut unit_damage = unit.damage(unit_field, enemy_field);
            if unit.is_ranged() {
                if let Some(tower_type) = enemy.tower_type {
                    // Nukes don't 1 shot silos.
                    unit_damage = tower_type.ranged_damage(unit_damage);
                }
            }

            // Cap damage at i32::MAX if infinite damage.
            let mut d = Unit::damage_to_finite(unit_damage);

            let is_defender = std::ptr::eq(enemy, attacker_ptr);
            let dir = if is_defender { -1 } else { 1 };

            // Nukes can't defend enemy nuclear annihilation.
            if prev_dmg * -dir > i32::MAX / 2 {
                d = d.min(1000);
            }
            d as i32 * dir
        };

        // Iterate in reverse order (enum iterator doesn't support rev).
        for field in [Field::Air, Field::Surface] {
            if DEBUG_FIGHT {
                println!("Field {field:?}");
            }
            loop {
                let next_unit_inner = |me: &Self, last: Option<Unit>, any_air: bool| {
                    let t = me.tower_type;
                    unused_units(me, last)
                        .filter_map(|(u, c)| {
                            let overflow = t.map_or(false, |t| c > me.units.capacity(u, Some(t)));
                            let unit_field = u.field(overflow, t.is_none(), any_air);
                            if unit_field >= field {
                                return Some((u, unit_field));
                            }
                            None
                        })
                        .next()
                };

                // Calls next_unit_inner up to two times to decide if shield is airborne.
                let next_unit = |me: &Self, last: Option<Unit>| {
                    let next = next_unit_inner(me, last, false);
                    if next.is_some() && field == Field::Air {
                        next_unit_inner(me, last, true)
                    } else {
                        next
                    }
                };

                let (next_attacker, next_defender) = match damage.cmp(&0) {
                    Ordering::Less => (next_unit(attacker, last_attacker), None),
                    Ordering::Greater => (None, next_unit(defender, last_defender)),
                    Ordering::Equal => {
                        let next_attacker = next_unit(attacker, last_attacker);
                        let next_defender = next_unit(defender, last_defender);
                        (next_defender.and(next_attacker), None)
                    }
                };

                damage += if let Some((next_attacker, unit_field)) = next_attacker {
                    replace_unit(
                        attacker,
                        &mut last_attacker,
                        last_defender,
                        Some(next_attacker),
                    );
                    damage_against(next_attacker, unit_field, defender, field, damage)
                } else if let Some((next_defender, unit_field)) = next_defender {
                    replace_unit(
                        defender,
                        &mut last_defender,
                        last_attacker,
                        Some(next_defender),
                    );
                    damage_against(next_defender, unit_field, attacker, field, damage)
                } else {
                    break;
                }
            }

            // TODO Make fields do more damage to fields under them.
        }

        // Ran out of units so kill last.
        if DEBUG_FIGHT {
            println!("Last damage {damage}");
        }
        let next_unused_unit =
            |me: &Self, last: Option<Unit>| unused_units(me, last).map(|(u, _)| u).next();

        let next_attacker = next_unused_unit(attacker, last_attacker);
        let next_defender = next_unused_unit(defender, last_defender);
        debug_assert!(
            next_attacker.and(next_defender).is_none(),
            "only 1 side should remain"
        );

        let mut ordering = [CombatSide::Attacker, CombatSide::Defender];
        if next_defender.is_some() {
            ordering.reverse();
        }

        for side in ordering {
            match side {
                CombatSide::Attacker if damage <= 0 => {
                    if let Some(unit) = next_attacker {
                        damage +=
                            damage_against(unit, Field::Surface, defender, Field::Surface, damage);
                    }
                    replace_unit(attacker, &mut last_attacker, last_defender, next_attacker);
                }
                CombatSide::Defender if damage >= 0 => {
                    if let Some(unit) = next_defender {
                        damage +=
                            damage_against(unit, Field::Surface, attacker, Field::Surface, damage);
                    }
                    replace_unit(defender, &mut last_defender, last_attacker, next_defender)
                }
                _ => (),
            }
        }

        let attacker_alive = attacker.units.is_alive();
        let defender_alive = defender.units.is_alive();
        debug_assert!(
            !(attacker_alive && defender_alive),
            "both alive attacker: {:?}, defender: {:?}, damage: {damage}",
            attacker.units,
            defender.units
        );

        if attacker_alive || (attacker.tower_type.is_some() && damage >= 0) {
            Some(CombatSide::Attacker)
        } else if defender_alive || (defender.tower_type.is_some() && damage <= 0) {
            Some(CombatSide::Defender)
        } else {
            None // Stalemate
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::combatants::{CombatInfo, CombatSide, Combatants};
    use crate::force::{Force, Path};
    use crate::tower::{Tower, TowerId, TowerType};
    use crate::unit::Unit;
    use crate::units::Units;
    use kodiak_common::PlayerId;

    fn make_tower_force() -> (Combatants<'static>, Combatants<'static>) {
        (make_tower(TowerType::Mine), make_force())
    }

    fn make_tower(tower_type: TowerType) -> Combatants<'static> {
        let mut tower = Box::new(Tower::with_type(tower_type));
        tower.set_player_id(Some(PlayerId::SOLO_OFFLINE));
        Combatants::tower(tower.tower_type, &mut Box::leak(tower).units)
    }

    fn make_force() -> Combatants<'static> {
        let path = Path::new(vec![TowerId::new(0, 0), TowerId::new(0, 1)]);
        let mut units = Units::default();
        units.add(Unit::Soldier, 1);
        let mut force = Box::new(Force::new(PlayerId::SOLO_OFFLINE, units, path));
        assert_eq!(force.units.subtract(Unit::Soldier, 1), 1);
        Combatants::force(&mut Box::leak(force).units)
    }

    #[test]
    fn fuzz() {
        use kodiak_common::rand::{thread_rng, Rng};
        fn random_units(shield: bool) -> Units {
            let mut ret = Units::default();
            let mut rng = thread_rng();
            for unit in Unit::iter() {
                let max = match unit {
                    Unit::Shield => {
                        if shield {
                            if rng.gen_bool(0.5) {
                                40
                            } else {
                                6
                            }
                        } else {
                            0
                        }
                    }
                    Unit::Fighter => 4,
                    Unit::Chopper => 4,
                    Unit::Bomber => 4,
                    Unit::Tank => 4,
                    Unit::Soldier => 8,
                    Unit::Shell => rng.gen_range(0..=3),
                    Unit::Emp => 1,
                    Unit::Nuke => 1,
                    Unit::Ruler => 1,
                };
                if max > 0 && rng.gen_bool(0.5) {
                    ret.add(unit, rng.gen_range(1..=max));
                }
            }
            ret
        }

        fn random_unit_pair(shield: bool) -> [Units; 2] {
            let units = random_units(shield);
            [units.clone(), units]
        }

        fn force_pair<'a>(units: &'a mut [Units; 2]) -> [Combatants<'a>; 2] {
            units.each_mut().map(move |units| Combatants::force(units))
        }

        fn random_tower_pair<'a>(units: &'a mut [Units; 2]) -> [Combatants<'a>; 2] {
            let tower_type = thread_rng().gen();
            units.each_mut().map(move |units| {
                units.reconcile(tower_type, true);
                Combatants::tower(tower_type, units)
            })
        }

        for _ in 0..1000 {
            let mut first_pair = random_unit_pair(false);
            let [mut attacker_1, mut defender_2] = force_pair(&mut first_pair);

            let is_tower = thread_rng().gen_bool(0.5);
            let mut pair = random_unit_pair(is_tower);
            let [mut defender_1, mut attacker_2] = if is_tower {
                random_tower_pair(&mut pair)
            } else {
                force_pair(&mut pair)
            };

            let string = format!("{attacker_1:?} {defender_1:?}");

            let r1 = Combatants::fight(&mut attacker_1, &mut defender_1, |_| {});
            let r2 = Combatants::fight(&mut attacker_2, &mut defender_2, |_| {});

            match (r1, r2) {
                (None, None) => {}
                (Some(a), Some(b)) => assert!(a != b, "different winner:\n - {string}\n - {r1:?} a: {attacker_1:?} d: {defender_1:?}\n - {r2:?} a: {attacker_2:?} d: {defender_2:?}\n"),
                _ => assert!(false, "winner/stalemate:\n - {string}\n - {r1:?} a: {attacker_1:?} d: {defender_1:?}\n - {r2:?} a: {attacker_2:?} d: {defender_2:?}\n")
            }
        }
    }

    #[test]
    fn bomber_wins_against_ruler_either_way() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Bomber, 1);
        tower.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));
        assert_eq!(info, [CombatInfo::DefenderLostRuler(Unit::Bomber)]);

        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Bomber, 1);
        tower.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut tower, &mut force, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));
        assert_eq!(info, [CombatInfo::AttackerLostRuler(Unit::Bomber)]);
    }

    #[test]
    fn solider_vs_ruler_either_way() {
        let mut solider = make_force();
        let mut ruler = make_force();
        let mut info = vec![];

        solider.add(Unit::Soldier, 1);
        ruler.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut solider, &mut ruler, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(solider.units.len(), 0);
        assert_eq!(ruler.units.len(), 0);
        assert_eq!(info, [CombatInfo::DefenderLostRuler(Unit::Soldier)]);

        let mut solider = make_force();
        let mut ruler = make_force();
        let mut info = vec![];

        solider.add(Unit::Soldier, 1);
        ruler.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut ruler, &mut solider, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(solider.units.len(), 0);
        assert_eq!(ruler.units.len(), 0);
        assert_eq!(info, [CombatInfo::AttackerLostRuler(Unit::Soldier)]);
    }

    #[test]
    fn tower_wins_against_force() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        tower.add(Unit::Soldier, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(tower.units.len(), 1);
        assert_eq!(info, [])
    }

    #[test]
    fn force_wins_against_tower() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Soldier, 2);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));

        assert_eq!(force.units.len(), 2);
        assert_eq!(info, []);
    }

    #[test]
    fn ruler_wins_against_tower() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));
        assert_eq!(info, []);
    }

    #[test]
    fn soldier_vs_soldier_stalemate() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Soldier, 1);
        tower.add(Unit::Soldier, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn soldier_vs_shield_stalemate() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Soldier, 1);
        tower.add(Unit::Shield, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn solider_vs_ruler_stalemate() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Soldier, 1);
        tower.add(Unit::Ruler, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::DefenderLostRuler(Unit::Soldier)]);
    }

    #[test]
    fn shields_win_against_soldiers() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        tower.add(Unit::Shield, 10);
        force.add(Unit::Soldier, 3);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 7);
        assert_eq!(info, []);
    }

    #[test]
    fn fighters_win_against_bombers() {
        let mut fighters = make_force();
        let mut bombers = make_force();
        let mut info = vec![];

        fighters.add(Unit::Fighter, 4);
        bombers.add(Unit::Bomber, 3);

        let winner = Combatants::fight(&mut fighters, &mut bombers, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));

        // 1 fighter can kill 3 bombers so all but 1 fighter should survive.
        assert_eq!(fighters.units.len(), 3);
        assert_eq!(bombers.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn bombers_overwhelm_fighters() {
        let mut fighters = make_force();
        let mut bombers = make_force();
        let mut info = vec![];

        fighters.add(Unit::Fighter, 2);
        bombers.add(Unit::Bomber, 8);

        let winner = Combatants::fight(&mut fighters, &mut bombers, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        // 1 fighter can only kill 3 bombers so 2 bombers should survive.
        assert_eq!(fighters.units.len(), 0);
        assert_eq!(bombers.units.len(), 2);
        assert_eq!(info, []);
    }

    #[test]
    fn fighters_vs_choppers_stalemate() {
        let mut fighters = make_force();
        let mut choppers = make_force();
        let mut info = vec![];

        fighters.add(Unit::Fighter, 3);
        choppers.add(Unit::Chopper, 3);

        let winner = Combatants::fight(&mut fighters, &mut choppers, |i| info.push(i));
        assert_eq!(winner, None);

        // ever since chopper damage increase, 3 fighters can kill 3 choppers.
        assert_eq!(fighters.units.len(), 0);
        assert_eq!(choppers.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn soldiers_vs_army() {
        let mut soldiers = make_force();
        let mut army = make_force();
        let mut info = vec![];

        soldiers.add(Unit::Soldier, 10);
        army.add(Unit::Soldier, 4);
        army.add(Unit::Tank, 1);

        let winner = Combatants::fight(&mut soldiers, &mut army, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));

        // 1 tank = 3 soldiers so army has 7 damage and soldiers have 10. 10 - 7 = 3.
        assert_eq!(soldiers.units.len(), 3);
        assert_eq!(army.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn bombers_vs_shield() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        tower.add(Unit::Shield, 10);
        force.add(Unit::Bomber, 4);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Attacker));

        // Two bombers are fully used, one is partially used and survives with another unused one.
        assert_eq!(force.units.len(), 2);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, []);
    }

    #[test]
    fn bombers_vs_airborne_shield() {
        let mut bombers = make_force();
        let mut airborne = make_force();
        let mut info = vec![];

        bombers.add(Unit::Bomber, 10);
        airborne.add(Unit::Shield, 10);
        airborne.add(Unit::Chopper, 1);

        let winner = Combatants::fight(&mut bombers, &mut airborne, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        // The airborne shields and bombers cancel and the one chopper remains.
        assert_eq!(bombers.units.len(), 0);
        assert_eq!(airborne.units.len(), 1);
        assert_eq!(info, []);
    }

    #[test]
    fn nuke_vs_nuke_stalemate() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 1);
        tower.add(Unit::Nuke, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn nuke_vs_fighter() {
        let mut nuke = make_force();
        let mut fighter = make_force();
        let mut info = vec![];

        nuke.add(Unit::Nuke, 1);
        fighter.add(Unit::Fighter, 1);

        let winner = Combatants::fight(&mut nuke, &mut fighter, |i| info.push(i));
        assert_eq!(winner, None);

        // Fighter triggers nuke.
        assert_eq!(nuke.units.len(), 0);
        assert_eq!(fighter.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn nuke_vs_town() {
        let mut tower = make_tower(TowerType::Town);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 1);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn two_nukes_vs_hq() {
        let mut tower = make_tower(TowerType::Headquarters);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 2);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn three_nukes_vs_hq() {
        let mut tower = make_tower(TowerType::Headquarters);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 3);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn four_nukes_vs_bunker() {
        let mut tower = make_tower(TowerType::Bunker);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 4);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn five_nukes_vs_bunker() {
        let mut tower = make_tower(TowerType::Bunker);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Nuke, 5);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::NuclearExplosion]);
    }

    #[test]
    fn one_shell_vs_three() {
        let mut attacker = make_force();
        let mut defender = make_force();
        let mut info = vec![];

        attacker.add(Unit::Shell, 3);
        defender.add(Unit::Shell, 1);

        let winner = Combatants::fight(&mut attacker, &mut defender, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(attacker.units.len(), 1);
        assert_eq!(defender.units.len(), 0);
        assert_eq!(info, [CombatInfo::ShellExplosion]);
    }

    #[test]
    fn _20_shells_vs_headquarters() {
        let mut tower = make_tower(TowerType::Headquarters);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Shell, 20);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::ShellExplosion]);
    }

    #[test]
    fn _21_shells_vs_headquarters() {
        let mut tower = make_tower(TowerType::Headquarters);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Shell, 21);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::ShellExplosion]);
    }

    #[test]
    fn _40_shells_vs_bunker() {
        let mut tower = make_tower(TowerType::Bunker);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Shell, 40);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, Some(CombatSide::Defender));

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::ShellExplosion]);
    }

    #[test]
    fn _41_shells_vs_bunker() {
        let mut tower = make_tower(TowerType::Bunker);
        let mut force = make_force();
        let mut info = vec![];

        force.add(Unit::Shell, 41);
        tower.add(Unit::Shield, usize::MAX);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);

        assert_eq!(force.units.len(), 0);
        assert_eq!(tower.units.len(), 0);
        assert_eq!(info, [CombatInfo::ShellExplosion]);
    }

    #[test]
    fn emp_vs_tower() {
        let (mut tower, mut force) = make_tower_force();
        let mut info = vec![];

        force.add(Unit::Emp, 1);

        let winner = Combatants::fight(&mut force, &mut tower, |i| info.push(i));
        assert_eq!(winner, None);
        assert_eq!(info, [CombatInfo::Emp(CombatSide::Attacker)]);
    }
}
