// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::prelude::*;
use core_protocol::serde_util::{F32Visitor, I16Visitor};
use glam::{Mat2, Vec2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::f32::consts::PI;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

pub type AngleRepr = i16;

/// Represents an angle with a `i16` instead of a `f32` to get wrapping for free and be 2 bytes
/// instead of 4. All [`Angle`]'s methods and trait `impl`s are cross-platform deterministic unlike
/// [`f32::sin`], [`f32::cos`], [`f32::atan2`] etc.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Encode, Decode)]
pub struct Angle(pub AngleRepr);

impl Angle {
    pub const ZERO: Self = Self(0);
    pub const MIN: Self = Self(AngleRepr::MIN);
    pub const MAX: Self = Self(AngleRepr::MAX);
    pub const PI: Self = Self(AngleRepr::MAX);
    pub const PI_2: Self = Self(AngleRepr::MAX / 2);

    pub fn new() -> Self {
        Self::ZERO
    }

    /// Replacement for [`f32::atan2`]. Uses cross-platform deterministic atan2.
    pub fn from_atan2(y: f32, x: f32) -> Self {
        Self::from_radians(deterministic_atan2(y, x))
    }

    /// Replacement for [`f32::sin_cos`] (returns `vec2(cos, sin)`). Uses cross-platform
    /// deterministic sin/cos.
    #[inline]
    pub fn to_vec(self) -> Vec2 {
        let radians = self.to_radians();
        Vec2::new(
            fastapprox::fast::cos(radians),
            fastapprox::fast::sin(radians),
        )
    }

    /// Replacement for `vec.y.atan2(vec.x)`. Uses cross-platform deterministic atan2.
    #[inline]
    pub fn from_vec(vec: Vec2) -> Self {
        Self::from_atan2(vec.y, vec.x)
    }

    /// Replacement for [`Mat2::from_angle`]. Uses cross-platform deterministic sin/cos.
    #[inline]
    pub fn to_mat2(self) -> Mat2 {
        let [cos, sin] = self.to_vec().to_array();
        Mat2::from_cols_array(&[cos, sin, -sin, cos])
    }

    /// Converts the [`Angle`] to an `f32` in radians in the range [-PI, PI]. Opposite of
    /// [`Angle::from_radians`].
    #[inline]
    pub fn to_radians(self) -> f32 {
        self.0 as f32 * (PI / Self::PI.0 as f32)
    }

    /// Converts an `f32` in radians to an [`Angle`]. Opposite of [`Angle::to_radians`].
    #[inline]
    pub fn from_radians(radians: f32) -> Self {
        Self((radians * (Self::PI.0 as f32 / PI)) as i32 as AngleRepr)
    }

    /// Like [`Angle::from_radians`] but angles greater than `PI` are clamped to `PI`, and angles
    /// less than -`PI` are clamped to -`PI`.
    #[inline]
    pub fn saturating_from_radians(radians: f32) -> Self {
        Self((radians * (Self::PI.0 as f32 / PI)) as AngleRepr)
    }

    /// Converts the [`Angle`] to an `f32` in degrees in the range [-180, 180]. Opposite of
    /// [`Angle::from_degrees`].
    pub fn to_degrees(self) -> f32 {
        self.to_radians().to_degrees()
    }

    /// Converts an `f32` in degrees to an [`Angle`]. Opposite of [`Angle::to_degrees`].
    pub fn from_degrees(degrees: f32) -> Self {
        Self::from_radians(degrees.to_radians())
    }

    /// Converts an `f32` in revolutions to an [`Angle`].  One revolution is 360 degrees.
    #[inline]
    pub fn from_revolutions(revolutions: f32) -> Self {
        Self((revolutions * (2.0 * AngleRepr::MAX as f32)) as i32 as AngleRepr)
    }

    pub fn abs(self) -> Self {
        if self.0 == AngleRepr::MIN {
            // Don't negate with overflow.
            return Angle::MAX;
        }
        Self(self.0.abs())
    }

    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    pub fn clamp_magnitude(self, max: Self) -> Self {
        if max.0 >= 0 {
            Self(self.0.clamp(-max.0, max.0))
        } else {
            // Clamping to over 180 degrees in either direction, any angle is valid.
            self
        }
    }

    pub fn lerp(self, other: Self, value: f32) -> Self {
        self + (other - self) * value
    }

    /// Increases clockwise with straight up being 0. Output always 0..=359, never 360.
    pub fn to_bearing(self) -> u16 {
        ((Self::PI_2 - self).0 as u16 as u32 * 360 / (u16::MAX as u32 + 1)) as u16
    }

    /// E, NE, SW, etc.
    pub fn to_cardinal(self) -> &'static str {
        let idx =
            ((self.0 as u16).wrapping_add(u16::MAX / 16)) / ((u16::MAX as u32 + 1) / 8) as u16;
        ["E", "NE", "N", "NW", "W", "SW", "S", "SE"][idx as usize]
    }
}

impl Default for Angle {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<Angle> for Vec2 {
    fn from(angle: Angle) -> Self {
        angle.to_vec()
    }
}

impl From<Vec2> for Angle {
    fn from(vec: Vec2) -> Self {
        Self::from_vec(vec)
    }
}

impl Add for Angle {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self(self.0.wrapping_add(other.0))
    }
}

impl AddAssign for Angle {
    fn add_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_add(other.0);
    }
}

impl Sub for Angle {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self(self.0.wrapping_sub(other.0))
    }
}

impl SubAssign for Angle {
    fn sub_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_sub(other.0);
    }
}

impl Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::ZERO - self
    }
}

impl Mul<f32> for Angle {
    type Output = Self;

    fn mul(self, other: f32) -> Self::Output {
        Self((self.0 as f32 * other) as i32 as AngleRepr)
    }
}

#[cfg(any(test, feature = "rand"))]
use rand::prelude::*;
#[cfg(any(test, feature = "rand"))]
impl Distribution<Angle> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Angle {
        Angle(rng.gen())
    }
}

impl fmt::Debug for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} degrees", self.to_degrees())
    }
}

impl Serialize for Angle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_f32(self.to_radians())
        } else {
            serializer.serialize_i16(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Angle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer
                .deserialize_f32(F32Visitor)
                .map(Self::from_radians)
        } else {
            deserializer.deserialize_i16(I16Visitor).map(Self)
        }
    }
}

#[allow(clippy::excessive_precision)]
pub(crate) fn deterministic_atan2(y: f32, x: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        return f32::NAN;
    }
    // https://math.stackexchange.com/a/1105038
    let (ax, ay) = (x.abs(), y.abs());
    let a = ax.min(ay) / ax.max(ay);
    let s = a * a;
    let mut r = ((-0.0464964749 * s + 0.15931422) * s - 0.327622764) * s * a + a;
    if ay > ax {
        r = 1.57079637 - r;
    }
    if x < 0.0 {
        r = 3.14159274 - r;
    }
    if y < 0.0 {
        r = -r;
    }
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::deterministic_atan2;
    use crate::angle::Angle;
    use glam::Vec2;
    use rand::distributions::Standard;
    use rand::prelude::*;
    use test::Bencher;

    #[test]
    fn det_atan2() {
        assert_eq!(deterministic_atan2(0.0, 0.0), 0f32.atan2(0.0));
        assert!(deterministic_atan2(1.0, f32::NAN).is_nan());
        assert!(deterministic_atan2(f32::NAN, 1.0).is_nan());

        let test_cases = (-1000000..=1000000)
            .step_by(10000)
            .map(|n| n as f32)
            .chain((-1000..=1000).step_by(5).map(|n| n as f32))
            .chain((-1000..=1000).map(|n| n as f32 * 0.01))
            .chain((-100..=100).map(|n| n as f32 * 0.0001))
            .chain((-100..=100).map(|n| n as f32 * 0.000001));
        for x in test_cases.clone() {
            for y in test_cases.clone() {
                let standard = y.atan2(x);
                let deterministic = deterministic_atan2(y, x);
                assert!(
                    (standard - deterministic).abs() < 0.001,
                    "atan2({x}, {y}) = std: {standard} det: {deterministic}"
                );
            }
        }
    }

    fn dataset<T>() -> Vec<T>
    where
        Standard: Distribution<T>,
    {
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(Default::default());
        (0..1000).map(|_| rng.gen()).collect()
    }

    #[bench]
    fn bench_to_vec(b: &mut Bencher) {
        let dataset = dataset::<Angle>();
        b.iter(|| {
            let mut sum = Vec2::ZERO;
            for a in dataset.as_slice() {
                sum += a.to_vec();
            }
            sum
        })
    }

    #[bench]
    fn bench_atan2(b: &mut Bencher) {
        let dataset = dataset::<Vec2>();
        b.iter(|| {
            let mut sum = 0.0;
            for v in dataset.as_slice() {
                sum += f32::atan2(v.x, v.y)
            }
            sum
        })
    }

    #[bench]
    fn bench_deterministic_atan2(b: &mut Bencher) {
        let dataset = dataset::<Vec2>();
        b.iter(|| {
            let mut sum = 0.0;
            for v in dataset.as_slice() {
                sum += deterministic_atan2(v.x, v.y)
            }
            sum
        })
    }

    #[test]
    fn radians() {
        for i in -1000..1000 {
            let r = (i as f32) / 100.0;
            let a = Angle::from_radians(r);
            let r2 = a.to_radians();
            let a2 = Angle::from_radians(r2);
            assert!((a - a2).to_radians().abs() < 0.0001, "{:?} -> {:?}", a, a2);
        }
    }

    #[test]
    fn serde() {
        for i in -1000..1000 {
            let r = (i as f32) / 100.0;
            let rs = format!("{}", r);
            let a: Angle = serde_json::from_str(&rs).unwrap();
            let rs2 = serde_json::to_string(&a).unwrap();
            let a2: Angle = serde_json::from_str(&rs2).unwrap();
            assert!((a - a2).to_radians().abs() < 0.0001, "{:?} -> {:?}", a, a2);
        }
    }

    #[test]
    fn pi() {
        // Just less than PI.
        let rs = "3.141592653589793";
        let a: Angle = serde_json::from_str(rs).unwrap();
        assert_eq!(a, Angle::PI);

        // Greater than PI.
        let rs2 = "3.141689";
        let a2: Angle = serde_json::from_str(rs2).unwrap();
        assert!(a2.to_radians() < -3.0, "{a2:?}");
    }

    #[test]
    fn unit_vec() {
        let v = Angle::ZERO.to_vec();
        assert!(v.abs_diff_eq(Vec2::X, 0.0001), "{v:?}");

        let v = Angle::PI_2.to_vec();
        assert!(v.abs_diff_eq(Vec2::Y, 0.0001), "{v:?}");
    }

    #[test]
    fn abs() {
        assert_eq!(Angle::from_radians(0.0).abs(), Angle::from_radians(0.0));
        assert_eq!(Angle::from_radians(0.5).abs(), Angle::from_radians(0.5));
        assert_eq!(Angle::from_radians(-0.5).abs(), Angle::from_radians(0.5));
    }

    #[test]
    fn min() {
        assert_eq!(
            Angle::from_radians(0.5).min(Angle::from_radians(0.6)),
            Angle::from_radians(0.5)
        );
        assert_eq!(
            Angle::from_radians(0.5).min(Angle::from_radians(0.4)),
            Angle::from_radians(0.4)
        );
        assert_eq!(
            Angle::from_radians(-0.5).min(Angle::from_radians(0.6)),
            Angle::from_radians(-0.5)
        );
        assert_eq!(
            Angle::from_radians(-0.5).min(Angle::from_radians(0.4)),
            Angle::from_radians(-0.5)
        );
    }

    #[test]
    fn clamp_magnitude() {
        assert_eq!(
            Angle::from_radians(0.5).clamp_magnitude(Angle::from_radians(0.6)),
            Angle::from_radians(0.5)
        );
        assert_eq!(
            Angle::from_radians(0.5).clamp_magnitude(Angle::from_radians(0.4)),
            Angle::from_radians(0.4)
        );
        assert_eq!(
            Angle::from_radians(-0.5).clamp_magnitude(Angle::from_radians(0.6)),
            Angle::from_radians(-0.5)
        );
        assert_eq!(
            Angle::from_radians(-0.5).clamp_magnitude(Angle::from_radians(0.4)),
            Angle::from_radians(-0.4)
        );
    }

    #[test]
    fn to_bearing() {
        assert_eq!(Angle::PI_2.to_bearing(), 0);
        assert_eq!(Angle::PI.to_bearing(), 270);

        for i in 0..i16::MAX {
            let b = Angle(i).to_bearing();
            assert!(b < 360, "{} -> {} >= 360", i, b);
        }
    }

    #[test]
    fn to_cardinal() {
        // Make sure it doesn't panic.
        for i in 0..i16::MAX {
            Angle(i).to_cardinal();
        }

        assert_eq!(Angle::ZERO.to_cardinal(), "E");
        assert_eq!(Angle::PI_2.to_cardinal(), "N");
        assert_eq!(Angle::PI.to_cardinal(), "W");
        assert_eq!(Angle(u16::MAX as i16).to_cardinal(), "E");
    }

    #[test]
    fn saturating_from_radians() {
        let a = Angle::saturating_from_radians(1000.0);
        let b = Angle::PI;
        assert_eq!(a, b);

        let a = Angle::saturating_from_radians(-1000.0);
        let b = Angle::MIN;
        assert_eq!(a, b);
    }
}
