// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

mod about_dialog;
mod alert_overlay;
mod button;
mod changelog_dialog;
mod help_dialog;
mod lock_dialog;
mod tower_icon;
mod tower_overlay;
mod towers_dialog;
mod unit_icon;
mod units_dialog;

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::settings::Unlocks;
use crate::translation::TowerTranslation;
use crate::tutorial::TutorialAlert;
use crate::ui::about_dialog::AboutDialog;
use crate::ui::alert_overlay::AlertOverlay;
use crate::ui::changelog_dialog::ChangelogDialog;
use crate::ui::help_dialog::HelpDialog;
use crate::ui::towers_dialog::TowersDialog;
use crate::TowerGame;
use common::alerts::Alerts;
use common::death_reason::DeathReason;
use common::tower::{Tower, TowerArray, TowerId, TowerType};
use common::unit::Unit;
use core_protocol::name::PlayerAlias;
use core_protocol::PlayerId;
use engine_macros::SmolRoutable;
use glam::IVec2;
use lock_dialog::LockDialog;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use strum::IntoEnumIterator;
use stylist::yew::styled_component;
use tower_overlay::TowerOverlay;
use units_dialog::UnitsDialog;
use yew::prelude::*;
use yew::virtual_dom::AttrValue;
use yew_frontend::component::account_menu::AccountMenu;
use yew_frontend::component::discord_icon::DiscordIcon;
use yew_frontend::component::invitation_link::InvitationLink;
use yew_frontend::component::language_menu::LanguageMenu;
use yew_frontend::component::positioner::{Align, Flex, Position, Positioner};
use yew_frontend::component::privacy_link::PrivacyLink;
use yew_frontend::component::route_link::RouteLink;
use yew_frontend::component::terms_link::TermsLink;
use yew_frontend::component::volume_icon::VolumeIcon;
use yew_frontend::component::zoom_icon::ZoomIcon;
use yew_frontend::frontend::{
    use_core_state, use_outbound_enabled, use_ui_event_callback, PropertiesWrapper,
};
use yew_frontend::overlay::chat::ChatOverlay;
use yew_frontend::overlay::leaderboard::LeaderboardOverlay;
use yew_frontend::overlay::spawn::SpawnOverlay;
use yew_frontend::translation::{use_translation, Translation};
use yew_router::prelude::*;

#[derive(Copy, Clone)]
pub enum TowerUiEvent {
    Alliance {
        with: PlayerId,
        break_alliance: bool,
    },
    DismissCaptureTutorial,
    DismissUpgradeTutorial,
    PanTo(TowerId),
    Spawn(PlayerAlias),
    Upgrade {
        tower_id: TowerId,
        tower_type: TowerType,
    },
    Unlock(TowerType),
    LockDialog(Option<TowerType>),
}

#[derive(Clone, PartialEq, Default)]
pub struct TowerUiProps {
    pub alive: bool,
    pub death_reason: Option<DeathReason>,
    pub selected_tower: Option<SelectedTower>,
    pub tower_counts: TowerArray<u8>,
    pub alerts: Alerts,
    pub tutorial_alert: Option<TutorialAlert>,
    pub unlocks: Unlocks,
    pub lock_dialog: Option<TowerType>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SelectedTower {
    /// HTML pixel coordinate of center of tower.
    pub client_position: IVec2,
    /// Color of tower.
    pub color: Color,
    /// Copy of selected tower data.
    pub tower: Tower,
    /// Selected tower id.
    pub tower_id: TowerId,
    /// If we are trying to ally with them or already allied with them.
    pub outgoing_alliance: bool,
}

#[styled_component(TowerUi)]
pub fn tower_ui(props: &PropertiesWrapper<TowerUiProps>) -> Html {
    let ui_event_callback = use_ui_event_callback::<TowerGame>();
    let on_play = ui_event_callback.reform(TowerUiEvent::Spawn);

    let header_css = css!(
        r#"
        color: white;
        text-align: center;
        font-size: 3rem;
        margin: 0;
    "#
    );

    let dot_com_css = css!(
        r#"
        font-size: 2.2rem;
        color: #a5a5a5;
        "#
    );

    let tower_icon_css = css!(
        r#"
        width: 3.8rem;
        vertical-align: bottom;
        margin-right: 0.1rem;
        "#
    );

    let death_reason_css = css!(
        r#"
        text-align: center;
        font-size: 1.2em;
        font-style: italic;
        margin: 0;
    "#
    );

    const HINTS: &[(&str, &[&str])] = &[
        ("Drag units from towers to expand your territory. Click towers to open the upgrade menu.", &["how", "play"]),
        ("Each Mine produces 1 point every second.", &["how", "earn"]),
        ("Click a tower to reveal upgrade options and their prerequisites.", &["how", "upgrade"]),
        ("Upgrade cliffs to quarries, then to silos.", &["how", "nuke"])
    ];

    let t = use_translation();
    let multi_server = use_core_state().servers.len() > 1;
    let outbound_enabled = use_outbound_enabled();

    // <SettingsIcon/>

    const MARGIN: &str = "0.75rem";

    html! {
        <>
            if props.alive {
                <Positioner position={Position::CenterRight{margin: MARGIN}} flex={Flex::Column}>
                    <ZoomIcon amount={-4}/>
                    <ZoomIcon amount={4}/>
                    <VolumeIcon/>
                    <LanguageMenu/>
                </Positioner>
                <LeaderboardOverlay position={Position::TopRight{margin: MARGIN}} style="max-width: 25%;"/>
                if let Some(SelectedTower{client_position, color, tower, tower_id, outgoing_alliance}) = props.selected_tower.clone() {
                    <TowerOverlay
                        {client_position}
                        {color}
                        {tower}
                        {tower_id}
                        {outgoing_alliance}
                        tower_counts={props.tower_counts}
                        tutorial_alert={props.tutorial_alert}
                        unlocks={props.unlocks.clone()}
                    />
                }
                <Positioner position={Position::BottomRight{margin: MARGIN}}>
                    <RouteLink<TowerRoute> route={TowerRoute::Help}>{t.help_hint()}</RouteLink<TowerRoute>>
                </Positioner>
                <Positioner position={Position::TopLeft{margin: MARGIN}} align={Align::Left} max_width="25%">
                    <AlertOverlay alerts={props.alerts} tutorial_alert={props.tutorial_alert}/>
                </Positioner>
                <ChatOverlay position={Position::BottomLeft{margin: MARGIN}} style="max-width: 25%;" hints={HINTS}/>
                if let Some(tower_type) = props.lock_dialog {
                    <LockDialog keys={props.unlocks.keys} {tower_type}/>
                }
            } else {
                <SpawnOverlay {on_play}>
                    <p class={header_css}>
                        <img
                            alt={"rampart"}
                            src={AttrValue::Static(SvgCache::get(PathId::Tower(TowerType::Rampart), Color::Blue))}
                            class={tower_icon_css}
                        />
                        {"Kiomet"}
                        <span class={dot_com_css}>{".com"}</span>
                    </p>
                    if let Some(death_reason) = props.death_reason {
                        <p class={death_reason_css}>{t.death_reason(death_reason)}</p>
                    }
                </SpawnOverlay>
                if multi_server {
                    <Positioner position={Position::TopLeft{margin: MARGIN}}>
                        <InvitationLink/>
                    </Positioner>
                }
                <Positioner position={Position::BottomMiddle{margin: MARGIN}} flex={Flex::Row}>
                    <RouteLink<TowerRoute> route={TowerRoute::Help}>{t.help_hint()}</RouteLink<TowerRoute>>
                    <RouteLink<TowerRoute> route={TowerRoute::About}>{t.about_hint()}</RouteLink<TowerRoute>>
                    <PrivacyLink/>
                    <TermsLink/>
                </Positioner>
                <AccountMenu position={Position::BottomLeft{margin: MARGIN}}/>
                <Positioner position={Position::TopRight{margin: MARGIN}} flex={Flex::Row}>
                    <LanguageMenu/>
                </Positioner>
                if outbound_enabled {
                    <Positioner position={Position::BottomRight{margin: MARGIN}} flex={Flex::Row}>
                        <DiscordIcon/>
                    </Positioner>
                }
            }
            <div>
                <Switch<TowerRoute> render={switch}/>
            </div>
        </>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, SmolRoutable)]
pub enum TowerRoute {
    #[at("/help")]
    Help,
    #[at("/towers/:selected")]
    TowersSelected { selected: PathParam<TowerType> },
    #[at("/towers")]
    Towers,
    #[at("/units/:selected")]
    UnitsSelected { selected: PathParam<Unit> },
    #[at("/units")]
    Units,
    #[at("/about")]
    About,
    #[at("/changelog")]
    Changelog,
    #[not_found]
    #[at("/")]
    Home,
}

impl TowerRoute {
    fn towers_specific(tower_type: TowerType) -> Self {
        Self::TowersSelected {
            selected: PathParam(tower_type),
        }
    }

    fn units_specific(unit: Unit) -> Self {
        Self::UnitsSelected {
            selected: PathParam(unit),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PathParam<T>(T);

impl<T: Debug> Display for PathParam<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.0).to_ascii_lowercase())
    }
}

impl<T: Debug + IntoEnumIterator> FromStr for PathParam<T> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        T::iter()
            .find(|typ| format!("{:?}", typ).eq_ignore_ascii_case(s))
            .map(Self)
            .ok_or(())
    }
}

fn switch(routes: TowerRoute) -> Html {
    match routes {
        TowerRoute::Help => html! {
            <HelpDialog/>
        },
        TowerRoute::About => html! {
            <AboutDialog/>
        },
        TowerRoute::Changelog => html! {
            <ChangelogDialog/>
        },
        TowerRoute::Towers => html! {
            <TowersDialog/>
        },
        TowerRoute::TowersSelected { selected } => html! {
            <TowersDialog selected={selected.0}/>
        },
        TowerRoute::Units => html! {
            <UnitsDialog/>
        },
        TowerRoute::UnitsSelected { selected } => html! {
            <UnitsDialog selected={selected.0}/>
        },
        TowerRoute::Home => html! {},
    }
}
