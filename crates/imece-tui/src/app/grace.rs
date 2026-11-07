//! What waits out a grace before it happens, and the question at quit.

use super::*;

impl App {
    /// `q` or `:quit`: the quit question; a `q` after it quits.
    pub(super) fn ask_quit(&mut self) {
        self.pending = Some('q');
        let ask = self.quit_ask();
        self.flash(ask);
    }

    /// The quit question, with what is in flight counted.
    pub(super) fn quit_ask(&self) -> String {
        let t = self.lang.text();
        let counts = [
            (t.quit_mid_turn, self.world.mid_turn().len()),
            (t.quit_changes, self.world.open_changes().len()),
        ];
        let mut said: Vec<String> = counts
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(words, n)| fill(words, &[&n.to_string()]))
            .collect();
        // `s` shows the counted ones; what counts down is only said.
        let shown = !said.is_empty();
        if let Some((what, _)) = self.grace {
            said.push(fill(t.quit_grace, &[&self.undoable_command(what)]));
        }
        match (said.is_empty(), shown) {
            (true, _) => t.quit_ask.to_string(),
            (false, true) => fill(t.quit_ask_busy, &[&said.join(", ")]),
            (false, false) => fill(t.quit_ask_said, &[&said.join(", ")]),
        }
    }

    /// The command that asked for what counts down, as written.
    fn undoable_command(&self, what: Undoable) -> String {
        match what {
            Undoable::Close(p) => format!(":close {}", self.world.projects[p].name),
            Undoable::Forget(id) => format!(":forget {}", self.world.agents[id].name),
            Undoable::Archive(p) => format!(":archive {}", self.world.projects[p].name),
            Undoable::Clear(c) => [":clear", ":clear inbox", ":clear journal"][c.index()].into(),
        }
    }

    /// A `:close` or `:forget` waits [`UNDO_GRACE`] from now; the bottom
    /// line counts it down.
    pub(super) fn wait_grace(&mut self, what: Undoable) {
        self.flash = None;
        self.grace = Some((what, Instant::now() + UNDO_GRACE));
    }

    /// `u` or `:undo`: the `:close` or `:forget` waiting does not happen.
    pub(super) fn undo(&mut self) {
        let t = self.lang.text();
        let said = match self.grace.take() {
            Some((Undoable::Close(p), _)) => fill(t.undone_close, &[&self.world.projects[p].name]),
            Some((Undoable::Forget(id), _)) => {
                fill(t.undone_forget, &[&self.world.agents[id].name])
            }
            Some((Undoable::Archive(p), _)) => {
                fill(t.undone_archive, &[&self.world.projects[p].name])
            }
            Some((Undoable::Clear(_), _)) => t.undone_clear.to_string(),
            None => t.nothing_to_undo.to_string(),
        };
        self.flash(said);
    }

    /// `U` or `:now`: what counts down acts at once, as when its time is
    /// up; the person need not wait it out.
    pub(super) fn grace_now(&mut self) {
        if self.grace.is_none() {
            return self.flash(self.lang.text().nothing_now);
        }
        self.end_grace();
        self.persist();
    }

    /// What waits out its grace at `now`, and the seconds it has left,
    /// rounded up.
    pub fn grace_left(&self, now: Instant) -> Option<(Undoable, u64)> {
        let (what, at) = self.grace?;
        let left = at.saturating_duration_since(now).as_millis();
        Some((what, left.div_ceil(1000) as u64))
    }

    /// The `:close` or `:forget` whose grace ran out by `now` acts.
    pub fn grace_due(&mut self, now: Instant) {
        if self.grace.is_some_and(|(_, at)| now >= at) {
            self.end_grace();
            self.persist();
        }
    }

    /// The `:close` or `:forget` waiting acts now: before another, and
    /// when imece quits.
    pub fn end_grace(&mut self) {
        match self.grace.take() {
            Some((Undoable::Close(p), _)) => self.close_now(p),
            Some((Undoable::Forget(id), _)) => self.forget_now(id),
            Some((Undoable::Archive(p), _)) => self.archive_now(p),
            Some((Undoable::Clear(what), _)) => self.clear_now(what),
            None => {}
        }
    }

    /// `:clear [inbox|journal]`: what TO DECIDE lists and the
    /// journal's lines go off the salon's screen after the grace, both
    /// without a word. Nothing is deleted; what still waits is counted.
    pub(super) fn clear(&mut self, arg: &str) {
        let t = self.lang.text();
        let Some(what) = Clear::parse(arg) else {
            return self.flash(fill(t.bad_clear, &[arg]));
        };
        // One waits at a time: the one before acts now.
        self.end_grace();
        if self.clearable(what) == (0, 0) {
            return self.flash(t.nothing_to_clear);
        }
        self.wait_grace(Undoable::Clear(what));
    }

    /// What `what` takes off now: the items TO DECIDE lists and the
    /// journal's rows not marked.
    fn clearable(&self, what: Clear) -> (usize, usize) {
        let w = &self.world;
        let inbox = match what.inbox() {
            true => w.shown_inbox().len(),
            false => 0,
        };
        let lines = match what.journal() {
            true => (ui::journal_rows(w).iter())
                .filter(|(i, _)| !w.marks.contains(i))
                .count(),
            false => 0,
        };
        (inbox, lines)
    }

    /// `:clear`'s grace ran out: off the screen now.
    fn clear_now(&mut self, what: Clear) {
        let t = self.lang.text();
        let (inbox, lines) = self.clearable(what);
        if what.inbox() {
            let shown = self.world.shown_inbox();
            self.world.cleared.inbox.extend(shown);
            self.inbox_sel = 0;
        }
        if what.journal() {
            self.world.cleared.journal = self.world.journal.len();
            let shown = self.journal_rows();
            self.journal_sel = self
                .journal_sel
                .filter(|i| shown.iter().any(|(r, _)| r == i));
        }
        let (inbox, lines) = (inbox.to_string(), lines.to_string());
        let said = match what {
            Clear::All => fill(t.cleared[0], &[&inbox, &lines]),
            Clear::Inbox => fill(t.cleared[1], &[&inbox]),
            Clear::Journal => fill(t.cleared[2], &[&lines]),
        };
        self.flash(said);
    }

    /// `:unclear [inbox|journal]`: what `:clear` took off comes back.
    pub(super) fn unclear(&mut self, arg: &str) {
        let t = self.lang.text();
        let Some(what) = Clear::parse(arg) else {
            return self.flash(fill(t.bad_clear, &[arg]));
        };
        let (inbox, lines) = (self.world.shown_inbox().len(), self.journal_rows().len());
        if what.inbox() {
            self.world.cleared.inbox.clear();
        }
        if what.journal() {
            self.world.cleared.journal = 0;
        }
        let inbox = self.world.shown_inbox().len() - inbox;
        let lines = self.journal_rows().len() - lines;
        if inbox + lines == 0 {
            return self.flash(t.nothing_cleared);
        }
        self.flash(fill(t.uncleared, &[&inbox.to_string(), &lines.to_string()]));
        self.persist();
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    /// A demo app with projects `shop-api`, `shop` and `more` open.
    fn three_projects() -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        app.command("open ~/work/shop");
        app.command("open ~/work/more");
        assert_eq!(app.world.open_projects().count(), 3);
        app.flash = None;
        app
    }

    #[test]
    fn a_close_waits_out_its_grace_and_u_keeps_it_open() {
        let mut app = three_projects();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.flash("before");
        app.command("close shop");
        assert_eq!(app.flash, None, "the count-down shows instead");
        let (what, at) = app.grace.unwrap();
        assert_eq!(what, Undoable::Close(1));
        assert!(!app.world.projects[1].closed);
        let ms = Duration::from_millis;
        assert_eq!(app.grace_left(at - UNDO_GRACE), Some((what, 10)));
        assert_eq!(app.grace_left(at - UNDO_GRACE + ms(1)), Some((what, 10)));
        assert_eq!(app.grace_left(at - ms(1001)), Some((what, 2)));
        assert_eq!(app.grace_left(at - ms(1)), Some((what, 1)));
        assert_eq!(app.grace_left(at), Some((what, 0)));
        app.grace_due(at - ms(1));
        assert!(!app.world.projects[1].closed, "not yet");
        app.grace_due(at);
        assert!(app.world.projects[1].closed, "its time");
        assert_eq!(app.grace, None);
        assert_eq!(
            said(&mut app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        assert_eq!(app.grace_left(at), None);
        // `u` keeps it open; again, nothing to undo.
        app.command("close more");
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(app.grace, None);
        assert_eq!(said(&mut app), "more stays open");
        app.grace_due(at + UNDO_GRACE * 2);
        assert!(!app.world.projects[2].closed);
        app.command("undo");
        assert_eq!(said(&mut app), Lang::En.text().nothing_to_undo);
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(said(&mut app), Lang::En.text().nothing_to_undo);
        // Quitting: what was asked happens.
        app.command("close more");
        app.end_grace();
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace, None);
        app.end_grace();
        assert_eq!(app.grace, None, "nothing waits, nothing happens");
    }

    #[test]
    fn capital_u_and_colon_now_do_what_counts_down_at_once() {
        let mut app = three_projects();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("close shop");
        app.on_key(key(KeyCode::Char('U')));
        assert!(app.world.projects[1].closed, "no wait");
        assert_eq!(app.grace, None);
        assert_eq!(
            said(&mut app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        app.command("close more");
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Close(2)));
        app.command("now");
        assert_eq!(app.grace, None);
        assert!(app.world.projects[2].closed);
        // Nothing counts down: said, nothing happens.
        app.flash = None;
        app.on_key(key(KeyCode::Char('U')));
        assert_eq!(said(&mut app), Lang::En.text().nothing_now);
        app.command("now");
        assert_eq!(said(&mut app), Lang::En.text().nothing_now);
        assert!(!app.world.projects[0].closed);
    }

    #[test]
    fn one_close_or_forget_waits_at_a_time() {
        let mut app = three_projects();
        app.command("close shop");
        app.command("close more");
        assert!(app.world.projects[1].closed, "the one before acts");
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Close(2)));
        // The last open project is refused at once, after the one waiting.
        app.command("close shop-api");
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace, None);
        assert_eq!(
            app.flash.take().map(|f| f.0).as_deref(),
            Some("shop-api is the last open project; q q quits")
        );
        // A :forget after a :close: the close acts.
        let mut app = three_projects();
        app.tell(0, "hello");
        app.command("close more");
        let lead = app.world.agents[0].name.clone();
        app.command(&format!("forget @{lead}"));
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Forget(0)));
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(
            app.flash.take().map(|f| f.0),
            Some(format!("{lead} keeps its session"))
        );
        assert_ne!(app.world.agents[0].status, Status::Asleep, "kept");
        // A refused :forget waits for nothing.
        app.command("forget @nobody");
        assert_eq!(app.grace, None);
        app.command(&format!("forget @{}", app.world.agents[1].name.clone()));
        assert_eq!(app.grace, None, "no session yet");
    }

    #[test]
    fn the_quit_question_counts_what_is_mid_turn_and_not_reviewed() {
        let mut app = quiet();
        let t = Lang::En.text();
        assert_eq!(asked(&mut app), t.quit_ask);
        app.world.agents[1].status = Status::Working;
        assert_eq!(
            asked(&mut app),
            "quit imece? agents mid-turn: 1 · q again quits · s shows them · any other key stays"
        );
        app.world.agents[2].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app.world.change_ready(a_change(3));
        app.world.agents[3].status = Status::Idle;
        assert_eq!(
            asked(&mut app),
            "quit imece? agents mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them · any other key stays"
        );
        app.world.agents[1].status = Status::Idle;
        app.world.agents[2].status = Status::Idle;
        assert_eq!(
            asked(&mut app),
            "quit imece? changes not reviewed: 1 · q again quits · s shows them · any other key stays"
        );
        app.lang = Lang::Tr;
        assert_eq!(
            asked(&mut app),
            "imece kapatılsın mı? incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır"
        );
        // Another key stays.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.quit && app.pending.is_none());
    }

    #[test]
    fn the_quit_question_says_what_counts_down_happens_now() {
        let mut app = quiet();
        let later = Instant::now() + UNDO_GRACE;
        let project = app.world.projects[0].name.clone();
        let agent = app.world.agents[2].name.clone();
        app.grace = Some((Undoable::Close(0), later));
        assert_eq!(
            asked(&mut app),
            format!(
                "quit imece? :close {project} happens now · q again quits · any other key stays"
            ),
            "nothing for s to show"
        );
        for (what, said) in [
            (Undoable::Forget(2), format!(":forget {agent}")),
            (Undoable::Archive(0), format!(":archive {project}")),
            (Undoable::Clear(Clear::All), ":clear".into()),
            (Undoable::Clear(Clear::Inbox), ":clear inbox".into()),
            (Undoable::Clear(Clear::Journal), ":clear journal".into()),
        ] {
            app.grace = Some((what, later));
            assert!(asked(&mut app).contains(&format!("? {said} happens now ·")));
        }
        app.world.agents[1].status = Status::Working;
        assert_eq!(
            asked(&mut app),
            "quit imece? agents mid-turn: 1, :clear journal happens now · q again quits · s shows them · any other key stays"
        );
        app.lang = Lang::Tr;
        app.world.agents[1].status = Status::Idle;
        assert_eq!(
            asked(&mut app),
            "imece kapatılsın mı? :clear journal şimdi yapılır · bir daha q çıkar · başka bir tuş kalır"
        );
        // q q still quits; the main loop lets it act (record 78).
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.quit && app.grace.is_some());
    }

    #[test]
    fn q_s_shows_what_quitting_would_leave_and_q_there_quits() {
        let mut app = quiet();
        // Nothing in flight: nothing to show, nothing quits.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(!app.leaving && !app.quit);
        // A turn under way: shown; another key goes back.
        app.world.agents[1].status = Status::Working;
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(
            app.leaving && app.flash.is_none(),
            "the question is answered"
        );
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.leaving && !app.quit && app.pending.is_none());
        // q in the window quits at once.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.quit && !app.leaving);
        // A change alone is shown too, and W there opens it.
        let mut app = quiet();
        app.world.change_ready(a_change(3));
        app.world.agents[3].status = Status::Idle;
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.leaving);
        app.on_key(key(KeyCode::Char('W')));
        assert!(!app.leaving && !app.quit);
        assert_eq!((app.zoom), (Zoom::Diff(3)));
    }

    #[test]
    fn clear_takes_waiting_and_the_journal_off_the_screen_after_the_grace() {
        let mut app = waiting(&[InboxKind::Change, InboxKind::Question]);
        for _ in 0..3 {
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
        }
        let marked = app.world.journal.len() - 2;
        app.world.marks.insert(marked);
        let rows = app.journal_rows().len();
        assert!(rows >= 3);
        // A line of the journal at hand, and the marked one.
        let first = app.journal_rows()[0].0;
        assert_ne!(first, marked);
        app.journal_sel = Some(first);
        app.inbox_sel = 1;
        app.command("clear");
        assert!(matches!(app.grace, Some((Undoable::Clear(Clear::All), _))));
        assert_eq!(app.inbox_rows().len(), 2, "not before the grace");
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert!(app.grace.is_none());
        assert_eq!(
            said(&app),
            format!(
                "2 waiting and {} journal lines off the screen; :unclear brings them back",
                rows - 1
            )
        );
        assert!(app.inbox_rows().is_empty());
        assert_eq!((app.inbox_sel, app.world.hidden_inbox()), (0, 2));
        assert_eq!(app.world.open_inbox(), [0, 1], "nothing closed");
        assert_eq!(app.journal_rows(), [(marked, 0)], "a mark stays");
        assert_eq!(app.journal_sel, None, "its line went");
        app.journal_sel = Some(marked);
        // What comes after shows.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: 3,
            detail: "new".into(),
            researcher: None,
            open: true,
            tick: 9,
            card: None,
        });
        app.world.note(Event::Failed { agent: 0 });
        assert_eq!(app.inbox_rows(), [2]);
        let newest = app.world.journal.len() - 1;
        assert_eq!(app.journal_rows(), [(newest, 0), (marked, 0)]);
        assert_eq!(app.world.hidden_inbox(), 2);
        // A card's screen still lists what was cleared.
        app.world.inbox[0].card = Some(0);
        app.card = Some(0);
        assert_eq!(app.inbox_rows(), [0]);
        app.card = None;
        // A closed one is not counted.
        app.world.inbox[1].open = false;
        assert_eq!(app.world.hidden_inbox(), 1);
        app.world.inbox[1].open = true;
        // `W` opens a cleared one and puts it back on the list.
        app.on_key(key(KeyCode::Char('W')));
        let at = app.waiting_at.expect("W opened one");
        assert!(!app.world.cleared.inbox.contains(&at));
        assert_eq!(app.inbox_rows()[app.inbox_sel], at);
        assert_eq!(app.world.hidden_inbox(), 1);
    }

    #[test]
    fn clear_one_side_undo_and_unclear_say_what_they_did() {
        let mut app = waiting(&[InboxKind::Reply]);
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        let lines = app.journal_rows().len();
        app.command("clear inbox");
        app.command("undo");
        assert_eq!(said(&app), "the screen stays as it is");
        assert!(app.grace.is_none());
        assert_eq!(app.world.cleared, Cleared::default());
        app.command("clear INBOX");
        assert!(matches!(
            app.grace,
            Some((Undoable::Clear(Clear::Inbox), _))
        ));
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert_eq!(
            said(&app),
            "1 waiting off the screen, still counted; :unclear brings them back"
        );
        assert_eq!(app.journal_rows().len(), lines, "the journal stays");
        app.command("clear inbox");
        assert_eq!(said(&app), "nothing on the screen to clear");
        assert!(app.grace.is_none());
        app.command("clear journal");
        assert!(matches!(
            app.grace,
            Some((Undoable::Clear(Clear::Journal), _))
        ));
        // Another clear acts on the one waiting first.
        app.command("clear");
        assert_eq!(said(&app), "nothing on the screen to clear");
        assert!(app.journal_rows().is_empty());
        assert_eq!(app.world.cleared.inbox, [0].into());
        app.command("clear everything");
        assert_eq!(said(&app), "everything is not inbox or journal");
        app.command("unclear journal");
        assert_eq!(
            said(&app),
            format!("back on the screen: 0 waiting, {lines} journal lines")
        );
        assert!(app.inbox_rows().is_empty(), "the inbox stays cleared");
        app.command("clear journal");
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert_eq!(
            said(&app),
            format!("{lines} journal lines off the screen; :unclear brings them back")
        );
        app.command("unclear inbox");
        assert_eq!(said(&app), "back on the screen: 1 waiting, 0 journal lines");
        assert!(app.journal_rows().is_empty(), "the journal stays cleared");
        app.command("unclear");
        assert_eq!(
            said(&app),
            format!("back on the screen: 0 waiting, {lines} journal lines")
        );
        app.command("unclear");
        assert_eq!(said(&app), "nothing is cleared off the screen");
        app.command("unclear x");
        assert_eq!(said(&app), "x is not inbox or journal");
        assert_eq!(app.world.cleared, Cleared::default());
    }

    #[test]
    fn clears_words_are_read_and_each_has_its_place() {
        assert_eq!(Clear::parse(""), Some(Clear::All));
        assert_eq!(Clear::parse(" Journal "), Some(Clear::Journal));
        assert_eq!(Clear::parse("inbox"), Some(Clear::Inbox));
        assert_eq!(Clear::parse("all"), None);
        for (c, i, inbox, journal) in [
            (Clear::All, 0, true, true),
            (Clear::Inbox, 1, true, false),
            (Clear::Journal, 2, false, true),
        ] {
            assert_eq!((c.index(), c.inbox(), c.journal()), (i, inbox, journal));
        }
        assert!(CLEARS.iter().all(|w| Clear::parse(w).is_some()));
    }
}
