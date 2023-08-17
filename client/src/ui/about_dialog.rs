use crate::ui::TowerRoute;
use common::tower::TowerType;
use common::unit::Unit;
use std::mem::variant_count;
use yew::{function_component, html, Html};
use yew_frontend::component::discord_icon::DiscordIcon;
use yew_frontend::component::link::Link;
use yew_frontend::component::route_link::RouteLink;
use yew_frontend::dialog::dialog::Dialog;
use yew_frontend::frontend::{use_game_id, use_outbound_enabled};
use yew_frontend::translation::{use_translation, Translation};
use yew_frontend::CONTACT_EMAIL;

#[function_component(AboutDialog)]
pub fn about_dialog() -> Html {
    let t = use_translation();
    let game_id = use_game_id();
    let game_name = game_id.name();
    let outbound_enabled = use_outbound_enabled();
    html! {
        <Dialog title={t.about_title(game_id)}>
            <p>
                {format!("{} is an online real-time-strategy game, in which you expand your territory by capturing towers. ", game_name)}
                {"There are currently "}
                <RouteLink<TowerRoute> route={TowerRoute::Towers}>{format!("{} towers", variant_count::<TowerType>())}</RouteLink<TowerRoute>>
                {" and "}
                <RouteLink<TowerRoute> route={TowerRoute::Units}>{format!("{} units", variant_count::<Unit>())}</RouteLink<TowerRoute>>
                {"."}
            </p>
            <p>{"To learn more about the game, visit the "}<RouteLink<TowerRoute> route={TowerRoute::Help}>{t.help_hint()}</RouteLink<TowerRoute>>{" page. For a list of recent changes, visit the "}<RouteLink<TowerRoute> route={TowerRoute::Changelog}>{t.changelog_hint()}</RouteLink<TowerRoute>>{" page."}</p>
            <h2>{"Technical Details"}</h2>
            <p>
                {"The game is written in the Rust programming language, using WebGL rendering and Yew GUI. "}
                <Link href="https://timbeek.com">{"Tim Beek"}</Link>
                {" composed the background music and Craiyon and DALL·E generated the tower paintings."}
            </p>
            if outbound_enabled {
                <h2>{"Contact Us"}</h2>
                <p>
                    {"If you have any feedback to share, business inquiries, or any other concern, please contact us on "}
                    <DiscordIcon size={"1.5rem"}/>
                    {" or by email at "}
                    <a href={format!("mailto:{}", CONTACT_EMAIL)}>{CONTACT_EMAIL}</a>
                    {"."}
                </p>
            }
        </Dialog>
    }
}
