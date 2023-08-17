// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{dialog::dialog::Dialog, frontend::use_ctw};
use std::borrow::Cow;
use yew::{function_component, html, Html};

#[function_component(StoreDialog)]
pub fn store_dialog() -> Html {
    let ctw = use_ctw();
    let session_id = ctw.setting_cache.session_id;

    html! {
        <Dialog title={"Store"}>
            if ctw.setting_cache.store_enabled {
                <iframe
                    style={"border: 0; width: calc(100% - 0.5em); height: calc(100% - 1em);"}
                    src={
                        format!(
                            "https://softbear.com/store/?gameId={:?}&hideNav{}",
                            ctw.game_id,
                            session_id
                                .map(|s| Cow::Owned(format!("&sessionId={}", s.0)))
                                .unwrap_or(Cow::Borrowed(""))
                        )
                    }
                />
            }
        </Dialog>
    }
}
