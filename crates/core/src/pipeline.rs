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
mod tests {
    use super::*;

    #[test]
    fn the_index_round_trips() {
        for kind in ALL {
            assert_eq!(PipelineKind::from_index(kind.index()), kind);
        }
    }

    #[test]
    fn an_unknown_index_is_the_output_pipeline() {
        assert_eq!(PipelineKind::from_index(7), PipelineKind::Output);
    }
}
