//! Field-solver validation: convergence, energy-derived capacitance
//! cross-checked against the closed-form trace model, and pattern-shape
//! sanity against textbook curves.

use super::*;
use crate::trace;

fn default_opts() -> FieldOptions {
    FieldOptions {
        width_mm: 3.0,
        height_mm: 1.6,
        er: 4.6,
        box_width_mult: 20.0,
        box_top_mult: 10.0,
        grid_cols: 220,
        v_conductor: 1.0,
    }
}

#[test]
fn microstrip_field_converges_and_estimates_z0() {
    let result = solve_microstrip_field(&default_opts()).expect("valid options");
    assert!(result.converged, "SOR did not converge");
    // The energy-derived Z0 against the closed-form Hammerstad–Jensen
    // value: grids this size should land within ~4 % (single-row Dirichlet
    // strip + box walls both push the same direction).
    let closed_form = trace::solve(&trace::TraceInput::Microstrip(trace::MicrostripInput {
        width_mm: 3.0,
        height_mm: 1.6,
        er: 4.6,
        thickness_um: 0.0,
        loss_tangent: 0.0,
        freq_ghz: 0.0,
    }))
    .unwrap();
    let deviation = (result.z0_field_estimate - closed_form.z0) / closed_form.z0;
    assert!(
        deviation.abs() < 0.04,
        "field Z0 = {} vs closed form {} ({:.2} %)",
        result.z0_field_estimate,
        closed_form.z0,
        deviation * 100.0
    );
    // Capacitance in a plausible band for a 50 Ω-ish FR-4 microstrip:
    // C' = t_pd/Z0 ≈ 6.2 ps/mm ÷ 50 Ω ≈ 0.124 pF/mm.
    assert!(
        (0.05..=0.2).contains(&result.c_pf_per_mm),
        "C = {} pF/mm",
        result.c_pf_per_mm
    );
}

#[test]
fn field_potential_respects_boundary_conditions() {
    let result = solve_microstrip_field(&default_opts()).unwrap();
    let cols = result.cols;
    // Box walls and floor grounded; conductor at V.
    let last = cols - 1;
    assert!(result.potential[last].abs() < 1e-9); // top-right corner (row 0, last col)
    let floor = (result.rows - 1) * cols;
    assert!(result.potential[floor + cols / 2].abs() < 1e-9);
    // Symmetry about the centerline. The centerline falls between grid
    // nodes, so the trace footprint snaps to the grid with ~µm asymmetry —
    // tolerance reflects that discretization, not solver noise.
    let trace_row = result.rows - 1 - (result.height_mm / result.dy).round() as usize;
    for col in 1..cols / 2 {
        let a = result.potential[trace_row * cols + col];
        let b = result.potential[trace_row * cols + (cols - 1 - col)];
        assert!((a - b).abs() < 5e-5, "asymmetric at col {col}: {a} vs {b}");
    }
}

#[test]
fn dipole_pattern_has_classical_shape() {
    // Half-wave: kl/2 = π/2. Peak at broadside (90°), nulls at the axis.
    let p = radiation_pattern(PatternKind::Dipole, std::f64::consts::FRAC_PI_2);
    assert!((p.normalized[90] - 1.0).abs() < 1e-9);
    assert!(p.normalized[0] < 1e-6 && p.normalized[180] < 1e-6);
    // Textbook −3 dB beamwidth of a half-wave dipole: 78°.
    assert!((p.beamwidth_deg - 78.0).abs() < 2.0, "{}", p.beamwidth_deg);
    // 3 dB down near ±39° from broadside.
    let at_51 = p.normalized[51];
    assert!((at_51 - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.05, "{at_51}");
}

#[test]
fn monopole_pattern_is_half_space() {
    let p = radiation_pattern(PatternKind::Monopole, std::f64::consts::FRAC_PI_2);
    assert_eq!(p.angle_deg.len(), 91);
    assert_eq!(p.angle_deg[90], 90.0); // horizon is the last sample
    assert!((p.normalized[90] - 1.0).abs() < 1e-9);
}

#[test]
fn patch_cuts_are_broadside_peaked() {
    // Typical patch: kL/2 ≈ π·L/λ ≈ π·0.33, kW/2 ≈ π·0.31.
    let e = radiation_pattern(PatternKind::PatchEPlane, std::f64::consts::PI * 0.33);
    assert!((e.normalized[0] - 1.0).abs() < 1e-9);
    assert!(e.normalized[90] < 0.55, "E-plane horizon = {}", e.normalized[90]);
    let h = radiation_pattern(PatternKind::PatchHPlane, std::f64::consts::PI * 0.31);
    assert!((h.normalized[0] - 1.0).abs() < 1e-9);
    assert!(h.normalized[90] < 0.8, "H-plane horizon = {}", h.normalized[90]);
}

#[test]
fn field_options_validation() {
    let mut opts = default_opts();
    opts.er = 0.0;
    let err = solve_microstrip_field(&opts).expect_err("must fail");
    assert!(err.0[0].contains("Dielectric constant"));
    opts = default_opts();
    opts.grid_cols = 10;
    assert!(solve_microstrip_field(&opts).is_err());
    opts = default_opts();
    opts.box_width_mult = 2.0;
    assert!(solve_microstrip_field(&opts).is_err());
}
