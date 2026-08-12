//! Application state shared by all UI screens.

use std::collections::HashMap;

use tuner_core::{
    cents::{freq_to_cents, key_name, key_to_freq, nearest_key},
    inharmonicity::{fit_inharmonicity, interpolate_b_values},
    temperament::Temperament,
    tuning_curve::{compute_overpull, generate_tuning_curve, IntervalWeights},
    tuning_file::{TemperamentData, TuningFile},
    NUM_KEYS,
};

use crate::storage;

/// Top level screens of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Tuner,
    Spectrum,
    Measurement,
    Settings,
    Library,
}

/// Cent tolerance within which a note counts as in tune.
pub const IN_TUNE_CENTS: f64 = 2.0;
/// Cent tolerance within which a note counts as close.
pub const CLOSE_CENTS: f64 = 5.0;

/// Everything the UI needs to render and everything the user can change.
#[derive(Debug, Clone)]
pub struct AppState {
    pub current_screen: Screen,
    pub a4_hz: f64,
    pub calibration_cents: f64,
    pub stretch: f64,
    pub interval_weights: IntervalWeights,
    pub temperament_index: usize,
    pub temperaments: Vec<Temperament>,

    // Pitch detection results
    pub detected_fundamental_hz: Option<f64>,
    pub detected_cents_deviation: f64,
    pub detected_key_index: Option<usize>,
    pub locked_key_index: Option<usize>,
    pub magnitude_spectrum: Vec<f32>,
    pub spectrum_bin_hz: f64,
    pub detected_partials: Vec<(u32, f64)>,

    // Inharmonicity measurements
    /// key index -> measured inharmonicity coefficient B.
    pub measured_b_values: HashMap<usize, f64>,
    /// 88 interpolated B values.
    pub b_curve: Vec<f64>,
    /// 88 cent offsets from equal temperament.
    pub tuning_curve: Vec<f64>,

    // Pitch raise
    pub pitch_raise_mode: bool,
    pub overpull_factor: f64,
    /// Current measured deviation per key, in cents (negative = flat).
    pub current_deviation: Vec<f64>,

    // Library
    pub saved_tunings: Vec<TuningFile>,
    pub current_tuning_name: String,
    pub status_message: String,
    pub error_message: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Tuner,
            a4_hz: 440.0,
            calibration_cents: 0.0,
            stretch: 1.0,
            interval_weights: IntervalWeights::default(),
            temperament_index: 0,
            temperaments: Temperament::all_presets(),
            detected_fundamental_hz: None,
            detected_cents_deviation: 0.0,
            detected_key_index: None,
            locked_key_index: None,
            magnitude_spectrum: Vec::new(),
            spectrum_bin_hz: 0.0,
            detected_partials: Vec::new(),
            measured_b_values: HashMap::new(),
            b_curve: vec![0.0002; NUM_KEYS],
            tuning_curve: vec![0.0; NUM_KEYS],
            pitch_raise_mode: false,
            overpull_factor: 0.3,
            current_deviation: vec![0.0; NUM_KEYS],
            saved_tunings: Vec::new(),
            current_tuning_name: "Unnamed Tuning".into(),
            status_message: String::new(),
            error_message: String::new(),
        }
    }
}

impl AppState {
    /// The temperament currently selected by the user.
    pub fn temperament(&self) -> &Temperament {
        self.temperaments
            .get(self.temperament_index)
            .unwrap_or(&self.temperaments[0])
    }

    /// The key the tuner is currently working on (locked key wins).
    pub fn active_key(&self) -> Option<usize> {
        self.locked_key_index.or(self.detected_key_index)
    }

    /// Recompute the inharmonicity curve and the stretched tuning curve.
    pub fn update_tuning_curve(&mut self) {
        let mut measured: Vec<(usize, f64)> = self
            .measured_b_values
            .iter()
            .map(|(&k, &b)| (k, b))
            .collect();
        measured.sort_by_key(|&(k, _)| k);
        if !measured.is_empty() {
            self.b_curve = interpolate_b_values(&measured);
        }
        self.tuning_curve = generate_tuning_curve(
            &self.b_curve,
            self.a4_hz,
            &self.interval_weights,
            self.stretch,
        );
    }

    /// Target frequency in Hz for a key, including stretch, temperament,
    /// calibration and (when enabled) pitch-raise overpull.
    pub fn target_freq(&self, key_index: usize) -> f64 {
        let curve = self.tuning_curve.get(key_index).copied().unwrap_or(0.0);
        let temperament = self.temperament().offset_for_key(key_index);
        let overpull = if self.pitch_raise_mode {
            compute_overpull(&self.current_deviation, self.overpull_factor)
                .get(key_index)
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let total = curve + temperament + self.calibration_cents - overpull;
        key_to_freq(key_index, self.a4_hz) * tuner_core::cents::cents_to_ratio(total)
    }

    /// Feed a new detection result in, updating the detected key and deviation.
    pub fn apply_detection(
        &mut self,
        fundamental_hz: f64,
        partials: Vec<(u32, f64)>,
        spectrum: Vec<f32>,
        bin_hz: f64,
    ) {
        self.detected_fundamental_hz = Some(fundamental_hz);
        self.detected_partials = partials;
        self.magnitude_spectrum = spectrum;
        self.spectrum_bin_hz = bin_hz;

        if self.locked_key_index.is_none() {
            self.detected_key_index = nearest_key(fundamental_hz, self.a4_hz);
        }

        if let Some(key) = self.active_key() {
            let deviation = freq_to_cents(fundamental_hz, self.target_freq(key));
            self.detected_cents_deviation = deviation;
            if self.pitch_raise_mode {
                if let Some(slot) = self.current_deviation.get_mut(key) {
                    *slot = freq_to_cents(
                        fundamental_hz,
                        key_to_freq(key, self.a4_hz)
                            * tuner_core::cents::cents_to_ratio(
                                self.tuning_curve.get(key).copied().unwrap_or(0.0),
                            ),
                    );
                }
            }
        }
    }

    /// Store the inharmonicity coefficient measured from the current partials.
    pub fn store_current_measurement(&mut self) -> Result<(usize, f64), String> {
        let key = self
            .active_key()
            .ok_or_else(|| "No note detected yet".to_string())?;
        if self.detected_partials.len() < 2 {
            return Err("Need at least 2 partials; play the note louder".into());
        }
        let (_, b) = fit_inharmonicity(&self.detected_partials)?;
        self.measured_b_values.insert(key, b);
        self.update_tuning_curve();
        Ok((key, b))
    }

    /// Forget a stored inharmonicity measurement.
    pub fn clear_measurement(&mut self, key_index: usize) {
        self.measured_b_values.remove(&key_index);
        if self.measured_b_values.is_empty() {
            self.b_curve = vec![0.0002; NUM_KEYS];
        }
        self.update_tuning_curve();
    }

    /// Build a [`TuningFile`] describing the current session.
    pub fn export_tuning_file(&self) -> TuningFile {
        let mut f = TuningFile::new(self.current_tuning_name.clone());
        f.a4_reference_hz = self.a4_hz;
        f.calibration_cents = self.calibration_cents;
        f.stretch = self.stretch;
        f.interval_weights = self.interval_weights.clone();
        f.tuning_curve_cents = self.tuning_curve.clone();
        f.temperament = TemperamentData::from(self.temperament());
        for (&k, &b) in &self.measured_b_values {
            f.inharmonicity.insert(key_name(k), b);
        }
        f
    }

    /// Apply a loaded [`TuningFile`] to the current session.
    pub fn import_tuning_file(&mut self, file: TuningFile) {
        self.a4_hz = file.a4_reference_hz;
        self.calibration_cents = file.calibration_cents;
        self.stretch = file.stretch;
        self.interval_weights = file.interval_weights.clone();
        self.current_tuning_name = file.name.clone();
        self.tuning_curve = file.tuning_curve_cents.clone();

        self.measured_b_values = file.inharmonicity_by_key().into_iter().collect();
        if !self.measured_b_values.is_empty() {
            let mut measured: Vec<(usize, f64)> = file.inharmonicity_by_key();
            measured.sort_by_key(|&(k, _)| k);
            self.b_curve = interpolate_b_values(&measured);
        }

        match self
            .temperaments
            .iter()
            .position(|t| t.name == file.temperament.name)
        {
            Some(idx) => self.temperament_index = idx,
            None => {
                // Keep custom temperaments from imported files available.
                self.temperaments.push(file.temperament());
                self.temperament_index = self.temperaments.len() - 1;
            }
        }
    }

    /// Persist the current tuning to the library directory.
    pub fn save_current_tuning(&mut self) {
        let file = self.export_tuning_file();
        match storage::save(&file) {
            Ok(path) => {
                self.status_message = format!("Saved to {}", path.display());
                self.error_message.clear();
                self.refresh_library();
            }
            Err(e) => {
                self.error_message = e;
            }
        }
    }

    /// Reload the list of tunings from disk.
    pub fn refresh_library(&mut self) {
        let (files, errors) = storage::load_all();
        self.saved_tunings = files;
        self.error_message = errors.join("; ");
    }

    /// Delete a tuning from disk and from the in-memory list.
    pub fn delete_tuning(&mut self, index: usize) {
        let Some(file) = self.saved_tunings.get(index) else {
            return;
        };
        let name = file.name.clone();
        match storage::delete(&name) {
            Ok(()) => {
                self.saved_tunings.remove(index);
                self.status_message = format!("Deleted {name}");
                self.error_message.clear();
            }
            Err(e) => self.error_message = e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_consistent() {
        let s = AppState::default();
        assert_eq!(s.tuning_curve.len(), NUM_KEYS);
        assert_eq!(s.b_curve.len(), NUM_KEYS);
        assert_eq!(s.current_deviation.len(), NUM_KEYS);
        assert_eq!(s.temperament().name, "Equal");
        assert_eq!(s.current_screen, Screen::Tuner);
    }

    #[test]
    fn test_target_freq_defaults_to_equal_temperament() {
        let s = AppState::default();
        assert!((s.target_freq(48) - 440.0).abs() < 1e-9);
        assert!((s.target_freq(0) - 27.5).abs() < 1e-6);
    }

    #[test]
    fn test_target_freq_applies_temperament_and_calibration() {
        let s = AppState {
            calibration_cents: 1200.0,
            ..Default::default()
        };
        assert!((s.target_freq(48) - 880.0).abs() < 1e-6);

        let mut s = AppState::default();
        s.temperament_index = s
            .temperaments
            .iter()
            .position(|t| t.name == "Werckmeister III")
            .unwrap();
        // C is unaltered in Werckmeister III, C# is not.
        assert!((s.target_freq(3) - key_to_freq(3, 440.0)).abs() < 1e-9);
        assert!(s.target_freq(4) < key_to_freq(4, 440.0));
    }

    #[test]
    fn test_apply_detection_selects_nearest_key() {
        let mut s = AppState::default();
        s.apply_detection(441.0, vec![(1, 441.0), (2, 882.5)], vec![0.0; 16], 5.0);
        assert_eq!(s.detected_key_index, Some(48));
        assert!(s.detected_cents_deviation > 0.0);
        assert!(s.detected_cents_deviation < 10.0);
        assert_eq!(s.spectrum_bin_hz, 5.0);
    }

    #[test]
    fn test_locked_key_overrides_detection() {
        let mut s = AppState {
            locked_key_index: Some(0),
            ..Default::default()
        };
        s.apply_detection(441.0, vec![(1, 441.0)], Vec::new(), 5.0);
        assert_eq!(s.active_key(), Some(0));
        // 441 Hz measured against A0 is about 4 octaves sharp.
        assert!(s.detected_cents_deviation > 4000.0);
    }

    #[test]
    fn test_store_measurement_requires_partials() {
        let mut s = AppState::default();
        assert!(s.store_current_measurement().is_err());

        s.apply_detection(440.0, vec![(1, 440.0)], Vec::new(), 1.0);
        assert!(s.store_current_measurement().is_err());

        let partials: Vec<(u32, f64)> = (1..=6)
            .map(|n| (n, tuner_core::inharmonicity::partial_freq(440.0, 0.0001, n)))
            .collect();
        s.apply_detection(440.0, partials, Vec::new(), 1.0);
        let (key, b) = s.store_current_measurement().unwrap();
        assert_eq!(key, 48);
        assert!((b - 0.0001).abs() < 0.00002);
        assert_eq!(s.tuning_curve.len(), NUM_KEYS);
        assert!(s.tuning_curve[48].abs() < 1e-9);

        s.clear_measurement(48);
        assert!(s.measured_b_values.is_empty());
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut s = AppState {
            current_tuning_name: "My Piano".into(),
            a4_hz: 442.0,
            stretch: 1.5,
            calibration_cents: -3.0,
            ..Default::default()
        };
        s.measured_b_values.insert(48, 0.000_02);
        s.update_tuning_curve();
        let file = s.export_tuning_file();
        file.validate().unwrap();

        let mut other = AppState::default();
        other.import_tuning_file(file);
        assert_eq!(other.current_tuning_name, "My Piano");
        assert!((other.a4_hz - 442.0).abs() < 1e-9);
        assert!((other.stretch - 1.5).abs() < 1e-9);
        assert_eq!(other.tuning_curve, s.tuning_curve);
        assert_eq!(other.measured_b_values.get(&48), Some(&0.000_02));
    }

    #[test]
    fn test_import_unknown_temperament_is_kept() {
        let mut s = AppState::default();
        let mut file = TuningFile::new("Custom");
        file.temperament = TemperamentData {
            name: "Historic Weirdness".into(),
            offsets_cents: [1.0; 12],
        };
        let before = s.temperaments.len();
        s.import_tuning_file(file);
        assert_eq!(s.temperaments.len(), before + 1);
        assert_eq!(s.temperament().name, "Historic Weirdness");
    }

    #[test]
    fn test_pitch_raise_overpull_shifts_target() {
        let mut s = AppState::default();
        let nominal = s.target_freq(0);
        s.pitch_raise_mode = true;
        s.current_deviation[0] = -30.0; // 30 cents flat
        let raised = s.target_freq(0);
        assert!(
            raised > nominal,
            "overpull should aim above the final pitch"
        );
    }
}
