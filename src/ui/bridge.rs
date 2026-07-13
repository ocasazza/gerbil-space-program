use crate::game::{
    set_camera_follow, CameraController, GameData, Lander, SimulationSettings, DEFAULT_CAMERA_ZOOM,
};
use crate::maneuver::{ManeuverNode, ManeuverPlan};
use crate::minimap::MinimapCamera;
use crate::ship_gen::{GeneratedShip, Manufacturer, PartSlot, Rarity, ShipGenConfig};
use crate::terrain::TerrainData;
use crate::GameState;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::cell::RefCell;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum UiScreen {
    #[default]
    Loading,
    Menu,
    Playing,
    Paused,
    GameOver,
    Settings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapBody {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub parent: i16,
    pub orbit_radius: f32,
}

pub const MAX_MAP_BODIES: usize = 16;
pub const MAX_SHIP_MODULES: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiShipModule {
    pub primitive: u8,
    pub role: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub width: f32,
    pub length: f32,
    pub height: f32,
    pub rotation: [f32; 4],
    pub color_layer: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiSnapshot {
    pub screen: UiScreen,
    pub time: f32,
    pub fuel: f32,
    pub score: u32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub angular_velocity: f32,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub ship_x: f32,
    pub ship_y: f32,
    pub map_bodies: [MapBody; MAX_MAP_BODIES],
    pub map_body_count: u8,
    pub gravity: f32,
    pub thrust: f32,
    pub time_scale: f32,
    pub trajectory: bool,
    pub trajectory_steps: u32,
    pub infinite_fuel: bool,
    pub camera_zoom: f32,
    pub camera_follow: bool,
    pub minimap_visible: bool,
    pub gravity_field: bool,
    pub maneuver_enabled: bool,
    pub maneuver_armed: bool,
    pub maneuver_elapsed: f32,
    pub maneuver_nodes: u32,
    pub maneuver_node_id: u32,
    pub maneuver_at: f32,
    pub maneuver_duration: f32,
    pub maneuver_prograde: f32,
    pub maneuver_radial: f32,
    pub maneuver_rotation: f32,
    pub maneuver_throttle: f32,
    pub ship_rarity: u8,
    pub ship_manufacturer: u8,
    pub ship_parts: u8,
    pub ship_seed: u64,
    pub ship_thrust: f32,
    pub ship_mass: f32,
    pub ship_armor: f32,
    pub ship_maneuverability: f32,
    pub ship_hardpoints: i32,
    pub ship_hull_variant: u8,
    pub ship_cockpit_variant: u8,
    pub ship_wing_variant: u8,
    pub ship_engine_variant: u8,
    pub ship_special_variant: u8,
    pub ship_hull_width: f32,
    pub ship_hull_length: f32,
    pub ship_wing_span: f32,
    pub ship_engine_count: u8,
    pub ship_archetype: u8,
    pub ship_nose_variant: u8,
    pub ship_tail_variant: u8,
    pub ship_armor_variant: u8,
    pub ship_utility_variant: u8,
    pub ship_decal_variant: u8,
    pub ship_section_count: u8,
    pub ship_hull_sections: [f32; 4],
    pub ship_wing_pairs: u8,
    pub ship_wing_chord: f32,
    pub ship_wing_sweep: f32,
    pub ship_engine_spread: f32,
    pub ship_engine_length: f32,
    pub ship_asymmetry: f32,
    pub ship_palette: u8,
    pub ship_wear: u8,
    pub ship_part_slots: [u8; 8],
    pub ship_part_manufacturers: [u8; 8],
    pub ship_part_rarities: [u8; 8],
    pub ship_modules: [UiShipModule; MAX_SHIP_MODULES],
    pub ship_module_count: u8,
    pub ship_center_of_mass: [f32; 3],
    pub ship_inertia: [f32; 3],
    pub ship_center_of_pressure: [f32; 3],
    pub ship_reference_area: f32,
    pub ship_frontal_area: f32,
    pub ship_lateral_area: f32,
    pub ship_top_area: f32,
    pub ship_drag_coefficients: [f32; 3],
    pub ship_lift_slope: f32,
    pub ship_stall_angle: f32,
    pub ship_joint_count: u8,
}

#[derive(Clone, Copy, Debug)]
pub enum UiCommand {
    Play,
    Pause,
    Resume,
    Menu,
    Settings,
    Gravity(f32),
    Thrust(f32),
    TimeScale(f32),
    Trajectory(bool),
    InfiniteFuel(bool),
    TrajectorySteps(u32),
    CameraZoom(f32),
    CameraFollow(bool),
    ResetCamera,
    ResetFlight,
    Minimap(bool),
    GravityField(bool),
    ManeuverMode(bool),
    ManeuverAdd,
    ManeuverArm(bool),
    ManeuverClear,
    ManeuverSelectRelative(i8),
    ManeuverDeleteSelected,
    GenerateShip(u8),
    ManeuverEdit {
        id: u32,
        at: f32,
        duration: f32,
        prograde: f32,
        radial: f32,
        rotation: f32,
        throttle: f32,
    },
}

thread_local! {
    static SNAPSHOT: RefCell<UiSnapshot> = RefCell::new(UiSnapshot::default());
    static COMMANDS: RefCell<Vec<UiCommand>> = const { RefCell::new(Vec::new()) };
}

pub fn snapshot() -> UiSnapshot {
    SNAPSHOT.with(|value| *value.borrow())
}

pub fn command(value: UiCommand) {
    let return_focus_to_game = matches!(
        value,
        UiCommand::Play | UiCommand::Resume | UiCommand::ResetFlight
    );
    COMMANDS.with(|commands| commands.borrow_mut().push(value));
    if return_focus_to_game {
        if let Some(canvas) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("bevy"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = canvas.focus();
        }
    }
}

pub struct WebUiBridgePlugin;

impl Plugin for WebUiBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_web_canvas_resolution, apply_commands, publish_snapshot).chain(),
        );
    }
}

fn sync_web_canvas_resolution(
    time: Res<Time>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut last_check: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_check < 0.1 {
        return;
    }
    *last_check = now;
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("bevy"))
    else {
        return;
    };
    let width = canvas.client_width().max(1) as u32;
    let height = canvas.client_height().max(1) as u32;
    if window.resolution.physical_width() != width || window.resolution.physical_height() != height
    {
        window.resolution.set(width as f32, height as f32);
    }
}

fn apply_commands(
    state: Res<State<GameState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut settings: ResMut<SimulationSettings>,
    terrain: Res<TerrainData>,
    mut data: ResMut<GameData>,
    mut lander: Query<
        (&mut Transform, &mut Lander, Option<&mut GeneratedShip>),
        (
            With<crate::player::Player>,
            Without<crate::game::GameCamera>,
        ),
    >,
    mut camera: Query<(&mut CameraController, &Transform), With<crate::game::GameCamera>>,
    mut minimap: Query<&mut Camera, With<MinimapCamera>>,
    mut maneuver: ResMut<ManeuverPlan>,
    mut ship_design: ResMut<ShipGenConfig>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match state.get() {
            GameState::Playing => next_state.set(GameState::Paused),
            GameState::Paused => next_state.set(GameState::Playing),
            GameState::Settings | GameState::GameOver => next_state.set(GameState::Menu),
            _ => {}
        }
    }
    let commands = COMMANDS.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    for command in commands {
        match command {
            UiCommand::Play if matches!(state.get(), GameState::Menu | GameState::GameOver) => {
                next_state.set(GameState::Playing)
            }
            UiCommand::Resume if *state.get() == GameState::Paused => {
                next_state.set(GameState::Playing)
            }
            UiCommand::Pause if *state.get() == GameState::Playing => {
                next_state.set(GameState::Paused)
            }
            UiCommand::Menu
                if matches!(
                    state.get(),
                    GameState::Playing
                        | GameState::Paused
                        | GameState::GameOver
                        | GameState::Settings
                ) =>
            {
                next_state.set(GameState::Menu)
            }
            UiCommand::Settings if *state.get() == GameState::Menu => {
                next_state.set(GameState::Settings)
            }
            UiCommand::Gravity(value) => settings.gravity_multiplier = value.clamp(0.0, 10.0),
            UiCommand::Thrust(value) => settings.thrust_multiplier = value.clamp(0.1, 3.0),
            UiCommand::TimeScale(value) => settings.time_multiplier = value.clamp(0.1, 500.0),
            UiCommand::Trajectory(value) => settings.show_trajectory = value,
            UiCommand::InfiniteFuel(value) => settings.infinite_fuel = value,
            UiCommand::TrajectorySteps(value) => {
                settings.trajectory_steps = value.clamp(60, 360_000)
            }
            UiCommand::CameraZoom(value) => {
                if let Ok((mut camera, _)) = camera.single_mut() {
                    camera.target_zoom = value.clamp(0.3, 900.0);
                }
            }
            UiCommand::CameraFollow(value) => {
                if let Ok((mut camera, transform)) = camera.single_mut() {
                    set_camera_follow(&mut camera, transform.translation.truncate(), value);
                }
            }
            UiCommand::ResetCamera => {
                if let Ok((mut camera, _)) = camera.single_mut() {
                    camera.zoom = DEFAULT_CAMERA_ZOOM;
                    camera.target_zoom = DEFAULT_CAMERA_ZOOM;
                    camera.pan_offset = Vec2::ZERO;
                    camera.follow_player = true;
                    camera.is_dragging = false;
                    camera.last_mouse_pos = None;
                }
            }
            UiCommand::ResetFlight if *state.get() == GameState::Playing => {
                data.time = 0.0;
                data.fuel = data.max_fuel.max(100.0);
                data.score = 0;
                if let Ok((mut transform, mut lander, _)) = lander.single_mut() {
                    let (spawn, velocity) = terrain
                        .circular_orbit_state(1, 240.0)
                        .unwrap_or((Vec2::new(0.0, 600.0), Vec2::ZERO));
                    transform.translation = spawn.extend(0.0);
                    transform.rotation = Quat::IDENTITY;
                    lander.velocity = velocity;
                    lander.angular_velocity = 0.0;
                    lander.main_thrust = 0.0;
                    lander.left_thrust = 0.0;
                    lander.right_thrust = 0.0;
                    lander.reverse_thrust = 0.0;
                    lander.angular_thrust = 0.0;
                }
                if let Ok((mut camera, _)) = camera.single_mut() {
                    camera.zoom = DEFAULT_CAMERA_ZOOM;
                    camera.target_zoom = DEFAULT_CAMERA_ZOOM;
                    camera.pan_offset = Vec2::ZERO;
                    camera.follow_player = true;
                }
            }
            UiCommand::Minimap(value) => {
                if let Ok(mut camera) = minimap.single_mut() {
                    camera.is_active = value;
                }
            }
            UiCommand::GravityField(value) => settings.show_gravity_field = value,
            UiCommand::ManeuverMode(value) => {
                maneuver.enabled = value;
                if !value {
                    maneuver.set_armed(false);
                }
            }
            UiCommand::ManeuverAdd if maneuver.enabled && !maneuver.armed => {
                maneuver.add_node();
            }
            UiCommand::ManeuverArm(value) if maneuver.enabled => maneuver.set_armed(value),
            UiCommand::ManeuverClear if !maneuver.armed => maneuver.clear(),
            UiCommand::ManeuverSelectRelative(delta) if maneuver.enabled && !maneuver.armed => {
                maneuver.select_relative(delta)
            }
            UiCommand::ManeuverDeleteSelected if maneuver.enabled && !maneuver.armed => {
                maneuver.delete_selected()
            }
            UiCommand::GenerateShip(rarity) => {
                let rarity = match rarity {
                    1 => Rarity::Uncommon,
                    2 => Rarity::Rare,
                    3 => Rarity::Epic,
                    4 => Rarity::Legendary,
                    _ => Rarity::Common,
                };
                let design = ship_design.generate(rarity).clone();
                if let Ok((_, mut lander, Some(mut active_design))) = lander.single_mut() {
                    lander.mass = design.total_stats.mass.clamp(0.35, 4.0);
                    lander.thrust_scale = design.total_stats.thrust.clamp(0.45, 3.5);
                    lander.maneuverability = design.total_stats.maneuverability.clamp(0.4, 3.0);
                    *active_design = design;
                }
            }
            UiCommand::ManeuverEdit {
                id,
                at,
                duration,
                prograde,
                radial,
                rotation,
                throttle,
            } if maneuver.enabled && !maneuver.armed => {
                maneuver.edit_selected(ManeuverNode {
                    id,
                    at,
                    duration,
                    prograde,
                    radial,
                    rotation,
                    throttle,
                });
            }
            _ => {}
        }
    }
}

fn publish_snapshot(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    state: Res<State<GameState>>,
    data: Res<GameData>,
    settings: Res<SimulationSettings>,
    terrain: Res<TerrainData>,
    lander: Query<(&Transform, &Lander), With<crate::player::Player>>,
    camera: Query<&CameraController>,
    minimap: Query<&Camera, With<MinimapCamera>>,
    maneuver: Res<ManeuverPlan>,
    ship_design: Res<ShipGenConfig>,
    mut cadence: Local<(f32, UiScreen)>,
) {
    let screen = match state.get() {
        GameState::Loading => UiScreen::Loading,
        GameState::Menu => UiScreen::Menu,
        GameState::Playing => UiScreen::Playing,
        GameState::Paused => UiScreen::Paused,
        GameState::GameOver => UiScreen::GameOver,
        GameState::Settings => UiScreen::Settings,
    };
    let now = time.elapsed_secs();
    if cadence.1 == screen && now - cadence.0 < 0.05 {
        return;
    }
    *cadence = (now, screen);
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default() as f32;
    let frame_time_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default() as f32;

    let (ship_x, ship_y, velocity_x, velocity_y, angular_velocity) = lander
        .single()
        .map(|(transform, lander)| {
            (
                transform.translation.x,
                transform.translation.y,
                lander.velocity.x,
                lander.velocity.y,
                lander.angular_velocity,
            )
        })
        .unwrap_or_default();
    let mut map_bodies = [MapBody::default(); MAX_MAP_BODIES];
    let map_body_count = terrain.planets.len().min(MAX_MAP_BODIES);
    for (index, body) in terrain.planets.iter().take(map_body_count).enumerate() {
        map_bodies[index] = MapBody {
            x: body.center.x,
            y: body.center.y,
            radius: body.radius,
            parent: body.orbit.map_or(-1, |orbit| orbit.parent as i16),
            orbit_radius: body.orbit.map_or(0.0, |orbit| orbit.radius),
        };
    }
    let (camera_zoom, camera_follow) = camera
        .single()
        .map(|camera| (camera.target_zoom, camera.follow_player))
        .unwrap_or((1.0, true));
    let minimap_visible = minimap
        .single()
        .map(|camera| camera.is_active)
        .unwrap_or(false);
    let node = maneuver.selected();
    let design = &ship_design.selected;
    let ship_rarity = match design.rarity {
        Rarity::Common => 0,
        Rarity::Uncommon => 1,
        Rarity::Rare => 2,
        Rarity::Epic => 3,
        Rarity::Legendary => 4,
    };
    let ship_manufacturer = match design.manufacturer {
        Manufacturer::OrionDynamics => 0,
        Manufacturer::VoidForge => 1,
        Manufacturer::SolarCollective => 2,
        Manufacturer::RustBeltCustoms => 3,
        Manufacturer::DeepSpaceMiningCorp => 4,
        Manufacturer::XenotechFoundry => 5,
    };
    let mut ship_part_slots = [0; 8];
    let mut ship_part_manufacturers = [0; 8];
    let mut ship_part_rarities = [0; 8];
    for (index, part) in design.parts.iter().take(8).enumerate() {
        ship_part_slots[index] = match part.slot {
            PartSlot::Hull => 0,
            PartSlot::Engine => 1,
            PartSlot::PowerPlant => 2,
            PartSlot::Cockpit => 3,
            PartSlot::Wings => 4,
            PartSlot::Stabilizer => 5,
            PartSlot::Special => 6,
        };
        ship_part_manufacturers[index] = match part.manufacturer {
            Manufacturer::OrionDynamics => 0,
            Manufacturer::VoidForge => 1,
            Manufacturer::SolarCollective => 2,
            Manufacturer::RustBeltCustoms => 3,
            Manufacturer::DeepSpaceMiningCorp => 4,
            Manufacturer::XenotechFoundry => 5,
        };
        ship_part_rarities[index] = match part.rarity {
            Rarity::Common => 0,
            Rarity::Uncommon => 1,
            Rarity::Rare => 2,
            Rarity::Epic => 3,
            Rarity::Legendary => 4,
        };
    }
    let mut ship_modules = [UiShipModule::default(); MAX_SHIP_MODULES];
    let ship_module_count = design.assembly.modules.len().min(MAX_SHIP_MODULES);
    for (index, module) in design
        .assembly
        .modules
        .iter()
        .take(ship_module_count)
        .enumerate()
    {
        ship_modules[index] = UiShipModule {
            primitive: module.primitive as u8,
            role: module.role as u8,
            x: module.transform.translation[0],
            y: module.transform.translation[1],
            z: module.transform.translation[2],
            width: module.dimensions[0],
            length: module.dimensions[1],
            height: module.dimensions[2],
            rotation: module.transform.rotation,
            color_layer: module.color_layer,
        };
    }
    SNAPSHOT.with(|snapshot| {
        *snapshot.borrow_mut() = UiSnapshot {
            screen,
            time: data.time,
            fuel: data.fuel,
            score: data.score,
            velocity_x,
            velocity_y,
            angular_velocity,
            fps,
            frame_time_ms,
            ship_x,
            ship_y,
            map_bodies,
            map_body_count: map_body_count as u8,
            gravity: settings.gravity_multiplier,
            thrust: settings.thrust_multiplier,
            time_scale: settings.time_multiplier,
            trajectory: settings.show_trajectory,
            trajectory_steps: settings.trajectory_steps,
            infinite_fuel: settings.infinite_fuel,
            camera_zoom,
            camera_follow,
            minimap_visible,
            gravity_field: settings.show_gravity_field,
            maneuver_enabled: maneuver.enabled,
            maneuver_armed: maneuver.armed,
            maneuver_elapsed: maneuver.elapsed,
            maneuver_nodes: maneuver.nodes.len() as u32,
            maneuver_node_id: node.map_or(0, |node| node.id),
            maneuver_at: node.map_or(10.0, |node| node.at),
            maneuver_duration: node.map_or(2.0, |node| node.duration),
            maneuver_prograde: node.map_or(1.0, |node| node.prograde),
            maneuver_radial: node.map_or(0.0, |node| node.radial),
            maneuver_rotation: node.map_or(0.0, |node| node.rotation),
            maneuver_throttle: node.map_or(0.5, |node| node.throttle),
            ship_rarity,
            ship_manufacturer,
            ship_parts: design.parts.len().min(u8::MAX as usize) as u8,
            ship_seed: design.seed,
            ship_thrust: design.total_stats.thrust,
            ship_mass: design.total_stats.mass,
            ship_armor: design.total_stats.armor,
            ship_maneuverability: design.total_stats.maneuverability,
            ship_hardpoints: design.total_stats.hardpoints,
            ship_hull_variant: design.visual.hull,
            ship_cockpit_variant: design.visual.cockpit,
            ship_wing_variant: design.visual.wings,
            ship_engine_variant: design.visual.engines,
            ship_special_variant: design.visual.special,
            ship_hull_width: design.visual.hull_width,
            ship_hull_length: design.visual.hull_length,
            ship_wing_span: design.visual.wing_span,
            ship_engine_count: design.visual.engine_count,
            ship_archetype: design.visual.archetype,
            ship_nose_variant: design.visual.nose,
            ship_tail_variant: design.visual.tail,
            ship_armor_variant: design.visual.armor,
            ship_utility_variant: design.visual.utility,
            ship_decal_variant: design.visual.decal,
            ship_section_count: design.visual.section_count,
            ship_hull_sections: design.visual.hull_sections,
            ship_wing_pairs: design.visual.wing_pairs,
            ship_wing_chord: design.visual.wing_chord,
            ship_wing_sweep: design.visual.wing_sweep,
            ship_engine_spread: design.visual.engine_spread,
            ship_engine_length: design.visual.engine_length,
            ship_asymmetry: design.visual.asymmetry,
            ship_palette: design.visual.palette,
            ship_wear: design.visual.wear,
            ship_part_slots,
            ship_part_manufacturers,
            ship_part_rarities,
            ship_modules,
            ship_module_count: ship_module_count as u8,
            ship_center_of_mass: design.assembly.mass.center_of_mass,
            ship_inertia: design.assembly.mass.inertia_diagonal,
            ship_center_of_pressure: design.assembly.aerodynamics.center_of_pressure,
            ship_reference_area: design.assembly.aerodynamics.reference_area,
            ship_frontal_area: design.assembly.aerodynamics.frontal_area,
            ship_lateral_area: design.assembly.aerodynamics.lateral_area,
            ship_top_area: design.assembly.aerodynamics.top_area,
            ship_drag_coefficients: design.assembly.aerodynamics.drag_coefficient,
            ship_lift_slope: design.assembly.aerodynamics.lift_slope,
            ship_stall_angle: design.assembly.aerodynamics.stall_angle_radians,
            ship_joint_count: design.assembly.joints.len().min(u8::MAX as usize) as u8,
        };
    });
}
