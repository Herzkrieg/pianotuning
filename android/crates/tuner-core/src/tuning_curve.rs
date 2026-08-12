//! Stretched tuning curve generation and pitch-raise overpull.

use crate::cents::{cents_to_ratio, key_to_freq};
use crate::temperament::Temperament;

/// Interval weights used for stretch optimization.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IntervalWeights {
    pub octave: f64,
    pub fifth: f64,
    pub twelfth: f64,
    pub double_octave: f64,
}

impl Default for IntervalWeights {
    fn default() -> Self {
        Self {
            octave: 1.0,
            fifth: 0.5,
            twelfth: 0.8,
            double_octave: 0.6,
        }
    }
}

/// Largest deviation from equal temperament the curve generator will produce.
const MAX_OFFSET_CENTS: f64 = 50.0;

/// Generate a Railsback-style stretched tuning curve.
///
/// * `b_values` – 88 inharmonicity coefficients (A0..C8).
/// * `a4_hz` – A4 reference frequency (unused by the shape, kept for API symmetry).
/// * `weights` – interval weights.
/// * `stretch` – global stretch amount multiplier.
///
/// Returns 88 cent offsets which are added to the equal-tempered target of each key.
/// The curve is anchored so that A4 (key 48) always has a zero offset.
///
/// # Panics
/// Panics if `b_values` does not contain exactly 88 entries. Callers should use
/// [`crate::inharmonicity::interpolate_b_values`], which always returns 88 values.
pub fn generate_tuning_curve(
    b_values: &[f64],
    a4_hz: f64,
    weights: &IntervalWeights,
    stretch: f64,
) -> Vec<f64> {
    assert_eq!(
        b_values.len(),
        crate::NUM_KEYS,
        "b_values must have 88 entries"
    );
    let _ = a4_hz;
    let mut offsets = vec![0.0_f64; crate::NUM_KEYS];

    // Mean inharmonicity of the bass and treble sections. Stiffer strings need
    // more stretch, so these drive how far the extremes depart from equal
    // temperament.
    let bass_b: f64 = b_values[0..30].iter().sum::<f64>() / 30.0;
    let treble_b: f64 = b_values[58..88].iter().sum::<f64>() / 30.0;

    // Empirical scaling: bass goes flat, treble goes sharp.
    let bass_stretch = -stretch * (bass_b / 0.0002).ln().max(0.0) * 3.0;
    let treble_stretch = stretch * (treble_b / 0.000_001).ln().max(0.0) * 1.5;

    for (k, offset) in offsets.iter_mut().enumerate() {
        let t = k as f64 / (crate::NUM_KEYS - 1) as f64; // 0.0 (A0) to 1.0 (C8)

        // Flat in the bass, neutral in the middle, sharp in the treble.
        let shape = if t < 0.5 {
            bass_stretch * (1.0 - 2.0 * t)
        } else {
            treble_stretch * (2.0 * t - 1.0)
        };

        // Local correction so that octaves whose partials differ in stiffness
        // are widened proportionally to the weight the tuner gives octaves.
        let octave_contrib = if k >= 12 {
            weights.octave * (b_values[k] - b_values[k - 12]) * 600.0
        } else {
            0.0
        };

        *offset = (shape + octave_contrib * stretch).clamp(-MAX_OFFSET_CENTS, MAX_OFFSET_CENTS);
    }

    // Anchor A4 (key 48) to a zero cent offset.
    let anchor = offsets[crate::A4_KEY_INDEX];
    for v in offsets.iter_mut() {
        *v -= anchor;
    }

    offsets
}

/// Combine the equal-tempered scale, a temperament and a stretch curve into the
/// final target frequency for every key.
///
/// `curve_cents` must have 88 entries; extra or missing entries are treated as zero.
pub fn target_frequencies(
    curve_cents: &[f64],
    temperament: &Temperament,
    a4_hz: f64,
    calibration_cents: f64,
) -> Vec<f64> {
    (0..crate::NUM_KEYS)
        .map(|k| {
            let curve = curve_cents.get(k).copied().unwrap_or(0.0);
            let total = curve + temperament.offset_for_key(k) + calibration_cents;
            key_to_freq(k, a4_hz) * cents_to_ratio(total)
        })
        .collect()
}

/// Compute overpull offsets for a pitch raise.
///
/// * `current_cents` – current measured cents deviation per key (negative = flat).
/// * `overpull_factor` – fraction of the deviation to add as overpull (e.g. `0.3`).
///
/// Returns the extra cents to aim for beyond the nominal target. Bass and
/// extreme treble sections get a larger factor because they settle more.
pub fn compute_overpull(current_cents: &[f64], overpull_factor: f64) -> Vec<f64> {
    current_cents
        .iter()
        .enumerate()
        .map(|(k, &dev)| {
            let t = k as f64 / (crate::NUM_KEYS - 1) as f64;
            let region_factor = if !(0.3..=0.85).contains(&t) { 1.4 } else { 1.0 };
            dev * overpull_factor * region_factor
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_b() -> Vec<f64> {
        vec![0.0002; 88]
    }

    #[test]
    fn test_tuning_curve_length() {
        let curve = generate_tuning_curve(&flat_b(), 440.0, &IntervalWeights::default(), 1.0);
        assert_eq!(curve.len(), 88);
    }

    #[test]
    fn test_a4_anchored_to_zero() {
        let curve = generate_tuning_curve(&flat_b(), 440.0, &IntervalWeights::default(), 1.0);
        assert!(
            curve[48].abs() < 1e-9,
            "A4 (key 48) should be 0 cents offset"
        );
    }

    #[test]
    fn test_treble_sharp_bass_flat_tendency() {
        let mut b = vec![0.0002; 88];
        // High B in bass, low B in treble.
        for v in b.iter_mut().take(30) {
            *v = 0.001;
        }
        for v in b.iter_mut().skip(58) {
            *v = 0.000_01;
        }
        let curve = generate_tuning_curve(&b, 440.0, &IntervalWeights::default(), 1.0);
        let bass_mean: f64 = curve[0..10].iter().sum::<f64>() / 10.0;
        let treble_mean: f64 = curve[78..88].iter().sum::<f64>() / 10.0;
        assert!(
            bass_mean <= treble_mean,
            "Bass should be flatter than treble"
        );
    }

    #[test]
    fn test_offsets_are_bounded() {
        let mut b = vec![0.05; 88];
        b[0] = 0.9;
        let curve = generate_tuning_curve(&b, 440.0, &IntervalWeights::default(), 5.0);
        assert!(curve.iter().all(|v| v.is_finite()));
        assert!(curve.iter().all(|v| v.abs() <= 2.0 * 50.0 + 1e-9));
    }

    #[test]
    fn test_stretch_zero_gives_zero_offsets() {
        let curve = generate_tuning_curve(&flat_b(), 440.0, &IntervalWeights::default(), 0.0);
        for v in &curve {
            assert!(v.abs() < 1e-9, "Zero stretch should give zero offsets");
        }
    }

    #[test]
    fn test_target_frequencies_equal_temperament() {
        let curve = vec![0.0; 88];
        let freqs = target_frequencies(&curve, &Temperament::equal(), 440.0, 0.0);
        assert_eq!(freqs.len(), 88);
        assert!((freqs[48] - 440.0).abs() < 1e-9);
        assert!((freqs[0] - 27.5).abs() < 1e-6);
    }

    #[test]
    fn test_target_frequencies_apply_calibration() {
        let curve = vec![0.0; 88];
        let freqs = target_frequencies(&curve, &Temperament::equal(), 440.0, 1200.0);
        assert!((freqs[48] - 880.0).abs() < 1e-6);
    }

    #[test]
    fn test_overpull_signs_and_regions() {
        let dev = vec![-10.0; 88];
        let pull = compute_overpull(&dev, 0.3);
        assert_eq!(pull.len(), 88);
        // Flat notes need extra flat-side overpull (same sign as the deviation).
        assert!(pull.iter().all(|&p| p < 0.0));
        // Bass gets a bigger factor than the midrange.
        assert!(pull[0].abs() > pull[44].abs());
        assert!(pull[87].abs() > pull[44].abs());
    }

    #[test]
    fn test_overpull_zero_factor() {
        let pull = compute_overpull(&[-10.0; 88], 0.0);
        assert!(pull.iter().all(|&p| p.abs() < 1e-12));
    }
}
