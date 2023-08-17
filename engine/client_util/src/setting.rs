// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::browser_storage::BrowserStorages;
use core_protocol::id::{CohortId, LanguageId, ServerNumber, SessionId};
use core_protocol::name::PlayerAlias;
use core_protocol::{PlayerId, SessionToken, Token, UnixTime};
pub use engine_macros::Settings;
use strum_macros::IntoStaticStr;

/// Settings backed by local storage.
pub trait Settings: Sized {
    /// Loads all settings from local storage.
    fn load(l: &BrowserStorages, default: Self) -> Self;

    /// Renders GUI widgets for certain settings.
    fn display(
        &self,
        checkbox: impl FnMut(
            SettingCategory,
            &'static str,
            bool,
            fn(&mut Self, bool, &mut BrowserStorages),
        ),
        dropdown: impl FnMut(
            SettingCategory,
            &'static str,
            &'static str,
            fn(usize) -> Option<(&'static str, &'static str)>,
            fn(&mut Self, &str, &mut BrowserStorages),
        ),
    );
}

#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Debug, Default, IntoStaticStr)]
pub enum SettingCategory {
    #[default]
    General,
    Audio,
    Graphics,
}

// Useful if you don't want settings.
impl Settings for () {
    fn load(_: &BrowserStorages, _: Self) -> Self {}
    fn display(
        &self,
        _: impl FnMut(SettingCategory, &'static str, bool, fn(&mut Self, bool, &mut BrowserStorages)),
        _: impl FnMut(
            SettingCategory,
            &'static str,
            &'static str,
            fn(usize) -> Option<(&'static str, &'static str)>,
            fn(&mut Self, &str, &mut BrowserStorages),
        ),
    ) {
    }
}

/// Settings of the infrastructure, common to all games.
#[derive(Clone, PartialEq, Settings)]
pub struct CommonSettings {
    /// Alias preference.
    #[setting(optional)]
    pub alias: Option<PlayerAlias>,
    /// Language preference.
    pub language: LanguageId,
    /// Volume preference (0 to 1).
    #[setting(range = "0.0..1.0", finite)]
    pub volume: f32,
    /// Music preference.
    #[setting(checkbox = "Audio/Music")]
    pub music: bool,
    /// Last [`CohortId`].
    #[setting(optional)]
    pub cohort_id: Option<CohortId>,
    /// Last-used/chosen [`ServerId`].
    #[setting(optional, volatile)]
    pub server_number: Option<ServerNumber>,
    /// Last-used [`PlayerId`].
    #[setting(optional)]
    pub player_id: Option<PlayerId>,
    /// Last-used [`Token`].
    #[setting(optional)]
    pub token: Option<Token>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub date_created: Option<UnixTime>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub session_id: Option<SessionId>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub session_token: Option<SessionToken>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub nick_name: Option<String>,
    #[setting(volatile)]
    pub store_enabled: bool,
    /// Pending chat message.
    #[setting(volatile)]
    pub chat_message: String,
    /// Whether to add a contrasting border behind UI elements.
    #[setting(checkbox = "High contrast")]
    #[cfg(feature = "high_contrast_setting")]
    pub high_contrast: bool,
    /// Whether team menu is open.
    #[setting(volatile)]
    pub team_dialog_shown: bool,
    /// Whether chat menu is open.
    #[setting(checkbox = "Chat")]
    pub chat_dialog_shown: bool,
    /// Whether leaderboard menu is open.
    #[setting(volatile)]
    pub leaderboard_dialog_shown: bool,
}

impl Default for CommonSettings {
    fn default() -> Self {
        Self {
            alias: None,
            language: LanguageId::default(),
            volume: 0.5,
            music: true,
            cohort_id: None,
            server_number: None,
            player_id: None,
            token: None,
            session_id: None,
            session_token: None,
            nick_name: None,
            store_enabled: false,
            date_created: None,
            chat_message: String::new(),
            #[cfg(feature = "high_contrast_setting")]
            high_contrast: false,
            team_dialog_shown: true,
            chat_dialog_shown: true,
            leaderboard_dialog_shown: true,
        }
    }
}
