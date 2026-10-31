use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use pwe_core::i18n::t;

use crate::format::{db_text, lufs_text, normalized_db};

pub const SEGMENTS: usize = 44;
pub const HISTORY: usize = 72;
const SEGMENT_GAP: f64 = 1.5;
const HOLD_DECAY: f64 = 0.006;

#[derive(Default)]
struct MeterState {
    peak: [f64; 2],
    rms: [f64; 2],
    hold: [f64; 2],
    history: Vec<f64>,
}

pub struct LevelDisplay {
    label: gtk::Label,
    state: Rc<RefCell<MeterState>>,
    area: gtk::DrawingArea,
    loudness: Option<gtk::Label>,
}

impl LevelDisplay {
    pub fn new(title: &str, with_history: bool, with_loudness: bool) -> (gtk::Box, Self) {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
        container.add_css_class("meter-block");
        container.add_css_class("card");
        container.set_hexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let name = gtk::Label::new(Some(title));
        name.set_xalign(0.0);
        name.add_css_class("caption");
        name.set_hexpand(true);
        let label = gtk::Label::new(Some(&db_text(-120.0)));
        label.add_css_class("caption");
        label.add_css_class("numeric");
        header.append(&name);
        header.append(&label);

        let area = gtk::DrawingArea::new();
        area.set_content_height(if with_history { 46 } else { 22 });
        area.set_hexpand(true);

        let state = Rc::new(RefCell::new(MeterState {
            history: if with_history {
                vec![0.0; HISTORY]
            } else {
                Vec::new()
            },
            ..MeterState::default()
        }));

        let borrowed = Rc::clone(&state);
        area.set_draw_func(move |area, context, width, height| {
            draw(
                area,
                context,
                width,
                height,
                &borrowed.borrow(),
                with_history,
            );
        });

        container.append(&header);
        container.append(&area);
        let loudness = with_loudness.then(|| {
            let label = gtk::Label::new(Some(&lufs_text(f32::NEG_INFINITY)));
            label.add_css_class("caption");
            label.add_css_class("dim-label");
            label.add_css_class("numeric");
            label.set_xalign(0.0);
            label.set_tooltip_text(Some(t(
                "EBU R128 short-term loudness of the processed audio",
            )));
            container.append(&label);
            label
        });

        (
            container,
            Self {
                label,
                state,
                area,
                loudness,
            },
        )
    }

    pub fn update_loudness(&self, lufs: f32) {
        if let Some(label) = &self.loudness {
            label.set_label(&lufs_text(lufs));
        }
    }

    pub fn update(&self, peak: [f32; 2], rms: [f32; 2]) {
        let loudest = peak[0].max(peak[1]);
        {
            let mut state = self.state.borrow_mut();
            for channel in 0..2 {
                let level = normalized_db(peak[channel]);
                state.peak[channel] = level;
                state.rms[channel] = normalized_db(rms[channel]);
                state.hold[channel] = (state.hold[channel] - HOLD_DECAY).max(level);
            }
            if !state.history.is_empty() {
                state.history.push(normalized_db(loudest));
                if state.history.len() > HISTORY {
                    state.history.remove(0);
                }
            }
        }
        self.label.set_label(&db_text(loudest));
        if loudest > -0.5 {
            self.label.add_css_class("error");
        } else {
            self.label.remove_css_class("error");
        }
        self.area.queue_draw();
    }
}

#[derive(Clone, Copy)]
struct Bar {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
struct Levels {
    peak: f64,
    rms: f64,
    hold: f64,
}

fn draw(
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    state: &MeterState,
    with_history: bool,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    if width <= 2.0 || height <= 2.0 {
        return;
    }
    let bars = if with_history { height - 20.0 } else { height };
    let bar_height = (bars - 3.0).max(4.0) / 2.0;
    for channel in 0..2 {
        let top = channel as f64 * (bar_height + 3.0);
        draw_bar(
            area,
            context,
            Bar {
                left: 0.0,
                top,
                width,
                height: bar_height,
            },
            Levels {
                peak: state.peak[channel],
                rms: state.rms[channel],
                hold: state.hold[channel],
            },
        );
    }
    if with_history {
        draw_history(
            context,
            0.0,
            bars + 4.0,
            width,
            height - bars - 4.0,
            &state.history,
        );
    }
}

fn draw_bar(area: &gtk::DrawingArea, context: &gtk::cairo::Context, bar: Bar, levels: Levels) {
    let gap = SEGMENT_GAP;
    let segment = (bar.width - gap * (SEGMENTS - 1) as f64) / SEGMENTS as f64;
    if segment <= 0.2 {
        return;
    }
    let lit = (levels.peak * SEGMENTS as f64).ceil() as usize;
    for index in 0..SEGMENTS {
        let x = bar.left + index as f64 * (segment + gap);
        if index < lit {
            let (red, green, blue) = segment_colour(index);
            context.set_source_rgba(red, green, blue, 1.0);
        } else {
            context.set_source_rgba(0.55, 0.55, 0.55, 0.16);
        }
        rounded_rect(
            context,
            x,
            bar.top,
            segment,
            bar.height,
            (segment / 2.0).min(2.0),
        );
        let _ = context.fill();
    }
    let foreground = area.color();
    if levels.rms > 0.005 {
        let x = bar.left + levels.rms.clamp(0.0, 1.0) * bar.width;
        context.set_source_rgba(0.42, 0.68, 1.0, 0.9);
        context.rectangle(x - 1.0, bar.top, 2.0, bar.height);
        let _ = context.fill();
    }
    if levels.hold > 0.01 {
        let x = bar.left + levels.hold.clamp(0.0, 1.0) * bar.width;
        context.set_source_rgba(
            f64::from(foreground.red()),
            f64::from(foreground.green()),
            f64::from(foreground.blue()),
            0.9,
        );
        context.rectangle(x - 1.0, bar.top, 2.0, bar.height);
        let _ = context.fill();
    }
}

fn draw_history(
    context: &gtk::cairo::Context,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    history: &[f64],
) {
    if history.len() < 2 || height <= 2.0 {
        return;
    }
    if history.iter().all(|value| *value <= 0.001) {
        return;
    }
    let step = width / (history.len() - 1) as f64;
    context.set_source_rgba(0.35, 0.62, 1.0, 0.22);
    context.move_to(left, top + height);
    for (index, value) in history.iter().enumerate() {
        let x = left + index as f64 * step;
        let y = top + height - value.clamp(0.0, 1.0) * height;
        context.line_to(x, y);
    }
    context.line_to(left + width, top + height);
    context.close_path();
    let _ = context.fill();

    context.set_source_rgba(0.42, 0.68, 1.0, 0.85);
    context.set_line_width(1.2);
    for (index, value) in history.iter().enumerate() {
        let x = left + index as f64 * step;
        let y = top + height - value.clamp(0.0, 1.0) * height;
        if index == 0 {
            context.move_to(x, y);
        } else {
            context.line_to(x, y);
        }
    }
    let _ = context.stroke();
}

fn rounded_rect(
    context: &gtk::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    context.new_sub_path();
    context.arc(
        x + width - radius,
        y + radius,
        radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    context.arc(
        x + radius,
        y + height - radius,
        radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    context.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    context.close_path();
}

fn segment_colour(index: usize) -> (f64, f64, f64) {
    let position = index as f64 / SEGMENTS as f64;
    if position < 0.70 {
        (0.24, 0.72, 0.44)
    } else if position < 0.88 {
        (0.94, 0.71, 0.24)
    } else {
        (0.91, 0.31, 0.29)
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
