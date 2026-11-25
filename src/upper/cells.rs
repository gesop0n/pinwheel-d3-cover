use std::collections::{BTreeSet, HashMap, HashSet};

use num_traits::Zero;

use crate::model::bound::{Rat, UpperBound};
use crate::model::fm::{LinearConstraint, fm_feasible};
use crate::model::word::{Box3, dominance_box, relabel};

/// 密度和の上限 5/6
pub fn density_bound() -> Rat {
    Rat::new(5, 6)
}

/// 領域 D_3 = { d1 >= d2 >= d3 > 0, d1 + d2 + d3 <= 5/6 } 上での各座標の上限
pub fn domain_caps() -> [Rat; 3] {
    [Rat::new(5, 6), Rat::new(5, 12), Rat::new(5, 18)]
}

/// D_3 の内側で等価になるよう、各座標の上界を D_3 の上限で切り詰める
pub fn cap_box_for_domain(b: Box3) -> Box3 {
    let caps = domain_caps();
    [0, 1, 2].map(|c| {
        if b[c].value > caps[c] {
            UpperBound::closed(caps[c])
        } else {
            b[c]
        }
    })
}

/// プール内の1候補（正規化済みの語とその支配箱）
#[derive(Debug, Clone)]
pub struct PoolItem {
    pub word: String,
    pub bounds: Box3,
    pub capped: Box3,
}

/// 候補語から D_3 内で包含極大な箱だけを残したプールを構築する
pub struct Pool {
    pub items: Vec<PoolItem>,
    pub distinct_in_domain: usize,
}

pub fn build_pool(words: &[String]) -> Result<Pool, String> {
    let mut by_key: HashMap<Box3, (String, Box3)> = HashMap::new();
    for raw in words {
        let w = relabel(raw)?;
        let b = dominance_box(&w)?;
        let k = cap_box_for_domain(b);
        match by_key.get_mut(&k) {
            None => {
                by_key.insert(k, (w, b));
            }
            Some((cur_w, cur_b)) => {
                if (w.len(), &w) < (cur_w.len(), &*cur_w) {
                    *cur_w = w;
                    *cur_b = b;
                }
            }
        }
    }

    let distinct_in_domain = by_key.len();
    // 辞書順降順に並べると、他の箱を支配する箱が必ず先に現れる
    let mut keys: Vec<Box3> = by_key.keys().copied().collect();
    keys.sort_by(|a, b| b.cmp(a));

    let mut maximal: Vec<Box3> = Vec::new();
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
            let (word, bounds) = by_key.remove(&k).unwrap();
            PoolItem {
                word,
                bounds,
                capped: k,
            }
        })
        .collect();

    Ok(Pool {
        items,
        distinct_in_domain,
    })
}

/// 1座標軸を閾値で分割した基本要素（点 または 有界開区間）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisElem {
    Point(Rat),
    Open(Rat, Rat),
}

impl AxisElem {
    pub fn lo(self) -> Rat {
        match self {
            AxisElem::Point(a) | AxisElem::Open(a, _) => a,
        }
    }

    pub fn hi(self) -> Rat {
        match self {
            AxisElem::Point(a) | AxisElem::Open(_, a) => a,
        }
    }

    /// この要素のすべての点が上界 `bnd` を満たすか
    pub fn is_covered_by(self, bnd: UpperBound) -> bool {
        match self {
            AxisElem::Point(a) => bnd.contains(a),
            // 開区間 (a, b) の全点が x < v (または <= v) を満たす <=> b <= v
            AxisElem::Open(_, b) => b <= bnd.value,
        }
    }

    /// 座標 `coord` がこの要素に属するための線形制約を `cons` に追加する
    pub fn push_constraints(self, coord: usize, cons: &mut Vec<LinearConstraint>) {
        let zero = Rat::zero();
        let mut pos = [zero; 3];
        let mut neg = [zero; 3];
        pos[coord] = Rat::from_integer(1);
        neg[coord] = Rat::from_integer(-1);
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

/// 細胞を覆う箱の集合をビット列で表現する（高速な積集合・包含判定用）
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

    /// `self` が `other` の部分集合か
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

/// スライス Δ = D_3 ∩ { d1 + d2 + d3 = 5/6 } 上の細胞分割の結果
pub struct SliceCells {
    /// 包含極小な被覆行（各行は「この細胞を覆える箱のインデックス集合」）
    pub minimal_rows: Vec<BitRow>,
    pub axis_sizes: [usize; 3],
    pub tested_cells: usize,
    pub feasible_cells: usize,
    pub distinct_cover_sets: usize,
}

/// プール内の箱の閾値でスライス Δ を細胞分割し、有限の集合被覆インスタンスへ帰着する
pub fn cells_on_slice(pool: &[PoolItem]) -> Result<SliceCells, String> {
    let bound = density_bound();
    let zero = Rat::zero();
    let one = Rat::from_integer(1);
    let m = pool.len();

    let mut elems: [Vec<AxisElem>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut ts3: Vec<Rat> = Vec::new();

    for c in 0..3 {
        let mut ts: BTreeSet<Rat> = BTreeSet::new();
        for item in pool {
            if item.bounds[c].value < bound {
                ts.insert(item.bounds[c].value);
            }
        }
        ts.insert(bound);
        let ts_vec: Vec<Rat> = ts.into_iter().collect();
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
        elems[c] = e;
    }

    // 各軸・各要素を覆う箱のビット列を事前計算
    let cov: [Vec<BitRow>; 3] = [0, 1, 2].map(|c| {
        elems[c]
            .iter()
            .map(|&el| {
                let mut row = BitRow::new(m);
                for (i, item) in pool.iter().enumerate() {
                    if el.is_covered_by(item.bounds[c]) {
                        row.set(i);
                    }
                }
                row
            })
            .collect()
    });

    // スライス Δ の基本制約: d0 >= d1, d1 >= d2, d2 > 0, d0 + d1 + d2 == 5/6
    let base = vec![
        LinearConstraint::le([-one, one, zero], zero),
        LinearConstraint::le([zero, -one, one], zero),
        LinearConstraint::lt([zero, zero, -one], zero),
        LinearConstraint::le([one, one, one], bound),
        LinearConstraint::le([-one, -one, -one], -bound),
    ];

    let idx3 = |v: Rat| -> usize {
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
                break; // これ以降は d2 <= d1 を満たせない
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
                    return Err("pool does not cover D_3: found an uncovered feasible cell".into());
                }
                rows_set.insert(c123);
            }
        }
    }

    let distinct_cover_sets = rows_set.len();
    // 要素数昇順に並べて、包含極小な行だけを残す
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
        // pinwheel-test のリファレンス値: L=12 で 1084 語 -> 極大 55 箱
        let words = enumerate_boxes(12);
        let pool = build_pool(&words).unwrap();
        assert_eq!(pool.items.len(), 55);
        let cells = cells_on_slice(&pool.items).unwrap();
        assert!(!cells.minimal_rows.is_empty());
    }
}
