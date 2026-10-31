#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use super::State;

impl State {
    pub fn remember_autoload(&mut self, device: &str, preset: &str) {
        if device.is_empty() || preset.is_empty() {
            return;
        }
        if self
            .settings
            .autoload
            .get(device)
            .is_some_and(|value| value == preset)
        {
            return;
        }
        self.settings
            .autoload
            .insert(device.to_owned(), preset.to_owned());
        self.persist();
    }

    pub fn forget_autoload(&mut self, device: &str) {
        if self.settings.autoload.remove(device).is_some() {
            self.persist();
        }
    }

    pub fn autoload_for(&self, device: &str) -> Option<&str> {
        self.settings.autoload.get(device).map(String::as_str)
    }
}
