#version 460
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; vec4 _pad[9]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragPosLightSpace;
layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outNormal;
layout(location = 2) out vec4 outMaterial;

void main() {
    vec3 N = normalize(fragNormal);
    vec3 base = pc.base_color.rgb;
    float rough = pc.material.x;
    float metal = pc.material.y;
    float ao = pc.material.z;
    float emissive = pc.material.w;

    outAlbedo = vec4(base, metal);
    outNormal = vec4(N, 0.0);
    outMaterial = vec4(rough, ao, emissive, 0.0);
}
