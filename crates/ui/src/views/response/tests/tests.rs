use super::*;

#[test]
fn the_log_axis_is_monotonic() {
    let low = log_position(20.0);
    let mid = log_position(632.0);
    let high = log_position(20_000.0);
    assert!(low < mid && mid < high);
    assert!(low.abs() < 1.0e-6);
    assert!((high - 1.0).abs() < 1.0e-6);
}

#[test]
fn the_axis_round_trips() {
    for position in [0.0f32, 0.25, 0.5, 1.0] {
        assert!((log_position(hz_at(position)) - position).abs() < 1.0e-4);
    }
}
