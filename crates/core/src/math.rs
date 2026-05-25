//! Re-exports of glam with additional game math utilities.

pub use glam::*;

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self {
            min: center - extents,
            max: center + extents,
        }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for p in points {
            min = min.min(*p);
            max = max.max(*p);
        }
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn contains(&self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.cmple(other.max).all() && self.max.cmpge(other.min).all()
    }

    pub fn transform(&self, transform: Mat4) -> Self {
        let center = transform.transform_point3(self.center());
        let extents = self.extents();
        let new_extents = transform.abs() * extents.extend(0.0);
        Self::from_center_extents(center, new_extents.truncate())
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    pub fn volume(&self) -> f32 {
        let s = self.size();
        s.x * s.y * s.z
    }

    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };
}

/// A bounding sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        let aabb = Aabb::from_points(points);
        let center = aabb.center();
        let radius = points
            .iter()
            .map(|p| p.distance(center))
            .fold(0.0f32, f32::max);
        Self { center, radius }
    }

    pub fn contains(&self, point: Vec3) -> bool {
        self.center.distance_squared(point) <= self.radius * self.radius
    }

    pub fn intersects(&self, other: &Sphere) -> bool {
        let dist_sq = self.center.distance_squared(other.center);
        let r_sum = self.radius + other.radius;
        dist_sq <= r_sum * r_sum
    }
}

/// View frustum defined by 6 planes.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

/// A plane defined by a normal and distance from origin.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn new(normal: Vec3, d: f32) -> Self {
        Self {
            normal: normal.normalize(),
            d,
        }
    }

    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Self {
        let normal = normal.normalize();
        Self {
            normal,
            d: -point.dot(normal),
        }
    }

    pub fn distance_to(&self, point: Vec3) -> f32 {
        point.dot(self.normal) + self.d
    }
}

impl Frustum {
    /// Extract frustum planes from a view-projection matrix.
    pub fn from_view_proj(view_proj: &Mat4) -> Self {
        let m = view_proj;

        // Left plane (column 4 + column 1)
        let left = Plane {
            normal: Vec3::new(m.w_axis.x + m.x_axis.x, m.w_axis.y + m.x_axis.y, m.w_axis.z + m.x_axis.z),
            d: m.w_axis.w + m.x_axis.w,
        };
        // Right plane (column 4 - column 1)
        let right = Plane {
            normal: Vec3::new(m.w_axis.x - m.x_axis.x, m.w_axis.y - m.x_axis.y, m.w_axis.z - m.x_axis.z),
            d: m.w_axis.w - m.x_axis.w,
        };
        // Bottom plane (column 4 + column 2)
        let bottom = Plane {
            normal: Vec3::new(m.w_axis.x + m.y_axis.x, m.w_axis.y + m.y_axis.y, m.w_axis.z + m.y_axis.z),
            d: m.w_axis.w + m.y_axis.w,
        };
        // Top plane (column 4 - column 2)
        let top = Plane {
            normal: Vec3::new(m.w_axis.x - m.y_axis.x, m.w_axis.y - m.y_axis.y, m.w_axis.z - m.y_axis.z),
            d: m.w_axis.w - m.y_axis.w,
        };
        // Near plane (column 4 + column 3)
        let near = Plane {
            normal: Vec3::new(m.w_axis.x + m.z_axis.x, m.w_axis.y + m.z_axis.y, m.w_axis.z + m.z_axis.z),
            d: m.w_axis.w + m.z_axis.w,
        };
        // Far plane (column 4 - column 3)
        let far = Plane {
            normal: Vec3::new(m.w_axis.x - m.z_axis.x, m.w_axis.y - m.z_axis.y, m.w_axis.z - m.z_axis.z),
            d: m.w_axis.w - m.z_axis.w,
        };

        Self {
            planes: [left, right, bottom, top, near, far],
        }
    }

    /// Check if an AABB is inside (or intersecting) the frustum.
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        let center = aabb.center();
        let extents = aabb.extents();

        for plane in &self.planes {
            let signed_extent = extents.x * plane.normal.x.abs()
                + extents.y * plane.normal.y.abs()
                + extents.z * plane.normal.z.abs();

            if plane.distance_to(center) + signed_extent < 0.0 {
                return false;
            }
        }

        true
    }

    /// Check if a sphere is inside (or intersecting) the frustum.
    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        for plane in &self.planes {
            if plane.distance_to(sphere.center) + sphere.radius < 0.0 {
                return false;
            }
        }
        true
    }
}

/// A 3D ray for intersection testing.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn intersect_aabb(&self, aabb: &Aabb) -> Option<f32> {
        let inv_dir = 1.0 / self.direction;
        let t1 = (aabb.min - self.origin) * inv_dir;
        let t2 = (aabb.max - self.origin) * inv_dir;

        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        let t_enter = tmin.max_element();
        let t_exit = tmax.min_element();

        if t_enter <= t_exit && t_exit >= 0.0 {
            Some(if t_enter >= 0.0 { t_enter } else { t_exit })
        } else {
            None
        }
    }

    pub fn intersect_plane(&self, plane: &Plane) -> Option<f32> {
        let denom = plane.normal.dot(self.direction);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = -(plane.normal.dot(self.origin) + plane.d) / denom;
        if t >= 0.0 { Some(t) } else { None }
    }
}

/// Colors in linear space.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba_bytes(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn linear_to_srgb(self) -> Self {
        fn linear_to_srgb_channel(c: f32) -> f32 {
            if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }
        Self {
            r: linear_to_srgb_channel(self.r),
            g: linear_to_srgb_channel(self.g),
            b: linear_to_srgb_channel(self.b),
            a: self.a,
        }
    }

    pub fn srgb_to_linear(self) -> Self {
        fn srgb_to_linear_channel(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        Self {
            r: srgb_to_linear_channel(self.r),
            g: srgb_to_linear_channel(self.g),
            b: srgb_to_linear_channel(self.b),
            a: self.a,
        }
    }

    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
}

/// Interpolation utilities.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
