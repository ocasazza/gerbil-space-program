//! Renderer-independent 3D spacecraft assembly and derived flight properties.
//!
//! The current game projects these rigid modules onto XY for its top-down
//! renderer. A future 3D renderer can consume the same transforms and shapes.

use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::parts::{Manufacturer, ShipPart, ShipVisualBlueprint, StatModifiers};

pub const MAX_SHIP_MODULES: usize = 32;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Primitive3d {
    Box,
    Wedge,
    Cylinder,
    Sphere,
    Ring,
    Truss,
    Sail,
    Crescent,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ModuleRole {
    Command,
    Hull,
    Engine,
    Cargo,
    Fuel,
    Sensor,
    Utility,
    Structure,
    LandingGear,
    Sail,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transform3d {
    pub translation: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AeroElement3d {
    pub center: [f32; 3],
    pub chord_direction: [f32; 3],
    pub span_direction: [f32; 3],
    pub area: f32,
    pub drag_coefficient_zero: f32,
    pub induced_drag_factor: f32,
    pub lift_slope: f32,
    pub stall_angle_radians: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ThrusterMount3d {
    pub position: [f32; 3],
    /// Force direction on the ship, not exhaust direction.
    pub force_direction: [f32; 3],
    pub maximum_force: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidModule3d {
    pub id: u16,
    pub parent: Option<u16>,
    pub role: ModuleRole,
    pub primitive: Primitive3d,
    pub manufacturer: Manufacturer,
    pub transform: Transform3d,
    /// Full local width (+X), length (+Y), and height (+Z).
    pub dimensions: [f32; 3],
    pub dry_mass: f32,
    pub local_center_of_mass: [f32; 3],
    pub aero: AeroElement3d,
    pub thruster: Option<ThrusterMount3d>,
    pub color_layer: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StructuralJoint {
    pub parent: u16,
    pub child: u16,
    pub anchor: [f32; 3],
    pub maximum_tension: f32,
    pub maximum_shear: f32,
    pub maximum_torque: f32,
    pub fatigue_limit: f32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct MassProperties3d {
    pub total_mass: f32,
    pub center_of_mass: [f32; 3],
    /// Diagonal of the assembly inertia tensor about its center of mass.
    pub inertia_diagonal: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AerodynamicProfile {
    pub reference_area: f32,
    pub frontal_area: f32,
    pub lateral_area: f32,
    pub top_area: f32,
    pub center_of_pressure: [f32; 3],
    pub drag_coefficient: [f32; 3],
    pub lift_slope: f32,
    pub stall_angle_radians: f32,
}

/// Density-dependent loads evaluated in the ship's local coordinate frame.
/// This keeps atmosphere sampling separate from the generated airframe.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AerodynamicLoads3d {
    pub force: [f32; 3],
    pub torque_about_center_of_mass: [f32; 3],
}

impl AerodynamicProfile {
    /// Evaluate quadratic pressure drag for body-frame air velocity.
    ///
    /// `relative_air_velocity` is atmosphere velocity minus ship velocity.
    /// The result is ready to add to the rigid body's force accumulator. Lift
    /// remains represented by `lift_slope`/`stall_angle_radians` so a future
    /// atmosphere model can select a subsonic, supersonic, or rarefied-flow
    /// law without regenerating the ship.
    pub fn evaluate_drag(
        &self,
        relative_air_velocity: [f32; 3],
        density: f32,
        center_of_mass: [f32; 3],
    ) -> AerodynamicLoads3d {
        let areas = [self.lateral_area, self.frontal_area, self.top_area];
        let mut force = [0.0; 3];
        for axis in 0..3 {
            let velocity = relative_air_velocity[axis];
            force[axis] = 0.5
                * density.max(0.0)
                * self.drag_coefficient[axis]
                * areas[axis]
                * velocity
                * velocity.abs();
        }
        let lever = [
            self.center_of_pressure[0] - center_of_mass[0],
            self.center_of_pressure[1] - center_of_mass[1],
            self.center_of_pressure[2] - center_of_mass[2],
        ];
        let torque_about_center_of_mass = [
            lever[1] * force[2] - lever[2] * force[1],
            lever[2] * force[0] - lever[0] * force[2],
            lever[0] * force[1] - lever[1] * force[0],
        ];
        AerodynamicLoads3d {
            force,
            torque_about_center_of_mass,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShipAssembly3d {
    pub category: u8,
    pub modules: Vec<RigidModule3d>,
    pub joints: Vec<StructuralJoint>,
    pub mass: MassProperties3d,
    pub aerodynamics: AerodynamicProfile,
}

struct AssemblyBuilder {
    modules: Vec<RigidModule3d>,
    joints: Vec<StructuralJoint>,
    manufacturer: Manufacturer,
}

impl AssemblyBuilder {
    fn new(manufacturer: Manufacturer) -> Self {
        Self {
            modules: Vec::new(),
            joints: Vec::new(),
            manufacturer,
        }
    }

    fn add(
        &mut self,
        parent: Option<u16>,
        role: ModuleRole,
        primitive: Primitive3d,
        position: [f32; 3],
        dimensions: [f32; 3],
        mass_bias: f32,
        color_layer: u8,
    ) -> u16 {
        let id = self.modules.len() as u16;
        let volume = (dimensions[0] * dimensions[1] * dimensions[2]).max(0.1);
        let drag = match primitive {
            Primitive3d::Truss => 0.45,
            Primitive3d::Sail => 1.35,
            Primitive3d::Sphere | Primitive3d::Cylinder => 0.55,
            Primitive3d::Wedge => 0.38,
            _ => 0.82,
        };
        self.modules.push(RigidModule3d {
            id,
            parent,
            role,
            primitive,
            manufacturer: self.manufacturer,
            transform: Transform3d {
                translation: position,
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            dimensions,
            dry_mass: volume * mass_bias,
            local_center_of_mass: [0.0, 0.0, 0.0],
            aero: AeroElement3d {
                center: position,
                chord_direction: [0.0, 1.0, 0.0],
                span_direction: [1.0, 0.0, 0.0],
                area: dimensions[0] * dimensions[1],
                drag_coefficient_zero: drag,
                induced_drag_factor: 0.08,
                lift_slope: if matches!(primitive, Primitive3d::Wedge | Primitive3d::Sail) {
                    3.8
                } else {
                    0.35
                },
                stall_angle_radians: 0.32,
            },
            thruster: None,
            color_layer,
        });
        if let Some(parent) = parent {
            let scale = dimensions[0].min(dimensions[1]).max(1.0);
            self.joints.push(StructuralJoint {
                parent,
                child: id,
                anchor: position,
                maximum_tension: 22.0 * scale,
                maximum_shear: 17.0 * scale,
                maximum_torque: 30.0 * scale * scale,
                fatigue_limit: 0.62,
            });
        }
        id
    }

    fn engine(&mut self, parent: u16, position: [f32; 3], size: [f32; 3], force: f32) {
        let id = self.add(
            Some(parent),
            ModuleRole::Engine,
            Primitive3d::Cylinder,
            position,
            size,
            0.032,
            2,
        );
        self.modules[id as usize].thruster = Some(ThrusterMount3d {
            position: [position[0], position[1] - size[1] * 0.5, position[2]],
            force_direction: [0.0, 1.0, 0.0],
            maximum_force: force,
        });
    }

    fn orient_y_axis_toward(&mut self, id: u16, direction: [f32; 3]) {
        self.modules[id as usize].transform.rotation = quaternion_from_y(direction);
    }

    /// Materialize structural members where a logical joint would otherwise
    /// cross empty space. The connector becomes part of the rigid-body graph,
    /// rather than being preview-only decoration.
    fn bridge_open_joints(&mut self) {
        let original_joints = std::mem::take(&mut self.joints);
        for joint in original_joints {
            let parent = &self.modules[joint.parent as usize];
            let child = &self.modules[joint.child as usize];
            if modules_overlap(parent, child) {
                self.joints.push(joint);
                continue;
            }

            let start = parent.transform.translation;
            let end = child.transform.translation;
            let delta = sub3(end, start);
            let distance = length3(delta);
            let midpoint = scale3(add3(start, end), 0.5);
            let thickness = parent
                .dimensions
                .iter()
                .copied()
                .fold(f32::INFINITY, f32::min)
                .min(
                    child
                        .dimensions
                        .iter()
                        .copied()
                        .fold(f32::INFINITY, f32::min),
                )
                .clamp(0.7, 3.5);
            let connector = self.add(
                Some(joint.parent),
                ModuleRole::Structure,
                Primitive3d::Truss,
                midpoint,
                [thickness, distance.max(thickness), thickness],
                0.004,
                1,
            );
            self.orient_y_axis_toward(connector, delta);
            if let Some(parent_joint) = self.joints.last_mut() {
                parent_joint.anchor = start;
            }
            self.modules[joint.child as usize].parent = Some(connector);
            self.joints.push(StructuralJoint {
                parent: connector,
                child: joint.child,
                anchor: end,
                ..joint
            });
        }
    }
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}
fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn quaternion_from_y(direction: [f32; 3]) -> [f32; 4] {
    let length = length3(direction).max(1.0e-6);
    let target = scale3(direction, 1.0 / length);
    // Shortest-arc quaternion rotating local +Y onto `target`.
    let mut quaternion = [target[2], 0.0, -target[0], 1.0 + target[1]];
    let norm = (quaternion.iter().map(|value| value * value).sum::<f32>()).sqrt();
    if norm < 1.0e-5 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    for value in &mut quaternion {
        *value /= norm;
    }
    quaternion
}

fn modules_overlap(a: &RigidModule3d, b: &RigidModule3d) -> bool {
    let delta = sub3(b.transform.translation, a.transform.translation);
    let distance = length3(delta);
    if distance < 1.0e-5 {
        return true;
    }
    let direction = scale3(delta, 1.0 / distance);
    distance <= support_radius(a, direction) + support_radius(b, scale3(direction, -1.0)) + 0.25
}

fn support_radius(module: &RigidModule3d, direction: [f32; 3]) -> f32 {
    if matches!(module.primitive, Primitive3d::Sphere) {
        return module.dimensions.iter().copied().fold(0.0, f32::max) * 0.5;
    }
    let half = scale3(module.dimensions, 0.5);
    let q = module.transform.rotation;
    let axes = [
        rotate_by_quaternion([1.0, 0.0, 0.0], q),
        rotate_by_quaternion([0.0, 1.0, 0.0], q),
        rotate_by_quaternion([0.0, 0.0, 1.0], q),
    ];
    half[0] * dot3(axes[0], direction).abs()
        + half[1] * dot3(axes[1], direction).abs()
        + half[2] * dot3(axes[2], direction).abs()
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rotate_by_quaternion(v: [f32; 3], q: [f32; 4]) -> [f32; 3] {
    let qv = [q[0], q[1], q[2]];
    let t = scale3(
        [
            qv[1] * v[2] - qv[2] * v[1],
            qv[2] * v[0] - qv[0] * v[2],
            qv[0] * v[1] - qv[1] * v[0],
        ],
        2.0,
    );
    add3(
        v,
        add3(
            scale3(t, q[3]),
            [
                qv[1] * t[2] - qv[2] * t[1],
                qv[2] * t[0] - qv[0] * t[2],
                qv[0] * t[1] - qv[1] * t[0],
            ],
        ),
    )
}

pub fn generate_assembly_3d(
    seed: u64,
    visual: &ShipVisualBlueprint,
    parts: &[ShipPart],
    stats: &StatModifiers,
    dominant_manufacturer: Manufacturer,
) -> ShipAssembly3d {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x3D5A_55E0_71C4_2026);
    let mut b = AssemblyBuilder::new(dominant_manufacturer);
    let w = visual.hull_width;
    let l = visual.hull_length;
    let h = (w * rng.gen_range(0.30..0.72)).max(3.0);
    let root = b.add(
        None,
        ModuleRole::Hull,
        Primitive3d::Box,
        [0.0, 0.0, 0.0],
        [w, l, h],
        0.018,
        0,
    );

    match visual.archetype {
        0 => {
            // needle / rocket
            b.modules[root as usize].primitive = Primitive3d::Wedge;
            b.add(
                Some(root),
                ModuleRole::Command,
                Primitive3d::Wedge,
                [0.0, l * 0.42, h * 0.28],
                [w * 0.72, l * 0.24, h * 0.62],
                0.022,
                2,
            );
        }
        1 => {
            // lifting body
            b.modules[root as usize].primitive = Primitive3d::Wedge;
            for side in [-1.0, 1.0] {
                b.add(
                    Some(root),
                    ModuleRole::Structure,
                    Primitive3d::Wedge,
                    [side * w * 0.72, -l * 0.05, 0.0],
                    [w * 0.92, l * 0.48, h * 0.25],
                    0.012,
                    1,
                );
            }
        }
        2 => {
            // saucer
            b.modules[root as usize].primitive = Primitive3d::Cylinder;
            b.modules[root as usize].dimensions = [w, w, h * 0.42];
            b.add(
                Some(root),
                ModuleRole::Command,
                Primitive3d::Sphere,
                [0.0, 0.0, h * 0.38],
                [w * 0.34, w * 0.34, h * 0.45],
                0.018,
                2,
            );
        }
        3 => {
            // spherical pod
            b.modules[root as usize].primitive = Primitive3d::Sphere;
            b.modules[root as usize].dimensions = [w, w, w];
            b.add(
                Some(root),
                ModuleRole::Structure,
                Primitive3d::Truss,
                [0.0, -w * 0.72, 0.0],
                [w * 0.18, w * 0.72, w * 0.15],
                0.008,
                1,
            );
        }
        4 => {
            // ring ship
            b.modules[root as usize].primitive = Primitive3d::Ring;
            b.modules[root as usize].dimensions = [w, w, h * 0.35];
            for spoke in 0..4 {
                let a = spoke as f32 * std::f32::consts::FRAC_PI_2;
                let id = b.add(
                    Some(root),
                    ModuleRole::Structure,
                    Primitive3d::Truss,
                    [a.cos() * w * 0.23, a.sin() * w * 0.23, 0.0],
                    [w * 0.08, w * 0.45, h * 0.15],
                    0.006,
                    1,
                );
                b.orient_y_axis_toward(id, [a.cos(), a.sin(), 0.0]);
            }
        }
        5 => {
            // dumbbell
            b.modules[root as usize].primitive = Primitive3d::Truss;
            b.modules[root as usize].dimensions = [w * 0.16, l, h * 0.15];
            for y in [-l * 0.42, l * 0.42] {
                b.add(
                    Some(root),
                    ModuleRole::Utility,
                    Primitive3d::Sphere,
                    [0.0, y, 0.0],
                    [w, w, w],
                    0.02,
                    if y > 0.0 { 2 } else { 1 },
                );
            }
        }
        6 => {
            // exposed spine
            b.modules[root as usize].primitive = Primitive3d::Truss;
            b.modules[root as usize].dimensions = [w * 0.18, l, h * 0.18];
            for index in 0..4 {
                let side = if index % 2 == 0 { -1.0 } else { 1.0 };
                b.add(
                    Some(root),
                    ModuleRole::Utility,
                    Primitive3d::Box,
                    [side * w * 0.52, l * (0.32 - index as f32 * 0.21), 0.0],
                    [w * 0.58, l * 0.17, h * 0.72],
                    0.018,
                    (index % 3) as u8,
                );
            }
        }
        7 => {
            // twin boom / catamaran
            b.modules[root as usize].primitive = Primitive3d::Truss;
            b.modules[root as usize].dimensions = [w * 1.8, l * 0.12, h * 0.16];
            for side in [-1.0, 1.0] {
                b.add(
                    Some(root),
                    ModuleRole::Hull,
                    Primitive3d::Wedge,
                    [side * w * 0.62, 0.0, 0.0],
                    [w * 0.52, l, h],
                    0.018,
                    0,
                );
            }
        }
        8 | 15 => {
            // barge / armored monitor
            b.modules[root as usize].dimensions = [w, l, h];
            for x in [-0.28, 0.28] {
                for y in [-0.25, 0.25] {
                    b.add(
                        Some(root),
                        ModuleRole::Cargo,
                        Primitive3d::Box,
                        [x * w, y * l, h * 0.62],
                        [w * 0.38, l * 0.34, h * 0.72],
                        0.025,
                        1,
                    );
                }
            }
        }
        9 => {
            // modular container stack
            b.modules[root as usize].primitive = Primitive3d::Truss;
            b.modules[root as usize].dimensions = [w * 0.14, l, h * 0.14];
            for index in 0..6 {
                let side = if index % 2 == 0 { -1.0 } else { 1.0 };
                b.add(
                    Some(root),
                    ModuleRole::Cargo,
                    Primitive3d::Box,
                    [side * w * 0.38, l * (0.34 - (index / 2) as f32 * 0.34), 0.0],
                    [w * 0.62, l * 0.22, h],
                    0.026,
                    (index % 3) as u8,
                );
            }
        }
        10 => {
            // mining rig
            for arm in 0..3 {
                let a = arm as f32 * std::f32::consts::TAU / 3.0 + visual.asymmetry;
                let id = b.add(
                    Some(root),
                    ModuleRole::Utility,
                    Primitive3d::Truss,
                    [a.cos() * w * 0.66, a.sin() * w * 0.66, 0.0],
                    [w * 0.14, w * 0.85, h * 0.18],
                    0.01,
                    1,
                );
                b.orient_y_axis_toward(id, [a.cos(), a.sin(), 0.0]);
            }
            b.add(
                Some(root),
                ModuleRole::Utility,
                Primitive3d::Wedge,
                [0.0, l * 0.58, 0.0],
                [w * 0.28, l * 0.48, h * 0.32],
                0.022,
                2,
            );
        }
        11 => {
            // solar sail
            b.modules[root as usize].dimensions = [w * 0.35, l * 0.30, h * 0.55];
            b.add(
                Some(root),
                ModuleRole::Sail,
                Primitive3d::Sail,
                [0.0, l * 0.58, h * 0.1],
                [w * 3.8, l * 1.45, 0.12],
                0.0004,
                2,
            );
        }
        12 => {
            // rotating habitat
            b.modules[root as usize].primitive = Primitive3d::Ring;
            b.modules[root as usize].dimensions = [w, w, h * 0.38];
            b.add(
                Some(root),
                ModuleRole::Structure,
                Primitive3d::Truss,
                [0.0, -l * 0.65, 0.0],
                [w * 0.13, l, h * 0.15],
                0.009,
                1,
            );
        }
        13 => {
            // alien crescent
            b.modules[root as usize].primitive = Primitive3d::Crescent;
            b.modules[root as usize].dimensions = [w, l, h];
            for node in 0..3 {
                b.add(
                    Some(root),
                    ModuleRole::Utility,
                    Primitive3d::Sphere,
                    [
                        w * (0.25 - node as f32 * 0.25),
                        l * (0.20 - node as f32 * 0.18),
                        h * 0.32,
                    ],
                    [w * 0.16, w * 0.16, h * 0.22],
                    0.012,
                    2,
                );
            }
        }
        14 => {
            // distributed swarm/cluster
            b.modules[root as usize].primitive = Primitive3d::Sphere;
            b.modules[root as usize].dimensions = [w * 0.22, w * 0.22, h * 0.35];
            for node in 0..7 {
                let a = node as f32 * 2.399_963;
                let radius = w * (0.35 + node as f32 * 0.055);
                b.add(
                    Some(root),
                    ModuleRole::Utility,
                    Primitive3d::Sphere,
                    [
                        a.cos() * radius,
                        a.sin() * radius,
                        (node % 3) as f32 * h * 0.12,
                    ],
                    [w * 0.16, w * 0.16, h * 0.28],
                    0.009,
                    (node % 3) as u8,
                );
            }
        }
        _ => {}
    }

    let engine_count = visual.engine_count.max(1);
    for index in 0..engine_count {
        let x = if engine_count == 1 {
            0.0
        } else {
            (index as f32 / (engine_count - 1) as f32 - 0.5) * w * visual.engine_spread
        };
        b.engine(
            root,
            [x, -l * 0.54, -h * 0.05],
            [w * 0.16, visual.engine_length, h * 0.42],
            150.0 * stats.thrust / engine_count as f32,
        );
    }

    b.bridge_open_joints();

    // Mixed manufacturers survive into the mechanical assembly.
    for (module, part) in b.modules.iter_mut().skip(1).zip(parts.iter().cycle()) {
        module.manufacturer = part.manufacturer;
    }

    let target_mass = stats.mass.clamp(0.35, 4.0);
    normalize_masses(&mut b.modules, target_mass);
    let mass = derive_mass_properties(&b.modules);
    let aerodynamics = derive_aerodynamic_profile(&b.modules);
    ShipAssembly3d {
        category: visual.archetype,
        modules: b.modules,
        joints: b.joints,
        mass,
        aerodynamics,
    }
}

fn normalize_masses(modules: &mut [RigidModule3d], target: f32) {
    let raw: f32 = modules.iter().map(|module| module.dry_mass).sum();
    let scale = target / raw.max(1.0e-6);
    for module in modules {
        module.dry_mass *= scale;
    }
}

pub fn derive_mass_properties(modules: &[RigidModule3d]) -> MassProperties3d {
    let total_mass: f32 = modules.iter().map(|m| m.dry_mass).sum();
    let mut com = [0.0; 3];
    for module in modules {
        for axis in 0..3 {
            com[axis] += module.dry_mass
                * (module.transform.translation[axis] + module.local_center_of_mass[axis]);
        }
    }
    for value in &mut com {
        *value /= total_mass.max(1.0e-6);
    }
    let mut inertia = [0.0; 3];
    for module in modules {
        let [x, y, z] = module.dimensions;
        let m = module.dry_mass;
        let local = [
            m * (y * y + z * z) / 12.0,
            m * (x * x + z * z) / 12.0,
            m * (x * x + y * y) / 12.0,
        ];
        let d = [
            module.transform.translation[0] - com[0],
            module.transform.translation[1] - com[1],
            module.transform.translation[2] - com[2],
        ];
        inertia[0] += local[0] + m * (d[1] * d[1] + d[2] * d[2]);
        inertia[1] += local[1] + m * (d[0] * d[0] + d[2] * d[2]);
        inertia[2] += local[2] + m * (d[0] * d[0] + d[1] * d[1]);
    }
    MassProperties3d {
        total_mass,
        center_of_mass: com,
        inertia_diagonal: inertia,
    }
}

pub fn derive_aerodynamic_profile(modules: &[RigidModule3d]) -> AerodynamicProfile {
    let mut profile = AerodynamicProfile::default();
    let mut cp_weight = 0.0;
    for module in modules {
        let [x, y, z] = module.dimensions;
        let cd = module.aero.drag_coefficient_zero;
        profile.frontal_area += x * z;
        profile.lateral_area += y * z;
        profile.top_area += x * y;
        profile.reference_area += module.aero.area;
        profile.drag_coefficient[0] += cd * y * z;
        profile.drag_coefficient[1] += cd * x * z;
        profile.drag_coefficient[2] += cd * x * y;
        profile.lift_slope += module.aero.lift_slope * module.aero.area;
        let weight = module.aero.area * cd;
        for axis in 0..3 {
            profile.center_of_pressure[axis] += weight * module.aero.center[axis];
        }
        cp_weight += weight;
        profile.stall_angle_radians = profile
            .stall_angle_radians
            .max(module.aero.stall_angle_radians);
    }
    profile.drag_coefficient[0] /= profile.lateral_area.max(1.0e-6);
    profile.drag_coefficient[1] /= profile.frontal_area.max(1.0e-6);
    profile.drag_coefficient[2] /= profile.top_area.max(1.0e-6);
    profile.lift_slope /= profile.reference_area.max(1.0e-6);
    for axis in 0..3 {
        profile.center_of_pressure[axis] /= cp_weight.max(1.0e-6);
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship_gen::{generate_ship, Rarity};

    #[test]
    fn generated_profiles_are_finite_and_physically_positive() {
        for seed in 0..128_u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ship = generate_ship(&mut rng, Rarity::Rare);
            let assembly = &ship.assembly;
            assert!(!assembly.modules.is_empty());
            assert!(assembly.modules.len() <= MAX_SHIP_MODULES);
            assert!(assembly.mass.total_mass.is_finite() && assembly.mass.total_mass > 0.0);
            assert!(assembly
                .mass
                .inertia_diagonal
                .iter()
                .all(|value| value.is_finite() && *value > 0.0));
            assert!(
                assembly.aerodynamics.reference_area.is_finite()
                    && assembly.aerodynamics.reference_area > 0.0
            );
            assert!(assembly
                .aerodynamics
                .drag_coefficient
                .iter()
                .all(|value| value.is_finite() && *value > 0.0));
            assert!(assembly
                .aerodynamics
                .center_of_pressure
                .iter()
                .all(|value| value.is_finite()));
            assert!(assembly.joints.iter().all(|joint| {
                joint.parent != joint.child
                    && (joint.parent as usize) < assembly.modules.len()
                    && (joint.child as usize) < assembly.modules.len()
                    && joint.maximum_tension > 0.0
                    && joint.maximum_shear > 0.0
                    && joint.maximum_torque > 0.0
            }));
            assert!(
                assembly.joints.iter().all(|joint| {
                    modules_overlap(
                        &assembly.modules[joint.parent as usize],
                        &assembly.modules[joint.child as usize],
                    )
                }),
                "every structural joint must have physical contact or a generated truss"
            );
            for module in assembly.modules.iter().skip(1) {
                assert_eq!(
                    assembly
                        .joints
                        .iter()
                        .filter(|joint| joint.child == module.id)
                        .count(),
                    1,
                    "each rigid module must have exactly one structural parent",
                );
            }
        }
    }

    #[test]
    fn drag_load_opposes_ship_motion_and_produces_cp_torque() {
        let profile = AerodynamicProfile {
            frontal_area: 4.0,
            lateral_area: 6.0,
            top_area: 8.0,
            center_of_pressure: [1.0, 0.0, 0.0],
            drag_coefficient: [0.5, 0.6, 0.7],
            ..Default::default()
        };
        // Air moves backward relative to a forward-moving ship.
        let loads = profile.evaluate_drag([0.0, -10.0, 0.0], 1.2, [0.0; 3]);
        assert!(loads.force[1] < 0.0);
        assert!(loads.torque_about_center_of_mass[2] < 0.0);
        assert_eq!(loads.force[0], 0.0);
        assert_eq!(loads.force[2], 0.0);
    }
}
