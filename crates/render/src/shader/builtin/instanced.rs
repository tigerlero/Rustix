use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

pub const PBR_INSTANCED_VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec4 instanceModelCol0;
layout(location = 3) in vec4 instanceModelCol1;
layout(location = 4) in vec4 instanceModelCol2;
layout(location = 5) in vec4 instanceModelCol3;
layout(location = 6) in vec4 instanceBaseColor;
layout(location = 7) in vec4 instanceMaterial;
layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec4 fragBaseColor;
layout(location = 3) out vec4 fragMaterial;
void main() {
    mat4 model = mat4(instanceModelCol0, instanceModelCol1, instanceModelCol2, instanceModelCol3);
    vec4 worldPos = model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(model) * inNormal;
    fragBaseColor = instanceBaseColor;
    fragMaterial = instanceMaterial;
}
"#;

pub const PBR_INSTANCED_FRAGMENT_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;
layout(binding = 1) uniform texture2D shadowMapTex;
layout(binding = 2) uniform sampler shadowMapSamp;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec4 fragBaseColor;
layout(location = 3) in vec4 fragMaterial;
layout(location = 0) out vec4 outColor;

const int SHADOW_PCF_RADIUS = 1;
const float PI = 3.14159265359;

float D_GGX(float NdotH, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

float G_SmithGGX(float NdotV, float roughness) {
    float a = roughness * roughness;
    return (2.0 * NdotV) / (NdotV + sqrt(a + (1.0 - a) * NdotV * NdotV));
}

float G_Smith(float NdotV, float NdotL, float roughness) {
    return G_SmithGGX(NdotV, roughness) * G_SmithGGX(NdotL, roughness);
}

vec3 F_Schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

vec3 pbrDirect(vec3 N, vec3 L, vec3 V, vec3 light_color, vec3 base, float roughness, float metallic) {
    float NdotL = max(dot(N, L), 0.0);
    if (NdotL <= 0.0) return vec3(0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotV <= 0.0) return vec3(0.0);
    vec3 H = normalize(L + V);
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);
    vec3 F0 = mix(vec3(0.04), base, metallic);
    float D = D_GGX(NdotH, roughness);
    float G = G_Smith(NdotV, NdotL, roughness);
    vec3 F = F_Schlick(HdotV, F0);
    vec3 spec = (D * G * F) / (4.0 * NdotV * NdotL + 0.0001);
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diff = base * kD / PI;
    return (diff + spec) * light_color * NdotL;
}

float pcfFilter(vec2 uv, float currentDepth, float bias, vec2 texelSize, int radius) {
    float shadow = 0.0;
    int count = 0;
    for(int x = -radius; x <= radius; ++x) {
        for(int y = -radius; y <= radius; ++y) {
            vec2 offset = vec2(x, y) * texelSize;
            float pcfDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), uv + offset).r;
            shadow += currentDepth - bias > pcfDepth ? 0.0 : 1.0;
            count++;
        }
    }
    return shadow / float(count);
}

float findBlockerDistance(vec2 uv, float currentDepth, float bias, vec2 texelSize, int searchRadius) {
    float blockerSum = 0.0;
    int blockerCount = 0;
    for(int x = -searchRadius; x <= searchRadius; ++x) {
        for(int y = -searchRadius; y <= searchRadius; ++y) {
            vec2 offset = vec2(x, y) * texelSize;
            float sampleDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), uv + offset).r;
            if (currentDepth - bias > sampleDepth) {
                blockerSum += sampleDepth;
                blockerCount++;
            }
        }
    }
    if (blockerCount == 0) return -1.0;
    return blockerSum / float(blockerCount);
}

float shadowFactor(vec4 fragLightSpace) {
    vec3 projCoords = fragLightSpace.xyz / fragLightSpace.w;
    projCoords = projCoords * 0.5 + 0.5;
    if (projCoords.z > 1.0 || projCoords.x < 0.0 || projCoords.x > 1.0 || projCoords.y < 0.0 || projCoords.y > 1.0) return 1.0;
    float currentDepth = projCoords.z;
    float bias = 0.005;
    vec2 texelSize = vec2(1.0 / 1024.0);

    float blockerDist = findBlockerDistance(projCoords.xy, currentDepth, bias, texelSize, 2);
    if (blockerDist < 0.0) return 1.0;

    float penumbra = (currentDepth - blockerDist) / blockerDist;
    int pcfRadius = int(clamp(penumbra * 4.0, 1.0, 4.0));

    return pcfFilter(projCoords.xy, currentDepth, bias, texelSize, pcfRadius);
}

vec3 evalShL1(vec3 n, vec4 shR, vec4 shG, vec4 shB) {
    return vec3(
        shR.x + shR.y * n.y + shR.z * n.z + shR.w * n.x,
        shG.x + shG.y * n.y + shG.z * n.z + shG.w * n.x,
        shB.x + shB.y * n.y + shB.z * n.z + shB.w * n.x
    );
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 L = normalize(pc.dir_light.xyz);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 lightCol = pc.dir_color.rgb;
    vec3 base = fragBaseColor.rgb;
    float rough = fragMaterial.x;
    float metal = fragMaterial.y;

    vec3 lit = pbrDirect(N, L, V, lightCol, base, rough, metal);
    vec3 ambient = evalShL1(N, ubo.sh_r, ubo.sh_g, ubo.sh_b) * base * (1.0 / PI);
    float shadow = shadowFactor(vec4(fragWorldPos, 1.0));
    outColor = vec4(ambient + shadow * lit, 1.0);
}
"#;

pub const GBUFFER_INSTANCED_VERTEX_GLSL: &str = r#"#version 460
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; vec4 _pad[9]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec4 instanceModelCol0;
layout(location = 3) in vec4 instanceModelCol1;
layout(location = 4) in vec4 instanceModelCol2;
layout(location = 5) in vec4 instanceModelCol3;
layout(location = 6) in vec4 instanceBaseColor;
layout(location = 7) in vec4 instanceMaterial;
layout(location = 0) out vec3 fragWorldPos;
layout(location = 1) out vec3 fragNormal;
layout(location = 2) out vec4 fragPosLightSpace;
layout(location = 3) out vec4 fragBaseColor;
layout(location = 4) out vec4 fragMaterial;
void main() {
    mat4 model = mat4(instanceModelCol0, instanceModelCol1, instanceModelCol2, instanceModelCol3);
    vec4 worldPos = model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(model) * inNormal;
    fragPosLightSpace = ubo.light_view_proj * worldPos;
    fragBaseColor = instanceBaseColor;
    fragMaterial = instanceMaterial;
}
"#;

pub const GBUFFER_INSTANCED_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; vec4 _pad[9]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; } pc;
layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragPosLightSpace;
layout(location = 3) in vec4 fragBaseColor;
layout(location = 4) in vec4 fragMaterial;
layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outNormal;
layout(location = 2) out vec4 outMaterial;
void main() {
    vec3 N = normalize(fragNormal);
    vec3 base = fragBaseColor.rgb;
    float rough = fragMaterial.x;
    float metal = fragMaterial.y;
    float ao = fragMaterial.z;
    float emissive = fragMaterial.w;
    outAlbedo = vec4(base, metal);
    outNormal = vec4(N, 0.0);
    outMaterial = vec4(rough, ao, emissive, 0.0);
}
"#;

pub const CULL_INSTANCES_COMP_GLSL: &str = r#"#version 460
struct CullInstance {
    vec4 model_col0;
    vec4 model_col1;
    vec4 model_col2;
    vec4 model_col3;
    vec4 base_color;
    vec4 material;
    vec4 aabb_min;
    vec4 aabb_max;
};
layout(binding = 0, std430) readonly buffer InstanceBuffer {
    CullInstance instances[];
} instance_buffer;
layout(binding = 1, std430) writeonly buffer VisibilityBuffer {
    uint flags[];
} visibility_buffer;
layout(push_constant) uniform PushConstants {
    mat4 view_proj;
    vec4 frustum_planes[6];
    uint instance_count;
    uint batch_count;
    uint _pad[2];
} pc;
bool aabb_intersects_frustum(vec3 center, vec3 extent, vec4 planes[6]) {
    for (int i = 0; i < 6; i++) {
        vec3 normal = planes[i].xyz;
        float d = planes[i].w;
        vec3 positive_vertex = center + vec3(
            normal.x >= 0.0 ? extent.x : -extent.x,
            normal.y >= 0.0 ? extent.y : -extent.y,
            normal.z >= 0.0 ? extent.z : -extent.z
        );
        if (dot(normal, positive_vertex) + d < 0.0) return false;
    }
    return true;
}
void main() {
    uint instance_id = gl_GlobalInvocationID.x;
    if (instance_id >= pc.instance_count) return;
    CullInstance inst = instance_buffer.instances[instance_id];
    mat4 model = mat4(inst.model_col0, inst.model_col1, inst.model_col2, inst.model_col3);
    vec3 local_center = (inst.aabb_min.xyz + inst.aabb_max.xyz) * 0.5;
    vec3 local_extent = (inst.aabb_max.xyz - inst.aabb_min.xyz) * 0.5;
    vec3 world_center = (model * vec4(local_center, 1.0)).xyz;
    vec3 world_extent = abs(mat3(model) * local_extent);
    bool visible = aabb_intersects_frustum(world_center, world_extent, pc.frustum_planes);
    visibility_buffer.flags[instance_id] = visible ? 1u : 0u;
}
"#;

pub const GEN_DRAW_CMDS_COMP_GLSL: &str = r#"#version 460
struct DrawCommand {
    uint index_count;
    uint instance_count;
    uint first_index;
    int  vertex_offset;
    uint first_instance;
};
struct BatchInfo {
    uint mesh_index;
    uint instance_offset;
    uint instance_count;
    uint index_count;
};
layout(binding = 0, std430) readonly buffer VisibilityBuffer {
    uint flags[];
} visibility_buffer;
layout(binding = 1, std430) writeonly buffer DrawCommandBuffer {
    DrawCommand commands[];
} draw_command_buffer;
layout(binding = 2, std430) readonly buffer BatchInfoBuffer {
    BatchInfo batches[];
} batch_info_buffer;
layout(push_constant) uniform PushConstants {
    uint batch_count;
    uint _pad[3];
} pc;
void main() {
    uint batch_id = gl_GlobalInvocationID.x;
    if (batch_id >= pc.batch_count) return;
    BatchInfo batch = batch_info_buffer.batches[batch_id];
    uint visible_count = 0;
    for (uint i = batch.instance_offset; i < batch.instance_offset + batch.instance_count; i++) {
        visible_count += visibility_buffer.flags[i];
    }
    draw_command_buffer.commands[batch_id] = DrawCommand(
        batch.index_count,
        visible_count,
        0,
        0,
        batch.instance_offset
    );
}
"#;

pub const PBR_MESH_GLSL: &str = r#"#version 460
#extension GL_NV_mesh_shader : require
layout(local_size_x = 32) in;
layout(triangles, max_vertices = 24, max_primitives = 12) out;
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; vec4 _pad[9]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { vec4 dir_light; vec4 dir_color; mat4 model; vec4 base_color; vec4 material; } pc;
layout(location = 0) out vec4 outWorldPos[];
layout(location = 1) out vec3 outNormal[];
layout(location = 2) out vec4 outPosLightSpace[];
layout(location = 3) out vec4 outBaseColor[];
layout(location = 4) out vec4 outMaterial[];
const vec3 V[24] = vec3[](
    vec3(-0.5,-0.5, 0.5), vec3( 0.5,-0.5, 0.5), vec3( 0.5, 0.5, 0.5), vec3(-0.5, 0.5, 0.5),
    vec3( 0.5,-0.5,-0.5), vec3(-0.5,-0.5,-0.5), vec3(-0.5, 0.5,-0.5), vec3( 0.5, 0.5,-0.5),
    vec3( 0.5,-0.5, 0.5), vec3( 0.5,-0.5,-0.5), vec3( 0.5, 0.5,-0.5), vec3( 0.5, 0.5, 0.5),
    vec3(-0.5,-0.5,-0.5), vec3(-0.5,-0.5, 0.5), vec3(-0.5, 0.5, 0.5), vec3(-0.5, 0.5,-0.5),
    vec3(-0.5, 0.5, 0.5), vec3( 0.5, 0.5, 0.5), vec3( 0.5, 0.5,-0.5), vec3(-0.5, 0.5,-0.5),
    vec3(-0.5,-0.5,-0.5), vec3( 0.5,-0.5,-0.5), vec3( 0.5,-0.5, 0.5), vec3(-0.5,-0.5, 0.5)
);
const vec3 N[6] = vec3[](vec3(0,0,1), vec3(0,0,-1), vec3(1,0,0), vec3(-1,0,0), vec3(0,1,0), vec3(0,-1,0));
const uint I[36] = uint[](0,1,2,0,2,3,4,5,6,4,6,7,8,9,10,8,10,11,12,13,14,12,14,15,16,17,18,16,18,19,20,21,22,20,22,23);
void main() {
    uint lid = gl_LocalInvocationID.x;
    if (lid == 0) { gl_PrimitiveCountNV = 12; for (uint i = 0; i < 36; i++) gl_PrimitiveIndicesNV[i] = I[i]; }
    if (lid < 24) {
        vec4 wp = pc.model * vec4(V[lid], 1.0);
        gl_MeshVerticesNV[lid].gl_Position = ubo.view_proj * wp;
        outWorldPos[lid] = wp;
        outNormal[lid] = mat3(transpose(inverse(pc.model))) * N[lid/4];
        outPosLightSpace[lid] = ubo.light_view_proj * wp;
        outBaseColor[lid] = pc.base_color;
        outMaterial[lid] = pc.material;
    }
}
"#;

pub fn pbr_instanced_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr_instanced.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "pbr_instanced.vert", PBR_INSTANCED_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn pbr_instanced_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr_instanced.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "pbr_instanced.frag", PBR_INSTANCED_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn gbuffer_instanced_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer_instanced.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "gbuffer_instanced.vert", GBUFFER_INSTANCED_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn gbuffer_instanced_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer_instanced.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "gbuffer_instanced.frag", GBUFFER_INSTANCED_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn cull_instances_compute_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "cull_instances.comp", vk::ShaderStageFlags::COMPUTE)
    } else {
        try_override(device, "cull_instances.comp", CULL_INSTANCES_COMP_GLSL, vk::ShaderStageFlags::COMPUTE)
    }
}
pub fn gen_draw_cmds_compute_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gen_draw_cmds.comp", vk::ShaderStageFlags::COMPUTE)
    } else {
        try_override(device, "gen_draw_cmds.comp", GEN_DRAW_CMDS_COMP_GLSL, vk::ShaderStageFlags::COMPUTE)
    }
}
pub fn pbr_mesh_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr_mesh.mesh", vk::ShaderStageFlags::MESH_NV)
    } else {
        try_override(device, "pbr_mesh.mesh", PBR_MESH_GLSL, vk::ShaderStageFlags::MESH_NV)
    }
}
