use crate::model::bound::{Rat, UpperBound};

// 仕事数3 の Dominance Box B(w) = {u_A, u_B, u_C}
pub type Box3 = [UpperBound; 3];

/// `word` の中の １文字 `letter` に対する密度上界を計算巣いる (論文 eq. (2))
///
///   u = min ( <= k/n, min_{1 <= l <= k} < l / (G(l) - 1) )
///
/// ここで k は出現回数、G(l) は円環上で l 回先の出現までの最大距離
pub fn task_bound(word: &[u8], leter: u8) -> Result<UpperBound, String> {
    let n = word.len();
    let pos: Vec<usize> = word
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| (c == leter).then_some(i))
        .collect();
    let k = pos.len();
    if k == 0 {
        return Err(format!(
            "letter `{}` does not appear in word",
            leter as char
        ));
    }

    // ２周分を並べて円環上の距離をそのまま引けるようにする
    let mut ext = Vec::with_capacity(2 * k);
    ext.extend_from_slice(&pos);
    ext.extend(pos.iter().map(|&p| p + n));

    let mut best = UpperBound::closed(Rat::new(k as i128, n as i128));
    for l in 1..=k {
        let g = (0..k).map(|j| ext[j + l] - ext[j]).max().unwrap();
        if g > 1 {
            let cand = UpperBound::strict(Rat::new(l as i128, (g - 1) as i128));
            best = best.min(cand);
        }
    }
    Ok(best)
}

/// {A, B, C} をすべて含む語 `word` の Dominance Box B(w) を計算する
pub fn dominance_box(word: &str) -> Result<Box3, String> {
    let bytes = word.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(|&b| matches!(b, b'A' | b'B' | b'C')) {
        return Err(format!("word must be non-empty over {{A, B, C}}: {word}"));
    }
    Ok([
        task_bound(bytes, b'A')?,
        task_bound(bytes, b'B')?,
        task_bound(bytes, b'C')?,
    ])
}

/// 上界が高順 ( u_1 >= u_2 >= u3) になっているか
pub fn is_density_ordered(b: &Box3) -> bool {
    b[0] >= b[1] && b[1] >= b[2]
}

/// 上界が大きい順に A, B, C とラベルを付け替え、回転の中で辞書順最小の代表語を返す
pub fn relabel(word: &str) -> Result<String, String> {
    let b = dominance_box(word)?;
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| b[j].cmp(&b[i]).then_with(|| i.cmp(&j)));

    // map[元の文字(0..3)] = 新しい文字(b'A'..=b'C')
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

// `WORD,b1,b2,b3` 形式のファイルをパーズする
pub fn parse_certificate(content: &str) -> Result<Vec<(String, Box3)>, String> {
    let mut out = Vec::new();
    for (lineno, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 4 {
            return Err(format!(
                "line {}: expected 4 comma-separeted fileds",
                lineno + 1
            ));
        }
        let word = parts[0].trim().to_string();
        let b = [parts[1].parse()?, parts[2].parse()?, parts[3].parse()?];
        out.push((word, b));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ababac_box_and_relabel() {
        let b = dominance_box("ABABAC").unwrap();
        assert_eq!(format!("{},{},{}", b[0], b[1], b[2]), "<=1/2,<1/3,<=1/6");
        assert!(is_density_ordered(&b));
        assert_eq!(relabel("BCBCBA").unwrap(), "ABABAC");
    }

    #[test]
    fn matches_11word_certificate() {
        let cert = "\
AABACAABACABAACABAACABAC,<=7/12,<=5/24,<=5/24
AABCAABCABACABACABACB,<7/15,<2/7,<=5/21
AABCABCABACBABCABC,<3/8,<1/3,<=5/18
AABCABABACBABACB,<3/7,<3/8,<=3/16
AAAAABAAAAAC,<=5/6,<=1/12,<=1/12
AAAABAAACAAB,<3/4,<1/6,<=1/12
AABABAABACAB,<=7/12,<1/3,<=1/12
AABABABACBAB,<1/2,<=5/12,<=1/12
AAABAACAB,<2/3,<=2/9,<=1/9
AABAABAC,<=5/8,<1/4,<=1/8
ABABAC,<=1/2,<1/3,<=1/6
";
        for (word, expected) in parse_certificate(cert).unwrap() {
            let actual = dominance_box(&word).unwrap();
            assert_eq!(actual, expected, "mismatch for {word}");
            assert!(
                is_density_ordered(&actual),
                "not density-ordered:
            {word}"
            );
        }
    }
}
