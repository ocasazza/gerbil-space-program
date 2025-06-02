use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::Rng;
use std::f32::consts::PI;

#[derive(Resource)]
pub struct TerrainData {
    pub planets: Vec<Planet>,
    #[allow(dead_code)]
    pub current_planet: usize,
    noise: Perlin,
    #[allow(dead_code)]
    seed: u32,
}

#[derive(Clone)]
pub struct Planet {
    pub center: Vec2,
    pub radius: f32,
    pub terrain_points: Vec<Vec2>,
    pub landing_zones: Vec<LandingZone>,
    pub surface_color: Color,
    pub core_color: Color,
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
        }
    }
}

impl TerrainData {
    #[allow(dead_code)]
    pub fn new_with_seed(seed: u32) -> Self {
        let noise = Perlin::new(seed);
        Self {
            planets: Vec::new(),
            current_planet: 0,
            noise,
            seed,
        }
    }

    pub fn generate_planets(&mut self, count: usize) {
        self.planets.clear();

        for i in 0..count {
            let planet = self.generate_planet(i);
            self.planets.push(planet);
        }
    }

    fn generate_planet(&self, index: usize) -> Planet {
        let mut rng = rand::thread_rng();

        // Position planets in a grid-like pattern with some randomness
        let grid_size = 3; // 3x3 grid for now
        let x_offset = (index % grid_size as usize) as f32 * 2000.0 - 1000.0;
        let y_offset = (index / grid_size as usize) as f32 * 2000.0;

        let center = Vec2::new(
            x_offset + rng.gen_range(-200.0..200.0),
            y_offset + rng.gen_range(-200.0..200.0),
        );

        // Vary planet sizes
        let base_radius = rng.gen_range(200.0..400.0);

        // Generate terrain points using Perlin noise
        let resolution = 128; // Number of points around the circumference
        let mut terrain_points = Vec::with_capacity(resolution);

        // Generate landing zones first
        let num_landing_zones = rng.gen_range(2..5);
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

        // Generate planet colors
        let hue = rng.gen_range(0.0..360.0);
        let surface_color = Color::hsl(hue, 0.6, 0.7);
        let core_color = Color::hsl(hue, 0.8, 0.4);

        Planet {
            center,
            radius: base_radius,
            terrain_points,
            landing_zones,
            surface_color,
            core_color,
        }
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
        for planet in &self.planets {
            if let Some(collision) = self.check_planet_collision(planet, position, radius) {
                return Some(collision);
            }
        }
        None
    }

    fn check_planet_collision(&self, planet: &Planet, position: Vec2, radius: f32) -> Option<CollisionInfo> {
        let to_center = position - planet.center;
        let distance_to_center = to_center.length();

        // Quick check if we're even close to the planet
        if distance_to_center > planet.radius + radius + 50.0 {
            return None;
        }

        // Find the closest terrain point
        let angle_to_position = to_center.y.atan2(to_center.x);
        let normalized_angle = ((angle_to_position % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

        let terrain_index = ((normalized_angle / (2.0 * PI)) * planet.terrain_points.len() as f32) as usize;
        let terrain_index = terrain_index.min(planet.terrain_points.len() - 1);

        let terrain_point = planet.terrain_points[terrain_index];
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

            let surface_normal = (terrain_point - planet.center).normalize();

            Some(CollisionInfo {
                point: terrain_point,
                normal: surface_normal,
                is_landing_zone,
                planet_center: planet.center,
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

pub fn draw_terrain(mut gizmos: Gizmos, terrain: Res<TerrainData>) {
    for planet in &terrain.planets {
        draw_planet(&mut gizmos, planet);
    }
}

fn draw_planet(gizmos: &mut Gizmos, planet: &Planet) {
    // Draw planet core (filled circle approximation)
    let core_radius = planet.radius * 0.8;
    let core_resolution = 32;
    for i in 0..core_resolution {
        let angle1 = (i as f32 / core_resolution as f32) * 2.0 * PI;
        let angle2 = ((i + 1) as f32 / core_resolution as f32) * 2.0 * PI;

        let point1 = planet.center + Vec2::new(core_radius * angle1.cos(), core_radius * angle1.sin());
        let point2 = planet.center + Vec2::new(core_radius * angle2.cos(), core_radius * angle2.sin());

        gizmos.line_2d(planet.center, point1, planet.core_color);
        gizmos.line_2d(point1, point2, planet.core_color);
    }

    // Draw terrain surface
    for i in 0..planet.terrain_points.len() {
        let current = planet.terrain_points[i];
        let next = planet.terrain_points[(i + 1) % planet.terrain_points.len()];

        gizmos.line_2d(current, next, planet.surface_color);

        // Draw lines from surface to core to create filled appearance
        if i % 4 == 0 { // Only draw every 4th line to avoid too much clutter
            let to_center_dir = (planet.center - current).normalize();
            let core_point = current + to_center_dir * (current.distance(planet.center) - core_radius);
            gizmos.line_2d(current, core_point, planet.surface_color.with_alpha(0.3));
        }
    }

    // Highlight landing zones
    for landing_zone in &planet.landing_zones {
        let resolution = 16;
        let angle_step = (landing_zone.end_angle - landing_zone.start_angle) / resolution as f32;

        for i in 0..resolution {
            let angle = landing_zone.start_angle + i as f32 * angle_step;
            let point = planet.center + Vec2::new(planet.radius * angle.cos(), planet.radius * angle.sin());

            // Draw landing zone markers
            let marker_start = point;
            let marker_end = point + (point - planet.center).normalize() * 20.0;
            gizmos.line_2d(marker_start, marker_end, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}
