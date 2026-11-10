//! What each fuzz target checks (record 139, `docs/decisions/139-fuzzing.md`).
//!
//! Every place where imece reads bytes it did not write has a target here:
//! what a Claude Code session prints (`stream`), what an agent's session asks
//! on the bus (`bus`) and through its permission prompt (`approve`), the
//! prompt imece writes back (`prompt`), the `run` tool's command line
//! (`split`) and its output (`tidy`), a worker's patch cut to hunks
//! (`patch`), a card's paths (`paths`), and agent text on its way to the
//! terminal (`clean`). A target is a function of bytes that panics when what
//! it checks does not hold; every input must leave it quiet.
//!
//! `fuzz/` (a cargo-fuzz project outside the workspace) hands these
//! functions to libFuzzer; `tests/inputs.rs` runs every saved input, the
//! seeds and each crash kept as a regression, through them with
//! `cargo test`, so a crash once found stays fixed.
//!
//! The screen (`crates/imece-tui`) is a program, not a library: the two
//! of its files a target needs, which name nothing else of it, are taken in
//! whole by `#[path]`, so the fuzzer runs the very code the screen does.

#![forbid(unsafe_code)]

pub mod approve;
pub mod bus;
pub mod clean;
pub mod patch;
pub mod paths;
pub mod prompt;
pub mod split;
pub mod stream;
pub mod tidy;

/// The screen's words and `clean`, as the screen has them.
#[allow(dead_code, clippy::all)]
#[path = "../../imece-tui/src/i18n.rs"]
pub mod i18n;

/// The screen's paths of a card, as the screen has them.
#[allow(dead_code, clippy::all)]
#[path = "../../imece-tui/src/paths.rs"]
pub mod owned;

/// What a target is: bytes in, a panic if what it holds does not.
pub type Check = fn(&[u8]);

/// Every target by its name, which is also its directory under
/// `fuzz/seeds/` and `fuzz/regressions/` and its binary in `fuzz/`.
pub const TARGETS: &[(&str, Check)] = &[
    ("approve", approve::check),
    ("bus", bus::check),
    ("clean", clean::check),
    ("patch", patch::check),
    ("paths", paths::check),
    ("prompt", prompt::check),
    ("split", split::check),
    ("stream", stream::check),
    ("tidy", tidy::check),
];

/// The first byte, to choose with, and the rest; nothing for no input.
pub fn choice(data: &[u8]) -> Option<(u8, &[u8])> {
    data.split_first().map(|(&c, rest)| (c, rest))
}

/// `data` as text, invalid UTF-8 replaced, as a reader of text would.
pub fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}
