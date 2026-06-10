use rustix_core::math::Vec3;
use rustix_render::mesh::Vertex;
use rustix_render::Renderer;
use crate::model_import::{ImportedModel, MaterialInfo, build_imported_model};

pub fn load_obj(renderer: &Renderer, data: &[u8], name: &str) -> Result<ImportedModel, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 decode: {e}"))?;
    let lines = text.lines();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();

    let mut faces: Vec<Vec<(usize, Option<usize>, Option<usize>)>> = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(cmd) = parts.next() else { continue };
        match cmd {
            "v" => {
                let x = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad v x")?;
                let y = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad v y")?;
                let z = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad v z")?;
                positions.push([x, y, z]);
            }
            "vn" => {
                let x = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad vn x")?;
                let y = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad vn y")?;
                let z = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad vn z")?;
                normals.push([x, y, z]);
            }
            "vt" => {
                let u = parts.next().and_then(|s| s.parse::<f32>().ok()).ok_or("bad vt u")?;
                let v = parts.next().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                uvs.push([u, v]);
            }
            "f" => {
                let mut face = Vec::new();
                for token in parts {
                    let mut indices = token.split('/');
                    let v = indices.next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or("bad face v")?;
                    let vt = indices.next().and_then(|s| {
                        if s.is_empty() { None } else { s.parse::<usize>().ok() }
                    });
                    let vn = indices.next().and_then(|s| s.parse::<usize>().ok());
                    face.push((v, vt, vn));
                }
                if face.len() >= 3 {
                    faces.push(face);
                }
            }
            _ => {}
        }
    }

    if positions.is_empty() {
        return Err("no vertices found".into());
    }

    let mut all_verts = Vec::new();
    let mut all_indices = Vec::<u16>::new();
    let mut index_map: std::collections::HashMap<(usize, Option<usize>, Option<usize>), u16> = std::collections::HashMap::new();

    for face in &faces {
        // Triangulate fan for polygons with >3 vertices
        for i in 1..face.len() - 1 {
            for idx in [0, i, i + 1] {
                let key = face[idx];
                let new_idx = *index_map.entry(key).or_insert_with(|| {
                    let v_idx = key.0.saturating_sub(1);
                    let pos = positions.get(v_idx).copied().unwrap_or([0.0, 0.0, 0.0]);

                    let n = if let Some(vn) = key.2 {
                        normals.get(vn.saturating_sub(1)).copied().unwrap_or([0.0, 1.0, 0.0])
                    } else {
                        [0.0, 1.0, 0.0]
                    };

                    let vert = Vertex { position: pos, normal: n };
                    let idx = all_verts.len() as u16;
                    all_verts.push(vert);
                    idx
                });
                all_indices.push(new_idx);
            }
        }
    }

    // Compute flat normals for faces without vertex normals
    if normals.is_empty() && !all_indices.is_empty() {
        for chunk in all_indices.chunks_exact(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;
            let p0 = Vec3::from_array(all_verts[i0].position);
            let p1 = Vec3::from_array(all_verts[i1].position);
            let p2 = Vec3::from_array(all_verts[i2].position);
            let n = (p1 - p0).cross(p2 - p0).normalize();
            let na = [n.x, n.y, n.z];
            all_verts[i0].normal = na;
            all_verts[i1].normal = na;
            all_verts[i2].normal = na;
        }
    }

    let ibuf = if all_indices.is_empty() {
        None
    } else {
        Some((&all_indices[..], all_indices.len() as u32))
    };

    let material = MaterialInfo::default();
    build_imported_model(renderer, name, &all_verts, ibuf, material, None)
}
