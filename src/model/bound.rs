use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use num_rational::Ratio;

/// Exact rational number, matching `Fraction` in the paper's repository
/// (`grzegorz-gutowski/pinwheel_scheduling`).
///
/// Uses `i128` to avoid intermediate overflow during Fourier–Motzkin elimination.
pub type Fraction = Ratio<i128>;

/// Ordered task density vector `d = (d_1, d_2, d_3)` with `d_1 >= d_2 >= d_3 > 0`.
pub type DensityVector = [Fraction; 3];

/// Coordinate upper bound of a dominance box: either `d < value` (strict) or `d <= value` (inclusive).
///
/// Matches `UpperBound` in the paper's `cpp_src/lib/bound.hpp` and
/// `pinwheel_scheduling_packing_prover.py`.
///
/// Ordering (`Ord`) reflects interval inclusion (tighter bound is smaller):
///   - smaller `value` is tighter (`1/3 < 1/2`),
///   - on equal `value`, strict `< v` is tighter than inclusive `<= v` (`(< v) < (<= v)`).
///
/// Note: the paper's `cpp_src/lib/bound.hpp` reverses `operator<` so that `std::max` picks the
/// tightest bound. Here we keep the natural inclusion order so `.min()` picks the tightest bound
/// and descending sort places wider bounds first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpperBound {
    pub value: Fraction,
    pub strict: bool,
}

impl UpperBound {
    pub fn inclusive(value: Fraction) -> Self {
        Self {
            value,
            strict: false,
        }
    }

    pub fn strict(value: Fraction) -> Self {
        Self {
            value,
            strict: true,
        }
    }

    /// Returns true iff density `d` satisfies this bound (`UpperBound::get_satisfied` in the
    /// paper's C++ implementation).
    pub fn satisfies(&self, d: Fraction) -> bool {
        if self.strict {
            d < self.value
        } else {
            d <= self.value
        }
    }
}

impl Ord for UpperBound {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value
            .cmp(&other.value)
            // On ties, strict (<) is tighter than inclusive (<=).
            .then_with(|| other.strict.cmp(&self.strict))
    }
}

impl PartialOrd for UpperBound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for UpperBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = if self.strict { "<" } else { "<=" };
        write!(f, "{op}{}", self.value)
    }
}

impl FromStr for UpperBound {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (strict, rest) = if let Some(r) = s.strip_prefix("<=") {
            (false, r)
        } else if let Some(r) = s.strip_prefix('<') {
            (true, r)
        } else {
            return Err(format!(
                "expected upper bound starting with '<' or '<=': {s}"
            ));
        };
        let value = parse_fraction(rest)?;
        Ok(Self { value, strict })
    }
}

/// Parses an exact fraction (`"p/q"` or integer `"p"`).
pub fn parse_fraction(s: &str) -> Result<Fraction, String> {
    let s = s.trim();
    if let Some((num, den)) = s.split_once('/') {
        let n: i128 = num
            .trim()
            .parse()
            .map_err(|e| format!("invalid numerator in '{s}': {e}"))?;
        let d: i128 = den
            .trim()
            .parse()
            .map_err(|e| format!("invalid denominator in '{s}': {e}"))?;
        if d == 0 {
            return Err(format!("zero denominator in '{s}'"));
        }
        Ok(Fraction::new(n, d))
    } else {
        let n: i128 = s
            .parse()
            .map_err(|e| format!("invalid integer '{s}': {e}"))?;
        Ok(Fraction::from_integer(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_reflects_interval_inclusion() {
        let third = Fraction::new(1, 3);
        let half = Fraction::new(1, 2);
        assert!(UpperBound::inclusive(third) < UpperBound::strict(half));
        assert!(UpperBound::strict(half) < UpperBound::inclusive(half));
        assert_eq!(
            UpperBound::strict(half).min(UpperBound::inclusive(half)),
            UpperBound::strict(half)
        );
        assert!(UpperBound::inclusive(half).satisfies(half));
        assert!(!UpperBound::strict(half).satisfies(half));
    }

    #[test]
    fn parses_and_prints_paper_format() {
        for text in ["<1/2", "<=5/12"] {
            assert_eq!(text.parse::<UpperBound>().unwrap().to_string(), text);
        }
        assert!("1/2".parse::<UpperBound>().is_err());
        assert!(parse_fraction("1/0").is_err());
    }
}
