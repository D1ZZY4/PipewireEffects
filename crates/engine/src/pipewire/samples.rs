use std::mem::{align_of, size_of};

pub(super) fn is_directly_viewable(pointer: *const u8) -> bool {
    cfg!(target_endian = "little") && (pointer as usize).is_multiple_of(align_of::<f32>())
}

pub(super) fn read_samples<'a>(
    bytes: &'a [u8],
    scratch: &'a mut Vec<f32>,
    channels: usize,
) -> &'a [f32] {
    let channels = channels.max(1);
    let count = (bytes.len() / size_of::<f32>()) / channels * channels;

    if is_directly_viewable(bytes.as_ptr()) {
        return unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<f32>(), count) };
    }

    if scratch.len() < count {
        scratch.resize(count, 0.0);
    }
    for (slot, chunk) in scratch[..count].iter_mut().zip(bytes.chunks_exact(4)) {
        *slot = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    &scratch[..count]
}

pub(super) fn write_samples(samples: &[f32], bytes: &mut [u8]) {
    let count = samples.len().min(bytes.len() / size_of::<f32>());
    if count == 0 {
        return;
    }
    let samples = &samples[..count];

    if is_directly_viewable(bytes.as_ptr()) {
        let destination =
            unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<f32>(), count) };
        destination.copy_from_slice(samples);
        return;
    }

    for (chunk, sample) in bytes.chunks_exact_mut(4).zip(samples) {
        chunk.copy_from_slice(&sample.to_le_bytes());
    }
}
