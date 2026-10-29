use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("cutoff", "Cutoff", ParamUnit::Hertz)
            .range(300.0, 2_000.0, 700.0)
            .logarithmic()
            .group("Crossfeed")
            .build(),
        ParamSpec::float("feed", "Feed", ParamUnit::Decibel)
            .range(1.0, 15.0, 4.5)
            .step(0.1)
            .group("Crossfeed")
            .build(),
    ]
}

pub struct Crossfeed {
    cutoff: Knob,
    feed: Knob,
    rate: u32,
    state: [f32; MAX_CHANNELS],
}

impl Crossfeed {
    pub fn new(params: ParamSet) -> Self {
        Self {
            cutoff: Knob::new(&params, "cutoff", 700.0),
            feed: Knob::new(&params, "feed", 4.5),
            rate: 48_000,
            state: [0.0; MAX_CHANNELS],
        }
    }
}

fn coefficient(cutoff: f32, rate: u32) -> f32 {
    let rate = rate.max(1) as f32;
    let cutoff = cutoff.clamp(1.0, rate * 0.45);
    (1.0 - (-std::f32::consts::TAU * cutoff / rate).exp()).clamp(0.0, 1.0)
}

impl Effect for Crossfeed {
    fn id(&self) -> &'static str {
        "crossfeed"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        self.state = [0.0; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        if channels < 2 {
            return;
        }
        let alpha = coefficient(self.cutoff.value(), self.rate);
        let gain = 0.5 * math::db_to_gain(self.feed.value());
        let frames = buffer.frames();

        for frame in 0..frames {
            let left = buffer.channel_mut(0)[frame];
            let right = buffer.channel_mut(1)[frame];
            let state = &mut self.state;
            state[0] = math::sanitize(state[0] + (left - state[0]) * alpha);
            state[1] = math::sanitize(state[1] + (right - state[1]) * alpha);
            let difference = state[1] - state[0];
            buffer.channel_mut(0)[frame] = math::sanitize(left + gain * difference);
            buffer.channel_mut(1)[frame] = math::sanitize(right - gain * difference);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, frames: usize, left: f32, right: f32) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for index in 0..frames {
            let value = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
            buffer.channel_mut(0)[index] = value * left;
            buffer.channel_mut(1)[index] = value * right;
        }
        buffer
    }

    fn effect(cutoff: f32, feed: f32) -> Crossfeed {
        let params = ParamSet::new(specs());
        params.set_f32("cutoff", cutoff).unwrap();
        params.set_f32("feed", feed).unwrap();
        let mut effect = Crossfeed::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    }

    fn leak(cutoff: f32, feed: f32, frequency: f32) -> f32 {
        let mut effect = effect(cutoff, feed);
        let mut buffer = tone(frequency, 9_600, 0.0, 0.5);
        for _ in 0..4 {
            buffer = tone(frequency, 9_600, 0.0, 0.5);
            effect.process(&mut buffer);
        }
        buffer
            .channel(0)
            .iter()
            .fold(0.0f32, |peak, value| peak.max(value.abs()))
    }

    #[test]
    fn a_mono_signal_passes_through_unchanged() {
        let mut effect = effect(700.0, 4.5);
        let mut buffer = tone(200.0, 4096, 0.5, 0.5);
        let source: Vec<f32> = buffer.channel(0).to_vec();
        effect.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(source) {
            assert!(
                (processed - original).abs() < 1.0e-6,
                "{processed} {original}"
            );
        }
    }

    #[test]
    fn low_frequencies_cross_to_the_other_channel() {
        let measured = leak(700.0, 9.5, 100.0);
        assert!(
            math::gain_to_db(measured) > -6.0,
            "{} dB",
            math::gain_to_db(measured)
        );
    }

    #[test]
    fn high_frequencies_stay_where_they_are() {
        let low = leak(700.0, 9.5, 100.0);
        let high = leak(700.0, 9.5, 10_000.0);
        assert!(high < low * 0.5, "{high} against {low}");
    }

    #[test]
    fn a_lower_cutoff_crosses_less() {
        let wide = leak(2_000.0, 9.5, 1_000.0);
        let narrow = leak(300.0, 9.5, 1_000.0);
        assert!(wide > narrow, "{wide} !> {narrow}");
    }

    #[test]
    fn a_single_channel_leaves_the_processed_side_finite() {
        let mut effect = effect(700.0, 15.0);
        let mut buffer = tone(80.0, 4096, 0.9, -0.9);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite(), "{sample}");
                assert!(sample.abs() < 10.0, "{sample}");
            }
        }
    }
}
