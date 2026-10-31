pub const MAX_OUTPUT: usize = 8;

pub struct LinearResampler {
    step: f64,
    next: f64,
    index: u64,
    previous: f32,
    started: bool,
}

impl LinearResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        let step = input_rate.max(1) as f64 / output_rate.max(1) as f64;
        Self {
            step,
            next: 0.0,
            index: 0,
            previous: 0.0,
            started: false,
        }
    }

    pub fn reset(&mut self) {
        self.next = 0.0;
        self.index = 0;
        self.previous = 0.0;
        self.started = false;
    }

    pub fn push(&mut self, sample: f32, out: &mut [f32; MAX_OUTPUT]) -> usize {
        if !self.started {
            self.previous = sample;
            self.started = true;
            return 0;
        }
        self.index += 1;
        let current = self.index as f64;
        let mut count = 0;
        while self.next < current && count < MAX_OUTPUT {
            let fraction = (self.next - (current - 1.0)) as f32;
            out[count] = self.previous + (sample - self.previous) * fraction;
            count += 1;
            self.next += self.step;
        }
        self.previous = sample;
        count
    }
}
