use crate::effect::Effect;

pub enum ChainCommand {
    Add(Box<dyn Effect>),
    Remove(usize),
    Move { from: usize, to: usize },
    SetEnabled { index: usize, enabled: bool },
    SetBypass(bool),
}

impl std::fmt::Debug for ChainCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add(effect) => f.debug_tuple("Add").field(&effect.id()).finish(),
            Self::Remove(index) => f.debug_tuple("Remove").field(index).finish(),
            Self::Move { from, to } => f
                .debug_struct("Move")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::SetEnabled { index, enabled } => f
                .debug_struct("SetEnabled")
                .field("index", index)
                .field("enabled", enabled)
                .finish(),
            Self::SetBypass(state) => f.debug_tuple("SetBypass").field(state).finish(),
        }
    }
}
