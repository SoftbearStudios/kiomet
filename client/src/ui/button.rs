// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use stylist::css;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Children, Classes, Html, MouseEvent, Properties};

#[derive(PartialEq, Properties)]
pub struct ButtonProps {
    pub children: Children,
    #[prop_or(None)]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub progress: f32,
    #[prop_or(None)]
    pub title: Option<AttrValue>,
    #[prop_or("background: #888888;".into())]
    pub style: AttrValue,
    #[prop_or(None)]
    pub class: Option<Classes>,
    #[prop_or(None)]
    pub content_class: Option<Classes>,
}

#[styled_component(Button)]
pub fn button(props: &ButtonProps) -> Html {
    // z-index: 0 create a new stacking context.
    let button_css = css!(
        r#"
        position: relative;
        display: inline-block;
        color: #fff;
        transition: filter 0.2s ease;
        border-radius: 0.5rem;
        overflow: hidden;
        user-select: none;
        text-align: center;
        line-height: 1.25rem;
        height: min-content;
        z-index: 0;
        "#
    );

    let disabled_css = css!(
        r#"
        filter: brightness(0.7) !important;
        cursor: initial !important;
        "#
    );

    let onclick_css = css!(
        r#"
        cursor: pointer;

        :hover {
            filter: brightness(1.1);
        }

        :active {
            filter: brightness(1.2);
        }
        "#
    );

    let contents_css = css!(
        r#"
        position: relative;
        padding-left: 0.3rem;
        padding-right: 0.3rem;
        padding-top: 0.1rem;
        padding-bottom: 0.1rem;
        z-index: 2;
        "#
    );

    let progress_css = css!(
        r#"
        position: absolute;
        left: 0;
        right: 0;
        bottom: 0;
        top: 0;
        background: inherit;
        filter: brightness(1.1);
        z-index: 1;
        "#
    );

    html! {
        <div
            onclick={props.onclick.as_ref().filter(|_| !props.disabled).cloned()}
            title={props.title.clone()}
            style={props.style.clone()}
            class={classes!(button_css, props.disabled.then_some(disabled_css), props.onclick.is_some().then_some(onclick_css), props.class.clone())}
        >
            <div class={classes!(contents_css, props.content_class.clone())}>{props.children.clone()}</div>
            <div style={format!("width: {}%;", props.progress * 100.0)} class={progress_css}/>
        </div>
    }
}
