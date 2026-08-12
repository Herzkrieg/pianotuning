//! Conversions between frequencies, cents and piano key indices.
//!
//! Key indices are 0-based from A0: `0 = A0`, `48 = A4`, `87 = C8`.

/// Note names in piano key order, starting at A.
const NOTE_NAMES: [&str; 12] = [
    "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
];

/// Convert measured frequency to cents deviation from target frequency.
///
/// Returns positive cents if measured > target (sharp), negative if flat.
/// Returns `0.0` for non-positive inputs rather than panicking, so that audio
/// glitches or an uninitialised target cannot take the app down.
pub fn freq_to_cents(measured_hz: f64, target_hz: f64) -> f64 {
    if target_hz <= 0.0 || measured_hz <= 0.0 || !measured_hz.is_finite() || !target_hz.is_finite()
    {
        return 0.0;
    }
    1200.0 * (measured_hz / target_hz).log2()
}

/// Convert a cents offset into a frequency ratio multiplier.
pub fn cents_to_ratio(cents: f64) -> f64 {
    2.0_f64.powf(cents / 1200.0)
}

/// Equal-tempered frequency of a piano key given the A4 reference frequency.
///
/// Key index 0 = A0, key index 48 = A4, key index 87 = C8.
pub fn key_to_freq(key_index: usize, a4_hz: f64) -> f64 {
    let semitones_from_a4 = key_index as f64 - crate::A4_KEY_INDEX as f64;
    a4_hz * 2.0_f64.powf(semitones_from_a4 / 12.0)
}

/// Key name (e.g. `"A0"`, `"C4"`, `"C#5"`) for a key index `0..=87`.
pub fn key_name(key_index: usize) -> String {
    let note = key_index % 12;
    // Piano keys start at A0, but scientific octave numbers roll over at C,
    // so A0/A#0/B0 stay in octave 0 while C1 starts the next octave.
    let octave = match note {
        0..=2 => key_index / 12,
        _ => key_index / 12 + 1,
    };
    format!("{}{}", NOTE_NAMES[note], octave)
}

/// Inverse of [`key_name`]: parse `"C#4"` / `"Bb3"` style names into a key index.
///
/// Returns `None` if the name cannot be parsed or is outside the 88-key range.
pub fn key_index_from_name(name: &str) -> Option<usize> {
    let name = name.trim();
    let mut chars = name.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let mut rest = chars.as_str();

    let mut semitone_from_a = match letter {
        'A' => 0i32,
        'B' => 2,
        'C' => 3,
        'D' => 5,
        'E' => 7,
        'F' => 8,
        'G' => 10,
        _ => return None,
    };

    if let Some(stripped) = rest.strip_prefix('#') {
        semitone_from_a += 1;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('b') {
        semitone_from_a -= 1;
        rest = stripped;
    }

    let octave: i32 = rest.parse().ok()?;
    // Octave numbering rolls over at C, so notes C..G# belong to the previous
    // key-index octave block.
    let block = if semitone_from_a >= 3 {
        octave - 1
    } else {
        octave
    };
    let index = block * 12 + semitone_from_a;
    if (0..crate::NUM_KEYS as i32).contains(&index) {
        Some(index as usize)
    } else {
        None
    }
}

/// Find the piano key whose equal-tempered frequency is closest to `freq_hz`.
pub fn nearest_key(freq_hz: f64, a4_hz: f64) -> Option<usize> {
    if freq_hz <= 0.0 || !freq_hz.is_finite() || a4_hz <= 0.0 {
        return None;
    }
    (0..crate::NUM_KEYS).min_by(|&a, &b| {
        let da = freq_to_cents(freq_hz, key_to_freq(a, a4_hz)).abs();
        let db = freq_to_cents(freq_hz, key_to_freq(b, a4_hz)).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_a4_to_a4_is_zero_cents() {
        assert_abs_diff_eq!(freq_to_cents(440.0, 440.0), 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_octave_is_1200_cents() {
        assert_abs_diff_eq!(freq_to_cents(880.0, 440.0), 1200.0, epsilon = 1e-6);
    }

    #[test]
    fn test_half_freq_is_minus_1200_cents() {
        assert_abs_diff_eq!(freq_to_cents(220.0, 440.0), -1200.0, epsilon = 1e-6);
    }

    #[test]
    fn test_non_positive_inputs_are_safe() {
        assert_abs_diff_eq!(freq_to_cents(0.0, 440.0), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(freq_to_cents(440.0, 0.0), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(freq_to_cents(-1.0, -1.0), 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_cents_to_ratio() {
        assert_abs_diff_eq!(cents_to_ratio(1200.0), 2.0, epsilon = 1e-9);
        assert_abs_diff_eq!(cents_to_ratio(0.0), 1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_key_to_freq_a4() {
        assert_abs_diff_eq!(key_to_freq(48, 440.0), 440.0, epsilon = 1e-9);
    }

    #[test]
    fn test_key_to_freq_a0() {
        // A0 is 4 octaves below A4
        assert_abs_diff_eq!(key_to_freq(0, 440.0), 27.5, epsilon = 1e-6);
    }

    #[test]
    fn test_key_to_freq_c8() {
        // C8 is key 87
        let expected = 440.0 * 2.0_f64.powf(39.0 / 12.0);
        assert_abs_diff_eq!(key_to_freq(87, 440.0), expected, epsilon = 1e-6);
    }

    #[test]
    fn test_key_name_a0() {
        assert_eq!(key_name(0), "A0");
    }

    #[test]
    fn test_key_name_c1() {
        assert_eq!(key_name(3), "C1");
    }

    #[test]
    fn test_key_name_a4() {
        assert_eq!(key_name(48), "A4");
    }

    #[test]
    fn test_key_name_c8() {
        assert_eq!(key_name(87), "C8");
    }

    #[test]
    fn test_key_name_roundtrip() {
        for k in 0..crate::NUM_KEYS {
            assert_eq!(key_index_from_name(&key_name(k)), Some(k), "key {}", k);
        }
    }

    #[test]
    fn test_key_index_from_name_flats_and_errors() {
        assert_eq!(key_index_from_name("Bb0"), key_index_from_name("A#0"));
        assert_eq!(key_index_from_name("H4"), None);
        assert_eq!(key_index_from_name("C9"), None);
        assert_eq!(key_index_from_name(""), None);
        assert_eq!(key_index_from_name("Cx"), None);
    }

    #[test]
    fn test_nearest_key() {
        assert_eq!(nearest_key(440.0, 440.0), Some(48));
        assert_eq!(nearest_key(27.5, 440.0), Some(0));
        assert_eq!(nearest_key(441.0, 440.0), Some(48));
        assert_eq!(nearest_key(0.0, 440.0), None);
    }
}
