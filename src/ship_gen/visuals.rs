use bevy::prelude::*;

use crate::game::Lander;
use crate::ship_gen::model::Primitive3d;
use crate::ship_gen::parts::{GeneratedShip, Manufacturer, Rarity};

/// Marker component for entities that have a generated ship visual.
#[derive(Component)]
pub struct ShipVisual;

/// Spawn a ship entity with visual components.
pub fn spawn_ship_visual(commands: &mut Commands, ship: &GeneratedShip, position: Vec2) -> Entity {
    commands
        .spawn((
            Name::new(ship.name.clone()),
            Transform::from_translation(position.extend(0.0)),
            ShipVisual,
            ship.clone(),
        ))
        .id()
}

/// System that draws all generated ship visuals using Bevy Gizmos.
pub fn draw_ship_visuals(
    mut gizmos: Gizmos,
    time: Res<Time>,
    ship_query: Query<(&Transform, &GeneratedShip, Option<&Lander>), With<ShipVisual>>,
) {
    for (transform, ship, lander) in &ship_query {
        let pos = transform.translation.truncate();
        let rotation = transform.rotation;

        draw_assembly_projection(&mut gizmos, pos, rotation, ship);

        if let Some(lander) = lander {
            draw_engine_exhaust(&mut gizmos, pos, rotation, ship, lander);
        }
        draw_rarity_glow(&mut gizmos, pos, ship.rarity, time.elapsed_secs());
    }
}

/// Orthographic +Z top projection of the authoritative 3D rigid-module model.
fn draw_assembly_projection(
    gizmos: &mut Gizmos,
    ship_position: Vec2,
    ship_rotation: Quat,
    ship: &GeneratedShip,
) {
    for module in &ship.assembly.modules {
        let local_center = Vec2::new(
            module.transform.translation[0],
            module.transform.translation[1],
        );
        let center = to_world(ship_position, ship_rotation, local_center);
        let width = module.dimensions[0];
        let length = module.dimensions[1];
        let color = palette_color(ship.visual.palette, module.color_layer);
        let module_rotation = Quat::from_array(module.transform.rotation);
        let module_forward = module_rotation * Vec3::Y;
        let projected_rotation =
            ship_rotation * Quat::from_rotation_z((-module_forward.x).atan2(module_forward.y));
        match module.primitive {
            Primitive3d::Box | Primitive3d::Truss | Primitive3d::Sail => {
                let points = [
                    Vec2::new(-width * 0.5, -length * 0.5),
                    Vec2::new(-width * 0.5, length * 0.5),
                    Vec2::new(width * 0.5, length * 0.5),
                    Vec2::new(width * 0.5, -length * 0.5),
                ];
                draw_polyline(gizmos, center, projected_rotation, &points, color, true);
                if matches!(module.primitive, Primitive3d::Truss) {
                    draw_polyline(
                        gizmos,
                        center,
                        projected_rotation,
                        &[points[0], points[2]],
                        color,
                        false,
                    );
                    draw_polyline(
                        gizmos,
                        center,
                        projected_rotation,
                        &[points[1], points[3]],
                        color,
                        false,
                    );
                }
            }
            Primitive3d::Wedge => {
                draw_polyline(
                    gizmos,
                    center,
                    projected_rotation,
                    &[
                        Vec2::new(0.0, length * 0.5),
                        Vec2::new(-width * 0.5, -length * 0.5),
                        Vec2::new(width * 0.5, -length * 0.5),
                    ],
                    color,
                    true,
                );
            }
            Primitive3d::Cylinder | Primitive3d::Sphere => {
                gizmos.circle_2d(center, width.max(length) * 0.5, color);
            }
            Primitive3d::Ring => {
                let radius = width.max(length) * 0.5;
                gizmos.circle_2d(center, radius, color);
                gizmos.circle_2d(center, radius * 0.58, color.with_alpha(0.72));
            }
            Primitive3d::Crescent => {
                let outer = width.max(length) * 0.5;
                let arc: Vec<Vec2> = (0..=20)
                    .map(|index| {
                        let angle = -2.35 + index as f32 / 20.0 * 4.70;
                        Vec2::new(angle.cos() * outer, angle.sin() * outer)
                    })
                    .chain((0..=20).rev().map(|index| {
                        let angle = -2.05 + index as f32 / 20.0 * 4.10;
                        Vec2::new(
                            angle.cos() * outer * 0.55 + outer * 0.28,
                            angle.sin() * outer * 0.55,
                        )
                    }))
                    .collect();
                draw_polyline(gizmos, center, projected_rotation, &arc, color, true);
            }
        }
    }
}

fn draw_modular_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let v = ship.visual;
    let color = palette_color(v.palette, 0);
    let secondary = palette_color(v.palette, 1);
    let accent = palette_color(v.palette, 2);
    let half_width = v.hull_width * 0.5;
    let half_length = v.hull_length * 0.5;

    // Chassis: three families, further diversified by continuous dimensions.
    let hull = match v.hull % 3 {
        0 => vec![
            Vec2::new(0.0, half_length),
            Vec2::new(-half_width, half_length * 0.15),
            Vec2::new(-half_width * 0.72, -half_length),
            Vec2::new(half_width * 0.72, -half_length),
            Vec2::new(half_width, half_length * 0.15),
        ],
        1 => vec![
            Vec2::new(0.0, half_length),
            Vec2::new(-half_width * 0.68, half_length * 0.48),
            Vec2::new(-half_width, -half_length * 0.45),
            Vec2::new(-half_width * 0.45, -half_length),
            Vec2::new(half_width * 0.45, -half_length),
            Vec2::new(half_width, -half_length * 0.45),
            Vec2::new(half_width * 0.68, half_length * 0.48),
        ],
        _ => vec![
            Vec2::new(0.0, half_length),
            Vec2::new(-half_width * 0.45, half_length * 0.55),
            Vec2::new(-half_width, half_length * 0.05),
            Vec2::new(-half_width * 0.55, -half_length),
            Vec2::new(half_width * 0.55, -half_length),
            Vec2::new(half_width, half_length * 0.05),
            Vec2::new(half_width * 0.45, half_length * 0.55),
        ],
    };
    draw_polyline(gizmos, pos, rotation, &hull, color, true);

    // Longitudinal chassis sections make the hull read as assembled modules,
    // not a single random polygon.
    let section_count = v.section_count.clamp(2, 4) as usize;
    for index in 1..section_count {
        let t = index as f32 / section_count as f32;
        let y = half_length - t * v.hull_length;
        let section_half = v.hull_sections[index] * 0.5;
        draw_polyline(
            gizmos,
            pos,
            rotation,
            &[Vec2::new(-section_half, y), Vec2::new(section_half, y)],
            secondary,
            false,
        );
    }

    // Nose attachment: probe, fork, or armored cap.
    match v.nose % 3 {
        0 => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(0.0, half_length),
                Vec2::new(0.0, half_length + 9.0),
            ],
            accent,
            false,
        ),
        1 => {
            for side in [-1.0, 1.0] {
                draw_polyline(
                    gizmos,
                    pos,
                    rotation,
                    &[
                        Vec2::new(side * half_width * 0.22, half_length * 0.78),
                        Vec2::new(side * half_width * 0.38, half_length + 7.0),
                    ],
                    accent,
                    false,
                );
            }
        }
        _ => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(-half_width * 0.55, half_length * 0.65),
                Vec2::new(0.0, half_length + 4.0),
                Vec2::new(half_width * 0.55, half_length * 0.65),
            ],
            accent,
            false,
        ),
    }

    // Symmetric wing modules keep even extreme combinations ship-like.
    let root_y = -half_length * 0.05;
    let tip_x = v.wing_span;
    let wing_sweep = v.wing_sweep * half_length;
    for pair in 0..v.wing_pairs.clamp(1, 2) {
        let pair_offset = pair as f32 * half_length * 0.42;
        let pair_scale = 1.0 - pair as f32 * 0.28;
        for side in [-1.0_f32, 1.0] {
            let asymmetry = 1.0 + v.asymmetry * side;
            let wing = [
                Vec2::new(
                    side * half_width * 0.75,
                    root_y + half_length * 0.25 - pair_offset,
                ),
                Vec2::new(
                    side * tip_x * pair_scale * asymmetry,
                    root_y + wing_sweep - pair_offset,
                ),
                Vec2::new(
                    side * tip_x * 0.76 * pair_scale * asymmetry,
                    root_y + wing_sweep - v.wing_chord - pair_offset,
                ),
                Vec2::new(
                    side * half_width * 0.62,
                    root_y - v.wing_chord * 0.62 - pair_offset,
                ),
            ];
            draw_polyline(
                gizmos,
                pos,
                rotation,
                &wing,
                if pair == 0 { color } else { secondary },
                true,
            );
            if v.wings % 3 == 2 {
                gizmos.circle_2d(to_world(pos, rotation, wing[1]), 2.4, accent);
            }
        }
    }

    // Cockpit module.
    let cockpit_color = accent;
    let cockpit_y = half_length * 0.28;
    match v.cockpit % 3 {
        0 => {
            gizmos.circle_2d(
                to_world(pos, rotation, Vec2::new(0.0, cockpit_y)),
                half_width * 0.34,
                cockpit_color,
            );
        }
        1 => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(0.0, cockpit_y + half_length * 0.25),
                Vec2::new(-half_width * 0.4, cockpit_y - half_length * 0.12),
                Vec2::new(half_width * 0.4, cockpit_y - half_length * 0.12),
            ],
            cockpit_color,
            true,
        ),
        _ => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(-half_width * 0.42, cockpit_y + 3.0),
                Vec2::new(-half_width * 0.28, cockpit_y - 6.0),
                Vec2::new(half_width * 0.28, cockpit_y - 6.0),
                Vec2::new(half_width * 0.42, cockpit_y + 3.0),
            ],
            cockpit_color,
            true,
        ),
    }

    // Engine pods and nozzles.
    let engine_spacing = half_width * v.engine_spread;
    for index in 0..v.engine_count {
        let x = if v.engine_count == 1 {
            0.0
        } else {
            (index as f32 / (v.engine_count - 1) as f32 - 0.5) * engine_spacing * 2.0
        };
        let pod_width = 2.8 + (v.engines % 3) as f32;
        draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(x - pod_width, -half_length * 0.72),
                Vec2::new(x - pod_width, -half_length - v.engine_length),
                Vec2::new(x + pod_width, -half_length - v.engine_length),
                Vec2::new(x + pod_width, -half_length * 0.72),
            ],
            color,
            true,
        );
    }

    // Tail/stabilizer and optional rare module.
    let tail_height = 7.0 + (v.tail % 6) as f32 * 2.0;
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-half_width * 0.22, -half_length * 0.55),
            Vec2::new(0.0, -half_length * 0.55 + tail_height),
            Vec2::new(half_width * 0.22, -half_length * 0.55),
        ],
        color,
        false,
    );
    if v.special != 0 {
        let special_color = rarity_color(ship.rarity);
        gizmos.circle_2d(
            pos,
            half_width * (0.28 + (v.special % 3) as f32 * 0.08),
            special_color,
        );
    }

    // Armor, reactor/utility geometry, decals, and deterministic wear provide
    // the high-frequency variation expected from loot-style generation.
    let plate_count = 1 + (v.armor % 4);
    for plate in 0..plate_count {
        let y = half_length * 0.22 - plate as f32 * 6.0;
        let inset = half_width * (0.28 + plate as f32 * 0.06);
        draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(-inset, y + 2.0),
                Vec2::new(-inset * 0.82, y - 3.0),
                Vec2::new(inset * 0.82, y - 3.0),
                Vec2::new(inset, y + 2.0),
            ],
            secondary,
            true,
        );
    }
    match v.utility % 4 {
        0 => {
            gizmos.circle_2d(
                to_world(pos, rotation, Vec2::new(0.0, -half_length * 0.18)),
                half_width * 0.23,
                accent,
            );
        }
        1 => {
            for side in [-1.0, 1.0] {
                gizmos.circle_2d(
                    to_world(pos, rotation, Vec2::new(side * half_width * 0.48, -2.0)),
                    2.2,
                    accent,
                );
            }
        }
        2 => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 11.0),
                Vec2::new(4.0, 15.0),
            ],
            accent,
            false,
        ),
        _ => draw_polyline(
            gizmos,
            pos,
            rotation,
            &[
                Vec2::new(-half_width * 0.55, -2.0),
                Vec2::new(half_width * 0.55, 2.0),
            ],
            accent,
            false,
        ),
    };
    let decal_side = if v.decal % 2 == 0 { -1.0 } else { 1.0 };
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(decal_side * half_width * 0.28, half_length * 0.08),
            Vec2::new(decal_side * half_width * 0.62, -half_length * 0.34),
        ],
        accent,
        false,
    );
    for scratch in 0..v.wear.min(5) {
        let x = -half_width * 0.65 + scratch as f32 * (half_width * 0.31);
        let y = -half_length * 0.05 + (scratch % 2) as f32 * 5.0;
        draw_polyline(
            gizmos,
            pos,
            rotation,
            &[Vec2::new(x, y), Vec2::new(x + 3.5, y - 2.5)],
            Color::srgba(0.8, 0.82, 0.86, 0.55),
            false,
        );
    }
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn palette_color(palette: u8, layer: u8) -> Color {
    const PALETTES: [[[f32; 3]; 3]; 6] = [
        [[0.72, 0.86, 1.0], [0.25, 0.42, 0.58], [0.35, 0.92, 1.0]],
        [[0.58, 0.34, 0.88], [0.22, 0.12, 0.34], [0.92, 0.42, 1.0]],
        [[0.96, 0.78, 0.22], [0.40, 0.27, 0.08], [1.0, 0.94, 0.58]],
        [[0.88, 0.43, 0.18], [0.34, 0.25, 0.19], [0.45, 0.94, 0.62]],
        [[0.68, 0.69, 0.61], [0.27, 0.29, 0.27], [1.0, 0.68, 0.20]],
        [[0.18, 0.94, 0.66], [0.08, 0.31, 0.28], [0.68, 1.0, 0.88]],
    ];
    let base = (palette / 3).min(5) as usize;
    let variant = (palette % 3) as f32;
    let [r, g, b] = PALETTES[base][layer.min(2) as usize];
    let factor = 0.90 + variant * 0.08;
    Color::srgb(
        (r * factor).min(1.0),
        (g * factor).min(1.0),
        (b * factor).min(1.0),
    )
}

fn manufacturer_color(manufacturer: Manufacturer) -> Color {
    match manufacturer {
        Manufacturer::OrionDynamics => Color::srgb(0.75, 0.9, 1.0),
        Manufacturer::VoidForge => Color::srgb(0.62, 0.38, 0.92),
        Manufacturer::SolarCollective => Color::srgb(1.0, 0.86, 0.28),
        Manufacturer::RustBeltCustoms => Color::srgb(0.95, 0.52, 0.22),
        Manufacturer::DeepSpaceMiningCorp => Color::srgb(0.74, 0.74, 0.64),
        Manufacturer::XenotechFoundry => Color::srgb(0.2, 1.0, 0.72),
    }
}

fn rarity_color(rarity: Rarity) -> Color {
    match rarity {
        Rarity::Common => Color::srgb(0.75, 0.75, 0.75),
        Rarity::Uncommon => Color::srgb(0.2, 1.0, 0.25),
        Rarity::Rare => Color::srgb(0.2, 0.55, 1.0),
        Rarity::Epic => Color::srgb(0.75, 0.25, 1.0),
        Rarity::Legendary => Color::srgb(1.0, 0.7, 0.12),
    }
}

fn draw_orion_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(0.75, 0.9, 1.0);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(0.0, 28.0),
            Vec2::new(-9.0, 4.0),
            Vec2::new(-24.0, -14.0),
            Vec2::new(-7.0, -9.0),
            Vec2::new(0.0, -24.0),
            Vec2::new(7.0, -9.0),
            Vec2::new(24.0, -14.0),
            Vec2::new(9.0, 4.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[Vec2::new(0.0, 20.0), Vec2::new(0.0, -19.0)],
        color,
        false,
    );
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_voidforge_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(0.55, 0.35, 0.85);
    let body = teardrop_points(24, 18.0, 30.0);
    draw_polyline(gizmos, pos, rotation, &body, color, true);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-10.0, -10.0),
            Vec2::new(-24.0, -22.0),
            Vec2::new(-30.0, -12.0),
        ],
        color,
        false,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(10.0, -10.0),
            Vec2::new(24.0, -22.0),
            Vec2::new(30.0, -12.0),
        ],
        color,
        false,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(0.0, -17.0),
            Vec2::new(-5.0, -31.0),
            Vec2::new(4.0, -37.0),
        ],
        color,
        false,
    );
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_solar_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(1.0, 0.9, 0.35);
    let panel = Color::srgb(0.35, 0.75, 1.0);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(0.0, 24.0),
            Vec2::new(-8.0, -18.0),
            Vec2::new(8.0, -18.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-10.0, 12.0),
            Vec2::new(-45.0, 18.0),
            Vec2::new(-45.0, -18.0),
            Vec2::new(-10.0, -12.0),
        ],
        panel,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(10.0, 12.0),
            Vec2::new(45.0, 18.0),
            Vec2::new(45.0, -18.0),
            Vec2::new(10.0, -12.0),
        ],
        panel,
        true,
    );
    for x in [-34.0, -22.0, 22.0, 34.0] {
        draw_polyline(
            gizmos,
            pos,
            rotation,
            &[Vec2::new(x, 16.0), Vec2::new(x, -16.0)],
            panel,
            false,
        );
    }
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_rustbelt_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(0.95, 0.55, 0.25);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-9.0, 23.0),
            Vec2::new(12.0, 18.0),
            Vec2::new(18.0, 3.0),
            Vec2::new(11.0, -5.0),
            Vec2::new(19.0, -20.0),
            Vec2::new(-5.0, -24.0),
            Vec2::new(-17.0, -13.0),
            Vec2::new(-25.0, -17.0),
            Vec2::new(-19.0, 8.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[Vec2::new(-16.0, 7.0), Vec2::new(16.0, 0.0)],
        color,
        false,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-6.0, -20.0),
            Vec2::new(4.0, -10.0),
            Vec2::new(16.0, -19.0),
        ],
        color,
        false,
    );
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_mining_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(0.72, 0.72, 0.62);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-18.0, 14.0),
            Vec2::new(18.0, 14.0),
            Vec2::new(18.0, -18.0),
            Vec2::new(-18.0, -18.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-8.0, 14.0),
            Vec2::new(0.0, 30.0),
            Vec2::new(8.0, 14.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[Vec2::new(0.0, 30.0), Vec2::new(0.0, 40.0)],
        Color::srgb(1.0, 0.8, 0.25),
        false,
    );
    for corner in [
        Vec2::new(-18.0, 14.0),
        Vec2::new(18.0, 14.0),
        Vec2::new(-18.0, -18.0),
        Vec2::new(18.0, -18.0),
    ] {
        gizmos.circle_2d(to_world(pos, rotation, corner), 3.0, color);
    }
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_xenotech_ship(gizmos: &mut Gizmos, pos: Vec2, rotation: Quat, ship: &GeneratedShip) {
    let color = Color::srgb(0.2, 1.0, 0.75);
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(5.0, 29.0),
            Vec2::new(-15.0, 13.0),
            Vec2::new(-20.0, -5.0),
            Vec2::new(-7.0, -22.0),
            Vec2::new(15.0, -15.0),
            Vec2::new(24.0, 5.0),
            Vec2::new(12.0, 18.0),
        ],
        color,
        true,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(-18.0, -2.0),
            Vec2::new(-34.0, -9.0),
            Vec2::new(-24.0, -18.0),
        ],
        color,
        false,
    );
    draw_polyline(
        gizmos,
        pos,
        rotation,
        &[
            Vec2::new(18.0, 2.0),
            Vec2::new(31.0, 13.0),
            Vec2::new(22.0, 22.0),
        ],
        color,
        false,
    );
    for dot in [
        Vec2::new(0.0, 15.0),
        Vec2::new(-7.0, -2.0),
        Vec2::new(8.0, -10.0),
    ] {
        gizmos.circle_2d(
            to_world(pos, rotation, dot),
            2.4,
            Color::srgb(0.55, 1.0, 0.9),
        );
    }
    draw_hardpoints(gizmos, pos, rotation, ship, color);
}

fn draw_rarity_glow(gizmos: &mut Gizmos, pos: Vec2, rarity: Rarity, elapsed_secs: f32) {
    let (radius, color) = match rarity {
        Rarity::Common => (33.0, Color::srgb(0.75, 0.75, 0.75)),
        Rarity::Uncommon => (36.0, Color::srgb(0.2, 1.0, 0.25)),
        Rarity::Rare => (39.0, Color::srgb(0.2, 0.55, 1.0)),
        Rarity::Epic => (42.0, Color::srgb(0.75, 0.25, 1.0)),
        Rarity::Legendary => {
            let pulse = 1.0 + elapsed_secs.sin() * 0.12;
            (47.0 * pulse, Color::srgb(1.0, 0.7, 0.12))
        }
    };
    gizmos.circle_2d(pos, radius, color);
    if rarity != Rarity::Common {
        gizmos.circle_2d(pos, radius + 4.0, color);
    }
}

fn draw_engine_exhaust(
    gizmos: &mut Gizmos,
    pos: Vec2,
    rotation: Quat,
    ship: &GeneratedShip,
    lander: &Lander,
) {
    if lander.main_thrust <= 0.001 {
        return;
    }
    let throttle = (lander.main_thrust / (150.0 * lander.thrust_scale)).clamp(0.0, 1.0);
    let flame_len = (12.0 * ship.total_stats.thrust.clamp(0.6, 2.4) * throttle).min(32.0);
    let count = ship.visual.engine_count.max(1);
    let spacing = ship.visual.hull_width * 0.5 * ship.visual.engine_spread;
    let nozzle_y = -ship.visual.hull_length * 0.5 - ship.visual.engine_length;
    for index in 0..count {
        let x = if count == 1 {
            0.0
        } else {
            (index as f32 / (count - 1) as f32 - 0.5) * spacing * 2.0
        };
        let start = Vec2::new(x, nozzle_y);
        let end = Vec2::new(x * 0.92, nozzle_y - flame_len);
        gizmos.line_2d(
            to_world(pos, rotation, start),
            to_world(pos, rotation, end),
            Color::srgb(1.0, 0.45, 0.05),
        );
        gizmos.line_2d(
            to_world(pos, rotation, start),
            to_world(pos, rotation, Vec2::new(x, nozzle_y - flame_len * 0.72)),
            Color::srgb(1.0, 0.9, 0.1),
        );
    }
}

fn draw_hardpoints(
    gizmos: &mut Gizmos,
    pos: Vec2,
    rotation: Quat,
    ship: &GeneratedShip,
    color: Color,
) {
    let count = ship.total_stats.hardpoints.clamp(0, 6);
    for index in 0..count {
        let x = (index as f32 - (count - 1) as f32 * 0.5) * 7.0;
        gizmos.circle_2d(to_world(pos, rotation, Vec2::new(x, -3.0)), 1.8, color);
    }
}

fn draw_polyline(
    gizmos: &mut Gizmos,
    pos: Vec2,
    rotation: Quat,
    points: &[Vec2],
    color: Color,
    closed: bool,
) {
    let mut world_points: Vec<Vec2> = points
        .iter()
        .map(|&point| to_world(pos, rotation, point))
        .collect();
    if closed {
        if let Some(first) = world_points.first().copied() {
            world_points.push(first);
        }
    }
    gizmos.linestrip_2d(world_points, color);
}

fn teardrop_points(steps: usize, width: f32, height: f32) -> Vec<Vec2> {
    (0..steps)
        .map(|i| {
            let t = i as f32 / steps as f32 * std::f32::consts::TAU;
            let y = t.cos() * height * 0.72 - 2.0;
            let taper = 0.45 + 0.55 * ((height - y.abs()).max(0.0) / height);
            Vec2::new(t.sin() * width * taper, y)
        })
        .collect()
}

fn to_world(pos: Vec2, rotation: Quat, point: Vec2) -> Vec2 {
    pos + (rotation * point.extend(0.0)).truncate()
}
