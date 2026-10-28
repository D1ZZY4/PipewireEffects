use std::cell::Cell;
use std::rc::Rc;

use gtk::cairo::Context;
use gtk::prelude::*;

use crate::style;

pub const FLOOR_DB: f64 = -60.0;
const HOLD_DECAY: f64 = 0.010;

const GREEN: (f64, f64, f64) = (0.20, 0.82, 0.35);
const AMBER: (f64, f64, f64) = (0.95, 0.72, 0.15);
const RED: (f64, f64, f64) = (0.90, 0.25, 0.20);

const SEGMENTS: [(f64, (f64, f64, f64)); 3] = [(-12.0, GREEN), (-3.0, AMBER), (0.0, RED)];

pub struct Meter {
    area: gtk::DrawingArea,
    peak: Cell<f64>,
    hold: Cell<f64>,
    clipping: Cell<bool>,
}

impl Meter {
    pub fn new(width: i32) -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .width_request(width)
            .height_request(9)
            .valign(gtk::Align::Center)
            .build();
        area.set_accessible_role(gtk::AccessibleRole::ProgressBar);

        let meter = Rc::new(Self {
            area,
            peak: Cell::new(FLOOR_DB),
            hold: Cell::new(FLOOR_DB),
            clipping: Cell::new(false),
        });

        let weak = Rc::downgrade(&meter);
        meter.area.set_draw_func(move |area, cr, width, height| {
            if let Some(meter) = weak.upgrade() {
                meter.draw(area, cr, width, height);
            }
        });
        meter
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn update(&self, amplitude: f32, label: &str) {
        let db = amplitude_to_db(amplitude);
        let peak = db.clamp(FLOOR_DB, 0.0);
        let previous = self.hold.get();
        let hold = if peak > previous {
            peak
        } else {
            (previous - HOLD_DECAY).max(peak)
        };

        if db >= 0.0 {
            self.clipping.set(true);
        }

        self.peak.set(peak);
        self.hold.set(hold);
        self.area
            .update_property(&[gtk::accessible::Property::ValueText(label)]);
        self.area.queue_draw();
    }

    pub fn reset(&self) {
        self.peak.set(FLOOR_DB);
        self.hold.set(FLOOR_DB);
        self.clipping.set(false);
        self.area.queue_draw();
    }

    pub fn is_clipping(&self) -> bool {
        self.clipping.get()
    }

    fn draw(&self, area: &gtk::DrawingArea, cr: &Context, width: i32, height: i32) {
        let width = f64::from(width);
        let height = f64::from(height);
        let radius = height / 2.0;

        let foreground = area.color();
        cr.set_source_rgba(
            f64::from(foreground.red()),
            f64::from(foreground.green()),
            f64::from(foreground.blue()),
            0.14,
        );
        rounded(cr, 0.0, 0.0, width, height, radius);
        let _ = cr.fill();

        let filled = width * position_of(self.peak.get());
        let mut left = 0.0;
        for (top_db, color) in SEGMENTS {
            let right = width * position_of(top_db);
            let visible = filled.min(right) - left;
            if visible > 0.5 {
                cr.set_source_rgba(color.0, color.1, color.2, 1.0);
                cr.rectangle(left, 0.0, visible, height);
                let _ = cr.fill();
            }
            left = right;
        }

        let hold_x = width * position_of(self.hold.get());
        if hold_x > 1.0 {
            cr.set_source_rgba(
                f64::from(foreground.red()),
                f64::from(foreground.green()),
                f64::from(foreground.blue()),
                0.85,
            );
            cr.rectangle((hold_x - 2.0).max(0.0), 0.0, 2.0, height);
            let _ = cr.fill();
        }

        if self.clipping.get() {
            cr.set_source_rgba(RED.0, RED.1, RED.2, 1.0);
            rounded(cr, (width - radius).max(0.0), 0.0, radius, height, radius);
            let _ = cr.fill();
        }
    }
}

fn rounded(cr: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    cr.new_sub_path();
    cr.arc(
        x + width - radius,
        y + radius,
        radius,
        -90f64.to_radians(),
        0.0,
    );
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        90f64.to_radians(),
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        90f64.to_radians(),
        180f64.to_radians(),
    );
    cr.arc(
        x + radius,
        y + radius,
        radius,
        180f64.to_radians(),
        270f64.to_radians(),
    );
    cr.close_path();
}

fn position_of(db: f64) -> f64 {
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

pub fn amplitude_to_db(amplitude: f32) -> f64 {
    if !amplitude.is_finite() || amplitude <= 1e-6 {
        return FLOOR_DB;
    }
    f64::from(20.0 * amplitude.log10())
}

pub fn format_db(amplitude: f32) -> String {
    let db = amplitude_to_db(amplitude);
    if db <= FLOOR_DB {
        return String::from("  -inf");
    }
    format!("{db:6.1}")
}

pub fn reading_classes(amplitude: f32) -> &'static str {
    let db = amplitude_to_db(amplitude);
    if db >= 0.0 {
        style::STATUS_CLIPPING
    } else if db >= -6.0 {
        style::STATUS_HOT
    } else {
        ""
    }
}

#[cfg(test)]
mod tests;
