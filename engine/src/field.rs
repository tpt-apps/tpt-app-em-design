//! 2D quasi-static field solve and radiation patterns (Pro edition).
//!
//! `solve_microstrip_field` discretizes the microstrip cross-section in a
//! grounded shield box and iterates Laplace's equation with SOR finite
//! differences, weighting vertical stencils across the dielectric
//! interface with the harmonic-mean permittivity (the flux-consistent
//! choice). The stored field energy yields a capacitance per unit length;
//! solving once with εr and once with air gives an independent Z0 estimate
//! that the validation suite cross-checks against the closed-form trace
//! model (typically within ~2–3 % on the grids used here).
//!
//! `radiation_pattern` samples the standard far-field cuts: the exact
//! thin-dipole pattern for dipole/monopole, and documented approximate
//! cuts for the rectangular patch (two-slot array factor along its
//! length, slot line-source element pattern along its width).

use std::fmt;

use crate::trace::C0;

/// Vacuum permittivity, F/m — the energy integral is evaluated in SI units
/// (V/m field, m² cells) so capacitance comes out in farads directly.
const EPS0: f64 = 8.854_187_812_8e-12;

/// Solver error.
#[derive(Debug, Clone)]
pub struct FieldError(pub Vec<String>);

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for issue in &self.0 {
            writeln!(f, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for FieldError {}

/// Microstrip cross-section to solve (mm / relative units; potentials in V).
#[derive(Debug, Clone)]
pub struct FieldOptions {
    /// Trace width w.
    pub width_mm: f64,
    /// Dielectric height h (trace-to-plane).
    pub height_mm: f64,
    /// Board relative permittivity (below y = h; air above).
    pub er: f64,
    /// Shield-box width as a multiple of h (walls are grounded V=0).
    pub box_width_mult: f64,
    /// Shield-box height above the trace as a multiple of h.
    pub box_top_mult: f64,
    /// Grid columns across the box width (rows follow from the aspect).
    pub grid_cols: usize,
    /// Trace conductor potential.
    pub v_conductor: f64,
}

impl Default for FieldOptions {
    fn default() -> Self {
        Self {
            width_mm: 3.0,
            height_mm: 1.6,
            er: 4.6,
            box_width_mult: 20.0,
            box_top_mult: 10.0,
            grid_cols: 240,
            v_conductor: 1.0,
        }
    }
}

/// A solved field: potential and |E| on a regular grid, plus the
/// capacitance/Z0 readouts extracted from the field energy.
#[derive(Debug, Clone)]
pub struct FieldResult {
    pub cols: usize,
    pub rows: usize,
    /// Grid spacing, mm (uniform in x and y).
    pub dx: f64,
    pub dy: f64,
    /// Potential, V, row-major; row 0 is the TOP of the box (screen-down
    /// drawing), so y = row·dy measures downward from the box top.
    pub potential: Vec<f64>,
    /// |E| magnitude, V/mm, same layout.
    pub e_mag: Vec<f64>,
    /// εr the solve was run with.
    pub er: f64,
    /// Trace height above the ground plane, mm (marks the interface row).
    pub height_mm: f64,
    /// Trace width actually energized, mm.
    pub width_mm: f64,
    /// Capacitance per unit length from the field energy, pF/mm.
    pub c_pf_per_mm: f64,
    /// Z0 from the two-solve capacitance pair (see module docs), Ω.
    pub z0_field_estimate: f64,
    pub iterations: usize,
    pub converged: bool,
}

impl FieldResult {
    /// |E| index lookup for probing at fractional grid coordinates.
    pub fn e_at(&self, col: usize, row: usize) -> f64 {
        self.e_mag[row * self.cols + col]
    }
}

/// Solves the microstrip cross-section (with air above the interface).
/// Runs the εr solve plus the air solve needed for the Z0 estimate.
pub fn solve_microstrip_field(opts: &FieldOptions) -> Result<FieldResult, FieldError> {
    let mut issues = Vec::new();
    if !(opts.width_mm.is_finite() && opts.width_mm > 0.0) {
        issues.push("Trace width must be a positive number.".to_string());
    }
    if !(opts.height_mm.is_finite() && opts.height_mm > 0.0) {
        issues.push("Dielectric height must be a positive number.".to_string());
    }
    if !(opts.er.is_finite() && opts.er >= 1.0) {
        issues.push("Dielectric constant must be at least 1.".to_string());
    }
    if opts.grid_cols < 60 || opts.grid_cols > 640 {
        issues.push("Grid size must be between 60 and 640 columns.".to_string());
    }
    if opts.box_width_mult < 6.0 || opts.box_top_mult < 3.0 {
        issues.push(
            "Shield box too small — width ≥ 6·h and top ≥ 3·h are needed for the \
             grounded-walls boundary condition to be unobtrusive."
                .to_string(),
        );
    }
    if !issues.is_empty() {
        return Err(FieldError(issues));
    }

    let width = opts.box_width_mult * opts.height_mm;
    let height = opts.box_top_mult * opts.height_mm;
    let dx = width / (opts.grid_cols - 1) as f64;
    let rows = ((height / dx).round() as usize).max(40) + 1;
    let dy = height / (rows - 1) as f64;

    let half_w = opts.width_mm / 2.0;
    let center = width / 2.0;
    // Interface (dielectric top) sits h above the ground plane; box top is
    // box_top_mult·h above the trace, ground at the box bottom.
    let interface_y = height - opts.height_mm; // y measured DOWN from box top
    let trace_row_f = interface_y / dy; // the same position in grid rows

    // Trace footprint as a Dirichlet band on the nearest row.
    let mut trace_cols: Vec<usize> = Vec::new();
    let trace_row = (trace_row_f.round() as usize).clamp(1, rows - 2);
    for col in 0..opts.grid_cols {
        let x = col as f64 * dx;
        if (x - center).abs() <= half_w {
            trace_cols.push(col);
        }
    }
    if trace_cols.len() < 2 {
        return Err(FieldError(vec![
            "Trace is narrower than the grid spacing — reduce the grid size or widen \
             the trace."
                .to_string(),
        ]));
    }
    let is_trace: Vec<bool> = (0..opts.grid_cols).map(|c| trace_cols.contains(&c)).collect();

    // Per-row permittivity: dielectric below y = h, air above. The stencil
    // weight on each arm is the permittivity of the material between the
    // node and its neighbor; the interface row's up-arm crosses materials
    // and takes the harmonic mean (flux-consistent).
    let eps_below = opts.er;
    let eps_above = 1.0;
    let eps_interface_up = 2.0 * eps_below * eps_above / (eps_below + eps_above);

    let mut potential = vec![0.0f64; opts.grid_cols * rows];

    // SOR sweep; ω from Young's estimate for a Dirichlet box, capped.
    let omega = (2.0 / (1.0 + std::f64::consts::PI / (opts.grid_cols as f64))).clamp(1.0, 1.95);
    let max_iterations = 30_000usize;
    let tolerance = 5e-7;
    let mut iterations = 0usize;
    let mut converged = false;

    let idx = |col: usize, row: usize| row * opts.grid_cols + col;

    for _ in 0..max_iterations {
        let mut max_delta = 0.0f64;
        for row in 1..rows - 1 {
            let on_interface = row == trace_row;
            let in_air = (row as f64) < trace_row_f;
            let (w_up, w_down, w_side) = if on_interface {
                (eps_interface_up, eps_below, eps_below)
            } else if in_air {
                (eps_above, eps_above, eps_above)
            } else {
                (eps_below, eps_below, eps_below)
            };
            let weight_sum = 2.0 * w_side + w_up + w_down;
            for col in 1..opts.grid_cols - 1 {
                let i = idx(col, row);
                if on_interface && is_trace[col] {
                    continue; // Dirichlet conductor node
                }
                let neighbor_sum = w_side * (potential[i - 1] + potential[i + 1])
                    + w_up * potential[idx(col, row - 1)]
                    + w_down * potential[idx(col, row + 1)];
                let updated = neighbor_sum / weight_sum;
                let delta = (updated - potential[i]).abs();
                if delta > max_delta {
                    max_delta = delta;
                }
                potential[i] += omega * (updated - potential[i]);
            }
            if on_interface {
                for &col in &trace_cols {
                    let i = idx(col, row);
                    let delta = (opts.v_conductor - potential[i]).abs();
                    if delta > max_delta {
                        max_delta = delta;
                    }
                    potential[i] = opts.v_conductor;
                }
            }
        }
        iterations += 1;
        if max_delta < tolerance {
            converged = true;
            break;
        }
    }

    // |E| for the viewer (V/mm), from central differences of the potential.
    let mut e_mag = vec![0.0f64; opts.grid_cols * rows];
    for row in 0..rows {
        for col in 0..opts.grid_cols {
            let i = idx(col, row);
            let ex = if col == 0 {
                (potential[i + 1] - potential[i]) / dx
            } else if col + 1 == opts.grid_cols {
                (potential[i] - potential[i - 1]) / dx
            } else {
                (potential[i + 1] - potential[i - 1]) / (2.0 * dx)
            };
            let ey = if row == 0 {
                (potential[i] - potential[idx(col, row + 1)]) / dy
            } else if row + 1 == rows {
                (potential[idx(col, row - 1)] - potential[i]) / dy
            } else {
                (potential[idx(col, row - 1)] - potential[idx(col, row + 1)]) / (2.0 * dy)
            };
            e_mag[i] = (ex * ex + ey * ey).sqrt();
        }
    }

    // Capacitance by Gauss-law flux integration over the grounded box
    // boundary: C = ε0·∮εr·E_n·ds / V. (Energy-based extraction loses the
    // trace-edge field singularity to central-difference smoothing; the
    // boundary fields are smooth, so the flux integral is accurate.)
    let flux_filled = boundary_flux(&potential, opts.grid_cols, rows, dx, dy, trace_row_f, eps_below);
    let c_filled_si = EPS0 * flux_filled / opts.v_conductor; // F/m
    let c_filled = c_filled_si * 1e9; // pF/mm

    // Air solve for the Z0 estimate (same geometry, εr = 1).
    let mut air_opts = opts.clone();
    air_opts.er = 1.0;
    let flux_air = solve_flux_only(&air_opts, &trace_cols, trace_row, rows, dx, dy);

    // Quasi-static Z0 = 1/(c·√(C_air·C_filled)), both in F/m.
    let c_air_si = EPS0 * flux_air / opts.v_conductor;
    let z0 = 1.0 / (C0 * (c_air_si * c_filled_si).sqrt());

    Ok(FieldResult {
        cols: opts.grid_cols,
        rows,
        dx,
        dy,
        potential,
        e_mag,
        er: opts.er,
        height_mm: opts.height_mm,
        width_mm: opts.width_mm,
        c_pf_per_mm: c_filled,
        z0_field_estimate: z0,
        iterations,
        converged,
    })
}

/// Air-reference rerun: solves the same geometry with uniform ε = 1 and
/// returns the dimensionless boundary flux (C = ε0·flux/V).
fn solve_flux_only(
    opts: &FieldOptions,
    trace_cols: &[usize],
    trace_row: usize,
    rows: usize,
    dx: f64,
    dy: f64,
) -> f64 {
    let cols = opts.grid_cols;
    let mut potential = vec![0.0f64; cols * rows];
    let is_trace: Vec<bool> = (0..cols).map(|c| trace_cols.contains(&c)).collect();
    let omega = (2.0 / (1.0 + std::f64::consts::PI / (cols as f64))).clamp(1.0, 1.95);
    let idx = |col: usize, row: usize| row * cols + col;
    for _ in 0..30_000usize {
        let mut max_delta = 0.0f64;
        for row in 1..rows - 1 {
            for col in 1..cols - 1 {
                let i = idx(col, row);
                if row == trace_row && is_trace[col] {
                    continue;
                }
                let updated = 0.25
                    * (potential[i - 1]
                        + potential[i + 1]
                        + potential[idx(col, row - 1)]
                        + potential[idx(col, row + 1)]);
                let delta = (updated - potential[i]).abs();
                if delta > max_delta {
                    max_delta = delta;
                }
                potential[i] += omega * (updated - potential[i]);
            }
            if row == trace_row {
                for &col in trace_cols {
                    potential[idx(col, row)] = opts.v_conductor;
                }
            }
        }
        if max_delta < 5e-7 {
            break;
        }
    }
    boundary_flux(&potential, cols, rows, dx, dy, trace_row as f64, 1.0)
}

/// Σ εr·|E_n|·ds over the grounded box boundary (dimensionless; multiply by
/// ε0/V for capacitance in F/m — see the extraction note above).
fn boundary_flux(
    potential: &[f64],
    cols: usize,
    rows: usize,
    dx: f64,
    dy: f64,
    trace_row_f: f64,
    eps_below: f64,
) -> f64 {
    let idx = |col: usize, row: usize| row * cols + col;
    let eps_at_row = |row: usize| {
        if (row as f64) < trace_row_f {
            1.0
        } else {
            eps_below
        }
    };
    let mut flux = 0.0f64;
    // Floor (row rows−1): field points down into the wall.
    for col in 0..cols {
        let e = (potential[idx(col, rows - 1)] - potential[idx(col, rows - 2)]) / dy;
        flux += eps_below * e.abs() * dx;
    }
    // Ceiling (row 0): field points up into the wall.
    for col in 0..cols {
        let e = (potential[idx(col, 0)] - potential[idx(col, 1)]) / dy;
        flux += 1.0 * e.abs() * dx;
    }
    // Side walls.
    for row in 0..rows {
        let e = (potential[idx(0, row)] - potential[idx(1, row)]) / dx;
        flux += eps_at_row(row) * e.abs() * dy;
        let e = (potential[idx(cols - 1, row)] - potential[idx(cols - 2, row)]) / dx;
        flux += eps_at_row(row) * e.abs() * dy;
    }
    flux
}

// ---------------------------------------------------------------------------
// Radiation patterns
// ---------------------------------------------------------------------------

/// Which antenna cut to sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternKind {
    /// Half-wave dipole, exact E-plane cut.
    Dipole,
    /// Quarter-wave monopole (dipole pattern over the ground half-space).
    Monopole,
    /// Patch E-plane (contains the length axis): two-slot array factor.
    PatchEPlane,
    /// Patch H-plane (contains the width axis): slot element pattern.
    PatchHPlane,
}

/// One sampled far-field cut, normalized to 1 at peak.
#[derive(Debug, Clone)]
pub struct PatternCurve {
    /// Angle from broadside (z-axis), degrees, 0…180 (monopole: 0…90).
    pub angle_deg: Vec<f64>,
    /// Field intensity normalized to the peak.
    pub normalized: Vec<f64>,
    /// −3 dB beamwidth computed from the samples, degrees (0 if unresolved).
    pub beamwidth_deg: f64,
    pub label: String,
}

/// Samples the requested far-field cut (1° steps).
pub fn radiation_pattern(kind: PatternKind, electrical_length: f64) -> PatternCurve {
    let max_angle = match kind {
        PatternKind::Monopole => 90.0f64,
        _ => 180.0,
    };
    let n = max_angle as usize + 1;
    let mut angle_deg = Vec::with_capacity(n);
    let mut normalized = Vec::with_capacity(n);

    for deg in 0..=max_angle as usize {
        let theta = deg as f64;
        let rad = theta.to_radians();
        let value = match kind {
            PatternKind::Dipole | PatternKind::Monopole => {
                // Thin-wire E-plane: [cos((kl/2)cosθ) − cos(kl/2)]/sinθ,
                // kl/2 = electrical_length (in radians of phase).
                let s = rad.sin();
                if s < 1e-6 {
                    1e-9
                } else {
                    ((electrical_length * rad.cos()).cos() - electrical_length.cos()) / s
                }
            }
            PatternKind::PatchEPlane => {
                // Two-slot array factor along L (slots λ/2·√εeff apart in
                // effect): |cos((kL/2)·sinθ)| with kL/2 passed in.
                (electrical_length * rad.sin()).cos().abs()
            }
            PatternKind::PatchHPlane => {
                // Slot element pattern (uniform magnetic line source of
                // length W over the ground): |sinc((kW/2)·sinθ)|·cosθ —
                // broadside peak, horizon null from the ground plane.
                let arg = electrical_length * rad.sin();
                let sinc = if arg.abs() < 1e-9 {
                    1.0
                } else {
                    arg.sin() / arg
                };
                sinc.abs() * rad.cos()
            }
        };
        angle_deg.push(theta);
        normalized.push(value);
    }

    let peak = normalized.iter().cloned().fold(0.0f64, f64::max).max(1e-12);
    for v in &mut normalized {
        *v /= peak;
    }
    let beamwidth = beamwidth_from(&angle_deg, &normalized);
    PatternCurve {
        angle_deg,
        normalized,
        beamwidth_deg: beamwidth,
        label: match kind {
            PatternKind::Dipole => "Dipole E-plane (exact thin-wire cut)".to_string(),
            PatternKind::Monopole => "Monopole elevation (ground half-space)".to_string(),
            PatternKind::PatchEPlane => "Patch E-plane (two-slot array factor)".to_string(),
            PatternKind::PatchHPlane => "Patch H-plane (slot element pattern)".to_string(),
        },
    }
}

/// −3 dB width from sampled cut: widest contiguous span above 1/√2.
fn beamwidth_from(angle_deg: &[f64], normalized: &[f64]) -> f64 {
    let threshold = std::f64::consts::FRAC_1_SQRT_2;
    let mut first: Option<usize> = None;
    let mut last: Option<usize> = None;
    for (i, v) in normalized.iter().enumerate() {
        if *v >= threshold {
            if first.is_none() {
                first = Some(i);
            }
            last = Some(i);
        }
    }
    match (first, last) {
        (Some(a), Some(b)) => angle_deg[b] - angle_deg[a],
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests;
