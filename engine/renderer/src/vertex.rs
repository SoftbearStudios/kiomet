// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::attribs::Attribs;
use bytemuck::{Pod, Zeroable};
use glam::*;

#[doc(hidden)]
pub use bytemuck::{Pod as __Pod, Zeroable as __Zeroable};
pub use engine_macros::Vertex;

/// Any data consisting of [`prim@f32`]s. You can derive it on a struct with
/// [`derive_vertex`][`crate::derive_vertex`].
pub trait Vertex: Pod {
    #[doc(hidden)]
    fn bind_attribs(attribs: &mut Attribs);
}

/// For easily deriving vertex and friends. Unfortunately requires putting `bytemuck = "1.9"` in
/// your `Cargo.toml`.
#[macro_export]
macro_rules! derive_vertex {
    ($s:item) => {
        #[derive(Copy, Clone, $crate::Vertex, $crate::__Pod, $crate::__Zeroable)]
        #[repr(C)]
        $s
    };
}

macro_rules! impl_vertex {
    ($a: ty, $count: literal, $function: ident) => {
        impl Vertex for $a {
            fn bind_attribs(attribs: &mut Attribs) {
                attribs.$function($count);
            }
        }
    };
}

macro_rules! int {
    ($a: ty, $count: literal, $function: ident) => {
        #[cfg(feature = "webgl2")]
        impl_vertex!($a, $count, $function);
    };
}

// Only implemented on arrays that are multiples of 4 bytes.
int!([i8; 4], 4, i8s);
int!([i16; 2], 2, i16s);
int!([i16; 4], 4, i16s);
int!(i32, 1, i32s);
int!([i32; 2], 2, i32s);
int!([i32; 3], 3, i32s);
int!([i32; 4], 4, i32s);
int!([u8; 4], 4, u8s);
int!([u16; 2], 2, u16s);
int!([u16; 4], 4, u16s);
int!(u32, 1, u32s);
int!([u32; 2], 2, u32s);
int!([u32; 3], 3, u32s);
int!([u32; 4], 4, u32s);

/// Vec4 but with 8 bits per component instead of 32. Capable of representing [-1, 1].
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, align(4))]
pub struct SmolVec4([i8; 4]);

impl Vertex for SmolVec4 {
    fn bind_attribs(attribs: &mut Attribs) {
        attribs.normalized_i8s(4)
    }
}

impl From<Vec4> for SmolVec4 {
    #[inline]
    fn from(v: Vec4) -> Self {
        let v = v * 127.0;
        Self([v.x as i8, v.y as i8, v.z as i8, v.w as i8])
    }
}

impl From<[i8; 4]> for SmolVec4 {
    #[inline]
    fn from(v: [i8; 4]) -> Self {
        Self(v)
    }
}

/// Like [`SmolVec4`] but unsigned. Capable of representing [0, 1].
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, align(4))]
pub struct SmolUVec4([u8; 4]);

impl Vertex for SmolUVec4 {
    fn bind_attribs(attribs: &mut Attribs) {
        attribs.normalized_u8s(4)
    }
}

impl From<Vec4> for SmolUVec4 {
    #[inline]
    fn from(v: Vec4) -> Self {
        let v = v * 255.0;
        Self([v.x as u8, v.y as u8, v.z as u8, v.w as u8])
    }
}

impl From<[u8; 4]> for SmolUVec4 {
    #[inline]
    fn from(v: [u8; 4]) -> Self {
        Self(v)
    }
}

macro_rules! impl_vertex_floats {
    ($a: ty, $count: literal) => {
        impl_vertex!($a, $count, f32s);
    };
}

impl_vertex_floats!(f32, 1);
impl_vertex_floats!(Vec2, 2);
impl_vertex_floats!(Vec3, 3);

// These are normally 16 byte aligned (breaking derive Pod) but not with glam's scalar-math feature.
impl_vertex_floats!(Vec4, 4);

macro_rules! impl_matrix {
    ($a: ty, $count: literal) => {
        impl Vertex for $a {
            fn bind_attribs(attribs: &mut Attribs) {
                for _ in 0..$count {
                    attribs.f32s($count);
                }
            }
        }
    };
}

impl_matrix!(Mat2, 2);
impl_matrix!(Mat3, 3);
impl_matrix!(Mat4, 4);
