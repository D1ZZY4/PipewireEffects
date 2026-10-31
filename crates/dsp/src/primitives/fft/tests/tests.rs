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
