use pwe_core::ALL_PIPELINES;

use super::State;

const OPTIONS: usize = 3;

impl State {
    pub fn set_spectrum_range(&mut self, range: u8) {
        if set(&mut self.settings.spectrum_range, range) {
            self.persist();
        }
    }

    pub fn set_spectrum_shape(&mut self, shape: u8) {
        if set(&mut self.settings.spectrum_shape, shape) {
            self.persist();
        }
    }

    pub fn set_spectrum_points(&mut self, points: u8) {
        if set(&mut self.settings.spectrum_points, points) {
            self.persist();
        }
    }

    pub fn set_spectrum_height(&mut self, height: u8) {
        if set(&mut self.settings.spectrum_height, height) {
            self.persist();
        }
    }

    pub fn set_spectrum_frequency_range(&mut self, range: u8) {
        if set(&mut self.settings.spectrum_frequency_range, range) {
            self.persist();
        }
    }

    pub fn set_spectrum_fft(&mut self, fft: u8) {
        if set(&mut self.settings.spectrum_fft, fft) {
            self.persist();
            self.apply_spectrum_options();
        }
    }

    pub fn set_spectrum_window(&mut self, window: u8) {
        if set(&mut self.settings.spectrum_window, window) {
            self.persist();
            self.apply_spectrum_options();
        }
    }

    pub fn set_spectrum_auto_range(&mut self, enabled: bool) {
        if self.settings.spectrum_auto_range == enabled {
            return;
        }
        self.settings.spectrum_auto_range = enabled;
        self.persist();
    }

    pub(super) fn apply_spectrum_options(&self) {
        for kind in ALL_PIPELINES {
            if let Some(engine) = self.chain(kind).engine() {
                engine
                    .shared()
                    .spectrum()
                    .set_options(self.settings.spectrum_fft, self.settings.spectrum_window);
            }
        }
    }
}

fn set(field: &mut u8, value: u8) -> bool {
    let value = value.min(OPTIONS as u8 - 1);
    if *field == value {
        return false;
    }
    *field = value;
    true
}
