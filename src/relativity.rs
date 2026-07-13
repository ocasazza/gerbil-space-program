//! Weak-field relativistic corrections for the flight simulation.
//!
//! Distances are interpreted as metres and simulation time as seconds.  The
//! game is many orders of magnitude outside the strong-field regime, so a full
//! numerical solution of Einstein's equations would add cost without changing
//! any representable gameplay result.  We instead use special-relativistic
//! force response and the standard first post-Newtonian (1PN) test-particle
//! acceleration around each moving spherical body.

use bevy::prelude::Vec2;

pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;
const C_SQUARED: f64 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;

/// Coordinate acceleration caused by a three-force on a particle with the
/// supplied coordinate velocity. At low speed this converges to `force/mass`.
pub fn acceleration_from_force(force: Vec2, mass: f32, velocity: Vec2) -> Vec2 {
    if mass <= 0.0 {
        return Vec2::ZERO;
    }
    let vx = velocity.x as f64;
    let vy = velocity.y as f64;
    let fx = force.x as f64;
    let fy = force.y as f64;
    let beta_squared = ((vx * vx + vy * vy) / C_SQUARED).min(1.0 - 1.0e-15);
    let gamma = 1.0 / (1.0 - beta_squared).sqrt();
    let projection = (fx * vx + fy * vy) / C_SQUARED;
    Vec2::new(
        ((fx - projection * vx) / (gamma * mass as f64)) as f32,
        ((fy - projection * vy) / (gamma * mass as f64)) as f32,
    )
}

/// Newtonian plus 1PN acceleration from one spherical, non-spinning body.
/// `relative_position` and `relative_velocity` are object minus body.
pub fn point_mass_acceleration_1pn(
    relative_position: Vec2,
    relative_velocity: Vec2,
    gravitational_parameter: f32,
    softening: f32,
) -> Vec2 {
    let rx = relative_position.x as f64;
    let ry = relative_position.y as f64;
    let vx = relative_velocity.x as f64;
    let vy = relative_velocity.y as f64;
    let mu = gravitational_parameter as f64;
    let radius_squared = rx * rx + ry * ry + (softening as f64).powi(2);
    if radius_squared <= f64::EPSILON || mu <= 0.0 {
        return Vec2::ZERO;
    }
    let radius = radius_squared.sqrt();
    let inv_r3 = 1.0 / (radius_squared * radius);
    let speed_squared = vx * vx + vy * vy;
    let radial_speed_product = rx * vx + ry * vy;

    // Harmonic-coordinate Schwarzschild test-particle equation through 1PN.
    let newton_x = -mu * rx * inv_r3;
    let newton_y = -mu * ry * inv_r3;
    let common = 4.0 * mu / radius - speed_squared;
    let pn_scale = mu * inv_r3 / C_SQUARED;
    Vec2::new(
        (newton_x + pn_scale * (common * rx + 4.0 * radial_speed_product * vx)) as f32,
        (newton_y + pn_scale * (common * ry + 4.0 * radial_speed_product * vy)) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_relativity_reduces_parallel_force_more_than_transverse_force() {
        let v = Vec2::new((0.8 * SPEED_OF_LIGHT) as f32, 0.0);
        let parallel = acceleration_from_force(Vec2::X, 1.0, v).x;
        let transverse = acceleration_from_force(Vec2::Y, 1.0, v).y;
        // a_parallel = F/(gamma^3 m), a_perpendicular = F/(gamma m)
        assert!((parallel - 0.216).abs() < 1.0e-4);
        assert!((transverse - 0.6).abs() < 1.0e-4);
    }

    #[test]
    fn first_post_newtonian_term_has_expected_circular_orbit_direction() {
        // Deliberately compact synthetic body so the correction survives f32;
        // real game bodies produce a physically tiny correction.
        let mu = 1.0e16_f32;
        let radius = 10.0_f32;
        let circular_speed = (mu / radius).sqrt();
        let corrected = point_mass_acceleration_1pn(
            Vec2::new(radius, 0.0),
            Vec2::new(0.0, circular_speed),
            mu,
            0.0,
        );
        let newtonian = -mu / radius.powi(2);
        assert!(
            corrected.x > newtonian,
            "1PN correction should be outward here"
        );
        assert_eq!(corrected.y, 0.0);
    }
}
