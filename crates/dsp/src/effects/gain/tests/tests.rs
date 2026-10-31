use super::*;

fn buffer() -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, 4);
    buffer.configure(2, 2).unwrap();
    buffer.channel_mut(0).copy_from_slice(&[0.5, -0.5]);
    buffer.channel_mut(1).copy_from_slice(&[0.5, -0.5]);
    buffer
}

#[test]
fn minus_six_db_halves_the_amplitude() {
    let params = ParamSet::new(specs());
    params.set_f32("gain", -6.0206).unwrap();
    let mut effect = Gain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buf = buffer();
    effect.process(&mut buf);
    assert!(
        (buf.channel(0)[0] - 0.25).abs() < 1.0e-3,
        "{}",
        buf.channel(0)[0]
    );
}

#[test]
fn the_offloadable_gain_matches_the_applied_gain() {
    let params = ParamSet::new(specs());
    params.set_f32("gain", -6.0).unwrap();
    let mut offload = Gain::new(params.clone());
    let mut local = Gain::new(params);
    offload.prepare(AudioFormat::new(48_000, 2));
    local.prepare(AudioFormat::new(48_000, 2));
    let gain = offload.elementwise_gain().expect("the gain is elementwise");
    let mut buf = buffer();
    local.process(&mut buf);
    assert!((buf.channel(0)[0] - 0.5 * gain).abs() < 1.0e-6);
}

#[test]
fn mute_silences_the_output() {
    let params = ParamSet::new(specs());
    params.set_bool("mute", true).unwrap();
    let mut effect = Gain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buf = buffer();
    effect.process(&mut buf);
    assert_eq!(buf.channel(0)[0], 0.0);
}
