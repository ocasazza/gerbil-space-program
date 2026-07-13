use bevy::prelude::*;

/// Aggression level from 0.0 (docile) to 1.0 (lethal)
#[derive(Component, Clone, Debug)]
pub struct Aggression(pub f32);

/// Behavior mode derived from aggression level
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorMode {
    Docile,      // 0.0-0.2: avoids player, maintains safe distance
    Curious,     // 0.2-0.4: approaches to observe, matches orbit
    Territorial, // 0.4-0.6: intercepts trajectory, warns
    Hostile,     // 0.6-0.8: actively pursues, attempts to ram
    Lethal,      // 0.8-1.0: optimal intercept, predicts evasion
}

impl Aggression {
    pub fn new(level: f32) -> Self {
        Self(level.clamp(0.0, 1.0))
    }

    pub fn mode(&self) -> BehaviorMode {
        match self.0 {
            x if x < 0.2 => BehaviorMode::Docile,
            x if x < 0.4 => BehaviorMode::Curious,
            x if x < 0.5 => BehaviorMode::Territorial,
            x if x < 0.8 => BehaviorMode::Hostile,
            _ => BehaviorMode::Lethal,
        }
    }

    /// How close the AI will approach the player (in world units)
    pub fn approach_distance(&self) -> f32 {
        match self.mode() {
            BehaviorMode::Docile => 500.0,
            BehaviorMode::Curious => 200.0,
            BehaviorMode::Territorial => 100.0,
            BehaviorMode::Hostile => 30.0,
            BehaviorMode::Lethal => 0.0, // rams
        }
    }

    /// Speed multiplier for pursuit
    pub fn pursuit_aggressiveness(&self) -> f32 {
        0.5 + self.0 * 1.5 // 0.5x to 2.0x
    }
}
