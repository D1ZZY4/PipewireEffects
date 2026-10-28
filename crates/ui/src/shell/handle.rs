use std::rc::Rc;

use crate::action::UiAction;
use crate::shell::State;

impl State {
    pub fn handle(self: &Rc<Self>, action: UiAction) {
        match action {
            UiAction::Select(index) => {
                if self.selected.get() != index {
                    self.selected.set(index);
                    self.refresh_content();
                }
            }
            UiAction::SetEnabled(index, enabled) => {
                self.report(self.runtime.controller().set_enabled(index, enabled));
                self.refresh_chain();
            }
            UiAction::MoveUp(index) if index > 0 => {
                self.report(self.runtime.controller().move_entry(index, index - 1));
                self.selected.set(index - 1);
                self.restructure();
            }
            UiAction::MoveDown(index) => {
                let last = self.runtime.controller().len().saturating_sub(1);
                if index < last {
                    self.report(self.runtime.controller().move_entry(index, index + 1));
                    self.selected.set(index + 1);
                    self.restructure();
                }
            }
            UiAction::Remove(index) => {
                self.report(self.runtime.controller().remove(index));
                let remaining = self.runtime.controller().len();
                self.selected
                    .set(self.selected.get().min(remaining.saturating_sub(1)));
                self.restructure();
            }
            UiAction::Add(kind) => match self.runtime.controller().add(&kind) {
                Ok(index) => {
                    self.selected.set(index);
                    self.set_active_preset(None);
                    self.restructure();
                }
                Err(error) => self.toast(&error.to_string()),
            },
            UiAction::SetBypassed(bypassed) => {
                self.report(self.runtime.controller().set_bypassed(bypassed));
                self.sync_bypass();
            }
            UiAction::Clear => {
                self.report(self.runtime.controller().clear());
                self.selected.set(0);
                self.set_active_preset(None);
                self.restructure();
            }
            UiAction::ParametersChanged => self.restructure(),
            UiAction::LoadPreset(name) => self.load_preset(&name),
            UiAction::SavePreset => self.prompt_preset_name(),
            UiAction::DeletePreset(name) => self.delete_preset(&name),
            UiAction::SelectDevice(name) => {
                let target = if name.is_empty() { None } else { Some(name) };
                if let Err(error) = self
                    .runtime
                    .restart_with(|config| config.target_sink = target)
                {
                    self.toast(&format!("Could not switch device: {error}"));
                }
                self.refresh_status();
            }
            UiAction::RefreshDevices => {
                self.refresh_devices();
                self.toast("Device list refreshed");
            }
            UiAction::RestartEngine => {
                if let Err(error) = self.runtime.start_engine() {
                    self.toast(&format!("Could not restart the engine: {error}"));
                }
                self.refresh_status();
            }
            _ => {}
        }
    }

    fn restructure(self: &Rc<Self>) {
        self.refresh_chain();
        self.refresh_content();
        self.refresh_presets();
        self.sync_bypass();
    }

    fn load_preset(self: &Rc<Self>, name: &str) {
        let preset = match self.runtime.presets().load(name) {
            Ok(preset) => preset,
            Err(error) => {
                self.toast(&format!("Could not load `{name}`: {error}"));
                return;
            }
        };

        match self.runtime.controller().apply_recipe(&preset.to_recipe()) {
            Ok(report) if report.is_complete() => {}
            Ok(report) => self.toast(&format!(
                "Loaded `{}` with {} skipped and {} unknown setting(s)",
                preset.name,
                report.skipped.len(),
                report.unknown_params.len()
            )),
            Err(error) => {
                self.toast(&format!("Could not apply `{name}`: {error}"));
                return;
            }
        }

        self.selected.set(0);
        self.set_active_preset(Some(preset.name.clone()));
        self.restructure();
    }

    pub(crate) fn delete_preset(self: &Rc<Self>, name: &str) {
        match self.runtime.presets().delete(name) {
            Ok(()) => {
                self.toast(&format!("Deleted `{name}`"));
                if self.active_preset.borrow().as_deref() == Some(name) {
                    self.set_active_preset(None);
                }
                self.refresh_presets();
            }
            Err(error) => self.toast(&format!("Could not delete `{name}`: {error}")),
        }
    }
}
