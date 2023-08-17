// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#[allow(clippy::module_inception)] // TODO
pub mod dialog;
#[cfg(feature = "health")]
pub mod health_dialog;
pub mod licensing_dialog;
pub mod privacy_dialog;
pub mod profile_dialog;
pub mod settings_dialog;
pub mod store_dialog;
pub mod terms_dialog;
