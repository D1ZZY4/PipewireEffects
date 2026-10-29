use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::{EngineState, Snapshot, StatsSnapshot};

use crate::diagnostics::Diagnostics;
use crate::pages::WorkspaceActions;

pub struct PreferencesView {
    autostart: adw::SwitchRow,
    developer: adw::SwitchRow,
    diagnostics: Rc<Diagnostics>,
    toggles: Rc<Cell<bool>>,
}

impl PreferencesView {
    pub fn new(actions: WorkspaceActions) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();

        let behaviour = adw::PreferencesGroup::new();
        behaviour.set_title(t("Behaviour"));
        let autostart = adw::SwitchRow::new();
        autostart.set_title(t("Start on login"));
        autostart.set_subtitle(t(
            "Begins processing as soon as you sign in, without a window",
        ));
        let toggles = Rc::new(Cell::new(false));
        {
            let flag = Rc::clone(&toggles);
            let on_autostart = Rc::clone(&actions.on_autostart);
            autostart.connect_active_notify(move |row| {
                if flag.get() {
                    return;
                }
                on_autostart(row.is_active());
            });
        }
        behaviour.add(&autostart);
        page.add(&behaviour);

        let troubleshooting = adw::PreferencesGroup::new();
        troubleshooting.set_title(t("Troubleshooting"));
        let developer = adw::SwitchRow::new();
        developer.set_title(t("Developer mode"));
        developer.set_subtitle(t("Shows the engine counters and diagnostic messages below"));
        troubleshooting.add(&developer);
        page.add(&troubleshooting);

        let diagnostics = Rc::new(Diagnostics::new());
        diagnostics.set_visible(false);
        page.add(diagnostics.counters());
        page.add(diagnostics.messages());

        {
            let flag = Rc::clone(&toggles);
            let on_developer_mode = Rc::clone(&actions.on_developer_mode);
            let diagnostics = Rc::clone(&diagnostics);
            developer.connect_active_notify(move |row| {
                diagnostics.set_visible(row.is_active());
                if flag.get() {
                    return;
                }
                on_developer_mode(row.is_active());
            });
        }

        let view = Self {
            autostart,
            developer,
            diagnostics,
            toggles,
        };
        (page, view)
    }

    pub fn set_autostart(&self, enabled: bool) {
        self.toggles.set(true);
        self.autostart.set_active(enabled);
        self.toggles.set(false);
    }

    pub fn set_developer_mode(&self, enabled: bool) {
        self.toggles.set(true);
        self.developer.set_active(enabled);
        self.diagnostics.set_visible(enabled);
        self.toggles.set(false);
    }

    pub fn expand_diagnostics(&self) {
        self.diagnostics.set_visible(true);
        self.diagnostics.expand();
    }

    pub fn update(&self, snapshot: &Snapshot, state: EngineState, stats: &StatsSnapshot) {
        self.diagnostics
            .update(snapshot, state, stats, &snapshot.warnings);
    }
}
