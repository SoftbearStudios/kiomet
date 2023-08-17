// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::Referrer;
use bitcode::{Decode, Encode};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt::{self, Debug, Display, Formatter};
use std::num::{NonZeroU32, NonZeroU64, NonZeroU8};
use std::str::FromStr;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};
use variant_count::VariantCount;

#[cfg(feature = "server")]
use rand::distributions::{Standard, WeightedIndex};
#[cfg(feature = "server")]
use rand::prelude::*;

macro_rules! impl_wrapper_from_str {
    ($typ:ty, $inner:ty) => {
        impl std::fmt::Display for $typ {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
                Display::fmt(&self.0, f)
            }
        }

        impl std::str::FromStr for $typ {
            type Err = <$inner as FromStr>::Err;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(FromStr::from_str(s)?))
            }
        }
    };
}

pub type ClientHash = u16;

/// WebSocket reconnection token.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct Token(pub NonZeroU32);
impl_wrapper_from_str!(Token, NonZeroU32);

#[cfg(feature = "server")]
impl Distribution<Token> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Token {
        Token(rng.gen())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct ArenaToken(pub NonZeroU32);
impl_wrapper_from_str!(ArenaToken, NonZeroU32);

/// Cohorts 1-4 are used for A/B testing.
/// The default for existing players is cohort 1.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct CohortId(pub NonZeroU8);
impl_wrapper_from_str!(CohortId, NonZeroU8);

impl CohortId {
    const WEIGHTS: [u8; 4] = [8, 4, 2, 1];

    pub fn new(n: u8) -> Option<Self> {
        NonZeroU8::new(n)
            .filter(|n| n.get() <= Self::WEIGHTS.len() as u8)
            .map(Self)
    }
}

impl Default for CohortId {
    fn default() -> Self {
        Self::new(1).unwrap()
    }
}

#[cfg(feature = "server")]
impl Distribution<CohortId> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CohortId {
        use std::sync::LazyLock;
        static DISTRIBUTION: LazyLock<WeightedIndex<u8>> =
            LazyLock::new(|| WeightedIndex::new(CohortId::WEIGHTS).unwrap());

        let n = DISTRIBUTION.sample(rng) + 1;
        debug_assert!(n > 0);
        debug_assert!(n <= CohortId::WEIGHTS.len());
        // The or default is purely defensive.
        CohortId::new(n as u8).unwrap_or_default()
    }
}

impl Serialize for CohortId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.get().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CohortId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <u8>::deserialize(deserializer)
            .and_then(|n| Self::new(n).ok_or(D::Error::custom("invalid cohort id")))
    }
}

/// The default for existing players is cohort 1.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct ServerToken(pub NonZeroU64);
impl_wrapper_from_str!(ServerToken, NonZeroU64);

#[cfg(feature = "server")]
impl Distribution<ServerToken> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ServerToken {
        ServerToken(rng.gen())
    }
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, EnumIter, Serialize, Encode, Decode,
)]
pub enum GameId {
    Kiomet,
    Mk48,
    Netquel,
    /// A placeholder for games we haven't released yet.
    Redacted,
}

impl GameId {
    pub fn name(self) -> &'static str {
        match self {
            Self::Kiomet => "Kiomet",
            Self::Mk48 => "Mk48.io",
            Self::Netquel => "Netquel",
            Self::Redacted => "Redacted",
        }
    }

    pub fn domain(self) -> &'static str {
        match self {
            Self::Kiomet => "kiomet.com",
            Self::Mk48 => "mk48.io",
            Self::Netquel => "netquel.com",
            Self::Redacted => "REDACTED",
        }
    }

    pub fn iter_non_redacted() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter().filter(|g| *g != Self::Redacted)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct InvitationId(pub NonZeroU32);

impl InvitationId {
    #[cfg(feature = "server")]
    pub fn generate(server_number: Option<ServerNumber>) -> Self {
        let r: u32 = rand::thread_rng().gen::<NonZeroU32>().get();
        Self(
            NonZeroU32::new(
                ((server_number.map(|id| id.0.get()).unwrap_or(0) as u32) << 24)
                    | (r & ((1 << 24) - 1)),
            )
            .unwrap(),
        )
    }

    pub fn server_number(self) -> Option<ServerNumber> {
        NonZeroU8::new((self.0.get() >> 24) as u8).map(ServerNumber)
    }
}

impl_wrapper_from_str!(InvitationId, NonZeroU32);

// The LanguageId enum may be extended with additional languages, such as:
// Bengali,
// Hindi,
// Indonesian,
// Korean,
// Portuguese,
// TraditionalChinese,

/// In order that they should be presented in a language picker.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, EnumIter, EnumString, Display)]
pub enum LanguageId {
    #[default]
    #[strum(serialize = "en")]
    English,
    #[strum(serialize = "es")]
    Spanish,
    #[strum(serialize = "fr")]
    French,
    #[strum(serialize = "de")]
    German,
    #[strum(serialize = "it")]
    Italian,
    #[strum(serialize = "ru")]
    Russian,
    #[strum(serialize = "ar")]
    Arabic,
    #[strum(serialize = "hi")]
    Hindi,
    #[strum(serialize = "zh")]
    SimplifiedChinese,
    #[strum(serialize = "ja")]
    Japanese,
    #[strum(serialize = "vi")]
    Vietnamese,
    #[strum(serialize = "xx-bork")]
    Bork,
}

impl LanguageId {
    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}

/// `PeriodId` is used by `LeaderboardScoreDto`.
#[derive(
    Clone,
    Copy,
    Debug,
    Hash,
    Eq,
    PartialEq,
    Deserialize,
    EnumIter,
    Serialize,
    VariantCount,
    Encode,
    Decode,
)]
pub enum PeriodId {
    #[serde(rename = "all")]
    AllTime = 0,
    #[serde(rename = "day")]
    Daily = 1,
    #[serde(rename = "week")]
    Weekly = 2,
}

impl From<usize> for PeriodId {
    fn from(i: usize) -> Self {
        match i {
            0 => Self::AllTime,
            1 => Self::Daily,
            2 => Self::Weekly,
            _ => panic!("invalid index"),
        }
    }
}

impl PeriodId {
    pub fn iter() -> impl Iterator<Item = Self> {
        <Self as IntoEnumIterator>::iter()
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct PlayerId(pub NonZeroU32);
impl_wrapper_from_str!(PlayerId, NonZeroU32);

impl PlayerId {
    pub const DAY_BITS: u32 = 10;
    pub const RANDOM_BITS: u32 = 32 - Self::DAY_BITS;
    pub const RANDOM_MASK: u32 = (1 << Self::RANDOM_BITS) - 1;
    pub const DAY_MASK: u32 = !Self::RANDOM_MASK;

    /// The player ID of the solo player in offline single player mode.
    /// TODO: This is not currently used.
    pub const SOLO_OFFLINE: Self = Self(NonZeroU32::new(1).unwrap());

    /// Gets the bot number associated with this id, or [`None`] if the id is not a bot.
    pub fn bot_number(self) -> Option<usize> {
        self.is_bot().then_some(self.0.get() as usize - 2)
    }

    /// Gets the nth id associated with bots.
    pub fn nth_bot(n: usize) -> Option<Self> {
        NonZeroU32::new(u32::try_from(n).ok()? + 2)
            .map(Self)
            .filter(|id| id.is_bot())
    }

    /// Returns true if the id is reserved for bots.
    pub const fn is_bot(self) -> bool {
        let n = self.0.get();
        n & Self::DAY_MASK == 0 && !self.is_solo()
    }

    /// Returns true if the id is reserved for offline solo play.
    pub const fn is_solo(self) -> bool {
        self.0.get() == 1
    }
}

/// Mirrors <https://github.com/finnbear/db_ip>: `Region`.
/// TODO use strum to implement FromStr
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    Hash,
    PartialEq,
    EnumIter,
    Serialize,
    Encode,
    Decode,
)]
pub enum RegionId {
    Africa,
    Asia,
    Europe,
    #[default]
    NorthAmerica,
    Oceania,
    SouthAmerica,
}

impl RegionId {
    /// Returns a relative distance to another region.
    /// It is not necessarily transitive.
    pub fn distance(self, other: Self) -> u8 {
        match self {
            Self::Africa => match other {
                Self::Africa => 0,
                Self::Asia => 2,
                Self::Europe => 1,
                Self::NorthAmerica => 2,
                Self::Oceania => 3,
                Self::SouthAmerica => 3,
            },
            Self::Asia => match other {
                Self::Africa => 2,
                Self::Asia => 0,
                Self::Europe => 2,
                Self::NorthAmerica => 2,
                Self::Oceania => 1,
                Self::SouthAmerica => 3,
            },
            Self::Europe => match other {
                Self::Africa => 1,
                Self::Asia => 2,
                Self::Europe => 0,
                Self::NorthAmerica => 2,
                Self::Oceania => 3,
                Self::SouthAmerica => 3,
            },
            Self::NorthAmerica => match other {
                Self::Africa => 3,
                Self::Asia => 3,
                Self::Europe => 2,
                Self::NorthAmerica => 0,
                Self::Oceania => 2,
                Self::SouthAmerica => 1,
            },
            Self::Oceania => match other {
                Self::Africa => 3,
                Self::Asia => 1,
                Self::Europe => 2,
                Self::NorthAmerica => 2,
                Self::Oceania => 0,
                Self::SouthAmerica => 3,
            },
            Self::SouthAmerica => match other {
                Self::Africa => 3,
                Self::Asia => 2,
                Self::Europe => 2,
                Self::NorthAmerica => 1,
                Self::Oceania => 2,
                Self::SouthAmerica => 0,
            },
        }
    }

    pub fn as_human_readable_str(self) -> &'static str {
        match self {
            Self::Africa => "Africa",
            Self::Asia => "Asia",
            Self::Europe => "Europe",
            Self::NorthAmerica => "North America",
            Self::Oceania => "Oceania",
            Self::SouthAmerica => "South America",
        }
    }

    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}

/// Wasn't a valid region string.
#[derive(Debug)]
pub struct InvalidRegionId;

impl Display for InvalidRegionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid region id string")
    }
}

impl FromStr for RegionId {
    type Err = InvalidRegionId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "af" | "africa" => Self::Africa,
            "as" | "asia" => Self::Asia,
            "eu" | "europe" => Self::Europe,
            "na" | "northamerica" => Self::NorthAmerica,
            "oc" | "oceania" => Self::Oceania,
            "sa" | "southamerica" => Self::SouthAmerica,
            _ => return Err(InvalidRegionId),
        })
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    EnumIter,
    EnumString,
    Display,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum ServerKind {
    /// #.domain.com
    Cloud,
    /// localhost
    Local,
}

impl ServerKind {
    pub fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl ServerNumber {
    pub fn new(val: u8) -> Option<Self> {
        NonZeroU8::new(val).map(Self)
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct ServerNumber(pub NonZeroU8);
impl_wrapper_from_str!(ServerNumber, NonZeroU8);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct ServerId {
    pub kind: ServerKind,
    pub number: ServerNumber,
}

impl ServerId {
    pub fn hostname(self, game_id: GameId) -> String {
        match self.kind {
            ServerKind::Cloud => format!("{}.{}", self.number, game_id.domain()),
            ServerKind::Local => format!("localhost:8443"),
        }
    }

    pub fn cloud_server_number(self) -> Option<ServerNumber> {
        if self.kind.is_cloud() {
            Some(self.number)
        } else {
            None
        }
    }
}

mod server_id_serde {
    use super::*;
    use crate::{ServerId, StrVisitor};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    #[derive(Serialize, Deserialize)]
    struct ServerIdPlaceholder {
        kind: ServerKind,
        number: ServerNumber,
    }

    impl Serialize for ServerId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if serializer.is_human_readable() {
                serializer.collect_str(self)
            } else {
                ServerIdPlaceholder {
                    kind: self.kind,
                    number: self.number,
                }
                .serialize(serializer)
            }
        }
    }

    impl<'de> Deserialize<'de> for ServerId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                deserializer.deserialize_str(StrVisitor).and_then(|s| {
                    Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid server id"))
                })
            } else {
                ServerIdPlaceholder::deserialize(deserializer).map(|placeholder| ServerId {
                    kind: placeholder.kind,
                    number: placeholder.number,
                })
            }
        }
    }
}

impl Display for ServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}/{}", self.kind, self.number)
    }
}

impl Debug for ServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

impl std::str::FromStr for ServerId {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, number) = s.split_once('/').ok_or(())?;
        Ok(Self {
            kind: ServerKind::from_str(kind).map_err(|_| ())?,
            number: ServerNumber::from_str(number).map_err(|_| ())?,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct SessionId(pub NonZeroU64);
impl_wrapper_from_str!(SessionId, NonZeroU64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct SessionToken(pub NonZeroU64);
impl_wrapper_from_str!(SessionToken, NonZeroU64);

/// A key like "default.js" or "1.foo.js" where "foo" is a referrer (referrer cannot contain ".").
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub struct SnippetId {
    pub cohort_id: Option<CohortId>,
    pub referrer: Option<Referrer>,
}

impl FromStr for SnippetId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.rsplit('.');
        let ext = iter.next().ok_or("missing extension")?;
        if !ext.eq_ignore_ascii_case("js") {
            return Err("invalid extension");
        }

        let referrer = iter.next().ok_or("missing referrer")?;
        let referrer = (!referrer.eq_ignore_ascii_case("default"))
            .then(|| Referrer::new(referrer).ok_or("invalid referrer"))
            .transpose()?;

        let cohort_id = iter
            .next()
            .map(|s| {
                s.parse::<u8>()
                    .ok()
                    .and_then(CohortId::new)
                    .ok_or("invalid cohort_id")
            })
            .transpose()?;

        if iter.next().is_some() {
            return Err("too many .");
        }

        Ok(SnippetId {
            cohort_id,
            referrer,
        })
    }
}

impl fmt::Display for SnippetId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let referrer = self.referrer.as_deref().unwrap_or("default");
        let ext = "js";

        if let Some(CohortId(cohort_id)) = self.cohort_id {
            write!(f, "{cohort_id}.{referrer}.{ext}")
        } else {
            write!(f, "{referrer}.{ext}")
        }
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct TeamId(pub NonZeroU32);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, EnumIter, Serialize, Deserialize, Encode, Decode,
)]
pub enum UserAgentId {
    ChromeOS,
    Desktop,
    DesktopChrome,
    DesktopFirefox,
    DesktopSafari,
    Mobile,
    Spider,
    Tablet,
}

impl UserAgentId {
    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}

// This will supersede [`PlayerId`] for persistent storage.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct UserId(pub NonZeroU64);
impl_wrapper_from_str!(UserId, NonZeroU64);

#[cfg(test)]
mod tests {
    use crate::id::PlayerId;
    use crate::{CohortId, Referrer, SnippetId};
    use std::str::FromStr;

    #[test]
    #[cfg(feature = "server")]
    fn invitation_id() {
        use crate::id::{InvitationId, ServerId};
        use std::num::NonZeroU8;

        for i in 1..=u8::MAX {
            let sid = ServerId(NonZeroU8::new(i).unwrap());
            let iid = InvitationId::generate(Some(sid));
            assert_eq!(iid.server_id(), Some(sid));
        }
    }

    #[test]
    fn snippet_id() {
        fn test(s: &str, id: Option<SnippetId>) {
            let id2 = SnippetId::from_str(s);
            if let Some(id) = id {
                assert_eq!(id2, Ok(id), "{}", s);
                assert_eq!(id2.unwrap().to_string(), s.to_ascii_lowercase(), "{}", s);
            } else {
                assert!(id2.is_err(), "{}", s)
            }
        }

        test("default.xyz", None);
        test("100.default.js", None);
        test(".default.js", None);
        test(".js", None);
        test("2..js", None);
        test(".1.foo.js", None);

        test("default.js", Some(Default::default()));
        test(
            "1.DEFAULT.js",
            Some(SnippetId {
                cohort_id: Some(CohortId::new(1).unwrap()),
                ..Default::default()
            }),
        );
        test(
            "softbear.js",
            Some(SnippetId {
                referrer: Some(Referrer::from_str("softbear").unwrap()),
                ..Default::default()
            }),
        );
        test(
            "3.softbear.js",
            Some(SnippetId {
                cohort_id: Some(CohortId::new(3).unwrap()),
                referrer: Some(Referrer::from_str("softbear").unwrap()),
            }),
        );
    }

    #[test]
    fn solo() {
        assert!(PlayerId::SOLO_OFFLINE.is_solo());
    }
}
