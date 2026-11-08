//! `:ps` on live sessions (record 129): the processes under each agent's
//! session, read on a thread of their own, and one of them ended.

use super::*;
use crate::procs::{self, Proc, Refusal};

type Read = Result<Vec<(AgentId, Vec<Proc>)>, String>;

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
        thread::spawn(move || {
            let _ = tx.send(procs::read(&roots));
        });
    }

    /// What a look found, when it came: whether anything did.
    pub(super) fn take_procs(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok(read) = self.looks.rx.try_recv() {
            any = true;
            w.procs.looking = false;
            match read {
                Ok(agents) => {
                    w.procs.agents = agents;
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
        assert!(!live.take_procs(&mut w), "one look, one answer");
        // A failed look says why and keeps what was read.
        w.procs.agents = vec![(1, Vec::new())];
        live.looks.tx.send(Err("ps did not answer".into())).unwrap();
        assert!(live.take_procs(&mut w));
        assert_eq!(w.procs.failed.as_deref(), Some("ps did not answer"));
        assert_eq!(w.procs.agents, vec![(1, Vec::new())]);
        live.looks.tx.send(Ok(Vec::new())).unwrap();
        live.take_procs(&mut w);
        assert_eq!(w.procs.failed, None);
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
