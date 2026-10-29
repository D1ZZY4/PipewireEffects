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
mod tests {
    use super::*;

    #[test]
    fn a_bin_centred_tone_lands_in_its_bin() {
        let size = 64;
        let fft = Fft::new(size);
        let mut re = vec![0.0f32; size];
        let mut im = vec![0.0f32; size];
        for (index, value) in re.iter_mut().enumerate() {
            *value = (TAU * 8.0 * index as f32 / size as f32).cos();
        }
        fft.forward(&mut re, &mut im);
        let magnitudes: Vec<f32> = (0..size)
            .map(|bin| (re[bin] * re[bin] + im[bin] * im[bin]).sqrt())
            .collect();
        let peak = magnitudes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(bin, _)| bin)
            .unwrap();
        assert_eq!(peak, 8);
        assert!(magnitudes[8] > magnitudes[7] * 5.0);
    }

    #[test]
    fn forward_then_inverse_is_the_identity() {
        let size = 128;
        let fft = Fft::new(size);
        let original: Vec<f32> = (0..size)
            .map(|index| (index as f32 * 0.37).sin() - 0.4 * (index as f32 * 1.1).cos())
            .collect();
        let mut re = original.clone();
        let mut im = vec![0.0f32; size];
        fft.forward(&mut re, &mut im);
        fft.inverse(&mut re, &mut im);
        for (index, expected) in original.iter().enumerate() {
            assert!((re[index] - expected).abs() < 1.0e-3, "{index}");
            assert!(im[index].abs() < 1.0e-3, "{index}");
        }
    }

    #[test]
    fn parseval_energy_is_preserved() {
        let size = 64;
        let fft = Fft::new(size);
        let mut re: Vec<f32> = (0..size).map(|index| (index as f32 * 0.7).sin()).collect();
        let mut im = vec![0.0f32; size];
        let time: f32 = re.iter().map(|value| value * value).sum();
        fft.forward(&mut re, &mut im);
        let spectrum: f32 = (0..size)
            .map(|bin| (re[bin] * re[bin] + im[bin] * im[bin]) / size as f32)
            .sum();
        assert!((time - spectrum).abs() < 1.0e-2, "{time} {spectrum}");
    }

    #[test]
    fn the_hann_window_is_zero_at_the_edges() {
        let window = hann(16);
        assert!(window[0].abs() < 1.0e-6);
        assert!((window[8] - 1.0).abs() < 1.0e-6);
        assert_eq!(window.len(), 16);
    }
}
