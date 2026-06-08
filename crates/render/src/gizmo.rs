//! Debug gizmo primitives for the 3D viewport.
//!
//! Generates line-segment vertex data for wireframe shapes (spheres, cones,
//! boxes) that a debug-line renderer can draw over the main scene.  This
//! module is intentionally pure — it only produces CPU-side vertex buffers.
//! The caller uploads them to GPU and issues a line-list draw call.

use rustix_core::math::Vec3;

/// A single colored vertex for a debug line.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl GizmoVertex {
    pub fn new(pos: [f32; 3], color: [f32; 4]) -> Self {
        Self { position: pos, color }
    }
}

/// A debug line segment (2 vertices).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoLine {
    pub a: GizmoVertex,
    pub b: GizmoVertex,
}

impl GizmoLine {
    pub fn new(a: GizmoVertex, b: GizmoVertex) -> Self {
        Self { a, b }
    }
}

/// Generate a wireframe sphere as line segments.
///
/// `segments` controls the resolution of the latitude/longitude rings.
/// Returns a flat list of line vertices (2 per segment).
pub fn wireframe_sphere(center: Vec3, radius: f32, color: [f32; 4], segments: u32) -> Vec<GizmoLine> {
    let mut lines = Vec::new();
    let seg = segments.max(3);

    // Longitude rings (vertical)
    for i in 0..seg {
        let phi0 = (i as f32 / seg as f32) * std::f32::consts::PI * 2.0;
        let phi1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::PI * 2.0;
        for j in 0..seg {
            let theta0 = (j as f32 / seg as f32) * std::f32::consts::PI;
            let _theta1 = ((j + 1) as f32 / seg as f32) * std::f32::consts::PI;

            // Only add lines on the "equator" ring (theta = PI/2) to keep density reasonable
            if (theta0 - std::f32::consts::FRAC_PI_2).abs() < 0.01 {
                let p0 = spherical_to_cartesian(radius, theta0, phi0) + center;
                let p1 = spherical_to_cartesian(radius, theta0, phi1) + center;
                lines.push(GizmoLine::new(
                    GizmoVertex::new(p0.into(), color),
                    GizmoVertex::new(p1.into(), color),
                ));
            }
        }
    }

    // Latitude rings (horizontal)
    for j in 0..=seg / 2 {
        let theta = (j as f32 / seg as f32) * std::f32::consts::PI;
        if theta < 0.01 || (theta - std::f32::consts::PI).abs() < 0.01 {
            continue; // skip poles
        }
        for i in 0..seg {
            let phi0 = (i as f32 / seg as f32) * std::f32::consts::PI * 2.0;
            let phi1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::PI * 2.0;
            let p0 = spherical_to_cartesian(radius, theta, phi0) + center;
            let p1 = spherical_to_cartesian(radius, theta, phi1) + center;
            lines.push(GizmoLine::new(
                GizmoVertex::new(p0.into(), color),
                GizmoVertex::new(p1.into(), color),
            ));
        }
    }

    lines
}

/// Generate a wireframe cone (direction indicator) as line segments.
pub fn wireframe_cone(origin: Vec3, direction: Vec3, length: f32, angle_deg: f32, color: [f32; 4], segments: u32) -> Vec<GizmoLine> {
    let mut lines = Vec::new();
    let seg = segments.max(3);
    let tip = origin + direction.normalize() * length;
    let base_radius = length * (angle_deg.to_radians() * 0.5).tan();

    // Base circle
    let up = if direction.abs().dot(Vec3::Y) > 0.99 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let right = direction.cross(up).normalize();
    let up = right.cross(direction).normalize();
    let base_center = origin + direction.normalize() * (length * 0.5);

    for i in 0..seg {
        let a0 = (i as f32 / seg as f32) * std::f32::consts::PI * 2.0;
        let a1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::PI * 2.0;
        let p0 = base_center + (right * a0.cos() + up * a0.sin()) * base_radius;
        let p1 = base_center + (right * a1.cos() + up * a1.sin()) * base_radius;
        lines.push(GizmoLine::new(
            GizmoVertex::new(p0.into(), color),
            GizmoVertex::new(p1.into(), color),
        ));
    }

    // Ribs from base circle to tip
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::PI * 2.0;
        let p = base_center + (right * a.cos() + up * a.sin()) * base_radius;
        lines.push(GizmoLine::new(
            GizmoVertex::new(p.into(), color),
            GizmoVertex::new(tip.into(), color),
        ));
    }

    lines
}

/// Generate a wireframe box (AABB) as 12 line segments.
pub fn wireframe_box(min: Vec3, max: Vec3, color: [f32; 4]) -> Vec<GizmoLine> {
    let mut lines = Vec::with_capacity(12);
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];

    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0), // bottom
        (4, 5), (5, 6), (6, 7), (7, 4), // top
        (0, 4), (1, 5), (2, 6), (3, 7), // sides
    ];

    for (a, b) in edges {
        lines.push(GizmoLine::new(
            GizmoVertex::new(corners[a].into(), color),
            GizmoVertex::new(corners[b].into(), color),
        ));
    }

    lines
}

// ── Audio source gizmo ──

/// Parameters for an audio-source debug gizmo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioGizmo {
    pub position: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    /// Optional forward direction for cone sources.
    pub direction: Option<Vec3>,
    pub inner_color: [f32; 4],
    pub outer_color: [f32; 4],
}

impl Default for AudioGizmo {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            min_distance: 1.0,
            max_distance: 10.0,
            direction: None,
            inner_color: [0.0, 1.0, 0.5, 1.0],   // cyan-green
            outer_color: [1.0, 0.5, 0.0, 0.4],   // orange, semi-transparent
        }
    }
}

/// Generate line segments for an audio-source gizmo.
///
/// Returns a flat list of line segments ready for a line-list draw call.
/// * Inner sphere at `min_distance` (solid color).
/// * Outer sphere at `max_distance` (faded color).
/// * Optional direction cone if `direction` is set.
pub fn generate_audio_gizmo(gizmo: &AudioGizmo) -> Vec<GizmoLine> {
    let mut lines = Vec::new();

    // Inner sphere (min distance)
    lines.extend(wireframe_sphere(gizmo.position, gizmo.min_distance, gizmo.inner_color, 16));

    // Outer sphere (max distance)
    lines.extend(wireframe_sphere(gizmo.position, gizmo.max_distance, gizmo.outer_color, 24));

    // Direction cone
    if let Some(dir) = gizmo.direction {
        lines.extend(wireframe_cone(
            gizmo.position,
            dir,
            gizmo.max_distance * 0.3,
            45.0,
            gizmo.inner_color,
            12,
        ));
    }

    lines
}

/// Flatten a list of `GizmoLine` into interleaved `[pos, color, pos, color]` f32 data.
pub fn flatten_gizmo_lines(lines: &[GizmoLine]) -> Vec<f32> {
    let mut out = Vec::with_capacity(lines.len() * 14); // 2 verts * (3 pos + 4 color)
    for line in lines {
        out.extend_from_slice(&line.a.position);
        out.extend_from_slice(&line.a.color);
        out.extend_from_slice(&line.b.position);
        out.extend_from_slice(&line.b.color);
    }
    out
}

// ── helpers ──

fn spherical_to_cartesian(r: f32, theta: f32, phi: f32) -> Vec3 {
    Vec3::new(
        r * theta.sin() * phi.cos(),
        r * theta.cos(),
        r * theta.sin() * phi.sin(),
    )
}
