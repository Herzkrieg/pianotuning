//! Historical and modern temperaments.

use serde::{Deserialize, Serialize};

/// A temperament is defined by 12 cent offsets from equal temperament,
/// indexed chromatically from C (0) to B (11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Temperament {
    pub name: String,
    pub offsets_cents: [f64; 12],
}

impl Default for Temperament {
    fn default() -> Self {
        Self::equal()
    }
}

impl Temperament {
    /// Build a user defined temperament from chromatic (C-based) offsets.
    pub fn custom(name: impl Into<String>, offsets_cents: [f64; 12]) -> Self {
        Self {
            name: name.into(),
            offsets_cents,
        }
    }

    pub fn equal() -> Self {
        Self {
            name: "Equal".into(),
            offsets_cents: [0.0; 12],
        }
    }

    pub fn werckmeister_iii() -> Self {
        Self {
            name: "Werckmeister III".into(),
            offsets_cents: [
                0.0, -9.78, -3.91, -5.87, -7.82, -1.96, -11.73, -1.96, -7.82, -3.91, -9.78, -5.87,
            ],
        }
    }

    pub fn kirnberger_iii() -> Self {
        Self {
            name: "Kirnberger III".into(),
            offsets_cents: [
                0.0, -10.27, -3.91, -6.18, -3.91, -1.96, -11.73, -1.96, -8.14, -3.91, -10.27, -5.87,
            ],
        }
    }

    pub fn vallotti() -> Self {
        Self {
            name: "Vallotti".into(),
            offsets_cents: [
                0.0, -5.87, -3.91, -3.91, -7.82, -1.96, -9.78, -1.96, -5.87, -3.91, -7.82, -3.91,
            ],
        }
    }

    pub fn young() -> Self {
        Self {
            name: "Young".into(),
            offsets_cents: [
                0.0, -6.08, -4.08, -4.08, -8.16, -2.04, -10.16, -2.04, -6.08, -4.08, -8.16, -4.08,
            ],
        }
    }

    pub fn meantone_quarter_comma() -> Self {
        Self {
            name: "Meantone (1/4 comma)".into(),
            offsets_cents: [
                0.0, -24.0, -6.84, -13.69, -13.69, -3.42, -27.37, -3.42, -20.52, -10.27, -20.52,
                -17.1,
            ],
        }
    }

    pub fn pythagorean() -> Self {
        Self {
            name: "Pythagorean".into(),
            offsets_cents: [
                0.0, 11.73, 3.91, 7.82, 7.82, -1.96, 9.78, -1.96, 5.87, 5.87, 9.78, 3.91,
            ],
        }
    }

    /// Every built-in temperament, in menu order.
    pub fn all_presets() -> Vec<Temperament> {
        vec![
            Self::equal(),
            Self::werckmeister_iii(),
            Self::kirnberger_iii(),
            Self::vallotti(),
            Self::young(),
            Self::meantone_quarter_comma(),
            Self::pythagorean(),
        ]
    }

    /// Look up a preset by name (case-insensitive).
    pub fn preset_by_name(name: &str) -> Option<Temperament> {
        Self::all_presets()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Cent offset for a given piano key index (`0 = A0` .. `87 = C8`).
    pub fn offset_for_key(&self, key_index: usize) -> f64 {
        // Key indices count from A, temperament offsets count from C.
        let c_index = (key_index % 12 + 9) % 12;
        self.offsets_cents[c_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_equal_all_zeros() {
        let t = Temperament::equal();
        for k in 0..88 {
            assert_abs_diff_eq!(t.offset_for_key(k), 0.0, epsilon = 1e-9);
        }
    }

    #[test]
    fn test_preset_count() {
        assert_eq!(Temperament::all_presets().len(), 7);
    }

    #[test]
    fn test_preset_names_unique() {
        let presets = Temperament::all_presets();
        for (i, a) in presets.iter().enumerate() {
            for b in presets.iter().skip(i + 1) {
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn test_werckmeister_c_is_zero() {
        let t = Temperament::werckmeister_iii();
        // C1 = key 3, C-based index 0 -> offset 0.0
        assert_abs_diff_eq!(t.offset_for_key(3), 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_offset_mapping_is_octave_invariant() {
        let t = Temperament::vallotti();
        for k in 0..76 {
            assert_abs_diff_eq!(
                t.offset_for_key(k),
                t.offset_for_key(k + 12),
                epsilon = 1e-12
            );
        }
    }

    #[test]
    fn test_offset_for_key_a_maps_to_c_index_nine() {
        let t = Temperament::custom("Probe", [0., 1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.]);
        assert_abs_diff_eq!(t.offset_for_key(0), 9.0, epsilon = 1e-12); // A0
        assert_abs_diff_eq!(t.offset_for_key(3), 0.0, epsilon = 1e-12); // C1
        assert_abs_diff_eq!(t.offset_for_key(11), 8.0, epsilon = 1e-12); // G#1
    }

    #[test]
    fn test_preset_by_name() {
        assert!(Temperament::preset_by_name("vallotti").is_some());
        assert!(Temperament::preset_by_name("Nope").is_none());
    }

    #[test]
    fn test_serde_roundtrip() {
        let t = Temperament::young();
        let json = serde_json::to_string(&t).unwrap();
        let back: Temperament = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
