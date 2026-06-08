#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 0) out vec4 outAccum;
layout(location = 1) out vec4 outReveal;

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

void main() {
    vec3 N = normalize(fragNormal);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 L = normalize(-pc.dir_light.xyz);
    vec3 base = pc.base_color.rgb;
    float roughness = pc.material.x;
    float metallic = pc.material.y;
    float ao = pc.material.z;
    float alpha = pc.base_color.a;

    vec3 color = pbrDirect(N, L, V, pc.dir_color.rgb, base, roughness, metallic) * ao;
    // ambient
    color += base * 0.03 * ao;

    // Weighted blended OIT weight function (simplified)
    float w = clamp(alpha * 10.0, 0.01, 100.0);
    outAccum = vec4(color * alpha * w, alpha * w);
    outReveal = vec4(alpha);
}
