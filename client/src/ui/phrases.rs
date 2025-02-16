// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::death_reason::DeathReason;
use common::tower::TowerType;
use common::unit::Unit;
use kodiak_client::{translate, PlayerAlias, Translator};

pub trait KiometPhrases {
    fn alert_capture_instruction(&self) -> String;
    fn alert_capture_hint(&self) -> String;
    fn alert_upgrade_instruction(&self) -> String;
    fn alert_upgrade_hint(&self) -> String;
    fn alert_ruler_unsafe_instruction(&self) -> String;
    fn alert_ruler_unsafe_hint(&self) -> String;
    fn alert_ruler_under_attack_warning(&self) -> String;
    fn alert_ruler_under_attack_hint(&self) -> String;
    fn alert_zombies_warning(&self) -> String;
    fn alert_zombies_hint(&self) -> String;
    fn alert_full_warning(&self) -> String;
    fn alert_full_hint(&self) -> String;
    fn alert_overflowing_warning(&self) -> String;
    fn alert_overflowing_hint(&self) -> String;
    fn break_alliance_hint(&self) -> String;
    fn cancel_alliance_hint(&self) -> String;
    fn death_reason(&self, death_reason: DeathReason) -> String;
    fn _demolish_hint(&self) -> String;
    fn owner_s(&self, owner: &str) -> String;
    fn request_alliance_hint(&self) -> String;
    fn ruler_killed(&self, alias: Option<PlayerAlias>, unit: &str) -> String;
    fn ruler_label(&self) -> String;
    fn _tower_label(&self) -> String;
    fn tower_type_label(&self, tower_type: TowerType) -> String;
    fn unit_label(&self, unit: Unit) -> String;
    fn zombie(&self) -> String;
}

impl KiometPhrases for Translator {
    fn tower_type_label(&self, tower_type: TowerType) -> String {
        use TowerType::*;
        match tower_type {
            Airfield => translate!(self, "Airfield"),
            Armory => translate!(self, "Armory"),
            Artillery => translate!(self, "Artillery"),
            Barracks => translate!(self, "Barracks"),
            Bunker => translate!(self, "Bunker"),
            // Capitol => "Capitol", // TODO
            Centrifuge => translate!(self, "Centrifuge"),
            City => translate!(self, "City"),
            Cliff => translate!(self, "Cliff"),
            Ews => translate!(self, "EWS"),
            Factory => translate!(self, "Factory"),
            Generator => translate!(self, "Generator"),
            Headquarters => translate!(self, "Headquarters"),
            Helipad => translate!(self, "Helipad"),
            //Icbm => "ICBM",   // TODO
            //Laser => "Laser", // TODO
            Launcher => translate!(self, "Launcher"),
            //Metropolis => "Metropolis", // TODO
            Mine => translate!(self, "Mine"),
            Projector => translate!(self, "Projector"),
            Quarry => translate!(self, "Quarry"),
            Radar => translate!(self, "Radar"),
            Rampart => translate!(self, "Rampart"),
            Reactor => translate!(self, "Reactor"),
            Refinery => translate!(self, "Refinery"),
            Rocket => translate!(self, "Rocket"),
            Runway => translate!(self, "Runway"),
            Satellite => translate!(self, "Satellite"),
            Silo => translate!(self, "Silo"),
            Town => translate!(self, "Town"),
            Village => translate!(self, "Village"),
        }
    }

    fn unit_label(&self, unit: Unit) -> String {
        use Unit::*;
        match unit {
            Bomber => translate!(self, "Bomber"),
            Chopper => translate!(self, "Chopper"),
            Emp => translate!(self, "EMP"),
            Fighter => translate!(self, "Fighter"),
            Nuke => translate!(self, "Nuke"),
            Ruler => self.ruler_label(),
            Shell => translate!(self, "Shell"),
            Shield => translate!(self, "Shield"),
            Soldier => translate!(self, "Soldier"),
            Tank => translate!(self, "Tank"),
        }
    }

    fn death_reason(&self, death_reason: DeathReason) -> String {
        use DeathReason::*;
        match death_reason {
            RulerKilled { alias, unit } => self.ruler_killed(
                alias,
                // TODO don't use to_lowercase as it adds 32.6 kb to the binary.
                &self.unit_label(unit),
            ),
        }
    }

    fn _tower_label(&self) -> String {
        translate!(self, "Tower")
    }

    fn _demolish_hint(&self) -> String {
        translate!(self, "Demolish")
    }

    fn request_alliance_hint(&self) -> String {
        translate!(self, "Request alliance")
    }

    fn cancel_alliance_hint(&self) -> String {
        translate!(self, "Cancel request")
    }

    fn break_alliance_hint(&self) -> String {
        translate!(self, "Break alliance")
    }

    fn alert_capture_instruction(&self) -> String {
        translate!(self, "Capture more towers")
    }

    fn alert_capture_hint(&self) -> String {
        translate!(
            self,
            "alert_capture_hint",
            "Drag units from your towers to outside your borders"
        )
    }

    fn alert_upgrade_instruction(&self) -> String {
        translate!(self, "Upgrade a tower")
    }

    fn alert_upgrade_hint(&self) -> String {
        translate!(
            self,
            "alert_upgrade_hint",
            "Click a tower to show upgrade options"
        )
    }

    fn alert_ruler_unsafe_instruction(&self) -> String {
        let ruler = self.ruler_label();
        translate!(
            self,
            "alert_ruler_unsafe_instruction",
            "Move your {ruler} to safety"
        )
    }

    fn alert_ruler_unsafe_hint(&self) -> String {
        // FIXME: Redundant tower names?
        translate!(self, "Shielded Headquarters or Bunkers near the center of your territory provide the most protection")
    }

    fn alert_ruler_under_attack_warning(&self) -> String {
        let ruler = self.ruler_label();
        translate!(
            self,
            "alert_ruler_under_attack_warning",
            "Your {ruler} is under attack!"
        )
    }

    fn alert_ruler_under_attack_hint(&self) -> String {
        translate!(
            self,
            "alert_ruler_under_attack_hint",
            "If they die, you lose the game"
        )
    }

    fn alert_zombies_warning(&self) -> String {
        translate!(self, "Zombies sighted")
    }

    fn alert_zombies_hint(&self) -> String {
        translate!(
            self,
            "alert_zombies_hint",
            "Escape them by moving in the opposite direction"
        )
    }

    fn alert_full_warning(&self) -> String {
        translate!(self, "A tower is full")
    }

    fn alert_full_hint(&self) -> String {
        translate!(
            self,
            "alert_full_hint",
            "Drag away units to make room for more"
        )
    }

    fn alert_overflowing_warning(&self) -> String {
        translate!(self, "A tower is overflowing")
    }

    fn alert_overflowing_hint(&self) -> String {
        translate!(
            self,
            "alert_overflowing_hint",
            "Drag away units to stop them from disappearing"
        )
    }

    fn owner_s(&self, alias: &str) -> String {
        translate!(self, "{alias}'s")
    }

    fn ruler_killed(&self, alias: Option<PlayerAlias>, unit: &str) -> String {
        let ruler = self.ruler_label();
        let owner = alias.map_or(self.zombie().into(), |alias| self.owner_s(&alias));
        translate!(self, "{ruler} killed by {owner} {unit}!")
    }

    fn ruler_label(&self) -> String {
        translate!(self, "King")
    }

    fn zombie(&self) -> String {
        translate!(self, "zombie")
    }
}
