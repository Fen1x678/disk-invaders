//! A two-layer neural network (multilayer perceptron), written from scratch.
//!
//! ```text
//!  input (8207 numbers, mostly 0)  ──W1──►  24 hidden neurons (ReLU)  ──W2──►  3 outputs (softmax)
//!                                                                                delete / review / keep
//! ```
//!
//! Training is plain stochastic gradient descent with backpropagation. No libraries:
//! every multiplication is right here, so you can read how a neural network actually learns.

use super::features::{Input, INPUT_DIM};

pub const HIDDEN: usize = 24;
pub const OUTPUTS: usize = 3;
const SMOOTHING: f32 = 0.1;
/// Weight decay: pulls unused weights back to zero so the brain doesn't memorize noise.
const DECAY: f32 = 1e-3;

#[derive(Debug, Clone)]
pub struct Mlp {
    /// `w1[i * HIDDEN + j]`: weight from input `i` to hidden neuron `j`.
    w1: Vec<f32>,
    b1: [f32; HIDDEN],
    w2: [[f32; OUTPUTS]; HIDDEN],
    b2: [f32; OUTPUTS],
}

/// One training example: input, correct class, importance.
pub struct Example {
    pub input: Input,
    pub class: usize,
    pub weight: f32,
}

impl Mlp {
    /// Small random weights. Same seed, same network: tests and training are reproducible.
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng(seed | 1);
        let mut w2 = [[0.0; OUTPUTS]; HIDDEN];
        for row in &mut w2 {
            for w in row.iter_mut() {
                *w = rng.uniform(0.4);
            }
        }
        Self {
            w1: (0..INPUT_DIM * HIDDEN).map(|_| rng.uniform(0.1)).collect(),
            b1: [0.0; HIDDEN],
            w2,
            b2: [0.0; OUTPUTS],
        }
    }

    /// Returns the class probabilities for an input.
    pub fn predict(&self, input: &Input) -> [f32; OUTPUTS] {
        self.forward(input).1
    }

    fn forward(&self, input: &Input) -> ([f32; HIDDEN], [f32; OUTPUTS]) {
        let mut hidden = self.b1;
        for &(i, x) in input {
            let row = &self.w1[i * HIDDEN..(i + 1) * HIDDEN];
            for (h, w) in hidden.iter_mut().zip(row) {
                *h += x * w;
            }
        }
        for h in &mut hidden {
            *h = h.max(0.0); // ReLU
        }
        let mut logits = self.b2;
        for (h, row) in hidden.iter().zip(&self.w2) {
            for (logit, w) in logits.iter_mut().zip(row) {
                *logit += h * w;
            }
        }
        (hidden, softmax(logits))
    }

    /// Stochastic gradient descent over `examples` for a number of epochs.
    pub fn train(&mut self, examples: &[Example], epochs: usize, learning_rate: f32, seed: u64) {
        let mut order: Vec<usize> = (0..examples.len()).collect();
        let mut rng = Rng(seed | 1);
        for epoch in 0..epochs {
            rng.shuffle(&mut order);
            // Take big steps first, small careful steps at the end.
            let lr = learning_rate * (1.0 - 0.9 * epoch as f32 / epochs.max(1) as f32);
            for &k in &order {
                self.step(&examples[k], lr);
            }
        }
    }

    /// One step of backpropagation: nudge every weight to make the right answer a bit more likely.
    fn step(&mut self, example: &Example, lr: f32) {
        let (hidden, probs) = self.forward(&example.input);
        // Gradient of the cross-entropy loss with respect to the logits is simply (p - target).
        // "Label smoothing": aim for 90% instead of 100%, so the brain stays humble.
        let mut d_logits = probs;
        for (k, d) in d_logits.iter_mut().enumerate() {
            *d -= if k == example.class { 1.0 - SMOOTHING } else { SMOOTHING / (OUTPUTS - 1) as f32 };
        }
        for d in &mut d_logits {
            *d *= example.weight;
        }
        // Walk back through layer 2: how much did each hidden neuron contribute to the error?
        let mut d_hidden = [0.0; HIDDEN];
        for ((dh, w2_row), h) in d_hidden.iter_mut().zip(&mut self.w2).zip(hidden) {
            for (w, d) in w2_row.iter_mut().zip(d_logits) {
                *dh += *w * d;
                *w -= lr * (h * d + DECAY * *w);
            }
            if h <= 0.0 {
                *dh = 0.0; // ReLU lets no gradient through when it was off
            }
        }
        for (b, d) in self.b2.iter_mut().zip(d_logits) {
            *b -= lr * d;
        }
        for (b, d) in self.b1.iter_mut().zip(d_hidden) {
            *b -= lr * d;
        }
        for &(i, x) in &example.input {
            let row = &mut self.w1[i * HIDDEN..(i + 1) * HIDDEN];
            for (w, d) in row.iter_mut().zip(&d_hidden) {
                *w -= lr * (x * d + DECAY * *w);
            }
        }
    }
}

fn softmax(logits: [f32; OUTPUTS]) -> [f32; OUTPUTS] {
    let max = logits.iter().cloned().fold(f32::MIN, f32::max);
    let exps = logits.map(|l| (l - max).exp());
    let sum: f32 = exps.iter().sum();
    exps.map(|e| e / sum)
}

/// xorshift64*: a tiny random number generator, good enough for shuffling and initial weights.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in [-scale, scale].
    pub fn uniform(&mut self, scale: f32) -> f32 {
        ((self.next() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0) * scale
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_a_toy_problem() {
        // Feature 0 means class 0, feature 1 means class 2.
        let examples: Vec<Example> = (0..40)
            .map(|k| Example { input: vec![(k % 2, 1.0)], class: if k % 2 == 0 { 0 } else { 2 }, weight: 1.0 })
            .collect();
        let mut net = Mlp::new(7);
        net.train(&examples, 60, 0.2, 7);
        assert!(net.predict(&vec![(0, 1.0)])[0] > 0.8);
        assert!(net.predict(&vec![(1, 1.0)])[2] > 0.8);
    }

    #[test]
    fn probabilities_sum_to_one() {
        let p = Mlp::new(1).predict(&vec![(5, 1.0), (100, 0.5)]);
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
}
