//! PHP-compatible `levenshtein()` — **byte-level** distance.
//!
//! PHP's levenshtein operates on bytes, not chars, and returns -1 when either
//! string exceeds 255 bytes. Both behaviors are reproduced because callers
//! rely on them (a -1 return satisfies `-1 < 3`, i.e. overlong inputs match —
//! the PHP quirk is preserved verbatim).

/// Returns the PHP levenshtein distance, or -1 if a side exceeds 255 bytes.
pub fn php_levenshtein(a: &str, b: &str) -> i64 {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() > 255 || b.len() > 255 {
        return -1;
    }
    levenshtein_bytes(a, b) as i64
}

fn levenshtein_bytes(a: &[u8], b: &[u8]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // Two-row DP, classic insert/delete/substitute cost 1.
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j] + cost).min((prev[j + 1] + 1).min(curr[j] + 1));
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_distances() {
        assert_eq!(php_levenshtein("kitten", "sitting"), 3);
        assert_eq!(php_levenshtein("abc", "abc"), 0);
        assert_eq!(php_levenshtein("", "abc"), 3);
        assert_eq!(php_levenshtein("flaw", "lawn"), 2);
    }

    #[test]
    fn byte_level_semantics() {
        // 'é' is 2 bytes in UTF-8 — PHP sees bytes, not chars.
        // "et" vs "été": byte distance = 4 (a char-level impl would say 2).
        assert_eq!(php_levenshtein("et", "été"), 4);
    }

    #[test]
    fn over_255_returns_minus_one() {
        let long = "a".repeat(256);
        assert_eq!(php_levenshtein(&long, "b"), -1);
        // and therefore satisfies PHP's `-1 < 3` match quirk.
        assert!(php_levenshtein(&long, "b") < 3);
    }
}
