#version 460
layout(binding = 1) uniform texture2D uSrc;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform Params {
    vec4 thresholdIntensity;
} pc;
void main() {
    vec3 hdr = texture(sampler2D(uSrc, uSamp), fragUv).rgb;
    float lum = dot(hdr, vec3(0.2126, 0.7152, 0.0722));
    float thresh = pc.thresholdIntensity.x;
    vec3 extracted = hdr * max(lum - thresh, 0.0) / max(lum, 0.0001);
    outColor = vec4(extracted * pc.thresholdIntensity.y, 1.0);
}
