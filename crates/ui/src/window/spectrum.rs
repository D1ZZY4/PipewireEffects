use super::Ui;

impl Ui {
    pub(super) fn sync_spectrum_options(&self) {
        let (range, shape, points, height, frequency, auto_range) = {
            let state = self.state.borrow();
            (
                state.settings.spectrum_range,
                state.settings.spectrum_shape,
                state.settings.spectrum_points,
                state.settings.spectrum_height,
                state.settings.spectrum_frequency_range,
                state.settings.spectrum_auto_range,
            )
        };
        for pane in &self.panes {
            pane.spectrum.set_range(range);
            pane.spectrum.set_shape(shape);
            pane.spectrum.set_points(points);
            pane.spectrum.set_height(height);
            pane.spectrum.set_frequency_range(frequency);
            pane.spectrum.set_auto_range(auto_range);
        }
    }
}
