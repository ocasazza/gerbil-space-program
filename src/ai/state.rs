use bevy::prelude::*;

/// AI-controlled ship marker
#[derive(Component, Default)]
pub struct AiShip;

/// Current AI behavioral state
#[derive(Component, Clone, Debug)]
pub enum AiState {
    Idle,
    Patrolling {
        waypoints: Vec<Vec2>,
        current: usize,
    },
    Observing {
        target: Entity,
        distance: f32,
    },
    Pursuing {
        target: Entity,
    },
    Attacking {
        target: Entity,
        attack_cooldown: f32,
    },
    Evading {
        threat: Entity,
    },
    Fleeing,
}

impl Default for AiState {
    fn default() -> Self {
        Self::Idle
    }
}

/// AI decision cooldown — prevents thrashing
#[derive(Component)]
pub struct AiCooldown(pub f32);

impl Default for AiCooldown {
    fn default() -> Self {
        Self(0.0)
    }
}

/// AI personality — biases behavior within aggression band
#[derive(Component, Clone, Debug)]
pub struct AiPersonality {
    pub caution: f32,     // 0.0 = reckless, 1.0 = paranoid
    pub curiosity: f32,   // 0.0 = indifferent, 1.0 = investigates everything
    pub persistence: f32, // 0.0 = gives up easily, 1.0 = never stops
}

impl Default for AiPersonality {
    fn default() -> Self {
        Self {
            caution: 0.5,
            curiosity: 0.5,
            persistence: 0.5,
        }
    }
}
