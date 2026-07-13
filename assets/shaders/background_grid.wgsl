#import bevy_sprite::mesh2d_functions as mesh_functions

const MAX_BODIES: u32 = 16u;

struct GridUniform {
    gravity_bodies: array<vec4<f32>, 16>,
    body_count: u32,
    gravity_enabled: u32,
    zoom: f32,
    _padding: f32,
};

@group(2) @binding(0) var<uniform> grid: GridUniform;

struct GridVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct GridVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) gravity: vec2<f32>,
};

fn grid_lines(world: vec2<f32>, spacing: f32) -> f32 {
    let cell = world / spacing;
    let distance_to_line = abs(fract(cell + 0.5) - 0.5);
    let footprint = max(fwidth(cell), vec2<f32>(0.0001));
    let line = vec2<f32>(1.0) - smoothstep(footprint * 0.55, footprint * 1.45, distance_to_line);
    return max(line.x, line.y);
}

fn gravity_at(world: vec2<f32>) -> vec2<f32> {
    var gravity = vec2<f32>(0.0);
    for (var i = 0u; i < MAX_BODIES; i += 1u) {
        if (i >= grid.body_count) { break; }
        let body = grid.gravity_bodies[i];
        let offset = body.xy - world;
        let softened_squared = dot(offset, offset) + body.w * body.w;
        // One reciprocal square root is substantially cheaper than a general
        // fractional pow on WebGL2 while producing the same r^-3 factor.
        let inverse_radius = inverseSqrt(softened_squared);
        gravity += offset * (body.z * inverse_radius * inverse_radius * inverse_radius);
    }
    return gravity;
}

@vertex
fn vertex(in: GridVertex) -> GridVertexOutput {
    var out: GridVertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    out.world_position = mesh_functions::mesh2d_position_local_to_world(
        world_from_local,
        vec4<f32>(in.position, 1.0),
    );
    out.position = mesh_functions::mesh2d_position_world_to_clip(out.world_position);
    // Evaluate the multi-body field on a 48x48 screen-aligned vertex lattice;
    // fragments interpolate it. This cuts field evaluations by roughly 100x
    // at the default panel size while retaining smooth saddle regions.
    out.gravity = gravity_at(out.world_position.xy);
    return out;
}

@fragment
fn fragment(in: GridVertexOutput) -> @location(0) vec4<f32> {
    let world = in.world_position.xy;

    // Cross-fade neighboring powers-of-two. Grid density therefore changes
    // without popping while zooming and remains anchored to world coordinates.
    let zoom_level = log2(max(grid.zoom, 0.001));
    let level = floor(zoom_level);
    let transition = smoothstep(0.2, 0.8, fract(zoom_level));
    let fine_spacing = 64.0 * exp2(level);
    let coarse_spacing = fine_spacing * 2.0;
    let fine = grid_lines(world, fine_spacing) * (1.0 - transition);
    let coarse = grid_lines(world, coarse_spacing) * (0.55 + 0.45 * transition);
    let major = grid_lines(world, coarse_spacing * 5.0);
    let line_alpha = clamp(max(fine * 0.18, coarse * 0.24) + major * 0.16, 0.0, 0.42);

    var color = vec3<f32>(0.48, 0.50, 0.54) * line_alpha;
    var alpha = line_alpha;

    if (grid.gravity_enabled != 0u) {
        let field = in.gravity;
        let magnitude = length(field);
        let direction = select(vec2<f32>(0.0), normalize(field), magnitude > 0.000001);
        let strength = clamp(log2(1.0 + magnitude) / 8.0, 0.0, 1.0);
        // Direction maps continuously through cool muted hues. Low-force areas
        // remain violet, making saddle/cancellation regions easy to search for.
        let directional = vec3<f32>(
            0.20 + 0.12 * (direction.x * 0.5 + 0.5),
            0.22 + 0.12 * (direction.y * 0.5 + 0.5),
            0.31 + 0.10 * (1.0 - abs(direction.x)),
        );
        let heat_alpha = 0.10 + 0.24 * strength;
        color = mix(color, directional, heat_alpha) + vec3<f32>(0.08, 0.07, 0.13) * (1.0 - strength) * 0.18;
        alpha = max(alpha, 0.16 + strength * 0.16);
    }

    return vec4<f32>(color, alpha);
}
