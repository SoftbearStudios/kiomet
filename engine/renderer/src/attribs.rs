// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::gl::*;
use crate::vertex::Vertex;
use std::mem::size_of;

/// For describing [`Vertex`] attributes to shaders. Not extensible right now.
pub struct Attribs<'a> {
    gl: &'a Gl,
    aia: Option<&'a Aia>,
    bytes: u32,
    index: u32,
    size: usize,
}

impl<'a> Attribs<'a> {
    pub(crate) fn new<V: Vertex>(gl: &'a Gl) -> Self {
        Self {
            gl,
            aia: None,
            bytes: 0,
            index: 0,
            size: size_of::<V>(),
        }
    }

    pub(crate) fn new_instanced<V: Vertex>(gl: &'a Gl, aia: &'a Aia, previous: Self) -> Self {
        let index = previous.index;
        Self {
            gl,
            aia: Some(aia),
            bytes: 0,
            index,
            size: size_of::<V>(),
        }
    }

    fn attrib(&mut self) -> u32 {
        let i = self.index;
        self.index += 1;

        self.gl.enable_vertex_attrib_array(i);
        if let Some(aia) = self.aia {
            aia.vertex_attrib_divisor_angle(i, 1);
        }
        i
    }

    fn offset(&mut self, bytes: usize) -> i32 {
        let b = self.bytes;
        self.bytes += bytes as u32;
        b as i32
    }

    fn vertex_attrib_pointer<T>(&mut self, count: usize, typ: u32, normalized: bool) {
        debug_assert!((1..=4).contains(&count), "invalid count: {count:?}");
        debug_assert_eq!(count * size_of::<T>() % 4, 0, "not aligned to 4 bytes");
        self.gl.vertex_attrib_pointer_with_i32(
            self.attrib(),
            count as i32,
            typ,
            normalized,
            self.size as i32,
            self.offset(count * size_of::<T>()),
        );
    }

    pub(crate) fn f32s(&mut self, count: usize) {
        self.vertex_attrib_pointer::<f32>(count, Gl::FLOAT, false)
    }

    pub(crate) fn normalized_i8s(&mut self, count: usize) {
        self.vertex_attrib_pointer::<i8>(count, Gl::BYTE, true)
    }

    pub(crate) fn normalized_u8s(&mut self, count: usize) {
        self.vertex_attrib_pointer::<u8>(count, Gl::UNSIGNED_BYTE, true)
    }

    #[cfg(feature = "webgl2")]
    fn vertex_attrib_i_pointer<T>(&mut self, count: usize, typ: u32) {
        debug_assert!((1..=4).contains(&count), "invalid count: {count:?}");
        debug_assert_eq!(count * size_of::<T>() % 4, 0, "not aligned to 4 bytes");
        self.gl.vertex_attrib_i_pointer_with_i32(
            self.attrib(),
            count as i32,
            typ,
            self.size as i32,
            self.offset(count * size_of::<T>()),
        );
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn i8s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<i8>(count, Gl::BYTE)
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn u8s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<u8>(count, Gl::UNSIGNED_BYTE)
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn i16s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<i16>(count, Gl::SHORT)
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn u16s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<u16>(count, Gl::UNSIGNED_SHORT)
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn i32s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<i32>(count, Gl::INT)
    }

    #[cfg(feature = "webgl2")]
    pub(crate) fn u32s(&mut self, count: usize) {
        self.vertex_attrib_i_pointer::<u32>(count, Gl::UNSIGNED_INT)
    }
}

impl<'a> Drop for Attribs<'a> {
    fn drop(&mut self) {
        // Make sure all attributes were added.
        assert_eq!(self.bytes as usize, self.size, "attributes don't add up");
    }
}
