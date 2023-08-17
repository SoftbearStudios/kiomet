// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::angle::Angle;
use glam::{Vec2, Vec2Swizzles, Vec4, Vec4Swizzles};

pub struct SatRect {
    position: Vec2,
    half_dimensions: Vec2,
    normal: Vec2,
}

impl SatRect {
    pub fn new(position: Vec2, dimensions: Vec2, direction: Angle) -> Self {
        Self::with_normal(position, dimensions, direction.to_vec())
    }

    pub fn with_normal(position: Vec2, dimensions: Vec2, normal: Vec2) -> Self {
        debug_assert!(normal.is_normalized());
        Self {
            position,
            half_dimensions: dimensions * 0.5, // Saves work if SatRect is used multiple times.
            normal,
        }
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn collides_with(&self, b: &Self) -> bool {
        let a = self;
        let relative_position = a.position - b.position;
        sat_collision_half(
            relative_position,
            a.normal,
            b.normal,
            a.half_dimensions,
            b.half_dimensions,
        ) && sat_collision_half(
            -relative_position,
            b.normal,
            a.normal,
            b.half_dimensions,
            a.half_dimensions,
        )
    }
}

/// sat_collision_half performs half an SAT test (checks of one of two rectangles).
fn sat_collision_half(
    relative_position: Vec2,
    mut a_axis_normal: Vec2,
    b_axis_normal: Vec2,
    a_half_dimensions: Vec2,
    b_half_dimensions: Vec2,
) -> bool {
    // Doesn't [Vec2; 4] because half the floats would be duplicates.
    let offset_x = b_axis_normal * b_half_dimensions.x;
    let offset_y = b_axis_normal.perp() * b_half_dimensions.y;
    let other_ps = offset_x.xyxy() + offset_y.xy().extend(-offset_y.x).extend(-offset_y.y);

    // Only need to loop twice since rectangles only have 2 unique axes.
    for f in 0..2 {
        let dimension = if f == 0 {
            a_half_dimensions.x
        } else {
            a_half_dimensions.y
        };

        let dot = relative_position.dot(a_axis_normal);

        // Dimension is always positive, so min < max.
        let min = dot - dimension;
        let max = dot + dimension;
        debug_assert!(min < max, "negative dimension");

        // Unrolled dot products are ~15% faster.
        let scaled = other_ps * a_axis_normal.xyxy();

        // vxor + vhadd
        let neg = -scaled;
        let p1 = scaled.xz() + scaled.yw();
        let p2 = neg.xz() + neg.yw();
        let projected = p1.extend(p2.x).extend(p2.y);

        if projected.cmplt(Vec4::splat(min)).all() {
            return false;
        }
        if projected.cmpgt(Vec4::splat(max)).all() {
            return false;
        }

        // Start over with next axis.
        a_axis_normal = a_axis_normal.perp();
    }

    true
}

#[cfg(test)]
mod tests {
    use crate::angle::Angle;
    use crate::collision::SatRect;
    use glam::{vec2, Vec2};
    use test::bench::{black_box, Bencher};

    #[bench]
    fn bench_collides_with_true(bencher: &mut Bencher) {
        let a = SatRect::new(Vec2::ZERO, Vec2::splat(2.0), Angle::from_degrees(10.0));
        let b = SatRect::new(Vec2::ONE, vec2(2.0, 1.0), Angle::from_degrees(-85.0));

        assert!(a.collides_with(&b));

        bencher.iter(|| black_box(black_box(&a).collides_with(black_box(&b))))
    }

    #[bench]
    fn bench_collides_with_false(bencher: &mut Bencher) {
        let a = SatRect::new(Vec2::ZERO, Vec2::splat(2.0), Angle::from_degrees(10.0));
        let b = SatRect::new(
            Vec2::splat(100.0),
            vec2(2.0, 1.0),
            Angle::from_degrees(-85.0),
        );

        assert!(!a.collides_with(&b));

        bencher.iter(|| black_box(black_box(&a).collides_with(black_box(&b))))
    }
}
