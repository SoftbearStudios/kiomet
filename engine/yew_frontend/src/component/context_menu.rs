// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::frontend::use_set_context_menu_callback;
use crate::WindowEventListener;
use gloo::timers::callback::Timeout;
use js_hooks::window;
use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::{
    function_component, hook, html, html::IntoPropValue, use_effect_with_deps, use_state, Callback,
    Children, Html, Properties,
};

#[derive(Clone, PartialEq, Properties)]
pub struct ContextMenuProps {
    pub position: ContextMenuPosition,
    pub children: Children,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ContextMenuPosition(String);

impl IntoPropValue<ContextMenuPosition> for &MouseEvent {
    fn into_prop_value(self) -> ContextMenuPosition {
        ContextMenuPosition(context_menu_position(self))
    }
}

/// Convert click location of mouse event into position style.
fn context_menu_position(event: &MouseEvent) -> String {
    let window = window();
    let x = event.x();
    let y = event.y();
    let window_width = window.inner_width().unwrap().as_f64().unwrap() as i32;
    let (horizontal_basis, horizontal_coordinate) = if x > window_width / 2 {
        ("right", window_width - x)
    } else {
        ("left", x)
    };
    let window_height = window.inner_height().unwrap().as_f64().unwrap() as i32;
    let (vertical_basis, vertical_coordinate) = if y > window_height / 2 {
        ("bottom", window_height - y)
    } else {
        ("top", y)
    };
    format!(
        "position: absolute; {horizontal_basis}: {horizontal_coordinate}px; {vertical_basis}: {vertical_coordinate}px;"
    )
}

#[function_component(ContextMenu)]
pub fn context_menu(props: &ContextMenuProps) -> Html {
    let style = format!("background-color: #444444aa; min-width: 100px; position: absolute; display: flex; flex-direction: column; {}", props.position.0);

    // Provide for closing the menu by rightclicking elsewhere.
    let set_context_menu_callback = use_set_context_menu_callback();
    let set_context_menu_callback_clone = set_context_menu_callback.clone();
    let activity = use_state(|| false);

    let onmousemove = {
        let activity = activity.clone();
        Callback::from(move |_| {
            activity.set(!*activity);
        })
    };

    use_effect_with_deps(
        |_| {
            let listener = WindowEventListener::new_body(
                "contextmenu",
                move |e: &MouseEvent| {
                    e.prevent_default();
                    e.stop_propagation();
                    set_context_menu_callback.emit(None)
                },
                true,
            );
            let timeout = Timeout::new(10000, move || {
                set_context_menu_callback_clone.emit(None);
            });
            || drop((listener, timeout))
        },
        (props.position.clone(), *activity),
    );

    html! {
        <div {style} {onmousemove}>
            {props.children.clone()}
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct ContextMenuButtonProps {
    pub children: Children,
    pub onclick: Option<Callback<MouseEvent>>,
    /// Close on click.
    #[prop_or(true)]
    pub close: bool,
}

#[styled_component(ContextMenuButton)]
pub fn context_menu_button(props: &ContextMenuButtonProps) -> Html {
    let class = css!(
        r#"
		color: white;
		background-color: #444444aa;
		border: 0;
		border-radius: 0;
		outline: 0;
		margin: 0;
		padding: 5px;

        :hover {
            filter: brightness(1.1);
        }

        :hover:active {
            filter: brightness(1.05);
        }
    "#
    );

    let set_context_menu_callback = use_set_context_menu_callback();
    let close = props.close;
    let onclick = props.onclick.clone().map(move |onclick| {
        onclick.reform(move |e| {
            if close {
                // Close the menu when an option is clicked.
                set_context_menu_callback.emit(None);
            }
            e
        })
    });

    html! {
        <button {onclick} {class}>
            {props.children.clone()}
        </button>
    }
}

/// Returns oncontextmenu callback that dismisses existing context menu.
#[hook]
pub fn use_dismiss_context_menu() -> Callback<MouseEvent> {
    let set_context_menu_callback = use_set_context_menu_callback();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        e.stop_propagation();
        set_context_menu_callback.emit(None)
    })
}
