use crate::YieldCurveError;

/// Validates, sorts, and deduplicates `(x, y)` points.
///
/// - Rejects NaN / infinite values.
/// - Rejects negative x (time cannot be negative).
/// - Rejects duplicate x (ambiguous for interpolation).
pub(crate) fn validate_and_sort(
    points: &[(f64, f64)],
    method: &'static str,
    need: usize,
) -> Result<Vec<(f64, f64)>, YieldCurveError> {
    if points.len() < need {
        return Err(YieldCurveError::InsufficientData {
            method,
            need,
            got: points.len(),
        });
    }
    for (x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            return Err(YieldCurveError::InvalidPoint(format!(
                "non-finite value (x={x}, y={y})"
            )));
        }
        if *x < 0.0 {
            return Err(YieldCurveError::InvalidPoint(format!(
                "negative t_years: {x}"
            )));
        }
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for w in sorted.windows(2) {
        if (w[1].0 - w[0].0).abs() < f64::EPSILON {
            return Err(YieldCurveError::InvalidPoint(format!(
                "duplicate t_years x={}",
                w[0].0
            )));
        }
    }
    Ok(sorted)
}
