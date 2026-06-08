#version 460
layout(binding = 1) uniform texture2D depthTex;
layout(binding = 2) uniform sampler samp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out float outOcclusion;
layout(push_constant) uniform Params {
    vec4 projParams; // x=near, y=far, z=1/tan(fov/2), w=aspect
    vec4 radiusBias; // x=radius, y=bias, z=power, w=intensity
    vec4 screenSize; // xy=screen pixels, zw=1/screen pixels
} pc;

float linearize_depth(float d) {
    float near = pc.projParams.x;
    float far = pc.projParams.y;
    return near * far / (far - d * (far - near));
}

vec3 view_pos_from_uv_depth(vec2 uv, float depth) {
    float z = linearize_depth(depth);
    float tan_half_fov = 1.0 / pc.projParams.z;
    vec2 ndc = uv * 2.0 - 1.0;
    float aspect = pc.projParams.w;
    vec3 view_pos;
    view_pos.x = ndc.x * tan_half_fov * aspect * z;
    view_pos.y = ndc.y * tan_half_fov * z;
    view_pos.z = -z;
    return view_pos;
}

vec3 reconstruct_normal(vec2 uv, float depth) {
    vec2 texel = pc.screenSize.zw;
    float d1 = texture(sampler2D(depthTex, samp), uv + vec2(texel.x, 0.0)).r;
    float d2 = texture(sampler2D(depthTex, samp), uv + vec2(0.0, texel.y)).r;
    vec3 p0 = view_pos_from_uv_depth(uv, depth);
    vec3 px = view_pos_from_uv_depth(uv + vec2(texel.x, 0.0), d1);
    vec3 py = view_pos_from_uv_depth(uv + vec2(0.0, texel.y), d2);
    vec3 n = normalize(cross(px - p0, py - p0));
    return n;
}

const vec2 poisson[16] = vec2[](
    vec2(-0.94201624, -0.39906216),
    vec2( 0.94558609, -0.76890725),
    vec2(-0.09418410, -0.92938870),
    vec2( 0.34495938,  0.29387760),
    vec2(-0.91588581,  0.45771432),
    vec2(-0.81544232, -0.87912464),
    vec2(-0.38277543,  0.27676845),
    vec2( 0.97484398,  0.75648379),
    vec2( 0.44323325, -0.97511554),
    vec2( 0.53742981, -0.47373420),
    vec2(-0.26496911, -0.41893023),
    vec2( 0.79197514,  0.19090188),
    vec2(-0.24188840,  0.99706507),
    vec2(-0.81409955,  0.91437590),
    vec2( 0.19984126,  0.78641367),
    vec2( 0.14383161, -0.14100790)
);

float rand(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    float depth = texture(sampler2D(depthTex, samp), fragUv).r;
    if (depth >= 1.0) {
        outOcclusion = 1.0;
        return;
    }

    vec3 origin = view_pos_from_uv_depth(fragUv, depth);
    vec3 normal = reconstruct_normal(fragUv, depth);

    float radius = pc.radiusBias.x;
    float bias = pc.radiusBias.y;
    float power = pc.radiusBias.z;
    float intensity = pc.radiusBias.w;

    float occlusion = 0.0;
    float sampleScale = radius / -origin.z;

    float rotAngle = rand(fragUv * pc.screenSize.xy) * 6.28318530718;
    float sin_r = sin(rotAngle);
    float cos_r = cos(rotAngle);

    for (int i = 0; i < 16; ++i) {
        vec2 offset = poisson[i];
        vec2 rotated = vec2(
            offset.x * cos_r - offset.y * sin_r,
            offset.x * sin_r + offset.y * cos_r
        );
        vec2 sample_uv = fragUv + rotated * sampleScale * pc.screenSize.zw;
        float sample_depth = texture(sampler2D(depthTex, samp), sample_uv).r;
        vec3 sample_pos = view_pos_from_uv_depth(sample_uv, sample_depth);
        vec3 diff = sample_pos - origin;
        float dist = length(diff);
        vec3 sample_dir = diff / dist;
        float angle = max(dot(normal, sample_dir), 0.0);
        float rangeCheck = smoothstep(0.0, 1.0, radius / dist);
        float do_occlude = (sample_pos.z < (origin.z - bias)) ? 1.0 : 0.0;
        occlusion += do_occlude * rangeCheck * angle;
    }

    occlusion = 1.0 - (occlusion / 16.0) * intensity;
    occlusion = pow(clamp(occlusion, 0.0, 1.0), power);
    outOcclusion = occlusion;
}
