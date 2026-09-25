//! Free WASM edition UI (tpt-appfront-dom).
//!
//! Rendering architecture (mirrors the fea-lite/cutlist-optimizer split):
//! - the **input panel is mounted once** (`tpt_appfront_dom::mount`) — typing
//!   never loses focus; field values are read straight from the DOM when
//!   Calculate is pressed;
//! - the **results section re-renders fine-grained** via
//!   `tpt_appfront_dom::render` driven by the `outcome` Signal;
//! - the (pro) **field viewer** is a raw web-sys `<canvas>` appended into a
//!   placeholder container and redrawn from an effect that watches the
//!   field outcome signals.
//!
//! Units are the ones PCB fabs quote: mm for geometry, relative
//! permittivity, µm copper, GHz.

use std::rc::Rc;

use tpt_appfront_core::{Signal, UITree};
use tpt_em_design_engine::{
    copper_thickness, solve, MicrostripInput, StriplineInput, TraceInput, TraceResult,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The results section's current content.
#[derive(Clone)]
pub(crate) enum Outcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<Solved>),
}

/// One solved trace plus the label the UI shows for it.
pub(crate) struct Solved {
    label: String,
    result: TraceResult,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Calculate,
    Sample(usize),
    /// Trace geometry kind changed — show/hide the height vs spacing field.
    KindChanged(String),
    /// Open a TPT Solutions URL — in a new browser tab for the in-browser
    /// free edition, or via the desktop host's system-browser IPC action for
    /// the Pro desktop shell. See [`visit_site`].
    VisitSite(String),
    /// (pro) design an antenna from the form.
    #[cfg(feature = "pro")]
    DesignAntenna,
    /// (pro) solve the 2D microstrip cross-section field.
    #[cfg(feature = "pro")]
    SolveField,
    /// (pro) synthesize the selected matching network.
    #[cfg(feature = "pro")]
    DesignMatch,
    /// (pro) save the current inputs as a named project.
    #[cfg(feature = "pro")]
    SaveProject,
    /// (pro) load saved project `name`.
    #[cfg(feature = "pro")]
    LoadProject(String),
    /// (pro) delete saved project `name`.
    #[cfg(feature = "pro")]
    DeleteProject(String),
}

/// Built-in sample problems: (label, applied by [`apply_sample`]).
const SAMPLES: [&str; 3] = [
    "50 \u{3a9} microstrip \u{b7} 1.6 mm FR-4",
    "50 \u{3a9} stripline \u{b7} 1.2 mm planes",
    "Wide power trace \u{b7} 0.8 mm FR-4",
];

const BASE_CSS: &str = r#"
.emd-app{--emd-bg:#f7f8fa;--emd-surface:#ffffff;--emd-panel:#f2f4f7;--emd-fg:#181d27;--emd-muted:#667085;--emd-border:#e3e7ee;--emd-accent:#4f46e5;--emd-accent-2:#6d28d9;--emd-accent-soft:#eef0fe;--emd-accent-fg:#ffffff;--emd-err:#b3261e;--emd-err-bg:#fbeeec;--emd-grid:rgba(20,30,45,.045);--emd-shadow:0 1px 2px rgba(16,24,40,.04),0 1px 3px rgba(16,24,40,.06);--emd-shadow-lg:0 8px 24px rgba(16,24,40,.08),0 2px 6px rgba(16,24,40,.05);max-width:1360px;margin:0 auto;background:var(--emd-bg);color:var(--emd-fg);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;font-size:14.5px;line-height:1.55;padding:0 4px 32px}
@media(prefers-color-scheme:dark){.emd-app{--emd-bg:#0b0f16;--emd-surface:#141a24;--emd-panel:#161d2a;--emd-fg:#e8ebf0;--emd-muted:#98a2b3;--emd-border:#262f3d;--emd-accent:#818cf8;--emd-accent-2:#a78bfa;--emd-accent-soft:#1e1b4b;--emd-accent-fg:#0b0f16;--emd-err:#f2b8b5;--emd-err-bg:#2c1a1c;--emd-grid:rgba(255,255,255,.045);--emd-shadow:0 1px 2px rgba(0,0,0,.3);--emd-shadow-lg:0 12px 28px rgba(0,0,0,.45)}}
.emd-app *{box-sizing:border-box}
.emd-app h2{font-size:1.36rem;margin:0;font-weight:700;letter-spacing:-.01em}
.emd-app h3{font-size:1rem;margin:0 0 6px;font-weight:700}

.emd-header{display:flex;align-items:flex-start;gap:14px;padding:22px 2px 18px;border-bottom:1px solid var(--emd-border);margin-bottom:22px}
.emd-brand{flex:0 0 auto;width:42px;height:42px;border-radius:11px;background:linear-gradient(135deg,var(--emd-accent),var(--emd-accent-2));display:flex;align-items:center;justify-content:center;color:var(--emd-accent-fg);font-weight:800;font-size:.82rem;letter-spacing:.01em;box-shadow:var(--emd-shadow)}
.emd-title-group{flex:1;min-width:0}
.emd-title-row{display:flex;align-items:center;gap:10px;flex-wrap:wrap}
.emd-chip{display:inline-flex;align-items:center;padding:3px 10px;border-radius:999px;font-size:.66rem;font-weight:800;letter-spacing:.05em;text-transform:uppercase;background:var(--emd-accent-soft);color:var(--emd-accent)}
.emd-sub{color:var(--emd-muted);margin:5px 0 0;font-size:.88rem}

.emd-shell{display:grid;grid-template-columns:minmax(320px,380px) 1fr;gap:22px;align-items:start}
@media(max-width:900px){.emd-shell{grid-template-columns:1fr}}
.emd-sidebar{display:flex;flex-direction:column;gap:16px;position:sticky;top:16px;max-height:calc(100vh - 32px);overflow-y:auto;padding:2px 4px 12px 2px}
@media(max-width:900px){.emd-sidebar{position:static;max-height:none;overflow:visible}}
.emd-main{display:flex;flex-direction:column;gap:16px;min-width:0}

.emd-card{background:var(--emd-surface);border:1px solid var(--emd-border);border-radius:14px;padding:16px 18px;box-shadow:var(--emd-shadow)}
.emd-hint{color:var(--emd-muted);font-size:.8rem;margin:0 0 10px}
.emd-row{display:flex;gap:12px;flex-wrap:wrap;margin-top:10px}
.emd-field{display:flex;flex-direction:column;gap:5px}
.emd-label{font-size:.72rem;color:var(--emd-muted);font-weight:600;text-transform:uppercase;letter-spacing:.03em}
.emd-input{width:110px;padding:7px 9px;border:1px solid var(--emd-border);border-radius:8px;background:var(--emd-bg);color:var(--emd-fg);font:inherit;transition:border-color .12s,box-shadow .12s}
select.emd-input{width:100%}
.emd-input:focus{outline:none;border-color:var(--emd-accent);box-shadow:0 0 0 3px var(--emd-accent-soft)}

.emd-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:14px;align-items:center}
.emd-actions-secondary{margin-top:8px;padding-top:10px;border-top:1px dashed var(--emd-border)}
.emd-btn{padding:9px 16px;border-radius:9px;border:1px solid var(--emd-border);background:var(--emd-surface);color:var(--emd-fg);font:inherit;font-weight:600;cursor:pointer;transition:border-color .12s,color .12s,transform .12s}
.emd-btn:hover{border-color:var(--emd-accent);color:var(--emd-accent);transform:translateY(-1px)}
.emd-primary{background:linear-gradient(135deg,var(--emd-accent),var(--emd-accent-2));border-color:transparent;color:var(--emd-accent-fg);box-shadow:var(--emd-shadow)}
.emd-primary:hover{color:var(--emd-accent-fg);filter:brightness(1.07)}
.emd-actions-secondary .emd-btn{padding:6px 13px;font-size:.8rem;font-weight:500;border-radius:999px;background:transparent}

.emd-z0{font-size:2.1rem;font-weight:800;letter-spacing:-.02em;color:var(--emd-fg);font-variant-numeric:tabular-nums}
.emd-z0 small{font-size:1rem;font-weight:600;color:var(--emd-muted)}

.emd-results-view{margin:4px 0 0}
.emd-summary{margin:0 0 14px;padding:14px 16px;background:var(--emd-accent-soft);border-radius:10px}
.emd-headline{font-weight:700;font-size:1.05rem;margin:0 0 4px;color:var(--emd-fg)}
.emd-detail{color:var(--emd-muted);font-size:.83rem;margin:0}
.emd-engine{color:var(--emd-muted);font-size:.8rem;margin:6px 0 0;font-style:italic}
.emd-issues:empty{display:none}
.emd-issues{margin:0 0 12px;padding:10px 12px;background:var(--emd-err-bg);border:1px solid var(--emd-err);border-radius:10px}
.emd-issue{color:var(--emd-err);font-size:.88rem;margin:2px 0}
.emd-app table{width:100%;border-collapse:collapse;font-size:.85rem;margin-top:4px}
.emd-app th{text-align:left;padding:8px;color:var(--emd-muted);font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.03em;border-bottom:2px solid var(--emd-border)}
.emd-app td{text-align:left;padding:8px;border-bottom:1px solid var(--emd-border)}
.emd-app tbody tr:hover{background:var(--emd-panel)}

.emd-viewer{position:relative;border-radius:12px;overflow:hidden;border:1px solid var(--emd-border)}
.emd-viewer canvas{width:100%;height:auto;display:block;background-color:var(--emd-panel)}
.emd-legend-gradient{background:linear-gradient(90deg,#0a0728,#3c0f6e,#821e78,#c83755,#f5821e,#faeb8c)}
.emd-legend{display:flex;align-items:center;gap:10px;margin-top:10px;font-size:.82rem;color:var(--emd-muted)}
.emd-legend-bar{flex:0 0 140px;height:10px;border-radius:5px;border:1px solid var(--emd-border)}

.emd-upsell-list{margin:6px 0 10px}
.emd-li{position:relative;padding-left:16px;margin:4px 0}
.emd-li::before{content:"\2022";position:absolute;left:2px;color:var(--emd-accent);font-weight:800}
.emd-upsell{border-radius:14px;padding:16px 18px;background:linear-gradient(135deg,var(--emd-accent-soft),transparent);border:1px dashed var(--emd-accent);color:var(--emd-fg);font-size:.88rem}
.emd-upsell h3{margin:0 0 6px}
.emd-upsell ul{margin:6px 0 10px;padding-left:20px}
.emd-upsell li{margin:3px 0}
.emd-upsell a{color:var(--emd-accent);font-weight:700;text-decoration:none}
.emd-upsell a:hover{text-decoration:underline}

.emd-footer{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:10px;margin-top:26px;padding-top:16px;border-top:1px solid var(--emd-border);color:var(--emd-muted);font-size:.82rem}
.emd-footer-brand{font-weight:700;color:var(--emd-fg)}
.emd-footer-links{display:flex;flex-wrap:wrap;gap:4px 16px}
.emd-footer-link{padding:0;border:none;background:none;color:var(--emd-accent);font:inherit;font-weight:600;cursor:pointer}
.emd-footer-link:hover{text-decoration:underline}
"#;

/// Default field values = sample 0 (50 Ω microstrip), so the first press of
/// Calculate works with no edits.
mod defaults {
    pub const WIDTH: &str = "3.0";
    pub const HEIGHT: &str = "1.6";
    pub const SPACING: &str = "1.2";
    pub const ER: &str = "4.6";
    pub const TAND: &str = "0.02";
    pub const FREQ: &str = "1.0";
}

/// Mounts the whole app into `container` (the hub's container div, or the
/// standalone `#tpt-appfront-root` marker).
pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    upsert_style(&document, "em-design-styles", BASE_CSS);
    // `.emd-app` on the mount root itself: every color/font/shadow custom
    // property lives here, so the whole theme depends on this class being
    // present on an ancestor of the tree the CSS above targets.
    let existing_class = container.get_attribute("class").unwrap_or_default();
    if !existing_class.split_whitespace().any(|c| c == "emd-app") {
        let combined = if existing_class.is_empty() {
            "emd-app".to_string()
        } else {
            format!("{existing_class} emd-app")
        };
        let _ = container.set_attribute("class", &combined);
    }

    let outcome: Signal<Outcome> = Signal::new(Outcome::Idle);
    #[cfg(feature = "pro")]
    let antenna_outcome: Signal<super::antenna_panel::AntennaOutcome> =
        Signal::new(super::antenna_panel::AntennaOutcome::Idle);
    #[cfg(feature = "pro")]
    let field_outcome: Signal<super::field_panel::FieldOutcome> =
        Signal::new(super::field_panel::FieldOutcome::Idle);
    #[cfg(feature = "pro")]
    let match_outcome: Signal<super::antenna_panel::MatchOutcome> =
        Signal::new(super::antenna_panel::MatchOutcome::Idle);
    #[cfg(feature = "pro")]
    let projects: Signal<Rc<Vec<super::projects::ProjectMeta>>> =
        Signal::new(Rc::new(super::projects::list()));

    let dispatch: Rc<dyn Fn(Msg)> = {
        let container = container.clone();
        let outcome = outcome.clone();
        #[cfg(feature = "pro")]
        let (antenna_outcome, field_outcome, match_outcome, projects) =
            (antenna_outcome.clone(), field_outcome.clone(), match_outcome.clone(), projects.clone());
        Rc::new(move |msg| match msg {
            Msg::Calculate => run_solve(&container, &outcome),
            Msg::Sample(index) => {
                apply_sample(&container, index);
                run_solve(&container, &outcome);
            }
            Msg::KindChanged(kind) => toggle_kind_fields(&container, &kind),
            Msg::VisitSite(url) => visit_site(&url),
            #[cfg(feature = "pro")]
            Msg::DesignAntenna => {
                super::antenna_panel::run_design(&container, &antenna_outcome);
            }
            #[cfg(feature = "pro")]
            Msg::SolveField => {
                super::field_panel::run_field(&container, &field_outcome);
            }
            #[cfg(feature = "pro")]
            Msg::DesignMatch => {
                super::antenna_panel::run_match(&container, &match_outcome);
            }
            #[cfg(feature = "pro")]
            Msg::SaveProject => {
                super::projects::save_from_form(&container);
                projects.set(Rc::new(super::projects::list()));
            }
            #[cfg(feature = "pro")]
            Msg::LoadProject(name) => {
                if super::projects::load_into_form(&container, &name) {
                    run_solve(&container, &outcome);
                }
            }
            #[cfg(feature = "pro")]
            Msg::DeleteProject(name) => {
                super::projects::delete(&name);
                projects.set(Rc::new(super::projects::list()));
            }
        })
    };

    // 1. Static shell — mounted exactly once.
    let shell = shell_tree();
    let root = tpt_appfront_dom::mount(container, &shell, dispatch.clone())?;
    std::mem::forget(root);

    // 2. Reactive results section into its placeholder.
    let results_el = container
        .query_selector(".emd-results")?
        .ok_or_else(|| JsValue::from_str("results placeholder missing"))?;
    let outcome_for_render = outcome.clone();
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || results_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    #[cfg(feature = "pro")]
    {
        // Pro: antenna results section, field viewer canvas + its results,
        // pattern canvas, match results, and the project library list.
        super::antenna_panel::wire(container, &antenna_outcome, &match_outcome, &dispatch)?;
        super::field_panel::wire(container, &field_outcome)?;
        let list_el = container
            .query_selector(".emd-project-list")?
            .ok_or_else(|| JsValue::from_str("project list placeholder missing"))?;
        let projects_for_render = projects.clone();
        let handle = tpt_appfront_dom::render(
            &list_el,
            Rc::new(move || super::projects::list_view(projects_for_render.get())),
            dispatch.clone(),
        )?;
        std::mem::forget(handle);
    }

    Ok(())
}

/// The mounted-once shell: header, setup cards, results placeholder,
/// (pro) antenna/field/project cards, upsell, footer.
fn shell_tree() -> UITree<Msg> {
    UITree::container(|c| {
        c.container(|header| {
            header.text("EM").class("emd-brand");
            header.container(|group| {
                group.container(|row| {
                    row.heading(2, "EM Design");
                    #[cfg(not(feature = "pro"))]
                    row.text("Free").class("emd-chip");
                    #[cfg(feature = "pro")]
                    row.text("Pro").class("emd-chip");
                })
                .class("emd-title-row");
                group.text("PCB trace impedance for everyday board and RF work — solved locally in your browser. Units: mm, \u{3bc}m copper, GHz.")
                    .class("emd-sub");
            })
            .class("emd-title-group");
        })
        .class("emd-header");

        c.container(|shell| {
            shell.container(|sidebar| {
                trace_card(sidebar);
                dielectric_card(sidebar);
                actions_card(sidebar);
                #[cfg(feature = "pro")]
                super::antenna_panel::sidebar_cards(sidebar);
                #[cfg(feature = "pro")]
                super::projects::sidebar_card(sidebar);
            })
            .class("emd-sidebar");

            shell.container(|main| {
                main.container(|card| {
                    card.heading(3, "Result");
                    card.container(|_| {}).class("emd-results");
                })
                .class("emd-card");
                #[cfg(feature = "pro")]
                super::antenna_panel::main_card(main);
                #[cfg(feature = "pro")]
                super::field_panel::main_card(main);
                #[cfg(not(feature = "pro"))]
                upsell_card(main);
            })
            .class("emd-main");
        })
        .class("emd-shell");

        c.container(|footer| {
            footer.text("TPT EM Design \u{b7} runs entirely in your browser \u{b7} TPT Solutions").class("emd-footer-brand");
            footer.container(|links| {
                footer_link(links, "More engineering tools", "https://tptsolutions.co.nz/tools");
                #[cfg(not(feature = "pro"))]
                footer_link(links, "Get the Pro edition", "https://tptsolutions.co.nz/tools/em-design");
            })
            .class("emd-footer-links");
        })
        .class("emd-footer");
    })
}

fn trace_card(sidebar: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Trace geometry");
        card.container(|row| {
            row.container(|field| {
                label(field, "Stackup");
                field
                    .select(
                        [
                            ("microstrip", "Microstrip (surface)"),
                            ("stripline", "Stripline (buried)"),
                        ],
                        "microstrip",
                    )
                    .class("emd-input emd-kind")
                    .on_input(Msg::KindChanged);
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Trace width w (mm)");
                field.input(defaults::WIDTH).class("emd-input emd-width");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Dielectric height h (mm)");
                field.input(defaults::HEIGHT).class("emd-input emd-height");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Plane spacing b (mm)");
                field
                    .input(defaults::SPACING)
                    .class("emd-input emd-spacing");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Copper weight");
                field
                    .select(
                        [
                            ("0", "0.5 oz"),
                            ("1", "1 oz"),
                            ("2", "2 oz"),
                            ("3", "3 oz"),
                        ],
                        "1",
                    )
                    .class("emd-input emd-copper");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.text(
            "Microstrip: trace on the outer layer over one reference plane — \
             enter h. Stripline: trace centered between two planes — enter b. \
             h is ignored for stripline and b for microstrip."
        )
        .class("emd-hint");
    })
    .class("emd-card");
}

fn dielectric_card(sidebar: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Dielectric & signal");
        card.container(|row| {
            row.container(|field| {
                label(field, "Dielectric constant \u{3b5}r");
                field.input(defaults::ER).class("emd-input emd-er");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Loss tangent tan \u{3b4}");
                field.input(defaults::TAND).class("emd-input emd-tand");
            })
            .class("emd-field");
            row.container(|field| {
                label(field, "Frequency (GHz)");
                field.input(defaults::FREQ).class("emd-input emd-freq");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.text(
            "\u{3b5}r: 4.2\u{2013}4.8 FR-4 \u{b7} 3.7\u{2013}4.0 Pyralux-family flex \
             \u{b7} 3.5 Rogers RO4350B \u{b7} 2.2 PTFE. Set frequency to 0 to skip \
             the loss estimate."
        )
        .class("emd-hint");
    })
    .class("emd-card");
}

fn actions_card(sidebar: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Calculate");
        card.container(|actions| {
            actions
                .button("Calculate")
                .class("emd-btn emd-primary")
                .on_click(Msg::Calculate);
        })
        .class("emd-actions");
        card.container(|actions| {
            for (index, label) in SAMPLES.iter().enumerate() {
                actions
                    .button(format!("Sample: {label}"))
                    .class("emd-btn")
                    .on_click(Msg::Sample(index));
            }
        })
        .class("emd-actions emd-actions-secondary");
    })
    .class("emd-card");
}

/// Free edition: pitch for the paid desktop edition.
#[cfg(not(feature = "pro"))]
fn upsell_card(main: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    main.container(|card| {
        card.heading(3, "Pro edition (one-time purchase, runs offline)");
        card.container(|body| {
            body.text("The free calculator covers the trace. The Windows Pro edition adds the rest of the EM toolkit:");
            body.container(|list| {
                for item in [
                    "Antenna design tools — dipole, monopole, and patch dimensioning with matching networks (quarter-wave, L, single-stub)",
                    "2D field visualization — solve the microstrip cross-section and see the potential and E-field, plus radiation-pattern cuts",
                    "Saved project library — keep every trace, antenna, and field study in one place and reload them anytime",
                ] {
                    list.container(|li| {
                        li.text(item);
                    })
                    .class("emd-li");
                }
            })
            .class("emd-upsell-list");
            body.container(|actions| {
                actions
                    .button("See TPT EM Design Pro \u{2192}")
                    .class("emd-btn emd-primary")
                    .on_click(Msg::VisitSite(
                        "https://tptsolutions.co.nz/tools/em-design".to_string(),
                    ));
            })
            .class("emd-actions");
        })
        .class("emd-upsell");
    })
    .class("emd-card");
}

/// The reactive results section.
fn results_view(outcome: Signal<Outcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let mut root_builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    root_builder
        .container(|c| {
            c.container(|summary| {
                let (z0_line, detail) = match &outcome {
                    Outcome::Idle => (
                        "Enter the stackup, then press Calculate.".to_string(),
                        "Try a sample problem for a one-click first run.".to_string(),
                    ),
                    Outcome::Invalid(issues) => (
                        format!(
                            "Fix {} issue{} before solving:",
                            issues.len(),
                            if issues.len() == 1 { "" } else { "s" }
                        ),
                        String::new(),
                    ),
                    Outcome::Solved(solved) => {
                        let r = &solved.result;
                        (
                            format!("Z\u{2080} \u{2248} {} \u{3a9}", fmt_g(r.z0)),
                            format!(
                                "{} \u{b7} \u{3b5}eff {} \u{b7} delay {} ps/mm",
                                solved.label,
                                fmt_g(r.epsilon_eff),
                                fmt_g(r.delay_ps_per_mm)
                            ),
                        )
                    }
                };
                summary.container(|z| {
                    styled_text(z, z0_line, "emd-z0");
                })
                .class("emd-summary");
                if !detail.is_empty() {
                    styled_text(summary, detail, "emd-detail");
                }
                styled_text(
                    summary,
                    "Engine: Hammerstad\u{2013}Jensen microstrip with Garg\u{2013}Bahl thickness \
                     correction; exact conformal-mapping stripline. Quasi-static, solved locally.",
                    "emd-engine",
                );
            })
            .class("emd-summary");

            c.container(|issues| {
                if let Outcome::Invalid(list) = &outcome {
                    for issue in list {
                        styled_text(issues, issue.clone(), "emd-issue");
                    }
                }
            })
            .class("emd-issues");

            let rows: Vec<Vec<String>> = match &outcome {
                Outcome::Solved(solved) => {
                    let r = &solved.result;
                    let mut rows = vec![
                        vec![
                            "Characteristic impedance".to_string(),
                            format!("{} \u{3a9}", fmt_g(r.z0)),
                            "the number you tune the width for".to_string(),
                        ],
                        vec![
                            "Effective permittivity".to_string(),
                            fmt_g(r.epsilon_eff),
                            if solved.label.starts_with("Microstrip") {
                                "field partly in air \u{2014} why microstrip beats stripline on speed"
                                    .to_string()
                            } else {
                                "homogeneous fill (stripline)".to_string()
                            },
                        ],
                        vec![
                            "Propagation delay".to_string(),
                            format!("{} ps/mm", fmt_g(r.delay_ps_per_mm)),
                            format!("{} ns per 100 mm trace", fmt_g(r.delay_ps_per_mm * 0.1)),
                        ],
                        vec![
                            "Capacitance".to_string(),
                            format!("{} pF/mm", fmt_g(r.c_pf_per_mm)),
                            "distributed, per unit length".to_string(),
                        ],
                        vec![
                            "Inductance".to_string(),
                            format!("{} nH/mm", fmt_g(r.l_nh_per_mm)),
                            "distributed, per unit length".to_string(),
                        ],
                        vec![
                            "Effective width".to_string(),
                            format!("{} mm", fmt_g(r.effective_width_mm)),
                            "after the copper-thickness correction".to_string(),
                        ],
                    ];
                    if r.loss_db_per_mm > 0.0 {
                        rows.push(vec![
                            "Loss estimate".to_string(),
                            format!("{} dB/mm", fmt_g(r.loss_db_per_mm)),
                            format!(
                                "\u{2248}{} dB per 100 mm (skin effect + tan \u{3b4}; roughness ignored)",
                                fmt_g(r.loss_db_per_mm * 100.0)
                            ),
                        ]);
                    }
                    rows
                }
                _ => Vec::new(),
            };
            c.data_grid(["Quantity", "Value", "Detail"], rows);

            c.container(|notes| {
                if let Outcome::Solved(solved) = &outcome {
                    for note in &solved.result.notes {
                        styled_text(notes, format!("\u{26a0} {note}"), "emd-detail");
                    }
                }
            })
            .class("emd-notes");
        })
        .class("emd-results-view");
    root_builder
        .into_only_child()
        .expect("results view builds exactly one root container")
}

// -- dispatch helpers -------------------------------------------------------

pub(crate) fn run_solve(container: &web_sys::Element, outcome: &Signal<Outcome>) {
    match read_trace(container) {
        Ok((label, input)) => match solve(&input) {
            Ok(result) => outcome.set(Outcome::Solved(Rc::new(Solved { label, result }))),
            Err(error) => outcome.set(Outcome::Invalid(error.0)),
        },
        Err(issues) => outcome.set(Outcome::Invalid(issues)),
    }
}

/// Builds the engine input from the form; all field errors surface at once.
fn read_trace(container: &web_sys::Element) -> Result<(String, TraceInput), Vec<String>> {
    let mut issues: Vec<String> = Vec::new();
    let kind = read_input(container, ".emd-kind");
    let width = num(container, ".emd-width", "Trace width", &mut issues);
    let height = num(container, ".emd-height", "Dielectric height", &mut issues);
    let spacing = num(container, ".emd-spacing", "Plane spacing", &mut issues);
    let er = num(container, ".emd-er", "Dielectric constant", &mut issues);
    let tand = num(container, ".emd-tand", "Loss tangent", &mut issues);
    let freq = num(container, ".emd-freq", "Frequency", &mut issues);
    let copper = read_input(container, ".emd-copper")
        .parse::<usize>()
        .map(copper_thickness)
        .unwrap_or(34.8);
    if !issues.is_empty() {
        return Err(issues);
    }
    if freq < 0.0 {
        issues.push("Frequency must not be negative.".to_string());
    }
    if tand < 0.0 {
        issues.push("Loss tangent must not be negative.".to_string());
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    let thickness_um = copper;
    match kind.as_str() {
        "stripline" => Ok((
            format!(
                "Stripline \u{b7} w {} mm \u{b7} b {} mm \u{b7} \u{3b5}r {}",
                fmt_g(width),
                fmt_g(spacing),
                fmt_g(er)
            ),
            TraceInput::Stripline(StriplineInput {
                width_mm: width,
                spacing_mm: spacing,
                er,
                thickness_um,
                loss_tangent: tand,
                freq_ghz: freq,
            }),
        )),
        _ => Ok((
            format!(
                "Microstrip \u{b7} w {} mm \u{b7} h {} mm \u{b7} \u{3b5}r {}",
                fmt_g(width),
                fmt_g(height),
                fmt_g(er)
            ),
            TraceInput::Microstrip(MicrostripInput {
                width_mm: width,
                height_mm: height,
                er,
                thickness_um,
                loss_tangent: tand,
                freq_ghz: freq,
            }),
        )),
    }
}

/// Fills the form with a built-in sample.
fn apply_sample(container: &web_sys::Element, index: usize) {
    match index {
        1 => {
            write_field(container, ".emd-kind", "stripline");
            toggle_kind_fields(container, "stripline");
            write_field(container, ".emd-width", "0.48");
            write_field(container, ".emd-spacing", "1.2");
            write_field(container, ".emd-er", "4.6");
            write_field(container, ".emd-copper", "1");
            write_field(container, ".emd-tand", "0.02");
            write_field(container, ".emd-freq", "1.0");
        }
        2 => {
            write_field(container, ".emd-kind", "microstrip");
            toggle_kind_fields(container, "microstrip");
            write_field(container, ".emd-width", "6.0");
            write_field(container, ".emd-height", "0.8");
            write_field(container, ".emd-er", "4.6");
            write_field(container, ".emd-copper", "2");
            write_field(container, ".emd-tand", "0.02");
            write_field(container, ".emd-freq", "0.1");
        }
        _ => {
            write_field(container, ".emd-kind", "microstrip");
            toggle_kind_fields(container, "microstrip");
            write_field(container, ".emd-width", "3.0");
            write_field(container, ".emd-height", "1.6");
            write_field(container, ".emd-er", "4.6");
            write_field(container, ".emd-copper", "1");
            write_field(container, ".emd-tand", "0.02");
            write_field(container, ".emd-freq", "1.0");
        }
    }
}

/// Show/hide the microstrip h field vs the stripline b field.
pub(crate) fn toggle_kind_fields(container: &web_sys::Element, kind: &str) {
    let hide = |container: &web_sys::Element, selector: &str, hidden: bool| {
        if let Some(el) = container.query_selector(selector).ok().flatten() {
            let _ = el.set_attribute(
                "style",
                if hidden { "display:none" } else { "display:flex" },
            );
        }
    };
    hide(container, ".emd-height", kind == "stripline");
    hide(container, ".emd-spacing", kind != "stripline");
}

fn visit_site(url: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let in_desktop_shell = window
        .location()
        .protocol()
        .map(|p| p == "app:")
        .unwrap_or(false);

    if in_desktop_shell {
        // Routed through the desktop host's allowlisted IPC bridge
        // (`window.__appfront.post`, injected by tpt-appfront-webview) so the
        // *native* process opens the OS default browser, leaving the app
        // window untouched. See `desktop/src/main.rs`'s `open_external`
        // handler.
        if let Ok(appfront) = js_sys::Reflect::get(&window, &JsValue::from_str("__appfront")) {
            if let Ok(post) = js_sys::Reflect::get(&appfront, &JsValue::from_str("post")) {
                if let Ok(post_fn) = post.dyn_into::<js_sys::Function>() {
                    let params = js_sys::Object::new();
                    let _ = js_sys::Reflect::set(
                        &params,
                        &JsValue::from_str("url"),
                        &JsValue::from_str(url),
                    );
                    let _ = post_fn.call2(&appfront, &JsValue::from_str("open_external"), &params);
                }
            }
        }
    } else {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
}

// -- DOM helpers ------------------------------------------------------------

/// `.text()` produces a raw DOM text node, which can't carry a `class`
/// attribute — wrap the label in a container so `.emd-label` styling
/// actually reaches the page.
pub(crate) fn label(field: &mut tpt_appfront_core::ContainerBuilder<Msg>, text: impl Into<String>) {
    styled_text(field, text, "emd-label");
}

/// Same fix as [`label`], generalized to any class: text wrapped in its own
/// container so multiple styled text nodes in the same parent don't run
/// together with no separation.
pub(crate) fn styled_text(
    c: &mut tpt_appfront_core::ContainerBuilder<Msg>,
    text: impl Into<String>,
    class: &str,
) {
    let text = text.into();
    c.container(move |c| {
        c.text(text);
    })
    .class(class);
}

/// A footer "link" rendered as a plain `<button>` (not `<a>`) dispatching
/// [`Msg::VisitSite`] — an anchor with both `href` and `on_click` would
/// navigate the local `app://` webview *and* fire the message.
fn footer_link(c: &mut tpt_appfront_core::ContainerBuilder<Msg>, label: &str, url: &str) {
    c.button(label)
        .class("emd-footer-link")
        .on_click(Msg::VisitSite(url.to_string()));
}

/// One field parse: trimmed, finite f64.
pub(crate) fn num(container: &web_sys::Element, selector: &str, name: &str, issues: &mut Vec<String>) -> f64 {
    let text = read_input(container, selector);
    match text.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => value,
        _ => {
            issues.push(format!("{name} must be a number (got \u{201c}{text}\u{201d})."));
            f64::NAN
        }
    }
}

/// Reads the current value of an input/select by class selector.
pub(crate) fn read_input(container: &web_sys::Element, selector: &str) -> String {
    container
        .query_selector(selector)
        .ok()
        .flatten()
        .and_then(|el| {
            if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
                Some(input.value())
            } else {
                el.dyn_ref::<web_sys::HtmlSelectElement>().map(|s| s.value())
            }
        })
        .unwrap_or_default()
}

pub(crate) fn write_field(container: &web_sys::Element, selector: &str, value: &str) {
    if let Some(el) = container.query_selector(selector).ok().flatten() {
        if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
            input.set_value(value);
        } else if let Some(select) = el.dyn_ref::<web_sys::HtmlSelectElement>() {
            select.set_value(value);
        }
    }
}

/// Resolves a CSS custom property from the app root (theme-aware canvas text).
#[cfg(feature = "pro")]
pub(crate) fn read_css(container: &web_sys::Element, name: &str, fallback: &str) -> String {
    let value = web_sys::window()
        .and_then(|w| w.get_computed_style(container).ok())
        .flatten()
        .and_then(|style| style.get_property_value(name).ok())
        .unwrap_or_default();
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

/// Creates or updates the `<style id>` in the document head.
fn upsert_style(document: &web_sys::Document, id: &str, css: &str) {
    if let Ok(Some(existing)) = document.query_selector(&format!("style#{id}")) {
        existing.set_text_content(Some(css));
        return;
    }
    let Some(head) = document.head() else {
        return;
    };
    if let Ok(style_el) = document.create_element("style") {
        let _ = style_el.set_attribute("id", id);
        style_el.set_text_content(Some(css));
        let _ = head.append_child(&style_el);
    }
}

/// Adaptive numeric formatting for results text.
pub(crate) fn fmt_g(v: f64) -> String {
    let a = v.abs();
    if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 100.0 {
        format!("{v:.1}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else if a >= 0.001 {
        format!("{v:.4}")
    } else if a == 0.0 {
        "0".to_string()
    } else {
        format!("{v:.2e}")
    }
}
