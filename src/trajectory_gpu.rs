use crate::game::{GameData, Lander, SimulationSettings, MAX_TRAJECTORY_SAMPLES};
use crate::player::Player;
use crate::terrain::TerrainData;
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{binding_types::storage_buffer, *},
        renderer::{RenderContext, RenderDevice},
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        Render, RenderApp, RenderSet,
    },
};

const SHADER: &str = "shaders/trajectory_compute.wgsl";

#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct TrajectoryGpuBuffers {
    pub params: Handle<ShaderStorageBuffer>,
    pub bodies: Handle<ShaderStorageBuffer>,
    pub output: Handle<ShaderStorageBuffer>,
}

pub(crate) struct TrajectoryComputePlugin;

impl Plugin for TrajectoryComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<TrajectoryGpuBuffers>::default())
            .add_systems(Startup, create_buffers)
            .add_systems(Update, upload_projection_state);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<TrajectoryComputePipeline>()
            .add_systems(
                Render,
                prepare_bind_group.in_set(RenderSet::PrepareBindGroups),
            );
        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(TrajectoryComputeLabel, TrajectoryComputeNode);
        graph.add_node_edge(
            TrajectoryComputeLabel,
            bevy::render::graph::CameraDriverLabel,
        );
    }
}

fn create_buffers(mut commands: Commands, mut buffers: ResMut<Assets<ShaderStorageBuffer>>) {
    let params = buffers.add(ShaderStorageBuffer::from(vec![Vec4::ZERO; 8]));
    let bodies = buffers.add(ShaderStorageBuffer::from(vec![Vec4::ZERO; 32]));
    let output_len = (MAX_TRAJECTORY_SAMPLES as usize + 1) * 2;
    let output = buffers.add(ShaderStorageBuffer::from(vec![Vec4::ZERO; output_len]));
    commands.insert_resource(TrajectoryGpuBuffers {
        params,
        bodies,
        output,
    });
}

fn upload_projection_state(
    handles: Option<Res<TrajectoryGpuBuffers>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    lander: Query<(&Transform, &Lander), With<Player>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    game: Res<GameData>,
    settings: Res<SimulationSettings>,
    terrain: Res<TerrainData>,
) {
    let (Some(handles), Ok((transform, lander))) = (handles, lander.single()) else {
        return;
    };
    let sample_count = settings.trajectory_steps.min(MAX_TRAJECTORY_SAMPLES);
    let dt = (settings.trajectory_steps as f32 / 60.0) / sample_count.max(1) as f32;
    let (_, _, angle) = transform.rotation.to_euler(EulerRot::XYZ);
    let main = keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp);
    let reverse = keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown);
    let left = keyboard.pressed(KeyCode::KeyA);
    let right = keyboard.pressed(KeyCode::KeyD);
    let rotation = keyboard.pressed(KeyCode::ArrowLeft) as i8 as f32
        - keyboard.pressed(KeyCode::ArrowRight) as i8 as f32;
    let params = [
        Vec4::new(
            transform.translation.x,
            transform.translation.y,
            lander.velocity.x,
            lander.velocity.y,
        ),
        Vec4::new(angle, lander.angular_velocity, lander.mass, dt),
        Vec4::new(
            lander.main_thrust,
            lander.reverse_thrust,
            lander.left_thrust,
            lander.right_thrust,
        ),
        Vec4::new(
            lander.angular_thrust,
            lander.thrust_scale,
            lander.maneuverability,
            settings.gravity_multiplier,
        ),
        Vec4::new(
            main as u8 as f32,
            reverse as u8 as f32,
            left as u8 as f32,
            right as u8 as f32,
        ),
        Vec4::new(
            rotation,
            game.fuel,
            settings.infinite_fuel as u8 as f32,
            sample_count as f32,
        ),
        Vec4::new(
            terrain.simulation_time(),
            terrain.planets.len().min(16) as f32,
            settings.thrust_multiplier,
            0.0,
        ),
        Vec4::ZERO,
    ];
    if let Some(buffer) = buffers.get_mut(&handles.params) {
        buffer.set_data(params.as_slice());
    }
    let mut body_data = vec![Vec4::ZERO; 32];
    for (index, body) in terrain.planets.iter().take(16).enumerate() {
        let parent = body.orbit.map_or(-1.0, |orbit| orbit.parent as f32);
        body_data[index * 2] = Vec4::new(
            body.gravitational_parameter,
            (body.radius * 0.12).max(24.0),
            body.radius + 20.0,
            parent,
        );
        body_data[index * 2 + 1] = body
            .orbit
            .map_or(Vec4::new(body.center.x, body.center.y, 0.0, 0.0), |orbit| {
                Vec4::new(orbit.radius, orbit.angular_speed, orbit.phase, 0.0)
            });
    }
    if let Some(buffer) = buffers.get_mut(&handles.bodies) {
        buffer.set_data(body_data.as_slice());
    }
}

#[derive(Resource)]
struct TrajectoryComputeBindGroup(BindGroup);

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<TrajectoryComputePipeline>,
    handles: Option<Res<TrajectoryGpuBuffers>>,
    gpu_buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    render_device: Res<RenderDevice>,
) {
    let Some(handles) = handles else { return };
    let (Some(params), Some(bodies), Some(output)) = (
        gpu_buffers.get(&handles.params),
        gpu_buffers.get(&handles.bodies),
        gpu_buffers.get(&handles.output),
    ) else {
        return;
    };
    commands.insert_resource(TrajectoryComputeBindGroup(render_device.create_bind_group(
        "trajectory compute bind group",
        &pipeline.layout,
        &BindGroupEntries::sequential((
            params.buffer.as_entire_buffer_binding(),
            bodies.buffer.as_entire_buffer_binding(),
            output.buffer.as_entire_buffer_binding(),
        )),
    )));
}

#[derive(Resource)]
struct TrajectoryComputePipeline {
    layout: BindGroupLayout,
    pipeline: CachedComputePipelineId,
}

impl FromWorld for TrajectoryComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let layout = device.create_bind_group_layout(
            "trajectory compute layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    storage_buffer::<Vec<Vec4>>(true),
                    storage_buffer::<Vec<Vec4>>(true),
                    storage_buffer::<Vec<Vec4>>(false),
                ),
            ),
        );
        let shader = world.load_asset(SHADER);
        let pipeline =
            world
                .resource::<PipelineCache>()
                .queue_compute_pipeline(ComputePipelineDescriptor {
                    label: Some("trajectory prediction compute".into()),
                    layout: vec![layout.clone()],
                    push_constant_ranges: vec![],
                    shader,
                    shader_defs: vec![],
                    entry_point: "project".into(),
                    zero_initialize_workgroup_memory: false,
                });
        Self { layout, pipeline }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct TrajectoryComputeLabel;

struct TrajectoryComputeNode;

impl render_graph::Node for TrajectoryComputeNode {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let (Some(bind_group), Some(pipeline)) = (
            world.get_resource::<TrajectoryComputeBindGroup>(),
            world
                .resource::<PipelineCache>()
                .get_compute_pipeline(world.resource::<TrajectoryComputePipeline>().pipeline),
        ) else {
            return Ok(());
        };
        let mut pass = context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("trajectory prediction"),
                ..default()
            });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group.0, &[]);
        pass.dispatch_workgroups(2, 1, 1);
        Ok(())
    }
}
