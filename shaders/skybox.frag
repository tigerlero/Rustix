#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

layout(push_constant) uniform SkyboxParams {
    mat4 inv_view_proj;
    vec4 sunDirAndIntensity;  // xyz=sun_dir, w=sun_intensity
    vec4 skyParams;           // x=rayleigh, y=mie, z=zenith_shift, w=exposure
} pc;

float sample_depth(vec2 uv) {
    return texture(sampler2D(uDepthTex, uSamp), uv).r;
}

vec3 world_dir_from_uv(vec2 uv) {
    vec4 clip = vec4(uv * 2.0 - 1.0, 1.0, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    vec3 p = w.xyz / w.w;
    // Camera position from inv_view_proj
    vec4 camW = pc.inv_view_proj * vec4(0.0, 0.0, -1.0, 1.0);
    vec3 camPos = camW.xyz / camW.w;
    return normalize(p - camPos);
}

vec3 sample_color(vec2 uv) {
    return texture(sampler2D(uColorTex, uSamp), uv).rgb;
}

void main() {
    float depth = sample_depth(fragUv);

    // If scene geometry is present, just pass through
    if (depth < 0.99999) {
        outColor = vec4(sample_color(fragUv), 1.0);
        return;
    }

    vec3 viewDir = world_dir_from_uv(fragUv);
    vec3 sunDir = normalize(pc.sunDirAndIntensity.xyz);
    float sunIntensity = pc.sunDirAndIntensity.w;

    float rayleigh = pc.skyParams.x;
    float mie = pc.skyParams.y;
    float zenithShift = pc.skyParams.z;
    float exposure = pc.skyParams.w;

    // Simple Rayleigh scattering
    float cosTheta = viewDir.y + zenithShift;
    float rayleighPhase = 0.0596831 * (1.0 + cosTheta * cosTheta);
    float zenithAngle = max(0.0, cosTheta);
    float zenithDensity = exp(-zenithAngle * 3.0);

    vec3 skyColor = vec3(0.2, 0.5, 1.0) * rayleighPhase * rayleigh * zenithDensity;
    vec3 sunColor = vec3(1.0, 0.95, 0.8) * sunIntensity;

    // Sun disc
    float cosSunAngle = dot(viewDir, sunDir);
    float sunDisc = smoothstep(0.999, 0.9999, cosSunAngle);
    float sunGlow = pow(max(cosSunAngle, 0.0), 256.0) * mie;

    skyColor += sunColor * (sunDisc * 2.0 + sunGlow * 0.5);

    // Horizon glow
    float horizonGlow = exp(-abs(viewDir.y) * 8.0) * 0.3;
    skyColor += vec3(0.8, 0.5, 0.3) * horizonGlow;

    outColor = vec4(skyColor * exposure, 1.0);
}
