#version 460
layout(binding = 1) uniform texture2D uSrc;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform Params {
    vec4 texelSize;
} pc;
void main() {
    vec2 ts = pc.texelSize.xy;
    vec3 a = texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, -ts.y)).rgb;
    vec3 b = texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, -ts.y)).rgb;
    vec3 c = texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x,  ts.y)).rgb;
    vec3 d = texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x,  ts.y)).rgb;
    outColor = vec4((a + b + c + d) * 0.25, 1.0);
}
