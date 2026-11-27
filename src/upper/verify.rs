use std::collections::BTreeSet;

use num_traits::Zero;

use crate::model::bound::{DensityVector, Fraction, UpperBound};
use crate::model::fm::{LinearConstraint, fm_feasible, fm_witness};
use crate::model::word::{
    DominanceBox, PeriodicSchedule, box_dominates, dominance_box, is_density_ordered,
};
use crate::upper::cells::density_bound;

/// Returns true iff `d` belongs to `D_3 = { d_1 >= d_2 >= d_3 > 0, d_1 + d_2 + d_3 <= 5/6 }`.
pub fn in_domain(d: &DensityVector) -> bool {
    d[0] >= d[1] && d[1] >= d[2] && d[2] > Fraction::zero() && d[0] + d[1] + d[2] <= density_bound()
}

/// One-dimensional cell of the full-space threshold arrangement (point, bounded open interval,
/// or unbounded ray `(a, +infinity)`).
#[derive(Debug, Clone, Copy)]
enum DomainElem {
    Point(Fraction),
    Open(Fraction, Option<Fraction>),
}

impl DomainElem {
    fn is_dominated_by(self, bnd: UpperBound) -> bool {
        match self {
            DomainElem::Point(a) => bnd.satisfies(a),
            DomainElem::Open(_, Some(b)) => b <= bnd.value,
            DomainElem::Open(_, None) => false,
        }
    }

    fn push_constraints(self, coord: usize, cons: &mut Vec<LinearConstraint>) {
        let zero = Fraction::zero();
        let mut pos = [zero; 3];
        let mut neg = [zero; 3];
        pos[coord] = Fraction::from_integer(1);
        neg[coord] = Fraction::from_integer(-1);
        match self {
            DomainElem::Point(a) => {
                cons.push(LinearConstraint::le(pos, a));
                cons.push(LinearConstraint::le(neg, -a));
            }
            DomainElem::Open(a, hi) => {
                cons.push(LinearConstraint::lt(neg, -a));
                if let Some(b) = hi {
                    cons.push(LinearConstraint::lt(pos, b));
                }
            }
        }
    }
}

/// Solver-free exact domination check over the full domain `D_3` (without assuming the slice
/// reduction), using threshold cell enumeration and exact rational Fourier–Motzkin elimination.
///
/// Analogous to `UndominatedVectorFinder` in `upstream/pinwheel_scheduling_packing_prover.py`,
/// but without Z3: returns `(cells_checked, Some(undominated_density_vector))` if any point in
/// `D_3` lies outside all active dominance boxes, or `(cells_checked, None)` if `D_3` is dominated.
pub fn find_undominated_density_vector(
    boxes: &[DominanceBox],
    active: &[usize],
) -> (usize, Option<DensityVector>) {
    let bound = density_bound();
    let zero = Fraction::zero();
    let one = Fraction::from_integer(1);

    let elems: [Vec<DomainElem>; 3] = [0, 1, 2].map(|c| {
        let mut ts: BTreeSet<Fraction> = BTreeSet::new();
        for &j in active {
            ts.insert(boxes[j][c].value);
        }
        ts.insert(bound);
        let ts_vec: Vec<Fraction> = ts.into_iter().collect();

        let mut e = Vec::with_capacity(2 * ts_vec.len() + 1);
        e.push(DomainElem::Open(zero, Some(ts_vec[0])));
        for i in 0..ts_vec.len() {
            e.push(DomainElem::Point(ts_vec[i]));
            let next = ts_vec.get(i + 1).copied();
            e.push(DomainElem::Open(ts_vec[i], next));
        }
        e
    });

    // Full domain D_3 directly from the paper's definition:
    //   d_1 >= d_2 >= d_3 > 0,  d_1 + d_2 + d_3 <= 5/6.
    let base = vec![
        LinearConstraint::le([-one, one, zero], zero),
        LinearConstraint::le([zero, -one, one], zero),
        LinearConstraint::lt([zero, zero, -one], zero),
        LinearConstraint::le([one, one, one], bound),
    ];

    let mut ncells = 0usize;
    let mut cons = Vec::with_capacity(base.len() + 6);

    for &e0 in &elems[0] {
        for &e1 in &elems[1] {
            for &e2 in &elems[2] {
                ncells += 1;
                let dominated = active.iter().any(|&j| {
                    e0.is_dominated_by(boxes[j][0])
                        && e1.is_dominated_by(boxes[j][1])
                        && e2.is_dominated_by(boxes[j][2])
                });
                if dominated {
                    continue;
                }

                cons.clear();
                cons.extend_from_slice(&base);
                e0.push_constraints(0, &mut cons);
                e1.push_constraints(1, &mut cons);
                e2.push_constraints(2, &mut cons);

                if fm_feasible(&cons) {
                    let witness = fm_witness(&cons)
                        .expect("FM reported feasible but witness reconstruction failed");
                    return (ncells, Some(witness));
                }
            }
        }
    }

    (ncells, None)
}

/// Leave-one-out necessity entry for a single word in a certificate.
#[derive(Debug, Clone)]
pub struct LeaveOneOutEntry {
    pub word: String,
    /// An explicit density vector in `D_3` that becomes undominated when `word` is removed
    /// (`None` means `word` is redundant).
    pub undominated_witness: Option<DensityVector>,
}

/// Full independent verification report for a certificate.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    pub word_count: usize,
    /// Number of threshold cells enumerated (cells inside some box are skipped without an FM call).
    pub total_cells_checked: usize,
    pub leave_one_out: Vec<LeaveOneOutEntry>,
}

impl VerificationReport {
    pub fn redundant_words(&self) -> Vec<&str> {
        self.leave_one_out
            .iter()
            .filter(|e| e.undominated_witness.is_none())
            .map(|e| e.word.as_str())
            .collect()
    }
}

/// Independently verifies a certificate over `D_3`:
///   1. Recomputes every dominance box from its word and checks annotation & density ordering.
///   2. Proves domination of `D_3` via solver-free Fourier–Motzkin cell enumeration.
///   3. Runs leave-one-out and re-checks every undominated witness with plain rational comparisons.
pub fn verify_certificate(schedules: &[PeriodicSchedule]) -> Result<VerificationReport, String> {
    if schedules.is_empty() {
        return Err("certificate is empty".into());
    }

    let mut boxes = Vec::with_capacity(schedules.len());
    for entry in schedules {
        let recomputed = dominance_box(&entry.word)?;
        if recomputed != entry.density_bounds {
            return Err(format!(
                "dominance box mismatch for {}: recomputed=({},{},{}), file=({},{},{})",
                entry.word,
                recomputed[0],
                recomputed[1],
                recomputed[2],
                entry.density_bounds[0],
                entry.density_bounds[1],
                entry.density_bounds[2]
            ));
        }
        if !is_density_ordered(&recomputed) {
            return Err(format!(
                "dominance box is not density-ordered for {}",
                entry.word
            ));
        }
        boxes.push(recomputed);
    }

    let all_idx: Vec<usize> = (0..boxes.len()).collect();
    let (total_cells_checked, undominated) = find_undominated_density_vector(&boxes, &all_idx);
    if let Some(d) = undominated {
        return Err(format!(
            "supplied words do NOT dominate D_3; undominated density vector: ({}, {}, {})",
            d[0], d[1], d[2]
        ));
    }

    let mut leave_one_out = Vec::with_capacity(schedules.len());
    for (i, entry) in schedules.iter().enumerate() {
        let others: Vec<usize> = (0..boxes.len()).filter(|&j| j != i).collect();
        let (_, witness) = find_undominated_density_vector(&boxes, &others);
        if let Some(d) = witness {
            if !in_domain(&d) {
                return Err(format!(
                    "internal error: witness for {} is outside D_3",
                    entry.word
                ));
            }
            if others.iter().any(|&j| box_dominates(&boxes[j], &d)) {
                return Err(format!(
                    "internal error: witness for {} is dominated by another box",
                    entry.word
                ));
            }
            if !box_dominates(&boxes[i], &d) {
                return Err(format!(
                    "internal error: witness for {} is not dominated by B({})",
                    entry.word, entry.word
                ));
            }
        }
        leave_one_out.push(LeaveOneOutEntry {
            word: entry.word.clone(),
            undominated_witness: witness,
        });
    }

    Ok(VerificationReport {
        word_count: schedules.len(),
        total_cells_checked,
        leave_one_out,
    })
}
