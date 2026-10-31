use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::t;

use crate::pages::WorkspaceActions;

pub(super) struct SpectrumPreferences {
    range: adw::ComboRow,
    shape: adw::ComboRow,
    points: adw::ComboRow,
    height: adw::ComboRow,
    frequency: adw::ComboRow,
    fft: adw::ComboRow,
    window: adw::ComboRow,
    auto_range: adw::SwitchRow,
    toggles: Rc<Cell<bool>>,
}

impl SpectrumPreferences {
    pub(super) fn new(
        group: &adw::PreferencesGroup,
        actions: &WorkspaceActions,
        toggles: &Rc<Cell<bool>>,
    ) -> Self {
        let range = combo(
            t("Spectrum range"),
            t("Wide is -72 to +6 dB, standard -60 to 0 dB, loud -48 to +12 dB"),
            &["Wide", "Standard", "Loud"],
            {
                let flag = Rc::clone(toggles);
                let on_range = Rc::clone(&actions.on_spectrum_range);
                move |index| {
                    if !flag.get() {
                        on_range(index);
                    }
                }
            },
        );
        let shape = combo(
            t("Spectrum shape"),
            t("Draws the live spectrum as bars, a line or a filled line"),
            &["Bars", "Line", "Filled line"],
            {
                let flag = Rc::clone(toggles);
                let on_shape = Rc::clone(&actions.on_spectrum_shape);
                move |index| {
                    if !flag.get() {
                        on_shape(index);
                    }
                }
            },
        );
        let points = combo(
            t("Spectrum resolution"),
            t("The number of points the spectrum is drawn with"),
            &["24 points", "48 points", "96 points"],
            {
                let flag = Rc::clone(toggles);
                let on_points = Rc::clone(&actions.on_spectrum_points);
                move |index| {
                    if !flag.get() {
                        on_points(index);
                    }
                }
            },
        );
        let height = combo(
            t("Spectrum height"),
            t("How tall the spectrum is drawn"),
            &["Compact", "Normal", "Tall"],
            {
                let flag = Rc::clone(toggles);
                let on_height = Rc::clone(&actions.on_spectrum_height);
                move |index| {
                    if !flag.get() {
                        on_height(index);
                    }
                }
            },
        );
        let frequency = combo(
            t("Spectrum frequency range"),
            t("The highest frequency the spectrum shows"),
            &[
                "Full (20 Hz to 20 kHz)",
                "Upper range (20 Hz to 10 kHz)",
                "Speech (20 Hz to 5 kHz)",
            ],
            {
                let flag = Rc::clone(toggles);
                let on_range = Rc::clone(&actions.on_spectrum_frequency_range);
                move |index| {
                    if !flag.get() {
                        on_range(index);
                    }
                }
            },
        );
        let fft = combo(
            t("Spectrum transform"),
            t("A larger transform resolves lower bands but updates more slowly"),
            &["2048 points", "4096 points", "8192 points", "16384 points"],
            {
                let flag = Rc::clone(toggles);
                let on_fft = Rc::clone(&actions.on_spectrum_fft);
                move |index| {
                    if !flag.get() {
                        on_fft(index);
                    }
                }
            },
        );
        let window = combo(
            t("Spectrum window"),
            t("The window applied before the transform"),
            &["Hann", "Hamming", "Blackman"],
            {
                let flag = Rc::clone(toggles);
                let on_window = Rc::clone(&actions.on_spectrum_window);
                move |index| {
                    if !flag.get() {
                        on_window(index);
                    }
                }
            },
        );
        let auto_range = adw::SwitchRow::new();
        auto_range.set_title(t("Automatic spectrum range"));
        auto_range.set_subtitle(t("Fits the drawn range to the loudest band"));
        {
            let flag = Rc::clone(toggles);
            let on_auto = Rc::clone(&actions.on_spectrum_auto_range);
            auto_range.connect_active_notify(move |row| {
                if !flag.get() {
                    on_auto(row.is_active());
                }
            });
        }
        for row in [&range, &shape, &points, &height, &frequency] {
            group.add(row);
        }
        group.add(&fft);
        group.add(&window);
        group.add(&auto_range);
        Self {
            range,
            shape,
            points,
            height,
            frequency,
            fft,
            window,
            auto_range,
            toggles: Rc::clone(toggles),
        }
    }

    pub(super) fn set_range(&self, range: u8) {
        self.apply(&self.range, range, 2);
    }

    pub(super) fn set_shape(&self, shape: u8) {
        self.apply(&self.shape, shape, 2);
    }

    pub(super) fn set_points(&self, points: u8) {
        self.apply(&self.points, points, 2);
    }

    pub(super) fn set_height(&self, height: u8) {
        self.apply(&self.height, height, 2);
    }

    pub(super) fn set_frequency_range(&self, range: u8) {
        self.apply(&self.frequency, range, 2);
    }

    pub(super) fn set_fft(&self, fft: u8) {
        self.apply(&self.fft, fft, 3);
    }

    pub(super) fn set_window(&self, window: u8) {
        self.apply(&self.window, window, 2);
    }

    pub(super) fn set_auto_range(&self, enabled: bool) {
        self.toggles.set(true);
        self.auto_range.set_active(enabled);
        self.toggles.set(false);
    }

    fn apply(&self, row: &adw::ComboRow, value: u8, maximum: u8) {
        self.toggles.set(true);
        row.set_selected(u32::from(value.min(maximum)));
        self.toggles.set(false);
    }
}

fn combo(
    title: &str,
    subtitle: &str,
    labels: &'static [&'static str],
    on_selected: impl Fn(u8) + 'static,
) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let model = gtk::StringList::new(&[]);
    for label in labels {
        model.append(t(label));
    }
    row.set_model(Some(&model));
    let count = labels.len().max(1);
    row.connect_selected_notify(move |row| {
        on_selected(row.selected().min((count - 1) as u32) as u8);
    });
    row
}
