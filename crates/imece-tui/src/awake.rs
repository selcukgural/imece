//! Keeping the machine awake while an agent is mid-turn (`--awake`,
//! `:awake`): the system's own tool runs as a child of imece, in a process
//! group of its own, while anything is mid-turn, and goes when nothing is
//! or imece ends.

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

/// The command that keeps this machine from sleeping while it runs, for
/// imece's process `pid`: `caffeinate -i -w PID` on macOS (it also ends
/// with imece), `systemd-inhibit` holding an idle and sleep lock on Linux;
/// none elsewhere.
pub fn command(pid: u32) -> Option<Vec<String>> {
    let words: Vec<String> = if cfg!(target_os = "macos") {
        vec![
            "caffeinate".into(),
            "-i".into(),
            "-w".into(),
            pid.to_string(),
        ]
    } else if cfg!(target_os = "linux") {
        [
            "systemd-inhibit",
            "--what=idle:sleep",
            "--who=imece",
            "--why=agents are mid-turn",
            "--mode=block",
            "sleep",
            "infinity",
        ]
        .map(String::from)
        .to_vec()
    } else {
        return None;
    };
    Some(words)
}

/// The hold on the machine's sleep.
pub struct Awake {
    /// Whether the person asked for it.
    pub on: bool,
    /// What holds it; none where there is nothing, or once it failed.
    program: Option<Vec<String>>,
    child: Option<Child>,
}

impl Awake {
    pub fn new(on: bool, program: Option<Vec<String>>) -> Awake {
        Awake {
            on,
            program,
            child: None,
        }
    }

    /// Whether the machine is held awake now.
    #[cfg(test)]
    pub fn holding(&self) -> bool {
        self.child.is_some()
    }

    /// Holds the machine awake while `busy` and asked for, lets it go
    /// otherwise. A program that does not start, or ends by itself, is
    /// given up on for the run: that is said once, as the error.
    pub fn hold(&mut self, busy: bool) -> Result<(), String> {
        if let Some(c) = &mut self.child
            && let Ok(Some(status)) = c.try_wait()
        {
            self.child = None;
            let name = self.give_up();
            return Err(format!("{name} ended ({status})"));
        }
        if !(busy && self.on) {
            self.release();
            return Ok(());
        }
        if self.child.is_some() {
            return Ok(());
        }
        let Some(words) = &self.program else {
            return Ok(());
        };
        let started = Command::new(&words[0])
            .args(&words[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
        match started {
            Ok(c) => {
                self.child = Some(c);
                Ok(())
            }
            Err(e) => {
                let name = self.give_up();
                Err(format!("{name}: {e}"))
            }
        }
    }

    /// No more tries this run; the program's name, to say what failed.
    fn give_up(&mut self) -> String {
        let words = self.program.take().unwrap_or_default();
        words.into_iter().next().unwrap_or_default()
    }

    /// The hold goes: its whole process group (systemd-inhibit's `sleep`
    /// with it), then the child is waited for.
    fn release(&mut self) {
        if let Some(mut c) = self.child.take() {
            if let Ok(pgid) = i32::try_from(c.id()) {
                // SAFETY: killpg only sends a signal to the group this
                // child leads.
                unsafe {
                    libc::killpg(pgid, libc::SIGTERM);
                }
            }
            let _ = c.wait();
        }
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alive(pid: u32) -> bool {
        // SAFETY: signal 0 only asks whether the process is there.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    fn sleeper() -> Option<Vec<String>> {
        Some(vec!["sleep".into(), "30".into()])
    }

    #[test]
    fn the_hold_comes_while_busy_and_asked_for_and_goes_otherwise() {
        let mut a = Awake::new(true, sleeper());
        a.hold(false).unwrap();
        assert!(!a.holding(), "nothing mid-turn");
        a.hold(true).unwrap();
        assert!(a.holding());
        let pid = a.child.as_ref().unwrap().id();
        assert!(alive(pid));
        a.hold(true).unwrap();
        assert_eq!(a.child.as_ref().unwrap().id(), pid, "the same one");
        a.hold(false).unwrap();
        assert!(!a.holding());
        assert!(!alive(pid), "its group went");
        // Not asked for: nothing, busy or not; asked for again: again.
        a.on = false;
        a.hold(true).unwrap();
        assert!(!a.holding());
        a.on = true;
        a.hold(true).unwrap();
        let pid = a.child.as_ref().unwrap().id();
        a.on = false;
        a.hold(true).unwrap();
        assert!(!a.holding() && !alive(pid), "turned off mid-turn");
        // Dropped while holding: it goes too.
        a.on = true;
        a.hold(true).unwrap();
        let pid = a.child.as_ref().unwrap().id();
        drop(a);
        assert!(!alive(pid));
    }

    #[test]
    fn a_program_that_fails_is_said_once_and_not_tried_again() {
        let mut a = Awake::new(true, Some(vec!["/nonexistent/caffeinate".into()]));
        let e = a.hold(true).unwrap_err();
        assert!(e.starts_with("/nonexistent/caffeinate: "), "{e}");
        assert_eq!(a.hold(true), Ok(()));
        assert!(!a.holding());
        // One that ends by itself: said when seen, then given up.
        let mut a = Awake::new(true, Some(vec!["true".into()]));
        a.hold(true).unwrap();
        let mut ended = None;
        for _ in 0..200 {
            if let Err(e) = a.hold(true) {
                ended = Some(e);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let e = ended.expect("it ended");
        assert!(e.starts_with("true ended (exit status: 0)"), "{e}");
        assert_eq!(a.hold(true), Ok(()));
        assert!(!a.holding());
        // Nowhere to hold it: nothing, quietly.
        let mut a = Awake::new(true, None);
        assert_eq!(a.hold(true), Ok(()));
        assert!(!a.holding());
    }

    #[test]
    fn each_system_has_its_own_tool() {
        let c = command(42);
        if cfg!(target_os = "macos") {
            assert_eq!(c.unwrap(), ["caffeinate", "-i", "-w", "42"]);
        } else if cfg!(target_os = "linux") {
            let c = c.unwrap();
            assert_eq!(c[0], "systemd-inhibit");
            assert!(c.contains(&"--what=idle:sleep".to_string()));
            assert_eq!(c[c.len() - 2..], ["sleep", "infinity"]);
        }
    }
}
