// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::component::positioner::Position;
use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, use_state, Callback, Children, Html, Properties};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct SectionProps {
    pub children: Children,
    #[prop_or(true)]
    pub closable: bool,
    #[prop_or(None)]
    pub id: Option<AttrValue>,
    pub name: AttrValue,
    pub rich_name: Option<Html>,
    pub position: Option<Position>,
    pub style: Option<AttrValue>,
    #[prop_or(true)]
    pub open: bool,
    /// If [`Some`], open is reactive.
    #[prop_or(None)]
    pub on_open_changed: Option<Callback<bool>>,
    #[prop_or_default]
    pub left_arrow: SectionArrow,
    #[prop_or_default]
    pub right_arrow: SectionArrow,
}

#[derive(Default, PartialEq)]
pub enum SectionArrow {
    /// Visible and clickable.
    Active(Callback<MouseEvent>),
    /// May become active; reserve space to avoid layout shift.
    Reserved,
    /// Will never become active.
    #[default]
    None,
}

impl SectionArrow {
    pub fn always(callback: Callback<MouseEvent>) -> Self {
        Self::Active(callback)
    }

    pub fn sometimes(option: Option<Callback<MouseEvent>>) -> Self {
        option.map(Self::Active).unwrap_or(Self::Reserved)
    }

    fn unpack(&self, open: bool) -> Option<Option<Callback<MouseEvent>>> {
        if open {
            match self {
                Self::Active(callback) => Some(Some(callback.reform(|e: MouseEvent| {
                    e.stop_propagation();
                    e
                }))),
                Self::Reserved => Some(None),
                Self::None => None,
            }
        } else {
            None
        }
    }
}

#[styled_component(Section)]
pub fn section(props: &SectionProps) -> Html {
    let open_state = use_state(|| props.open);
    let open = if props.on_open_changed.is_some() {
        props.open
    } else {
        *open_state
    };

    let onclick = props.closable.then(|| {
        if let Some(on_open_changed) = props.on_open_changed.clone() {
            Callback::from(move |_| {
                on_open_changed.emit(!open);
            })
        } else {
            Callback::from(move |_| open_state.set(!open))
        }
    });

    let h2_css_class = css!(
        r#"
        color: white;
        font-weight: bold;
        margin: 0;
        user-select: none;
    "#
    );

    let h2_clickable_css_class = css!(
        r#"
        cursor: pointer;
        transition: filter 0.1s;

        :hover {
            filter: opacity(0.85);
        }
        "#
    );

    /*
       @media (min-width: 1000px) {
           h2 {
               white-space: nowrap;
           }
       }
    */

    let span_css_class = css!(
        r#"
        .disable {
            opacity: 0;
        }
    "#
    );

    let reserved_style = css!(
        r#"
        visibility: hidden;
        "#
    );

    #[cfg(feature = "high_contrast_setting")]
    let high_contrast_style = css!(
        r#"
        background-color: #00000035;
        padding: 0.5rem;
        border-radius: 0.5rem;
        "#
    );
    #[cfg(feature = "high_contrast_setting")]
    let high_contrast = crate::frontend::use_ctw().setting_cache.high_contrast;
    #[cfg(feature = "high_contrast_setting")]
    let high_contrast_class = high_contrast.then_some(high_contrast_style);

    #[cfg(not(feature = "high_contrast_setting"))]
    let high_contrast_class = Option::<&'static str>::None;

    const ICON_WIDTH: &str = "1.5rem";
    const ICON_HEIGHT: &str = "1.2rem";

    let mut style = String::new();
    if let Some(s) = &props.style {
        style.push_str(s.as_str());
    }
    if let Some(position) = props.position {
        use std::fmt::Write;
        write!(&mut style, "{}", position).unwrap();
    }

    html! {
        <>
            <div id={props.id.clone()} {style} class={high_contrast_class}>
                <h2
                    class={classes!(h2_css_class, onclick.is_some().then_some(h2_clickable_css_class))}
                    {onclick}
                    >
                    if let Some(maybe_callback) = props.left_arrow.unpack(open) {
                        <span class={classes!(span_css_class.clone(), maybe_callback.is_none().then(|| reserved_style.clone()))} onclick={maybe_callback}>
                            <Icon icon_id={IconId::FontAwesomeSolidSquareCaretLeft} width={ICON_WIDTH.to_string()} height={ICON_HEIGHT.to_string()}/>
                        </span>
                    }
                    if let Some(rich_name) = props.rich_name.as_ref() {
                        {rich_name.clone()}
                    } else {
                        {&props.name}
                    }
                    if let Some(maybe_callback) = props.right_arrow.unpack(open) {
                        <span class={classes!(span_css_class, maybe_callback.is_none().then_some(reserved_style))} onclick={maybe_callback}>
                            <Icon icon_id={IconId::FontAwesomeSolidSquareCaretRight} width={ICON_WIDTH.to_string()} height={ICON_HEIGHT.to_string()}/>
                        </span>
                    }
                </h2>
                if open {
                    {props.children.clone()}
                }
            </div>
        </>
    }
}
