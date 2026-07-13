//! Composition-first astronomical body taxonomy and physical properties.
//!
//! These are scaled simulation units, not literal SI coordinates. Mass,
//! radius, surface gravity, and gravitational parameter remain internally
//! consistent through `mu = G * mass = g_surface * radius^2`.

/// Scaled gravitational constant used to express body masses without forcing
/// Bevy's f32 world coordinates into literal metre-scale astronomical values.
pub const GRAVITATIONAL_CONSTANT: f64 = 6.674_30e-5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarClass {
    MainSequence,
    RedDwarf,
    Giant,
    NeutronStar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanetClass {
    Terrestrial,
    SuperEarth,
    GasGiant,
    IceGiant,
    DwarfPlanet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoonClass {
    Rocky,
    Icy,
    Captured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsteroidClass {
    Carbonaceous,
    Silicate,
    Metallic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlackHoleClass {
    Stellar,
    Intermediate,
    Supermassive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyClass {
    Star(StarClass),
    Planet(PlanetClass),
    Moon(MoonClass),
    Asteroid(AsteroidClass),
    BlackHole(BlackHoleClass),
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicalProperties {
    pub radius: f32,
    pub mass: f64,
    pub mean_density: f64,
    pub surface_gravity: f32,
    pub gravitational_parameter: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct AstronomicalBody {
    pub class: BodyClass,
    pub physical: PhysicalProperties,
}

impl AstronomicalBody {
    pub fn from_surface_gravity(class: BodyClass, radius: f32, surface_gravity: f32) -> Self {
        let gravitational_parameter = surface_gravity * radius * radius;
        let mass = gravitational_parameter as f64 / GRAVITATIONAL_CONSTANT;
        let volume = 4.0 / 3.0 * std::f64::consts::PI * (radius as f64).powi(3);
        Self {
            class,
            physical: PhysicalProperties {
                radius,
                mass,
                mean_density: mass / volume,
                surface_gravity,
                gravitational_parameter,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_properties_are_internally_consistent() {
        let body = AstronomicalBody::from_surface_gravity(
            BodyClass::Planet(PlanetClass::Terrestrial),
            760.0,
            11.0,
        );
        let expected_mu = body.physical.surface_gravity * body.physical.radius.powi(2);
        assert!((body.physical.gravitational_parameter - expected_mu).abs() < 0.01);
        let mass_mu = GRAVITATIONAL_CONSTANT * body.physical.mass;
        assert!((mass_mu - expected_mu as f64).abs() < expected_mu as f64 * 1.0e-6);
        assert!(body.physical.mean_density.is_finite());
        assert!(body.physical.mean_density > 0.0);
    }

    #[test]
    fn taxonomy_supports_composeable_future_body_types() {
        let classes = [
            BodyClass::Moon(MoonClass::Icy),
            BodyClass::Asteroid(AsteroidClass::Metallic),
            BodyClass::Star(StarClass::NeutronStar),
            BodyClass::BlackHole(BlackHoleClass::Stellar),
        ];
        for class in classes {
            assert_eq!(
                AstronomicalBody::from_surface_gravity(class, 10.0, 2.0).class,
                class
            );
        }
    }
}
