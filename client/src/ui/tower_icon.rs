// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::KiometPhrases;
use crate::KiometRoute;
use common::tower::TowerType;
use kodiak_client::use_translator;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Html, MouseEvent, Properties};
use yew_router::hooks::use_navigator;

#[derive(PartialEq, Properties)]
pub struct TowerIconProps {
    pub tower_type: TowerType,
    #[prop_or("1.5rem".into())]
    pub size: AttrValue,
    /// Implies filled.
    #[prop_or(false)]
    pub selected: bool,
    #[prop_or(true)]
    pub filled: bool,
    #[prop_or(Color::Blue)]
    pub fill: Color,
}

#[styled_component(TowerIcon)]
pub fn tower_icon(props: &TowerIconProps) -> Html {
    let tower_css = css!(
        r#"
        user-drag: none;
        -webkit-user-drag: none;
        "#
    );

    let tower_unselected_css = css!(
        r#"
        cursor: pointer;
        transition: opacity 0.2s;

        :hover {
            opacity: 0.8;
        }
        "#
    );

    let t = use_translator();
    let onclick = {
        let tower_type = props.tower_type;
        let navigator = use_navigator().unwrap();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&KiometRoute::towers_specific(tower_type));
        })
    };
    let title = t.tower_type_label(props.tower_type);
    let alt = title.clone();

    html! {
        <img
            src={AttrValue::Static(SvgCache::get(PathId::Tower(props.tower_type), props.fill))}
            {onclick}
            class={classes!(tower_css, tower_unselected_css.clone())}
            style={format!("width: {}; height: {}; vertical-align: bottom;", props.size, props.size)}
            {alt}
            {title}
        />
    }
}
