// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::tower::{Tower, TowerId, TowerMap, TowerRectangle, TowerType};
use common::world::{World, WorldChunks};
use kodiak_client::{PlayerId, U16Vec2};
use std::num::NonZeroU16;

#[derive(Default)]
pub struct Visible {
    previous: TowerMap<TowerType>,
    refs: TowerMap<NonZeroU16>,
    ticked: bool,
}

impl Visible {
    pub fn contains(&self, tower_id: TowerId) -> bool {
        self.refs.contains(tower_id)
    }

    pub fn iter<'a>(
        &'a self,
        towers: &'a WorldChunks,
    ) -> impl Iterator<Item = (TowerId, &'a Tower)> {
        self.refs
            .iter()
            .filter_map(|(id, _)| Some(id).zip(towers.get(id)))
    }

    /// Only set each game tick (ie 4 times per second).
    pub fn ticked(&mut self) {
        self.ticked = true;
    }

    pub fn update(&mut self, world: &World, me: PlayerId, all_visible: bool) {
        // Towers can only change every tick.
        if !std::mem::take(&mut self.ticked) {
            return;
        }

        let iter = world
            .chunk
            .iter_towers()
            .filter(|(_, t)| all_visible || t.player_id == Some(me));

        let mut min = U16Vec2::splat(WorldChunks::SIZE as u16 - 1);
        let mut max = U16Vec2::ZERO;

        for (id, _) in iter.clone() {
            min = min.min_components(id.0);
            max = max.max_components(id.0);
        }

        let view_rect = TowerRectangle::new(TowerId(min), TowerId(max));
        let sensor_rect = view_rect.add_margin(TowerType::max_range());

        // Grow refs. (TODO: allow shrinking)
        let union_rect = sensor_rect.union(self.refs.bounds());
        if self.refs.bounds() != union_rect {
            let mut new_refs = TowerMap::with_bounds(union_rect);
            for (tower_id, &v) in self.refs.iter() {
                new_refs.insert(tower_id, v);
            }
            self.refs = new_refs;
        }

        // Add towers that appeared or switched types.
        let mut next = TowerMap::with_bounds(view_rect);
        for (id, tower) in iter {
            // Set typ to Mine if not active since it has the default sensor radius.
            let typ = if tower.active() {
                tower.tower_type
            } else {
                TowerType::Mine
            };
            next.insert(id, typ);

            let previous = self.previous.remove(id);
            if previous != Some(typ) {
                if let Some(previous) = previous {
                    decrement_refs(&mut self.refs, id, previous);
                }
                increment_refs(&mut self.refs, id, typ);
            }
        }

        // Remove towers that disappeared.
        for (id, &typ) in self.previous.iter() {
            decrement_refs(&mut self.refs, id, typ);
        }
        self.previous = next;
    }
}

fn increment_refs(refs: &mut TowerMap<NonZeroU16>, id: TowerId, typ: TowerType) {
    for id in id.iter_radius(typ.sensor_radius()) {
        if let Some(r) = refs.get_mut(id) {
            *r = r.checked_add(1).unwrap();
        } else {
            refs.insert(id, NonZeroU16::MIN);
        }
    }
}

fn decrement_refs(refs: &mut TowerMap<NonZeroU16>, id: TowerId, typ: TowerType) {
    for id in id.iter_radius(typ.sensor_radius()) {
        let r = refs.get_mut(id).unwrap();
        if let Some(new) = NonZeroU16::new(r.get() - 1) {
            *r = new;
        } else {
            refs.remove(id);
        }
    }
}
