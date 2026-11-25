use std::{fmt, str::FromStr};

use num_rational::Ratio;

// 厳密な有理数型
pub type Rat = Ratio<i128>;

// 密度の上界
// (`d < value` または `d <= value`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpperBound {
    pub value: Rat,
    pub strict: bool,
}

impl UpperBound {
    pub fn closed(value: Rat) -> Self {
        Self {
            value,
            strict: false,
        }
    }

    pub fn strict(value: Rat) -> Self {
        Self {
            value,
            strict: true,
        }
    }

    // 密度 `d` がこの上界を満たすか
    pub fn contains(&self, d: Rat) -> bool {
        if self.strict {
            d < self.value
        } else {
            d <= self.value
        }
    }
}

impl Ord for UpperBound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value
            .cmp(&other.value)
            // 同値なら strict = true (<) の方が strict = false (<=) より小さい
            .then_with(|| other.strict.cmp(&self.strict))
    }
}

impl PartialOrd for UpperBound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
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
        } else if let Some(r) = s.strip_prefix("<") {
            (true, r)
        } else {
            return Err(format!("invalid bound (expected '<' or '<='): {s}"));
        };
        let value = parse_rat(rest)?;
        Ok(Self { value, strict })
    }
}

pub fn parse_rat(s: &str) -> Result<Rat, String> {
    let s = s.trim();
    if let Some((num, den)) = s.split_once('/') {
        let n: i128 = num
            .trim()
            .parse()
            .map_err(|e| format!("bad numer '{s}': {e}"))?;
        let d: i128 = den
            .trim()
            .parse()
            .map_err(|e| format!("bad denom '{s}': {e}"))?;
        if d == 0 {
            return Err(format!("zero denominator: {s}"));
        }
        Ok(Rat::new(n, d))
    } else {
        let n: i128 = s.parse().map_err(|e| format!("bad integer '{s}': {e}"))?;
        Ok(Rat::from_integer(n))
    }
}
