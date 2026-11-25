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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::word::dominance_box;

    #[test]
    fn enumerate_len_12_matches_reference_count() {
        // pinwheel-test (enum_boxes.cpp) の既知のリファレンス値: L=12 で 1084 箱
        let words = enumerate_boxes(12);
        assert_eq!(words.len(), 1084);
        assert_eq!(words[0], "ABC");
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
