#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::rc::Rc;

use gtk::glib;
use pwe_core::PipelineKind;
use pwe_core::i18n::tr;

use super::Ui;

impl Ui {
    pub(super) fn autoload(self: &Rc<Self>, snapshot: &pwe_engine::Snapshot, kind: PipelineKind) {
        let target = snapshot.target.clone();
        let slot = kind.index();
        let changed = {
            let mut last = self.last_targets.borrow_mut();
            if last[slot] == target {
                false
            } else {
                last[slot] = target.clone();
                true
            }
        };
        if !changed || target.is_empty() {
            return;
        }
        let preset = self.address_preset(&target);
        let Some(preset) = preset else {
            return;
        };
        if self.state.borrow().loaded_preset() == Some(preset.as_str()) {
            return;
        }
        let ui = Rc::clone(self);
        glib::idle_add_local_once(move || {
            let loaded = ui.state.borrow_mut().load_preset(&preset);
            match loaded {
                Ok(warnings) => {
                    if !warnings.is_empty() {
                        ui.show_banner(&tr(
                            "The autoloaded preset '{0}' loaded with warnings: {1}",
                            &[&preset, &warnings.join("; ")],
                        ));
                    }
                    ui.rebuild_chain();
                    ui.rebuild_editor();
                }
                Err(error) => ui.show_banner(&error.to_string()),
            }
        });
    }

    fn address_preset(&self, device: &str) -> Option<String> {
        self.state.borrow().autoload_for(device).map(str::to_owned)
    }
}
