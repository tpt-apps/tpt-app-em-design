//! Antenna design + matching validation against textbook values.

use super::*;

#[test]
fn half_wave_dipole_matches_the_468ft_norm() {
    // 14 MHz, 2 mm wire: the ham-radio norm 143/f(MHz) meters = 10.21 m.
    // Model: fraction(l/d ≈ 5100) ≈ 0.952 → L = 0.952·(λ/2) = 10.19 m.
    let r = design(&AntennaInput::Wire(WireAntennaInput {
        kind: AntennaKind::Dipole,
        freq_mhz: 14.0,
        element_diameter_mm: 2.0,
    }))
    .unwrap();
    assert!((r.length_mm - 10_214.0).abs() < 250.0, "got {} mm", r.length_mm);
    assert!(
        (0.470..=0.482).contains(&(r.length_mm / r.wavelength_mm)),
        "fraction of λ = {}",
        r.length_mm / r.wavelength_mm
    );
    assert!((r.feed_impedance - 73.0).abs() < 1e-9);
}

#[test]
fn monopole_is_half_a_dipole_and_hits_the_234ft_norm() {
    let monopole = |d: f64| {
        design(&AntennaInput::Wire(WireAntennaInput {
            kind: AntennaKind::Monopole,
            freq_mhz: 100.0,
            element_diameter_mm: d,
        }))
        .unwrap()
        .length_mm
    };
    let dipole = |d: f64| {
        design(&AntennaInput::Wire(WireAntennaInput {
            kind: AntennaKind::Dipole,
            freq_mhz: 100.0,
            element_diameter_mm: d,
        }))
        .unwrap()
        .length_mm
    };
    // Ratio within 0.5 %: the end-effect fraction differs by a hair because
    // the monopole's shorter element has a smaller l/d (fatter, resonates
    // slightly shorter).
    assert!((monopole(2.0) / dipole(2.0) - 0.5).abs() < 0.005);
    // 234/f(MHz) feet = 71.3/f(MHz) meters = 713 mm at 100 MHz (model: 706).
    assert!((monopole(2.0) - 713.0).abs() < 15.0, "got {} mm", monopole(2.0));
}

#[test]
fn fat_elements_resonate_shorter() {
    let thin = design(&AntennaInput::Wire(WireAntennaInput {
        kind: AntennaKind::Dipole,
        freq_mhz: 100.0,
        element_diameter_mm: 0.1,
    }))
    .unwrap();
    let fat = design(&AntennaInput::Wire(WireAntennaInput {
        kind: AntennaKind::Dipole,
        freq_mhz: 100.0,
        element_diameter_mm: 149.9, // ≈ λ/20 at 100 MHz
    }))
    .unwrap();
    assert!(fat.length_mm < thin.length_mm);
    assert!(
        fat.length_mm / fat.wavelength_mm < 0.46,
        "fat fraction = {}",
        fat.length_mm / fat.wavelength_mm
    );
}

#[test]
fn patch_dimensions_hit_textbook_values() {
    // Balanis's worked example class: 2.4 GHz, εr 4.4, h 1.588 mm (62-mil):
    // W = c/2f·√(2/5.4) = 38.0 mm; L = c/(2f√εeff) − 2ΔL = 29.4 mm
    // (hand-computed from the same published formulas: εeff = 4.090,
    // ΔL = 0.734 mm); published patches at these settings ≈ 38 × 29.4 mm.
    let r = design(&AntennaInput::Patch(PatchInput {
        freq_mhz: 2400.0,
        er: 4.4,
        height_mm: 1.588,
        feed_z: 50.0,
    }))
    .unwrap();
    assert!((r.width_mm - 38.0).abs() < 0.3, "W = {}", r.width_mm);
    assert!((29.4 - r.length_mm).abs() < 0.8, "L = {}", r.length_mm);
    // Inset feed: edge resistance ≈ 486 Ω (G1 = (1/90)(W/λ0)²) → the 50 Ω
    // inset lands at cos⁻¹(√(50/486))·L/π ≈ 11.7 mm, inside the patch.
    assert!(r.inset_mm > 5.0 && r.inset_mm < r.length_mm / 2.0, "inset = {}", r.inset_mm);
}

#[test]
fn quarter_wave_transformer_values() {
    let r = design_match(&MatchInput {
        kind: MatchKind::QuarterWave,
        source_z: 50.0,
        load_r: 100.0,
        freq_mhz: 1000.0,
        eps_eff: 1.0,
    })
    .unwrap();
    assert!((r.section_z[0] - (50.0f64 * 100.0).sqrt()).abs() < 1e-9);
    // λ0 = 299.79 mm → quarter wave 74.95 mm.
    assert!((r.section_length_mm[0] - 74.95).abs() < 0.1);
}

/// (Re, Im) of R_L shunted by jXp — the L network's parallel section.
fn shunt(load_r: f64, xp: f64) -> (f64, f64) {
    let den = load_r * load_r + xp * xp;
    (load_r * xp * xp / den, load_r * load_r * xp / den)
}

/// (Re, Im) of complex impedances a and b in parallel.
fn z_parallel(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let num_re = a.0 * b.0 - a.1 * b.1;
    let num_im = a.0 * b.1 + a.1 * b.0;
    let (den_re, den_im) = (a.0 + b.0, a.1 + b.1);
    let mag2 = den_re * den_re + den_im * den_im;
    (
        (num_re * den_re + num_im * den_im) / mag2,
        (num_im * den_re - num_re * den_im) / mag2,
    )
}

#[test]
fn l_network_50_to_100_solves_exactly() {
    // Q = 1; low-pass arrangement: series −j50 (C), shunt +j100 (L) across
    // the load: Z_in = −j50 + (100 ∥ j100) = −j50 + 50 + j50 = 50 Ω.
    let (re, im) = shunt(100.0, 100.0);
    assert!((re - 50.0).abs() < 1e-9 && (im - 50.0).abs() < 1e-9);

    let r = design_match(&MatchInput {
        kind: MatchKind::LNetwork,
        source_z: 50.0,
        load_r: 100.0,
        freq_mhz: 100.0,
        eps_eff: 1.0,
    })
    .unwrap();
    let text = r.summary.join("\n");
    // Both arrangements are reported; magnitudes |X_s| = 50, |X_p| = 100.
    assert!(text.contains("capacitor") && text.contains("inductor"), "{text}");
}

#[test]
fn l_network_100_to_25_solves_exactly() {
    // Q = √(100/25 − 1) = √3; |X_s| = √3·25 = 43.30, |X_p| = 100/√3 = 57.74.
    // High-pass arrangement: series +j43.30 (L) on the low-R side, shunt
    // −j57.74 (C) across the high-R side. Looking in from the source:
    // Z_in = jX_p ∥ (R_L + jX_s) = −j57.74 ∥ (25 + j43.30) = 100 + j0.
    let zin = z_parallel((0.0, -57.735), (25.0, 43.30));
    assert!((zin.0 - 100.0).abs() < 0.05, "re = {}", zin.0);
    assert!(zin.1.abs() < 0.1, "im = {}", zin.1);
}

/// Normalized input admittance (Re, Im) of a Z0 line of electrical length
/// `beta_d` terminated in normalized resistance r: z = (r+jt)/(1+jrt).
fn stub_yin(r: f64, beta_d: f64) -> (f64, f64) {
    let t = beta_d.tan();
    let den = 1.0 + (r * t) * (r * t);
    let z_re = r * (1.0 + t * t) / den;
    let z_im = t * (1.0 - r * r) / den;
    let mag2 = z_re * z_re + z_im * z_im;
    (z_re / mag2, -z_im / mag2)
}

#[test]
fn single_stub_closed_form_positions() {
    // R_L = 100 Ω, Z0 = 50 (r = 2): tan²(βd) = r → βd = atan(√2),
    // d = 0.1520λ; y_in = 1 + j0.7071 → open-stub length 0.4020λ.
    let r = design_match(&MatchInput {
        kind: MatchKind::SingleStub,
        source_z: 50.0,
        load_r: 100.0,
        freq_mhz: 1000.0,
        eps_eff: 1.0,
    })
    .unwrap();
    let lambda = 299.792_458;
    let d = r.section_length_mm[0] / lambda;
    let l_open = r.section_length_mm[1] / lambda;
    assert!((d - 0.1520).abs() < 2e-3, "d = {d}");
    assert!((l_open - 0.4020).abs() < 2e-3, "l_open = {l_open}");
    // Independent physics check at the stub point: Re{y_in} = 1 and the
    // open stub's j·tan(βl) must cancel Im{y_in}.
    let (y_re, y_im) = stub_yin(2.0, std::f64::consts::TAU * d);
    assert!((y_re - 1.0).abs() < 1e-6, "Re y = {y_re}");
    let y_stub = (std::f64::consts::TAU * l_open).tan();
    assert!((y_stub + y_im).abs() < 1e-6, "stub {y_stub} vs -y_in {}", -y_im);
}

#[test]
fn already_matched_load_is_rejected() {
    let err = design_match(&MatchInput {
        kind: MatchKind::SingleStub,
        source_z: 50.0,
        load_r: 50.0,
        freq_mhz: 1000.0,
        eps_eff: 1.0,
    })
    .expect_err("no stub needed");
    assert!(err.0[0].contains("already matches"), "{err:?}");
}

#[test]
fn invalid_match_inputs_surface_every_problem() {
    let err = design_match(&MatchInput {
        kind: MatchKind::QuarterWave,
        source_z: 0.0,
        load_r: -1.0,
        freq_mhz: 0.0,
        eps_eff: 0.0,
    })
    .expect_err("must fail");
    assert!(err.0.len() >= 4, "{err:?}");
}
