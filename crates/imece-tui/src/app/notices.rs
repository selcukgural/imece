//! What reaches a person away from the window: focus, notices, push, the digest, staying awake.

use super::*;

impl App {
    /// The terminal lost the focus (`gained` false) at `now`, or got it
    /// back: after `:away`'s time or more, what happened meanwhile shows
    /// at once, as after a key that long after the last (built from the
    /// journal, no model asked). Not over what the person is writing or
    /// another window.
    pub fn focus(&mut self, gained: bool, now: Instant) {
        // Back from the editor: the key file is looked at at once.
        self.keys_looked = None;
        if !gained {
            self.blurred = Some((now, self.world.tick));
            self.left_tick = self.world.tick;
            // Nothing moves in another window: no frame is waited for.
            self.drawn_motion = None;
            return;
        }
        let Some((at, tick)) = self.blurred.take() else {
            return;
        };
        let busy = self.typing
            || self.help
            || self.solo_pick.is_some()
            || self.archives.is_some()
            || self.viewing.is_some()
            || self.leaving
            || self.window_open();
        if busy || !self.away.over(now.duration_since(at)) {
            return;
        }
        self.digest = Digest::since(&self.world, tick);
    }

    /// `:notify`: how the person is told in another window.
    pub(super) fn set_notify(&mut self, how: Notify) {
        let t = self.lang.text();
        let said = match how {
            Notify::Off => t.notify_off.to_string(),
            Notify::Bell => t.notify_bell.to_string(),
            Notify::Text => t.notify_text.to_string(),
            Notify::Push if self.demo() => t.notify_push_demo.to_string(),
            Notify::Push => match self.read_push() {
                Ok(server) => fill(t.notify_push, &[&server]),
                Err(why) => return self.flash(why),
            },
        };
        self.notify = how;
        self.flash(said);
    }

    /// Reads the person's push settings for `:notify push`: the server
    /// they go to, or why not (the settings as they were are kept then).
    fn read_push(&mut self) -> Result<String, String> {
        match crate::push::read(self.push_file.as_deref()) {
            Ok(p) => {
                let server = p.server.clone();
                self.push = Some(p);
                self.push_failing = false;
                Ok(server)
            }
            Err(why) => Err(ui::words::push_why(self.lang, &why)),
        }
    }

    /// At start, `:notify push` kept from the last run: its settings read
    /// again; when they no longer hold, nothing is pushed (`:notify off`)
    /// and the flash says why.
    pub fn start_push(&mut self) {
        if self.notify != Notify::Push || self.demo() {
            return;
        }
        if let Err(why) = self.read_push() {
            self.notify = Notify::Off;
            self.flash(why);
        }
    }

    /// Sends `word` to the person's phone when `:notify push` is on: on a
    /// thread of its own, its end told to [`App::pushed`]; the demo only
    /// shows it in the journal.
    pub fn push_word(&mut self, word: &notify::Word) {
        if self.notify != Notify::Push {
            return;
        }
        let Some(agent) = word.agent else {
            return;
        };
        if self.demo() {
            self.world.note(Event::Pushed {
                agent,
                words: word.body.clone(),
                sent: false,
            });
            self.changed = true;
            return;
        }
        let Some(p) = self.push.clone() else {
            return;
        };
        let (tx, body, loud) = (self.pushes.0.clone(), word.body.clone(), word.loud);
        std::thread::spawn(move || {
            let went = crate::push::send(&p, &body, loud);
            let _ = tx.send((agent, body, went));
        });
    }

    /// The pushes that ended since last time, in the journal: each that
    /// went; one that failed only when the one before did not, so a server
    /// that is down is said once, not at every push.
    pub fn pushed(&mut self) {
        while let Ok((agent, words, went)) = self.pushes.1.try_recv() {
            match went {
                Ok(()) => {
                    self.push_failing = false;
                    let sent = true;
                    self.world.note(Event::Pushed { agent, words, sent });
                }
                Err(_) if self.push_failing => continue,
                Err(why) => {
                    self.push_failing = true;
                    self.world.note(Event::PushFailed { agent, why });
                }
            }
            self.changed = true;
        }
    }

    /// `:motion`: how much moves on the screen. Full over SSH says it
    /// runs as calm.
    /// `:away`: how long away opens the digest by itself.
    pub(super) fn set_away(&mut self, away: Away) {
        self.away = away;
        let t = self.lang.text();
        match away.time() {
            Some(_) => self.flash(fill(t.away_set, &[&away.name()])),
            None => self.flash(t.away_off),
        }
    }

    /// `:digest`: what happened since the person last left the window, or
    /// since the run began.
    pub(super) fn open_digest(&mut self) {
        self.digest = Digest::since(&self.world, self.left_tick);
        if self.digest.is_none() {
            self.flash(self.lang.text().digest_none);
        }
    }

    /// What to say to the person in another window right now, in the
    /// words of [`crate::notify`]: what came to wait for them while the
    /// terminal had no focus and is still there [`notify::AFTER`] later,
    /// once each; several at once as a count. Nothing while `:notify` is
    /// off, in the window, or for what was on screen before leaving.
    pub fn notices(&mut self, now: Instant) -> Option<notify::Word> {
        if self.notify == Notify::Off {
            return None;
        }
        let w = &self.world;
        let open = w.open_inbox();
        // A stuck agent whose item waits in the inbox is noted once.
        let stuck = |(id, n): (AgentId, Need)| {
            (n == Need::Stuck && !open.iter().any(|&i| w.inbox[i].agent == id)).then_some(id)
        };
        let waiting: Vec<Note> = open
            .iter()
            .copied()
            .map(Note::Item)
            .chain(
                self.needs()
                    .into_iter()
                    .enumerate()
                    .filter_map(stuck)
                    .map(Note::Stuck),
            )
            .collect();
        // Away from the machine the terminal may still have the focus: a
        // push also counts the keys' quiet, as long as `:away` says.
        let idle = self.notify == Notify::Push
            && self.away.over(now.saturating_duration_since(self.last_key));
        let due = self
            .notices
            .due(&waiting, self.blurred.is_some() || idle, now);
        let t = self.lang.text();
        let body = match due[..] {
            [] => return None,
            [one] if self.notify == Notify::Push => ui::words::pushed(self.lang, &self.world, one),
            [one] => ui::words::notice(self.lang, &self.world, one),
            _ => fill(t.notify_many, &[&due.len().to_string()]),
        };
        let agent = due.first().map(|&n| match n {
            Note::Stuck(id) => id,
            Note::Item(i) => self.world.inbox[i].agent,
        });
        // Louder for what is stuck or failed.
        let loud = due.iter().any(|&n| match n {
            Note::Stuck(_) => true,
            Note::Item(i) => self.world.inbox[i].kind == InboxKind::Fault,
        });
        self.notice_count += 1;
        let id = self.notice_count;
        self.noticed.push((id, due));
        let over = self.noticed.len().saturating_sub(NOTICED);
        self.noticed.drain(..over);
        Some(notify::Word {
            body,
            id,
            loud,
            agent,
        })
    }

    /// kitty reported notification `id` clicked: what it was about opens,
    /// over whatever window was open: the item that waits, the agent that
    /// is stuck, or for several the next item waiting (`W`). A number imece
    /// did not send, or one sent too long ago, does nothing.
    pub fn clicked(&mut self, id: u64) {
        let Some((_, notes)) = self.noticed.iter().find(|(n, _)| *n == id).cloned() else {
            return;
        };
        self.viewing = None;
        self.help = false;
        self.archives = None;
        self.peek = None;
        self.hints = None;
        self.news.clear();
        self.welcome = false;
        self.digest = None;
        self.left_off = None;
        self.leaving = false;
        let open = |i: usize| self.world.inbox.get(i).is_some_and(|x| x.open);
        match notes[..] {
            [Note::Item(i)] if open(i) => self.open_waiting(i),
            [Note::Stuck(a)] => self.go_agent(a),
            _ => self.next_waiting(false),
        }
    }

    /// The machine is held awake while an agent is mid-turn and the person
    /// asked for it (`--awake`, `:awake`), and let go otherwise; a hold
    /// that fails is said once.
    pub(super) fn keep_awake(&mut self) {
        let busy = !self.world.mid_turn().is_empty();
        if let Err(why) = self.awake.hold(busy) {
            let t = self.lang.text();
            self.flash(fill(t.awake_failed, &[&why]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn notify_is_off_bell_or_text_by_word_or_round_and_the_menu_says_which() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(app.command_note("notify"), "(off)");
        app.command("notify");
        assert_eq!(app.notify, Notify::Bell);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_bell));
        assert_eq!(app.command_note("notify"), "(bell)");
        app.command("notify");
        assert_eq!(app.notify, Notify::Text);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_text));
        app.command("notify");
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_off));
        app.command("notify text");
        assert_eq!(app.notify, Notify::Text);
        assert_eq!(app.command_note("notify"), "(text)");
        app.command("notify beep");
        assert_eq!(app.notify, Notify::Text, "as it was");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("beep is not off, bell, text or push")
        );
    }

    #[test]
    fn in_another_window_what_came_to_wait_is_said_once_after_the_wait() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let item = |kind, agent| InboxItem {
            kind,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        // Off: nothing, whatever came while away.
        app.focus(false, at(0));
        app.world.inbox.push(item(InboxKind::Question, 2));
        assert_eq!(app.notices(at(9)), None);
        app.focus(true, at(10));
        app.command("notify bell");
        // In the window: nothing, and what is there is seen.
        assert_eq!(app.notices(at(11)), None);
        app.focus(false, at(12));
        assert_eq!(app.notices(at(60)), None, "seen before leaving");
        // A question while away: said once, after the wait, in imece's
        // words with the agent's name, never the agent's.
        app.world.inbox.push(item(InboxKind::Question, 3));
        assert_eq!(app.notices(at(100)), None);
        assert_eq!(app.notices(at(104)), None, "under the wait");
        let name = app.world.agents[3].name.clone();
        assert_eq!(
            app.notices(at(105)),
            Some(notify::Word {
                body: format!("imece · {name} asks you"),
                id: 1,
                loud: false,
                agent: Some(3),
            }),
            "numbered from 1; a question is not loud"
        );
        assert_eq!(app.notices(at(200)), None, "once");
        // Its question answered while away: no news, and an idle agent
        // without an item is not stuck.
        let q3 = app.world.inbox.len() - 1;
        assert_eq!(app.world.inbox[q3].agent, 3);
        app.world.inbox[q3].open = false;
        assert_eq!(app.notices(at(201)), None);
        assert_eq!(app.notices(at(260)), None, "nothing new");
        // Several at once: the count. An agent stuck counts too, an agent
        // that waits in the inbox only once: the change of agent 4 and
        // agent 0 blocked; not agent 4 again as stuck, nor agent 1, blocked
        // with its change (the first item) long known.
        app.world.inbox.push(item(InboxKind::Change, 4));
        app.world.agents[0].status = Status::Blocked;
        app.world.agents[1].status = Status::Blocked;
        app.world.agents[4].status = Status::Stopped;
        assert_eq!(app.world.inbox[0].agent, 1);
        assert_eq!(app.notices(at(300)), None);
        assert_eq!(
            app.notices(at(305)),
            Some(notify::Word {
                body: "imece · 2 things wait for you".into(),
                id: 2,
                loud: true,
                agent: Some(4),
            }),
            "an agent stuck among them: loud"
        );
        // Back in the window: what was about to ring is dropped.
        app.world.inbox.push(item(InboxKind::Reply, 2));
        assert_eq!(app.notices(at(400)), None);
        app.focus(true, at(401));
        assert_eq!(app.notices(at(402)), None);
        app.focus(false, at(403));
        assert_eq!(app.notices(at(500)), None, "on screen when back");
        // Closed: gone from what could ring.
        app.world.inbox.push(item(InboxKind::Approval, 3));
        assert_eq!(app.notices(at(600)), None);
        app.world.inbox.last_mut().unwrap().open = false;
        assert_eq!(app.notices(at(700)), None);
    }

    #[test]
    fn a_push_says_the_kind_the_agent_and_the_project_and_the_demo_only_shows_it() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        app.command("notify");
        app.command("notify");
        app.command("notify");
        assert_eq!(app.notify, Notify::Off, "push is never in the round");
        app.command("notify push");
        assert_eq!(app.notify, Notify::Push);
        assert_eq!(
            flashed(&app).as_deref(),
            Some(app.lang.text().notify_push_demo)
        );
        assert_eq!(app.push, None, "the demo reads no settings");
        app.focus(true, at(0));
        app.last_key = at(0);
        assert_eq!(app.notices(at(1)), None);
        // Away from the machine, its terminal still focused: the keys'
        // quiet counts, as long as `:away` says (30 min).
        let away = 30 * 60;
        assert_eq!(app.notices(at(away - 1)), None, "not away yet");
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: 3,
            detail: "the secret plan".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away)), None, "away from now");
        assert_eq!(app.notices(at(away + 4)), None, "under the wait");
        let word = app.notices(at(away + 5)).expect("pushed");
        let (name, project) = (
            app.world.agents[3].name.clone(),
            app.world.projects[app.world.agents[3].project].name.clone(),
        );
        assert_eq!(word.body, format!("imece · {name} asks you · {project}"));
        assert!(!word.body.contains("secret"), "never what an agent wrote");
        assert_eq!(word.agent, Some(3));
        let before = app.world.journal.len();
        app.push_word(&word);
        assert_eq!(app.world.journal.len(), before + 1);
        assert_eq!(
            app.world.journal[before].event,
            Event::Pushed {
                agent: 3,
                words: word.body.clone(),
                sent: false
            }
        );
        let line = ui::words::event(Lang::En, &app.world, &app.world.journal[before].event);
        assert_eq!(
            line,
            format!(
                "a push to your phone would say (the demo sends nothing): {}",
                word.body
            )
        );
        // Not pushed: with another way, or a word about no one.
        app.notify = Notify::Text;
        app.push_word(&word);
        app.notify = Notify::Push;
        app.push_word(&notify::Word {
            agent: None,
            ..word
        });
        assert_eq!(app.world.journal.len(), before + 1);
        // The keys' quiet counts only for a push.
        app.notify = Notify::Text;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: 2,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away + 100)), None);
        assert_eq!(app.notices(at(away + 200)), None, "in the window");
        // `:away off`: only another window.
        app.notify = Notify::Push;
        app.away = Away::parse("off").unwrap();
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Change,
            agent: 4,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away + 300)), None);
        assert_eq!(app.notices(at(away * 3)), None, "never away by the keys");
    }

    #[test]
    fn live_push_settings_are_read_at_push_and_a_failure_is_said_once() {
        let dir = std::env::temp_dir().join(format!("imece-app-push-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("push");
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.push_file = Some(file.clone());
        // No settings: refused, as it was.
        app.command("notify push");
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(
            flashed(&app),
            Some(format!(
                "no push settings: write {} (server, topic, token)",
                file.display()
            ))
        );
        std::fs::write(&file, "topic = short\n").unwrap();
        app.command("notify push");
        assert_eq!(app.notify, Notify::Off);
        assert!(
            flashed(&app)
                .unwrap()
                .contains("on ntfy.sh a topic of at least 16")
        );
        std::fs::write(&file, "server = http://127.0.0.1:9\ntopic = t\n").unwrap();
        app.command("notify push");
        assert_eq!(app.notify, Notify::Push);
        assert_eq!(
            flashed(&app).as_deref(),
            Some(
                "a push to your phone through http://127.0.0.1:9 when something waits and you are away"
            )
        );
        assert_eq!(app.push.as_ref().map(|p| p.topic.as_str()), Some("t"));
        // How pushes end, in the journal: a failure once until one goes.
        let tx = app.pushes.0.clone();
        let before = app.world.journal.len();
        let ends: [Result<(), String>; 5] = [
            Ok(()),
            Err("down".into()),
            Err("down".into()),
            Ok(()),
            Err("refused".into()),
        ];
        for end in ends {
            tx.send((1, "imece · w".into(), end)).unwrap();
        }
        app.changed = false;
        app.pushed();
        assert!(app.changed);
        let got: Vec<Event> = app.world.journal[before..]
            .iter()
            .map(|e| e.event.clone())
            .collect();
        let sent = |agent| Event::Pushed {
            agent,
            words: "imece · w".into(),
            sent: true,
        };
        let failed = |why: &str| Event::PushFailed {
            agent: 1,
            why: why.into(),
        };
        assert_eq!(got, [sent(1), failed("down"), sent(1), failed("refused")]);
        app.changed = false;
        app.pushed();
        assert!(!app.changed, "nothing new");
        // A push sent for real: curl to a port no one listens on fails.
        app.push_failing = false;
        let word = notify::Word {
            body: "imece · x".into(),
            id: 1,
            loud: true,
            agent: Some(0),
        };
        app.push_word(&word);
        let before = app.world.journal.len();
        for _ in 0..300 {
            app.pushed();
            if app.world.journal.len() > before {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            matches!(
                &app.world.journal[before].event,
                Event::PushFailed { agent: 0, .. }
            ),
            "{:?}",
            app.world.journal.get(before)
        );
        // The settings read again at the next start: gone, nothing pushed.
        std::fs::remove_file(&file).unwrap();
        app.start_push();
        assert_eq!(app.notify, Notify::Off);
        assert!(flashed(&app).unwrap().starts_with("no push settings"));
        // Kept and still good: as it was, said nothing.
        std::fs::write(&file, "server = http://127.0.0.1:9\ntopic = t\n").unwrap();
        app.notify = Notify::Push;
        app.flash = None;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Push, None));
        // Another way kept: nothing read.
        std::fs::remove_file(&file).unwrap();
        app.notify = Notify::Bell;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Bell, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_demo_keeps_push_as_it_was_at_start() {
        let mut app = waiting(&[]);
        app.push_file = None;
        app.notify = Notify::Push;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Push, None));
    }

    #[test]
    fn a_click_on_a_notification_opens_what_it_was_about() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let item = |kind, agent| InboxItem {
            kind,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        app.command("notify text");
        app.focus(true, at(0));
        assert_eq!(app.notices(at(1)), None);
        app.focus(false, at(2));
        // One question: its notification opens it.
        app.world.inbox.push(item(InboxKind::Question, 3));
        let q = app.world.inbox.len() - 1;
        assert_eq!(app.notices(at(3)), None);
        let asked = app.notices(at(9)).unwrap();
        // A fault is loud on its own.
        app.world.inbox.push(item(InboxKind::Fault, 2));
        assert_eq!(app.notices(at(10)), None);
        let fault = app.notices(at(16)).unwrap();
        assert!(fault.loud && !asked.loud);
        assert_eq!(fault.id, asked.id + 1);
        // Agent 0 stuck, alone.
        app.world.agents[0].status = Status::Blocked;
        assert_eq!(app.notices(at(17)), None);
        let stuck = app.notices(at(23)).unwrap();
        assert!(stuck.loud);
        // Over windows that would hide it.
        app.help = true;
        app.peek = Some(1);
        app.welcome = true;
        app.leaving = true;
        app.archives = Some(Browser::new(&app.world, Vec::new()));
        app.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En)));
        app.clicked(asked.id);
        assert!(!app.help && app.peek.is_none() && !app.welcome && !app.leaving);
        assert!(app.archives.is_none() && app.viewing.is_none());
        assert_eq!(app.waiting_at, Some(q), "the question it was about");
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Inbox));
        app.clicked(stuck.id);
        assert_eq!(app.selected, Sel::Agent(0), "the stuck agent");
        // The question answered meanwhile: the next item waiting instead.
        app.world.inbox[q].open = false;
        app.waiting_at = None;
        app.clicked(asked.id);
        assert_ne!(app.waiting_at, Some(q));
        assert!(app.waiting_at.is_some(), "{:?}", app.waiting_at);
        // A number imece did not send does nothing.
        app.selected = Sel::Agent(2);
        app.help = true;
        app.clicked(999);
        assert!(app.help && app.selected == Sel::Agent(2));
        // Only the last NOTICED are kept.
        for n in 0..NOTICED + 2 {
            app.noticed.push((100 + n as u64, vec![Note::Stuck(1)]));
        }
        let over = app.noticed.len() - NOTICED;
        app.noticed.drain(..over);
        app.clicked(100);
        assert_eq!(app.selected, Sel::Agent(2), "forgotten");
        app.clicked(101 + NOTICED as u64);
        assert_eq!(app.selected, Sel::Agent(1));
    }

    #[test]
    fn several_notices_at_once_are_one_that_opens_the_next_waiting() {
        let mut app = waiting(&[InboxKind::Change, InboxKind::Question]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        app.command("notify text");
        app.focus(false, at(0));
        // The two items came while away: one notice for both.
        let word = app.notices(at(1));
        assert_eq!(word, None);
        let word = app.notices(at(7)).unwrap();
        assert!(word.body.contains('2'), "{}", word.body);
        app.help = true;
        app.clicked(word.id);
        assert!(!app.help);
        assert!(app.waiting_at.is_some());
        // The list keeps NOTICED notices, the oldest going first.
        for n in 0..NOTICED + 3 {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Reply,
                agent: n % 4,
                detail: "x".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
            let s = 10 + 10 * n as u64;
            assert_eq!(app.notices(at(s)), None);
            assert!(app.notices(at(s + 6)).is_some());
        }
        assert_eq!(app.noticed.len(), NOTICED);
        assert_eq!(app.noticed[0].0, word.id + 4, "the oldest gone");
    }

    #[test]
    fn a_notice_names_the_kind_of_what_waits_and_its_agent() {
        use crate::notify::Note;
        let mut app = waiting(&[
            InboxKind::Sensitive,
            InboxKind::Reply,
            InboxKind::Change,
            InboxKind::Question,
        ]);
        // Two more kinds, from the first two agents again.
        for (n, kind) in [(1, InboxKind::Research), (2, InboxKind::Approval)] {
            let mut item = app.world.inbox[0].clone();
            item.kind = kind;
            item.agent = n;
            app.world.inbox.push(item);
        }
        let w = &app.world;
        let name = |n: usize| w.agents[n].name.clone();
        let said: Vec<String> = (0..6)
            .map(|i| ui::words::notice(Lang::En, w, Note::Item(i)))
            .collect();
        assert_eq!(
            said,
            [
                format!("imece · {}: something sensitive waits for you", name(1)),
                format!("imece · {} answered you", name(2)),
                format!("imece · {} finished: a change waits for you", name(3)),
                format!("imece · {} asks you", name(4)),
                format!("imece · {}: something sensitive waits for you", name(1)),
                format!("imece · {} waits for your yes", name(2)),
            ]
        );
        assert_eq!(
            ui::words::notice(Lang::Tr, w, Note::Stuck(2)),
            format!("imece · {} takıldı", name(2))
        );
        // A push adds the project of whom it is about.
        let project = |id: AgentId| w.projects[w.agents[id].project].name.clone();
        assert_eq!(
            ui::words::pushed(Lang::En, w, Note::Stuck(2)),
            format!("imece · {} is stuck · {}", name(2), project(2))
        );
        assert_eq!(
            ui::words::pushed(Lang::En, w, Note::Item(3)),
            format!("imece · {} asks you · {}", name(4), project(4))
        );
    }

    #[test]
    fn each_refusal_of_push_settings_says_what_to_mend() {
        use crate::push::Why;
        let say = |why: Why| ui::words::push_why(Lang::En, &why);
        assert_eq!(
            say(Why::NoFile("/c/push".into())),
            "no push settings: write /c/push (server, topic, token)"
        );
        assert_eq!(
            say(Why::Unreadable("not text".into())),
            "push settings: not text"
        );
        assert!(say(Why::Line(3)).starts_with("push settings, line 3: "));
        assert_eq!(say(Why::NoTopic), "push settings: no topic");
        assert!(
            say(Why::Server("http://a.b".into())).starts_with("push settings: http://a.b is not")
        );
        assert!(say(Why::Topic).ends_with("at most 64"));
        assert!(say(Why::Guessable).contains("at least 16 with a digit"));
        assert_eq!(
            say(Why::Token("/t: not one token".into())),
            "push settings: the token file /t: not one token"
        );
        assert!(ui::words::push_why(Lang::Tr, &Why::NoTopic).contains("topic yok"));
    }

    #[test]
    fn the_machine_is_held_awake_while_an_agent_is_mid_turn_when_asked() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        let sleeper = || Some(vec!["sleep".to_string(), "30".to_string()]);
        app.awake = crate::awake::Awake::new(false, sleeper());
        let tick = app.world.tick;
        for a in &mut app.world.agents {
            a.set_status(Status::Idle, tick);
        }
        assert_eq!(app.command_note("awake"), "(off)");
        app.world.agents[0].set_status(Status::Working, tick);
        app.keep_awake();
        assert!(!app.awake.holding(), "not asked for");
        app.command("awake");
        assert_eq!(flashed(&app).as_deref(), Some(t.awake_on));
        assert_eq!(app.command_note("awake"), "(on)");
        assert!(app.awake.holding(), "at once, mid-turn");
        app.world.agents[0].set_status(Status::Idle, tick);
        app.keep_awake();
        assert!(!app.awake.holding(), "nothing mid-turn");
        app.world.agents[0].set_status(Status::Working, tick);
        app.keep_awake();
        assert!(app.awake.holding());
        app.command("awake");
        assert_eq!(flashed(&app).as_deref(), Some(t.awake_off));
        assert!(!app.awake.holding(), "turned off: let go at once");
        // One that can not start says so.
        app.awake = crate::awake::Awake::new(true, Some(vec!["/nonexistent/caffeinate".into()]));
        app.keep_awake();
        let said = flashed(&app).unwrap();
        assert!(
            said.starts_with("could not keep the machine awake: /nonexistent/caffeinate: "),
            "{said}"
        );
    }

    #[test]
    fn coming_back_after_half_an_hour_away_shows_what_happened_at_once() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let away = |app: &mut App, secs: u64| {
            app.focus(false, t0);
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.focus(true, t0 + Duration::from_secs(secs));
        };
        // Not long enough: nothing, and the blur is forgotten.
        away(&mut app, 1799);
        assert!(app.digest.is_none() && app.blurred.is_none());
        // Half an hour: what happened since the blur.
        away(&mut app, 1800);
        let d = app.digest.take().expect("a digest");
        assert_eq!(d.highlights.len(), 1, "only what came after leaving");
        // Focus back without a blur first: nothing.
        app.focus(true, t0 + Duration::from_secs(3600));
        assert!(app.digest.is_none());
        // Nothing new: nothing to tell.
        app.focus(false, t0);
        app.focus(true, t0 + Duration::from_secs(3600));
        assert!(app.digest.is_none());
        // Not over writing or another window.
        for busy in [
            |a: &mut App| a.typing = true,
            |a: &mut App| a.help = true,
            |a: &mut App| a.leaving = true,
            |a: &mut App| a.left_off = Some(LeftOff::default()),
            |a: &mut App| a.solo_pick = Some(SoloPick::new(None, None)),
            |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new())),
            |a: &mut App| a.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En))),
        ] {
            let mut app = App::new(1, 1, 0, 1, Lang::En);
            busy(&mut app);
            away(&mut app, 3600);
            assert!(app.digest.is_none());
        }
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.digest = Some(Digest {
            minutes: 42,
            ..Digest::default()
        });
        away(&mut app, 3600);
        assert_eq!(
            app.digest.as_ref().unwrap().minutes,
            42,
            "the one open stays"
        );
    }

    #[test]
    fn away_sets_how_long_until_what_happened_shows_and_off_never() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let away = |app: &mut App, secs: u64| {
            app.focus(false, t0);
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.focus(true, t0 + Duration::from_secs(secs));
            app.digest.take().is_some()
        };
        assert_eq!(app.command_note("away"), "(30m)");
        app.command("away 5m");
        assert_eq!(
            said(&app),
            "back after 5m or more away, what happened shows at once"
        );
        assert_eq!(app.command_note("away"), "(5m)");
        assert!(!away(&mut app, 299));
        assert!(away(&mut app, 300));
        // Bare: the next one round.
        app.command("away");
        assert_eq!(app.away, Away::parse("15m").unwrap());
        app.command("away 1h");
        app.command("away");
        assert_eq!(app.away, Away::parse("off").unwrap());
        assert_eq!(
            said(&app),
            "what happened never shows by itself; :digest shows it"
        );
        assert!(!away(&mut app, 86_400 * 7), "off: never");
        app.command("away soon");
        assert_eq!(
            said(&app),
            "soon is not off, minutes (30m) or hours (1h), up to a day"
        );
        assert_eq!(app.away, Away::parse("off").unwrap(), "kept");
    }

    #[test]
    fn a_key_after_half_an_hour_without_one_shows_what_happened_first() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let ago = |secs| {
            Instant::now()
                .checked_sub(Duration::from_secs(secs))
                .unwrap()
        };
        let idle = |app: &mut App, secs: u64| {
            app.last_key = ago(secs);
            app.last_key_tick = app.world.tick;
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.on_key(key(KeyCode::Char('j')));
            app.digest.take().is_some()
        };
        assert!(!idle(&mut app, 1790), "under half an hour");
        assert!(idle(&mut app, 1801));
        // The key that opened it did nothing else; the next acts.
        assert!(!idle(&mut app, 0));
        app.command("away 2m");
        assert!(!idle(&mut app, 110));
        assert!(idle(&mut app, 130));
        app.command("away off");
        assert!(!idle(&mut app, 86_400));
    }

    #[test]
    fn digest_shows_what_happened_since_the_person_last_left() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        // Nothing yet this run.
        app.command("digest");
        assert!(app.digest.is_none());
        assert_eq!(said(&app), "nothing new since you last left");
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        app.command("digest");
        let d = app.digest.take().expect("since the run began");
        assert_eq!(d.highlights.len(), 1);
        // Left and back at once: since leaving, not since the start.
        let t0 = Instant::now();
        app.focus(false, t0);
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        app.world.note(Event::Failed { agent: 0 });
        app.focus(true, t0 + Duration::from_secs(1));
        assert!(app.digest.is_none(), "too short to open by itself");
        app.command("digest");
        let d = app.digest.take().expect("since leaving");
        assert_eq!(d.highlights.len(), 2);
        assert_eq!(d.minutes, SECONDS_PER_TICK / 60);
    }
}
