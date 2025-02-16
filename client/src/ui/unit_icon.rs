// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::KiometPhrases;
use crate::KiometRoute;
use common::unit::Unit;
use kodiak_client::use_translator;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Html, MouseEvent, Properties};
use yew_router::hooks::use_navigator;

#[derive(PartialEq, Properties)]
pub struct UnitIconProps {
    pub unit: Unit,
    #[prop_or("1.5rem".into())]
    pub size: AttrValue,
    #[prop_or(Color::Blue)]
    pub fill: Color,
}

#[styled_component(UnitIcon)]
pub fn unit_icon(props: &UnitIconProps) -> Html {
    let unit_css = css!(
        r#"
        user-drag: none;
        -webkit-user-drag: none;
        "#
    );

    let unit_unselected_css = css!(
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
        let unit = props.unit;
        let navigator = use_navigator().unwrap();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&KiometRoute::units_specific(unit));
        })
    };
    let title = t.unit_label(props.unit);
    let alt = title.clone();

    html! {
        <img
            src={AttrValue::Static(SvgCache::get(PathId::Unit(props.unit), props.fill))}
            {onclick}
            class={classes!(unit_css, unit_unselected_css.clone())}
            style={format!("width: {}; height: {}; vertical-align: bottom;", props.size, props.size)}
            {alt}
            {title}
        />
    }
}
