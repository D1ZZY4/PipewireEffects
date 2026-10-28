use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use pipewire_effects_engine::{ConnectionState, EngineEvent, StreamRole};

use crate::chain;
use crate::editor;
use crate::shell::State;
use crate::widgets::meter;

impl State {
    pub fn refresh_chain(self: &Rc<Self>) {
        let Some(list) = self.list.borrow().clone() else {
            return;
        };
        let dispatch = self.dispatch();
        self.runtime.controller().with_model(|model| {
            chain::fill_list(&list, model, &dispatch);
            select_row(&list, model.len(), self.selected.get());
        });
        self.update_title();
    }

    pub fn refresh_content(self: &Rc<Self>) {
        let (Some(editor), Some(stack)) =
            (self.editor.borrow().clone(), self.stack.borrow().clone())
        else {
            return;
        };

        while let Some(child) = editor.first_child() {
            editor.remove(&child);
        }

        let index = self.selected.get();
        let dispatch = self.dispatch();
        let sample_rate = self.runtime.config().format.sample_rate_f32();
        let page = self.runtime.controller().with_model(|model| {
            model
                .entry(index)
                .map(|entry| editor::build_page(entry, index, sample_rate, &dispatch))
        });

        match page {
            Some(page) => {
                editor.append(&page);
                stack.set_visible_child_name("effect");
            }
            None => stack.set_visible_child_name("empty"),
        }
    }

    pub fn refresh_devices(self: &Rc<Self>) {
        let Some(button) = self.device_button.borrow().clone() else {
            return;
        };
        let state = Rc::clone(self);
        button.set_popover(Some(&state.build_device_popover()));
    }

    pub fn refresh_presets(self: &Rc<Self>) {
        let Some(button) = self.preset_button.borrow().clone() else {
            return;
        };
        let state = Rc::clone(self);
        button.set_popover(Some(&state.build_preset_popover()));
    }

    pub fn update_counters(self: &Rc<Self>) {
        self.refresh_status();
    }

    pub fn refresh_status(self: &Rc<Self>) {
        let Some(handles) = self.runtime.handles() else {
            set_text(&self.rate_reading, "—");
            set_text(&self.latency_reading, "—");
            return;
        };
        let format = self.runtime.config().format;
        let stats = handles.stats();
        if let Some(caption) = self.caption_out.borrow().as_ref() {
            let device = self
                .runtime
                .config()
                .target_sink
                .unwrap_or_else(|| String::from("system default output"));
            caption.set_text(&device);
        }
        set_text(
            &self.rate_reading,
            &format!("{:.1} kHz", f64::from(format.sample_rate) / 1_000.0),
        );
        set_text(
            &self.latency_reading,
            &format!(
                "{:.1} ms",
                stats.latency_ms(format.sample_rate, format.channels)
            ),
        );
    }

    pub fn update_meters(self: &Rc<Self>) {
        let Some(handles) = self.runtime.handles() else {
            self.reset_meters();
            return;
        };

        let snapshot = handles.output_meters().snapshot();
        let left = snapshot.peak_of(0);
        let right = snapshot.peak_of(1);

        if let Some(meter) = self.left_meter.borrow().as_ref() {
            meter.update(left, &meter::format_db(left));
        }
        if let Some(meter) = self.right_meter.borrow().as_ref() {
            meter.update(right, &meter::format_db(right));
        }

        set_reading(&self.left_reading, left);
        set_reading(&self.right_reading, right);

        let clipping = self
            .left_meter
            .borrow()
            .as_ref()
            .is_some_and(|meter| meter.is_clipping())
            || self
                .right_meter
                .borrow()
                .as_ref()
                .is_some_and(|meter| meter.is_clipping());

        if let Some(warning) = self.clip_warning.borrow().as_ref() {
            warning.set_visible(clipping);
        }
    }

    fn reset_meters(self: &Rc<Self>) {
        if let Some(meter) = self.left_meter.borrow().as_ref() {
            meter.reset();
        }
        if let Some(meter) = self.right_meter.borrow().as_ref() {
            meter.reset();
        }
        set_text(&self.left_reading, "—");
        set_text(&self.right_reading, "—");
    }

    pub fn drain_events(self: &Rc<Self>) {
        for event in self.runtime.poll_events() {
            match event {
                EngineEvent::ConnectionChanged { role, state } => {
                    if role == StreamRole::Playback && state == ConnectionState::Error {
                        self.toast("The playback stream failed");
                    }
                }
                EngineEvent::FormatNegotiated { role, format } => {
                    if format != self.runtime.config().format {
                        self.toast(&format!("The {role} stream negotiated {format}"));
                    }
                }
                EngineEvent::Failed { message } => {
                    self.toast(&format!("Audio engine failed: {message}"));
                }
                _ => {}
            }
        }
    }
}

fn select_row(list: &gtk::ListBox, len: usize, index: usize) {
    if index >= len {
        return;
    }
    #[allow(clippy::cast_possible_wrap)]
    if let Some(row) = list.row_at_index(index as i32) {
        list.select_row(Some(&row));
    }
}

fn set_text(cell: &RefCell<Option<gtk::Label>>, text: &str) {
    if let Some(label) = cell.borrow().as_ref() {
        label.set_text(text);
    }
}

pub fn set_reading(cell: &RefCell<Option<gtk::Label>>, amplitude: f32) {
    let borrowed = cell.borrow();
    let Some(label) = borrowed.as_ref() else {
        return;
    };
    label.set_text(&meter::format_db(amplitude));
    label.remove_css_class(crate::style::STATUS_HOT);
    label.remove_css_class(crate::style::STATUS_CLIPPING);
    let class = meter::reading_classes(amplitude);
    if !class.is_empty() {
        label.add_css_class(class);
    }
}
