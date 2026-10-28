use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use pipewire_effects_core::ChainModel;

use crate::action::Dispatch;
use crate::runtime::Runtime;
use crate::widgets::Meter;

pub struct State {
    pub runtime: Rc<Runtime>,
    window: RefCell<gtk::glib::WeakRef<adw::ApplicationWindow>>,
    pub toast: adw::ToastOverlay,
    pub dispatch: RefCell<Option<Dispatch>>,
    pub title: RefCell<Option<adw::WindowTitle>>,
    pub bypass: RefCell<Option<gtk::ToggleButton>>,
    pub list: RefCell<Option<gtk::ListBox>>,
    pub stack: RefCell<Option<gtk::Stack>>,
    pub editor: RefCell<Option<gtk::Box>>,
    pub caption_out: RefCell<Option<gtk::Label>>,
    pub preset_button: RefCell<Option<gtk::MenuButton>>,
    pub device_button: RefCell<Option<gtk::MenuButton>>,
    pub left_meter: RefCell<Option<Rc<Meter>>>,
    pub right_meter: RefCell<Option<Rc<Meter>>>,
    pub left_reading: RefCell<Option<gtk::Label>>,
    pub right_reading: RefCell<Option<gtk::Label>>,
    pub rate_reading: RefCell<Option<gtk::Label>>,
    pub latency_reading: RefCell<Option<gtk::Label>>,
    pub clip_warning: RefCell<Option<gtk::Image>>,
    pub selected: Cell<usize>,
    pub active_preset: RefCell<Option<String>>,
    pub closing: Cell<bool>,
}

impl State {
    pub fn new(runtime: Rc<Runtime>, toast: adw::ToastOverlay) -> Rc<Self> {
        Rc::new(Self {
            runtime,
            window: RefCell::new(gtk::glib::WeakRef::new()),
            toast,
            dispatch: RefCell::new(None),
            title: RefCell::new(None),
            bypass: RefCell::new(None),
            list: RefCell::new(None),
            stack: RefCell::new(None),
            editor: RefCell::new(None),
            caption_out: RefCell::new(None),
            preset_button: RefCell::new(None),
            device_button: RefCell::new(None),
            left_meter: RefCell::new(None),
            right_meter: RefCell::new(None),
            left_reading: RefCell::new(None),
            right_reading: RefCell::new(None),
            rate_reading: RefCell::new(None),
            latency_reading: RefCell::new(None),
            clip_warning: RefCell::new(None),
            selected: Cell::new(0),
            active_preset: RefCell::new(None),
            closing: Cell::new(false),
        })
    }

    pub fn dispatch(&self) -> Dispatch {
        self.dispatch
            .borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(|_| {}))
    }

    pub fn set_dispatch(&self, dispatch: &Dispatch) {
        *self.dispatch.borrow_mut() = Some(Rc::clone(dispatch));
    }

    pub fn with_model<T>(&self, f: impl FnOnce(&ChainModel) -> T) -> T {
        self.runtime.controller().with_model(f)
    }

    pub fn set_window(&self, window: &adw::ApplicationWindow) {
        *self.window.borrow_mut() = window.downgrade();
    }

    pub fn window(&self) -> Option<adw::ApplicationWindow> {
        self.window.borrow().upgrade()
    }

    pub fn toast(&self, message: &str) {
        self.toast.add_toast(adw::Toast::new(message));
    }

    pub fn report(&self, result: Result<(), pipewire_effects_core::CoreError>) {
        if let Err(error) = result {
            self.toast(&error.to_string());
        }
    }

    pub fn set_active_preset(&self, name: Option<String>) {
        *self.active_preset.borrow_mut() = name;
        self.update_title();
    }

    pub fn update_title(&self) {
        let Some(title) = self.title.borrow().clone() else {
            return;
        };
        let preset = self.active_preset.borrow().clone();
        let effects = self.runtime.controller().len();
        let subtitle = match preset {
            Some(name) => format!("{name} · {effects} effect(s)"),
            None => format!("{effects} effect(s) · not saved"),
        };
        title.set_subtitle(&subtitle);
    }

    pub fn sync_bypass(&self) {
        let bypassed = self.runtime.controller().is_bypassed();
        if let Some(bypass) = self.bypass.borrow().as_ref() {
            if bypass.is_active() != !bypassed {
                bypass.set_active(!bypassed);
            }
        }
    }

    pub fn begin(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
            let Some(state) = weak.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            state.drain_events();
            state.update_meters();
            state.update_counters();
            if state.closing.get() {
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    }
}
