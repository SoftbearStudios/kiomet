use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::TowerRoute;
use common::tower::TowerType;
use common::unit::Unit;
use yew::{function_component, html, AttrValue, Html};
use yew_frontend::component::route_link::RouteLink;
use yew_frontend::dialog::dialog::Dialog;
use yew_frontend::frontend::use_game_id;
use yew_frontend::translation::{use_translation, Translation};

#[function_component(HelpDialog)]
pub fn help_dialog() -> Html {
    let t = use_translation();
    let game_id = use_game_id();
    let game_name = game_id.name();
    html! {
        <Dialog title={t.help_title(game_id)}>
            <p>
                {format!("{} is an online real-time-strategy game, in which you expand your territory by sending ", game_name)}
                <RouteLink<TowerRoute> route={TowerRoute::Units}>{"units"}</RouteLink<TowerRoute>>
                {" to capture "}
                <RouteLink<TowerRoute> route={TowerRoute::Towers}>{"towers"}</RouteLink<TowerRoute>>
                {"."}
            </p>
            <h2>{"How to Play"}</h2>
            <p>{"Drag units to capture towers. To upgrade a tower, click it and then click an available upgrade. Upgrades have their requirements listed next to them."}</p>
            <h2>{"How to Win"}</h2>
            <p>
                {"To earn points, capture more towers and hold them for as long as possible. Protect your "}
                <UnitIcon unit={Unit::Ruler}/>
                {" as losing it will cost you the game! You might want to move your "}
                <UnitIcon unit={Unit::Ruler}/>
                {" to a "}
                {TowerType::iter().filter(|t| t.max_ranged_damage() != Unit::INFINITE_DAMAGE).map(|tower_type| html! {
                     <TowerIcon {tower_type}/>
                }).intersperse_with(|| html!({{" or "}})).collect::<Html>()}
                {" which can survive a few "}
                <UnitIcon unit={Unit::Nuke}/>
                {"."}
            </p>
            <h2>{"Supply Lines"}</h2>
            <p>
            {TowerType::iter().filter(TowerType::generates_mobile_units).map(|tower_type| html! {
                 <TowerIcon {tower_type}/>
            }).intersperse_with(|| html!({{" "}})).collect::<Html>()}
            {" can automatically send units via supply lines. To create a supply line, click a tower to open its menu. Then drag from the tower as normal. If the resulting path has moving arrows, you've succeeded. Hold R to display all your supply lines. To delete a supply line, create the same one again or hold Shift + R."}</p>
            <h2>{"Alliances"}</h2>
            <p>
                {"Select an enemy tower and click "}
                <img
                    src={AttrValue::Static(SvgCache::get(PathId::RequestAlliance, Color::Purple))}
                    style={"width: 1.5rem; height: 1.5rem; vertical-align: bottom;"}
                    alt={"the handshake button"}
                />
                {" to request or accept an alliance. Until broken, the alliance will prevent each side from attacking."}</p>
            <h2>{"Chat"}</h2>
            <p>{"Use the panel in the bottom left to send messages to other players. Remember to never share personal information in chat!"}</p>
        </Dialog>
    }
}
