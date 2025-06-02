#![allow(clippy::type_complexity)]

mod actions;
mod audio;
mod game;
mod game_over;
mod loading;
mod menu;
mod pause;
mod settings;
mod terrain;

use crate::actions::ActionsPlugin;
use crate::audio::InternalAudioPlugin;
use crate::game::GamePlugin as InternalGamePlugin;
use crate::game_over::GameOverPlugin;
use crate::loading::LoadingPlugin;
use crate::menu::MenuPlugin;
use crate::pause::PausePlugin;
use crate::settings::SettingsPlugin;

use bevy::app::App;
#[cfg(debug_assertions)]
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;

// This example game uses States to separate logic
// See https://bevy-cheatbook.github.io/programming/states.html
// Or https://github.com/bevyengine/bevy/blob/main/examples/ecs/state.rs
#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
enum GameState {
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

            InternalGamePlugin,
            PausePlugin,
            SettingsPlugin,
            GameOverPlugin,
        ));

        #[cfg(debug_assertions)]
        {
            app.add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                LogDiagnosticsPlugin::default(),
            ));
        }
    }
}
