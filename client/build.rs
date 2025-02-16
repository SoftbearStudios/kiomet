// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use glam::Vec2;
use kodiak_common::Angle;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let points = poisson_disk_sampling(512, 0.075);
    let binary: &[u8] = bytemuck::cast_slice(&points);
    std::fs::write(Path::new(&out_dir).join("unit_formation.bin"), binary).unwrap();
}

fn poisson_disk_sampling(n: usize, r: f32) -> Vec<Vec2> {
    use rand::prelude::*;
    use rand_chacha::ChaCha20Rng;
    let mut rng = ChaCha20Rng::from_seed(Default::default());

    let individual_area = r.powi(2) * std::f32::consts::PI * 1.1;

    let mut points: Vec<Vec2> = Vec::with_capacity(n);
    while points.len() < n {
        let area = points.len() as f32 * individual_area;
        let mut radius = (area * (1.0 / std::f32::consts::PI)).sqrt();
        loop {
            let angle = rng.gen::<Angle>();
            let new_point = angle.to_vec() * radius;

            let mut ok = true;
            for &point in &points {
                if new_point.distance_squared(point) < r {
                    ok = false;
                }
            }
            if ok {
                points.push(new_point);
                break;
            }
            radius = radius * 1.05 + r * 0.5;
        }
    }

    let first = points[0].clone();
    points.iter_mut().for_each(|p| *p = *p - first);

    points
}
