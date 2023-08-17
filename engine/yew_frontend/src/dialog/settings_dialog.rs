// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeMap, str::FromStr};

use crate::{
    dialog::dialog::Dialog,
    frontend::{use_core_state, use_ctw, use_gctw},
};
use client_util::{
    browser_storage::BrowserStorages,
    game_client::GameClient,
    setting::{SettingCategory, Settings},
};
use core_protocol::{ServerDto, ServerNumber};
use stylist::{yew::styled_component, StyleSource};
use web_sys::{HtmlSelectElement, InputEvent};
use yew::{html, html_nested, Callback, Html, TargetCast};

#[styled_component(SettingsDialog)]
pub fn settings_dialog<G: GameClient>() -> Html {
    let ctw = use_ctw();
    let gctw = use_gctw::<G>();

    let select_style = css! {
        r#"
        border-radius: 0.25rem;
        box-sizing: border-box;
        cursor: pointer;
        font-size: 1em;
        font-weight: bold;
        outline: 0;
        padding: 0.7rem;
        pointer-events: all;
        white-space: nowrap;
        margin-top: 0.25rem;
        margin-bottom: 0.25rem;
        border: 0;
        color: white;
	    background-color: #0075ff;
	    display: block;
        "#
    };

    fn checkbox<S: 'static>(
        label: &'static str,
        checked: bool,
        callback: fn(&mut S, bool, &mut BrowserStorages),
        change_settings: &Callback<Box<dyn FnOnce(&mut S, &mut BrowserStorages)>>,
    ) -> Html {
        let oninput = change_settings.reform(move |_| {
            Box::new(
                move |settings: &mut S, browser_storages: &mut BrowserStorages| {
                    callback(settings, !checked, browser_storages);
                },
            )
        });

        html! {
            <label
                style="display: block; user-select: none; margin-bottom: 0.4em;"
            >
                <input type="checkbox" {checked} {oninput}/>
                {label}
            </label>
        }
    }

    fn dropdown<S: 'static>(
        _label: &'static str,
        selected: &'static str,
        options: fn(usize) -> Option<(&'static str, &'static str)>,
        callback: fn(&mut S, &str, &mut BrowserStorages),
        change_settings: &Callback<Box<dyn FnOnce(&mut S, &mut BrowserStorages)>>,
        style: &StyleSource,
    ) -> Html {
        let oninput = change_settings.reform(move |event: InputEvent| {
            let string = event.target_unchecked_into::<HtmlSelectElement>().value();
            Box::new(
                move |settings: &mut S, browser_storages: &mut BrowserStorages| {
                    callback(settings, &string, browser_storages);
                },
            )
        });

        let mut n = 0;

        html! {
            <select {oninput} class={style.clone()}>
                {std::iter::from_fn(move || {
                    let ret = options(n);
                    n += 1;
                    ret
                }).map(|(value, message)| html_nested!(
                    <option {value} selected={value == selected}>{message}</option>
                )).collect::<Html>()}
            </select>
        }
    }

    let core_state = use_core_state();
    let selected_server_number = ctw.setting_cache.server_number;
    let on_select_server_number = {
        ctw.set_server_number_callback
            .reform(move |event: InputEvent| {
                let value = event.target_unchecked_into::<HtmlSelectElement>().value();
                ServerNumber::from_str(&value).ok()
            })
    };

    let categories =
        std::cell::RefCell::new(BTreeMap::<SettingCategory, BTreeMap<&'static str, Html>>::new());

    categories.borrow_mut().entry(SettingCategory::General).or_default().insert("Server", html!{
        <select
            oninput={on_select_server_number}
            class={select_style.clone()}
        >
            if selected_server_number.is_none() || core_state.servers.is_empty() {
                <option value="unknown" selected={true}>{"Unknown server"}</option>
            }
            {core_state.servers.values().map(|&ServerDto{server_number, region_id, player_count, ..}| {
                let region_str = region_id.as_human_readable_str();
                html_nested!{
                    <option value={server_number.0.to_string()} selected={selected_server_number == Some(server_number)}>
                        {format!("Server {server_number} - {region_str} ({player_count} players)")}
                    </option>
                }
            }).collect::<Html>()}
        </select>
    });
    gctw.settings_cache.display(
        |a, b, c, d| {
            categories
                .borrow_mut()
                .entry(a)
                .or_default()
                .insert(b, checkbox(b, c, d, &gctw.change_settings_callback));
        },
        |a, b, c, d, e| {
            categories.borrow_mut().entry(a).or_default().insert(
                b,
                dropdown(b, c, d, e, &gctw.change_settings_callback, &select_style),
            );
        },
    );
    ctw.setting_cache.display(
        |a, b, c, d| {
            categories
                .borrow_mut()
                .entry(a)
                .or_default()
                .insert(b, checkbox(b, c, d, &ctw.change_common_settings_callback));
        },
        |a, b, c, d, e| {
            categories.borrow_mut().entry(a).or_default().insert(
                b,
                dropdown(
                    b,
                    c,
                    d,
                    e,
                    &ctw.change_common_settings_callback,
                    &select_style,
                ),
            );
        },
    );

    html! {
        <Dialog title={"Settings"}>
            {categories.into_inner().into_iter().map(|(category, settings)| {
                let category: &'static str = category.into();
                html!{<>
                    <h3>{category}</h3>
                    {settings.into_values().collect::<Html>()}
                </>}
            }).collect::<Html>()}
        </Dialog>
    }
}
