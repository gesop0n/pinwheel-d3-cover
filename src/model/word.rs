use std::fmt;

use crate::model::bound::{DensityVector, Fraction, UpperBound};

/// Dominance box `B(w)` of a 3-task periodic schedule: coordinate upper bounds for tasks A, B, C.
pub type DominanceBox = [UpperBound; 3];

/// A periodic schedule represented by a finite word `w` over `{A, B, C}` together with its
/// dominance box `density_bounds`.
///
/// Matches `PeriodicSchedule` in the paper's `pinwheel_scheduling_packing_prover.py` and
/// `PlanPack` in `cpp_src/lib/plan_pack.hpp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodicSchedule {
    pub word: String,
    pub density_bounds: DominanceBox,
}

impl PeriodicSchedule {
    /// Constructs a `PeriodicSchedule` by computing the dominance box directly from `word`.
    pub fn from_word(word: &str) -> Result<Self, String> {
        let density_bounds = dominance_box(word)?;
        Ok(Self {
            word: word.to_string(),
            density_bounds,
        })
    }

    /// Returns a canonicalized copy: tasks are relabeled in descending order of `density_bounds`
    /// and the word is rotated to its lexicographically smallest cyclic shift
    /// (`PlanPack::repair` in the paper's `cpp_src/lib/plan_pack.hpp`).
    pub fn canonicalize(&self) -> Result<Self, String> {
        let word = repair_word(&self.word)?;
        Self::from_word(&word)
    }

    /// Returns true iff this schedule's dominance box contains `density_vector`
    /// (i.e., the periodic schedule `S_w` dominates that density vector).
    pub fn dominates(&self, density_vector: &DensityVector) -> bool {
        box_dominates(&self.density_bounds, density_vector)
    }
}

impl fmt::Display for PeriodicSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{},{}",
            self.word, self.density_bounds[0], self.density_bounds[1], self.density_bounds[2]
        )
    }
}

/// Returns true iff every coordinate of `density_vector` satisfies the corresponding upper bound.
pub fn box_dominates(bounds: &DominanceBox, density_vector: &DensityVector) -> bool {
    bounds[0].satisfies(density_vector[0])
        && bounds[1].satisfies(density_vector[1])
        && bounds[2].satisfies(density_vector[2])
}

/// Computes the admissible density upper bound for a single task `letter` in periodic word `word`
/// (paper Section 3, eq. (2); equivalent to `Schedule::get_upper_bound` in the paper's
/// `cpp_src/lib/schedule.hpp`).
///
/// A schedule is valid for task period `a` (density `d = 1/a`) iff for every `l >= 1`, every
/// window of `ceil(l * a)` consecutive days contains at least `l` occurrences of the task.
/// Let `G(l)` be the maximum cyclic distance from an occurrence to its `l`-th following occurrence.
/// Then validity is equivalent to `ceil(l * a) >= G(l)` for all `l`, i.e., `d < l / (G(l) - 1)`.
/// Only `1 <= l <= k` matter (where `k` is the occurrence count in one period of length `n`),
/// while `l -> infinity` yields the inclusive average frequency bound `d <= k / n`.
pub fn task_upper_bound(word: &[u8], letter: u8) -> Result<UpperBound, String> {
    let n = word.len();
    let pos: Vec<usize> = word
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| (c == letter).then_some(i))
        .collect();
    let k = pos.len();
    if k == 0 {
        return Err(format!(
            "letter '{}' does not appear in word",
            letter as char
        ));
    }

    // Duplicate positions across two periods so cyclic gaps are direct differences.
    let mut ext = Vec::with_capacity(2 * k);
    ext.extend_from_slice(&pos);
    ext.extend(pos.iter().map(|&p| p + n));

    let mut best = UpperBound::inclusive(Fraction::new(k as i128, n as i128));
    for l in 1..=k {
        let g = (0..k).map(|j| ext[j + l] - ext[j]).max().unwrap();
        if g > 1 {
            let cand = UpperBound::strict(Fraction::new(l as i128, (g - 1) as i128));
            best = best.min(cand);
        }
    }
    Ok(best)
}

/// Computes the dominance box `B(w)` for a word `w` using all three letters `{A, B, C}`.
pub fn dominance_box(word: &str) -> Result<DominanceBox, String> {
    let bytes = word.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(|&b| matches!(b, b'A' | b'B' | b'C')) {
        return Err(format!("word must be non-empty over {{A, B, C}}: {word}"));
    }
    Ok([
        task_upper_bound(bytes, b'A')?,
        task_upper_bound(bytes, b'B')?,
        task_upper_bound(bytes, b'C')?,
    ])
}

/// Returns true iff `bounds` are in descending (density-ordered) order: `u_1 >= u_2 >= u_3`.
pub fn is_density_ordered(bounds: &DominanceBox) -> bool {
    bounds[0] >= bounds[1] && bounds[1] >= bounds[2]
}

/// Relabels `{A, B, C}` in descending order of their density upper bounds and rotates the word to
/// its lexicographically smallest cyclic permutation.
///
/// Equivalent to `PlanPack::repair` in the paper's `cpp_src/lib/plan_pack.hpp`.
pub fn repair_word(word: &str) -> Result<String, String> {
    let b = dominance_box(word)?;
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| b[j].cmp(&b[i]).then_with(|| i.cmp(&j)));

    // map[original letter 0..3] = new letter (b'A'..=b'C')
    let mut map = [0u8; 3];
    for (rank, &orig) in order.iter().enumerate() {
        map[orig] = b'A' + rank as u8;
    }

    let mapped: Vec<u8> = word.bytes().map(|c| map[(c - b'A') as usize]).collect();
    let n = mapped.len();
    let best = (0..n)
        .map(|shift| {
            let mut rot = Vec::with_capacity(n);
            rot.extend_from_slice(&mapped[shift..]);
            rot.extend_from_slice(&mapped[..shift]);
            rot
        })
        .min()
        .unwrap();

    Ok(String::from_utf8(best).unwrap())
}

/// Parses a certificate file in the paper's format (`WORD,b1,b2,b3`, ignoring empty and `#` lines).
pub fn parse_certificate(content: &str) -> Result<Vec<PeriodicSchedule>, String> {
    let mut schedules = Vec::new();
    for (lineno, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 4 {
            return Err(format!(
                "line {}: expected 4 comma-separated fields (word,b1,b2,b3)",
                lineno + 1
            ));
        }
        let word = parts[0].trim().to_string();
        let density_bounds = [parts[1].parse()?, parts[2].parse()?, parts[3].parse()?];
        schedules.push(PeriodicSchedule {
            word,
            density_bounds,
        });
    }
    Ok(schedules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_example_and_repair() {
        // Paper README example: AABACB,<1/2,<=1/3,<=1/6
        let ex = PeriodicSchedule::from_word("AABACB").unwrap();
        assert_eq!(ex.to_string(), "AABACB,<1/2,<=1/3,<=1/6");

        let b = dominance_box("ABABAC").unwrap();
        assert_eq!(format!("{},{},{}", b[0], b[1], b[2]), "<=1/2,<1/3,<=1/6");
        assert!(is_density_ordered(&b));
        assert_eq!(repair_word("BCBCBA").unwrap(), "ABABAC");
    }

    #[test]
    fn canonicalize_matches_paper_quality_tool() {
        // Output of the paper's `packing_schedule_quality BCBCBA ABCABC`.
        let c = |w: &str| {
            PeriodicSchedule::from_word(w)
                .unwrap()
                .canonicalize()
                .unwrap()
                .to_string()
        };
        assert_eq!(c("BCBCBA"), "ABABAC,<=1/2,<1/3,<=1/6");
        assert_eq!(c("ABCABC"), "ABCABC,<=1/3,<=1/3,<=1/3");
        assert_eq!(c("AABACB"), "AABACB,<1/2,<=1/3,<=1/6");
    }

    #[test]
    fn repair_word_tie_breaking_is_deterministic() {
        // All three tasks have equal upper bound <=1/3: relabeling preserves original letter order
        // (A->A, B->B, C->C) and rotates to the lexicographically smallest cyclic shift.
        assert_eq!(repair_word("CBA").unwrap(), "ACB");
        assert_eq!(repair_word("BCA").unwrap(), "ABC");

        // Tasks A and C tie at <=1/4 while B has <=1/2: B maps to A, and the tie between A and C
        // keeps original letter order (A->B, C->C), followed by smallest cyclic rotation.
        assert_eq!(repair_word("CBAB").unwrap(), "ABAC");
        assert_eq!(repair_word("BACB").unwrap(), "AABC");
        assert_eq!(repair_word("BCAB").unwrap(), "AACB");
    }
}
