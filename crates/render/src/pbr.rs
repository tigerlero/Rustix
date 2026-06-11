//! Cook-Torrance GGX PBR BRDF
//!
//! CPU-side reference implementation matching the GPU shaders in
//! `shader/builtin/forward.rs`, `instanced.rs`, and `oit.rs`.
//!
//! Provides validation utilities to ensure energy conservation and
//! agreement with standard microfacet models.

use glam::Vec3;

const PI: f32 = std::f32::consts::PI;

/// Schlick Fresnel approximation.
/// `cos_theta` = dot(H, V) or dot(N, V)
/// `f0` = reflectance at normal incidence (0.04 for dielectrics, albedo for metals)
pub fn fresnel_schlick(cos_theta: f32, f0: Vec3) -> Vec3 {
    f0 + (Vec3::ONE - f0) * (1.0f32 - cos_theta).powi(5).max(0.0)
}

/// GGX (Trowbridge-Reitz) normal distribution function.
pub fn distribution_ggx(ndoth: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = ndoth * ndoth * (a2 - 1.0) + 1.0;
    a2 / (PI * denom * denom).max(0.0001)
}

/// Smith GGX geometry function for a single direction.
pub fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    ndotv / (ndotv * (1.0 - a) + a).max(0.0001)
}

/// Smith GGX geometry for both view and light directions.
pub fn geometry_smith(ndotv: f32, ndotl: f32, roughness: f32) -> f32 {
    geometry_schlick_ggx(ndotv, roughness) * geometry_schlick_ggx(ndotl, roughness)
}

/// Cook-Torrance specular BRDF term.
/// Returns the specular reflectance per unit solid angle.
pub fn specular_brdf(ndoth: f32, ndotv: f32, ndotl: f32, roughness: f32, f0: Vec3) -> Vec3 {
    let d = distribution_ggx(ndoth, roughness);
    let g = geometry_smith(ndotv, ndotl, roughness);
    let f = fresnel_schlick(ndoth.max(0.0), f0);
    let denom = (4.0 * ndotv * ndotl).max(0.0001);
    (d * g * f) / denom
}

/// Full direct-light PBR shading function matching the GPU `pbrDirect`.
///
/// Energy is conserved by attenuating Lambertian diffuse with `(1.0 - Fresnel)`
/// and zeroing diffuse for metals via `(1.0 - metallic)`.
///
/// Returns radiance (not just BRDF weight) — already multiplied by
/// `light_color * NdotL`.
pub fn pbr_direct(
    n: Vec3, l: Vec3, v: Vec3,
    light_color: Vec3, base: Vec3,
    roughness: f32, metallic: f32,
) -> Vec3 {
    let ndotl = n.dot(l).max(0.0);
    if ndotl <= 0.0 {
        return Vec3::ZERO;
    }
    let ndotv = n.dot(v).max(0.0);
    if ndotv <= 0.0 {
        return Vec3::ZERO;
    }

    let h = (l + v).normalize();
    let ndoth = n.dot(h).max(0.0);
    let hdotv = h.dot(v).max(0.0);

    let f0 = Vec3::splat(0.04).lerp(base, metallic);
    let f = fresnel_schlick(hdotv, f0);

    let spec = specular_brdf(ndoth, ndotv, ndotl, roughness, f0);

    // Energy-conserving diffuse: kD = (1 - F) * (1 - metallic)
    let k_d = (Vec3::ONE - f) * (1.0 - metallic);
    let diff = base * k_d / PI;

    (diff + spec) * light_color * ndotl
}

/// Hemisphere-integral approximation used for energy-conservation sanity checks.
/// For a perfectly white diffuse lambert surface the integral over a
/// hemisphere of NdotL is PI.  Therefore `diffuse * PI` should never exceed
/// `base`.
pub fn diffuse_hemisphere_integral(diffuse: Vec3) -> Vec3 {
    diffuse * PI
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec3(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }

    #[test]
    fn fresnel_schlick_at_one_is_f0() {
        // cos_theta = 1.0 → normal incidence → Fresnel = F0
        let f0 = vec3(0.04, 0.04, 0.04);
        let f = fresnel_schlick(1.0, f0);
        assert!((f - f0).length() < 0.001, "F(1.0) should be F0");
    }

    #[test]
    fn fresnel_schlick_at_zero_is_one() {
        // cos_theta = 0.0 → grazing angle → Fresnel → 1.0
        let f0 = vec3(0.04, 0.04, 0.04);
        let f = fresnel_schlick(0.0, f0);
        assert!((f - Vec3::ONE).length() < 0.001, "F(0.0) should be 1.0 at grazing");
    }

    #[test]
    fn metallic_produces_no_diffuse() {
        let n = Vec3::Y;
        let l = Vec3::Y;
        let v = Vec3::Y;
        let light_color = Vec3::ONE;
        let base = vec3(1.0, 0.0, 0.0);
        let roughness = 0.5;
        let metallic = 1.0;

        let result = pbr_direct(n, l, v, light_color, base, roughness, metallic);
        // For metallic=1.0, diffuse is zero.  Result should be purely specular.
        // At N=V=L=up, H=V, cosTheta=1.0 → Fresnel = F0 = base (since metallic=1).
        // Specular should be positive.
        assert!(result.x > 0.0, "metallic surface should still reflect specularly");
        // Diffuse component should be zero → total RGB dominated by red channel.
        assert!(result.y.abs() < 0.001 && result.z.abs() < 0.001,
            "pure metallic diffuse should be zero, leaving only albedo-tinted specular");
    }

    #[test]
    fn dielectric_has_small_specular_and_full_diffuse() {
        let n = Vec3::Y;
        let l = Vec3::Y;
        let v = Vec3::Y;
        let light_color = Vec3::ONE;
        let base = vec3(0.5, 0.5, 0.5);
        let roughness = 0.5;
        let metallic = 0.0;

        let result = pbr_direct(n, l, v, light_color, base, roughness, metallic);
        // For dielectric F0 = 0.04, so specular is small.
        // Diffuse = base * (1 - F) / PI * light * NdotL ≈ 0.5 * 0.96 / PI ≈ 0.153
        // Total should be dominated by diffuse.
        assert!(result.x > 0.1 && result.x < 0.25,
            "dielectric diffuse should dominate and be in expected range");
    }

    #[test]
    fn energy_conservation_for_white_diffuse() {
        // A perfectly white Lambert surface should reflect at most all incoming energy.
        let n = Vec3::Y;
        let l = Vec3::Y;
        let v = Vec3::Y;
        let light_color = Vec3::ONE;
        let base = Vec3::ONE;
        let roughness = 1.0; // maximally rough → tiny specular
        let metallic = 0.0;

        let result = pbr_direct(n, l, v, light_color, base, roughness, metallic);
        // result = (diff + spec) * 1.0 * 1.0
        // diff = base * (1-F) / PI * 1.0 * 1.0 ≈ 1.0 * 0.96 / PI ≈ 0.306
        // spec is tiny for roughness=1.0
        // So result should be < 1.0
        assert!(result.x < 1.0, "energy should not exceed incoming light");
        assert!(result.x > 0.25, "white diffuse should reflect a fair amount");
    }

    #[test]
    fn energy_conservation_metallic_at_grazing() {
        // At grazing angles Fresnel approaches 1.0 for any material.
        // For a metal, diffuse is already zero, so all energy goes to specular.
        let n = Vec3::Y;
        let l = Vec3::new(0.0, 1.0, 0.001).normalize(); // almost exactly up
        let v = Vec3::new(0.0, 0.001, 1.0).normalize(); // grazing view
        let light_color = Vec3::ONE;
        let base = Vec3::ONE;
        let roughness = 0.1;
        let metallic = 1.0;

        let result = pbr_direct(n, l, v, light_color, base, roughness, metallic);
        // The specular BRDF can be very bright at grazing, but the
        // hemispherical integral over all view directions should still
        // conserve energy for Cook-Torrance.
        // At this specific configuration result should be finite.
        assert!(result.is_finite(), "result should be finite at grazing angles");
        assert!(result.x > 0.0, "metallic grazing should still produce reflection");
    }

    #[test]
    fn roughness_one_produces_minimal_specular() {
        let n = Vec3::Y;
        let l = Vec3::Y;
        let v = Vec3::Y;
        let light_color = Vec3::ONE;
        let base = Vec3::ONE;
        let roughness = 1.0;
        let metallic = 0.0;

        let result = pbr_direct(n, l, v, light_color, base, roughness, metallic);
        // D_GGX(1.0, 1.0) = 1.0 / PI
        // G = ~0.5
        // F = 0.04
        // spec ≈ (1/PI * 0.5 * 0.04) / 4  ≈ 0.0016
        // diff ≈ 1.0 * (1 - 0.04) / PI ≈ 0.306
        let specular_component = result.x - (base.x * (1.0 - 0.04) / PI);
        assert!(specular_component.abs() < 0.02,
            "roughness=1.0 should produce minimal specular compared to diffuse");
    }

    #[test]
    fn distribution_ggx_is_normalized() {
        // Integrate D_GGX * cos(theta) over hemisphere numerically.
        // Should be ≈ 1.0 for any roughness.
        let roughness = 0.5;
        let steps = 1000;
        let mut integral = 0.0f32;
        for i in 0..steps {
            let theta = (i as f32 + 0.5) * (std::f32::consts::FRAC_PI_2 / steps as f32);
            let ndoth = theta.cos();
            let d = distribution_ggx(ndoth.max(0.0001), roughness);
            let cos_theta = ndoth;
            let sin_theta = theta.sin();
            integral += d * cos_theta * sin_theta;
        }
        integral *= std::f32::consts::FRAC_PI_2 / steps as f32;
        integral *= 2.0 * PI; // azimuthal integration
        assert!((integral - 1.0).abs() < 0.05,
            "GGX distribution integral should be ~1.0, got {}", integral);
    }

    #[test]
    fn brdf_matches_reference_for_perpendicular() {
        // When N=V=L, H=V=N, ndoth=ndotv=ndotl=1.0
        // For dielectric F0=0.04, roughness=0.5
        let ndoth = 1.0f32;
        let ndotv = 1.0f32;
        let ndotl = 1.0f32;
        let roughness = 0.5;
        let f0 = Vec3::splat(0.04);

        let spec = specular_brdf(ndoth, ndotv, ndotl, roughness, f0);
        // D_GGX(1.0, 0.5): a=0.25, a2=0.0625, denom=0.0625, D≈5.093
        // G_Smith(1.0, 1.0, 0.5) = 1.0
        // F_Schlick(1.0, 0.04) = 0.04
        // spec = (5.093 * 1.0 * 0.04) / 4.0 ≈ 0.051
        assert!(spec.x > 0.04 && spec.x < 0.07,
            "perpendicular specular for dielectric with roughness=0.5 should be ~0.051, got {}", spec.x);
    }
}
