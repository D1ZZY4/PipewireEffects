use pwe_core::AudioBuffer;
use pwe_core::math;

use crate::effects::sidechain::SidechainFilter;

pub fn filtered_measure(
    filter: &mut SidechainFilter,
    buffer: &AudioBuffer,
    channels: usize,
    frame: usize,
) -> (f32, f32) {
    if filter.bypassed() {
        let mut peak = 0.0f32;
        let mut square = 0.0f32;
        for channel in 0..channels {
            let value = buffer.channel(channel)[frame];
            peak = peak.max(value.abs());
            square += value * value;
        }
        return (peak, square / channels.max(1) as f32);
    }
    let mut peak = 0.0f32;
    let mut square = 0.0f32;
    for channel in 0..channels {
        let value = filter.process(channel, buffer.channel(channel)[frame]);
        peak = peak.max(value.abs());
        square += value * value;
    }
    (peak, square / channels.max(1) as f32)
}

pub fn soft_knee_reduction_db(level_db: f32, threshold_db: f32, knee_db: f32, ratio: f32) -> f32 {
    if !level_db.is_finite() {
        return 0.0;
    }
    let over = level_db - threshold_db;
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if knee_db <= 0.0 {
        if over > 0.0 { -over * slope } else { 0.0 }
    } else if over <= -0.5 * knee_db {
        0.0
    } else if over >= 0.5 * knee_db {
        -over * slope
    } else {
        let shifted = over + 0.5 * knee_db;
        -(slope * shifted * shifted / (2.0 * knee_db))
    }
}

pub fn soft_knee_expansion_db(level_db: f32, threshold_db: f32, knee_db: f32, ratio: f32) -> f32 {
    if !level_db.is_finite() {
        return 0.0;
    }
    let under = level_db - threshold_db;
    let slope = ratio.max(1.0) - 1.0;
    if knee_db <= 0.0 {
        if under < 0.0 { under * slope } else { 0.0 }
    } else if under >= 0.5 * knee_db {
        0.0
    } else if under <= -0.5 * knee_db {
        under * slope
    } else {
        let shifted = under - 0.5 * knee_db;
        -slope * shifted * shifted / (2.0 * knee_db)
    }
}

pub fn ballistics(attack_ms: f32, release_ms: f32, rate: u32) -> (f32, f32) {
    let rate = rate.max(1) as f32;
    let attack = 1.0 - (-1.0 / ((attack_ms.max(0.01) / 1000.0) * rate)).exp();
    let release = 1.0 - (-1.0 / ((release_ms.max(0.01) / 1000.0) * rate)).exp();
    (attack.clamp(0.0, 1.0), release.clamp(0.0, 1.0))
}

pub fn blend(dry: f32, wet: f32, mix: f32) -> f32 {
    dry * (1.0 - mix) + wet * mix
}

#[derive(Default)]
pub struct Detector {
    level: f32,
    rms: f32,
    attack: f32,
    release: f32,
    rms_coefficient: f32,
}

impl Detector {
    pub fn set_ballistics(&mut self, attack_ms: f32, release_ms: f32, rate: u32) {
        let (attack, release) = ballistics(attack_ms, release_ms, rate);
        self.attack = attack;
        self.release = release;
        self.rms_coefficient = math::one_pole_alpha(10.0, rate);
    }

    pub fn reset(&mut self) {
        self.level = 0.0;
        self.rms = 0.0;
    }

    pub fn track_measure(&mut self, peak: f32, square: f32, use_rms: bool) -> f32 {
        let level = if use_rms {
            self.rms += (square - self.rms) * self.rms_coefficient;
            self.rms.max(0.0).sqrt()
        } else {
            peak
        };
        let coefficient = if level > self.level {
            self.attack
        } else {
            self.release
        };
        self.level += (level - self.level) * coefficient;
        if !self.level.is_finite() {
            self.level = 0.0;
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_the_knee_there_is_no_reduction() {
        assert_eq!(soft_knee_reduction_db(-40.0, -18.0, 6.0, 4.0), 0.0);
    }

    #[test]
    fn above_the_knee_matches_the_ratio() {
        let reduction = soft_knee_reduction_db(0.0, -18.0, 0.0, 4.0);
        let expected = -18.0 * 0.75;
        assert!((reduction - expected).abs() < 1.0e-4, "{reduction}");
    }

    #[test]
    fn expansion_leaves_the_loud_half_alone() {
        assert_eq!(soft_knee_expansion_db(-6.0, -18.0, 6.0, 4.0), 0.0);
        assert_eq!(soft_knee_expansion_db(-18.0, -18.0, 0.0, 4.0), 0.0);
    }

    #[test]
    fn expansion_matches_the_ratio_below_the_threshold() {
        let gain = soft_knee_expansion_db(-30.0, -18.0, 0.0, 4.0);
        assert!((gain + 36.0).abs() < 1.0e-4, "{gain}");
    }

    #[test]
    fn the_expansion_knee_is_continuous() {
        let threshold = -18.0;
        let knee = 6.0;
        let ratio = 4.0;
        let below = soft_knee_expansion_db(threshold - knee * 0.5 - 1.0e-4, threshold, knee, ratio);
        let above = soft_knee_expansion_db(threshold - knee * 0.5 + 1.0e-4, threshold, knee, ratio);
        assert!((below - above).abs() < 1.0e-3, "{below} {above}");
        assert!((soft_knee_expansion_db(threshold, threshold, knee, ratio) + 2.25).abs() < 1.0e-2);
    }

    #[test]
    fn the_knee_is_continuous() {
        let threshold = -18.0;
        let knee = 6.0;
        let ratio = 4.0;
        let below = soft_knee_reduction_db(threshold - knee * 0.5 - 1.0e-4, threshold, knee, ratio);
        let above = soft_knee_reduction_db(threshold + knee * 0.5 + 1.0e-4, threshold, knee, ratio);
        assert!(below.abs() < 1.0e-3);
        assert!((above - (-knee * 0.5 * 0.75)).abs() < 1.0e-2);
    }

    #[test]
    fn ballistics_reach_their_targets() {
        let (attack, release) = ballistics(10.0, 100.0, 48_000);
        assert!(attack > release);
        let mut envelope = 0.0f32;
        for _ in 0..48_000 {
            let target = -6.0;
            let coefficient = if target < envelope { attack } else { release };
            envelope += (target - envelope) * coefficient;
        }
        assert!((envelope + 6.0).abs() < 0.01);
    }

    #[test]
    fn the_detector_follows_a_steady_level() {
        let mut buffer = AudioBuffer::new(2, 64);
        buffer.configure(2, 64).unwrap();
        for channel in 0..2 {
            buffer.channel_mut(channel).fill(0.5);
        }
        let mut detector = Detector::default();
        detector.set_ballistics(1.0, 50.0, 48_000);
        let mut level = 0.0;
        for _ in 0..200 {
            for frame in 0..64 {
                let sample = buffer.channel(0)[frame];
                level = detector.track_measure(sample, sample * sample, false);
            }
        }
        assert!((level - 0.5).abs() < 1.0e-3, "{level}");
    }

    #[test]
    fn the_mix_blend_crosses_between_dry_and_wet() {
        assert!((blend(0.0, 1.0, 0.25) - 0.25).abs() < 1.0e-6);
        assert!((blend(0.4, 0.4, 1.0) - 0.4).abs() < 1.0e-6);
    }
}
