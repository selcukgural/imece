//! Colours by role, not by name: the screen asks for `warn` or `accent`
//! and the theme says what that is. Three themes: dark (the default),
//! light for light terminals, and mono for no colour at all (bold, dim
//! and reverse carry every signal, as the glyphs already do).

use std::cell::Cell;

use ratatui::style::Color;

pub struct Theme {
    pub name: &'static str,
    /// No colours: `dim` is the DIM attribute, badges are reversed.
    pub mono: bool,
    /// imece's own colour: the brand badge, the focused border, marks.
    pub accent: Color,
    pub dim: Color,
    /// Titles and names.
    pub strong: Color,
    /// Text on a coloured badge.
    pub on_badge: Color,
    /// Text on a key hint.
    pub on_key: Color,
    /// The background of a key hint.
    pub key: Color,
    /// The background of the selected row.
    pub selection: Color,
    pub code: Color,
    pub ok: Color,
    pub warn: Color,
    pub bad: Color,
    pub stopped: Color,
    /// Conversations, verifying: calm news.
    pub info: Color,
    /// Leaders and plans.
    pub plan: Color,
    pub research: Color,
    /// Claude, Codex, Gemini, Qwen, local.
    pub vendors: [Color; 5],
    /// A speaker's own colour in conversations, by agent id.
    pub speakers: [Color; 12],
}

const fn c(n: u8) -> Color {
    Color::Indexed(n)
}

pub const DARK: Theme = Theme {
    name: "dark",
    mono: false,
    accent: c(173),
    dim: c(244),
    strong: Color::White,
    on_badge: Color::Black,
    on_key: Color::Black,
    key: c(250),
    selection: c(236),
    code: c(180),
    ok: Color::Green,
    warn: Color::Yellow,
    bad: Color::Red,
    stopped: Color::LightRed,
    info: Color::Cyan,
    plan: Color::Magenta,
    research: Color::Blue,
    vendors: [c(173), c(36), c(69), c(99), c(250)],
    speakers: [
        c(75),
        c(179),
        c(114),
        c(176),
        c(216),
        c(79),
        c(147),
        c(186),
        c(211),
        c(110),
        c(151),
        c(223),
    ],
};

/// Darker shades of the same roles, readable on a white background.
pub const LIGHT: Theme = Theme {
    name: "light",
    mono: false,
    accent: c(166),
    dim: c(243),
    strong: Color::Black,
    on_badge: Color::White,
    on_key: Color::Black,
    key: c(252),
    selection: c(254),
    code: c(94),
    ok: c(28),
    warn: c(130),
    bad: c(160),
    stopped: c(196),
    info: c(30),
    plan: c(127),
    research: c(25),
    vendors: [c(166), c(29), c(26), c(92), c(240)],
    speakers: [
        c(26),
        c(130),
        c(28),
        c(127),
        c(166),
        c(29),
        c(93),
        c(100),
        c(161),
        c(24),
        c(64),
        c(94),
    ],
};

pub const MONO: Theme = Theme {
    name: "mono",
    mono: true,
    accent: Color::Reset,
    dim: Color::Reset,
    strong: Color::Reset,
    on_badge: Color::Reset,
    on_key: Color::Reset,
    key: Color::Reset,
    selection: Color::Reset,
    code: Color::Reset,
    ok: Color::Reset,
    warn: Color::Reset,
    bad: Color::Reset,
    stopped: Color::Reset,
    info: Color::Reset,
    plan: Color::Reset,
    research: Color::Reset,
    vendors: [Color::Reset; 5],
    speakers: [Color::Reset; 12],
};

pub const ALL: [&Theme; 3] = [&DARK, &LIGHT, &MONO];

thread_local! {
    // Per thread: the screen draws on one thread, and tests that change
    // the theme do not change it for each other.
    static CURRENT: Cell<usize> = const { Cell::new(0) };
}

/// The theme in use.
pub fn pal() -> &'static Theme {
    ALL[CURRENT.with(Cell::get)]
}

/// Switches to the theme called `name`; `false` if there is none.
pub fn set(name: &str) -> bool {
    match ALL.iter().position(|t| t.name == name) {
        Some(i) => {
            CURRENT.with(|c| c.set(i));
            true
        }
        None => false,
    }
}

/// Switches to the next theme and returns it.
pub fn next() -> &'static Theme {
    let i = (CURRENT.with(Cell::get) + 1) % ALL.len();
    CURRENT.with(|c| c.set(i));
    ALL[i]
}

/// The names, for usage and error messages.
pub fn names() -> String {
    ALL.iter().map(|t| t.name).collect::<Vec<_>>().join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_are_found_by_name_and_cycle() {
        assert_eq!(pal().name, "dark");
        assert!(set("light"));
        assert_eq!(pal().name, "light");
        assert!(!set("neon"));
        assert_eq!(pal().name, "light", "an unknown name changes nothing");
        assert_eq!(next().name, "mono");
        assert_eq!(next().name, "dark");
        assert_eq!(names(), "dark|light|mono");
    }

    #[test]
    fn mono_has_no_colour_at_all() {
        let m = &MONO;
        let all = [
            m.accent,
            m.dim,
            m.strong,
            m.on_badge,
            m.on_key,
            m.key,
            m.selection,
            m.code,
            m.ok,
            m.warn,
            m.bad,
            m.stopped,
            m.info,
            m.plan,
            m.research,
        ];
        assert!(
            all.iter()
                .chain(&m.vendors)
                .chain(&m.speakers)
                .all(|&c| c == Color::Reset)
        );
        assert!(m.mono);
    }
}
