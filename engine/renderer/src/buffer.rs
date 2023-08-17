// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::attribs::Attribs;
use crate::gl::*;
use crate::index::Index;
use crate::renderer::Renderer;
use crate::vertex::Vertex;
use crate::DefaultRender;
use bytemuck::Pod;
use std::convert::TryInto;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Range;
use web_sys::{WebGlBuffer, WebGlVertexArrayObject};

/// Vertex indices of a traingle, in counter-clockwise order.
pub type Triangle<I> = [I; 3];

/// Vertex indices of a quad, in counter-clockwise order.
pub type Quad<I> = [I; 4];

/// Allows building a triangle mesh presumably to draw with [`TriangleBuffer`].
#[derive(Debug)]
pub struct MeshBuilder<V, I = u16> {
    /// Vertices of a mesh that are indexed by indices.
    pub vertices: Vec<V>,
    /// Indices into `vertices` that form counter-clockwise triangles.
    pub indices: Vec<I>,
    default_indices: bool,
}

impl<V: Vertex, I: Index> Default for MeshBuilder<V, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Vertex, I: Index> MeshBuilder<V, I> {
    /// Create an empty [`MeshBuilder`].
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            default_indices: false,
        }
    }

    /// Pushes a single [`Triangle`] to `indices`.
    pub fn push_triangle(&mut self, triangle: Triangle<I>) {
        self.indices.extend_from_slice(&triangle);
    }

    /// Pushes a single [`Quad`] to `indices`.
    pub fn push_quad(&mut self, quad: Quad<I>) {
        self.indices
            .extend_from_slice(&[quad[0], quad[1], quad[2], quad[2], quad[3], quad[0]]);
    }

    /// Pushes triangles from `start` until `self.vertices.len()`.
    pub fn push_triangles(&mut self, start: usize) {
        let n = self.vertices.len() - start;
        assert_eq!(n % 3, 0);
        self.indices
            .extend((start..self.vertices.len()).map(|i| I::from_usize(i)))
    }

    /// Pushes quads from `start` until `self.vertices.len()`.
    pub fn push_quads(&mut self, start: usize) {
        let n = self.vertices.len() - start;
        assert_eq!(n % 4, 0);
        let quads = n / 4;

        for quad in 0..quads {
            let i = start + quad * 4;
            self.push_quad([
                I::from_usize(i),
                I::from_usize(i + 1),
                I::from_usize(i + 2),
                I::from_usize(i + 3),
            ]);
        }
    }

    /// Pushes a [`Triangle`] to `indices` for every 3 `vertices`. Next mutation must be
    /// [`MeshBuilder::clear`].
    pub fn push_default_triangles(&mut self) {
        assert_eq!(self.vertices.len() % 3, 0);
        assert!(self.indices.is_empty());
        self.default_indices = true;
    }

    /// Pushes a [`Quad`] to `indices` for every 4 `vertices`.
    pub fn push_default_quads(&mut self) {
        assert!(self.indices.is_empty());
        self.push_quads(0);
    }

    /// Clears `vertices` and `indices`, but preserves their allocations for reuse.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.default_indices = false;
    }

    /// Returns true if `vertices.is_empty()`.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

pub(crate) enum GpuBufferType {
    Array,
    Element,
}

impl GpuBufferType {
    pub(crate) const fn to(self) -> bool {
        match self {
            Self::Array => true,
            Self::Element => false,
        }
    }

    const fn fr(v: bool) -> Self {
        match v {
            true => Self::Array,
            false => Self::Element,
        }
    }

    const fn target(self) -> u32 {
        match self {
            Self::Array => Gl::ARRAY_BUFFER,
            Self::Element => Gl::ELEMENT_ARRAY_BUFFER,
        }
    }

    const fn parameter(self) -> u32 {
        match self {
            Self::Array => Gl::ARRAY_BUFFER_BINDING,
            Self::Element => Gl::ELEMENT_ARRAY_BUFFER_BINDING,
        }
    }
}

/// TODO find a way to keep this private and implement Deque.
#[doc(hidden)]
pub struct GpuBuffer<E, const B: bool> {
    elements: WebGlBuffer,
    length: usize,   // The amount of valid elements in the buffer.
    capacity: usize, // The amount of capacity (in elements) that is available in the buffer.
    element: PhantomData<E>,
}

impl<E: Pod, const B: bool> GpuBuffer<E, B> {
    pub fn new(gl: &Gl) -> Self {
        Self {
            elements: gl.create_buffer().unwrap(),
            length: 0,
            capacity: 0,
            element: PhantomData,
        }
    }

    #[must_use]
    pub(crate) fn bind<'a>(&'a self, gl: &'a Gl) -> GpuBufferBinding<'a, E, B> {
        GpuBufferBinding::new(gl, self)
    }

    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Only used once for an optimization in instance.rs.
    pub(crate) fn _elements(&self) -> &WebGlBuffer {
        &self.elements
    }

    pub(crate) fn buffer(&mut self, gl: &Gl, elements: &[E]) {
        self.buffer_inner(gl, elements.len(), elements, 0);
    }

    /// Overwrites part of the [`GpuBuffer`] leaving the rest untouched. Cannot expand the
    /// [`GpuBuffer`].
    pub(crate) fn buffer_sub_data(&mut self, gl: &Gl, elements: &[E], start: usize) {
        self.buffer_inner(gl, self.length, elements, start);
    }

    /// Expands the [`GpuBuffer`] with zeroed memory possibly clearing it.
    pub(crate) fn resize_zeroed(&mut self, gl: &Gl, capacity: usize) {
        self.buffer_inner(gl, capacity, &[], 0);
    }

    fn buffer_inner(&mut self, gl: &Gl, length: usize, elements: &[E], start: usize) {
        assert!(elements.len() + start <= length, "out of bounds");
        self.length = length;

        // This can easily mess up the bind_buffer calls.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        // Make sure element's binding was cleared.
        debug_assert!(gl
            .get_parameter(GpuBufferType::fr(B).parameter())
            .unwrap()
            .is_null());

        let copy_data = !elements.is_empty();
        let grow_buffer = length != 0;

        fn elements_to_bytes<E>(i: usize) -> i32 {
            (i * size_of::<E>()).try_into().unwrap()
        }

        // Don't bind if not copying data or growing buffer.
        if copy_data || grow_buffer {
            // Buffer elements.
            let target = GpuBufferType::fr(B).target();
            gl.bind_buffer(target, Some(&self.elements));

            // Allocate buffer to nearest power of 2 (never shrinks).
            let new_cap = length.next_power_of_two();
            if new_cap > self.capacity {
                gl.buffer_data_with_i32(target, elements_to_bytes::<E>(new_cap), Gl::DYNAMIC_DRAW);
                self.capacity = new_cap;
            }

            if copy_data {
                let b = |a| {
                    gl.buffer_sub_data_with_i32_and_array_buffer_view(
                        target,
                        elements_to_bytes::<E>(start),
                        a,
                    )
                };

                unsafe {
                    match GpuBufferType::fr(B) {
                        GpuBufferType::Array => {
                            b(&js_sys::Float32Array::view(bytemuck::cast_slice(elements)))
                        }
                        GpuBufferType::Element => match size_of::<E>() {
                            1 => b(&js_sys::Uint8Array::view(bytemuck::cast_slice(elements))),
                            2 => b(&js_sys::Uint16Array::view(bytemuck::cast_slice(elements))),
                            4 => b(&js_sys::Uint32Array::view(bytemuck::cast_slice(elements))),
                            _ => panic!("invalid index size"),
                        },
                    }
                }
            }

            // Unbind (not required in release mode).
            #[cfg(debug_assertions)]
            gl.bind_buffer(target, None);
        }
    }
}

pub(crate) struct GpuBufferBinding<'a, E: Pod, const B: bool> {
    gl: &'a Gl,
    element: PhantomData<E>,
}

impl<'a, E: Pod, const B: bool> GpuBufferBinding<'a, E, B> {
    fn new(gl: &'a Gl, buffer: &GpuBuffer<E, B>) -> Self {
        // Make sure buffer element's binding was cleared.
        debug_assert!(gl
            .get_parameter(GpuBufferType::fr(B).parameter())
            .unwrap()
            .is_null());

        // Bind buffer's elements.
        gl.bind_buffer(GpuBufferType::fr(B).target(), Some(&buffer.elements));

        Self {
            gl,
            element: PhantomData,
        }
    }
}

impl<'a, V: Vertex> GpuBufferBinding<'a, V, { GpuBufferType::Array.to() }> {
    pub(crate) fn bind_attribs(&self) -> Attribs<'a> {
        let mut attribs = Attribs::new::<V>(self.gl);
        V::bind_attribs(&mut attribs);
        attribs
    }

    pub(crate) fn bind_attribs_instanced(&self, aia: &Aia, previous: Attribs<'a>) {
        V::bind_attribs(&mut Attribs::new_instanced::<V>(self.gl, aia, previous));
    }
}

impl<'a, E: Pod, const B: bool> Drop for GpuBufferBinding<'a, E, B> {
    fn drop(&mut self) {
        // Unbind (not required in release mode).
        #[cfg(debug_assertions)]
        self.gl.bind_buffer(GpuBufferType::fr(B).target(), None);
    }
}

/// [`TriangleBuffer`] facilitates drawing a triangle mesh.
pub struct TriangleBuffer<V, I = u16> {
    pub(crate) vertices: GpuBuffer<V, { GpuBufferType::Array.to() }>,
    pub(crate) indices: GpuBuffer<I, { GpuBufferType::Element.to() }>,
    vao: WebGlVertexArrayObject,
}

impl<V: Vertex, I: Index> DefaultRender for TriangleBuffer<V, I> {
    fn new(renderer: &Renderer) -> Self {
        let gl = &renderer.gl;
        let ovao = &renderer.ovao;

        let buffer = Self {
            vertices: GpuBuffer::new(gl),
            indices: GpuBuffer::new(gl),
            vao: ovao.create_vertex_array_oes().unwrap(),
        };

        // Make sure VAO was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));

        // Bind array buffer.
        let array_binding = buffer.vertices.bind(gl);
        array_binding.bind_attribs();

        // Bind element buffer.
        let element_binding = buffer.indices.bind(gl);

        // Unbinding VAO is ALWAYS required (unlike all other render unbinds).
        ovao.bind_vertex_array_oes(None);

        // Unbind both buffers.
        drop(array_binding);
        drop(element_binding);

        buffer
    }
}

impl<V: Vertex, I: Index> TriangleBuffer<V, I> {
    /// Returns true if the [`TriangleBuffer`] has no triangles to draw.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() // Indices can't be empty if vertices isn't empty.
    }

    /// Binds the [`TriangleBuffer`] to draw triangles.
    #[must_use]
    pub fn bind<'a>(&'a self, renderer: &'a Renderer) -> TriangleBufferBinding<'a, V, I> {
        TriangleBufferBinding::new(&renderer.gl, &renderer.ovao, self)
    }

    /// Copies a whole [`MeshBuilder`] into the buffer. The [`MeshBuilder`] must have indices.
    pub fn buffer_mesh(&mut self, renderer: &Renderer, mesh: &MeshBuilder<V, I>) {
        if !mesh.is_empty() {
            assert!(
                mesh.default_indices ^ !mesh.indices.is_empty(),
                "mesh has invalid indices"
            );
        }
        self.buffer(renderer, mesh.vertices.as_slice(), mesh.indices.as_slice());
    }

    /// Copies vertices and indices into the [`TriangleBuffer`].
    /// If indices is empty it performs array based rendering.
    pub fn buffer(&mut self, renderer: &Renderer, vertices: &[V], indices: &[I]) {
        assert!(!vertices.is_empty(), "buffering no vertices");
        let gl = &renderer.gl;
        self.vertices.buffer(gl, vertices);
        self.indices.buffer(gl, indices);
    }
}

/// A bound [`TriangleBuffer`] that can draw triangles.
pub struct TriangleBufferBinding<'a, V: Vertex, I: Index> {
    gl: &'a Gl,
    ovao: &'a Ovao,
    buffer: &'a TriangleBuffer<V, I>,
}

impl<'a, V: Vertex, I: Index> TriangleBufferBinding<'a, V, I> {
    fn new(gl: &'a Gl, ovao: &'a Ovao, buffer: &'a TriangleBuffer<V, I>) -> Self {
        // Make sure buffer was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));
        Self { gl, ovao, buffer }
    }

    /// Draws triangles.
    pub fn draw(&self) {
        let primitive = Gl::TRIANGLES;
        if !self.buffer.indices.is_empty() {
            self.gl.draw_elements_with_i32(
                primitive,
                self.buffer.indices.len() as i32,
                I::GL_ENUM,
                0,
            );
        } else if !self.buffer.vertices.is_empty() {
            self.gl
                .draw_arrays(primitive, 0, self.buffer.vertices.len() as i32)
        }
    }
}

impl<'a, V: Vertex, I: Index> Drop for TriangleBufferBinding<'a, V, I> {
    fn drop(&mut self) {
        // Unbind ALWAYS required (unlike all other render unbinds).
        self.ovao.bind_vertex_array_oes(None);
    }
}

/// [`PointBuffer`] facilitates drawing points. If points last a set number of frames consider using
/// [`PointDeque`][`crate::PointDeque`] instead.
pub struct PointBuffer<V> {
    pub(crate) points: GpuBuffer<V, { GpuBufferType::Array.to() }>,
    vao: WebGlVertexArrayObject,
}

impl<P: Vertex> DefaultRender for PointBuffer<P> {
    fn new(renderer: &Renderer) -> Self {
        let gl = &renderer.gl;
        let ovao = &renderer.ovao;

        let buffer = Self {
            points: GpuBuffer::new(gl),
            vao: ovao.create_vertex_array_oes().unwrap(),
        };

        // Make sure VAO was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));

        // Bind array buffer.
        let array_binding = buffer.points.bind(gl);
        array_binding.bind_attribs();

        // Unbinding VAO is ALWAYS required (unlike all other render unbinds).
        ovao.bind_vertex_array_oes(None);

        // Unbind both buffers.
        drop(array_binding);

        buffer
    }
}

impl<P: Vertex> PointBuffer<P> {
    /// Returns true if the [`PointBuffer`] has no points to draw.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Binds the [`PointBuffer`] to draw points.
    #[must_use]
    pub fn bind<'a>(&'a self, renderer: &'a Renderer) -> PointBufferBinding<'a, P> {
        PointBufferBinding::new(&renderer.gl, &renderer.ovao, self)
    }

    /// Copies points and indices into the [`PointBuffer`].
    pub fn buffer(&mut self, renderer: &Renderer, points: &[P]) {
        self.points.buffer(&renderer.gl, points);
    }
}

/// A bound [`PointBuffer`] that can draw points.
pub struct PointBufferBinding<'a, P: Vertex> {
    gl: &'a Gl,
    ovao: &'a Ovao,
    buffer: &'a PointBuffer<P>,
}

impl<'a, P: Vertex> PointBufferBinding<'a, P> {
    fn new(gl: &'a Gl, ovao: &'a Ovao, buffer: &'a PointBuffer<P>) -> Self {
        // Make sure buffer was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));
        Self { gl, ovao, buffer }
    }

    /// Draws points.
    pub fn draw(&self) {
        self.draw_range(0..self.buffer.points.len());
    }

    /// Draws a specified `range` of points.
    pub fn draw_range(&self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        if range.end > self.buffer.points.len() {
            panic!("out of bounds")
        }
        let count = range.end - range.start;
        self.gl.draw_arrays(
            Gl::POINTS,
            range.start.try_into().unwrap(),
            count.try_into().unwrap(),
        )
    }
}

impl<'a, P: Vertex> Drop for PointBufferBinding<'a, P> {
    fn drop(&mut self) {
        // Unbind ALWAYS required (unlike all other render unbinds).
        self.ovao.bind_vertex_array_oes(None);
    }
}

// TODO deduplicate code with [`TriangleBuffer`].
/// [`LineBuffer`] facilitates drawing a line mesh.
pub struct LineBuffer<V, I = u16> {
    vertices: GpuBuffer<V, { GpuBufferType::Array.to() }>,
    indices: GpuBuffer<I, { GpuBufferType::Element.to() }>,
    vao: WebGlVertexArrayObject,
}

impl<V: Vertex, I: Index> DefaultRender for LineBuffer<V, I> {
    fn new(renderer: &Renderer) -> Self {
        let gl = &renderer.gl;
        let ovao = &renderer.ovao;

        let buffer = Self {
            vertices: GpuBuffer::new(gl),
            indices: GpuBuffer::new(gl),
            vao: ovao.create_vertex_array_oes().unwrap(),
        };

        // Make sure VAO was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));

        // Bind array buffer.
        let array_binding = buffer.vertices.bind(gl);
        array_binding.bind_attribs();

        // Bind element buffer.
        let element_binding = buffer.indices.bind(gl);

        // Unbinding VAO is ALWAYS required (unlike all other render unbinds).
        ovao.bind_vertex_array_oes(None);

        // Unbind both buffers.
        drop(array_binding);
        drop(element_binding);

        buffer
    }
}

impl<V: Vertex, I: Index> LineBuffer<V, I> {
    /// Returns true if the [`LineBuffer`] has no lines to draw.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() // Indices can't be empty if vertices isn't empty.
    }

    /// Binds the [`LineBuffer`] to draw lines.
    #[must_use]
    pub fn bind<'a>(&'a self, renderer: &'a Renderer) -> LineBufferBinding<'a, V, I> {
        LineBufferBinding::new(&renderer.gl, &renderer.ovao, self)
    }

    /// Copies vertices and indices into the [`LineBuffer`].
    /// If indices is empty it performs array based rendering.
    pub fn buffer(&mut self, renderer: &Renderer, vertices: &[V], indices: &[I]) {
        assert!(!vertices.is_empty(), "buffering no vertices");
        let gl = &renderer.gl;
        self.vertices.buffer(gl, vertices);
        self.indices.buffer(gl, indices);
    }
}

/// A bound [`LineBuffer`] that can draw lines.
pub struct LineBufferBinding<'a, V: Vertex, I: Index> {
    gl: &'a Gl,
    ovao: &'a Ovao,
    buffer: &'a LineBuffer<V, I>,
}

impl<'a, V: Vertex, I: Index> LineBufferBinding<'a, V, I> {
    fn new(gl: &'a Gl, ovao: &'a Ovao, buffer: &'a LineBuffer<V, I>) -> Self {
        // Make sure buffer was unbound.
        debug_assert!(gl
            .get_parameter(Ovao::VERTEX_ARRAY_BINDING_OES)
            .unwrap()
            .is_null());

        ovao.bind_vertex_array_oes(Some(&buffer.vao));
        Self { gl, ovao, buffer }
    }

    /// Draws lines.
    pub fn draw(&self) {
        let primitive = Gl::LINES;
        if !self.buffer.indices.is_empty() {
            self.gl.draw_elements_with_i32(
                primitive,
                self.buffer.indices.len() as i32,
                I::GL_ENUM,
                0,
            );
        } else if !self.buffer.vertices.is_empty() {
            self.gl
                .draw_arrays(primitive, 0, self.buffer.vertices.len() as i32)
        }
    }
}

impl<'a, V: Vertex, I: Index> Drop for LineBufferBinding<'a, V, I> {
    fn drop(&mut self) {
        // Unbind ALWAYS required (unlike all other render unbinds).
        self.ovao.bind_vertex_array_oes(None);
    }
}
