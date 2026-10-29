use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};
use pwe_engine::EffectSlot;

use crate::chain_view;
use crate::dialogs;
use crate::editor;
use crate::harness;
use crate::meter::LevelDisplay;
use crate::pages::Workspace;
use crate::presets_ui;
use crate::settings::SettingsView;
use crate::state::State;

type ChainSignature = (PipelineKind, (usize, bool, Option<EffectSlot>));

pub struct Ui {
    pub state: Rc<RefCell<State>>,
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub chain_list: gtk::ListBox,
    pub chain_placeholder: gtk::Widget,
    pub editor_host: gtk::Box,
    pub preset_list: gtk::ListBox,
    pub add_button: gtk::Button,
    pub bypass: gtk::ToggleButton,
    pub banner: adw::Banner,
    pub meter_in: LevelDisplay,
    pub meter_out: LevelDisplay,
    pub settings_view: SettingsView,
    pub stack: adw::ViewStack,
    pub pipeline_buttons: [gtk::ToggleButton; 2],
    pipeline_guard: Cell<bool>,
    signature: RefCell<ChainSignature>,
}

impl Ui {
    pub fn build(app: &adw::Application, state: State) -> Rc<Self> {
        let state = Rc::new(RefCell::new(state));
        let actions = crate::actions::workspace_actions(&state);
        let workspace = Workspace::build(app, actions);

        let ui = Rc::new(Self {
            state,
            window: workspace.window,
            toast: workspace.toast,
            chain_list: workspace.chain_list,
            chain_placeholder: workspace.chain_placeholder,
            editor_host: workspace.editor_host,
            preset_list: workspace.preset_list,
            add_button: workspace.add_button,
            bypass: workspace.bypass,
            banner: workspace.banner,
            meter_in: workspace.meter_in,
            meter_out: workspace.meter_out,
            settings_view: workspace.settings_view,
            stack: workspace.stack,
            pipeline_buttons: workspace.pipeline_buttons,
            pipeline_guard: Cell::new(false),
            signature: RefCell::new((PipelineKind::Output, (0, false, None))),
        });

        ui.wire(workspace.preset_buttons);
        presets_ui::populate(&ui);
        ui.sync_pipeline_buttons();
        ui.sync_desktop_switches();
        ui.rebuild_chain();
        ui.rebuild_editor();
        ui.start_polling();
        harness::install(&ui);
        ui
    }

    pub fn active(&self) -> PipelineKind {
        self.state.borrow().active()
    }

    pub fn set_pipeline(self: &Rc<Self>, kind: PipelineKind) {
        if self.pipeline_guard.get() || self.state.borrow().active() == kind {
            return;
        }
        self.state.borrow_mut().set_active(kind);
        self.sync_pipeline_buttons();
        self.rebuild_chain();
        self.rebuild_editor();
        self.refresh();
    }

    fn sync_pipeline_buttons(self: &Rc<Self>) {
        let active = self.active();
        self.pipeline_guard.set(true);
        for kind in pwe_core::ALL_PIPELINES {
            self.pipeline_buttons[kind.index()].set_active(kind == active);
        }
        self.pipeline_guard.set(false);
    }

    pub fn sync_desktop_switches(self: &Rc<Self>) {
        let state = self.state.borrow();
        self.settings_view
            .set_running(state.chain(PipelineKind::Input).running());
        self.settings_view.set_autostart(state.settings.autostart);
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
            if ui.state.borrow().bypassed() == wanted {
                return;
            }
            ui.state.borrow_mut().set_bypass(wanted);
        });

        let ui = Rc::clone(self);
        self.chain_list.connect_row_activated(move |_, row| {
            let Ok(value) = row.widget_name().parse::<u64>() else {
                return;
            };
            ui.state.borrow_mut().set_selected(Some(EffectSlot(value)));
            ui.rebuild_editor();
        });

        for kind in pwe_core::ALL_PIPELINES {
            let ui = Rc::clone(self);
            self.pipeline_buttons[kind.index()].connect_toggled(move |button| {
                if button.is_active() {
                    ui.set_pipeline(kind);
                }
            });
        }

        presets_ui::wire(self, preset_buttons);

        let ui = Rc::clone(self);
        self.banner.connect_button_clicked(move |_| {
            ui.banner.set_revealed(false);
        });
    }

    pub fn add_effect(self: &Rc<Self>, kind: &str) {
        let added = self.state.borrow_mut().add(kind);
        if added.is_none() {
            self.show_banner(t("The effect could not be added; the chain may be full"));
            return;
        }
        self.rebuild_chain();
        self.rebuild_editor();
        self.stack.set_visible_child_name("effects");
    }

    pub fn toggle_input(self: &Rc<Self>, enabled: bool) {
        if let Err(error) = self.state.borrow_mut().set_input_enabled(enabled) {
            self.show_banner(&tr(
                "The microphone pipeline could not start: {0}",
                &[&error.to_string()],
            ));
        }
        self.sync_desktop_switches();
        self.sync_chain_views();
    }

    pub fn sync_chain_views(self: &Rc<Self>) {
        self.rebuild_chain();
        self.rebuild_editor();
    }

    pub fn rebuild_chain(self: &Rc<Self>) {
        while let Some(child) = self.chain_list.first_child() {
            self.chain_list.remove(&child);
        }
        let (rows, total, running) = {
            let state = self.state.borrow();
            let chain = state.active_chain();
            let rows: Vec<(EffectSlot, &'static str, bool)> = chain
                .slots
                .iter()
                .map(|slot| (slot.id, slot.kind, slot.enabled))
                .collect();
            (rows, chain.slots.len(), chain.running())
        };
        for (id, kind, enabled) in rows {
            self.chain_list
                .append(&chain_view::row(self, id, kind, enabled));
        }
        self.chain_placeholder.set_visible(total == 0);
        self.chain_list.set_visible(total > 0);
        if !running && total == 0 {
            self.chain_placeholder
                .set_tooltip_text(Some(t("Turn on microphone processing in the Devices page")));
        }
    }

    pub fn rebuild_editor(self: &Rc<Self>) {
        let (built, selection) = {
            let state = self.state.borrow();
            let chain = state.active_chain();
            let built = chain.selected.and_then(|id| {
                chain
                    .slot(id)
                    .map(|slot| (slot.kind, editor::build(self, slot.kind, &slot.params)))
            });
            let selection = chain
                .slots
                .iter()
                .position(|slot| Some(slot.id) == chain.selected);
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
        let kind = self.active();
        let (snapshot, meters, stats, state, error) = {
            let state = self.state.borrow();
            let snapshot = state.snapshot_of(kind);
            let meters = state
                .chain(kind)
                .engine()
                .map_or_else(pwe_engine::MeterSnapshot::default, |engine| {
                    engine.shared().meters().snapshot()
                });
            (
                snapshot,
                meters,
                state.stats_of(kind),
                state.state_of(kind),
                state.error_of(kind),
            )
        };
        self.sync_view_signature();
        self.meter_in
            .update(meters.input_peak_db, meters.input_rms_db);
        self.meter_out
            .update(meters.output_peak_db, meters.output_rms_db);
        if self.bypass.is_active() != snapshot.bypassed {
            self.bypass.set_active(snapshot.bypassed);
            self.bypass.set_tooltip_text(Some(if snapshot.bypassed {
                t("The chain is bypassed; press to let the audio through")
            } else {
                t("Bypass the effect chain")
            }));
        }
        self.set_status(snapshot.bypassed);
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

    fn sync_view_signature(self: &Rc<Self>) {
        let (kind, signature) = {
            let state = self.state.borrow();
            let chain = state.active_chain();
            (
                state.active(),
                (
                    chain.slots.len(),
                    chain.running(),
                    chain.slots.iter().map(|slot| slot.id).max(),
                ),
            )
        };
        if *self.signature.borrow() == (kind, signature) {
            return;
        }
        *self.signature.borrow_mut() = (kind, signature);
        self.sync_desktop_switches();
        self.rebuild_chain();
    }

    fn start_polling(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            ui.refresh();
            glib::ControlFlow::Continue
        });
    }
}
