use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::{Detector, blend, filtered_measure, soft_knee_expansion_db};
use crate::effects::sidechain::{MODES as SIDECHAIN_MODES, SidechainFilter};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const DETECTORS: [&str; 2] = [mark("Peak"), mark("RMS")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-60.0, 0.0, -12.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::choice("detector", "Detector", &DETECTORS, 0).group("Detector"),
        ParamSpec::float("ratio", "Ratio", ParamUnit::Ratio)
            .range(1.0, 100.0, 4.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("knee", "Knee", ParamUnit::Decibel)
            .range(0.0, 24.0, 6.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, 2000.0, 20.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(0.1, 5000.0, 100.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("makeup", "Makeup", ParamUnit::Decibel)
            .range(-60.0, 60.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::choice("hpf_mode", "High-pass", &SIDECHAIN_MODES, 0).group("Sidechain"),
        ParamSpec::float("hpf_frequency", "High-pass frequency", ParamUnit::Hertz)
            .range(10.0, 20_000.0, 10.0)
            .logarithmic()
            .group("Sidechain")
            .build(),
        ParamSpec::choice("lpf_mode", "Low-pass", &SIDECHAIN_MODES, 0).group("Sidechain"),
        ParamSpec::float("lpf_frequency", "Low-pass frequency", ParamUnit::Hertz)
            .range(10.0, 20_000.0, 20_000.0)
            .logarithmic()
            .group("Sidechain")
            .build(),
    ]
}

pub struct Expander {
    threshold: Knob,
    detector: Knob,
    ratio: Knob,
    knee: Knob,
    attack: Knob,
    release: Knob,
    makeup: Knob,
    mix: Knob,
    high_mode: Knob,
    high_frequency: Knob,
    low_mode: Knob,
    low_frequency: Knob,
    sidechain: SidechainFilter,
    state: Detector,
    rate: u32,
}

impl Expander {
    pub fn new(params: ParamSet) -> Self {
        Self {
            threshold: Knob::new(&params, "threshold", -12.0),
            detector: Knob::new(&params, "detector", 0.0),
            ratio: Knob::new(&params, "ratio", 4.0),
            knee: Knob::new(&params, "knee", 6.0),
            attack: Knob::new(&params, "attack", 20.0),
            release: Knob::new(&params, "release", 100.0),
            makeup: Knob::new(&params, "makeup", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            high_mode: Knob::new(&params, "hpf_mode", 0.0),
            high_frequency: Knob::new(&params, "hpf_frequency", 10.0),
            low_mode: Knob::new(&params, "lpf_mode", 0.0),
            low_frequency: Knob::new(&params, "lpf_frequency", 20_000.0),
            sidechain: SidechainFilter::new(48_000),
            state: Detector::default(),
            rate: 48_000,
        }
    }
}

impl Effect for Expander {
    fn id(&self) -> &'static str {
        "expander"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.sidechain.prepare(self.rate);
        self.state.reset();
    }

    fn reset(&mut self) {
        self.sidechain.reset();
        self.state.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.state
            .set_ballistics(self.attack.value(), self.release.value(), self.rate);
        let threshold = self.threshold.value();
        let ratio = self.ratio.value();
        let knee = self.knee.value();
        let makeup = math::db_to_gain(self.makeup.value());
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let use_rms = self.detector.index() == 1;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        self.sidechain.update(
            self.high_mode.index(),
            self.high_frequency.value(),
            self.low_mode.index(),
            self.low_frequency.value(),
        );
        for frame in 0..frames {
            let (peak, square) = filtered_measure(&mut self.sidechain, buffer, channels, frame);
            let level = self.state.track_measure(peak, square, use_rms);
            let expansion = soft_knee_expansion_db(math::gain_to_db(level), threshold, knee, ratio);
            let gain = math::db_to_gain(expansion) * makeup;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                let dry = plan[frame];
                plan[frame] = math::sanitize(blend(dry, dry * gain, mix));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 400.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    fn settled(settings: &[(&str, f32)], amplitude: f32) -> f32 {
        let params = ParamSet::new(specs());
        params.set_f32("knee", 0.0).unwrap();
        params.set_f32("attack", 0.5).unwrap();
        params.set_f32("release", 0.5).unwrap();
        for (key, value) in settings {
            params.set_f32(key, *value).unwrap();
        }
        let mut effect = Expander::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(amplitude, 1024);
        for _ in 0..256 {
            buffer = tone(amplitude, 1024);
            effect.process(&mut buffer);
        }
        buffer.peak()
    }

    #[test]
    fn signal_above_the_threshold_is_untouched() {
        let peak = settled(&[], 0.5);
        assert!((peak - 0.5).abs() < 0.01, "{peak}");
    }

    #[test]
    fn signal_below_the_threshold_is_expanded_downwards() {
        let peak = settled(&[], 0.01);
        assert!(peak < 1.0e-5, "the quiet signal was not expanded: {peak}");
    }

    #[test]
    fn a_higher_ratio_expands_harder() {
        let gentle = settled(&[("ratio", 2.0)], 0.01);
        let aggressive = settled(&[("ratio", 20.0)], 0.01);
        assert!(aggressive < gentle, "{aggressive} !< {gentle}");
    }

    #[test]
    fn makeup_is_applied_to_the_expanded_signal() {
        let plain = settled(&[("threshold", -12.0), ("ratio", 2.0)], 0.05);
        let lifted = settled(
            &[("threshold", -12.0), ("ratio", 2.0), ("makeup", 12.0)],
            0.05,
        );
        assert!(
            (math::gain_to_db(lifted) - math::gain_to_db(plain) - 12.0).abs() < 0.5,
            "{} {}",
            math::gain_to_db(plain),
            math::gain_to_db(lifted)
        );
    }

    #[test]
    fn a_zero_mix_keeps_the_dry_signal() {
        let peak = settled(&[("mix", 0.0)], 0.01);
        assert!((peak - 0.01).abs() < 1.0e-4, "{peak}");
    }

    #[test]
    fn extreme_settings_stay_finite() {
        let peak = settled(
            &[("ratio", 100.0), ("threshold", -60.0), ("makeup", 60.0)],
            1.0,
        );
        assert!(peak.is_finite(), "{peak}");
        assert!(peak.abs() < 1.0e6, "{peak}");
    }
}
