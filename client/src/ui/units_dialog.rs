// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::{KiometPhrases, KiometRoute};
use common::field::Field;
use common::tower::{Tower, TowerType};
use common::unit::{Range, Speed, Unit};
use kodiak_client::{translate, use_translator, NexusDialog, RouteLink, Translator};
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Html, Properties};
use yew_router::prelude::use_navigator;

#[derive(PartialEq, Properties)]
pub struct UnitsDialogProps {
    #[prop_or(None)]
    pub selected: Option<Unit>,
}

#[styled_component(UnitsDialog)]
pub fn units_dialog(props: &UnitsDialogProps) -> Html {
    let unit_unselected_css = css!(
        r#"
        cursor: pointer;
        transition: opacity 0.2s;

        :hover {
            opacity: 0.8;
        }
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

    const SCALE: u32 = 3;
    const UNIT_SCALE: u32 = 2;

    let t = use_translator();
    let navigator = use_navigator().unwrap();
    let total_breadth = std::mem::variant_count::<Unit>() as u32 * SCALE + UNIT_SCALE - SCALE;

    fn speed(t: &Translator, unit: Unit) -> String {
        match unit.speed(None) {
            Speed::Immobile => translate!(t, "Is immobile."),
            Speed::Slow => translate!(t, "Travels at a slow speed."),
            Speed::Normal => translate!(t, "Travels at a moderate speed."),
            Speed::Fast => translate!(t, "Travels at a fast speed."),
        }
    }

    fn range(t: &Translator, unit: Unit) -> Option<String> {
        Some(match unit.range()? {
            Range::Short => translate!(t, "Has a short range."),
            Range::Medium => translate!(t, "Has a medium range."),
            Range::Long => translate!(t, "Has a long range."),
        })
    }

    fn damage(t: &Translator, unit: Unit) -> String {
        fn format_damage(t: &Translator, damage: u8) -> String {
            if damage == Unit::INFINITE_DAMAGE {
                translate!(t, "infinite")
            } else {
                format!("{}", damage)
            }
        }

        let surface = unit
            .damage(Field::Surface, Field::Surface)
            .max(unit.damage(Field::Air, Field::Surface));
        let air = unit
            .damage(Field::Surface, Field::Air)
            .max(unit.damage(Field::Air, Field::Air));
        if surface == air {
            let damage = format_damage(t, surface);
            translate!(t, "Does {damage} damage.")
        } else {
            let surface = format_damage(t, surface);
            let air = format_damage(t, air);
            translate!(
                t,
                "Does {surface} damage against surface, {air} damage against air."
            )
        }
    }

    html! {
        <NexusDialog title={props.selected.map(|selected| t.unit_label(selected)).unwrap_or(translate!(t, "Units"))}>
            if let Some(selected) = props.selected {
                if TowerType::iter().any(|tower_type| tower_type.unit_generation(selected).is_some()) {
                    <p>
                        if TowerType::iter().all(|tower_type| tower_type.unit_generation(selected).is_some()) {
                            {"Produced by all "}
                            <RouteLink<KiometRoute> route={KiometRoute::Towers}>{"towers"}</RouteLink<KiometRoute>>
                            {"."}
                        } else {
                            {"Produced by "}
                            {TowerType::iter().filter(|&tower_type| tower_type.unit_generation(selected).is_some()).map(|tower_type| html!{
                                <TowerIcon {tower_type}/>
                            }).intersperse_with(|| html!{{", "}}).collect::<Html>()}
                            {"."}
                        }
                    </p>
                }
                if selected == Unit::Ruler {
                    <p>
                        {"Boosts "}
                        <UnitIcon unit={Unit::Shield}/>
                        {format!(" capacity by {}.", Tower::RULER_SHIELD_BOOST)}
                    </p>
                    <p>{translate!(t, "If it dies, you lose the game.")}</p>
                }
            <p>{damage(&t, selected)}</p>
                <p>
                    if selected == Unit::Shield {
                        {"Immobile unless sent from "}
                        <TowerIcon tower_type={TowerType::Projector}/>
                        {", in which case it travels at a fast speed."}
                    } else {
                        {speed(&t, selected)}
                    }
                </p>
                if let Some(range) = range(&t, selected) {
                    <p>{range}</p>
                }
                if selected.weight() != 0 {
                    <p>
                        {format!("Has weight of {} for", selected.weight())}
                        <UnitIcon unit={Unit::Chopper}/>
                        {"."}
                    </p>
                }
                if selected == Unit::Chopper {
                    <p>{"Can carry other units (weight of 4)."}</p>
                } else if selected == Unit::Emp {
                    <p>{(|| {
                        let seconds = Unit::EMP_SECONDS;
                        translate!(t, "Disables tower for {seconds}.")
                    })()}</p>
                }
                if selected.max_overflow() > 0 {
                    <p>{(|| {
                        let count = selected.max_overflow();
                        translate!(t, "Up to {count} can temporarily overflow a tower.")
                    })()}</p>
                }
            } else {
                <p>
                    {(|| {
                        let count = std::mem::variant_count::<Unit>();
                        translate!(t, "Each of the {count} units are represented by one of the following symbols. They generally fight in the order listed, e.g. shield always absorbs damage first. Click one of them to learn more!")
                    })()}
                </p>
            }
            <svg width={"100%"} viewBox={format!("0 0 {total_breadth} {UNIT_SCALE}")} class={diagram_css}>
                {Unit::iter().enumerate().map(|(i, unit)| {
                    let navigator = navigator.clone();
                    let selected = Some(unit) == props.selected;

                    html!{
                        <>
                            <image
                                x={(i * SCALE as usize).to_string()}
                                y={"0"}
                                width={UNIT_SCALE.to_string()}
                                height={UNIT_SCALE.to_string()}
                                href={AttrValue::Static(SvgCache::get(PathId::Unit(unit), if selected { Color::Blue } else { Color::Gray }))}
                                onclick={Callback::from(move |_| navigator.push(&KiometRoute::units_specific(unit)))}
                                class={classes!((!selected).then(|| unit_unselected_css.clone()))}
                            >
                                <title>{t.unit_label(unit)}</title>
                            </image>
                        </>
                    }
                }).collect::<Html>()}
            </svg>
        </NexusDialog>
    }
}
