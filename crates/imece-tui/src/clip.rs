//! Copying text to the person's clipboard: the system's own tool where
//! there is one, otherwise the terminal's (OSC 52, which also works over
//! ssh in terminals that allow it).

use std::io::{self, Write};
use std::process::{Command, Stdio};

/// Copies `text`; says how, or why it could not.
pub fn copy(text: &str) -> Result<&'static str, String> {
    for (program, args) in tools() {
        if pipe(program, args, text).is_ok() {
            return Ok(program.rsplit('/').next().unwrap_or(program));
        }
    }
    osc52(text).map_err(|e| e.to_string())?;
    Ok("OSC 52")
}

/// The system's clipboard tools to try, in order.
fn tools() -> Vec<(&'static str, &'static [&'static str])> {
    if cfg!(target_os = "macos") {
        // A fixed path: a `pbcopy` earlier on PATH does not get the text.
        return vec![("/usr/bin/pbcopy", &[])];
    }
    let mut tools: Vec<(&'static str, &'static [&'static str])> = Vec::new();
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        tools.push(("wl-copy", &[]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        tools.push(("xclip", &["-selection", "clipboard"]));
        tools.push(("xsel", &["--clipboard", "--input"]));
    }
    tools
}

fn pipe(program: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no stdin"))?
        .write_all(text.as_bytes())?;
    if child.wait()?.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{program} failed")))
    }
}

fn osc52(text: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
            ("çok", "w6dvaw=="),
        ] {
            assert_eq!(base64(raw.as_bytes()), enc, "{raw}");
        }
    }
}
