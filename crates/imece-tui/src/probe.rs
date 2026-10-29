//! Asks the terminal, before the screen starts, what its background and
//! foreground are (xterm's OSC 11 and OSC 10 queries, which iTerm2,
//! Ghostty, Kitty, WezTerm, Alacritty, foot and tmux answer), so the
//! `terminal` theme knows whether it is dark and what to shade a
//! selection with. A terminal that does not answer in time is taken as
//! dark, or as `COLORFGBG` says.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::theme::{Back, Rgb};

/// How long the terminal has to answer, all in all.
const WAIT: Duration = Duration::from_millis(200);

/// What the terminal on stdin and stdout says about itself.
pub fn terminal() -> Back {
    let mut back = Back::default();
    if let Some(dark) = std::env::var("COLORFGBG").ok().and_then(|v| colorfgbg(&v)) {
        back.dark = dark;
    }
    // SAFETY: isatty only reads the descriptor's state.
    let tty = unsafe { libc::isatty(0) == 1 && libc::isatty(1) == 1 };
    if !tty || std::env::var("TERM").is_ok_and(|t| t == "dumb") {
        return back;
    }
    let (bg, fg) = parse(&ask().unwrap_or_default());
    back.bg = bg;
    back.fg = fg;
    if let Some(bg) = bg {
        back.dark = bg.is_dark();
    }
    back
}

/// Sends the two queries and collects what comes back within `WAIT`.
fn ask() -> io::Result<Vec<u8>> {
    enable_raw_mode()?;
    let result = (|| {
        let mut out = io::stdout();
        out.write_all(b"\x1b]11;?\x1b\\\x1b]10;?\x1b\\")?;
        out.flush()?;
        let mut reply = Vec::new();
        let deadline = Instant::now() + WAIT;
        while answers(&reply) < 2 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !readable(left) {
                break;
            }
            let mut buf = [0u8; 256];
            // SAFETY: the buffer is as long as declared; stdin is open.
            let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                break;
            }
            reply.extend_from_slice(&buf[..n as usize]);
        }
        Ok(reply)
    })();
    disable_raw_mode()?;
    result
}

/// Whether stdin has bytes to read within `wait`.
fn readable(wait: Duration) -> bool {
    let mut fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = i32::try_from(wait.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: one pollfd, as declared.
    unsafe { libc::poll(&mut fd, 1, ms) > 0 }
}

/// How many complete OSC replies `reply` holds (each ends with ESC \ or
/// BEL).
fn answers(reply: &[u8]) -> usize {
    let bells = reply.iter().filter(|&&b| b == 7).count();
    let sts = reply.windows(2).filter(|w| w == b"\x1b\\").count();
    bells + sts
}

/// The background and foreground in the replies, which look like
/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB ESC \`.
fn parse(reply: &[u8]) -> (Option<Rgb>, Option<Rgb>) {
    let text = String::from_utf8_lossy(reply);
    let find = |code: &str| {
        let key = format!("]{code};rgb:");
        let rest = &text[text.find(&key)? + key.len()..];
        let mut parts = rest.split('/');
        let r = channel(parts.next()?)?;
        let g = channel(parts.next()?)?;
        let b = channel(parts.next()?)?;
        Some(Rgb(r, g, b))
    };
    (find("11"), find("10"))
}

/// One channel: one to four hex digits, scaled to a byte.
fn channel(s: &str) -> Option<u8> {
    let digits: String = s
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .take(4)
        .collect();
    let v = u32::from_str_radix(&digits, 16).ok()?;
    let max = (1u32 << (4 * digits.len() as u32)) - 1;
    Some((v * 255 / max) as u8)
}

/// `COLORFGBG` (`15;0`, set by rxvt and Konsole): the last field is the
/// background's ANSI index, dark for 0 to 6 and 8.
fn colorfgbg(v: &str) -> Option<bool> {
    let bg: u8 = v.rsplit(';').next()?.trim().parse().ok()?;
    Some(matches!(bg, 0..=6 | 8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_replies_give_both_colours_in_any_order_and_width() {
        let reply = b"\x1b]10;rgb:f8f8/f8f8/f2f2\x1b\\\x1b]11;rgb:2828/2a2a/3636\x07";
        assert_eq!(
            parse(reply),
            (Some(Rgb(0x28, 0x2a, 0x36)), Some(Rgb(0xf8, 0xf8, 0xf2)))
        );
        assert_eq!(answers(reply), 2);
        assert_eq!(answers(b"\x1b]11;rgb:28/2a/36"), 0, "not ended yet");
        assert_eq!(
            parse(b"\x1b]11;rgb:28/2a/36\x1b\\"),
            (Some(Rgb(0x28, 0x2a, 0x36)), None)
        );
        assert_eq!(
            parse(b"\x1b]11;rgb:f/0/8\x1b\\"),
            (Some(Rgb(255, 0, 0x88)), None)
        );
        assert_eq!(
            parse(b"\x1b]11;rgb:2828/2a2a\x1b\\"),
            (None, None),
            "a channel short"
        );
        assert_eq!(parse(b"\x1b]11;rgb:zz/00/00\x1b\\"), (None, None));
        assert_eq!(parse(b"garbage"), (None, None));
        assert_eq!(parse(b""), (None, None));
    }

    #[test]
    fn colorfgbg_names_dark_backgrounds() {
        assert_eq!(colorfgbg("15;0"), Some(true));
        assert_eq!(colorfgbg("0;15"), Some(false));
        assert_eq!(colorfgbg("15;default;8"), Some(true));
        assert_eq!(colorfgbg("7"), Some(false));
        assert_eq!(colorfgbg("15;default"), None);
        assert_eq!(colorfgbg(""), None);
    }
}
