mod scale;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::cairo::Context;
use gtk::prelude::*;

use crate::style;
use crate::widgets::band_strip::scale::{format_frequency, gain_at, position_of};

const BAR_WIDTH: i32 = 30;
const BAR_HEIGHT: i32 = 96;
const HANDLE_HEIGHT: f64 = 7.0;

pub type BandCallback = Rc<dyn Fn(usize)>;
pub type GainCallback = Rc<dyn Fn(usize, f32)>;

pub struct BandStrip {
    root: gtk::Box,
    cells: RefCell<Vec<BandCell>>,
    selected: Cell<usize>,
    on_select: RefCell<Option<BandCallback>>,
    on_gain: RefCell<Option<GainCallback>>,
    on_toggle: RefCell<Option<BandCallback>>,
}

#[derive(Clone)]
struct BandCell {
    container: gtk::Box,
    area: gtk::DrawingArea,
    gain_label: gtk::Label,
    frequency_label: gtk::Label,
    gain: Rc<Cell<f32>>,
    enabled: Rc<Cell<bool>>,
}

impl BandStrip {
    pub fn new(count: usize) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.set_halign(gtk::Align::Center);

        let strip = Rc::new(Self {
            root,
            cells: RefCell::new(Vec::with_capacity(count)),
            selected: Cell::new(usize::MAX),
            on_select: RefCell::new(None),
            on_gain: RefCell::new(None),
            on_toggle: RefCell::new(None),
        });

        for index in 0..count {
            let cell = strip.build_cell(index);
            strip.root.append(&cell.container);
            strip.cells.borrow_mut().push(cell);
        }

        strip
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn set_on_select(&self, callback: BandCallback) {
        *self.on_select.borrow_mut() = Some(callback);
    }

    pub fn set_on_gain(&self, callback: GainCallback) {
        *self.on_gain.borrow_mut() = Some(callback);
    }

    pub fn set_on_toggle(&self, callback: BandCallback) {
        *self.on_toggle.borrow_mut() = Some(callback);
    }

    pub fn set_band(&self, index: usize, frequency: f32, gain_db: f32, enabled: bool) {
        let cells = self.cells.borrow();
        let Some(cell) = cells.get(index) else {
            return;
        };
        cell.gain.set(gain_db);
        cell.enabled.set(enabled);
        cell.gain_label.set_text(&format!("{gain_db:+.1}"));
        cell.frequency_label.set_text(&format_frequency(frequency));
        style::set_inactive(&cell.container, !enabled);
        cell.area.queue_draw();
    }

    pub fn set_selected(&self, index: usize) {
        let previous = self.selected.replace(index);
        for (position, cell) in self.cells.borrow().iter().enumerate() {
            let active = position == index;
            if active {
                style::add(&cell.container, style::BAND_CELL_ACTIVE);
            } else {
                style::remove(&cell.container, style::BAND_CELL_ACTIVE);
            }
            if position == previous || active {
                cell.area.queue_draw();
            }
        }
    }

    fn emit_select(&self, index: usize) {
        self.set_selected(index);
        let callback = self.on_select.borrow().clone();
        if let Some(callback) = callback {
            callback(index);
        }
    }

    fn emit_gain(&self, index: usize, gain_db: f32) {
        let callback = self.on_gain.borrow().clone();
        if let Some(callback) = callback {
            callback(index, gain_db);
        }
    }

    fn emit_toggle(&self, index: usize) {
        let callback = self.on_toggle.borrow().clone();
        if let Some(callback) = callback {
            callback(index);
        }
    }

    fn build_cell(self: &Rc<Self>, index: usize) -> BandCell {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
        style::add(&container, style::BAND_CELL);

        let gain_label = gtk::Label::new(Some("+0.0"));
        style::add(&gain_label, style::NUMERIC);
        gain_label.set_width_chars(5);
        gain_label.set_xalign(0.5);

        let area = gtk::DrawingArea::builder()
            .width_request(BAR_WIDTH)
            .height_request(BAR_HEIGHT)
            .build();
        area.set_accessible_role(gtk::AccessibleRole::Slider);
        area.set_tooltip_text(Some("Drag to set the gain, right click to toggle the band"));

        let frequency_label = gtk::Label::new(Some("1k"));
        style::add(&frequency_label, style::NUMERIC);
        frequency_label.set_width_chars(5);
        frequency_label.set_xalign(0.5);

        container.append(&gain_label);
        container.append(&area);
        container.append(&frequency_label);

        let cell = BandCell {
            container,
            area,
            gain_label,
            frequency_label,
            gain: Rc::new(Cell::new(0.0)),
            enabled: Rc::new(Cell::new(true)),
        };

        let weak = Rc::downgrade(self);
        let cell_for_draw = cell.clone();
        cell.area.set_draw_func(move |area, cr, width, height| {
            draw_band(
                area,
                cr,
                width,
                height,
                cell_for_draw.gain.get(),
                cell_for_draw.enabled.get(),
                weak.upgrade()
                    .is_some_and(|strip| strip.selected.get() == index),
            );
        });

        let weak = Rc::downgrade(self);
        let gain = Rc::clone(&cell.gain);
        let label = cell.gain_label.clone();
        let area_for_drag = cell.area.clone();
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(move |_, _x, y| {
            if let Some(strip) = weak.upgrade() {
                strip.emit_select(index);
                let _ = (y, &gain, &label, &area_for_drag);
            }
        });

        let weak = Rc::downgrade(self);
        let gain = Rc::clone(&cell.gain);
        let label = cell.gain_label.clone();
        let area_for_update = cell.area.clone();
        drag.connect_drag_update(move |_, _x, y| {
            let Some(strip) = weak.upgrade() else {
                return;
            };
            let value = gain_at(y, area_for_update.height());
            gain.set(value);
            label.set_text(&format!("{value:+.1}"));
            area_for_update.queue_draw();
            strip.emit_gain(index, value);
        });
        cell.area.add_controller(drag);

        let weak = Rc::downgrade(self);
        let click = gtk::GestureClick::new();
        click.set_button(gtk::gdk::BUTTON_SECONDARY);
        click.connect_pressed(move |_, _, _, _| {
            if let Some(strip) = weak.upgrade() {
                strip.emit_toggle(index);
            }
        });
        cell.area.add_controller(click);

        let weak = Rc::downgrade(self);
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |_, _, _, _| {
            if let Some(strip) = weak.upgrade() {
                strip.emit_select(index);
            }
        });
        cell.frequency_label.add_controller(click);

        cell
    }
}

fn draw_band(
    area: &gtk::DrawingArea,
    cr: &Context,
    width: i32,
    height: i32,
    gain_db: f32,
    enabled: bool,
    selected: bool,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    let color = area.color();
    let center_x = width / 2.0;
    let zero_y = height * position_of(0.0);
    let value_y = height * position_of(f64::from(gain_db));

    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        0.14,
    );
    cr.rectangle(center_x - 2.0, 0.0, 4.0, height);
    let _ = cr.fill();

    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        0.35,
    );
    cr.set_line_width(1.0);
    cr.move_to(center_x - 7.0, zero_y);
    cr.line_to(center_x + 7.0, zero_y);
    let _ = cr.stroke();

    let alpha = if !enabled {
        0.25
    } else if selected {
        1.0
    } else {
        0.7
    };
    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        alpha,
    );
    let top = value_y.min(zero_y);
    let bar_height = (value_y - zero_y).abs().max(2.0);
    cr.rectangle(center_x - 5.0, top, 10.0, bar_height);
    let _ = cr.fill();

    cr.rectangle(
        center_x - 9.0,
        value_y - HANDLE_HEIGHT / 2.0,
        18.0,
        HANDLE_HEIGHT,
    );
    let _ = cr.fill();
}
