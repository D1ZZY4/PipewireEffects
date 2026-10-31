use pwe_core::i18n::mark;

pub const RESPONSES: &[&str] = &[
    mark("Small room"),
    mark("Bright hall"),
    mark("Dark hall"),
    mark("Plate"),
    mark("Vocal booth"),
];

struct Shape {
    duration_ms: f32,
    decay_ms: f32,
    tone: f32,
    reflections: [(f32, f32); 3],
}

const SHAPES: [Shape; 5] = [
    Shape {
        duration_ms: 180.0,
        decay_ms: 45.0,
        tone: 0.35,
        reflections: [(3.0, 0.35), (7.0, -0.22), (11.0, 0.16)],
    },
    Shape {
        duration_ms: 420.0,
        decay_ms: 150.0,
        tone: 0.65,
        reflections: [(12.0, 0.28), (19.0, -0.18), (27.0, 0.12)],
    },
    Shape {
        duration_ms: 600.0,
        decay_ms: 230.0,
        tone: 0.18,
        reflections: [(15.0, 0.30), (23.0, -0.20), (31.0, 0.14)],
    },
    Shape {
        duration_ms: 300.0,
        decay_ms: 110.0,
        tone: 0.80,
        reflections: [(2.0, 0.20), (5.0, -0.14), (9.0, 0.10)],
    },
    Shape {
        duration_ms: 90.0,
        decay_ms: 26.0,
        tone: 0.45,
        reflections: [(1.5, 0.24), (4.0, -0.15), (6.0, 0.10)],
    },
];

const MAX_SAMPLES: usize = 16_384;

pub fn response(choice: usize, rate: u32, channel: usize) -> Vec<f32> {
    let shape = &SHAPES[choice.min(SHAPES.len() - 1)];
    let rate = rate.max(8_000);
    let requested = (rate as f32 * shape.duration_ms / 1000.0) as usize;
    let count = requested.clamp(64, MAX_SAMPLES.min(rate as usize / 2));
    let spread = if channel == 0 { 1.0 } else { 1.08 };
    let mut state = 0x9E37_79B9u32 ^ ((choice as u32) << 9) ^ (channel as u32 * 0x85EB_CA6B);
    state |= 1;
    let mut low = 0.0f32;
    let mut impulse = Vec::with_capacity(count);
    for index in 0..count {
        let seconds = index as f32 / rate as f32;
        let envelope = (-seconds * 1000.0 / shape.decay_ms).exp();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let noise = state as f32 / u32::MAX as f32 * 2.0 - 1.0;
        low += shape.tone.clamp(0.01, 0.99) * (noise - low);
        let mut value = low * envelope;
        for (milliseconds, gain) in shape.reflections {
            let target = ((milliseconds * spread) * rate as f32 / 1000.0) as usize;
            if index == target {
                value += gain * envelope;
            }
        }
        impulse.push(value);
    }
    scale_to_peak(&mut impulse, 0.5);
    impulse
}

pub fn energy(impulse: &[f32]) -> f32 {
    impulse.iter().map(|value| value * value).sum()
}

pub fn scale_to_energy(impulse: &mut [f32]) {
    let total = energy(impulse);
    if total > 0.0 {
        let scale = 1.0 / total.sqrt();
        for value in impulse {
            *value *= scale;
        }
    }
}

fn scale_to_peak(impulse: &mut [f32], peak: f32) {
    let maximum = impulse
        .iter()
        .fold(0.0f32, |current, value| current.max(value.abs()));
    if maximum > 0.0 {
        let scale = peak / maximum;
        for value in impulse {
            *value *= scale;
        }
    }
}

pub fn normalised(choice: usize, rate: u32, channel: usize, auto_gain: bool) -> Vec<f32> {
    let mut impulse = response(choice, rate, channel);
    if auto_gain {
        scale_to_energy(&mut impulse);
    }
    impulse
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
