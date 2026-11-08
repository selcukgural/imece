//! `:ps` on live sessions (record 129): the processes under each agent's
//! session, read on a thread of their own, and one of them ended; and
//! the processes left from an earlier run of imece, ended on the person's
//! word.

use super::*;
use crate::procs::{self, Left, Proc, Refusal};

type Read = Result<(Vec<(AgentId, Vec<Proc>)>, Vec<Left>), String>;

/// The name imece's program runs under.
fn own() -> String {
    procs::own_name().unwrap_or_else(|| "imece".into())
}

/// The looks at the processes, one at a time.
pub(super) struct Looks {
    tx: Sender<Read>,
    rx: Receiver<Read>,
}

impl Default for Looks {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Looks { tx, rx }
    }
}

impl Live {
    /// Each agent's session's process, for those running one.
    fn session_pids(&self) -> Vec<(AgentId, u32)> {
        (self.slots.iter().enumerate())
            .filter_map(|(id, slot)| match slot {
                Slot::Running(s) => s.pid().filter(|&p| p > 0).map(|p| (id, p)),
                _ => None,
            })
            .collect()
    }

    /// Reads the sessions' processes on a thread; the answer comes to
    /// [`take_procs`](Self::take_procs). Nothing while a look is under way.
    pub fn look_procs(&mut self, w: &mut World) {
        if w.procs.looking {
            return;
        }
        w.procs.looking = true;
        let (roots, tx) = (self.session_pids(), self.looks.tx.clone());
        let worktrees = self.worktrees();
        thread::spawn(move || {
            let read = procs::read(&roots).and_then(|agents| {
                let left = procs::leftovers(&worktrees, &own(), std::process::id())?;
                Ok((agents, left))
            });
            let _ = tx.send(read);
        });
    }

    /// What a look found, when it came: whether anything did.
    pub(super) fn take_procs(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok(read) = self.looks.rx.try_recv() {
            any = true;
            w.procs.looking = false;
            match read {
                Ok((agents, left)) => {
                    w.procs.agents = agents;
                    w.procs.left = left;
                    w.procs.failed = None;
                }
                Err(why) => w.procs.failed = Some(why),
            }
        }
        any
    }

    /// Ends process `pid` under agent `id`'s session (`procs::end`: never
    /// the session itself or imece's relay); the journal says so, and the
    /// processes are read again.
    pub fn end_proc(&mut self, w: &mut World, id: AgentId, pid: u32) -> Result<(), Refusal> {
        let root = match self.slots.get(id) {
            Some(Slot::Running(s)) => s.pid().ok_or(Refusal::NotUnder)?,
            _ => return Err(Refusal::NotUnder),
        };
        let name = proc_name(w, id, pid);
        procs::end(pid, root, &name)?;
        w.agents[id].trace(w.tick, format!("ended by the person: {name} ({pid})"));
        w.note(Event::Killed {
            agent: id,
            pid,
            name,
        });
        self.look_procs(w);
        Ok(())
    }
}

impl Live {
    /// Ends the processes `asked` left from an earlier run (each with the
    /// first of its tree), each checked again first (`procs::end_left`):
    /// how many were; the processes are read again.
    pub fn end_left(&mut self, w: &mut World, asked: &[(u32, u32)]) -> usize {
        let n = procs::end_left(asked, &self.worktrees(), &own(), std::process::id());
        self.look_procs(w);
        n
    }
}

/// The name process `pid` of agent `id` had when last read; empty when
/// it was not.
pub(crate) fn proc_name(w: &World, id: AgentId, pid: u32) -> String {
    (w.procs.agents.iter())
        .filter(|(a, _)| *a == id)
        .flat_map(|(_, ps)| ps.iter())
        .find(|p| p.pid == pid)
        .map_or(String::new(), |p| p.name.clone())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn nothing_without_a_session_and_a_look_comes_once() {
        let mut w = team(1);
        let mut live = idle_live(2);
        assert_eq!(live.end_proc(&mut w, 1, 1234), Err(Refusal::NotUnder));
        assert_eq!(live.end_proc(&mut w, 9, 1234), Err(Refusal::NotUnder));
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Killed { .. }))
        );
        live.look_procs(&mut w);
        assert!(w.procs.looking);
        live.look_procs(&mut w);
        let start = std::time::Instant::now();
        while !live.take_procs(&mut w) {
            assert!(start.elapsed().as_secs() < 10, "no answer");
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!w.procs.looking);
        assert_eq!(w.procs.agents, vec![], "no session runs");
        assert_eq!(w.procs.left, vec![], "no run left anything");
        assert!(!live.take_procs(&mut w), "one look, one answer");
        // A failed look says why and keeps what was read.
        w.procs.agents = vec![(1, Vec::new())];
        live.looks.tx.send(Err("ps did not answer".into())).unwrap();
        assert!(live.take_procs(&mut w));
        assert_eq!(w.procs.failed.as_deref(), Some("ps did not answer"));
        assert_eq!(w.procs.agents, vec![(1, Vec::new())]);
        let left = Left {
            place: "p/claude-1".into(),
            procs: Vec::new(),
        };
        (live.looks.tx.send(Ok((Vec::new(), vec![left.clone()])))).unwrap();
        live.take_procs(&mut w);
        assert_eq!(w.procs.failed, None);
        assert_eq!(w.procs.left, vec![left]);
        assert_eq!(w.procs.agents, vec![]);
        // Nothing left there to end: none is, and a look is asked.
        while live.take_procs(&mut w) {}
        w.procs.looking = false;
        assert_eq!(live.end_left(&mut w, &[(u32::MAX - 1, u32::MAX - 1)]), 0);
        assert!(w.procs.looking);
    }

    #[test]
    fn imece_is_known_by_its_own_program_s_name() {
        assert_eq!(own(), procs::own_name().unwrap());
    }

    #[test]
    fn a_process_is_named_as_last_read() {
        let mut w = team(2);
        let p = |pid, name: &str| Proc {
            pid,
            ppid: 1,
            cpu: 0,
            rss: 0,
            name: name.into(),
        };
        w.procs.agents = vec![
            (1, vec![p(10, "claude"), p(11, "cargo")]),
            (2, vec![p(12, "node")]),
        ];
        assert_eq!(proc_name(&w, 1, 11), "cargo");
        assert_eq!(proc_name(&w, 2, 11), "", "not its");
        assert_eq!(proc_name(&w, 2, 12), "node");
        assert_eq!(proc_name(&w, 1, 99), "");
    }
}
