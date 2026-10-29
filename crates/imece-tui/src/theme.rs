//! Colours by role, not by name: the screen asks for `warn` or `accent`
//! and the theme says what that is. Three sources make one `Theme`:
//!
//! - `terminal` (the default) names the terminal's own 16 colours, so
//!   the terminal's theme (iTerm2, Ghostty, Kitty, …) colours imece and
//!   a change there shows at once; the background and foreground the
//!   terminal reports (`probe`) give the selection and key colours and
//!   say whether it is dark. `dark` and `light` are the same with the
//!   background assumed, for a terminal that does not answer. `mono`
//!   has no colour at all: bold, dim and reverse carry every signal, as
//!   the glyphs already do.
//! - A built-in scheme (`dracula`, `nord`, …): a base16 palette from
//!   `schemes.rs` with the roles taken from its slots. The screen paints
//!   its own background then, so the scheme looks the same in every
//!   terminal.
//! - A file in Ghostty's or Kitty's colour format (`--theme FILE`,
//!   `:theme FILE`): the 16 palette slots, background and foreground, as
//!   mbadolato/iTerm2-Color-Schemes exports hundreds of them.
//!
//! A theme is built once when chosen, never while drawing. RGB values
//! are rounded to the 256-colour cube when the terminal does not claim
//! true colour (`COLORTERM`), so every theme shows everywhere.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ratatui::style::Color;

use crate::schemes::{SCHEMES, Scheme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// What the screen calls it: a built-in name, or a file's stem.
    pub name: String,
    /// No colours: `dim` is the DIM attribute, badges are reversed.
    pub mono: bool,
    /// The whole screen's background and text; `Reset` leaves the
    /// terminal's own.
    pub bg: Color,
    pub fg: Color,
    /// Panel borders when not focused.
    pub border: Color,
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

/// A colour as three bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// `percent` of the way from `self` to `to`.
    pub fn blend(self, to: Rgb, percent: u8) -> Rgb {
        let mix = |a: u8, b: u8| {
            let (a, b, t) = (i32::from(a), i32::from(b), i32::from(percent));
            (a + (b - a) * t / 100) as u8
        };
        Rgb(mix(self.0, to.0), mix(self.1, to.1), mix(self.2, to.2))
    }

    /// Perceived brightness, 0..=255.
    pub fn luma(self) -> u8 {
        let (r, g, b) = (u32::from(self.0), u32::from(self.1), u32::from(self.2));
        ((299 * r + 587 * g + 114 * b) / 1000) as u8
    }

    pub fn is_dark(self) -> bool {
        self.luma() < 128
    }

    /// The nearest colour of the 256-colour palette's cube (16..=231)
    /// and greys (232..=255).
    pub fn indexed(self) -> u8 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let near = |v: u8| {
            let mut best = 0;
            for (i, &l) in LEVELS.iter().enumerate() {
                if (i32::from(v) - i32::from(l)).abs()
                    < (i32::from(v) - i32::from(LEVELS[best])).abs()
                {
                    best = i;
                }
            }
            (best as u8, LEVELS[best])
        };
        let ((ri, r), (gi, g), (bi, b)) = (near(self.0), near(self.1), near(self.2));
        let cube = (16 + 36 * ri + 6 * gi + bi, Rgb(r, g, b));
        let avg = (i32::from(self.0) + i32::from(self.1) + i32::from(self.2)) / 3;
        let k = ((avg - 8 + 5) / 10).clamp(0, 23);
        let level = (8 + 10 * k) as u8;
        let grey = (232 + k as u8, Rgb(level, level, level));
        let dist = |c: Rgb| {
            let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
            d(self.0, c.0) + d(self.1, c.1) + d(self.2, c.2)
        };
        if dist(grey.1) < dist(cube.1) {
            grey.0
        } else {
            cube.0
        }
    }
}

/// What the terminal said about itself before the screen started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Back {
    pub dark: bool,
    pub bg: Option<Rgb>,
    pub fg: Option<Rgb>,
}

impl Default for Back {
    fn default() -> Self {
        Back {
            dark: true,
            bg: None,
            fg: None,
        }
    }
}

/// The 16 palette slots, background and foreground of a terminal colour
/// scheme, as Ghostty and Kitty files hold them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub ansi: [Rgb; 16],
    pub bg: Rgb,
    pub fg: Rgb,
    pub selection: Option<Rgb>,
}

/// Reads a Ghostty (`palette = 4=#rrggbb`, `background = #rrggbb`) or
/// Kitty (`color4 #rrggbb`, `background #rrggbb`) colour file. Other
/// keys are ignored, so a whole Ghostty config file passes too, as long
/// as it holds the colours (a `theme =` line pointing elsewhere does
/// not).
pub fn parse_palette(text: &str) -> Result<Palette, String> {
    let mut ansi = [None; 16];
    let (mut bg, mut fg, mut selection) = (None, None, None);
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line
            .split_once('=')
            .or_else(|| line.split_once(char::is_whitespace))
        else {
            return Err(format!("line {}: not `key = value`", n + 1));
        };
        let (key, value) = (key.trim(), value.trim());
        let slot = if key == "palette" {
            let Some((i, v)) = value.split_once('=') else {
                return Err(format!("line {}: palette needs `N=#rrggbb`", n + 1));
            };
            Some((i.trim(), v.trim()))
        } else {
            key.strip_prefix("color").map(|i| (i, value))
        };
        match slot {
            Some((i, value)) => {
                let Ok(i) = i.parse::<usize>() else {
                    return Err(format!("line {}: {i:?} is not a palette slot", n + 1));
                };
                // Slots past the 16 named ones are the cube; not ours.
                if i < 16 {
                    ansi[i] = Some(hex(value, n + 1)?);
                }
            }
            None => match key {
                "background" => bg = Some(hex(value, n + 1)?),
                "foreground" => fg = Some(hex(value, n + 1)?),
                "selection-background" | "selection_background" => {
                    selection = Some(hex(value, n + 1)?)
                }
                _ => {}
            },
        }
    }
    let mut slots = [Rgb::default(); 16];
    for (i, slot) in ansi.iter().enumerate() {
        slots[i] = slot.ok_or_else(|| format!("missing palette colour {i}"))?;
    }
    Ok(Palette {
        ansi: slots,
        bg: bg.ok_or("missing background")?,
        fg: fg.ok_or("missing foreground")?,
        selection,
    })
}

fn hex(value: &str, line: usize) -> Result<Rgb, String> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    match (digits.len(), u32::from_str_radix(digits, 16)) {
        (6, Ok(v)) => Ok(Rgb::hex(v)),
        _ => Err(format!("line {line}: {value:?} is not #rrggbb")),
    }
}

/// A colour file may be this big, at most.
const FILE_CAP: u64 = 64 * 1024;

fn load(path: &Path) -> Result<Palette, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > FILE_CAP {
        return Err(format!("{size} bytes; a colour file is under {FILE_CAP}"));
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_palette(&text)
}

/// Whether the terminal shows RGB as it is (`COLORTERM`); else RGB is
/// rounded to the 256-colour cube.
pub fn truecolor(colorterm: Option<&str>) -> bool {
    matches!(colorterm, Some("truecolor" | "24bit"))
}

/// Where the theme in use came from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    Builtin(usize),
    File(PathBuf, Palette),
}

struct State {
    back: Back,
    truecolor: bool,
    source: Source,
    theme: Rc<Theme>,
}

thread_local! {
    // Per thread: the screen draws on one thread, and tests that change
    // the theme do not change it for each other.
    static STATE: RefCell<State> = RefCell::new(State {
        back: Back::default(),
        truecolor: true,
        source: Source::Builtin(0),
        theme: Rc::new(build(&Source::Builtin(0), Back::default(), true)),
    });
}

/// The built-in names before the schemes.
const HEAD: [&str; 4] = ["terminal", "dark", "light", "mono"];

/// Every built-in name, in the order `T` cycles them.
pub fn builtins() -> impl Iterator<Item = &'static str> {
    HEAD.into_iter().chain(SCHEMES.iter().map(|s| s.slug))
}

/// The names, for usage and error messages.
pub fn names() -> String {
    builtins().collect::<Vec<_>>().join("|")
}

/// The theme in use.
pub fn pal() -> Rc<Theme> {
    STATE.with(|s| s.borrow().theme.clone())
}

/// What to remember for the next run: a built-in name, or the file.
pub fn chosen() -> String {
    STATE.with(|s| match &s.borrow().source {
        Source::Builtin(i) => builtins().nth(*i).unwrap_or("terminal").to_string(),
        Source::File(path, _) => path.to_string_lossy().into_owned(),
    })
}

fn switch(source: Source) -> Rc<Theme> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.theme = Rc::new(build(&source, s.back, s.truecolor));
        s.source = source;
        s.theme.clone()
    })
}

/// Switches to the built-in theme or the colour file `spec` names.
pub fn choose(spec: &str) -> Result<Rc<Theme>, String> {
    if let Some(i) = builtins().position(|n| n == spec) {
        return Ok(switch(Source::Builtin(i)));
    }
    let path = Path::new(spec);
    if spec.contains('/') || path.is_file() {
        let palette = load(path)?;
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        return Ok(switch(Source::File(path, palette)));
    }
    Err(format!("not one of {} and no such file", names()))
}

/// Switches to the built-in theme called `name`; `false` if there is none.
#[cfg(test)]
pub fn set(name: &str) -> bool {
    builtins().position(|n| n == name).is_some_and(|i| {
        switch(Source::Builtin(i));
        true
    })
}

/// Switches to the next built-in theme and returns it.
pub fn next() -> Rc<Theme> {
    let i = STATE.with(|s| match s.borrow().source {
        Source::Builtin(i) => (i + 1) % builtins().count(),
        Source::File(..) => 0,
    });
    switch(Source::Builtin(i))
}

/// What the terminal said (`probe`), and whether it has true colour:
/// the theme in use is built again with it.
pub fn probed(back: Back, truecolor: bool) {
    let source = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.back = back;
        s.truecolor = truecolor;
        s.source.clone()
    });
    switch(source);
}

fn build(source: &Source, back: Back, truecolor: bool) -> Theme {
    let paint = Paint { truecolor };
    match source {
        Source::Builtin(0) => terminal("terminal", back, paint),
        Source::Builtin(1) => terminal("dark", Back::default(), paint),
        Source::Builtin(2) => terminal(
            "light",
            Back {
                dark: false,
                ..Back::default()
            },
            paint,
        ),
        Source::Builtin(3) => Theme {
            name: "mono".to_string(),
            ..MONO
        },
        Source::Builtin(i) => scheme(&SCHEMES[i - HEAD.len()], paint),
        Source::File(path, palette) => {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            from_palette(name, palette, paint)
        }
    }
}

/// Turns RGB into what the terminal can show.
#[derive(Clone, Copy)]
struct Paint {
    truecolor: bool,
}

impl Paint {
    fn c(self, c: Rgb) -> Color {
        if self.truecolor {
            Color::Rgb(c.0, c.1, c.2)
        } else {
            Color::Indexed(c.indexed())
        }
    }
}

/// The terminal's 16 colours by name: whatever its theme makes them.
const NAMED: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// The roles the 16 ANSI slots give: blue leads, red warns, the bright
/// ones vary. The rest is left `Reset` for the caller to fill.
fn from_slots(name: String, ansi: [Color; 16]) -> Theme {
    let s = |i: usize| ansi[i];
    Theme {
        name,
        mono: false,
        bg: Color::Reset,
        fg: Color::Reset,
        border: Color::Reset,
        accent: s(4),
        dim: Color::Reset,
        strong: Color::Reset,
        on_badge: Color::Reset,
        on_key: Color::Reset,
        key: Color::Reset,
        selection: Color::Reset,
        code: s(3),
        ok: s(2),
        warn: s(3),
        bad: s(1),
        stopped: s(9),
        info: s(6),
        plan: s(5),
        research: s(12),
        vendors: [s(3), s(2), s(12), s(5), s(8)],
        speakers: [
            s(4),
            s(3),
            s(2),
            s(5),
            s(6),
            s(1),
            s(12),
            s(11),
            s(10),
            s(13),
            s(14),
            s(9),
        ],
    }
}

fn terminal(name: &str, back: Back, paint: Paint) -> Theme {
    let either = |dark: u8, light: u8| Color::Indexed(if back.dark { dark } else { light });
    let (selection, key, on_key) = match (back.bg, back.fg) {
        (Some(bg), Some(fg)) => (
            paint.c(bg.blend(fg, 15)),
            paint.c(bg.blend(fg, 30)),
            Color::Reset,
        ),
        _ => (either(236, 254), either(250, 252), Color::Black),
    };
    let on_badge = match back.bg {
        Some(bg) => paint.c(bg),
        None if back.dark => Color::Black,
        None => Color::White,
    };
    Theme {
        border: Color::DarkGray,
        dim: Color::DarkGray,
        on_badge,
        on_key,
        key,
        selection,
        ..from_slots(name.to_string(), NAMED)
    }
}

/// A terminal colour file: the slots give the roles, the background and
/// foreground the rest, so an odd palette still reads.
fn from_palette(name: String, p: &Palette, paint: Paint) -> Theme {
    let (bg, fg) = (p.bg, p.fg);
    Theme {
        bg: paint.c(bg),
        fg: paint.c(fg),
        border: paint.c(fg.blend(bg, 60)),
        dim: paint.c(fg.blend(bg, 40)),
        strong: paint.c(fg),
        on_badge: paint.c(bg),
        on_key: paint.c(fg),
        key: paint.c(bg.blend(fg, 30)),
        selection: paint.c(p.selection.unwrap_or(bg.blend(fg, 15))),
        code: paint.c(p.ansi[3].blend(fg, 40)),
        ..from_slots(name, p.ansi.map(|c| paint.c(c)))
    }
}

/// A base16 scheme: every slot has a role of its own, so imece's roles
/// map straight onto them (base0F, brown, is left out: in some schemes
/// it is too close to the background).
fn scheme(s: &Scheme, paint: Paint) -> Theme {
    let b = s.base.map(Rgb::hex);
    let c = |i: usize| paint.c(b[i]);
    // A key hint sits on the comment grey (base03); its text is the
    // lighter or darker of the text colours, whichever reads on it.
    let apart = |i: usize| (i32::from(b[i].luma()) - i32::from(b[3].luma())).abs();
    let on_key = if apart(7) >= apart(5) { c(7) } else { c(5) };
    let pale_rgb = |i: usize| b[i].blend(b[5], 45);
    let mut speakers = [
        b[0xD],
        b[0xA],
        b[0xB],
        b[0xE],
        b[9],
        b[0xC],
        b[8],
        pale_rgb(0xD),
        pale_rgb(0xA),
        pale_rgb(0xB),
        pale_rgb(0xE),
        pale_rgb(0xC),
    ];
    // A scheme may give two slots one colour (Rose Pine's orange is its
    // purple): a speaker that repeats an earlier one fades a step
    // towards the background until it does not.
    for i in 1..speakers.len() {
        for _ in 0..4 {
            if speakers[..i].contains(&speakers[i]) {
                speakers[i] = speakers[i].blend(b[0], 25);
            }
        }
    }
    Theme {
        name: s.slug.to_string(),
        mono: false,
        bg: c(0),
        fg: c(5),
        border: c(3),
        accent: c(9),
        dim: c(4),
        strong: c(6),
        on_badge: c(0),
        on_key,
        key: c(3),
        selection: c(2),
        code: paint.c(b[0xA].blend(b[5], 35)),
        ok: c(0xB),
        warn: c(0xA),
        bad: c(8),
        stopped: paint.c(b[8].blend(b[5], 40)),
        info: c(0xC),
        plan: c(0xE),
        research: c(0xD),
        vendors: [c(9), c(0xB), c(0xD), c(0xE), c(4)],
        speakers: speakers.map(|s| paint.c(s)),
    }
}

const MONO: Theme = Theme {
    name: String::new(),
    mono: true,
    bg: Color::Reset,
    fg: Color::Reset,
    border: Color::Reset,
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

impl Theme {
    /// Every role colour, for tests that check them all.
    #[cfg(test)]
    fn roles(&self) -> Vec<Color> {
        let mut all = vec![
            self.border,
            self.accent,
            self.dim,
            self.strong,
            self.on_badge,
            self.on_key,
            self.key,
            self.selection,
            self.code,
            self.ok,
            self.warn,
            self.bad,
            self.stopped,
            self.info,
            self.plan,
            self.research,
        ];
        all.extend(self.vendors);
        all.extend(self.speakers);
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GHOSTTY: &str = "palette = 0=#21222c
palette = 1=#ff5555
palette = 2=#50fa7b
palette = 3=#f1fa8c
palette = 4=#bd93f9
palette = 5=#ff79c6
palette = 6=#8be9fd
palette = 7=#f8f8f2
palette = 8=#6272a4
palette = 9=#ff6e6e
palette = 10=#69ff94
palette = 11=#ffffa5
palette = 12=#d6acff
palette = 13=#ff92df
palette = 14=#a4ffff
palette = 15=#ffffff
background = #282a36
foreground = #f8f8f2
cursor-color = #f8f8f2
cursor-text = #282a36
selection-background = #44475a
selection-foreground = #ffffff
";

    fn kitty() -> String {
        let mut text = String::from("# Dracula for kitty\nfont_size 12\n");
        for (i, line) in GHOSTTY.lines().take(16).enumerate() {
            let hex = line.rsplit('=').next().unwrap();
            text.push_str(&format!("color{i}   {hex}\n"));
        }
        text.push_str("background #282a36\nforeground f8f8f2\nselection_background #44475a\n");
        text
    }

    fn reset() {
        probed(Back::default(), true);
        assert!(set("terminal"));
    }

    #[test]
    fn themes_are_found_by_name_and_cycle() {
        reset();
        assert_eq!(pal().name, "terminal");
        assert!(set("light"));
        assert_eq!(pal().name, "light");
        assert!(!set("neon"));
        assert_eq!(pal().name, "light", "an unknown name changes nothing");
        assert_eq!(next().name, "mono");
        assert_eq!(next().name, "dracula");
        assert_eq!(chosen(), "dracula");
        assert!(names().starts_with("terminal|dark|light|mono|dracula|nord|"));
        let last = builtins().last().unwrap();
        assert!(set(last));
        assert_eq!(next().name, "terminal", "around the end");
        assert_eq!(
            choose("gruvbox-light-medium").unwrap().name,
            "gruvbox-light-medium"
        );
        let e = choose("neon").unwrap_err();
        assert!(
            e.contains("terminal|dark") && e.ends_with("no such file"),
            "{e}"
        );
        assert_eq!(pal().name, "gruvbox-light-medium");
        reset();
    }

    #[test]
    fn mono_has_no_colour_at_all() {
        reset();
        let m = choose("mono").unwrap();
        assert!(m.mono);
        assert_eq!(m.name, "mono");
        assert!(m.roles().iter().all(|&c| c == Color::Reset));
        assert_eq!((m.bg, m.fg), (Color::Reset, Color::Reset));
        reset();
    }

    #[test]
    fn the_terminal_theme_names_the_terminals_colours_and_shades_with_what_it_said() {
        reset();
        let t = pal();
        assert!(!t.mono);
        assert_eq!(
            (t.bg, t.fg, t.strong),
            (Color::Reset, Color::Reset, Color::Reset)
        );
        let named = |c: &Color| !matches!(c, Color::Rgb(..) | Color::Indexed(_));
        assert!(t.roles().iter().filter(|c| named(c)).count() == t.roles().len() - 2);
        // Unasked, the terminal is taken as dark: neutral greys and black
        // on badges.
        assert_eq!(
            (t.selection, t.key, t.on_key),
            (Color::Indexed(236), Color::Indexed(250), Color::Black)
        );
        assert_eq!(t.on_badge, Color::Black);
        assert_eq!(
            (t.accent, t.bad, t.ok, t.warn),
            (Color::Blue, Color::Red, Color::Green, Color::Yellow)
        );
        assert_eq!(t.vendors[0], Color::Yellow);
        assert_eq!(
            t.speakers
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            12
        );
        // A light terminal that answered: its own background on badges,
        // the selection a shade of its text over its background.
        let (bg, fg) = (Rgb(0xfd, 0xf6, 0xe3), Rgb(0x58, 0x6e, 0x75));
        probed(
            Back {
                dark: false,
                bg: Some(bg),
                fg: Some(fg),
            },
            true,
        );
        let t = pal();
        assert_eq!(t.on_badge, Color::Rgb(0xfd, 0xf6, 0xe3));
        assert_eq!(t.on_key, Color::Reset);
        let sel = bg.blend(fg, 15);
        assert_eq!(t.selection, Color::Rgb(sel.0, sel.1, sel.2));
        assert_ne!(t.key, t.selection);
        // `dark` and `light` assume instead of asking.
        assert_eq!(choose("dark").unwrap().on_badge, Color::Black);
        let light = choose("light").unwrap();
        assert_eq!(light.on_badge, Color::White);
        assert_eq!(
            (light.selection, light.key),
            (Color::Indexed(254), Color::Indexed(252))
        );
        // A terminal that said only that it is dark (COLORFGBG).
        probed(Back::default(), true);
        assert!(set("terminal"));
        assert_eq!(pal().on_badge, Color::Black);
        reset();
    }

    #[test]
    fn without_true_colour_rgb_is_rounded_to_the_cube() {
        reset();
        assert!(truecolor(Some("truecolor")));
        assert!(truecolor(Some("24bit")));
        assert!(!truecolor(Some("yes")));
        assert!(!truecolor(None));
        probed(Back::default(), false);
        let d = choose("dracula").unwrap();
        let bg = Rgb::hex(SCHEMES[0].base[0]);
        assert_eq!(d.bg, Color::Indexed(bg.indexed()));
        assert!(d.roles().iter().all(|c| matches!(c, Color::Indexed(_))));
        probed(Back::default(), true);
        assert_eq!(
            pal().bg,
            Color::Rgb(bg.0, bg.1, bg.2),
            "built again with true colour"
        );
        reset();
    }

    #[test]
    fn rgb_arithmetic() {
        assert_eq!(Rgb::hex(0x282a36), Rgb(0x28, 0x2a, 0x36));
        assert_eq!(Rgb(0, 0, 0).blend(Rgb(100, 200, 50), 50), Rgb(50, 100, 25));
        assert_eq!(
            Rgb(100, 100, 100).blend(Rgb(0, 0, 0), 0),
            Rgb(100, 100, 100)
        );
        assert_eq!(Rgb(100, 100, 100).blend(Rgb(0, 0, 0), 100), Rgb(0, 0, 0));
        assert_eq!(Rgb(255, 255, 255).luma(), 255);
        assert_eq!(Rgb(0, 0, 0).luma(), 0);
        assert_eq!(Rgb(255, 0, 0).luma(), 76);
        assert!(Rgb(0x28, 0x2a, 0x36).is_dark());
        assert!(!Rgb(0xfd, 0xf6, 0xe3).is_dark());
        assert!(!Rgb(128, 128, 128).is_dark());
        assert_eq!(Rgb(0, 0, 0).indexed(), 16);
        assert_eq!(Rgb(255, 255, 255).indexed(), 231);
        assert_eq!(Rgb(95, 135, 175).indexed(), 67);
        assert_eq!(Rgb(128, 128, 128).indexed(), 244, "a grey");
        assert_eq!(Rgb(18, 18, 18).indexed(), 233);
        assert_eq!(Rgb(250, 250, 250).indexed(), 231, "nearer the cube's white");
        assert_eq!(Rgb(0x28, 0x2a, 0x36).indexed(), 236);
        assert_eq!(Rgb(0xff, 0x55, 0x55).indexed(), 203);
    }

    #[test]
    fn every_scheme_reads_on_its_own_background() {
        reset();
        for s in &SCHEMES {
            let t = choose(s.slug).unwrap();
            assert_eq!(t.name, s.slug);
            let b = s.base.map(Rgb::hex);
            assert_eq!(b[0].is_dark(), s.dark, "{}", s.slug);
            let rgb = |c: Color| match c {
                Color::Rgb(r, g, b) => Rgb(r, g, b),
                other => panic!("{}: {other:?} is not RGB", s.slug),
            };
            let apart = |c: Color| (i32::from(rgb(c).luma()) - i32::from(b[0].luma())).abs();
            assert_eq!(rgb(t.bg), b[0]);
            let text = [
                t.fg, t.strong, t.accent, t.code, t.ok, t.warn, t.bad, t.stopped, t.info, t.plan,
                t.research,
            ];
            for c in text.iter().chain(&t.vendors).chain(&t.speakers) {
                assert!(apart(*c) >= 40, "{}: {c:?} on {:?}", s.slug, b[0]);
            }
            for c in [t.dim, t.border] {
                assert!(apart(c) >= 25, "{}: {c:?} on {:?}", s.slug, b[0]);
            }
            assert!(apart(t.selection) >= 10, "{}: selection", s.slug);
            assert_eq!(rgb(t.on_badge), b[0]);
            assert!(
                (i32::from(rgb(t.on_key).luma()) - i32::from(rgb(t.key).luma())).abs() >= 60,
                "{}: key",
                s.slug
            );
            assert_eq!(rgb(t.accent), b[9], "orange leads, as imece's own colour");
            assert_eq!((rgb(t.bad), rgb(t.ok)), (b[8], b[0xB]));
            assert_eq!(
                t.speakers
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                12,
                "{}",
                s.slug
            );
        }
        reset();
    }

    #[test]
    fn ghostty_and_kitty_files_give_the_same_palette() {
        let p = parse_palette(GHOSTTY).unwrap();
        assert_eq!(p.ansi[0], Rgb(0x21, 0x22, 0x2c));
        assert_eq!(p.ansi[15], Rgb(0xff, 0xff, 0xff));
        assert_eq!(p.bg, Rgb(0x28, 0x2a, 0x36));
        assert_eq!(p.fg, Rgb(0xf8, 0xf8, 0xf2));
        assert_eq!(p.selection, Some(Rgb(0x44, 0x47, 0x5a)));
        assert_eq!(parse_palette(&kitty()).unwrap(), p);
        let no_selection =
            GHOSTTY.replace("selection-background = #44475a\n", "palette = 16=#000000\n");
        assert_eq!(parse_palette(&no_selection).unwrap().selection, None);
        let errors = [
            (
                GHOSTTY.replace("palette = 3=#f1fa8c\n", ""),
                "missing palette colour 3",
            ),
            (
                GHOSTTY.replace("background = #282a36\n", ""),
                "missing background",
            ),
            (
                GHOSTTY.replace("foreground = #f8f8f2\n", ""),
                "missing foreground",
            ),
            (
                GHOSTTY.replace("#282a36", "#282a3"),
                "line 17: \"#282a3\" is not #rrggbb",
            ),
            (
                GHOSTTY.replace("foreground = #f8f8f2", "foreground = #f8f8fg"),
                "line 18: \"#f8f8fg\" is not #rrggbb",
            ),
            (
                GHOSTTY.replace("palette = 0=#21222c", "palette = #21222c"),
                "line 1: palette needs `N=#rrggbb`",
            ),
            (
                GHOSTTY.replace("palette = 0=#21222c", "palette = x=#21222c"),
                "line 1: \"x\" is not a palette slot",
            ),
            (
                GHOSTTY.replace("palette = 0=#21222c", "what"),
                "line 1: not `key = value`",
            ),
        ];
        for (text, why) in errors {
            assert_eq!(parse_palette(&text).unwrap_err(), why);
        }
    }

    #[test]
    fn a_colour_file_is_a_theme_named_after_it() {
        reset();
        let dir = std::env::temp_dir().join(format!("imece-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Dracula.conf");
        fs::write(&file, GHOSTTY).unwrap();
        let t = choose(file.to_str().unwrap()).unwrap();
        assert_eq!(t.name, "Dracula");
        assert_eq!(chosen(), fs::canonicalize(&file).unwrap().to_string_lossy());
        assert_eq!(t.bg, Color::Rgb(0x28, 0x2a, 0x36));
        assert_eq!(t.fg, Color::Rgb(0xf8, 0xf8, 0xf2));
        assert_eq!(t.on_badge, t.bg);
        assert_eq!(t.selection, Color::Rgb(0x44, 0x47, 0x5a), "the file's own");
        assert_eq!(t.accent, Color::Rgb(0xbd, 0x93, 0xf9), "blue");
        assert_eq!(t.bad, Color::Rgb(0xff, 0x55, 0x55));
        assert_ne!(t.dim, t.fg);
        assert_ne!(t.border, t.dim);
        assert_eq!(next().name, "terminal", "T after a file starts over");
        // Said as a path, a missing file is an error, not a name.
        let gone = dir.join("gone.conf");
        let e = choose(gone.to_str().unwrap()).unwrap_err();
        assert!(e.contains("No such file") || e.contains("not found"), "{e}");
        assert_eq!(pal().name, "terminal", "unchanged");
        fs::write(&gone, "x".repeat(FILE_CAP as usize + 1)).unwrap();
        let e = choose(gone.to_str().unwrap()).unwrap_err();
        assert!(e.contains("a colour file is under"), "{e}");
        fs::write(&gone, "font_size 12\n").unwrap();
        assert_eq!(
            choose(gone.to_str().unwrap()).unwrap_err(),
            "missing palette colour 0"
        );
        // A bare file name in the current directory counts when it is one.
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let t = choose("Dracula.conf");
        std::env::set_current_dir(cwd).unwrap();
        assert_eq!(t.unwrap().name, "Dracula");
        fs::remove_dir_all(&dir).unwrap();
        reset();
    }
}
