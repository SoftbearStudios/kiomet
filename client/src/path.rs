// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use common::tower::TowerType;
use common::unit::Unit;
use kodiak_client::fxhash::FxHashMap;
use kodiak_client::glam::{Vec2, Vec3, Vec4};
use kodiak_client::renderer::{
    self, derive_vertex, include_shader, DefaultRender, InstanceLayer, Layer, MeshBuilder,
    RenderLayer, Renderer, Shader,
};
use kodiak_client::renderer2d::Camera2d;
use lyon_path::math::Vector;
use lyon_path::path::Builder;
use lyon_svg::path::PathEvent;
use lyon_svg::path_utils::PathSerializer;
use lyon_tessellation::geometry_builder::simple_builder;
use lyon_tessellation::math::{point, size, Point, Rect, Size};
use lyon_tessellation::path::builder::PathBuilder;
use lyon_tessellation::path::{Path, Winding};
use lyon_tessellation::{
    FillOptions, FillTessellator, StrokeOptions, StrokeTessellator, VertexBuffers,
};
use std::cell::RefCell;

const STROKE_WIDTH: f32 = 0.05;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PathId {
    Circle(u8),
    Tower(TowerType),
    Unit(Unit),
    /// Chopper blades, maybe blurred.
    Blades(bool),
    /// Nuke ground-zero warning.
    Target,
    /// Marker for finding specific towers.
    Marker,
    /// Explosions are circles that render on top of everything game-related.
    Explosion,
    /// Break alliance with another player.
    BreakAlliance,
    /// Request alliance with another player.
    RequestAlliance,
    /// Cursor renders on top of everything.
    Cursor,
    /// Key is HUD-only.
    Key,
}

impl PathId {
    fn path(self) -> Path {
        match self {
            PathId::Blades(blurred) => blades(blurred),
            PathId::BreakAlliance => break_alliance(),
            PathId::Circle(radius) => circle(radius as f32),
            PathId::Cursor => cursor(),
            PathId::Explosion => circle(1.0),
            PathId::Key => key(),
            PathId::Marker => marker(),
            PathId::RequestAlliance => request_alliance(),
            PathId::Target => target(),
            PathId::Tower(tower_type) => tower(tower_type),
            PathId::Unit(u) => unit(u),
        }
    }
}

#[derive(Copy, Clone)]
struct Transform {
    translation: Vec2,
    rotation: f32,
    scale: f32,
}

impl From<Transform> for Vec4 {
    fn from(t: Transform) -> Self {
        t.translation.extend(t.rotation).extend(t.scale)
    }
}

derive_vertex!(
    pub struct Instance {
        transform: Vec4,
        color: Vec4,
    }
);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MeshId {
    path_id: PathId,
    /// Use opposite of fill so true > false and outline renders above filled.
    outline: bool,
}

type PathInstanceLayer = InstanceLayer<Vec2, u16, Instance, MeshId>;

#[derive(Layer)]
pub struct PathLayer {
    #[layer]
    instances: PathInstanceLayer,
    shader: Shader,
}

impl RenderLayer<&Camera2d> for PathLayer {
    fn render(&mut self, renderer: &Renderer, camera: &Camera2d) {
        if let Some(binding) = self.shader.bind(renderer) {
            camera.prepare(&binding);
            self.instances.render(renderer, &binding);
        }
    }
}

impl DefaultRender for PathLayer {
    fn new(renderer: &Renderer) -> Self {
        Self {
            instances: PathInstanceLayer::new(renderer),
            shader: include_shader!(renderer, "path"),
        }
    }
}

impl PathLayer {
    pub fn draw_circle(
        &mut self,
        center: Vec2,
        radius: f32,
        stroke: Option<Vec4>,
        fill: Option<Vec4>,
    ) {
        debug_assert!(radius >= 0.0);
        if radius <= 0.0 {
            return;
        }
        let geo_radius = radius.ceil();
        self.draw_path_a(
            PathId::Circle(geo_radius as u8),
            center,
            0.0,
            radius / geo_radius,
            stroke,
            fill,
            false,
        );
    }

    pub fn draw_path(
        &mut self,
        path_id: PathId,
        center: Vec2,
        angle: f32,
        scale: f32,
        stroke: Option<Vec3>,
        fill: Option<Vec3>,
        active: bool,
    ) {
        let e = |v: Vec3| v.extend(1.0);
        let stroke = stroke.map(e);
        let fill = fill.map(e);
        self.draw_path_a(path_id, center, angle, scale, stroke, fill, active);
    }

    pub fn draw_path_a(
        &mut self,
        path_id: PathId,
        center: Vec2,
        angle: f32,
        scale: f32,
        stroke: Option<Vec4>,
        fill: Option<Vec4>,
        active: bool,
    ) {
        let transform = Transform {
            translation: center,
            rotation: angle,
            scale,
        }
        .into();

        if path_id == PathId::Unit(Unit::Chopper) {
            let a = if active { 0.15 } else { 1.0 };
            let stroke = stroke.map(|f| f.truncate().extend(f.w * a));
            let fill = fill.map(|f| f.truncate().extend(f.w * a));
            self.draw_path_a(
                PathId::Blades(active),
                center,
                angle,
                scale,
                stroke,
                fill,
                active,
            );
        }

        if let Some(fill) = fill {
            self.draw_path_inner(
                path_id,
                Instance {
                    transform,
                    color: fill,
                },
                true,
            );
        }
        if let Some(stroke) = stroke {
            self.draw_path_inner(
                path_id,
                Instance {
                    transform,
                    color: stroke,
                },
                false,
            );
        }
    }

    fn draw_path_inner(&mut self, path_id: PathId, instance: Instance, fill: bool) {
        let mesh_id = MeshId {
            path_id,
            outline: !fill,
        };
        self.instances.draw(mesh_id, instance, || {
            let buffers = Self::create_mesh(mesh_id);
            let mut mesh = MeshBuilder::new();
            mesh.vertices = bytemuck::allocation::cast_vec(buffers.vertices);
            mesh.indices = buffers.indices;
            mesh
        });
    }

    fn create_mesh(mesh_id: MeshId) -> VertexBuffers<Point, u16> {
        let path = &mesh_id.path_id.path();

        let mut buffers: VertexBuffers<Point, u16> = VertexBuffers::new();
        let mut vertex_builder = simple_builder(&mut buffers);

        let tolerance = 0.005 / 10.0;

        let _res = if !mesh_id.outline {
            FillTessellator::new().tessellate_path(
                path,
                &FillOptions::default().with_tolerance(tolerance),
                &mut vertex_builder,
            )
        } else {
            StrokeTessellator::new().tessellate_path(
                path,
                &StrokeOptions::default()
                    .with_line_width(STROKE_WIDTH)
                    .with_tolerance(tolerance),
                &mut vertex_builder,
            )
        };
        #[cfg(debug_assertions)]
        _res.unwrap();

        buffers
    }
}

#[derive(Default)]
pub struct SvgCache {
    svg: FxHashMap<PathId, SvgEntry>,
}

struct SvgEntry {
    /// Just the path of the SVG.
    path: &'static str,
    /// Base 64 encoded complete SVGs. They're [`None`] until requested.
    colored: [Option<&'static str>; std::mem::variant_count::<Color>()],
}

impl SvgCache {
    pub fn get(path_id: PathId, color: Color) -> &'static str {
        thread_local! {
             static S: RefCell<Option<SvgCache>> = RefCell::new(None);
        }
        S.with(|s: &RefCell<Option<SvgCache>>| {
            s.borrow_mut()
                .get_or_insert_default()
                .get_inner(path_id, color)
        })
    }

    fn get_inner(&mut self, path_id: PathId, color: Color) -> &'static str {
        fn color_to_string(color: Option<Vec3>) -> String {
            if let Some(color) = color {
                let mut hex = 0u32;
                hex |= ((color.x * 255.0) as u8 as u32) << 24;
                hex |= ((color.y * 255.0) as u8 as u32) << 16;
                hex |= ((color.z * 255.0) as u8 as u32) << 8;
                hex |= (/*color.w*/1f32 * 255.0) as u8 as u32;
                renderer::rgba_array_to_css(hex.to_be_bytes())
            } else {
                String::from("none")
            }
        }

        let entry = self.get_svg_entry(path_id);
        entry.colored[color as usize].get_or_insert_with(|| {
            let (stroke, fill) = color.ui_colors();
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" fill="{}" stroke="{}" stroke-width="{}"><path d="{}"/></svg>"##,
                -0.5 - STROKE_WIDTH * 0.5,
                -0.5 - STROKE_WIDTH * 0.5,
                1.0 + STROKE_WIDTH,
                1.0 + STROKE_WIDTH,
                color_to_string(fill),
                color_to_string(stroke),
                STROKE_WIDTH,
                entry.path,
            );

            format!("data:image/svg+xml;base64,{}", base64::encode(svg)).leak()
        })
    }

    fn get_svg_entry(&mut self, path_id: PathId) -> &mut SvgEntry {
        self.svg.entry(path_id).or_insert_with(|| {
            use lyon_path::builder::{Build, SvgPathBuilder};
            let mut svg_builder = PathSerializer::new();

            // WARNING: This makes several assumptions:
            // - paths were constructed with the normal path builder
            // - all paths are closed
            // Both are mostly checked in debug mode.

            // NOTE: The y coordinate is negated, since SVG's origin is on top.

            #[cfg(debug_assertions)]
            let mut last = None;

            for event in &path_id.path() {
                #[cfg_attr(not(debug_assertions), allow(unused))]
                match event {
                    PathEvent::Begin { at } => {
                        svg_builder.move_to(point(at.x, -at.y));
                        #[cfg(debug_assertions)]
                        {
                            assert_eq!(last, None, "{:?}", path_id);
                            last = Some(at);
                        }
                    }
                    PathEvent::Line { from, to } => {
                        svg_builder.line_to(point(to.x, -to.y));
                        #[cfg(debug_assertions)]
                        {
                            assert_eq!(last, Some(from), "{:?}", path_id);
                            last = Some(to);
                        }
                    }
                    PathEvent::Quadratic { from, ctrl, to } => {
                        svg_builder.quadratic_bezier_to(point(ctrl.x, -ctrl.y), point(to.x, -to.y));
                        #[cfg(debug_assertions)]
                        {
                            assert_eq!(last, Some(from), "{:?}", path_id);
                            last = Some(to);
                        }
                    }
                    PathEvent::Cubic {
                        from,
                        ctrl1,
                        ctrl2,
                        to,
                    } => {
                        svg_builder.cubic_bezier_to(
                            point(ctrl1.x, -ctrl1.y),
                            point(ctrl2.x, -ctrl2.y),
                            point(to.x, -to.y),
                        );
                        #[cfg(debug_assertions)]
                        {
                            assert_eq!(last, Some(from), "{:?}", path_id);
                            last = Some(to);
                        }
                    }
                    PathEvent::End { close, .. } => {
                        svg_builder.close();
                        #[cfg(debug_assertions)]
                        {
                            assert!(close, "{:?}", path_id);
                            assert_ne!(last, None, "{:?}", path_id);
                            last = None;
                        }
                    }
                }
            }
            SvgEntry {
                path: svg_builder.build().leak(),
                colored: Default::default(),
            }
        })
    }
}

fn rect(center: Point, size: Size) -> Rect {
    Rect::new(
        point(center.x - size.width * 0.5, center.y - size.height * 0.5),
        size,
    )
}

fn circle(radius: f32) -> Path {
    let mut p = Path::builder();
    // 0.5 as that is the offset.
    p.add_circle(pt(0.5, 0.5), radius, Winding::Positive);
    p.build()
}

fn tower(tower_type: TowerType) -> Path {
    use TowerType::*;
    match tower_type {
        Airfield => airstrip(0.4),
        Armory => armory(),
        Artillery => artillery(),
        Barracks => barracks(),
        Bunker => bunker(),
        //Capitol => capitol(),
        Centrifuge => centrifuge(),
        City => city(),
        Cliff => cliff(),
        Ews => ews(),
        Factory => factory(),
        Generator => generator(),
        Headquarters => headquarters(),
        Helipad => helipad(),
        //Icbm => icbm(),
        //Laser => laser(),
        Launcher => launcher(),
        //Metropolis => metropolis(),
        Mine => mine(),
        Projector => projector(),
        Quarry => quarry(),
        Radar => radar(),
        Rampart => rampart(),
        Reactor => reactor(),
        Refinery => refinery(),
        Rocket => rocket(),
        Runway => airstrip(0.25),
        Satellite => satellite(),
        Silo => silo(),
        Town => town(),
        Village => village(),
    }
}

fn unit(unit: Unit) -> Path {
    match unit {
        Unit::Bomber => bomber(),
        Unit::Chopper => chopper(),
        Unit::Emp => emp(),
        Unit::Fighter => fighter(),
        Unit::Nuke => nuke(),
        Unit::Ruler => ruler(),
        Unit::Shell => shell(),
        Unit::Shield => circle(0.4),
        Unit::Soldier => soldier(),
        Unit::Tank => tank(),
    }
}

/// Maps 0..1 to -0.5..0.5
fn pt(x: f32, y: f32) -> Point {
    point(x - 0.5, y - 0.5)
}

fn offset_pt(scale: f32, offset: Vec2) -> impl Fn(f32, f32) -> Point + Copy {
    move |x, y| {
        let pos = (Vec2::new(x, y) - 0.5) * scale + 0.5 + offset;
        pt(pos.x, pos.y)
    }
}

fn target() -> Path {
    let mut p = Path::builder();
    const RADIUS: f32 = 0.4;
    p.add_circle(pt(0.5, 0.5), RADIUS, Winding::Positive);

    const LENGTH: f32 = 0.2;
    // Left line.
    p.begin(pt(0.5 - RADIUS - LENGTH * 0.5, 0.5));
    p.line_to(pt(0.5 - RADIUS + LENGTH * 0.5, 0.5));
    p.end(false);
    // Right line.
    p.begin(pt(0.5 + RADIUS - LENGTH * 0.5, 0.5));
    p.line_to(pt(0.5 + RADIUS + LENGTH * 0.5, 0.5));
    p.end(false);
    // Bottom line.
    p.begin(pt(0.5, 0.5 - RADIUS - LENGTH * 0.5));
    p.line_to(pt(0.5, 0.5 - RADIUS + LENGTH * 0.5));
    p.end(false);
    // Top line.
    p.begin(pt(0.5, 0.5 + RADIUS - LENGTH * 0.5));
    p.line_to(pt(0.5, 0.5 + RADIUS + LENGTH * 0.5));
    p.end(false);
    p.build()
}

fn marker() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.2, 0.8));
    p.line_to(pt(0.5, 0.7));
    p.line_to(pt(0.8, 0.8));
    p.line_to(pt(0.5, 0.15));
    p.close();
    p.build()
}

fn airstrip(width: f32) -> Path {
    let mut p = Path::builder();
    p.add_rectangle(&rect(pt(0.5, 0.5), size(1.0, width)), Winding::Positive);
    const COUNT: u8 = 5;
    for i in 0..=COUNT {
        p.add_rectangle(
            &rect(
                pt((i + 1) as f32 / (COUNT + 2) as f32, 0.5),
                size(0.5 / (COUNT + 2) as f32, 0.02),
            ),
            Winding::Negative,
        );
    }
    p.build()
}

fn armory() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.6));
    p.quadratic_bezier_to(pt(0.5, 0.8), pt(0.9, 0.6));
    p.line_to(pt(0.9, 0.2));

    // Door
    p.line_to(pt(0.6, 0.2));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.4, 0.2));

    p.close();
    p.build()
}

fn artillery() -> Path {
    let mut p = Path::builder();
    let pt = offset_pt(1.4, Vec2::new(-0.08, 0.0));

    // Base
    p.begin(pt(0.25, 0.35));
    p.line_to(pt(0.35, 0.4));
    p.line_to(pt(0.4, 0.55));
    p.line_to(pt(0.3, 0.525));
    p.line_to(pt(0.275, 0.6));
    p.line_to(pt(0.575, 0.7));
    p.line_to(pt(0.6, 0.625));
    p.line_to(pt(0.475, 0.575));
    p.quadratic_bezier_to(pt(0.625, 0.55), pt(0.75, 0.35));
    p.close();

    // Barrel
    p.begin(pt(0.5925, 0.6625));
    p.line_to(pt(0.8625, 0.75));
    p.close();

    p.build()
}

fn barracks() -> Path {
    let f = 0.8 / 3.0;
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.1, 0.5));
    p.cubic_bezier_to(pt(0.1, 0.75), pt(0.1 + f, 0.75), pt(0.1 + f, 0.5));
    p.cubic_bezier_to(
        pt(0.1 + f * 1.0, 0.75),
        pt(0.1 + f * 2.0, 0.75),
        pt(0.1 + f * 2.0, 0.5),
    );
    p.cubic_bezier_to(pt(0.1 + f * 2.0, 0.75), pt(0.9, 0.75), pt(0.9, 0.5));
    p.line_to(pt(0.9, 0.3));
    p.close();
    p.build()
}

fn bunker() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.quadratic_bezier_to(pt(0.5, 0.9), pt(0.9, 0.3));
    p.line_to(pt(0.6, 0.3));
    p.quadratic_bezier_to(pt(0.5, 0.5), pt(0.4, 0.3));
    p.close();
    p.build()
}

#[allow(unused)]
fn capitol() -> Path {
    let pt = offset_pt(1.0, Vec2::new(0.0, 0.075));
    let mut p = Path::builder();
    let side = 0.05;
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.3));
    p.line_to(pt(0.1 + side, 0.3));
    p.line_to(pt(0.1 + side, 0.5));
    p.line_to(pt(0.1, 0.5));
    p.line_to(pt(0.1, 0.6));
    p.line_to(pt(0.3, 0.6));
    p.quadratic_bezier_to(pt(0.3, 0.8), pt(0.5, 0.8));
    p.quadratic_bezier_to(pt(0.7, 0.8), pt(0.7, 0.6));
    p.line_to(pt(0.9, 0.6));
    p.line_to(pt(0.9, 0.5));
    p.line_to(pt(0.9 - side, 0.5));
    p.line_to(pt(0.9 - side, 0.3));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn city() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.1));
    p.line_to(pt(0.1, 0.2));
    p.line_to(pt(0.2, 0.2));
    p.line_to(pt(0.2, 0.7));
    p.line_to(pt(0.5, 0.8));
    p.line_to(pt(0.5, 0.6));
    p.line_to(pt(0.8, 0.6));
    p.line_to(pt(0.8, 0.2));
    p.line_to(pt(0.9, 0.2));
    p.line_to(pt(0.9, 0.1));
    p.close();
    p.build()
}

fn centrifuge() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.2, 0.2));
    p.line_to(pt(0.2, 0.3));
    p.line_to(pt(0.3, 0.4));
    p.line_to(pt(0.3, 0.8));
    p.line_to(pt(0.4, 0.8));
    p.line_to(pt(0.4, 0.9));
    p.line_to(pt(0.6, 0.9));
    p.line_to(pt(0.6, 0.8));
    p.line_to(pt(0.7, 0.8));
    p.line_to(pt(0.7, 0.4));
    p.line_to(pt(0.8, 0.3));
    p.line_to(pt(0.8, 0.2));
    p.close();
    p.build()
}

#[allow(unused)]
fn icbm() -> Path {
    let pt = offset_pt(1.25, Vec2::new(0.0, -0.15));
    let mut p = Path::builder();
    silo_inner(&mut p, pt);
    p.begin(pt(0.4, 0.65));
    p.line_to(pt(0.5, 0.9));
    p.line_to(pt(0.6, 0.65));
    p.line_to(pt(0.6, 0.55));
    p.quadratic_bezier_to(pt(0.5, 0.5), pt(0.4, 0.55));
    p.close();
    p.build()
}

#[allow(unused)]
fn laser() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.3, 0.1));
    // Body
    p.line_to(pt(0.3, 0.7));
    p.line_to(pt(0.7, 0.9));
    p.line_to(pt(0.7, 0.1));
    p.close();
    // Heat sinks
    for (x, dir) in [(0.3, -1.0), (0.7, 1.0)] {
        let x2 = x + dir * 0.09;
        for i in 0..4 {
            let y = 0.17 + i as f32 * 0.11;
            p.begin(pt(x, y));
            p.line_to(pt(x2, y));
            p.close()
        }
    }
    p.build()
}

fn launcher() -> Path {
    let mut p = Path::builder();
    // Base.
    p.begin(pt(0.1, 0.1));
    p.line_to(pt(0.1, 0.2));
    p.line_to(pt(0.35, 0.2));
    p.line_to(pt(0.35, 0.35));
    // Back of tube.
    p.line_to(pt(0.25, 0.25));
    p.line_to(pt(0.225, 0.275));
    p.line_to(pt(0.2, 0.275));
    p.line_to(pt(0.125, 0.35));
    p.line_to(pt(0.125, 0.375));
    p.line_to(pt(0.1, 0.4));
    // Top of tube.
    p.line_to(pt(0.6, 0.9));
    // Front of tube.
    p.line_to(pt(0.625, 0.875));
    p.quadratic_bezier_to(pt(0.85, 1.0), pt(0.725, 0.775));
    p.line_to(pt(0.75, 0.75));
    // Bottom of tube.
    p.line_to(pt(0.575, 0.575));
    // Front strut.
    p.line_to(pt(0.75, 0.2));
    // Base.
    p.line_to(pt(0.9, 0.2));
    p.line_to(pt(0.9, 0.1));
    p.close();
    // Cutout.
    p.begin(pt(0.45, 0.25));
    p.line_to(pt(0.625, 0.25));
    p.line_to(pt(0.5, 0.5));
    p.line_to(pt(0.45, 0.45));
    p.close();
    p.build()
}

fn headquarters() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.1, 0.5));
    p.line_to(pt(0.3, 0.5));
    p.line_to(pt(0.3, 0.7));
    p.line_to(pt(0.7, 0.7));
    p.line_to(pt(0.7, 0.5));
    p.line_to(pt(0.9, 0.5));
    p.line_to(pt(0.9, 0.3));
    p.close();
    p.build()
}

fn helipad() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.2, 0.4));
    p.line_to(pt(0.2, 0.6));
    p.quadratic_bezier_to(pt(0.2, 0.8), pt(0.4, 0.8));
    p.line_to(pt(0.6, 0.8));
    p.quadratic_bezier_to(pt(0.8, 0.8), pt(0.8, 0.6));
    p.line_to(pt(0.8, 0.4));
    p.quadratic_bezier_to(pt(0.8, 0.2), pt(0.6, 0.2));
    p.line_to(pt(0.4, 0.2));
    p.quadratic_bezier_to(pt(0.2, 0.2), pt(0.2, 0.4));
    p.close();
    p.begin(pt(0.35, 0.3));
    p.line_to(pt(0.35, 0.7));
    p.line_to(pt(0.45, 0.7));
    p.line_to(pt(0.45, 0.55));
    p.line_to(pt(0.55, 0.55));
    p.line_to(pt(0.55, 0.7));
    p.line_to(pt(0.65, 0.7));
    p.line_to(pt(0.65, 0.3));
    p.line_to(pt(0.55, 0.3));
    p.line_to(pt(0.55, 0.45));
    p.line_to(pt(0.45, 0.45));
    p.line_to(pt(0.45, 0.3));
    p.close();
    p.build()
}

fn factory() -> Path {
    let f = 0.8 / 3.0;
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.1, 0.54));
    p.line_to(pt(0.1 + f, 0.7));
    p.line_to(pt(0.1 + f, 0.55));
    p.line_to(pt(0.1 + f * 2.0, 0.7));
    p.line_to(pt(0.1 + f * 2.0, 0.55));
    p.line_to(pt(0.9, 0.7));
    p.line_to(pt(0.9, 0.3));
    p.close();
    p.build()
}

#[allow(unused)]
fn metropolis() -> Path {
    let pt = offset_pt(2.0 / 3.0, Vec2::new(0.0, -0.04));
    let mut p = Path::builder();
    let base = 0.15;
    p.begin(pt(0.2 - base, 0.2 - base));
    p.line_to(pt(0.2 - base, 0.2));
    p.line_to(pt(0.2, 0.2));
    p.line_to(pt(0.2, 0.6));
    p.line_to(pt(0.4, 0.7));
    p.line_to(pt(0.4, 1.2));
    p.line_to(pt(0.8, 1.0));
    p.line_to(pt(0.8, 0.2));
    p.line_to(pt(0.8 + base, 0.2));
    p.line_to(pt(0.8 + base, 0.2 - base));
    p.close();
    p.build()
}

fn mine() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.4, 0.6));
    p.line_to(pt(0.5, 0.5));
    p.line_to(pt(0.7, 0.7));
    p.line_to(pt(0.9, 0.2));
    p.line_to(pt(0.6, 0.2));
    p.line_to(pt(0.6, 0.3));
    p.quadratic_bezier_to(pt(0.5, 0.4), pt(0.4, 0.3));
    p.line_to(pt(0.4, 0.2));
    p.close();
    p.build()
}

fn radar() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.3));
    p.line_to(pt(0.3, 0.3));
    p.line_to(pt(0.5, 0.6));
    p.quadratic_bezier_to(pt(0.4, 0.8), pt(0.4, 0.9));
    p.line_to(pt(0.9, 0.4));
    p.quadratic_bezier_to(pt(0.8, 0.4), pt(0.6, 0.5));
    p.line_to(pt(0.7, 0.3));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn village() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.1, 0.4));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.2, 0.5));
    p.line_to(pt(0.3, 0.6));
    p.line_to(pt(0.4, 0.5));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.5, 0.4));
    p.line_to(pt(0.5, 0.5));
    p.line_to(pt(0.7, 0.5));
    p.line_to(pt(0.7, 0.4));
    p.line_to(pt(0.9, 0.4));
    p.line_to(pt(0.9, 0.3));
    p.close();
    p.build()
}

fn town() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.1, 0.4));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.2, 0.6));
    p.line_to(pt(0.3, 0.7));
    p.line_to(pt(0.4, 0.6));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.6, 0.7));
    p.line_to(pt(0.8, 0.6));
    p.line_to(pt(0.8, 0.4));
    p.line_to(pt(0.9, 0.4));
    p.line_to(pt(0.9, 0.3));
    p.close();
    p.build()
}

fn ews() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.2, 0.2));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.3, 0.5));
    p.line_to(pt(0.3, 0.7));
    p.line_to(pt(0.4, 0.8));
    p.line_to(pt(0.6, 0.8));
    p.line_to(pt(0.7, 0.7));
    p.line_to(pt(0.7, 0.5));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.8, 0.4));
    p.line_to(pt(0.8, 0.2));
    p.close();
    p.build()
}

fn rampart() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.65));
    p.line_to(pt(0.2, 0.65));
    p.line_to(pt(0.2, 0.6));
    p.line_to(pt(0.3, 0.6));
    p.line_to(pt(0.3, 0.65));
    p.line_to(pt(0.4, 0.65));
    p.line_to(pt(0.4, 0.6));
    p.line_to(pt(0.5, 0.6));
    p.line_to(pt(0.5, 0.65));
    p.line_to(pt(0.6, 0.65));
    p.line_to(pt(0.6, 0.6));
    p.line_to(pt(0.7, 0.6));
    p.line_to(pt(0.7, 0.7));
    p.line_to(pt(0.8, 0.75));
    p.line_to(pt(0.9, 0.7));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn reactor() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.1));
    p.line_to(pt(0.1, 0.3));
    p.line_to(pt(0.2, 0.3));
    p.quadratic_bezier_to(pt(0.35, 0.6), pt(0.2, 0.9));
    p.line_to(pt(0.8, 0.9));
    p.quadratic_bezier_to(pt(0.65, 0.6), pt(0.8, 0.3));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.9, 0.1));
    p.close();
    p.build()
}

fn refinery() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.3));
    p.line_to(pt(0.2, 0.3));
    p.line_to(pt(0.2, 0.5));
    p.line_to(pt(0.175, 0.5));
    p.line_to(pt(0.175, 0.55));
    p.line_to(pt(0.2, 0.55));
    p.line_to(pt(0.2, 0.65));
    p.quadratic_bezier_to(pt(0.3, 0.7), pt(0.4, 0.65));
    p.line_to(pt(0.4, 0.55));
    p.line_to(pt(0.425, 0.55));
    p.line_to(pt(0.425, 0.5));
    p.line_to(pt(0.4, 0.5));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.6, 0.45));
    p.line_to(pt(0.7, 0.75));
    p.line_to(pt(0.8, 0.45));
    p.line_to(pt(0.8, 0.3));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn rocket() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.1));
    p.line_to(pt(0.1, 0.3));
    p.line_to(pt(0.2, 0.3));
    p.line_to(pt(0.2, 0.75));
    p.line_to(pt(0.25, 0.9));
    p.line_to(pt(0.3, 0.9));
    // Gantry.
    p.line_to(pt(0.3, 0.75));
    p.line_to(pt(0.35, 0.75));
    p.line_to(pt(0.35, 0.725));
    p.line_to(pt(0.45, 0.725));
    // Rocket.
    p.line_to(pt(0.45, 0.75));
    p.cubic_bezier_to(pt(0.45, 0.9), pt(0.55, 0.9), pt(0.55, 0.75));
    p.line_to(pt(0.55, 0.5));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.6, 0.35));
    p.line_to(pt(0.55, 0.375));
    p.line_to(pt(0.55, 0.35));
    p.line_to(pt(0.525, 0.35));
    p.line_to(pt(0.55, 0.3));
    // Base.
    p.line_to(pt(0.7, 0.3));
    p.line_to(pt(0.8, 0.2));
    p.line_to(pt(0.9, 0.2));
    p.line_to(pt(0.9, 0.1));
    p.close();
    // Cutout (Base).
    p.begin(pt(0.3, 0.3));
    p.line_to(pt(0.45, 0.3));
    // Rocket.
    p.line_to(pt(0.475, 0.35));
    p.line_to(pt(0.45, 0.35));
    p.line_to(pt(0.45, 0.375));
    p.line_to(pt(0.4, 0.35));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.45, 0.5));
    // Gantry.
    p.line_to(pt(0.45, 0.675));
    p.line_to(pt(0.35, 0.675));
    p.line_to(pt(0.35, 0.65));
    p.line_to(pt(0.3, 0.65));
    p.close();
    p.build()
}

fn satellite() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.3, 0.2));
    p.line_to(pt(0.35, 0.45));
    p.quadratic_bezier_to(pt(0.2, 0.5), pt(0.1, 0.6));
    p.line_to(pt(0.1, 0.65));
    p.quadratic_bezier_to(pt(0.25, 0.55), pt(0.45, 0.55));
    p.line_to(pt(0.5, 0.8));
    p.line_to(pt(0.55, 0.55));
    p.quadratic_bezier_to(pt(0.75, 0.55), pt(0.9, 0.65));
    p.line_to(pt(0.9, 0.6));
    p.quadratic_bezier_to(pt(0.8, 0.5), pt(0.65, 0.45));
    p.line_to(pt(0.7, 0.2));
    p.close();
    p.build()
}

fn generator() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.85));
    p.line_to(pt(0.2, 0.85));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.3, 0.4));
    p.line_to(pt(0.3, 0.85));
    p.line_to(pt(0.4, 0.85));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.5, 0.4));
    p.line_to(pt(0.5, 0.6));
    p.line_to(pt(0.9, 0.6));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn quarry() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.6));
    p.line_to(pt(0.2, 0.6));
    p.line_to(pt(0.3, 0.4));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.5, 0.3));
    p.line_to(pt(0.6, 0.3));
    p.line_to(pt(0.8, 0.6));
    p.line_to(pt(0.9, 0.6));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn cliff() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.2));
    p.line_to(pt(0.1, 0.6));
    p.line_to(pt(0.2, 0.6));
    p.line_to(pt(0.4, 0.55));
    p.line_to(pt(0.6, 0.65));
    p.line_to(pt(0.8, 0.6));
    p.line_to(pt(0.9, 0.6));
    p.line_to(pt(0.9, 0.2));
    p.close();
    p.build()
}

fn projector() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.25, 0.15));
    p.line_to(pt(0.25, 0.3));
    p.line_to(pt(0.35, 0.4));
    p.line_to(pt(0.35, 0.45));
    p.quadratic_bezier_to(pt(0.3, 0.5), pt(0.3, 0.6));
    p.quadratic_bezier_to(pt(0.3, 0.8), pt(0.5, 0.8));
    p.quadratic_bezier_to(pt(0.7, 0.8), pt(0.7, 0.6));
    p.quadratic_bezier_to(pt(0.7, 0.5), pt(0.65, 0.45));
    p.line_to(pt(0.65, 0.4));
    p.line_to(pt(0.75, 0.3));
    p.line_to(pt(0.75, 0.15));
    p.close();
    // Cutout
    p.begin(pt(0.45, 0.35));
    p.line_to(pt(0.55, 0.35));
    p.line_to(pt(0.55, 0.5));
    p.line_to(pt(0.45, 0.5));
    p.close();
    p.build()
}

fn silo_inner(p: &mut Builder, pt: impl Fn(f32, f32) -> Point) {
    p.begin(pt(0.2, 0.4));
    p.line_to(pt(0.25, 0.6));
    p.line_to(pt(0.3, 0.6));
    p.quadratic_bezier_to(pt(0.5, 0.45), pt(0.7, 0.6));
    p.line_to(pt(0.75, 0.6));
    p.line_to(pt(0.8, 0.4));
    p.close();
}

fn silo() -> Path {
    let mut p = Path::builder();
    silo_inner(&mut p, pt);
    p.build()
}

fn soldier() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.4, 0.1));
    p.line_to(pt(0.4, 0.5));
    p.quadratic_bezier_to(pt(0.5, 0.55), pt(0.6, 0.45));
    p.line_to(pt(0.6, 0.1));
    p.close();
    p.add_circle(pt(0.5, 0.7), 0.1, Winding::Positive);
    p.build()
}

fn tank() -> Path {
    let mut p = Path::builder();

    // Chassis
    p.begin(pt(0.1, 0.4));
    p.line_to(pt(0.2, 0.55));
    p.line_to(pt(0.3, 0.55));
    p.line_to(pt(0.35, 0.7));
    p.line_to(pt(0.55, 0.7));
    p.line_to(pt(0.6, 0.55));
    p.line_to(pt(0.8, 0.55));
    p.line_to(pt(0.9, 0.4));
    p.line_to(pt(0.8, 0.3));
    p.line_to(pt(0.2, 0.3));
    p.close();

    // Turret
    p.begin(pt(0.575, 0.625));
    p.line_to(pt(0.775, 0.675));
    p.close();

    p.build()
}

fn fighter() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.35, 0.1));
    p.line_to(pt(0.4, 0.25));
    p.line_to(pt(0.2, 0.3));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.4, 0.5));
    p.line_to(pt(0.4, 0.6));
    p.line_to(pt(0.45, 0.65));
    p.line_to(pt(0.5, 0.9));
    p.line_to(pt(0.55, 0.65));
    p.line_to(pt(0.6, 0.6));
    p.line_to(pt(0.6, 0.5));
    p.line_to(pt(0.8, 0.4));
    p.line_to(pt(0.8, 0.3));
    p.line_to(pt(0.6, 0.25));
    p.line_to(pt(0.65, 0.1));
    p.line_to(pt(0.5, 0.2));
    p.close();
    p.build()
}

fn bomber() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.5, 0.8));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.8, 0.2));
    p.line_to(pt(0.7, 0.3));
    p.line_to(pt(0.5, 0.2));
    p.line_to(pt(0.3, 0.3));
    p.line_to(pt(0.2, 0.2));
    p.close();
    p.build()
}

fn chopper() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.5, 0.9));
    p.quadratic_bezier_to(pt(0.65, 0.9), pt(0.65, 0.75));
    p.line_to(pt(0.65, 0.5));
    p.quadratic_bezier_to(pt(0.65, 0.45), pt(0.55, 0.35));
    p.line_to(pt(0.55, 0.15));
    p.line_to(pt(0.65, 0.125));
    p.line_to(pt(0.65, 0.1));
    p.line_to(pt(0.35, 0.1));
    p.line_to(pt(0.35, 0.125));
    p.line_to(pt(0.45, 0.15));
    p.line_to(pt(0.45, 0.35));
    p.quadratic_bezier_to(pt(0.35, 0.45), pt(0.35, 0.5));
    p.line_to(pt(0.35, 0.75));
    p.quadratic_bezier_to(pt(0.35, 0.9), pt(0.5, 0.9));
    p.close();
    p.build()
}

fn blades(blurred: bool) -> Path {
    let center = pt(0.5, 0.6);

    let mut p = Path::builder();
    if blurred {
        p.add_circle(center, 0.4, Winding::Positive)
    } else {
        fn only_x(v: Vector) -> Vector {
            Vector::new(v.x, 0.0)
        }
        fn flip_x(v: Vector) -> Vector {
            Vector::new(-v.x, v.y)
        }
        fn only_y(v: Vector) -> Vector {
            Vector::new(0.0, v.y)
        }
        fn flip_y(v: Vector) -> Vector {
            Vector::new(v.x, -v.y)
        }

        let o = Vector::new(0.25, 0.25);
        let t = Vector::new(0.05, 0.05) * 0.5;

        p.begin(center + only_y(t));
        p.line_to(center + o + only_y(t));
        p.line_to(center + o + only_x(t));
        p.line_to(center + only_x(t));

        p.line_to(center + flip_y(o) + only_x(t));
        p.line_to(center + flip_y(o) - only_y(t));
        p.line_to(center - only_y(t));

        p.line_to(center - o - only_y(t));
        p.line_to(center - o - only_x(t));
        p.line_to(center - only_x(t));

        p.line_to(center + flip_x(o) - only_x(t));
        p.line_to(center + flip_x(o) + only_y(t));
        p.line_to(center + only_y(t));
        p.close();
    }
    p.build()
}

fn emp() -> Path {
    let pt = offset_pt(1.3, Vec2::new(0.0, 0.05));

    let mut p = Path::builder();
    p.begin(pt(0.4, 0.2));
    p.line_to(pt(0.4, 0.25));
    p.line_to(pt(0.3, 0.25));
    p.line_to(pt(0.4, 0.5));
    p.line_to(pt(0.4, 0.6));
    p.cubic_bezier_to(pt(0.4, 0.8), pt(0.6, 0.8), pt(0.6, 0.6));
    p.line_to(pt(0.6, 0.5));
    p.line_to(pt(0.7, 0.25));
    p.line_to(pt(0.6, 0.25));
    p.line_to(pt(0.6, 0.2));
    p.line_to(pt(0.525, 0.2));
    p.line_to(pt(0.55, 0.15));
    p.line_to(pt(0.45, 0.15));
    p.line_to(pt(0.475, 0.2));
    p.close();
    p.build()
}

fn ruler() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.25, 0.2));
    p.line_to(pt(0.1, 0.8));
    p.line_to(pt(0.35, 0.6));
    p.line_to(pt(0.5, 0.85));
    p.line_to(pt(0.65, 0.6));
    p.line_to(pt(0.9, 0.8));
    p.line_to(pt(0.75, 0.2));
    p.close();

    let mut circle = |center: Point| {
        p.add_circle(center, STROKE_WIDTH, Winding::Positive);
    };
    const O: f32 = 0.025;
    circle(pt(0.1 + O, 0.8 - O));
    circle(pt(0.5, 0.85));
    circle(pt(0.9 - O, 0.8 - O));

    p.build()
}

fn nuke() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.3, 0.1));
    p.line_to(pt(0.3, 0.3));
    p.line_to(pt(0.4, 0.4));
    p.line_to(pt(0.3, 0.5));
    p.line_to(pt(0.3, 0.7));
    p.line_to(pt(0.5, 0.9));
    p.line_to(pt(0.7, 0.7));
    p.line_to(pt(0.7, 0.5));
    p.line_to(pt(0.6, 0.4));
    p.line_to(pt(0.7, 0.3));
    p.line_to(pt(0.7, 0.1));
    p.line_to(pt(0.6, 0.2));
    p.line_to(pt(0.4, 0.2));
    p.close();
    p.build()
}

fn shell() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.35, 0.45));
    p.line_to(pt(0.5, 0.85));
    p.line_to(pt(0.65, 0.45));
    p.line_to(pt(0.65, 0.2));
    p.quadratic_bezier_to(pt(0.5, 0.1), pt(0.35, 0.2));
    p.close();
    p.build()
}

fn break_alliance() -> Path {
    let pt = offset_pt(1.2, Vec2::new(0.0, 0.1));
    let mut p = Path::builder();

    for scale in [-1.0, 1.0] {
        let pt = |x: f32, y: f32| pt((x - 0.5) * scale + 0.5, y);

        // Handle
        p.begin(pt(0.1, 0.1));
        p.line_to(pt(0.1, 0.15));
        p.line_to(pt(0.2, 0.25));
        p.line_to(pt(0.25, 0.2));
        p.line_to(pt(0.15, 0.1));
        p.close();

        // Cross-guard
        p.begin(pt(0.1, 0.35));
        p.line_to(pt(0.1, 0.4));
        p.line_to(pt(0.15, 0.4));
        p.line_to(pt(0.4, 0.15));
        p.line_to(pt(0.4, 0.1));
        p.line_to(pt(0.35, 0.1));
        p.close();
    }

    // Blade
    p.begin(pt(0.2, 0.35));
    p.line_to(pt(0.6, 0.75));
    p.line_to(pt(0.75, 0.75));
    p.line_to(pt(0.75, 0.6));
    p.line_to(pt(0.35, 0.2));
    p.close();

    // Blade Inner
    p.begin(pt(0.35, 0.35));
    p.line_to(pt(0.65, 0.65));
    p.close();

    // Blade 2
    p.begin(pt(0.5, 0.35));
    p.line_to(pt(0.65, 0.5));
    p.line_to(pt(0.8, 0.35));
    p.line_to(pt(0.65, 0.2));
    p.close();
    p.begin(pt(0.35, 0.5));
    p.line_to(pt(0.25, 0.6));
    p.line_to(pt(0.25, 0.75));
    p.line_to(pt(0.4, 0.75));
    p.line_to(pt(0.5, 0.65));
    p.close();

    // Blade 2 Inner
    p.begin(pt(0.65, 0.35));
    p.line_to(pt(0.575, 0.425));
    p.close();
    p.begin(pt(0.35, 0.65));
    p.line_to(pt(0.425, 0.575));
    p.close();

    p.build()
}

fn cursor() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.7, 0.05));
    p.line_to(pt(0.6, 0.2));
    p.line_to(pt(0.5, 0.15));
    p.line_to(pt(0.5, 0.5));
    p.line_to(pt(0.8, 0.3));
    p.line_to(pt(0.7, 0.25));
    p.line_to(pt(0.8, 0.1));
    p.close();
    p.build()
}

fn key() -> Path {
    let mut p = Path::builder();
    p.begin(pt(0.1, 0.1));
    p.line_to(pt(0.1, 0.25));
    p.line_to(pt(0.45, 0.6));
    p.quadratic_bezier_to(pt(0.4, 0.95), pt(0.75, 0.9));
    p.quadratic_bezier_to(pt(0.86, 0.86), pt(0.9, 0.75));
    p.quadratic_bezier_to(pt(0.95, 0.4), pt(0.6, 0.45));
    p.line_to(pt(0.45, 0.3));
    p.line_to(pt(0.35, 0.3));
    p.line_to(pt(0.35, 0.2));
    p.line_to(pt(0.25, 0.2));
    p.line_to(pt(0.25, 0.1));
    p.close();
    // 0.5 as that is the offset.
    p.add_circle(pt(0.7, 0.7), 0.075, Winding::Negative);
    p.build()
}

fn request_alliance() -> Path {
    let pt = offset_pt(1.3, Vec2::new(0.0, 0.085));
    let mut p = Path::builder();

    // Outline
    p.begin(pt(0.1, 0.3));
    p.line_to(pt(0.2, 0.4));
    p.line_to(pt(0.25, 0.55));
    p.line_to(pt(0.3, 0.6));
    p.line_to(pt(0.3, 0.65));
    p.line_to(pt(0.35, 0.65));
    p.line_to(pt(0.4, 0.6));
    p.line_to(pt(0.35, 0.65));
    p.line_to(pt(0.35, 0.7));
    p.line_to(pt(0.4, 0.7));
    p.line_to(pt(0.45, 0.65));
    p.line_to(pt(0.4, 0.7));
    p.line_to(pt(0.4, 0.75));
    p.line_to(pt(0.45, 0.75));
    p.line_to(pt(0.5, 0.7));
    p.line_to(pt(0.35, 0.55));
    p.line_to(pt(0.3, 0.6));
    p.line_to(pt(0.35, 0.55));
    p.line_to(pt(0.5, 0.7));
    p.line_to(pt(0.55, 0.75));
    p.line_to(pt(0.6, 0.75));
    p.line_to(pt(0.6, 0.7));
    p.line_to(pt(0.65, 0.7));
    p.line_to(pt(0.65, 0.65));
    p.line_to(pt(0.7, 0.65));
    p.line_to(pt(0.7, 0.6));
    p.line_to(pt(0.75, 0.55));
    p.line_to(pt(0.8, 0.4));
    p.line_to(pt(0.9, 0.3));
    p.line_to(pt(0.7, 0.1));
    p.line_to(pt(0.5, 0.3));
    p.line_to(pt(0.3, 0.1));
    p.close();

    // Detail
    p.begin(pt(0.5, 0.3));
    let points = [
        pt(0.7, 0.5),
        pt(0.65, 0.55),
        pt(0.55, 0.45),
        pt(0.65, 0.55),
        pt(0.6, 0.6),
        pt(0.5, 0.5),
        pt(0.6, 0.6),
        pt(0.55, 0.65),
        pt(0.45, 0.55),
        pt(0.55, 0.65),
        pt(0.55, 0.7),
        pt(0.6, 0.7),
    ];
    for point in points {
        p.line_to(point);
    }
    p.line_to(pt(0.7, 0.6));
    for point in points.into_iter().rev() {
        p.line_to(point);
    }
    p.close();

    // Cuffs
    p.begin(pt(0.2, 0.4));
    p.line_to(pt(0.4, 0.2));
    p.close();
    p.begin(pt(0.8, 0.4));
    p.line_to(pt(0.6, 0.2));
    p.close();

    p.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::tower::TowerType;

    #[test]
    fn test_svg_data_url() {
        for t in TowerType::iter() {
            // Make sure this doesn't panic.
            SvgCache::get(PathId::Tower(t), Color::Blue);
        }
    }
}
