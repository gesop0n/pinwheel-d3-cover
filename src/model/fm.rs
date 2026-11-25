use num_traits::Zero;

use crate::model::bound::Rat;

/// 3変数線形不等式制約:
///   `coeffs[0]*d0 + coeffs[1]*d1 + coeffs[2]*d2 <= rhs` (strict == false)
///   `coeffs[0]*d0 + coeffs[1]*d1 + coeffs[2]*d2 <  rhs` (strict == true)
#[derive(Debug, Clone)]
pub struct LinearConstraint {
    pub coeffs: [Rat; 3],
    pub rhs: Rat,
    pub strict: bool,
}

impl LinearConstraint {
    pub fn le(coeffs: [Rat; 3], rhs: Rat) -> Self {
        Self {
            coeffs,
            rhs,
            strict: false,
        }
    }

    pub fn lt(coeffs: [Rat; 3], rhs: Rat) -> Self {
        Self {
            coeffs,
            rhs,
            strict: true,
        }
    }

    /// 点 `p` がこの制約を満たすか
    pub fn satisfied_by(&self, p: &[Rat; 3]) -> bool {
        let lhs = self.coeffs[0] * p[0] + self.coeffs[1] * p[1] + self.coeffs[2] * p[2];
        if self.strict {
            lhs < self.rhs
        } else {
            lhs <= self.rhs
        }
    }
}

/// 1つの変数 `var` を Fourier–Motzkin 消去する。
/// 矛盾する定数制約が現れたら `None` を返す。
fn eliminate_var(cons: &[LinearConstraint], var: usize) -> Option<Vec<LinearConstraint>> {
    let zero = Rat::zero();
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    let mut out = Vec::new();

    for c in cons {
        let a = c.coeffs[var];
        if a > zero {
            pos.push(c);
        } else if a < zero {
            neg.push(c);
        } else {
            if c.coeffs.iter().all(|x| x.is_zero()) {
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
    }

    for u in &pos {
        let au = u.coeffs[var]; // > 0
        for l in &neg {
            let al = -l.coeffs[var]; // > 0
            // 係数 var を +1 と -1 に正規化してから足すことで桁膨張を防ぐ
            let mut coeffs = [zero; 3];
            for k in 0..3 {
                if k != var {
                    coeffs[k] = u.coeffs[k] / au + l.coeffs[k] / al;
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

/// 3変数の線形不等式系に実数解（有理数解）が存在するかを厳密判定する
pub fn fm_feasible(cons: &[LinearConstraint]) -> bool {
    let Some(l1) = eliminate_var(cons, 0) else {
        return false;
    };
    let Some(l2) = eliminate_var(&l1, 1) else {
        return false;
    };
    eliminate_var(&l2, 2).is_some()
}

/// 3変数の線形不等式系に解が存在すれば、具体的な有理数解（witness）を1点返す
pub fn fm_witness(cons: &[LinearConstraint]) -> Option<[Rat; 3]> {
    let l1 = eliminate_var(cons, 0)?;
    let l2 = eliminate_var(&l1, 1)?;
    let _ = eliminate_var(&l2, 2)?;

    let layers: [&[LinearConstraint]; 3] = [cons, &l1, &l2];
    let zero = Rat::zero();
    let one = Rat::from_integer(1);
    let two = Rat::from_integer(2);
    let mut point = [zero; 3];

    // 逆順に d2 -> d1 -> d0 を決定（バックサブスティチューション）
    for var in (0..3).rev() {
        let mut lo: Option<(Rat, bool)> = None; // d_var >= val (strict なら >)
        let mut hi: Option<(Rat, bool)> = None; // d_var <= val (strict なら <)

        for c in layers[var] {
            let a = c.coeffs[var];
            if a.is_zero() {
                continue;
            }
            let mut other = zero;
            for k in (var + 1)..3 {
                other += c.coeffs[k] * point[k];
            }
            let val = (c.rhs - other) / a;
            if a > zero {
                // d_var <= val (or <)
                let tighter = match hi {
                    None => true,
                    Some((hv, hs)) => val < hv || (val == hv && c.strict && !hs),
                };
                if tighter {
                    hi = Some((val, c.strict));
                }
            } else {
                // a < 0 で割るので不等号反転: d_var >= val (or >)
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
                if l == h {
                    l
                } else {
                    (l + h) / two
                }
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
        let z = Rat::zero();
        let o = Rat::from_integer(1);
        let bound = Rat::new(5, 6);
        let cons = vec![
            LinearConstraint::le([-o, o, z], z),       // d0 >= d1
            LinearConstraint::le([z, -o, o], z),       // d1 >= d2
            LinearConstraint::lt([z, z, -o], z),       // d2 > 0
            LinearConstraint::le([o, o, o], bound),    // sum <= 5/6
            LinearConstraint::le([-o, -o, -o], -bound), // sum >= 5/6
        ];
        assert!(fm_feasible(&cons));
        let p = fm_witness(&cons).expect("witness should exist");
        assert!(cons.iter().all(|c| c.satisfied_by(&p)));
        assert_eq!(p[0] + p[1] + p[2], bound);
    }

    #[test]
    fn strict_infeasibility_detected() {
        let z = Rat::zero();
        let o = Rat::from_integer(1);
        let half = Rat::new(1, 2);
        // d0 < 1/2 かつ d0 >= 1/2 は解なし
        let cons = vec![
            LinearConstraint::lt([o, z, z], half),
            LinearConstraint::le([-o, z, z], -half),
        ];
        assert!(!fm_feasible(&cons));
        assert!(fm_witness(&cons).is_none());
    }
}
