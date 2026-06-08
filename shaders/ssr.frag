#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(binding = 4) uniform texture2D uNormalTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform SSRParams {
    mat4 inv_view_proj;
    vec4 camPosAndMaxSteps; // xyz=cam_pos, w=max_steps
    vec4 screenAndStride;   // x=screen_w, y=screen_h, z=stride, w=max_dist
} pc;

float sample_depth(vec2 uv) {
    return texture(sampler2D(uDepthTex, uSamp), uv).r;
}

vec3 world_from_uv_depth(vec2 uv, float d) {
    vec4 clip = vec4(uv * 2.0 - 1.0, d, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    return w.xyz / w.w;
}

vec3 view_from_world(vec3 p) {
    // simple view-space extraction from depth-only; assume camera looks -Z
    return p - pc.camPosAndMaxSteps.xyz;
}

vec3 sample_normal(vec2 uv) {
    return texture(sampler2D(uNormalTex, uSamp), uv).rgb * 2.0 - 1.0;
}

vec3 sample_color(vec2 uv) {
    return texture(sampler2D(uColorTex, uSamp), uv).rgb;
}

void main() {
    float depth = sample_depth(fragUv);
    if (depth >= 0.99999) { outColor = vec4(0.0); return; }

    vec3 worldPos = world_from_uv_depth(fragUv, depth);
    vec3 normal = normalize(sample_normal(fragUv));
    vec3 viewDir = normalize(pc.camPosAndMaxSteps.xyz - worldPos);
    vec3 reflectDir = reflect(-viewDir, normal);

    vec3 startPos = worldPos + reflectDir * 0.05;
    vec3 endPos = startPos + reflectDir * pc.screenAndStride.w;

    vec4 startClip = pc.inv_view_proj * vec4(startPos, 1.0); startClip.xyz /= startClip.w;
    vec4 endClip = pc.inv_view_proj * vec4(endPos, 1.0); endClip.xyz /= endClip.w;

    vec2 startUV = startClip.xy * 0.5 + 0.5;
    vec2 endUV = endClip.xy * 0.5 + 0.5;

    vec2 delta = endUV - startUV;
    float len = length(delta * vec2(pc.screenAndStride.x, pc.screenAndStride.y));
    int steps = clamp(int(len / pc.screenAndStride.z), 4, int(pc.camPosAndMaxSteps.w));
    vec2 stepUV = delta / float(max(steps, 1));

    vec2 uv = startUV;
    float prevDepth = startClip.z;
    vec3 reflColor = vec3(0.0);
    float fade = 0.0;

    for (int i = 0; i < steps; ++i) {
        uv += stepUV;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) break;

        float rayDepth = mix(startClip.z, endClip.z, float(i) / float(steps));
        float sceneDepth = sample_depth(uv);

        if (sceneDepth < rayDepth && (prevDepth - sceneDepth) < 0.05) {
            reflColor = sample_color(uv);
            fade = 1.0 - float(i) / float(steps);
            fade *= smoothstep(0.0, 0.1, min(uv.x, uv.y));
            fade *= smoothstep(1.0, 0.9, max(uv.x, uv.y));
            break;
        }
        prevDepth = rayDepth;
    }

    vec3 baseColor = sample_color(fragUv);
    float roughness = 0.5; // could sample from material texture
    float reflectivity = (1.0 - roughness) * fade * 0.5;
    outColor = vec4(mix(baseColor, reflColor, reflectivity), 1.0);
}
