use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use pwe_engine::EffectSlot;

use crate::chain_view;
use crate::dialogs;
use crate::editor;
use crate::format;
use crate::harness;
use crate::meter::LevelDisplay;
use crate::pages::Workspace;
use crate::presets_ui;
use crate::settings::SettingsView;
use crate::state::State;

pub struct Ui {
    pub state: Rc<RefCell<State>>,
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub chain_list: gtk::ListBox,
    pub chain_placeholder: gtk::Widget,
    pub status_label: gtk::Label,
    pub editor_host: gtk::Box,
    pub preset_list: gtk::ListBox,
    pub add_button: gtk::Button,
    pub bypass: gtk::ToggleButton,
    pub banner: adw::Banner,
    pub meter_in: LevelDisplay,
    pub meter_out: LevelDisplay,
    pub settings_view: SettingsView,
    pub stack: adw::ViewStack,
}

impl Ui {
    pub fn build(app: &adw::Application, state: State) -> Rc<Self> {
        let state = Rc::new(RefCell::new(state));
        let target_state = Rc::clone(&state);
        let workspace = Workspace::build(
            app,
            Rc::new(move |target: String| {
                if let Err(error) = target_state.borrow().engine.set_target_device(target) {
                    target_state
                        .borrow()
                        .engine
                        .shared()
                        .add_warning(error.to_string());
                }
            }),
        );

        let ui = Rc::new(Self {
            state,
            window: workspace.window,
            toast: workspace.toast,
            chain_list: workspace.chain_list,
            chain_placeholder: workspace.chain_placeholder,
            status_label: workspace.status_label,
            editor_host: workspace.editor_host,
            preset_list: workspace.preset_list,
            add_button: workspace.add_button,
            bypass: workspace.bypass,
            banner: workspace.banner,
            meter_in: workspace.meter_in,
            meter_out: workspace.meter_out,
            settings_view: workspace.settings_view,
            stack: workspace.stack,
        });

        ui.wire(workspace.preset_buttons);
        presets_ui::populate(&ui);
        ui.rebuild_chain();
        ui.rebuild_editor();
        ui.start_polling();
        harness::install(&ui);
        ui
    }

    fn wire(self: &Rc<Self>, preset_buttons: (gtk::Button, gtk::Button, gtk::Button, gtk::Button)) {
        let ui = Rc::clone(self);
        self.add_button.connect_clicked(move |_| {
            let ui = Rc::clone(&ui);
            let window = ui.window.clone();
            dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
        });

        let ui = Rc::clone(self);
        self.bypass.connect_toggled(move |button| {
            let wanted = button.is_active();
            if ui.state.borrow().bypassed == wanted {
                return;
            }
            ui.state.borrow_mut().set_bypass(wanted);
        });

        let ui = Rc::clone(self);
        self.chain_list.connect_row_activated(move |_, row| {
            let Ok(value) = row.widget_name().parse::<u64>() else {
                return;
            };
            ui.state.borrow_mut().selected = Some(EffectSlot(value));
            ui.rebuild_editor();
        });

        presets_ui::wire(self, preset_buttons);

        let ui = Rc::clone(self);
        self.banner.connect_button_clicked(move |_| {
            ui.banner.set_revealed(false);
        });
    }

    pub fn add_effect(self: &Rc<Self>, kind: &str) {
        let added = self.state.borrow_mut().add(kind);
        if added.is_none() {
            self.show_banner("The effect could not be added; the chain may be full");
            return;
        }
        self.rebuild_chain();
        self.rebuild_editor();
        self.stack.set_visible_child_name("effects");
    }

    pub fn rebuild_chain(self: &Rc<Self>) {
        while let Some(child) = self.chain_list.first_child() {
            self.chain_list.remove(&child);
        }
        let (rows, total) = {
            let state = self.state.borrow();
            let rows: Vec<(EffectSlot, &'static str, bool)> = state
                .slots
                .iter()
                .map(|slot| (slot.id, slot.kind, slot.enabled))
                .collect();
            (rows, state.slots.len())
        };
        for (id, kind, enabled) in rows {
            self.chain_list
                .append(&chain_view::row(self, id, kind, enabled));
        }
        self.chain_placeholder.set_visible(total == 0);
        self.chain_list.set_visible(total > 0);
    }

    pub fn rebuild_editor(self: &Rc<Self>) {
        let (built, selection) = {
            let state = self.state.borrow();
            let built = state.selected.and_then(|id| {
                state
                    .slot(id)
                    .map(|slot| (slot.kind, editor::build(self, slot.kind, &slot.params)))
            });
            let selection = state
                .slots
                .iter()
                .position(|slot| Some(slot.id) == state.selected);
            (built, selection)
        };
        while let Some(child) = self.editor_host.first_child() {
            self.editor_host.remove(&child);
        }
        match built {
            Some((_kind, widget)) => {
                self.editor_host.append(&widget);
                if let Some(index) = selection
                    && let Some(row) = self.chain_list.row_at_index(index as i32)
                {
                    self.chain_list.select_row(Some(&row));
                }
            }
            None => {
                self.editor_host.append(&editor::empty_state(self));
            }
        }
    }

    pub fn show_banner(&self, message: &str) {
        self.banner.set_title(message);
        self.banner.set_revealed(true);
    }

    pub fn toast(&self, message: &str) {
        self.toast.add_toast(adw::Toast::new(message));
    }

    pub fn refresh(self: &Rc<Self>) {
        let (snapshot, meters, stats, state, error) = {
            let state = self.state.borrow();
            let shared = state.engine.shared();
            (
                shared.snapshot(),
                shared.meters().snapshot(),
                shared.stats().snapshot(),
                shared.state(),
                shared.error(),
            )
        };
        self.meter_in
            .update(meters.input_peak_db, meters.input_rms_db);
        self.meter_out
            .update(meters.output_peak_db, meters.output_rms_db);
        if self.bypass.is_active() != snapshot.bypassed {
            self.bypass.set_active(snapshot.bypassed);
        }
        self.set_status(snapshot.bypassed);
        self.status_label
            .set_label(&format::status_text(&snapshot, state));
        self.settings_view.update(&snapshot, &stats, state);
        if let Some(error) = error {
            self.banner.set_title(&error);
            self.banner.set_revealed(true);
        }
    }

    fn set_status(&self, bypassed: bool) {
        if bypassed {
            self.chain_list.add_css_class("chain-bypassed");
        } else {
            self.chain_list.remove_css_class("chain-bypassed");
        }
    }

    fn start_polling(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            ui.refresh();
            glib::ControlFlow::Continue
        });
    }
}
