#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;

layout(location = 2) in vec4 instanceModelCol0;
layout(location = 3) in vec4 instanceModelCol1;
layout(location = 4) in vec4 instanceModelCol2;
layout(location = 5) in vec4 instanceModelCol3;
layout(location = 6) in vec4 instanceBaseColor;
layout(location = 7) in vec4 instanceMaterial;

layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec4 fragBaseColor;
layout(location = 3) out vec4 fragMaterial;

void main() {
    mat4 model = mat4(instanceModelCol0, instanceModelCol1, instanceModelCol2, instanceModelCol3);
    vec4 worldPos = model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(model) * inNormal;
    fragBaseColor = instanceBaseColor;
    fragMaterial = instanceMaterial;
}
