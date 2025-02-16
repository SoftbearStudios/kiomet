// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::game::KiometGame;
use crate::path::{PathId, SvgCache};
use crate::ui::tower_icon::TowerIcon;
use crate::ui::{KiometPhrases, KiometUiEvent};
use common::tower::TowerType;
use kodiak_client::{
    is_mobile, use_rewarded_ad, use_translator, use_ui_event_callback, Curtain, Position,
    Positioner, RewardedAd,
};
use stylist::yew::styled_component;
use yew::{html, AttrValue, Callback, Html, MouseEvent, Properties};

#[derive(PartialEq, Properties)]
pub struct LockDialogProps {
    pub tower_type: TowerType,
    pub keys: usize,
}

#[styled_component(LockDialog)]
pub fn lock_dialog(props: &LockDialogProps) -> Html {
    let button_style = css!(
        r#"
        border: none;
        border-radius: 0.5rem;
        padding: 0.5rem;
        color: white;
        transition: filter 0.1s;
        font-size: 1.1rem;
        appearance: none;

        :hover {
            filter: brightness(0.85);
        }

        :active {
            filter: brightness(0.7);
        }
    "#
    );

    fn attr<T: Into<AttrValue>>(s: T) -> Option<AttrValue> {
        Some(s.into())
    }

    let t = use_translator();
    let ui_event_callback = use_ui_event_callback::<KiometGame>();

    let rewarded_ad = use_rewarded_ad();
    let tower_type = props.tower_type;
    let on_ok = if props.keys == 0 {
        let request_ad = if let RewardedAd::Available { request } = rewarded_ad.clone() {
            Some(request)
        } else {
            None
        };
        let ui_event_callback = ui_event_callback.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(request_ad) = request_ad.as_ref() {
                request_ad.emit(Some(
                    ui_event_callback.reform(move |_: ()| KiometUiEvent::Unlock(tower_type)),
                ))
            }
        })
    } else {
        ui_event_callback.reform(move |_: MouseEvent| KiometUiEvent::Unlock(tower_type))
    };

    let on_close = ui_event_callback.reform(|_: MouseEvent| KiometUiEvent::LockDialog(None));

    let icon_factory = |path_id: PathId| {
        html! {
            <img
                alt={"key"}
                src={attr(SvgCache::get(path_id, Color::Blue))}
                style={"width: 1.5rem; vertical-align: bottom;"}
            />
        }
    };

    let key = icon_factory(PathId::Key);

    html! {
        <Curtain opacity={127} onclick={on_close.clone()}>
            <Positioner position={Position::Center}>
                <div
                    style="display: flex; flex-direction: row; background-color: #2c3e50; overflow: hidden; border-radius: 0.5rem;"
                    onclick={|e: MouseEvent| e.stop_propagation()}
                >
                    <div style="display: flex; flex-direction: column; gap: 1rem; text-align: left; padding: 1rem; min-width: 16rem; max-width: 20rem;">
                        <h2 style="margin: 0; font-size: 1.6rem;">
                            <TowerIcon tower_type={props.tower_type} size={"2rem"}/>
                            {format!(" Unlock {}", t.tower_type_label(props.tower_type))}
                        </h2>

                        <p style="margin: 0;">
                            {format!(
                                "Congratulations! You may upgrade to {}.",
                                t.tower_type_label(props.tower_type)
                            )}
                        </p>
                        <p style="margin: 0;">
                            if props.keys == 0 {
                                {"Unfortunately, you are currently out of "}
                                {key.clone()}
                                {"'s. You can still unlock this upgrade by watching an ad."}
                            } else {
                                {"Spend "}
                                if props.keys > 1 {
                                    {"one of "}
                                }
                                {"your "}
                                {props.keys}
                                {" "}
                                {key.clone()}
                                if props.keys > 1 {
                                    {"'s"}
                                }
                                {" to unlock this upgrade."}
                            }
                        </p>

                        <div style="margin-top: auto; display: flex; flex-direction: column; gap: 1rem; justify-content: center;">
                            <button
                                style="background-color: #34ace0; font-weight: bold;"
                                class={button_style.clone()}
                                onclick={on_ok}
                            >
                                if props.keys == 0 {
                                    {"Watch ad to unlock 🎬"}
                                } else {
                                    {"Unlock with "}
                                    {key}
                                }
                            </button>
                            <button
                                style="background-color: #4a6784;"
                                class={button_style}
                                onclick={on_close}
                            >{"Return to game"}</button>
                        </div>
                    </div>
                    if !is_mobile() {
                        <img
                            src={format!("/data/paintings/{tower_type}.webp")}
                            style="width: 30rem; min-height: 30rem; height: auto; background-color: #a25e5f; user-drag: none; -webkit-user-drag: none;"
                        />
                    }
                </div>
            </Positioner>
        </Curtain>
    }
}
