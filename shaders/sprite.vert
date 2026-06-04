#version 460
layout(binding = 0) uniform ViewProj { mat4 view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; } pc;
layout(location = 0) in vec2 inPosition;
layout(location = 1) in vec2 inUv;
layout(location = 2) in vec4 inColor;
layout(location = 0) out vec2 fragUv;
layout(location = 1) out vec4 fragColor;
void main() {
    gl_Position = ubo.view_proj * pc.model * vec4(inPosition, 0.0, 1.0);
    fragUv = inUv;
    fragColor = inColor;
}
