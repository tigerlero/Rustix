#version 460
layout(binding = 1) uniform texture2D uTex;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 1) in vec4 fragColor;
layout(location = 0) out vec4 outColor;
void main() {
    outColor = texture(sampler2D(uTex, uSamp), fragUv) * fragColor;
}
