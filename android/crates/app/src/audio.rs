//! Microphone capture and continuous pitch analysis.
//!
//! Audio capture uses `cpal` (backed by Oboe/AAudio) on Android. On other
//! platforms the engine still exists and can be fed samples manually, which
//! keeps the analysis path unit testable on the host.

use std::sync::{Arc, Mutex};

use tuner_core::pitch_detection::{PitchDetector, PitchResult};

/// Number of samples kept for analysis (~1.4 s at 48 kHz).
pub const RING_BUFFER_SIZE: usize = 65_536;
/// FFT size used for pitch detection.
pub const FFT_SIZE: usize = 8_192;
/// Lowest frequency the tuner looks for (below A0).
pub const MIN_HZ: f64 = 20.0;
/// Highest frequency the tuner looks for (above C8).
pub const MAX_HZ: f64 = 8_000.0;
/// Fallback sample rate used before a device has been opened.
pub const DEFAULT_SAMPLE_RATE: f64 = 48_000.0;

/// Shared analysis state written by the audio thread and read by the UI.
pub struct AudioState {
    /// Most recent successful analysis, if any.
    pub latest_result: Option<PitchResult>,
    /// Sample rate of the capture device.
    pub sample_rate: f64,
    /// Human readable error to surface in the UI, if capture failed.
    pub error: Option<String>,
    ring_buffer: Vec<f32>,
    detector: Option<PitchDetector>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            latest_result: None,
            sample_rate: DEFAULT_SAMPLE_RATE,
            error: None,
            ring_buffer: vec![0.0; RING_BUFFER_SIZE],
            detector: PitchDetector::new(FFT_SIZE),
        }
    }
}

impl AudioState {
    /// Append captured samples to the ring buffer and re-run pitch detection.
    ///
    /// Safe to call with buffers of any length, including ones larger than the
    /// ring buffer itself.
    pub fn push_samples(&mut self, data: &[f32]) {
        if data.is_empty() {
            return;
        }
        let buf = &mut self.ring_buffer;
        if data.len() >= RING_BUFFER_SIZE {
            buf.copy_from_slice(&data[data.len() - RING_BUFFER_SIZE..]);
        } else {
            buf.rotate_left(data.len());
            let start = RING_BUFFER_SIZE - data.len();
            buf[start..].copy_from_slice(data);
        }

        let sample_rate = self.sample_rate;
        if let Some(detector) = self.detector.as_mut() {
            if let Some(result) = detector.process(&self.ring_buffer, sample_rate, MIN_HZ, MAX_HZ) {
                self.latest_result = Some(result);
            }
        }
    }

    /// Read-only view of the captured samples (oldest first).
    pub fn ring_buffer(&self) -> &[f32] {
        &self.ring_buffer
    }
}

/// Owns the capture stream and the shared [`AudioState`].
pub struct AudioEngine {
    #[cfg(target_os = "android")]
    _stream: Option<cpal::Stream>,
    pub state: Arc<Mutex<AudioState>>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    /// Create the engine and, on Android, start capturing from the default input.
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(AudioState::default()));

        #[cfg(target_os = "android")]
        {
            let stream = match Self::try_start_stream(Arc::clone(&state)) {
                Ok(stream) => Some(stream),
                Err(err) => {
                    log::error!("Audio capture unavailable: {err}");
                    if let Ok(mut s) = state.lock() {
                        s.error = Some(err);
                    }
                    None
                }
            };
            return Self {
                _stream: stream,
                state,
            };
        }

        #[cfg(not(target_os = "android"))]
        {
            if let Ok(mut s) = state.lock() {
                s.error = Some("Microphone capture is only available on Android".into());
            }
            Self { state }
        }
    }

    /// Feed samples into the analysis pipeline manually (host builds and tests).
    pub fn feed_samples(&self, data: &[f32]) {
        if let Ok(mut state) = self.state.lock() {
            state.push_samples(data);
        }
    }

    /// Latest analysis result, cloned out of the shared state.
    pub fn latest_result(&self) -> Option<PitchResult> {
        self.state.lock().ok()?.latest_result.clone()
    }

    #[cfg(target_os = "android")]
    fn try_start_stream(state: Arc<Mutex<AudioState>>) -> Result<cpal::Stream, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("No usable input config: {e}"))?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let sample_rate = config.sample_rate.0 as f64;
        let channels = config.channels.max(1) as usize;

        {
            let mut s = state
                .lock()
                .map_err(|_| "Audio state lock poisoned".to_string())?;
            s.sample_rate = sample_rate;
            s.error = None;
        }

        let err_state = Arc::clone(&state);
        let err_fn = move |err| {
            log::error!("Audio stream error: {err}");
            if let Ok(mut s) = err_state.lock() {
                s.error = Some(format!("Audio stream error: {err}"));
            }
        };

        macro_rules! build_stream {
            ($sample:ty) => {{
                let state = Arc::clone(&state);
                let mut mono: Vec<f32> = Vec::new();
                device.build_input_stream(
                    &config,
                    move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                        mono.clear();
                        mono.extend(data.chunks(channels).map(|frame| {
                            let sum: f32 = frame
                                .iter()
                                .map(|s| cpal::Sample::to_float_sample(*s))
                                .sum();
                            sum / frame.len() as f32
                        }));
                        if let Ok(mut s) = state.lock() {
                            s.push_samples(&mono);
                        }
                    },
                    err_fn,
                    None,
                )
            }};
        }

        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream!(f32),
            cpal::SampleFormat::I16 => build_stream!(i16),
            cpal::SampleFormat::U16 => build_stream!(u16),
            other => return Err(format!("Unsupported sample format {other:?}")),
        }
        .map_err(|e| format!("Failed to open input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start input stream: {e}"))?;
        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, sample_rate: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sample_rate).sin() as f32)
            .collect()
    }

    #[test]
    fn test_push_samples_detects_pitch() {
        let mut state = AudioState {
            sample_rate: 48_000.0,
            ..Default::default()
        };
        state.push_samples(&sine(440.0, 48_000.0, 16_384));
        let result = state.latest_result.expect("pitch should be detected");
        assert!((result.fundamental_hz - 440.0).abs() < 2.0);
    }

    #[test]
    fn test_push_samples_handles_oversized_and_empty_buffers() {
        let mut state = AudioState::default();
        state.push_samples(&[]);
        assert!(state.latest_result.is_none());
        state.push_samples(&sine(440.0, DEFAULT_SAMPLE_RATE, RING_BUFFER_SIZE * 2));
        assert_eq!(state.ring_buffer().len(), RING_BUFFER_SIZE);
        assert!(state.latest_result.is_some());
    }

    #[test]
    fn test_ring_buffer_keeps_newest_samples() {
        let mut state = AudioState::default();
        let data: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        state.push_samples(&data);
        let ring = state.ring_buffer();
        assert_eq!(ring.len(), RING_BUFFER_SIZE);
        assert_eq!(ring[RING_BUFFER_SIZE - 1], 999.0);
        assert_eq!(ring[RING_BUFFER_SIZE - 1000], 0.0);
    }

    #[test]
    fn test_engine_feed_and_read() {
        let engine = AudioEngine::new();
        assert!(engine.latest_result().is_none());
        engine.feed_samples(&sine(220.0, DEFAULT_SAMPLE_RATE, 16_384));
        let result = engine.latest_result().expect("pitch should be detected");
        assert!((result.fundamental_hz - 220.0).abs() < 2.0);
    }
}
