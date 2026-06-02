use ash::vk;
use gpu_allocator::MemoryLocation;
use rustix_core::math::{Vec3, Aabb};
use crate::memory::GpuBuffer;
use crate::{Renderer, RenderError};

pub struct Mesh {
    pub vertex_buffer: GpuBuffer,
    pub index_buffer: Option<GpuBuffer>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub has_indices: bool,
    pub aabb: Aabb,
}

impl Mesh {
    pub fn new(renderer: &Renderer, name: &str, vertices: &[u8], vertex_count: u32, indices: Option<(&[u16], u32)>) -> Result<Self, RenderError> {
        let vb = renderer.create_buffer(&format!("{name}_vb"), vertices.len() as u64, vk::BufferUsageFlags::VERTEX_BUFFER, MemoryLocation::CpuToGpu)?;
        vb.write(vertices);
        let (ib, has_indices, index_count) = if let Some((idx_data, idx_count)) = indices {
            let ib = renderer.create_buffer(&format!("{name}_ib"), (idx_data.len() * 2) as u64, vk::BufferUsageFlags::INDEX_BUFFER, MemoryLocation::CpuToGpu)?;
            ib.write(bytemuck::cast_slice(idx_data)); (Some(ib), true, idx_count)
        } else { (None, false, 0) };
        let aabb = compute_aabb_from_vertices(vertices);
        Ok(Self { vertex_buffer: vb, index_buffer: ib, vertex_count, index_count, has_indices, aabb })
    }
}

fn compute_aabb_from_vertices(vertices: &[u8]) -> Aabb {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    // Each vertex is 24 bytes: position[3*f32] + normal[3*f32]
    let stride = 24usize;
    for chunk in vertices.chunks_exact(stride) {
        let x = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let y = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        let z = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
        min = min.min(Vec3::new(x, y, z));
        max = max.max(Vec3::new(x, y, z));
    }
    if min.x == f32::MAX {
        Aabb { min: Vec3::ZERO, max: Vec3::ZERO }
    } else {
        Aabb { min, max }
    }
}

/// Vertex format: position[f32;3] + normal[f32;3] = 24 bytes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub mod procedural {
    use super::*;

    fn vert(p: [f32;3], n: [f32;3]) -> Vertex { Vertex { position: p, normal: normalize(n) } }
    fn normalize(v: [f32;3]) -> [f32;3] {
        let len = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
        if len > 0.0 { [v[0]/len, v[1]/len, v[2]/len] } else { [0.0,1.0,0.0] }
    }

    pub fn cube(size: f32) -> (Vec<Vertex>, Vec<u16>) {
        let s = size * 0.5;
        let p = [
            [-s,-s,-s],[ s,-s,-s],[ s, s,-s],[-s, s,-s],
            [-s,-s, s],[ s,-s, s],[ s, s, s],[-s, s, s],
        ];
        let faces: [(usize,usize,usize,usize,[f32;3]);6] = [
            (0,1,2,3,[-1.0,0.0,0.0]),(4,7,6,5,[1.0,0.0,0.0]),(0,4,5,1,[0.0,-1.0,0.0]),
            (3,2,6,7,[0.0,1.0,0.0]),(0,3,7,4,[0.0,0.0,-1.0]),(1,5,6,2,[0.0,0.0,1.0]),
        ];
        let mut v = Vec::new();
        let mut idx = Vec::new();
        let mut base = 0u16;
        for (a,b,c,d,n) in faces {
            v.push(vert(p[a],n)); v.push(vert(p[b],n)); v.push(vert(p[c],n)); v.push(vert(p[c],n)); v.push(vert(p[d],n)); v.push(vert(p[a],n));
            idx.extend_from_slice(&[base,base+1,base+2,base+2,base+3,base]);
            base += 4;
        }
        (v, idx)
    }

    pub fn quad(size: f32, sub: u32) -> (Vec<Vertex>, Vec<u16>) {
        let mut verts = Vec::new();
        let normal = [0.0, 1.0, 0.0];
        for r in 0..=sub { for c in 0..=sub {
            let x = (c as f32 / sub as f32 - 0.5) * size;
            let z = (r as f32 / sub as f32 - 0.5) * size;
            verts.push(vert([x,0.0,z], normal));
        }}
        let idx = quad_indices(sub+1, sub+1);
        (verts, idx)
    }

    pub fn torus(major: f32, minor: f32, maj_segs: u32, min_segs: u32) -> (Vec<Vertex>, Vec<u16>) {
        let mut verts = Vec::new();
        for i in 0..=maj_segs {
            let phi = 2.0 * std::f32::consts::PI * i as f32 / maj_segs as f32;
            for j in 0..=min_segs {
                let theta = 2.0 * std::f32::consts::PI * j as f32 / min_segs as f32;
                let r = major + minor * theta.cos();
                let x = phi.cos() * r;
                let y = theta.sin() * minor;
                let z = phi.sin() * r;
                // Normal: from torus center ring outward
                let nx = phi.cos() * theta.cos();
                let ny = theta.sin();
                let nz = phi.sin() * theta.cos();
                verts.push(vert([x,y,z], [nx,ny,nz]));
            }
        }
        let idx = quad_indices(maj_segs+1, min_segs+1);
        (verts, idx)
    }

    pub fn uv_sphere(radius: f32, rings: u32, sectors: u32) -> (Vec<Vertex>, Vec<u16>) {
        let mut verts = Vec::new();
        for r in 0..=rings {
            let phi = std::f32::consts::PI * r as f32 / rings as f32;
            let y = phi.cos() * radius;
            let ring_r = phi.sin() * radius;
            for s in 0..=sectors {
                let theta = 2.0 * std::f32::consts::PI * s as f32 / sectors as f32;
                let x = theta.cos() * ring_r;
                let z = theta.sin() * ring_r;
                verts.push(vert([x,y,z], [x/radius, y/radius, z/radius]));
            }
        }
        let idx = quad_indices(rings+1, sectors+1);
        (verts, idx)
    }

    pub fn icosphere(radius: f32, sub: u32) -> (Vec<Vertex>, Vec<u16>) {
        let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
        let base = [
            [-1.0,t,0.0],[1.0,t,0.0],[-1.0,-t,0.0],[1.0,-t,0.0],
            [0.0,-1.0,t],[0.0,1.0,t],[0.0,-1.0,-t],[0.0,1.0,-t],
            [t,0.0,-1.0],[t,0.0,1.0],[-t,0.0,-1.0],[-t,0.0,1.0],
        ];
        let mut verts: Vec<[f32;3]> = base.iter().map(|v| {
            let len = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
            [v[0]/len*radius, v[1]/len*radius, v[2]/len*radius]
        }).collect();
        let faces: [[usize;3];20] = [
            [0,11,5],[0,5,1],[0,1,7],[0,7,10],[0,10,11],[1,5,9],[5,11,4],[11,10,2],[10,7,6],[7,1,8],
            [3,9,4],[3,4,2],[3,2,6],[3,6,8],[3,8,9],[4,9,5],[2,4,11],[6,2,10],[8,6,7],[9,8,1],
        ];
        let mut idx = Vec::new();
        for [a,b,c] in faces {
            subdivide(a,b,c,sub,&mut verts,&mut idx);
        }
        let vertices: Vec<Vertex> = verts.iter().map(|p| {
            let n = normalize(*p);
            Vertex{position:*p, normal:n}
        }).collect();
        (vertices, idx)
    }

    fn subdivide(a:usize,b:usize,c:usize,depth:u32,verts:&mut Vec<[f32;3]>,idx:&mut Vec<u16>) {
        if depth == 0 { idx.extend_from_slice(&[a as u16,b as u16,c as u16]); return; }
        let ab = verts.len(); verts.push(mid(verts[a],verts[b]));
        let bc = verts.len(); verts.push(mid(verts[b],verts[c]));
        let ca = verts.len(); verts.push(mid(verts[c],verts[a]));
        let radius = (verts[a][0]*verts[a][0]+verts[a][1]*verts[a][1]+verts[a][2]*verts[a][2]).sqrt();
        for &i in &[ab,bc,ca] { let n=normalize(verts[i]); verts[i]=[n[0]*radius,n[1]*radius,n[2]*radius]; }
        subdivide(a,ab,ca,depth-1,verts,idx);
        subdivide(ab,b,bc,depth-1,verts,idx);
        subdivide(ca,bc,c,depth-1,verts,idx);
        subdivide(ab,bc,ca,depth-1,verts,idx);
    }

    fn mid(a:[f32;3],b:[f32;3]) -> [f32;3] { [(a[0]+b[0])*0.5,(a[1]+b[1])*0.5,(a[2]+b[2])*0.5] }
    fn quad_indices(r:u32,s:u32) -> Vec<u16> {
        let mut idx=Vec::new();
        for i in 0..r-1 { for j in 0..s-1 {
            let a=i*s+j; let b=a+1; let c=(i+1)*s+j; let d=c+1;
            idx.extend_from_slice(&[a as u16,b as u16,c as u16, c as u16,b as u16,d as u16]);
        }}
        idx
    }

    pub fn capsule(radius: f32, height: f32, rings: u32, sectors: u32) -> (Vec<Vertex>, Vec<u16>) {
        let mut verts = Vec::new();
        let half_h = height * 0.5;
        let total_rings = rings * 2;
        for r in 0..=total_rings {
            let (y, ring_r, center_y) = if r <= rings {
                let phi = std::f32::consts::PI * r as f32 / rings as f32;
                (-half_h - radius * phi.cos(), radius * phi.sin(), -half_h)
            } else {
                let phi = std::f32::consts::PI * (r - rings) as f32 / rings as f32;
                (half_h + radius * phi.cos(), radius * phi.sin(), half_h)
            };
            for s in 0..=sectors {
                let theta = 2.0 * std::f32::consts::PI * s as f32 / sectors as f32;
                let x = theta.cos() * ring_r;
                let z = theta.sin() * ring_r;
                let nx = if ring_r > 0.001 { x / ring_r } else { 0.0 };
                let nz = if ring_r > 0.001 { z / ring_r } else { 0.0 };
                let ny = if ring_r > 0.001 { (y - center_y) / radius } else { 0.0 };
                let nlen = (nx*nx + ny*ny + nz*nz).sqrt();
                let (nx, ny, nz) = if nlen > 0.0 { (nx/nlen, ny/nlen, nz/nlen) } else { (0.0, 1.0, 0.0) };
                verts.push(vert([x, y, z], [nx, ny, nz]));
            }
        }
        let idx = quad_indices(total_rings + 1, sectors + 1);
        (verts, idx)
    }
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
