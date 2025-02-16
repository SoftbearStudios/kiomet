// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::ui::tower_icon::TowerIcon;
use crate::ui::unit_icon::UnitIcon;
use crate::ui::KiometRoute;
use common::tower::TowerType;
use common::unit::Unit;
use kodiak_client::{
    markdown, translated_text, use_features, use_game_constants, use_translator, DiscordButton,
    IconButton, Link, MarkdownOptions, NexusDialog, RouteLink,
};
use yew::{function_component, html, AttrValue, Html};
use yew_icons::IconId;

#[function_component(HelpDialog)]
pub fn help_dialog() -> Html {
    let t = use_translator();
    let features = use_features();
    let game_constants = use_game_constants();
    let md = translated_text!(t, "help_md");
    let components = Box::new(|href: &str, content: &str| match href {
        "/towers/" => Some(html! {
            <RouteLink<KiometRoute> route={KiometRoute::Towers}>{content.replace('#', &std::mem::variant_count::<TowerType>().to_string())}</RouteLink<KiometRoute>>
        }),
        "/units/" => Some(html! {
            <RouteLink<KiometRoute> route={KiometRoute::Units}>{content.replace('#', &std::mem::variant_count::<Unit>().to_string())}</RouteLink<KiometRoute>>
        }),
        "UnitProducingTowers" => Some(
            TowerType::iter()
                .filter(TowerType::generates_mobile_units)
                .map(|tower_type| {
                    html! {
                        <TowerIcon {tower_type}/>
                    }
                })
                .intersperse_with(|| html!({ " " }))
                .collect::<Html>(),
        ),
        "RequestAlliance" => Some(html! {
            <img
                src={AttrValue::Static(SvgCache::get(PathId::RequestAlliance, Color::Purple))}
                style={"width: 1.5rem; height: 1.5rem; vertical-align: bottom;"}
                alt={"the handshake button"}
            />
        }),
        "Ruler" => Some(html! {
            <UnitIcon unit={Unit::Ruler}/>
        }),
        "Nuke" => Some(html! {
            <UnitIcon unit={Unit::Nuke}/>
        }),
        "Bunker" => Some(html! {
            <TowerIcon tower_type={TowerType::Bunker}/>
        }),
        "Headquarters" => Some(html! {
            <TowerIcon tower_type={TowerType::Headquarters}/>
        }),
        _ => None,
    });
    html! {
        <NexusDialog title={t.help_title(game_constants)}>
            {markdown(&md, &MarkdownOptions{components, ..Default::default()})}
            if features.outbound.social_media {
                <h3>{"Resources"}</h3>
                <p>
                    {"You are encouraged to join "}
                    <DiscordButton size={"1.5rem"}/>
                    {" if you have a question."}
                </p>
                <p>
                    <Link href="https://www.youtube.com/watch?v=46UO-Bub4Sk">
                        {"How to Kiomet"}
                    </Link>
                    {" by Leecros on "}
                    <IconButton
                        icon_id={IconId::BootstrapYoutube}
                        title={"YouTube"}
                        link={"https://www.youtube.com/watch?v=46UO-Bub4Sk"}
                        size={"1.5rem"}
                    />
                    {" is a good video tutorial!"}
                </p>
            }
        </NexusDialog>
    }
}
