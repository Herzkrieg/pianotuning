//! Persistent `.ptun` tuning file format (JSON).

use crate::cents::{key_index_from_name, key_name};
use crate::temperament::Temperament;
use crate::tuning_curve::IntervalWeights;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Version of the on-disk format written by this build.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// Conventional file extension for tuning files.
pub const FILE_EXTENSION: &str = "ptun";

/// A complete, self-contained tuning for one instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuningFile {
    pub format_version: u32,
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub notes: String,
    pub a4_reference_hz: f64,
    pub temperament: TemperamentData,
    pub calibration_cents: f64,
    /// Measured inharmonicity coefficients keyed by note name (e.g. `"C4"`).
    pub inharmonicity: HashMap<String, f64>,
    /// 88 cent offsets from equal temperament, A0..C8.
    pub tuning_curve_cents: Vec<f64>,
    pub interval_weights: IntervalWeights,
    pub stretch: f64,
}

/// Serialized form of a [`Temperament`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemperamentData {
    pub name: String,
    pub offsets_cents: [f64; 12],
}

impl From<&Temperament> for TemperamentData {
    fn from(t: &Temperament) -> Self {
        Self {
            name: t.name.clone(),
            offsets_cents: t.offsets_cents,
        }
    }
}

impl From<&TemperamentData> for Temperament {
    fn from(t: &TemperamentData) -> Self {
        Temperament {
            name: t.name.clone(),
            offsets_cents: t.offsets_cents,
        }
    }
}

impl TuningFile {
    /// Create an empty tuning with sensible defaults, stamped with the current time.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            name: name.into(),
            created_at: now_iso8601(),
            notes: String::new(),
            a4_reference_hz: 440.0,
            temperament: TemperamentData::from(&Temperament::equal()),
            calibration_cents: 0.0,
            inharmonicity: HashMap::new(),
            tuning_curve_cents: vec![0.0; crate::NUM_KEYS],
            interval_weights: IntervalWeights::default(),
            stretch: 1.0,
        }
    }

    /// Record a measured inharmonicity coefficient for a key index.
    pub fn set_inharmonicity(&mut self, key_index: usize, b: f64) {
        self.inharmonicity.insert(key_name(key_index), b);
    }

    /// Measured inharmonicity coefficients as `(key_index, B)` pairs, sorted by key.
    /// Entries with unparseable note names are skipped.
    pub fn inharmonicity_by_key(&self) -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = self
            .inharmonicity
            .iter()
            .filter_map(|(name, &b)| key_index_from_name(name).map(|k| (k, b)))
            .collect();
        out.sort_by_key(|&(k, _)| k);
        out
    }

    /// The temperament stored in this file.
    pub fn temperament(&self) -> Temperament {
        Temperament::from(&self.temperament)
    }

    /// Serialize to a pretty JSON string.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("JSON encode error: {e}"))
    }

    /// Deserialize from a JSON string, validating the result.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let file: TuningFile =
            serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;
        file.validate()?;
        Ok(file)
    }

    /// Write the tuning to `path` as JSON.
    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), String> {
        self.validate()?;
        let json = self.to_json()?;
        std::fs::write(path.as_ref(), json)
            .map_err(|e| format!("Failed to write {}: {e}", path.as_ref().display()))
    }

    /// Read and validate a tuning file from `path`.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read {}: {e}", path.as_ref().display()))?;
        Self::from_json(&text)
    }

    /// Validate file contents.
    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != CURRENT_FORMAT_VERSION {
            return Err(format!(
                "Unsupported format version {}; expected {}",
                self.format_version, CURRENT_FORMAT_VERSION
            ));
        }
        if self.name.trim().is_empty() {
            return Err("Tuning file name must not be empty".into());
        }
        if !self.a4_reference_hz.is_finite()
            || self.a4_reference_hz < 380.0
            || self.a4_reference_hz > 500.0
        {
            return Err(format!(
                "a4_reference_hz {} out of reasonable range [380, 500]",
                self.a4_reference_hz
            ));
        }
        if !self.calibration_cents.is_finite() || self.calibration_cents.abs() > 100.0 {
            return Err(format!(
                "calibration_cents {} out of reasonable range [-100, 100]",
                self.calibration_cents
            ));
        }
        if !self.stretch.is_finite() || !(0.0..=10.0).contains(&self.stretch) {
            return Err(format!("stretch {} out of range [0, 10]", self.stretch));
        }
        if self.tuning_curve_cents.len() != crate::NUM_KEYS {
            return Err(format!(
                "tuning_curve_cents must have 88 entries, got {}",
                self.tuning_curve_cents.len()
            ));
        }
        if let Some(bad) = self.tuning_curve_cents.iter().find(|v| !v.is_finite()) {
            return Err(format!(
                "tuning_curve_cents contains a non-finite value: {bad}"
            ));
        }
        if self
            .temperament
            .offsets_cents
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 100.0)
        {
            return Err("temperament offsets must be finite and within +/-100 cents".into());
        }
        for (name, b) in &self.inharmonicity {
            if key_index_from_name(name).is_none() {
                return Err(format!("Unknown note name in inharmonicity map: {name}"));
            }
            if !b.is_finite() || *b < 0.0 {
                return Err(format!("Invalid inharmonicity value for {name}: {b}"));
            }
        }
        Ok(())
    }
}

/// Current UTC time formatted as an ISO-8601 / RFC-3339 timestamp.
///
/// Implemented locally to avoid pulling a date-time dependency into the core crate.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_iso8601(secs)
}

/// Format seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
fn format_iso8601(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let mut f = TuningFile::new("Test Piano");
        f.a4_reference_hz = 442.0;
        f.tuning_curve_cents = (0..88).map(|i| i as f64 * 0.1 - 4.0).collect();
        f.set_inharmonicity(48, 0.000_02);

        let json = f.to_json().unwrap();
        let loaded = TuningFile::from_json(&json).unwrap();

        assert_eq!(loaded.name, "Test Piano");
        assert!((loaded.a4_reference_hz - 442.0).abs() < 1e-9);
        assert_eq!(loaded.tuning_curve_cents.len(), 88);
        assert_eq!(loaded.inharmonicity_by_key(), vec![(48, 0.000_02)]);
        for (a, b) in loaded
            .tuning_curve_cents
            .iter()
            .zip(f.tuning_curve_cents.iter())
        {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn test_wrong_version_rejected() {
        let mut f = TuningFile::new("Test");
        f.format_version = 99;
        let json = serde_json::to_string(&f).unwrap();
        let result = TuningFile::from_json(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported format version"));
    }

    #[test]
    fn test_wrong_curve_length_rejected() {
        let mut f = TuningFile::new("Test");
        f.tuning_curve_cents = vec![0.0; 10];
        let json = serde_json::to_string(&f).unwrap();
        assert!(TuningFile::from_json(&json).is_err());
    }

    #[test]
    fn test_malformed_json_rejected() {
        let result = TuningFile::from_json("{not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JSON parse error"));
    }

    #[test]
    fn test_empty_name_rejected() {
        let mut f = TuningFile::new("Test");
        f.name = "   ".into();
        let json = serde_json::to_string(&f).unwrap();
        assert!(TuningFile::from_json(&json).is_err());
    }

    #[test]
    fn test_out_of_range_values_rejected() {
        let mut f = TuningFile::new("Test");
        f.a4_reference_hz = 1000.0;
        assert!(f.validate().is_err());

        let mut f = TuningFile::new("Test");
        f.stretch = -1.0;
        assert!(f.validate().is_err());

        let mut f = TuningFile::new("Test");
        f.calibration_cents = 500.0;
        assert!(f.validate().is_err());

        let mut f = TuningFile::new("Test");
        f.tuning_curve_cents[3] = f64::NAN;
        assert!(f.validate().is_err());
    }

    #[test]
    fn test_bad_inharmonicity_entries_rejected() {
        let mut f = TuningFile::new("Test");
        f.inharmonicity.insert("H9".into(), 0.0001);
        assert!(f.validate().is_err());

        let mut f = TuningFile::new("Test");
        f.inharmonicity.insert("C4".into(), -1.0);
        assert!(f.validate().is_err());
    }

    #[test]
    fn test_missing_notes_field_defaults() {
        let mut f = TuningFile::new("Test");
        f.notes = "keep".into();
        let mut value: serde_json::Value = serde_json::from_str(&f.to_json().unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("notes");
        let loaded = TuningFile::from_json(&value.to_string()).unwrap();
        assert_eq!(loaded.notes, "");
    }

    #[test]
    fn test_temperament_conversion_roundtrip() {
        let t = Temperament::vallotti();
        let data = TemperamentData::from(&t);
        assert_eq!(Temperament::from(&data), t);
    }

    #[test]
    fn test_file_io_roundtrip() {
        let dir = std::env::current_dir().unwrap().join("target");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_tuning_io.ptun");
        let f = TuningFile::new("IO Test");
        f.save_to_path(&path).unwrap();
        let loaded = TuningFile::load_from_path(&path).unwrap();
        assert_eq!(loaded.name, "IO Test");
        let _ = std::fs::remove_file(&path);

        assert!(TuningFile::load_from_path(dir.join("does_not_exist.ptun")).is_err());
    }

    #[test]
    fn test_timestamp_format() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
        let now = now_iso8601();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z'));
    }
}
