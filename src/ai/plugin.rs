use bevy::prelude::*;

use crate::ai::systems::{ai_decision_system, ai_movement_system, debug_spawn_ai};

pub struct AiShipPlugin;

impl Plugin for AiShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ai_decision_system, ai_movement_system, debug_spawn_ai)
                .chain()
                .run_if(in_state(crate::GameState::Playing)),
        );
    }
}
