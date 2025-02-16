// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ui::KiometRoute;
use common::tower::TowerType;
use common::unit::Unit;
use kodiak_client::{
    markdown, translated_text, use_features, use_game_constants, use_translator, EngineNexus, Link,
    MarkdownOptions, NexusDialog, RouteLink,
};
use std::mem::variant_count;
use yew::{function_component, html, Html};

#[function_component(AboutDialog)]
pub fn about_dialog() -> Html {
    let t = use_translator();
    let game_constants = use_game_constants();
    let features = use_features();
    let credits = features.outbound.credits;
    let md = translated_text!(t, "about_md");
    let components = Box::new(move |href: &str, content: &str| match href {
        "/help/" => Some(html! {
            <RouteLink<KiometRoute> route={KiometRoute::Help}>{content.to_owned()}</RouteLink<KiometRoute>>
        }),
        "/towers/" => Some(html! {
            <RouteLink<KiometRoute> route={KiometRoute::Towers}>{content.replace('#', &variant_count::<TowerType>().to_string())}</RouteLink<KiometRoute>>
        }),
        "/units/" => Some(html! {
            <RouteLink<KiometRoute> route={KiometRoute::Units}>{content.replace('#', &variant_count::<Unit>().to_string())}</RouteLink<KiometRoute>>
        }),
        "/licensing/" => Some(html! {
            <RouteLink<EngineNexus> route={EngineNexus::Licensing}>{content}</RouteLink<EngineNexus>>
        }),
        "https://timbeek.com" => Some(html! {
            <Link href="https://timbeek.com" enabled={credits}>{"Tim Beek"}</Link>
        }),
        _ => None,
    });

    let markdown_options = MarkdownOptions {
        components,
        ..Default::default()
    };

    html! {
        <NexusDialog title={t.about_title(game_constants)}>
            {markdown(&md, &markdown_options)}
            {markdown(include_str!("./translations/credits.md"), &markdown_options)}
            if features.outbound.contact_info {
                {t.about_contact()}
            }
        </NexusDialog>
    }
}
