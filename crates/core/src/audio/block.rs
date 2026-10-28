use super::format::{AudioFormat, Sample};

#[derive(Debug)]
pub struct AudioBlock<'a> {
    samples: &'a mut [Sample],
    frames: usize,
    format: AudioFormat,
}

impl<'a> AudioBlock<'a> {
    #[must_use]
    pub fn new(samples: &'a mut [Sample], format: AudioFormat) -> Self {
        let frames = format.frames_of(samples.len());
        Self {
            samples,
            frames,
            format,
        }
    }

    #[must_use]
    pub fn with_frames(samples: &'a mut [Sample], format: AudioFormat, frames: usize) -> Self {
        let channels = format.channels_usize().max(1);
        let max_frames = samples.len() / channels;
        Self {
            samples,
            frames: frames.min(max_frames),
            format,
        }
    }

    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.format.sample_rate
    }

    #[must_use]
    pub const fn channels(&self) -> usize {
        self.format.channels as usize
    }

    #[must_use]
    pub const fn frames(&self) -> usize {
        self.frames
    }

    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        self.samples
    }

    pub fn samples_mut(&mut self) -> &mut [Sample] {
        self.samples
    }

    #[must_use]
    pub fn active_samples(&self) -> &[Sample] {
        let len = self.format.samples_of(self.frames).min(self.samples.len());
        &self.samples[..len]
    }

    pub fn active_samples_mut(&mut self) -> &mut [Sample] {
        let len = self.format.samples_of(self.frames).min(self.samples.len());
        &mut self.samples[..len]
    }

    pub fn frames_iter(&self) -> std::slice::ChunksExact<'_, Sample> {
        let channels = self.channels().max(1);
        self.active_samples().chunks_exact(channels)
    }

    pub fn frames_iter_mut(&mut self) -> std::slice::ChunksExactMut<'_, Sample> {
        let channels = self.channels().max(1);
        self.active_samples_mut().chunks_exact_mut(channels)
    }

    pub fn fill_silence(&mut self) {
        self.active_samples_mut().fill(0.0);
    }

    pub fn apply_gain(&mut self, factor: f32) {
        for sample in self.active_samples_mut() {
            *sample *= factor;
        }
    }

    #[must_use]
    pub fn peak(&self) -> f32 {
        self.active_samples()
            .iter()
            .fold(0.0_f32, |acc, s| acc.max(s.abs()))
    }

    #[must_use]
    pub fn rms(&self) -> f32 {
        let samples = self.active_samples();
        if samples.is_empty() {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let count = samples.len() as f32;
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / count).sqrt()
    }

    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.active_samples().iter().all(|s| *s == 0.0)
    }

    pub fn sanitise(&mut self) {
        for sample in self.active_samples_mut() {
            if !sample.is_finite() {
                *sample = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests;
