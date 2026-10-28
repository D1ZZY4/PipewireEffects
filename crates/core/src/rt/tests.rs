use super::*;
use std::sync::Arc;
use std::thread;

#[test]
fn capacity_is_rounded_up_to_a_power_of_two() {
    assert_eq!(AudioRing::with_capacity(1_000).capacity(), 1_024);
    assert_eq!(AudioRing::with_capacity(1).capacity(), 2);
}

#[test]
fn write_then_read_round_trips() {
    let ring = AudioRing::with_capacity(8);
    let report = ring.write(&[1.0, 2.0, 3.0]);
    assert_eq!(report.moved, 3);
    assert!(report.is_complete());
    assert_eq!(ring.len(), 3);

    let mut out = [0.0; 3];
    assert_eq!(ring.read(&mut out).moved, 3);
    assert_eq!(out, [1.0, 2.0, 3.0]);
    assert!(ring.is_empty());
}

#[test]
fn writes_beyond_capacity_are_truncated_not_wrapped_over() {
    let ring = AudioRing::with_capacity(4);
    let report = ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(report.moved, 4);
    assert_eq!(report.shortfall, 2);
    let mut out = [0.0; 4];
    ring.read(&mut out);
    assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn reads_past_the_end_leave_the_rest_untouched() {
    let ring = AudioRing::with_capacity(4);
    ring.write(&[7.0, 8.0]);
    let mut out = [9.0; 4];
    let report = ring.read(&mut out);
    assert_eq!(report.moved, 2);
    assert_eq!(report.shortfall, 2);
    assert_eq!(out, [7.0, 8.0, 9.0, 9.0]);
}

#[test]
fn indices_wrap_correctly_across_many_cycles() {
    let ring = AudioRing::with_capacity(4);
    for round in 0..100 {
        #[allow(clippy::cast_precision_loss)]
        let base = round as f32;
        ring.write(&[base, base + 1.0, base + 2.0, base + 3.0]);
        let mut out = [0.0; 4];
        assert_eq!(ring.read(&mut out).moved, 4);
        assert_eq!(out, [base, base + 1.0, base + 2.0, base + 3.0]);
    }
    assert!(ring.is_empty());
}

#[test]
fn discard_oldest_trims_from_the_reading_end() {
    let ring = AudioRing::with_capacity(8);
    ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(ring.discard_oldest(2), 2);
    assert_eq!(ring.len(), 3);
    let mut out = [0.0; 3];
    ring.read(&mut out);
    assert_eq!(out, [3.0, 4.0, 5.0]);
}

#[test]
fn clear_empties_the_buffer() {
    let ring = AudioRing::with_capacity(8);
    ring.write(&[1.0, 2.0, 3.0]);
    ring.clear();
    assert!(ring.is_empty());
    assert_eq!(ring.spare(), ring.capacity());
}

#[test]
fn concurrent_producer_and_consumer_transfer_everything_in_order() {
    const TOTAL_BLOCKS: usize = 20_000;
    const BLOCK: usize = 32;

    let ring = Arc::new(AudioRing::with_capacity(256));
    let producer = {
        let ring = Arc::clone(&ring);
        thread::spawn(move || {
            let mut produced = 0_usize;
            while produced < TOTAL_BLOCKS {
                let mut block = [0.0_f32; BLOCK];
                let base = produced * BLOCK;
                for (offset, slot) in block.iter_mut().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    {
                        *slot = (base + offset) as f32;
                    }
                }
                if ring.write(&block).is_complete() {
                    produced += 1;
                } else {
                    thread::yield_now();
                }
            }
        })
    };

    let consumer = {
        let ring = Arc::clone(&ring);
        thread::spawn(move || {
            let mut consumed = 0_usize;
            let mut block = [0.0_f32; BLOCK];
            while consumed < TOTAL_BLOCKS {
                let report = ring.read(&mut block);
                if report.is_complete() {
                    for (offset, sample) in block.iter().enumerate() {
                        #[allow(clippy::cast_precision_loss)]
                        let expected = (consumed * BLOCK + offset) as f32;
                        assert_eq!(*sample, expected, "sample out of order");
                    }
                    consumed += 1;
                } else {
                    thread::yield_now();
                }
            }
        })
    };

    producer.join().unwrap();
    consumer.join().unwrap();
}

#[test]
fn for_duration_sizes_the_buffer_for_the_requested_window() {
    let ring = AudioRing::for_duration(48_000, 2, Duration::from_millis(500));
    assert!(ring.capacity() >= 48_000);
}
