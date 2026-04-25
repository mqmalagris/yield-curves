use crate::YieldCurveError;

/// Nelder-Mead simplex — derivative-free minimization.
///
/// Classical parameters per Lagarias et al. (1998):
/// α = 1.0 (reflection), γ = 2.0 (expansion), ρ = 0.5 (contraction),
/// σ = 0.5 (shrink). Stops when the cost spread falls below `tol` or the
/// iteration budget `max_iter` is exhausted.
pub(crate) fn nelder_mead<F>(
    f: F,
    initial: Vec<f64>,
    step: Vec<f64>,
    max_iter: usize,
    tol: f64,
) -> Result<(Vec<f64>, f64), YieldCurveError>
where
    F: Fn(&[f64]) -> f64,
{
    let n = initial.len();
    if step.len() != n {
        return Err(YieldCurveError::FitFailed(
            "step must match initial length".into(),
        ));
    }

    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(initial.clone());
    for i in 0..n {
        let mut v = initial.clone();
        v[i] += step[i];
        simplex.push(v);
    }
    let mut values: Vec<f64> = simplex.iter().map(|v| f(v)).collect();

    let alpha = 1.0;
    let gamma = 2.0;
    let rho = 0.5;
    let sigma = 0.5;

    for _ in 0..max_iter {
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| {
            values[a]
                .partial_cmp(&values[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let best = order[0];
        let second_worst = order[n - 1];
        let worst = order[n];

        let spread = values[worst] - values[best];
        if spread.abs() < tol {
            return Ok((simplex[best].clone(), values[best]));
        }

        let mut centroid = vec![0.0; n];
        for &idx in &order[..n] {
            for j in 0..n {
                centroid[j] += simplex[idx][j] / n as f64;
            }
        }

        let reflected: Vec<f64> = (0..n)
            .map(|j| centroid[j] + alpha * (centroid[j] - simplex[worst][j]))
            .collect();
        let f_reflected = f(&reflected);

        if f_reflected < values[best] {
            let expanded: Vec<f64> = (0..n)
                .map(|j| centroid[j] + gamma * (reflected[j] - centroid[j]))
                .collect();
            let f_expanded = f(&expanded);
            if f_expanded < f_reflected {
                simplex[worst] = expanded;
                values[worst] = f_expanded;
            } else {
                simplex[worst] = reflected;
                values[worst] = f_reflected;
            }
        } else if f_reflected < values[second_worst] {
            simplex[worst] = reflected;
            values[worst] = f_reflected;
        } else {
            let contracted: Vec<f64> = (0..n)
                .map(|j| centroid[j] + rho * (simplex[worst][j] - centroid[j]))
                .collect();
            let f_contracted = f(&contracted);
            if f_contracted < values[worst] {
                simplex[worst] = contracted;
                values[worst] = f_contracted;
            } else {
                let best_pt = simplex[best].clone();
                for &idx in &order[1..] {
                    for j in 0..n {
                        simplex[idx][j] = best_pt[j] + sigma * (simplex[idx][j] - best_pt[j]);
                    }
                    values[idx] = f(&simplex[idx]);
                }
            }
        }
    }

    Err(YieldCurveError::FitFailed(format!(
        "Nelder-Mead did not converge in {max_iter} iterations"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn finds_parabola_minimum() {
        let f = |p: &[f64]| (p[0] - 3.0).powi(2) + (p[1] + 2.0).powi(2);
        let (best, val) = nelder_mead(f, vec![0.0, 0.0], vec![1.0, 1.0], 1000, 1e-12).unwrap();
        assert!(approx_eq(best[0], 3.0, 1e-4));
        assert!(approx_eq(best[1], -2.0, 1e-4));
        assert!(val < 1e-6);
    }

    #[test]
    fn rosenbrock() {
        let f = |p: &[f64]| (1.0 - p[0]).powi(2) + 100.0 * (p[1] - p[0].powi(2)).powi(2);
        let (best, val) = nelder_mead(f, vec![0.0, 0.0], vec![0.5, 0.5], 10_000, 1e-12).unwrap();
        assert!(approx_eq(best[0], 1.0, 1e-3));
        assert!(approx_eq(best[1], 1.0, 1e-3));
        assert!(val < 1e-6);
    }
}
