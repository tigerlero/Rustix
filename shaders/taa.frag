#version 460
layout(binding = 1) uniform texture2D currentTex;
layout(binding = 2) uniform sampler samp;
layout(binding = 3) uniform texture2D historyTex;
layout(binding = 4) uniform texture2D depthTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform TAAParams {
    mat4 inv_view_proj;
    mat4 prev_view_proj;
    vec4 blendAndSize; // x=blend_factor, yz=screen_size, w=1/screen_size
} pc;

vec3 sample_current(vec2 uv) {
    return texture(sampler2D(currentTex, samp), uv).rgb;
}

vec3 sample_history(vec2 uv) {
    return texture(sampler2D(historyTex, samp), uv).rgb;
}

float sample_depth(vec2 uv) {
    return texture(sampler2D(depthTex, samp), uv).r;
}

vec3 world_pos_from_uv_depth(vec2 uv, float depth) {
    vec4 clip = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 world = pc.inv_view_proj * clip;
    return world.xyz / world.w;
}

vec2 prev_uv_from_world(vec3 world_pos) {
    vec4 prev_clip = pc.prev_view_proj * vec4(world_pos, 1.0);
    vec3 prev_ndc = prev_clip.xyz / prev_clip.w;
    return prev_ndc.xy * 0.5 + 0.5;
}

void main() {
    vec2 ts = vec2(1.0 / pc.blendAndSize.z, 1.0 / pc.blendAndSize.w);

    vec3 current = sample_current(fragUv);

    float depth = sample_depth(fragUv);
    vec3 world_pos = world_pos_from_uv_depth(fragUv, depth);
    vec2 prev_uv = prev_uv_from_world(world_pos);

    // Motion rejection: if reprojected UV is off-screen, don't use history
    float off_screen = float(prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0);

    vec3 history = sample_history(prev_uv);

    // 3x3 neighborhood min/max (clamping history to reduce ghosting)
    vec3 min_color = current;
    vec3 max_color = current;
    for (int x = -1; x <= 1; ++x) {
        for (int y = -1; y <= 1; ++y) {
            vec2 offset = vec2(float(x), float(y)) * ts;
            vec3 neighbor = sample_current(fragUv + offset);
            min_color = min(min_color, neighbor);
            max_color = max(max_color, neighbor);
        }
    }

    vec3 clamped_history = clamp(history, min_color, max_color);

    float blend = pc.blendAndSize.x;
    // Reduce blend factor when there's significant motion or off-screen
    vec2 motion = abs(prev_uv - fragUv);
    float motion_factor = smoothstep(0.001, 0.01, length(motion));
    blend = mix(blend, 0.0, motion_factor);
    blend = mix(blend, 0.0, off_screen);

    vec3 resolved = mix(current, clamped_history, blend);
    outColor = vec4(resolved, 1.0);
}
