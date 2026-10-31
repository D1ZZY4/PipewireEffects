#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseMode {
    Independent,
    Laminar,
}

impl PhaseMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Laminar,
            _ => Self::Independent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientMode {
    Smooth,
    Mixed,
    Crisp,
}

impl TransientMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Mixed,
            2 => Self::Crisp,
            _ => Self::Smooth,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientDetector {
    Compound,
    Percussive,
    Soft,
}

impl TransientDetector {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Percussive,
            2 => Self::Soft,
            _ => Self::Compound,
        }
    }

    pub fn sensitivity(self) -> f32 {
        match self {
            Self::Percussive => 1.3,
            Self::Compound => 1.8,
            Self::Soft => 2.6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VocoderOptions {
    pub phase: PhaseMode,
    pub transients: TransientMode,
    pub detector: TransientDetector,
}

impl Default for VocoderOptions {
    fn default() -> Self {
        Self {
            phase: PhaseMode::Independent,
            transients: TransientMode::Smooth,
            detector: TransientDetector::Compound,
        }
    }
}
