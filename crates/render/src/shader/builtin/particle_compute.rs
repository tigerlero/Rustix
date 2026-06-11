//! Builtin GLSL compute shaders for GPU particle simulation and sorting.

use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

const PARTICLE_SIMULATE_GLSL: &str = r#"#version 460
layout(local_size_x = 256, local_size_y = 1, local_size_z = 1) in;

struct Particle {
    vec4 pos_size;
    vec4 color;
    vec4 velocity;
    vec4 params;
};

layout(std430, binding = 0) buffer ParticleBuffer {
    Particle particles[];
};

layout(push_constant) uniform SimParams {
    float dt;
    float gravity;
    float ground_plane_y;
    float bounce;
    uint particle_count;
    uint enable_collision;
    vec3 cam_pos;
    uint sort_by_depth;
} params;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= params.particle_count) return;

    Particle p = particles[idx];
    float alive = p.params.w;
    if (alive < 0.5) return;

    float lifetime = p.velocity.w;
    lifetime -= params.dt;
    if (lifetime <= 0.0) {
        p.params.w = 0.0;
        particles[idx] = p;
        return;
    }

    // Gravity
    p.velocity.y += params.gravity * params.dt;
    // Update position
    p.pos_size.xyz += p.velocity.xyz * params.dt;
    p.velocity.w = lifetime;

    // Ground plane collision
    if (params.enable_collision != 0 && params.ground_plane_y > -999.0) {
        if (p.pos_size.y < params.ground_plane_y) {
            p.pos_size.y = params.ground_plane_y;
            p.velocity.y = -p.velocity.y * params.bounce;
            p.velocity.xz *= params.bounce;
        }
    }

    // Size interpolation over lifetime
    float max_life = p.params.x;
    float t = 1.0 - clamp(lifetime / max_life, 0.0, 1.0);
    float start_size = p.params.y;
    float end_size = p.params.z;
    p.pos_size.w = mix(start_size, end_size, t);

    particles[idx] = p;
}
"#;

const PARTICLE_SORT_GLSL: &str = r#"#version 460
layout(local_size_x = 256, local_size_y = 1, local_size_z = 1) in;

struct Particle {
    vec4 pos_size;
    vec4 color;
    vec4 velocity;
    vec4 params;
};

layout(std430, binding = 0) buffer ParticleBuffer {
    Particle particles[];
};

layout(push_constant) uniform SortParams {
    uint stage;
    uint step;
    uint particle_count;
    vec3 cam_pos;
} params;

// Bitonic sort: each invocation compares and optionally swaps two elements.
void main() {
    uint idx = gl_GlobalInvocationID.x;
    uint pair_distance = params.step;
    uint block_width = params.stage;
    if (idx >= params.particle_count / 2) return;

    uint left_idx = (idx / pair_distance) * pair_distance * 2 + (idx % pair_distance);
    uint right_idx = left_idx + pair_distance;
    if (right_idx >= params.particle_count) return;

    float left_depth = length(particles[left_idx].pos_size.xyz - params.cam_pos);
    float right_depth = length(particles[right_idx].pos_size.xyz - params.cam_pos);

    bool descending = (left_idx / block_width) % 2 == 1;
    bool swap = descending ? (left_depth < right_depth) : (left_depth > right_depth);

    if (swap) {
        Particle tmp = particles[left_idx];
        particles[left_idx] = particles[right_idx];
        particles[right_idx] = tmp;
    }
}
"#;

pub fn particle_simulate_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    try_override(device, "particle_simulate.comp", PARTICLE_SIMULATE_GLSL, vk::ShaderStageFlags::COMPUTE)
}

pub fn particle_sort_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    try_override(device, "particle_sort.comp", PARTICLE_SORT_GLSL, vk::ShaderStageFlags::COMPUTE)
}
