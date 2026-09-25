//! Validation suite — closed-form reference cases and model identities.
//!
//! Reference values: (1) analytical limits (air lines, monotonicity),
//! (2) values computed independently from the published formulas (shown
//! in each test's comment with its derivation constants), and (3) the
//! known engineering anchors (the 468/f dipole norm, ~50 Ω FR-4 trace
//! widths). Tolerances are the documented accuracy bounds, deliberately
//! not tighter than the models themselves.

use super::*;

fn microstrip(width_mm: f64, height_mm: f64, er: f64, thickness_um: f64) -> TraceResult {
    solve(&TraceInput::Microstrip(MicrostripInput {
        width_mm,
        height_mm,
        er,
        thickness_um,
        loss_tangent: 0.0,
        freq_ghz: 0.0,
    }))
    .expect("valid microstrip input")
}

fn stripline(width_mm: f64, spacing_mm: f64, er: f64, thickness_um: f64) -> TraceResult {
    solve(&TraceInput::Stripline(StriplineInput {
        width_mm,
        spacing_mm,
        er,
        thickness_um,
        loss_tangent: 0.0,
        freq_ghz: 0.0,
    }))
    .expect("valid stripline input")
}

#[test]
fn agm_matches_known_elliptic_integral() {
    // K(k = 1/√2) = Γ(1/4)²/(4√π) = 1.8540746773013719 (known closed form).
    let k = std::f64::consts::FRAC_1_SQRT_2;
    let k_prime = (1.0 - k * k).sqrt();
    let big_k = std::f64::consts::PI / (2.0 * agm(1.0, k_prime));
    assert!(
        (big_k - 1.854_074_677_301_371_9).abs() < 1e-12,
        "AGM elliptic integral wrong: {big_k}"
    );
    // AGM(1, √2) = 1.1981402347355922 (Gauss's classic value).
    assert!((agm(1.0, 2.0f64.sqrt()) - 1.198_140_234_735_592_2).abs() < 1e-12);
}

#[test]
fn microstrip_air_line_matches_wheeler_limit() {
    // εr = 1, w/h = 1: HJ evaluates to 126.42 Ω
    // (F = 6 + (2π−6)·e^−(30.666)^0.7528 ≈ 6.000, ln(F + √5) = 2.10872,
    // Z0 = η0/2π·2.10872 = 126.42). Published air-line tables: 126.5 Ω.
    let r = microstrip(1.0, 1.0, 1.0, 0.0);
    assert!((r.z0 - 126.42).abs() < 0.3, "got {}", r.z0);
}

#[test]
fn microstrip_fr4_reference_point() {
    // εr = 4.4, w/h = 2, t = 0, computed step-by-step from HJ-80:
    // Z0_air = 59.952·ln(6.0001/2 + √2) = 89.02 Ω, εeff = 3.3433,
    // Z0 = 89.02/√3.3433 = 48.68 Ω. Cross-checked against the published
    // Hammerstad curves (~48.5–49 Ω) and common online calculators.
    let r = microstrip(2.0, 1.0, 4.4, 0.0);
    assert!((r.z0 - 48.68).abs() < 0.6, "got {}", r.z0);
    assert!((r.epsilon_eff - 3.3433).abs() < 0.01, "got {}", r.epsilon_eff);
}

#[test]
fn microstrip_engineering_anchor_50ohm_fr4() {
    // The well-known rule: 50 Ω on FR-4 (εr 4.6) needs w/h ≈ 1.8 ± 0.1.
    // Values cross-checked against an independent Python evaluation of the
    // same published formulas: 49.92 / 49.02 / 49.28 Ω.
    for (w, h) in [(1.8, 1.0), (3.0, 1.6), (0.9, 0.5)] {
        let r = microstrip(w, h, 4.6, 34.8);
        assert!((r.z0 - 50.0).abs() < 2.5, "w/h = {w}/{h} gave {} Ω", r.z0);
    }
}

#[test]
fn microstrip_thickness_lowens_impedance() {
    // 0.16 mm core with 3 oz copper is an extreme t/h = 0.65; the published
    // Garg–Bahl correction gives ~12 % there (49.62 → 43.56 Ω, independently
    // evaluated). Typical boards sit under 2 %.
    let thin = microstrip(0.3, 0.16, 4.6, 0.0);
    let thick = microstrip(0.3, 0.16, 4.6, 104.4);
    assert!(thick.z0 < thin.z0);
    assert!(thick.effective_width_mm > thin.effective_width_mm);
    let drop = (thin.z0 - thick.z0) / thin.z0;
    assert!(drop < 0.13, "drop {drop}");
    assert!(drop > 0.08, "drop {drop}");
}

#[test]
fn microstrip_readouts_are_self_consistent() {
    let r = microstrip(3.0, 1.6, 4.6, 34.8);
    // Z0² = L'/C' (L nH/mm, C pF/mm → factor 1e3).
    assert!((r.l_nh_per_mm / r.c_pf_per_mm * 1e3 - r.z0.powi(2)).abs() < 1e-6);
    // L'·C' = t_pd² (nH·pF = ps²·1e-3).
    assert!((r.l_nh_per_mm * r.c_pf_per_mm - r.delay_ps_per_mm.powi(2) * 1e-3).abs() < 1e-9);
    // Delay equals √εeff/c.
    let expected_delay = r.epsilon_eff.sqrt() / C0 * 1e9;
    assert!((r.delay_ps_per_mm - expected_delay).abs() < 1e-9);
}

#[test]
fn stripline_conformal_mapping_reference_points() {
    // Zero-thickness symmetric stripline, exact result
    // Z0 = 30π/√εr·K(k)/K(k′), k = sech(πw/2b). Both values independently
    // evaluated (Python AGM, double precision): 49.544 and 65.399 Ω.
    let r = stripline(0.46, 1.0, 4.5, 0.0);
    assert!((r.z0 - 49.544).abs() < 0.1, "got {}", r.z0);
    let air = stripline(1.0, 1.0, 1.0, 0.0);
    assert!((air.z0 - 65.399).abs() < 0.1, "got {}", air.z0);
}

#[test]
fn stripline_scales_as_inverse_sqrt_er() {
    let a = stripline(0.35, 1.2, 2.0, 0.0);
    let b = stripline(0.35, 1.2, 8.0, 0.0);
    assert!((a.z0 / b.z0 - (8.0f64 / 2.0).sqrt()).abs() < 1e-9);
    // Homogeneous fill: εeff is exactly εr.
    assert!((a.epsilon_eff - 2.0).abs() < 1e-12);
}

#[test]
fn stripline_geometry_scales_linearly() {
    // Z0 depends only on aspect ratios — doubling every dimension at fixed
    // ratios must give the same impedance.
    let a = stripline(0.25, 0.8, 4.6, 17.4);
    let b = stripline(0.5, 1.6, 4.6, 34.8);
    assert!((a.z0 - b.z0).abs() / a.z0 < 2e-3);
}

#[test]
fn stripline_thickness_lowens_impedance() {
    let thin = stripline(0.25, 0.8, 4.6, 0.0);
    let thick = stripline(0.25, 0.8, 4.6, 104.4);
    assert!(thick.z0 < thin.z0);
}

#[test]
fn loss_estimate_is_positive_and_plausible() {
    // 1 GHz on FR-4 50 Ω: published total loss ≈ 0.03–0.1 dB/cm →
    // 0.003–0.01 dB/mm for this first-order estimate.
    let m = solve(&TraceInput::Microstrip(MicrostripInput {
        width_mm: 3.0,
        height_mm: 1.6,
        er: 4.6,
        thickness_um: 34.8,
        loss_tangent: 0.02,
        freq_ghz: 1.0,
    }))
    .unwrap();
    assert!(
        m.loss_db_per_mm > 0.001 && m.loss_db_per_mm < 0.02,
        "{}",
        m.loss_db_per_mm
    );
}

#[test]
fn quasi_static_validity_note_fires_for_electrically_thick_boards() {
    let r = solve(&TraceInput::Microstrip(MicrostripInput {
        width_mm: 3.0,
        height_mm: 20.0, // λ/10 at 1.6 GHz-ish scale
        er: 4.6,
        thickness_um: 34.8,
        loss_tangent: 0.0,
        freq_ghz: 10.0,
    }))
    .unwrap();
    assert!(r.notes.iter().any(|n| n.contains("full-wave")));
}

#[test]
fn invalid_inputs_surface_every_problem() {
    let err = solve(&TraceInput::Microstrip(MicrostripInput {
        width_mm: -1.0,
        height_mm: 0.0,
        er: 0.5,
        thickness_um: -3.0,
        loss_tangent: -0.1,
        freq_ghz: -1.0,
    }))
    .expect_err("must fail");
    assert!(err.0.len() >= 5, "{err:?}");
}

#[test]
fn copper_weight_table() {
    assert!((copper_thickness(0) - 17.4).abs() < 1e-9);
    assert!((copper_thickness(1) - 34.8).abs() < 1e-9);
    assert!((copper_thickness(2) - 69.6).abs() < 1e-9);
    assert_eq!(copper_thickness(99), 34.8); // fallback default
}
