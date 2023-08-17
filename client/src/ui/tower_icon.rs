use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::translation::TowerTranslation;
use crate::TowerRoute;
use common::tower::TowerType;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{classes, html, Callback, Html, MouseEvent, Properties};
use yew_frontend::translation::use_translation;
use yew_router::hooks::use_navigator;

#[derive(PartialEq, Properties)]
pub struct TowerIconProps {
    pub tower_type: TowerType,
    #[prop_or("1.5rem".into())]
    pub size: AttrValue,
    /// Implies filled.
    #[prop_or(false)]
    pub selected: bool,
    #[prop_or(true)]
    pub filled: bool,
    #[prop_or(Color::Blue)]
    pub fill: Color,
}

#[styled_component(TowerIcon)]
pub fn tower_icon(props: &TowerIconProps) -> Html {
    let tower_css = css!(
        r#"
        user-drag: none;
        -webkit-user-drag: none;
        "#
    );

    let tower_unselected_css = css!(
        r#"
        cursor: pointer;
        transition: opacity 0.2s;

        :hover {
            opacity: 0.8;
        }
        "#
    );

    let t = use_translation();
    let onclick = {
        let tower_type = props.tower_type;
        let navigator = use_navigator().unwrap();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&TowerRoute::towers_specific(tower_type));
        })
    };
    let title = t.tower_type_label(props.tower_type);

    html! {
        <img
            src={AttrValue::Static(SvgCache::get(PathId::Tower(props.tower_type), props.fill))}
            {onclick}
            class={classes!(tower_css, tower_unselected_css.clone())}
            style={format!("width: {}; height: {}; vertical-align: bottom;", props.size, props.size)}
            alt={title}
            {title}
        />
    }
}
