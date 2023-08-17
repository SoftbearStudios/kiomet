// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    component::{account_menu::AccountMenu, positioner::Position},
    dialog::dialog::Dialog,
    frontend::use_ctw,
};
use std::borrow::Cow;
use yew::{function_component, html, Html};

#[function_component(ProfileDialog)]
pub fn profile_dialog() -> Html {
    let ctw = use_ctw();

    html! {
        <Dialog title={"Profile"}>
            if ctw.setting_cache.nick_name.is_some() {
                <iframe
                    style={"border: 0; width: calc(100% - 0.5em); height: calc(100% - 1em);"}
                    src={
                        format!(
                            "https://softbear.com/profile/?gameId={:?}&hideNav{}",
                            ctw.game_id,
                            ctw.setting_cache.session_id
                                .map(|s| Cow::Owned(format!("&sessionId={}", s.0)))
                                .unwrap_or(Cow::Borrowed(""))
                        )
                    }
                />
            } else {
                <AccountMenu position={Position::Center}/>
            }
        </Dialog>
    }
}
