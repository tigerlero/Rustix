#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;
layout(binding = 10) uniform CsmUBO {
    mat4 light_view_proj[3];
    vec4 cascade_splits;
} csm;
layout(binding = 11) uniform texture2D csmTex0;
layout(binding = 12) uniform texture2D csmTex1;
layout(binding = 13) uniform texture2D csmTex2;
layout(binding = 14) uniform sampler csmSamp;
layout(binding = 15) uniform textureCubeArray pointShadowTex;
layout(binding = 16) uniform sampler pointShadowSamp;
layout(binding = 17) uniform texture2DArray spotShadowTex;
layout(binding = 18) uniform sampler spotShadowSamp;
layout(binding = 19) uniform SpotShadowUBO {
    mat4 view_proj[4];
    vec4 params[4];
} spot_shadow;
struct GpuLight {
    vec4 position_radius;
    vec4 color;
};
layout(binding = 3, std430) readonly buffer LightBuffer {
    GpuLight lights[];
} light_buffer;
layout(binding = 4, std430) readonly buffer TileLightList {
    uint data[];
} tile_list;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec4 fragBaseColor;
layout(location = 3) in vec4 fragMaterial;
layout(location = 0) out vec4 outColor;

const int SHADOW_PCF_RADIUS = 1;
#define TILE_SIZE 16
#define MAX_LIGHTS_PER_TILE 32

const float PI = 3.14159265359;

float D_GGX(float NdotH, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

float G_SmithGGX(float NdotV, float roughness) {
    float a = roughness * roughness;
    return (2.0 * NdotV) / (NdotV + sqrt(a + (1.0 - a) * NdotV * NdotV));
}

float G_Smith(float NdotV, float NdotL, float roughness) {
    return G_SmithGGX(NdotV, roughness) * G_SmithGGX(NdotL, roughness);
}

vec3 F_Schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

vec3 pbrDirect(vec3 N, vec3 L, vec3 V, vec3 light_color, vec3 base, float roughness, float metallic) {
    float NdotL = max(dot(N, L), 0.0);
    if (NdotL <= 0.0) return vec3(0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotV <= 0.0) return vec3(0.0);

    vec3 H = normalize(L + V);
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);

    vec3 F0 = mix(vec3(0.04), base, metallic);

    float D = D_GGX(NdotH, roughness);
    float G = G_Smith(NdotV, NdotL, roughness);
    vec3 F = F_Schlick(HdotV, F0);

    vec3 spec = (D * G * F) / (4.0 * NdotV * NdotL + 0.0001);
    vec3 diff = base * (1.0 - metallic) / PI;

    return (diff + spec) * light_color * NdotL;
}

float sampleShadow(int cascade, vec2 uv, float currentDepth, float bias) {
    float pcfDepth;
    if (cascade == 0) pcfDepth = texture(sampler2D(csmTex0, csmSamp), uv).r;
    else if (cascade == 1) pcfDepth = texture(sampler2D(csmTex1, csmSamp), uv).r;
    else pcfDepth = texture(sampler2D(csmTex2, csmSamp), uv).r;
    return (currentDepth - bias > pcfDepth) ? 0.0 : 1.0;
}

float pointShadow(vec3 worldPos, vec3 lightPos, float lightIdx) {
    vec3 to_light = worldPos - lightPos;
    float currentDepth = length(to_light);
    float sampled = texture(samplerCubeArray(pointShadowTex, pointShadowSamp), vec4(to_light, lightIdx)).r;
    float far_plane = 25.0;
    sampled *= far_plane;
    float bias = 0.05;
    return (currentDepth - bias > sampled) ? 0.0 : 1.0;
}

float spotShadow(vec3 worldPos, int idx) {
    vec4 lsPos = spot_shadow.view_proj[idx] * vec4(worldPos, 1.0);
    vec3 proj = lsPos.xyz / lsPos.w;
    proj = proj * 0.5 + 0.5;
    if (proj.z > 1.0 || proj.x < 0.0 || proj.x > 1.0 || proj.y < 0.0 || proj.y > 1.0) return 1.0;
    float layer = spot_shadow.params[idx].w;
    float sampled = texture(sampler2DArray(spotShadowTex, spotShadowSamp), vec3(proj.xy, layer)).r;
    float bias = 0.005;
    return (proj.z - bias > sampled) ? 0.0 : 1.0;
}

float csmShadow(vec3 worldPos) {
    float dist = length(worldPos - ubo.cam_pos.xyz);
    int cascade = 0;
    if (dist > csm.cascade_splits.x) cascade = 1;
    if (dist > csm.cascade_splits.y) cascade = 2;

    vec4 lsPos = csm.light_view_proj[cascade] * vec4(worldPos, 1.0);
    vec3 proj = lsPos.xyz / lsPos.w;
    proj = proj * 0.5 + 0.5;
    if (proj.z > 1.0 || proj.x < 0.0 || proj.x > 1.0 || proj.y < 0.0 || proj.y > 1.0) return 1.0;

    float currentDepth = proj.z;
    float bias = 0.005;
    vec2 texelSize = vec2(1.0 / 2048.0);
    float shadow = 0.0;
    for(int x = -SHADOW_PCF_RADIUS; x <= SHADOW_PCF_RADIUS; ++x) {
        for(int y = -SHADOW_PCF_RADIUS; y <= SHADOW_PCF_RADIUS; ++y) {
            shadow += sampleShadow(cascade, proj.xy + vec2(x, y) * texelSize, currentDepth, bias);
        }
    }
    int samples = (2 * SHADOW_PCF_RADIUS + 1) * (2 * SHADOW_PCF_RADIUS + 1);
    return shadow / float(samples);
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 base = fragBaseColor.rgb;
    float rough = fragMaterial.x;
    float metal = fragMaterial.y;
    float ao = fragMaterial.z;
    float emissive = fragMaterial.w;

    vec3 L_dir = normalize(pc.dir_light.xyz);
    vec3 lit = pbrDirect(N, L_dir, V, pc.dir_color.rgb, base, rough, metal);
    float shadow = csmShadow(fragWorldPos);
    lit *= shadow;

    vec2 screen = gl_FragCoord.xy;
    uvec2 tile_id = uvec2(screen) / uvec2(TILE_SIZE);
    uvec2 tile_count = uvec2(ceil(vec2(ubo.fog.zw) / float(TILE_SIZE)));
    if (tile_id.x < tile_count.x && tile_id.y < tile_count.y) {
        uint tile_index = tile_id.y * tile_count.x + tile_id.x;
        uint base_offset = tile_index * (MAX_LIGHTS_PER_TILE + 1);
        uint count = tile_list.data[base_offset];
        for (uint i = 0; i < count; i++) {
            uint light_idx = tile_list.data[base_offset + 1 + i];
            vec3 light_pos = light_buffer.lights[light_idx].position_radius.xyz;
            float radius = light_buffer.lights[light_idx].position_radius.w;
            vec3 to_light = light_pos - fragWorldPos;
            float dist = length(to_light);
            if (dist > radius) continue;
            vec3 L = to_light / dist;
            vec3 light_col = light_buffer.lights[light_idx].color.rgb;
            float atten = 1.0 - (dist / radius);
            atten *= atten;
            float sh = 1.0;
            if (light_idx < 4) {
                sh = spotShadow(fragWorldPos, int(light_idx));
            } else if (light_idx < 8) {
                int cubeIdx = int(light_idx) - 4;
                sh = pointShadow(fragWorldPos, light_pos, float(cubeIdx));
            }
            lit += pbrDirect(N, L, V, light_col * atten * sh, base, rough, metal);
        }
    }

    vec3 ambient = base * 0.03 * ao;
    vec3 emissiveCol = base * emissive;
    outColor = vec4(ambient + lit + emissiveCol, 1.0);
}
