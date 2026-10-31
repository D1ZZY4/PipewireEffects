use super::*;

#[test]
fn the_grid_positions_are_inside_the_plot_and_ordered() {
    let positions: Vec<f64> = GRID_HZ.iter().map(|hz| position_of(*hz)).collect();
    assert!(positions[0] < positions[1] && positions[1] < positions[2]);
    assert!(positions.iter().all(|value| (0.0..=1.0).contains(value)));
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
