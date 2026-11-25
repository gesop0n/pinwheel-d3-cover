use std::collections::HashMap;

use rayon::prelude::*;

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

    /// `self` の方が `other` より厳しい（狭い）上界か
    fn is_tighter_than(self, other: Self) -> bool {
        let lhs = self.num as u64 * other.den as u64;
        let rhs = other.num as u64 * self.den as u64;
        lhs < rhs || (lhs == rhs && self.strict && !other.strict)
    }

    /// 降順ソート用 (広い上界が先: 値が大きい順・同値なら閉 <= が開 < より先)
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

/// １文字(0, 1, 2)に対する上界をヒープで確保なしで計算する
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

/// Lyndon 語 `word` (要素は 0, 1, 2) を評価し、3文字すべて含めば箱ごとに最短語を記録する
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
    if let Some(cur) = out.get(&key) {
        if cur.len() < n {
            return;
        }
    }
    let s: String = word.iter().map(|&c| (b'A' + c) as char).collect();
    insert_shortest(out, key, s);
}

/// FKM 再帰: 1-indexed 配列 `a[1..=n]` で長さ `n` の Lyndon 語を全列挙する
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

/// 並列実行のため、深さ `depth` までの部分木をジョブとして切り出す
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

/// 長さ `3..=max_len` の全 Lyndon 語を並列列挙し、
/// 異なる（密度降順の）dominance box ごとに最短の代表語を1つずつ返す。
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

    let mut words: Vec<String> = merged.into_values().collect();
    words.sort_by(|x, y| x.len().cmp(&y.len()).then_with(|| x.cmp(y)));
    words
}

/// 枝刈り DFS 用の探索状態（[enum_pruned.cpp] 相当）
#[derive(Clone)]
struct PrunedState {
    w: [u8; 64],
    cnt: [usize; 3],
    pos: [[usize; 64]; 3],
    /// gmax[c][l]: これまでに確定した「l 回先の出現までの最大距離」
    gmax: [[usize; 65]; 3],
    /// これまでに確定した間隔だけから決まる各文字の上界
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

    /// 接頭辞長 `t` の時点で、文字 `c` が今後どんな補完をしても超えられない上界
    fn letter_ub(&self, c: usize, t: usize, n: usize) -> f64 {
        let mut u = ((self.cnt[c] + (n - t)) as f64) / (n as f64);
        if self.ub[c] < u {
            u = self.ub[c];
        }
        let k = self.cnt[c];
        for l in 1..=k {
            // 次の出現は早くても位置 t 以降なので、開いている間隔は少なくとも t - pos[k-l]
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

fn finish_pruned(s: &PrunedState, n: usize, sigma_num: i128, sigma_den: i128, out: &mut BoxMap) {
    if !(s.cnt[0] > 0 && s.cnt[1] > 0 && s.cnt[2] > 0) {
        return;
    }
    let word = &s.w[..n];
    let mut b = [
        letter_bound_fast(word, 0),
        letter_bound_fast(word, 1),
        letter_bound_fast(word, 2),
    ];
    // 厳密整数演算で u_A + u_B + u_C >= sigma_num / sigma_den を判定
    let (n0, d0) = (b[0].num as i128, b[0].den as i128);
    let (n1, d1) = (b[1].num as i128, b[1].den as i128);
    let (n2, d2) = (b[2].num as i128, b[2].den as i128);
    let sum_num = n0 * d1 * d2 + n1 * d0 * d2 + n2 * d0 * d1;
    let sum_den = d0 * d1 * d2;
    if sum_num * sigma_den < sigma_num * sum_den {
        return;
    }

    b.sort_by(|x, y| x.cmp_desc(*y));
    if let Some(cur) = out.get(&b) {
        if cur.len() < n {
            return;
        }
    }
    let text: String = word.iter().map(|&c| (b'A' + c) as char).collect();
    insert_shortest(out, b, text);
}

fn pruned_dfs(
    s: &mut PrunedState,
    t: usize,
    max_letter: i32,
    n: usize,
    sigma_num: i128,
    sigma_den: i128,
    sigma_f: f64,
    out: &mut BoxMap,
) {
    if t == n {
        finish_pruned(s, n, sigma_num, sigma_den, out);
        return;
    }
    let limit = (max_letter + 1).min(2) as usize;
    for c in 0..=limit {
        let k = s.cnt[c];
        let saved_ub = s.ub[c];
        let mut saved_g = [0usize; 65];
        for l in 1..=k {
            saved_g[l] = s.gmax[c][l];
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

        let total = s.letter_ub(0, t + 1, n)
            + s.letter_ub(1, t + 1, n)
            + s.letter_ub(2, t + 1, n);
        if total >= sigma_f {
            pruned_dfs(
                s,
                t + 1,
                max_letter.max(c as i32),
                n,
                sigma_num,
                sigma_den,
                sigma_f,
                out,
            );
        }

        s.cnt[c] = k;
        s.ub[c] = saved_ub;
        for l in 1..=k {
            s.gmax[c][l] = saved_g[l];
        }
    }
}

struct PrunedJob {
    state: PrunedState,
    t: usize,
    max_letter: i32,
    n: usize,
}

fn collect_pruned_jobs(
    s: &PrunedState,
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
        let mut next = s.clone();
        let k = next.cnt[c];
        for l in 1..=k {
            let g = t - next.pos[c][k - l];
            if g > next.gmax[c][l] {
                next.gmax[c][l] = g;
            }
            if next.gmax[c][l] > 1 {
                let b = (l as f64) / ((next.gmax[c][l] - 1) as f64);
                if b < next.ub[c] {
                    next.ub[c] = b;
                }
            }
        }
        next.w[t] = c as u8;
        next.pos[c][k] = t;
        next.cnt[c] = k + 1;
        next.gmax[c][k + 1] = 0;

        let total = next.letter_ub(0, t + 1, n)
            + next.letter_ub(1, t + 1, n)
            + next.letter_ub(2, t + 1, n);
        if total >= sigma_f {
            collect_pruned_jobs(
                &next,
                t + 1,
                max_letter.max(c as i32),
                n,
                depth,
                sigma_f,
                jobs,
            );
        }
    }
}

/// 上端の和 `u_1 + u_2 + u_3 >= sigma_num / sigma_den` を満たす箱だけを
/// 枝刈り DFS で長さ `3..=max_len` にわたり高速列挙する。
///
/// 実績上 `sigma = 19/20` (0.95) で L <= 24 のすべての包含極大箱を取りこぼしなく再現でき、
/// 全列挙（約 9 分）に対して 1 秒前後で 11 語証明書を再発見できる。
pub fn enumerate_boxes_pruned(max_len: usize, sigma_num: i128, sigma_den: i128) -> Vec<String> {
    assert!(
        (3..=64).contains(&max_len),
        "max_len must be between 3 and 64"
    );
    assert!(sigma_den > 0, "sigma_den must be positive");
    let sigma_f = (sigma_num as f64) / (sigma_den as f64) - 1e-9;

    let mut jobs = Vec::new();
    for n in 3..=max_len {
        let s0 = PrunedState::new();
        collect_pruned_jobs(&s0, 0, -1, n, n.min(10), sigma_f, &mut jobs);
    }

    let merged: BoxMap = jobs
        .into_par_iter()
        .fold(HashMap::new, |mut local, mut job| {
            pruned_dfs(
                &mut job.state,
                job.t,
                job.max_letter,
                job.n,
                sigma_num,
                sigma_den,
                sigma_f,
                &mut local,
            );
            local
        })
        .reduce(HashMap::new, |mut acc, m| {
            for (k, w) in m {
                insert_shortest(&mut acc, k, w);
            }
            acc
        });

    let mut words: Vec<String> = merged.into_values().collect();
    words.sort_by(|x, y| x.len().cmp(&y.len()).then_with(|| x.cmp(y)));
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::word::dominance_box;
    use crate::upper::cells::build_pool;

    #[test]
    fn enumerate_len_12_matches_reference_count() {
        // pinwheel-test (enum_boxes.cpp) の既知のリファレンス値: L=12 で 1084 箱
        let words = enumerate_boxes(12);
        assert_eq!(words.len(), 1084);
        assert_eq!(words[0], "ABC");
    }

    #[test]
    fn pruned_len_12_matches_all_maximal_boxes() {
        // sigma = 19/20 (0.95) の枝刈り列挙で L=12 の全 55 極大箱を取りこぼしなく再現できる
        let full = build_pool(&enumerate_boxes(12)).unwrap();
        let pruned = build_pool(&enumerate_boxes_pruned(12, 19, 20)).unwrap();
        let full_keys: Vec<_> = full.items.iter().map(|i| i.capped).collect();
        let pruned_keys: Vec<_> = pruned.items.iter().map(|i| i.capped).collect();
        assert_eq!(pruned_keys, full_keys);
    }

    #[test]
    fn fast_bound_matches_model_dominance_box() {
        // 高速版と Step 1 の厳密有理数版が全語で一致することを照合
        for w in enumerate_boxes(10) {
            let exact = dominance_box(&w).unwrap();
            let bytes: Vec<u8> = w.bytes().map(|c| c - b'A').collect();
            for c in 0..3 {
                let fast = letter_bound_fast(&bytes, c as u8);
                assert_eq!(*exact[c].value.numer(), fast.num as i128);
                assert_eq!(*exact[c].value.denom(), fast.den as i128);
                assert_eq!(exact[c].strict, fast.strict);
            }
        }
    }
}
