mod accelerator;
mod dispatch;
mod probe;
mod resources;
mod scaler;

use std::sync::Arc;

pub use accelerator::Accelerator;
pub use dispatch::scale;
pub use probe::{DeviceInfo, devices};
pub use scaler::Scaler;

use crate::shared::Shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioBackend {
    #[default]
    Cpu,
    Gpu,
    Both,
}

impl AudioBackend {
    pub fn parse(value: &str) -> Self {
        match value {
            "gpu" => Self::Gpu,
            "both" => Self::Both,
            _ => Self::Cpu,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Both => "both",
        }
    }

    pub fn accelerator(
        self,
        capacity: usize,
        shared: Arc<Shared>,
    ) -> Option<Box<dyn pwe_core::Accelerator>> {
        match self {
            Self::Cpu => None,
            Self::Gpu => Accelerator::new(capacity, false, shared)
                .map(|value| Box::new(value) as Box<dyn pwe_core::Accelerator>),
            Self::Both => Accelerator::new(capacity, true, shared)
                .map(|value| Box::new(value) as Box<dyn pwe_core::Accelerator>),
        }
    }
}

pub fn available() -> bool {
    !devices().is_empty()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn the_probe_reports_every_vulkan_device() {
        let found = devices();
        if found.is_empty() {
            eprintln!("no Vulkan device is available; the probe test was not executed");
            return;
        }
        for device in &found {
            eprintln!("vulkan device: {} ({:?})", device.name, device.kind);
        }
        assert!(found.iter().all(|device| !device.name.is_empty()));
    }

    #[test]
    fn a_persistent_scaler_processes_repeated_blocks() {
        let Some(mut scaler) = Scaler::new(2048) else {
            eprintln!("no Vulkan device is available; the scaler test was not executed");
            return;
        };
        for round in 1..=4 {
            let mut block: Vec<f32> = (0..2048).map(|index| index as f32 / 2048.0).collect();
            let expected = block.clone();
            assert!(scaler.process(&mut block, 0.25, 10_000_000));
            for (value, source) in block.iter().zip(expected.iter()) {
                assert!(
                    (value - source * 0.25).abs() < 1.0e-6,
                    "round {round}: {value}"
                );
            }
        }
    }

    #[test]
    fn the_compute_pipeline_scales_a_buffer_without_clipping() {
        let input: Vec<f32> = (0..1000).map(|index| index as f32 / 1000.0).collect();
        let Some(output) = scale(&input, 4.0) else {
            eprintln!("no Vulkan device is available; the compute test was not executed");
            return;
        };
        assert_eq!(output.len(), input.len());
        for (value, expected) in output.iter().zip(input.iter()) {
            assert!(
                (value - expected * 4.0).abs() < 1.0e-5,
                "{value} is not {}",
                expected * 4.0
            );
        }
    }

    #[test]
    fn every_backend_key_round_trips() {
        for backend in [AudioBackend::Cpu, AudioBackend::Gpu, AudioBackend::Both] {
            assert_eq!(AudioBackend::parse(backend.key()), backend);
        }
        assert_eq!(AudioBackend::parse("nonsense"), AudioBackend::Cpu);
    }

    #[test]
    fn the_accelerator_matches_the_cpu_on_a_real_block() {
        use pwe_core::Accelerator as _;

        let shared = Arc::new(Shared::new());
        let Some(mut accelerator) = Accelerator::new(4096, false, Arc::clone(&shared)) else {
            eprintln!("no Vulkan device is available; the accelerator test was not executed");
            return;
        };
        let mut buffer = pwe_core::AudioBuffer::new(2, 2048);
        buffer
            .configure(2, 1024)
            .expect("the buffer should configure");
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample = ((index as f32) * 0.001 + channel as f32).sin() * 0.75;
            }
        }
        let expected: Vec<f32> = (0..2)
            .flat_map(|channel| {
                buffer
                    .channel(channel)
                    .iter()
                    .map(|v| v * 0.25)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(accelerator.scale(&mut buffer, 0.25, 10_000_000));
        let produced: Vec<f32> = (0..2)
            .flat_map(|channel| buffer.channel(channel).to_vec())
            .collect();
        for (value, reference) in produced.iter().zip(expected.iter()) {
            assert!(
                (value - reference).abs() < 1.0e-6,
                "{value} is not {reference}"
            );
        }
        assert_eq!(shared.stats().gpu_blocks(), 1);
        assert_eq!(shared.stats().gpu_fallbacks(), 0);
    }

    #[test]
    fn the_verifying_backend_reports_an_agreement() {
        use pwe_core::Accelerator as _;

        let shared = Arc::new(Shared::new());
        let Some(mut accelerator) = Accelerator::new(4096, true, Arc::clone(&shared)) else {
            eprintln!("no Vulkan device is available; the verification test was not executed");
            return;
        };
        let mut buffer = pwe_core::AudioBuffer::new(2, 256);
        buffer
            .configure(2, 128)
            .expect("the buffer should configure");
        for channel in 0..2 {
            buffer.channel_mut(channel).fill(0.5);
        }
        assert!(accelerator.scale(&mut buffer, 0.5, 10_000_000));
        assert_eq!(shared.stats().gpu_blocks(), 1);
        assert_eq!(shared.stats().gpu_disagreements(), 0);
        assert!((buffer.channel(0)[0] - 0.25).abs() < 1.0e-6);
    }

    #[test]
    #[ignore = "a measurement, not a pass or fail check"]
    fn the_gpu_path_is_slower_than_the_cpu_for_one_block() {
        use pwe_core::Accelerator as _;

        let shared = Arc::new(Shared::new());
        let Some(mut accelerator) = Accelerator::new(4096, false, Arc::clone(&shared)) else {
            eprintln!("no Vulkan device is available; the timing test was not executed");
            return;
        };
        let mut buffer = pwe_core::AudioBuffer::new(2, 2048);
        buffer
            .configure(2, 1024)
            .expect("the buffer should configure");
        for channel in 0..2 {
            buffer.channel_mut(channel).fill(0.25);
        }
        let rounds = 100;
        let start = Instant::now();
        for _ in 0..rounds {
            for channel in buffer.channels_mut() {
                for sample in channel {
                    *sample *= 0.5;
                }
            }
            std::hint::black_box(buffer.channel(0)[0]);
        }
        let cpu = start.elapsed().as_secs_f64() / f64::from(rounds);
        let start = Instant::now();
        for _ in 0..rounds {
            assert!(accelerator.scale(&mut buffer, 1.0, 10_000_000));
            std::hint::black_box(buffer.channel(0)[0]);
        }
        let persistent = start.elapsed().as_secs_f64() / f64::from(rounds);
        let start = Instant::now();
        let oneshot = scale(&vec![0.25f32; 2048], 1.0);
        let setup = start.elapsed().as_secs_f64();
        assert!(oneshot.is_some());
        println!(
            "one 1024-frame stereo block: CPU {:.3} us, GPU stage {:.1} us, GPU with setup {:.1} us",
            cpu * 1.0e6,
            persistent * 1.0e6,
            setup * 1.0e6
        );
        assert!(
            persistent > cpu,
            "the GPU path is no longer slower than the CPU; revisit the backend decision"
        );
    }
}
