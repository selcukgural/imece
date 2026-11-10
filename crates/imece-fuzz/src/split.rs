//! `split`: the command line an agent gives the `run` tool, split into a
//! program and its arguments with no shell (`run::split`).
//!
//! Holds: what is taken splits again to the same words once each word is
//! quoted, so quoting is read one way only; nothing a shell would interpret
//! outside quotes is let through.

use imece_agent::run::split;

pub fn check(data: &[u8]) {
    let line = crate::text(data);
    let Ok(words) = split(&line) else {
        return;
    };
    let quoted: Vec<String> = (words.iter())
        .map(|w| format!("'{}'", w.replace('\'', r"'\''")))
        .collect();
    assert_eq!(split(&quoted.join(" ")).as_ref(), Ok(&words), "{line:?}");
    // Outside quotes and escapes, none of what a shell reads.
    let (mut quote, mut escaped) = (None, false);
    for c in line.chars() {
        match (quote, c) {
            _ if escaped => escaped = false,
            (None, '\\') | (Some('"'), '\\') => escaped = true,
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`') => {
                panic!("{c:?} outside quotes was taken: {line:?}")
            }
            (Some('"'), '$' | '`') => panic!("{c:?} in double quotes was taken: {line:?}"),
            _ => {}
        }
    }
}
