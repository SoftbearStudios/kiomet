// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::gl::Gl;
use crate::{DefaultRender, Renderer};
use web_sys::WebGlQuery;

/// A query that can test if any pixels of an object are rendered. Only returns results after a few
/// frames.
pub struct OcclusionQuery {
    in_progress: bool,
    query: WebGlQuery,
    visible: Option<bool>,
}

const QUERY_TYPE: u32 = Gl::ANY_SAMPLES_PASSED_CONSERVATIVE;

impl DefaultRender for OcclusionQuery {
    fn new(renderer: &Renderer) -> Self {
        Self {
            in_progress: false,
            query: renderer.gl.create_query().unwrap(),
            visible: None,
        }
    }
}

impl OcclusionQuery {
    /// Binds the [`OcclusionQuery`] to record draws.
    pub fn bind<'a>(&'a mut self, renderer: &'a Renderer) -> Option<OcclusionQueryBinding<'a>> {
        let gl = &renderer.gl;

        if self.in_progress
            && gl
                .get_query_parameter(&self.query, Gl::QUERY_RESULT_AVAILABLE)
                .is_truthy()
        {
            self.in_progress = false;
            self.visible = Some(
                gl.get_query_parameter(&self.query, Gl::QUERY_RESULT)
                    .is_truthy(),
            );
        }

        (!self.in_progress).then(|| {
            self.in_progress = true;
            gl.begin_query(QUERY_TYPE, &self.query);
            OcclusionQueryBinding { renderer }
        })
    }

    /// Returns true if the objects drawn during the [`OcclusionQuery`] were visible. Returns
    /// [`None`] if the query hasn't finished.
    pub fn visible(&self) -> Option<bool> {
        self.visible
    }
}

/// A bound [`OcclusionQuery`] that records draws.
pub struct OcclusionQueryBinding<'a> {
    renderer: &'a Renderer,
}

impl<'a> Drop for OcclusionQueryBinding<'a> {
    fn drop(&mut self) {
        self.renderer.gl.end_query(QUERY_TYPE);
    }
}
