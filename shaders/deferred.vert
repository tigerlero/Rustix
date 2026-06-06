#version 460
layout(location = 0) out vec2 uv;
void main() {
    vec2 pos = vec2(
        float(gl_VertexIndex % 2) * 4.0 - 1.0,  // x: 0->-1, 1->3
        float(gl_VertexIndex / 2) * 4.0 - 1.0   // y: 0->-1, 1->3
    );
    gl_Position = vec4(pos, 0.0, 1.0);
    uv = pos * 0.5 + 0.5;
}
