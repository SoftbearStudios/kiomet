// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tutorial::TutorialAlert;
use crate::ui::{KiometPhrases, KiometUiEvent};
use crate::KiometGame;
use common::alerts::{AlertFlag, Alerts};
use common::tower::TowerId;
use kodiak_client::{use_translator, use_ui_event_callback};
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, hook, html, use_state, Callback, Html, MouseEvent, Properties, UseStateHandle};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct AlertOverlayProps {
    pub alerts: Alerts,
    #[prop_or(None)]
    pub tutorial_alert: Option<TutorialAlert>,
}

#[styled_component(AlertOverlay)]
pub fn alert_overlay(props: &AlertOverlayProps) -> Html {
    let send_event = use_ui_event_callback::<KiometGame>();
    let send_event_factory =
        |event: KiometUiEvent| -> Callback<MouseEvent> { send_event.reform(move |_| event) };

    let pan_to = send_event.reform(KiometUiEvent::PanTo);
    let pan_to_factory =
        |tower_id: TowerId| -> Callback<MouseEvent> { pan_to.reform(move |_| tower_id) };

    let overlay_css = css!(
        r#"
        font-size: 1rem;
        transition: opacity 1s;
        "#
    );

    #[hook]
    fn use_dismissible<T>() -> (UseStateHandle<bool>, Callback<T>) {
        let state = use_state(|| true);
        let state_clone = state.clone();
        (
            state,
            Callback::from(move |_| {
                state_clone.set(false);
            }),
        )
    }

    let (show_ruler_not_safe, dismiss_ruler_not_safe) = use_dismissible();
    let (show_full, dismiss_full) = use_dismissible();
    let (show_overflowing, dismiss_overflowing) = use_dismissible();
    let (show_zombies, dismiss_zombies) = use_dismissible();

    let t = use_translator();

    html! {
        <table class={overlay_css}>
            if props.alerts.flags().contains(AlertFlag::RulerUnderAttack) {
                <Alert
                    instruction={t.alert_ruler_under_attack_warning()}
                    hint={t.alert_ruler_under_attack_hint()}
                    icon_id={IconId::FontAwesomeSolidLocationCrosshairs}
                    onclick={props.alerts.ruler_position.map(pan_to_factory)}
                />
            } else if let Some(tutorial_alert) = props.tutorial_alert {
                if let TutorialAlert::Capture(tower_id) = tutorial_alert {
                    <Alert
                        instruction={t.alert_capture_instruction()}
                        hint={t.alert_capture_hint()}
                        icon_id={IconId::FontAwesomeSolidWarehouse}
                        onclick={pan_to_factory(tower_id)}
                        onclick_dismiss={send_event_factory(KiometUiEvent::DismissCaptureTutorial)}
                    />
                } else if let TutorialAlert::Upgrade(tower_id) = tutorial_alert {
                    <Alert
                        instruction={t.alert_upgrade_instruction()}
                        hint={t.alert_upgrade_hint()}
                        icon_id={IconId::FontAwesomeSolidCircleArrowUp}
                        onclick={pan_to_factory(tower_id)}
                        onclick_dismiss={send_event_factory(KiometUiEvent::DismissUpgradeTutorial)}
                    />
                }
            } else if *show_ruler_not_safe && props.alerts.flags().contains(AlertFlag::RulerNotSafe) {
                <Alert
                    instruction={t.alert_ruler_unsafe_instruction()}
                    hint={t.alert_ruler_unsafe_hint()}
                    icon_id={IconId::FontAwesomeSolidHouseCircleExclamation}
                    onclick={props.alerts.ruler_position.map(pan_to_factory)}
                    onclick_dismiss={dismiss_ruler_not_safe}
                />
            }
            if let Some(tower_id) = props.alerts.full.filter(|_| *show_full) {
                <Alert
                    instruction={t.alert_full_warning()}
                    hint={t.alert_full_hint()}
                    icon_id={IconId::BootstrapExclamationTriangleFill}
                    onclick={pan_to_factory(tower_id)}
                    onclick_dismiss={dismiss_full}
                />
            }
            if let Some(tower_id) = props.alerts.overflowing.filter(|_| *show_overflowing) {
                <Alert
                    instruction={t.alert_overflowing_warning()}
                    hint={t.alert_overflowing_hint()}
                    icon_id={IconId::FontAwesomeSolidCircleInfo}
                    onclick={pan_to_factory(tower_id)}
                    onclick_dismiss={dismiss_overflowing}
                />
            }
            if let Some(tower_id) = props.alerts.zombies.filter(|_| *show_zombies) {
                <Alert
                    instruction={t.alert_zombies_warning()}
                    hint={t.alert_zombies_hint()}
                    icon_id={IconId::FontAwesomeSolidPersonWalkingDashedLineArrowRight}
                    onclick={pan_to_factory(tower_id)}
                    onclick_dismiss={dismiss_zombies}
                />
            }
        </table>
    }
}

#[derive(PartialEq, Properties)]
struct AlertProps {
    icon_id: IconId,
    instruction: AttrValue,
    #[prop_or(None)]
    hint: Option<AttrValue>,
    #[prop_or(None)]
    onclick: Option<Callback<MouseEvent>>,
    #[prop_or(None)]
    onclick_dismiss: Option<Callback<MouseEvent>>,
}

#[styled_component(Alert)]
fn alert(props: &AlertProps) -> Html {
    let clickable_css = css!(
        r#"
        cursor: pointer;
        "#
    );

    let dismiss_css = css!(
        r#"
        cursor: pointer;
        opacity: 0.6;

        :hover {
            opacity: 0.8;
        }
        "#
    );

    let t = use_translator();

    html! {
        <tr title={props.hint.clone()} class={classes!(props.onclick.is_some().then_some(clickable_css))}>
            <td>
                <Icon icon_id={props.icon_id} width={"1rem"} height={"1rem"}/>
            </td>
            <td onclick={props.onclick.clone()}>{props.instruction.clone()}</td>
            if let Some(onclick_dismiss) = props.onclick_dismiss.clone() {
                <td title={t.alert_dismiss()} onclick={onclick_dismiss} class={dismiss_css}>{"✘"}</td>
            }
        </tr>
    }
}
