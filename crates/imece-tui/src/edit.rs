//! The command line's editing keys, as readline (and so bash, zsh and
//! most REPLs) has them: ctrl-a / ctrl-e to the start / end, ctrl-b /
//! ctrl-f and the arrows a character, alt-b / alt-f (and alt or ctrl
//! with the arrows) a word, ctrl-w cuts back to the space before,
//! alt-backspace / alt-d a word back / on, ctrl-u / ctrl-k the line
//! before / after the cursor, ctrl-y puts the last cut back, ctrl-d and
//! delete take the character under the cursor. Pure: the app hands over
//! the text, the cursor and the cut, and gets the cursor back.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What the key does to `text` with the cursor at `at` (a byte offset on
/// a character's edge): the cursor after it, or `None` when the key is
/// not an editing key. What a cut takes goes to `cut`, unless it took
/// nothing (then the last cut stays for ctrl-y).
pub fn key(k: KeyEvent, text: &mut String, at: usize, cut: &mut String) -> Option<usize> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let end = text.len();
    let word = char::is_alphanumeric;
    match k.code {
        KeyCode::Char(c) if ctrl => match c {
            'a' => Some(0),
            'e' => Some(end),
            'b' => Some(left(text, at)),
            'f' => Some(right(text, at)),
            'w' => {
                let from = back(text, at, |c| !c.is_whitespace());
                Some(take(text, from, at, cut))
            }
            'u' => Some(take(text, 0, at, cut)),
            'k' => Some(take(text, at, end, cut)),
            'd' => {
                let to = right(text, at);
                Some(remove(text, at, to))
            }
            'y' => {
                text.insert_str(at, cut);
                Some(at + cut.len())
            }
            _ => None,
        },
        KeyCode::Char('b') if alt => Some(back(text, at, word)),
        KeyCode::Char('f') if alt => Some(on(text, at, word)),
        KeyCode::Char('d') if alt => {
            let to = on(text, at, word);
            Some(take(text, at, to, cut))
        }
        KeyCode::Backspace if alt => {
            let from = back(text, at, word);
            Some(take(text, from, at, cut))
        }
        KeyCode::Left if ctrl || alt => Some(back(text, at, word)),
        KeyCode::Right if ctrl || alt => Some(on(text, at, word)),
        KeyCode::Left => Some(left(text, at)),
        KeyCode::Right => Some(right(text, at)),
        KeyCode::Home => Some(0),
        KeyCode::End => Some(end),
        KeyCode::Backspace => {
            let from = left(text, at);
            Some(remove(text, from, at))
        }
        KeyCode::Delete => {
            let to = right(text, at);
            Some(remove(text, at, to))
        }
        KeyCode::Char(c) => {
            text.insert(at, c);
            Some(at + c.len_utf8())
        }
        _ => None,
    }
}

/// Where the character before `at` starts (`at` at the start).
fn left(s: &str, at: usize) -> usize {
    s[..at]
        .chars()
        .next_back()
        .map_or(at, |c| at - c.len_utf8())
}

/// Where the character at `at` ends (`at` at the end).
fn right(s: &str, at: usize) -> usize {
    s[at..].chars().next().map_or(at, |c| at + c.len_utf8())
}

/// Back from `at` over what is not part of a word, then over the word:
/// where that word starts.
fn back(s: &str, at: usize, part: impl Fn(char) -> bool) -> usize {
    let mut start = at;
    let mut seen = false;
    for (i, c) in s[..at].char_indices().rev() {
        if part(c) {
            seen = true;
        } else if seen {
            break;
        }
        start = i;
    }
    start
}

/// On from `at` over what is not part of a word, then over the word:
/// where that word ends.
fn on(s: &str, at: usize, part: impl Fn(char) -> bool) -> usize {
    let mut seen = false;
    for (i, c) in s[at..].char_indices() {
        if part(c) {
            seen = true;
        } else if seen {
            return at + i;
        }
    }
    s.len()
}

/// Removes `from..to` and keeps it as the cut (when there was something
/// to remove); the cursor goes to `from`.
fn take(text: &mut String, from: usize, to: usize, cut: &mut String) -> usize {
    if from < to {
        *cut = text[from..to].to_string();
    }
    remove(text, from, to)
}

/// Removes `from..to`; the cursor goes to `from`.
fn remove(text: &mut String, from: usize, to: usize) -> usize {
    text.replace_range(from..to, "");
    from
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `line` has the cursor where `|` is; the key is pressed; the result
    /// is drawn the same way, with the cut after a `/`.
    fn press(line: &str, cut: &str, code: KeyCode, mods: KeyModifiers) -> Option<String> {
        let at = line.find('|').unwrap();
        let mut text = line.replacen('|', "", 1);
        let mut cut = cut.to_string();
        let to = key(KeyEvent::new(code, mods), &mut text, at, &mut cut)?;
        text.insert(to, '|');
        Some(format!("{text}/{cut}"))
    }

    fn ctrl(line: &str, c: char) -> String {
        press(line, "old", KeyCode::Char(c), KeyModifiers::CONTROL).unwrap()
    }

    fn alt(line: &str, code: KeyCode) -> String {
        press(line, "old", code, KeyModifiers::ALT).unwrap()
    }

    fn plain(line: &str, code: KeyCode) -> String {
        press(line, "old", code, KeyModifiers::NONE).unwrap()
    }

    #[test]
    fn ctrl_a_and_e_go_to_the_ends() {
        assert_eq!(ctrl("ab|cd", 'a'), "|abcd/old");
        assert_eq!(ctrl("ab|cd", 'e'), "abcd|/old");
        assert_eq!(plain("ab|cd", KeyCode::Home), "|abcd/old");
        assert_eq!(plain("ab|cd", KeyCode::End), "abcd|/old");
    }

    #[test]
    fn a_character_at_a_time_counts_characters_not_bytes() {
        assert_eq!(ctrl("aç|b", 'b'), "a|çb/old");
        assert_eq!(ctrl("a|çb", 'f'), "aç|b/old");
        assert_eq!(plain("aç|b", KeyCode::Left), "a|çb/old");
        assert_eq!(plain("a|çb", KeyCode::Right), "aç|b/old");
        // At the ends nothing moves.
        assert_eq!(plain("|ab", KeyCode::Left), "|ab/old");
        assert_eq!(plain("ab|", KeyCode::Right), "ab|/old");
    }

    #[test]
    fn alt_b_and_f_go_a_word_back_and_on() {
        assert_eq!(
            alt("@lead fix the| parser", KeyCode::Char('b')),
            "@lead fix |the parser/old"
        );
        assert_eq!(
            alt("@lead fix |the parser", KeyCode::Char('b')),
            "@lead |fix the parser/old"
        );
        assert_eq!(alt("@lead| fix", KeyCode::Char('b')), "@|lead fix/old");
        assert_eq!(alt("@|lead", KeyCode::Char('b')), "|@lead/old");
        assert_eq!(
            alt("@lead |fix the", KeyCode::Char('f')),
            "@lead fix| the/old"
        );
        assert_eq!(
            alt("@lead fix| the", KeyCode::Char('f')),
            "@lead fix the|/old"
        );
        assert_eq!(alt("a|-- ", KeyCode::Char('f')), "a-- |/old");
        assert_eq!(alt("ça|ğı şu", KeyCode::Char('f')), "çağı| şu/old");
        assert_eq!(alt("çağı şu|", KeyCode::Char('b')), "çağı |şu/old");
        // Ctrl or alt with the arrows do the same.
        for mods in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
            let p = |line, code| press(line, "", code, mods).unwrap();
            assert_eq!(p("ab cd|", KeyCode::Left), "ab |cd/");
            assert_eq!(p("|ab cd", KeyCode::Right), "ab| cd/");
        }
    }

    #[test]
    fn ctrl_w_cuts_back_to_the_space() {
        assert_eq!(ctrl(":open ~/src/imece|", 'w'), ":open |/~/src/imece");
        assert_eq!(ctrl(":open ~/src/imece  |x", 'w'), ":open |x/~/src/imece  ");
        assert_eq!(ctrl("one|", 'w'), "|/one");
        // Nothing to cut keeps the last cut.
        assert_eq!(ctrl("|one", 'w'), "|one/old");
    }

    #[test]
    fn alt_backspace_and_alt_d_cut_a_word() {
        assert_eq!(
            alt(":open ~/src/imece|", KeyCode::Backspace),
            ":open ~/src/|/imece"
        );
        assert_eq!(alt("|", KeyCode::Backspace), "|/old");
        assert_eq!(
            alt("fix |the parser", KeyCode::Char('d')),
            "fix | parser/the"
        );
        assert_eq!(
            alt("fix| the parser", KeyCode::Char('d')),
            "fix| parser/ the"
        );
        assert_eq!(alt("fix|", KeyCode::Char('d')), "fix|/old");
    }

    #[test]
    fn ctrl_u_and_k_cut_the_line_before_and_after() {
        assert_eq!(ctrl("ab|cd", 'u'), "|cd/ab");
        assert_eq!(ctrl("ab|cd", 'k'), "ab|/cd");
        assert_eq!(ctrl("|cd", 'u'), "|cd/old");
        assert_eq!(ctrl("ab|", 'k'), "ab|/old");
    }

    #[test]
    fn ctrl_y_puts_the_cut_back_at_the_cursor() {
        assert_eq!(ctrl("ab|cd", 'y'), "abold|cd/old");
        assert_eq!(
            press("a|b", "", KeyCode::Char('y'), KeyModifiers::CONTROL).unwrap(),
            "a|b/"
        );
        // Cut and put back: the line is whole again.
        let mut text = "fix the parser".to_string();
        let mut cut = String::new();
        let at = key(
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            &mut text,
            8,
            &mut cut,
        )
        .unwrap();
        assert_eq!((at, text.as_str()), (0, "parser"));
        let at = key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            &mut text,
            at,
            &mut cut,
        )
        .unwrap();
        let at = key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            &mut text,
            at,
            &mut cut,
        )
        .unwrap();
        assert_eq!((at, text.as_str()), (14, "parserfix the "));
    }

    #[test]
    fn deleting_and_typing_happen_at_the_cursor() {
        assert_eq!(plain("aç|b", KeyCode::Backspace), "a|b/old");
        assert_eq!(plain("|ab", KeyCode::Backspace), "|ab/old");
        assert_eq!(plain("a|çb", KeyCode::Delete), "a|b/old");
        assert_eq!(ctrl("a|çb", 'd'), "a|b/old");
        assert_eq!(plain("ab|", KeyCode::Delete), "ab|/old");
        assert_eq!(plain("a|b", KeyCode::Char('ş')), "aş|b/old");
        assert_eq!(
            press("a|b", "", KeyCode::Char('X'), KeyModifiers::SHIFT).unwrap(),
            "aX|b/"
        );
        // An alt key that edits nothing types its character, as before
        // (some keyboards make `@` with alt).
        assert_eq!(alt("a|b", KeyCode::Char('@')), "a@|b/old");
    }

    #[test]
    fn other_keys_are_not_editing_keys() {
        for (code, mods) in [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::Up, KeyModifiers::NONE),
            (KeyCode::Char('n'), KeyModifiers::CONTROL),
            (KeyCode::Char('p'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(press("a|b", "old", code, mods), None, "{code:?} {mods:?}");
        }
    }
}
