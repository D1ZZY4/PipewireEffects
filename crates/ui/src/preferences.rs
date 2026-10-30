use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::t;

use crate::pages::WorkspaceActions;

pub struct PreferencesView {
    autostart: adw::SwitchRow,
    developer: adw::SwitchRow,
    backend: adw::ComboRow,
    toggles: Rc<Cell<bool>>,
}

impl PreferencesView {
    pub fn new(actions: WorkspaceActions) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();
        let toggles = Rc::new(Cell::new(false));

        let behaviour = adw::PreferencesGroup::new();
        behaviour.set_title(t("Behaviour"));
        let autostart = adw::SwitchRow::new();
        autostart.set_title(t("Start on login"));
        autostart.set_subtitle(t(
            "Begins processing as soon as you sign in, without a window",
        ));
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

        let processing = adw::PreferencesGroup::new();
        processing.set_title(t("Audio processing"));
        let backend = adw::ComboRow::new();
        backend.set_title(t("Processing backend"));
        backend.set_subtitle(t(
            "GPU off-loads the gain stage; filters and dynamics stay on the CPU. Both compares them.",
        ));
        let model = gtk::StringList::new(&[]);
        model.append(t("CPU"));
        model.append(t("GPU"));
        model.append(t("Both"));
        backend.set_model(Some(&model));
        {
            let flag = Rc::clone(&toggles);
            let on_backend = Rc::clone(&actions.on_audio_backend);
            backend.connect_selected_notify(move |row| {
                if flag.get() {
                    return;
                }
                let value = match row.selected() {
                    1 => "gpu",
                    2 => "both",
                    _ => "cpu",
                };
                on_backend(value.to_owned());
            });
        }
        processing.add(&backend);
        page.add(&processing);

        let troubleshooting = adw::PreferencesGroup::new();
        troubleshooting.set_title(t("Troubleshooting"));
        let developer = adw::SwitchRow::new();
        developer.set_title(t("Developer mode"));
        developer.set_subtitle(t("Adds a Diagnostics entry to the main menu"));
        troubleshooting.add(&developer);
        page.add(&troubleshooting);

        {
            let flag = Rc::clone(&toggles);
            let on_developer_mode = Rc::clone(&actions.on_developer_mode);
            developer.connect_active_notify(move |row| {
                if flag.get() {
                    return;
                }
                on_developer_mode(row.is_active());
            });
        }

        let view = Self {
            autostart,
            developer,
            backend,
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
        self.toggles.set(false);
    }

    pub fn set_audio_backend(&self, value: &str) {
        let index = match value {
            "gpu" => 1,
            "both" => 2,
            _ => 0,
        };
        self.toggles.set(true);
        self.backend.set_selected(index);
        self.toggles.set(false);
    }
}
