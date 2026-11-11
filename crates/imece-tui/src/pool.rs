//! The words the composer suggests while the person writes (record 149):
//! a drop-down list under the word being typed, as an IDE has it, with no
//! model behind it. Pure: the screen hands in what was said (the
//! conversation at hand, the rest of the project's talk and the lines
//! sent, newest first), the names in
//! the code and the project's files, and gets the ranked picks back.
//!
//! A word is a run of letters, digits and `_` (`retry_policy`,
//! `PaymentClient`, `çalışan`); an apostrophe, a dot or a dash ends it, so
//! a Turkish suffix (`retry_policy'yi`) or a file's extension stays out.
//! Every text is cleaned (`i18n::clean`) before its words are read, so a
//! secret it holds is never offered; matching folds case and Turkish
//! letters (`ui::fold`).

use std::collections::HashSet;

use crate::i18n::clean;
use crate::ui::fold;

/// Letters typed before the list opens by itself.
pub const TYPED: usize = 2;

/// The shortest word offered: a shorter one is quicker typed than chosen.
pub const SHORTEST: usize = 4;

/// The longest word offered, in characters: past it a word is a hash or a
/// token, not a name the person would type.
pub const LONGEST: usize = 48;

/// Picks offered at most; the list shows a few and scrolls through them.
pub const SHOWN: usize = 30;

/// The texts read from each place said words come from, newest first:
/// the conversation at hand, the rest said in the project, the lines
/// sent.
pub const SAID_BACK: usize = 200;

/// The texts [`picks`] reads at most: [`SAID_BACK`] from each place.
pub const TEXTS: usize = 3 * SAID_BACK;

/// The journal's lines looked through for the rest said in the project.
pub const JOURNAL_BACK: usize = 2_000;

/// Where a pick comes from, which the list says beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    /// The conversation at hand, the project's talk, a line sent.
    Said,
    /// A name in the repository's code.
    Code,
    /// A file of the project, by its path.
    File,
}

/// One word the list offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    /// What taking it puts in place of the word typed.
    pub text: String,
    pub from: From,
}

/// Whether `c` belongs to a word.
pub fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Where the word that ends at byte `at` of `input` starts: the run of
/// word characters right before it (`at` itself when there is none).
pub fn word_start(input: &str, at: usize) -> usize {
    let before = &input[..at];
    before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| word_char(c))
        .last()
        .map_or(at, |(i, _)| i)
}

/// The words of `text` worth offering, in the order they stand: at least
/// [`SHORTEST`] and at most [`LONGEST`] characters, starting with a letter
/// or `_` (no numbers, no `3rd`).
pub fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !word_char(c)).filter(|w| {
        let n = w.chars().count();
        (SHORTEST..=LONGEST).contains(&n) && w.chars().next().is_some_and(|c| !c.is_numeric())
    })
}

/// The picks for `typed`, the word being written: the said words (from
/// `said`, newest text first, its words last to first, so the word seen
/// last comes first), then the names in `code` as given (`names::of`: the
/// most used first), then the `files`
/// whose name (the path's last part) starts with the word, shorter paths
/// first; each starting with `typed` (folded), never `typed` itself, a
/// word once (folded: the first spelling met stays), at most [`SHOWN`].
/// Nothing for a word shorter than [`TYPED`] unless `forced` (`tab`).
pub fn picks<'a>(
    typed: &str,
    forced: bool,
    said: impl IntoIterator<Item = &'a str>,
    code: &[&str],
    files: &[&str],
) -> Vec<Pick> {
    if typed.is_empty() || (!forced && typed.chars().count() < TYPED) {
        return Vec::new();
    }
    let want = fold(typed);
    let mut seen: HashSet<String> = HashSet::from([want.clone()]);
    let mut out = Vec::new();
    let mut take = |text: &str, from: From, out: &mut Vec<Pick>| {
        let folded = fold(text);
        if out.len() < SHOWN && folded.starts_with(&want) && seen.insert(folded) {
            out.push(Pick {
                text: text.to_string(),
                from,
            });
        }
    };
    for text in said.into_iter().take(TEXTS) {
        // Most texts have no word that starts so: they are not cleaned.
        if !fold(text).contains(&want) {
            continue;
        }
        let cleaned = clean(text);
        let found: Vec<&str> = words(&cleaned).collect();
        for w in found.into_iter().rev() {
            take(w, From::Said, &mut out);
        }
    }
    for name in code {
        // Most names do not start so: they are not cleaned.
        if out.len() < SHOWN && fold(name).starts_with(&want) && clean(name) == *name {
            take(name, From::Code, &mut out);
        }
    }
    let name_of = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut named: Vec<&str> = files
        .iter()
        .copied()
        .filter(|p| fold(&name_of(p)).starts_with(&want))
        .collect();
    named.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
    for path in named {
        // A file is offered by its path; folded whole, so two files of
        // one name in two directories are both there.
        let folded = fold(path);
        if out.len() < SHOWN && folded != want && seen.insert(folded) {
            out.push(Pick {
                text: path.to_string(),
                from: From::File,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(picks: &[Pick]) -> Vec<&str> {
        picks.iter().map(|p| p.text.as_str()).collect()
    }

    #[test]
    fn a_word_is_letters_digits_and_underscores() {
        let found: Vec<&str> =
            words("keep retry_policy'yi as it is, PaymentClient.go v2 3rds çalışan-x").collect();
        assert_eq!(found, ["keep", "retry_policy", "PaymentClient", "çalışan"]);
    }

    #[test]
    fn a_word_offered_is_four_to_forty_eight_characters() {
        let short = "abc";
        let four = "abcd";
        let longest = "a".repeat(LONGEST);
        let longer = "b".repeat(LONGEST + 1);
        let text = format!("{short} {four} {longest} {longer}");
        let found: Vec<&str> = words(&text).collect();
        assert_eq!(found, [four, longest.as_str()]);
        // Characters, not bytes: four Turkish letters are a word.
        assert_eq!(words("çğış").count(), 1);
    }

    #[test]
    fn the_word_typed_starts_after_the_last_non_word_character() {
        assert_eq!(word_start("keep the retr", 13), 9);
        assert_eq!(word_start("`retr", 5), 1);
        assert_eq!(word_start("retr", 4), 0);
        assert_eq!(word_start("retry'yi", 8), 6);
        assert_eq!(word_start("ends with ", 10), 10);
        assert_eq!(word_start("çalış", "çalış".len()), 0);
        // Only what stands before `at` counts.
        assert_eq!(word_start("one two", 3), 0);
    }

    #[test]
    fn the_list_opens_from_two_letters_and_tab_opens_it_from_one() {
        let said = ["retry_policy"];
        assert!(picks("r", false, said, &[], &[]).is_empty());
        assert_eq!(texts(&picks("re", false, said, &[], &[])), ["retry_policy"]);
        assert_eq!(texts(&picks("r", true, said, &[], &[])), ["retry_policy"]);
        assert!(picks("", true, said, &[], &[]).is_empty());
    }

    #[test]
    fn said_words_come_newest_first_then_code_then_files() {
        let said = [
            "the retry_after_ms field",
            "old retry_policy and retry_budget",
        ];
        let code = ["RetryPolicy", "retry_policy"];
        let files = ["src/net/retry.rs", "retry.md", "src/other.rs"];
        let got = picks("retr", false, said, &code, &files);
        assert_eq!(
            texts(&got),
            [
                "retry_after_ms",
                "retry_budget",
                "retry_policy",
                "RetryPolicy",
                "retry.md",
                "src/net/retry.rs"
            ]
        );
        let from: Vec<From> = got.iter().map(|p| p.from).collect();
        assert_eq!(
            from,
            [
                From::Said,
                From::Said,
                From::Said,
                From::Code,
                From::File,
                From::File
            ]
        );
    }

    #[test]
    fn matching_folds_case_and_turkish_letters_and_keeps_the_first_spelling() {
        let said = ["Çalışan geldi", "calisan gitti"];
        assert_eq!(texts(&picks("cal", false, said, &[], &[])), ["Çalışan"]);
        assert_eq!(
            texts(&picks("ÇAL", false, ["calisan"], &[], &[])),
            ["calisan"]
        );
    }

    #[test]
    fn the_word_typed_is_never_offered() {
        let said = ["retry retry_policy Retry"];
        assert_eq!(
            texts(&picks("retry", false, said, &[], &[])),
            ["retry_policy"]
        );
        assert!(picks("retry.rs", true, [], &[], &["retry.rs"]).is_empty());
    }

    #[test]
    fn a_secret_is_never_offered() {
        let token = format!("ghp_{}", "a1B2".repeat(9));
        let said = [format!("use {token} for it")];
        assert!(picks("ghp", false, said.iter().map(String::as_str), &[], &[]).is_empty());
        let named = format!("API_KEY=secretvalue{}", "x".repeat(4));
        assert!(picks("secr", false, [named.as_str()], &[], &[]).is_empty());
        assert!(picks("ghp", false, [], &[token.as_str()], &[]).is_empty());
    }

    #[test]
    fn at_most_thirty_picks_and_the_texts_read_are_capped() {
        let many: Vec<String> = (0..SHOWN + 5).map(|i| format!("word{i:02}x")).collect();
        let said = many.iter().map(String::as_str);
        assert_eq!(picks("word", false, said, &[], &[]).len(), SHOWN);
        let files: Vec<String> = (0..SHOWN + 5).map(|i| format!("d/word{i:02}.rs")).collect();
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        assert_eq!(picks("word", false, [], &[], &files).len(), SHOWN);
        // The texts past TEXTS are not read.
        let mut texts_in: Vec<String> = vec!["filler".to_string(); TEXTS - 1];
        texts_in.push("lastread".into());
        texts_in.push("notread".into());
        let said = texts_in.iter().map(String::as_str);
        assert_eq!(
            texts(&picks("lastr", false, said.clone(), &[], &[])),
            ["lastread"]
        );
        assert!(picks("notr", false, said, &[], &[]).is_empty());
    }

    #[test]
    fn files_match_by_name_shorter_path_first() {
        let files = [
            "a/b/retry.rs",
            "retry_test.rs",
            "src/notretry.rs",
            "z/retry.rs",
        ];
        assert_eq!(
            texts(&picks("retr", false, [], &[], &files)),
            ["z/retry.rs", "a/b/retry.rs", "retry_test.rs"]
        );
    }
}
