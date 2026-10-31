#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::{mark, t};

use crate::pages::WorkspaceActions;

const INTERVAL_SECONDS: [u32; 5] = [2, 5, 10, 30, 60];
const INTERVAL_LABELS: [&str; 5] = [
    mark("2 seconds"),
    mark("5 seconds"),
    mark("10 seconds"),
    mark("30 seconds"),
    mark("60 seconds"),
];

pub struct PreferencesView {
    autostart: adw::SwitchRow,
    developer: adw::SwitchRow,
    auto_save: adw::SwitchRow,
    auto_save_interval: adw::ComboRow,
    backend: adw::ComboRow,
    spectrum_range: adw::ComboRow,
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

        let interface = adw::PreferencesGroup::new();
        interface.set_title(t("Interface"));
        let spectrum_range = adw::ComboRow::new();
        spectrum_range.set_title(t("Spectrum range"));
        spectrum_range.set_subtitle(t("The decibel range the spectrum is drawn across"));
        let model = gtk::StringList::new(&[]);
        model.append(t("Wide (-72 to +6 dB)"));
        model.append(t("Standard (-60 to 0 dB)"));
        model.append(t("Loud (-48 to +12 dB)"));
        spectrum_range.set_model(Some(&model));
        {
            let flag = Rc::clone(&toggles);
            let on_spectrum_range = Rc::clone(&actions.on_spectrum_range);
            spectrum_range.connect_selected_notify(move |row| {
                if flag.get() {
                    return;
                }
                on_spectrum_range(row.selected().min(2) as u8);
            });
        }
        interface.add(&spectrum_range);
        page.add(&interface);

        let presets = adw::PreferencesGroup::new();
        presets.set_title(t("Presets"));
        let auto_save = adw::SwitchRow::new();
        auto_save.set_title(t("Auto-save preset edits"));
        auto_save.set_subtitle(t(
            "Writes the loaded preset and the settings back after an edit",
        ));
        let auto_save_interval = adw::ComboRow::new();
        auto_save_interval.set_title(t("Update interval"));
        auto_save_interval.set_subtitle(t("How often the automatic update runs"));
        auto_save_interval.set_sensitive(false);
        let model = gtk::StringList::new(&[]);
        for label in INTERVAL_LABELS {
            model.append(t(label));
        }
        auto_save_interval.set_model(Some(&model));
        {
            let flag = Rc::clone(&toggles);
            let on_auto_save = Rc::clone(&actions.on_auto_save);
            let interval = auto_save_interval.clone();
            auto_save.connect_active_notify(move |row| {
                interval.set_sensitive(row.is_active());
                if flag.get() {
                    return;
                }
                on_auto_save(row.is_active());
            });
        }
        {
            let flag = Rc::clone(&toggles);
            let on_interval = Rc::clone(&actions.on_auto_save_interval);
            auto_save_interval.connect_selected_notify(move |row| {
                if flag.get() {
                    return;
                }
                let index = (row.selected() as usize).min(INTERVAL_SECONDS.len() - 1);
                on_interval(INTERVAL_SECONDS[index]);
            });
        }
        presets.add(&auto_save);
        presets.add(&auto_save_interval);
        page.add(&presets);

        let autoload = adw::PreferencesGroup::new();
        autoload.set_title(t("Preset autoload"));
        let remember = adw::ActionRow::new();
        remember.set_title(t("Remember the loaded preset for this device"));
        remember.set_subtitle(t("Loads it again when this device is the destination"));
        let remember_button = gtk::Button::with_label(t("Remember"));
        remember_button.set_valign(gtk::Align::Center);
        remember_button.add_css_class("flat");
        remember.add_suffix(&remember_button);
        autoload.add(&remember);
        let forget = adw::ActionRow::new();
        forget.set_title(t("Forget the autoload rule for this device"));
        forget.set_subtitle(t(
            "Removes the rule, so the device keeps whatever preset is loaded",
        ));
        let forget_button = gtk::Button::with_label(t("Forget"));
        forget_button.set_valign(gtk::Align::Center);
        forget_button.add_css_class("flat");
        forget.add_suffix(&forget_button);
        autoload.add(&forget);
        page.add(&autoload);
        {
            let on_remember = Rc::clone(&actions.on_autoload_remember);
            remember_button.connect_clicked(move |_| on_remember());
        }
        {
            let on_forget = Rc::clone(&actions.on_autoload_forget);
            forget_button.connect_clicked(move |_| on_forget());
        }

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
            auto_save,
            auto_save_interval,
            backend,
            spectrum_range,
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

    pub fn set_auto_save(&self, enabled: bool) {
        self.toggles.set(true);
        self.auto_save.set_active(enabled);
        self.auto_save_interval.set_sensitive(enabled);
        self.toggles.set(false);
    }

    pub fn set_auto_save_interval(&self, seconds: u32) {
        let index = INTERVAL_SECONDS
            .iter()
            .position(|value| *value == seconds)
            .unwrap_or(INTERVAL_SECONDS.len() - 1);
        self.toggles.set(true);
        self.auto_save_interval.set_selected(index as u32);
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

    pub fn set_spectrum_range(&self, range: u8) {
        self.toggles.set(true);
        self.spectrum_range.set_selected(u32::from(range.min(2)));
        self.toggles.set(false);
    }
}
