#version 460
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; vec4 _pad[9]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 0) out vec3 fragWorldPos;
layout(location = 1) out vec3 fragNormal;
void main() {
    vec4 worldPos = pc.model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(pc.model) * inNormal;
}
