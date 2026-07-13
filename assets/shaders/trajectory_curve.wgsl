#import bevy_sprite::{
    mesh2d_functions as mesh_functions,
    mesh2d_view_bindings::view,
}

struct TrajectoryUniform {
    color: vec4<f32>,
    half_width: f32,
    dash_period: f32,
    dash_duty: f32,
    glow: f32,
    uncertainty: f32,
    path_index: u32,
    sample_count: u32,
    sample_dt: f32,
};

@group(2) @binding(0) var<uniform> trajectory: TrajectoryUniform;

struct CurveVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) ends: vec4<f32>,
    @location(2) tangents: vec4<f32>,
    // t, ribbon side, opacity, cumulative chord distance
    @location(3) params: vec4<f32>,
    @location(4) sample_ids: vec2<f32>,
};

struct CurveVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) edge: f32,
    @location(1) opacity: f32,
    @location(2) distance: f32,
    @location(3) progress: f32,
};

fn hermite_position(p0: vec2<f32>, p1: vec2<f32>, m0: vec2<f32>, m1: vec2<f32>, t: f32) -> vec2<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return (2.0 * t3 - 3.0 * t2 + 1.0) * p0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * p1
        + (t3 - t2) * m1;
}

fn hermite_derivative(p0: vec2<f32>, p1: vec2<f32>, m0: vec2<f32>, m1: vec2<f32>, t: f32) -> vec2<f32> {
    let t2 = t * t;
    return (6.0 * t2 - 6.0 * t) * p0
        + (3.0 * t2 - 4.0 * t + 1.0) * m0
        + (-6.0 * t2 + 6.0 * t) * p1
        + (3.0 * t2 - 2.0 * t) * m1;
}

@vertex
fn vertex(in: CurveVertex) -> CurveVertexOutput {
    // The mesh endpoints and tangents are one coherent prediction snapshot.
    // Do not replace only those positions with an asynchronously updated
    // compute buffer: doing so combines two different simulation frames and
    // visibly detaches the curve from the lander. Hermite evaluation, ribbon
    // extrusion, dashes, and uncertainty remain GPU vertex/fragment work.
    let p0 = in.ends.xy;
    let p1 = in.ends.zw;
    let m0 = in.tangents.xy;
    let m1 = in.tangents.zw;
    let t = in.params.x;
    let center = hermite_position(p0, p1, m0, m1, t);
    let tangent = normalize(hermite_derivative(p0, p1, m0, m1, t) + vec2<f32>(0.000001, 0.0));
    let progress = clamp((1.0 - in.params.z) / 0.75, 0.0, 1.0);
    // The ribbon expands into a translucent confidence corridor over time.
    // This is a deterministic uncertainty visualization, not a Monte Carlo PDF.
    let field_width = 1.0 + trajectory.uncertainty * progress * progress;
    var out: CurveVertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let world_position = mesh_functions::mesh2d_position_local_to_world(
        world_from_local,
        vec4<f32>(center, 0.0, 1.0),
    );
    let tangent_world = mesh_functions::mesh2d_position_local_to_world(
        world_from_local,
        vec4<f32>(center + tangent, 0.0, 1.0),
    );
    var center_clip = mesh_functions::mesh2d_position_world_to_clip(world_position);
    let tangent_clip = mesh_functions::mesh2d_position_world_to_clip(tangent_world);
    let center_ndc = center_clip.xy / center_clip.w;
    let tangent_ndc = tangent_clip.xy / tangent_clip.w;
    let viewport = max(view.viewport.zw, vec2<f32>(1.0));
    let tangent_pixels = normalize((tangent_ndc - center_ndc) * viewport + vec2<f32>(0.000001, 0.0));
    let normal_pixels = vec2<f32>(-tangent_pixels.y, tangent_pixels.x);
    let offset_ndc = normal_pixels
        * (2.0 * trajectory.half_width * field_width * in.params.y / viewport);
    center_clip.xy += offset_ndc * center_clip.w;
    out.position = center_clip;
    out.edge = in.params.y;
    out.opacity = in.params.z;
    out.distance = in.params.w;
    out.progress = progress;
    return out;
}

@fragment
fn fragment(in: CurveVertexOutput) -> @location(0) vec4<f32> {
    let edge_width = max(fwidth(in.edge), 0.015);
    let ribbon = 1.0 - smoothstep(1.0 - edge_width, 1.0, abs(in.edge));
    let field_width = 1.0 + trajectory.uncertainty * in.progress * in.progress;
    let core_edge = abs(in.edge) * field_width;
    let core_aa = max(fwidth(core_edge), 0.02);
    let core = 1.0 - smoothstep(0.58 - core_aa, 0.58 + core_aa, core_edge);

    var dash = 1.0;
    if (trajectory.dash_period > 0.0) {
        let phase = fract(in.distance / trajectory.dash_period);
        let dash_aa = max(fwidth(in.distance / trajectory.dash_period), 0.008);
        dash = 1.0 - smoothstep(
            trajectory.dash_duty - dash_aa,
            trajectory.dash_duty + dash_aa,
            phase,
        );
    }

    let field = ribbon * exp(-3.4 * abs(in.edge)) * in.progress * in.progress;
    let glow = ribbon * trajectory.glow / field_width;
    let alpha = trajectory.color.a * in.opacity
        * max(dash * core, glow * dash + field * 0.16);
    let rgb = trajectory.color.rgb * (0.72 + core * 0.38);
    return vec4<f32>(rgb, alpha);
}
