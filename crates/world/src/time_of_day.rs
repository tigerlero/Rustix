//! Time-of-day system: sun/moon cycle, dynamic sky, and light color.

use rustix_core::math::Vec3;

/// 24-hour time represented as hours [0, 24).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeOfDay {
    pub hours: f32,
}

impl TimeOfDay {
    pub fn new(hours: f32) -> Self {
        Self { hours: hours.rem_euclid(24.0) }
    }

    pub fn advance(&mut self, delta_hours: f32) {
        self.hours = (self.hours + delta_hours).rem_euclid(24.0);
    }

    /// Sun direction (simplified: rises east, sets west, south at noon).
    pub fn sun_direction(&self) -> Vec3 {
        let t = (self.hours - 6.0) / 12.0; // 0 at 6am, 1 at 6pm
        let angle = t * std::f32::consts::PI;
        Vec3::new(-angle.cos(), angle.sin(), 0.0).normalize_or_zero()
    }

    /// Moon direction (opposite the sun).
    pub fn moon_direction(&self) -> Vec3 {
        -self.sun_direction()
    }

    /// Ambient light color based on time of day.
    pub fn ambient_color(&self) -> [f32; 3] {
        let t = self.hours / 24.0;
        // Night -> dawn -> day -> dusk -> night
        if t < 0.2 || t > 0.8 {
            [0.05, 0.05, 0.1] // night
        } else if t < 0.25 {
            [0.3, 0.2, 0.15] // dawn
        } else if t < 0.75 {
            [0.6, 0.6, 0.55] // day
        } else {
            [0.3, 0.2, 0.15] // dusk
        }
    }

    /// Sun light color based on time of day.
    pub fn sun_color(&self) -> [f32; 3] {
        let t = self.hours / 24.0;
        if t < 0.2 || t > 0.8 {
            [0.0, 0.0, 0.0] // night
        } else if t < 0.25 {
            [1.0, 0.6, 0.3] // dawn
        } else if t < 0.75 {
            [1.0, 0.98, 0.95] // day
        } else {
            [1.0, 0.5, 0.2] // dusk
        }
    }
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self::new(12.0)
    }
}
