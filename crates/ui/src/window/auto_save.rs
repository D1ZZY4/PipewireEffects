use std::rc::Rc;
use std::time::Duration;

use pwe_core::i18n::t;

use super::Ui;

impl Ui {
    pub(crate) fn auto_save(self: &Rc<Self>) {
        let (enabled, interval) = {
            let state = self.state.borrow();
            (
                state.settings.auto_save,
                u64::from(state.settings.auto_save_interval.max(1)),
            )
        };
        if !enabled {
            return;
        }
        let modified = {
            let state = self.state.borrow();
            state.is_modified(self.active())
        };
        if !modified {
            self.auto_save_clock.set(std::time::Instant::now());
            return;
        }
        if self.auto_save_clock.get().elapsed() < Duration::from_secs(interval) {
            return;
        }
        self.auto_save_clock.set(std::time::Instant::now());
        let saved = self.state.borrow_mut().auto_save_active();
        if saved {
            self.toast(t("Preset auto-saved"));
        }
    }
}
