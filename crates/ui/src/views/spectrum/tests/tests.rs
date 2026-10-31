use super::*;

#[test]
fn the_grid_positions_are_inside_the_plot_and_ordered() {
    let top = top_frequency(0);
    let positions: Vec<f64> = GRID_HZ.iter().map(|hz| position_in(*hz, top)).collect();
    assert!(positions[0] < positions[1] && positions[1] < positions[2]);
    assert!(positions.iter().all(|value| (0.0..=1.0).contains(value)));
}

#[test]
fn a_narrower_range_removes_grid_lines_above_it() {
    let top = top_frequency(2);
    assert!(GRID_HZ.iter().filter(|hz| **hz <= top).count() < GRID_HZ.len());
}

#[test]
fn the_floor_maps_to_the_bottom_and_the_ceiling_to_the_top() {
    let (floor, ceiling) = RANGES[0];
    let span = ceiling - floor;
    let bottom = f64::from(((floor - floor) / span).clamp(0.0, 1.0));
    let top = f64::from(((ceiling - floor) / span).clamp(0.0, 1.0));
    assert!(bottom.abs() < 1.0e-6);
    assert!((top - 1.0).abs() < 1.0e-6);
}

#[test]
fn every_range_has_a_positive_span() {
    for (floor, ceiling) in RANGES {
        assert!(ceiling > floor, "{floor} {ceiling}");
    }
}

#[test]
fn every_resolution_and_range_produces_the_requested_points() {
    for points in 0..3u8 {
        for frequency in 0..3u8 {
            let levels =
                sampled_levels(&[0.0; BANDS], point_count(points), top_frequency(frequency));
            assert_eq!(levels.len(), point_count(points));
        }
    }
}

#[test]
fn a_flat_spectrum_stays_flat_at_every_resolution() {
    let bands = [-24.0; BANDS];
    for points in 0..3u8 {
        let levels = sampled_levels(&bands, point_count(points), top_frequency(0));
        assert!(levels.iter().all(|value| (*value + 24.0).abs() < 1.0e-5));
    }
}

#[test]
fn the_automatic_range_keeps_a_sixty_decibel_span() {
    for peak in [-120.0f32, -60.0, -24.0, -6.0, 0.0] {
        let (floor, ceiling) = auto_bounds(peak);
        assert!(
            (ceiling - floor - 60.0).abs() < 1.0e-3,
            "{peak}: {floor} {ceiling}"
        );
        assert!(floor >= -120.0 && floor + 60.0 > floor);
    }
}

#[test]
fn the_highest_point_samples_the_tone_at_the_top_of_the_range() {
    let mut bands = [-120.0; BANDS];
    let last = BANDS - 1;
    bands[last] = 0.0;
    let levels = sampled_levels(&bands, 48, top_frequency(0));
    assert!(levels[47] > -1.0);
    assert!(levels[0] < -100.0);
}
