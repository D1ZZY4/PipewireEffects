use super::DELAY_LENGTH;

pub(super) struct Channel {
    pub(super) delay: Vec<f32>,
    pub(super) head: usize,
    pub(super) hold: f32,
    pub(super) envelope: f32,
    goal: f32,
    slope: f32,
    steps: usize,
    clear: usize,
    pub(super) window: usize,
}

impl Channel {
    pub(super) fn new() -> Self {
        Self {
            delay: vec![0.0; DELAY_LENGTH],
            head: 0,
            hold: 1.0,
            envelope: 1.0,
            goal: 1.0,
            slope: 0.0,
            steps: 0,
            clear: 0,
            window: 1,
        }
    }

    pub(super) fn reset(&mut self) {
        self.delay.fill(0.0);
        self.head = 0;
        self.hold = 1.0;
        self.envelope = 1.0;
        self.goal = 1.0;
        self.slope = 0.0;
        self.steps = 0;
        self.clear = 0;
    }

    fn release_step(&mut self, release: f32, alr: bool) {
        let depth = (1.0 - self.envelope).clamp(0.0, 1.0);
        let coefficient = if alr {
            (release * (1.0 + 6.0 * depth)).min(1.0)
        } else {
            release
        };
        self.envelope += (1.0 - self.envelope) * coefficient;
    }

    pub(super) fn advance(
        &mut self,
        need: f32,
        target: f32,
        ramp: usize,
        release: f32,
        exact: f32,
        alr: bool,
    ) {
        if need >= 1.0 {
            self.clear = (self.clear + 1).min(self.window);
        } else {
            self.clear = 0;
        }
        if self.clear < self.window {
            if target < self.goal {
                self.goal = target;
                self.slope = (self.envelope - target) / ramp.max(1) as f32;
                self.steps = ramp;
            }
            if self.steps > 0 {
                self.steps -= 1;
                self.envelope = (self.envelope - self.slope).max(self.goal);
            } else {
                self.goal = 1.0;
                self.release_step(release, alr);
            }
        } else {
            self.goal = 1.0;
            self.steps = 0;
            self.release_step(release, alr);
        }
        self.envelope = self.envelope.min(exact).max(1.0e-5);
    }
}
