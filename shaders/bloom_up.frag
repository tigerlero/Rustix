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
    vec3 color = texture(sampler2D(uSrc, uSamp), fragUv).rgb * 4.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, 0.0)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, 0.0)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(0.0, -ts.y)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(0.0,  ts.y)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, -ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, -ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x,  ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x,  ts.y)).rgb;
    outColor = vec4(color / 16.0, 1.0);
}
