//! Antenna dimensioning and impedance matching (Pro edition).
//!
//! Standard textbook closed forms — Balanis *Antenna Theory* for the
//! dipole/monopole/patch dimensioning, Pozar *Microwave Engineering* for
//! the matching networks. These are first-order design tools: they size an
//! antenna and its matching network for a target frequency and feed
//! impedance; EM simulation is still needed for final production tuning.

use std::fmt;

use crate::trace::C0;

/// Single error type carrying every validation message at once.
#[derive(Debug, Clone)]
pub struct AntennaError(pub Vec<String>);

impl fmt::Display for AntennaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for issue in &self.0 {
            writeln!(f, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AntennaError {}

/// Which antenna to dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntennaKind {
    /// Center-fed half-wave dipole.
    Dipole,
    /// Quarter-wave monopole over a ground plane.
    Monopole,
    /// Rectangular microstrip patch, inset fed.
    Patch,
}

/// Dipole/monopole design input.
#[derive(Debug, Clone)]
pub struct WireAntennaInput {
    pub kind: AntennaKind,
    /// Design (resonance) frequency, MHz.
    pub freq_mhz: f64,
    /// Element diameter, mm — drives the end-effect shortening.
    pub element_diameter_mm: f64,
}

/// Patch antenna design input.
#[derive(Debug, Clone)]
pub struct PatchInput {
    /// Design frequency, MHz.
    pub freq_mhz: f64,
    /// Board relative permittivity.
    pub er: f64,
    /// Board height h, mm.
    pub height_mm: f64,
    /// Feed impedance to inset-match to, Ω (typically 50).
    pub feed_z: f64,
}

#[derive(Debug, Clone)]
pub enum AntennaInput {
    Wire(WireAntennaInput),
    Patch(PatchInput),
}

/// Dimensioned antenna with the readouts the UI shows.
#[derive(Debug, Clone)]
pub struct AntennaResult {
    /// Which antenna this result describes.
    pub kind: AntennaKind,
    /// Headline dimension: element length, mm.
    pub length_mm: f64,
    /// Patch width (patch only; 0 otherwise), mm.
    pub width_mm: f64,
    /// Inset feed depth from a radiating edge (patch only), mm.
    pub inset_mm: f64,
    /// Free-space wavelength, mm.
    pub wavelength_mm: f64,
    /// Expected feed impedance at resonance, Ω.
    pub feed_impedance: f64,
    /// Gain estimate, dBi (documented as a typical range mid-point for
    /// patches; classical textbook value for wires).
    pub gain_dbi: f64,
    /// −3 dB beamwidth estimate, degrees (0 when not meaningful).
    pub beamwidth_deg: f64,
    pub notes: Vec<String>,
}

/// Dimensions whichever antenna the input describes.
pub fn design(input: &AntennaInput) -> Result<AntennaResult, AntennaError> {
    match input {
        AntennaInput::Wire(w) => design_wire(w),
        AntennaInput::Patch(p) => design_patch(p),
    }
}

/// Measured resonant-length shortening of a center-fed cylindrical dipole
/// as a fraction of the free-space half wavelength, versus l/d (King–Moore
/// measured-data norm, log-interpolated; ±1.5 %):
/// fat elements resonate short, thin wires approach ~0.48λ.
fn resonant_fraction(l_over_d: f64) -> f64 {
    // (l/d, fraction of λ/2) anchor points from the classic charts.
    const TABLE: [(f64, f64); 5] = [
        (10.0, 0.87),
        (50.0, 0.93),
        (500.0, 0.944),
        (10_000.0, 0.954),
        (1_000_000.0, 0.958),
    ];
    if l_over_d <= TABLE[0].0 {
        return TABLE[0].1;
    }
    for window in TABLE.windows(2) {
        let (lo, flo) = window[0];
        let (hi, fhi) = window[1];
        if l_over_d <= hi {
            let t = (l_over_d.ln() - lo.ln()) / (hi.ln() - lo.ln());
            return flo + t * (fhi - flo);
        }
    }
    TABLE[TABLE.len() - 1].1
}

fn design_wire(w: &WireAntennaInput) -> Result<AntennaResult, AntennaError> {
    let mut issues = Vec::new();
    if !(w.freq_mhz.is_finite() && w.freq_mhz > 0.0) {
        issues.push("Frequency must be a positive number.".to_string());
    }
    if !(w.element_diameter_mm.is_finite() && w.element_diameter_mm > 0.0) {
        issues.push("Element diameter must be a positive number.".to_string());
    }
    if !issues.is_empty() {
        return Err(AntennaError(issues));
    }

    let lambda_mm = C0 / (w.freq_mhz * 1e6) * 1e3;
    // Self-consistent shortening: the resonant length is `fraction` of the
    // free-space half wavelength (quarter for a monopole), but `fraction`
    // depends on l/d, which depends on the length — iterate the fixed point
    // (converges in a few passes).
    let mut length_mm = match w.kind {
        AntennaKind::Monopole => 0.235 * lambda_mm,
        _ => 0.47 * lambda_mm,
    };
    let mut fraction = 0.94;
    for _ in 0..24 {
        fraction = resonant_fraction(length_mm / w.element_diameter_mm);
        let target = match w.kind {
            AntennaKind::Monopole => fraction * lambda_mm / 4.0,
            _ => fraction * lambda_mm / 2.0,
        };
        if (target - length_mm).abs() < 1e-10 * lambda_mm {
            length_mm = target;
            break;
        }
        length_mm = target;
    }

    let (feed_impedance, gain_dbi, beamwidth_deg) = match w.kind {
        AntennaKind::Dipole => (73.0, 2.15, 78.0),
        AntennaKind::Monopole => (36.5, 5.15, 45.0),
        AntennaKind::Patch => unreachable!("wire design handles dipole/monopole only"),
    };

    let mut notes = Vec::new();
    match w.kind {
        AntennaKind::Monopole => notes.push(
            "Monopole impedance and beamwidth assume an infinite, lossless ground \
             plane; a real radial ground raises the impedance somewhat."
                .to_string(),
        ),
        AntennaKind::Dipole => notes.push(
            "Feed impedance ≈ 73 Ω at exact resonance; trim the length a few percent \
             to tune out the reactive part on a real install."
                .to_string(),
        ),
        AntennaKind::Patch => unreachable!(),
    }
    if w.element_diameter_mm > lambda_mm / 20.0 {
        notes.push(
            "Element diameter > λ/20 is unusually fat; verify with a \
             method-of-moments run."
                .to_string(),
        );
    }
    notes.push(format!(
        "End-effect shortening applied: element resonates at {:.3}·λ/2 (l/d = {:.0}).",
        fraction,
        length_mm / w.element_diameter_mm
    ));

    Ok(AntennaResult {
        kind: w.kind,
        length_mm,
        width_mm: 0.0,
        inset_mm: 0.0,
        wavelength_mm: lambda_mm,
        feed_impedance,
        gain_dbi,
        beamwidth_deg,
        notes,
    })
}

fn design_patch(p: &PatchInput) -> Result<AntennaResult, AntennaError> {
    let mut issues = Vec::new();
    if !(p.freq_mhz.is_finite() && p.freq_mhz > 0.0) {
        issues.push("Frequency must be a positive number.".to_string());
    }
    if !(p.er.is_finite() && p.er >= 1.0) {
        issues.push("Dielectric constant must be at least 1 (air).".to_string());
    }
    if !(p.height_mm.is_finite() && p.height_mm > 0.0) {
        issues.push("Board height must be a positive number.".to_string());
    }
    if !(p.feed_z.is_finite() && p.feed_z > 0.0) {
        issues.push("Feed impedance must be a positive number.".to_string());
    }
    if !issues.is_empty() {
        return Err(AntennaError(issues));
    }

    let lambda_mm = C0 / (p.freq_mhz * 1e6) * 1e3;
    // Width for good radiation efficiency (Balanis 14-6a):
    // W = c/(2f)·√(2/(εr+1)).
    let width_mm = (C0 / (p.freq_mhz * 1e6) / 2.0) * 1e3 * (2.0 / (p.er + 1.0)).sqrt();
    let u = width_mm / p.height_mm;
    // Effective permittivity at the patch width (Hammerstad fits, same as
    // the trace module).
    let a = 1.0 + (1.0 / 49.0) * ((u.powi(4) + (u / 52.0).powi(2)) / (u.powi(4) + 0.432)).ln()
        + (1.0 / 18.7) * (1.0 + (u / 18.1).powi(3)).ln();
    let b_fit = 0.564 * ((p.er - 0.9) / (p.er + 3.0)).powf(0.053);
    let eps_eff = (p.er + 1.0) / 2.0 + (p.er - 1.0) / 2.0 * (1.0 + 10.0 / u).powf(-a * b_fit);
    // Fringing extension (Balanis 14-7): ΔL/h = 0.412·(εeff+0.3)(u+0.264) /
    // ((εeff−0.258)(u+0.8)).
    let dl = 0.412 * (eps_eff + 0.3) * (u + 0.264) / ((eps_eff - 0.258) * (u + 0.8)) * p.height_mm;
    // Resonant length (Balanis 14-6): L = c/(2f√εeff) − 2ΔL.
    let length_mm = (C0 / (p.freq_mhz * 1e6) / 2.0) * 1e3 / eps_eff.sqrt() - 2.0 * dl;

    // Edge feed resistance from the single-slot conductance (Balanis 14-12,
    // first term G1 = (1/90)(W/λ0)²; slot mutual conductance G12 ignored —
    // first-order): Rin(edge) = 1/(2G1).
    let wl = width_mm / lambda_mm;
    let g1 = (1.0 / 90.0) * wl * wl;
    let rin_edge = 1.0 / (2.0 * g1);

    // Inset depth for the requested feed impedance:
    // Rin(y0) = Rin(edge)·cos²(π·y0/L) → y0 = (L/π)·acos(√(Zfeed/Rin_edge)).
    let ratio = (p.feed_z / rin_edge).clamp(0.0, 1.0);
    let inset_mm = (length_mm / std::f64::consts::PI) * ratio.sqrt().acos();

    let mut notes = Vec::new();
    if p.height_mm > lambda_mm / 20.0 {
        notes.push(
            "Board height > λ0/20 (electrically thick substrate); surface waves and \
             the first-order model both become unreliable."
                .to_string(),
        );
    }
    if length_mm <= 0.0 || width_mm <= 0.0 {
        notes.push("Resulting patch dimensions are not physical — check the inputs.".to_string());
    }
    if ratio >= 1.0 {
        notes.push(
            "Requested feed impedance is at/above the edge resistance; an inset can \
             only reduce impedance — quarter-wave transform the edge instead."
                .to_string(),
        );
    } else {
        notes.push(format!(
            "Edge resistance ≈ {rin_edge:.0} Ω; the inset depth brings the feed point \
             to ≈ {:.0} Ω. Trim length ±2 % to center resonance in practice.",
            p.feed_z
        ));
    }
    notes.push(
        "Gain estimate is the typical measured 6–9 dBi band mid-point for a \
         rectangular patch on a modest ground plane."
            .to_string(),
    );

    Ok(AntennaResult {
        kind: AntennaKind::Patch,
        length_mm,
        width_mm,
        inset_mm,
        wavelength_mm: lambda_mm,
        feed_impedance: p.feed_z,
        gain_dbi: 7.5,
        beamwidth_deg: 0.0,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Matching networks
// ---------------------------------------------------------------------------

/// Which matching network to synthesize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Quarter-wave transformer (real, resistive load only).
    QuarterWave,
    /// Two-element L network (handles a resistive load; absorb any load
    /// reactance into the first element's value).
    LNetwork,
    /// Single shunt stub + series line (real load).
    SingleStub,
}

#[derive(Debug, Clone)]
pub struct MatchInput {
    pub kind: MatchKind,
    /// System/feedline impedance, Ω (typically 50).
    pub source_z: f64,
    /// Load impedance, Ω (resistive; the UI notes when a load reactance
    /// must be absorbed first).
    pub load_r: f64,
    /// Design frequency, MHz (sets physical lengths).
    pub freq_mhz: f64,
    /// Effective permittivity of the interconnect for length computation.
    pub eps_eff: f64,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Summary line(s) the UI renders directly.
    pub summary: Vec<String>,
    /// Transformer section impedance(s), Ω (empty for the L network,
    /// which is reported as lumped reactances).
    pub section_z: Vec<f64>,
    /// Physical lengths of the transmission-line sections, mm.
    pub section_length_mm: Vec<f64>,
    pub notes: Vec<String>,
}

/// Match-specific error (same shape as [`AntennaError`], separate type so
/// callers can distinguish design vs matching failures).
#[derive(Debug, Clone)]
pub struct MatchError(pub Vec<String>);

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for issue in &self.0 {
            writeln!(f, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MatchError {}

/// Synthesizes the requested matching network at the design frequency.
pub fn design_match(m: &MatchInput) -> Result<MatchResult, MatchError> {
    let mut issues = Vec::new();
    if !(m.source_z.is_finite() && m.source_z > 0.0) {
        issues.push("Source impedance must be a positive number.".to_string());
    }
    if !(m.load_r.is_finite() && m.load_r > 0.0) {
        issues.push("Load resistance must be a positive number.".to_string());
    }
    if !(m.freq_mhz.is_finite() && m.freq_mhz > 0.0) {
        issues.push("Frequency must be a positive number.".to_string());
    }
    if !(m.eps_eff.is_finite() && m.eps_eff >= 1.0) {
        issues.push("Effective permittivity must be at least 1.".to_string());
    }
    if !issues.is_empty() {
        return Err(MatchError(issues));
    }

    let lambda0_mm = C0 / (m.freq_mhz * 1e6) * 1e3;
    let lambda_mm = lambda0_mm / m.eps_eff.sqrt();
    let quarter_mm = lambda_mm / 4.0;

    match m.kind {
        MatchKind::QuarterWave => {
            let section_z = (m.source_z * m.load_r).sqrt();
            Ok(MatchResult {
                summary: vec![format!(
                    "Quarter-wave transformer: Z_t = √(Z_s·R_L) = {section_z:.2} Ω, \
                     length λ/4·εeff^(−½) = {quarter_mm:.3} mm at {} MHz.",
                    m.freq_mhz
                )],
                section_z: vec![section_z],
                section_length_mm: vec![quarter_mm],
                notes: vec![
                    "Works only for a purely resistive load — tune out any load \
                     reactance first (or use the L network)."
                        .to_string(),
                    "Exact at f0; usable bandwidth is roughly ±20 % of f0 for \
                     typical impedance ratios."
                        .to_string(),
                ],
            })
        }
        MatchKind::LNetwork => {
            // Two-element L (Pozar §5.9): Q from the resistance ratio;
            // magnitudes |X_s| = Q·R_small (series, low-R side) and
            // |X_p| = R_big/Q (shunt, high-R side), with OPPOSITE reactance
            // signs — e.g. source 50 → load 100: series −j50 (C) with shunt
            // +j100 (L) gives Z_in = −j50 + (100∥j100) = 50 Ω exactly.
            let (r_big, r_small) = if m.source_z >= m.load_r {
                (m.source_z, m.load_r)
            } else {
                (m.load_r, m.source_z)
            };
            let q = (r_big / r_small - 1.0).sqrt();
            let x_series = q * r_small; // magnitude
            let x_shunt = r_big / q; // magnitude
            let omega = 2.0 * std::f64::consts::PI * m.freq_mhz * 1e6;
            let component = |x: f64| {
                if x > 0.0 {
                    format!("inductor {:.3} nH", x / omega * 1e9)
                } else {
                    format!("capacitor {:.3} pF", -1.0 / (omega * x) * 1e12)
                }
            };
            Ok(MatchResult {
                summary: vec![
                    format!(
                        "Low-pass arrangement: series {s}, shunt {p} — series element \
                         on the low-R side.",
                        s = component(-x_series),
                        p = component(x_shunt),
                    ),
                    format!(
                        "High-pass alternative: series {s}, shunt {p} (same match, \
                         different harmonic behavior).",
                        s = component(x_series),
                        p = component(-x_shunt),
                    ),
                    format!(
                        "Loaded Q = {q:.3} — match bandwidth ≈ ±{:.0} % of f0.",
                        50.0 / q
                    ),
                ],
                section_z: vec![],
                section_length_mm: vec![],
                notes: vec![
                    "If the load has reactance, absorb it: adjust the series \
                     element's value by the load reactance (subtract a series \
                     inductance's reactance, add capacitance, per the sign)."
                        .to_string(),
                ],
            })
        }
        MatchKind::SingleStub => {
            // Shunt single-stub match of a REAL load (Pozar §5.4):
            // normalized r = R_L/Z0; the line length follows from
            // Re{y_in} = 1 ⇔ tan²(βd) = r, and the stub then supplies
            // b_stub = −Im{y_in} where y_in = [r(1+t²) + jt(r²−1)]/(r²+t²).
            let r = m.load_r / m.source_z;
            let gamma = (r - 1.0) / (r + 1.0);
            if r <= 0.0 {
                return Err(MatchError(vec![
                    "Single-stub matching needs a positive load resistance.".to_string(),
                ]));
            }
            if (r - 1.0).abs() < 1e-9 {
                return Err(MatchError(vec![
                    "Load already matches the line (within tolerance) — no stub needed."
                        .to_string(),
                ]));
            }
            let t = r.sqrt();
            let beta_d = t.atan();
            let d_mm = beta_d / (2.0 * std::f64::consts::PI) * lambda_mm;
            let b_in = t * (r * r - 1.0) / (r * r + t * t);
            let b_stub = -b_in;
            // Open stub: y = j·tan(βl); short stub: y = −j·cot(βl).
            // Normalize the atan branch to a positive physical length.
            let mut l_open = b_stub.atan();
            if l_open < 0.0 {
                l_open += std::f64::consts::PI;
            }
            let l_open_mm = l_open / (2.0 * std::f64::consts::PI) * lambda_mm;
            let l_short_mm = if l_open_mm >= lambda_mm / 4.0 {
                l_open_mm - lambda_mm / 4.0
            } else {
                l_open_mm + lambda_mm / 4.0
            };
            Ok(MatchResult {
                summary: vec![
                    format!(
                        "Place a shunt stub {:.3} mm ({:.2}°) from the load.",
                        d_mm,
                        beta_d / std::f64::consts::PI * 180.0
                    ),
                    format!(
                        "Stub susceptance needed: {b_stub:+.4} (normalized) — open-circuit \
                         stub {:.3} mm, or short-circuit stub {:.3} mm.",
                        l_open_mm, l_short_mm
                    ),
                    format!(
                        "Load Γ = {gamma:+.3} ({:.1} dB return loss before matching).",
                        -20.0 * gamma.abs().log10()
                    ),
                ],
                section_z: vec![m.source_z],
                section_length_mm: vec![d_mm, l_open_mm],
                notes: vec![
                    "Assumes a resistive load and a lossless line/stub at Z_s — \
                     include any load reactance in the load length before cutting metal."
                        .to_string(),
                ],
            })
        }
    }
}

#[cfg(test)]
mod tests;
