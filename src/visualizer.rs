use rustfft::{num_complex::Complex, FftPlanner};

const NUM_BANDS: usize = 64; // Increased from 40 for more detail
const SMOOTHING_FACTOR: f32 = 0.35; // Slightly smoother
const FFT_SIZE: usize = 2048;
const DEFAULT_WAVEFORM_WIDTH: usize = 200; // Default, will be updated dynamically

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerMode {
    FrequencyBars,
    Waveform,
    Off,
}

impl VisualizerMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::FrequencyBars => Self::Waveform,
            Self::Waveform => Self::Off,
            Self::Off => Self::FrequencyBars,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FrequencyBars => "Spectrum",
            Self::Waveform => "Waveform",
            Self::Off => "Off",
        }
    }
}

pub struct Visualizer {
    pub mode: VisualizerMode,
    pub bars: Vec<f32>,
    pub left_bars: Vec<f32>,
    pub right_bars: Vec<f32>,
    pub waveform: Vec<f32>,
    pub peak_bars: Vec<f32>, // Peak hold for falling peaks effect
    planner: FftPlanner<f32>,
    prev_bars: Vec<f32>,
    prev_left: Vec<f32>,
    prev_right: Vec<f32>,
    hanning_window: Vec<f32>,
}

impl Visualizer {
    pub fn new() -> Self {
        let mut hanning_window = vec![0.0f32; FFT_SIZE];
        for i in 0..FFT_SIZE {
            hanning_window[i] =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos());
        }

        Self {
            mode: VisualizerMode::FrequencyBars,
            bars: vec![0.0; NUM_BANDS],
            left_bars: vec![0.0; NUM_BANDS],
            right_bars: vec![0.0; NUM_BANDS],
            waveform: vec![0.0; DEFAULT_WAVEFORM_WIDTH],
            peak_bars: vec![0.0; NUM_BANDS],
            planner: FftPlanner::new(),
            prev_bars: vec![0.0; NUM_BANDS],
            prev_left: vec![0.0; NUM_BANDS],
            prev_right: vec![0.0; NUM_BANDS],
            hanning_window,
        }
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        match self.mode {
            VisualizerMode::FrequencyBars => self.process_fft(samples),
            VisualizerMode::Waveform => self.process_waveform(samples),
            VisualizerMode::Off => {}
        }
    }

    fn process_stereo_fft(&mut self, samples: &[f32]) {
        // Simulate stereo by processing different frequency emphasis for L/R
        // In real stereo, we'd receive interleaved samples
        if samples.len() < FFT_SIZE {
            return;
        }

        // Process main spectrum
        self.process_fft(samples);

        // Create pseudo-stereo by phase-shifting the bars
        for i in 0..NUM_BANDS {
            let base = self.bars[i];
            // Left channel emphasizes lower frequencies
            let left_weight = 1.0 - (i as f32 / NUM_BANDS as f32) * 0.3;
            // Right channel emphasizes higher frequencies
            let right_weight = 0.7 + (i as f32 / NUM_BANDS as f32) * 0.3;

            let new_left = base * left_weight;
            let new_right = base * right_weight;

            // Smooth the values
            self.left_bars[i] = self.prev_left[i] * 0.7 + new_left * 0.3;
            self.right_bars[i] = self.prev_right[i] * 0.7 + new_right * 0.3;
        }

        self.prev_left = self.left_bars.clone();
        self.prev_right = self.right_bars.clone();
    }

    fn process_waveform(&mut self, samples: &[f32]) {
        // Use a larger display width for smoother waveform
        let display_width = DEFAULT_WAVEFORM_WIDTH;
        self.waveform.resize(display_width, 0.0);

        if samples.is_empty() {
            self.waveform.fill(0.0);
            return;
        }

        // Downsample to display width with averaging for smoother output
        let step = samples.len() as f32 / display_width as f32;
        for i in 0..display_width {
            let start_idx = (i as f32 * step) as usize;
            let end_idx = ((i + 1) as f32 * step) as usize;
            let end_idx = end_idx.min(samples.len());

            if start_idx < samples.len() {
                // Average samples in this window for smoother waveform
                let count = (end_idx - start_idx).max(1);
                let sum: f32 = samples[start_idx..end_idx].iter().sum();
                self.waveform[i] = sum / count as f32;
            }
        }
    }

    fn process_fft(&mut self, samples: &[f32]) {
        if samples.len() < FFT_SIZE {
            // Pad with zeros if not enough samples
            let mut padded = samples.to_vec();
            padded.resize(FFT_SIZE, 0.0);
            self.run_fft(&padded);
        } else {
            // Use the last FFT_SIZE samples
            let start = samples.len() - FFT_SIZE;
            self.run_fft(&samples[start..]);
        }
    }

    fn run_fft(&mut self, samples: &[f32]) {
        // Apply Hanning window
        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .enumerate()
            .map(|(i, &s)| Complex::new(s * self.hanning_window[i], 0.0))
            .collect();

        let fft = self.planner.plan_fft_forward(FFT_SIZE);
        fft.process(&mut buffer);

        // Take magnitudes of first half (positive frequencies)
        let half = FFT_SIZE / 2;
        let magnitudes: Vec<f32> = buffer[..half]
            .iter()
            .map(|c| c.norm() / half as f32)
            .collect();

        // Bin into frequency bands with logarithmic spacing
        let mut new_bars = vec![0.0f32; NUM_BANDS];
        for band in 0..NUM_BANDS {
            let lo = log_bin_start(band, NUM_BANDS, half);
            let hi = log_bin_start(band + 1, NUM_BANDS, half);
            let lo = lo.min(half);
            let hi = hi.min(half).max(lo + 1);

            let sum: f32 = magnitudes[lo..hi].iter().sum();
            let count = (hi - lo) as f32;
            new_bars[band] = sum / count;
        }

        // Apply smoothing (exponential moving average)
        for i in 0..NUM_BANDS {
            self.bars[i] = self.prev_bars[i] * (1.0 - SMOOTHING_FACTOR) + new_bars[i] * SMOOTHING_FACTOR;
        }
        self.prev_bars = self.bars.clone();

        // Normalize to 0.0-1.0 range
        let max = self.bars.iter().cloned().fold(0.0f32, f32::max);
        if max > 0.001 {
            for bar in &mut self.bars {
                *bar = (*bar / max).min(1.0);
            }
        }

        // Update peak hold
        self.update_peaks();
    }

    pub fn decay(&mut self) {
        for bar in &mut self.bars {
            *bar *= 0.85;
        }
        for bar in &mut self.left_bars {
            *bar *= 0.85;
        }
        for bar in &mut self.right_bars {
            *bar *= 0.85;
        }
        for peak in &mut self.peak_bars {
            *peak *= 0.92; // Peaks fall slower
        }
        self.prev_bars = self.bars.clone();
        self.prev_left = self.left_bars.clone();
        self.prev_right = self.right_bars.clone();
        for w in &mut self.waveform {
            *w *= 0.85;
        }
    }

    /// Update peak hold values
    fn update_peaks(&mut self) {
        for (i, &bar) in self.bars.iter().enumerate() {
            if bar > self.peak_bars[i] {
                self.peak_bars[i] = bar;
            }
        }
    }
}

/// Compute the starting FFT bin for a given band using logarithmic spacing.
fn log_bin_start(band: usize, num_bands: usize, num_bins: usize) -> usize {
    if band == 0 {
        return 1; // Skip DC component
    }
    let log_min = 1.0f32.ln();
    let log_max = (num_bins as f32).ln();
    let log_pos = log_min + (log_max - log_min) * (band as f32 / num_bands as f32);
    log_pos.exp() as usize
}
