use glam::Vec2;

pub struct PanZoom {
    center: Vec2,
    zoom: f32,
    aspect_ratio: f32,
    bottom_left: Vec2,
    top_right: Vec2,
    debug: bool,
    ready: bool,
}

impl Default for PanZoom {
    fn default() -> Self {
        Self::new()
    }
}

impl PanZoom {
    pub fn new() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            aspect_ratio: 1.0,
            bottom_left: Vec2::splat(-1.0),
            top_right: Vec2::splat(1.0),
            debug: false,
            ready: false,
        }
    }

    /// Gets center in world space.
    pub fn get_center(&self) -> Vec2 {
        self.center
    }

    /// Gets zoom in world space.
    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    /// Gets zoom on both x and y axis.
    pub fn get_zooms(&self) -> Vec2 {
        Vec2::new(self.zoom, self.zoom / self.aspect_ratio)
    }

    /// Takes transformation origin in world space.
    pub fn multiply_zoom(&mut self, origin: Vec2, factor: f32) {
        debug_assert!(factor.is_finite());

        // Invariant.
        let relative_origin = (origin - self.center) / self.zoom;

        self.zoom = (self.zoom * factor).clamp(self.min_zoom(), self.max_zoom());

        self.center = origin - relative_origin * self.zoom;

        #[cfg(debug_assertions)]
        {
            let new_relative_origin = (origin - self.center) / self.zoom;
            debug_assert!(relative_origin.distance(new_relative_origin) < 1.0);
        }

        self.clamp_center();
    }

    /// Takes mouse movement in world space.
    pub fn pan(&mut self, delta: Vec2) {
        self.center -= delta;
        self.clamp_center();
    }

    pub fn pan_to(&mut self, target: Vec2) {
        self.center = target;
        self.clamp_center();
    }

    fn clamp_center(&mut self) {
        let min = self.bottom_left;
        let max = self.top_right;
        for i in 0..2 {
            if min[i] > max[i] {
                self.center[i] = (self.top_right[i] + self.bottom_left[i]) * 0.5;
            } else {
                self.center[i] = self.center[i].clamp(min[i], max[i]);
            }
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
    }

    /// Takes bounds in world space.
    pub fn set_bounds(&mut self, bottom_left: Vec2, top_right: Vec2, debug: bool) {
        debug_assert!(bottom_left.is_finite());
        debug_assert!(top_right.is_finite());
        self.bottom_left = bottom_left;
        self.top_right = top_right;
        self.debug = debug;
        debug_assert!(self.top_right.cmpge(self.bottom_left).all());
        if !self.ready
            || !(0..2)
                .map(|i| (self.bottom_left[i]..=self.top_right[i]).contains(&self.center[i]))
                .all(|b| b)
            || self.zoom < self.min_zoom()
        {
            self.reset_center();
            self.reset_zoom();
            self.ready = true;
        }
    }

    /// Sets center to that of bounds.
    pub fn reset_center(&mut self) {
        self.center = (self.bottom_left + self.top_right) * 0.5;
    }

    /// Sets zoom to halfway.
    pub fn reset_zoom(&mut self) {
        self.zoom = (self.min_zoom() + self.max_zoom()) * 0.5;
    }

    fn min_zoom(&self) -> f32 {
        if self.debug {
            0.5
        } else {
            10.0
        }
    }

    fn max_zoom(&self) -> f32 {
        let span = self.top_right - self.bottom_left;
        span.max_element() * 0.75
    }
}
