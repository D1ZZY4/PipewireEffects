use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};
use pwe_engine::EffectSlot;

use crate::chain_view;
use crate::devices::DevicesView;
use crate::diagnostics::Diagnostics;
use crate::editor;
use crate::harness;
use crate::pages::{ChainPane, Workspace, page_name};
use crate::preferences::PreferencesView;
use crate::presets_ui;
use crate::state::State;

mod wiring;

type ChainSignature = (PipelineKind, (usize, bool, Option<EffectSlot>));

pub struct Ui {
    pub state: Rc<RefCell<State>>,
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub panes: [ChainPane; 2],
    pub banner: adw::Banner,
    pub devices_view: DevicesView,
    pub preferences_view: PreferencesView,
    pub diagnostics_view: Diagnostics,
    pub devices_dialog: adw::PreferencesDialog,
    pub preferences_dialog: adw::PreferencesDialog,
    pub diagnostics_dialog: adw::PreferencesDialog,
    diagnostics_action: gio::SimpleAction,
    pub stack: adw::ViewStack,
    pub sidebar_stack: gtk::Stack,
    pub split: adw::OverlaySplitView,
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
            panes: workspace.panes,
            banner: workspace.banner,
            devices_view: workspace.devices_view,
            preferences_view: workspace.preferences_view,
            diagnostics_view: workspace.diagnostics_view,
            devices_dialog: workspace.devices_dialog,
            preferences_dialog: workspace.preferences_dialog,
            diagnostics_dialog: workspace.diagnostics_dialog,
            diagnostics_action: workspace.diagnostics_action,
            stack: workspace.stack,
            sidebar_stack: workspace.sidebar_stack,
            split: workspace.split,
            pipeline_guard: Cell::new(false),
            signature: RefCell::new((PipelineKind::Output, (0, false, None))),
        });

        ui.wire();
        presets_ui::populate(&ui);
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

    fn pane(&self) -> &ChainPane {
        &self.panes[self.active().index()]
    }

    pub fn set_pipeline(self: &Rc<Self>, kind: PipelineKind) {
        if self.pipeline_guard.get() || self.state.borrow().active() == kind {
            return;
        }
        self.state.borrow_mut().set_active(kind);
        self.pipeline_guard.set(true);
        self.stack.set_visible_child_name(page_name(kind));
        self.sidebar_stack.set_visible_child_name(page_name(kind));
        self.pipeline_guard.set(false);
        self.rebuild_chain();
        self.rebuild_editor();
        self.refresh();
    }

    pub fn sync_desktop_switches(self: &Rc<Self>) {
        let state = self.state.borrow();
        self.devices_view
            .set_running(state.chain(PipelineKind::Input).running());
        self.preferences_view
            .set_autostart(state.settings.autostart);
        self.preferences_view
            .set_developer_mode(state.settings.developer_mode);
        self.diagnostics_action
            .set_enabled(state.settings.developer_mode);
    }

    pub fn add_effect(self: &Rc<Self>, kind: &str) {
        let added = self.state.borrow_mut().add(kind);
        if added.is_none() {
            self.show_banner(t("The effect could not be added; the chain may be full"));
            return;
        }
        self.rebuild_chain();
        self.rebuild_editor();
        self.stack.set_visible_child_name(page_name(self.active()));
    }

    pub fn toggle_input(self: &Rc<Self>, enabled: bool) {
        let result = self.state.borrow_mut().set_input_enabled(enabled);
        if let Err(error) = result {
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
        let pane = self.pane();
        let (slots, running) = {
            let state = self.state.borrow();
            let chain = state.active_chain();
            let slots: Vec<(EffectSlot, &'static str, bool)> = chain
                .slots
                .iter()
                .map(|slot| (slot.id, slot.kind, slot.enabled))
                .collect();
            (slots, chain.running())
        };
        chain_view::populate(self, &pane.list, &slots);
        let total = slots.len();
        pane.placeholder.set_visible(total == 0);
        pane.list.set_visible(total > 0);
        if !running && total == 0 {
            pane.placeholder.set_tooltip_text(Some(t(
                "Turn on microphone processing from the main menu, under Devices",
            )));
        }
        presets_ui::populate(self);
        self.restore_selection();
    }

    pub fn rebuild_editor(self: &Rc<Self>) {
        let pane = self.pane();
        let built = {
            let state = self.state.borrow();
            let chain = state.active_chain();
            chain.selected.and_then(|id| {
                chain
                    .slot(id)
                    .map(|slot| (slot.kind, editor::build(self, slot.kind, &slot.params)))
            })
        };
        while let Some(child) = pane.editor_host.first_child() {
            pane.editor_host.remove(&child);
        }
        match built {
            Some((_kind, widget)) => pane.editor_host.append(&widget),
            None => pane.editor_host.append(&editor::empty_state(self)),
        }
        self.restore_selection();
    }

    fn restore_selection(&self) {
        let pane = self.pane();
        let selected = self.state.borrow().active_chain().selected;
        let mut target: Option<gtk::ListBoxRow> = None;
        let mut child = pane.list.first_child();
        while let Some(current) = child {
            if let Some(row) = current.downcast_ref::<gtk::ListBoxRow>()
                && selected.map(|slot| slot.0.to_string()).as_deref() == Some(&row.widget_name())
            {
                target = Some(row.clone());
                break;
            }
            child = current.next_sibling();
        }
        pane.list.select_row(target.as_ref());
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
        let (snapshot, meters, stats, state, error, bypassed, developer) = {
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
                state.bypassed(),
                state.settings.developer_mode,
            )
        };
        self.diagnostics_action.set_enabled(developer);
        self.sync_view_signature();
        let pane = self.pane();
        pane.meter_in
            .update(meters.input_peak_db, meters.input_rms_db);
        pane.meter_out
            .update(meters.output_peak_db, meters.output_rms_db);
        for pane in &self.panes {
            let bypassed = self.state.borrow().chain(pane.kind).bypassed;
            if pane.effects.is_active() == bypassed {
                pane.effects.set_active(!bypassed);
                pane.effects.set_tooltip_text(Some(if bypassed {
                    t("The chain is bypassed; switch it on to run the effects")
                } else {
                    t("The chain is running; switch it off to bypass the effects")
                }));
            }
        }
        self.set_status(bypassed);
        self.devices_view.update(&snapshot, &stats, state);
        self.diagnostics_view
            .update(&snapshot, state, &stats, &snapshot.warnings);
        if let Some(error) = error {
            self.banner.set_title(&error);
            self.banner.set_revealed(true);
        }
    }

    fn set_status(&self, bypassed: bool) {
        let list = &self.pane().list;
        if bypassed {
            list.add_css_class("chain-bypassed");
        } else {
            list.remove_css_class("chain-bypassed");
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
