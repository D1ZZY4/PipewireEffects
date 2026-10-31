use std::f32::consts::TAU;

pub struct Fft {
    size: usize,
    reversed: Vec<usize>,
    cos: Vec<f32>,
    sin: Vec<f32>,
}

impl Fft {
    pub fn new(size: usize) -> Self {
        let size = size.max(2).next_power_of_two();
        let bits = size.trailing_zeros();
        let mut reversed = vec![0usize; size];
        for (index, slot) in reversed.iter_mut().enumerate() {
            *slot = index.reverse_bits() >> (usize::BITS - bits);
        }
        let mut cos = vec![0.0f32; size / 2];
        let mut sin = vec![0.0f32; size / 2];
        for (index, (cosine, sine)) in cos.iter_mut().zip(sin.iter_mut()).enumerate() {
            let angle = TAU * index as f32 / size as f32;
            *cosine = angle.cos();
            *sine = angle.sin();
        }
        Self {
            size,
            reversed,
            cos,
            sin,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn forward(&self, re: &mut [f32], im: &mut [f32]) {
        self.transform(re, im, false);
    }

    pub fn inverse(&self, re: &mut [f32], im: &mut [f32]) {
        self.transform(re, im, true);
    }

    fn transform(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        let size = self.size;
        if re.len() < size || im.len() < size {
            return;
        }
        for index in 0..size {
            let target = self.reversed[index];
            if target > index {
                re.swap(index, target);
                im.swap(index, target);
            }
        }
        let mut span = 2;
        while span <= size {
            let half = span / 2;
            let step = size / span;
            for start in (0..size).step_by(span) {
                for offset in 0..half {
                    let twiddle = offset * step;
                    let (cosine, sine) = if inverse {
                        (self.cos[twiddle], self.sin[twiddle])
                    } else {
                        (self.cos[twiddle], -self.sin[twiddle])
                    };
                    let low = start + offset;
                    let high = low + half;
                    let tr = re[high] * cosine - im[high] * sine;
                    let ti = re[high] * sine + im[high] * cosine;
                    re[high] = re[low] - tr;
                    im[high] = im[low] - ti;
                    re[low] += tr;
                    im[low] += ti;
                }
            }
            span *= 2;
        }
        if inverse {
            let scale = 1.0 / size as f32;
            for index in 0..size {
                re[index] *= scale;
                im[index] *= scale;
            }
        }
    }
}

pub fn hann(size: usize) -> Vec<f32> {
    (0..size)
        .map(|index| {
            let angle = TAU * index as f32 / size.max(1) as f32;
            0.5 - 0.5 * angle.cos()
        })
        .collect()
}

pub fn magnitude_db(re: f32, im: f32) -> f32 {
    let power = re * re + im * im;
    if power <= 1.0e-24 {
        -144.0
    } else {
        10.0 * power.log10()
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
