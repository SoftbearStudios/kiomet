// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::{KiometPhrases, KiometRoute};
use common::tower::{TowerArray, TowerType};
use common::unit::Unit;
use kodiak_client::glam::UVec2;
use kodiak_client::{translate, use_translator, NexusDialog};
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, html_nested, Callback, Html, Properties};
use yew_router::prelude::use_navigator;

#[derive(PartialEq, Properties)]
pub struct TowersDialogProps {
    #[prop_or(None)]
    pub selected: Option<TowerType>,
}

#[styled_component(TowersDialog)]
pub fn towers_dialog(props: &TowersDialogProps) -> Html {
    let tower_unselected_css = css!(
        r#"
        cursor: pointer;
        transition: opacity 0.2s;

        :hover {
            opacity: 0.8;
        }
        "#
    );

    let upgrade_css = css!(
        r#"
        stroke: white;
        stroke-width: 0.25;
        stroke-linecap: round;
        opacity: 0.9;
        "#
    );

    let prerequisite_css = css!(
        r#"
        stroke: white;
        stroke-width: 0.15;
        stroke-dasharray: 0.5, 0.5;
        opacity: 0.25;
        "#
    );

    let diagram_css = css!(
        r#"
        position: absolute;
        left: 50%;
        transform: translate(-50%, 0);
        bottom: 0;
        margin-bottom: 1.5rem;
        width: calc(100% - 3rem);
        max-height: 40vh;
        "#
    );

    let mut layout = TowerArray::<UVec2>::new();

    // returns columns used.
    fn do_layout(
        layout: &mut TowerArray<UVec2>,
        offset: u32,
        towers: impl Iterator<Item = TowerType>,
    ) -> u32 {
        let mut used = 0;
        for tower in towers {
            layout[tower] = UVec2::new(offset + used, tower.level() as u32);
            used += do_layout(layout, offset + used, tower.upgrades()).max(1);
        }
        used
    }

    const SCALE: u32 = 4;
    const TOWER_SCALE: u32 = 2;

    fn coord(c: u32) -> u32 {
        c * SCALE
    }

    fn coord_middle(c: u32) -> u32 {
        coord(c) + TOWER_SCALE / 2
    }

    fn coord_bottom(c: u32) -> u32 {
        coord(c) + TOWER_SCALE
    }

    fn coord_string(c: u32) -> String {
        coord(c).to_string()
    }

    fn coord_middle_string(c: u32) -> String {
        coord_middle(c).to_string()
    }

    fn coord_bottom_string(c: u32) -> String {
        coord_bottom(c).to_string()
    }

    let t = use_translator();
    let navigator = use_navigator().unwrap();
    let total_depth =
        coord(TowerType::iter().map(|t| t.level()).max().unwrap() as u32 + 1) + TOWER_SCALE - SCALE;
    let total_breadth = coord(do_layout(
        &mut layout,
        0,
        TowerType::iter().filter(|t| t.level() == 0),
    )) + TOWER_SCALE
        - SCALE;

    fn tower_ranged_damages(tower_type: TowerType) -> Html {
        let mut iter = Unit::iter().filter(|u| u.is_ranged()).filter_map(|u| {
            let damage = u.force_ground_damage();
            let ranged_damage = tower_type.ranged_damage(damage);
            (ranged_damage < damage).then_some((u, ranged_damage))
        });
        let collected = iter
            .clone()
            .map(|(unit, damage)| {
                html! {
                    <>
                        {format!("{} damage from", damage)}
                        <UnitIcon unit={unit}/>
                    </>
                }
            })
            .intersperse_with(|| html! {{", "}})
            .collect::<Html>();

        if iter.next().is_none() {
            Html::default()
        } else {
            html! {
                <p>
                    {"Only takes "}
                    {collected}
                    {"."}
                </p>
            }
        }
    }

    html! {
        <NexusDialog title={props.selected.map(|selected| t.tower_type_label(selected)).unwrap_or(translate!(t, "Towers"))}>
             if let Some(selected) = props.selected {
                if let Some(downgrade) = selected.downgrade() {
                    <p>
                        {"Upgrades from "}
                        <TowerIcon tower_type={downgrade}/>
                        {format!(" (in {}s)", selected.delay().to_whole_secs())}
                        if selected.prerequisites().next().is_some() {
                            {", requires "}
                            {selected.prerequisites().map(|(prerequisite, count)| {
                                html! {
                                    <span>
                                        {format!("{}", count)}
                                        <TowerIcon tower_type={prerequisite}/>
                                    </span>
                                }
                            }).intersperse_with(|| html!{{", "}}).collect::<Html>()}
                        }
                        {"."}
                    </p>
                }
                <p>
                    {"Can contain "}
                    {Unit::iter().filter_map(|unit| {
                        let capacity = selected.raw_unit_capacity(unit);
                        (capacity > 0).then(|| html!{
                            <>
                                {format!("{}", capacity)}
                                <UnitIcon {unit}/>
                            </>
                        })
                    }).intersperse_with(|| html!{{", "}}).collect::<Html>()}
                    {"."}
                </p>
                <p>
                    {"Generates "}
                    {Unit::iter().filter_map(|unit| {
                        selected.unit_generation(unit).map(|rate| html!{
                            <>
                                <UnitIcon {unit}/>
                                {format!("(every {}s)", rate.to_whole_secs())}
                            </>
                        })
                    })
                        .chain(std::iter::once(html!{{t.score(selected.score_weight())}}))
                        .intersperse_with(|| html!{{", "}}).collect::<Html>()}
                    {"."}
                </p>
                {tower_ranged_damages(selected)}
                if selected.sensor_radius() > TowerType::Mine.sensor_radius() {
                    <p>{(|| {
                        let percent = 100 * (selected.sensor_radius() - TowerType::Mine.sensor_radius()) / TowerType::Mine.sensor_radius();
                        translate!(t, "Has a {percent}% higher visual range.")
                    })()}</p>
                }
                if selected == TowerType::Projector {
                    <p>
                        {"Can send "}
                        <UnitIcon unit={Unit::Shield}/>
                        {" along roads."}
                    </p>
                }
            } else {
                <p>
                    {(|| {
                        let count = std::mem::variant_count::<TowerType>();
                        translate!(t, "Each of the {count} towers are represented by one of the following symbols. The solid lines show upgrades, and the dashed lines show prerequisites. Click one of them to learn more!")
                    })()}
                </p>
            }
            <svg width={"100%"} viewBox={format!("0 0 {total_breadth} {total_depth}")} class={diagram_css}>
                {TowerType::iter().map(|tower| {
                    let navigator = navigator.clone();
                    let offset = layout[tower];
                    let navigator = navigator.clone();
                    let upgrade_css = upgrade_css.clone();
                    let prerequisite_css = prerequisite_css.clone();
                    let selected = Some(tower) == props.selected;

                    html!{
                        <>
                            <image
                                x={coord_string(offset.x)}
                                y={coord_string(offset.y)}
                                width={TOWER_SCALE.to_string()}
                                height={TOWER_SCALE.to_string()}
                                href={AttrValue::Static(SvgCache::get(PathId::Tower(tower), if selected { Color::Blue } else { Color::Gray }))}
                                onclick={Callback::from(move |_| navigator.push(&KiometRoute::towers_specific(tower)))}
                                class={classes!((!selected).then(|| tower_unselected_css.clone()))}
                            >
                            <title>{t.tower_type_label(tower)}</title>
                            </image>
                            if let Some(downgrade) = tower.downgrade().map(|downgrade| layout[downgrade]) {
                                <line x1={coord_middle_string(downgrade.x)} y1={coord_bottom_string(downgrade.y)} x2={coord_middle_string(offset.x)} y2={coord_string(offset.y)} class={upgrade_css} />
                            }
                            {tower.prerequisites().map(|(prerequisite, _)| {
                                let prerequisite = layout[prerequisite];
                                let prerequisite_css = prerequisite_css.clone();
                                html_nested! {
                                    <line x1={coord_middle_string(prerequisite.x)} y1={coord_bottom_string(prerequisite.y)} x2={coord_middle_string(offset.x)} y2={coord_string(offset.y)} class={prerequisite_css} />
                                }
                            }).collect::<Html>()}
                        </>
                    }
                }).collect::<Html>()}
            </svg>
        </NexusDialog>
    }
}
