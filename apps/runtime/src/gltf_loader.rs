use rustix_core::math::{Vec3, Mat4, Quat};
use rustix_render::mesh::Vertex;
use rustix_render::Renderer;
use rustix_animation::Skeleton;
use crate::model_import::{ImportedModel, MaterialInfo, build_imported_model};

pub fn load_glb(renderer: &Renderer, data: &[u8], name: &str) -> Result<ImportedModel, String> {
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
    let material = MaterialInfo {
        base_color: pbr_base,
        roughness: pbr_roughness,
        metallic: pbr_metallic,
        ao: 1.0,
        emissive: 0.0,
    };

    // Extract skeleton from first skin if present
    let skeleton = extract_skeleton(&doc, &buffers);

    build_imported_model(renderer, name, &all_verts, ibuf, material, skeleton)
}

fn extract_skeleton(doc: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Option<Skeleton> {
    let skin = doc.skins().next()?;
    let joints: Vec<gltf::Node<'_>> = skin.joints().collect();
    if joints.is_empty() {
        return None;
    }

    // Build node_index -> bone_index map
    let mut node_to_bone: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (i, joint) in joints.iter().enumerate() {
        node_to_bone.insert(joint.index(), i);
    }

    // Build child -> parent reverse map from scene graph
    let mut child_to_parent: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in doc.nodes() {
        for child in node.children() {
            child_to_parent.insert(child.index(), node.index());
        }
    }

    // Read inverse bind matrices
    let ibm_reader = skin.reader(|buf| Some(&buffers[buf.index()]));
    let ibms: Vec<Mat4> = if let Some(iter) = ibm_reader.read_inverse_bind_matrices() {
        iter.map(|m| {
            let cols = m;
            Mat4::from_cols_array(&[
                cols[0][0], cols[0][1], cols[0][2], cols[0][3],
                cols[1][0], cols[1][1], cols[1][2], cols[1][3],
                cols[2][0], cols[2][1], cols[2][2], cols[2][3],
                cols[3][0], cols[3][1], cols[3][2], cols[3][3],
            ])
        }).collect()
    } else {
        vec![Mat4::IDENTITY; joints.len()]
    };

    let mut bones = Vec::with_capacity(joints.len());
    for (i, joint) in joints.iter().enumerate() {
        let name = joint.name().unwrap_or("bone");
        let mut name_arr = [0u8; 32];
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        name_arr[..len].copy_from_slice(&bytes[..len]);

        // Find parent within the joint set
        let parent = child_to_parent.get(&joint.index())
            .and_then(|p| node_to_bone.get(p))
            .copied()
            .unwrap_or(u16::MAX as usize);

        // Extract local TRS from node transform
        let (t, r, s) = joint.transform().decomposed();
        let local_pos = Vec3::new(t[0], t[1], t[2]);
        let quat = Quat::from_array([r[0], r[1], r[2], r[3]]);
        let (rx, ry, rz) = quat.to_euler(rustix_core::math::EulerRot::XYZ);
        let local_rot = Vec3::new(rx, ry, rz);
        let local_scl = Vec3::new(s[0], s[1], s[2]);

        bones.push(rustix_animation::Bone {
            name: name_arr,
            parent: parent as u16,
            local_pos,
            local_rot,
            local_scl,
            inverse_bind: ibms.get(i).copied().unwrap_or(Mat4::IDENTITY),
        });
    }

    Some(Skeleton::new(bones))
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
