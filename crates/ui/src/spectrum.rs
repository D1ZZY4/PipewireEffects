use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::{BANDS, band_centre};

use crate::format::{db_text, frequency_text};

const DISPLAY_FLOOR_DB: f32 = -72.0;
const DISPLAY_CEILING_DB: f32 = 6.0;
const DISPLAY_SPAN: f32 = DISPLAY_CEILING_DB - DISPLAY_FLOOR_DB;
const GRID_HZ: [f32; 3] = [100.0, 1_000.0, 10_000.0];

pub struct SpectrumDisplay {
    area: gtk::DrawingArea,
    label: gtk::Label,
    state: Rc<RefCell<[f32; BANDS]>>,
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
        area.set_content_height(54);
        area.set_hexpand(true);
        area.update_property(&[gtk::accessible::Property::Label(t("Live audio spectrum"))]);

        let state: Rc<RefCell<[f32; BANDS]>> = Rc::new(RefCell::new([DISPLAY_FLOOR_DB; BANDS]));
        let borrowed = Rc::clone(&state);
        area.set_draw_func(move |area, context, width, height| {
            draw(area, context, width, height, &borrowed.borrow());
        });

        container.append(&header);
        container.append(&area);
        (container, Self { area, label, state })
    }

    pub fn update(&self, bands: [f32; BANDS]) {
        let mut peak_index = 0;
        let mut peak_db = DISPLAY_FLOOR_DB;
        for (index, value) in bands.iter().enumerate() {
            if *value > peak_db {
                peak_db = *value;
                peak_index = index;
            }
        }
        let caption = if peak_db <= DISPLAY_FLOOR_DB {
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

fn draw(
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    bands: &[f32; BANDS],
) {
    let width = f64::from(width);
    let height = f64::from(height);
    if width <= 2.0 || height <= 2.0 {
        return;
    }
    let foreground = area.color();
    context.set_source_rgba(
        f64::from(foreground.red()),
        f64::from(foreground.green()),
        f64::from(foreground.blue()),
        0.14,
    );
    context.set_line_width(1.0);
    for hz in GRID_HZ {
        let x = position_of(hz) * width;
        context.move_to(x.round() + 0.5, 0.0);
        context.line_to(x.round() + 0.5, height);
    }
    context.move_to(0.0, height - 0.5);
    context.line_to(width, height - 0.5);
    let _ = context.stroke();

    let gap = 1.0;
    let bar_width = (width - gap * (BANDS - 1) as f64) / BANDS as f64;
    if bar_width <= 0.2 {
        return;
    }
    for (index, value) in bands.iter().enumerate() {
        let level = f64::from(
            ((value.clamp(DISPLAY_FLOOR_DB, DISPLAY_CEILING_DB) - DISPLAY_FLOOR_DB) / DISPLAY_SPAN)
                .clamp(0.0, 1.0),
        );
        if level <= 0.0 {
            continue;
        }
        let bar_height = level * (height - 1.0);
        let x = index as f64 * (bar_width + gap);
        context.set_source_rgba(0.35, 0.62, 1.0, 0.9);
        context.rectangle(x, height - 1.0 - bar_height, bar_width, bar_height);
        let _ = context.fill();
    }
}

fn position_of(hz: f32) -> f64 {
    let low = band_centre(0).ln();
    let high = band_centre(BANDS - 1).ln();
    (f64::from(hz.ln() - low) / f64::from(high - low)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_positions_are_inside_the_plot_and_ordered() {
        let positions: Vec<f64> = GRID_HZ.iter().map(|hz| position_of(*hz)).collect();
        assert!(positions[0] < positions[1] && positions[1] < positions[2]);
        assert!(positions.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn the_floor_maps_to_the_bottom_and_the_ceiling_to_the_top() {
        let bottom =
            f64::from(((DISPLAY_FLOOR_DB - DISPLAY_FLOOR_DB) / DISPLAY_SPAN).clamp(0.0, 1.0));
        let top =
            f64::from(((DISPLAY_CEILING_DB - DISPLAY_FLOOR_DB) / DISPLAY_SPAN).clamp(0.0, 1.0));
        assert!(bottom.abs() < 1.0e-6);
        assert!((top - 1.0).abs() < 1.0e-6);
    }
}
