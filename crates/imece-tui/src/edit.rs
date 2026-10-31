//! The command line's editing keys, as readline (and so bash, zsh and
//! most REPLs) has them: ctrl-a / ctrl-e to the start / end, ctrl-b /
//! ctrl-f and the arrows a character, alt-b / alt-f (and alt or ctrl
//! with the arrows) a word, ctrl-w cuts back to the space before,
//! alt-backspace / alt-d a word back / on, ctrl-u / ctrl-k the line
//! before / after the cursor, ctrl-y puts the last cut back, ctrl-d and
//! delete take the character under the cursor. The text may run over
//! lines: shift-enter and alt-enter start a new one, as does enter after
//! a `\` (where the terminal sends shift-enter as a plain enter); the
//! keys that go to or cut to an end (ctrl-a, ctrl-e, home, end, ctrl-u,
//! ctrl-k) stop at the line's, and `↑` `↓` go a line up and down. Pure:
//! the app hands over the text, the cursor and the cut, and gets the
//! cursor back.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What the key does to `text` with the cursor at `at` (a byte offset on
/// a character's edge): the cursor after it, or `None` when the key is
/// not an editing key. What a cut takes goes to `cut`, unless it took
/// nothing (then the last cut stays for ctrl-y). A plain enter is an
/// editing key only after a `\`, which it turns into a new line; else it
/// is the app's, to send.
pub fn key(k: KeyEvent, text: &mut String, at: usize, cut: &mut String) -> Option<usize> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let shift = k.modifiers.contains(KeyModifiers::SHIFT);
    // The ends of the line the cursor is on.
    let (start, end) = (line_start(text, at), line_end(text, at));
    let word = char::is_alphanumeric;
    match k.code {
        KeyCode::Enter if shift || alt => {
            text.insert(at, '\n');
            Some(at + 1)
        }
        KeyCode::Enter if text[..at].ends_with('\\') => {
            text.replace_range(at - 1..at, "\n");
            Some(at)
        }
        KeyCode::Up => Some(up(text, at)),
        KeyCode::Down => Some(down(text, at)),
        KeyCode::Char(c) if ctrl => match c {
            'a' => Some(start),
            'e' => Some(end),
            'b' => Some(left(text, at)),
            'f' => Some(right(text, at)),
            'w' => {
                let from = back(text, at, |c| !c.is_whitespace());
                Some(take(text, from, at, cut))
            }
            'u' => Some(take(text, start, at, cut)),
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
        KeyCode::Home => Some(start),
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

/// Where the line `at` is on starts: after the new line before it.
fn line_start(s: &str, at: usize) -> usize {
    s[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// Where the line `at` is on ends: at the new line after it.
fn line_end(s: &str, at: usize) -> usize {
    s[at..].find('\n').map_or(s.len(), |i| at + i)
}

/// The place `column` characters into the line starting at `start`, or
/// that line's end when it is shorter.
fn column(s: &str, start: usize, column: usize) -> usize {
    let end = line_end(s, start);
    s[start..end]
        .char_indices()
        .nth(column)
        .map_or(end, |(i, _)| start + i)
}

/// How many characters into its line `at` is.
fn column_of(s: &str, at: usize) -> usize {
    s[line_start(s, at)..at].chars().count()
}

/// The same column a line up (`at` on the first line).
fn up(s: &str, at: usize) -> usize {
    match line_start(s, at) {
        0 => at,
        start => column(s, line_start(s, start - 1), column_of(s, at)),
    }
}

/// The same column a line down (`at` on the last line).
fn down(s: &str, at: usize) -> usize {
    match line_end(s, at) {
        end if end == s.len() => at,
        end => column(s, end + 1, column_of(s, at)),
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
            (KeyCode::Char('n'), KeyModifiers::CONTROL),
            (KeyCode::Char('p'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(press("a|b", "old", code, mods), None, "{code:?} {mods:?}");
        }
        // Enter sends unless a `\\` is just before the cursor.
        for line in ["a\\b|", "a|\\", "|", "a\n|b"] {
            assert_eq!(plain_maybe(line, KeyCode::Enter), None, "{line:?}");
        }
    }

    fn plain_maybe(line: &str, code: KeyCode) -> Option<String> {
        press(line, "old", code, KeyModifiers::NONE)
    }

    #[test]
    fn shift_or_alt_enter_and_a_backslash_enter_start_a_new_line() {
        for mods in [KeyModifiers::SHIFT, KeyModifiers::ALT] {
            let p = |line| press(line, "", KeyCode::Enter, mods).unwrap();
            assert_eq!(p("ab|cd"), "ab\n|cd/", "{mods:?}");
            assert_eq!(p("ç|"), "ç\n|/", "{mods:?}");
        }
        // The `\\` becomes the new line, as in Claude Code.
        assert_eq!(plain("fix\\|", KeyCode::Enter), "fix\n|/old");
        assert_eq!(plain("a\\|b", KeyCode::Enter), "a\n|b/old");
        assert_eq!(plain("ş\\|", KeyCode::Enter), "ş\n|/old");
    }

    #[test]
    fn the_ends_and_the_cuts_to_them_are_the_line_s() {
        let three = "ab\ncd|ef\ngh";
        assert_eq!(ctrl(three, 'a'), "ab\n|cdef\ngh/old");
        assert_eq!(ctrl(three, 'e'), "ab\ncdef|\ngh/old");
        assert_eq!(plain(three, KeyCode::Home), "ab\n|cdef\ngh/old");
        assert_eq!(plain(three, KeyCode::End), "ab\ncdef|\ngh/old");
        assert_eq!(ctrl(three, 'u'), "ab\n|ef\ngh/cd");
        assert_eq!(ctrl(three, 'k'), "ab\ncd|\ngh/ef");
        // The first line starts the text; the last ends it.
        assert_eq!(ctrl("a|b\ncd", 'e'), "ab|\ncd/old");
        assert_eq!(ctrl("ab\nc|d", 'a'), "ab\n|cd/old");
        assert_eq!(ctrl("ab\nc|d", 'e'), "ab\ncd|/old");
        assert_eq!(ctrl("ab\nc|d", 'k'), "ab\nc|/d");
        // At a line's start or end there is nothing to cut on that side,
        // and the new line itself is not cut.
        assert_eq!(ctrl("ab\n|cd", 'u'), "ab\n|cd/old");
        assert_eq!(ctrl("ab|\ncd", 'k'), "ab|\ncd/old");
    }

    #[test]
    fn up_and_down_keep_the_column_in_characters() {
        let three = "abc\nde|f\nghij";
        assert_eq!(plain(three, KeyCode::Up), "ab|c\ndef\nghij/old");
        assert_eq!(plain(three, KeyCode::Down), "abc\ndef\ngh|ij/old");
        assert_eq!(plain("ab\ncd\nef|", KeyCode::Up), "ab\ncd|\nef/old");
        assert_eq!(plain("a|b\ncd\nef", KeyCode::Down), "ab\nc|d\nef/old");
        assert_eq!(plain("ab\n|cd", KeyCode::Up), "|ab\ncd/old");
        // A shorter line: its end.
        assert_eq!(plain("a\nbcd|", KeyCode::Up), "a|\nbcd/old");
        assert_eq!(plain("abc|\nd", KeyCode::Down), "abc\nd|/old");
        assert_eq!(plain("çağ\nab|", KeyCode::Up), "ça|ğ\nab/old");
        assert_eq!(plain("ab|\nçağ", KeyCode::Down), "ab\nça|ğ/old");
        // On the first line up stays, on the last down does.
        assert_eq!(plain("a|b\nc", KeyCode::Up), "a|b\nc/old");
        assert_eq!(plain("a\nb|c", KeyCode::Down), "a\nb|c/old");
        assert_eq!(plain("a|b", KeyCode::Up), "a|b/old");
    }
}
