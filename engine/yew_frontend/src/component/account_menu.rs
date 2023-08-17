// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::component::context_menu::ContextMenu;
use crate::{
    component::positioner::{Position, Positioner},
    frontend::{use_change_common_settings_callback, use_client_request_callback, use_ctw},
    window::event_listener::WindowEventListener,
};
use client_util::{browser_storage::BrowserStorages, setting::CommonSettings};
use core_protocol::{ClientRequest, GameId, SessionId, SessionToken, UserId};
use js_hooks::window;
use serde::Deserialize;
use std::{borrow::Cow, num::NonZeroU64, str::FromStr};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    FormData, MessageEvent, MouseEvent, Request, RequestCredentials, RequestInit, RequestMode,
    Response,
};
use yew::{
    function_component, hook, html, use_effect_with_deps, use_state_eq, Callback, Html, Properties,
    UseStateHandle,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    session_id: SessionId,
    session_token: SessionToken,
    nick_name: Option<String>,
    #[serde(default)]
    store_enabled: bool,
}

#[derive(PartialEq, Properties)]
pub struct AccountMenuProps {
    pub position: Position,
}

#[function_component(AccountMenu)]
pub fn account_menu(props: &AccountMenuProps) -> Html {
    let ctw: crate::frontend::Ctw = use_ctw();
    let previous_session_id = ctw.setting_cache.session_id;

    let client_request_callback = use_client_request_callback();
    let change_common_settings = use_change_common_settings_callback();
    let set_login = set_login(client_request_callback, change_common_settings.clone());

    {
        let set_login = set_login.clone();
        use_effect_with_deps(
            move |_| {
                let listener = WindowEventListener::new(
                    "message",
                    move |e: &MessageEvent| {
                        js_hooks::console_log!("msg: {:?}", e.data());

                        if !e.data().is_object() {
                            return;
                        }

                        let pmcsrf = js_sys::Reflect::get(&e.data(), &JsValue::from_str("pmcsrf"))
                            .ok()
                            .and_then(|v| v.as_string())
                            .and_then(|s| u64::from_str(&s).ok());

                        let session_id =
                            js_sys::Reflect::get(&e.data(), &JsValue::from_str("sessionId"))
                                .ok()
                                .and_then(|v| v.as_string())
                                .and_then(|s| u64::from_str(&s).ok());

                        let (pmcsrf, session_id) = if let Some(tuple) = pmcsrf.zip(session_id) {
                            tuple
                        } else {
                            return;
                        };

                        let set_login = set_login.clone();

                        let _ = future_to_promise(async move {
                            let url = format!(
                                "https://softbear.com/api/auth/session?sessionId={session_id}"
                            );

                            let body = FormData::new().unwrap();
                            body.append_with_str("pmcsrf", &pmcsrf.to_string()).unwrap();
                            body.append_with_str("sessionId", &session_id.to_string())
                                .unwrap();

                            let mut opts = RequestInit::new();
                            opts.method("POST");
                            opts.mode(RequestMode::Cors);
                            opts.credentials(RequestCredentials::Include);
                            opts.body(Some(&body));

                            let request = Request::new_with_str_and_init(&url, &opts)
                                .map_err(|e| format!("{:?}", e))?;

                            let window = web_sys::window().unwrap();
                            let resp_value = JsFuture::from(window.fetch_with_request(&request))
                                .await
                                .map_err(|e| format!("{:?}", e))?;
                            let resp: Response =
                                resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
                            if resp.ok() {
                                let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
                                let json: String = JsFuture::from(json_promise)
                                    .await
                                    .map_err(|e| format!("{:?}", e))?
                                    .as_string()
                                    .ok_or(String::from("JSON not string"))?;
                                let decoded: Login =
                                    serde_json::from_str(&json).map_err(|e| e.to_string())?;
                                set_login.emit(decoded);
                            }

                            Ok(JsValue::NULL)
                        });
                    },
                    false,
                );

                || drop(listener)
            },
            (),
        );
    }

    let onclick_login = previous_session_id.map(|session_id| {
        Callback::from(move |_: MouseEvent| {
            let endpoint = format!("https://softbear.com/api/discord/redirect?pmcsrf={session_id}");
            let features = "popup,left=200,top=200,width=700,height=700";
            let _ = window().open_with_url_and_target_and_features(&endpoint, "oauth2", features);
        })
    });

    let onclick_logout = previous_session_id.map(|_| {
        let set_login = set_login.clone();
        Callback::from(move |_: MouseEvent| {
            logout(set_login.clone());
        })
    });

    // Trick yew into not warning about bad practice.
    let href: &'static str = "javascript:void(0)";

    html! {
        <Positioner id={"account"} position={props.position}>
            if let Some(nick_name) = ctw.setting_cache.nick_name {
                <a {href} onclick={onclick_logout}>{"Sign out "}{nick_name}</a>
            } else if let Some(onclick_login) = onclick_login {
                <a {href} onclick={onclick_login}>{"Sign in with Discord"}</a>
            }
        </Positioner>
    }
}

#[derive(Debug, PartialEq, Deserialize)]
#[allow(unused)]
#[serde(rename_all = "camelCase")]
struct Profile {
    #[serde(default)]
    pub date_created: u64,
    #[serde(default)]
    pub follower_count: usize,
    #[serde(default)]
    pub is_follower: bool,
    #[serde(default)]
    pub is_following: bool,
    #[serde(default)]
    pub moderator: bool,
    #[serde(default)]
    pub nick_name: Option<String>,
}

#[hook]
fn use_profile(user_id: Option<UserId>) -> UseStateHandle<Option<Profile>> {
    let my_session_id = use_ctw().setting_cache.session_id;

    let profile = use_state_eq(|| None);
    {
        let profile = profile.clone();
        use_effect_with_deps(
            move |_| {
                let _ = future_to_promise(async move {
                    let url = if let Some(user_id) = user_id {
                        Cow::Owned(format!(
                            "https://softbear.com/api/social/profile.json?userId={}",
                            user_id.0
                        ))
                    } else {
                        Cow::Borrowed("https://softbear.com/api/social/profile.json")
                    };

                    let mut opts = RequestInit::new();
                    opts.method("GET");
                    opts.mode(RequestMode::Cors);
                    opts.credentials(RequestCredentials::Include);

                    let request = Request::new_with_str_and_init(&url, &opts)
                        .map_err(|e| format!("{:?}", e))?;

                    let window = web_sys::window().unwrap();
                    let resp_value = JsFuture::from(window.fetch_with_request(&request))
                        .await
                        .map_err(|e| format!("{:?}", e))?;
                    let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
                    if resp.ok() {
                        let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
                        let json: String = JsFuture::from(json_promise)
                            .await
                            .map_err(|e| format!("{:?}", e))?
                            .as_string()
                            .ok_or(String::from("JSON not string"))?;
                        let decoded: Profile =
                            serde_json::from_str(&json).map_err(|e| e.to_string())?;
                        profile.set(Some(decoded));
                    }
                    Ok(JsValue::NULL)
                });
            },
            my_session_id.filter(|_| user_id.is_none()),
        );
    }
    profile
}

pub fn logout(set_login: Callback<Login>) {
    let url = "https://softbear.com/api/auth/session";

    let _ = future_to_promise(async move {
        let mut opts = RequestInit::new();
        opts.method("DELETE");
        opts.mode(RequestMode::Cors);
        opts.credentials(RequestCredentials::Include);

        let request = Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{:?}", e))?;

        let window = web_sys::window().unwrap();
        let _resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("{:?}", e))?;
        set_login.emit(Login {
            session_id: SessionId(NonZeroU64::new(1).unwrap()),
            session_token: SessionToken(NonZeroU64::new(1).unwrap()),
            nick_name: None,
            store_enabled: false,
        });

        Ok(JsValue::NULL)
    });
}

#[allow(clippy::type_complexity)]
pub fn set_login(
    client_request_callback: Callback<ClientRequest>,
    change_common_settings_callback: Callback<
        Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>,
    >,
) -> Callback<Login> {
    change_common_settings_callback.reform(move |login: Login| {
        client_request_callback.emit(ClientRequest::Login(login.session_token));
        Box::new(move |common_settings, browser_storages| {
            common_settings.set_session_id(Some(login.session_id), browser_storages);
            common_settings.set_session_token(Some(login.session_token), browser_storages);
            common_settings.set_nick_name(login.nick_name, browser_storages);
            common_settings.set_store_enabled(login.store_enabled, browser_storages);
        })
    })
}

pub fn profile_factory(
    game_id: GameId,
    session_id: Option<SessionId>,
    set_context_menu_callback: Callback<Option<Html>>,
) -> impl Fn(UserId) -> Option<Callback<MouseEvent>> + Clone {
    move |user_id: UserId| -> Option<Callback<MouseEvent>> {
        let set_context_menu_callback = set_context_menu_callback.clone();
        session_id.map(move |session_id| Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();
            set_context_menu_callback.emit(Some(html! {
                <ContextMenu position={&e}>
                    <iframe
                        style="border: 0;"
                        src={format!("https://softbear.com/brief/?gameId={game_id:?}&sessionId={}&hideNav&userId={user_id}", session_id.0)}
                    />
                </ContextMenu>
            }));
        }))
    }
}

#[allow(clippy::type_complexity)]
pub fn renew_session(
    client_request_callback: Callback<ClientRequest>,
    common_settings: &CommonSettings,
    change_common_settings_callback: Callback<
        Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>,
    >,
) {
    let set_login = set_login(client_request_callback, change_common_settings_callback);

    let url = format!(
        "https://softbear.com/api/auth/session.json?sessionId={}",
        common_settings
            .session_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| "1".to_owned())
    );

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);
    opts.credentials(RequestCredentials::Include);

    let window = web_sys::window().unwrap();

    let _ = future_to_promise(async move {
        let request =
            Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("{:?}", e))?;
        let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
        if resp.ok() {
            let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
            let json: String = JsFuture::from(json_promise)
                .await
                .map_err(|e| format!("{:?}", e))?
                .as_string()
                .ok_or(String::from("JSON not string"))?;
            let decoded: Login = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            set_login.emit(decoded);
        }

        Ok(JsValue::NULL)
    });
}
