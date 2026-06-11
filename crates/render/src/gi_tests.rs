use glam::Vec3;
use crate::sh::{ShL1, ShL2, ShIrradianceL1};

#[test]
fn gi_sh_eval_at_y_up_matches_coeff_sum() {
    let sh = ShL1 { c: [1.0, 2.0, 3.0, 4.0] };
    let n = Vec3::Y;
    let v = sh.eval(n);
    assert!((v - 3.0).abs() < 0.001, "SH eval at Y-up should equal c0 + c1");
}

#[test]
fn gi_sh_directional_light_is_brighter_along_axis() {
    let mut sh = ShIrradianceL1::default();
    let dir = Vec3::new(0.0, 1.0, 0.0);
    sh.project_directional(dir, Vec3::new(1.0, 0.9, 0.8));

    let lit = sh.eval(dir);
    let unlit = sh.eval(-dir);
    assert!(lit.x > unlit.x, "red channel should be brighter facing light");
    assert!(lit.y > unlit.y, "green channel should be brighter facing light");
    assert!(lit.z > unlit.z, "blue channel should be brighter facing light");
}

#[test]
fn gi_sh_ambient_adds_uniform_energy() {
    let mut sh = ShIrradianceL1::default();
    sh.project_ambient(Vec3::new(0.5, 0.5, 0.5));

    let v1 = sh.eval(Vec3::X);
    let v2 = sh.eval(Vec3::Y);
    let v3 = sh.eval(Vec3::Z);
    // Ambient is uniform, so all normals should give the same result.
    assert!((v1 - v2).length() < 0.001, "ambient-only SH should be uniform");
    assert!((v2 - v3).length() < 0.001, "ambient-only SH should be uniform");
}

#[test]
fn gi_sh_combined_directional_and_ambient() {
    let dir = Vec3::new(1.0, 0.0, 0.0);
    let color = Vec3::new(1.0, 0.8, 0.6);
    let ambient = Vec3::new(0.1, 0.1, 0.1);
    let sh = ShIrradianceL1::from_directional_and_ambient(dir, color, ambient);

    // Facing the light should be brighter than facing away.
    let lit = sh.eval(dir);
    let unlit = sh.eval(-dir);
    assert!(lit.x > unlit.x, "facing light should be brighter");
    assert!(lit.y > unlit.y, "facing light should be brighter");
    assert!(lit.z > unlit.z, "facing light should be brighter");

    // Lit side should be clearly above ambient-only energy.
    let ambient_only = ShIrradianceL1::from_directional_and_ambient(dir, Vec3::ZERO, ambient);
    let ambient_val = ambient_only.eval(dir);
    assert!(lit.x > ambient_val.x, "lit side should exceed ambient-only");
    assert!(lit.y > ambient_val.y, "lit side should exceed ambient-only");
    assert!(lit.z > ambient_val.z, "lit side should exceed ambient-only");
}

#[test]
fn gi_sh_l2_symmetry_about_z_axis() {
    let sh = ShL2 { c: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0] };
    let v_pos = sh.eval(Vec3::Z);
    let v_neg = sh.eval(-Vec3::Z);
    assert!((v_pos - v_neg).abs() < 0.001, "L00+L20 should be symmetric about Z");
}

#[test]
fn gi_sh_scale_affects_all_channels() {
    let mut sh = ShIrradianceL1::default();
    sh.project_directional(Vec3::Y, Vec3::new(1.0, 2.0, 3.0));
    sh.scale(0.5);

    let v = sh.eval(Vec3::Y);
    // After scaling by 0.5, the directional contribution should be halved.
    assert!((v.x - 0.5 * 1.0 * 0.2820948 - 0.5 * 1.0 * 0.4886025).abs() < 0.001,
        "scaled red SH should be half of original");
}
