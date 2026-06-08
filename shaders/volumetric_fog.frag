#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

layout(push_constant) uniform FogParams {
    mat4 inv_view_proj;
    vec4 camPosAndMaxSteps;   // xyz=cam_pos, w=max_steps
    vec4 fogAndScattering;    // x=density, y=scattering, z=height_falloff, w=max_dist
    vec4 lightDirAndIntensity;// xyz=light_dir, w=sun_intensity
} pc;

float hash(vec3 p) {
    p = fract(p * vec3(0.1031, 0.1030, 0.0973));
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float noise3d(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float n = mix(
        mix(mix(hash(i), hash(i + vec3(1,0,0)), f.x),
            mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x), f.y),
        mix(mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x),
            mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x), f.y),
        f.z
    );
    return n;
}

float sample_depth(vec2 uv) {
    return texture(sampler2D(uDepthTex, uSamp), uv).r;
}

vec3 world_from_uv_depth(vec2 uv, float d) {
    vec4 clip = vec4(uv * 2.0 - 1.0, d, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    return w.xyz / w.w;
}

vec3 sample_color(vec2 uv) {
    return texture(sampler2D(uColorTex, uSamp), uv).rgb;
}

void main() {
    float depth = sample_depth(fragUv);
    vec3 worldPos = world_from_uv_depth(fragUv, depth);
    vec3 camPos = pc.camPosAndMaxSteps.xyz;
    vec3 rayDir = worldPos - camPos;
    float rayLen = length(rayDir);
    rayDir = rayDir / max(rayLen, 0.001);

    float maxDist = min(rayLen, pc.fogAndScattering.w);
    int steps = clamp(int(pc.camPosAndMaxSteps.w), 8, 128);
    float stepSize = maxDist / float(steps);

    float density = pc.fogAndScattering.x;
    float scattering = pc.fogAndScattering.y;
    float heightFalloff = pc.fogAndScattering.z;
    vec3 lightDir = normalize(pc.lightDirAndIntensity.xyz);
    float sunIntensity = pc.lightDirAndIntensity.w;
    vec3 fogColor = vec3(0.6, 0.7, 0.8);
    vec3 sunColor = vec3(1.0, 0.95, 0.8) * sunIntensity;

    float transmittance = 1.0;
    vec3 inScattered = vec3(0.0);

    for (int i = 0; i < steps; ++i) {
        float t = (float(i) + 0.5) * stepSize;
        vec3 pos = camPos + rayDir * t;

        float heightFactor = exp(-max(pos.y, 0.0) * heightFalloff);
        float noise = noise3d(pos * 0.5) * 0.5 + 0.5;
        float localDensity = density * heightFactor * (0.7 + 0.3 * noise);

        float stepTransmittance = exp(-localDensity * stepSize * scattering);

        float cosTheta = max(dot(rayDir, lightDir), 0.0);
        float phase = 0.25 + 0.75 * cosTheta * cosTheta; // Henyey-Greenstein-ish
        vec3 stepLight = sunColor * phase * localDensity * stepSize;

        inScattered += transmittance * stepLight;
        transmittance *= stepTransmittance;
    }

    vec3 sceneColor = sample_color(fragUv);
    vec3 fogged = sceneColor * transmittance + inScattered * fogColor;
    outColor = vec4(fogged, 1.0);
}
