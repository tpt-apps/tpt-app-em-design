//! tpt-em-design-engine — the analysis core of TPT EM Design.
//!
//! Closed-form electromagnetics for everyday PCB and RF work, implemented
//! here directly rather than via a dependency: the intended base crate
//! `tpt-electromagnetics` has no implemented crates yet (empty workspace —
//! surveyed 2026-09), and its spec explicitly defers PCB/antenna
//! calculators to application code. Everything is plain `std` Rust with no
//! wasm/UI dependencies, so the accuracy bounds are host-testable:
//! `cargo test -p tpt-em-design-engine` runs the validation cases before
//! any browser is involved.
//!
//! Free tier (default features):
//! - [`trace`]: microstrip and symmetric-stripline characteristic impedance
//!   (Hammerstad–Jensen 1980 with a Garg–Bahl finite-thickness correction;
//!   stripline via the exact zero-thickness conformal-mapping result with a
//!   first-order thickness correction), effective permittivity, delay,
//!   distributed L and C.
//!
//! Pro tier (`pro` feature):
//! - [`antenna`]: dipole/monopole/patch dimensioning and matching
//!   (quarter-wave transformer, L-network, single-stub).
//! - [`field`]: 2D quasi-static Laplace field solve for a microstrip
//!   cross-section (SOR finite differences, dielectric interface handled by
//!   permittivity-weighted stencils) plus 2D radiation-pattern sampling.

pub mod trace;

#[cfg(feature = "pro")]
pub mod antenna;
#[cfg(feature = "pro")]
pub mod field;

pub use trace::{MicrostripInput, StriplineInput, TraceInput, TraceResult};
