pub struct Dither {
    state: u32,
}

impl Dither {
    pub fn new() -> Self {
        Self { state: 0x9E37_79B9 }
    }

    fn uniform(&mut self) -> f32 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.state = value;
        (value as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn sample(&mut self, mode: usize, amplitude: f32) -> f32 {
        match mode {
            1 => self.uniform() * amplitude,
            2 => (self.uniform() + self.uniform()) * 0.5 * amplitude,
            _ => 0.0,
        }
    }
}
