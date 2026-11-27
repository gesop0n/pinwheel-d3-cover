use std::collections::HashMap;

use rayon::prelude::*;

use crate::model::word::repair_word;

/// Compact representation of an `UpperBound` (reduced `num/den` and `strict` flag)
/// used inside the enumeration hot loop to avoid heap allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CompactBound {
    num: u32,
    den: u32,
    strict: bool,
}

impl CompactBound {
    fn new(num: u32, den: u32, strict: bool) -> Self {
        let g = gcd(num, den);
        Self {
            num: num / g,
            den: den / g,
            strict,
        }
    }

    /// Returns true iff `self` is a tighter (smaller) upper bound than `other`.
    fn is_tighter_than(self, other: Self) -> bool {
        let lhs = self.num as u64 * other.den as u64;
        let rhs = other.num as u64 * self.den as u64;
        lhs < rhs || (lhs == rhs && self.strict && !other.strict)
    }

    /// Comparator for descending (density-ordered) sort: wider bounds first.
    fn cmp_desc(self, other: Self) -> std::cmp::Ordering {
        let lhs = self.num as u64 * other.den as u64;
        let rhs = other.num as u64 * self.den as u64;
        rhs.cmp(&lhs).then_with(|| self.strict.cmp(&other.strict))
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// Allocation-free equivalent of `model::word::task_upper_bound` for words over `{0, 1, 2}`.
fn letter_bound_fast(word: &[u8], letter: u8) -> CompactBound {
    let n = word.len();
    debug_assert!(n <= 64);
    let mut pos = [0usize; 128];
    let mut k = 0usize;
    for (i, &c) in word.iter().enumerate() {
        if c == letter {
            pos[k] = i;
            k += 1;
        }
    }
    for i in 0..k {
        pos[k + i] = pos[i] + n;
    }

    let mut best = CompactBound {
        num: k as u32,
        den: n as u32,
        strict: false,
    };
    for l in 1..=k {
        let mut g = 0usize;
        for j in 0..k {
            let d = pos[j + l] - pos[j];
            if d > g {
                g = d;
            }
        }
        if g > 1 {
            let cand = CompactBound {
                num: l as u32,
                den: (g - 1) as u32,
                strict: true,
            };
            if cand.is_tighter_than(best) {
                best = cand;
            }
        }
    }
    CompactBound::new(best.num, best.den, best.strict)
}

type BoxKey = [CompactBound; 3];
type BoxMap = HashMap<BoxKey, String>;

fn insert_shortest(map: &mut BoxMap, key: BoxKey, word: String) {
    match map.get_mut(&key) {
        None => {
            map.insert(key, word);
        }
        Some(cur) => {
            if word.len() < cur.len() || (word.len() == cur.len() && word < *cur) {
                *cur = word;
            }
        }
    }
}

/// Evaluates a primitive Lyndon word over `{0, 1, 2}` and keeps one shortest word per distinct
/// density-ordered dominance box.
fn visit(word: &[u8], out: &mut BoxMap) {
    let mut has = [false; 3];
    for &c in word {
        has[c as usize] = true;
    }
    if !(has[0] && has[1] && has[2]) {
        return;
    }

    let mut key = [
        letter_bound_fast(word, 0),
        letter_bound_fast(word, 1),
        letter_bound_fast(word, 2),
    ];
    key.sort_by(|a, b| a.cmp_desc(*b));

    let n = word.len();
    if let Some(cur) = out.get(&key)
        && cur.len() < n
    {
        return;
    }
    let s: String = word.iter().map(|&c| (b'A' + c) as char).collect();
    insert_shortest(out, key, s);
}

/// FKM pre-necklace recursion generating all primitive necklaces (Lyndon words) of length `n`
/// in 1-indexed array `a[1..=n]`.
fn fkm_gen(a: &mut [u8; 65], t: usize, p: usize, n: usize, out: &mut BoxMap) {
    if t > n {
        if p == n {
            visit(&a[1..=n], out);
        }
        return;
    }
    a[t] = a[t - p];
    fkm_gen(a, t + 1, p, n, out);
    for j in (a[t - p] + 1)..3 {
        a[t] = j;
        fkm_gen(a, t + 1, t, n, out);
    }
}

struct Job {
    a: [u8; 65],
    t: usize,
    p: usize,
    n: usize,
}

/// Splits the FKM search tree at `depth` into independent jobs for parallel execution.
fn collect_jobs(a: &mut [u8; 65], t: usize, p: usize, n: usize, depth: usize, jobs: &mut Vec<Job>) {
    if t > n || t > depth {
        jobs.push(Job { a: *a, t, p, n });
        return;
    }
    a[t] = a[t - p];
    collect_jobs(a, t + 1, p, n, depth, jobs);
    for j in (a[t - p] + 1)..3 {
        a[t] = j;
        collect_jobs(a, t + 1, t, n, depth, jobs);
    }
}

/// Canonicalizes each representative word (`repair_word`, as upstream `PlanPack::repair`) and
/// sorts by length, then lexicographically.
fn canonical_sorted(map: BoxMap) -> Vec<String> {
    let mut words: Vec<String> = map
        .into_values()
        .map(|w| repair_word(&w).expect("enumerated words use all of A, B, C"))
        .collect();
    words.sort_by(|x, y| x.len().cmp(&y.len()).then_with(|| x.cmp(y)));
    words
}

/// Exhaustively enumerates all primitive necklaces (Lyndon words) over `{A, B, C}` of lengths
/// `3..=max_len` in parallel and returns one shortest representative word per distinct
/// density-ordered dominance box.
pub fn enumerate_boxes(max_len: usize) -> Vec<String> {
    assert!(
        (3..=64).contains(&max_len),
        "max_len must be between 3 and 64"
    );

    let mut jobs = Vec::new();
    for n in 3..=max_len {
        let mut a = [0u8; 65];
        collect_jobs(&mut a, 1, 1, n, 8, &mut jobs);
    }

    let merged: BoxMap = jobs
        .into_par_iter()
        .fold(HashMap::new, |mut local, mut job| {
            fkm_gen(&mut job.a, job.t, job.p, job.n, &mut local);
            local
        })
        .reduce(HashMap::new, |mut acc, m| {
            for (k, w) in m {
                insert_shortest(&mut acc, k, w);
            }
            acc
        });

    canonical_sorted(merged)
}

/// Prefix state for sum-pruned DFS enumeration.
#[derive(Clone)]
struct PrunedState {
    w: [u8; 64],
    cnt: [usize; 3],
    pos: [[usize; 64]; 3],
    /// `gmax[c][l]`: largest closed distance to the `l`-th next occurrence of letter `c` so far.
    gmax: [[usize; 65]; 3],
    /// Upper bound on letter `c`'s density from closed gaps seen so far.
    ub: [f64; 3],
}

impl PrunedState {
    fn new() -> Self {
        Self {
            w: [0; 64],
            cnt: [0; 3],
            pos: [[0; 64]; 3],
            gmax: [[0; 65]; 3],
            ub: [2.0; 3],
        }
    }

    /// Sound upper bound on letter `c`'s final density bound across all completions of `w[..t]`.
    fn letter_ub(&self, c: usize, t: usize, n: usize) -> f64 {
        let mut u = ((self.cnt[c] + (n - t)) as f64) / (n as f64);
        if self.ub[c] < u {
            u = self.ub[c];
        }
        let k = self.cnt[c];
        for l in 1..=k {
            // Open gap: the next occurrence of letter `c` cannot appear before index `t`.
            let g = t - self.pos[c][k - l];
            if g > 1 {
                let b = (l as f64) / ((g - 1) as f64);
                if b < u {
                    u = b;
                }
            }
        }
        u
    }
}

#[derive(Clone, Copy)]
struct PrunedConfig {
    n: usize,
    sigma_num: i128,
    sigma_den: i128,
    sigma_f: f64,
}

fn finish_pruned(s: &PrunedState, cfg: PrunedConfig, out: &mut BoxMap) {
    if !(s.cnt[0] > 0 && s.cnt[1] > 0 && s.cnt[2] > 0) {
        return;
    }
    let word = &s.w[..cfg.n];
    let mut b = [
        letter_bound_fast(word, 0),
        letter_bound_fast(word, 1),
        letter_bound_fast(word, 2),
    ];
    // Exact check: u_1 + u_2 + u_3 >= sigma_num / sigma_den.
    let (n0, d0) = (b[0].num as i128, b[0].den as i128);
    let (n1, d1) = (b[1].num as i128, b[1].den as i128);
    let (n2, d2) = (b[2].num as i128, b[2].den as i128);
    let sum_num = n0 * d1 * d2 + n1 * d0 * d2 + n2 * d0 * d1;
    let sum_den = d0 * d1 * d2;
    if sum_num * cfg.sigma_den < cfg.sigma_num * sum_den {
        return;
    }

    b.sort_by(|x, y| x.cmp_desc(*y));
    if let Some(cur) = out.get(&b)
        && cur.len() < cfg.n
    {
        return;
    }
    let text: String = word.iter().map(|&c| (b'A' + c) as char).collect();
    insert_shortest(out, b, text);
}

fn pruned_dfs(s: &mut PrunedState, t: usize, max_letter: i32, cfg: PrunedConfig, out: &mut BoxMap) {
    if t == cfg.n {
        finish_pruned(s, cfg, out);
        return;
    }
    // Symmetry breaking: first letter is A, and B appears before C.
    let limit = (max_letter + 1).min(2) as usize;
    for c in 0..=limit {
        let k = s.cnt[c];
        let saved_ub = s.ub[c];
        let mut saved_g = [0usize; 65];
        saved_g[1..=k].copy_from_slice(&s.gmax[c][1..=k]);
        for l in 1..=k {
            let g = t - s.pos[c][k - l];
            if g > s.gmax[c][l] {
                s.gmax[c][l] = g;
            }
            if s.gmax[c][l] > 1 {
                let b = (l as f64) / ((s.gmax[c][l] - 1) as f64);
                if b < s.ub[c] {
                    s.ub[c] = b;
                }
            }
        }
        s.w[t] = c as u8;
        s.pos[c][k] = t;
        s.cnt[c] = k + 1;
        s.gmax[c][k + 1] = 0;

        let total = s.letter_ub(0, t + 1, cfg.n)
            + s.letter_ub(1, t + 1, cfg.n)
            + s.letter_ub(2, t + 1, cfg.n);
        if total >= cfg.sigma_f {
            pruned_dfs(s, t + 1, max_letter.max(c as i32), cfg, out);
        }

        s.cnt[c] = k;
        s.ub[c] = saved_ub;
        s.gmax[c][1..=k].copy_from_slice(&saved_g[1..=k]);
    }
}

struct PrunedJob {
    state: PrunedState,
    t: usize,
    max_letter: i32,
    n: usize,
}

fn collect_pruned_jobs(
    s: &mut PrunedState,
    t: usize,
    max_letter: i32,
    n: usize,
    depth: usize,
    sigma_f: f64,
    jobs: &mut Vec<PrunedJob>,
) {
    if t == depth || t == n {
        jobs.push(PrunedJob {
            state: s.clone(),
            t,
            max_letter,
            n,
        });
        return;
    }
    let limit = (max_letter + 1).min(2) as usize;
    for c in 0..=limit {
        let k = s.cnt[c];
        let saved_ub = s.ub[c];
        let mut saved_g = [0usize; 65];
        saved_g[1..=k].copy_from_slice(&s.gmax[c][1..=k]);
        for l in 1..=k {
            let g = t - s.pos[c][k - l];
            if g > s.gmax[c][l] {
                s.gmax[c][l] = g;
            }
            if s.gmax[c][l] > 1 {
                let b = (l as f64) / ((s.gmax[c][l] - 1) as f64);
                if b < s.ub[c] {
                    s.ub[c] = b;
                }
            }
        }
        s.w[t] = c as u8;
        s.pos[c][k] = t;
        s.cnt[c] = k + 1;
        s.gmax[c][k + 1] = 0;

        let total = s.letter_ub(0, t + 1, n) + s.letter_ub(1, t + 1, n) + s.letter_ub(2, t + 1, n);
        if total >= sigma_f {
            collect_pruned_jobs(s, t + 1, max_letter.max(c as i32), n, depth, sigma_f, jobs);
        }

        s.cnt[c] = k;
        s.ub[c] = saved_ub;
        s.gmax[c][1..=k].copy_from_slice(&saved_g[1..=k]);
    }
}

/// Pruned enumeration of words of lengths `3..=max_len` whose dominance box satisfies
/// `u_1 + u_2 + u_3 >= sigma_num / sigma_den`.
///
/// The pruning is a heuristic for the cover search: nothing proves that every inclusion-maximal
/// box has a large bound sum. Empirically, with `sigma = 19/20` the maximal boxes coincide with
/// exhaustive enumeration for every tested length up to 24 (233 maximal boxes in `D_3` at length
/// 24), while enumeration takes ~4 s (and full `upper solve` ~10-20 s) instead of ~15 min on a
/// 10-core machine.
pub fn enumerate_boxes_pruned(max_len: usize, sigma_num: i128, sigma_den: i128) -> Vec<String> {
    assert!(
        (3..=64).contains(&max_len),
        "max_len must be between 3 and 64"
    );
    assert!(sigma_den > 0, "sigma_den must be positive");
    let sigma_f = (sigma_num as f64) / (sigma_den as f64) - 1e-9;

    let mut jobs = Vec::new();
    // Schedule longer lengths first and split deeper so heavy subtrees distribute evenly across cores.
    for n in (3..=max_len).rev() {
        let mut s0 = PrunedState::new();
        collect_pruned_jobs(&mut s0, 0, -1, n, n.min(13), sigma_f, &mut jobs);
    }

    let merged: BoxMap = jobs
        .into_par_iter()
        .fold(HashMap::new, |mut local, mut job| {
            let cfg = PrunedConfig {
                n: job.n,
                sigma_num,
                sigma_den,
                sigma_f,
            };
            pruned_dfs(&mut job.state, job.t, job.max_letter, cfg, &mut local);
            local
        })
        .reduce(HashMap::new, |mut acc, m| {
            for (k, w) in m {
                insert_shortest(&mut acc, k, w);
            }
            acc
        });

    canonical_sorted(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::word::dominance_box;
    use crate::upper::cells::build_pool;

    #[test]
    fn enumerate_len_12_matches_reference_count() {
        let words = enumerate_boxes(12);
        assert_eq!(words.len(), 1084);
        assert_eq!(words[0], "ABC");
    }

    #[test]
    fn pruned_matches_all_maximal_boxes_up_to_len_14() {
        for max_len in [12, 14] {
            let full = build_pool(&enumerate_boxes(max_len)).unwrap();
            let pruned = build_pool(&enumerate_boxes_pruned(max_len, 19, 20)).unwrap();
            let full_keys: Vec<_> = full.items.iter().map(|i| i.capped).collect();
            let pruned_keys: Vec<_> = pruned.items.iter().map(|i| i.capped).collect();
            assert_eq!(pruned_keys, full_keys, "max_len = {max_len}");
        }
    }

    #[test]
    fn enumerated_words_are_canonical() {
        for w in enumerate_boxes(10)
            .into_iter()
            .chain(enumerate_boxes_pruned(10, 19, 20))
        {
            assert_eq!(repair_word(&w).unwrap(), w);
            assert!(crate::model::word::is_density_ordered(
                &dominance_box(&w).unwrap()
            ));
        }
    }

    #[test]
    fn fast_bound_matches_model_dominance_box() {
        for w in enumerate_boxes(10) {
            let exact = dominance_box(&w).unwrap();
            let bytes: Vec<u8> = w.bytes().map(|c| c - b'A').collect();
            for (c, exact_bound) in exact.iter().enumerate() {
                let fast = letter_bound_fast(&bytes, c as u8);
                assert_eq!(*exact_bound.value.numer(), fast.num as i128);
                assert_eq!(*exact_bound.value.denom(), fast.den as i128);
                assert_eq!(exact_bound.strict, fast.strict);
            }
        }
    }
}
