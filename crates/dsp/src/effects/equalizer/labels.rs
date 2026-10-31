use pwe_core::i18n::mark;

use crate::biquad::FilterKind;

pub const BAND_COUNT: usize = 30;

pub const BAND_FREQUENCIES: [f32; BAND_COUNT] = [
    20.0, 25.0, 31.25, 40.0, 50.0, 62.5, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_500.0, 16_000.0,
];

pub const BAND_IDS: [&str; BAND_COUNT] = [
    "band11_gain",
    "band12_gain",
    "band1_gain",
    "band13_gain",
    "band14_gain",
    "band2_gain",
    "band15_gain",
    "band16_gain",
    "band3_gain",
    "band17_gain",
    "band18_gain",
    "band4_gain",
    "band19_gain",
    "band20_gain",
    "band5_gain",
    "band21_gain",
    "band22_gain",
    "band6_gain",
    "band23_gain",
    "band24_gain",
    "band7_gain",
    "band25_gain",
    "band26_gain",
    "band8_gain",
    "band27_gain",
    "band28_gain",
    "band9_gain",
    "band29_gain",
    "band30_gain",
    "band10_gain",
];

pub const BAND_LABELS: [&str; BAND_COUNT] = [
    mark("20 Hz"),
    mark("25 Hz"),
    mark("31 Hz"),
    mark("40 Hz"),
    mark("50 Hz"),
    mark("62 Hz"),
    mark("80 Hz"),
    mark("100 Hz"),
    mark("125 Hz"),
    mark("160 Hz"),
    mark("200 Hz"),
    mark("250 Hz"),
    mark("315 Hz"),
    mark("400 Hz"),
    mark("500 Hz"),
    mark("630 Hz"),
    mark("800 Hz"),
    mark("1 kHz"),
    mark("1.25 kHz"),
    mark("1.6 kHz"),
    mark("2 kHz"),
    mark("2.5 kHz"),
    mark("3.15 kHz"),
    mark("4 kHz"),
    mark("5 kHz"),
    mark("6.3 kHz"),
    mark("8 kHz"),
    mark("10 kHz"),
    mark("12.5 kHz"),
    mark("16 kHz"),
];

macro_rules! band_ids {
    ($prefix:literal, $suffix:literal; $($number:literal),* $(,)?) => {
        [$(concat!($prefix, $number, $suffix)),*]
    };
}

pub const BAND_TYPE_IDS: [&str; BAND_COUNT] = band_ids!("band", "_type"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const BAND_FREQUENCY_IDS: [&str; BAND_COUNT] = band_ids!("band", "_frequency"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const BAND_Q_IDS: [&str; BAND_COUNT] = band_ids!("band", "_q"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const RIGHT_BAND_IDS: [&str; BAND_COUNT] = band_ids!("right_band", "_gain"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const RIGHT_BAND_TYPE_IDS: [&str; BAND_COUNT] = band_ids!("right_band", "_type"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const RIGHT_BAND_FREQUENCY_IDS: [&str; BAND_COUNT] = band_ids!("right_band", "_frequency"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const RIGHT_BAND_Q_IDS: [&str; BAND_COUNT] = band_ids!("right_band", "_q"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);

macro_rules! label_list {
    ($($label:expr),* $(,)?) => {
        [$($label),*]
    };
}

pub const BAND_TYPE_LABELS: [&str; BAND_COUNT] = label_list!(
    mark("20 Hz type"),
    mark("25 Hz type"),
    mark("31 Hz type"),
    mark("40 Hz type"),
    mark("50 Hz type"),
    mark("62 Hz type"),
    mark("80 Hz type"),
    mark("100 Hz type"),
    mark("125 Hz type"),
    mark("160 Hz type"),
    mark("200 Hz type"),
    mark("250 Hz type"),
    mark("315 Hz type"),
    mark("400 Hz type"),
    mark("500 Hz type"),
    mark("630 Hz type"),
    mark("800 Hz type"),
    mark("1 kHz type"),
    mark("1.25 kHz type"),
    mark("1.6 kHz type"),
    mark("2 kHz type"),
    mark("2.5 kHz type"),
    mark("3.15 kHz type"),
    mark("4 kHz type"),
    mark("5 kHz type"),
    mark("6.3 kHz type"),
    mark("8 kHz type"),
    mark("10 kHz type"),
    mark("12.5 kHz type"),
    mark("16 kHz type")
);
pub const BAND_FREQUENCY_LABELS: [&str; BAND_COUNT] = label_list!(
    mark("20 Hz frequency"),
    mark("25 Hz frequency"),
    mark("31 Hz frequency"),
    mark("40 Hz frequency"),
    mark("50 Hz frequency"),
    mark("62 Hz frequency"),
    mark("80 Hz frequency"),
    mark("100 Hz frequency"),
    mark("125 Hz frequency"),
    mark("160 Hz frequency"),
    mark("200 Hz frequency"),
    mark("250 Hz frequency"),
    mark("315 Hz frequency"),
    mark("400 Hz frequency"),
    mark("500 Hz frequency"),
    mark("630 Hz frequency"),
    mark("800 Hz frequency"),
    mark("1 kHz frequency"),
    mark("1.25 kHz frequency"),
    mark("1.6 kHz frequency"),
    mark("2 kHz frequency"),
    mark("2.5 kHz frequency"),
    mark("3.15 kHz frequency"),
    mark("4 kHz frequency"),
    mark("5 kHz frequency"),
    mark("6.3 kHz frequency"),
    mark("8 kHz frequency"),
    mark("10 kHz frequency"),
    mark("12.5 kHz frequency"),
    mark("16 kHz frequency")
);
pub const BAND_Q_LABELS: [&str; BAND_COUNT] = label_list!(
    mark("20 Hz Q"),
    mark("25 Hz Q"),
    mark("31 Hz Q"),
    mark("40 Hz Q"),
    mark("50 Hz Q"),
    mark("62 Hz Q"),
    mark("80 Hz Q"),
    mark("100 Hz Q"),
    mark("125 Hz Q"),
    mark("160 Hz Q"),
    mark("200 Hz Q"),
    mark("250 Hz Q"),
    mark("315 Hz Q"),
    mark("400 Hz Q"),
    mark("500 Hz Q"),
    mark("630 Hz Q"),
    mark("800 Hz Q"),
    mark("1 kHz Q"),
    mark("1.25 kHz Q"),
    mark("1.6 kHz Q"),
    mark("2 kHz Q"),
    mark("2.5 kHz Q"),
    mark("3.15 kHz Q"),
    mark("4 kHz Q"),
    mark("5 kHz Q"),
    mark("6.3 kHz Q"),
    mark("8 kHz Q"),
    mark("10 kHz Q"),
    mark("12.5 kHz Q"),
    mark("16 kHz Q")
);

pub const DEFAULT_Q: f32 = 1.414;
pub const CHANNEL_MODE_ID: &str = "channel_mode";
pub const CHANNEL_LABELS: [&str; 2] = [mark("Stereo"), mark("Left and right")];
pub const PEAK_INDEX: usize = FilterKind::Peaking as usize;

pub fn band_group(frequency: f32) -> &'static str {
    if frequency < 250.0 {
        mark("Lows")
    } else if frequency < 2_000.0 {
        mark("Mids")
    } else {
        mark("Highs")
    }
}

pub fn right_group(frequency: f32) -> &'static str {
    if frequency < 250.0 {
        mark("Right lows")
    } else if frequency < 2_000.0 {
        mark("Right mids")
    } else {
        mark("Right highs")
    }
}
