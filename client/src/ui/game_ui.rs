// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::color::Color;
use crate::path::{PathId, SvgCache};
use crate::settings::Unlocks;
use crate::tutorial::TutorialAlert;
use crate::ui::about_dialog::AboutDialog;
use crate::ui::alert_overlay::AlertOverlay;
use crate::ui::help_dialog::HelpDialog;
use crate::ui::lock_dialog::LockDialog;
use crate::ui::tower_overlay::TowerOverlay;
use crate::ui::towers_dialog::TowersDialog;
use crate::ui::units_dialog::UnitsDialog;
use crate::ui::KiometPhrases;
use crate::KiometGame;
use common::alerts::Alerts;
use common::death_reason::DeathReason;
use common::tower::{Tower, TowerArray, TowerId, TowerType};
use common::unit::Unit;
use kodiak_client::glam::IVec2;
use kodiak_client::{
    splash_links, splash_nexus_icons, splash_sign_in_link, splash_social_media, translate, use_ctw,
    use_translator, use_ui_event_callback, Align, ChatOverlay, GameClient, LeaderboardOverlay,
    PathParam, PlayerAlias, PlayerId, Position, Positioner, PropertiesWrapper, RoutableExt,
    SmolRoutable, SpawnOverlay, SplashSocialMediaProps, Translator, SPLASH_MARGIN,
};
use std::fmt::Debug;
use stylist::yew::styled_component;
use yew::prelude::*;
use yew::virtual_dom::AttrValue;
use yew_router::prelude::*;

#[derive(Copy, Clone)]
pub enum KiometUiEvent {
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
pub struct KiometUiProps {
    pub alive: bool,
    pub death_reason: Option<DeathReason>,
    pub selected_tower: Option<SelectedTower>,
    pub tower_counts: TowerArray<u16>,
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

#[styled_component(KiometUi)]
pub fn kiomet_ui(props: &PropertiesWrapper<KiometUiProps>) -> Html {
    let ui_event_callback = use_ui_event_callback::<KiometGame>();
    let on_play = ui_event_callback.reform(KiometUiEvent::Spawn);

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

    let ctw = use_ctw();
    let t = use_translator();
    let nexus = ctw.escaping.is_escaping();
    let social_media_props = SplashSocialMediaProps::default()
        .github("https://github.com/SoftbearStudios/kiomet")
        .google_play("https://play.google.com/store/apps/details?id=com.softbear.kiomet");

    html! {
        <>
            if props.alive && !nexus {
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
                <Positioner position={Position::TopLeft{margin: SPLASH_MARGIN}} align={Align::Left} max_width="25%">
                    <AlertOverlay alerts={props.alerts} tutorial_alert={props.tutorial_alert}/>
                </Positioner>
                <ChatOverlay position={Position::BottomLeft{margin: SPLASH_MARGIN}} style="max-width: 25%;" hints={HINTS}/>
                if let Some(tower_type) = props.lock_dialog {
                    <LockDialog keys={props.unlocks.keys} {tower_type}/>
                }
            } else {
                if !props.alive {
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
                }
                {splash_social_media(&ctw, social_media_props)}
                {splash_links(&ctw, &[KiometRoute::Help], Default::default())}
                {splash_sign_in_link(&ctw)}
            }
            {splash_nexus_icons(&ctw, Default::default())}
            <LeaderboardOverlay
                position={Position::TopRight{margin: SPLASH_MARGIN}}
                style="max-width: 25%;"
                liveboard={props.alive && !nexus}
            />
        </>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, SmolRoutable)]
pub enum KiometRoute {
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
}

impl RoutableExt for KiometRoute {
    fn category(&self) -> Option<&'static str> {
        match self {
            Self::About
            | Self::Help
            | Self::Towers
            | Self::TowersSelected { .. }
            | Self::Units
            | Self::UnitsSelected { .. } => Some("help"),
        }
    }

    fn label(&self, t: &Translator) -> String {
        match self {
            Self::Help => t.help_hint(),
            Self::About => t.about_hint(),
            Self::Towers | Self::TowersSelected { .. } => translate!(t, "Towers"),
            Self::Units | Self::UnitsSelected { .. } => translate!(t, "Units"),
        }
    }

    fn render<G: GameClient>(self) -> Html {
        match self {
            Self::Help => html! {
                <HelpDialog/>
            },
            Self::About => html! {
                <AboutDialog/>
            },
            Self::Towers => html! {
                <TowersDialog/>
            },
            Self::TowersSelected { selected } => html! {
                <TowersDialog selected={selected.0}/>
            },
            Self::Units => html! {
                <UnitsDialog/>
            },
            Self::UnitsSelected { selected } => html! {
                <UnitsDialog selected={selected.0}/>
            },
        }
    }

    fn tabs() -> impl Iterator<Item = Self> + 'static {
        [Self::Help, Self::Towers, Self::Units, Self::About].into_iter()
    }
}

impl KiometRoute {
    pub(crate) fn towers_specific(tower_type: TowerType) -> Self {
        Self::TowersSelected {
            selected: PathParam(tower_type),
        }
    }

    pub(crate) fn units_specific(unit: Unit) -> Self {
        Self::UnitsSelected {
            selected: PathParam(unit),
        }
    }
}
