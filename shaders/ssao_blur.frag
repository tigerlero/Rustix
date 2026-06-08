#version 460
layout(binding = 1) uniform texture2D ssaoTex;
layout(binding = 2) uniform sampler samp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out float outOcclusion;
layout(push_constant) uniform Params {
    vec4 texelSize; // xy = 1.0 / size
} pc;
void main() {
    vec2 ts = pc.texelSize.xy;
    float result = 0.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x, -ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x, -ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x,  ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x,  ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( 0.0, -ts.y)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( 0.0,  ts.y)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x,  0.0)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x,  0.0)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv).r * 4.0;
    outOcclusion = result / 16.0;
}
