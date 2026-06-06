#version 460
layout(push_constant) uniform PC { mat4 model; mat4 light_view_proj; } pc;
layout(location = 0) in vec3 inPosition;
void main() {
    gl_Position = pc.light_view_proj * pc.model * vec4(inPosition, 1.0);
}
