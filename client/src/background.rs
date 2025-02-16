// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::finite_index::FiniteArena;
use crate::game::{is_visible, KiometGame};
use common::tower::{TowerId, TowerRectangle};
use kodiak_client::glam::{uvec2, IVec2, UVec2, Vec2};
use kodiak_client::renderer::{
    include_shader, DefaultRender, Layer, RenderLayer, Renderer, Shader, Texture, TextureFormat,
};
use kodiak_client::renderer2d::{BackgroundLayer, Camera2d, Invalidation};
use kodiak_client::{ClientContext, Mask2d, PlayerId, U16Vec2};

#[derive(Default, PartialEq)]
struct TowerView {
    center: TowerId,
    dim: U16Vec2,
}

impl TowerView {
    fn new(camera: Vec2, aspect: f32, zoom: f32) -> Self {
        let mut center = TowerId::rounded(camera);

        // The "dim" calculation results in 0 if center coordinates are 0, which crashes.
        // This may cause degenerate behavior though.
        center.x = center.x.max(1);
        center.y = center.y.max(1);

        let width: u16 = (2 * ((zoom / TowerId::CONVERSION as f32).ceil().max(2.0) as usize + 1)
            + 1)
        .try_into()
        .unwrap();
        let height: u16 = (2
            * ((zoom / (TowerId::CONVERSION as f32 * aspect))
                .ceil()
                .max(2.0) as usize
                + 1)
            + 1)
        .try_into()
        .unwrap();

        Self {
            center,
            dim: U16Vec2::new(width.min(center.x * 2), height.min(center.y * 2)),
        }
    }

    fn start(&self) -> U16Vec2 {
        self.center.0 - self.dim / 2
    }

    fn tower_id_to_uv_space(&self) -> (Vec2, Vec2) {
        let scale = (self.dim.as_vec2()).recip();
        let offset = -(self.center.0.as_vec2() - self.dim.as_vec2() * 0.5);
        (scale, offset * scale)
    }
}

#[derive(Layer)]
pub struct TowerBackgroundLayer {
    #[layer]
    background: BackgroundLayer,
    invalidation: Option<Invalidation>,
    index_arena: FiniteArena<u16>,
    last_tower_data: Vec<u32>,
    last_view: TowerView,
    shader: Shader,
    tower_texture: Texture,
}

impl TowerBackgroundLayer {
    pub fn new(renderer: &Renderer) -> Self {
        let tower_texture =
            Texture::new_empty(renderer, TextureFormat::Rgba { premultiply: false }, false);

        Self {
            background: BackgroundLayer::new(renderer),
            index_arena: Default::default(),
            invalidation: Default::default(),
            last_tower_data: Default::default(),
            last_view: Default::default(),
            shader: include_shader!(renderer, "background"),
            tower_texture,
        }
    }

    pub fn update(
        &mut self,
        camera: Vec2,
        zoom: f32,
        context: &ClientContext<KiometGame>,
        renderer: &Renderer,
    ) {
        let towers = &context.state.game.world.chunk;

        self.index_arena.tick();
        let mut get_index = |id: PlayerId| {
            self.index_arena
                .get(id.0.get(), || match Color::new(context, Some(id)) {
                    Color::Blue => 1..=1,
                    Color::Gray => unreachable!(),
                    Color::Purple => 2..=127,
                    Color::Red => 128..=u8::MAX,
                })
        };

        // Create current tower texture.
        let view = TowerView::new(camera, renderer.aspect_ratio(), zoom);
        let rect = TowerRectangle::new_centered(view.center, view.dim);

        // Not zero since it causes artifacts.
        const INVISIBLE: u32 = u32::from_le_bytes([127, 127, 0, 0]);

        let mut tower_data: Vec<_> = towers
            .iter_towers_rect(rect)
            .map(|t| {
                t.map_or(INVISIBLE, |(tower_id, tower)| {
                    let id = tower.player_id.map_or(0, &mut get_index);
                    let offset = tower_id.offset() * (u8::MAX as u16 / TowerId::CONVERSION);
                    let dx = offset.x as u8;
                    let dy = offset.y as u8;

                    let visibility = if is_visible(context, tower_id) {
                        255
                    } else {
                        0
                    };
                    u32::from_le_bytes([dx, dy, id, visibility])
                })
            })
            .collect();

        let dim: UVec2 = view.dim.into();
        assert_eq!(tower_data.len(), dim.x as usize * dim.y as usize);

        // Clear data of invisible towers surrounded by invisible towers to avoid creating
        // invalidations if data changes in a way that couldn't be observed.
        for y in 0..dim.y {
            for x in 0..dim.x {
                let y_range = y.saturating_sub(1)..(y + 2).min(dim.y);
                let x_range = x.saturating_sub(1)..(x + 2).min(dim.x);

                let is_invisible = y_range.clone().all(|y| {
                    let start = (y * dim.x + x_range.start) as usize;
                    let end = (y * dim.x + x_range.end) as usize;

                    tower_data[start..end].iter().all(|data| {
                        let visibility = data.to_le_bytes()[3];
                        visibility == 0
                    })
                });

                if is_invisible {
                    tower_data[(y * dim.x + x) as usize] = INVISIBLE;
                }
            }
        }
        let tower_data = tower_data;

        // Compare with previous tower texture to produce updated points.
        let mut updated_points = vec![];
        let offset: UVec2 = view.start().into();
        let last_offset: UVec2 = self.last_view.start().into();
        let last_dim: UVec2 = self.last_view.dim.into();

        for y in 0..dim.y {
            for x in 0..dim.x {
                let last_pos = (uvec2(x, y) + offset).as_ivec2() - last_offset.as_ivec2();
                let last_data = if last_pos.cmplt(IVec2::ZERO).any()
                    || last_pos.cmpge(last_dim.as_ivec2()).any()
                {
                    INVISIBLE
                } else {
                    let last_pos = last_pos.as_uvec2();
                    let j = last_pos.x + last_pos.y * last_dim.x;
                    self.last_tower_data[j as usize]
                };

                let i = x + y * dim.x;
                let data = tower_data[i as usize];
                if data != last_data {
                    updated_points.push(uvec2(x, y))
                }
            }
        }

        let any_updates = !updated_points.is_empty();
        let moved = self.last_view != view;

        // Don't buffer texture if it hasn't changed.
        if any_updates || moved {
            // Moves can happen without updates and vice versa.
            if any_updates {
                let dim = view.dim.into();
                let offset: UVec2 = view.start().into();
                self.invalidation = Some(Invalidation::Rects(
                    Mask2d::new_expanded(updated_points, dim, 3)
                        .take_rects()
                        .into_iter()
                        .map(|(start, end)| {
                            (
                                ((start + offset) * TowerId::CONVERSION as u32).as_vec2(),
                                ((end + offset + 1) * TowerId::CONVERSION as u32).as_vec2(),
                            )
                        })
                        .collect(),
                ));
            }

            self.tower_texture.realloc_with_opt_bytes(
                renderer,
                view.dim.into(),
                Some(bytemuck::cast_slice(&tower_data)),
            );

            self.last_view = view;
            self.last_tower_data = tower_data;
        }
    }
}

impl RenderLayer<&Camera2d> for TowerBackgroundLayer {
    fn render(&mut self, renderer: &Renderer, camera: &Camera2d) {
        if let Some(binding) = self.shader.bind(renderer) {
            let (mul, add) = self.last_view.tower_id_to_uv_space();
            let unit = self.last_view.dim.as_vec2().recip();

            binding.uniform("uDerivative", camera.derivative());
            binding.uniform("uTransform", mul.extend(add.x).extend(add.y));
            binding.uniform("uUnit", unit);
            binding.uniform("uTowers", &self.tower_texture);

            self.background.render(
                renderer,
                (
                    binding,
                    camera,
                    Some(std::mem::take(&mut self.invalidation)),
                ),
            );
        }
    }
}
