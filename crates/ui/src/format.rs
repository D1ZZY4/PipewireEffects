pub const ICON_BYPASS: &str = "system-shutdown-symbolic";
pub const ICON_MENU: &str = "open-menu-symbolic";
pub const ICON_SIDEBAR: &str = "sidebar-show-symbolic";
pub const ICON_BACK: &str = "go-previous-symbolic";
pub const ICON_PAGE_OUTPUT: &str = "audio-speakers-symbolic";
pub const ICON_PAGE_INPUT: &str = "audio-input-microphone-symbolic";
pub const ICON_PAGE_DEVICES: &str = "audio-card-symbolic";
pub const ICON_PREFERENCES: &str = "preferences-system-symbolic";
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
pub const ICON_SAVE: &str = "document-save-symbolic";
pub const ICON_IMPORT: &str = "document-open-symbolic";
pub const ICON_EXPORT: &str = "document-send-symbolic";
pub const ICON_DIAGNOSTICS: &str = "dialog-information-symbolic";

pub const CHROME_ICONS: &[&str] = &[
    ICON_BYPASS,
    ICON_MENU,
    ICON_SIDEBAR,
    ICON_BACK,
    ICON_PAGE_OUTPUT,
    ICON_PAGE_INPUT,
    ICON_PAGE_DEVICES,
    ICON_PREFERENCES,
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
    ICON_SAVE,
    ICON_IMPORT,
    ICON_EXPORT,
    ICON_DIAGNOSTICS,
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
    pwe_dsp::find(kind).map_or_else(
        || pwe_core::i18n::t("Effect"),
        |descriptor| pwe_core::i18n::t(descriptor.label),
    )
}

pub fn effect_summary(kind: &str) -> &'static str {
    pwe_dsp::find(kind).map_or("", |descriptor| pwe_core::i18n::t(descriptor.summary))
}

pub fn effect_icon(kind: &str) -> &'static str {
    match kind {
        "equalizer" => "pipewire-effects-equalizer-symbolic",
        "midside_equalizer" => "pipewire-effects-midside_equalizer-symbolic",
        "filter" => "pipewire-effects-filter-symbolic",
        "compressor" => "pipewire-effects-compressor-symbolic",
        "limiter" => "pipewire-effects-limiter-symbolic",
        "maximizer" => "pipewire-effects-maximizer-symbolic",
        "gate" => "pipewire-effects-gate-symbolic",
        "multiband_compressor" => "pipewire-effects-multiband_compressor-symbolic",
        "multiband_gate" => "pipewire-effects-multiband_gate-symbolic",
        "expander" => "pipewire-effects-expander-symbolic",
        "deesser" => "pipewire-effects-deesser-symbolic",
        "autogain" => "pipewire-effects-autogain-symbolic",
        "bass_enhancer" => "pipewire-effects-bass_enhancer-symbolic",
        "crystalizer" => "pipewire-effects-crystalizer-symbolic",
        "crusher" => "pipewire-effects-crusher-symbolic",
        "exciter" => "pipewire-effects-exciter-symbolic",
        "stereo_tools" => "pipewire-effects-stereo_tools-symbolic",
        "crossfeed" => "pipewire-effects-crossfeed-symbolic",
        "crosstalk" => "pipewire-effects-crosstalk-symbolic",
        "voice_suppressor" => "pipewire-effects-voice_suppressor-symbolic",
        "noise_reduction" => "pipewire-effects-noise_reduction-symbolic",
        "pitch_shift" => "pipewire-effects-pitch_shift-symbolic",
        "gain" => "pipewire-effects-gain-symbolic",
        "delay" => "pipewire-effects-delay-symbolic",
        "reverb" => "pipewire-effects-reverb-symbolic",
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

pub fn rate_text(rate: u32) -> String {
    let kilohertz = f64::from(rate) / 1000.0;
    if (kilohertz.fract()).abs() < 1.0e-9 {
        format!("{kilohertz:.0} kHz")
    } else {
        format!("{kilohertz:.1} kHz")
    }
}

pub fn frequency_text(hz: f32) -> String {
    if hz >= 1_000.0 {
        let kilohertz = f64::from(hz) / 1000.0;
        if (kilohertz.fract()).abs() < 1.0e-9 {
            format!("{kilohertz:.0} kHz")
        } else {
            format!("{kilohertz:.1} kHz")
        }
    } else {
        format!("{hz:.0} Hz")
    }
}

pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let length = digits.len();
    let mut text = String::with_capacity(length + length / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (length - index).is_multiple_of(3) {
            text.push('\u{202f}');
        }
        text.push(character);
    }
    text
}

pub fn duration_text(frames: u64, rate: u32) -> String {
    if rate == 0 {
        return String::new();
    }
    let milliseconds = frames as f64 / f64::from(rate) * 1000.0;
    if milliseconds < 10.0 {
        format!("{milliseconds:.1} ms")
    } else {
        format!("{milliseconds:.0} ms")
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
    fn rate_text_drops_a_trailing_zero_fraction() {
        assert_eq!(rate_text(48_000), "48 kHz");
        assert_eq!(rate_text(44_100), "44.1 kHz");
        assert_eq!(rate_text(96_000), "96 kHz");
    }

    #[test]
    fn frequency_text_spans_hertz_and_kilohertz() {
        assert_eq!(frequency_text(100.0), "100 Hz");
        assert_eq!(frequency_text(1_000.0), "1 kHz");
        assert_eq!(frequency_text(1_046.5), "1.0 kHz");
        assert_eq!(frequency_text(19_000.0), "19 kHz");
    }

    #[test]
    fn grouped_inserts_a_separator_every_three_digits() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1\u{202f}000");
        assert_eq!(grouped(180_224), "180\u{202f}224");
        assert_eq!(grouped(1_234_567), "1\u{202f}234\u{202f}567");
    }

    #[test]
    fn duration_text_reports_milliseconds() {
        assert_eq!(duration_text(256, 48_000), "5.3 ms");
        assert_eq!(duration_text(4_800, 48_000), "100 ms");
        assert_eq!(duration_text(512, 0), "");
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
    fn every_effect_icon_file_exists_is_symbolic_and_keeps_each_tag_on_one_line() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/icons/hicolor/symbolic/apps");
        for descriptor in pwe_dsp::EFFECTS {
            let file = format!("{}.svg", effect_icon(descriptor.id));
            let path = directory.join(&file);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                text.contains("currentColor"),
                "{file} does not draw from the theme's foreground colour"
            );
            assert!(
                text.contains("16") && text.contains("viewBox"),
                "{file} is not a 16 px symbolic icon"
            );
            for (index, line) in text.lines().enumerate() {
                assert!(
                    !(line.contains('<') && !line.contains('>')),
                    "{file}:{} leaves a tag open across lines, which the palette does not do",
                    index + 1
                );
            }
        }
    }
}
