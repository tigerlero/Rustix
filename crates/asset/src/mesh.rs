//! Mesh asset format and glTF 2.0 importer.
//!
//! `.rxmesh` is a compact binary format storing vertex + index data
//! for fast engine-side loading without external dependencies at runtime.

use std::future::Future;
use std::pin::Pin;

use rustix_core::math::{Vec3, Aabb};

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Vertex format ──

/// Engine vertex: position + normal (24 bytes).
/// Matches the renderer's `Vertex` struct and pipeline stride.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3]) -> Self {
        Self { position, normal }
    }
}

// ── Mesh Asset ──

/// CPU-side mesh data that can be serialized to `.rxmesh` and later
/// uploaded to the GPU via `Mesh::from_asset`.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshAsset {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub aabb: Aabb,
}

impl MeshAsset {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        let aabb = compute_aabb(&vertices);
        Self { vertices, indices, aabb }
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    pub fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn has_indices(&self) -> bool {
        !self.indices.is_empty()
    }
}

impl Asset for MeshAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::MeshAsset")
    }
}

fn compute_aabb(vertices: &[Vertex]) -> Aabb {
    if vertices.is_empty() {
        return Aabb::new(Vec3::ZERO, Vec3::ZERO);
    }
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in vertices {
        let p = Vec3::from(v.position);
        min = min.min(p);
        max = max.max(p);
    }
    Aabb::new(min, max)
}

// ── .rxmesh binary format ──

const RXMESH_MAGIC: &[u8; 4] = b"RXM1";
const RXMESH_VERSION: u32 = 1;

/// Import a `.rxmesh` file from raw bytes.
pub fn import_rxmesh(bytes: &[u8]) -> ImportResult<MeshAsset> {
    if bytes.len() < 40 {
        return Err("rxmesh: file too small for header".to_string());
    }

    let magic = &bytes[0..4];
    if magic != RXMESH_MAGIC {
        return Err(format!(
            "rxmesh: invalid magic {:?}, expected {:?}",
            std::str::from_utf8(magic).unwrap_or("???"),
            std::str::from_utf8(RXMESH_MAGIC).unwrap()
        ));
    }

    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXMESH_VERSION {
        return Err(format!("rxmesh: unsupported version {version}"));
    }

    let vertex_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let index_count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

    let aabb_min = Vec3::new(
        f32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        f32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
        f32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
    );
    let aabb_max = Vec3::new(
        f32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
        f32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]),
        f32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]),
    );

    let vertex_data_start = 40;
    let vertex_data_size = vertex_count as usize * std::mem::size_of::<Vertex>();
    let index_data_start = vertex_data_start + vertex_data_size;
    let index_data_size = index_count as usize * 2;

    if bytes.len() < index_data_start + index_data_size {
        return Err("rxmesh: file too small for vertex/index data".to_string());
    }

    let vertices: &[Vertex] = bytemuck::cast_slice(&bytes[vertex_data_start..index_data_start]);
    let indices: &[u16] = bytemuck::cast_slice(&bytes[index_data_start..index_data_start + index_data_size]);

    Ok(MeshAsset {
        vertices: vertices.to_vec(),
        indices: indices.to_vec(),
        aabb: Aabb::new(aabb_min, aabb_max),
    })
}

/// Export a `MeshAsset` to `.rxmesh` binary format.
pub fn export_rxmesh(asset: &MeshAsset) -> Vec<u8> {
    let vertex_data_size = asset.vertices.len() * std::mem::size_of::<Vertex>();
    let index_data_size = asset.indices.len() * 2;
    let total = 40 + vertex_data_size + index_data_size;

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXMESH_MAGIC);
    out.extend_from_slice(&RXMESH_VERSION.to_le_bytes());
    out.extend_from_slice(&(asset.vertex_count()).to_le_bytes());
    out.extend_from_slice(&(asset.index_count()).to_le_bytes());
    out.extend_from_slice(&asset.aabb.min.x.to_le_bytes());
    out.extend_from_slice(&asset.aabb.min.y.to_le_bytes());
    out.extend_from_slice(&asset.aabb.min.z.to_le_bytes());
    out.extend_from_slice(&asset.aabb.max.x.to_le_bytes());
    out.extend_from_slice(&asset.aabb.max.y.to_le_bytes());
    out.extend_from_slice(&asset.aabb.max.z.to_le_bytes());
    out.extend_from_slice(bytemuck::cast_slice(&asset.vertices));
    out.extend_from_slice(bytemuck::cast_slice(&asset.indices));
    out
}

// ── glTF 2.0 Importer ──

/// Importer that reads glTF / GLB files and produces a `MeshAsset`.
pub struct GltfMeshImporter;

impl Importer for GltfMeshImporter {
    type Asset = MeshAsset;

    fn name(&self) -> &'static str {
        "gltf_mesh"
    }

    fn extensions(&self) -> &[&'static str] {
        &["gltf", "glb"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_gltf(bytes)))
    }
}

pub fn import_gltf(bytes: &[u8]) -> ImportResult<MeshAsset> {
    let (doc, buffers, _images) = gltf::import_slice(bytes)
        .map_err(|e| format!("glTF parse: {e}"))?;

    let mut all_verts = Vec::new();
    let mut all_indices = Vec::<u16>::new();
    let mut base = 0u32;

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
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

    if all_verts.is_empty() {
        return Err("glTF file contains no mesh data".to_string());
    }

    Ok(MeshAsset::new(all_verts, all_indices))
}
