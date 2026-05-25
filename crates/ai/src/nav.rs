use rustix_core::math::Vec3;

/// A navigation mesh triangle.
#[derive(Debug, Clone)]
pub struct NavTriangle {
    pub vertices: [Vec3; 3],
    pub neighbors: [Option<usize>; 3],
}

impl NavTriangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { vertices: [a, b, c], neighbors: [None; 3] }
    }

    /// Check if a 2D point (xz plane) is inside this triangle.
    pub fn contains_point(&self, point: Vec3) -> bool {
        let a = self.vertices[0];
        let b = self.vertices[1];
        let c = self.vertices[2];

        let v0 = c - a;
        let v1 = b - a;
        let v2 = point - a;

        let dot00 = v0.x * v0.x + v0.z * v0.z;
        let dot01 = v0.x * v1.x + v0.z * v1.z;
        let dot02 = v0.x * v2.x + v0.z * v2.z;
        let dot11 = v1.x * v1.x + v1.z * v1.z;
        let dot12 = v1.x * v2.x + v1.z * v2.z;

        let inv = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv;
        let v = (dot00 * dot12 - dot01 * dot02) * inv;

        u >= 0.0 && v >= 0.0 && u + v <= 1.0
    }

    pub fn center(&self) -> Vec3 {
        (self.vertices[0] + self.vertices[1] + self.vertices[2]) / 3.0
    }
}

/// A navigation mesh built from triangles with connectivity.
#[derive(Debug, Clone)]
pub struct NavMesh {
    pub triangles: Vec<NavTriangle>,
}

impl NavMesh {
    pub fn new() -> Self { Self { triangles: Vec::new() } }

    /// Add a triangle and auto-connect neighbors.
    pub fn add_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) -> usize {
        let idx = self.triangles.len();
        let mut tri = NavTriangle::new(a, b, c);

        // Find shared edges with existing triangles
        let mut neighbor_links: Vec<(usize, usize, usize)> = Vec::new(); // (new_tri_edge, existing_idx, existing_edge)
        for (i, existing) in self.triangles.iter().enumerate() {
            for edge_idx in 0..3 {
                let e0 = tri.vertices[edge_idx];
                let e1 = tri.vertices[(edge_idx + 1) % 3];
                for other_edge in 0..3 {
                    let o0 = existing.vertices[other_edge];
                    let o1 = existing.vertices[(other_edge + 1) % 3];
                    if (e0 - o0).length_squared() < 0.001 && (e1 - o1).length_squared() < 0.001
                        || (e0 - o1).length_squared() < 0.001 && (e1 - o0).length_squared() < 0.001
                    {
                        neighbor_links.push((edge_idx, i, other_edge));
                    }
                }
            }
        }

        self.triangles.push(tri);
        for (new_edge, existing_idx, existing_edge) in neighbor_links {
            self.triangles[idx].neighbors[new_edge] = Some(existing_idx);
            self.triangles[existing_idx].neighbors[existing_edge] = Some(idx);
        }
        idx
    }

    /// Find the triangle containing a 2D point (xz plane).
    pub fn find_triangle(&self, point: Vec3) -> Option<usize> {
        self.triangles.iter().position(|tri| tri.contains_point(point))
    }

    /// Build a pathfinder graph from the navmesh (center of each triangle = node).
    pub fn to_pathfinder(&self) -> super::path::PathFinder {
        use super::path::PathNode;
        let nodes: Vec<PathNode> = self.triangles.iter().enumerate()
            .map(|(i, tri)| {
                let c = tri.center();
                PathNode::new(i, c.x, c.z)
            })
            .collect();

        let edges: Vec<Vec<(usize, f32)>> = self.triangles.iter().enumerate()
            .map(|(i, tri)| {
                tri.neighbors.iter()
                    .filter_map(|&n| n.map(|nid| {
                        let dist = (nodes[i].x - nodes[nid].x).hypot(nodes[i].y - nodes[nid].y);
                        (nid, dist)
                    }))
                    .collect()
            })
            .collect();

        super::path::PathFinder::new(nodes, edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_contains() {
        let tri = NavTriangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
        );
        assert!(tri.contains_point(Vec3::new(0.5, 0.0, 0.5)));
        assert!(!tri.contains_point(Vec3::new(1.5, 0.0, 1.5)));
    }

    #[test]
    fn test_navmesh_pathfinding() {
        let mut nav = NavMesh::new();
        // Create two adjacent triangles
        nav.add_triangle(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        nav.add_triangle(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 1.0));

        let pf = nav.to_pathfinder();
        let path = pf.find_path(0, 1);
        assert!(path.is_some());
    }
}
