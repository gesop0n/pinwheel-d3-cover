use num_traits::Zero;

use crate::model::bound::{DensityVector, Fraction};

/// Linear inequality in three real variables `(d_1, d_2, d_3)`:
///   `coeffs · d <= rhs` (when `strict == false`)
///   `coeffs · d <  rhs` (when `strict == true`)
#[derive(Debug, Clone)]
pub struct LinearConstraint {
    pub coeffs: [Fraction; 3],
    pub rhs: Fraction,
    pub strict: bool,
}

impl LinearConstraint {
    pub fn le(coeffs: [Fraction; 3], rhs: Fraction) -> Self {
        Self {
            coeffs,
            rhs,
            strict: false,
        }
    }

    pub fn lt(coeffs: [Fraction; 3], rhs: Fraction) -> Self {
        Self {
            coeffs,
            rhs,
            strict: true,
        }
    }

    /// Returns true iff `point` satisfies this linear constraint.
    pub fn satisfied_by(&self, point: &DensityVector) -> bool {
        let lhs = self.coeffs[0] * point[0] + self.coeffs[1] * point[1] + self.coeffs[2] * point[2];
        if self.strict {
            lhs < self.rhs
        } else {
            lhs <= self.rhs
        }
    }
}

/// Eliminates variable `var` via exact rational Fourier–Motzkin elimination.
/// Returns `None` as soon as a contradictory constant constraint is derived.
fn eliminate_var(cons: &[LinearConstraint], var: usize) -> Option<Vec<LinearConstraint>> {
    let zero = Fraction::zero();
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    let mut out = Vec::new();

    for c in cons {
        let a = c.coeffs[var];
        if a > zero {
            pos.push(c);
        } else if a < zero {
            neg.push(c);
        } else if c.coeffs.iter().all(|x| x.is_zero()) {
            let ok = if c.strict {
                zero < c.rhs
            } else {
                zero <= c.rhs
            };
            if !ok {
                return None;
            }
        } else {
            out.push(c.clone());
        }
    }

    for u in &pos {
        let au = u.coeffs[var]; // > 0
        for l in &neg {
            let al = -l.coeffs[var]; // > 0
            // Normalize the eliminated variable's coefficients to +1 and -1 before adding
            // to keep numerators and denominators small.
            let mut coeffs = [zero; 3];
            for (k, coeff) in coeffs.iter_mut().enumerate() {
                if k != var {
                    *coeff = u.coeffs[k] / au + l.coeffs[k] / al;
                }
            }
            let rhs = u.rhs / au + l.rhs / al;
            let strict = u.strict || l.strict;

            if coeffs.iter().all(|x| x.is_zero()) {
                let ok = if strict { zero < rhs } else { zero <= rhs };
                if !ok {
                    return None;
                }
            } else {
                out.push(LinearConstraint {
                    coeffs,
                    rhs,
                    strict,
                });
            }
        }
    }

    Some(out)
}

/// Decides exactly (over `Q`, without any external solver) whether a 3-variable system of
/// strict/non-strict linear inequalities is feasible.
pub fn fm_feasible(cons: &[LinearConstraint]) -> bool {
    let Some(l1) = eliminate_var(cons, 0) else {
        return false;
    };
    let Some(l2) = eliminate_var(&l1, 1) else {
        return false;
    };
    eliminate_var(&l2, 2).is_some()
}

/// If the system is feasible, reconstructs an explicit rational witness point via back-substitution.
pub fn fm_witness(cons: &[LinearConstraint]) -> Option<DensityVector> {
    let l1 = eliminate_var(cons, 0)?;
    let l2 = eliminate_var(&l1, 1)?;
    let _ = eliminate_var(&l2, 2)?;

    let layers: [&[LinearConstraint]; 3] = [cons, &l1, &l2];
    let zero = Fraction::zero();
    let one = Fraction::from_integer(1);
    let two = Fraction::from_integer(2);
    let mut point = [zero; 3];

    // Back-substitute in reverse order: d_3 -> d_2 -> d_1.
    for var in (0..3).rev() {
        let mut lo: Option<(Fraction, bool)> = None; // d_var >= val (or > if strict)
        let mut hi: Option<(Fraction, bool)> = None; // d_var <= val (or < if strict)

        for c in layers[var] {
            let a = c.coeffs[var];
            if a.is_zero() {
                continue;
            }
            let mut other = zero;
            for (k, &coord) in point.iter().enumerate().skip(var + 1) {
                other += c.coeffs[k] * coord;
            }
            let val = (c.rhs - other) / a;
            if a > zero {
                let tighter = match hi {
                    None => true,
                    Some((hv, hs)) => val < hv || (val == hv && c.strict && !hs),
                };
                if tighter {
                    hi = Some((val, c.strict));
                }
            } else {
                // Dividing by a < 0 flips the inequality to a lower bound.
                let tighter = match lo {
                    None => true,
                    Some((lv, ls)) => val > lv || (val == lv && c.strict && !ls),
                };
                if tighter {
                    lo = Some((val, c.strict));
                }
            }
        }

        point[var] = match (lo, hi) {
            (Some((l, ls)), Some((h, hs))) => {
                if l > h || (l == h && (ls || hs)) {
                    return None;
                }
                if l == h { l } else { (l + h) / two }
            }
            (Some((l, _)), None) => l + one,
            (None, Some((h, _))) => h - one,
            (None, None) => zero,
        };
    }

    debug_assert!(cons.iter().all(|c| c.satisfied_by(&point)));
    Some(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feasible_and_witness_on_simplex_slice() {
        let z = Fraction::zero();
        let o = Fraction::from_integer(1);
        let bound = Fraction::new(5, 6);
        let cons = vec![
            LinearConstraint::le([-o, o, z], z),
            LinearConstraint::le([z, -o, o], z),
            LinearConstraint::lt([z, z, -o], z),
            LinearConstraint::le([o, o, o], bound),
            LinearConstraint::le([-o, -o, -o], -bound),
        ];
        assert!(fm_feasible(&cons));
        let p = fm_witness(&cons).expect("witness should exist");
        assert!(cons.iter().all(|c| c.satisfied_by(&p)));
        assert_eq!(p[0] + p[1] + p[2], bound);
    }

    #[test]
    fn strict_infeasibility_detected() {
        let z = Fraction::zero();
        let o = Fraction::from_integer(1);
        let half = Fraction::new(1, 2);
        let cons = vec![
            LinearConstraint::lt([o, z, z], half),
            LinearConstraint::le([-o, z, z], -half),
        ];
        assert!(!fm_feasible(&cons));
        assert!(fm_witness(&cons).is_none());
    }

    #[test]
    fn unbounded_system_yields_valid_witness() {
        let z = Fraction::zero();
        let o = Fraction::from_integer(1);
        // d_1 > d_2, d_2 > 1, d_3 unconstrained.
        let cons = vec![
            LinearConstraint::lt([-o, o, z], z),
            LinearConstraint::lt([z, -o, z], -o),
        ];
        assert!(fm_feasible(&cons));
        let p = fm_witness(&cons).expect("witness should exist");
        assert!(cons.iter().all(|c| c.satisfied_by(&p)));
    }

    #[test]
    fn equality_pinned_point_yields_exact_witness() {
        let z = Fraction::zero();
        let o = Fraction::from_integer(1);
        let p1 = Fraction::new(1, 2);
        let p2 = Fraction::new(1, 4);
        let p3 = Fraction::new(1, 12);
        let cons = vec![
            LinearConstraint::le([o, z, z], p1),
            LinearConstraint::le([-o, z, z], -p1),
            LinearConstraint::le([z, o, z], p2),
            LinearConstraint::le([z, -o, z], -p2),
            LinearConstraint::le([z, z, o], p3),
            LinearConstraint::le([z, z, -o], -p3),
        ];
        assert!(fm_feasible(&cons));
        assert_eq!(fm_witness(&cons), Some([p1, p2, p3]));
    }
}
