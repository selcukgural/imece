//! `clean`: agent text on its way to the terminal (`i18n::clean`, and
//! `i18n::fill`, which cleans every argument). Nothing an agent wrote
//! reaches the screen another way (CLAUDE.md).
//!
//! Holds: no control character comes out (an agent's text must not drive
//! the terminal); cleaning again changes nothing, so a hidden secret stays
//! hidden and nothing hidden comes back; `fill` puts exactly the cleaned
//! argument in its place.

use crate::i18n::{clean, fill};

pub fn check(data: &[u8]) {
    let s = crate::text(data);
    let c = clean(&s);
    assert!(!c.chars().any(char::is_control), "{c:?}");
    assert_eq!(clean(&c), c, "cleaned twice: {s:?}");
    assert_eq!(fill("<{}>", &[&s]), format!("<{c}>"));
}
