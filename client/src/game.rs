// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::animation::{Animation, AnimationType};
use crate::background::TowerBackgroundLayer;
use crate::color::Color;
use crate::key_dispenser::KeyDispenser;
use crate::layout::{force_layout, tower_layout};
use crate::path::*;
use crate::road::RoadLayer;
use crate::settings::TowerSettings;
use crate::state::TowerState;
use crate::territory::Territories;
use crate::tutorial::Tutorial;
use crate::ui::{KiometRoute, KiometUi, KiometUiEvent, KiometUiProps, SelectedTower};
use common::chunk::ChunkRectangle;
use common::force::{Force, Path};
use common::info::{GainedTowerReason, Info, InfoEvent};
use common::protocol::{Command, Update};
use common::tower::{Tower, TowerId, TowerRectangle, TowerType};
use common::unit::Unit;
use common::units::Units;
use common::world::{World, WorldChunks};
use common::KIOMET_CONSTANTS;
use kodiak_client::glam::{IVec2, Vec2, Vec3, Vec4};
use kodiak_client::renderer::{DefaultRender, Layer, RenderChain, TextStyle};
use kodiak_client::renderer2d::{Camera2d, TextLayer};
use kodiak_client::{
    include_audio, js_hooks, translate, ClientContext, FatalError, GameClient, GameConstants, Key,
    MouseButton, MouseEvent, PanZoom, RankNumber, RateLimiter, Translator,
};
use std::f32::consts::PI;

include_audio!("/data/audio.mp3" "./audio.json");

pub struct KiometGame {
    animations: Vec<Animation>,
    camera: Camera2d,
    drag: Option<Drag>,
    key_dispenser: KeyDispenser,
    lock_dialog: Option<TowerType>,
    pan_zoom: PanZoom,
    panning: bool,
    render_chain: RenderChain<TowerLayer>,
    selected_tower_id: Option<TowerId>,
    territories: Territories,
    tutorial: Tutorial,
    was_alive: bool,
    set_viewport_rate_limit: RateLimiter,
}

impl KiometGame {
    fn move_world_space(&mut self, world_space: Vec2, context: &mut ClientContext<Self>) {
        if let Some(drag) = self.drag.as_mut() {
            if let Some(closest) = get_closest(world_space, context) {
                if Some(closest) != drag.current.map(|(start, _)| start) {
                    drag.current = Some((closest, context.client.time_seconds));
                }
            } else {
                drag.current = None;
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Drag {
    start: TowerId,
    current: Option<(TowerId, f32)>,
}

impl Drag {
    fn zip(drag: Option<Self>) -> Option<(TowerId, TowerId, f32)> {
        drag.and_then(move |drag| {
            drag.current
                .map(|(current, current_start)| (drag.start, current, current_start))
        })
    }
}

#[derive(Layer)]
#[render(&Camera2d)]
pub struct TowerLayer {
    background: TowerBackgroundLayer,
    roads: RoadLayer,
    paths: PathLayer,
    text: TextLayer,
}

impl KiometGame {
    const RULER_DRAG_DELAY: f32 = 1.2;
}

impl GameClient for KiometGame {
    const GAME_CONSTANTS: &'static GameConstants = KIOMET_CONSTANTS;
    const LICENSES: &'static str = concat!(
        include_str!("../../assets/audio/README.md"),
        include_str!("ui/translations/licenses.md")
    );

    type Audio = Audio;
    type GameRequest = Command;
    type GameState = TowerState;
    type UiEvent = KiometUiEvent;
    type UiProps = KiometUiProps;
    type UiRoute = KiometRoute;
    type Ui = KiometUi;
    type GameUpdate = Update;
    type GameSettings = TowerSettings;

    fn new(_: &mut ClientContext<Self>) -> Result<Self, FatalError> {
        let render_chain = RenderChain::new([45, 52, 54, 255], true, |renderer| {
            renderer.enable_angle_instanced_arrays();

            TowerLayer {
                background: TowerBackgroundLayer::new(&*renderer),
                roads: RoadLayer::new(&*renderer),
                paths: PathLayer::new(&*renderer),
                text: TextLayer::new(&*renderer),
            }
        })?;

        Ok(Self {
            animations: Default::default(),
            camera: Camera2d::default(),
            drag: Default::default(),
            key_dispenser: Default::default(),
            lock_dialog: None,
            pan_zoom: Default::default(),
            panning: Default::default(),
            render_chain,
            selected_tower_id: Default::default(),
            territories: Default::default(),
            tutorial: Default::default(),
            was_alive: Default::default(),
            set_viewport_rate_limit: RateLimiter::new(0.15),
        })
    }

    fn translate_rank_number(t: &Translator, n: RankNumber) -> String {
        match n {
            RankNumber::Rank1 => translate!(t, "rank_1", "Candidate"),
            RankNumber::Rank2 => translate!(t, "rank_2", "Tactician"),
            RankNumber::Rank3 => translate!(t, "rank_3", "Strategist"),
            RankNumber::Rank4 => translate!(t, "rank_4", "Representative"),
            RankNumber::Rank5 => translate!(t, "rank_5", "Senator"),
            RankNumber::Rank6 => translate!(t, "rank_6", "Supreme Leader"),
        }
    }

    fn translate_rank_benefits(t: &Translator, n: RankNumber) -> Vec<String> {
        match n {
            RankNumber::Rank1 => vec![],
            RankNumber::Rank2 => vec![translate!(t, "Spawn with more soldiers")],
            RankNumber::Rank3 => vec![translate!(t, "Unlock all towers")],
            RankNumber::Rank4 => vec![],
            RankNumber::Rank5 => vec![translate!(t, "Spawn with fighters")],
            RankNumber::Rank6 => vec![],
        }
    }

    fn peek_mouse(&mut self, event: &MouseEvent, context: &mut ClientContext<Self>) {
        update_visible(context);

        match *event {
            MouseEvent::MoveViewSpace(view_space) => {
                if self.panning {
                    if let Some(old_view_space) = context.mouse.view_position {
                        let world_space = self.camera.to_world_position(view_space);
                        let old_world_space = self.camera.to_world_position(old_view_space);
                        self.pan_zoom.pan(world_space - old_world_space);
                    }
                }
            }
            MouseEvent::Button { button, down, .. } => match button {
                #[cfg(debug_assertions)]
                MouseButton::Middle => {
                    if down {
                        self.animations.push(Animation::new(
                            self.camera
                                .to_world_position(context.mouse.view_position.unwrap_or_default()),
                            AnimationType::Emp(Color::Red),
                            context.client.time_seconds,
                        ));
                    }
                }
                MouseButton::Left => {
                    if down {
                        if self.drag.is_none() && !self.panning {
                            if let Some(drag_start) = context.mouse.view_position.and_then(|v| {
                                get_closest(self.camera.to_world_position(v), context)
                            }) {
                                self.drag = Some(Drag {
                                    start: drag_start,
                                    current: Some((drag_start, context.client.time_seconds)),
                                });
                                if self.selected_tower_id != Some(drag_start) {
                                    // If they were equal, wait for mouse up before clearing selection.
                                    self.selected_tower_id = None;
                                }
                            } else {
                                self.selected_tower_id = None;
                            }
                        }
                    } else {
                        if let Some((start, current, current_start_time)) = Drag::zip(self.drag) {
                            if start == current {
                                if self.selected_tower_id == Some(start) {
                                    // Double click to deselect.
                                    // TODO don't deselect tower if tried dragging a path.
                                    self.selected_tower_id = None;
                                } else {
                                    self.selected_tower_id = Some(start);
                                }
                            } else if let Some((source_tower, _destination_tower)) = context
                                .state
                                .game
                                .world
                                .chunk
                                .get(start)
                                .zip(context.state.game.world.chunk.get(current))
                            {
                                if self.selected_tower_id != Some(start) {
                                    self.selected_tower_id = None;
                                }

                                let strength = source_tower.force_units();
                                let tower_edge_distance = source_tower.tower_type.ranged_distance();
                                let strength_edge_distance =
                                    (!strength.is_empty()).then(|| strength.max_edge_distance());
                                let max_edge_distance = strength_edge_distance
                                    .map_or(tower_edge_distance, |e| e.min(tower_edge_distance));
                                let shorter_max_edge_distance =
                                    max_edge_distance != tower_edge_distance;
                                let supply_tower_id = self.selected_tower_id.filter(|_| {
                                    source_tower.generates_mobile_units()
                                        && !shorter_max_edge_distance
                                });

                                let path = context.state.game.world.find_best_path(
                                    start,
                                    current,
                                    max_edge_distance,
                                    context.player_id().unwrap(),
                                    |tower_id| is_visible(context, tower_id),
                                );

                                if let Some(path) = path {
                                    let perilous =
                                        path.iter().any(|&tower_id| is_perilous(context, tower_id));

                                    if !perilous
                                        || !strength.contains(Unit::Ruler)
                                        || context.client.time_seconds
                                            >= current_start_time + Self::RULER_DRAG_DELAY
                                    {
                                        context.send_to_game(
                                            if let Some(tower_id) = supply_tower_id {
                                                let path = Path::new(path);
                                                Command::SetSupplyLine {
                                                    tower_id,
                                                    // TODO accept any invalid path.
                                                    path: (source_tower.supply_line.as_ref()
                                                        != Some(&path))
                                                    .then_some(path),
                                                }
                                            } else {
                                                Command::deploy_force_from_path(path)
                                            },
                                        );
                                    }
                                }
                            } else {
                                self.selected_tower_id = None;
                            }
                        } else {
                            self.selected_tower_id = None;
                        }
                        self.drag = None;
                    }
                }
                MouseButton::Right => {
                    self.close_tower_menu();
                    self.panning = down;
                }
                #[cfg(not(debug_assertions))]
                _ => {}
            },
            MouseEvent::Wheel(delta) => {
                self.close_tower_menu();

                self.pan_zoom.multiply_zoom(
                    self.camera
                        .to_world_position(context.mouse.view_position.unwrap_or_default()),
                    2f32.powf(delta * (1.0 / 3.0)),
                );
            }
            _ => {}
        }
    }

    fn render(&mut self, elapsed_seconds: f32, context: &ClientContext<Self>) {
        let mut frame = self.render_chain.begin(context.client.time_seconds);
        let (renderer, layer) = frame.draw();

        let camera = self.pan_zoom.get_center();
        let zoom = self.pan_zoom.get_zoom();
        let canvas_size = renderer.canvas_size();
        self.camera.update(camera, zoom, canvas_size);
        let zoom_per_pixel = zoom / canvas_size.x as f32;

        // Make sure this is after `Renderer::set_camera`.
        layer.background.update(camera, zoom, context, renderer);

        self.tutorial.render(
            &mut layer.paths,
            self.selected_tower_id,
            context.client.time_seconds,
        );

        let hovered_tower_id = context
            .mouse
            .view_position
            .and_then(|v| TowerId::closest(self.camera.to_world_position(v)));
        let show_similar_towers = self
            .selected_tower_id
            .filter(|_| context.keyboard.is_down(Key::T))
            .and_then(|id| context.state.game.world.chunk.get(id))
            .map(|t| t.tower_type);
        let get_visibility = |id| is_visible(context, id).then_some(1.0).unwrap_or_default();
        let me = context.player_id();

        for (tower_id, tower) in context
            .state
            .game
            .visible
            .iter(&context.state.game.world.chunk)
        {
            if !context.state.game.margin_viewport.contains(tower_id) {
                // TODO iter viewport intersection visible and towers.
                continue;
            }

            let tower_position = tower_id.as_vec2();
            let hovered = hovered_tower_id == Some(tower_id);
            let selected = self.selected_tower_id == Some(tower_id);
            let tower_scale = tower.tower_type.scale() as f32;

            if zoom_per_pixel < 0.3 {
                for nearby_tower_id in tower_id.neighbors() {
                    if !exists(context, nearby_tower_id) {
                        continue; // Hasn't been generated yet.
                    }

                    let visible = is_visible(context, nearby_tower_id);
                    if nearby_tower_id >= tower_id && visible {
                        continue; // Don't draw twice.
                    }

                    // Fade out roads of invisible towers.
                    let s = Vec3::splat(1.0).extend(0.05);
                    let e = if visible { s.w } else { 0.0 };

                    layer
                        .roads
                        .draw_road(tower_position, nearby_tower_id.as_vec2(), 0.12, s, e);
                }
            }

            let show_supply_lines = context.keyboard.is_down(Key::R);
            if show_supply_lines
                || Some(tower_id) == self.selected_tower_id
                || Some(tower_id) == hovered_tower_id
            {
                let is_selected = Some(tower_id) == self.selected_tower_id;
                let is_hover = Some(tower_id) == hovered_tower_id && !is_selected;
                let is_dragging = Some(tower_id) == self.drag.map(|Drag { start, .. }| start);

                if show_supply_lines || !is_hover || !is_dragging {
                    if let Some(path) = &tower.supply_line {
                        if tower.player_id.is_some() && (tower.player_id == me || context.cheats())
                        {
                            let alpha = if is_selected {
                                if is_dragging {
                                    0.5 // Darken selected while changing it.
                                } else {
                                    1.0
                                }
                            } else if is_hover && show_supply_lines {
                                0.5 // Make hovered stand out against the other supply lines.
                            } else {
                                0.3
                            };

                            layer.roads.draw_path(
                                path.iter(),
                                Some(u32::MAX), // Existing supply lines must be valid.
                                usize::MAX,
                                true,
                                |id| get_visibility(id) * alpha,
                            );
                        }
                    }
                }
            }

            fn draw_shield(
                layer: &mut PathLayer,
                position: Vec2,
                intensity: f32,
                radius: f32,
                color: Color,
                selected: bool,
            ) {
                if intensity <= 0.0 || radius <= 0.0 {
                    return;
                }

                layer.draw_circle(
                    position,
                    radius,
                    selected.then_some(Vec3::splat(1.0).extend(0.33)),
                    (intensity > 0.0).then(|| color.shield_color().extend(intensity.sqrt())),
                );
            }

            let (shield_intensity, shield_radius) = tower_shield_intensity_radius(tower);
            let color = Color::new(context, tower.player_id);

            if zoom_per_pixel < 0.4 {
                draw_shield(
                    &mut layer.paths,
                    tower_position,
                    shield_intensity,
                    shield_radius,
                    color,
                    selected,
                );
            }

            let mut nuke = None;
            for force in &tower.inbound_forces {
                if force.units.contains(Unit::Nuke)
                    && (force.units.len() == 1
                        || (!tower.units.is_empty() && tower.player_id != force.player_id))
                {
                    let color = Color::new(context, force.player_id);
                    nuke = nuke.max(Some(color.make_gray_red()));
                }
            }
            if let Some(color) = nuke {
                let t = (renderer.time * PI).sin();
                let angle = (t * 0.075 + 0.25) * PI;
                let scale = shield_radius.max(0.55) * 3.6 + t * 0.075;
                let (stroke, _) = color.colors(true, hovered, selected);

                layer.paths.draw_path_a(
                    PathId::Target,
                    tower_position,
                    angle,
                    scale,
                    stroke.map(|v| v.extend(0.45)),
                    None,
                    false,
                );
            }

            let active = tower.active();
            let (stroke_color, fill_color) = color.colors(active, hovered, selected);

            // TODO draw simple sprite above certain zoom_per_pixel.
            layer.paths.draw_path(
                PathId::Tower(tower.tower_type),
                tower_position,
                0.0,
                tower_scale,
                stroke_color,
                fill_color,
                active,
            );

            if show_similar_towers == Some(tower.tower_type) {
                let x = (renderer.time * PI).sin().abs();
                let scale = (zoom * 0.025).max(2.0) * 0.75;
                let offset = Vec2::new(0.0, tower_scale * 0.75 + scale * 0.45 + scale * (x * 0.12));
                let color = 1.0 - x * 0.1;

                layer.paths.draw_path(
                    PathId::Marker,
                    tower_position + offset,
                    0.0,
                    scale,
                    Some(Vec3::splat(color * 1.0)),
                    Some(Vec3::splat(color * 0.73)),
                    true,
                )
            }

            let (stroke_color, fill_color) = color.colors(true, hovered, selected);
            if zoom_per_pixel < 0.2 {
                for unit_layout in tower_layout(tower, context.client.time_seconds) {
                    layer.paths.draw_path(
                        PathId::Unit(unit_layout.unit),
                        tower_position + unit_layout.relative_position,
                        unit_layout.angle,
                        unit_layout.scale,
                        stroke_color,
                        fill_color,
                        unit_layout.active,
                    );
                }
            }

            let mut draw_force = |force: &Force| {
                let force_position =
                    force.interpolated_position(context.state.game.time_since_last_tick);

                let color = Color::new(context, force.player_id);
                let (stroke_color, fill_color) = color.colors(true, hovered, selected);

                let (shield_intensity, shield_radius) =
                    shield_intensity_radius(force.units.available(Unit::Shield));
                draw_shield(
                    &mut layer.paths,
                    force_position,
                    shield_intensity,
                    shield_radius,
                    color,
                    false,
                );

                for unit_layout in force_layout(force) {
                    layer.paths.draw_path(
                        PathId::Unit(unit_layout.unit),
                        force_position + unit_layout.relative_position,
                        unit_layout.angle,
                        unit_layout.scale,
                        stroke_color,
                        fill_color,
                        unit_layout.active,
                    );
                }
            };

            if zoom_per_pixel < 0.4 {
                // Draw inbound forces and outbound forces heading to invisible towers.
                tower
                    .inbound_forces
                    .iter()
                    .for_each(|force| draw_force(force));
                tower
                    .outbound_forces
                    .iter()
                    .filter(|f| !is_visible(context, f.current_destination()))
                    .for_each(|force| draw_force(force));
            }

            if !context.state.game.tight_viewport.contains(tower_id) {
                continue;
            }

            if let Some(player_id) = tower.player_id {
                self.territories.record(tower_id, player_id);
            }
        }

        // Draw keys.
        if context.client.rewarded_ads
            && let Some((key, opacity)) = self.key_dispenser.key(context.client.time_seconds)
            && is_visible(context, key)
        {
            let (stroke, fill) = Color::Blue.colors(true, hovered_tower_id == Some(key), false);
            layer.paths.draw_path_a(
                PathId::Key,
                key.as_vec2() + Vec2::new(0.0, 1.5),
                0.0,
                1.0,
                stroke.map(|s| s.extend(opacity)),
                fill.map(|f| f.extend(opacity)),
                false,
            )
        }

        self.animations.retain(|animation| {
            animation.render(
                |center: Vec2, radius: f32, color: Vec4| {
                    layer.paths.draw_path_a(
                        PathId::Explosion,
                        center,
                        0.0,
                        radius,
                        None,
                        Some(color),
                        false,
                    );
                },
                context.client.time_seconds,
            )
        });

        self.territories
            .update(elapsed_seconds, |player_id, center, count| {
                if let Some(player) = context.state.core.player_or_bot(player_id) {
                    let outgoing_request = me
                        .map(|me| {
                            context
                                .state
                                .game
                                .world
                                .player(me)
                                .allies
                                .contains(&player_id)
                        })
                        .unwrap_or(false);
                    let incoming_request = me
                        .map(|me| {
                            context
                                .state
                                .game
                                .world
                                .player(player_id)
                                .allies
                                .contains(&me)
                        })
                        .unwrap_or(false);

                    let is_me = me == Some(player_id);
                    let color = if is_me {
                        Vec3::splat(0.88)
                    } else {
                        Vec3::splat(0.67)
                    };

                    if !is_me || zoom > 30.0 {
                        let tower_area = count as f32 * (TowerId::CONVERSION as f32).powi(2);
                        let max_text_height = tower_area.sqrt() * 0.5;
                        let text_height = (zoom * 0.05).min(max_text_height);
                        let center = center + Vec2::Y * (text_height * 0.5 + 1.0);

                        layer.text.draw(
                            player.alias.as_str(),
                            center,
                            text_height,
                            [color.x, color.y, color.z, 1.0].map(|c| (c * 255.0) as u8),
                            TextStyle::italic_if(
                                context
                                    .state
                                    .core
                                    .player_or_bot(player_id)
                                    .map(|p| p.authentic)
                                    .unwrap_or(false),
                            ),
                        );
                        if outgoing_request ^ incoming_request {
                            let alliance_color = if incoming_request {
                                Color::Purple
                            } else {
                                Color::Gray
                            };
                            let (stroke, fill) = alliance_color.ui_colors();
                            layer.paths.draw_path(
                                PathId::RequestAlliance,
                                center + Vec2::new(0.0, text_height * 0.8),
                                0.0,
                                text_height * 0.7,
                                stroke,
                                fill,
                                false,
                            );
                        }
                    }
                }
            });

        Self::draw_drag_path(
            self.drag,
            self.selected_tower_id,
            &get_visibility,
            context,
            layer,
        );

        frame.end(&self.camera);
    }

    fn ui(&mut self, event: KiometUiEvent, context: &mut ClientContext<Self>) {
        match event {
            KiometUiEvent::Alliance {
                with,
                break_alliance,
            } => {
                context.send_to_game(Command::Alliance {
                    with,
                    break_alliance,
                });
                self.close_tower_menu();
            }
            KiometUiEvent::DismissCaptureTutorial => {
                self.tutorial.dismiss_capture();
            }
            KiometUiEvent::DismissUpgradeTutorial => {
                self.tutorial.dismiss_upgrade();
            }
            KiometUiEvent::Spawn(alias) => {
                context.send_to_game(Command::Spawn(alias));
            }
            KiometUiEvent::PanTo(tower_id) => {
                self.pan_zoom.pan_to(tower_id.as_vec2());
            }
            KiometUiEvent::Upgrade {
                tower_id,
                tower_type,
            } => {
                if let Some(unlocks) = context.settings.unlocks.unlock(tower_type) {
                    context
                        .settings
                        .set_unlocks(unlocks, &mut context.browser_storages);
                }
                context.send_to_game(Command::Upgrade {
                    tower_id,
                    tower_type,
                });
                self.close_tower_menu();
            }
            KiometUiEvent::Unlock(tower_type) => {
                if let Some(unlocks) = context.settings.unlocks.unlock(tower_type) {
                    context
                        .settings
                        .set_unlocks(unlocks, &mut context.browser_storages);
                }
                self.lock_dialog = None;
            }
            KiometUiEvent::LockDialog(show) => {
                self.lock_dialog = show;
            }
        }
    }

    fn update(&mut self, elapsed_seconds: f32, context: &mut ClientContext<Self>) {
        let me = context.player_id();

        // Has it's own method of determining ticked (because it's used in peek_mouse).
        update_visible(context);

        if let Some(world_space) = context
            .mouse
            .view_position
            .map(|v| self.camera.to_world_position(v))
        {
            // Must come after visibility update.
            self.move_world_space(world_space, context);
        }

        let ticked = std::mem::take(&mut context.state.game.ticked);
        if ticked {
            self.tutorial.update(context);
            if context.client.rewarded_ads && self.key_dispenser.update(context) {
                context.settings.set_unlocks(
                    context.settings.unlocks.add_key(),
                    &mut context.browser_storages,
                );
            }
        }

        if context.keyboard.is_down(Key::R) && context.keyboard.is_down(Key::Shift) {
            if let Some(tower_id) = self.selected_tower_id {
                // Clear supply line of selected tower.
                if let Some(tower) = context.state.game.world.chunk.get(tower_id) {
                    if tower.supply_line.is_some() {
                        context.send_to_game(Command::SetSupplyLine {
                            tower_id,
                            path: None,
                        })
                    }
                }
            } else if ticked {
                // Clear all visible supply lines (but only 1 per tick).
                let tower = context
                    .state
                    .game
                    .visible
                    .iter(&context.state.game.world.chunk)
                    .filter(|&(id, t)| {
                        context.state.game.margin_viewport.contains(id)
                            && t.supply_line.is_some()
                            && t.player_id.is_some()
                            && t.player_id == me
                    })
                    .next();
                if let Some((tower_id, _)) = tower {
                    // TODO iter viewport intersection visible and towers.
                    context.send_to_game(Command::SetSupplyLine {
                        tower_id,
                        path: None,
                    });
                }
            }
        }

        self.pan_zoom
            .set_aspect_ratio(self.render_chain.renderer().aspect_ratio());

        if context.cheats() && context.keyboard.is_down(Key::B) {
            self.pan_zoom.set_bounds(
                Vec2::splat(-100.0),
                Vec2::splat(WorldChunks::SIZE as f32 * TowerId::CONVERSION as f32 + 100.0),
                true,
            );
        } else if context.state.game.bounding_rectangle.is_valid() {
            let bounding_rectangle = context.state.game.bounding_rectangle;
            let bottom_left = bounding_rectangle.bottom_left.floor_position();
            let top_right = bounding_rectangle.top_right.ceil_position();

            self.pan_zoom.set_bounds(
                bottom_left,
                top_right,
                context.cheats() && context.keyboard.is_down(Key::N),
            );
        }

        context.audio.set_muted_by_game(!context.state.game.alive);

        if context.state.game.alive {
            if !context.audio.is_playing(Audio::Music) {
                context.audio.play(Audio::Music);
            }

            if !self.was_alive {
                self.pan_zoom.reset_center();
                self.pan_zoom.reset_zoom()
            }

            let mut pan = Vec2::ZERO;
            let mut any = false;

            if context
                .keyboard
                .state(Key::Left)
                .combined(context.keyboard.state(Key::A))
                .is_down()
            {
                pan.x += 1.0;
                any = true;
            }
            if context
                .keyboard
                .state(Key::Right)
                .combined(context.keyboard.state(Key::D))
                .is_down()
            {
                pan.x -= 1.0;
                any = true;
            }
            if context
                .keyboard
                .state(Key::Down)
                .combined(context.keyboard.state(Key::S))
                .is_down()
            {
                pan.y += 1.0;
                any = true;
            }
            if context
                .keyboard
                .state(Key::Up)
                .combined(context.keyboard.state(Key::W))
                .is_down()
            {
                pan.y -= 1.0;
                any = true;
            }
            self.pan_zoom
                .pan(pan * elapsed_seconds * self.pan_zoom.get_zooms().max_element() * 1.5);

            if context.keyboard.is_down(Key::H) {
                if let Some(king) = context.state.game.alerts.ruler_position {
                    self.pan_zoom.pan_to(king.as_vec2());
                }
            }

            let mut zoom = 1.0;
            if context.keyboard.state(Key::Q).is_down() {
                zoom -= (elapsed_seconds * 2.5).min(1.0);
                any = true;
            }
            if context.keyboard.state(Key::E).is_down() {
                zoom += (elapsed_seconds * 2.5).min(1.0);
                any = true;
            }
            self.pan_zoom
                .multiply_zoom(self.pan_zoom.get_center(), zoom);

            // Hide tower menu on keyboard movement.
            if any {
                self.close_tower_menu();
            }
        } else {
            context.audio.stop_playing(Audio::Music);
            self.selected_tower_id = None;
            self.drag = None;
            self.pan_zoom.reset_center();
            self.pan_zoom.reset_zoom();
        }

        // Time passed.
        context.state.game.time_since_last_tick += elapsed_seconds;

        for InfoEvent { position, info } in std::mem::take(&mut context.state.game.info_events) {
            let volume = 1.0 / (1.0 + position.distance(self.pan_zoom.get_center()));

            let animation_type = match info {
                Info::Emp(player_id) => {
                    let color = Color::new(context, player_id);
                    Some(AnimationType::Emp(color.make_gray_red()))
                }
                Info::NuclearExplosion => Some(AnimationType::NuclearExplosion),
                Info::ShellExplosion => Some(AnimationType::ShellExplosion),
                _ => None,
            };

            if let Some(animation_type) = animation_type {
                self.animations.push(Animation::new(
                    position,
                    animation_type,
                    context.client.time_seconds,
                ));
            }

            match info {
                Info::GainedTower {
                    player_id, reason, ..
                } if Some(player_id) == me
                    && matches!(reason, GainedTowerReason::CapturedFrom(_)) =>
                {
                    context.audio.play_with_volume(Audio::Success, volume);
                }
                Info::LostTower { player_id, .. } if Some(player_id) == me => {
                    context.audio.play_with_volume(Audio::Loss, volume);
                }
                Info::LostForce(player_id) if Some(player_id) == me => {
                    context.audio.play_with_volume(Audio::Pain, volume);
                }
                _ => {}
            }
        }

        let center = self.pan_zoom.get_center();
        let bottom_left = center - self.pan_zoom.get_zooms();
        let top_right = center + self.pan_zoom.get_zooms();
        context.state.game.tight_viewport =
            TowerRectangle::new(TowerId::floor(bottom_left), TowerId::ceil(top_right));
        context.state.game.margin_viewport = context.state.game.tight_viewport.add_margin(2);

        let send_viewport = ChunkRectangle::from(context.state.game.margin_viewport);
        self.set_viewport_rate_limit.update(elapsed_seconds);
        if send_viewport != context.state.game.set_viewport && self.set_viewport_rate_limit.ready()
        {
            context.state.game.set_viewport = send_viewport;
            context.send_to_game(Command::SetViewport(send_viewport));
        }

        context.set_ui_props(
            KiometUiProps {
                lock_dialog: self.lock_dialog,
                alive: context.state.game.alive,
                death_reason: context.state.game.death_reason.into(),
                selected_tower: self.selected_tower_id.and_then(|tower_id| {
                    // Don't obstruct drag.
                    if self.drag.is_some() {
                        return None;
                    }
                    context
                        .state
                        .game
                        .world
                        .chunk
                        .get(tower_id)
                        .cloned()
                        .map(|tower| SelectedTower {
                            client_position: to_client_position(&self.camera, tower_id.as_vec2()),
                            color: Color::new(context, tower.player_id),
                            outgoing_alliance: context
                                .state
                                .core
                                .player_id
                                .zip(tower.player_id)
                                .map(|(us, them)| {
                                    context.state.game.world.player(us).allies.contains(&them)
                                })
                                .unwrap_or(false),
                            tower,
                            tower_id,
                        })
                }),
                tower_counts: context.state.game.tower_counts,
                alerts: context.state.game.alerts,
                tutorial_alert: self.tutorial.alert(),
                unlocks: context.settings.unlocks.clone(),
            },
            context.state.game.alive,
        );

        self.was_alive = context.state.game.alive;
    }
}

/// Should attempts to send the player's ruler through this tower be warned against?
fn is_perilous(context: &ClientContext<KiometGame>, tower_id: TowerId) -> bool {
    context
        .state
        .game
        .world
        .chunk
        .get(tower_id)
        .map(|tower| {
            // Different player or unclaimed land is perilous.
            tower.player_id != context.player_id()
        })
        .unwrap_or(false)
}

impl KiometGame {
    fn close_tower_menu(&mut self) {
        // Ui is already hidden while dragging.
        if self.drag.is_none() {
            self.selected_tower_id = None;
        }
    }

    fn draw_drag_path(
        drag: Option<Drag>,
        selected_tower_id: Option<TowerId>,
        get_visibility: &impl Fn(TowerId) -> f32,
        context: &ClientContext<KiometGame>,
        layer: &mut TowerLayer,
    ) {
        if let Some((start, current, current_start_time)) = Drag::zip(drag) {
            let Some(source_tower) = context.state.game.world.chunk.get(start) else {
                return;
            };
            if source_tower.player_id.is_none() || source_tower.player_id != context.player_id() {
                return;
            }

            // TODO don't duplicate this code with find best incomplete path.
            let strength = source_tower.force_units();
            let tower_edge_distance = source_tower.tower_type.ranged_distance();
            let strength_edge_distance =
                (!strength.is_empty()).then(|| strength.max_edge_distance());
            let max_edge_distance =
                strength_edge_distance.map_or(tower_edge_distance, |e| e.min(tower_edge_distance));
            let shorter_max_edge_distance = max_edge_distance != tower_edge_distance;

            let do_supply_line = selected_tower_id.is_some()
                && source_tower.generates_mobile_units()
                && !shorter_max_edge_distance;

            // Can drag supply lines even without units.
            if strength.is_empty() && !do_supply_line {
                return;
            }

            let mut perilous = false;
            let viable = layer.roads.draw_path(
                context
                    .state
                    .game
                    .world
                    .find_best_incomplete_path(
                        start,
                        current,
                        max_edge_distance,
                        context.player_id().unwrap(),
                        &|tower_id| is_visible(context, tower_id),
                    )
                    .into_iter()
                    .filter(|&tower_id| tower_id != current)
                    .chain(std::iter::once(current))
                    .inspect(|&tower_id| perilous |= is_perilous(context, tower_id)),
                max_edge_distance,
                World::MAX_PATH_ROADS,
                do_supply_line,
                get_visibility,
            );

            if viable && perilous && strength.contains(Unit::Ruler) {
                let progress = (context.client.time_seconds - current_start_time)
                    * (1.0 / Self::RULER_DRAG_DELAY);
                let ready = progress > 1.0;
                // Snap to provide a clear indication of waiting long enough.
                let fade = if ready { 1.0 } else { progress * 0.6 };
                let (stroke, fill) = Color::Blue.colors(false, true, ready);
                layer.paths.draw_path_a(
                    PathId::Unit(Unit::Ruler),
                    current.as_vec2(),
                    0.0,
                    1.8,
                    stroke.map(|stroke| stroke.extend(fade)),
                    fill.map(|fill| fill.extend(fade * 0.8)),
                    false,
                )
            }
        }
    }
}

pub fn exists(context: &ClientContext<KiometGame>, tower_id: TowerId) -> bool {
    context.state.game.world.chunk.get(tower_id).is_some()
}

pub fn is_visible(context: &ClientContext<KiometGame>, tower_id: TowerId) -> bool {
    context.state.game.visible.contains(tower_id)
}

/// Updates the visible towers (only does work each game tick).
fn update_visible(context: &mut ClientContext<KiometGame>) {
    let Some(me) = context.player_id() else {
        return;
    };

    let all_visible =
        !context.state.game.alive || (context.cheats() && context.keyboard.is_down(Key::B));
    context
        .state
        .game
        .visible
        .update(&context.state.game.world, me, all_visible)
}

fn get_closest(point: Vec2, context: &ClientContext<KiometGame>) -> Option<TowerId> {
    TowerId::closest(point).and_then(|center| {
        context
            .state
            .game
            .world
            .chunk
            .iter_towers_square(center, 1)
            .filter(|(tower_id, _)| is_visible(context, *tower_id))
            .fold(None, |best: Option<TowerId>, (pos, _)| {
                if best
                    .map(|best| {
                        pos.as_vec2().distance_squared(point)
                            < best.as_vec2().distance_squared(point)
                    })
                    .unwrap_or(true)
                {
                    Some(pos)
                } else {
                    best
                }
            })
    })
}

/// TODO find a place in engine for this.
pub fn to_client_position(camera: &Camera2d, world_position: Vec2) -> IVec2 {
    // In the range [0, 1] divided by the device pixel ratio.
    let zero_to_one = (camera.to_view_position(world_position) + 1.0)
        * (0.5 / js_hooks::window().device_pixel_ratio() as f32);
    (zero_to_one * camera.viewport.as_vec2()).as_ivec2()
}

fn shield_intensity_radius_inner(shield: usize, scale: f32) -> (f32, f32) {
    let shield_intensity = shield as f32 * (1.0 / Units::CAPACITY as f32);
    let shield_radius = (0.5 * scale + shield_intensity * 2.0).min(0.9 * scale);
    (shield_intensity, shield_radius)
}

fn shield_intensity_radius(shield: usize) -> (f32, f32) {
    shield_intensity_radius_inner(shield, 1.0)
}

fn tower_shield_intensity_radius(tower: &Tower) -> (f32, f32) {
    shield_intensity_radius_inner(
        tower.units.available(Unit::Shield),
        tower.tower_type.scale() as f32,
    )
}
