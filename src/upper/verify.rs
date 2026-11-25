use std::collections::BTreeSet;

use num_traits::Zero;

use crate::model::bound::{Rat, UpperBound};
use crate::model::fm::{LinearConstraint, fm_feasible, fm_witness};
use crate::model::word::{Box3, dominance_box, is_density_ordered};
use crate::upper::cells::density_bound;

/// 点 `p` が領域 D_3 = { d1 >= d2 >= d3 > 0, d1 + d2 + d3 <= 5/6 } に属するか
pub fn in_domain(p: &[Rat; 3]) -> bool {
    p[0] >= p[1] && p[1] >= p[2] && p[2] > Rat::zero() && p[0] + p[1] + p[2] <= density_bound()
}

/// 点 `p` が支配箱 `b` に含まれるか
pub fn in_box(p: &[Rat; 3], b: &Box3) -> bool {
    b[0].contains(p[0]) && b[1].contains(p[1]) && b[2].contains(p[2])
}

/// 全空間の軸分割要素（有界点、有界開区間、または非有界開区間 (a, +∞)）
#[derive(Debug, Clone, Copy)]
enum DomainElem {
    Point(Rat),
    Open(Rat, Option<Rat>),
}

impl DomainElem {
    fn is_covered_by(self, bnd: UpperBound) -> bool {
        match self {
            DomainElem::Point(a) => bnd.contains(a),
            DomainElem::Open(_, Some(b)) => b <= bnd.value,
            DomainElem::Open(_, None) => false,
        }
    }

    fn push_constraints(self, coord: usize, cons: &mut Vec<LinearConstraint>) {
        let zero = Rat::zero();
        let mut pos = [zero; 3];
        let mut neg = [zero; 3];
        pos[coord] = Rat::from_integer(1);
        neg[coord] = Rat::from_integer(-1);
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

/// ソルバを使わず、閾値による細胞分割と厳密有理数 Fourier–Motzkin 消去だけで
/// `active` の箱たちが D_3 全体を覆っているかを検証する。
///
/// 覆い残しがあれば、その中の具体的な有理数点（witness）を返す。
pub fn fm_find_uncovered(boxes: &[Box3], active: &[usize]) -> (usize, Option<[Rat; 3]>) {
    let bound = density_bound();
    let zero = Rat::zero();
    let one = Rat::from_integer(1);

    let elems: [Vec<DomainElem>; 3] = [0, 1, 2].map(|c| {
        let mut ts: BTreeSet<Rat> = BTreeSet::new();
        for &j in active {
            ts.insert(boxes[j][c].value);
        }
        ts.insert(bound);
        let ts_vec: Vec<Rat> = ts.into_iter().collect();

        let mut e = Vec::with_capacity(2 * ts_vec.len() + 1);
        e.push(DomainElem::Open(zero, Some(ts_vec[0])));
        for i in 0..ts_vec.len() {
            e.push(DomainElem::Point(ts_vec[i]));
            let next = ts_vec.get(i + 1).copied();
            e.push(DomainElem::Open(ts_vec[i], next));
        }
        e
    });

    // D_3 の定義そのまま（スライス帰着は仮定しない）:
    //   d0 >= d1, d1 >= d2, d2 > 0, d0 + d1 + d2 <= 5/6
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
                let covered = active.iter().any(|&j| {
                    e0.is_covered_by(boxes[j][0])
                        && e1.is_covered_by(boxes[j][1])
                        && e2.is_covered_by(boxes[j][2])
                });
                if covered {
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

/// 1語を外したときの必要性チェック結果
#[derive(Debug, Clone)]
pub struct LeaveOneOutEntry {
    pub word: String,
    /// この語を外すと覆えなくなる D_3 内の有理点（None ならこの語は冗長）
    pub witness: Option<[Rat; 3]>,
}

/// 証明書全体の独立検証レポート
#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub word_count: usize,
    pub total_cells_checked: usize,
    pub leave_one_out: Vec<LeaveOneOutEntry>,
}

impl VerifyReport {
    pub fn redundant_words(&self) -> Vec<&str> {
        self.leave_one_out
            .iter()
            .filter(|e| e.witness.is_none())
            .map(|e| e.word.as_str())
            .collect()
    }
}

/// 証明書ファイルの内容を独立に完全検証する:
///   1. 全語の支配箱 B(w) を語から再計算し、注釈一致・密度降順を確認
///   2. ソルバ不要の FM 細胞列挙で D_3 全体の被覆を確認
///   3. 各語を外した leave-one-out チェックと、覆い残し witness 点の単純分数検算
pub fn verify_certificate(entries: &[(String, Box3)]) -> Result<VerifyReport, String> {
    if entries.is_empty() {
        return Err("certificate is empty".into());
    }

    let mut boxes = Vec::with_capacity(entries.len());
    for (word, annotated) in entries {
        let recomputed = dominance_box(word)?;
        if recomputed != *annotated {
            return Err(format!(
                "box annotation mismatch for {word}: recomputed=({},{},{}), file=({},{},{})",
                recomputed[0], recomputed[1], recomputed[2], annotated[0], annotated[1], annotated[2]
            ));
        }
        if !is_density_ordered(&recomputed) {
            return Err(format!("box is not density-ordered for {word}"));
        }
        boxes.push(recomputed);
    }

    let all_idx: Vec<usize> = (0..boxes.len()).collect();
    let (total_cells_checked, uncovered) = fm_find_uncovered(&boxes, &all_idx);
    if let Some(p) = uncovered {
        return Err(format!(
            "certificate does NOT cover D_3: uncovered point ({}, {}, {})",
            p[0], p[1], p[2]
        ));
    }

    let mut leave_one_out = Vec::with_capacity(entries.len());
    for (i, (word, _)) in entries.iter().enumerate() {
        let others: Vec<usize> = (0..boxes.len()).filter(|&j| j != i).collect();
        let (_, witness) = fm_find_uncovered(&boxes, &others);
        if let Some(p) = witness {
            // 単純な分数演算だけで 3 条件を再検算する
            if !in_domain(&p) {
                return Err(format!("internal error: witness for {word} is outside D_3"));
            }
            if others.iter().any(|&j| in_box(&p, &boxes[j])) {
                return Err(format!(
                    "internal error: witness for {word} is covered by another box"
                ));
            }
            if !in_box(&p, &boxes[i]) {
                return Err(format!(
                    "internal error: witness for {word} is not covered by B({word})"
                ));
            }
        }
        leave_one_out.push(LeaveOneOutEntry {
            word: word.clone(),
            witness,
        });
    }

    Ok(VerifyReport {
        word_count: entries.len(),
        total_cells_checked,
        leave_one_out,
    })
}
