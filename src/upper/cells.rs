use std::collections::{BTreeSet, HashMap, HashSet};

use num_traits::Zero;

use crate::model::bound::{Fraction, UpperBound};
use crate::model::fm::{LinearConstraint, fm_feasible};
use crate::model::word::{DominanceBox, PeriodicSchedule, repair_word};

/// Density bound `5/6` defining the domain `D_3`.
pub fn density_bound() -> Fraction {
    Fraction::new(5, 6)
}

/// Coordinate suprema over `D_3 = { d_1 >= d_2 >= d_3 > 0, d_1 + d_2 + d_3 <= 5/6 }`.
pub fn domain_caps() -> [Fraction; 3] {
    [
        Fraction::new(5, 6),
        Fraction::new(5, 12),
        Fraction::new(5, 18),
    ]
}

/// Caps each coordinate bound at the supremum of `d_c` over `D_3` so that two boxes covering the
/// exact same subset of `D_3` share the same key.
pub fn cap_box_for_domain(bounds: DominanceBox) -> DominanceBox {
    let caps = domain_caps();
    [0, 1, 2].map(|c| {
        if bounds[c].value > caps[c] {
            UpperBound::inclusive(caps[c])
        } else {
            bounds[c]
        }
    })
}

/// Candidate periodic schedule in the inclusion-maximal pool.
#[derive(Debug, Clone)]
pub struct PoolItem {
    pub schedule: PeriodicSchedule,
    pub capped: DominanceBox,
}

/// Inclusion-maximal dominance boxes over `D_3` extracted from a candidate word list.
pub struct Pool {
    pub items: Vec<PoolItem>,
    pub distinct_in_domain: usize,
}

pub fn build_pool(words: &[String]) -> Result<Pool, String> {
    let mut by_key: HashMap<DominanceBox, PeriodicSchedule> = HashMap::new();
    for raw in words {
        let canonical = repair_word(raw)?;
        let sched = PeriodicSchedule::from_word(&canonical)?;
        let k = cap_box_for_domain(sched.density_bounds);
        match by_key.get_mut(&k) {
            None => {
                by_key.insert(k, sched);
            }
            Some(cur) => {
                if (sched.word.len(), &sched.word) < (cur.word.len(), &cur.word) {
                    *cur = sched;
                }
            }
        }
    }

    let distinct_in_domain = by_key.len();
    // Sorting keys lexicographically descending guarantees any dominating box appears earlier.
    let mut keys: Vec<DominanceBox> = by_key.keys().copied().collect();
    keys.sort_by(|a, b| b.cmp(a));

    let mut maximal: Vec<DominanceBox> = Vec::new();
    for k in keys {
        let dominated = maximal
            .iter()
            .any(|o| k[0] <= o[0] && k[1] <= o[1] && k[2] <= o[2]);
        if !dominated {
            maximal.push(k);
        }
    }

    let items = maximal
        .into_iter()
        .map(|k| {
            let schedule = by_key.remove(&k).unwrap();
            PoolItem {
                schedule,
                capped: k,
            }
        })
        .collect();

    Ok(Pool {
        items,
        distinct_in_domain,
    })
}

/// One-dimensional cell in the threshold arrangement: either a single point or a bounded open interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisElem {
    Point(Fraction),
    Open(Fraction, Fraction),
}

impl AxisElem {
    pub fn lo(self) -> Fraction {
        match self {
            AxisElem::Point(a) | AxisElem::Open(a, _) => a,
        }
    }

    pub fn hi(self) -> Fraction {
        match self {
            AxisElem::Point(a) | AxisElem::Open(_, a) => a,
        }
    }

    /// Returns true iff every value in this element satisfies `bnd`.
    pub fn is_dominated_by(self, bnd: UpperBound) -> bool {
        match self {
            AxisElem::Point(a) => bnd.satisfies(a),
            // Every x in (a, b) satisfies x < v (or x <= v) iff b <= v.
            AxisElem::Open(_, b) => b <= bnd.value,
        }
    }

    /// Appends the linear inequalities confining coordinate `coord` to this element.
    pub fn push_constraints(self, coord: usize, cons: &mut Vec<LinearConstraint>) {
        let zero = Fraction::zero();
        let mut pos = [zero; 3];
        let mut neg = [zero; 3];
        pos[coord] = Fraction::from_integer(1);
        neg[coord] = Fraction::from_integer(-1);
        match self {
            AxisElem::Point(a) => {
                cons.push(LinearConstraint::le(pos, a));
                cons.push(LinearConstraint::le(neg, -a));
            }
            AxisElem::Open(a, b) => {
                cons.push(LinearConstraint::lt(neg, -a));
                cons.push(LinearConstraint::lt(pos, b));
            }
        }
    }
}

/// Bitset representing the subset of pool schedules that dominate a given cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BitRow {
    words: Vec<u64>,
}

impl BitRow {
    pub fn new(num_cols: usize) -> Self {
        Self {
            words: vec![0; num_cols.div_ceil(64)],
        }
    }

    pub fn set(&mut self, idx: usize) {
        self.words[idx / 64] |= 1u64 << (idx % 64);
    }

    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            words: self
                .words
                .iter()
                .zip(&other.words)
                .map(|(&a, &b)| a & b)
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(&other.words)
            .all(|(&a, &b)| (a & !b) == 0)
    }

    pub fn iter_ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, &w)| {
            let mut bits = w;
            let base = wi * 64;
            std::iter::from_fn(move || {
                if bits == 0 {
                    None
                } else {
                    let tz = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    Some(base + tz)
                }
            })
        })
    }
}

/// Finite set-cover reduction on the top slice `Delta = D_3 ∩ { d_1 + d_2 + d_3 = 5/6 }`.
///
/// Because dominance boxes are down-closed, dominating `D_3` is equivalent to dominating `Delta`.
pub struct SliceCells {
    /// Inclusion-minimal cover sets (each row is the set of pool indices dominating that cell).
    pub minimal_rows: Vec<BitRow>,
    pub axis_sizes: [usize; 3],
    pub tested_cells: usize,
    pub feasible_cells: usize,
    pub distinct_cover_sets: usize,
}

/// Partitions each axis by the pool's thresholds, filters product cells meeting `Delta` via exact
/// Fourier–Motzkin elimination, and returns the resulting inclusion-minimal set-cover rows.
pub fn cells_on_slice(pool: &[PoolItem]) -> Result<SliceCells, String> {
    let bound = density_bound();
    let zero = Fraction::zero();
    let one = Fraction::from_integer(1);
    let m = pool.len();

    let mut elems: [Vec<AxisElem>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut ts3: Vec<Fraction> = Vec::new();

    for (c, axis_elems) in elems.iter_mut().enumerate() {
        let mut ts: BTreeSet<Fraction> = BTreeSet::new();
        for item in pool {
            if item.schedule.density_bounds[c].value < bound {
                ts.insert(item.schedule.density_bounds[c].value);
            }
        }
        ts.insert(bound);
        let ts_vec: Vec<Fraction> = ts.into_iter().collect();
        if c == 2 {
            ts3 = ts_vec.clone();
        }

        let mut e = Vec::with_capacity(2 * ts_vec.len());
        e.push(AxisElem::Open(zero, ts_vec[0]));
        for i in 0..ts_vec.len() {
            e.push(AxisElem::Point(ts_vec[i]));
            if i + 1 < ts_vec.len() {
                e.push(AxisElem::Open(ts_vec[i], ts_vec[i + 1]));
            }
        }
        *axis_elems = e;
    }

    let cov: [Vec<BitRow>; 3] = [0, 1, 2].map(|c| {
        elems[c]
            .iter()
            .map(|&el| {
                let mut row = BitRow::new(m);
                for (i, item) in pool.iter().enumerate() {
                    if el.is_dominated_by(item.schedule.density_bounds[c]) {
                        row.set(i);
                    }
                }
                row
            })
            .collect()
    });

    // Slice constraints: d_1 >= d_2 >= d_3 > 0 and d_1 + d_2 + d_3 == 5/6.
    let base = vec![
        LinearConstraint::le([-one, one, zero], zero),
        LinearConstraint::le([zero, -one, one], zero),
        LinearConstraint::lt([zero, zero, -one], zero),
        LinearConstraint::le([one, one, one], bound),
        LinearConstraint::le([-one, -one, -one], -bound),
    ];

    let idx3 = |v: Fraction| -> usize {
        let i = ts3.partition_point(|&t| t < v);
        if i < ts3.len() && ts3[i] == v {
            2 * i + 1
        } else {
            (2 * i).min(elems[2].len() - 1)
        }
    };

    let mut rows_set: HashSet<BitRow> = HashSet::new();
    let mut tested_cells = 0usize;
    let mut feasible_cells = 0usize;
    let mut cons = Vec::with_capacity(base.len() + 6);

    for (i1, &e1) in elems[0].iter().enumerate() {
        for (i2, &e2) in elems[1].iter().enumerate() {
            if e2.lo() > e1.hi() {
                break; // d_2 <= d_1 impossible for subsequent e2
            }
            let lo3 = bound - e1.hi() - e2.hi();
            let hi3 = bound - e1.lo() - e2.lo();
            if hi3 <= zero || lo3 > e2.hi() {
                continue;
            }
            let c12 = cov[0][i1].intersect(&cov[1][i2]);
            let start = idx3(lo3.max(zero));
            let end = idx3(hi3);
            for i3 in start..=end {
                let e3 = elems[2][i3];
                tested_cells += 1;

                cons.clear();
                cons.extend_from_slice(&base);
                e1.push_constraints(0, &mut cons);
                e2.push_constraints(1, &mut cons);
                e3.push_constraints(2, &mut cons);

                if !fm_feasible(&cons) {
                    continue;
                }
                feasible_cells += 1;

                let c123 = c12.intersect(&cov[2][i3]);
                if c123.is_empty() {
                    return Err(
                        "candidate pool does not dominate D_3: found an undominated feasible cell"
                            .into(),
                    );
                }
                rows_set.insert(c123);
            }
        }
    }

    let distinct_cover_sets = rows_set.len();
    let mut all_rows: Vec<BitRow> = rows_set.into_iter().collect();
    all_rows.sort_by_key(|r| r.count_ones());

    let mut minimal_rows: Vec<BitRow> = Vec::new();
    for r in all_rows {
        if !minimal_rows.iter().any(|o| o.is_subset_of(&r)) {
            minimal_rows.push(r);
        }
    }

    Ok(SliceCells {
        minimal_rows,
        axis_sizes: [elems[0].len(), elems[1].len(), elems[2].len()],
        tested_cells,
        feasible_cells,
        distinct_cover_sets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upper::enumerate::enumerate_boxes;

    #[test]
    fn pool_len_12_has_55_maximal_boxes() {
        let words = enumerate_boxes(12);
        let pool = build_pool(&words).unwrap();
        assert_eq!(pool.items.len(), 55);
        let cells = cells_on_slice(&pool.items).unwrap();
        assert!(!cells.minimal_rows.is_empty());
    }
}
