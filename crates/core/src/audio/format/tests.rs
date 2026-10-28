use super::*;

#[test]
fn layout_is_derived_from_channel_count() {
    assert_eq!(ChannelLayout::from_channels(1), ChannelLayout::Mono);
    assert_eq!(ChannelLayout::from_channels(2), ChannelLayout::Stereo);
    assert_eq!(
        AudioFormat::new(48_000, 6).layout,
        ChannelLayout::Surround51
    );
    assert_eq!(AudioFormat::new(48_000, 4).layout, ChannelLayout::Discrete);
}

#[test]
fn implausible_formats_are_rejected() {
    assert!(!AudioFormat::new(0, 2).is_plausible());
    assert!(!AudioFormat::new(48_000, 0).is_plausible());
    assert!(AudioFormat::default_stereo().is_plausible());
}

#[test]
fn a_format_with_no_channels_has_no_frames() {
    assert_eq!(AudioFormat::new(48_000, 0).frames_of(1_024), 0);
    assert_eq!(AudioFormat::new(48_000, 2).frames_of(1_024), 512);
}

#[test]
fn duration_follows_the_sample_rate() {
    let format = AudioFormat::new(48_000, 2);
    assert!((format.duration_seconds(48_000) - 1.0).abs() < 1e-9);
    assert_eq!(AudioFormat::new(0, 2).duration_seconds(10), 0.0);
}
