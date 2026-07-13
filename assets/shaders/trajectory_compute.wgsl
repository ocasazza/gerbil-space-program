const MAX_SAMPLES: u32 = 3600u;
const C2: f32 = 89875517873681764.0;

@group(0) @binding(0) var<storage, read> params: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> bodies: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> output: array<vec4<f32>>;

fn body_position(index: i32, t: f32) -> vec2<f32> {
    var result = vec2<f32>(0.0);
    var cursor = index;
    for (var depth = 0; depth < 4; depth++) {
        if (cursor < 0) { break; }
        let physical = bodies[u32(cursor) * 2u];
        let orbit = bodies[u32(cursor) * 2u + 1u];
        let parent = i32(physical.w);
        if (parent < 0) {
            result += orbit.xy;
            break;
        }
        let angle = orbit.z + orbit.y * (params[6].x + t);
        result += vec2<f32>(cos(angle), sin(angle)) * orbit.x;
        cursor = parent;
    }
    return result;
}

fn body_velocity(index: i32, t: f32) -> vec2<f32> {
    var result = vec2<f32>(0.0);
    var cursor = index;
    for (var depth = 0; depth < 4; depth++) {
        if (cursor < 0) { break; }
        let physical = bodies[u32(cursor) * 2u];
        let orbit = bodies[u32(cursor) * 2u + 1u];
        if (i32(physical.w) < 0) { break; }
        let angle = orbit.z + orbit.y * (params[6].x + t);
        result += vec2<f32>(-sin(angle), cos(angle)) * orbit.x * orbit.y;
        cursor = i32(physical.w);
    }
    return result;
}

fn gravity(position: vec2<f32>, velocity: vec2<f32>, t: f32) -> vec2<f32> {
    var total = vec2<f32>(0.0);
    let count = u32(params[6].y);
    for (var i = 0u; i < 16u; i++) {
        if (i >= count) { break; }
        let physical = bodies[i * 2u];
        let r = position - body_position(i32(i), t);
        let v = velocity - body_velocity(i32(i), t);
        let r2 = dot(r, r) + physical.y * physical.y;
        let radius = sqrt(r2);
        let inv_r3 = 1.0 / max(r2 * radius, 0.000001);
        let mu = physical.x;
        let newton = -mu * r * inv_r3;
        let common = 4.0 * mu / radius - dot(v, v);
        let pn = mu * inv_r3 / C2 * (common * r + 4.0 * dot(r, v) * v);
        total += newton + pn;
    }
    return total * params[3].w;
}

fn approach(current: f32, target: f32, dt: f32) -> f32 {
    let response = select(0.18, 0.28, abs(target) > abs(current));
    return current + (target - current) * (1.0 - exp(-dt / response));
}

@compute @workgroup_size(1)
fn project(@builtin(global_invocation_id) id: vec3<u32>) {
    let path = id.x;
    if (path > 1u) { return; }
    let count = u32(params[5].w);
    let dt = params[1].w;
    let base = path * (MAX_SAMPLES + 1u);
    var position = params[0].xy;
    var velocity = params[0].zw;
    var angle = params[1].x;
    var angular_velocity = params[1].y;
    let mass = max(params[1].z, 0.0001);
    var thrust = params[2];
    var angular_thrust = params[3].x;
    var fuel = params[5].y;
    output[base] = vec4<f32>(position, velocity);

    for (var step = 0u; step < count; step++) {
        let active = path == 1u;
        if (active) {
            thrust.x = approach(thrust.x, params[4].x * 150.0 * params[3].y, min(dt, 0.1));
            thrust.y = approach(thrust.y, params[4].y * 40.0 * params[3].y, min(dt, 0.1));
            thrust.z = approach(thrust.z, params[4].z * 60.0 * params[3].y, min(dt, 0.1));
            thrust.w = approach(thrust.w, params[4].w * 60.0 * params[3].y, min(dt, 0.1));
            angular_thrust = approach(angular_thrust, params[5].x * 3.0 * params[3].z, min(dt, 0.1));
        }
        let can_burn = params[5].z > 0.5 || fuel > 0.0;
        var force = vec2<f32>(0.0);
        if (active && can_burn) {
            let forward = vec2<f32>(-sin(angle), cos(angle));
            let right = vec2<f32>(cos(angle), sin(angle));
            force = forward * (thrust.x - thrust.y) + right * (thrust.z - thrust.w);
            force *= params[6].z;
            if (params[5].z < 0.5) {
                let burn = thrust.x / 150.0 + 0.4 * thrust.y / 40.0
                    + 0.6 * thrust.z / 60.0 + 0.6 * thrust.w / 60.0
                    + 0.5 * abs(angular_thrust) / 3.0;
                fuel = max(0.0, fuel - 10.0 * burn * dt);
            }
            angular_velocity += angular_thrust * dt;
        }
        velocity += (gravity(position, velocity, f32(step + 1u) * dt) + force / mass) * dt;
        angular_velocity *= 0.98;
        position += velocity * dt;
        angle += angular_velocity * dt;
        output[base + step + 1u] = vec4<f32>(position, velocity);
    }
}
