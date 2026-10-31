#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineKind {
    Output,
    Input,
}

pub const ALL: [PipelineKind; 2] = [PipelineKind::Output, PipelineKind::Input];

impl PipelineKind {
    pub const fn index(self) -> usize {
        match self {
            Self::Output => 0,
            Self::Input => 1,
        }
    }

    pub fn label(self) -> &'static str {
        crate::i18n::t(match self {
            Self::Output => crate::i18n::mark("Output"),
            Self::Input => crate::i18n::mark("Input"),
        })
    }

    pub fn noun(self) -> &'static str {
        crate::i18n::t(match self {
            Self::Output => crate::i18n::mark("output"),
            Self::Input => crate::i18n::mark("input"),
        })
    }

    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Input,
            _ => Self::Output,
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
