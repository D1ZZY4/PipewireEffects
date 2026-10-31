use super::*;

#[test]
fn db_text_handles_the_floor() {
    assert_eq!(db_text(-144.0), "-inf dB");
    assert_eq!(db_text(-6.0), "-6.0 dB");
}

#[test]
fn the_loudness_caption_names_the_measurement_and_hides_a_floor() {
    assert_eq!(loudness_text(f32::NEG_INFINITY), None);
    assert_eq!(loudness_text(f32::NAN), None);
    assert_eq!(loudness_text(-80.0), None);
    assert_eq!(loudness_text(-23.0).as_deref(), Some("Loudness -23.0 LUFS"));
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
        assert_symbolic_classes(&file, &text);
    }
}

fn assert_symbolic_classes(file: &str, text: &str) {
    let drawing = ["<path", "<rect", "<circle"];
    let mut elements = 0;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !drawing.iter().any(|tag| trimmed.starts_with(tag)) {
            continue;
        }
        elements += 1;
        let classes = trimmed
            .split_once("class=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(classes, _)| classes)
            .unwrap_or_else(|| {
                panic!(
                    "{file}:{} draws without a symbolic class, so GTK paints it solid",
                    index + 1
                )
            });
        for class in classes.split_whitespace() {
            assert!(
                matches!(
                    class,
                    "foreground-fill" | "foreground-stroke" | "transparent-fill"
                ),
                "{file}:{} uses the unsupported symbolic class {class}",
                index + 1
            );
        }
        if trimmed.contains("fill=\"none\"") && trimmed.contains("stroke=") {
            assert!(
                classes.contains("foreground-stroke"),
                "{file}:{} strokes without the foreground-stroke class",
                index + 1
            );
        }
    }
    assert!(elements > 0, "{file} has no drawing element");
}
