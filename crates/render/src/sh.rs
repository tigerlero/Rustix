//! Spherical Harmonics (SH) utilities for irradiance representation.
//!
//! Provides L0-L1 (4 coefficients) and full L2 (9 coefficients) per-channel
//! SH evaluation, plus helpers for projecting directional light and ambient
//! into SH coefficients.  Used by the irradiance-volume GI fallback.

use glam::Vec3;

/// L1 spherical harmonics coefficients for one RGB channel.
/// Layout: [L00, L1n1, L10, L11] where:
/// - L00  = constant term
/// - L1n1 = y-band (aligned with +Y)
/// - L10  = z-band (aligned with +Z)
/// - L11  = x-band (aligned with +X)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShL1 {
    pub c: [f32; 4],
}

/// Full L2 spherical harmonics coefficients for one RGB channel (9 coefficients).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShL2 {
    pub c: [f32; 9],
}

/// RGB irradiance represented as three independent SH L1 sets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShIrradianceL1 {
    pub r: ShL1,
    pub g: ShL1,
    pub b: ShL1,
}

impl ShL1 {
    /// Evaluate the SH at a given normal direction.
    /// `n` should be normalized.
    pub fn eval(&self, n: Vec3) -> f32 {
        self.c[0]
            + self.c[1] * n.y
            + self.c[2] * n.z
            + self.c[3] * n.x
    }

    /// Project a single directional light of given intensity into L1 SH.
    /// `dir` should point TOWARD the light and be normalized.
    /// `intensity` is the radiance (or irradiance) contribution.
    pub fn project_directional(&mut self, dir: Vec3, intensity: f32) {
        self.c[0] += intensity * 0.282_094_8; // sqrt(1/4π)
        self.c[1] += intensity * 0.488_602_5 * dir.y; // sqrt(3/4π) * Y
        self.c[2] += intensity * 0.488_602_5 * dir.z; // sqrt(3/4π) * Z
        self.c[3] += intensity * 0.488_602_5 * dir.x; // sqrt(3/4π) * X
    }

    /// Project a uniform ambient term into L0.
    pub fn project_ambient(&mut self, ambient: f32) {
        self.c[0] += ambient * 0.282_094_8;
    }

    /// Scale all coefficients by a scalar.
    pub fn scale(&mut self, s: f32) {
        for v in &mut self.c {
            *v *= s;
        }
    }
}

impl ShIrradianceL1 {
    /// Evaluate RGB irradiance at the given normal.
    pub fn eval(&self, n: Vec3) -> Vec3 {
        Vec3::new(
            self.r.eval(n),
            self.g.eval(n),
            self.b.eval(n),
        )
    }

    /// Project a colored directional light.
    pub fn project_directional(&mut self, dir: Vec3, color: Vec3) {
        self.r.project_directional(dir, color.x);
        self.g.project_directional(dir, color.y);
        self.b.project_directional(dir, color.z);
    }

    /// Project a colored ambient term.
    pub fn project_ambient(&mut self, color: Vec3) {
        self.r.project_ambient(color.x);
        self.g.project_ambient(color.y);
        self.b.project_ambient(color.z);
    }

    /// Scale all coefficients.
    pub fn scale(&mut self, s: f32) {
        self.r.scale(s);
        self.g.scale(s);
        self.b.scale(s);
    }

    /// Create an SH representation from a single directional light + ambient.
    pub fn from_directional_and_ambient(dir: Vec3, color: Vec3, ambient: Vec3) -> Self {
        let mut sh = Self::default();
        sh.project_directional(dir, color);
        sh.project_ambient(ambient);
        sh
    }
}

/// L2 spherical harmonics evaluation.
impl ShL2 {
    /// Evaluate the SH at a given normal direction.
    pub fn eval(&self, n: Vec3) -> f32 {
        let x = n.x;
        let y = n.y;
        let z = n.z;

        self.c[0]                            // L00
        + self.c[1] * y                       // L1-1
        + self.c[2] * z                       // L10
        + self.c[3] * x                       // L11
        + self.c[4] * (y * x)                 // L2-2
        + self.c[5] * (y * z)                // L2-1
        + self.c[6] * (3.0 * z * z - 1.0)    // L20
        + self.c[7] * (z * x)                // L21
        + self.c[8] * (x * x - y * y)        // L22
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_eval_at_y_up_is_const_plus_y_band() {
        let sh = ShL1 { c: [1.0, 2.0, 3.0, 4.0] };
        let n = Vec3::Y;
        let v = sh.eval(n);
        assert!((v - 3.0).abs() < 0.001, "eval at Y-up should be c0 + c1, got {}", v);
    }

    #[test]
    fn l1_eval_at_neg_y_is_const_minus_y_band() {
        let sh = ShL1 { c: [1.0, 2.0, 3.0, 4.0] };
        let n = -Vec3::Y;
        let v = sh.eval(n);
        assert!((v - (-1.0)).abs() < 0.001, "eval at -Y should be c0 - c1, got {}", v);
    }

    #[test]
    fn l1_project_directional_roundtrip() {
        let mut sh = ShL1::default();
        let dir = Vec3::Y;
        sh.project_directional(dir, 1.0);
        let v = sh.eval(dir);
        assert!(v > 0.0, "projected directional should evaluate positively along its axis");
    }

    #[test]
    fn irradiance_l1_directional_and_ambient() {
        let dir = Vec3::new(0.0, 1.0, 0.0);
        let color = Vec3::new(1.0, 0.5, 0.25);
        let ambient = Vec3::new(0.1, 0.1, 0.1);
        let sh = ShIrradianceL1::from_directional_and_ambient(dir, color, ambient);

        // Facing the light should be brighter than facing away.
        let lit = sh.eval(dir);
        let unlit = sh.eval(-dir);
        assert!(lit.x > unlit.x, "facing light should be brighter than facing away");
        assert!(lit.y > unlit.y, "facing light should be brighter than facing away");
        assert!(lit.z > unlit.z, "facing light should be brighter than facing away");
    }

    #[test]
    fn l2_eval_is_symmetric_for_z_aligned() {
        let sh = ShL2 { c: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0] };
        let v1 = sh.eval(Vec3::Z);
        let v2 = sh.eval(-Vec3::Z);
        // Only L00 and L20 terms used; L20: (3*z^2 - 1) is symmetric in z.
        assert!((v1 - v2).abs() < 0.001, "L00+L20 SH should be symmetric about Z, got {} and {}", v1, v2);
    }

    #[test]
    fn l1_scale_preserves_ratios() {
        let mut sh = ShL1 { c: [1.0, 2.0, 3.0, 4.0] };
        sh.scale(0.5);
        assert_eq!(sh.c, [0.5, 1.0, 1.5, 2.0]);
    }
}
