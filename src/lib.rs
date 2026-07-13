#![allow(clippy::type_complexity)]

mod actions;
mod ai;
mod astronomy;
mod audio;
mod background_grid;
mod game;
mod game_over;
mod gravity_field;
mod loading;
mod maneuver;
mod menu;
mod minimap;
mod pause;
mod player;
mod relativity;
mod settings;
pub mod ship_gen;
mod terrain;
mod trajectory_gpu;
#[cfg(target_arch = "wasm32")]
mod ui;

use crate::actions::ActionsPlugin;
use crate::ai::AiShipPlugin;
use crate::audio::InternalAudioPlugin;
use crate::background_grid::BackgroundGridPlugin;
use crate::game::GamePlugin as InternalGamePlugin;
use crate::game_over::GameOverPlugin;
use crate::gravity_field::GravityFieldPlugin;
use crate::loading::LoadingPlugin;
use crate::maneuver::ManeuverPlugin;
use crate::menu::MenuPlugin;
use crate::minimap::MinimapPlugin;
use crate::pause::PausePlugin;
use crate::settings::SettingsPlugin;
use crate::ship_gen::ShipGenPlugin;

use bevy::app::App;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
#[cfg(debug_assertions)]
use bevy::diagnostic::LogDiagnosticsPlugin;
use bevy::prelude::*;

// This example game uses States to separate logic
// See https://bevy-cheatbook.github.io/programming/states.html
// Or https://github.com/bevyengine/bevy/blob/main/examples/ecs/state.rs
#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub(crate) enum GameState {
    // During the loading State the LoadingPlugin will load our assets
    #[default]
    Loading,
    // Here the menu is drawn and waiting for player interaction
    Menu,
    // During this State the actual game logic is executed
    Playing,
    // Game is paused
    Paused,
    // Game over screen
    GameOver,
    // Settings menu
    Settings,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>().add_plugins((
            LoadingPlugin,
            MenuPlugin,
            ActionsPlugin,
            InternalAudioPlugin,
            AiShipPlugin,
            BackgroundGridPlugin,
            ManeuverPlugin,
            InternalGamePlugin,
            GravityFieldPlugin,
            MinimapPlugin,
            PausePlugin,
            SettingsPlugin,
            GameOverPlugin,
            ShipGenPlugin,
        ));

        #[cfg(target_arch = "wasm32")]
        app.add_plugins(ui::WebUiBridgePlugin);

        // Keep the inexpensive rolling frame-time diagnostic in production so
        // the panel-kit telemetry and adaptive quality loop see real runtime
        // performance instead of debug-only estimates.
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());

        #[cfg(debug_assertions)]
        {
            app.add_plugins(LogDiagnosticsPlugin::default());
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn launch_web_ui() {
    ui::launch();
}
