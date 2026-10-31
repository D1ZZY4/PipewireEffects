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
#[path = "tests/tests.rs"]
mod tests;
