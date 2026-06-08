//! Debug draw primitives for AI visualization.
//!
//! Provides CPU-side line / point / cone data that a renderer can
//! consume to draw paths, state labels, sensor ranges, and influence
//! maps.

use rustix_core::math::Vec3;

/// A colored line segment for debug rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

impl DebugLine {
    pub fn new(start: Vec3, end: Vec3, color: [f32; 4]) -> Self {
        Self { start, end, color }
    }

    pub fn red(start: Vec3, end: Vec3) -> Self {
        Self::new(start, end, [1.0, 0.0, 0.0, 1.0])
    }

    pub fn green(start: Vec3, end: Vec3) -> Self {
        Self::new(start, end, [0.0, 1.0, 0.0, 1.0])
    }

    pub fn blue(start: Vec3, end: Vec3) -> Self {
        Self::new(start, end, [0.0, 0.0, 1.0, 1.0])
    }
}

/// A debug point / sphere marker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugPoint {
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 4],
}

impl DebugPoint {
    pub fn new(position: Vec3, radius: f32, color: [f32; 4]) -> Self {
        Self { position, radius, color }
    }
}

/// A debug text label anchored in world space.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugLabel {
    pub position: Vec3,
    pub text: String,
    pub color: [f32; 4],
}

impl DebugLabel {
    pub fn new(position: Vec3, text: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            position,
            text: text.into(),
            color,
        }
    }
}

/// Accumulated debug geometry for a single frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiDebugDraw {
    pub lines: Vec<DebugLine>,
    pub points: Vec<DebugPoint>,
    pub labels: Vec<DebugLabel>,
}

impl AiDebugDraw {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.points.clear();
        self.labels.clear();
    }

    pub fn line(&mut self, start: Vec3, end: Vec3, color: [f32; 4]) {
        self.lines.push(DebugLine::new(start, end, color));
    }

    pub fn point(&mut self, position: Vec3, radius: f32, color: [f32; 4]) {
        self.points.push(DebugPoint::new(position, radius, color));
    }

    pub fn label(&mut self, position: Vec3, text: impl Into<String>, color: [f32; 4]) {
        self.labels.push(DebugLabel::new(position, text, color));
    }

    /// Draw a path as a polyline of green segments.
    pub fn draw_path(&mut self, waypoints: &[Vec3]) {
        for window in waypoints.windows(2) {
            self.line(window[0], window[1], [0.0, 1.0, 0.0, 1.0]);
        }
    }

    /// Draw a vision cone as a wireframe wedge.
    pub fn draw_vision_cone(&mut self, origin: Vec3, forward: Vec3, fov_deg: f32, max_distance: f32) {
        let half_fov = fov_deg.to_radians() * 0.5;
        let left = rotate_y(forward, half_fov);
        let right = rotate_y(forward, -half_fov);
        let left_end = origin + left * max_distance;
        let right_end = origin + right * max_distance;
        self.line(origin, left_end, [0.0, 1.0, 1.0, 0.5]);
        self.line(origin, right_end, [0.0, 1.0, 1.0, 0.5]);
        self.line(left_end, right_end, [0.0, 1.0, 1.0, 0.5]);
    }

    /// Draw a hearing radius as a wireframe circle (XZ plane).
    pub fn draw_hearing_radius(&mut self, origin: Vec3, radius: f32) {
        const SEGMENTS: usize = 32;
        for i in 0..SEGMENTS {
            let a0 = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
            let p0 = origin + Vec3::new(a0.cos() * radius, 0.0, a0.sin() * radius);
            let p1 = origin + Vec3::new(a1.cos() * radius, 0.0, a1.sin() * radius);
            self.line(p0, p1, [1.0, 1.0, 0.0, 0.5]);
        }
    }

    /// Draw an influence map as a grid of colored points.
    pub fn draw_influence_map(
        &mut self,
        map: &crate::influence::InfluenceMap,
        y_offset: f32,
    ) {
        for y in 0..map.height {
            for x in 0..map.width {
                let v = map.get(x, y);
                if v.abs() < 0.01 {
                    continue;
                }
                let (wx, wz) = map.grid_to_world(x, y);
                let pos = Vec3::new(wx, y_offset, wz);
                let color = if v > 0.0 {
                    [0.0, v.min(1.0), 0.0, 0.6]
                } else {
                    [v.abs().min(1.0), 0.0, 0.0, 0.6]
                };
                self.point(pos, map.cell_size * 0.4, color);
            }
        }
    }

    /// Draw a state-machine label above an agent.
    pub fn draw_fsm_state(&mut self, agent_pos: Vec3, state_name: &str) {
        self.label(
            agent_pos + Vec3::new(0.0, 2.0, 0.0),
            state_name.to_string(),
            [1.0, 1.0, 1.0, 1.0],
        );
    }
}

fn rotate_y(v: Vec3, angle: f32) -> Vec3 {
    let cos = angle.cos();
    let sin = angle.sin();
    Vec3::new(v.x * cos - v.z * sin, v.y, v.x * sin + v.z * cos)
}
