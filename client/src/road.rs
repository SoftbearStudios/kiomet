// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::tower::TowerId;
use common::world::World;
use kodiak_client::glam::{vec2, Vec2, Vec4};
use kodiak_client::renderer::{
    derive_vertex, include_shader, DefaultRender, InstanceLayer, Layer, MeshBuilder, RenderLayer,
    Renderer, Shader,
};
use kodiak_client::renderer2d::Camera2d;

derive_vertex!(
    struct Instance {
        center: Vec2,
        scale: Vec2,
        rotation: f32,
        color: Vec4,
        end_alpha: f32,
        uv: Vec2,
    }
);

type RoadInstanceLayer = InstanceLayer<Vec2, u16, Instance, ()>;

#[derive(Layer)]
pub struct RoadLayer {
    #[layer]
    instances: RoadInstanceLayer,
    shader: Shader,
}

impl RenderLayer<&Camera2d> for RoadLayer {
    fn render(&mut self, renderer: &Renderer, camera: &Camera2d) {
        if let Some(binding) = self.shader.bind(renderer) {
            camera.prepare(&binding);
            binding.uniform("uTime", renderer.time);
            self.instances.render(renderer, &binding);
        }
    }
}

impl RoadLayer {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            instances: RoadInstanceLayer::new(renderer),
            shader: include_shader!(renderer, "road"),
        }
    }

    /// Returns true iff the path is viable (non-hypothetical).
    pub fn draw_path(
        &mut self,
        iter: impl Iterator<Item = TowerId>,
        max_edge_distance: Option<u32>,
        max_edges: usize,
        supply: bool,
        mut get_visibility: impl FnMut(TowerId) -> f32,
    ) -> bool {
        let max_edge_distance = max_edge_distance.unwrap_or(World::MAX_ROAD_LENGTH);

        let mut uv = 0.0;
        let mut previous_pos = None;
        let mut iter = iter
            .map(|id| {
                if supply {
                    let pos = id.as_vec2();
                    let d = std::mem::replace(&mut previous_pos, Some(pos))
                        .map_or(0.0, |p| pos.distance(p));

                    // Round uv to nearest arrow (prevents multiple supply lines having different
                    // offsets at the cost of having them animate at slightly different speeds).
                    uv = ((uv + d) * (1.0 / 0.75)).ceil() * 0.75;
                }
                (id, get_visibility(id), uv)
            })
            .enumerate();
        let (_, mut prev) = iter.next().unwrap();
        let mut hypothetical = false;

        for (i, next) in iter {
            if next.0 == prev.0 {
                continue;
            }
            let color =
                if hypothetical || i >= max_edges || next.0.distance(prev.0) > max_edge_distance {
                    hypothetical = true;
                    Vec4::new(0.8, 0.4, 0.2, 0.5)
                } else {
                    Vec4::splat(0.9)
                };

            self.draw_road_uv(
                prev.0.as_vec2(),
                next.0.as_vec2(),
                0.2,
                color.truncate().extend(color.w * prev.1),
                color.w * next.1,
                vec2(prev.2, next.2),
            );
            prev = next;
        }

        !hypothetical
    }

    pub fn draw_road(&mut self, start: Vec2, end: Vec2, width: f32, color: Vec4, end_alpha: f32) {
        self.draw_road_uv(start, end, width, color, end_alpha, Vec2::ZERO);
    }

    fn draw_road_uv(
        &mut self,
        start: Vec2,
        end: Vec2,
        width: f32,
        color: Vec4,
        end_alpha: f32,
        uv: Vec2,
    ) {
        // Invisible.
        if color.w == 0.0 && end_alpha == 0.0 {
            return;
        }

        let center = (start + end) * 0.5;
        let length = start.distance(end);
        let scale = vec2(length, width);
        let diff = end - start;
        let rotation = diff.y.atan2(diff.x);

        let instance = Instance {
            center,
            scale,
            rotation,
            color,
            end_alpha,
            uv,
        };

        self.instances.draw((), instance, || {
            let mut mesh = MeshBuilder::new();
            mesh.vertices.extend([
                vec2(-0.5, -0.5), // bottom left
                vec2(0.5, -0.5),  // bottom right
                vec2(0.5, 0.5),   // top right
                vec2(-0.5, 0.5),  // top left
            ]);
            mesh.push_default_quads();
            mesh
        })
    }
}
