use crate::fft::Fft;

pub const BLOCK: usize = 256;

pub struct Partitioned {
    block: usize,
    size: usize,
    fft: Fft,
    spectra_re: Vec<Vec<f32>>,
    spectra_im: Vec<Vec<f32>>,
    rings_re: Vec<Vec<f32>>,
    rings_im: Vec<Vec<f32>>,
    newest: usize,
    history: Vec<f32>,
    input: Vec<f32>,
    input_pos: usize,
    pending: Vec<f32>,
    pending_pos: usize,
    work_re: Vec<f32>,
    work_im: Vec<f32>,
    sum_re: Vec<f32>,
    sum_im: Vec<f32>,
}

impl Partitioned {
    pub fn new(block: usize, impulse: &[f32]) -> Self {
        let block = block.max(2).next_power_of_two();
        let size = block * 2;
        let splits = impulse.len().div_ceil(block).max(1);
        let mut spectra_re = vec![vec![0.0; size]; splits];
        let mut spectra_im = vec![vec![0.0; size]; splits];
        let fft = Fft::new(size);
        for (index, (re, im)) in spectra_re.iter_mut().zip(spectra_im.iter_mut()).enumerate() {
            let start = index * block;
            let end = (start + block).min(impulse.len());
            if start < end {
                re[..end - start].copy_from_slice(&impulse[start..end]);
            }
            fft.forward(re, im);
        }
        Self {
            block,
            size,
            fft,
            spectra_re,
            spectra_im,
            rings_re: vec![vec![0.0; size]; splits],
            rings_im: vec![vec![0.0; size]; splits],
            newest: 0,
            history: vec![0.0; block],
            input: vec![0.0; block],
            input_pos: 0,
            pending: vec![0.0; block],
            pending_pos: 0,
            work_re: vec![0.0; size],
            work_im: vec![0.0; size],
            sum_re: vec![0.0; size],
            sum_im: vec![0.0; size],
        }
    }

    pub fn reset(&mut self) {
        for ring in &mut self.rings_re {
            ring.fill(0.0);
        }
        for ring in &mut self.rings_im {
            ring.fill(0.0);
        }
        self.history.fill(0.0);
        self.input.fill(0.0);
        self.pending.fill(0.0);
        self.newest = 0;
        self.input_pos = 0;
        self.pending_pos = 0;
    }

    pub fn feed(&mut self, sample: f32) -> f32 {
        let output = self.pending[self.pending_pos];
        self.pending_pos += 1;
        if self.pending_pos == self.block {
            self.pending_pos = 0;
        }
        self.input[self.input_pos] = sample;
        self.input_pos += 1;
        if self.input_pos == self.block {
            self.input_pos = 0;
            self.convolve();
        }
        output
    }

    fn convolve(&mut self) {
        let size = self.size;
        let block = self.block;
        for index in 0..block {
            self.work_re[index] = self.history[index];
            self.work_re[block + index] = self.input[index];
        }
        self.work_im.fill(0.0);
        self.fft.forward(&mut self.work_re, &mut self.work_im);
        self.rings_re[self.newest].copy_from_slice(&self.work_re[..size]);
        self.rings_im[self.newest].copy_from_slice(&self.work_im[..size]);
        self.newest = (self.newest + 1) % self.spectra_re.len();
        self.sum_re.fill(0.0);
        self.sum_im.fill(0.0);
        let partitions = self.spectra_re.len();
        for partition in 0..partitions {
            let slot = (self.newest + partitions - 1 - partition) % partitions;
            let xr = &self.rings_re[slot];
            let xi = &self.rings_im[slot];
            let hr = &self.spectra_re[partition];
            let hi = &self.spectra_im[partition];
            for bin in 0..size {
                self.sum_re[bin] += xr[bin] * hr[bin] - xi[bin] * hi[bin];
                self.sum_im[bin] += xr[bin] * hi[bin] + xi[bin] * hr[bin];
            }
        }
        self.fft.inverse(&mut self.sum_re, &mut self.sum_im);
        for index in 0..block {
            self.pending[index] = self.sum_re[block + index];
        }
        self.history.copy_from_slice(&self.input);
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
