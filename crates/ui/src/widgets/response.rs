mod axis;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::cairo::Context;
use gtk::prelude::*;

use crate::widgets::response::axis::{
    MAX_DB, MIN_DB, frequency_of, frequency_position, gain_of, gain_position,
};

const CURVE_POINTS: usize = 200;
const PAD_LEFT: f64 = 30.0;
const PAD_BOTTOM: f64 = 16.0;
const PAD_TOP: f64 = 10.0;
const PAD_RIGHT: f64 = 10.0;
const HANDLE_RADIUS: f64 = 6.0;
const GRID_HZ: [f64; 9] = [
    20.0, 50.0, 100.0, 250.0, 1_000.0, 2_500.0, 5_000.0, 10_000.0, 20_000.0,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandHandle {
    pub frequency: f32,
    pub gain_db: f32,
    pub enabled: bool,
}

pub type CurveEvaluator = Rc<dyn Fn(f32) -> Option<f32>>;
pub type BandMoved = Rc<dyn Fn(usize, f32, f32)>;

pub struct ResponseCurve {
    area: gtk::DrawingArea,
    bands: RefCell<Vec<BandHandle>>,
    evaluate: RefCell<Option<CurveEvaluator>>,
    sample_rate: Cell<f32>,
    selected: Cell<usize>,
    on_drag: RefCell<Option<BandMoved>>,
    drag_index: Cell<Option<usize>>,
    drag_origin: Cell<(f64, f64)>,
}

impl ResponseCurve {
    pub fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .height_request(190)
            .hexpand(true)
            .build();
        area.set_accessible_role(gtk::AccessibleRole::Img);
        area.update_property(&[gtk::accessible::Property::Label("Frequency response")]);
        area.set_tooltip_text(Some("Drag a point to change that band"));

        let curve = Rc::new(Self {
            area,
            bands: RefCell::new(Vec::new()),
            evaluate: RefCell::new(None),
            sample_rate: Cell::new(48_000.0),
            selected: Cell::new(usize::MAX),
            on_drag: RefCell::new(None),
            drag_index: Cell::new(None),
            drag_origin: Cell::new((0.0, 0.0)),
        });

        let weak = Rc::downgrade(&curve);
        curve.area.set_draw_func(move |area, cr, width, height| {
            if let Some(curve) = weak.upgrade() {
                curve.draw(area, cr, width, height);
            }
        });

        curve.install_drag();
        curve
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn set_bands(&self, bands: Vec<BandHandle>) {
        *self.bands.borrow_mut() = bands;
        self.area.queue_draw();
    }

    pub fn set_evaluator(&self, evaluate: CurveEvaluator, sample_rate: f32) {
        *self.evaluate.borrow_mut() = Some(evaluate);
        self.sample_rate.set(sample_rate);
        self.area.queue_draw();
    }

    pub fn set_selected(&self, index: usize) {
        self.selected.set(index);
        self.area.queue_draw();
    }

    pub fn set_on_drag(&self, on_drag: BandMoved) {
        *self.on_drag.borrow_mut() = Some(on_drag);
    }

    pub fn refresh(&self) {
        self.area.queue_draw();
    }

    fn install_drag(self: &Rc<Self>) {
        let drag = gtk::GestureDrag::new();
        let weak = Rc::downgrade(self);

        drag.connect_drag_begin(move |_, x, y| {
            let Some(curve) = weak.upgrade() else {
                return;
            };
            curve.drag_origin.set((x, y));
            curve.drag_index.set(curve.nearest_band(x, y));
            if let Some(index) = curve.drag_index.get() {
                curve.set_selected(index);
            }
        });

        let weak = Rc::downgrade(self);
        drag.connect_drag_update(move |_, offset_x, offset_y| {
            let Some(curve) = weak.upgrade() else {
                return;
            };
            let Some(index) = curve.drag_index.get() else {
                return;
            };
            let (x0, y0) = curve.drag_origin.get();
            curve.commit_drag(index, x0 + offset_x, y0 + offset_y);
        });

        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |_, offset_x, offset_y| {
            let Some(curve) = weak.upgrade() else {
                return;
            };
            if let Some(index) = curve.drag_index.take() {
                let (x0, y0) = curve.drag_origin.get();
                curve.commit_drag(index, x0 + offset_x, y0 + offset_y);
            }
        });

        self.area.add_controller(drag);
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        let width = f64::from(self.area.width());
        let height = f64::from(self.area.height());
        (
            PAD_LEFT,
            PAD_TOP,
            (width - PAD_LEFT - PAD_RIGHT).max(1.0),
            (height - PAD_TOP - PAD_BOTTOM).max(1.0),
        )
    }

    fn nearest_band(&self, x: f64, y: f64) -> Option<usize> {
        let bands = self.bands.borrow();
        if bands.is_empty() {
            return None;
        }
        let (_, _, plot_w, plot_h) = self.bounds();
        let mut best = None;
        let mut best_distance = f64::MAX;
        for (index, band) in bands.iter().enumerate() {
            let bx = PAD_LEFT + plot_w * frequency_position(f64::from(band.frequency));
            let by = PAD_TOP + plot_h * gain_position(f64::from(band.gain_db));
            let distance = (bx - x).powi(2) + (by - y).powi(2);
            if distance < best_distance {
                best_distance = distance;
                best = Some(index);
            }
        }
        best
    }

    fn commit_drag(&self, index: usize, x: f64, y: f64) {
        let (_, _, plot_w, plot_h) = self.bounds();
        let fx = ((x - PAD_LEFT) / plot_w).clamp(0.0, 1.0);
        let fy = ((y - PAD_TOP) / plot_h).clamp(0.0, 1.0);
        let frequency = frequency_of(fx) as f32;
        let gain_db = gain_of(fy) as f32;

        if let Some(band) = self.bands.borrow_mut().get_mut(index) {
            band.frequency = frequency;
            band.gain_db = gain_db;
        }
        self.area.queue_draw();

        let on_drag = self.on_drag.borrow().clone();
        if let Some(on_drag) = on_drag {
            on_drag(index, frequency, gain_db);
        }
    }

    fn draw(&self, area: &gtk::DrawingArea, cr: &Context, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let foreground = area.color();
        let (r, g, b) = (
            f64::from(foreground.red()),
            f64::from(foreground.green()),
            f64::from(foreground.blue()),
        );
        let (_, _, plot_w, plot_h) = self.bounds();

        cr.set_source_rgba(r, g, b, 0.06);
        cr.rectangle(PAD_LEFT, PAD_TOP, plot_w, plot_h);
        let _ = cr.fill();

        cr.set_line_width(1.0);
        cr.set_source_rgba(r, g, b, 0.16);
        for hz in GRID_HZ {
            let x = PAD_LEFT + plot_w * frequency_position(hz);
            cr.move_to(x, PAD_TOP);
            cr.line_to(x, PAD_TOP + plot_h);
        }
        for db in [MIN_DB, MIN_DB / 2.0, 0.0, MAX_DB / 2.0, MAX_DB] {
            let y = PAD_TOP + plot_h * gain_position(db);
            cr.move_to(PAD_LEFT, y);
            cr.line_to(PAD_LEFT + plot_w, y);
        }
        let _ = cr.stroke();

        cr.set_source_rgba(r, g, b, 0.35);
        let zero = PAD_TOP + plot_h * gain_position(0.0);
        cr.move_to(PAD_LEFT, zero);
        cr.line_to(PAD_LEFT + plot_w, zero);
        let _ = cr.stroke();

        self.draw_curve(area, cr, plot_w, plot_h);
        self.draw_handles(area, cr, plot_w, plot_h);
    }

    fn draw_curve(&self, area: &gtk::DrawingArea, cr: &Context, plot_w: f64, plot_h: f64) {
        let Some(evaluate) = self.evaluate.borrow().clone() else {
            return;
        };
        let color = area.color();
        cr.set_source_rgba(
            f64::from(color.red()),
            f64::from(color.green()),
            f64::from(color.blue()),
            0.9,
        );
        cr.set_line_width(2.0);

        for step in 0..=CURVE_POINTS {
            let fx = step as f64 / CURVE_POINTS as f64;
            let hz = frequency_of(fx);
            #[allow(clippy::cast_possible_truncation)]
            let Some(db) = evaluate(hz as f32) else {
                continue;
            };
            let x = PAD_LEFT + plot_w * fx;
            let y = PAD_TOP + plot_h * gain_position(f64::from(db).clamp(MIN_DB, MAX_DB));
            if step == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        let _ = cr.stroke();
    }

    fn draw_handles(&self, area: &gtk::DrawingArea, cr: &Context, plot_w: f64, plot_h: f64) {
        let bands = self.bands.borrow();
        let selected = self.selected.get();
        let accent = area.color();

        for (index, band) in bands.iter().enumerate() {
            let x = PAD_LEFT + plot_w * frequency_position(f64::from(band.frequency));
            let y = PAD_TOP + plot_h * gain_position(f64::from(band.gain_db));

            if index == selected {
                cr.set_source_rgba(
                    f64::from(accent.red()),
                    f64::from(accent.green()),
                    f64::from(accent.blue()),
                    0.9,
                );
            } else if band.enabled {
                cr.set_source_rgba(
                    f64::from(accent.red()),
                    f64::from(accent.green()),
                    f64::from(accent.blue()),
                    0.55,
                );
            } else {
                cr.set_source_rgba(
                    f64::from(accent.red()),
                    f64::from(accent.green()),
                    f64::from(accent.blue()),
                    0.2,
                );
            }

            cr.arc(x, y, HANDLE_RADIUS, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.fill();

            cr.set_source_rgba(
                f64::from(accent.red()),
                f64::from(accent.green()),
                f64::from(accent.blue()),
                if index == selected { 1.0 } else { 0.0 },
            );
            cr.arc(x, y, HANDLE_RADIUS + 2.5, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.stroke();
        }
    }
}
