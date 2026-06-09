use rustix_render::mesh::{Mesh, Vertex};
use rustix_render::Renderer;

pub struct GlbResult {
    pub mesh: Mesh,
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
}

pub fn load_glb(renderer: &Renderer, data: &[u8], name: &str) -> Result<GlbResult, String> {
    let (doc, buffers, _images) = gltf::import_slice(data)
        .map_err(|e| format!("glTF parse: {e}"))?;

    let mut all_verts = Vec::new();
    let mut all_indices = Vec::<u16>::new();
    let mut base = 0u32;

    let mut pbr_base = [0.8f32, 0.8, 0.8];
    let mut pbr_roughness = 0.5f32;
    let mut pbr_metallic = 0.0f32;
    let mut material_found = false;

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            if !material_found {
                let mat = prim.material();
                let pbr = mat.pbr_metallic_roughness();
                let bc = pbr.base_color_factor();
                pbr_base = [bc[0], bc[1], bc[2]];
                pbr_roughness = pbr.roughness_factor();
                pbr_metallic = pbr.metallic_factor();
                material_found = true;
            }

            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or("missing POSITIONS")?
                .collect();

            let vertex_count = positions.len();

            let normals: Vec<[f32; 3]> = if let Some(niter) = reader.read_normals() {
                niter.collect()
            } else {
                vec![[0.0, 1.0, 0.0]; vertex_count]
            };

            for i in 0..vertex_count {
                if vertex_count == 0 { break; }
                let pos = positions[i];
                let n = if normals.is_empty() { [0.0, 1.0, 0.0] } else { normals[i.min(normals.len() - 1)] };
                all_verts.push(Vertex { position: pos, normal: n });
            }

            if let Some(idx_iter) = reader.read_indices() {
                for idx in idx_iter.into_u32() {
                    all_indices.push((idx + base) as u16);
                }
            }
            base += vertex_count as u32;
        }
    }

    let ibuf = if all_indices.is_empty() { None } else { Some((&all_indices[..], all_indices.len() as u32)) };
    let mesh = Mesh::new(renderer, name, bytemuck::cast_slice(&all_verts), all_verts.len() as u32, ibuf)
        .map_err(|e| format!("mesh upload: {e}"))?;

    Ok(GlbResult {
        mesh,
        base_color: pbr_base,
        roughness: pbr_roughness,
        metallic: pbr_metallic,
    })
}

/// Generate a minimal cube in GLB format (valid glTF 2.0 binary).
pub fn generate_cube_glb() -> Vec<u8> {
    // Cube vertices: 8 corners repeated 3 times each for flat shading (24 verts)
    let positions: [[f32;3]; 24] = [
        // +X
        [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [ 0.5,  0.5, -0.5], [ 0.5, -0.5, -0.5],
        // -X
        [-0.5, -0.5, -0.5], [-0.5,  0.5, -0.5], [-0.5,  0.5,  0.5], [-0.5, -0.5,  0.5],
        // +Y
        [-0.5,  0.5,  0.5], [ 0.5,  0.5,  0.5], [ 0.5,  0.5, -0.5], [-0.5,  0.5, -0.5],
        // -Y
        [-0.5, -0.5, -0.5], [ 0.5, -0.5, -0.5], [ 0.5, -0.5,  0.5], [-0.5, -0.5,  0.5],
        // +Z
        [-0.5, -0.5,  0.5], [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [-0.5,  0.5,  0.5],
        // -Z
        [ 0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5,  0.5, -0.5], [ 0.5,  0.5, -0.5],
    ];

    let normals: [[f32;3]; 24] = [
        [1.0,0.0,0.0],[1.0,0.0,0.0],[1.0,0.0,0.0],[1.0,0.0,0.0],
        [-1.0,0.0,0.0],[-1.0,0.0,0.0],[-1.0,0.0,0.0],[-1.0,0.0,0.0],
        [0.0,1.0,0.0],[0.0,1.0,0.0],[0.0,1.0,0.0],[0.0,1.0,0.0],
        [0.0,-1.0,0.0],[0.0,-1.0,0.0],[0.0,-1.0,0.0],[0.0,-1.0,0.0],
        [0.0,0.0,1.0],[0.0,0.0,1.0],[0.0,0.0,1.0],[0.0,0.0,1.0],
        [0.0,0.0,-1.0],[0.0,0.0,-1.0],[0.0,0.0,-1.0],[0.0,0.0,-1.0],
    ];

    let indices: [u16; 36] = [
        0,1,2, 0,2,3,   4,5,6, 4,6,7,
        8,9,10, 8,10,11, 12,13,14, 12,14,15,
        16,17,18, 16,18,19, 20,21,22, 20,22,23,
    ];

    // Write binary data
    let mut bin = Vec::new();
    for p in &positions { bin.extend_from_slice(bytemuck::bytes_of(p)); }
    for n in &normals { bin.extend_from_slice(bytemuck::bytes_of(n)); }
    for i in &indices { bin.extend_from_slice(&i.to_le_bytes()); }

    let json = format!(r#"{{
    "asset":{{"version":"2.0","generator":"Rustix"}},
    "scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
    "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1}},"indices":2,"mode":4}}]}}],
    "accessors":[
        {{"bufferView":0,"componentType":5126,"count":24,"type":"VEC3","min":[-0.5,-0.5,-0.5],"max":[0.5,0.5,0.5]}},
        {{"bufferView":1,"componentType":5126,"count":24,"type":"VEC3"}},
        {{"bufferView":2,"componentType":5123,"count":36,"type":"SCALAR"}}
    ],
    "bufferViews":[
        {{"buffer":0,"byteOffset":0,"byteLength":288}},
        {{"buffer":0,"byteOffset":288,"byteLength":288}},
        {{"buffer":0,"byteOffset":576,"byteLength":72}}
    ],
    "buffers":[{{"byteLength":648}}]
}}"#);

    let json_bytes = json.as_bytes();
    let json_padded = ((json_bytes.len() + 3) / 4) * 4;

    let total = 12 + 8 + json_padded as usize + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);

    // Header
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
    glb.extend_from_slice(&2u32.to_le_bytes());           // version
    glb.extend_from_slice(&(total as u32).to_le_bytes()); // length

    // JSON chunk (length includes padding)
    glb.extend_from_slice(&(json_padded as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(json_bytes);
    for _ in json_bytes.len()..json_padded { glb.push(0x20); }

    // BIN chunk
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);

    glb
}
