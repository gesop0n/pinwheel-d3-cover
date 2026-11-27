use good_lp::{
    Expression, ProblemVariables, Solution, SolverModel, Variable, constraint, highs, variable,
};

use crate::model::word::PeriodicSchedule;
use crate::upper::cells::{BitRow, Pool, PoolItem, SliceCells, build_pool, cells_on_slice};

/// Result of finding a minimum dominating family of words over a candidate pool.
pub struct MinCoverResult {
    pub pool_total_words: usize,
    pub pool: Pool,
    pub cells: SliceCells,
    pub chosen: Vec<PeriodicSchedule>,
}

fn verify_row_cover(chosen: &[usize], rows: &[BitRow], stage: &str) -> Result<(), String> {
    let mut selected = vec![false; chosen.iter().copied().max().map_or(0, |m| m + 1)];
    for &idx in chosen {
        selected[idx] = true;
    }
    for (row_idx, row) in rows.iter().enumerate() {
        let covered = row.iter_ones().any(|i| i < selected.len() && selected[i]);
        if !covered {
            return Err(format!(
                "HiGHS ({stage}) rounded solution does not cover constraint row {row_idx}"
            ));
        }
    }
    Ok(())
}

/// Solves the exact set-cover instance with HiGHS in two stages:
///   1. Minimize the number of words `k`.
///   2. Among the `k`-word covers drawn from `pool`, minimize total word length.
///
/// `pool` holds only inclusion-maximal boxes, so stage 2 is a tie-break within that pool, not a
/// global minimum of total length: a shorter word whose box is contained in a longer word's box
/// (e.g. `ABC` once length-14 words are available) is not a candidate.
///
/// Optimality of `k` rests on HiGHS (floating-point MIP); it is not independently certified.
pub fn solve_min_cover(pool: &[PoolItem], rows: &[BitRow]) -> Result<Vec<usize>, String> {
    let m = pool.len();

    // Stage 1: minimize word count.
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
    let chosen1: Vec<usize> = (0..m).filter(|&i| sol1.value(x1[i]) > 0.5).collect();
    verify_row_cover(&chosen1, rows, "stage 1")?;
    let k = chosen1.len();

    // Stage 2: tie-break by shortest total word length at cardinality k.
    let mut vars2 = ProblemVariables::new();
    let x2: Vec<Variable> = (0..m).map(|_| vars2.add(variable().binary())).collect();
    let obj2: Expression = x2
        .iter()
        .enumerate()
        .map(|(i, &v)| (pool[i].schedule.word.len() as f64) * v)
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
    if chosen.len() != k {
        return Err(format!(
            "HiGHS (stage 2) rounded solution has {} words, expected {k}",
            chosen.len()
        ));
    }
    verify_row_cover(&chosen, rows, "stage 2")?;
    chosen.sort_by(|&i, &j| {
        pool[j]
            .schedule
            .word
            .len()
            .cmp(&pool[i].schedule.word.len())
            .then_with(|| pool[i].schedule.word.cmp(&pool[j].schedule.word))
    });
    Ok(chosen)
}

/// Runs the full upper-bound pipeline: maximal pool -> slice cell arrangement -> HiGHS min cover.
pub fn find_min_cover(words: &[String]) -> Result<MinCoverResult, String> {
    let pool = build_pool(words)?;
    let cells = cells_on_slice(&pool.items)?;
    let chosen_idx = solve_min_cover(&pool.items, &cells.minimal_rows)?;
    let chosen = chosen_idx
        .into_iter()
        .map(|i| pool.items[i].schedule.clone())
        .collect();
    Ok(MinCoverResult {
        pool_total_words: words.len(),
        pool,
        cells,
        chosen,
    })
}

/// Formats schedules in the paper's certificate format (`WORD,b1,b2,b3`).
pub fn format_certificate(schedules: &[PeriodicSchedule], header_comment: &str) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(header_comment);
    out.push('\n');
    for sched in schedules {
        out.push_str(&sched.to_string());
        out.push('\n');
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
        assert!(res.chosen.iter().all(|s| s.word.len() <= 12));
    }
}
