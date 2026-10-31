use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::{BANDS, FLOOR_DB, band_centre};

use crate::format::{db_text, frequency_text};

const RANGES: [(f32, f32); 3] = [(-72.0, 6.0), (-60.0, 0.0), (-48.0, 12.0)];
const GRID_HZ: [f32; 3] = [100.0, 1_000.0, 10_000.0];
const POINT_COUNTS: [usize; 3] = [24, 48, 96];
const HEIGHTS: [i32; 3] = [54, 84, 120];
const MAX_HZ: [f32; 3] = [20_000.0, 10_000.0, 5_000.0];

#[derive(Debug, Clone, Copy, PartialEq)]
struct Options {
    range: u8,
    auto: bool,
    bounds: (f32, f32),
    shape: u8,
    points: u8,
    height: u8,
    frequency: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            range: 0,
            auto: false,
            bounds: RANGES[0],
            shape: 0,
            points: 1,
            height: 1,
            frequency: 0,
        }
    }
}

pub struct SpectrumDisplay {
    area: gtk::DrawingArea,
    label: gtk::Label,
    state: Rc<RefCell<[f32; BANDS]>>,
    options: Rc<RefCell<Options>>,
}

impl SpectrumDisplay {
    pub fn new() -> (gtk::Box, Self) {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
        container.add_css_class("card");
        container.add_css_class("spectrum-block");
        container.set_hexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let name = gtk::Label::new(Some(t("Spectrum")));
        name.set_xalign(0.0);
        name.add_css_class("caption");
        name.set_hexpand(true);
        let label = gtk::Label::new(Some(EMPTY));
        label.add_css_class("caption");
        label.add_css_class("numeric");
        header.append(&name);
        header.append(&label);

        let area = gtk::DrawingArea::new();
        area.set_content_height(HEIGHTS[1]);
        area.set_hexpand(true);
        area.update_property(&[gtk::accessible::Property::Label(t("Live audio spectrum"))]);

        let state: Rc<RefCell<[f32; BANDS]>> = Rc::new(RefCell::new([RANGES[0].0; BANDS]));
        let options = Rc::new(RefCell::new(Options::default()));
        let borrowed = Rc::clone(&state);
        let settings = Rc::clone(&options);
        area.set_draw_func(move |area, context, width, height| {
            draw(
                area,
                context,
                width,
                height,
                &borrowed.borrow(),
                *settings.borrow(),
            );
        });

        container.append(&header);
        container.append(&area);
        (
            container,
            Self {
                area,
                label,
                state,
                options,
            },
        )
    }

    pub fn set_range(&self, range: u8) {
        let range = range.min(RANGES.len() as u8 - 1);
        let mut options = self.options.borrow_mut();
        if options.range == range {
            return;
        }
        options.range = range;
        if !options.auto {
            options.bounds = RANGES[range as usize];
        }
        drop(options);
        self.area.queue_draw();
    }

    pub fn set_auto_range(&self, enabled: bool) {
        let mut options = self.options.borrow_mut();
        if options.auto == enabled {
            return;
        }
        options.auto = enabled;
        if !enabled {
            options.bounds = RANGES[options.range as usize];
        }
        drop(options);
        self.area.queue_draw();
    }

    pub fn set_shape(&self, shape: u8) {
        if self.options.borrow().shape == shape.min(2) {
            return;
        }
        self.options.borrow_mut().shape = shape.min(2);
        self.area.queue_draw();
    }

    pub fn set_points(&self, points: u8) {
        if self.options.borrow().points == points.min(2) {
            return;
        }
        self.options.borrow_mut().points = points.min(2);
        self.area.queue_draw();
    }

    pub fn set_height(&self, height: u8) {
        let height = height.min(2);
        if self.options.borrow().height == height {
            return;
        }
        self.options.borrow_mut().height = height;
        self.area.set_content_height(HEIGHTS[height as usize]);
        self.area.queue_draw();
    }

    pub fn set_frequency_range(&self, range: u8) {
        if self.options.borrow().frequency == range.min(2) {
            return;
        }
        self.options.borrow_mut().frequency = range.min(2);
        self.area.queue_draw();
    }

    pub fn update(&self, bands: [f32; BANDS]) {
        let floor = self.options.borrow().bounds.0;
        let top = top_frequency(self.options.borrow().frequency);
        let mut peak_index = 0;
        let mut peak_db = floor;
        for (index, value) in bands.iter().enumerate() {
            if band_centre(index) <= top && *value > peak_db {
                peak_db = *value;
                peak_index = index;
            }
        }
        if self.options.borrow().auto && peak_db > FLOOR_DB {
            self.options.borrow_mut().bounds = auto_bounds(peak_db);
        }
        let caption = if peak_db <= floor {
            EMPTY.to_owned()
        } else {
            format!(
                "{} {}",
                frequency_text(band_centre(peak_index)),
                db_text(peak_db)
            )
        };
        self.label.set_label(&caption);
        *self.state.borrow_mut() = bands;
        self.area.queue_draw();
    }
}

const EMPTY: &str = "\u{2014}";

fn auto_bounds(peak: f32) -> (f32, f32) {
    let floor = (peak - 60.0).clamp(-120.0, -6.0);
    (floor, floor + 60.0)
}

fn top_frequency(frequency: u8) -> f32 {
    MAX_HZ[frequency.min(MAX_HZ.len() as u8 - 1) as usize]
}

fn point_count(points: u8) -> usize {
    POINT_COUNTS[points.min(POINT_COUNTS.len() as u8 - 1) as usize]
}

fn position_in(hz: f32, top: f32) -> f64 {
    let low = band_centre(0).ln();
    let high = top.ln();
    (f64::from(hz.ln() - low) / f64::from(high - low)).clamp(0.0, 1.0)
}

fn sampled_levels(bands: &[f32; BANDS], count: usize, top: f32) -> Vec<f32> {
    let low = band_centre(0);
    let span = (top / low).ln();
    let first = band_centre(0).ln();
    (0..count)
        .map(|index| {
            let fraction = if count <= 1 {
                0.0
            } else {
                index as f32 / (count - 1) as f32
            };
            let hz = low * (span * fraction).exp();
            let centre = ((hz.ln() - first) / (20_000f32 / low).ln() * BANDS as f32) - 0.5;
            let centre = centre.clamp(0.0, (BANDS - 1) as f32);
            let lower = centre.floor() as usize;
            let upper = (lower + 1).min(BANDS - 1);
            let weight = centre - lower as f32;
            bands[lower] * (1.0 - weight) + bands[upper] * weight
        })
        .collect()
}

fn draw(
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    bands: &[f32; BANDS],
    options: Options,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    if width <= 2.0 || height <= 2.0 {
        return;
    }
    let (floor, ceiling) = options.bounds;
    let span = (ceiling - floor).max(1.0);
    let top = top_frequency(options.frequency);
    let foreground = area.color();
    context.set_source_rgba(
        f64::from(foreground.red()),
        f64::from(foreground.green()),
        f64::from(foreground.blue()),
        0.14,
    );
    context.set_line_width(1.0);
    for hz in GRID_HZ {
        if hz > top {
            continue;
        }
        let x = position_in(hz, top) * width;
        context.move_to(x.round() + 0.5, 0.0);
        context.line_to(x.round() + 0.5, height);
    }
    context.move_to(0.0, height - 0.5);
    context.line_to(width, height - 0.5);
    let _ = context.stroke();

    let count = point_count(options.points);
    let levels = sampled_levels(bands, count, top);
    let x_of = |index: usize| {
        if count <= 1 {
            0.0
        } else {
            index as f64 / (count - 1) as f64 * width
        }
    };
    let y_of = |value: f32| {
        let level = f64::from(((value.clamp(floor, ceiling) - floor) / span).clamp(0.0, 1.0));
        height - 1.0 - level * (height - 1.0)
    };
    context.set_source_rgba(0.35, 0.62, 1.0, 0.9);
    match options.shape {
        1 => {
            for (index, value) in levels.iter().enumerate() {
                let (x, y) = (x_of(index), y_of(*value));
                if index == 0 {
                    context.move_to(x, y);
                } else {
                    context.line_to(x, y);
                }
            }
            let _ = context.stroke();
        }
        2 => {
            context.move_to(0.0, height - 1.0);
            for (index, value) in levels.iter().enumerate() {
                context.line_to(x_of(index), y_of(*value));
            }
            context.line_to(width, height - 1.0);
            context.close_path();
            context.set_source_rgba(0.35, 0.62, 1.0, 0.35);
            let _ = context.fill();
            context.set_source_rgba(0.35, 0.62, 1.0, 0.9);
            for (index, value) in levels.iter().enumerate() {
                let (x, y) = (x_of(index), y_of(*value));
                if index == 0 {
                    context.move_to(x, y);
                } else {
                    context.line_to(x, y);
                }
            }
            let _ = context.stroke();
        }
        _ => {
            let gap = 1.0;
            let bar_width = (width - gap * (count - 1) as f64) / count as f64;
            if bar_width <= 0.2 {
                return;
            }
            for (index, value) in levels.iter().enumerate() {
                let level =
                    f64::from(((value.clamp(floor, ceiling) - floor) / span).clamp(0.0, 1.0));
                if level <= 0.0 {
                    continue;
                }
                let bar_height = level * (height - 1.0);
                let x = index as f64 * (bar_width + gap);
                context.rectangle(x, height - 1.0 - bar_height, bar_width, bar_height);
                let _ = context.fill();
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
