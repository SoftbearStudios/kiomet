use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::translation::TowerTranslation;
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::TowerRoute;
use common::field::Field;
use common::tower::{Tower, TowerType};
use common::unit::{Range, Speed, Unit};
use std::borrow::Cow;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Html, Properties};
use yew_frontend::component::route_link::RouteLink;
use yew_frontend::dialog::dialog::Dialog;
use yew_frontend::translation::use_translation;
use yew_router::prelude::use_navigator;

#[derive(PartialEq, Properties)]
pub struct UnitsDialogProps {
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

    let t = use_translation();
    let navigator = use_navigator().unwrap();
    let total_breadth = std::mem::variant_count::<Unit>() as u32 * SCALE + UNIT_SCALE - SCALE;

    fn speed(unit: Unit) -> &'static str {
        match unit.speed(None) {
            Speed::Immobile => "Is immobile.",
            Speed::Slow => "Travels at a slow speed.",
            Speed::Normal => "Travels at a moderate speed.",
            Speed::Fast => "Travels at a fast speed.",
        }
    }

    fn range(unit: Unit) -> Option<&'static str> {
        Some(match unit.range()? {
            Range::Short => "Has a short range.",
            Range::Medium => "Has a medium range.",
            Range::Long => "Has a long range.",
        })
    }

    fn damage(unit: Unit) -> String {
        fn format_damage(damage: u8) -> Cow<'static, str> {
            if damage == Unit::INFINITE_DAMAGE {
                Cow::Borrowed("infinite")
            } else {
                Cow::Owned(format!("{}", damage))
            }
        }

        let surface = unit
            .damage(Field::Surface, Field::Surface)
            .max(unit.damage(Field::Air, Field::Surface));
        let air = unit
            .damage(Field::Surface, Field::Air)
            .max(unit.damage(Field::Air, Field::Air));
        if surface == air {
            format!("Does {} damage.", format_damage(surface))
        } else {
            format!(
                "Does {} damage against surface, {} damage against air.",
                format_damage(surface),
                format_damage(air)
            )
        }
    }

    html! {
        <Dialog title={props.selected.map(|selected| t.unit_label(selected)).unwrap_or("Units")}>
            if let Some(selected) = props.selected {
                if TowerType::iter().any(|tower_type| tower_type.unit_generation(selected).is_some()) {
                    <p>
                        if TowerType::iter().all(|tower_type| tower_type.unit_generation(selected).is_some()) {
                            {"Produced by all "}
                            <RouteLink<TowerRoute> route={TowerRoute::Towers}>{"towers"}</RouteLink<TowerRoute>>
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
                    <p>{"If it dies, you lose the game."}</p>
                }
            <p>{damage(selected)}</p>
                <p>
                    if selected == Unit::Shield {
                        {"Immobile unless sent from "}
                        <TowerIcon tower_type={TowerType::Projector}/>
                        {"."}
                    } else {
                        {speed(selected)}
                    }
                </p>
                if let Some(range) = range(selected) {
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
                    <p>{format!("Disables tower for {} seconds.", Unit::EMP_SECONDS)}</p>
                }
                if selected.max_overflow() > 0 {
                    <p>{format!("Up to {} can temporarily overflow a tower.", selected.max_overflow())}</p>
                }
            } else {
                <p>
                    {format!("Each of the {} units are represented by one of the following symbols. They generally fight in the order listed, e.g. shield always absorbs damage first. Click one of them to learn more!", std::mem::variant_count::<Unit>())}
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
                                onclick={Callback::from(move |_| navigator.push(&TowerRoute::units_specific(unit)))}
                                class={classes!((!selected).then(|| unit_unselected_css.clone()))}
                            >
                                <title>{t.unit_label(unit)}</title>
                            </image>
                        </>
                    }
                }).collect::<Html>()}
            </svg>
        </Dialog>
    }
}
