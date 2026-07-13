use bevy::prelude::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

use super::parts::{generate_ship, GeneratedShip, Rarity};
use super::visuals::{draw_ship_visuals, spawn_ship_visual};
use crate::GameState;

pub struct ShipGenPlugin;

impl Plugin for ShipGenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShipGenConfig>().add_systems(
            Update,
            (debug_spawn_ship, draw_ship_visuals).run_if(in_state(GameState::Playing)),
        );
    }
}

#[derive(Resource)]
pub struct ShipGenConfig {
    pub seed: u64,
    pub selected: GeneratedShip,
}

impl Default for ShipGenConfig {
    fn default() -> Self {
        let seed = 0x51A5_EED5;
        let mut rng = StdRng::seed_from_u64(seed);
        Self {
            seed: rng.gen(),
            selected: generate_ship(&mut rng, Rarity::Common),
        }
    }
}

impl ShipGenConfig {
    /// Reroll the currently selected design using the shared Rust generator.
    pub fn generate(&mut self, rarity: Rarity) -> &GeneratedShip {
        let mut rng = StdRng::seed_from_u64(self.seed);
        self.selected = generate_ship(&mut rng, rarity);
        self.seed = rng.gen();
        &self.selected
    }
}

#[derive(Component, Clone, Debug)]
pub struct GeneratedShipComponent(pub GeneratedShip);

fn debug_spawn_ship(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<ShipGenConfig>,
) {
    if !keys.just_pressed(KeyCode::F5) {
        return;
    }

    let mut rng = StdRng::seed_from_u64(config.seed);
    let rarity = random_rarity(&mut rng);
    let ship = generate_ship(&mut rng, rarity);
    config.selected = ship.clone();
    info!("Generated ship: {ship:#?}");
    let spawn_x = rng.gen_range(-320.0..=320.0);
    let spawn_y = rng.gen_range(-180.0..=180.0);
    spawn_ship_visual(&mut commands, &ship, Vec2::new(spawn_x, spawn_y));
    config.seed = rng.gen();
}

fn random_rarity(rng: &mut impl Rng) -> Rarity {
    match rng.gen_range(0..100) {
        0..=49 => Rarity::Common,
        50..=74 => Rarity::Uncommon,
        75..=89 => Rarity::Rare,
        90..=97 => Rarity::Epic,
        _ => Rarity::Legendary,
    }
}
