use pwe_core::{AudioBuffer, math};

use crate::effects::util::MAX_CHANNELS;

use super::{DITHER_LSB, Limiter};

impl Limiter {
    pub(super) fn process_oversampled(&mut self, buffer: &mut AudioBuffer) {
        let input_gain = self.input_smoothed.advance() * self.boost_makeup();
        let ceiling = math::db_to_gain(self.limit.value()).max(1.0e-5);
        let factor = self.factor;
        let lookahead = self.frames * factor;
        let ramp = self.ramp;
        let release = self.release_coefficient;
        let forget = self.forget;
        let link = (self.link.value() / 100.0).clamp(0.0, 1.0);
        let alr = self.release_mode.index() == 1;
        let dither_mode = self.dithering.index();
        let channels = buffer.channels().min(self.channels.len());
        let frames = buffer.frames();
        let mut interpolated = [0.0f32; MAX_CHANNELS];
        let mut delayed = [0.0f32; MAX_CHANNELS];
        let mut need = [1.0f32; MAX_CHANNELS];
        let mut gains = [1.0f32; MAX_CHANNELS];
        let mut limited = [0.0f32; MAX_CHANNELS];

        for frame in 0..frames {
            let mut frame_required = 1.0f32;
            for channel in 0..channels {
                self.resampler.push(channel, buffer.channel(channel)[frame]);
            }
            for phase in 0..factor {
                for (channel, slot) in interpolated.iter_mut().enumerate().take(channels) {
                    *slot = self.resampler.interpolate(channel, phase) * input_gain;
                }
                for channel in 0..channels {
                    let state = &mut self.channels[channel];
                    let length = state.delay.len();
                    let read = (state.head + length - lookahead) % length;
                    delayed[channel] = state.delay[read];
                    state.delay[state.head] = interpolated[channel];
                    state.head = (state.head + 1) % length;
                    need[channel] = Self::required(interpolated[channel], ceiling);
                }
                let shared = need
                    .iter()
                    .take(channels)
                    .fold(1.0f32, |acc, value| acc.min(*value));
                frame_required = frame_required.min(shared);
                for (value, state) in need.iter().zip(self.channels.iter_mut()).take(channels) {
                    state.hold = if *value < state.hold {
                        *value
                    } else {
                        state.hold + (1.0 - state.hold) * forget
                    };
                }
                for ((value, state), gain) in delayed
                    .iter()
                    .enumerate()
                    .zip(self.channels.iter_mut())
                    .zip(gains.iter_mut())
                    .take(channels)
                {
                    let channel = value.0;
                    let value = value.1;
                    let own = state.hold;
                    let target = (shared * link + own * (1.0 - link)).min(own);
                    let exact = Self::required(*value, ceiling);
                    state.advance(need[channel], target, ramp, release, exact, alr);
                    *gain = state.envelope;
                }
                for channel in 0..channels {
                    limited[channel] = delayed[channel] * gains[channel];
                }
                self.resampler.push_decim(channels, &limited);
                if phase + 1 == factor {
                    for channel in 0..channels {
                        let filtered = self.resampler.decimate(channel);
                        let noise = self.dither.sample(dither_mode, DITHER_LSB);
                        let sample = (filtered + noise).clamp(-ceiling, ceiling);
                        buffer.channel_mut(channel)[frame] = math::sanitize(sample);
                    }
                }
            }
            let coefficient = if frame_required < self.boost_reference {
                self.boost_attack
            } else {
                self.boost_release
            };
            self.boost_reference += (frame_required - self.boost_reference) * coefficient;
            if !self.boost_reference.is_finite() {
                self.boost_reference = 1.0;
            }
        }
    }
}
