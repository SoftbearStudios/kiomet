// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use stylist::yew::styled_component;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct CurtainProps {
    pub children: Children,
    #[prop_or(33)]
    pub opacity: u8,
    pub onclick: Option<Callback<MouseEvent>>,
}

#[styled_component(Curtain)]
pub fn curtain(props: &CurtainProps) -> Html {
    let curtain_style = css!(
        r#"
        bottom: 0;
        left: 0;
        position: absolute;
        right: 0;
        top: 0;
    "#
    );

    html! {
        <div onclick={props.onclick.clone()} class={curtain_style} style={format!("background-color: #000000{:02X};", props.opacity)}>
            {props.children.clone()}
        </div>
    }
}
