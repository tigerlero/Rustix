//! Tests for weather system state and interpolation.

use crate::weather::{WeatherState, lerp_weather};
use rustix_core::math::Vec3;

#[test]
fn weather_default_is_clear() {
    let w = WeatherState::new();
    assert!(!w.is_precipitating());
    assert!(!w.is_raining());
    assert!(!w.is_snowing());
}

#[test]
fn weather_rain_sets_properties() {
    let w = WeatherState::new().rain(0.8);
    assert!(w.is_precipitating());
    assert!(w.is_raining());
    assert!(!w.is_snowing());
    assert_eq!(w.snow_factor, 0.0);
    assert!(w.fog_density > 0.0);
}

#[test]
fn weather_snow_sets_properties() {
    let w = WeatherState::new().snow(0.8);
    assert!(w.is_precipitating());
    assert!(!w.is_raining());
    assert!(w.is_snowing());
    assert_eq!(w.snow_factor, 1.0);
}

#[test]
fn weather_clear_resets() {
    let w = WeatherState::new().rain(1.0).clear();
    assert!(!w.is_precipitating());
    assert_eq!(w.precipitation, 0.0);
}

#[test]
fn weather_wind_sets_direction() {
    let w = WeatherState::new().wind(Vec3::Y, 10.0);
    assert!((w.wind.length() - 10.0).abs() < 1e-4);
    assert!(w.wind.y > 0.0);
}

#[test]
fn lerp_weather_midpoint() {
    let a = WeatherState::new();
    let b = WeatherState::new().rain(1.0);
    let mid = lerp_weather(&a, &b, 0.5);
    assert!((mid.precipitation - 0.5).abs() < 1e-4);
    assert!((mid.fog_density - b.fog_density * 0.5).abs() < 1e-4);
}

#[test]
fn lerp_weather_clamps_t() {
    let a = WeatherState::new();
    let b = WeatherState::new().rain(1.0);
    let over = lerp_weather(&a, &b, 2.0);
    assert_eq!(over.precipitation, 1.0);
    let under = lerp_weather(&a, &b, -1.0);
    assert_eq!(under.precipitation, 0.0);
}

#[test]
fn lerp_weather_wind_interpolation() {
    let mut a = WeatherState::new();
    a.wind = Vec3::X * 5.0;
    let mut b = WeatherState::new();
    b.wind = Vec3::X * 15.0;
    let mid = lerp_weather(&a, &b, 0.5);
    assert!((mid.wind.x - 10.0).abs() < 1e-4);
}
