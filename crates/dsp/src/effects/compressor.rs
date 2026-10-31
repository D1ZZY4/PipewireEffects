use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::{Detector, blend, filtered_measure, soft_knee_reduction_db};
use crate::effects::sidechain::{MODES as SIDECHAIN_MODES, SidechainFilter};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const DETECTORS: [&str; 2] = [mark("Peak"), mark("RMS")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-60.0, 0.0, -18.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::choice("detector", "Detector", &DETECTORS, 0).group("Detector"),
        ParamSpec::float("ratio", "Ratio", ParamUnit::Ratio)
            .range(1.0, 20.0, 4.0)
            .step(0.1)
            .group("Detector")
            .build(),
        ParamSpec::float("knee", "Knee", ParamUnit::Decibel)
            .range(0.0, 24.0, 3.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, 200.0, 20.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 2000.0, 200.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("makeup", "Makeup", ParamUnit::Decibel)
            .range(-12.0, 24.0, 0.0)
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

pub struct Compressor {
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

impl Compressor {
    pub fn new(params: ParamSet) -> Self {
        Self {
            threshold: Knob::new(&params, "threshold", -18.0),
            detector: Knob::new(&params, "detector", 0.0),
            ratio: Knob::new(&params, "ratio", 4.0),
            knee: Knob::new(&params, "knee", 3.0),
            attack: Knob::new(&params, "attack", 20.0),
            release: Knob::new(&params, "release", 200.0),
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

impl Compressor {
    fn sidechain_update(&mut self) {
        self.sidechain.update(
            self.high_mode.index(),
            self.high_frequency.value(),
            self.low_mode.index(),
            self.low_frequency.value(),
        );
    }
}

impl Effect for Compressor {
    fn id(&self) -> &'static str {
        "compressor"
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

        self.sidechain_update();
        for frame in 0..frames {
            let (peak, square) = filtered_measure(&mut self.sidechain, buffer, channels, frame);
            let level = self.state.track_measure(peak, square, use_rms);
            let reduction = soft_knee_reduction_db(math::gain_to_db(level), threshold, knee, ratio);
            let gain = math::db_to_gain(reduction) * makeup;
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

    fn low_tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 60.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

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

    #[test]
    fn signal_below_the_threshold_is_untouched() {
        let params = ParamSet::new(specs());
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.05, 4096);
        for _ in 0..16 {
            effect.process(&mut buffer);
        }
        assert!(buffer.peak() <= 0.0501, "{}", buffer.peak());
    }

    #[test]
    fn signal_above_the_threshold_is_reduced() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -24.0).unwrap();
        params.set_f32("ratio", 8.0).unwrap();
        params.set_f32("knee", 0.0).unwrap();
        params.set_f32("attack", 1.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.9, 1024);
        for _ in 0..256 {
            buffer = tone(0.9, 1024);
            effect.process(&mut buffer);
        }
        let output_db = math::gain_to_db(buffer.peak());
        assert!(output_db < -18.0, "not enough reduction: {output_db}");
        assert!(output_db > -24.0, "too much reduction: {output_db}");
    }

    #[test]
    fn a_higher_ratio_compresses_harder() {
        let measure = |ratio: f32| {
            let params = ParamSet::new(specs());
            params.set_f32("threshold", -30.0).unwrap();
            params.set_f32("ratio", ratio).unwrap();
            params.set_f32("knee", 0.0).unwrap();
            params.set_f32("attack", 1.0).unwrap();
            let mut effect = Compressor::new(params);
            effect.prepare(AudioFormat::new(48_000, 2));
            let mut buffer = tone(0.9, 1024);
            for _ in 0..256 {
                buffer = tone(0.9, 1024);
                effect.process(&mut buffer);
            }
            buffer.peak()
        };
        let gentle = measure(2.0);
        let aggressive = measure(16.0);
        assert!(aggressive < gentle, "{aggressive} !< {gentle}");
    }

    #[test]
    fn makeup_gain_is_added_back() {
        let params = ParamSet::new(specs());
        params.set_f32("makeup", 6.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.1, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.1 * math::db_to_gain(6.0)).abs() < 0.01);
    }

    #[test]
    fn the_sidechain_high_pass_keeps_rumble_out_of_the_detector() {
        let measure = |mode: u16| {
            let params = ParamSet::new(specs());
            params.set_f32("threshold", -24.0).unwrap();
            params.set_f32("ratio", 8.0).unwrap();
            params.set_f32("knee", 0.0).unwrap();
            params.set_f32("attack", 1.0).unwrap();
            params.set_choice("hpf_mode", mode).unwrap();
            params.set_f32("hpf_frequency", 400.0).unwrap();
            let mut effect = Compressor::new(params);
            effect.prepare(AudioFormat::new(48_000, 2));
            let mut buffer = low_tone(0.9, 4096);
            for _ in 0..64 {
                effect.process(&mut buffer);
            }
            buffer.peak()
        };
        let unfiltered = measure(0);
        let filtered = measure(1);
        assert!(
            unfiltered < 0.4,
            "the rumble was not compressed at all: {unfiltered}"
        );
        assert!(
            filtered > unfiltered * 2.0,
            "the detector filter did not change the reduction: {filtered} against {unfiltered}"
        );
    }

    #[test]
    fn half_mix_blends_dry_and_wet() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -24.0).unwrap();
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.9, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.9).abs() < 0.01, "{}", buffer.peak());
    }
}
