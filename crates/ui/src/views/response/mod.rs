use gtk::prelude::*;
use pwe_core::ParamSet;
use pwe_dsp::effects::{BAND_COUNT, BAND_FREQUENCIES, BAND_IDS, response_db};

const MAX_DB: f32 = 24.0;
const MIN_HZ: f32 = 20.0;
const MAX_HZ: f32 = 20_000.0;
const MAIN_COLOR: [f64; 4] = [0.35, 0.62, 1.0, 1.0];
const RIGHT_COLOR: [f64; 4] = [0.35, 0.82, 0.55, 1.0];

pub fn build(params: &ParamSet) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(160);
    area.set_hexpand(true);
    area.add_css_class("response");
    let params = params.clone();
    area.set_draw_func(move |area, context, width, height| {
        draw(area, context, width, height, &params);
    });
    area
}

fn draw(
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    params: &ParamSet,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    if width <= 1.0 || height <= 1.0 {
        return;
    }
    draw_grid(context, width, height);
    let zero = height * 0.5;
    draw_curve(
        context, width, height, zero, params, false, MAIN_COLOR, true,
    );
    if split_channels(params) {
        draw_curve(
            context,
            width,
            height,
            zero,
            params,
            true,
            RIGHT_COLOR,
            false,
        );
    }
    draw_markers(area, context, width, zero, params);
}

fn draw_grid(context: &gtk::cairo::Context, width: f64, height: f64) {
    context.set_source_rgba(0.5, 0.5, 0.5, 0.25);
    context.set_line_width(1.0);
    for step in 0..=4 {
        let y = height * f64::from(step) / 4.0;
        context.move_to(0.0, y);
        context.line_to(width, y);
    }
    for frequency in [100.0f32, 1_000.0, 10_000.0] {
        let x = width * f64::from(log_position(frequency));
        context.move_to(x, 0.0);
        context.line_to(x, height);
    }
    let _ = context.stroke();
    let zero = height * 0.5;
    context.set_source_rgba(0.5, 0.5, 0.5, 0.6);
    context.move_to(0.0, zero);
    context.line_to(width, zero);
    let _ = context.stroke();
}

#[allow(clippy::too_many_arguments)]
fn draw_curve(
    context: &gtk::cairo::Context,
    width: f64,
    height: f64,
    zero: f64,
    params: &ParamSet,
    right: bool,
    color: [f64; 4],
    fill: bool,
) {
    let mut path: Vec<(f64, f64)> = Vec::with_capacity(256);
    for step in 0..256 {
        let position = step as f32 / 255.0;
        let frequency = hz_at(position);
        let db = response_db(48_000.0, params, right, frequency);
        let x = width * f64::from(position);
        let y = zero - f64::from(db / MAX_DB) * (height * 0.5);
        path.push((x, y.clamp(0.0, height)));
    }
    if fill && let Some((first, _)) = path.first().copied() {
        context.move_to(first, height);
        context.line_to(first, path[0].1);
        for (x, y) in &path {
            context.line_to(*x, *y);
        }
        if let Some((last, _)) = path.last().copied() {
            context.line_to(last, height);
        }
        context.close_path();
        context.set_source_rgba(color[0], color[1], color[2], 0.22);
        let _ = context.fill();
    }
    context.set_source_rgba(color[0], color[1], color[2], color[3]);
    context.set_line_width(2.0);
    for (index, (x, y)) in path.iter().enumerate() {
        if index == 0 {
            context.move_to(*x, *y);
        } else {
            context.line_to(*x, *y);
        }
    }
    let _ = context.stroke();
}

fn draw_markers(
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: f64,
    zero: f64,
    params: &ParamSet,
) {
    let foreground = area.color();
    context.set_source_rgba(
        f64::from(foreground.red()),
        f64::from(foreground.green()),
        f64::from(foreground.blue()),
        0.75,
    );
    context.set_line_width(1.0);
    let split = split_channels(params);
    for band in 0..BAND_COUNT {
        for right in [false, true] {
            if right && !split {
                continue;
            }
            if band_gain(params, band, right).abs() < 0.05 {
                continue;
            }
            let x = width * f64::from(log_position(band_frequency(params, band, right)));
            context.move_to(x, zero - 3.0);
            context.line_to(x, zero + 3.0);
        }
    }
    let _ = context.stroke();
}

fn field_id(gain_id: &str, field: &str) -> String {
    format!("{}_{}", gain_id.trim_end_matches("_gain"), field)
}

fn channel_id(gain_id: &str, field: &str, right: bool) -> String {
    let id = field_id(gain_id, field);
    if right { format!("right_{id}") } else { id }
}

fn band_frequency(params: &ParamSet, band: usize, right: bool) -> f32 {
    params
        .get_f32(&channel_id(BAND_IDS[band], "frequency", right))
        .unwrap_or(BAND_FREQUENCIES[band])
}

fn band_gain(params: &ParamSet, band: usize, right: bool) -> f32 {
    params
        .get_f32(&channel_id(BAND_IDS[band], "gain", right))
        .unwrap_or(0.0)
}

fn split_channels(params: &ParamSet) -> bool {
    params.get_f32("channel_mode").unwrap_or(0.0) >= 0.5
}

fn log_position(frequency: f32) -> f32 {
    (frequency / MIN_HZ).ln() / (MAX_HZ / MIN_HZ).ln()
}

fn hz_at(position: f32) -> f32 {
    MIN_HZ * (MAX_HZ / MIN_HZ).powf(position.clamp(0.0, 1.0))
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
