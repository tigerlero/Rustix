//! Weather system: rain, snow, fog, wind, procedural clouds.

use rustix_core::math::Vec3;

/// Current weather state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherState {
    /// Precipitation intensity [0, 1].
    pub precipitation: f32,
    /// 0 = rain, 1 = snow.
    pub snow_factor: f32,
    /// Fog density [0, 1].
    pub fog_density: f32,
    /// Fog color (RGB).
    pub fog_color: [f32; 3],
    /// Wind direction and strength (m/s).
    pub wind: Vec3,
    /// Cloud coverage [0, 1].
    pub cloud_coverage: f32,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            precipitation: 0.0,
            snow_factor: 0.0,
            fog_density: 0.0,
            fog_color: [0.7, 0.75, 0.8],
            wind: Vec3::new(1.0, 0.0, 0.0),
            cloud_coverage: 0.3,
        }
    }
}

impl WeatherState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rain(mut self, intensity: f32) -> Self {
        self.precipitation = intensity.clamp(0.0, 1.0);
        self.snow_factor = 0.0;
        self.fog_density = intensity * 0.3;
        self.cloud_coverage = 0.5 + intensity * 0.4;
        self
    }

    pub fn snow(mut self, intensity: f32) -> Self {
        self.precipitation = intensity.clamp(0.0, 1.0);
        self.snow_factor = 1.0;
        self.fog_density = intensity * 0.5;
        self.cloud_coverage = 0.6 + intensity * 0.3;
        self
    }

    pub fn wind(mut self, direction: Vec3, speed: f32) -> Self {
        self.wind = direction.normalize_or_zero() * speed;
        self
    }

    pub fn clear(mut self) -> Self {
        self.precipitation = 0.0;
        self.fog_density = 0.0;
        self.cloud_coverage = 0.1;
        self
    }

    /// Is it currently precipitating?
    pub fn is_precipitating(&self) -> bool {
        self.precipitation > 0.01
    }

    /// Is snow falling?
    pub fn is_snowing(&self) -> bool {
        self.is_precipitating() && self.snow_factor > 0.5
    }

    /// Is rain falling?
    pub fn is_raining(&self) -> bool {
        self.is_precipitating() && self.snow_factor <= 0.5
    }
}

/// Interpolate between two weather states.
pub fn lerp_weather(a: &WeatherState, b: &WeatherState, t: f32) -> WeatherState {
    let t = t.clamp(0.0, 1.0);
    WeatherState {
        precipitation: a.precipitation + (b.precipitation - a.precipitation) * t,
        snow_factor: a.snow_factor + (b.snow_factor - a.snow_factor) * t,
        fog_density: a.fog_density + (b.fog_density - a.fog_density) * t,
        fog_color: [
            a.fog_color[0] + (b.fog_color[0] - a.fog_color[0]) * t,
            a.fog_color[1] + (b.fog_color[1] - a.fog_color[1]) * t,
            a.fog_color[2] + (b.fog_color[2] - a.fog_color[2]) * t,
        ],
        wind: a.wind + (b.wind - a.wind) * t,
        cloud_coverage: a.cloud_coverage + (b.cloud_coverage - a.cloud_coverage) * t,
    }
}
