// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use client_util::browser_storage::BrowserStorages;
use client_util::setting::{SettingCategory, Settings};
use common::tower::TowerType;
use std::collections::HashSet;
use std::fmt::{self, Display, Formatter, Write};
use std::str::FromStr;

#[derive(Clone, Default, PartialEq, Settings)]
pub struct TowerSettings {
    pub(crate) unlocks: Unlocks,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unlocks {
    pub keys: usize,
    pub towers: HashSet<TowerType>,
}

impl Unlocks {
    pub const MAX: usize = 3;

    pub fn contains(&self, tower_type: TowerType) -> bool {
        tower_type.level() == 0 || self.towers.contains(&tower_type)
    }

    pub fn add_key(&self) -> Self {
        let mut ret = self.clone();
        ret.keys = ret.keys.saturating_add(1).min(Self::MAX);
        ret
    }

    pub fn unlock(&self, tower_type: TowerType) -> Option<Self> {
        if self.contains(tower_type) {
            None
        } else {
            Some(Self {
                keys: self.keys.saturating_sub(1),
                towers: {
                    let mut ret = self.towers.clone();
                    ret.insert(tower_type);
                    ret
                },
            })
        }
    }
}

impl Default for Unlocks {
    fn default() -> Self {
        Self {
            keys: 3,
            towers: Default::default(),
        }
    }
}

impl Display for Unlocks {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.keys, f)?;
        for tower_type in &self.towers {
            f.write_char(',')?;
            Display::fmt(&tower_type, f)?;
        }
        Ok(())
    }
}

impl FromStr for Unlocks {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ret = Self::default();
        let mut iter = s.split(',');
        if let Some(keys) = iter.next().and_then(|s| usize::from_str(s).ok()) {
            ret.keys = keys.min(Self::MAX);
        }
        for tower_type in iter {
            if let Ok(tower_type) = TowerType::from_str(tower_type) {
                ret.towers.insert(tower_type);
            }
        }
        Ok(ret)
    }
}
