use pwe_core::Effect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectSlot(pub u64);

pub enum EngineCommand {
    SetTargetDevice(String),
    SetBypass(bool),
    AddEffect {
        slot: EffectSlot,
        effect: Box<dyn Effect>,
    },
    RemoveEffect {
        slot: EffectSlot,
    },
    MoveEffect {
        slot: EffectSlot,
        position: usize,
    },
    SetEffectEnabled {
        slot: EffectSlot,
        enabled: bool,
    },
    ResetChain,
    RefreshDevices,
    Stop,
}

impl std::fmt::Debug for EngineCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}

impl EngineCommand {
    pub fn describe(&self) -> String {
        match self {
            Self::SetTargetDevice(target) => format!("target device '{target}'"),
            Self::SetBypass(state) => format!("bypass {state}"),
            Self::AddEffect { slot, effect } => format!("add '{}' at slot {}", effect.id(), slot.0),
            Self::RemoveEffect { slot } => format!("remove slot {}", slot.0),
            Self::MoveEffect { slot, position } => {
                format!("move slot {} to position {position}", slot.0)
            }
            Self::SetEffectEnabled { slot, enabled } => {
                format!("slot {} enabled {enabled}", slot.0)
            }
            Self::ResetChain => "reset the chain".to_owned(),
            Self::RefreshDevices => "refresh the device list".to_owned(),
            Self::Stop => "stop the engine".to_owned(),
        }
    }
}
