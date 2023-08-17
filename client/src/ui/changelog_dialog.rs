use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use common::tower::TowerType;
use common::unit::Unit;
use yew::{function_component, html, Html};
use yew_frontend::dialog::dialog::Dialog;
use yew_frontend::frontend::use_game_id;
use yew_frontend::translation::{use_translation, Translation};

#[function_component(ChangelogDialog)]
pub fn changelog_dialog() -> Html {
    let t = use_translation();
    let game_id = use_game_id();
    html! {
        <Dialog title={t.changelog_title(game_id)}>
            <p>{"Warning: This changelog may not always be fully up to date"}</p>

            <h2>{"2023"}</h2>

            <h3>{"8/4/2023"}</h3>

            <ul>
                <li>{"Add alliances."}</li>
                <li>{"Zombies are in hibernation."}</li>
                <li>{"Units no longer have to wait between supply line segments."}</li>
                <li><UnitIcon unit={Unit::Chopper}/>{" can pick up units between supply lines."}</li>
                <li>{"Units can pick up "}<UnitIcon unit={Unit::Shield}/>{" from "}<TowerIcon tower_type={TowerType::Projector}/></li>
                <li>{"Allow cyclic supply lines."}</li>
                <li>{"Show score and rank on leaderboard."}</li>
                <li>{"Add accounts."}</li>
                <li>{"Improve visibility of player names."}</li>
                <li>{"Move chat to bottom left."}</li>
                <li>{"Make towers highlighted by holding T more noticeable."}</li>
                <li>{"Fix rare blank screen bug."}</li>
                <li>{"Optimize bandwidth and performance."}</li>
            </ul>

            <h2>{"2022"}</h2>

            <h3>{"10/27/2022"}</h3>

            <ul>
                <li>{"Bots and world border zombies are less aggressive."}</li>
                <li>{"Interstitial zombies are more aggressive."}</li>
                <li>{"Fix the Shift + R feature to reliably remove visible supply lines."}</li>
                <li>{"Fix the background music."}</li>
            </ul>

            <h3>{"10/23/2022"}</h3>

            <ul>
                <li>
                    {"Added tower "}
                    <TowerIcon tower_type={TowerType::Projector}/>
                    {"."}
                </li>
                <li>
                    <UnitIcon unit={Unit::Ruler}/>
                    {" boosts "}
                    <UnitIcon unit={Unit::Shield}/>
                    {" capacity of its tower."}
                </li>
                <li>
                    {"Increase damage of "}
                    <UnitIcon unit={Unit::Chopper}/>
                    {"."}
                </li>
                <li>
                    {"Add spawn protection in the form of "}
                    <UnitIcon unit={Unit::Shield}/>
                    {"."}
                </li>
                <li>
                    <UnitIcon unit={Unit::Emp}/>
                    {" can now suppress supply lines."}
                </li>
                <li>{"World border resists unwarranted expansion."}</li>
                <li>{"Bots and zombies are smarter."}</li>
                <li>{"Supply lines (of selected or visible towers) can be cancelled by holding Shift + R."}</li>
                <li>{"Added option to demolish an upgraded tower back to level 1."}</li>
                <li>{"All units travel a bit faster."}</li>
                <li>{"The T key will now highlight other towers of the same type as the selected tower."}</li>
                <li>{"Show the owner of enemy towers in the tower menu."}</li>
                <li>{"Make sure the tutorial doesn't go on forever."}</li>
                <li>{"Save nickname on splash screen."}</li>
                <li>{"Add a warning when the connection is temporarily lost."}</li>
                <li>{"Convert emoji shortcodes like :smile: to Unicode emoji like 😄 (in chat)."}</li>
            </ul>

            <h3>{"9/12/2022"}</h3>

            <ul>
                <li>
                    {"Added towers "}
                    {[TowerType::Artillery, TowerType::Refinery, TowerType::Satellite, TowerType::Launcher, TowerType::Rocket].into_iter().map(|tower_type| html!{
                        <TowerIcon {tower_type}/>
                    }).intersperse_with(|| html!{{", "}}).collect::<Html>()}
                    {" and units "}
                    <UnitIcon unit={Unit::Shell}/>
                    {", "}
                    <UnitIcon unit={Unit::Emp}/>
                    {"."}
                </li>
                <li>{"Added long-distance supply lines."}</li>
                <li>{"Added visual tutorial for deploying units and upgrading."}</li>
                <li>{"Pressing 'r' now shows all your supply lines."}</li>
                <li>
                    {"Pressing 'h' teleports your view to your "}
                    <UnitIcon unit={Unit::Ruler}/>
                    {" (if it is in a tower)."}
                </li>
                <li>
                    {"Add a visible delay when moving your "}
                    <UnitIcon unit={Unit::Ruler}/>
                    {" outside your territory to minimize errors."}
                </li>
                <li>{"Bots can now fight and upgrade towers at the same time."}</li>
                <li>{"The time window in which you can reconnect to save your progress is doubled."}</li>
                <li>
                    {"Change appearance of "}
                    <TowerIcon tower_type={TowerType::Barracks}/>
                    {" as demonstrated by TheMrPancake."}
                </li>
                <li>{"Add navigation buttons to bottom of each dialog."}</li>
                <li>{"Add visible mentions to chat."}</li>
                <li>{"Translate the game to Hindi."}</li>
                <li>{"Add error message if WebGL is unsupported."}</li>
            </ul>

            <h3>{"9/1/2022"}</h3>

            <ul>
                <li>{"Release the game, ending public beta period 🎉"}</li>
            </ul>
        </Dialog>
    }
}
