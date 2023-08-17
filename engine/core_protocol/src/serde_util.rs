// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer};
use std::fmt;

pub fn is_default<T: Default + PartialEq>(x: &T) -> bool {
    x == &T::default()
}

pub fn nested_option<'de, D, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        Option::<T>::deserialize(deserializer).map(Some)
    } else {
        Option::<Option<T>>::deserialize(deserializer)
    }
}

pub struct F32Visitor;

impl<'de> Visitor<'de> for F32Visitor {
    type Value = f32;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a floating point value")
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_f32(value as f32)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_f32(value as f32)
    }

    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_f32(value as f32)
    }
}

pub struct I8Visitor;

impl<'de> Visitor<'de> for I8Visitor {
    type Value = i8;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an i8")
    }

    fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

pub struct U8Visitor;

impl<'de> Visitor<'de> for U8Visitor {
    type Value = u8;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a u8")
    }

    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

pub struct U16Visitor;

impl<'de> Visitor<'de> for U16Visitor {
    type Value = u16;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a u16")
    }

    fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

pub struct I16Visitor;

impl<'de> Visitor<'de> for I16Visitor {
    type Value = i16;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an i16")
    }

    fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

pub struct BytesVisitor;

impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_byte_buf(Vec::from(value))
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

pub struct StrVisitor;

impl<'de> Visitor<'de> for StrVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a str")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(String::from(value))
    }
}
