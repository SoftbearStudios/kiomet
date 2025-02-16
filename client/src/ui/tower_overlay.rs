// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::settings::Unlocks;
use crate::tutorial::TutorialAlert;
use crate::ui::button::Button;
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::{KiometPhrases, KiometUiEvent};
use crate::KiometGame;
use common::tower::{Tower, TowerArray, TowerId, TowerType};
use kodiak_client::glam::IVec2;
use kodiak_client::{
    use_core_state, use_rewarded_ad, use_translator, use_ui_event_callback, RankNumber, Translator,
};
use stylist::css;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, html_nested, Callback, Html, MouseEvent, Properties};

#[derive(PartialEq, Properties)]
pub struct TowerOverlayProps {
    pub color: Color,
    pub outgoing_alliance: bool,
    pub tower_id: TowerId,
    pub tower: Tower,
    pub client_position: IVec2,
    pub tower_counts: TowerArray<u16>,
    pub tutorial_alert: Option<TutorialAlert>,
    pub unlocks: Unlocks,
}

#[styled_component(TowerOverlay)]
pub fn tower_overlay(props: &TowerOverlayProps) -> Html {
    let header_css = css!(
        r#"
        margin: 0;
        span + span {
            margin-left: 0.25rem;
        }
        "#
    );

    let div_css = css!(
        r#"
        background-color: #00000022 !important;
        padding: 0.5rem;
        position: absolute !important;
        text-align: left !important;
        width: fit-content;
        "#
    );

    let inner_div_css = css!(
        r#"
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        padding: 0 !important;
        "#
    );

    let large_css = css!(
        r#"
        width: 2.5rem;
        height: 2.5rem;
        vertical-align: bottom;
        user-drag: none;
        -webkit-user-drag: none;
        "#
    );

    let cursor_css = css!(
        r#"
        position: absolute;
        width: 3rem;
        bottom: 36%;
        left: 66%;
        transform: translate(-50%, 50%);
        animation: breathing 2s ease infinite alternate;

        @keyframes breathing {
            0% {
                filter: brightness(0.75);
            }
            100% {
                filter: brightness(1.0);
            }
        }
        "#
    );

    let tower_id = props.tower_id;
    let player_id = props.tower.player_id;

    let on_upgrade_factory = {
        let send_ui_event = use_ui_event_callback::<KiometGame>();

        move |tower_type: TowerType| {
            send_ui_event.reform(move |_: MouseEvent| KiometUiEvent::Upgrade {
                tower_id,
                tower_type,
            })
        }
    };

    let on_alliance_factory = {
        let send_ui_event = use_ui_event_callback::<KiometGame>();

        move |break_alliance: bool| {
            send_ui_event.reform(move |_: MouseEvent| KiometUiEvent::Alliance {
                with: player_id.unwrap(),
                break_alliance,
            })
        }
    };

    let core_state = use_core_state();
    let rewarded_ad = use_rewarded_ad();

    let locked = {
        let any_locked = core_state.rank().flatten() < Some(RankNumber::Rank3);
        let unlocks = props.unlocks.clone();
        let available = !rewarded_ad.is_unavailable();
        move |tower_type: TowerType| -> bool {
            let locked = available && tower_type.level() > 0 && !unlocks.contains(tower_type);
            any_locked && locked
        }
    };

    let ui_event_callback = use_ui_event_callback::<KiometGame>();
    let on_open_lock_dialog_factory = {
        let ui_event_callback = ui_event_callback.clone();
        move |tower_type: TowerType| -> Callback<MouseEvent> {
            ui_event_callback.reform(move |_| KiometUiEvent::LockDialog(Some(tower_type)))
        }
    };

    let unit_color = props.color;
    let outgoing_alliance = props.outgoing_alliance;
    let is_mine = unit_color == Color::Blue;
    let enemy_player_alias = player_id
        .filter(|_| !is_mine)
        .and_then(|player_id| core_state.player_or_bot(player_id))
        .map(|p| p.alias);

    let t = use_translator();
    fn attr<T: Into<AttrValue>>(s: T) -> Option<AttrValue> {
        Some(s.into())
    }
    let tower_type = props.tower.tower_type;
    let basis = tower_type.basis();

    // Only render cursor once.
    let mut has_cursor = true;

    html! {
        <Button
            style={format!("left: {}px; bottom: {}px;", props.client_position.x + 10, props.client_position.y + 10)}
            class={classes!(div_css)}
            content_class={classes!(inner_div_css)}
            progress={props.tower.delay.map(|delay| 1.0 - delay.get() as f32 / props.tower.tower_type.delay().0 as f32).unwrap_or(0.0)}
        >
            <h2 class={header_css}>
                <span><TowerIcon tower_type={tower_type} size={"1.25rem"} fill={unit_color}/></span>
                <span>{t.tower_type_label(props.tower.tower_type)}</span>
            </h2>
            {props.tower.units.iter_with_zeros().filter(|(unit, count)| props.tower.unit_generation(*unit).is_some() || *count > 0).map(|(unit, count)| {
                html_nested!{
                    <p style="margin: 0;" title={t.unit_label(unit)}>
                        <UnitIcon {unit} size={"1.25rem"} fill={unit_color}/>
                        {format!("{}/{}", count, props.tower.units.capacity(unit, Some(props.tower.tower_type)))}
                    </p>
                }
            }).collect::<Html>()}
            if is_mine && props.tower.active() {
                {props.tower.tower_type.upgrades().chain((basis != tower_type).then_some(basis)).map(|upgrade| {
                    let locked = locked(upgrade);
                    let downgrade = basis == upgrade;
                    let upgradable = upgrade.has_prerequisites(&props.tower_counts);
                    let color = if downgrade { Color::Red } else { Color::Blue };
                    html_nested!{
                        <div style="display: flex; flex-direction: row; gap: 0.5rem;">
                            <Button
                                disabled={!upgradable}
                                onclick={if locked { on_open_lock_dialog_factory(upgrade) } else { on_upgrade_factory(upgrade) }}
                                title={(if downgrade { Translator::downgrade_to_label } else { Translator::upgrade_to_label })(&t, &t.tower_type_label(upgrade))}
                                style={format!("overflow: visible; background-color: {};", color.background_color_css())}
                            >
                                <img
                                    alt={"tower"}
                                    src={attr(SvgCache::get(PathId::Tower(upgrade), color))}
                                    class={large_css.clone()}
                                    style={locked.then_some("visibility: hidden;")}
                                />
                                if locked {
                                    <span style="font-size: 2rem; font-weight: bold; color: #ececec; position: absolute; left: 50%; bottom: 50%; transform: translate(-50%, 50%);">
                                        <span style={(!upgradable).then_some("filter: grayscale(1) brightness(0.8);")}>
                                            {"🔒"}
                                        </span>
                                    </span>
                                }
                                if upgradable && !downgrade && props.tutorial_alert == Some(TutorialAlert::Upgrade(tower_id)) && std::mem::take(&mut has_cursor) {
                                    <img
                                        alt={"cursor"}
                                        src={attr(SvgCache::get(PathId::Cursor, Color::Blue))}
                                        class={cursor_css.clone()}
                                    />
                                }
                            </Button>
                            if !downgrade {
                                <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                                    {upgrade.prerequisites().map(|(prerequisite, requirement)| {
                                        let count = props.tower_counts[prerequisite];
                                        let color = if count >= requirement as u16 {
                                            Color::Blue
                                        } else {
                                            Color::Gray
                                        };
                                        html_nested!{
                                            <p style="margin: 0;" title={t.tower_type_label(prerequisite)}>
                                                <TowerIcon tower_type={prerequisite} size={"1.25rem"} fill={color}/>
                                                {format!("{}/{}", count, requirement)}
                                            </p>
                                        }
                                    }).collect::<Html>()}
                                </div>
                            }
                        </div>
                    }
                }).collect::<Html>()}
            }
            {enemy_player_alias.map(|enemy_player_alias| {
                let break_alliance = outgoing_alliance;
                let (color, path_id, title) = if break_alliance {
                    (Color::Red, PathId::BreakAlliance, if unit_color == Color::Purple {
                        t.break_alliance_hint()
                    } else {
                        t.cancel_alliance_hint()
                    })
                } else {
                    (Color::Purple, PathId::RequestAlliance, t.request_alliance_hint())
                };
                let alt = title.clone();

                html_nested! {
                    <div style="display: flex; flex-direction: row; gap: 0.5rem;">
                        <Button
                            onclick={on_alliance_factory(break_alliance)}
                            {title}
                            style={format!("background-color: {};", color.background_color_css())}
                        >
                            <img {alt} style={"width: 2.5rem; height: 2.5rem; vertical-align: bottom; user-drag: none; -webkit-user-drag: none;"} src={attr(SvgCache::get(path_id, color))}/>
                        </Button>
                        <p style="margin: 0;">{enemy_player_alias.to_string()}</p>
                    </div>
                }
            })}
        </Button>
    }
}
