//! Diagnostic visualization of the instantaneous inertial gravitational field.

use crate::{game::SimulationSettings, GameState};
use bevy::prelude::*;

pub struct GravityFieldPlugin;

impl Plugin for GravityFieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            toggle_gravity_field.run_if(in_state(GameState::Playing)),
        );
    }
}

fn toggle_gravity_field(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<SimulationSettings>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        settings.show_gravity_field = !settings.show_gravity_field;
    }
}
