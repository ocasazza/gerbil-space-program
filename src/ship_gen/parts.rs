use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::model::{generate_assembly_3d, ShipAssembly3d};
use super::names::generate_name;

/// Rarity tiers — gates which part pools are available
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// Which slot a part fills on the ship
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartSlot {
    Hull,
    Engine,
    PowerPlant,
    Cockpit,
    Wings,
    Stabilizer,
    Special,
}

/// Manufacturer identity — each has a gimmick and visual style
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Manufacturer {
    OrionDynamics,
    VoidForge,
    SolarCollective,
    RustBeltCustoms,
    DeepSpaceMiningCorp,
    XenotechFoundry,
}

impl Manufacturer {
    /// Returns the manufacturer's gimmick description (flavor text)
    pub fn gimmick(&self) -> &'static str {
        match self {
            Self::OrionDynamics => "Overclock: Boost all systems at the cost of heat damage",
            Self::VoidForge => "Adaptive Armor: Regenerates when not taking damage",
            Self::SolarCollective => "Solar Sails: Passive energy regeneration, fragile hull",
            Self::RustBeltCustoms => "Scrapheap: Extra part slot, parts may be damaged",
            Self::DeepSpaceMiningCorp => "Reinforced Hull: Ram damage bonus, no self-damage",
            Self::XenotechFoundry => "Alien Tech: Unpredictable stat ranges, unique effects",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::OrionDynamics => "Orion Dynamics",
            Self::VoidForge => "Void Forge",
            Self::SolarCollective => "Solar Collective",
            Self::RustBeltCustoms => "Rust Belt Customs",
            Self::DeepSpaceMiningCorp => "Deep Space Mining Corp",
            Self::XenotechFoundry => "Xenotech Foundry",
        }
    }
}

/// Stat modifiers that parts apply to the ship
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatModifiers {
    pub thrust: f32,
    pub mass: f32,
    pub armor: f32,
    pub heat_capacity: f32,
    pub sensor_range: f32,
    pub maneuverability: f32,
    pub cargo: f32,
    pub signature: f32,
    pub hardpoints: i32,
    pub energy_regen: f32,
}

impl Default for StatModifiers {
    fn default() -> Self {
        Self {
            thrust: 1.0,
            mass: 1.0,
            armor: 1.0,
            heat_capacity: 1.0,
            sensor_range: 1.0,
            maneuverability: 1.0,
            cargo: 1.0,
            signature: 1.0,
            hardpoints: 0,
            energy_regen: 1.0,
        }
    }
}

/// A single ship part with manufacturer, stats, and flavor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShipPart {
    pub slot: PartSlot,
    pub manufacturer: Manufacturer,
    pub rarity: Rarity,
    pub name: String,
    pub stats: StatModifiers,
    pub flavor_text: Option<String>,
}

/// A complete generated ship
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedShip {
    pub name: String,
    pub rarity: Rarity,
    pub manufacturer: Manufacturer,
    pub parts: Vec<ShipPart>,
    pub total_stats: StatModifiers,
    pub seed: u64,
    pub visual: ShipVisualBlueprint,
    pub assembly: ShipAssembly3d,
}

/// Geometry recipe shared by the Bevy renderer and panel preview. Individual
/// modules vary independently, while common proportions keep every result
/// readable as a flyable spacecraft.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ShipVisualBlueprint {
    pub archetype: u8,
    pub hull: u8,
    pub nose: u8,
    pub cockpit: u8,
    pub wings: u8,
    pub engines: u8,
    pub stabilizer: u8,
    pub tail: u8,
    pub armor: u8,
    pub utility: u8,
    pub decal: u8,
    pub special: u8,
    pub hull_width: f32,
    pub hull_length: f32,
    pub hull_sections: [f32; 4],
    pub section_count: u8,
    pub wing_span: f32,
    pub wing_chord: f32,
    pub wing_sweep: f32,
    pub wing_pairs: u8,
    pub engine_count: u8,
    pub engine_spread: f32,
    pub engine_length: f32,
    pub asymmetry: f32,
    pub palette: u8,
    pub wear: u8,
}

pub fn generate_part(
    rng: &mut impl Rng,
    slot: PartSlot,
    manufacturer: Manufacturer,
    rarity: Rarity,
) -> ShipPart {
    let mut stats = base_slot_stats(rng, slot, rarity);
    apply_manufacturer_bias(rng, &mut stats, manufacturer);

    let rarity_prefix = match rarity {
        Rarity::Common => "Standard",
        Rarity::Uncommon => "Tuned",
        Rarity::Rare => "Prototype",
        Rarity::Epic => "Ascendant",
        Rarity::Legendary => "Mythic",
    };
    let slot_name = match slot {
        PartSlot::Hull => "Hull",
        PartSlot::Engine => "Drive",
        PartSlot::PowerPlant => "Reactor",
        PartSlot::Cockpit => "Cockpit",
        PartSlot::Wings => "Wing Assembly",
        PartSlot::Stabilizer => "Stabilizer",
        PartSlot::Special => "Special Module",
    };

    let flavor_text = (rarity == Rarity::Legendary || slot == PartSlot::Special)
        .then(|| format!("{} — {}", manufacturer.name(), manufacturer.gimmick()));

    ShipPart {
        slot,
        manufacturer,
        rarity,
        name: format!("{} {} {}", manufacturer.name(), rarity_prefix, slot_name),
        stats,
        flavor_text,
    }
}

pub fn generate_ship(rng: &mut impl Rng, rarity: Rarity) -> GeneratedShip {
    let seed = rng.gen::<u64>();
    let manufacturer = pick_manufacturer(rng);
    let mut slots = vec![
        PartSlot::Hull,
        PartSlot::Engine,
        PartSlot::PowerPlant,
        PartSlot::Cockpit,
        PartSlot::Wings,
        PartSlot::Stabilizer,
    ];

    if matches!(rarity, Rarity::Rare | Rarity::Epic | Rarity::Legendary) {
        slots.push(PartSlot::Special);
    }
    if manufacturer == Manufacturer::RustBeltCustoms && rng.gen_bool(0.35) {
        slots.push(extra_rust_belt_slot(rng));
    }

    let parts: Vec<_> = slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            let part_rarity = pick_part_rarity(rng, rarity, index);
            // A dominant chassis manufacturer supplies most modules, but the
            // remaining parts can come from other yards. This is the core of
            // the Borderlands-like combinatorial design space.
            let part_manufacturer = if rng.gen_bool(0.68) {
                manufacturer
            } else {
                pick_manufacturer(rng)
            };
            generate_part(rng, slot, part_manufacturer, part_rarity)
        })
        .collect();
    let total_stats = combine_stats(&parts);
    let visual = generate_visual_blueprint(seed, rarity, &parts, &total_stats);
    let assembly = generate_assembly_3d(seed, &visual, &parts, &total_stats, manufacturer);
    let (name, flavor_text) = generate_name(rng, rarity, manufacturer);
    let parts = if let Some(flavor_text) = flavor_text {
        parts
            .into_iter()
            .map(|mut part| {
                if part.slot == PartSlot::Special || part.rarity == Rarity::Legendary {
                    part.flavor_text = Some(flavor_text.clone());
                }
                part
            })
            .collect()
    } else {
        parts
    };

    GeneratedShip {
        name,
        rarity,
        manufacturer,
        parts,
        total_stats,
        seed,
        visual,
        assembly,
    }
}

fn generate_visual_blueprint(
    seed: u64,
    rarity: Rarity,
    parts: &[ShipPart],
    _stats: &StatModifiers,
) -> ShipVisualBlueprint {
    let mut visual_rng = rand::rngs::StdRng::seed_from_u64(seed ^ 0xD351_6E5A_91C4_77B3);
    let variant = |slot: PartSlot, salt: u8| {
        let manufacturer = parts
            .iter()
            .find(|part| part.slot == slot)
            .map(|part| manufacturer_index(part.manufacturer))
            .unwrap_or(0);
        // Multiplying by two deliberately shifts the geometry family before
        // the renderer's modulo selection, so both manufacturer and variant
        // influence the resulting silhouette.
        manufacturer * 2 + salt
    };
    let quality = match rarity {
        Rarity::Common => 0.0,
        Rarity::Uncommon => 0.08,
        Rarity::Rare => 0.16,
        Rarity::Epic => 0.24,
        Rarity::Legendary => 0.34,
    };
    let hull_yard = parts
        .iter()
        .find(|part| part.slot == PartSlot::Hull)
        .map(|part| manufacturer_index(part.manufacturer))
        .unwrap_or(0);
    // Topology is selected before cosmetic detail. Rarity and stats may tune
    // modules, but they never collapse every vehicle back into an airplane.
    let archetype = visual_rng.gen_range(0..16);
    let hull_width = match archetype {
        0 | 6 => visual_rng.gen_range(9.0..15.0), // needle / spine
        2 | 3 | 4 | 12 => visual_rng.gen_range(24.0..38.0), // disc / pod / ring / habitat
        8 | 9 | 15 => visual_rng.gen_range(23.0..36.0), // barge / modules / monitor
        10 | 13 | 14 => visual_rng.gen_range(18.0..31.0), // rig / crescent / swarm
        _ => visual_rng.gen_range(13.0..21.0),
    } * (1.0 + quality * 0.25);
    let hull_length = match archetype {
        0 | 6 => visual_rng.gen_range(42.0..66.0),
        2 | 3 | 4 | 12 => visual_rng.gen_range(22.0..36.0),
        8 | 15 => visual_rng.gen_range(20.0..35.0),
        _ => visual_rng.gen_range(30.0..48.0),
    } * (1.0 + quality * 0.18);
    let section_count = match rarity {
        Rarity::Common => visual_rng.gen_range(2..=3),
        Rarity::Uncommon | Rarity::Rare => visual_rng.gen_range(3..=4),
        Rarity::Epic | Rarity::Legendary => 4,
    };
    let mut hull_sections = [hull_width; 4];
    for (index, width) in hull_sections.iter_mut().enumerate() {
        let longitudinal_taper = 1.0 - index as f32 * 0.09;
        *width = hull_width * longitudinal_taper * visual_rng.gen_range(0.78..1.16);
    }
    let wing_pairs = if matches!(archetype, 1 | 7) {
        if matches!(rarity, Rarity::Epic | Rarity::Legendary) && visual_rng.gen_bool(0.48) {
            2
        } else {
            1
        }
    } else {
        0
    };
    ShipVisualBlueprint {
        archetype,
        hull: variant(PartSlot::Hull, visual_rng.gen_range(0..3)),
        nose: variant(PartSlot::Hull, visual_rng.gen_range(3..9)),
        cockpit: variant(PartSlot::Cockpit, visual_rng.gen_range(0..3)),
        wings: variant(PartSlot::Wings, visual_rng.gen_range(0..3)),
        engines: variant(PartSlot::Engine, visual_rng.gen_range(0..3)),
        stabilizer: variant(PartSlot::Stabilizer, visual_rng.gen_range(0..3)),
        tail: variant(PartSlot::Stabilizer, visual_rng.gen_range(3..9)),
        armor: variant(PartSlot::Hull, visual_rng.gen_range(0..12)),
        utility: variant(PartSlot::PowerPlant, visual_rng.gen_range(0..12)),
        decal: visual_rng.gen_range(0..16),
        special: parts
            .iter()
            .find(|part| part.slot == PartSlot::Special)
            .map(|part| manufacturer_index(part.manufacturer) * 2 + visual_rng.gen_range(1..3))
            .unwrap_or(0),
        hull_width,
        hull_length,
        hull_sections,
        section_count,
        wing_span: visual_rng.gen_range(hull_width * 1.35..hull_width * 3.2)
            * (1.0 + quality * 0.25),
        wing_chord: visual_rng.gen_range(hull_length * 0.20..hull_length * 0.48),
        wing_sweep: visual_rng.gen_range(-0.7..0.55),
        wing_pairs,
        engine_count: match archetype {
            0 | 1 => visual_rng.gen_range(2..=4),
            8 | 9 | 15 => visual_rng.gen_range(2..=5),
            11 => 1,
            _ => visual_rng.gen_range(1..=4),
        },
        engine_spread: visual_rng.gen_range(0.45..1.45),
        engine_length: visual_rng.gen_range(6.0..16.0) * (1.0 + quality * 0.2),
        asymmetry: if matches!(hull_yard, 3 | 5) {
            visual_rng.gen_range(-0.16..0.16)
        } else {
            0.0
        },
        palette: hull_yard * 3 + visual_rng.gen_range(0..3),
        wear: if hull_yard == 3 {
            visual_rng.gen_range(2..=5)
        } else {
            visual_rng.gen_range(0..=3)
        },
    }
}

fn manufacturer_index(manufacturer: Manufacturer) -> u8 {
    match manufacturer {
        Manufacturer::OrionDynamics => 0,
        Manufacturer::VoidForge => 1,
        Manufacturer::SolarCollective => 2,
        Manufacturer::RustBeltCustoms => 3,
        Manufacturer::DeepSpaceMiningCorp => 4,
        Manufacturer::XenotechFoundry => 5,
    }
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_fleet_has_high_visual_cardinality_and_coherent_geometry() {
        let mut signatures = HashSet::new();
        let mut mixed_yard_designs = 0;
        for seed in 0..128_u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let ship = generate_ship(&mut rng, Rarity::Rare);
            let visual = ship.visual;
            signatures.insert((
                visual.archetype,
                visual.hull,
                visual.nose,
                visual.cockpit,
                visual.wings,
                visual.engines,
                visual.engine_count,
                visual.section_count,
                visual.hull_width.to_bits(),
                visual.hull_length.to_bits(),
                visual.wing_span.to_bits(),
            ));
            let yards: HashSet<_> = ship.parts.iter().map(|part| part.manufacturer).collect();
            mixed_yard_designs += usize::from(yards.len() > 1);

            assert!((2..=4).contains(&visual.section_count));
            assert!((1..=5).contains(&visual.engine_count));
            assert!(visual.wing_pairs <= 2);
            assert!(visual.hull_width > 0.0 && visual.hull_length > 0.0);
            assert!(visual.wing_span > 0.0);
            assert!(visual.engine_length > 0.0);
        }
        assert!(
            signatures.len() >= 124,
            "visual recipes should very rarely collide"
        );
        assert!(
            mixed_yard_designs >= 80,
            "most ships should combine multiple yards"
        );
    }

    #[test]
    fn blueprint_is_deterministic_for_a_seeded_generator() {
        let mut first = rand::rngs::StdRng::seed_from_u64(42);
        let mut second = rand::rngs::StdRng::seed_from_u64(42);
        let a = generate_ship(&mut first, Rarity::Legendary);
        let b = generate_ship(&mut second, Rarity::Legendary);
        assert_eq!(a.seed, b.seed);
        assert_eq!(a.visual.hull_sections, b.visual.hull_sections);
        assert_eq!(a.visual.palette, b.visual.palette);
        assert_eq!(a.parts.len(), b.parts.len());
    }
}

fn base_slot_stats(rng: &mut impl Rng, slot: PartSlot, rarity: Rarity) -> StatModifiers {
    let quality = match rarity {
        Rarity::Common => 1.0,
        Rarity::Uncommon => 1.12,
        Rarity::Rare => 1.25,
        Rarity::Epic => 1.42,
        Rarity::Legendary => 1.65,
    };
    let jitter = |rng: &mut dyn rand::RngCore, spread: f32| 1.0 + rng.gen_range(-spread..spread);
    let mut stats = StatModifiers::default();

    match slot {
        PartSlot::Hull => {
            stats.armor = quality * jitter(rng, 0.12);
            stats.mass = (1.0 + (quality - 1.0) * 0.35) * jitter(rng, 0.08);
            stats.hardpoints = rng.gen_range(1..=3);
        }
        PartSlot::Engine => {
            stats.thrust = quality * jitter(rng, 0.15);
            stats.signature = (1.0 + (quality - 1.0) * 0.2) * jitter(rng, 0.08);
        }
        PartSlot::PowerPlant => {
            stats.energy_regen = quality * jitter(rng, 0.13);
            stats.heat_capacity = (1.0 + (quality - 1.0) * 0.6) * jitter(rng, 0.10);
        }
        PartSlot::Cockpit => {
            stats.sensor_range = quality * jitter(rng, 0.12);
            stats.signature = (1.0 - (quality - 1.0) * 0.1).max(0.65) * jitter(rng, 0.06);
        }
        PartSlot::Wings => {
            stats.maneuverability = quality * jitter(rng, 0.14);
            stats.hardpoints = rng.gen_range(0..=2);
        }
        PartSlot::Stabilizer => {
            stats.maneuverability = (1.0 + (quality - 1.0) * 0.55) * jitter(rng, 0.10);
            stats.cargo = quality * jitter(rng, 0.12);
        }
        PartSlot::Special => {
            stats.thrust = (1.0 + (quality - 1.0) * 0.35) * jitter(rng, 0.16);
            stats.armor = (1.0 + (quality - 1.0) * 0.35) * jitter(rng, 0.16);
            stats.energy_regen = (1.0 + (quality - 1.0) * 0.45) * jitter(rng, 0.16);
            stats.hardpoints = rng.gen_range(0..=1);
        }
    }
    stats
}

fn apply_manufacturer_bias(
    rng: &mut impl Rng,
    stats: &mut StatModifiers,
    manufacturer: Manufacturer,
) {
    match manufacturer {
        Manufacturer::OrionDynamics => {
            stats.thrust *= 1.22;
            stats.maneuverability *= 1.08;
            stats.heat_capacity *= 0.88;
            stats.signature *= 1.12;
        }
        Manufacturer::VoidForge => {
            stats.armor *= 1.24;
            stats.energy_regen *= 1.08;
            stats.signature *= 0.9;
            stats.mass *= 1.08;
        }
        Manufacturer::SolarCollective => {
            stats.energy_regen *= 1.35;
            stats.sensor_range *= 1.12;
            stats.armor *= 0.82;
            stats.mass *= 1.15;
        }
        Manufacturer::RustBeltCustoms => {
            let damage = rng.gen_range(0.78..1.28);
            stats.thrust *= damage;
            stats.armor *= rng.gen_range(0.82..1.32);
            stats.hardpoints += rng.gen_range(0..=1);
            stats.signature *= 1.18;
        }
        Manufacturer::DeepSpaceMiningCorp => {
            stats.armor *= 1.35;
            stats.cargo *= 1.3;
            stats.mass *= 1.22;
            stats.maneuverability *= 0.86;
        }
        Manufacturer::XenotechFoundry => {
            stats.thrust *= rng.gen_range(0.85..1.45);
            stats.armor *= rng.gen_range(0.85..1.45);
            stats.energy_regen *= rng.gen_range(0.85..1.55);
            stats.signature *= rng.gen_range(0.65..1.25);
        }
    }
}

fn combine_stats(parts: &[ShipPart]) -> StatModifiers {
    parts
        .iter()
        .fold(StatModifiers::default(), |mut total, part| {
            total.thrust *= part.stats.thrust;
            total.mass *= part.stats.mass;
            total.armor *= part.stats.armor;
            total.heat_capacity *= part.stats.heat_capacity;
            total.sensor_range *= part.stats.sensor_range;
            total.maneuverability *= part.stats.maneuverability;
            total.cargo *= part.stats.cargo;
            total.signature *= part.stats.signature;
            total.hardpoints += part.stats.hardpoints;
            total.energy_regen *= part.stats.energy_regen;
            total
        })
}

fn pick_manufacturer(rng: &mut impl Rng) -> Manufacturer {
    match rng.gen_range(0..100) {
        0..=24 => Manufacturer::OrionDynamics,
        25..=45 => Manufacturer::VoidForge,
        46..=64 => Manufacturer::SolarCollective,
        65..=80 => Manufacturer::RustBeltCustoms,
        81..=93 => Manufacturer::DeepSpaceMiningCorp,
        _ => Manufacturer::XenotechFoundry,
    }
}

fn pick_part_rarity(rng: &mut impl Rng, ship_rarity: Rarity, index: usize) -> Rarity {
    match ship_rarity {
        Rarity::Common => Rarity::Common,
        Rarity::Uncommon if index == 0 => Rarity::Uncommon,
        Rarity::Uncommon => {
            if rng.gen_bool(0.2) {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        Rarity::Rare => match rng.gen_range(0..100) {
            0..=14 => Rarity::Rare,
            15..=54 => Rarity::Uncommon,
            _ => Rarity::Common,
        },
        Rarity::Epic => match rng.gen_range(0..100) {
            0..=19 => Rarity::Epic,
            20..=54 => Rarity::Rare,
            55..=84 => Rarity::Uncommon,
            _ => Rarity::Common,
        },
        Rarity::Legendary => match rng.gen_range(0..100) {
            0..=14 => Rarity::Legendary,
            15..=44 => Rarity::Epic,
            45..=74 => Rarity::Rare,
            _ => Rarity::Uncommon,
        },
    }
}

fn extra_rust_belt_slot(rng: &mut impl Rng) -> PartSlot {
    match rng.gen_range(0..6) {
        0 => PartSlot::Engine,
        1 => PartSlot::PowerPlant,
        2 => PartSlot::Wings,
        3 => PartSlot::Stabilizer,
        4 => PartSlot::Cockpit,
        _ => PartSlot::Hull,
    }
}
