// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game::KiometGame;
use kodiak_client::glam::Vec3;
use kodiak_client::renderer::{rgb_hex, rgba_array_to_css};
use kodiak_client::{ClientContext, PlayerId};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum Color {
    Blue = 0u8,
    Gray,
    Purple,
    Red,
}

impl Color {
    pub fn new(context: &ClientContext<KiometGame>, player_id: Option<PlayerId>) -> Self {
        let Some(player_id) = player_id else {
            return Self::Gray;
        };
        let Some(me) = context.player_id().filter(|_| context.state.game.alive) else {
            return Self::Red;
        };

        if player_id == me {
            Self::Blue
        } else {
            if context.state.game.world.have_alliance(me, player_id) {
                Self::Purple
            } else {
                Self::Red
            }
        }
    }

    /// Certain effects don't look good with gray so we can replace them with red.
    pub fn make_gray_red(self) -> Self {
        if self == Self::Gray {
            Self::Red
        } else {
            self
        }
    }

    pub fn color_hex_rgb(self) -> u32 {
        use Color::*;
        match self {
            Blue => 0x74b9ff,
            Gray => 0x666666,
            Purple => 0x8644fc,
            Red => 0xc0392b,
        }
    }

    pub fn shield_color(self) -> Vec3 {
        use Color::*;

        // TODO function of color.
        rgb_hex(match self {
            Blue => 0x667fcc,
            Gray => 0,          // No zombie shields.
            Purple => 0x794D99, // TODO
            Red => 0x6e4c3a,
        })
    }

    pub fn background_color_css(self) -> String {
        let [_, r, g, b] = self.color_hex_rgb().to_be_bytes().map(|c| c / 3 * 2);
        rgba_array_to_css([r, g, b, 255])
    }

    /// Colors used for svg and ui elements.
    pub fn ui_colors(self) -> (Option<Vec3>, Option<Vec3>) {
        self.colors(true, false, true)
    }

    pub fn colors(
        self,
        active: bool,
        hovered: bool,
        selected: bool,
    ) -> (Option<Vec3>, Option<Vec3>) {
        let color = rgb_hex(self.color_hex_rgb());

        fn highlight(color: Vec3, a: f32) -> Vec3 {
            (color + a) * (1.0 + a)
        }

        let mut dim = 0.0;
        if !active {
            dim = 0.15;
        }
        let color = highlight(color, -dim);

        let mut boost = 0.1;
        if selected {
            boost += 0.1;
        }
        if hovered {
            boost += 0.05;
        }

        let stroke_color = Some(highlight(color, boost));
        let fill_color = (self != Self::Gray).then_some(color);

        (stroke_color, fill_color)
    }
}
