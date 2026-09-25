//! PCB trace characteristic impedance — microstrip and symmetric stripline.
//!
//! Formulas (all quasi-static, accurate within their stated bounds for
//! h > 0 and frequencies where line dimensions ≪ λ):
//!
//! **Microstrip** — Hammerstad & Jensen (1980) for both Z0 and εeff
//! (typically within ~1% of full-wave results for 0.01 ≤ w/h ≤ 100,
//! εr ≤ 128), with the Garg–Bahl finite-metal-thickness correction
//! (effective width, valid for t ≪ h):
//!
//! ```text
//! Z0 = η0/(2π) · ln( F/u + √(1 + (2/u)²) ),  F(u) = 6 + (2π−6)·exp(−(30.666/u)^0.7528)
//! εeff = (εr+1)/2 + (εr−1)/2 · (1 + 10/u)^(−a(u)·b(εr))
//! ```
//!
//! **Stripline** — the exact zero-thickness conformal-mapping result for a
//! centered strip between planes spaced b,
//!
//! ```text
//! Z0 = (30π/√εr) · K(k)/K(k′),  k = sech(πw/2b), k′ = √(1−k²)
//! ```
//!
//! evaluated at an effective width w + Δw (first-order thickness
//! correction of the same Garg–Bahl form used for microstrip), K computed
//! by the arithmetic-geometric mean (exact to double precision). Published
//! calculator cross-checks agree within ~3%.

use std::fmt;

/// Speed of light in vacuum, m/s.
pub const C0: f64 = 299_792_458.0;
/// Free-space wave impedance, Ω (η0 = √(μ0/ε0)).
pub const ETA0: f64 = 376.730_313_668;

/// Copper clad thickness by common plating weights (1 oz ≈ 34.8 µm).
pub const COPPER_WEIGHTS_UM: [(&str, f64); 4] = [
    ("0.5 oz", 17.4),
    ("1 oz", 34.8),
    ("2 oz", 69.6),
    ("3 oz", 104.4),
];

/// Which trace geometry to solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    /// Trace on a surface over a single reference plane.
    Microstrip,
    /// Trace centered between two reference planes (plane spacing `b`).
    Stripline,
}

/// A single combined error type so the wasm layer can surface every
/// validation problem at once.
#[derive(Debug, Clone)]
pub struct TraceError(pub Vec<String>);

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for issue in &self.0 {
            writeln!(f, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TraceError {}

/// Microstrip inputs. Units are mm / relative permittivity / µm — the
/// units PCB fabs quote, so the UI passes them through untouched.
#[derive(Debug, Clone)]
pub struct MicrostripInput {
    /// Trace width w.
    pub width_mm: f64,
    /// Dielectric height h (trace-to-plane).
    pub height_mm: f64,
    /// Relative permittivity of the board (e.g. 4.6 for FR-4).
    pub er: f64,
    /// Copper thickness t (e.g. 34.8 for 1 oz).
    pub thickness_um: f64,
    /// Loss tangent of the board, for the optional conductor+dielectric
    /// loss readout (0 to skip).
    pub loss_tangent: f64,
    /// Frequency (GHz) for the loss readout (0 to skip). Also drives the
    /// quasi-static validity note.
    pub freq_ghz: f64,
}

/// Symmetric stripline inputs (trace centered between planes).
#[derive(Debug, Clone)]
pub struct StriplineInput {
    /// Trace width w.
    pub width_mm: f64,
    /// Plane-to-plane spacing b (the trace sits at b/2).
    pub spacing_mm: f64,
    /// Relative permittivity of the board laminate.
    pub er: f64,
    /// Copper thickness t.
    pub thickness_um: f64,
    /// Loss tangent, for the optional loss readout (0 to skip).
    pub loss_tangent: f64,
    /// Frequency (GHz) for the loss readout (0 to skip).
    pub freq_ghz: f64,
}

/// Discriminated union so the UI can pass one structure through `solve`.
#[derive(Debug, Clone)]
pub enum TraceInput {
    Microstrip(MicrostripInput),
    Stripline(StriplineInput),
}

/// Everything the UI shows for one solved trace.
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// Characteristic impedance, Ω.
    pub z0: f64,
    /// Effective (filled) relative permittivity. Equals εr for stripline.
    pub epsilon_eff: f64,
    /// Propagation delay, ps per mm of trace.
    pub delay_ps_per_mm: f64,
    /// Distributed capacitance, pF per mm.
    pub c_pf_per_mm: f64,
    /// Distributed inductance, nH per mm.
    pub l_nh_per_mm: f64,
    /// Thickness-corrected effective width actually used, mm.
    pub effective_width_mm: f64,
    /// Estimated attenuation at `freq_ghz`, dB per mm (0 when skipped).
    pub loss_db_per_mm: f64,
    /// Validity notes the UI should surface (quasi-static bounds, wide/narrow
    /// approximations in play, …).
    pub notes: Vec<String>,
}

/// Solves whichever trace the input describes, validating every field first.
pub fn solve(input: &TraceInput) -> Result<TraceResult, TraceError> {
    match input {
        TraceInput::Microstrip(m) => solve_microstrip(m),
        TraceInput::Stripline(s) => solve_stripline(s),
    }
}

/// Convenience constructor the UI uses for copper-weight presets.
pub fn copper_thickness(weight_index: usize) -> f64 {
    COPPER_WEIGHTS_UM
        .get(weight_index)
        .map(|(_, um)| *um)
        .unwrap_or(34.8)
}

fn validate_common(
    width_mm: f64,
    height_mm: f64,
    er: f64,
    thickness_um: f64,
    issues: &mut Vec<String>,
) {
    if !(width_mm.is_finite() && width_mm > 0.0) {
        issues.push("Trace width must be a positive number.".to_string());
    }
    if !(height_mm.is_finite() && height_mm > 0.0) {
        issues.push("Dielectric height must be a positive number.".to_string());
    }
    if !(er.is_finite() && er >= 1.0) {
        issues.push("Dielectric constant must be at least 1 (air).".to_string());
    }
    if !(thickness_um.is_finite() && thickness_um >= 0.0) {
        issues.push("Copper thickness must be zero or a positive number.".to_string());
    }
}

fn solve_microstrip(m: &MicrostripInput) -> Result<TraceResult, TraceError> {
    let mut issues = Vec::new();
    validate_common(m.width_mm, m.height_mm, m.er, m.thickness_um, &mut issues);
    if m.loss_tangent < 0.0 {
        issues.push("Loss tangent must not be negative.".to_string());
    }
    if m.freq_ghz < 0.0 {
        issues.push("Frequency must not be negative.".to_string());
    }
    if !issues.is_empty() {
        return Err(TraceError(issues));
    }

    let h = m.height_mm;
    let t = m.thickness_um / 1000.0; // mm
    // Garg–Bahl finite-thickness effective width (t ≪ h).
    let dw = if t > 0.0 && h > 0.0 {
        let raw = if m.width_mm >= h / (2.0 * std::f64::consts::PI) {
            (t / std::f64::consts::PI) * (1.0 + (2.0 * h / t).ln())
        } else {
            (t / std::f64::consts::PI) * (1.0 + (4.0 * std::f64::consts::PI * m.width_mm / t).ln())
        };
        raw.max(0.0)
    } else {
        0.0
    };
    let weff = m.width_mm + dw;
    let u = weff / h;

    // Hammerstad–Jensen Z0 (air line).
    let f_term = 6.0 + (2.0 * std::f64::consts::PI - 6.0) * (-(30.666 / u).powf(0.7528)).exp();
    let z0_air = ETA0 / (2.0 * std::f64::consts::PI) * (f_term / u + (1.0 + (2.0 / u).powi(2)).sqrt()).ln();

    // Hammerstad–Jensen εeff.
    let a = 1.0 + (1.0 / 49.0) * ((u.powi(4) + (u / 52.0).powi(2)) / (u.powi(4) + 0.432)).ln()
        + (1.0 / 18.7) * (1.0 + (u / 18.1).powi(3)).ln();
    let b = 0.564 * ((m.er - 0.9) / (m.er + 3.0)).powf(0.053);
    let eps_eff = (m.er + 1.0) / 2.0 + (m.er - 1.0) / 2.0 * (1.0 + 10.0 / u).powf(-a * b);

    let delay = delay_ps_per_mm(eps_eff);
    let c = c_pf_per_mm(z0_air / eps_eff.sqrt(), delay);
    let l = l_nh_per_mm(z0_air / eps_eff.sqrt(), delay);
    let loss = attenuation_db_per_mm(m.freq_ghz, m.loss_tangent, eps_eff, z0_air / eps_eff.sqrt(), m.width_mm);

    let mut notes = Vec::new();
    if u < 0.01 || u > 100.0 {
        notes.push(
            "w/h is outside 0.01–100, where the microstrip model is no longer reliable."
                .to_string(),
        );
    }
    if m.er > 128.0 {
        notes.push("εr above 128 is outside the fitted range of the model.".to_string());
    }
    if t > 0.3 * h {
        notes.push("Copper thicker than 0.3·h; the thickness correction loses accuracy.".to_string());
    }
    notes.extend(loss_and_quasi_static_notes(
        m.freq_ghz,
        m.loss_tangent,
        eps_eff,
        h,
    ));

    Ok(TraceResult {
        z0: z0_air / eps_eff.sqrt(),
        epsilon_eff: eps_eff,
        delay_ps_per_mm: delay,
        c_pf_per_mm: c,
        l_nh_per_mm: l,
        effective_width_mm: weff,
        loss_db_per_mm: loss,
        notes,
    })
}

fn solve_stripline(s: &StriplineInput) -> Result<TraceResult, TraceError> {
    let mut issues = Vec::new();
    validate_common(s.width_mm, s.spacing_mm, s.er, s.thickness_um, &mut issues);
    if s.loss_tangent < 0.0 {
        issues.push("Loss tangent must not be negative.".to_string());
    }
    if s.freq_ghz < 0.0 {
        issues.push("Frequency must not be negative.".to_string());
    }
    if !issues.is_empty() {
        return Err(TraceError(issues));
    }

    let b = s.spacing_mm;
    let t = s.thickness_um / 1000.0;
    // First-order thickness correction in the Garg–Bahl form: widen the
    // strip as if some of the field fringing were replaced by metal.
    let dw = if t > 0.0 && b > t {
        ((t / std::f64::consts::PI) * (1.0 + (2.0 * (b - t) / t).ln())).max(0.0)
    } else {
        0.0
    };
    let weff = s.width_mm + dw;

    let k = 1.0 / (std::f64::consts::PI * weff / (2.0 * b)).cosh(); // sech
    let z0 = 30.0 * std::f64::consts::PI / s.er.sqrt() * ellipk_ratio(k);

    // Homogeneous dielectric: εeff = εr exactly; TEM propagation.
    let delay = delay_ps_per_mm(s.er);
    let c = c_pf_per_mm(z0, delay);
    let l = l_nh_per_mm(z0, delay);
    let loss = attenuation_db_per_mm(s.freq_ghz, s.loss_tangent, s.er, z0, s.width_mm);

    let mut notes = Vec::new();
    if t > 0.25 * b {
        notes.push("Copper thicker than b/4; the thickness correction loses accuracy.".to_string());
    }
    if weff / b > 2.0 {
        notes.push(
            "Very wide trace for this plane spacing (w_eff/b > 2) — verify against a \
             field solver before trusting the value."
                .to_string(),
        );
    }
    notes.extend(loss_and_quasi_static_notes(s.freq_ghz, s.loss_tangent, s.er, b / 2.0));

    Ok(TraceResult {
        z0,
        epsilon_eff: s.er,
        delay_ps_per_mm: delay,
        c_pf_per_mm: c,
        l_nh_per_mm: l,
        effective_width_mm: weff,
        loss_db_per_mm: loss,
        notes,
    })
}

/// K(k)/K(k′) by the AGM — K(k) = π/(2·AGM(1, k′)), so the ratio collapses
/// to AGM(1,k)/AGM(1,k′). Exact to double precision in ~15 iterations.
fn ellipk_ratio(k: f64) -> f64 {
    let kp = (1.0 - k * k).sqrt().max(1e-300);
    agm(1.0, k) / agm(1.0, kp)
}

fn agm(mut a: f64, mut b: f64) -> f64 {
    for _ in 0..60 {
        let (na, nb) = (0.5 * (a + b), (a * b).sqrt());
        if (na - nb).abs() < 1e-15 * na.abs() {
            return na;
        }
        a = na;
        b = nb;
    }
    a
}

/// Quasi-static delay, ps/mm: √(εeff)/c (air: 3.336 ps/mm).
fn delay_ps_per_mm(eps_eff: f64) -> f64 {
    eps_eff.sqrt() / C0 * 1e9
}

fn c_pf_per_mm(z0: f64, delay_ps_per_mm: f64) -> f64 {
    // C' = t_pd / Z0 (t_pd in s per m scale; here per mm consistently).
    delay_ps_per_mm * 1e-12 / z0 * 1e12
}

fn l_nh_per_mm(z0: f64, delay_ps_per_mm: f64) -> f64 {
    // L' = Z0 · t_pd.
    z0 * delay_ps_per_mm * 1e-12 * 1e9
}

/// First-order attenuation estimate — dielectric loss plus skin-effect
/// conductor loss with a uniform-current flat strip, dB/mm (0 when the
/// frequency is unset). Surface roughness, plating, and current crowding
/// are ignored, so treat it as a lower bound; the UI labels it an estimate.
fn attenuation_db_per_mm(freq_ghz: f64, loss_tangent: f64, eps_eff: f64, z0: f64, width_mm: f64) -> f64 {
    if freq_ghz <= 0.0 {
        return 0.0;
    }
    // Dielectric loss: αd = (π·f·√εeff/c)·tanδ, converted Np→dB.
    let f_hz = freq_ghz * 1e9;
    let alpha_d_np_per_m = std::f64::consts::PI * f_hz * eps_eff.sqrt() / C0 * loss_tangent;
    // Conductor loss: αc = Rs/(2·Z0·w) with Rs = √(π·f·µ0/σ), copper σ = 5.8e7.
    let rs = (std::f64::consts::PI * f_hz * 4.0e-7 * std::f64::consts::PI / 5.8e7).sqrt();
    let width_m = width_mm.max(1e-6) * 1e-3;
    let alpha_c_np_per_m = rs / (2.0 * z0 * width_m);
    (alpha_d_np_per_m + alpha_c_np_per_m) * 8.686 / 1000.0
}

fn loss_and_quasi_static_notes(freq_ghz: f64, loss_tangent: f64, eps_eff: f64, h_mm: f64) -> Vec<String> {
    let mut notes = Vec::new();
    if freq_ghz > 0.0 {
        // Quasi-static models hold while the cross-section is electrically
        // small; flag past ~λ/10 of the largest dimension.
        let lambda_mm = C0 / (freq_ghz * 1e9) * 1e3 / eps_eff.sqrt();
        if h_mm > lambda_mm / 10.0 {
            notes.push(format!(
                "At {freq_ghz} GHz the board is electrically thick (h > λ/10); a \
                 full-wave solver is needed for accurate results."
            ));
        }
    }
    if loss_tangent > 0.0 && freq_ghz <= 0.0 {
        notes.push("Enter a frequency to include the loss estimate (tan δ is set).".to_string());
    }
    notes
}

#[cfg(test)]
mod tests;
