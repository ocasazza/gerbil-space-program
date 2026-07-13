use crate::maneuver::{apply_maneuver_inputs, ManeuverPlan};
use crate::player::Player;
use crate::relativity::acceleration_from_force;
use crate::ship_gen::{ShipGenConfig, ShipVisual};
use crate::terrain::{
    draw_terrain_overlays, spawn_terrain_meshes, sync_terrain_meshes, TerrainData, TerrainVisual,
};
use crate::GameState;
use bevy::input::mouse::MouseWheel;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    reflect::TypePath,
    render::{
        mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef},
        render_resource::{
            AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderRef, ShaderType,
            SpecializedMeshPipelineError, VertexFormat,
        },
        view::NoFrustumCulling,
    },
    sprite::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin},
};

pub struct GamePlugin;

const TRAJECTORY_SHADER_PATH: &str = "shaders/trajectory_curve.wgsl";
const ATTRIBUTE_CURVE_ENDS: MeshVertexAttribute = MeshVertexAttribute::new(
    "TrajectoryCurveEnds",
    2_137_464_970,
    VertexFormat::Float32x4,
);
const ATTRIBUTE_CURVE_TANGENTS: MeshVertexAttribute = MeshVertexAttribute::new(
    "TrajectoryCurveTangents",
    2_137_464_971,
    VertexFormat::Float32x4,
);
const ATTRIBUTE_CURVE_PARAMS: MeshVertexAttribute = MeshVertexAttribute::new(
    "TrajectoryCurveParams",
    2_137_464_972,
    VertexFormat::Float32x4,
);
const ATTRIBUTE_CURVE_SAMPLE_IDS: MeshVertexAttribute = MeshVertexAttribute::new(
    "TrajectoryCurveSampleIds",
    2_137_464_973,
    VertexFormat::Float32x2,
);
const CURVE_SUBDIVISIONS: usize = 6;
const MAX_CURVE_KNOTS: usize = 192;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<TrajectoryMaterial>::default())
        .init_resource::<GameData>()
        .init_resource::<TerrainData>()
        .init_resource::<SimulationSettings>()
        .init_resource::<TrajectoryCache>()
        .add_systems(
            OnTransition {
                exited: GameState::Menu,
                entered: GameState::Playing,
            },
            (setup_game, spawn_terrain_meshes, spawn_trajectory_visuals).chain(),
        )
        .add_systems(
            OnTransition {
                exited: GameState::GameOver,
                entered: GameState::Playing,
            },
            (setup_game, spawn_terrain_meshes, spawn_trajectory_visuals).chain(),
        )
        .add_systems(
            Update,
            (
                update_planet_orbits,
                handle_game_input,
                apply_maneuver_inputs,
                update_physics,
                update_camera,
                update_ui,
                check_game_over,
                update_trajectory_meshes,
                draw_vector_graphics,
                sync_terrain_meshes,
                draw_terrain_overlays,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            OnTransition {
                exited: GameState::Playing,
                entered: GameState::GameOver,
            },
            cleanup_game,
        )
        .add_systems(
            OnTransition {
                exited: GameState::Playing,
                entered: GameState::Menu,
            },
            cleanup_game,
        )
        .add_systems(
            OnTransition {
                exited: GameState::Paused,
                entered: GameState::Menu,
            },
            cleanup_game,
        );
    }
}

#[derive(Resource, Default)]
pub struct GameData {
    pub time: f32,
    pub fuel: f32,
    pub max_fuel: f32,
    pub score: u32,
}

#[derive(Resource)]
pub struct SimulationSettings {
    pub gravity_multiplier: f32,
    pub thrust_multiplier: f32,
    pub show_trajectory: bool,
    pub trajectory_steps: u32,
    pub infinite_fuel: bool,
    pub show_gravity_field: bool,
    pub time_multiplier: f32,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            gravity_multiplier: 1.0,
            thrust_multiplier: 1.0,
            show_trajectory: true,
            trajectory_steps: 600,
            infinite_fuel: false,
            show_gravity_field: false,
            time_multiplier: 1.0,
        }
    }
}

#[derive(Component)]
pub struct Lander {
    pub velocity: Vec2,
    pub angular_velocity: f32,
    pub main_thrust: f32,    // Forward thruster (strongest)
    pub left_thrust: f32,    // Left side thruster
    pub right_thrust: f32,   // Right side thruster
    pub reverse_thrust: f32, // Reverse thruster
    pub angular_thrust: f32, // Rotation thrusters
    pub mass: f32,
    pub thrust_scale: f32,
    pub maneuverability: f32,
}

#[derive(Component)]
pub struct GameUI;

#[derive(Component)]
pub struct GameCamera;

#[derive(Component, Clone, Copy)]
enum TrajectoryVisual {
    Coast,
    ActiveInput,
}

#[derive(ShaderType, Clone, Debug)]
struct TrajectoryUniform {
    color: Vec4,
    half_width: f32,
    dash_period: f32,
    dash_duty: f32,
    glow: f32,
    uncertainty: f32,
    path_index: u32,
    sample_count: u32,
    sample_dt: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct TrajectoryMaterial {
    #[uniform(0)]
    params: TrajectoryUniform,
}

impl Material2d for TrajectoryMaterial {
    fn vertex_shader() -> ShaderRef {
        TRAJECTORY_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        TRAJECTORY_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_CURVE_ENDS.at_shader_location(1),
            ATTRIBUTE_CURVE_TANGENTS.at_shader_location(2),
            ATTRIBUTE_CURVE_PARAMS.at_shader_location(3),
            ATTRIBUTE_CURVE_SAMPLE_IDS.at_shader_location(4),
        ])?];
        Ok(())
    }
}

#[derive(Component)]
pub struct CameraController {
    pub zoom: f32,
    pub target_zoom: f32,
    pub pan_offset: Vec2,
    pub follow_player: bool,
    pub is_dragging: bool,
    pub last_mouse_pos: Option<Vec2>,
}

pub(crate) fn set_camera_follow(
    controller: &mut CameraController,
    camera_position: Vec2,
    follow: bool,
) {
    if controller.follow_player == follow {
        return;
    }
    controller.pan_offset = if follow {
        // Follow mode interprets this as a lander-relative offset.
        Vec2::ZERO
    } else {
        // Free-camera mode interprets it as an absolute world position.
        camera_position
    };
    controller.follow_player = follow;
    controller.is_dragging = false;
    controller.last_mouse_pos = None;
}

pub const DEFAULT_CAMERA_ZOOM: f32 = 3.0;
const MIN_CAMERA_ZOOM: f32 = 0.3;
const MAX_CAMERA_ZOOM: f32 = 900.0;
const MAX_TIME_MULTIPLIER: f32 = 500.0;

// Game constants
const MAX_MAIN_THRUST: f32 = 150.0; // Main thruster (strongest)
const MAX_SIDE_THRUST: f32 = 60.0; // Side thrusters (weaker)
const MAX_REVERSE_THRUST: f32 = 40.0; // Reverse thruster (weakest)
const MAX_ANGULAR_THRUST: f32 = 3.0; // Rotation thrusters
const MAX_TRAJECTORY_HORIZON_TICKS: u32 = 360_000; // 6,000 seconds at 60 Hz
pub(crate) const MAX_TRAJECTORY_SAMPLES: u32 = 3_600;
const FUEL_CONSUMPTION_RATE: f32 = 10.0;
const STARTING_FUEL: f32 = 100.0;
// Engines and RCS valves cannot change thrust instantaneously.  The shorter
// release time keeps the controls responsive while still avoiding binary
// impulses when a key is tapped.
const THROTTLE_RISE_TIME: f32 = 0.28;
const THROTTLE_FALL_TIME: f32 = 0.18;
const MAX_REAL_FRAME_DT: f32 = 1.0 / 30.0;
const PHYSICS_STEP_DT: f32 = 1.0 / 60.0;
const MAX_PHYSICS_SUBSTEPS: u32 = 16;

fn approach_throttle(current: f32, target: f32, dt: f32) -> f32 {
    let response_time = if target.abs() > current.abs() {
        THROTTLE_RISE_TIME
    } else {
        THROTTLE_FALL_TIME
    };
    let blend = 1.0 - (-dt / response_time).exp();
    let next = current + (target - current) * blend;
    if target == 0.0 && next.abs() < 0.001 {
        0.0
    } else {
        next
    }
}

fn bounded_physics_step(frame_dt: f32, time_multiplier: f32) -> (u32, f32) {
    let simulated_dt = frame_dt.max(0.0).min(MAX_REAL_FRAME_DT) * time_multiplier.max(0.0);
    let substeps = (simulated_dt / PHYSICS_STEP_DT)
        .ceil()
        .clamp(1.0, MAX_PHYSICS_SUBSTEPS as f32) as u32;
    (substeps, simulated_dt / substeps as f32)
}

fn setup_game(
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    mut terrain_data: ResMut<TerrainData>,
    mut maneuver_plan: ResMut<ManeuverPlan>,
    mut trajectory_cache: ResMut<TrajectoryCache>,
    ship_design: Res<ShipGenConfig>,
) {
    info!("Setting up game");

    // Reset game data
    game_data.time = 0.0;
    game_data.fuel = STARTING_FUEL;
    game_data.max_fuel = STARTING_FUEL;
    game_data.score = 0;
    *maneuver_plan = ManeuverPlan::default();
    // Trajectory mesh entities are recreated for every flight. Their cache
    // must follow the same lifecycle or a restart can pair empty new meshes
    // with an apparently fresh prediction and never upload curve geometry.
    *trajectory_cache = TrajectoryCache::default();

    // Generate the deterministic solar system before choosing a safe spawn.
    terrain_data.generate_planets(5);
    let (spawn, spawn_velocity) = terrain_data
        .circular_orbit_state(1, 240.0)
        .unwrap_or((Vec2::new(0.0, 600.0), Vec2::ZERO));

    // Spawn camera with controller
    commands.spawn((
        Camera2d,
        Transform::from_translation(spawn.extend(0.0)),
        Msaa::Off,
        CameraController {
            zoom: DEFAULT_CAMERA_ZOOM,
            target_zoom: DEFAULT_CAMERA_ZOOM,
            pan_offset: Vec2::ZERO,
            follow_player: true,
            is_dragging: false,
            last_mouse_pos: None,
        },
        GameCamera,
    ));

    // Spawn the lander (vector graphics only, no sprite)
    let selected_ship = ship_design.selected.clone();
    commands.spawn((
        Transform::from_translation(spawn.extend(0.0)),
        Player,
        ShipVisual,
        selected_ship.clone(),
        Lander {
            velocity: spawn_velocity,
            angular_velocity: 0.0,
            main_thrust: 0.0,
            left_thrust: 0.0,
            right_thrust: 0.0,
            reverse_thrust: 0.0,
            angular_thrust: 0.0,
            mass: selected_ship.total_stats.mass.clamp(0.35, 4.0),
            thrust_scale: selected_ship.total_stats.thrust.clamp(0.45, 3.5),
            maneuverability: selected_ship.total_stats.maneuverability.clamp(0.4, 3.0),
        },
    ));

    // Native keeps Bevy UI. Web renders the HUD through Dioxus/panel-kit.
    #[cfg(not(target_arch = "wasm32"))]
    setup_game_ui(&mut commands);
}

fn spawn_trajectory_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TrajectoryMaterial>>,
) {
    for (kind, path_index, depth, color, dash_period, dash_duty, uncertainty) in [
        (
            TrajectoryVisual::Coast,
            0,
            -0.25,
            Vec4::new(0.25, 0.8, 1.0, 0.92),
            11.0,
            0.20,
            3.5,
        ),
        (
            TrajectoryVisual::ActiveInput,
            1,
            -0.20,
            Vec4::new(1.0, 0.88, 0.08, 0.98),
            13.0,
            0.22,
            5.0,
        ),
    ] {
        let material = materials.add(TrajectoryMaterial {
            params: TrajectoryUniform {
                color,
                half_width: 1.6,
                dash_period,
                dash_duty,
                glow: 0.28,
                uncertainty,
                path_index,
                sample_count: 0,
                sample_dt: 1.0 / 60.0,
            },
        });
        commands.spawn((
            Mesh2d(meshes.add(empty_trajectory_mesh())),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, depth),
            Visibility::Hidden,
            // The vertex shader replaces chord positions with Hermite points
            // and expands them into a zoom-scaled uncertainty ribbon. Its
            // rendered bounds can exceed the CPU mesh AABB, so ordinary
            // frustum culling can incorrectly drop it while zooming.
            NoFrustumCulling,
            kind,
        ));
    }
}

fn empty_trajectory_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // WGPU rejects zero-byte vertex buffers. Keep an inert triangle with the
    // complete curve layout until the first prediction is available.
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]; 3]);
    mesh.insert_attribute(ATTRIBUTE_CURVE_ENDS, vec![[0.0; 4]; 3]);
    mesh.insert_attribute(ATTRIBUTE_CURVE_TANGENTS, vec![[0.0; 4]; 3]);
    mesh.insert_attribute(ATTRIBUTE_CURVE_PARAMS, vec![[0.0; 4]; 3]);
    mesh.insert_attribute(ATTRIBUTE_CURVE_SAMPLE_IDS, vec![[0.0; 2]; 3]);
    mesh
}

fn setup_game_ui(commands: &mut Commands) {
    // Main UI container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GameUI,
        ))
        .with_children(|parent| {
            // Top UI panel
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(120.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Start,
                    padding: UiRect::all(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|top_panel| {
                    // Left side - Game stats
                    top_panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|left| {
                            left.spawn((
                                Text::new("TIME: 0.0s"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TimeDisplay,
                            ));
                            left.spawn((
                                Text::new("FUEL: 100.0"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                FuelDisplay,
                            ));
                        });

                    // Center - Velocity and acceleration
                    top_panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|center| {
                            center.spawn((
                                Text::new("VEL: (0.0, 0.0)"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                VelocityDisplay,
                            ));
                            center.spawn((
                                Text::new("THRUST: (0.0, 0.0)"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                ThrustDisplay,
                            ));
                            center.spawn((
                                Text::new("ROT VEL: 0.0"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                AngularVelocityDisplay,
                            ));
                            center.spawn((
                                Text::new("GRAVITY: 1.0x"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.8, 0.8, 1.0)),
                                GravityDisplay,
                            ));
                            center.spawn((
                                Text::new("THRUST: 1.0x"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.8, 0.8)),
                                ThrustMultiplierDisplay,
                            ));
                            center.spawn((
                                Text::new("TIME: 1.0x"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.8, 1.0, 1.0)),
                                TimeMultiplierDisplay,
                            ));
                        });

                    // Right side - Controls
                    top_panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|right| {
                            right.spawn((
                                Text::new("W/Up: Main Thrust"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("A/D: Side Thrust"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("S/Down: Reverse"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("Left/Right: Rotate"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("+/-: Zoom"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("R: Reset Camera"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("F: Toggle Follow"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                            ));
                            right.spawn((
                                Text::new("G/H: Gravity +/-"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.8, 0.8, 1.0)),
                            ));
                            right.spawn((
                                Text::new("T/Y: Thrust +/-"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(1.0, 0.8, 0.8)),
                            ));
                            right.spawn((
                                Text::new("P: Toggle Trajectory"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.8, 1.0, 0.8)),
                            ));
                            right.spawn((
                                Text::new("[/]: Prediction +/-"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.8, 1.0, 0.8)),
                            ));
                            right.spawn((
                                Text::new("I: Infinite Fuel"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(1.0, 1.0, 0.8)),
                            ));
                            right.spawn((
                                Text::new("U/J: Time Speed +/-"),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::linear_rgb(0.8, 1.0, 1.0)),
                            ));
                        });
                });

            // Bottom UI panel - Game controls
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(80.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::End,
                    padding: UiRect::all(Val::Px(20.0)),
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    ..default()
                })
                .with_children(|bottom_panel| {
                    // Game control buttons
                    let button_style = Node {
                        width: Val::Px(100.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::horizontal(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    };

                    // Reset button
                    bottom_panel
                        .spawn((
                            Button,
                            button_style.clone(),
                            BackgroundColor(Color::BLACK),
                            BorderColor(Color::WHITE),
                            ResetButton,
                        ))
                        .with_child((
                            Text::new("RESET"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));

                    // Pause button
                    bottom_panel
                        .spawn((
                            Button,
                            button_style.clone(),
                            BackgroundColor(Color::BLACK),
                            BorderColor(Color::WHITE),
                            PauseButton,
                        ))
                        .with_child((
                            Text::new("PAUSE"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));

                    // Quit button
                    bottom_panel
                        .spawn((
                            Button,
                            button_style,
                            BackgroundColor(Color::BLACK),
                            BorderColor(Color::WHITE),
                            QuitButton,
                        ))
                        .with_child((
                            Text::new("QUIT"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                });
        });
}

// UI component markers
#[derive(Component)]
struct TimeDisplay;

#[derive(Component)]
struct FuelDisplay;

#[derive(Component)]
struct VelocityDisplay;

#[derive(Component)]
struct ThrustDisplay;

#[derive(Component)]
struct ResetButton;

#[derive(Component)]
struct PauseButton;

#[derive(Component)]
struct QuitButton;

#[derive(Component)]
struct AngularVelocityDisplay;

#[derive(Component)]
struct GravityDisplay;

#[derive(Component)]
struct ThrustMultiplierDisplay;

#[derive(Component)]
struct TimeMultiplierDisplay;

fn handle_game_input(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut lander_query: Query<(&mut Transform, &mut Lander), (With<Player>, Without<GameCamera>)>,
    mut game_data: ResMut<GameData>,
    mut camera_query: Query<(&mut CameraController, &mut Transform), With<GameCamera>>,
    mut simulation_settings: ResMut<SimulationSettings>,
    mut terrain_data: ResMut<TerrainData>,
    mut next_state: ResMut<NextState<GameState>>,
    mut button_query: Query<
        (
            &Interaction,
            Option<&ResetButton>,
            Option<&PauseButton>,
            Option<&QuitButton>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    // Handle button clicks
    for (interaction, reset, pause, quit) in button_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            if reset.is_some() {
                // Reset game state
                game_data.time = 0.0;
                game_data.fuel = STARTING_FUEL;
                game_data.score = 0;

                // Regenerate the same deterministic hierarchy before deriving
                // the spawn frame and its inertial orbital velocity.
                terrain_data.generate_planets(5);

                // Reset lander state
                if let Ok((mut transform, mut lander)) = lander_query.single_mut() {
                    let spawn_body = &terrain_data.planets[1];
                    transform.translation =
                        (spawn_body.center + Vec2::Y * (spawn_body.radius + 120.0)).extend(0.0);
                    transform.rotation = Quat::IDENTITY;
                    lander.velocity = terrain_data.orbital_velocity(1);
                    lander.angular_velocity = 0.0;
                    lander.main_thrust = 0.0;
                    lander.left_thrust = 0.0;
                    lander.right_thrust = 0.0;
                    lander.reverse_thrust = 0.0;
                    lander.angular_thrust = 0.0;
                }

                // Reset camera
                if let Ok((mut camera_controller, _)) = camera_query.single_mut() {
                    camera_controller.zoom = DEFAULT_CAMERA_ZOOM;
                    camera_controller.target_zoom = DEFAULT_CAMERA_ZOOM;
                    camera_controller.pan_offset = Vec2::ZERO;
                    camera_controller.follow_player = true;
                    camera_controller.is_dragging = false;
                    camera_controller.last_mouse_pos = None;
                }

                // Reset simulation settings
                *simulation_settings = SimulationSettings::default();

                return;
            } else if pause.is_some() {
                next_state.set(GameState::Paused);
                return;
            } else if quit.is_some() {
                next_state.set(GameState::Menu);
                return;
            }
        }
    }

    // Handle lander controls
    if let Ok((_transform, mut lander)) = lander_query.single_mut() {
        let dt = time.delta_secs().min(0.1);
        let can_thrust = game_data.fuel > 0.0 || simulation_settings.infinite_fuel;
        let pressed = |primary, alternate| {
            can_thrust && (keyboard_input.pressed(primary) || keyboard_input.pressed(alternate))
        };

        let main_target = if pressed(KeyCode::KeyW, KeyCode::ArrowUp) {
            MAX_MAIN_THRUST * lander.thrust_scale
        } else {
            0.0
        };
        let reverse_target = if pressed(KeyCode::KeyS, KeyCode::ArrowDown) {
            MAX_REVERSE_THRUST * lander.thrust_scale
        } else {
            0.0
        };
        // A/D are translation controls. Arrow-left/right command a balanced
        // RCS couple, producing torque without the old accidental side force.
        let left_target = if can_thrust && keyboard_input.pressed(KeyCode::KeyA) {
            MAX_SIDE_THRUST * lander.thrust_scale
        } else {
            0.0
        };
        let right_target = if can_thrust && keyboard_input.pressed(KeyCode::KeyD) {
            MAX_SIDE_THRUST * lander.thrust_scale
        } else {
            0.0
        };
        let angular_target = if can_thrust {
            let left = keyboard_input.pressed(KeyCode::ArrowLeft) as i8 as f32;
            let right = keyboard_input.pressed(KeyCode::ArrowRight) as i8 as f32;
            (left - right) * MAX_ANGULAR_THRUST * lander.maneuverability
        } else {
            0.0
        };

        lander.main_thrust = approach_throttle(lander.main_thrust, main_target, dt);
        lander.reverse_thrust = approach_throttle(lander.reverse_thrust, reverse_target, dt);
        lander.left_thrust = approach_throttle(lander.left_thrust, left_target, dt);
        lander.right_thrust = approach_throttle(lander.right_thrust, right_target, dt);
        lander.angular_thrust = approach_throttle(lander.angular_thrust, angular_target, dt);

        // Only apply thrust if we have fuel or infinite fuel is enabled
        if can_thrust && !simulation_settings.infinite_fuel {
            let fuel_rate = FUEL_CONSUMPTION_RATE
                * (lander.main_thrust / MAX_MAIN_THRUST
                    + 0.4 * lander.reverse_thrust / MAX_REVERSE_THRUST
                    + 0.6 * lander.left_thrust / MAX_SIDE_THRUST
                    + 0.6 * lander.right_thrust / MAX_SIDE_THRUST
                    + 0.5 * lander.angular_thrust.abs() / MAX_ANGULAR_THRUST);
            game_data.fuel = (game_data.fuel - fuel_rate * dt).max(0.0);
        }
    }

    // Camera controls
    if let Ok((mut camera_controller, mut camera_transform)) = camera_query.single_mut() {
        // Zoom controls
        if keyboard_input.pressed(KeyCode::Equal) || keyboard_input.pressed(KeyCode::NumpadAdd) {
            camera_controller.target_zoom =
                (camera_controller.target_zoom * 1.1).min(MAX_CAMERA_ZOOM);
        }
        if keyboard_input.pressed(KeyCode::Minus) || keyboard_input.pressed(KeyCode::NumpadSubtract)
        {
            camera_controller.target_zoom =
                (camera_controller.target_zoom * 0.9).max(MIN_CAMERA_ZOOM);
        }
        for event in mouse_wheel_events.read() {
            let factor = (-event.y * 0.12).exp();
            camera_controller.target_zoom =
                (camera_controller.target_zoom * factor).clamp(MIN_CAMERA_ZOOM, MAX_CAMERA_ZOOM);
        }

        // Reset camera
        if keyboard_input.just_pressed(KeyCode::KeyR) {
            camera_controller.target_zoom = DEFAULT_CAMERA_ZOOM;
            camera_controller.pan_offset = Vec2::ZERO;
            camera_controller.follow_player = true;
        }

        // Toggle camera follow
        if keyboard_input.just_pressed(KeyCode::KeyF) {
            let follow = !camera_controller.follow_player;
            set_camera_follow(
                &mut camera_controller,
                camera_transform.translation.truncate(),
                follow,
            );
        }

        // Dragging pans both modes. In follow mode this is a temporary look;
        // releasing the mouse snaps the view back to the lander.
        if mouse_input.just_pressed(MouseButton::Left) {
            camera_controller.is_dragging = true;
            camera_controller.last_mouse_pos = cursor_moved_events
                .read()
                .last()
                .map(|event| event.position);
        } else if camera_controller.is_dragging {
            for event in cursor_moved_events.read() {
                let current_mouse_pos = event.position;
                if let Some(last_pos) = camera_controller.last_mouse_pos {
                    let mouse_delta = current_mouse_pos - last_pos;
                    let world_delta =
                        Vec2::new(-mouse_delta.x, mouse_delta.y) * camera_controller.zoom;
                    camera_controller.pan_offset += world_delta;
                }
                camera_controller.last_mouse_pos = Some(current_mouse_pos);
            }
        } else {
            cursor_moved_events.clear();
        }
        if mouse_input.just_released(MouseButton::Left) {
            camera_controller.is_dragging = false;
            camera_controller.last_mouse_pos = None;
            if camera_controller.follow_player {
                camera_controller.pan_offset = Vec2::ZERO;
                if let Ok((lander_transform, _)) = lander_query.single() {
                    camera_transform.translation.x = lander_transform.translation.x;
                    camera_transform.translation.y = lander_transform.translation.y;
                }
            }
        }

        // Free-camera recenter keeps follow disabled so the pilot can inspect
        // another area again without changing modes.
        if !camera_controller.follow_player && keyboard_input.just_pressed(KeyCode::Space) {
            if let Ok((lander_transform, _)) = lander_query.single() {
                let center = lander_transform.translation.truncate();
                camera_controller.pan_offset = center;
                camera_transform.translation.x = center.x;
                camera_transform.translation.y = center.y;
            }
        }
    }

    // Simulation settings controls
    // Gravity multiplier controls (G/H keys)
    if keyboard_input.pressed(KeyCode::KeyG) {
        simulation_settings.gravity_multiplier =
            (simulation_settings.gravity_multiplier + 0.02).min(10.0);
    }
    if keyboard_input.pressed(KeyCode::KeyH) {
        simulation_settings.gravity_multiplier =
            (simulation_settings.gravity_multiplier - 0.02).max(0.0);
    }

    // Thrust multiplier controls (T/Y keys)
    if keyboard_input.pressed(KeyCode::KeyT) {
        simulation_settings.thrust_multiplier =
            (simulation_settings.thrust_multiplier + 0.02).min(3.0);
    }
    if keyboard_input.pressed(KeyCode::KeyY) {
        simulation_settings.thrust_multiplier =
            (simulation_settings.thrust_multiplier - 0.02).max(0.1);
    }

    // Trajectory prediction controls
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        simulation_settings.show_trajectory = !simulation_settings.show_trajectory;
    }
    if keyboard_input.pressed(KeyCode::BracketLeft) {
        simulation_settings.trajectory_steps =
            (simulation_settings.trajectory_steps.saturating_sub(60)).max(60);
    }
    if keyboard_input.pressed(KeyCode::BracketRight) {
        simulation_settings.trajectory_steps =
            (simulation_settings.trajectory_steps + 60).min(MAX_TRAJECTORY_HORIZON_TICKS);
    }

    // Infinite fuel toggle
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        simulation_settings.infinite_fuel = !simulation_settings.infinite_fuel;
    }

    // Time multiplier controls (U/J keys)
    if keyboard_input.pressed(KeyCode::KeyU) {
        let step = (simulation_settings.time_multiplier * 0.02).max(0.02);
        simulation_settings.time_multiplier =
            (simulation_settings.time_multiplier + step).min(MAX_TIME_MULTIPLIER);
    }
    if keyboard_input.pressed(KeyCode::KeyJ) {
        simulation_settings.time_multiplier = (simulation_settings.time_multiplier - 0.02).max(0.1);
    }
}

fn update_physics(
    time: Res<Time>,
    mut lander_query: Query<(&mut Transform, &mut Lander)>,
    terrain_data: Res<TerrainData>,
    simulation_settings: Res<SimulationSettings>,
    mut game_data: ResMut<GameData>,
) {
    // Shader compilation, panel resizing, and tab suspension can produce a
    // large render delta on wasm. Bound it and substep accelerated simulation
    // time instead of feeding one unstable jump to the orbital integrator.
    let base_dt = time.delta_secs().min(MAX_REAL_FRAME_DT);
    let (substeps, dt) = bounded_physics_step(base_dt, simulation_settings.time_multiplier);
    game_data.time += base_dt;

    for (mut transform, mut lander) in lander_query.iter_mut() {
        for _ in 0..substeps {
            let lander_pos = transform.translation.truncate();
            let total_gravity =
                terrain_data.relativistic_gravity_at_time(lander_pos, lander.velocity, 0.0)
                    * simulation_settings.gravity_multiplier;

            let rotation = transform.rotation;
            let mut total_thrust = Vec2::ZERO;
            let engines_available = simulation_settings.infinite_fuel || game_data.fuel > 0.0;
            if engines_available && lander.main_thrust > 0.0 {
                total_thrust += (rotation * Vec3::Y).truncate() * lander.main_thrust;
            }
            if engines_available && lander.reverse_thrust > 0.0 {
                total_thrust += (rotation * Vec3::NEG_Y).truncate() * lander.reverse_thrust;
            }
            if engines_available && lander.left_thrust > 0.0 {
                total_thrust += (rotation * Vec3::X).truncate() * lander.left_thrust;
            }
            if engines_available && lander.right_thrust > 0.0 {
                total_thrust += (rotation * Vec3::NEG_X).truncate() * lander.right_thrust;
            }
            total_thrust *= simulation_settings.thrust_multiplier;

            let acceleration =
                total_gravity + acceleration_from_force(total_thrust, lander.mass, lander.velocity);
            lander.velocity += acceleration * dt;
            lander.angular_velocity += lander.angular_thrust * dt;
            // Preserve the original 60 Hz damping response under substepping.
            lander.angular_velocity *= 0.98_f32.powf(dt / PHYSICS_STEP_DT);
            transform.translation.x += lander.velocity.x * dt;
            transform.translation.y += lander.velocity.y * dt;
            transform.rotation *= Quat::from_rotation_z(lander.angular_velocity * dt);
        }
    }
}

fn update_planet_orbits(
    time: Res<Time>,
    settings: Res<SimulationSettings>,
    mut terrain: ResMut<TerrainData>,
) {
    terrain.advance_orbits(time.delta_secs().min(MAX_REAL_FRAME_DT) * settings.time_multiplier);
}

fn update_ui(
    game_data: Res<GameData>,
    simulation_settings: Res<SimulationSettings>,
    lander_query: Query<&Lander, With<Player>>,
    mut time_query: Query<
        &mut Text,
        (
            With<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut fuel_query: Query<
        &mut Text,
        (
            With<FuelDisplay>,
            Without<TimeDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut velocity_query: Query<
        &mut Text,
        (
            With<VelocityDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut thrust_query: Query<
        &mut Text,
        (
            With<ThrustDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut angular_velocity_query: Query<
        &mut Text,
        (
            With<AngularVelocityDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut gravity_query: Query<
        &mut Text,
        (
            With<GravityDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
    mut thrust_multiplier_query: Query<
        &mut Text,
        (
            With<ThrustMultiplierDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<TimeMultiplierDisplay>,
        ),
    >,
    mut time_multiplier_query: Query<
        &mut Text,
        (
            With<TimeMultiplierDisplay>,
            Without<TimeDisplay>,
            Without<FuelDisplay>,
            Without<VelocityDisplay>,
            Without<ThrustDisplay>,
            Without<AngularVelocityDisplay>,
            Without<GravityDisplay>,
            Without<ThrustMultiplierDisplay>,
        ),
    >,
) {
    // Update time display
    if let Ok(mut text) = time_query.single_mut() {
        **text = format!("TIME: {:.1}s", game_data.time);
    }

    // Update fuel display
    if let Ok(mut text) = fuel_query.single_mut() {
        if simulation_settings.infinite_fuel {
            **text = "FUEL: ∞ (INFINITE)".to_string();
        } else {
            **text = format!("FUEL: {:.1}", game_data.fuel);
        }
    }

    // Update lander-specific displays
    if let Ok(lander) = lander_query.single() {
        // Update velocity display
        if let Ok(mut text) = velocity_query.single_mut() {
            **text = format!("VEL: ({:.1}, {:.1})", lander.velocity.x, lander.velocity.y);
        }

        // Update thrust display (show total thrust from all thrusters)
        if let Ok(mut text) = thrust_query.single_mut() {
            let total_thrust = lander.main_thrust
                + lander.reverse_thrust
                + lander.left_thrust
                + lander.right_thrust;
            **text = format!("THRUST: {:.1}", total_thrust);
        }

        // Update angular velocity display
        if let Ok(mut text) = angular_velocity_query.single_mut() {
            **text = format!("ROT VEL: {:.2}", lander.angular_velocity);
        }
    }

    // Update simulation settings displays
    if let Ok(mut text) = gravity_query.single_mut() {
        **text = format!("GRAVITY: {:.2}x", simulation_settings.gravity_multiplier);
    }

    if let Ok(mut text) = thrust_multiplier_query.single_mut() {
        **text = format!("THRUST: {:.2}x", simulation_settings.thrust_multiplier);
    }

    if let Ok(mut text) = time_multiplier_query.single_mut() {
        **text = format!("TIME: {:.2}x", simulation_settings.time_multiplier);
    }
}

fn check_game_over(
    lander_query: Query<&Transform, (With<Lander>, With<Player>)>,
    terrain_data: Res<TerrainData>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Ok(transform) = lander_query.single() {
        let lander_pos = transform.translation.truncate();
        let lander_radius = 20.0; // Approximate lander size

        // Check collision with terrain
        if let Some(_collision) = terrain_data.check_collision(lander_pos, lander_radius) {
            // For now, any collision ends the game
            // In the future, we could check landing conditions here
            next_state.set(GameState::GameOver);
        }
    }
}

fn update_camera(
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<GameCamera>>,
    lander_query: Query<&Transform, (With<Lander>, With<Player>, Without<GameCamera>)>,
    time: Res<Time>,
) {
    if let Ok((mut camera_transform, mut controller)) = camera_query.single_mut() {
        // Smooth zoom
        let zoom_speed = 5.0;
        controller.zoom = controller
            .zoom
            .lerp(controller.target_zoom, time.delta_secs() * zoom_speed);

        // Apply zoom to camera transform scale
        camera_transform.scale = Vec3::splat(controller.zoom);

        // Follow player if enabled
        if controller.follow_player {
            if let Ok(lander_transform) = lander_query.single() {
                let target_pos = lander_transform.translation.truncate() + controller.pan_offset;
                let camera_speed = 3.0;
                let current_pos = camera_transform.translation.truncate();
                let new_pos = current_pos.lerp(target_pos, time.delta_secs() * camera_speed);
                camera_transform.translation = new_pos.extend(camera_transform.translation.z);
            }
        } else {
            // When not following player, use pan offset directly
            camera_transform.translation =
                controller.pan_offset.extend(camera_transform.translation.z);
        }
    }
}

fn draw_vector_graphics(
    mut gizmos: Gizmos,
    lander_query: Query<(&Transform, &Lander), With<Player>>,
    camera_query: Query<&CameraController>,
    terrain_data: Res<TerrainData>,
    simulation_settings: Res<SimulationSettings>,
    trajectory_cache: Res<TrajectoryCache>,
) {
    // Terrain is now drawn by the draw_terrain system

    // Draw lander as vector graphics
    if let Ok((transform, lander)) = lander_query.single() {
        let pos = transform.translation.truncate();
        let rotation = transform.rotation;

        if let Ok(camera) = camera_query.single() {
            draw_navigation_indicator(
                &mut gizmos,
                pos,
                rotation,
                lander,
                camera.zoom,
                terrain_data.gravity_at(pos) * simulation_settings.gravity_multiplier,
            );
        }

        // The generated ship renderer owns the hull. This system contributes
        // only navigation and live RCS/thruster effects, avoiding the old
        // triangular lander being drawn over procedural geometry.

        // Reverse thruster (top)
        if lander.reverse_thrust > 0.0 {
            let thrust_magnitude = lander.reverse_thrust / MAX_REVERSE_THRUST;
            let thrust_length = 15.0 * thrust_magnitude;

            let thrust_start = rotation * Vec3::new(0.0, 10.0, 0.0);
            let thrust_direction = rotation * Vec3::new(0.0, 1.0, 0.0); // Opposite to reverse
            let thrust_end = thrust_start + thrust_direction * thrust_length;

            gizmos.line_2d(
                pos + thrust_start.truncate(),
                pos + thrust_end.truncate(),
                Color::srgb(0.8, 0.4, 1.0), // Purple for reverse thrust
            );
        }

        // Left side thruster
        if lander.left_thrust > 0.0 {
            let thrust_magnitude = lander.left_thrust / MAX_SIDE_THRUST;
            let thrust_length = 18.0 * thrust_magnitude;

            let thrust_start = rotation * Vec3::new(-12.0, 0.0, 0.0);
            let thrust_direction = rotation * Vec3::new(-1.0, 0.0, 0.0); // Opposite to left
            let thrust_end = thrust_start + thrust_direction * thrust_length;

            gizmos.line_2d(
                pos + thrust_start.truncate(),
                pos + thrust_end.truncate(),
                Color::srgb(0.0, 1.0, 0.5), // Green for side thrust
            );
        }

        // Right side thruster
        if lander.right_thrust > 0.0 {
            let thrust_magnitude = lander.right_thrust / MAX_SIDE_THRUST;
            let thrust_length = 18.0 * thrust_magnitude;

            let thrust_start = rotation * Vec3::new(12.0, 0.0, 0.0);
            let thrust_direction = rotation * Vec3::new(1.0, 0.0, 0.0); // Opposite to right
            let thrust_end = thrust_start + thrust_direction * thrust_length;

            gizmos.line_2d(
                pos + thrust_start.truncate(),
                pos + thrust_end.truncate(),
                Color::srgb(0.0, 1.0, 0.5), // Green for side thrust
            );
        }

        // Draw rotation thrust visualization from tip nozzle
        if lander.angular_thrust.abs() > 0.0 {
            let angular_thrust_magnitude = lander.angular_thrust.abs() / MAX_ANGULAR_THRUST;
            let thrust_length = 15.0 * angular_thrust_magnitude;

            // Nozzle position at the tip of the lander
            let nozzle_pos = rotation * Vec3::new(0.0, 15.0, 0.0);

            // Thrust direction perpendicular to the lander's orientation
            let thrust_direction = if lander.angular_thrust > 0.0 {
                // Clockwise rotation - thrust to the right
                rotation * Vec3::new(1.0, 0.0, 0.0)
            } else {
                // Counter-clockwise rotation - thrust to the left
                rotation * Vec3::new(-1.0, 0.0, 0.0)
            };

            let thrust_end = nozzle_pos + thrust_direction * thrust_length;

            gizmos.line_2d(
                pos + nozzle_pos.truncate(),
                pos + thrust_end.truncate(),
                Color::srgb(0.0, 0.8, 1.0), // Cyan for rotation thrust
            );

            // Small particles for rotation thrust
            for i in 0..2 {
                let offset = (i as f32 - 0.5) * 2.0;
                let particle_start = nozzle_pos + rotation * Vec3::new(0.0, offset, 0.0);
                let particle_end = particle_start + thrust_direction * thrust_length * 0.6;

                gizmos.line_2d(
                    pos + particle_start.truncate(),
                    pos + particle_end.truncate(),
                    Color::srgb(0.5, 0.9, 1.0), // Light cyan particles
                );
            }
        }

        // Draw velocity vector (for debugging)
        if lander.velocity.length() > 5.0 {
            let vel_end = pos + lander.velocity * 0.1;
            gizmos.line_2d(pos, vel_end, Color::srgb(0.0, 1.0, 0.0));
        }

        // Draw trajectory prediction
        if simulation_settings.show_trajectory {
            let marker_zoom = camera_query
                .single()
                .map_or(DEFAULT_CAMERA_ZOOM, |camera| camera.zoom);
            draw_cached_collision_markers(&mut gizmos, &trajectory_cache, marker_zoom);
        }
    }
}

fn update_trajectory_meshes(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    game_data: Res<GameData>,
    lander_query: Query<(&Transform, &Lander), With<Player>>,
    camera_query: Query<&CameraController, With<GameCamera>>,
    terrain_data: Res<TerrainData>,
    simulation_settings: Res<SimulationSettings>,
    mut cache: ResMut<TrajectoryCache>,
    visuals: Query<(
        &TrajectoryVisual,
        &Mesh2d,
        &MeshMaterial2d<TrajectoryMaterial>,
        &mut Visibility,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TrajectoryMaterial>>,
) {
    let Ok((transform, lander)) = lander_query.single() else {
        return;
    };
    if !simulation_settings.show_trajectory {
        for (_, _, _, mut visibility) in visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let control_intent = TrajectoryControlIntent::from_keyboard(&keyboard_input);
    let has_control_input = lander_has_control_input(lander) || control_intent.is_active();
    // Keep the held-input forecast alive even when the tank reaches zero. Its
    // integrator will correctly apply no thrust and converge onto the coast
    // path; hiding the visual here caused the ribbon to pop out exactly at
    // fuel exhaustion (and made infinite-fuel toggles look like corruption).
    let has_active_input = has_control_input;
    let zoom = camera_query
        .single()
        .map_or(DEFAULT_CAMERA_ZOOM, |camera| camera.zoom);
    let position = transform.translation.truncate();
    let now = time.elapsed_secs();
    let prediction_stale = now - cache.computed_at >= 1.0 / 30.0
        || cache.coast.is_none()
        || cache.trajectory_steps != simulation_settings.trajectory_steps
        || cache.had_active_input != has_active_input
        || cache.infinite_fuel != simulation_settings.infinite_fuel
        || (cache.gravity_multiplier - simulation_settings.gravity_multiplier).abs() > 0.0001
        || (cache.thrust_multiplier - simulation_settings.thrust_multiplier).abs() > 0.0001
        || cache.anchor_position.distance_squared(position) > 2_500.0_f32.powi(2);
    if prediction_stale {
        cache.coast = Some(predict_trajectory(
            transform,
            lander,
            &terrain_data,
            &simulation_settings,
            false,
            game_data.fuel,
            TrajectoryControlIntent::default(),
        ));
        cache.active = has_active_input.then(|| {
            predict_trajectory(
                transform,
                lander,
                &terrain_data,
                &simulation_settings,
                true,
                game_data.fuel,
                control_intent,
            )
        });
        cache.computed_at = now;
        cache.trajectory_steps = simulation_settings.trajectory_steps;
        cache.had_active_input = has_active_input;
        cache.infinite_fuel = simulation_settings.infinite_fuel;
        cache.gravity_multiplier = simulation_settings.gravity_multiplier;
        cache.thrust_multiplier = simulation_settings.thrust_multiplier;
        cache.anchor_position = position;
    }

    // Zoom only changes screen-space ribbon width and dot spacing in the
    // material. Re-simplifying the curve based on zoom selected different
    // knots at arbitrary thresholds and made the trajectory visibly pop or
    // disappear during wheel input.
    let visual_stale = prediction_stale;
    let curve_tolerance = 0.65;

    for (kind, mesh_handle, material_handle, mut visibility) in visuals {
        let prediction = match kind {
            TrajectoryVisual::Coast => cache.coast.as_ref(),
            TrajectoryVisual::ActiveInput => cache.active.as_ref(),
        };
        if visual_stale {
            if let (Some(prediction), Some(mesh)) = (prediction, meshes.get_mut(mesh_handle)) {
                *mesh = trajectory_curve_mesh(prediction, curve_tolerance);
            }
        }
        if let Some(material) = materials.get_mut(material_handle) {
            material.params.half_width =
                (1.6 * (1.0 + 0.18 * (zoom / DEFAULT_CAMERA_ZOOM).max(1.0).ln())).clamp(1.6, 3.4);
            material.params.dash_period = match kind {
                TrajectoryVisual::Coast => 11.0 * zoom,
                TrajectoryVisual::ActiveInput => 13.0 * zoom,
            };
            material.params.sample_count = prediction.map_or(0, |value| value.points.len() as u32);
            material.params.sample_dt = prediction.map_or(1.0 / 60.0, |value| value.sample_dt);
        }
        *visibility = if prediction.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn lander_has_control_input(lander: &Lander) -> bool {
    lander.main_thrust > 0.0
        || lander.reverse_thrust > 0.0
        || lander.left_thrust > 0.0
        || lander.right_thrust > 0.0
        || lander.angular_thrust.abs() > f32::EPSILON
}

#[derive(Clone, Copy, Default)]
struct TrajectoryControlIntent {
    main: bool,
    reverse: bool,
    left: bool,
    right: bool,
    rotation: f32,
}

impl TrajectoryControlIntent {
    fn from_keyboard(keyboard: &ButtonInput<KeyCode>) -> Self {
        Self {
            main: keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp),
            reverse: keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown),
            left: keyboard.pressed(KeyCode::KeyA),
            right: keyboard.pressed(KeyCode::KeyD),
            rotation: keyboard.pressed(KeyCode::ArrowLeft) as i8 as f32
                - keyboard.pressed(KeyCode::ArrowRight) as i8 as f32,
        }
    }

    fn is_active(self) -> bool {
        self.main || self.reverse || self.left || self.right || self.rotation != 0.0
    }
}

fn trajectory_curve_mesh(prediction: &TrajectoryPrediction, tolerance: f32) -> Mesh {
    let visible_len = prediction.visible_len.clamp(1, prediction.points.len());
    let knot_indices = adaptive_curve_indices(
        &prediction.points[..visible_len],
        tolerance,
        MAX_CURVE_KNOTS,
    );
    if knot_indices.len() < 2 {
        return empty_trajectory_mesh();
    }

    let spans = knot_indices.len() - 1;
    let vertex_count = spans * (CURVE_SUBDIVISIONS + 1) * 2;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut ends = Vec::with_capacity(vertex_count);
    let mut tangent_pairs = Vec::with_capacity(vertex_count);
    let mut params = Vec::with_capacity(vertex_count);
    let mut sample_ids = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(spans * CURVE_SUBDIVISIONS * 6);
    let mut distance = 0.0;

    for span in 0..spans {
        let start_index = knot_indices[span];
        let end_index = knot_indices[span + 1];
        let p0 = prediction.points[start_index];
        let p1 = prediction.points[end_index];
        let chord = p0.distance(p1);
        let span_seconds = (end_index - start_index) as f32 * prediction.sample_dt;
        // Hermite derivatives come from the actual predicted state, not from
        // an aesthetic guess based on neighboring chords. The magnitude cap
        // prevents a sparse LOD span from overshooting far beyond its samples.
        let tangent_limit = chord * 2.5;
        let m0 = bounded_curve_tangent(
            prediction.velocities[start_index] * span_seconds,
            tangent_limit,
            p1 - p0,
        );
        let m1 = bounded_curve_tangent(
            prediction.velocities[end_index] * span_seconds,
            tangent_limit,
            p1 - p0,
        );
        let base_vertex = positions.len() as u32;
        for step in 0..=CURVE_SUBDIVISIONS {
            let t = step as f32 / CURVE_SUBDIVISIONS as f32;
            let base = p0.lerp(p1, t);
            let progress = (span as f32 + t) / spans as f32;
            let alpha = 1.0 - progress * 0.75;
            for side in [-1.0, 1.0] {
                // The position bounds follow the chord for correct frustum
                // culling; the shader replaces it with the Hermite position.
                positions.push([base.x, base.y, 0.0]);
                ends.push([p0.x, p0.y, p1.x, p1.y]);
                tangent_pairs.push([m0.x, m0.y, m1.x, m1.y]);
                params.push([t, side, alpha, distance + chord * t]);
                sample_ids.push([start_index as f32, end_index as f32]);
            }
        }
        for step in 0..CURVE_SUBDIVISIONS as u32 {
            let a = base_vertex + step * 2;
            indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
        }
        distance += chord;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(ATTRIBUTE_CURVE_ENDS, ends);
    mesh.insert_attribute(ATTRIBUTE_CURVE_TANGENTS, tangent_pairs);
    mesh.insert_attribute(ATTRIBUTE_CURVE_PARAMS, params);
    mesh.insert_attribute(ATTRIBUTE_CURVE_SAMPLE_IDS, sample_ids);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn bounded_curve_tangent(tangent: Vec2, limit: f32, fallback: Vec2) -> Vec2 {
    if !tangent.is_finite() {
        return fallback;
    }
    tangent.clamp_length_max(limit.max(0.001))
}

fn adaptive_curve_indices(points: &[Vec2], initial_tolerance: f32, max_knots: usize) -> Vec<usize> {
    if points.len() <= 2 {
        return (0..points.len()).collect();
    }
    let mut tolerance = initial_tolerance;
    let mut indices = simplify_curve_indices(points, tolerance);
    for _ in 0..10 {
        if indices.len() <= max_knots {
            break;
        }
        tolerance *= 1.7;
        indices = simplify_curve_indices(points, tolerance);
    }
    if indices.len() > max_knots {
        let last = indices.len() - 1;
        indices = (0..max_knots)
            .map(|index| indices[index * last / (max_knots - 1)])
            .collect();
    }
    indices
}

fn simplify_curve_indices(points: &[Vec2], tolerance: f32) -> Vec<usize> {
    let last = points.len() - 1;
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[last] = true;
    let mut stack = vec![(0, last)];
    let tolerance_squared = tolerance * tolerance;

    while let Some((start, end)) = stack.pop() {
        if end <= start + 1 {
            continue;
        }
        let segment = points[end] - points[start];
        let length_squared = segment.length_squared();
        let mut furthest = start;
        let mut maximum_error = 0.0;
        for index in start + 1..end {
            let t = if length_squared > f32::EPSILON {
                ((points[index] - points[start]).dot(segment) / length_squared).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let error = points[index].distance_squared(points[start] + segment * t);
            if error > maximum_error {
                maximum_error = error;
                furthest = index;
            }
        }
        if maximum_error > tolerance_squared {
            keep[furthest] = true;
            stack.push((start, furthest));
            stack.push((furthest, end));
        }
    }

    keep.into_iter()
        .enumerate()
        .filter_map(|(index, keep)| keep.then_some(index))
        .collect()
}

#[cfg(test)]
mod trajectory_curve_tests {
    use super::*;

    #[test]
    fn straight_prediction_collapses_to_endpoints() {
        let points: Vec<_> = (0..100).map(|x| Vec2::new(x as f32, 0.0)).collect();
        let knots = adaptive_curve_indices(&points, 0.1, MAX_CURVE_KNOTS);
        assert_eq!(knots, vec![0, 99]);
    }

    #[test]
    fn adaptive_knots_preserve_bends_and_endpoints() {
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(20.0, 20.0),
            Vec2::new(30.0, 20.0),
        ];
        let knots = adaptive_curve_indices(&points, 0.5, MAX_CURVE_KNOTS);
        assert_eq!(knots.first(), Some(&0));
        assert_eq!(knots.last(), Some(&(points.len() - 1)));
        assert!(knots.len() > 2);
    }

    #[test]
    fn adaptive_knots_respect_gpu_budget() {
        let points: Vec<_> = (0..2_000)
            .map(|index| {
                let x = index as f32 * 0.1;
                Vec2::new(x, (x * 2.7).sin() * 50.0)
            })
            .collect();
        let knots = adaptive_curve_indices(&points, 0.01, 64);
        assert_eq!(knots.first(), Some(&0));
        assert_eq!(knots.last(), Some(&(points.len() - 1)));
        assert!(knots.len() <= 64);
    }

    #[test]
    fn renderer_hitches_cannot_become_giant_physics_steps() {
        let (substeps, dt) = bounded_physics_step(2.0, 1.0);
        assert_eq!(substeps, 2);
        assert!(dt <= PHYSICS_STEP_DT);
        assert!((substeps as f32 * dt - MAX_REAL_FRAME_DT).abs() < 0.000_001);
    }

    #[test]
    fn curve_tangents_are_finite_and_bounded() {
        let tangent = bounded_curve_tangent(Vec2::new(100.0, 0.0), 5.0, Vec2::Y);
        assert!((tangent.length() - 5.0).abs() < 0.000_001);
        assert_eq!(
            bounded_curve_tangent(Vec2::splat(f32::NAN), 5.0, Vec2::Y),
            Vec2::Y
        );
    }

    #[test]
    fn fuel_mode_changes_keep_a_valid_active_control_prediction() {
        let mut terrain = TerrainData::new_with_seed(17);
        terrain.generate_planets(5);
        let (position, velocity) = terrain.circular_orbit_state(1, 240.0).unwrap();
        let transform = Transform::from_translation(position.extend(0.0));
        let lander = Lander {
            velocity,
            angular_velocity: 0.0,
            main_thrust: MAX_MAIN_THRUST,
            left_thrust: 0.0,
            right_thrust: 0.0,
            reverse_thrust: 0.0,
            angular_thrust: 0.0,
            mass: 1.0,
            thrust_scale: 1.0,
            maneuverability: 1.0,
        };
        let mut settings = SimulationSettings::default();
        let coast = predict_trajectory(
            &transform,
            &lander,
            &terrain,
            &settings,
            false,
            0.0,
            TrajectoryControlIntent::default(),
        );
        let no_fuel = predict_trajectory(
            &transform,
            &lander,
            &terrain,
            &settings,
            true,
            0.0,
            TrajectoryControlIntent::default(),
        );
        settings.infinite_fuel = true;
        let infinite = predict_trajectory(
            &transform,
            &lander,
            &terrain,
            &settings,
            true,
            0.0,
            TrajectoryControlIntent::default(),
        );

        assert_eq!(coast.points.len(), no_fuel.points.len());
        assert_eq!(coast.points.len(), infinite.points.len());
        assert!(lander_has_control_input(&lander));
        let mut held_keys = ButtonInput::default();
        held_keys.press(KeyCode::KeyW);
        assert!(TrajectoryControlIntent::from_keyboard(&held_keys).is_active());
        assert!(
            coast
                .points
                .last()
                .unwrap()
                .distance(*no_fuel.points.last().unwrap())
                < 0.01
        );
        assert!(
            coast
                .points
                .last()
                .unwrap()
                .distance(*infinite.points.last().unwrap())
                > 1.0
        );
        assert!(infinite.points.iter().all(|point| point.is_finite()));
    }

    #[test]
    fn held_command_projects_throttle_ramp_before_live_thrust_has_risen() {
        let mut terrain = TerrainData::new_with_seed(23);
        terrain.generate_planets(5);
        let (position, velocity) = terrain.circular_orbit_state(1, 240.0).unwrap();
        let transform = Transform::from_translation(position.extend(0.0));
        let lander = Lander {
            velocity,
            angular_velocity: 0.0,
            main_thrust: 0.0,
            left_thrust: 0.0,
            right_thrust: 0.0,
            reverse_thrust: 0.0,
            angular_thrust: 0.0,
            mass: 1.0,
            thrust_scale: 1.0,
            maneuverability: 1.0,
        };
        let settings = SimulationSettings::default();
        let coast = predict_trajectory(
            &transform,
            &lander,
            &terrain,
            &settings,
            false,
            100.0,
            TrajectoryControlIntent::default(),
        );
        let held = predict_trajectory(
            &transform,
            &lander,
            &terrain,
            &settings,
            true,
            100.0,
            TrajectoryControlIntent {
                main: true,
                ..Default::default()
            },
        );

        assert_eq!(coast.points.len(), held.points.len());
        assert!(
            coast
                .points
                .last()
                .unwrap()
                .distance(*held.points.last().unwrap())
                > 1.0
        );
    }
}

fn draw_navigation_indicator(
    gizmos: &mut Gizmos,
    position: Vec2,
    rotation: Quat,
    lander: &Lander,
    zoom: f32,
    acceleration: Vec2,
) {
    if zoom < 4.0 {
        return;
    }

    // World-space sizes grow with camera scale, yielding stable screen-space
    // symbology when the physical ship becomes sub-pixel.
    let ring_radius = 18.0 * zoom;
    gizmos.circle_2d(position, ring_radius, Color::srgba(0.82, 0.88, 0.94, 0.75));

    let heading = (rotation * Vec3::Y).truncate().normalize_or_zero();
    gizmos.arrow_2d(
        position,
        position + heading * ring_radius * 1.35,
        Color::srgb(0.92, 0.94, 1.0),
    );

    if lander.velocity.length_squared() > 0.01 {
        let length = ring_radius * (1.2 + lander.velocity.length().ln_1p() * 0.16).min(2.8);
        gizmos.arrow_2d(
            position,
            position + lander.velocity.normalize() * length,
            Color::srgb(0.25, 1.0, 0.55),
        );
    }

    if acceleration.length_squared() > 0.0001 {
        let length = ring_radius * (1.0 + acceleration.length().ln_1p() * 0.12).min(2.2);
        gizmos.arrow_2d(
            position,
            position + acceleration.normalize() * length,
            Color::srgb(1.0, 0.58, 0.28),
        );
    }

    if lander.angular_velocity.abs() > 0.01 {
        let color = if lander.angular_velocity > 0.0 {
            Color::srgb(0.4, 0.75, 1.0)
        } else {
            Color::srgb(0.85, 0.5, 1.0)
        };
        let side = lander.angular_velocity.signum();
        let tangent = Vec2::new(-heading.y, heading.x) * side;
        gizmos.arrow_2d(
            position + heading * ring_radius,
            position + heading * ring_radius + tangent * ring_radius * 0.65,
            color,
        );
    }
}

fn draw_cached_collision_markers(gizmos: &mut Gizmos, cache: &TrajectoryCache, zoom: f32) {
    if let Some(coast) = &cache.coast {
        draw_dotted_prediction(gizmos, coast, Color::srgb(0.25, 0.8, 1.0), zoom);
        draw_collision_dots(gizmos, &coast.collisions, Color::srgb(1.0, 0.2, 0.15), zoom);
        draw_trajectory_endpoint(
            gizmos,
            coast.points.last().copied(),
            Color::srgb(0.25, 0.8, 1.0),
            zoom,
        );
    }
    if let Some(active) = &cache.active {
        draw_dotted_prediction(gizmos, active, Color::srgb(1.0, 0.88, 0.08), zoom);
        draw_collision_dots(
            gizmos,
            &active.collisions,
            Color::srgb(1.0, 0.35, 0.75),
            zoom,
        );
        draw_trajectory_endpoint(
            gizmos,
            active.points.last().copied(),
            Color::srgb(1.0, 0.9, 0.1),
            zoom,
        );
    }
}

fn draw_dotted_prediction(
    gizmos: &mut Gizmos,
    prediction: &TrajectoryPrediction,
    color: Color,
    zoom: f32,
) {
    let visible_len = prediction.visible_len.min(prediction.points.len());
    let points = &prediction.points[..visible_len];
    if points.len() < 2 {
        return;
    }

    // Camera transform scale maps one screen pixel to roughly `zoom` world
    // units. Accumulating physical distance before emitting a tiny cross keeps
    // dots stable and legible without making the simulation horizon depend on
    // zoom. Gizmos themselves are rendered through Bevy's wgpu pipeline.
    let spacing = 9.0 * zoom.max(MIN_CAMERA_ZOOM);
    let radius = 1.35 * zoom.max(MIN_CAMERA_ZOOM);
    let mut distance_to_next = 0.0;
    let mut previous = points[0];
    draw_prediction_dot(gizmos, previous, radius, color);

    for &point in &points[1..] {
        let segment = point - previous;
        let length = segment.length();
        if length > f32::EPSILON {
            let direction = segment / length;
            let mut travelled = spacing - distance_to_next;
            while travelled <= length {
                draw_prediction_dot(gizmos, previous + direction * travelled, radius, color);
                travelled += spacing;
            }
            distance_to_next = (distance_to_next + length) % spacing;
        }
        previous = point;
    }
}

fn draw_prediction_dot(gizmos: &mut Gizmos, point: Vec2, radius: f32, color: Color) {
    gizmos.line_2d(point - Vec2::X * radius, point + Vec2::X * radius, color);
    gizmos.line_2d(point - Vec2::Y * radius, point + Vec2::Y * radius, color);
}

fn draw_trajectory_endpoint(gizmos: &mut Gizmos, endpoint: Option<Vec2>, color: Color, zoom: f32) {
    if let Some(endpoint) = endpoint {
        gizmos.circle_2d(endpoint, 3.0 * zoom, color);
    }
}

struct TrajectoryPrediction {
    points: Vec<Vec2>,
    velocities: Vec<Vec2>,
    collisions: Vec<Vec2>,
    sample_dt: f32,
    visible_len: usize,
}

#[derive(Resource, Default)]
struct TrajectoryCache {
    coast: Option<TrajectoryPrediction>,
    active: Option<TrajectoryPrediction>,
    computed_at: f32,
    trajectory_steps: u32,
    had_active_input: bool,
    anchor_position: Vec2,
    infinite_fuel: bool,
    gravity_multiplier: f32,
    thrust_multiplier: f32,
}

fn predict_trajectory(
    transform: &Transform,
    lander: &Lander,
    terrain_data: &TerrainData,
    simulation_settings: &SimulationSettings,
    apply_current_input: bool,
    initial_fuel: f32,
    control_intent: TrajectoryControlIntent,
) -> TrajectoryPrediction {
    // `trajectory_steps` stores the requested horizon in 60 Hz ticks. Long
    // interplanetary projections use a wider integration step so the 100x
    // range increase does not create hundreds of thousands of samples twice
    // per rendered frame.
    let sample_count = simulation_settings
        .trajectory_steps
        .min(MAX_TRAJECTORY_SAMPLES);
    let horizon_seconds = simulation_settings.trajectory_steps as f32 / 60.0;
    let dt = horizon_seconds / sample_count as f32;
    let mut pred_pos = transform.translation.truncate();
    let mut pred_velocity = lander.velocity;
    let mut pred_rotation = transform.rotation;
    let mut pred_angular_velocity = lander.angular_velocity;
    let mut pred_main_thrust = lander.main_thrust;
    let mut pred_reverse_thrust = lander.reverse_thrust;
    let mut pred_left_thrust = lander.left_thrust;
    let mut pred_right_thrust = lander.right_thrust;
    let mut pred_angular_thrust = lander.angular_thrust;
    let mut points = Vec::with_capacity(sample_count as usize + 1);
    let mut velocities = Vec::with_capacity(sample_count as usize + 1);
    let mut collisions = Vec::new();
    let mut was_colliding = false;
    let mut pred_fuel = initial_fuel;
    let mut visible_len = sample_count as usize + 1;
    let mut terminated = false;
    points.push(pred_pos);
    velocities.push(pred_velocity);

    for step in 0..sample_count {
        if terminated {
            points.push(pred_pos);
            velocities.push(Vec2::ZERO);
            continue;
        }
        let prediction_time = (step + 1) as f32 * dt;
        let total_gravity =
            terrain_data.relativistic_gravity_at_time(pred_pos, pred_velocity, prediction_time)
                * simulation_settings.gravity_multiplier;
        let mut total_thrust = Vec2::ZERO;
        let can_burn = simulation_settings.infinite_fuel || pred_fuel > f32::EPSILON;

        // A keyboard forecast represents the held command continuing, not a
        // frozen sample of the engine's current throttle. Continue the same
        // valve response curve used by live flight so a new press immediately
        // produces a meaningful yellow projection.
        if apply_current_input && control_intent.is_active() {
            let response_dt = dt.min(0.1);
            pred_main_thrust = approach_throttle(
                pred_main_thrust,
                if control_intent.main {
                    MAX_MAIN_THRUST * lander.thrust_scale
                } else {
                    0.0
                },
                response_dt,
            );
            pred_reverse_thrust = approach_throttle(
                pred_reverse_thrust,
                if control_intent.reverse {
                    MAX_REVERSE_THRUST * lander.thrust_scale
                } else {
                    0.0
                },
                response_dt,
            );
            pred_left_thrust = approach_throttle(
                pred_left_thrust,
                if control_intent.left {
                    MAX_SIDE_THRUST * lander.thrust_scale
                } else {
                    0.0
                },
                response_dt,
            );
            pred_right_thrust = approach_throttle(
                pred_right_thrust,
                if control_intent.right {
                    MAX_SIDE_THRUST * lander.thrust_scale
                } else {
                    0.0
                },
                response_dt,
            );
            pred_angular_thrust = approach_throttle(
                pred_angular_thrust,
                control_intent.rotation * MAX_ANGULAR_THRUST * lander.maneuverability,
                response_dt,
            );
        }

        if apply_current_input && can_burn && pred_main_thrust > 0.0 {
            let forward_dir = pred_rotation * Vec3::new(0.0, 1.0, 0.0);
            total_thrust += forward_dir.truncate() * pred_main_thrust;
        }
        if apply_current_input && can_burn && pred_reverse_thrust > 0.0 {
            let backward_dir = pred_rotation * Vec3::new(0.0, -1.0, 0.0);
            total_thrust += backward_dir.truncate() * pred_reverse_thrust;
        }
        if apply_current_input && can_burn && pred_left_thrust > 0.0 {
            let right_dir = pred_rotation * Vec3::new(1.0, 0.0, 0.0);
            total_thrust += right_dir.truncate() * pred_left_thrust;
        }
        if apply_current_input && can_burn && pred_right_thrust > 0.0 {
            let left_dir = pred_rotation * Vec3::new(-1.0, 0.0, 0.0);
            total_thrust += left_dir.truncate() * pred_right_thrust;
        }
        total_thrust *= simulation_settings.thrust_multiplier;
        if apply_current_input && can_burn && !simulation_settings.infinite_fuel {
            let burn_fraction = pred_main_thrust / MAX_MAIN_THRUST
                + 0.4 * pred_reverse_thrust / MAX_REVERSE_THRUST
                + 0.6 * pred_left_thrust / MAX_SIDE_THRUST
                + 0.6 * pred_right_thrust / MAX_SIDE_THRUST
                + 0.5 * pred_angular_thrust.abs() / MAX_ANGULAR_THRUST;
            pred_fuel = (pred_fuel - FUEL_CONSUMPTION_RATE * burn_fraction * dt).max(0.0);
        }

        pred_velocity += (total_gravity
            + acceleration_from_force(total_thrust, lander.mass, pred_velocity))
            * dt;
        if apply_current_input && can_burn {
            pred_angular_velocity += pred_angular_thrust * dt;
        }
        pred_angular_velocity *= 0.98;

        pred_pos += pred_velocity * dt;
        pred_rotation *= Quat::from_rotation_z(pred_angular_velocity * dt);
        points.push(pred_pos);
        velocities.push(pred_velocity);
        let collision = terrain_data.check_collision_at_time(pred_pos, 20.0, prediction_time);
        if let Some(collision) = collision.as_ref() {
            if !was_colliding {
                collisions.push(collision.point);
                pred_pos = collision.point;
                pred_velocity = Vec2::ZERO;
                if let Some(point) = points.last_mut() {
                    *point = collision.point;
                }
                if let Some(velocity) = velocities.last_mut() {
                    *velocity = Vec2::ZERO;
                }
                visible_len = points.len();
                terminated = true;
            }
            was_colliding = true;
        } else {
            was_colliding = false;
        }
    }
    debug_assert_eq!(points.len(), sample_count as usize + 1);
    debug_assert_eq!(velocities.len(), points.len());
    TrajectoryPrediction {
        points,
        velocities,
        collisions,
        sample_dt: dt,
        visible_len,
    }
}

fn draw_collision_dots(gizmos: &mut Gizmos, collisions: &[Vec2], color: Color, zoom: f32) {
    for &point in collisions {
        // Concentric rings read as a solid impact dot at ordinary zoom while
        // the crosshair remains recognizable in system-scale views.
        for screen_radius in [1.5, 3.0, 4.5] {
            gizmos.circle_2d(point, screen_radius * zoom, color);
        }
        let cross_radius = 5.5 * zoom;
        gizmos.line_2d(
            point + Vec2::splat(-cross_radius),
            point + Vec2::splat(cross_radius),
            color,
        );
        gizmos.line_2d(
            point + Vec2::new(-cross_radius, cross_radius),
            point + Vec2::new(cross_radius, -cross_radius),
            color,
        );
    }
}

fn cleanup_game(
    mut commands: Commands,
    game_entities: Query<
        Entity,
        Or<(
            With<Lander>,
            With<GameUI>,
            With<GameCamera>,
            With<TerrainVisual>,
            With<TrajectoryVisual>,
        )>,
    >,
) {
    for entity in game_entities.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod camera_tests {
    use super::*;

    #[test]
    fn follow_toggle_preserves_free_camera_then_clears_relative_offset() {
        let mut controller = CameraController {
            zoom: 3.0,
            target_zoom: 3.0,
            pan_offset: Vec2::new(10.0, 20.0),
            follow_player: true,
            is_dragging: true,
            last_mouse_pos: Some(Vec2::ONE),
        };
        set_camera_follow(&mut controller, Vec2::new(800.0, -300.0), false);
        assert_eq!(controller.pan_offset, Vec2::new(800.0, -300.0));
        assert!(!controller.follow_player);

        set_camera_follow(&mut controller, Vec2::new(900.0, -250.0), true);
        assert_eq!(controller.pan_offset, Vec2::ZERO);
        assert!(controller.follow_player);
        assert!(!controller.is_dragging);
        assert_eq!(controller.last_mouse_pos, None);
    }
}
