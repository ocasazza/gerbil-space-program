//! World-anchored navigation grid and GPU gravitational-field heat map.

use crate::{
    game::{CameraController, GameCamera, SimulationSettings},
    terrain::TerrainData,
    GameState,
};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    reflect::TypePath,
    render::{
        mesh::Indices,
        render_resource::{AsBindGroup, PrimitiveTopology, ShaderRef, ShaderType},
    },
    sprite::{AlphaMode2d, Material2d, Material2dPlugin},
    window::PrimaryWindow,
};

const SHADER_PATH: &str = "shaders/background_grid.wgsl";
const MAX_GRAVITY_BODIES: usize = 16;

pub struct BackgroundGridPlugin;

impl Plugin for BackgroundGridPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GridMaterial>::default())
            .add_systems(
                OnTransition {
                    exited: GameState::Menu,
                    entered: GameState::Playing,
                },
                spawn_grid,
            )
            .add_systems(
                OnTransition {
                    exited: GameState::GameOver,
                    entered: GameState::Playing,
                },
                spawn_grid,
            )
            .add_systems(
                Update,
                (fit_grid_to_camera, update_grid_material).run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                OnTransition {
                    exited: GameState::Playing,
                    entered: GameState::GameOver,
                },
                despawn_grid,
            )
            .add_systems(
                OnTransition {
                    exited: GameState::Playing,
                    entered: GameState::Menu,
                },
                despawn_grid,
            )
            .add_systems(
                OnTransition {
                    exited: GameState::Paused,
                    entered: GameState::Menu,
                },
                despawn_grid,
            );
    }
}

#[derive(Component)]
struct BackgroundGrid;

#[derive(ShaderType, Clone, Debug)]
struct GridUniform {
    gravity_bodies: [Vec4; MAX_GRAVITY_BODIES],
    body_count: u32,
    gravity_enabled: u32,
    zoom: f32,
    _padding: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct GridMaterial {
    #[uniform(0)]
    params: GridUniform,
}

impl Material2d for GridMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

fn spawn_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    commands.spawn((
        Mesh2d(meshes.add(tessellated_grid_mesh(48))),
        MeshMaterial2d(materials.add(GridMaterial {
            params: GridUniform {
                gravity_bodies: [Vec4::ZERO; MAX_GRAVITY_BODIES],
                body_count: 0,
                gravity_enabled: 0,
                zoom: 1.0,
                _padding: 0.0,
            },
        })),
        Transform::from_xyz(0.0, 0.0, -100.0),
        BackgroundGrid,
    ));
}

fn tessellated_grid_mesh(subdivisions: u32) -> Mesh {
    let side = subdivisions + 1;
    let mut positions = Vec::with_capacity((side * side) as usize);
    let mut indices = Vec::with_capacity((subdivisions * subdivisions * 6) as usize);
    for y in 0..=subdivisions {
        for x in 0..=subdivisions {
            positions.push([
                x as f32 / subdivisions as f32 - 0.5,
                y as f32 / subdivisions as f32 - 0.5,
                0.0,
            ]);
        }
    }
    for y in 0..subdivisions {
        for x in 0..subdivisions {
            let a = y * side + x;
            let b = a + 1;
            let c = a + side;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn fit_grid_to_camera(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Transform, &CameraController), (With<GameCamera>, Without<BackgroundGrid>)>,
    mut grid: Query<&mut Transform, (With<BackgroundGrid>, Without<GameCamera>)>,
) {
    let (Ok(window), Ok((camera_transform, controller)), Ok(mut transform)) =
        (windows.single(), camera.single(), grid.single_mut())
    else {
        return;
    };

    // Slight overscan prevents exposed corners while camera interpolation runs.
    let extent = Vec2::new(window.width(), window.height()) * controller.zoom * 1.08;
    transform.translation.x = camera_transform.translation.x;
    transform.translation.y = camera_transform.translation.y;
    transform.scale = extent.extend(1.0);
}

fn update_grid_material(
    terrain: Res<TerrainData>,
    settings: Res<SimulationSettings>,
    camera: Query<&CameraController, With<GameCamera>>,
    grid: Query<&MeshMaterial2d<GridMaterial>, With<BackgroundGrid>>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    let (Ok(controller), Ok(handle)) = (camera.single(), grid.single()) else {
        return;
    };
    let Some(material) = materials.get_mut(handle) else {
        return;
    };

    if settings.show_gravity_field {
        material.params.gravity_bodies.fill(Vec4::ZERO);
        for (target, body) in material
            .params
            .gravity_bodies
            .iter_mut()
            .zip(terrain.planets.iter())
        {
            // xy = center, z = GM, w = softening radius.
            *target = Vec4::new(
                body.center.x,
                body.center.y,
                body.gravitational_parameter * settings.gravity_multiplier,
                (body.radius * 0.12).max(24.0),
            );
        }
    }
    material.params.body_count = if settings.show_gravity_field {
        terrain.planets.len().min(MAX_GRAVITY_BODIES) as u32
    } else {
        0
    };
    material.params.gravity_enabled = u32::from(settings.show_gravity_field);
    material.params.zoom = controller.zoom.max(0.001);
}

fn despawn_grid(mut commands: Commands, grid: Query<Entity, With<BackgroundGrid>>) {
    for entity in &grid {
        commands.entity(entity).despawn();
    }
}
