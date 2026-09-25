//! 2D field visualization panel (Pro edition).
//!
//! Main-column card holding the cross-section solve controls, the canvas
//! placeholder that [`wire`] fills with a raw `<canvas>` redrawn from an
//! effect on the field outcome, and the reactive field readouts.
//!
//! The solve runs synchronously on the UI thread — the SOR sweep on the
//! grids offered here lands well under a second in wasm release builds,
//! so no worker plumbing.

use std::rc::Rc;

use tpt_appfront_core::{create_effect, Signal, UITree};
use tpt_em_design_engine::field::{solve_microstrip_field, FieldOptions, FieldResult};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::{fmt_g, label, num, read_input, styled_text, Msg};
use crate::viewer;

/// The field section's current content.
#[derive(Clone)]
pub enum FieldOutcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<FieldResult>),
}

/// Main-column card: controls + viewer + readouts.
pub fn main_card(main: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    main.container(|card| {
        card.heading(3, "2D field viewer (Pro)");
        card.text(
            "Solves the microstrip cross-section above (width, h, \u{3b5}r) with \
             SOR finite differences in a grounded shield box, then draws the \
             potential with equipotential lines \u{2014} or |E| \u{2014} as a heatmap."
        )
        .class("emd-hint");
        card.container(|row| {
            row.container(|field| {
                label(field, "Grid");
                field
                    .select(
                        [
                            ("160", "Coarse (fast)"),
                            ("240", "Medium"),
                            ("320", "Fine (slowest)"),
                        ],
                        "240",
                    )
                    .class("emd-input emd-field-grid");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Show");
                field
                    .select(
                        [
                            ("potential", "Potential + equipotentials"),
                            ("e", "|E| magnitude"),
                        ],
                        "potential",
                    )
                    .class("emd-input emd-field-quantity")
                    .on_input(|_| Msg::SolveField);
            })
            .class("emd-field");
            row.container(|actions| {
                actions
                    .button("Solve field")
                    .class("emd-btn emd-primary")
                    .on_click(Msg::SolveField);
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|_| {}).class("emd-viewer emd-field-viewer");
        card.container(|legend| {
            legend.container(|_| {}).class("emd-legend-bar emd-legend-gradient");
            legend.text("0 V \u{2192} 1 V (trace)");
        })
        .class("emd-legend");
        card.container(|_| {}).class("emd-field-results");
    })
    .class("emd-card");
}

/// Mounts the canvas + reactive readouts; called once from
/// [`crate::app::mount_app`].
pub fn wire(
    container: &web_sys::Element,
    field_outcome: &Signal<FieldOutcome>,
) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let holder = container
        .query_selector(".emd-field-viewer")?
        .ok_or_else(|| JsValue::from_str("field viewer placeholder missing"))?;
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")?
        .dyn_into()
        .map_err(|_| JsValue::from_str("canvas cast failed"))?;
    canvas.set_width(960);
    canvas.set_height(560);
    holder.append_child(&canvas)?;

    // "Show" select re-solves so the redraw stays a pure function of the
    // outcome signal (cheap enough on these grids, keeps one code path).
    let results_el = container
        .query_selector(".emd-field-results")?
        .ok_or_else(|| JsValue::from_str("field results placeholder missing"))?;
    let outcome_for_render = field_outcome.clone();
    // Field outcomes only change through run_field (dispatched by the app
    // root), so this section-local dispatcher is a no-op.
    let dispatch: Rc<dyn Fn(Msg)> = Rc::new(|_msg: Msg| {});
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || field_view(outcome_for_render.clone())),
        dispatch,
    )?;
    std::mem::forget(handle);

    let container_for_css = container.clone();
    let canvas_for_draw = canvas.clone();
    let outcome_for_draw = field_outcome.clone();
    let handle = create_effect(move || {
        if let FieldOutcome::Solved(field) = outcome_for_draw.get() {
            let fg = crate::app::read_css(&container_for_css, "--emd-fg", "#1f2430");
            let muted = crate::app::read_css(&container_for_css, "--emd-muted", "#667085");
            let _ = viewer::draw_field(&canvas_for_draw, &field, &fg, &muted);
        }
    });
    std::mem::forget(handle);

    Ok(())
}

/// Reads the trace form (width/h/εr — shared with the impedance calculator)
/// plus the grid select and runs the cross-section solve.
pub fn run_field(container: &web_sys::Element, outcome: &Signal<FieldOutcome>) {
    let mut issues = Vec::new();
    let kind = read_input(container, ".emd-kind");
    let width = num(container, ".emd-width", "Trace width", &mut issues);
    let height = num(container, ".emd-height", "Dielectric height", &mut issues);
    let er = num(container, ".emd-er", "Dielectric constant", &mut issues);
    let grid = read_input(container, ".emd-field-grid")
        .parse::<usize>()
        .unwrap_or(240);
    if !issues.is_empty() {
        outcome.set(FieldOutcome::Invalid(issues));
        return;
    }
    if kind != "microstrip" {
        outcome.set(FieldOutcome::Invalid(vec![
            "The 2D field viewer solves the microstrip stackup \u{2014} switch \
             Stackup to Microstrip first (stripline cross-sections are on the \
             roadmap)."
                .to_string(),
        ]));
        return;
    }
    let opts = FieldOptions {
        width_mm: width,
        height_mm: height,
        er,
        grid_cols: grid,
        ..FieldOptions::default()
    };
    match solve_microstrip_field(&opts) {
        Ok(field) => outcome.set(FieldOutcome::Solved(Rc::new(field))),
        Err(error) => outcome.set(FieldOutcome::Invalid(error.0)),
    }
}

fn field_view(outcome: Signal<FieldOutcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let mut builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    builder.container(|c| match &outcome {
        FieldOutcome::Idle => {
            styled_text(c, "Press Solve field to compute the cross-section.", "emd-hint");
        }
        FieldOutcome::Invalid(issues) => {
            c.container(|issues_view| {
                for issue in issues {
                    styled_text(issues_view, issue.clone(), "emd-issue");
                }
            })
            .class("emd-issues");
        }
        FieldOutcome::Solved(field) => {
            c.container(|summary| {
                styled_text(
                    summary,
                    format!(
                        "Field-solver cross-check: Z\u{2080} \u{2248} {} \u{3a9} \u{b7} C\u{2032} = {} pF/mm",
                        fmt_g(field.z0_field_estimate),
                        fmt_g(field.c_pf_per_mm)
                    ),
                    "emd-headline",
                );
                styled_text(
                    summary,
                    format!(
                        "{} \u{d7} {} grid \u{b7} {} SOR iterations{}",
                        field.cols,
                        field.rows,
                        field.iterations,
                        if field.converged { "" } else { " (hit the cap)" }
                    ),
                    "emd-detail",
                );
                styled_text(
                    summary,
                    "The Z\u{2080} here comes from the field solve alone (Gauss-law \
                     capacitance, two permittivity runs) \u{2014} an independent check \
                     on the closed-form calculator above; a few percent of grid \
                     discretization is expected.",
                    "emd-engine",
                );
            })
            .class("emd-summary");
        }
    });
    builder
        .into_only_child()
        .expect("field view builds exactly one root container")
}
