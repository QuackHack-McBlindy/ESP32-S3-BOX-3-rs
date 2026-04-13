#![no_std]

//! BARELY_FUZZY 
//! Fuzzy matching designed for bare metal no_std environments. 
//! Based on Levenshtein distance with heap allocation.
//!
//! provides:
//! - `trigram_similarity`: dice coefficient on character trigrams
//! - `levenshtein`: Classic edit distance.
//! - `levenshtein_similarity`: percentage score with first‑character bonus.
//! - `best_fuz`: two‑stage filter matcher (requires `alloc` feature)
//! 
//! all functions operate on byte slices (`&[u8]`) for maximum flexibility.
//! if you need Unicode‑aware trigrams, convert to `char`s first.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;


/// TRIGRAM SIMILARITY - between two byte slices.
/// without `alloc`: O(n*m) double loop no heap memory.
/// with `alloc`: builds two `Vec<[u8;3]>` for faster matching
#[cfg(not(feature = "alloc"))]
pub fn trigram_similarity(a: &[u8], b: &[u8]) -> u8 {
    if a.len() < 3 || b.len() < 3 {
        return 0;
    }
    let a_trigrams = a.len() - 2;
    let b_trigrams = b.len() - 2;
    let mut matches = 0;
    for i in 0..a_trigrams {
        let tri_a = &a[i..i + 3];
        for j in 0..b_trigrams {
            if tri_a == &b[j..j + 3] {
                matches += 1;
                break;
            }
        }
    }
    let total = a_trigrams + b_trigrams;
    if total == 0 {
        0
    } else {
        ((100 * 2 * matches) / total) as u8
    }
}

#[cfg(feature = "alloc")]
pub fn trigram_similarity(a: &[u8], b: &[u8]) -> u8 {
    if a.len() < 3 || b.len() < 3 {
        return 0;
    }
    let mut tri1 = Vec::new();
    let mut tri2 = Vec::new();
    for i in 0..a.len() - 2 {
        tri1.push([a[i], a[i + 1], a[i + 2]]);
    }
    for i in 0..b.len() - 2 {
        tri2.push([b[i], b[i + 1], b[i + 2]]);
    }
    let matches = tri1.iter().filter(|t| tri2.contains(t)).count();
    let total = tri1.len() + tri2.len();
    if total == 0 {
        0
    } else {
        ((100 * 2 * matches) / total) as u8
    }
}


/// LEVENSHTEIN DISTANCE - (byte‑wise) using O(min(|a|,|b|)) memory.
#[cfg(feature = "alloc")]
pub fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let len_a = a.len();
    let len_b = b.len();
    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }
    let (row_len, col_len, short, long) = if len_a <= len_b {
        (len_a, len_b, a, b)
    } else {
        (len_b, len_a, b, a)
    };
    let mut prev_row: Vec<usize> = (0..=row_len).collect();
    let mut curr_row = alloc::vec![0; row_len + 1];
    for i in 1..=col_len {
        curr_row[0] = i;
        for j in 1..=row_len {
            let cost = if long[i - 1] == short[j - 1] { 0 } else { 1 };
            let del = prev_row[j] + 1;
            let ins = curr_row[j - 1] + 1;
            let sub = prev_row[j - 1] + cost;
            curr_row[j] = del.min(ins).min(sub);
        }
        core::mem::swap(&mut prev_row, &mut curr_row);
    }
    prev_row[row_len]
}

/// LEVENSHTEIN based similarity (0‑100) with a +10 bonus if the first bytes match
#[cfg(feature = "alloc")]
pub fn levenshtein_similarity(a: &[u8], b: &[u8]) -> u8 {
    let len_a = a.len();
    let len_b = b.len();
    let max_len = len_a.max(len_b);
    if max_len == 0 {
        return 100;
    }
    let dist = levenshtein(a, b);
    let mut score = 100 - (dist * 100 / max_len);
    if !a.is_empty() && !b.is_empty() && a[0] == b[0] {
        score += 10;
    }
    score.min(100) as u8
}

/// normalize ASCII bytes to lowercase (returns new `Vec<u8>`)
#[cfg(feature = "alloc")]
pub fn normalize_ascii_lowercase(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for &b in input {
        out.push(if b.is_ascii_uppercase() { b + 32 } else { b });
    }
    out
}

/// BEST_FUZ
/// BEST FUZZY MATCH - TWO STAGE FILTER
/// 1. normalize input and each candidate.
/// 2. quick trigram filter – skip candidate if trigram similarity < `trigram_threshold`
/// 3. for survivors compute Levenshtein similarity
/// 4. return the best candidate and its Levenshtein score
#[cfg(feature = "alloc")]
pub fn best_fuz<'a>(
    input: &[u8],
    candidates: &'a [&[u8]],
    trigram_threshold: u8,
) -> (&'a [u8], u8) {
    if candidates.is_empty() {
        return (&[], 0);
    }
    let norm_input = normalize_ascii_lowercase(input);
    let mut best_candidate = candidates[0];
    let mut best_score = 0u8;
    for &candidate in candidates {
        let norm_candidate = normalize_ascii_lowercase(candidate);
        let trigram_score = trigram_similarity(&norm_input, &norm_candidate);
        if trigram_score < trigram_threshold {
            continue;
        }
        let lev_score = levenshtein_similarity(&norm_input, &norm_candidate);
        if lev_score > best_score {
            best_score = lev_score;
            best_candidate = candidate;
        }
    }
    (best_candidate, best_score)
}


/// TESTS
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigram_basic() {
        assert_eq!(trigram_similarity(b"hello", b"hella"), 66);
        assert_eq!(trigram_similarity(b"abcdef", b"abcdef"), 100);
        assert_eq!(trigram_similarity(b"ab", b"abc"), 0);
        assert_eq!(trigram_similarity(b"", b"abc"), 0);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_levenshtein_basic() {
        assert_eq!(levenshtein(b"kitten", b"sitting"), 3);
        assert_eq!(levenshtein(b"rust", b"rust"), 0);
        assert_eq!(levenshtein(b"", b"abc"), 3);
        assert_eq!(levenshtein(b"abc", b""), 3);
        assert_eq!(levenshtein(b"a", b"b"), 1);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_levenshtein_similarity_basic() {
        assert_eq!(levenshtein_similarity(b"kitten", b"sitting"), 57);
        assert_eq!(levenshtein_similarity(b"rust", b"rust"), 100);
        assert_eq!(levenshtein_similarity(b"abc", b"abd"), 77);
        assert_eq!(levenshtein_similarity(b"", b""), 100);
    }
}
