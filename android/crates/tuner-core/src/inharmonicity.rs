//! String inharmonicity modelling.
//!
//! Real piano strings are stiff, so their partials are not exact integer
//! multiples of the fundamental. The standard model is
//!
//! ```text
//! f_n = n * f1 * sqrt(1 + B * n^2)
//! ```
//!
//! where `B` is the inharmonicity coefficient of the string.

/// Maximum number of Gauss-Newton refinement iterations used by [`fit_inharmonicity`].
const MAX_REFINE_ITERS: usize = 50;

/// Frequency of a given partial for a string with fundamental `f1` and coefficient `b`.
pub fn partial_freq(f1: f64, b: f64, partial: u32) -> f64 {
    let n = partial as f64;
    n * f1 * (1.0 + b * n * n).sqrt()
}

/// Fit the inharmonicity model to a set of measured partials.
///
/// `partials` holds `(partial_number, frequency_hz)` pairs with 1-based partial
/// numbers. Returns `(f1_hz, B)` or a human readable error.
///
/// A log-linearised least-squares fit provides the initial guess which is then
/// refined with Gauss-Newton iterations on the exact model.
pub fn fit_inharmonicity(partials: &[(u32, f64)]) -> Result<(f64, f64), String> {
    if partials.len() < 2 {
        return Err("Need at least 2 partials to fit inharmonicity".into());
    }

    // Linearise: ln(f_n / n) = ln(f1) + 0.5 * ln(1 + B*n^2) ~= ln(f1) + 0.5*B*n^2
    // so with y = ln(f_n/n) and x = n^2/2 we get y = a + B*x.
    let n = partials.len() as f64;
    let mut sx = 0.0_f64;
    let mut sy = 0.0_f64;
    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;

    for &(partial_num, freq) in partials {
        if freq <= 0.0 || !freq.is_finite() || partial_num == 0 {
            return Err("Invalid partial data".into());
        }
        let x = (partial_num as f64).powi(2) / 2.0;
        let y = (freq / partial_num as f64).ln();
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
    }

    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-15 {
        return Err("Degenerate least-squares system".into());
    }

    let mut b = ((n * sxy - sx * sy) / denom).max(0.0);
    let mut a = (sy - b * sx) / n;

    // Gauss-Newton refinement of the exact model:
    // r_n = ln(f_n/n) - a - 0.5*ln(1 + B*n^2)
    for _ in 0..MAX_REFINE_ITERS {
        let (mut jaa, mut jab, mut jbb) = (0.0_f64, 0.0_f64, 0.0_f64);
        let (mut ga, mut gb) = (0.0_f64, 0.0_f64);
        for &(partial_num, freq) in partials {
            let nn = (partial_num as f64).powi(2);
            let denom_n = 1.0 + b * nn;
            if denom_n <= 0.0 {
                break;
            }
            let r = (freq / partial_num as f64).ln() - a - 0.5 * denom_n.ln();
            let da = 1.0_f64; // -d r / d a
            let db = 0.5 * nn / denom_n; // -d r / d B
            jaa += da * da;
            jab += da * db;
            jbb += db * db;
            ga += da * r;
            gb += db * r;
        }
        let det = jaa * jbb - jab * jab;
        if det.abs() < 1e-18 {
            break;
        }
        let delta_a = (jbb * ga - jab * gb) / det;
        let delta_b = (jaa * gb - jab * ga) / det;
        a += delta_a;
        b = (b + delta_b).max(0.0);
        if delta_a.abs() < 1e-12 && delta_b.abs() < 1e-14 {
            break;
        }
    }

    let f1 = a.exp();
    if !f1.is_finite() || !b.is_finite() {
        return Err("Inharmonicity fit did not converge".into());
    }

    Ok((f1, b.max(0.0)))
}

/// Interpolate/extrapolate `B` values across all 88 keys using log-linear regression.
///
/// `measured` is a list of `(key_index 0..=87, B_value)` pairs. Returns a `Vec`
/// of 88 `B` values. Missing or degenerate input yields a flat curve instead of
/// an error so the UI always has something usable.
pub fn interpolate_b_values(measured: &[(usize, f64)]) -> Vec<f64> {
    let mut result = vec![0.0_f64; crate::NUM_KEYS];
    if measured.is_empty() {
        return result;
    }
    if measured.len() == 1 {
        let b = measured[0].1;
        return vec![b.max(0.0); crate::NUM_KEYS];
    }

    // Fit log(B) = intercept + slope*key_index using least squares (skip B <= 0).
    let valid: Vec<(f64, f64)> = measured
        .iter()
        .filter(|&&(_, b)| b > 0.0 && b.is_finite())
        .map(|&(k, b)| (k as f64, b.ln()))
        .collect();

    if valid.is_empty() {
        return result;
    }
    if valid.len() == 1 {
        return vec![valid[0].1.exp(); crate::NUM_KEYS];
    }

    let n = valid.len() as f64;
    let sx: f64 = valid.iter().map(|(x, _)| x).sum();
    let sy: f64 = valid.iter().map(|(_, y)| y).sum();
    let sxx: f64 = valid.iter().map(|(x, _)| x * x).sum();
    let sxy: f64 = valid.iter().map(|(x, y)| x * y).sum();

    let denom = n * sxx - sx * sx;
    let (slope, intercept) = if denom.abs() < 1e-15 {
        (0.0, sy / n)
    } else {
        let s = (n * sxy - sx * sy) / denom;
        let i = (sy - s * sx) / n;
        (s, i)
    };

    for (k, val) in result.iter_mut().enumerate() {
        *val = (intercept + slope * k as f64).exp().max(0.0);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn synthetic_partials(f1: f64, b: f64, count: u32) -> Vec<(u32, f64)> {
        (1..=count).map(|n| (n, partial_freq(f1, b, n))).collect()
    }

    #[test]
    fn test_fit_known_b() {
        let f1 = 27.5;
        let b = 0.0004;
        let partials = synthetic_partials(f1, b, 8);
        let (f1_fit, b_fit) = fit_inharmonicity(&partials).unwrap();
        assert_abs_diff_eq!(f1_fit, f1, epsilon = f1 * 0.01);
        assert_abs_diff_eq!(b_fit, b, epsilon = b * 0.05);
    }

    #[test]
    fn test_fit_small_b_treble() {
        let f1 = 1046.5;
        let b = 0.000_02;
        let partials = synthetic_partials(f1, b, 6);
        let (f1_fit, b_fit) = fit_inharmonicity(&partials).unwrap();
        assert_abs_diff_eq!(f1_fit, f1, epsilon = f1 * 0.01);
        assert_abs_diff_eq!(b_fit, b, epsilon = b * 0.20);
    }

    #[test]
    fn test_fit_requires_two_partials() {
        assert!(fit_inharmonicity(&[(1, 440.0)]).is_err());
        assert!(fit_inharmonicity(&[]).is_err());
    }

    #[test]
    fn test_fit_rejects_invalid_data() {
        assert!(fit_inharmonicity(&[(1, 440.0), (0, 880.0)]).is_err());
        assert!(fit_inharmonicity(&[(1, -440.0), (2, 880.0)]).is_err());
    }

    #[test]
    fn test_fit_never_returns_negative_b() {
        // Partials that are flatter than harmonic (impossible physically).
        let partials = vec![(1, 440.0), (2, 870.0), (3, 1300.0)];
        let (_, b) = fit_inharmonicity(&partials).unwrap();
        assert!(b >= 0.0);
    }

    #[test]
    fn test_interpolate_returns_88_values() {
        let measured = vec![(0, 0.0004), (87, 0.000001)];
        let result = interpolate_b_values(&measured);
        assert_eq!(result.len(), 88);
        assert!(result.iter().all(|&b| b >= 0.0));
    }

    #[test]
    fn test_interpolate_matches_measured_points() {
        let measured = vec![(12, 0.0002), (60, 0.000_01)];
        let result = interpolate_b_values(&measured);
        assert_abs_diff_eq!(result[12], 0.0002, epsilon = 1e-9);
        assert_abs_diff_eq!(result[60], 0.000_01, epsilon = 1e-9);
    }

    #[test]
    fn test_interpolate_empty_and_single() {
        assert_eq!(interpolate_b_values(&[]), vec![0.0; 88]);
        let single = interpolate_b_values(&[(40, 0.0003)]);
        assert_eq!(single.len(), 88);
        assert!(single.iter().all(|&b| (b - 0.0003).abs() < 1e-12));
    }
}
