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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output: PipelineSettings,
    pub input: PipelineSettings,
    pub preset: String,
    pub process_input: bool,
    pub autostart: bool,
    pub developer_mode: bool,
    pub audio_backend: String,
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
            audio_backend: "both".to_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_document_yields_the_documented_defaults() {
        let settings = Settings::default();
        assert!(settings.output.follows_the_system_default());
        assert!(settings.preset().is_none());
        assert!(!settings.process_input);
        assert!(!settings.autostart);
        assert!(!settings.developer_mode);
    }

    #[test]
    fn partial_documents_keep_the_missing_fields() {
        let settings: Settings =
            serde_json::from_str("{\"output\":{\"device\":\"usb\"}}").expect("should parse");
        assert_eq!(settings.output.device, "usb");
        assert_eq!(settings.input.device, AUTO);
        assert!(!settings.autostart);
    }

    #[test]
    fn the_processing_backend_round_trips_through_the_document() {
        let settings: Settings =
            serde_json::from_str("{\"audio_backend\":\"both\"}").expect("should parse");
        assert_eq!(settings.audio_backend, "both");
        let text = serde_json::to_string(&settings).expect("should write");
        let reloaded: Settings = serde_json::from_str(&text).expect("should parse again");
        assert_eq!(reloaded.audio_backend, "both");
    }

    #[test]
    fn the_kind_selects_the_pipeline() {
        let mut settings = Settings::default();
        settings.pipeline_mut(PipelineKind::Input).bypassed = true;
        assert!(settings.pipeline(PipelineKind::Input).bypassed);
        assert!(!settings.pipeline(PipelineKind::Output).bypassed);
    }
}
