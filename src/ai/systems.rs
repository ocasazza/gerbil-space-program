use bevy::prelude::*;
use rand::Rng;

use crate::ai::aggression::{Aggression, BehaviorMode};
use crate::ai::state::{AiCooldown, AiPersonality, AiShip, AiState};
use crate::game::Lander;
use crate::player::Player;
use crate::ship_gen::parts::{generate_ship, Rarity};
use crate::ship_gen::visuals::ShipVisual;

/// Main AI decision system — runs every frame for each AI ship
pub fn ai_decision_system(
    time: Res<Time>,
    mut ai_ships: Query<
        (
            &Transform,
            &Aggression,
            &mut AiState,
            &mut AiCooldown,
            &AiPersonality,
        ),
        With<AiShip>,
    >,
    player: Query<(Entity, &Transform), With<Player>>,
) {
    let Ok((player_entity, player_transform)) = player.single() else {
        return;
    };
    let dt = time.delta_secs();

    for (transform, aggression, mut state, mut cooldown, personality) in &mut ai_ships {
        cooldown.0 -= dt;
        if cooldown.0 > 0.0 {
            continue;
        }

        let decision_interval =
            (0.25 + personality.caution * 0.1 - personality.persistence * 0.05).clamp(0.1, 0.5);
        cooldown.0 = decision_interval;

        let distance = transform.translation.distance(player_transform.translation);
        let mode = aggression.mode();

        let curious_distance =
            aggression.approach_distance() * (1.0 + personality.curiosity * 0.25);
        let hostile_attack_range = 150.0 * (1.0 - personality.caution * 0.25);
        let lethal_attack_range = 80.0 * (1.0 - personality.caution * 0.15);

        let new_state = match mode {
            BehaviorMode::Docile => {
                if distance < aggression.approach_distance() {
                    AiState::Evading {
                        threat: player_entity,
                    }
                } else {
                    AiState::Idle
                }
            }
            BehaviorMode::Curious => {
                if distance > curious_distance {
                    AiState::Observing {
                        target: player_entity,
                        distance: curious_distance,
                    }
                } else {
                    AiState::Idle
                }
            }
            BehaviorMode::Territorial => {
                if distance < aggression.approach_distance() {
                    AiState::Pursuing {
                        target: player_entity,
                    }
                } else {
                    AiState::Idle
                }
            }
            BehaviorMode::Hostile => {
                if distance < hostile_attack_range {
                    AiState::Attacking {
                        target: player_entity,
                        attack_cooldown: 0.0,
                    }
                } else {
                    AiState::Pursuing {
                        target: player_entity,
                    }
                }
            }
            BehaviorMode::Lethal => {
                // Always pursuing, attacks when close
                if distance < lethal_attack_range {
                    AiState::Attacking {
                        target: player_entity,
                        attack_cooldown: 0.0,
                    }
                } else {
                    AiState::Pursuing {
                        target: player_entity,
                    }
                }
            }
        };

        *state = new_state;
    }
}

/// AI movement system — executes the current state
pub fn ai_movement_system(
    mut ai_ships: Query<(&Transform, &Aggression, &AiState, &mut Lander), With<AiShip>>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };

    for (transform, aggression, state, mut lander) in &mut ai_ships {
        let player_pos = player_transform.translation.truncate();
        let ai_pos = transform.translation.truncate();
        let to_player = player_pos - ai_pos;
        let distance = to_player.length();
        let direction = to_player.normalize_or_zero();

        // Get AI's forward direction.
        let forward = (transform.rotation * Vec3::Y).truncate();

        // Reset thrusters before setting intent for this frame.
        lander.main_thrust = 0.0;
        lander.left_thrust = 0.0;
        lander.right_thrust = 0.0;
        lander.reverse_thrust = 0.0;
        lander.angular_thrust = 0.0;

        match state {
            AiState::Idle => {
                // Gentle drift with tiny random angular thrust; gravity does the rest.
                lander.angular_thrust = (rand::random::<f32>() - 0.5) * 0.1;
            }
            AiState::Observing {
                target: _,
                distance: target_dist,
            } => {
                if distance > *target_dist {
                    // Move toward player to get within observation range.
                    steer_toward(&mut lander, forward, direction);
                    lander.main_thrust = 30.0 * aggression.pursuit_aggressiveness();
                } else if distance < *target_dist * 0.8 {
                    // Back off if too close.
                    steer_toward(&mut lander, forward, -direction);
                    lander.main_thrust = 20.0;
                }
            }
            AiState::Pursuing { target: _ } => {
                steer_toward(&mut lander, forward, direction);
                lander.main_thrust = 80.0 * aggression.pursuit_aggressiveness();
            }
            AiState::Attacking {
                target: _,
                attack_cooldown: _,
            } => {
                // Boost toward player for ramming.
                steer_toward(&mut lander, forward, direction);
                lander.main_thrust = 120.0 * aggression.pursuit_aggressiveness();
            }
            AiState::Evading { threat: _ } => {
                let away = -direction;
                steer_toward(&mut lander, forward, away);
                lander.main_thrust = 100.0;
            }
            AiState::Fleeing => {
                let away = -direction;
                steer_toward(&mut lander, forward, away);
                lander.main_thrust = 150.0;
            }
            AiState::Patrolling { .. } => {}
        }
    }
}

/// Steer the AI ship toward a target direction using angular thrust.
pub fn steer_toward(lander: &mut Lander, forward: Vec2, target_dir: Vec2) {
    let cross = forward.x * target_dir.y - forward.y * target_dir.x;
    // cross > 0 means target is to the left, need to rotate counter-clockwise.
    // cross < 0 means target is to the right, need to rotate clockwise.
    let max_angular = 2.0;
    lander.angular_thrust = (cross * 5.0).clamp(-max_angular, max_angular);
}

/// Spawns AI ships for testing — press F6 to spawn a random AI ship.
/// Aggression is tied to ship rarity: rarer ships are more aggressive.
pub fn debug_spawn_ai(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    player: Query<&Transform, With<Player>>,
) {
    if !keyboard.just_pressed(KeyCode::F6) {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };

    let mut rng = rand::thread_rng();

    // Generate a ship — rarity determines aggression
    let rarity = match rng.gen_range(0..100) {
        0..=40 => Rarity::Common,
        41..=70 => Rarity::Uncommon,
        71..=88 => Rarity::Rare,
        89..=97 => Rarity::Epic,
        _ => Rarity::Legendary,
    };
    let ship = generate_ship(&mut rng, rarity);

    // Map rarity to aggression: rarer = more dangerous
    let aggression = Aggression::new(rarity_to_aggression(rarity));
    let personality = AiPersonality {
        caution: rng.gen(),
        curiosity: rng.gen(),
        persistence: rng.gen(),
    };

    // Spawn near player but at a random offset
    let angle = rng.gen::<f32>() * std::f32::consts::TAU;
    let distance = 300.0 + rng.gen::<f32>() * 400.0;
    let spawn_pos =
        player_transform.translation.truncate() + Vec2::new(angle.cos(), angle.sin()) * distance;

    info!(
        "Spawning AI ship: {} ({:?}), aggression={:.2}, mode={:?}",
        ship.name,
        ship.rarity,
        aggression.0,
        aggression.mode()
    );

    commands.spawn((
        AiShip,
        aggression,
        AiState::default(),
        AiCooldown::default(),
        personality,
        Transform::from_translation(spawn_pos.extend(0.0)),
        Lander {
            velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            main_thrust: 0.0,
            left_thrust: 0.0,
            right_thrust: 0.0,
            reverse_thrust: 0.0,
            angular_thrust: 0.0,
            mass: 1.0,
            thrust_scale: ship.total_stats.thrust.clamp(0.45, 3.5),
            maneuverability: ship.total_stats.maneuverability.clamp(0.4, 3.0),
        },
        ShipVisual,
        ship,
    ));
}

/// Map ship rarity to an aggression level.
/// Common ships are docile/curious, Legendary ships are lethal.
fn rarity_to_aggression(rarity: Rarity) -> f32 {
    match rarity {
        Rarity::Common => 0.05 + rand::random::<f32>() * 0.25, // 0.05–0.30 (Docile/Curious)
        Rarity::Uncommon => 0.15 + rand::random::<f32>() * 0.35, // 0.15–0.50 (Curious/Territorial)
        Rarity::Rare => 0.35 + rand::random::<f32>() * 0.40,   // 0.35–0.75 (Territorial/Hostile)
        Rarity::Epic => 0.55 + rand::random::<f32>() * 0.35,   // 0.55–0.90 (Hostile/Lethal)
        Rarity::Legendary => 0.75 + rand::random::<f32>() * 0.25, // 0.75–1.00 (Lethal)
    }
}
