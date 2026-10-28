use pwe_engine::Snapshot;

pub const ICON_BYPASS: &str = "media-playback-pause-symbolic";
pub const ICON_MENU: &str = "open-menu-symbolic";
pub const ICON_SIDEBAR: &str = "sidebar-show-symbolic";
pub const ICON_PAGE_EFFECTS: &str = "audio-x-generic-symbolic";
pub const ICON_PAGE_PRESETS: &str = "view-list-symbolic";
pub const ICON_PAGE_DEVICES: &str = "audio-speakers-symbolic";
pub const ICON_EMPTY: &str = "applications-multimedia-symbolic";
pub const ICON_COPY: &str = "edit-copy-symbolic";
pub const ICON_COPIED: &str = "object-select-symbolic";
pub const ICON_ROW_MENU: &str = "view-more-symbolic";
pub const ICON_MOVE_UP: &str = "go-up-symbolic";
pub const ICON_MOVE_DOWN: &str = "go-down-symbolic";
pub const ICON_REMOVE: &str = "user-trash-symbolic";
pub const ICON_OPEN: &str = "go-next-symbolic";
pub const ICON_RESET: &str = "edit-undo-symbolic";
pub const ICON_ADD: &str = "list-add-symbolic";

pub const CHROME_ICONS: &[&str] = &[
    ICON_BYPASS,
    ICON_MENU,
    ICON_SIDEBAR,
    ICON_PAGE_EFFECTS,
    ICON_PAGE_PRESETS,
    ICON_PAGE_DEVICES,
    ICON_EMPTY,
    ICON_COPY,
    ICON_COPIED,
    ICON_ROW_MENU,
    ICON_MOVE_UP,
    ICON_MOVE_DOWN,
    ICON_REMOVE,
    ICON_OPEN,
    ICON_RESET,
    ICON_ADD,
];

pub fn missing_icons(theme: &gtk::IconTheme) -> Vec<&'static str> {
    let mut missing: Vec<&'static str> = Vec::new();
    for descriptor in pwe_dsp::EFFECTS {
        let icon = effect_icon(descriptor.id);
        if !theme.has_icon(icon) && !missing.contains(&icon) {
            missing.push(icon);
        }
    }
    for icon in CHROME_ICONS {
        if !theme.has_icon(icon) && !missing.contains(icon) {
            missing.push(icon);
        }
    }
    missing
}

pub fn effect_label(kind: &str) -> &'static str {
    pwe_dsp::find(kind).map_or("Effect", |descriptor| descriptor.label)
}

pub fn effect_summary(kind: &str) -> &'static str {
    pwe_dsp::find(kind).map_or("", |descriptor| descriptor.summary)
}

pub fn effect_icon(kind: &str) -> &'static str {
    match kind {
        "equalizer" => "view-list-symbolic",
        "filter" => "view-sort-descending-symbolic",
        "compressor" => "audio-volume-medium-symbolic",
        "limiter" => "audio-volume-high-symbolic",
        "gate" => "microphone-sensitivity-low-symbolic",
        "deesser" => "audio-headphones-symbolic",
        "autogain" => "microphone-sensitivity-high-symbolic",
        "bass_enhancer" => "audio-speakers-symbolic",
        "crystalizer" => "starred-symbolic",
        "stereo_tools" => "object-flip-horizontal-symbolic",
        "gain" => "multimedia-volume-control-symbolic",
        "delay" => "alarm-symbolic",
        "reverb" => "document-open-recent-symbolic",
        _ => category_icon(kind),
    }
}

pub fn category_icon(kind: &str) -> &'static str {
    match pwe_dsp::find(kind).map(|descriptor| descriptor.category) {
        Some(pwe_dsp::EffectCategory::Dynamics) => "audio-volume-high-symbolic",
        Some(pwe_dsp::EffectCategory::Equalisation) => "view-list-symbolic",
        Some(pwe_dsp::EffectCategory::Tone) => "audio-speakers-symbolic",
        Some(pwe_dsp::EffectCategory::Time) => "document-open-recent-symbolic",
        Some(pwe_dsp::EffectCategory::Analysis) => "preferences-system-symbolic",
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

    #[test]
    fn every_effect_has_its_own_icon() {
        let mut seen: Vec<&str> = Vec::new();
        for descriptor in pwe_dsp::EFFECTS {
            let icon = effect_icon(descriptor.id);
            assert!(
                !seen.contains(&icon),
                "{} and another effect share the icon {icon}",
                descriptor.id
            );
            seen.push(icon);
        }
    }

    #[test]
    fn the_status_text_follows_the_engine_state() {
        use pwe_engine::EngineState;
        let mut snapshot = Snapshot::default();
        snapshot.sample_rate = 48_000;
        snapshot.channels = 2;
        assert_eq!(
            status_text(&snapshot, EngineState::Running),
            "48000 Hz · 2 channels"
        );
        snapshot.bypassed = true;
        assert_eq!(status_text(&snapshot, EngineState::Running), "Bypassed");
        assert_eq!(
            status_text(&snapshot, EngineState::Starting),
            "Connecting to PipeWire"
        );
        assert_eq!(
            status_text(&snapshot, EngineState::Failed),
            "PipeWire connection failed"
        );
        assert_eq!(status_text(&snapshot, EngineState::Stopped), "Stopped");
    }
}
