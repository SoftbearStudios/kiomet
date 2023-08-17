// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::gl::*;
use crate::renderer::Renderer;
use crate::rgb::rgba_array_to_css;
use glam::UVec2;
use js_hooks::document;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use web_sys::{HtmlImageElement, WebGlTexture};

/// Required for [`Texture::load`]'s callback.
struct TextureInner {
    texture: WebGlTexture,
    dimensions: Cell<UVec2>,
}

/// A 2d array of pixels that you can sample in a [`Shader`][`crate::shader::Shader`]. There
/// are several options for creating one. It's as cheap to clone as an [`Rc`]. It implements
/// [`Uniform`][`crate::Uniform`].
#[derive(Clone)]
pub struct Texture {
    inner: Rc<TextureInner>,
    format: TextureFormat,
    typ: TextureType,
}

/// A format of a [`Texture`]. Describes `bytes` in [`Texture::realloc_with_opt_bytes`] or the image
/// in [`Texture::load`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextureFormat {
    /// 1 channel as alpha.
    Alpha,
    /// 1 floating point channel as depth.
    #[cfg(feature = "depth_texture")]
    Depth,
    /// 3 channels as RGB.
    Rgb,
    /// 4 channels as RGBA.
    Rgba {
        /// Whether the RGB will be premultiplied by the alpha.
        premultiply: bool,
    },
    /// 4 f16 channels as RGBA.
    #[cfg(feature = "render_float")]
    RgbaF16,
    /// 4 f32 channels as RGBA.
    #[cfg(feature = "render_float")]
    RgbaF32,
    /// 3 channels as sRGB.
    #[cfg(feature = "srgb")]
    Srgb,
    /// 4 channels as sRGB + alpha.
    #[cfg(feature = "srgb")]
    Srgba {
        /// Whether the RGB will be premultiplied by the alpha.
        premultiply: bool,
    },
}

impl TextureFormat {
    /// 4 channels RGBA or sRGB + alpha if `srgb` feature is enabled.
    pub const COLOR_RGBA: Self = {
        #[cfg(not(feature = "srgb"))]
        let ret = Self::Rgba { premultiply: true };
        #[cfg(feature = "srgb")]
        let ret = Self::Srgba { premultiply: true };
        ret
    };

    /// 4 channels RGBA or sRGB + straight alpha if `srgb` feature is enabled.
    pub const COLOR_RGBA_STRAIGHT: Self = {
        #[cfg(not(feature = "srgb"))]
        let ret = Self::Rgba { premultiply: false };
        #[cfg(feature = "srgb")]
        let ret = Self::Srgba { premultiply: false };
        ret
    };

    /// Size of one pixel in bytes.
    fn pixel_size(&self) -> u32 {
        match self {
            Self::Alpha => 1,
            #[cfg(feature = "depth_texture")]
            Self::Depth => 2,
            Self::Rgb => 3,
            Self::Rgba { .. } => 4,
            #[cfg(feature = "render_float")]
            Self::RgbaF16 => 8,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => 16,
            #[cfg(feature = "srgb")]
            Self::Srgb => 3,
            #[cfg(feature = "srgb")]
            Self::Srgba { .. } => 4,
        }
    }

    /// Alignment between pixels in bytes.
    fn pixel_align(&self) -> u32 {
        match self {
            Self::Alpha => 1,
            #[cfg(feature = "depth_texture")]
            Self::Depth => 2,
            Self::Rgb => 1,
            Self::Rgba { .. } => 4,
            #[cfg(feature = "render_float")]
            Self::RgbaF16 => 4,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => 4,
            #[cfg(feature = "srgb")]
            Self::Srgb => 1,
            #[cfg(feature = "srgb")]
            Self::Srgba { .. } => 4,
        }
    }

    /// Get the underlying WebGL internal format.
    fn internal_format(&self) -> i32 {
        (match self {
            Self::Alpha => Gl::ALPHA,
            #[cfg(all(feature = "depth_texture", not(feature = "webgl2")))]
            Self::Depth => Gl::DEPTH_COMPONENT,
            #[cfg(all(feature = "depth_texture", feature = "webgl2"))]
            Self::Depth => Gl::DEPTH_COMPONENT16,

            #[cfg(not(feature = "webgl2"))]
            Self::Rgb => Gl::RGB,
            #[cfg(not(feature = "webgl2"))]
            Self::Rgba { .. } => Gl::RGBA,
            // More specific (webgl2's texStorage can't take less specific one).
            #[cfg(feature = "webgl2")]
            Self::Rgb => Gl::RGB8,
            #[cfg(feature = "webgl2")]
            Self::Rgba { .. } => Gl::RGBA8,

            #[cfg(feature = "render_float")]
            Self::RgbaF16 => Gl::RGBA16F,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => Gl::RGBA32F,
            #[cfg(all(not(feature = "webgl2"), feature = "srgb"))]
            Self::Srgb => Srgb::SRGB_EXT,
            #[cfg(all(feature = "webgl2", feature = "srgb"))]
            Self::Srgb => Gl::SRGB8,
            #[cfg(all(not(feature = "webgl2"), feature = "srgb"))]
            Self::Srgba { .. } => Srgb::SRGB_ALPHA_EXT,
            #[cfg(all(feature = "webgl2", feature = "srgb"))]
            Self::Srgba { .. } => Srgb::SRGB8_ALPHA8_EXT,
        }) as i32
    }

    /// Get the underlying WebGL src format.
    fn src_format(&self) -> u32 {
        #[cfg(not(feature = "webgl2"))]
        return self.internal_format() as u32;
        #[cfg(feature = "webgl2")]
        match self {
            Self::Alpha => Gl::ALPHA,
            #[cfg(feature = "depth_texture")]
            Self::Depth => Gl::DEPTH_COMPONENT,
            Self::Rgb => Gl::RGB,
            Self::Rgba { .. } => Gl::RGBA,
            #[cfg(feature = "render_float")]
            Self::RgbaF16 => Gl::RGBA,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => Gl::RGBA,
            #[cfg(feature = "srgb")]
            Self::Srgb => Gl::RGB,
            #[cfg(feature = "srgb")]
            Self::Srgba { .. } => Gl::RGBA,
        }
    }

    /// Get the underlying WebGL src type.
    fn src_type(&self) -> u32 {
        match self {
            #[cfg(feature = "depth_texture")]
            Self::Depth => Gl::UNSIGNED_SHORT,
            #[cfg(feature = "render_float")]
            Self::RgbaF16 => Gl::HALF_FLOAT,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => Gl::FLOAT,
            _ => Gl::UNSIGNED_BYTE,
        }
    }

    /// Returns if a texture of this format can generate mipmaps. WebGL can't generate sRGB/sRGBA
    /// mipmaps. WebGL2 can generate sRGBA mipmaps but not sRGB ones for *some* reason.
    fn can_generate_mipmaps(&self) -> bool {
        match self {
            #[cfg(feature = "depth_texture")]
            Self::Depth => false,
            #[cfg(feature = "render_float")]
            Self::RgbaF16 => true,
            #[cfg(feature = "render_float")]
            Self::RgbaF32 => false,
            #[cfg(feature = "srgb")]
            Self::Srgb => false,
            #[cfg(feature = "srgb")]
            Self::Srgba { .. } => cfg!(feature = "webgl2"),
            _ => true,
        }
    }

    pub(crate) fn is_srgb(&self) -> bool {
        #[cfg(not(feature = "srgb"))]
        return false;
        #[cfg(feature = "srgb")]
        matches!(self, Self::Srgb | Self::Srgba { .. })
    }

    fn has_alpha(&self) -> bool {
        #[allow(unused_mut)]
        let mut alpha = matches!(self, Self::Alpha | Self::Rgba { .. });
        #[cfg(feature = "render_float")]
        {
            alpha |= matches!(self, Self::RgbaF16 | Self::RgbaF32);
        }
        #[cfg(feature = "srgb")]
        {
            alpha |= matches!(self, Self::Srgba { .. });
        }
        alpha
    }

    pub(crate) fn premultiply_alpha(&self) -> bool {
        match self {
            Self::Rgba { premultiply } => *premultiply,
            #[cfg(feature = "srgb")]
            Self::Srgba { premultiply } => *premultiply,
            _ => false,
        }
    }
}

/// Determines the number of faces a [`Texture`] has. Get with [`typ`][`Texture::typ`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextureType {
    /// A 2 dimensional [`Texture`].
    D2,
    /// A 2 dimensional array [`Texture`].
    #[cfg(feature = "webgl2")]
    D2Array(u8),
    /// A cube map [`Texture`].
    Cube,
}

impl TextureType {
    /// Returns the depth of the [`Texture`].
    #[cfg(feature = "webgl2")]
    pub(crate) fn depth(self) -> u32 {
        match self {
            #[cfg(feature = "webgl2")]
            Self::D2Array(depth) => depth as u32,
            _ => 1,
        }
    }

    /// For [`Gl::bind_texture`] calls.
    pub(crate) fn target(self) -> u32 {
        match self {
            Self::D2 => Gl::TEXTURE_2D,
            #[cfg(feature = "webgl2")]
            Self::D2Array(_) => Gl::TEXTURE_2D_ARRAY,
            Self::Cube => Gl::TEXTURE_CUBE_MAP,
        }
    }

    /// For [`Gl::get_parameter`] calls.
    pub(crate) fn target_parameter(self) -> u32 {
        match self {
            Self::D2 => Gl::TEXTURE_BINDING_2D,
            #[cfg(feature = "webgl2")]
            Self::D2Array(_) => Gl::TEXTURE_BINDING_2D_ARRAY,
            Self::Cube => Gl::TEXTURE_BINDING_CUBE_MAP,
        }
    }

    /// Returns an iterator over the various faces of a [`Texture`] of this type.
    pub(crate) fn faces(self) -> impl Iterator<Item = TextureFace> {
        use TextureFace::*;
        match self {
            Self::D2 => [D2].as_slice(),
            #[cfg(feature = "webgl2")]
            Self::D2Array(_) => [D2Array].as_slice(),
            Self::Cube => [PX, NX, PY, NY, PZ, NZ].as_slice(),
        }
        .iter()
        .copied()
    }
}

/// Regular [`D2`][`TextureType::D2`] face and faces of [`Cube`][`TextureType::Cube`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum TextureFace {
    /// A 2 dimensional [`Texture`]'s face.
    D2,
    /// A 3 dimensional [`Texture`]'s face of [`TextureType::D2Array`].
    #[cfg(feature = "webgl2")]
    D2Array,
    /// Positive X face of [`TextureType::Cube`].
    PX,
    /// Negative X face of [`TextureType::Cube`].
    NX,
    /// Positive Y face of [`TextureType::Cube`].
    PY,
    /// Negative Y face of [`TextureType::Cube`].
    NY,
    /// Positive Z face of [`TextureType::Cube`].
    PZ,
    /// Negative z face of [`TextureType::Cube`].
    NZ,
}

impl TextureFace {
    /// Color that is set as placeholder color in [`Texture::load_inner`] if none was provided.
    fn default_color(self) -> [u8; 3] {
        match self {
            Self::D2 => [0; 3],
            #[cfg(feature = "webgl2")]
            Self::D2Array => [0; 3],
            Self::PX => [255, 127, 127],
            Self::NX => [0, 127, 127],
            Self::PY => [127, 255, 127],
            Self::NY => [127, 0, 127],
            Self::PZ => [127, 127, 255],
            Self::NZ => [127, 127, 0],
        }
    }

    /// Returns a 2d target or an error with a 3d target.
    pub(crate) fn target_2d(self) -> Result<u32, u32> {
        Ok(match self {
            Self::D2 => Gl::TEXTURE_2D,
            #[cfg(feature = "webgl2")]
            Self::D2Array => return Err(Gl::TEXTURE_2D_ARRAY),
            Self::PX => Gl::TEXTURE_CUBE_MAP_POSITIVE_X,
            Self::NX => Gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
            Self::PY => Gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
            Self::NY => Gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
            Self::PZ => Gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
            Self::NZ => Gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        })
    }

    fn url(self, img_url: &str) -> String {
        let face = match self {
            Self::D2 => return img_url.to_owned(),
            #[cfg(feature = "webgl2")]
            Self::D2Array => return img_url.to_owned(),
            Self::PX => "px",
            Self::NX => "nx",
            Self::PY => "py",
            Self::NY => "ny",
            Self::PZ => "pz",
            Self::NZ => "nz",
        };

        // "foo.png" => "foo_px.png"
        let (name, ext) = img_url.split_once('.').unwrap_or((img_url, ""));
        format!("{name}_{face}.{ext}")
    }
}

impl Texture {
    pub(crate) fn new(gl: &Gl, dimensions: UVec2, format: TextureFormat, typ: TextureType) -> Self {
        Self {
            inner: Rc::new(TextureInner {
                texture: gl.create_texture().unwrap(),
                dimensions: Cell::new(dimensions),
            }),
            format,
            typ,
        }
    }

    pub(crate) fn inner(&self) -> &WebGlTexture {
        &self.inner.texture
    }

    /// Gets aspect ratio (width / height).
    pub fn aspect(&self) -> f32 {
        let [width, height] = self.dimensions().as_vec2().to_array();
        width / height
    }

    /// Gets dimensions in pixels.
    pub fn dimensions(&self) -> UVec2 {
        self.inner.dimensions.get()
    }

    /// Gets the [`TextureType`] of the [`Texture`].
    pub fn typ(&self) -> TextureType {
        self.typ
    }

    /// Creates a new empty [`Texture`] with the given `format` and `linear_filter`. Mipmaps and repeating
    /// cannot be used.
    pub fn new_empty(renderer: &Renderer, format: TextureFormat, linear_filter: bool) -> Self {
        let gl = &renderer.gl;
        let typ = TextureType::D2;

        let texture = Self::new(gl, UVec2::ZERO, format, typ);
        let target = typ.target();
        let binding = texture.bind(renderer, 0);

        // Can't be repeating because size isn't known yet.
        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);

        let filter = if linear_filter {
            Gl::LINEAR
        } else {
            Gl::NEAREST
        } as i32;

        gl.tex_parameteri(target, Gl::TEXTURE_MIN_FILTER, filter);
        gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, filter);

        #[cfg(feature = "depth_texture")]
        if format == TextureFormat::Depth && linear_filter {
            #[cfg(not(feature = "webgl2"))]
            panic!("linear filtering of depth textures is only supported in webgl2");
            #[cfg(feature = "webgl2")]
            {
                gl.tex_parameteri(
                    target,
                    Gl::TEXTURE_COMPARE_MODE,
                    Gl::COMPARE_REF_TO_TEXTURE as i32,
                );
                gl.tex_parameteri(target, Gl::TEXTURE_COMPARE_FUNC, Gl::LESS as i32);
            }
        }

        drop(binding);
        texture
    }

    /// Copies the `bytes` to the [`Texture`], resizing to `dimensions` if necessary. The
    /// [`Texture`] must have been created with [`Texture::new_empty`].
    pub fn realloc_with_opt_bytes(
        &mut self,
        renderer: &Renderer,
        dimensions: UVec2,
        bytes: Option<&[u8]>,
    ) {
        let typ = self.typ;
        assert_eq!(typ, TextureType::D2);
        let target = typ.target();
        let gl = &renderer.gl;
        let binding = self.bind(renderer, 0);

        // No mipmaps.
        let level = 0;
        let src_format = self.format.src_format();
        let src_type = self.format.src_type();
        let [width, height] = dimensions.to_array();

        if let Some(bytes) = bytes {
            let pixel_size = self.format.pixel_size();
            assert_eq!(
                width * height * pixel_size,
                bytes.len() as u32,
                "{}x{}x{}",
                width,
                height,
                pixel_size
            );
        }

        // Set alignment if it's not the default.
        let align = self.format.pixel_align();
        if align != 4 {
            gl.pixel_storei(Gl::UNPACK_ALIGNMENT, align as i32);
        }

        // Don't reallocate if dimensions haven't changed.
        if self.dimensions() == dimensions {
            gl.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
                target,
                level,
                0,
                0,
                width as i32,
                height as i32,
                src_format,
                src_type,
                bytes,
            )
            .unwrap();
        } else {
            self.inner.dimensions.set(dimensions);

            let internal_format = self.format.internal_format();
            let border = 0;

            gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                target,
                level,
                internal_format,
                width as i32,
                height as i32,
                border,
                src_format,
                src_type,
                bytes,
            )
            .unwrap();
        }

        // Reset to the default alignment.
        if align != 4 {
            gl.pixel_storei(Gl::UNPACK_ALIGNMENT, 4);
        }

        drop(binding);
    }

    /// Creates a [`Texture`] from `text`, with variable length and constant height. It's format
    /// will be `TextureFormat::COLOR_RGBA`. Pass `color` to this function instead of coloring in a
    /// [`Shader`][`crate::shader::Shader`] so emoji colors are preserved.
    pub fn from_text(renderer: &Renderer, text: &str, color: [u8; 4]) -> Self {
        let (canvas, context) = create_canvas();

        const FONT: &str = "30px Arial";
        const HEIGHT: u32 = 36; // 32 -> 36 to fit "😊".

        context.set_font(FONT);
        context.set_text_baseline("bottom");
        let text_width = context.measure_text(text).unwrap().width();

        let canvas_width = text_width as u32 + 2;
        canvas.set_width(canvas_width);
        canvas.set_height(HEIGHT);

        let color_string = rgba_array_to_css(color);

        context.set_fill_style(&JsValue::from_str(&color_string));
        context.set_font(FONT);
        context.set_text_baseline("bottom");

        context
            .fill_text(text, 1.0, (HEIGHT - 1) as f64)
            .expect("could not fill text on canvas");

        let format = TextureFormat::COLOR_RGBA;
        let dimensions = UVec2::new(canvas_width, HEIGHT);

        let gl = &renderer.gl;
        let texture = Self::new(gl, dimensions, format, TextureType::D2);
        let target = texture.typ.target();
        let binding = texture.bind(renderer, 0);

        // No mipmaps since not always a power of 2.
        let level = 0;

        // Always use RGBA because text can have colored unicode.
        let internal_format = format.internal_format();
        let src_format = format.src_format();
        let src_type = format.src_type();

        let premultiply = format.premultiply_alpha();
        if premultiply {
            gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1); // Canvas isn't premultiplied.
        }

        gl.tex_image_2d_with_u32_and_u32_and_canvas(
            target,
            level,
            internal_format,
            src_format,
            src_type,
            &canvas,
        )
        .expect("could not draw canvas to texture");

        if premultiply {
            gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
        }

        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(target, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);

        drop(binding);
        texture
    }

    /// Loads a [`Texture`] from `img_url`. You may specify a `placeholder` color for use before
    /// the image loads. You may use `repeating: true` if the loaded image has power of 2
    /// dimensions or the `webgl2` feature is enabled.
    pub fn load(
        renderer: &Renderer,
        img_url: &str,
        format: TextureFormat,
        placeholder: Option<[u8; 3]>,
        repeating: bool,
    ) -> Self {
        Self::load_inner(
            renderer,
            img_url,
            format,
            placeholder,
            repeating,
            TextureType::D2,
        )
    }

    /// Loads an array [`Texture`] from `img_url`. You may specify a `placeholder` color for use
    /// before the image loads. The image should contain layers in a vertical column.
    #[cfg(feature = "webgl2")]
    pub fn load_array(
        renderer: &Renderer,
        img_url: &str,
        format: TextureFormat,
        placeholder: Option<[u8; 3]>,
        repeating: bool,
        layers: usize,
    ) -> Self {
        Self::load_inner(
            renderer,
            img_url,
            format,
            placeholder,
            repeating,
            TextureType::D2Array(layers.try_into().expect("max layers is 255")),
        )
    }

    /// Loads a [`cube map`](https://en.wikipedia.org/wiki/Cube_mapping) [`Texture`] from `img_url`. You may specify a `placeholder` color for use
    /// before each image loads.
    pub fn load_cube(
        renderer: &Renderer,
        img_url: &str,
        format: TextureFormat,
        placeholder: Option<[u8; 3]>,
    ) -> Self {
        Self::load_inner(
            renderer,
            img_url,
            format,
            placeholder,
            false,
            TextureType::Cube,
        )
    }

    fn load_inner(
        renderer: &Renderer,
        img_url: &str,
        format: TextureFormat,
        placeholder: Option<[u8; 3]>,
        repeating: bool,
        typ: TextureType,
    ) -> Self {
        assert!(!matches!(format, TextureFormat::Alpha), "not supported");

        let gl = &renderer.gl;
        let texture = Self::new(gl, UVec2::ONE, format, typ);
        let target = typ.target();
        let binding = texture.bind(renderer, 0);

        let internal_format = format.internal_format();
        let src_format = format.src_format();
        let src_type = format.src_type();

        // Unloaded textures are single pixel of placeholder or 0 alpha.
        let level = 0;
        let width = 1;
        let height = 1;
        #[cfg(feature = "webgl2")]
        let depth = typ.depth();
        let border = 0;

        for face in typ.faces() {
            // Always set placeholder or some browsers show a warning in console. Different faces
            // have different default placeholders.
            let p = placeholder.unwrap_or_else(|| face.default_color());
            let alpha_p;

            let pixel = if format.has_alpha() {
                alpha_p = [p[0], p[1], p[2], placeholder.is_some() as u8 * 255];
                alpha_p.as_slice()
            } else {
                p.as_slice()
            };

            match face.target_2d() {
                Ok(target_2d) => {
                    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                        target_2d,
                        level,
                        internal_format,
                        width,
                        height,
                        border,
                        src_format,
                        src_type,
                        Some(pixel),
                    )
                    .unwrap();
                }
                #[cfg(feature = "webgl2")]
                Err(target_3d) => {
                    let pixels: Vec<_> = (0..depth).flat_map(|_| pixel.iter().copied()).collect();

                    gl.tex_image_3d_with_opt_u8_array(
                        target_3d,
                        level,
                        internal_format,
                        width,
                        height,
                        depth as i32,
                        border,
                        src_format,
                        src_type,
                        Some(&pixels),
                    )
                    .unwrap();
                }
                #[cfg(not(feature = "webgl2"))]
                _ => unimplemented!(),
            }
        }

        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(
            target,
            Gl::TEXTURE_MIN_FILTER,
            Gl::LINEAR_MIPMAP_LINEAR as i32,
        );
        gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);

        drop(binding);

        let images: Rc<[HtmlImageElement]> = typ
            .faces()
            .map(|_| HtmlImageElement::new().unwrap())
            .collect();

        let gl = Rc::new(gl.clone());

        // Can't borrow renderer inside callback.
        #[cfg(feature = "anisotropy")]
        let anisotropy = renderer.anisotropy;
        #[cfg(feature = "webgl2")]
        let max_array_texture_layers =
            matches!(typ, TextureType::D2Array(_)).then(|| renderer.max_array_texture_layers());
        let max_texture_size = renderer.max_texture_size();

        for (img, face) in images.iter().zip(typ.faces()) {
            let gl = Rc::clone(&gl);
            let inner = texture.inner.clone();
            let images = Rc::clone(&images);

            // Callback when image is done loading.
            let closure = Closure::once(move || {
                // Wait for all images to load. Uses FnOnce to make sure Rc<Gl> is dropped after
                // each call.
                if Rc::strong_count(&gl) != 1 {
                    return;
                }

                bind_texture_checked(&gl, typ, &inner.texture);
                let premultiply = format.premultiply_alpha();
                if premultiply {
                    gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1);
                }
                gl.pixel_storei(Gl::UNPACK_COLORSPACE_CONVERSION_WEBGL, Gl::NONE as i32);

                let mut previous_dimensions = None;

                for (img, face) in images.iter().zip(typ.faces()) {
                    let dimensions = UVec2::new(img.width(), img.height());
                    if typ == TextureType::Cube {
                        assert_eq!(
                            dimensions.x, dimensions.y,
                            "cube map must have square faces"
                        );
                    }

                    match face.target_2d() {
                        Ok(target_2d) => {
                            // Polyfill: clamp to max texture size to avoid errors.
                            let max_dimensions = UVec2::splat(max_texture_size);
                            let old_dim = dimensions;
                            let dimensions = dimensions.min(max_dimensions);

                            let prev = previous_dimensions.get_or_insert_with(|| {
                                inner.dimensions.set(dimensions);
                                dimensions
                            });
                            assert_eq!(*prev, dimensions, "cube map face size mismatch");

                            if dimensions != old_dim {
                                // Resize with canvas if needed.
                                let (canvas, context) = create_canvas();

                                canvas.set_width(dimensions.x);
                                canvas.set_height(dimensions.y);

                                context
                                    .draw_image_with_html_image_element_and_dw_and_dh(
                                        img,
                                        0.0,
                                        0.0,
                                        dimensions.x as f64,
                                        dimensions.y as f64,
                                    )
                                    .expect("failed to resize image");

                                gl.tex_image_2d_with_u32_and_u32_and_canvas(
                                    target_2d,
                                    level,
                                    internal_format,
                                    src_format,
                                    src_type,
                                    &canvas,
                                )
                                .expect("failed to load resized image");
                            } else {
                                gl.tex_image_2d_with_u32_and_u32_and_image(
                                    target_2d,
                                    level,
                                    internal_format,
                                    src_format,
                                    src_type,
                                    img,
                                )
                                .expect("failed to load image");
                            }
                        }
                        #[cfg(feature = "webgl2")]
                        Err(target_3d) => {
                            let width = dimensions.x;
                            let height = dimensions.y / depth;

                            assert!(width <= max_texture_size);
                            assert!(height <= max_texture_size);

                            let max_array_texture_layers = max_array_texture_layers.unwrap();
                            assert!(depth <= max_array_texture_layers);

                            // Chrome has a bug where an image larger than max_texture_size can't be
                            // passed to texImage or texSubImage. To work around this we split up
                            // the image into multiple canvases, each within max_texture_size.
                            if dimensions.y > max_texture_size {
                                let levels = (width.max(height).ilog2() + 1) as i32;
                                gl.tex_storage_3d(
                                    target_3d,
                                    levels,
                                    internal_format as u32,
                                    width as i32,
                                    height as i32,
                                    depth as i32,
                                );

                                let step = max_texture_size / height;
                                for start in (0..depth).step_by(step as usize) {
                                    let end = (start + step).min(depth);
                                    let layers = end - start;

                                    let (canvas, context) = create_canvas();
                                    let canvas_height = height * layers;
                                    canvas.set_width(width);
                                    canvas.set_height(canvas_height);

                                    context
                                        .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                            img,
                                            0.0,
                                            (start * height) as f64,
                                            width as f64,
                                            canvas_height as f64,
                                            0.0,
                                            0.0,
                                            width as f64,
                                            canvas_height as f64,
                                        ).unwrap();

                                    gl.tex_sub_image_3d_with_html_canvas_element(
                                        target_3d,
                                        0,
                                        0,
                                        0,
                                        start as i32,
                                        width as i32,
                                        height as i32,
                                        layers as i32,
                                        src_format,
                                        src_type,
                                        &canvas,
                                    )
                                    .unwrap()
                                }
                            } else {
                                gl.tex_image_3d_with_html_image_element(
                                    target_3d,
                                    level,
                                    internal_format,
                                    width as i32,
                                    height as i32,
                                    depth as i32,
                                    border,
                                    src_format,
                                    src_type,
                                    img,
                                )
                                .unwrap();
                            }
                        }
                        #[cfg(not(feature = "webgl2"))]
                        _ => unimplemented!(),
                    }
                }

                gl.pixel_storei(
                    Gl::UNPACK_COLORSPACE_CONVERSION_WEBGL,
                    Gl::BROWSER_DEFAULT_WEBGL as i32,
                );
                if premultiply {
                    gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
                }

                let dimensions = inner.dimensions.get();
                let is_pow2_or_webgl2 = cfg!(feature = "webgl2")
                    || (dimensions.x.is_power_of_two() && dimensions.y.is_power_of_two());

                if is_pow2_or_webgl2 && format.can_generate_mipmaps() {
                    gl.generate_mipmap(target);
                    gl.tex_parameteri(
                        target,
                        Gl::TEXTURE_MIN_FILTER,
                        Gl::LINEAR_MIPMAP_LINEAR as i32,
                    );
                } else {
                    gl.tex_parameteri(target, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
                }

                #[cfg(feature = "anisotropy")]
                if let Some(anisotropy) = anisotropy {
                    gl.tex_parameteri(target, Ani::TEXTURE_MAX_ANISOTROPY_EXT, anisotropy as i32);
                }

                gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);
                if repeating {
                    if !is_pow2_or_webgl2 {
                        panic!("repeating texture must be power of two")
                    }
                    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::REPEAT as i32);
                    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::REPEAT as i32);
                } else {
                    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
                    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
                }
                unbind_texture_cfg_debug(&gl, typ);
            });

            img.set_onload(Some(closure.as_ref().unchecked_ref()));
            closure.forget();

            // For compatibility with redirect scheme.
            img.set_cross_origin(Some("anonymous"));

            // Start loading images.
            // Cube maps have multiple faces so "foo.png" would map to "foo_px.png", "foo_nx.png"...
            img.set_src(&face.url(img_url));
        }

        texture
    }

    /// Bind a texture for affecting subsequent draw calls.
    #[must_use]
    pub(crate) fn bind<'a>(&self, renderer: &'a Renderer, index: usize) -> TextureBinding<'a> {
        TextureBinding::new(renderer, index, self)
    }
}

/// Creates a temporary canvas for drawing and then converting into a texture.
fn create_canvas() -> (HtmlCanvasElement, CanvasRenderingContext2d) {
    let canvas: HtmlCanvasElement = document()
        .create_element("canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap();

    let context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<CanvasRenderingContext2d>()
        .unwrap();

    (canvas, context)
}

pub(crate) struct TextureBinding<'a> {
    renderer: &'a Renderer,
    index: usize,
    texture_type: TextureType,
}

impl<'a> TextureBinding<'a> {
    fn new(renderer: &'a Renderer, index: usize, texture: &Texture) -> Self {
        renderer.active_texture(index);
        bind_texture_checked(&renderer.gl, texture.typ, &texture.inner.texture);

        Self {
            renderer,
            index,
            texture_type: texture.typ,
        }
    }

    /// Texture must have been created from the same index and passed to [`std::mem::forget`].
    pub(crate) fn drop_raw_parts(renderer: &'a Renderer, index: usize, texture_type: TextureType) {
        drop(Self {
            renderer,
            index,
            texture_type,
        })
    }
}

impl<'a> Drop for TextureBinding<'a> {
    fn drop(&mut self) {
        // Set active texture (not required in release mode because not unbinding).
        if cfg!(debug_assertions) {
            self.renderer.active_texture(self.index);
            unbind_texture_cfg_debug(&self.renderer.gl, self.texture_type)
        }
    }
}

/// Like Gl::bind_texture but debug asserts that no texture was bound.
fn bind_texture_checked(gl: &Gl, typ: TextureType, texture: &WebGlTexture) {
    // Make sure binding was cleared.
    debug_assert!(
        gl.get_parameter(typ.target_parameter()).unwrap().is_null(),
        "texture already bound"
    );

    gl.bind_texture(typ.target(), Some(texture));
}

// Unbind texture in debug mode (not required in release mode).
fn unbind_texture_cfg_debug(gl: &Gl, typ: TextureType) {
    if cfg!(debug_assertions) {
        gl.bind_texture(typ.target(), None);
    }
}
