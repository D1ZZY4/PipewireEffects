//! The vocabulary the interface's widgets use to talk to the application.
//!
//! Widgets never touch the chain, the engine or the preset store directly.
//! They emit a [`UiAction`] through a [`Dispatch`], and the window decides what
//! that means. It is the same discipline the engine uses for audio: components
//! publish intent, and one place owns the consequences.

use std::rc::Rc;

/// Something the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    /// Show an effect's settings.
    Select(usize),
    /// Enable or disable one effect.
    SetEnabled(usize, bool),
    /// Move an effect one place earlier.
    MoveUp(usize),
    /// Move an effect one place later.
    MoveDown(usize),
    /// Remove an effect.
    Remove(usize),
    /// Append an effect of the given kind.
    Add(String),
    /// Bypass or re-engage the whole chain.
    SetBypassed(bool),
    /// Remove every effect.
    Clear,
    /// Parameter values changed through a control, so summaries need refreshing.
    ParametersChanged,
    /// Load a preset by name.
    LoadPreset(String),
    /// Ask for a name, then save the chain as a preset.
    SavePreset,
    /// Delete a user preset.
    DeletePreset(String),
    /// Play the processed audio on a different device.
    SelectDevice(String),
    /// Re-read the list of devices.
    RefreshDevices,
    /// Stop and restart the audio engine.
    RestartEngine,
}

impl UiAction {
    /// A short description, used in the status line.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Select(_) => "selected an effect",
            Self::SetEnabled(_, true) => "enabled an effect",
            Self::SetEnabled(_, false) => "disabled an effect",
            Self::MoveUp(_) | Self::MoveDown(_) => "reordered the chain",
            Self::Remove(_) => "removed an effect",
            Self::Add(_) => "added an effect",
            Self::SetBypassed(true) => "bypassed the chain",
            Self::SetBypassed(false) => "re-enabled the chain",
            Self::Clear => "cleared the chain",
            Self::ParametersChanged => "changed a setting",
            Self::LoadPreset(_) => "loaded a preset",
            Self::SavePreset => "saved a preset",
            Self::DeletePreset(_) => "deleted a preset",
            Self::SelectDevice(_) => "selected an output device",
            Self::RefreshDevices => "refreshed the device list",
            Self::RestartEngine => "restarted the engine",
        }
    }
}

/// The function widgets call to report a [`UiAction`].
pub type Dispatch = Rc<dyn Fn(UiAction)>;

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn every_action_has_a_description() {
        let actions = [
            UiAction::Select(0),
            UiAction::SetEnabled(0, true),
            UiAction::SetEnabled(0, false),
            UiAction::MoveUp(0),
            UiAction::MoveDown(0),
            UiAction::Remove(0),
            UiAction::Add(String::from("gain")),
            UiAction::SetBypassed(true),
            UiAction::SetBypassed(false),
            UiAction::Clear,
            UiAction::ParametersChanged,
            UiAction::LoadPreset(String::from("x")),
            UiAction::SavePreset,
            UiAction::DeletePreset(String::from("x")),
            UiAction::SelectDevice(String::from("x")),
            UiAction::RefreshDevices,
            UiAction::RestartEngine,
        ];
        for action in actions {
            assert!(
                !action.description().is_empty(),
                "{action:?} has no description"
            );
        }
    }

    #[test]
    fn dispatching_invokes_the_handler() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_handler = Rc::clone(&seen);
        let dispatch: Dispatch = Rc::new(move |action| {
            seen_for_handler.borrow_mut().push(action);
        });
        dispatch(UiAction::Clear);
        assert_eq!(seen.borrow().len(), 1);
    }
}
