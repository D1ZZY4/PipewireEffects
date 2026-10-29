use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::StatsSnapshot;

use crate::diagnostics::Diagnostics;
use crate::pages::WorkspaceActions;

pub struct PreferencesView {
    page: adw::PreferencesPage,
    autostart: adw::SwitchRow,
    developer: adw::SwitchRow,
    diagnostics_group: adw::PreferencesGroup,
    diagnostics: Diagnostics,
    toggles: Rc<Cell<bool>>,
}

impl PreferencesView {
    pub fn new(actions: WorkspaceActions) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();

        let desktop = adw::PreferencesGroup::new();
        desktop.set_title(t("Desktop"));
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
        desktop.add(&autostart);
        page.add(&desktop);

        let developer_group = adw::PreferencesGroup::new();
        developer_group.set_title(t("Developer"));
        let developer = adw::SwitchRow::new();
        developer.set_title(t("Developer mode"));
        developer.set_subtitle(t(
            "Adds engine counters and diagnostic messages for troubleshooting",
        ));
        developer_group.add(&developer);
        page.add(&developer_group);

        let diagnostics_group = adw::PreferencesGroup::new();
        diagnostics_group.set_title(t("Diagnostics"));
        let diagnostics = Diagnostics::new();
        diagnostics_group.add(diagnostics.row());
        diagnostics_group.set_visible(false);
        page.add(&diagnostics_group);

        {
            let group = diagnostics_group.clone();
            let flag = Rc::clone(&toggles);
            let on_developer_mode = Rc::clone(&actions.on_developer_mode);
            developer.connect_active_notify(move |row| {
                group.set_visible(row.is_active());
                if flag.get() {
                    return;
                }
                on_developer_mode(row.is_active());
            });
        }

        let view = Self {
            page: page.clone(),
            autostart,
            developer,
            diagnostics_group,
            diagnostics,
            toggles,
        };
        (page, view)
    }

    pub fn page(&self) -> &adw::PreferencesPage {
        &self.page
    }

    pub fn set_autostart(&self, enabled: bool) {
        self.toggles.set(true);
        self.autostart.set_active(enabled);
        self.toggles.set(false);
    }

    pub fn set_developer_mode(&self, enabled: bool) {
        self.toggles.set(true);
        self.developer.set_active(enabled);
        self.diagnostics_group.set_visible(enabled);
        self.toggles.set(false);
    }

    pub fn developer_mode(&self) -> bool {
        self.developer.is_active()
    }

    pub fn expand_diagnostics(&self) {
        self.diagnostics_group.set_visible(true);
        self.diagnostics.expand();
    }

    pub fn update(&self, stats: &StatsSnapshot, warnings: &[String]) {
        self.diagnostics.update(stats, warnings);
    }
}
