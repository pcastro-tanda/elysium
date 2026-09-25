//! `RuboCop::NameSimilarity.find_similar_name`, which delegates to Ruby's
//! `DidYouMean::SpellChecker` (`did_you_mean/spell_checker.rb`,
//! `jaro_winkler.rb`, `levenshtein.rb` as shipped with Ruby 3.4).
//!
//! The whole chain is ported because the exact suggestion text is part of a
//! cop's message: `Lint/UselessAssignment` appends ``Did you mean `foo`?``
//! only when the spell checker picks a candidate, and RuboCop's specs pin
//! both the positive and the "does not suggest any name" cases.
//!
//! Identifier lengths and match counts are far below `f64`'s exact-integer
//! range, and the Levenshtein loop keeps the reference implementation's
//! single-letter names so it can be diffed against it line by line.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]

/// `DidYouMean::SpellChecker#normalize`: downcase, then drop `@`.
fn normalize(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).filter(|&c| c != '@').collect()
}

/// `DidYouMean::Jaro.distance`.
fn jaro(a: &[char], b: &[char]) -> f64 {
    let (short, long) = if a.len() > b.len() { (b, a) } else { (a, b) };
    let (length1, length2) = (short.len(), long.len());
    if length1 == 0 {
        return 0.0;
    }
    let range = if length2 > 3 { length2 / 2 - 1 } else { 0 };

    let mut flags1 = vec![false; length1];
    let mut flags2 = vec![false; length2];
    let mut matches = 0.0_f64;

    for i in 0..length1 {
        let last = i + range;
        let mut j = i.saturating_sub(range);
        while j <= last {
            if j < length2 && !flags2[j] && short[i] == long[j] {
                flags2[j] = true;
                flags1[i] = true;
                matches += 1.0;
                break;
            }
            j += 1;
        }
    }
    if matches == 0.0 {
        return 0.0;
    }

    let mut transpositions = 0.0_f64;
    let mut k = 0usize;
    for i in 0..length1 {
        if !flags1[i] {
            continue;
        }
        let mut j = k;
        let mut index = k;
        while j < length2 {
            index = j;
            if flags2[j] {
                k = j + 1;
                break;
            }
            j += 1;
        }
        if long.get(index) != Some(&short[i]) {
            transpositions += 1.0;
        }
    }
    let transpositions = (transpositions / 2.0).floor();

    let (l1, l2) = (length1 as f64, length2 as f64);
    (matches / l1 + matches / l2 + (matches - transpositions) / matches) / 3.0
}

/// `DidYouMean::JaroWinkler.distance`.
fn jaro_winkler(a: &[char], b: &[char]) -> f64 {
    let distance = jaro(a, b);
    if distance <= 0.7 {
        return distance;
    }
    let mut prefix = 0usize;
    for ch in a {
        if prefix < 4 && b.get(prefix) == Some(ch) {
            prefix += 1;
        } else {
            break;
        }
    }
    distance + (prefix as f64) * 0.1 * (1.0 - distance)
}

/// `DidYouMean::Levenshtein.distance`.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut d: Vec<usize> = (0..=m).collect();
    let mut x = 0usize;
    for (row, &c1) in a.iter().enumerate() {
        let mut i = row + 1;
        for j in 0..m {
            let cost = usize::from(c1 != b[j]);
            x = (d[j + 1] + 1).min(i + 1).min(d[j] + cost);
            d[j] = i;
            i = x;
        }
        d[m] = x;
    }
    x
}

/// `RuboCop::NameSimilarity.find_similar_name`: the best
/// `DidYouMean::SpellChecker` candidate for `target` among `names`, or
/// `None`.
pub(crate) fn find_similar_name(target: &str, names: &[String]) -> Option<String> {
    let input = normalize(target);
    let threshold = if input.len() > 3 { 0.834 } else { 0.77 };

    let mut words: Vec<(&String, f64)> = names
        .iter()
        .filter(|word| jaro_winkler(&normalize(word), &input) >= threshold)
        .filter(|word| word.as_str() != target)
        .map(|word| (word, jaro_winkler(&word.chars().collect::<Vec<_>>(), &input)))
        .collect();
    // `sort_by! { … }` then `reverse!`: ascending by score, then flipped.
    words.sort_by(|a, b| a.1.total_cmp(&b.1));
    words.reverse();

    let mistype_threshold = ((input.len() as f64) * 0.25).ceil() as usize;
    let mistype =
        words.iter().find(|(word, _)| levenshtein(&normalize(word), &input) <= mistype_threshold);
    if let Some((word, _)) = mistype {
        return Some((*word).clone());
    }

    words
        .iter()
        .find(|(word, _)| {
            let word = normalize(word);
            let length = input.len().min(word.len());
            levenshtein(&word, &input) < length
        })
        .map(|(word, _)| (*word).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    #[test]
    fn suggests_a_transposed_name() {
        let names = dict(&["another_symbol", "environment", "enviromnent"]);
        assert_eq!(find_similar_name("enviromnent", &names).as_deref(), Some("environment"));
    }

    #[test]
    fn suggests_nothing_for_a_distant_name() {
        let names = dict(&["another_symbol", "envelope", "enviromnent"]);
        assert_eq!(find_similar_name("enviromnent", &names), None);
    }

    #[test]
    fn suggests_nothing_when_only_the_target_itself_is_known() {
        let names = dict(&["another_symbol", "enviromnent"]);
        assert_eq!(find_similar_name("enviromnent", &names), None);
    }

    #[test]
    fn short_names_use_the_lower_threshold_but_still_need_overlap() {
        assert_eq!(find_similar_name("j", &dict(&["items", "i"])), None);
        assert_eq!(find_similar_name("item", &dict(&["items"])).as_deref(), Some("items"));
    }

    #[test]
    fn levenshtein_matches_the_reference_implementation() {
        let a: Vec<char> = "environment".chars().collect();
        let b: Vec<char> = "enviromnent".chars().collect();
        assert_eq!(levenshtein(&a, &b), 2);
        assert_eq!(levenshtein(&[], &b), 11);
        assert_eq!(levenshtein(&a, &[]), 11);
    }
}
