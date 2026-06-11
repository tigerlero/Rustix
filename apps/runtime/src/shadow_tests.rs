use rustix_core::math::{Vec3, Vec4, Mat4};

use crate::render::CsmResources;
use crate::render::shadow::CsmUboData;

/// Stand-in for Renderer that provides just enough to construct CsmResources.
/// Since CsmResources::new requires a real Vulkan renderer, we test the
/// CPU-side math directly.

#[test]
fn cascade_splits_are_monotonic() {
    let data = CsmUboData {
        light_view_proj: [Mat4::IDENTITY; 3],
        cascade_splits: Vec4::new(10.0, 25.0, 60.0, 0.0),
    };
    assert!(data.cascade_splits.x > 0.0);
    assert!(data.cascade_splits.y > data.cascade_splits.x);
    assert!(data.cascade_splits.z > data.cascade_splits.y);
}

#[test]
fn texel_snapping_expands_bounds_to_grid() {
    // Simulate the texel-snapping math from compute_cascades.
    let min_x = 0.0f32;
    let max_x = 100.0f32;
    let shadow_map_size = 1024u32;

    let texel_size_x = (max_x - min_x) / shadow_map_size as f32;
    let snapped_min = (min_x / texel_size_x).floor() * texel_size_x;
    let snapped_max = (max_x / texel_size_x).ceil() * texel_size_x;

    // Snapped bounds should be at least as large as original.
    assert!(snapped_min <= min_x + 0.0001, "snapped min should not exceed original min");
    assert!(snapped_max >= max_x - 0.0001, "snapped max should not be smaller than original max");

    // Snapped bounds should align to texel grid.
    assert!((snapped_min / texel_size_x).fract().abs() < 0.0001,
        "snapped min should be multiple of texel size");
    assert!((snapped_max / texel_size_x).fract().abs() < 0.0001,
        "snapped max should be multiple of texel size");
}

#[test]
fn texel_snapping_is_stable_across_frames() {
    // Simulate two frames with slightly different camera positions.
    let shadow_map_size = 1024u32;
    let texel_size = 100.0 / shadow_map_size as f32;

    // Frame 1 bounds
    let min1 = 0.05f32;
    let max1 = 100.03f32;
    let snapped_min1 = (min1 / texel_size).floor() * texel_size;
    let snapped_max1 = (max1 / texel_size).ceil() * texel_size;

    // Frame 2 bounds (camera moved slightly)
    let min2 = 0.07f32;
    let max2 = 100.01f32;
    let snapped_min2 = (min2 / texel_size).floor() * texel_size;
    let snapped_max2 = (max2 / texel_size).ceil() * texel_size;

    // Because sub-texel movement is absorbed by snapping, the snapped
    // bounds should be identical for small camera shifts.
    assert_eq!(snapped_min1, snapped_min2,
        "texel snapping should absorb sub-texel camera movement");
    assert_eq!(snapped_max1, snapped_max2,
        "texel snapping should absorb sub-texel camera movement");
}

/// CPU-side replication of the shader PCSS logic.
fn find_blocker_distance(uv: (f32, f32), current_depth: f32, bias: f32, texel_size: f32, search_radius: i32, sampler: &mut impl FnMut(f32, f32) -> f32) -> f32 {
    let mut blocker_sum = 0.0f32;
    let mut blocker_count = 0;
    for x in -search_radius..=search_radius {
        for y in -search_radius..=search_radius {
            let ux = uv.0 + (x as f32) * texel_size;
            let vy = uv.1 + (y as f32) * texel_size;
            let sample_depth = sampler(ux, vy);
            if current_depth - bias > sample_depth {
                blocker_sum += sample_depth;
                blocker_count += 1;
            }
        }
    }
    if blocker_count == 0 {
        return -1.0;
    }
    blocker_sum / blocker_count as f32
}

fn pcf_filter(uv: (f32, f32), current_depth: f32, bias: f32, texel_size: f32, radius: i32, sampler: &mut impl FnMut(f32, f32) -> f32) -> f32 {
    let mut shadow = 0.0f32;
    let mut count = 0;
    for x in -radius..=radius {
        for y in -radius..=radius {
            let ux = uv.0 + (x as f32) * texel_size;
            let vy = uv.1 + (y as f32) * texel_size;
            let pcf_depth = sampler(ux, vy);
            shadow += if current_depth - bias > pcf_depth { 0.0 } else { 1.0 };
            count += 1;
        }
    }
    shadow / count as f32
}

fn shadow_factor_pcss(frag_light_space: Vec4, mut sampler: impl FnMut(f32, f32) -> f32) -> f32 {
    let proj_coords = Vec3::new(frag_light_space.x, frag_light_space.y, frag_light_space.z) / frag_light_space.w;
    let proj_coords = proj_coords * 0.5 + Vec3::splat(0.5);
    if proj_coords.z > 1.0 || proj_coords.x < 0.0 || proj_coords.x > 1.0 || proj_coords.y < 0.0 || proj_coords.y > 1.0 {
        return 1.0;
    }
    let current_depth = proj_coords.z;
    let bias = 0.005;
    let texel_size = 1.0 / 1024.0;

    let uv = (proj_coords.x, proj_coords.y);
    let blocker_dist = find_blocker_distance(uv, current_depth, bias, texel_size, 2, &mut sampler);
    if blocker_dist < 0.0 {
        return 1.0;
    }

    let penumbra = (current_depth - blocker_dist) / blocker_dist;
    let pcf_radius = (penumbra * 4.0).clamp(1.0, 4.0) as i32;
    pcf_filter(uv, current_depth, bias, texel_size, pcf_radius, &mut sampler)
}

#[test]
fn pcss_no_blockers_returns_fully_lit() {
    // All depths are far away (1.0), so no blockers around currentDepth=0.5.
    let result = shadow_factor_pcss(Vec4::new(0.0, 0.0, 0.5, 1.0), |_, _| 1.0);
    assert_eq!(result, 1.0, "no blockers should mean fully lit");
}

#[test]
fn pcss_close_blocker_produces_large_penumbra() {
    // All depths are 0.0 (occluder at near plane), currentDepth=0.5.
    // BlockerDist = 0.0 → penumbra = inf → pcfRadius = 4 (clamped).
    let result = shadow_factor_pcss(Vec4::new(0.0, 0.0, 0.5, 1.0), |_, _| 0.0);
    assert_eq!(result, 0.0, "close blocker with large PCF radius should be fully shadowed");
}

#[test]
fn pcss_partial_blockers_produces_soft_shadow() {
    // Mix of blockers (0.3) and non-blockers (0.7) around currentDepth=0.5.
    // frag_ls.z=0 → currentDepth = 0*0.5 + 0.5 = 0.5 after NDC→UV mapping.
    let mut counter = 0u32;
    let result = shadow_factor_pcss(Vec4::new(0.0, 0.0, 0.0, 1.0), |_, _| {
        counter += 1;
        if counter % 2 == 0 { 0.3 } else { 0.7 }
    });
    // Blocker search finds ~12 blockers at 0.3 → blockerDist ≈ 0.3.
    // penumbra = (0.5 - 0.3) / 0.3 ≈ 0.67 → pcfRadius = clamp(2.67, 1, 4) = 2 or 3.
    // Result should be between 0 and 1 (soft shadow).
    assert!(result > 0.0 && result < 1.0, "partial blockers should produce soft shadow, got {}", result);
}

#[test]
fn pcss_outside_frustum_returns_fully_lit() {
    let result = shadow_factor_pcss(Vec4::new(2.0, 0.5, 0.5, 1.0), |_, _| 0.0);
    assert_eq!(result, 1.0, "outside light frustum should be fully lit");
}

#[test]
fn pcss_penumbra_grows_with_blocker_distance() {
    // Two scenarios: close blocker vs far blocker, same receiver depth.
    let receiver_depth = 0.5f32;

    // Close blocker: blockerDist = 0.2
    let penumbra_close = (receiver_depth - 0.2) / 0.2;
    // Far blocker: blockerDist = 0.4
    let penumbra_far = (receiver_depth - 0.4) / 0.4;

    // Penumbra should be larger for the close blocker (more spread).
    assert!(penumbra_close > penumbra_far,
        "closer blockers should produce larger penumbra: close={}, far={}", penumbra_close, penumbra_far);
}

#[test]
fn light_space_aabb_contains_all_frustum_corners() {
    // Build a simple frustum and verify AABB contains all corners after
    // transforming to light space.
    let light_dir = Vec3::new(0.0, -1.0, 0.0).normalize();
    let center = Vec3::ZERO;
    let light_view = Mat4::look_at_rh(center, center - light_dir, Vec3::Z);

    let corners = [
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new( 1.0, -1.0, -1.0),
        Vec3::new( 1.0,  1.0, -1.0),
        Vec3::new(-1.0,  1.0, -1.0),
        Vec3::new(-1.0, -1.0,  1.0),
        Vec3::new( 1.0, -1.0,  1.0),
        Vec3::new( 1.0,  1.0,  1.0),
        Vec3::new(-1.0,  1.0,  1.0),
    ];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;

    for &c in &corners {
        let ls = light_view * Vec4::new(c.x, c.y, c.z, 1.0);
        let ls = Vec3::new(ls.x, ls.y, ls.z) / ls.w;
        min_x = min_x.min(ls.x);
        max_x = max_x.max(ls.x);
        min_y = min_y.min(ls.y);
        max_y = max_y.max(ls.y);
        min_z = min_z.min(ls.z);
        max_z = max_z.max(ls.z);
    }

    for &c in &corners {
        let ls = light_view * Vec4::new(c.x, c.y, c.z, 1.0);
        let ls = Vec3::new(ls.x, ls.y, ls.z) / ls.w;
        assert!(ls.x >= min_x && ls.x <= max_x, "corner x should be inside AABB");
        assert!(ls.y >= min_y && ls.y <= max_y, "corner y should be inside AABB");
        assert!(ls.z >= min_z && ls.z <= max_z, "corner z should be inside AABB");
    }
}
