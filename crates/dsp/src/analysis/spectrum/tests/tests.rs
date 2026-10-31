use super::*;

const RATE: f32 = 48_000.0;

struct Generator {
    rate: f32,
    frame: usize,
    partials: Vec<f32>,
}

impl Generator {
    fn new(rate: f32, partials: &[f32]) -> Self {
        Self {
            rate,
            frame: 0,
            partials: partials.to_vec(),
        }
    }

    fn block(&mut self, frames: usize, amplitude: f32) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for index in 0..frames {
            let time = (self.frame + index) as f32 / self.rate;
            let mut value = 0.0;
            for partial in &self.partials {
                value += (std::f32::consts::TAU * partial * time).sin();
            }
            let value = amplitude * value;
            buffer.channel_mut(0)[index] = value;
            buffer.channel_mut(1)[index] = value;
        }
        self.frame += frames;
        buffer
    }
}

fn analyse(
    analyzer: &mut SpectrumAnalyzer,
    generator: &mut Generator,
    amplitude: f32,
) -> [f32; BANDS] {
    let mut last = None;
    for _ in 0..12 {
        if let Some(values) = analyzer.push(&generator.block(1024, amplitude)) {
            last = Some(values);
        }
    }
    last.expect("a frame should be produced")
}

fn loudest(values: &[f32; BANDS]) -> (usize, f32) {
    values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map_or((0, FLOOR_DB), |(index, value)| (index, *value))
}

#[test]
fn a_tone_lands_in_its_band_at_its_level() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[1_000.0]);
    let values = analyse(&mut analyzer, &mut generator, 0.5);
    let (index, peak) = loudest(&values);
    assert!((band_centre(index) - 1_000.0).abs() < 250.0, "{index}");
    assert!((peak + 6.02).abs() < 0.6, "{peak}");
}

#[test]
fn a_quiet_tone_at_several_frequencies_reads_its_own_level() {
    let mut errors = Vec::new();
    for frequency in [100.0f32, 440.0, 1_000.0, 5_000.0, 12_000.0] {
        let mut analyzer = SpectrumAnalyzer::new();
        analyzer.prepare(48_000);
        let mut generator = Generator::new(RATE, &[frequency]);
        let values = analyse(&mut analyzer, &mut generator, 0.0272);
        let (_, peak) = loudest(&values);
        errors.push((frequency, peak + 31.31));
        assert!(
            (peak + 31.31).abs() < 1.5,
            "{frequency} Hz at 0.0272 read {peak}"
        );
    }
    assert_eq!(errors.len(), 5);
}

#[test]
fn a_full_scale_tone_reads_near_zero() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[1_000.0]);
    let values = analyse(&mut analyzer, &mut generator, 1.0);
    let (_, peak) = loudest(&values);
    assert!(peak.abs() < 0.6, "{peak}");
}

#[test]
fn a_band_sums_every_tone_it_contains() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[1_000.0, 1_100.0]);
    let values = analyse(&mut analyzer, &mut generator, 0.5);
    let (index, peak) = loudest(&values);
    assert!((band_centre(index) - 1_000.0).abs() < 250.0, "{index}");
    assert!((peak + 3.01).abs() < 0.8, "{peak}");
}

#[test]
fn a_tone_below_the_resolution_is_still_reported_at_its_level() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[30.0]);
    let values = analyse(&mut analyzer, &mut generator, 1.0);
    let (index, peak) = loudest(&values);
    assert!(band_centre(index) < 60.0, "{index}");
    assert!(peak > -3.0 && peak < 1.0, "{peak}");
}

#[test]
fn silence_stays_at_the_floor() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut quiet = AudioBuffer::new(2, 1024);
    quiet.configure(2, 1024).unwrap();
    let mut last = None;
    for _ in 0..12 {
        if let Some(values) = analyzer.push(&quiet) {
            last = Some(values);
        }
    }
    let values = last.expect("a frame should be produced");
    assert!(values.iter().all(|value| *value <= FLOOR_DB));
}

#[test]
fn a_short_history_reports_nothing() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[1_000.0]);
    assert!(analyzer.push(&generator.block(128, 0.5)).is_none());
}

#[test]
fn a_rate_without_room_for_the_top_bands_leaves_them_at_the_floor() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(8_000);
    let mut generator = Generator::new(8_000.0, &[1_000.0]);
    let values = analyse(&mut analyzer, &mut generator, 0.5);
    assert!(values[BANDS - 1] <= FLOOR_DB);
    let (index, peak) = loudest(&values);
    assert!((band_centre(index) - 1_000.0).abs() < 300.0, "{index}");
    assert!((peak + 6.02).abs() < 0.6, "{peak}");
}

#[test]
fn the_analysis_resolves_almost_every_band() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut resolved = 0;
    for (index, (low, high)) in analyzer.bands.iter().enumerate() {
        if high - low >= analyzer.resolution_bandwidth {
            resolved += 1;
        } else {
            assert!(index < BANDS / 4, "band {index} is below the resolution");
        }
    }
    assert!(resolved > BANDS * 3 / 4, "{resolved} of {BANDS}");
}

#[test]
fn the_band_centres_rise_and_span_the_audio_range() {
    assert!(band_centre(0) < 40.0);
    assert!(band_centre(BANDS - 1) > 15_000.0);
    for index in 1..BANDS {
        assert!(band_centre(index) > band_centre(index - 1));
    }
}

#[test]
fn resetting_clears_the_history() {
    let mut analyzer = SpectrumAnalyzer::new();
    analyzer.prepare(48_000);
    let mut generator = Generator::new(RATE, &[1_000.0]);
    analyse(&mut analyzer, &mut generator, 0.5);
    analyzer.reset();
    assert!(analyzer.push(&generator.block(1024, 0.5)).is_none());
}
