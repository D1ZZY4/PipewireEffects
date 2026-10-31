use std::rc::Rc;

use pwe_core::PipelineKind;

use super::Ui;

impl Ui {
    pub fn sync_desktop_switches(self: &Rc<Self>) {
        let state = self.state.borrow();
        self.devices_view
            .set_running(state.chain(PipelineKind::Input).running());
        self.preferences_view
            .set_autostart(state.settings.autostart);
        self.preferences_view
            .set_developer_mode(state.settings.developer_mode);
        self.preferences_view
            .set_auto_save(state.settings.auto_save);
        self.preferences_view
            .set_auto_save_interval(state.settings.auto_save_interval);
        self.preferences_view
            .set_audio_backend(&state.settings.audio_backend);
        self.preferences_view
            .set_spectrum_range(state.settings.spectrum_range);
        self.preferences_view
            .set_spectrum_shape(state.settings.spectrum_shape);
        self.preferences_view
            .set_spectrum_points(state.settings.spectrum_points);
        self.preferences_view
            .set_spectrum_height(state.settings.spectrum_height);
        self.preferences_view
            .set_spectrum_frequency_range(state.settings.spectrum_frequency_range);
        self.preferences_view
            .set_spectrum_fft(state.settings.spectrum_fft);
        self.preferences_view
            .set_spectrum_window(state.settings.spectrum_window);
        self.preferences_view
            .set_spectrum_auto_range(state.settings.spectrum_auto_range);
        self.diagnostics_action
            .set_enabled(state.settings.developer_mode);
    }
}
