use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    Select(usize),
    SetEnabled(usize, bool),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    Add(String),
    SetBypassed(bool),
    Clear,
    ParametersChanged,
    LoadPreset(String),
    SavePreset,
    DeletePreset(String),
    SelectDevice(String),
    RefreshDevices,
    RestartEngine,
}

impl UiAction {
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

pub type Dispatch = Rc<dyn Fn(UiAction)>;

#[cfg(test)]
mod tests;
