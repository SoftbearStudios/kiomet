// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use kodiak_client::glam::{Vec2, Vec3, Vec4};

pub struct Animation {
    position: Vec2,
    animation_type: AnimationType,
    start_seconds: f32,
}

pub enum AnimationType {
    Emp(Color),
    NuclearExplosion,
    ShellExplosion,
}

impl Animation {
    pub fn new(position: Vec2, animation_type: AnimationType, time_seconds: f32) -> Self {
        Self {
            position,
            animation_type,
            start_seconds: time_seconds,
        }
    }

    /// Returns a boolean of whether animation is *not* done.
    pub fn render<F: FnMut(Vec2, f32, Vec4)>(
        &self,
        mut draw_filled_circle: F,
        time_seconds: f32,
    ) -> bool {
        let mut draw =
            |time_delay: f32, time_scale: f32, max_radius: f32, max_alpha: f32, color: Vec3| {
                let t = time_seconds - self.start_seconds;
                let elapsed = if time_scale < 0.0 {
                    time_delay - t
                } else {
                    t - time_delay
                }
                .max(0.0);

                let s = time_scale.abs();
                let radius = (elapsed * s * 4.0).min(max_radius);
                let alpha = (1.0 - elapsed * s * 0.3).clamp(0.0, max_alpha);

                if alpha > 0.0 {
                    draw_filled_circle(self.position, radius, color.extend(alpha));
                    true
                } else {
                    false
                }
            };

        let white = Vec3::ONE;
        match self.animation_type {
            AnimationType::Emp(color) => {
                let (stroke, _) = color.colors(true, true, false);
                let color = stroke.unwrap(); // TODO don't return option that's always Some.
                draw(1.2, -0.5, 1.0, 0.3, color)
            }
            AnimationType::NuclearExplosion => {
                draw(0.0, 0.33, 1.5, 0.6, white) | draw(0.0, 1.0, 1.0, 1.0, white)
            }
            AnimationType::ShellExplosion => draw(-0.25, 2.0, 0.3, 0.7, white),
        }
    }
}
