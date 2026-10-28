use super::*;

#[test]
fn a_fresh_bank_is_idle_and_healthy() {
    let stats = StreamStats::new();
    let snapshot = stats.snapshot();
    assert!(!snapshot.is_running());
    assert!(snapshot.is_healthy());
    assert_eq!(snapshot.glitch_samples(), 0);
}

#[test]
fn counters_accumulate_independently() {
    let stats = StreamStats::new();
    stats.add_capture_frames(480);
    stats.add_capture_frames(480);
    stats.add_processed_frames(960);
    stats.add_underrun_samples(64);

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.capture_frames, 960);
    assert_eq!(snapshot.processed_frames, 960);
    assert_eq!(snapshot.underrun_samples, 64);
    assert!(snapshot.is_running());
    assert!(!snapshot.is_healthy());
    assert_eq!(snapshot.glitch_samples(), 64);
}

#[test]
fn trims_are_counted_as_events_and_samples() {
    let stats = StreamStats::new();
    stats.add_trim(1_024);
    stats.add_trim(512);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.trim_events, 2);
    assert_eq!(snapshot.trimmed_samples, 1_536);
    assert!(snapshot.is_healthy());
}

#[test]
fn latency_is_reported_in_milliseconds() {
    let stats = StreamStats::new();
    stats.set_latency_samples(1_920);
    let snapshot = stats.snapshot();
    assert!((snapshot.latency_ms(48_000, 2) - 20.0).abs() < 1e-9);
}

#[test]
fn latency_of_a_degenerate_format_is_zero_rather_than_infinite() {
    let snapshot = StatsSnapshot {
        latency_samples: 1_920,
        ..StatsSnapshot::default()
    };
    assert_eq!(snapshot.latency_ms(0, 0), 0.0);
}

#[test]
fn resetting_clears_problems_but_keeps_totals() {
    let stats = StreamStats::new();
    stats.add_capture_frames(480);
    stats.add_processed_frames(480);
    stats.add_overrun_samples(32);
    stats.reset_problems();

    let snapshot = stats.snapshot();
    assert!(snapshot.is_healthy());
    assert_eq!(snapshot.capture_frames, 480);
    assert_eq!(snapshot.overrun_samples, 0);
}
