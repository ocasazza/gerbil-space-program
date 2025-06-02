use crate::GameState;
use crate::terrain::{TerrainData, draw_terrain};
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameData>()
            .init_resource::<TerrainData>()
            .init_resource::<SimulationSettings>()
            .add_systems(OnEnter(GameState::Playing), setup_game)
            .add_systems(
                Update,
                (
                    handle_game_input,
                    update_physics,
                    update_camera,
                    update_ui,
                    check_game_over,
                    draw_vector_graphics,
                    draw_terrain,
                ).run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_game);
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
    pub time_multiplier: f32,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            gravity_multiplier: 1.0,
            thrust_multiplier: 1.0,
            show_trajectory: true,
            trajectory_steps: 100,
            infinite_fuel: false,
            time_multiplier: 1.0,
        }
    }
}

#[derive(Component)]
pub struct Lander {
    pub velocity: Vec2,
    pub angular_velocity: f32,
    pub main_thrust: f32,      // Forward thruster (strongest)
    pub left_thrust: f32,      // Left side thruster
    pub right_thrust: f32,     // Right side thruster
    pub reverse_thrust: f32,   // Reverse thruster
    pub angular_thrust: f32,   // Rotation thrusters
    pub mass: f32,
}

#[derive(Component)]
pub struct GameUI;

#[derive(Component)]
pub struct GameCamera;

#[derive(Component)]
pub struct CameraController {
    pub zoom: f32,
    pub target_zoom: f32,
    pub pan_offset: Vec2,
    pub follow_player: bool,
    pub is_dragging: bool,
    pub last_mouse_pos: Option<Vec2>,
}

// Game constants
const GRAVITY: f32 = 98.0; // pixels per second squared
const MAX_MAIN_THRUST: f32 = 150.0;     // Main thruster (strongest)
const MAX_SIDE_THRUST: f32 = 60.0;      // Side thrusters (weaker)
const MAX_REVERSE_THRUST: f32 = 40.0;   // Reverse thruster (weakest)
const MAX_ANGULAR_THRUST: f32 = 3.0;    // Rotation thrusters
const FUEL_CONSUMPTION_RATE: f32 = 10.0;
const STARTING_FUEL: f32 = 100.0;

fn setup_game(mut commands: Commands, mut game_data: ResMut<GameData>, mut terrain_data: ResMut<TerrainData>) {
    info!("Setting up game");

    // Reset game data
    game_data.time = 0.0;
    game_data.fuel = STARTING_FUEL;
    game_data.max_fuel = STARTING_FUEL;
    game_data.score = 0;

    // Spawn camera with controller
    commands.spawn((
        Camera2d,
        Msaa::Off,
        CameraController {
            zoom: 1.0,
            target_zoom: 1.0,
            pan_offset: Vec2::ZERO,
            follow_player: true,
            is_dragging: false,
            last_mouse_pos: None,
        },
        GameCamera,
    ));

    // Spawn the lander (vector graphics only, no sprite)
    commands.spawn((
        Transform::from_translation(Vec3::new(0.0, 300.0, 0.0)),
        Lander {
            velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            main_thrust: 0.0,
            left_thrust: 0.0,
            right_thrust: 0.0,
            reverse_thrust: 0.0,
            angular_thrust: 0.0,
            mass: 1.0,
        },
    ));

    // Generate terrain - multiple planets
    terrain_data.generate_planets(5);

    // Setup UI
    setup_game_ui(&mut commands);
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
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut lander_query: Query<(&mut Transform, &mut Lander)>,
    mut game_data: ResMut<GameData>,
    mut camera_query: Query<&mut CameraController>,
    mut simulation_settings: ResMut<SimulationSettings>,
    mut terrain_data: ResMut<TerrainData>,
    mut next_state: ResMut<NextState<GameState>>,
    mut button_query: Query<
        (&Interaction, Option<&ResetButton>, Option<&PauseButton>, Option<&QuitButton>),
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

                // Reset lander state
                if let Ok((mut transform, mut lander)) = lander_query.single_mut() {
                    transform.translation = Vec3::new(0.0, 300.0, 0.0);
                    transform.rotation = Quat::IDENTITY;
                    lander.velocity = Vec2::ZERO;
                    lander.angular_velocity = 0.0;
                    lander.main_thrust = 0.0;
                    lander.left_thrust = 0.0;
                    lander.right_thrust = 0.0;
                    lander.reverse_thrust = 0.0;
                    lander.angular_thrust = 0.0;
                }

                // Reset camera
                if let Ok(mut camera_controller) = camera_query.single_mut() {
                    camera_controller.zoom = 1.0;
                    camera_controller.target_zoom = 1.0;
                    camera_controller.pan_offset = Vec2::ZERO;
                    camera_controller.follow_player = true;
                    camera_controller.is_dragging = false;
                    camera_controller.last_mouse_pos = None;
                }

                // Reset simulation settings
                *simulation_settings = SimulationSettings::default();

                // Regenerate terrain
                terrain_data.generate_planets(5);

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
        // Reset all thrusters
        lander.main_thrust = 0.0;
        lander.left_thrust = 0.0;
        lander.right_thrust = 0.0;
        lander.reverse_thrust = 0.0;
        lander.angular_thrust = 0.0;

        // Only apply thrust if we have fuel or infinite fuel is enabled
        if game_data.fuel > 0.0 || simulation_settings.infinite_fuel {
            let mut fuel_used = 0.0;

            // Main thruster (forward in lander's direction)
            if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
                lander.main_thrust = MAX_MAIN_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE;
            }

            // Reverse thruster (backward relative to lander)
            if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
                lander.reverse_thrust = MAX_REVERSE_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE * 0.4; // Less fuel for weaker thruster
            }

            // Left side thruster (moves lander right)
            if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
                lander.left_thrust = MAX_SIDE_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE * 0.6; // Moderate fuel for side thruster
            }

            // Right side thruster (moves lander left)
            if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
                lander.right_thrust = MAX_SIDE_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE * 0.6; // Moderate fuel for side thruster
            }

            // Rotation controls (arrow keys)
            if keyboard_input.pressed(KeyCode::ArrowLeft) {
                lander.angular_thrust += MAX_ANGULAR_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE * 0.5;
            }
            if keyboard_input.pressed(KeyCode::ArrowRight) {
                lander.angular_thrust -= MAX_ANGULAR_THRUST;
                fuel_used += FUEL_CONSUMPTION_RATE * 0.5;
            }

            // Consume fuel (only if infinite fuel is disabled)
            if fuel_used > 0.0 && !simulation_settings.infinite_fuel {
                game_data.fuel = (game_data.fuel - fuel_used * 0.016).max(0.0); // Assuming 60 FPS
            }
        }
    }

    // Camera controls
    if let Ok(mut camera_controller) = camera_query.single_mut() {
        // Zoom controls
        if keyboard_input.pressed(KeyCode::Equal) || keyboard_input.pressed(KeyCode::NumpadAdd) {
            camera_controller.target_zoom = (camera_controller.target_zoom * 1.1).min(3.0);
        }
        if keyboard_input.pressed(KeyCode::Minus) || keyboard_input.pressed(KeyCode::NumpadSubtract) {
            camera_controller.target_zoom = (camera_controller.target_zoom * 0.9).max(0.3);
        }

        // Reset camera
        if keyboard_input.just_pressed(KeyCode::KeyR) {
            camera_controller.target_zoom = 1.0;
            camera_controller.pan_offset = Vec2::ZERO;
            camera_controller.follow_player = true;
        }

        // Toggle camera follow
        if keyboard_input.just_pressed(KeyCode::KeyF) {
            camera_controller.follow_player = !camera_controller.follow_player;
        }

        // Mouse drag controls (only when not following player)
        if !camera_controller.follow_player {
            // Handle mouse button press/release
            if mouse_input.just_pressed(MouseButton::Left) {
                camera_controller.is_dragging = true;
            }
            if mouse_input.just_released(MouseButton::Left) {
                camera_controller.is_dragging = false;
                camera_controller.last_mouse_pos = None;
            }

            // Handle mouse movement during drag
            if camera_controller.is_dragging {
                for event in cursor_moved_events.read() {
                    let current_mouse_pos = event.position;

                    if let Some(last_pos) = camera_controller.last_mouse_pos {
                        // Calculate mouse delta
                        let mouse_delta = current_mouse_pos - last_pos;

                        // Convert screen space delta to world space delta
                        // Invert Y axis and scale by zoom level
                        let world_delta = Vec2::new(-mouse_delta.x, mouse_delta.y) * camera_controller.zoom;

                        // Update pan offset
                        camera_controller.pan_offset += world_delta;
                    }

                    camera_controller.last_mouse_pos = Some(current_mouse_pos);
                }
            }
        }
    }

    // Simulation settings controls
    // Gravity multiplier controls (G/H keys)
    if keyboard_input.pressed(KeyCode::KeyG) {
        simulation_settings.gravity_multiplier = (simulation_settings.gravity_multiplier + 0.02).min(10.0);
    }
    if keyboard_input.pressed(KeyCode::KeyH) {
        simulation_settings.gravity_multiplier = (simulation_settings.gravity_multiplier - 0.02).max(0.0);
    }

    // Thrust multiplier controls (T/Y keys)
    if keyboard_input.pressed(KeyCode::KeyT) {
        simulation_settings.thrust_multiplier = (simulation_settings.thrust_multiplier + 0.02).min(3.0);
    }
    if keyboard_input.pressed(KeyCode::KeyY) {
        simulation_settings.thrust_multiplier = (simulation_settings.thrust_multiplier - 0.02).max(0.1);
    }

    // Trajectory prediction controls
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        simulation_settings.show_trajectory = !simulation_settings.show_trajectory;
    }
    if keyboard_input.pressed(KeyCode::BracketLeft) {
        simulation_settings.trajectory_steps = (simulation_settings.trajectory_steps.saturating_sub(5)).max(10);
    }
    if keyboard_input.pressed(KeyCode::BracketRight) {
        simulation_settings.trajectory_steps = (simulation_settings.trajectory_steps + 5).min(10000);
    }

    // Infinite fuel toggle
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        simulation_settings.infinite_fuel = !simulation_settings.infinite_fuel;
    }

    // Time multiplier controls (U/J keys)
    if keyboard_input.pressed(KeyCode::KeyU) {
        simulation_settings.time_multiplier = (simulation_settings.time_multiplier + 0.02).min(5.0);
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
    let base_dt = time.delta_secs();
    let dt = base_dt * simulation_settings.time_multiplier;
    game_data.time += base_dt; // Keep real time for UI

    for (mut transform, mut lander) in lander_query.iter_mut() {
        let lander_pos = transform.translation.truncate();

        // Calculate gravitational forces from all planets
        let mut total_gravity = Vec2::ZERO;
        let mut near_planet = false;

        for planet in &terrain_data.planets {
            let to_planet = planet.center - lander_pos;
            let distance = to_planet.length();

            // Only apply gravity if we're reasonably close to the planet
            if distance < planet.radius * 3.0 && distance > 1.0 {
                // Gravitational force: F = G * m1 * m2 / r^2
                // Simplified: stronger gravity for larger planets, falls off with distance
                let gravity_strength = (planet.radius * 0.5) / (distance * distance) * 10000.0;
                let gravity_direction = to_planet.normalize(); // This points toward the planet (correct)
                total_gravity += gravity_direction * gravity_strength;
                near_planet = true;
            }
        }

        // Only add fallback downward gravity if we're not near any planet
        if !near_planet {
            total_gravity += Vec2::new(0.0, -GRAVITY * 0.1);
        }

        // Apply gravity multiplier
        total_gravity *= simulation_settings.gravity_multiplier;

        // Calculate thrust forces relative to lander orientation
        let rotation = transform.rotation;
        let mut total_thrust = Vec2::ZERO;

        // Main thruster (forward direction)
        if lander.main_thrust > 0.0 {
            let forward_dir = rotation * Vec3::new(0.0, 1.0, 0.0); // Lander's forward direction
            total_thrust += forward_dir.truncate() * lander.main_thrust;
        }

        // Reverse thruster (backward direction)
        if lander.reverse_thrust > 0.0 {
            let backward_dir = rotation * Vec3::new(0.0, -1.0, 0.0); // Lander's backward direction
            total_thrust += backward_dir.truncate() * lander.reverse_thrust;
        }

        // Left side thruster (pushes lander right)
        if lander.left_thrust > 0.0 {
            let right_dir = rotation * Vec3::new(1.0, 0.0, 0.0); // Lander's right direction
            total_thrust += right_dir.truncate() * lander.left_thrust;
        }

        // Right side thruster (pushes lander left)
        if lander.right_thrust > 0.0 {
            let left_dir = rotation * Vec3::new(-1.0, 0.0, 0.0); // Lander's left direction
            total_thrust += left_dir.truncate() * lander.right_thrust;
        }

        // Apply thrust multiplier
        total_thrust *= simulation_settings.thrust_multiplier;

        // Calculate total force
        let total_force = total_gravity + total_thrust;

        // Update velocity (F = ma, so a = F/m)
        let acceleration = total_force / lander.mass;
        lander.velocity += acceleration * dt;

        // Update angular velocity
        lander.angular_velocity += lander.angular_thrust * dt;

        // Apply damping to angular velocity
        lander.angular_velocity *= 0.98;

        // Update position
        transform.translation.x += lander.velocity.x * dt;
        transform.translation.y += lander.velocity.y * dt;

        // Update rotation
        transform.rotation *= Quat::from_rotation_z(lander.angular_velocity * dt);
    }
}

fn update_ui(
    game_data: Res<GameData>,
    simulation_settings: Res<SimulationSettings>,
    lander_query: Query<&Lander>,
    mut time_query: Query<&mut Text, (With<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut fuel_query: Query<&mut Text, (With<FuelDisplay>, Without<TimeDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut velocity_query: Query<&mut Text, (With<VelocityDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut thrust_query: Query<&mut Text, (With<ThrustDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut angular_velocity_query: Query<&mut Text, (With<AngularVelocityDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut gravity_query: Query<&mut Text, (With<GravityDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<ThrustMultiplierDisplay>)>,
    mut thrust_multiplier_query: Query<&mut Text, (With<ThrustMultiplierDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<TimeMultiplierDisplay>)>,
    mut time_multiplier_query: Query<&mut Text, (With<TimeMultiplierDisplay>, Without<TimeDisplay>, Without<FuelDisplay>, Without<VelocityDisplay>, Without<ThrustDisplay>, Without<AngularVelocityDisplay>, Without<GravityDisplay>, Without<ThrustMultiplierDisplay>)>,
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
            let total_thrust = lander.main_thrust + lander.reverse_thrust + lander.left_thrust + lander.right_thrust;
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
    lander_query: Query<&Transform, With<Lander>>,
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
    lander_query: Query<&Transform, (With<Lander>, Without<GameCamera>)>,
    time: Res<Time>,
) {
    if let Ok((mut camera_transform, mut controller)) = camera_query.single_mut() {
        // Smooth zoom
        let zoom_speed = 5.0;
        controller.zoom = controller.zoom.lerp(controller.target_zoom, time.delta_secs() * zoom_speed);

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
            camera_transform.translation = controller.pan_offset.extend(camera_transform.translation.z);
        }
    }
}

fn draw_vector_graphics(
    mut gizmos: Gizmos,
    lander_query: Query<(&Transform, &Lander)>,
    terrain_data: Res<TerrainData>,
    simulation_settings: Res<SimulationSettings>,
) {
    // Terrain is now drawn by the draw_terrain system

    // Draw lander as vector graphics
    if let Ok((transform, lander)) = lander_query.single() {
        let pos = transform.translation.truncate();
        let rotation = transform.rotation;

        // Lander body (triangle)
        let body_points = vec![
            Vec2::new(0.0, 15.0),   // Top
            Vec2::new(-10.0, -15.0), // Bottom left
            Vec2::new(10.0, -15.0),  // Bottom right
        ];

        // Rotate and translate points
        let rotated_points: Vec<Vec2> = body_points
            .iter()
            .map(|&point| {
                let rotated = rotation * point.extend(0.0);
                pos + rotated.truncate()
            })
            .collect();

        // Draw lander body
        for i in 0..rotated_points.len() {
            let start = rotated_points[i];
            let end = rotated_points[(i + 1) % rotated_points.len()];
            gizmos.line_2d(start, end, Color::WHITE);
        }

        // Draw landing legs
        let leg_length = 8.0;
        let leg_width = 6.0;

        // Left leg
        let left_leg_top = rotation * Vec3::new(-8.0, -10.0, 0.0);
        let left_leg_bottom = rotation * Vec3::new(-8.0 - leg_width, -10.0 - leg_length, 0.0);
        gizmos.line_2d(
            pos + left_leg_top.truncate(),
            pos + left_leg_bottom.truncate(),
            Color::WHITE,
        );

        // Right leg
        let right_leg_top = rotation * Vec3::new(8.0, -10.0, 0.0);
        let right_leg_bottom = rotation * Vec3::new(8.0 + leg_width, -10.0 - leg_length, 0.0);
        gizmos.line_2d(
            pos + right_leg_top.truncate(),
            pos + right_leg_bottom.truncate(),
            Color::WHITE,
        );

        // Draw individual thruster visualizations

        // Main thruster (bottom, forward direction)
        if lander.main_thrust > 0.0 {
            let thrust_magnitude = lander.main_thrust / MAX_MAIN_THRUST;
            let thrust_length = 25.0 * thrust_magnitude;

            let thrust_start = rotation * Vec3::new(0.0, -15.0, 0.0);
            let thrust_direction = rotation * Vec3::new(0.0, -1.0, 0.0); // Opposite to forward
            let thrust_end = thrust_start + thrust_direction * thrust_length;

            gizmos.line_2d(
                pos + thrust_start.truncate(),
                pos + thrust_end.truncate(),
                Color::srgb(1.0, 0.5, 0.0), // Orange for main thrust
            );

            // Main thrust particles
            for i in 0..3 {
                let offset = (i as f32 - 1.0) * 2.0;
                let particle_start = thrust_start + rotation * Vec3::new(offset, 0.0, 0.0);
                let particle_end = particle_start + thrust_direction * thrust_length * 0.8;

                gizmos.line_2d(
                    pos + particle_start.truncate(),
                    pos + particle_end.truncate(),
                    Color::srgb(1.0, 0.8, 0.0), // Yellow particles
                );
            }
        }

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
            draw_trajectory_prediction(&mut gizmos, transform, lander, &terrain_data, &simulation_settings);
        }
    }
}

fn draw_trajectory_prediction(
    gizmos: &mut Gizmos,
    transform: &Transform,
    lander: &Lander,
    terrain_data: &TerrainData,
    simulation_settings: &SimulationSettings,
) {
    let dt = 0.016; // Assume 60 FPS for prediction
    let steps = simulation_settings.trajectory_steps;

    // Start with current state
    let mut pred_pos = transform.translation.truncate();
    let mut pred_velocity = lander.velocity;
    let mut pred_rotation = transform.rotation;
    let mut pred_angular_velocity = lander.angular_velocity;

    // Current thrust forces (assuming they continue)
    let mut pred_main_thrust = lander.main_thrust;
    let mut pred_reverse_thrust = lander.reverse_thrust;
    let mut pred_left_thrust = lander.left_thrust;
    let mut pred_right_thrust = lander.right_thrust;
    let mut pred_angular_thrust = lander.angular_thrust;

    let mut trajectory_points = Vec::new();
    trajectory_points.push(pred_pos);

    for _step in 0..steps {
        // Calculate gravitational forces from all planets
        let mut total_gravity = Vec2::ZERO;
        let mut near_planet = false;

        for planet in &terrain_data.planets {
            let to_planet = planet.center - pred_pos;
            let distance = to_planet.length();

            if distance < planet.radius * 3.0 && distance > 1.0 {
                let gravity_strength = (planet.radius * 0.5) / (distance * distance) * 10000.0;
                let gravity_direction = to_planet.normalize();
                total_gravity += gravity_direction * gravity_strength;
                near_planet = true;
            }
        }

        // Only add fallback gravity if we're not near any planet
        if !near_planet {
            total_gravity += Vec2::new(0.0, -GRAVITY * 0.1);
        }
        total_gravity *= simulation_settings.gravity_multiplier;

        // Calculate thrust forces relative to predicted orientation
        let mut total_thrust = Vec2::ZERO;

        if pred_main_thrust > 0.0 {
            let forward_dir = pred_rotation * Vec3::new(0.0, 1.0, 0.0);
            total_thrust += forward_dir.truncate() * pred_main_thrust;
        }

        if pred_reverse_thrust > 0.0 {
            let backward_dir = pred_rotation * Vec3::new(0.0, -1.0, 0.0);
            total_thrust += backward_dir.truncate() * pred_reverse_thrust;
        }

        if pred_left_thrust > 0.0 {
            let right_dir = pred_rotation * Vec3::new(1.0, 0.0, 0.0);
            total_thrust += right_dir.truncate() * pred_left_thrust;
        }

        if pred_right_thrust > 0.0 {
            let left_dir = pred_rotation * Vec3::new(-1.0, 0.0, 0.0);
            total_thrust += left_dir.truncate() * pred_right_thrust;
        }

        total_thrust *= simulation_settings.thrust_multiplier;

        // Calculate total force and acceleration
        let total_force = total_gravity + total_thrust;
        let acceleration = total_force / lander.mass;

        // Update predicted state
        pred_velocity += acceleration * dt;
        pred_angular_velocity += pred_angular_thrust * dt;
        pred_angular_velocity *= 0.98; // Apply damping

        pred_pos += pred_velocity * dt;
        pred_rotation *= Quat::from_rotation_z(pred_angular_velocity * dt);

        // Gradually reduce thrust over time (simulating fuel consumption or user releasing keys)
        let decay_factor = 0.995;
        pred_main_thrust *= decay_factor;
        pred_reverse_thrust *= decay_factor;
        pred_left_thrust *= decay_factor;
        pred_right_thrust *= decay_factor;
        pred_angular_thrust *= decay_factor;

        trajectory_points.push(pred_pos);

        // Stop prediction if we hit terrain
        let lander_radius = 20.0;
        if terrain_data.check_collision(pred_pos, lander_radius).is_some() {
            break;
        }
    }

    // Draw trajectory as dotted line
    for i in 0..trajectory_points.len().saturating_sub(1) {
        let start = trajectory_points[i];
        let end = trajectory_points[i + 1];

        // Create dotted effect by only drawing every other segment
        if i % 3 == 0 {
            // Color fades from bright to dim over distance
            let alpha = 1.0 - (i as f32 / trajectory_points.len() as f32) * 0.7;
            let color = Color::srgba(1.0, 1.0, 0.0, alpha); // Yellow trajectory
            gizmos.line_2d(start, end, color);
        }
    }

    // Draw prediction endpoint
    if let Some(&end_point) = trajectory_points.last() {
        // Draw a small circle at the end
        let circle_segments = 8;
        let radius = 5.0;
        for i in 0..circle_segments {
            let angle1 = (i as f32 / circle_segments as f32) * std::f32::consts::TAU;
            let angle2 = ((i + 1) as f32 / circle_segments as f32) * std::f32::consts::TAU;

            let p1 = end_point + Vec2::new(angle1.cos(), angle1.sin()) * radius;
            let p2 = end_point + Vec2::new(angle2.cos(), angle2.sin()) * radius;

            gizmos.line_2d(p1, p2, Color::srgb(1.0, 0.5, 0.0)); // Orange endpoint
        }
    }
}

fn cleanup_game(mut commands: Commands, game_entities: Query<Entity, Or<(With<Lander>, With<GameUI>, With<GameCamera>)>>) {
    for entity in game_entities.iter() {
        commands.entity(entity).despawn();
    }
}
