use pwe_engine::Snapshot;

pub fn effect_label(kind: &str) -> &'static str {
    pwe_dsp::find(kind).map_or("Effect", |descriptor| descriptor.label)
}

pub fn effect_summary(kind: &str) -> &'static str {
    pwe_dsp::find(kind).map_or("", |descriptor| descriptor.summary)
}

pub fn category_icon(kind: &str) -> &'static str {
    match pwe_dsp::find(kind).map(|descriptor| descriptor.category) {
        Some(pwe_dsp::EffectCategory::Dynamics) => "audio-volume-high-symbolic",
        Some(pwe_dsp::EffectCategory::Equalisation) => "view-list-symbolic",
        Some(pwe_dsp::EffectCategory::Tone) => "emblem-music-symbolic",
        Some(pwe_dsp::EffectCategory::Time) => "document-open-recent-symbolic",
        Some(pwe_dsp::EffectCategory::Analysis) => "utilities-system-monitor-symbolic",
        None => "application-x-addon-symbolic",
    }
}

pub fn db_text(db: f32) -> String {
    if db <= -120.0 {
        "-inf dB".to_owned()
    } else {
        format!("{db:.1} dB")
    }
}

pub fn normalized_db(db: f32) -> f64 {
    ((db.clamp(-60.0, 0.0) + 60.0) / 60.0) as f64
}

pub fn status_text(snapshot: &Snapshot, state: pwe_engine::EngineState) -> String {
    match state {
        pwe_engine::EngineState::Running => {
            if snapshot.bypassed {
                "Bypassed".to_owned()
            } else if snapshot.sample_rate == 0 {
                "Waiting for the graph".to_owned()
            } else {
                format!(
                    "{} Hz · {} channels",
                    snapshot.sample_rate, snapshot.channels
                )
            }
        }
        pwe_engine::EngineState::Starting => "Connecting to PipeWire".to_owned(),
        pwe_engine::EngineState::Failed => "PipeWire connection failed".to_owned(),
        pwe_engine::EngineState::Stopped => "Stopped".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_text_handles_the_floor() {
        assert_eq!(db_text(-144.0), "-inf dB");
        assert_eq!(db_text(-6.0), "-6.0 dB");
    }

    #[test]
    fn meter_normalisation_maps_the_range() {
        assert!((normalized_db(0.0) - 1.0).abs() < 1.0e-6);
        assert!((normalized_db(-60.0)).abs() < 1.0e-6);
        assert!((normalized_db(-120.0)).abs() < 1.0e-6);
    }

    #[test]
    fn known_effects_have_labels() {
        assert_eq!(effect_label("equalizer"), "Equalizer");
        assert_eq!(effect_label("unknown"), "Effect");
    }
}
