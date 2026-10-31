use super::*;

#[test]
fn the_four_band_defaults_keep_the_historical_splits() {
    assert_eq!(&DEFAULT_SPLITS[..3], &[120.0, 800.0, 5_000.0]);
}

#[test]
fn the_defaults_rise_with_the_split_index() {
    for pair in DEFAULT_SPLITS.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
}

#[test]
fn every_default_lies_inside_its_own_range() {
    for (index, (min, max)) in SPLIT_RANGES.iter().enumerate() {
        let value = DEFAULT_SPLITS[index];
        assert!((*min..=*max).contains(&value), "split{} {value}", index + 1);
    }
}

#[test]
fn the_split_count_matches_the_largest_band_count() {
    assert_eq!(DEFAULT_SPLITS.len(), MAX_BANDS - 1);
    const { assert!(DEFAULT_BANDS < MAX_BANDS) };
}
