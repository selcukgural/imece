//! The outside items cards close (record 142) on live sessions: each is
//! looked at once, so the person reads its title beside the number a
//! leader wrote, and one whose card's change is in is closed, through the
//! person's `gh` (`imece_agent::board`) on a thread of its own, one call
//! at a time. imece closes it, never an agent: no model's turn, and the
//! leader stays one that writes nowhere.

use super::*;
use imece_agent::board::{self, Closed, Seen, Why};

/// What is asked of the board.
enum Job {
    Look(ProjectId, Item),
    Close(CardId, Item),
}

/// What came of it.
enum Came {
    Looked(ProjectId, Item, Seen),
    Closed(CardId, ProjectId, Item, Closed),
}

/// The calls to the outside board, one at a time.
pub(super) struct Items {
    tx: Sender<Came>,
    rx: Receiver<Came>,
    busy: bool,
    /// The program asked; none where there is no outside board to reach
    /// (the stand-in's demo, a test), and nothing is looked at or closed.
    pub program: Option<String>,
}

impl Default for Items {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Items {
            tx,
            rx,
            busy: false,
            program: None,
        }
    }
}

/// What the screen keeps of a look.
fn outside_of(seen: Seen) -> Outside {
    match seen {
        Seen::Open(title) => Outside::Open(title),
        Seen::Closed(title) => Outside::Closed(title),
        Seen::Pull => Outside::Pull,
        Seen::NoSuch => Outside::NoSuch,
        Seen::Unread(_) => Outside::Unread,
    }
}

/// Why an item was not closed, from what the board answered.
fn why_of(seen: Seen) -> ItemWhy {
    match seen {
        Seen::Pull => ItemWhy::Pull,
        Seen::NoSuch => ItemWhy::NoSuch,
        Seen::Unread(Why::NoProgram) => ItemWhy::NoProgram,
        Seen::Unread(Why::SignedOut) => ItemWhy::SignedOut,
        Seen::Unread(Why::NoAnswer) => ItemWhy::NoAnswer,
        Seen::Unread(Why::Said(said)) => ItemWhy::Said(said),
        // Never an answer to a close: an open or closed issue is closed
        // or found so.
        Seen::Open(_) | Seen::Closed(_) => ItemWhy::NoAnswer,
    }
}

/// What the leader hears of card `card`'s item: imece's own words, never
/// what the board printed.
fn leader_note(card: CardId, item: Item, news: &ItemNews) -> String {
    let card = card + 1;
    match news {
        ItemNews::Closed => {
            format!("imece closed issue #{item}: the change of card #{card} is in. Nothing to do.")
        }
        ItemNews::Already => format!(
            "Issue #{item} was closed already when the change of card #{card} came in. \
             Nothing to do."
        ),
        _ => format!(
            "imece could not close issue #{item} after the change of card #{card} came in; \
             the person has been told and it stays open. Do not have a worker close it."
        ),
    }
}

impl Live {
    /// Asks the board for the next thing: a card whose item is due is
    /// closed first, else an item no look has read is looked at. Nothing
    /// while a call is under way, or where there is no board to reach.
    pub(super) fn items_due(&mut self, w: &mut World) {
        let Some(program) = self.items.program.clone() else {
            return;
        };
        if self.items.busy {
            return;
        }
        let git = |p: ProjectId| self.places.get(p).is_some_and(|pl| pl.git);
        let close = (w.items_due().into_iter())
            .find(|&(_, p, _)| git(p))
            .map(|(c, p, item)| (Job::Close(c, item), p));
        let look = || {
            (w.items_unread().into_iter())
                .find(|&(p, _)| git(p))
                .map(|(p, item)| (Job::Look(p, item), p))
        };
        let Some((job, p)) = close.or_else(look) else {
            return;
        };
        if let Job::Look(p, item) = job {
            w.items.insert((p, item), Outside::Asking);
        }
        let checkout = self.places[p].repo.clone();
        self.items.busy = true;
        let tx = self.items.tx.clone();
        thread::spawn(move || {
            let came = match job {
                Job::Look(p, item) => Came::Looked(p, item, board::look(&program, &checkout, item)),
                Job::Close(c, item) => {
                    Came::Closed(c, p, item, board::close(&program, &checkout, item))
                }
            };
            let _ = tx.send(came);
        });
    }

    /// What the board answered, when it did: whether anything came.
    pub(super) fn take_items(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok(came) = self.items.rx.try_recv() {
            any = true;
            self.items.busy = false;
            match came {
                Came::Looked(p, item, seen) => {
                    w.items.insert((p, item), outside_of(seen));
                }
                Came::Closed(c, p, item, closed) => {
                    let news = match closed {
                        Closed::Now(title) => {
                            w.items.insert((p, item), Outside::Open(title));
                            ItemNews::Closed
                        }
                        Closed::Already(title) => {
                            w.items.insert((p, item), Outside::Closed(title));
                            ItemNews::Already
                        }
                        Closed::Not(seen) => ItemNews::Failed(why_of(seen)),
                    };
                    let leader = w.projects[p].leader;
                    if w.agents[leader].role == Role::Leader && w.cards[c].item == ItemState::Due {
                        self.notes[leader].push(leader_note(c, item, &news));
                    }
                    w.item_settled(c, news);
                }
            }
        }
        any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;
    use std::os::unix::fs::PermissionsExt;

    /// A `gh` in a directory of its own that answers a look with `state`
    /// and logs its calls.
    fn gh(name: &str, state: &str, close: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("imece-items-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (gh, log) = (dir.join("gh"), dir.join("log"));
        let script = format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$2\" in\n\
             view) echo '{{\"state\":\"{state}\",\"title\":\"A 529 stops it\",\
             \"url\":\"https://github.com/o/r/issues/'\"$3\"'\"}}' ;;\nclose) {close} ;;\nesac\n",
            log.display()
        );
        std::fs::write(&gh, script).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        (gh, log)
    }

    /// Steps the board until nothing is under way and nothing is asked.
    fn settle(live: &mut Live, w: &mut World) {
        for _ in 0..500 {
            live.take_items(w);
            live.items_due(w);
            if !live.items.busy {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the board did not answer");
    }

    fn calls(log: &Path) -> Vec<String> {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        text.lines().map(str::to_string).collect()
    }

    /// A team whose first card closes item 58, with the board at `gh`.
    fn with_card(gh: &Path) -> (Live, World) {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.places[0].git = true;
        live.items.program = Some(gh.display().to_string());
        live.post(
            &mut w,
            0,
            &json!({ "title": "Retry", "body": "x", "closes": 58 }),
        )
        .unwrap();
        (live, w)
    }

    #[test]
    fn an_item_is_looked_at_once_and_closed_when_its_card_is_due() {
        let (gh, log) = gh("close", "OPEN", "exit 0");
        let (mut live, mut w) = with_card(&gh);
        settle(&mut live, &mut w);
        assert_eq!(
            w.items.get(&(0, 58)),
            Some(&Outside::Open("A 529 stops it".into()))
        );
        settle(&mut live, &mut w);
        assert_eq!(calls(&log).len(), 1, "one look, and nothing closed yet");
        w.cards[0].item = ItemState::Due;
        settle(&mut live, &mut w);
        assert_eq!(
            calls(&log)[1..],
            ["issue view 58 --json title,state,url", "issue close 58"]
        );
        assert_eq!(w.cards[0].item, ItemState::Closed);
        assert!(matches!(
            w.journal.last().map(|e| &e.event),
            Some(Event::Item { agent: 0, card: 0, item: 58, title, news: ItemNews::Closed })
                if title == "A 529 stops it"
        ));
        assert_eq!(
            live.notes[0],
            ["imece closed issue #58: the change of card #1 is in. Nothing to do."]
        );
        assert!(!live.lost[0], "no turn for it");
        assert_eq!(
            w.items.get(&(0, 58)),
            Some(&Outside::Closed("A 529 stops it".into()))
        );
        settle(&mut live, &mut w);
        assert_eq!(calls(&log).len(), 3, "never again");
    }

    #[test]
    fn an_item_that_does_not_close_is_said_once_and_stays_open() {
        let (gh, log) = gh("fail", "OPEN", "echo 'HTTP 403' >&2; exit 1");
        let (mut live, mut w) = with_card(&gh);
        w.cards[0].item = ItemState::Due;
        settle(&mut live, &mut w);
        assert_eq!(w.cards[0].item, ItemState::Failed);
        assert!(matches!(
            w.journal.last().map(|e| &e.event),
            Some(Event::Item { news: ItemNews::Failed(ItemWhy::Said(s)), .. }) if s == "HTTP 403"
        ));
        assert_eq!(live.notes[0].len(), 1);
        assert!(
            live.notes[0][0].starts_with("imece could not close issue #58")
                && !live.notes[0][0].contains("403"),
            "imece's words, not the board's: {:?}",
            live.notes[0]
        );
        let n = calls(&log).len();
        settle(&mut live, &mut w);
        assert_eq!(calls(&log).len(), n, "not tried again");
        // One closed already is found so, and nothing is closed.
        let (gh, log) = super::tests::gh("already", "CLOSED", "exit 0");
        let (mut live, mut w) = with_card(&gh);
        w.cards[0].item = ItemState::Due;
        settle(&mut live, &mut w);
        assert_eq!(w.cards[0].item, ItemState::Closed);
        assert!(calls(&log).iter().all(|c| c.starts_with("issue view")));
        assert!(live.notes[0][0].starts_with("Issue #58 was closed already"));
    }

    #[test]
    fn nothing_is_asked_without_a_board_a_checkout_or_while_a_call_is_under_way() {
        let (gh, log) = gh("none", "OPEN", "exit 0");
        let (mut live, mut w) = with_card(&gh);
        live.items.program = None;
        settle(&mut live, &mut w);
        live.items.program = Some(gh.display().to_string());
        live.places[0].git = false;
        w.cards[0].item = ItemState::Due;
        settle(&mut live, &mut w);
        assert!(calls(&log).is_empty() && w.items.is_empty());
        assert_eq!(w.cards[0].item, ItemState::Due, "waits for a board");
        live.places[0].git = true;
        live.items.busy = true;
        live.items_due(&mut w);
        assert!(w.items.is_empty(), "one call at a time");
        assert_eq!(why_of(Seen::Pull), ItemWhy::Pull);
        assert_eq!(why_of(Seen::NoSuch), ItemWhy::NoSuch);
        assert_eq!(why_of(Seen::Unread(Why::NoProgram)), ItemWhy::NoProgram);
        assert_eq!(why_of(Seen::Unread(Why::SignedOut)), ItemWhy::SignedOut);
        assert_eq!(why_of(Seen::Unread(Why::NoAnswer)), ItemWhy::NoAnswer);
        assert_eq!(outside_of(Seen::Pull), Outside::Pull);
        assert_eq!(outside_of(Seen::NoSuch), Outside::NoSuch);
        assert_eq!(outside_of(Seen::Unread(Why::NoAnswer)), Outside::Unread);
        assert_eq!(
            outside_of(Seen::Closed("t".into())),
            Outside::Closed("t".into())
        );
    }
}
