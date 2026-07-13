use rand::Rng;

use super::parts::{Manufacturer, Rarity};

/// Generates a ship name based on rarity and manufacturer
/// Common/Uncommon: "[Adjective] [Noun]"
/// Rare/Epic: "[Adjective] [Noun] [Suffix]"
/// Legendary: Unique name with flavor text
pub fn generate_name(
    rng: &mut impl Rng,
    rarity: Rarity,
    manufacturer: Manufacturer,
) -> (String, Option<String>) {
    if rarity == Rarity::Legendary {
        let (name, flavor) = LEGENDARY_NAMES[rng.gen_range(0..LEGENDARY_NAMES.len())];
        return (name.to_string(), Some(flavor.to_string()));
    }

    let adjective = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = manufacturer_noun(rng, manufacturer);
    if matches!(rarity, Rarity::Rare | Rarity::Epic) {
        let suffix = SUFFIXES[rng.gen_range(0..SUFFIXES.len())];
        (format!("{adjective} {noun} {suffix}"), None)
    } else {
        (format!("{adjective} {noun}"), None)
    }
}

const ADJECTIVES: &[&str] = &[
    "Abyssal", "Ancient", "Astral", "Burning", "Crimson", "Distant", "Drifting", "Eclipsed",
    "Feral", "Golden", "Hollow", "Ionized", "Jade", "Luminous", "Nebular", "Obsidian", "Phantom",
    "Quantum", "Ragged", "Silent", "Solar", "Starlit", "Umbral", "Vagrant", "Voidborn", "Wild",
];

const NOUNS: &[&str] = &[
    "Aegis", "Arrow", "Bastion", "Comet", "Corsair", "Dagger", "Drake", "Falcon", "Firefly",
    "Hammer", "Harrier", "Horizon", "Javelin", "Kestrel", "Lancer", "Manta", "Nomad", "Pioneer",
    "Raptor", "Revenant", "Skiff", "Sparrow", "Tempest", "Valkyrie", "Warden", "Wayfarer",
];

const SUFFIXES: &[&str] = &[
    "EX",
    "Mk II",
    "Prime",
    "Zero",
    "Vector",
    "Ascendant",
    "Black",
    "Redline",
    "Omega",
    "Longshot",
    "Vanguard",
    "Ghost",
    "Overdrive",
    "Reliquary",
];

const LEGENDARY_NAMES: &[(&str, &str)] = &[
    (
        "Event Horizon",
        "Where we're going, we won't need eyes to see.",
    ),
    ("The Long Goodbye", "It leaves before the battle starts."),
    ("Saint of Rust", "Every scar is a scripture."),
    (
        "Black Sun Rising",
        "Dawn arrives wearing an executioner's mask.",
    ),
    ("Mercy of the Void", "It only spares the ones who run."),
    ("Last Argument", "Diplomacy, chambered and loaded."),
    ("Grin of the Wolf", "Predators do not ask permission."),
    ("Cathedral of Sparks", "Pray the reactor keeps listening."),
    ("Pale Horse", "A quiet shape at the end of the scopes."),
    ("The Unblinking Eye", "It saw you before you were born."),
    ("Dividend of Ash", "Profit measured in wreckage."),
    ("Yesterday's Knife", "Old wounds cut deepest in hyperspace."),
];

fn manufacturer_noun(rng: &mut impl Rng, manufacturer: Manufacturer) -> &'static str {
    let branded = match manufacturer {
        Manufacturer::OrionDynamics => &["Vector", "Redline", "Comet", "Lancer"][..],
        Manufacturer::VoidForge => &["Wraith", "Revenant", "Aegis", "Horizon"][..],
        Manufacturer::SolarCollective => &["Heliostat", "Sparrow", "Firefly", "Sail"][..],
        Manufacturer::RustBeltCustoms => &["Rattletrap", "Hammer", "Skiff", "Nomad"][..],
        Manufacturer::DeepSpaceMiningCorp => &["Bastion", "Warden", "Drill", "Mule"][..],
        Manufacturer::XenotechFoundry => &["Anomaly", "Seraph", "Manta", "Oracle"][..],
    };

    if rng.gen_bool(0.35) {
        branded[rng.gen_range(0..branded.len())]
    } else {
        NOUNS[rng.gen_range(0..NOUNS.len())]
    }
}
