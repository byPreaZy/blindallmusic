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
        assert_eq!(php_levenshtein("et", "été"), 4);
    }

    #[test]
    fn over_255_returns_minus_one() {
        let long = "a".repeat(256);
        assert_eq!(php_levenshtein(&long, "b"), -1);
        assert!(php_levenshtein(&long, "b") < 3);
    }
}
