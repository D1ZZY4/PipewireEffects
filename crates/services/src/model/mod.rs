#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::collections::BTreeMap;

use pwe_core::PipelineKind;
use serde::{Deserialize, Serialize};

pub const AUTO: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineSettings {
    pub device: String,
    pub bypassed: bool,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            device: AUTO.to_owned(),
            bypassed: false,
        }
    }
}

impl PipelineSettings {
    pub fn follows_the_system_default(&self) -> bool {
        self.device.is_empty() || self.device == AUTO
    }
}

pub const DEFAULT_AUTO_SAVE_INTERVAL: u32 = 10;
pub const DEFAULT_SPECTRUM_RANGE: u8 = 0;
pub const DEFAULT_SPECTRUM_SHAPE: u8 = 0;
pub const DEFAULT_SPECTRUM_POINTS: u8 = 1;
pub const DEFAULT_SPECTRUM_HEIGHT: u8 = 1;
pub const DEFAULT_SPECTRUM_FREQUENCY_RANGE: u8 = 0;
pub const DEFAULT_SPECTRUM_FFT: u8 = 2;
pub const DEFAULT_SPECTRUM_WINDOW: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output: PipelineSettings,
    pub input: PipelineSettings,
    pub preset: String,
    pub process_input: bool,
    pub autostart: bool,
    pub developer_mode: bool,
    pub auto_save: bool,
    pub auto_save_interval: u32,
    pub audio_backend: String,
    pub spectrum_range: u8,
    pub spectrum_shape: u8,
    pub spectrum_points: u8,
    pub spectrum_height: u8,
    pub spectrum_frequency_range: u8,
    pub spectrum_fft: u8,
    pub spectrum_window: u8,
    pub spectrum_auto_range: bool,
    pub autoload: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output: PipelineSettings::default(),
            input: PipelineSettings::default(),
            preset: String::new(),
            process_input: false,
            autostart: false,
            developer_mode: false,
            auto_save: false,
            auto_save_interval: DEFAULT_AUTO_SAVE_INTERVAL,
            audio_backend: "both".to_owned(),
            spectrum_range: DEFAULT_SPECTRUM_RANGE,
            spectrum_shape: DEFAULT_SPECTRUM_SHAPE,
            spectrum_points: DEFAULT_SPECTRUM_POINTS,
            spectrum_height: DEFAULT_SPECTRUM_HEIGHT,
            spectrum_frequency_range: DEFAULT_SPECTRUM_FREQUENCY_RANGE,
            spectrum_fft: DEFAULT_SPECTRUM_FFT,
            spectrum_window: DEFAULT_SPECTRUM_WINDOW,
            spectrum_auto_range: false,
            autoload: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn pipeline(&self, kind: PipelineKind) -> &PipelineSettings {
        match kind {
            PipelineKind::Output => &self.output,
            PipelineKind::Input => &self.input,
        }
    }

    pub fn pipeline_mut(&mut self, kind: PipelineKind) -> &mut PipelineSettings {
        match kind {
            PipelineKind::Output => &mut self.output,
            PipelineKind::Input => &mut self.input,
        }
    }

    pub fn preset(&self) -> Option<&str> {
        (!self.preset.is_empty()).then_some(self.preset.as_str())
    }
}
