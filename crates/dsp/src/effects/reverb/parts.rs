use pwe_core::math;

#[derive(Debug)]
pub(super) struct Comb {
    buffer: Vec<f32>,
    length: usize,
    index: usize,
    filter_state: f32,
    pub(super) feedback: f32,
    pub(super) damping: f32,
}

impl Comb {
    pub(super) fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buffer: vec![0.0; capacity],
            length: capacity,
            index: 0,
            filter_state: 0.0,
            feedback: 0.5,
            damping: 0.5,
        }
    }

    pub(super) fn set_length(&mut self, length: usize) {
        let length = length.clamp(1, self.buffer.len());
        if length != self.length {
            self.length = length;
            self.index %= length;
        }
    }

    pub(super) fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter_state =
            math::sanitize(output * (1.0 - self.damping) + self.filter_state * self.damping);
        self.buffer[self.index] = math::sanitize(input + self.filter_state * self.feedback);
        self.index += 1;
        if self.index >= self.length {
            self.index = 0;
        }
        output
    }

    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.filter_state = 0.0;
        self.index = 0;
    }
}

#[derive(Debug)]
pub(super) struct AllPass {
    buffer: Vec<f32>,
    length: usize,
    index: usize,
    pub(super) feedback: f32,
}

impl AllPass {
    pub(super) fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buffer: vec![0.0; capacity],
            length: capacity,
            index: 0,
            feedback: 0.5,
        }
    }

    pub(super) fn set_length(&mut self, length: usize) {
        let length = length.clamp(1, self.buffer.len());
        if length != self.length {
            self.length = length;
            self.index %= length;
        }
    }

    pub(super) fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = -input + buffered;
        self.buffer[self.index] = math::sanitize(input + buffered * self.feedback);
        self.index += 1;
        if self.index >= self.length {
            self.index = 0;
        }
        output
    }

    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}
