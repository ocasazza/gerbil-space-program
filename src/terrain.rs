use crate::astronomy::{AstronomicalBody, BodyClass, MoonClass, PlanetClass, StarClass};
use crate::relativity::point_mass_acceleration_1pn;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
};
use noise::{NoiseFn, Perlin};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::f32::consts::PI;

const PRIMARY_RADIUS: f32 = 5_000.0;
const PRIMARY_SURFACE_GRAVITY: f32 = 120.0;
const PRIMARY_GRAVITATIONAL_PARAMETER: f32 =
    PRIMARY_SURFACE_GRAVITY * PRIMARY_RADIUS * PRIMARY_RADIUS;
/// One deliberately scaled astronomical unit. Keeping AU-scale positions near
/// 10^6 preserves useful f32 precision for the lander while making the system
/// several orders of magnitude wider than its bodies.
pub const ASTRONOMICAL_UNIT: f32 = 1_000_000.0;

#[derive(Resource)]
pub struct TerrainData {
    pub planets: Vec<Planet>,
    #[allow(dead_code)]
    pub current_planet: usize,
    noise: Perlin,
    #[allow(dead_code)]
    seed: u32,
    simulation_time: f32,
}

#[derive(Clone)]
pub struct Planet {
    pub body: AstronomicalBody,
    pub center: Vec2,
    pub terrain_points: Vec<Vec2>,
    pub landing_zones: Vec<LandingZone>,
    pub surface_color: Color,
    pub core_color: Color,
    pub orbit: Option<Orbit>,
}

impl std::ops::Deref for Planet {
    type Target = crate::astronomy::PhysicalProperties;

    fn deref(&self) -> &Self::Target {
        &self.body.physical
    }
}

#[derive(Clone, Copy)]
pub struct Orbit {
    /// Index of the body this orbit is relative to.
    pub parent: usize,
    pub radius: f32,
    pub angular_speed: f32,
    pub phase: f32,
}

#[derive(Clone)]
pub struct LandingZone {
    pub start_angle: f32,
    pub end_angle: f32,
    #[allow(dead_code)]
    pub center: Vec2,
    #[allow(dead_code)]
    pub width: f32,
}

impl Default for TerrainData {
    fn default() -> Self {
        let seed = rand::thread_rng().gen();
        let noise = Perlin::new(seed);

        Self {
            planets: Vec::new(),
            current_planet: 0,
            noise,
            seed,
            simulation_time: 0.0,
        }
    }
}

impl TerrainData {
    pub(crate) fn simulation_time(&self) -> f32 {
        self.simulation_time
    }

    fn planet_center_after(&self, index: usize, seconds: f32) -> Vec2 {
        let planet = &self.planets[index];
        let Some(orbit) = planet.orbit else {
            return planet.center;
        };
        let angle = orbit.phase + orbit.angular_speed * (self.simulation_time + seconds);
        self.planet_center_after(orbit.parent, seconds)
            + Vec2::new(orbit.radius * angle.cos(), orbit.radius * angle.sin())
    }

    /// Future inertial center used by navigation and maneuver planning.
    pub(crate) fn body_center_at_time(&self, index: usize, seconds: f32) -> Vec2 {
        self.planet_center_after(index, seconds)
    }

    #[allow(dead_code)]
    pub fn new_with_seed(seed: u32) -> Self {
        let noise = Perlin::new(seed);
        Self {
            planets: Vec::new(),
            current_planet: 0,
            noise,
            seed,
            simulation_time: 0.0,
        }
    }

    pub fn generate_planets(&mut self, count: usize) {
        self.planets.clear();
        self.simulation_time = 0.0;

        for i in 0..count {
            let planet = self.generate_planet(i);
            self.planets.push(planet);
        }

        // Add a small, deterministic satellite system to every planet. Moon
        // orbits are selected between the Roche limit and 25% of the Hill
        // radius, a conservative prograde stability region.
        let planet_count = self.planets.len();
        for parent in 1..planet_count {
            // One substantial moon per generated planet avoids unmodeled
            // satellite-satellite resonance and mutual-Hill instability.
            let moon_count = 1;
            for moon_slot in 0..moon_count {
                if let Some(moon) = self.generate_moon(parent, moon_slot) {
                    self.planets.push(moon);
                }
            }
        }
    }

    fn generate_planet(&self, index: usize) -> Planet {
        // A seeded system is repeatable across runs and platforms. Body zero is
        // the fixed primary; all other bodies follow prescribed circular orbits.
        let mut rng = StdRng::seed_from_u64(self.seed as u64 ^ (index as u64 * 0x9e37_79b9));
        let orbit = if index == 0 {
            None
        } else {
            // Keep the bodies widely separated so interplanetary flight and the
            // system-level minimap have meaningful scale.
            // System-scale separation. This is deliberately much larger than
            // body radii so planets read as distinct destinations rather than
            // adjacent toys, while remaining within f32's precise gameplay
            // range for Bevy transforms.
            let orbital_au = if let Some(au) = [0.35, 0.70, 1.20, 2.00].get(index - 1) {
                *au
            } else {
                2.0 + (index.saturating_sub(4)) as f32 * 0.8
            };
            let radius = orbital_au * ASTRONOMICAL_UNIT;
            Some(Orbit {
                parent: 0,
                radius,
                // Circular two-body solution around the primary: omega² = mu/r³.
                angular_speed: (PRIMARY_GRAVITATIONAL_PARAMETER / radius.powi(3)).sqrt(),
                phase: index as f32 * 2.399_963_1,
            })
        };
        let center = orbit
            .map(|o| Vec2::new(o.radius * o.phase.cos(), o.radius * o.phase.sin()))
            .unwrap_or(Vec2::ZERO);

        let (class, base_radius, surface_gravity) = match index {
            0 => (
                BodyClass::Star(StarClass::MainSequence),
                PRIMARY_RADIUS,
                PRIMARY_SURFACE_GRAVITY,
            ),
            1 => (BodyClass::Planet(PlanetClass::Terrestrial), 760.0, 11.0),
            2 => (BodyClass::Planet(PlanetClass::SuperEarth), 1_080.0, 17.0),
            3 => (BodyClass::Planet(PlanetClass::GasGiant), 2_200.0, 27.0),
            4 => (BodyClass::Planet(PlanetClass::IceGiant), 1_450.0, 16.0),
            _ => (BodyClass::Planet(PlanetClass::DwarfPlanet), 420.0, 4.0),
        };
        let body = AstronomicalBody::from_surface_gravity(class, base_radius, surface_gravity);

        // Generate terrain points using Perlin noise
        let resolution = 128; // Number of points around the circumference
        let mut terrain_points = Vec::with_capacity(resolution);

        // Generate landing zones first
        let num_landing_zones = if matches!(class, BodyClass::Star(_)) {
            0
        } else {
            rng.gen_range(2..5)
        };
        let mut landing_zones = Vec::new();

        for _ in 0..num_landing_zones {
            let angle = rng.gen_range(0.0..2.0 * PI);
            let width = rng.gen_range(0.3..0.8); // Width in radians

            landing_zones.push(LandingZone {
                start_angle: angle - width / 2.0,
                end_angle: angle + width / 2.0,
                center: Vec2::new(
                    center.x + base_radius * angle.cos(),
                    center.y + base_radius * angle.sin(),
                ),
                width: width * base_radius,
            });
        }

        // Generate terrain points
        for i in 0..resolution {
            let angle = (i as f32 / resolution as f32) * 2.0 * PI;

            // Check if this angle is in a landing zone
            let in_landing_zone = landing_zones.iter().any(|zone| {
                let normalized_angle = ((angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
                let start = ((zone.start_angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
                let end = ((zone.end_angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

                if start <= end {
                    normalized_angle >= start && normalized_angle <= end
                } else {
                    normalized_angle >= start || normalized_angle <= end
                }
            });

            let radius = if in_landing_zone {
                // Flat landing zones
                base_radius
            } else {
                // Use Perlin noise for terrain variation
                let noise_scale = 0.02;
                let noise_amplitude = base_radius * 0.15;

                let noise_value = self.noise.get([
                    (center.x + angle.cos() * 100.0) as f64 * noise_scale,
                    (center.y + angle.sin() * 100.0) as f64 * noise_scale,
                    index as f64 * 0.1, // Different noise for each planet
                ]);

                base_radius + noise_value as f32 * noise_amplitude
            };

            let point = Vec2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );

            terrain_points.push(point);
        }

        let (surface_color, core_color) = match class {
            BodyClass::Star(_) => (Color::srgb(1.0, 0.84, 0.38), Color::srgb(1.0, 0.42, 0.08)),
            BodyClass::Planet(PlanetClass::Terrestrial) => {
                (Color::srgb(0.38, 0.68, 0.46), Color::srgb(0.15, 0.30, 0.20))
            }
            BodyClass::Planet(PlanetClass::SuperEarth) => {
                (Color::srgb(0.72, 0.48, 0.32), Color::srgb(0.34, 0.17, 0.10))
            }
            BodyClass::Planet(PlanetClass::GasGiant) => {
                (Color::srgb(0.88, 0.68, 0.46), Color::srgb(0.48, 0.28, 0.18))
            }
            BodyClass::Planet(PlanetClass::IceGiant) => {
                (Color::srgb(0.42, 0.72, 0.90), Color::srgb(0.16, 0.36, 0.55))
            }
            _ => (Color::srgb(0.62, 0.60, 0.58), Color::srgb(0.28, 0.27, 0.26)),
        };

        Planet {
            body,
            center,
            terrain_points,
            landing_zones,
            surface_color,
            core_color,
            orbit,
        }
    }

    fn generate_moon(&self, parent: usize, slot: usize) -> Option<Planet> {
        let parent_body = self.planets.get(parent)?;
        let parent_orbit = parent_body.orbit?;
        let primary_mu = self.planets.first()?.gravitational_parameter;
        let hill_radius =
            parent_orbit.radius * (parent_body.gravitational_parameter / (3.0 * primary_mu)).cbrt();
        let roche_limit = parent_body.radius * 2.5;
        let stable_outer = hill_radius * 0.25;
        if stable_outer <= roche_limit * 1.35 {
            return None;
        }

        let fraction = (0.38 + slot as f32 * 0.34).min(0.82);
        let orbit_radius = roche_limit + (stable_outer - roche_limit) * fraction;
        let radius = (parent_body.radius * (0.18 + slot as f32 * 0.06)).clamp(32.0, 72.0);
        let phase = (parent as f32 * 1.618_034 + slot as f32 * 2.399_963_1) % (2.0 * PI);
        let orbit = Orbit {
            parent,
            radius: orbit_radius,
            angular_speed: (parent_body.gravitational_parameter / orbit_radius.powi(3)).sqrt(),
            phase,
        };
        let center =
            parent_body.center + Vec2::new(orbit_radius * phase.cos(), orbit_radius * phase.sin());
        let resolution = 64;
        let terrain_points = (0..resolution)
            .map(|i| {
                let angle = i as f32 / resolution as f32 * 2.0 * PI;
                center + Vec2::new(angle.cos(), angle.sin()) * radius
            })
            .collect();

        let moon_class = match (parent + slot) % 3 {
            0 => MoonClass::Rocky,
            1 => MoonClass::Icy,
            _ => MoonClass::Captured,
        };
        Some(Planet {
            body: AstronomicalBody::from_surface_gravity(BodyClass::Moon(moon_class), radius, 1.6),
            center,
            terrain_points,
            landing_zones: Vec::new(),
            surface_color: Color::srgb(0.62, 0.66, 0.72),
            core_color: Color::srgb(0.30, 0.34, 0.42),
            orbit: Some(orbit),
        })
    }

    /// Advances the prescribed ephemeris and translates all world-space terrain
    /// geometry with its owning body, keeping rendering and collision coherent.
    pub fn advance_orbits(&mut self, dt: f32) {
        self.simulation_time += dt;
        let centers: Vec<Vec2> = (0..self.planets.len())
            .map(|index| self.planet_center_after(index, 0.0))
            .collect();
        for (planet, new_center) in self.planets.iter_mut().zip(centers) {
            let delta = new_center - planet.center;
            planet.center = new_center;
            for point in &mut planet.terrain_points {
                *point += delta;
            }
            for zone in &mut planet.landing_zones {
                zone.center += delta;
            }
        }
    }

    /// Inertial velocity of a body, including every parent orbit in the chain.
    pub fn orbital_velocity(&self, index: usize) -> Vec2 {
        self.orbital_velocity_after(index, 0.0)
    }

    /// Safe prograde circular orbit above the body's highest generated terrain.
    /// The returned velocity is inertial: parent-chain velocity plus the local
    /// two-body circular velocity around this body.
    pub fn circular_orbit_state(&self, index: usize, clearance: f32) -> Option<(Vec2, Vec2)> {
        let body = self.planets.get(index)?;
        let surface_radius = body
            .terrain_points
            .iter()
            .map(|point| point.distance(body.center))
            .fold(body.radius, f32::max);
        let orbital_radius = surface_radius + clearance.max(20.0);
        let radial = Vec2::Y * orbital_radius;
        let prograde_tangent = Vec2::new(-radial.y, radial.x).normalize();
        let local_speed = (body.gravitational_parameter / orbital_radius).sqrt();
        Some((
            body.center + radial,
            self.orbital_velocity(index) + prograde_tangent * local_speed,
        ))
    }

    fn orbital_velocity_after(&self, index: usize, seconds: f32) -> Vec2 {
        let planet = &self.planets[index];
        let Some(orbit) = planet.orbit else {
            return Vec2::ZERO;
        };
        let angle = orbit.phase + orbit.angular_speed * (self.simulation_time + seconds);
        self.orbital_velocity_after(orbit.parent, seconds)
            + Vec2::new(-angle.sin(), angle.cos()) * orbit.radius * orbit.angular_speed
    }

    /// Vector sum of every body's softened inverse-square gravity. Softening
    /// prevents singular accelerations without introducing a global direction.
    pub fn gravity_at(&self, position: Vec2) -> Vec2 {
        self.gravity_at_time(position, 0.0)
    }

    /// Gravity using the prescribed future ephemeris for every orbiting body.
    /// This is used by flight planning so long predictions do not treat moving
    /// planets as if they were frozen at their current positions.
    pub fn gravity_at_time(&self, position: Vec2, seconds: f32) -> Vec2 {
        self.planets
            .iter()
            .enumerate()
            .fold(Vec2::ZERO, |gravity, (index, planet)| {
                let offset = self.planet_center_after(index, seconds) - position;
                let distance_squared = offset.length_squared();
                let softening = (planet.radius * 0.12).max(24.0);
                let softened_squared = distance_squared + softening * softening;
                gravity + offset * (planet.gravitational_parameter / softened_squared.powf(1.5))
            })
    }

    /// Weak-field gravity through first post-Newtonian order. Body velocity is
    /// included so predictions use the same moving ephemeris as rendering.
    pub fn relativistic_gravity_at_time(
        &self,
        position: Vec2,
        velocity: Vec2,
        seconds: f32,
    ) -> Vec2 {
        self.planets
            .iter()
            .enumerate()
            .fold(Vec2::ZERO, |gravity, (index, body)| {
                let body_position = self.planet_center_after(index, seconds);
                let body_velocity = self.orbital_velocity_after(index, seconds);
                gravity
                    + point_mass_acceleration_1pn(
                        position - body_position,
                        velocity - body_velocity,
                        body.gravitational_parameter,
                        (body.radius * 0.12).max(24.0),
                    )
            })
    }

    #[allow(dead_code)]
    pub fn get_current_planet(&self) -> Option<&Planet> {
        self.planets.get(self.current_planet)
    }

    #[allow(dead_code)]
    pub fn get_planet_at_position(&self, position: Vec2) -> Option<&Planet> {
        self.planets.iter().find(|planet| {
            let distance = position.distance(planet.center);
            distance <= planet.radius * 1.2 // Add some buffer for detection
        })
    }

    pub fn check_collision(&self, position: Vec2, radius: f32) -> Option<CollisionInfo> {
        self.check_collision_at_time(position, radius, 0.0)
    }

    /// Collision test against terrain translated to each body's future orbital
    /// position. Planet rotation is intentionally not modeled yet.
    pub fn check_collision_at_time(
        &self,
        position: Vec2,
        radius: f32,
        seconds: f32,
    ) -> Option<CollisionInfo> {
        for (index, planet) in self.planets.iter().enumerate() {
            let future_center = self.planet_center_after(index, seconds);
            if let Some(collision) =
                self.check_planet_collision(planet, future_center, position, radius)
            {
                return Some(collision);
            }
        }
        None
    }

    fn check_planet_collision(
        &self,
        planet: &Planet,
        planet_center: Vec2,
        position: Vec2,
        radius: f32,
    ) -> Option<CollisionInfo> {
        let to_center = position - planet_center;
        let distance_to_center = to_center.length();

        // Quick check if we're even close to the planet
        if distance_to_center > planet.radius + radius + 50.0 {
            return None;
        }

        // Find the closest terrain point
        let angle_to_position = to_center.y.atan2(to_center.x);
        let normalized_angle = ((angle_to_position % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

        let terrain_index =
            ((normalized_angle / (2.0 * PI)) * planet.terrain_points.len() as f32) as usize;
        let terrain_index = terrain_index.min(planet.terrain_points.len() - 1);

        let terrain_point = planet.terrain_points[terrain_index] + (planet_center - planet.center);
        let distance_to_surface = position.distance(terrain_point);

        if distance_to_surface <= radius {
            // Check if this is a landing zone
            let is_landing_zone = planet.landing_zones.iter().any(|zone| {
                let start = ((zone.start_angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
                let end = ((zone.end_angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

                if start <= end {
                    normalized_angle >= start && normalized_angle <= end
                } else {
                    normalized_angle >= start || normalized_angle <= end
                }
            });

            let surface_normal = (terrain_point - planet_center).normalize();

            Some(CollisionInfo {
                point: terrain_point,
                normal: surface_normal,
                is_landing_zone,
                planet_center,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct CollisionInfo {
    #[allow(dead_code)]
    pub point: Vec2,
    #[allow(dead_code)]
    pub normal: Vec2,
    #[allow(dead_code)]
    pub is_landing_zone: bool,
    #[allow(dead_code)]
    pub planet_center: Vec2,
}

#[derive(Component)]
pub struct TerrainVisual {
    body_index: usize,
}

/// Upload the expensive irregular body surfaces once. Subsequent orbital
/// motion changes only entity transforms, leaving vertex processing to WGPU.
pub fn spawn_terrain_meshes(
    mut commands: Commands,
    terrain: Res<TerrainData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (body_index, planet) in terrain.planets.iter().enumerate() {
        let core = meshes.add(Circle::new(planet.radius * 0.8));
        let shell = meshes.add(terrain_shell_mesh(planet));
        commands.spawn((
            Mesh2d(core),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(planet.core_color))),
            Transform::from_xyz(planet.center.x, planet.center.y, -2.0),
            TerrainVisual { body_index },
        ));
        commands.spawn((
            Mesh2d(shell),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(planet.surface_color))),
            Transform::from_xyz(planet.center.x, planet.center.y, -1.0),
            TerrainVisual { body_index },
        ));
    }
}

fn terrain_shell_mesh(planet: &Planet) -> Mesh {
    let count = planet.terrain_points.len();
    let mut positions = Vec::with_capacity(count * 2);
    let mut uvs = Vec::with_capacity(count * 2);
    let mut indices = Vec::with_capacity(count * 6);
    let core_radius = planet.radius * 0.8;
    for point in &planet.terrain_points {
        let outer = *point - planet.center;
        let inner = outer.normalize_or_zero() * core_radius;
        positions.push([inner.x, inner.y, 0.0]);
        positions.push([outer.x, outer.y, 0.0]);
        uvs.push([0.5, 0.5]);
        uvs.push([0.5, 0.5]);
    }
    for index in 0..count {
        let next = (index + 1) % count;
        let inner = (index * 2) as u32;
        let outer = inner + 1;
        let next_inner = (next * 2) as u32;
        let next_outer = next_inner + 1;
        indices.extend_from_slice(&[inner, outer, next_outer, inner, next_outer, next_inner]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn sync_terrain_meshes(
    terrain: Res<TerrainData>,
    mut visuals: Query<(&TerrainVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut visuals {
        if let Some(body) = terrain.planets.get(visual.body_index) {
            transform.translation.x = body.center.x;
            transform.translation.y = body.center.y;
        }
    }
}

/// Lightweight dynamic overlays. Planet fill and terrain silhouettes are
/// persistent GPU meshes rather than thousands of rebuilt Gizmo segments.
pub fn draw_terrain_overlays(mut gizmos: Gizmos, terrain: Res<TerrainData>) {
    for planet in &terrain.planets {
        if let Some(orbit) = planet.orbit {
            let orbit_center = terrain
                .planets
                .get(orbit.parent)
                .map(|parent| parent.center)
                .unwrap_or(Vec2::ZERO);
            gizmos.circle_2d(
                orbit_center,
                orbit.radius,
                Color::srgba(0.35, 0.45, 0.65, 0.18),
            );
        }
        for landing_zone in &planet.landing_zones {
            let resolution = 16;
            let angle_step =
                (landing_zone.end_angle - landing_zone.start_angle) / resolution as f32;
            for i in 0..resolution {
                let angle = landing_zone.start_angle + i as f32 * angle_step;
                let point = planet.center
                    + Vec2::new(planet.radius * angle.cos(), planet.radius * angle.sin());
                gizmos.line_2d(
                    point,
                    point + (point - planet.center).normalize() * 20.0,
                    Color::srgb(0.0, 1.0, 0.0),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_satellites_are_hierarchical_and_keplerian() {
        let mut terrain = TerrainData::new_with_seed(7);
        terrain.generate_planets(5);
        assert_eq!(terrain.planets.len(), 9);

        for (index, body) in terrain.planets.iter().enumerate().skip(1) {
            let orbit = body
                .orbit
                .expect("every non-primary body must orbit a parent");
            assert!(orbit.parent < index, "parents must precede children");
            let parent = &terrain.planets[orbit.parent];
            let expected = (parent.gravitational_parameter / orbit.radius.powi(3)).sqrt();
            assert!((orbit.angular_speed - expected).abs() < expected * 1.0e-5);

            if orbit.parent != 0 {
                assert!(orbit.radius > parent.radius * 2.5);
                let parent_orbit = parent.orbit.unwrap();
                let hill = parent_orbit.radius
                    * (parent.gravitational_parameter
                        / (3.0 * terrain.planets[0].gravitational_parameter))
                        .cbrt();
                assert!(orbit.radius <= hill * 0.25);
            }
        }

        terrain.advance_orbits(123.0);
        for body in terrain.planets.iter().skip(1) {
            let orbit = body.orbit.unwrap();
            let separation = body.center.distance(terrain.planets[orbit.parent].center);
            assert!((separation - orbit.radius).abs() < orbit.radius * 1.0e-4);
        }
    }

    #[test]
    fn launch_state_clears_terrain_and_has_circular_relative_speed() {
        let mut terrain = TerrainData::new_with_seed(7);
        terrain.generate_planets(5);
        let body = &terrain.planets[1];
        let (position, velocity) = terrain.circular_orbit_state(1, 240.0).unwrap();
        let orbital_radius = position.distance(body.center);
        let highest_terrain = body
            .terrain_points
            .iter()
            .map(|point| point.distance(body.center))
            .fold(body.radius, f32::max);
        assert!(orbital_radius >= highest_terrain + 239.9);

        let relative_velocity = velocity - terrain.orbital_velocity(1);
        let expected_speed = (body.gravitational_parameter / orbital_radius).sqrt();
        assert!((relative_velocity.length() - expected_speed).abs() < 0.001);
        assert!((relative_velocity.dot(position - body.center)).abs() < 0.1);
    }
}
