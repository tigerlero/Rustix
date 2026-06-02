use glam::{Vec3, Vec4, Vec4Swizzles};

/// Replicates the fragment shader's `blinn_phong` + `ambient + shadow * lit` logic.
fn blinn_phong(
    n: Vec3, l: Vec3, v: Vec3, light_color: Vec3, base: Vec3, roughness: f32, metallic: f32,
) -> Vec3 {
    let h = (l + v).normalize();
    let ndotl = n.dot(l).max(0.0);
    let ndoth = n.dot(h).max(0.0);
    let spec_pow = 32.0 / (roughness * roughness + 0.001);
    let spec = ndoth.powf(spec_pow);

    let f0 = Vec3::splat(0.04).lerp(base, metallic);
    let specular = spec * light_color * f0 * (1.0 - roughness) * 0.5;
    let diffuse = ndotl * light_color * base * (1.0 - metallic);

    diffuse + specular
}

fn shade(base: Vec3, shadow: f32, n: Vec3, l: Vec3, v: Vec3, light_color: Vec3, roughness: f32, metallic: f32) -> Vec4 {
    let lit = blinn_phong(n, l, v, light_color, base, roughness, metallic);
    let ambient = base * 0.1;
    let color = ambient + shadow * lit;
    Vec4::new(color.x, color.y, color.z, 1.0)
}

#[test]
fn fragment_ambient_lit_blending() {
    let n = Vec3::new(0.0, 0.0, 1.0);
    let l = Vec3::new(0.0, 0.0, 1.0);
    let v = Vec3::new(0.0, 0.0, 1.0);
    let light_color = Vec3::new(1.0, 1.0, 1.0);
    let base = Vec3::new(0.5, 0.5, 0.5);
    let roughness = 0.5;
    let metallic = 0.0;

    // When shadow = 0.0, only ambient remains.
    let unlit = shade(base, 0.0, n, l, v, light_color, roughness, metallic);
    let ambient = base * 0.1;
    assert!((unlit.x - ambient.x).abs() < 0.001, "unlit R should be ambient");
    assert!((unlit.y - ambient.y).abs() < 0.001, "unlit G should be ambient");
    assert!((unlit.z - ambient.z).abs() < 0.001, "unlit B should be ambient");

    // When shadow = 1.0, result is ambient + lit.
    let lit = shade(base, 1.0, n, l, v, light_color, roughness, metallic);
    let expected_lit = blinn_phong(n, l, v, light_color, base, roughness, metallic);
    let expected = ambient + expected_lit;
    assert!((lit.x - expected.x).abs() < 0.001, "lit R should be ambient + lit");
    assert!((lit.y - expected.y).abs() < 0.001, "lit G should be ambient + lit");
    assert!((lit.z - expected.z).abs() < 0.001, "lit B should be ambient + lit");

    // Sanity: lit color should be noticeably brighter than ambient-only.
    assert!(lit.x > unlit.x * 2.0, "lit color should be much brighter than ambient-only");
}

#[test]
fn pcf_partial_shadow_values() {
    let n = Vec3::new(0.0, 0.0, 1.0);
    let l = Vec3::new(0.0, 0.0, 1.0);
    let v = Vec3::new(0.0, 0.0, 1.0);
    let light_color = Vec3::new(1.0, 1.0, 1.0);
    let base = Vec3::new(0.5, 0.5, 0.5);
    let roughness = 0.5;
    let metallic = 0.0;

    let ambient = base * 0.1;
    let full_lit = blinn_phong(n, l, v, light_color, base, roughness, metallic);

    // With PCF, shadow values can be partial (e.g. 4/9, 6/9).
    for shadow in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let color = shade(base, shadow, n, l, v, light_color, roughness, metallic);
        let expected = ambient + shadow * full_lit;
        assert!(
            (color.x - expected.x).abs() < 0.001,
            "shadow={} should produce ambient + {} * lit", shadow, shadow
        );
    }
}

/// Replicates the shadow comparison `currentDepth - bias > pcfDepth` from the shader.
/// Returns true if the sample is in shadow (pcfDepth < currentDepth - bias).
fn shadowed(current_depth: f32, pcf_depth: f32, bias: f32) -> bool {
    current_depth - bias > pcf_depth
}

#[test]
fn bias_prevents_self_shadowing() {
    let bias = 0.005;

    // A surface point whose stored depth exactly matches the current fragment depth.
    let current_depth = 0.5;
    let pcf_depth = 0.5;

    // Without bias, floating-point imprecision could shadow it.
    // With bias: 0.5 - 0.005 = 0.495 > 0.5 is FALSE → NOT shadowed.
    assert!(!shadowed(current_depth, pcf_depth, bias),
        "surface should not shadow itself when depth matches exactly");

    // Even if the stored depth is slightly larger (fragment is slightly closer to light),
    // the bias should still prevent self-shadowing for small epsilon.
    assert!(!shadowed(current_depth, pcf_depth + 0.001, bias),
        "surface should not shadow itself for small epsilon differences");

    // But a point clearly behind the occluder should still be shadowed.
    assert!(shadowed(current_depth, pcf_depth - 0.01, bias),
        "a point clearly behind the occluder should still be shadowed");
}

#[test]
fn bias_does_not_bleed_light_into_shadow() {
    let bias = 0.005;

    // A point well behind the occluder (stored depth is much smaller).
    // currentDepth = 0.8, pcfDepth = 0.5 → 0.8 - 0.005 = 0.795 > 0.5 → shadowed.
    assert!(shadowed(0.8, 0.5, bias),
        "a point well behind an occluder should remain shadowed despite bias");
}

/// Replicates the full `shadowFactor` GLSL function CPU-side.
/// `sampler` is a mock that takes (x, y) UV coords and returns depth.
fn shadow_factor(frag_light_space: Vec4, mut sampler: impl FnMut(f32, f32) -> f32) -> f32 {
    let mut proj_coords = frag_light_space.xyz() / frag_light_space.w;
    proj_coords = proj_coords * 0.5 + Vec3::splat(0.5);
    if proj_coords.z > 1.0 || proj_coords.x < 0.0 || proj_coords.x > 1.0 || proj_coords.y < 0.0 || proj_coords.y > 1.0 {
        return 1.0;
    }
    let current_depth = proj_coords.z;
    let bias = 0.005;
    let texel_size = 1.0 / 1024.0;
    let mut shadow = 0.0f32;
    for x in -1..=1 {
        for y in -1..=1 {
            let ux = proj_coords.x + (x as f32) * texel_size;
            let vy = proj_coords.y + (y as f32) * texel_size;
            let pcf_depth = sampler(ux, vy);
            shadow += if current_depth - bias > pcf_depth { 0.0 } else { 1.0 };
        }
    }
    shadow / 9.0
}

#[test]
fn shadow_factor_fully_lit_when_outside_light_frustum() {
    // fragLightSpace.w = 1.0, xyz = (2.0, 0.5, 0.5) → projCoords.x = 2.0 > 1.0
    let result = shadow_factor(Vec4::new(2.0, 0.5, 0.5, 1.0), |_, _| 0.0);
    assert_eq!(result, 1.0, "outside light frustum should be fully lit");
}

#[test]
fn shadow_factor_fully_lit_when_occluder_depth_is_one() {
    // All 9 PCF samples return depth=1.0 (no occluder, or far plane).
    // currentDepth=0.5, bias=0.005 → 0.495 > 1.0 is false for all samples → all lit.
    let result = shadow_factor(Vec4::new(0.0, 0.0, 0.5, 1.0), |_, _| 1.0);
    assert_eq!(result, 1.0, "when all depths are 1.0, everything should be fully lit");
}

#[test]
fn shadow_factor_fully_shadowed_when_occluder_is_close() {
    // All 9 PCF samples return depth=0.0 (occluder right at near plane).
    // currentDepth=0.5, bias=0.005 → 0.495 > 0.0 is true for all samples → all shadowed.
    let result = shadow_factor(Vec4::new(0.0, 0.0, 0.5, 1.0), |_, _| 0.0);
    assert_eq!(result, 0.0, "when all depths are 0.0, everything should be fully shadowed");
}

#[test]
fn shadow_factor_partial_pcf_value() {
    // Use fragLightSpace with z=0 so currentDepth=0.5 after NDC→UV mapping.
    // Mock: every other sample returns depth 0.4 (shadowed) vs 0.6 (lit).
    // currentDepth=0.5, bias=0.005 → threshold = 0.495.
    // pcfDepth 0.4 → 0.495 > 0.4 → shadowed (0.0)
    // pcfDepth 0.6 → 0.495 > 0.6 → lit (1.0)
    let mut counter = 0u32;
    let result = shadow_factor(Vec4::new(0.0, 0.0, 0.0, 1.0), |_, _| {
        counter += 1;
        if counter % 2 == 0 { 0.4 } else { 0.6 }
    });
    // With 9 samples and alternating, result should be 5/9 (5 lit, 4 shadowed).
    assert!((result - 5.0/9.0).abs() < 0.001,
        "partial shadow should be 5/9, got {}", result);
}

#[test]
fn shadow_factor_ndc_uv_mapping() {
    // NDC (-1, -1, 0) → UV (0, 0, 0.5)
    let frag_ls = Vec4::new(-1.0, -1.0, 0.0, 1.0);
    let mut sampled_uv = None;
    shadow_factor(frag_ls, |x, y| {
        sampled_uv = Some((x, y));
        1.0 // fully lit so result doesn't matter
    });
    let (ux, vy) = sampled_uv.unwrap();
    assert!((ux - 0.0).abs() < 0.001, "NDC x=-1 should map to UV u≈0, got {}", ux);
    assert!((vy - 0.0).abs() < 0.001, "NDC y=-1 should map to UV v≈0, got {}", vy);
}
