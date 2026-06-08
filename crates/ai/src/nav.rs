use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_physics::{BodyType, Collider, ColliderShape, RigidBody};

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

    /// Find a path from `start_pos` to `goal_pos` using A* over the
    /// navmesh triangle graph.
    ///
    /// Returns the sequence of triangle indices to traverse, or `None`
    /// if either point is outside the mesh or no path exists.
    pub fn find_path_triangles(&self, start_pos: Vec3, goal_pos: Vec3) -> Option<Vec<usize>> {
        let start_tri = self.find_triangle(start_pos)?;
        let goal_tri = self.find_triangle(goal_pos)?;
        let pf = self.to_pathfinder();
        pf.find_path(start_tri, goal_tri)
    }

    /// Find a path from `start_pos` to `goal_pos` and return the
    /// sequence of waypoints (triangle centers) in world space.
    ///
    /// This is a higher-level wrapper around `find_path_triangles`
    /// that returns `Vec<Vec3>` waypoints instead of raw triangle
    /// indices.
    pub fn find_path_waypoints(&self, start_pos: Vec3, goal_pos: Vec3) -> Option<Vec<Vec3>> {
        let indices = self.find_path_triangles(start_pos, goal_pos)?;
        Some(indices.iter().map(|&i| self.triangles[i].center()).collect())
    }
}

/// Explicit source geometry for navmesh generation.
///
/// Attach this to an entity alongside a `Transform` to include its
/// triangle mesh in the navmesh build.
#[derive(Debug, Clone)]
pub struct NavMeshSource {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
}

impl NavMeshSource {
    pub fn new(vertices: Vec<Vec3>, indices: Vec<[u32; 3]>) -> Self {
        Self { vertices, indices }
    }

    pub fn from_box(half_extents: Vec3) -> Self {
        let hx = half_extents.x;
        let hy = half_extents.y;
        let hz = half_extents.z;
        let verts = vec![
            Vec3::new(-hx, -hy, -hz),
            Vec3::new(hx, -hy, -hz),
            Vec3::new(hx, -hy, hz),
            Vec3::new(-hx, -hy, hz),
            Vec3::new(-hx, hy, -hz),
            Vec3::new(hx, hy, -hz),
            Vec3::new(hx, hy, hz),
            Vec3::new(-hx, hy, hz),
        ];
        let idxs = vec![
            // Top face (+Y)
            [4, 5, 6], [4, 6, 7],
            // Bottom face (-Y)
            [0, 2, 1], [0, 3, 2],
            // Front face (+Z)
            [2, 3, 7], [2, 7, 6],
            // Back face (-Z)
            [0, 1, 5], [0, 5, 4],
            // Right face (+X)
            [1, 2, 6], [1, 6, 5],
            // Left face (-X)
            [0, 7, 3], [0, 4, 7],
        ];
        Self::new(verts, idxs)
    }
}

/// Generates a `NavMesh` from static colliders and explicit
/// `NavMeshSource` geometry.
#[derive(Debug, Clone)]
pub struct NavMeshGenerator {
    max_slope_cos: f32,
    triangles: Vec<(Vec3, Vec3, Vec3)>,
}

impl Default for NavMeshGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl NavMeshGenerator {
    pub fn new() -> Self {
        Self {
            max_slope_cos: 0.7071, // 45 degrees
            triangles: Vec::new(),
        }
    }

    /// Set the maximum walkable slope angle in radians.
    /// Default is 45° (π/4).
    pub fn max_slope_angle(mut self, radians: f32) -> Self {
        self.max_slope_cos = radians.cos();
        self
    }

    /// Collect walkable triangles from static `Box` colliders.
    ///
    /// For each static entity with a `Collider` (Box), `RigidBody`
    /// (Static), and `Transform`, this generates the top-face triangles
    /// and adds them if they are within the slope limit.
    pub fn from_colliders(&mut self, world: &EcsWorld) {
        use rustix_core::components::Transform;

        for (body, collider, transform) in world.query::<(&RigidBody, &Collider, &Transform)>().iter() {
            if body.body_type != BodyType::Static {
                continue;
            }
            if let ColliderShape::Box { half_extents } = collider.shape {
                let matrix = transform.matrix();
                // Top face of a box: corners at y = +half_extents.y
                let hy = half_extents.y;
                let hx = half_extents.x;
                let hz = half_extents.z;
                let corners = [
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(-hx, hy, hz),
                ];
                let world_corners: Vec<Vec3> = corners
                    .iter()
                    .map(|&v| {
                        let p = matrix.transform_point3(v);
                        p
                    })
                    .collect();

                // Two triangles for the top face
                self.add_triangle_if_walkable(world_corners[0], world_corners[1], world_corners[2]);
                self.add_triangle_if_walkable(world_corners[0], world_corners[2], world_corners[3]);
            }
        }
    }

    /// Collect triangles from entities with an explicit `NavMeshSource`.
    pub fn from_sources(&mut self, world: &EcsWorld) {
        use rustix_core::components::Transform;

        for (source, transform) in world.query::<(&NavMeshSource, &Transform)>().iter() {
            let matrix = transform.matrix();
            for idx in &source.indices {
                let a = matrix.transform_point3(source.vertices[idx[0] as usize]);
                let b = matrix.transform_point3(source.vertices[idx[1] as usize]);
                let c = matrix.transform_point3(source.vertices[idx[2] as usize]);
                self.add_triangle_if_walkable(a, b, c);
            }
        }
    }

    /// Build the final `NavMesh` from all collected walkable triangles.
    pub fn build(self) -> NavMesh {
        let mut nav = NavMesh::new();
        for (a, b, c) in self.triangles {
            nav.add_triangle(a, b, c);
        }
        nav
    }

    fn add_triangle_if_walkable(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let normal = (b - a).cross(c - a);
        let len = normal.length();
        if len < 1e-6 {
            return; // degenerate
        }
        let ny = normal.y / len;
        if ny.abs() >= self.max_slope_cos {
            self.triangles.push((a, b, c));
        }
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

    #[test]
    fn test_query_counts_entities() {
        use rustix_core::components::Transform;
        use rustix_physics::{Collider, RigidBody, BodyType, ColliderShape};

        let mut world = EcsWorld::new();
        world.spawn((
            RigidBody {
                body_type: BodyType::Static,
                ..Default::default()
            },
            Collider {
                shape: ColliderShape::Box { half_extents: Vec3::new(5.0, 0.5, 5.0) },
                ..Default::default()
            },
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                ..Default::default()
            },
        ));

        let count = world.query::<(&RigidBody, &Collider, &Transform)>().iter().count();
        assert_eq!(count, 1, "query should find exactly 1 entity with all 3 components");
    }

    #[test]
    fn test_navmesh_generator_from_box_colliders() {
        use rustix_core::components::Transform;
        use rustix_physics::{Collider, RigidBody, BodyType, ColliderShape};

        let mut world = EcsWorld::new();

        // Static floor box at y=0
        let _floor = world.spawn((
            RigidBody {
                body_type: BodyType::Static,
                ..Default::default()
            },
            Collider {
                shape: ColliderShape::Box { half_extents: Vec3::new(5.0, 0.5, 5.0) },
                ..Default::default()
            },
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                ..Default::default()
            },
        ));

        // Dynamic box (should be ignored)
        let _dynamic = world.spawn((
            RigidBody {
                body_type: BodyType::Dynamic,
                ..Default::default()
            },
            Collider {
                shape: ColliderShape::Box { half_extents: Vec3::new(1.0, 1.0, 1.0) },
                ..Default::default()
            },
            Transform {
                translation: Vec3::new(10.0, 0.0, 0.0),
                ..Default::default()
            },
        ));

        let mut gen = NavMeshGenerator::new();
        gen.from_colliders(&world);
        let nav = gen.build();

        // Should have 2 triangles from the static box top face
        assert_eq!(nav.triangles.len(), 2, "expected 2 triangles from static box top face, got {}", nav.triangles.len());
    }

    #[test]
    fn test_navmesh_generator_from_sources() {
        use rustix_core::components::Transform;

        let mut world = EcsWorld::new();

        let source = NavMeshSource::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2]],
        );

        world.spawn((
            source,
            Transform {
                translation: Vec3::new(5.0, 0.0, 0.0),
                ..Default::default()
            },
        ));

        let mut gen = NavMeshGenerator::new();
        gen.from_sources(&world);
        let nav = gen.build();

        assert_eq!(nav.triangles.len(), 1);
        // Should be translated by (5, 0, 0)
        assert!((nav.triangles[0].vertices[0].x - 5.0).abs() < 0.001);
    }
}
