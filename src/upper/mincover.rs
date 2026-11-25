use good_lp::{
    Expression, ProblemVariables, Solution, SolverModel, Variable, constraint, highs, variable,
};

use crate::upper::cells::{BitRow, Pool, PoolItem, SliceCells, build_pool, cells_on_slice};

/// 最小集合被覆の結果
pub struct MinCoverResult {
    pub pool_total_words: usize,
    pub pool: Pool,
    pub cells: SliceCells,
    pub chosen: Vec<PoolItem>,
}

/// 集合被覆問題を HiGHS で 2 段階に解く:
///   1. 選ぶ語数を最小化 -> 最適値 k
///   2. 語数 == k のもとで総語長を最小化（短い語を優先）
pub fn solve_min_cover(pool: &[PoolItem], rows: &[BitRow]) -> Result<Vec<usize>, String> {
    let m = pool.len();

    // --- 1段階目: 語数を最小化 ---
    let mut vars1 = ProblemVariables::new();
    let x1: Vec<Variable> = (0..m).map(|_| vars1.add(variable().binary())).collect();
    let obj1: Expression = x1.iter().copied().sum();
    let mut model1 = vars1.minimise(obj1).using(highs);
    for row in rows {
        let lhs: Expression = row.iter_ones().map(|i| x1[i]).sum();
        model1 = model1.with(constraint!(lhs >= 1));
    }
    let sol1 = model1
        .solve()
        .map_err(|e| format!("HiGHS failed (stage 1): {e}"))?;
    let k: usize = x1.iter().filter(|&&v| sol1.value(v) > 0.5).count();

    // --- 2段階目: 語数 = k 固定で総語長を最小化 ---
    let mut vars2 = ProblemVariables::new();
    let x2: Vec<Variable> = (0..m).map(|_| vars2.add(variable().binary())).collect();
    let obj2: Expression = x2
        .iter()
        .enumerate()
        .map(|(i, &v)| (pool[i].word.len() as f64) * v)
        .sum();
    let mut model2 = vars2.minimise(obj2).using(highs);
    for row in rows {
        let lhs: Expression = row.iter_ones().map(|i| x2[i]).sum();
        model2 = model2.with(constraint!(lhs >= 1));
    }
    let total_words: Expression = x2.iter().copied().sum();
    model2 = model2.with(constraint!(total_words == k as f64));

    let sol2 = model2
        .solve()
        .map_err(|e| format!("HiGHS failed (stage 2): {e}"))?;
    let mut chosen: Vec<usize> = (0..m).filter(|&i| sol2.value(x2[i]) > 0.5).collect();
    chosen.sort_by(|&i, &j| {
        pool[j]
            .word
            .len()
            .cmp(&pool[i].word.len())
            .then_with(|| pool[i].word.cmp(&pool[j].word))
    });
    Ok(chosen)
}

/// 候補語リストから極大箱プール構築 → スライス細胞分割 → HiGHS 最小被覆を一括実行する
pub fn find_min_cover(words: &[String]) -> Result<MinCoverResult, String> {
    let pool = build_pool(words)?;
    let cells = cells_on_slice(&pool.items)?;
    let chosen_idx = solve_min_cover(&pool.items, &cells.minimal_rows)?;
    let chosen = chosen_idx.into_iter().map(|i| pool.items[i].clone()).collect();
    Ok(MinCoverResult {
        pool_total_words: words.len(),
        pool,
        cells,
        chosen,
    })
}

/// 選ばれた語を元論文 prover 互換の証明書形式（`WORD,b1,b2,b3`）に整形する
pub fn format_certificate(chosen: &[PoolItem], header_comment: &str) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(header_comment);
    out.push('\n');
    for item in chosen {
        out.push_str(&format!(
            "{},{},{},{}\n",
            item.word, item.bounds[0], item.bounds[1], item.bounds[2]
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upper::enumerate::enumerate_boxes;

    #[test]
    fn min_cover_len_12_is_12_words() {
        let words = enumerate_boxes(12);
        let res = find_min_cover(&words).unwrap();
        assert_eq!(res.pool.items.len(), 55);
        assert_eq!(res.chosen.len(), 12);
        // すべての語が長さ <= 12
        assert!(res.chosen.iter().all(|item| item.word.len() <= 12));
    }
}
