#version 460
layout(binding = 1) uniform texture2D accumTex;
layout(binding = 2) uniform sampler accumSamp;
layout(binding = 3) uniform texture2D revealTex;
layout(binding = 4) uniform sampler revealSamp;
layout(binding = 5) uniform texture2D opaqueTex;
layout(binding = 6) uniform sampler opaqueSamp;
layout(location = 0) out vec4 outColor;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    vec4 accum = texelFetch(sampler2D(accumTex, accumSamp), coord, 0);
    float reveal = texelFetch(sampler2D(revealTex, revealSamp), coord, 0).r;
    vec3 opaque = texelFetch(sampler2D(opaqueTex, opaqueSamp), coord, 0).rgb;

    float a = accum.a;
    vec3 transColor = accum.rgb / max(a, 0.00001);
    float transAlpha = 1.0 - reveal;
    transAlpha = clamp(transAlpha, 0.0, 1.0);

    outColor = vec4(mix(opaque, transColor, transAlpha), 1.0);
}
