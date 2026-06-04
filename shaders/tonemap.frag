#version 460
layout(binding = 1) uniform texture2D uHdrTex;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

const int TONEMAP_ALGORITHM = 0; // 0=ACES, 1=Reinhard

vec3 reinhard(vec3 v) { return v / (v + vec3(1.0)); }
vec3 aces_fitted(vec3 v) {
    float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return clamp((v * (a * v + b)) / (v * (c * v + d) + e), 0.0, 1.0);
}
void main() {
    vec3 hdr = texture(sampler2D(uHdrTex, uSamp), fragUv).rgb;
    vec3 mapped;
    if (TONEMAP_ALGORITHM == 0) {
        mapped = aces_fitted(hdr);
    } else {
        mapped = reinhard(hdr);
    }
    mapped = pow(mapped, vec3(1.0 / 2.2));
    outColor = vec4(mapped, 1.0);
}
