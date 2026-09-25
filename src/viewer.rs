//! Canvas renderers (Pro edition): the 2D cross-section field heatmap and
//! the radiation-pattern polar plot. Raw web-sys canvas — the DOM backend
//! has no canvas node kind.

use tpt_em_design_engine::field::{FieldResult, PatternCurve};
use wasm_bindgen::JsCast;

/// Field viewer canvas size (CSS scales it to the card width).
const FW: f64 = 960.0;
const FH: f64 = 560.0;
/// Pattern viewer canvas size.
const PW: f64 = 960.0;
const PH: f64 = 480.0;

/// Draws the solved cross-section: potential (with equipotential lines) or
/// |E| as a heatmap, with the trace and dielectric interface annotated.
pub fn draw_field(
    canvas: &web_sys::HtmlCanvasElement,
    field: &FieldResult,
    fg: &str,
    muted: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    let ctx = render_ctx(canvas)?;
    // The grid is rasterized at native resolution into an offscreen canvas,
    // then drawn scaled onto the display canvas (smooth interpolation free).
    let (cols, rows) = (field.cols, field.rows);
    let mut data = vec![0u8; cols * rows * 4];
    for i in 0..cols * rows {
        let value = field.potential[i]; // 0..1 by construction
        let (r, g, b) = inferno(value);
        let o = i * 4;
        data[o] = r;
        data[o + 1] = g;
        data[o + 2] = b;
        data[o + 3] = 255;
    }
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");
    let offscreen: web_sys::HtmlCanvasElement = document
        .create_element("canvas")?
        .dyn_into()
        .map_err(|_| wasm_bindgen::JsValue::from_str("offscreen canvas cast failed"))?;
    offscreen.set_width(cols as u32);
    offscreen.set_height(rows as u32);
    let offscreen_ctx = render_ctx(&offscreen)?;
    let array = js_sys::Uint8ClampedArray::from(data.as_slice());
    let image = web_sys::ImageData::new_with_js_u8_clamped_array_and_sh(
        &array,
        cols as u32,
        rows as u32,
    )?;
    offscreen_ctx.put_image_data(&image, 0.0, 0.0)?;

    // Fit the grid width to the canvas, centered vertically.
    let scale = FW / cols as f64;
    let draw_h = rows as f64 * scale;
    let y0 = (FH - draw_h) / 2.0;
    ctx.clear_rect(0.0, 0.0, FW, FH);
    ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
        &offscreen,
        0.0,
        y0,
        FW,
        draw_h,
    )?;

    // Equipotential lines at ~0.1 V steps: stroke every grid-edge crossing
    // of each level (marching-squares-lite; unconnected subpaths are fine).
    ctx.save();
    ctx.set_stroke_style_str("rgba(255,255,255,0.55)");
    ctx.set_line_width(1.2);
    let steps = 9usize;
    let px = |c: f64| c * scale;
    let py = |r: f64| y0 + r * scale;
    for level_i in 1..=steps {
        let level = level_i as f64 / (steps + 1) as f64;
        ctx.begin_path();
        for row in 0..rows - 1 {
            for col in 0..cols - 1 {
                let v00 = field.potential[row * cols + col];
                let v10 = field.potential[row * cols + col + 1];
                let v01 = field.potential[(row + 1) * cols + col];
                if (v00 - level) * (v10 - level) < 0.0 {
                    let t = (level - v00) / (v10 - v00);
                    ctx.move_to(px(col as f64 + t), py(row as f64));
                    ctx.line_to(px(col as f64 + t), py(row as f64 + 1.0));
                }
                if (v00 - level) * (v01 - level) < 0.0 {
                    let t = (level - v00) / (v01 - v00);
                    ctx.move_to(px(col as f64), py(row as f64 + t));
                    ctx.line_to(px(col as f64 + 1.0), py(row as f64 + t));
                }
            }
        }
        ctx.stroke();
    }
    ctx.restore();

    // Trace bar + interface annotation.
    ctx.save();
    ctx.set_fill_style_str(fg);
    let trace_half = field.width_mm / 2.0;
    let cx = field.cols as f64 * scale / 2.0;
    let trace_px = (trace_half * 2.0) / (field.dx) * scale;
    let interface_y = field.rows as f64 - (field.height_mm / field.dy);
    let ty = y0 + interface_y * scale;
    ctx.fill_rect(cx - trace_px / 2.0, ty - 2.0, trace_px, 4.0);
    ctx.set_fill_style_str(muted);
    ctx.set_font("12px system-ui, sans-serif");
    let _ = ctx.fill_text_with_max_width(
        "trace (V = 1)",
        cx + trace_px / 2.0 + 8.0,
        ty + 4.0,
        200.0,
    );
    let _ = ctx.fill_text_with_max_width(
        "dielectric \u{2193} \u{b7} \u{3b5}r",
        8.0,
        y0 + draw_h - 8.0,
        240.0,
    );
    let _ = ctx.fill_text_with_max_width("air \u{2191}", 8.0, y0 + 14.0, 80.0);
    ctx.restore();

    Ok(())
}

/// Draws one or two normalized pattern cuts as a polar plot with a legend.
pub fn draw_pattern(
    canvas: &web_sys::HtmlCanvasElement,
    curves: &[PatternCurve],
    fg: &str,
    muted: &str,
    accent: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    let ctx = render_ctx(canvas)?;
    ctx.clear_rect(0.0, 0.0, PW, PH);
    let cx = PW / 2.0;
    let cy = PH / 2.0;
    let radius = (PH / 2.0 - 46.0).min(PW / 2.0 - 90.0);

    // Radial grid: rings at 0.25/0.5/0.75/1.0, spokes every 30°.
    ctx.save();
    ctx.set_stroke_style_str(muted);
    ctx.set_line_width(1.0);
    for ring in [0.25f64, 0.5, 0.75, 1.0] {
        ctx.begin_path();
        let _ = ctx.arc(cx, cy, radius * ring, 0.0, std::f64::consts::TAU);
        ctx.stroke();
    }
    for spoke in 0..12 {
        let angle = spoke as f64 * std::f64::consts::FRAC_PI_6;
        ctx.begin_path();
        ctx.move_to(cx, cy);
        ctx.line_to(cx + angle.sin() * radius, cy - angle.cos() * radius);
        ctx.stroke();
    }
    // Angle labels.
    ctx.set_fill_style_str(muted);
    ctx.set_font("12px system-ui, sans-serif");
    let _ = ctx.fill_text_with_max_width("0\u{b0}", cx - 8.0, cy - radius - 8.0, 40.0);
    let _ = ctx.fill_text_with_max_width(
        "180\u{b0}",
        cx - 16.0,
        cy + radius + 18.0,
        40.0,
    );
    let _ = ctx.fill_text_with_max_width("90\u{b0}", cx + radius + 8.0, cy + 4.0, 40.0);
    let _ = ctx.fill_text_with_max_width("270\u{b0}", cx - radius - 44.0, cy + 4.0, 40.0);
    ctx.restore();

    // Curves: angle measured from broadside (up); 0..180 = right half, then
    // mirrored (these cuts are symmetric), so plot the full circle.
    let colors = [accent, "#e0709a"];
    ctx.save();
    ctx.set_line_width(2.2);
    for (idx, curve) in curves.iter().enumerate() {
        let color = colors.get(idx).unwrap_or(&accent);
        ctx.set_stroke_style_str(color);
        ctx.begin_path();
        let mut first = true;
        for angle in 0..360 {
            let sample = if angle <= 180 {
                curve.normalized[angle]
            } else {
                curve.normalized[360 - angle]
            };
            let rad = angle as f64 * std::f64::consts::PI / 180.0;
            let r = sample * radius;
            let x = cx + rad.sin() * r;
            let y = cy - rad.cos() * r;
            if first {
                ctx.move_to(x, y);
                first = false;
            } else {
                ctx.line_to(x, y);
            }
        }
        ctx.close_path();
        ctx.stroke();
        // Soft fill.
        ctx.set_global_alpha(0.12);
        ctx.set_fill_style_str(color);
        ctx.fill();
        ctx.set_global_alpha(1.0);
    }
    ctx.restore();

    // Legend.
    ctx.save();
    ctx.set_font("13px system-ui, sans-serif");
    for (idx, curve) in curves.iter().enumerate() {
        let color = colors.get(idx).unwrap_or(&accent);
        ctx.set_fill_style_str(color);
        ctx.fill_rect(16.0, 18.0 + idx as f64 * 22.0, 14.0, 4.0);
        ctx.set_fill_style_str(fg);
        let label = if curve.beamwidth_deg > 0.0 {
            format!("{} \u{b7} \u{2212}3 dB \u{2248} {} \u{b0}", curve.label, curve.beamwidth_deg)
        } else {
            curve.label.clone()
        };
        let _ = ctx.fill_text_with_max_width(&label, 38.0, 24.0 + idx as f64 * 22.0, 430.0);
    }
    ctx.restore();

    Ok(())
}

fn render_ctx(
    canvas: &web_sys::HtmlCanvasElement,
) -> Result<web_sys::CanvasRenderingContext2d, wasm_bindgen::JsValue> {
    canvas
        .get_context("2d")?
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("2d context unavailable"))?
        .dyn_into()
        .map_err(|_| wasm_bindgen::JsValue::from_str("2d context cast failed"))
}

/// Inferno-ish colormap (0..1 → RGB): dark navy → purple → red → orange →
/// light yellow. Good separation for both potential bands and |E| tails.
fn inferno(t: f64) -> (u8, u8, u8) {
    let stops: [(f64, [u8; 3]); 6] = [
        (0.0, [10, 7, 40]),
        (0.2, [60, 15, 110]),
        (0.4, [130, 30, 120]),
        (0.6, [200, 55, 85]),
        (0.8, [245, 130, 30]),
        (1.0, [250, 235, 140]),
    ];
    let t = t.clamp(0.0, 1.0);
    for window in stops.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * f).round() as u8;
            return (mix(c0[0], c1[0]), mix(c0[1], c1[1]), mix(c0[2], c1[2]));
        }
    }
    (250, 235, 140)
}
