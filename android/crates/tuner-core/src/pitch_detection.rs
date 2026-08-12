//! FFT based pitch and partial detection.

use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;

/// Highest partial number the detector will look for.
pub const MAX_PARTIAL: u32 = 8;

/// Minimum RMS level of a frame before it is considered to contain a note.
const SILENCE_RMS: f32 = 1e-5;

/// Result of pitch detection for one audio frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchResult {
    /// Estimated frequency of the first partial, in Hz.
    pub fundamental_hz: f64,
    /// Detected partials as `(partial number, frequency in Hz)`.
    pub partials: Vec<(u32, f64)>,
    /// Half magnitude spectrum of the analysed frame.
    pub magnitude_spectrum: Vec<f32>,
    /// Frequency resolution of `magnitude_spectrum`, in Hz per bin.
    pub bin_hz: f64,
}

/// Reusable FFT pitch detector. Creating one detector and reusing it avoids
/// re-planning the FFT on every audio callback.
pub struct PitchDetector {
    fft_size: usize,
    fft: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    window: Vec<f32>,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
}

impl PitchDetector {
    /// Create a detector for a fixed FFT size (must be non-zero and even).
    pub fn new(fft_size: usize) -> Option<Self> {
        if fft_size < 64 || fft_size & 1 != 0 {
            return None;
        }
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
            })
            .collect();
        let input = fft.make_input_vec();
        let spectrum = fft.make_output_vec();
        Some(Self {
            fft_size,
            fft,
            window,
            input,
            spectrum,
        })
    }

    /// FFT size this detector was built for.
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Analyse the most recent `fft_size` samples of `samples`.
    pub fn process(
        &mut self,
        samples: &[f32],
        sample_rate: f64,
        min_hz: f64,
        max_hz: f64,
    ) -> Option<PitchResult> {
        if samples.len() < self.fft_size || sample_rate <= 0.0 {
            return None;
        }

        let start = samples.len() - self.fft_size;
        let frame = &samples[start..];

        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / self.fft_size as f32).sqrt();
        if !rms.is_finite() || rms < SILENCE_RMS {
            return None;
        }

        for ((dst, &src), &w) in self.input.iter_mut().zip(frame).zip(self.window.iter()) {
            *dst = src * w;
        }

        if self
            .fft
            .process(&mut self.input, &mut self.spectrum)
            .is_err()
        {
            return None;
        }

        let half = self.fft_size / 2;
        let magnitudes: Vec<f32> = self.spectrum[..half].iter().map(|c| c.norm()).collect();
        let bin_hz = sample_rate / self.fft_size as f64;

        analyse_spectrum(magnitudes, bin_hz, min_hz, max_hz)
    }
}

/// Detect pitch from a mono `f32` audio buffer.
///
/// * `sample_rate` – e.g. 44100 or 48000.
/// * `fft_size` – e.g. 8192 or 16384.
/// * `min_hz` / `max_hz` – frequency search range.
///
/// Returns `None` for silence, too-short buffers or invalid parameters.
pub fn detect_pitch(
    samples: &[f32],
    sample_rate: f64,
    fft_size: usize,
    min_hz: f64,
    max_hz: f64,
) -> Option<PitchResult> {
    let mut detector = PitchDetector::new(fft_size)?;
    detector.process(samples, sample_rate, min_hz, max_hz)
}

/// Locate the fundamental and its partials in a magnitude spectrum.
fn analyse_spectrum(
    magnitudes: Vec<f32>,
    bin_hz: f64,
    min_hz: f64,
    max_hz: f64,
) -> Option<PitchResult> {
    let half = magnitudes.len();
    if half < 4 || bin_hz <= 0.0 || min_hz >= max_hz {
        return None;
    }

    let min_bin = (min_hz / bin_hz).ceil().max(1.0) as usize;
    let max_bin = ((max_hz / bin_hz).floor().max(0.0) as usize).min(half - 2);
    if min_bin >= max_bin {
        return None;
    }

    let peak_bin = (min_bin..=max_bin).max_by(|&a, &b| magnitudes[a].total_cmp(&magnitudes[b]))?;
    let peak_mag = magnitudes[peak_bin];
    if peak_mag < 1e-6 {
        return None;
    }

    let fundamental_hz = parabolic_interpolation(&magnitudes, peak_bin, bin_hz);
    if fundamental_hz <= 0.0 || !fundamental_hz.is_finite() {
        return None;
    }

    let mut partials = vec![(1u32, fundamental_hz)];
    for n in 2..=MAX_PARTIAL {
        // Partials of a stiff string are sharp, so search slightly above the
        // harmonic position as well as below it.
        let expected_hz = fundamental_hz * n as f64;
        if expected_hz > max_hz * 1.5 {
            break;
        }
        let expected_bin = (expected_hz / bin_hz) as usize;
        if expected_bin + 2 >= half {
            break;
        }
        let search = ((expected_hz * 0.05) / bin_hz) as usize + 1;
        let lo = expected_bin.saturating_sub(search).max(1);
        let hi = (expected_bin + search).min(half - 2);
        if lo >= hi {
            continue;
        }
        let Some(p_bin) = (lo..=hi).max_by(|&a, &b| magnitudes[a].total_cmp(&magnitudes[b])) else {
            continue;
        };
        if magnitudes[p_bin] < peak_mag * 0.01 {
            continue; // too weak to trust
        }
        let p_hz = parabolic_interpolation(&magnitudes, p_bin, bin_hz);
        if p_hz > 0.0 && p_hz.is_finite() {
            partials.push((n, p_hz));
        }
    }

    Some(PitchResult {
        fundamental_hz,
        partials,
        magnitude_spectrum: magnitudes,
        bin_hz,
    })
}

/// Refine a spectral peak position with parabolic interpolation over the
/// neighbouring bins, giving sub-bin (and therefore sub-cent) resolution.
fn parabolic_interpolation(magnitudes: &[f32], bin: usize, bin_hz: f64) -> f64 {
    if bin == 0 || bin + 1 >= magnitudes.len() {
        return bin as f64 * bin_hz;
    }
    let y0 = magnitudes[bin - 1] as f64;
    let y1 = magnitudes[bin] as f64;
    let y2 = magnitudes[bin + 1] as f64;
    let denom = y0 - 2.0 * y1 + y2;
    if denom.abs() < 1e-15 {
        return bin as f64 * bin_hz;
    }
    let delta = (0.5 * (y0 - y2) / denom).clamp(-0.5, 0.5);
    (bin as f64 + delta) * bin_hz
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn sine_wave(freq: f64, sample_rate: f64, n_samples: usize) -> Vec<f32> {
        (0..n_samples)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sample_rate).sin() as f32)
            .collect()
    }

    fn inharmonic_tone(f1: f64, b: f64, sample_rate: f64, n_samples: usize) -> Vec<f32> {
        (0..n_samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (1..=6)
                    .map(|n| {
                        let f = crate::inharmonicity::partial_freq(f1, b, n);
                        (2.0 * std::f64::consts::PI * f * t).sin() / n as f64
                    })
                    .sum::<f64>() as f32
            })
            .collect()
    }

    #[test]
    fn test_detect_440hz() {
        let sr = 44100.0;
        let samples = sine_wave(440.0, sr, 16384);
        let result = detect_pitch(&samples, sr, 8192, 20.0, 8000.0).unwrap();
        assert_abs_diff_eq!(result.fundamental_hz, 440.0, epsilon = 1.0);
    }

    #[test]
    fn test_detect_returns_none_for_silence() {
        let samples = vec![0.0f32; 8192];
        let result = detect_pitch(&samples, 44100.0, 8192, 20.0, 8000.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_returns_none_for_short_buffer() {
        let samples = sine_wave(440.0, 44100.0, 1000);
        assert!(detect_pitch(&samples, 44100.0, 8192, 20.0, 8000.0).is_none());
    }

    #[test]
    fn test_invalid_parameters_are_rejected() {
        let samples = sine_wave(440.0, 44100.0, 16384);
        assert!(detect_pitch(&samples, 44100.0, 0, 20.0, 8000.0).is_none());
        assert!(detect_pitch(&samples, 0.0, 8192, 20.0, 8000.0).is_none());
        assert!(detect_pitch(&samples, 44100.0, 8192, 8000.0, 20.0).is_none());
    }

    #[test]
    fn test_detects_partials_of_inharmonic_tone() {
        let sr = 44100.0;
        let f1 = 220.0;
        let b = 0.0003;
        let samples = inharmonic_tone(f1, b, sr, 32768);
        let result = detect_pitch(&samples, sr, 16384, 100.0, 8000.0).unwrap();
        assert_abs_diff_eq!(result.fundamental_hz, f1, epsilon = 1.0);
        assert!(result.partials.len() >= 4, "expected several partials");
        for &(n, f) in &result.partials {
            let expected = crate::inharmonicity::partial_freq(f1, b, n);
            assert_abs_diff_eq!(f, expected, epsilon = expected * 0.01);
        }
    }

    #[test]
    fn test_reused_detector_matches_one_shot() {
        let sr = 48000.0;
        let samples = sine_wave(261.6, sr, 16384);
        let mut detector = PitchDetector::new(8192).unwrap();
        assert_eq!(detector.fft_size(), 8192);
        let a = detector.process(&samples, sr, 20.0, 8000.0).unwrap();
        let b = detect_pitch(&samples, sr, 8192, 20.0, 8000.0).unwrap();
        assert_abs_diff_eq!(a.fundamental_hz, b.fundamental_hz, epsilon = 1e-6);
        assert_abs_diff_eq!(a.bin_hz, sr / 8192.0, epsilon = 1e-9);
        assert_eq!(a.magnitude_spectrum.len(), 4096);
    }

    #[test]
    fn test_detector_rejects_bad_sizes() {
        assert!(PitchDetector::new(0).is_none());
        assert!(PitchDetector::new(1023).is_none());
    }
}
