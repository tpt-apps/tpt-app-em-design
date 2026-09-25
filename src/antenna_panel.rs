//! Antenna design + matching panel (Pro edition).
//!
//! Sidebar: the antenna form card. Main: one card holding the reactive
//! antenna-results section, the matching form + results, and the
//! radiation-pattern canvas placeholder that [`wire`] fills with a raw
//! `<canvas>` redrawn from an effect on the antenna outcome.

use std::rc::Rc;

use tpt_appfront_core::{create_effect, Signal, UITree};
use tpt_em_design_engine::antenna::{
    design, design_match, AntennaInput, AntennaKind, MatchInput, MatchKind, PatchInput,
    WireAntennaInput,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::{fmt_g, label, num, read_input, styled_text, Msg};
use crate::viewer;

/// The antenna results section's current content.
#[derive(Clone)]
pub enum AntennaOutcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<tpt_em_design_engine::antenna::AntennaResult>),
}

/// The matching results section's current content.
#[derive(Clone)]
pub enum MatchOutcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<tpt_em_design_engine::antenna::MatchResult>),
}

/// Sidebar form card.
pub fn sidebar_cards(sidebar: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Antenna design (Pro)");
        card.container(|row| {
            row.container(|field| {
                label(field, "Antenna");
                field
                    .select(
                        [
                            ("dipole", "Half-wave dipole"),
                            ("monopole", "Quarter-wave monopole"),
                            ("patch", "Microstrip patch"),
                        ],
                        "dipole",
                    )
                    .class("emd-input emd-ant-type");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Frequency (MHz)");
                field.input("2400").class("emd-input emd-ant-freq");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Element diameter (mm)");
                field.input("2.0").class("emd-input emd-ant-diam");
            })
            .class("emd-field");
        })
        .class("emd-row emd-ant-wire");
        card.container(|row| {
            row.container(|field| {
                label(field, "Board \u{3b5}r (patch)");
                field.input("4.4").class("emd-input emd-ant-er");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Board height h (mm)");
                field.input("1.588").class("emd-input emd-ant-h");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Feed impedance (\u{3a9})");
                field.input("50").class("emd-input emd-ant-feed");
            })
            .class("emd-field");
        })
        .class("emd-row emd-ant-patch");
        card.text(
            "Dipole/monopole use the element diameter for the end-effect \
             shortening; patch fields are ignored for wires, and the diameter \
             is ignored for patches."
        )
        .class("emd-hint");
        card.container(|actions| {
            actions
                .button("Design antenna")
                .class("emd-btn emd-primary")
                .on_click(Msg::DesignAntenna);
        })
        .class("emd-actions");
    })
    .class("emd-card");
}

/// Main-column card: results, matching, pattern viewer placeholders.
pub fn main_card(main: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    main.container(|card| {
        card.heading(3, "Antenna & matching (Pro)");
        card.container(|_| {}).class("emd-ant-results");

        card.container(|row| {
            row.container(|field| {
                label(field, "Matching network");
                field
                    .select(
                        [
                            ("quarter", "Quarter-wave transformer"),
                            ("l", "L network (2 element)"),
                            ("stub", "Single shunt stub"),
                        ],
                        "quarter",
                    )
                    .class("emd-input emd-match-kind");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Source Z (\u{3a9})");
                field.input("50").class("emd-input emd-match-z");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Load R (\u{3a9})");
                field.input("73").class("emd-input emd-match-load");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Frequency (MHz)");
                field.input("2400").class("emd-input emd-match-freq");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Line \u{3b5}eff");
                field.input("1.0").class("emd-input emd-match-eps");
            })
            .class("emd-field");
            row.container(|actions| {
                actions
                    .button("Design matching")
                    .class("emd-btn")
                    .on_click(Msg::DesignMatch);
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|_| {}).class("emd-match-results");

        card.heading(3, "Radiation pattern (2D cut)");
        card.container(|_| {})
            .class("emd-viewer emd-pattern-viewer");
        card.text(
            "Normalized field intensity vs angle from broadside. Wire cuts are \
             the exact thin-antenna pattern; patch cuts are the standard \
             array-factor/element-pattern approximations."
        )
        .class("emd-hint");
    })
    .class("emd-card");
}

/// Mounts the reactive sections and the pattern canvas; called once from
/// [`crate::app::mount_app`].
pub fn wire(
    container: &web_sys::Element,
    antenna_outcome: &Signal<AntennaOutcome>,
    match_outcome: &Signal<MatchOutcome>,
    dispatch: &Rc<dyn Fn(Msg)>,
) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let ant_el = container
        .query_selector(".emd-ant-results")?
        .ok_or_else(|| JsValue::from_str("antenna results placeholder missing"))?;
    let outcome_for_render = antenna_outcome.clone();
    let handle = tpt_appfront_dom::render(
        &ant_el,
        Rc::new(move || antenna_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    let match_el = container
        .query_selector(".emd-match-results")?
        .ok_or_else(|| JsValue::from_str("match results placeholder missing"))?;
    let match_for_render = match_outcome.clone();
    let handle = tpt_appfront_dom::render(
        &match_el,
        Rc::new(move || match_view(match_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // Pattern canvas + redraw effect.
    let holder = container
        .query_selector(".emd-pattern-viewer")?
        .ok_or_else(|| JsValue::from_str("pattern viewer placeholder missing"))?;
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")?
        .dyn_into()
        .map_err(|_| JsValue::from_str("canvas cast failed"))?;
    canvas.set_width(960);
    canvas.set_height(480);
    holder.append_child(&canvas)?;

    let container_for_css = container.clone();
    let canvas_for_draw = canvas.clone();
    let outcome_for_draw = antenna_outcome.clone();
    let handle = create_effect(move || {
        if let AntennaOutcome::Solved(result) = outcome_for_draw.get() {
            let fg = crate::app::read_css(&container_for_css, "--emd-fg", "#1f2430");
            let muted = crate::app::read_css(&container_for_css, "--emd-muted", "#667085");
            let accent = crate::app::read_css(&container_for_css, "--emd-accent", "#4f46e5");
            let curves = pattern_curves(&result);
            let _ = viewer::draw_pattern(&canvas_for_draw, &curves, &fg, &muted, &accent);
        }
    });
    std::mem::forget(handle);

    Ok(())
}

/// Which pattern cut(s) to draw for a designed antenna.
fn pattern_curves(
    result: &tpt_em_design_engine::antenna::AntennaResult,
) -> Vec<tpt_em_design_engine::field::PatternCurve> {
    use tpt_em_design_engine::antenna::AntennaKind;
    use tpt_em_design_engine::field::{radiation_pattern, PatternKind};
    match result.kind {
        AntennaKind::Patch => {
            let l_half = std::f64::consts::PI * result.length_mm / result.wavelength_mm;
            let w_half = std::f64::consts::PI * result.width_mm / result.wavelength_mm;
            vec![
                radiation_pattern(PatternKind::PatchEPlane, l_half),
                radiation_pattern(PatternKind::PatchHPlane, w_half),
            ]
        }
        kind => {
            // kl/2 for the designed (end-effect shortened) element.
            let electrical = std::f64::consts::PI * result.length_mm / result.wavelength_mm;
            vec![radiation_pattern(
                if kind == AntennaKind::Monopole {
                    PatternKind::Monopole
                } else {
                    PatternKind::Dipole
                },
                electrical,
            )]
        }
    }
}

/// Reads the antenna form and runs the engine design.
pub fn run_design(container: &web_sys::Element, outcome: &Signal<AntennaOutcome>) {
    let mut issues = Vec::new();
    let kind_text = read_input(container, ".emd-ant-type");
    let freq = num(container, ".emd-ant-freq", "Frequency", &mut issues);
    let diam = num(container, ".emd-ant-diam", "Element diameter", &mut issues);
    let er = num(container, ".emd-ant-er", "Board \u{3b5}r", &mut issues);
    let h = num(container, ".emd-ant-h", "Board height", &mut issues);
    let feed = num(container, ".emd-ant-feed", "Feed impedance", &mut issues);
    if !issues.is_empty() {
        outcome.set(AntennaOutcome::Invalid(issues));
        return;
    }
    let input = match kind_text.as_str() {
        "patch" => AntennaInput::Patch(PatchInput {
            freq_mhz: freq,
            er,
            height_mm: h,
            feed_z: feed,
        }),
        kind => AntennaInput::Wire(WireAntennaInput {
            kind: if kind == "monopole" {
                AntennaKind::Monopole
            } else {
                AntennaKind::Dipole
            },
            freq_mhz: freq,
            element_diameter_mm: diam,
        }),
    };
    match design(&input) {
        Ok(result) => outcome.set(AntennaOutcome::Solved(Rc::new(result))),
        Err(error) => outcome.set(AntennaOutcome::Invalid(error.0)),
    }
}

/// Reads the matching form and synthesizes the network.
pub fn run_match(container: &web_sys::Element, outcome: &Signal<MatchOutcome>) {
    let mut issues = Vec::new();
    let kind_text = read_input(container, ".emd-match-kind");
    let z = num(container, ".emd-match-z", "Source Z", &mut issues);
    let load = num(container, ".emd-match-load", "Load R", &mut issues);
    let freq = num(container, ".emd-match-freq", "Frequency", &mut issues);
    let eps = num(container, ".emd-match-eps", "Line \u{3b5}eff", &mut issues);
    if !issues.is_empty() {
        outcome.set(MatchOutcome::Invalid(issues));
        return;
    }
    let input = MatchInput {
        kind: match kind_text.as_str() {
            "l" => MatchKind::LNetwork,
            "stub" => MatchKind::SingleStub,
            _ => MatchKind::QuarterWave,
        },
        source_z: z,
        load_r: load,
        freq_mhz: freq,
        eps_eff: eps,
    };
    match design_match(&input) {
        Ok(result) => outcome.set(MatchOutcome::Solved(Rc::new(result))),
        Err(error) => outcome.set(MatchOutcome::Invalid(error.0)),
    }
}

fn antenna_view(outcome: Signal<AntennaOutcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let mut builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    builder.container(|c| match &outcome {
        AntennaOutcome::Idle => {
            styled_text(c, "Pick an antenna and press Design.", "emd-hint");
        }
        AntennaOutcome::Invalid(issues) => {
            c.container(|issues_view| {
                for issue in issues {
                    styled_text(issues_view, issue.clone(), "emd-issue");
                }
            })
            .class("emd-issues");
        }
        AntennaOutcome::Solved(r) => {
            c.container(|summary| {
                let headline = if r.width_mm > 0.0 {
                    format!(
                        "Patch: L = {} mm \u{b7} W = {} mm \u{b7} inset {} mm",
                        fmt_g(r.length_mm),
                        fmt_g(r.width_mm),
                        fmt_g(r.inset_mm)
                    )
                } else {
                    format!("Element length: {} mm", fmt_g(r.length_mm))
                };
                styled_text(summary, headline, "emd-headline");
                styled_text(
                    summary,
                    format!(
                        "\u{3bb}\u{2080} = {} mm \u{b7} feed \u{2248} {} \u{3a9} \u{b7} gain \u{2248} {} dBi",
                        fmt_g(r.wavelength_mm),
                        fmt_g(r.feed_impedance),
                        fmt_g(r.gain_dbi)
                    ),
                    "emd-detail",
                );
            })
            .class("emd-summary");
            let mut rows = Vec::new();
            if r.width_mm > 0.0 {
                rows.push(vec![
                    "Patch width W".to_string(),
                    format!("{} mm", fmt_g(r.width_mm)),
                    "c/(2f)\u{b7}\u{221a}(2/(\u{3b5}r+1))".to_string(),
                ]);
                rows.push(vec![
                    "Patch length L".to_string(),
                    format!("{} mm", fmt_g(r.length_mm)),
                    "resonant, fringing-corrected".to_string(),
                ]);
                rows.push(vec![
                    "Inset feed depth".to_string(),
                    format!("{} mm", fmt_g(r.inset_mm)),
                    "from a radiating edge".to_string(),
                ]);
            } else {
                rows.push(vec![
                    "Element length".to_string(),
                    format!("{} mm", fmt_g(r.length_mm)),
                    "end-effect shortened".to_string(),
                ]);
                rows.push(vec![
                    "Feed impedance".to_string(),
                    format!("\u{2248}{} \u{3a9}", fmt_g(r.feed_impedance)),
                    "at exact resonance".to_string(),
                ]);
                rows.push(vec![
                    "\u{2212}3 dB beamwidth".to_string(),
                    format!("{} \u{b0}", fmt_g(r.beamwidth_deg)),
                    "textbook value for this class".to_string(),
                ]);
            }
            rows.push(vec![
                "Free-space wavelength".to_string(),
                format!("{} mm", fmt_g(r.wavelength_mm)),
                "at the design frequency".to_string(),
            ]);
            c.data_grid(["Quantity", "Value", "Detail"], rows);
            c.container(|notes| {
                for note in &r.notes {
                    styled_text(notes, format!("\u{26a0} {note}"), "emd-detail");
                }
            })
            .class("emd-notes");
        }
    });
    builder
        .into_only_child()
        .expect("antenna view builds exactly one root container")
}

fn match_view(outcome: Signal<MatchOutcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let mut builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    builder.container(|c| match &outcome {
        MatchOutcome::Idle => {
            styled_text(c, "Matching synthesis fills in here.", "emd-hint");
        }
        MatchOutcome::Invalid(issues) => {
            c.container(|issues_view| {
                for issue in issues {
                    styled_text(issues_view, issue.clone(), "emd-issue");
                }
            })
            .class("emd-issues");
        }
        MatchOutcome::Solved(r) => {
            c.container(|summary| {
                styled_text(summary, "Matching network synthesized", "emd-headline");
            })
            .class("emd-summary");
            c.container(|lines| {
                for line in &r.summary {
                    styled_text(lines, line.clone(), "emd-detail");
                }
            })
            .class("emd-match-summary");
            c.container(|notes| {
                for note in &r.notes {
                    styled_text(notes, format!("\u{26a0} {note}"), "emd-detail");
                }
            })
            .class("emd-notes");
        }
    });
    builder
        .into_only_child()
        .expect("match view builds exactly one root container")
}
