//! A conversation: following it, folding its messages, notes on its words, marks, export.

use super::*;

impl App {
    /// Keeps an open conversation current: the journal lines of its two
    /// parties that came since it was opened (or last followed) are
    /// appended. The person's own line, just written from the window, is
    /// selected, and so is a line that came while the last one was
    /// selected, so the window follows the talk; a selection further up
    /// stays where the person is reading.
    pub(crate) fn follow_thread(&mut self) {
        let Some(th) = &mut self.thread else {
            return;
        };
        // A chat nothing was said in yet follows its agent and the person.
        let pair = match (th.entries.first(), th.about) {
            (Some(&first), _) => {
                ui::words::said(&self.world.journal[first].event).map(|s| s.pair())
            }
            (None, Some(id)) => {
                let me = Party::Agent(id);
                Some((me.min(Party::Person), me.max(Party::Person)))
            }
            (None, None) => None,
        };
        let Some(pair) = pair else {
            return;
        };
        let from = th.entries.last().map_or(0, |&l| l + 1);
        let at_end = th.sel + 1 >= th.entries.len();
        let mut written = false;
        for (i, e) in self.world.journal.iter().enumerate().skip(from) {
            if ui::words::said(&e.event).is_some_and(|s| s.pair() == pair) {
                th.entries.push(i);
                written |= matches!(e.event, Event::YouSaid { .. });
            }
        }
        if (written || at_end) && th.sel + 1 < th.entries.len() {
            th.sel = th.entries.len() - 1;
            th.follow.set(true);
        }
    }

    /// Opens the conversation between the person and agent `id`, if they
    /// have one.
    pub(super) fn open_talk_with(&mut self, id: AgentId) {
        let me = Party::Agent(id);
        let pair = (me.min(Party::Person), me.max(Party::Person));
        let mine = |e: &Entry| ui::words::said(&e.event).is_some_and(|s| s.pair() == pair);
        match self.world.journal.iter().rposition(mine) {
            Some(i) => self.open_thread(i),
            None => {
                let name = self.world.agents[id].name.clone();
                let t = self.lang.text();
                self.flash(fill(t.no_talk_yet, &[&name]));
            }
        }
    }

    /// The agent the open conversation is the person's with: a chat
    /// (record 144), where a line written goes to that agent and stays
    /// open. None over a conversation between two agents, or an event.
    pub fn chat_agent(&self) -> Option<AgentId> {
        ui::chat_with(&self.world, self.thread.as_ref()?)
    }

    /// Opens the person's conversation with agent `id` at its newest
    /// line; with nothing said yet it opens empty, to be written in.
    pub(crate) fn open_chat(&mut self, id: AgentId) {
        let entries = ui::chat_of(&self.world, id);
        let sel = entries.len().saturating_sub(1);
        self.thread = Some(Thread {
            entries,
            sel,
            scroll: Cell::new(0),
            follow: Cell::new(true),
            page: Cell::new(10),
            copy: None,
            words: None,
            folds: Folds::every(self.fold_talk),
            room: Cell::new(usize::MAX),
            about: Some(id),
            passing: false,
        });
    }

    /// `/` `i` (the leader of the project at hand) and `@` (the agent at
    /// hand): the conversation with agent `id` above, the line to write
    /// on under it. A chat with that agent already open is written in as
    /// it stands, wherever the person reads in it.
    pub(super) fn write_to(&mut self, id: AgentId) {
        // An archive is read: nothing is written to its agents, so no
        // chat opens for a line that will be refused.
        if self.chat_agent() != Some(id) && !self.viewing_archive() {
            self.open_chat(id);
        }
        self.typing = true;
        self.input.clear();
        self.after = 0;
        self.menu = None;
    }

    /// While a message is written, the conversation with the agent its
    /// line names (`@claude-1 `, the name whole) stands above the line, so
    /// what was said before is read while writing; it goes when the line
    /// is sent, dropped or names that agent no more. Not over a change:
    /// what is written there is about the lines in view. Called after
    /// every key of the line.
    pub(super) fn chat_follows(&mut self) {
        let passing = self.thread.as_ref().is_some_and(|th| th.passing);
        let named = self.named_on_line();
        match named {
            Some(id) if self.chat_agent() == Some(id) => {}
            // Its conversation, in place of nothing or of one opened the
            // same way; one the person opened stays.
            Some(id) if passing || (self.thread.is_none() && self.diff_zoom().is_none()) => {
                self.open_chat(id);
                if let Some(th) = &mut self.thread {
                    th.passing = true;
                }
            }
            None if passing => self.thread = None,
            _ => {}
        }
    }

    /// The agent the message being written names, its name whole
    /// (`@claude-1 `); none when nothing is written to an agent.
    fn named_on_line(&self) -> Option<AgentId> {
        if !self.typing || self.noting.is_some() || self.rejecting.is_some() || self.searching {
            return None;
        }
        let rest = self.input.strip_prefix('@')?;
        let (name, _) = rest.split_once(char::is_whitespace)?;
        let id = self.world.agent_by_name(name)?;
        (!self.world.agents[id].gone).then_some(id)
    }

    /// A key for the open conversation; `false` leaves it to the rest of
    /// the screen (typing, switching views, quitting).
    pub(super) fn thread_key(&mut self, k: KeyEvent) -> bool {
        let Some(th) = &mut self.thread else {
            return false;
        };
        // A chat nothing was said in yet has no message to move over,
        // copy or fold: its keys do nothing, the rest work as anywhere.
        if th.entries.is_empty() {
            return match k.code {
                KeyCode::Esc => {
                    self.thread = None;
                    true
                }
                KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End => true,
                KeyCode::Char(c) => "jk G[]%cyCYvhln".contains(c),
                _ => false,
            };
        }
        let last = th.entries.len().saturating_sub(1);
        match k.code {
            KeyCode::Esc => self.thread = None,
            KeyCode::Up | KeyCode::Char('k') => {
                th.sel = th.sel.saturating_sub(1);
                th.follow.set(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                th.sel = (th.sel + 1).min(last);
                th.follow.set(true);
            }
            KeyCode::PageUp => th.scroll.set(th.scroll.get().saturating_sub(th.page.get())),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                th.scroll.set(th.scroll.get().saturating_add(th.page.get()))
            }
            KeyCode::Home => {
                th.sel = 0;
                th.follow.set(true);
            }
            KeyCode::End | KeyCode::Char('G') => {
                th.sel = last;
                th.follow.set(true);
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let starts = ui::turn_starts(&self.world, &th.entries);
                if let Some(to) = jump(&starts, th.sel, k.code == KeyCode::Char(']')) {
                    th.sel = to;
                    th.follow.set(true);
                }
            }
            KeyCode::Char('%') => {
                if let Some(to) = ui::reply_jump(&self.world, &th.entries, th.sel) {
                    th.sel = to;
                    th.follow.set(true);
                }
            }
            KeyCode::Char('c') | KeyCode::Char('y') => {
                let one = th.entries[th.sel];
                self.copy = Some(ui::copy_text(self.lang, &self.world, &[one]));
            }
            KeyCode::Char('C') | KeyCode::Char('Y') => {
                self.copy = Some(ui::copy_text(self.lang, &self.world, &th.entries));
            }
            KeyCode::Char('v') => {
                let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
                let first = lines.iter().position(|l| l.entry == th.sel);
                th.copy = Some(Copying::new(first.unwrap_or(0)));
                th.words = None;
                th.follow.set(true);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                th.folds.set(th.sel, true);
                th.follow.set(true);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                th.folds.set(th.sel, false);
                th.follow.set(true);
            }
            KeyCode::Char('n') => self.send_talk_notes(),
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// `zM` (`fold`) or `zR` in a conversation, `:fold` `:unfold` with one
    /// open: every message folded, or open; the conversations opened
    /// after it in this run start so too.
    pub(super) fn fold_talk(&mut self, fold: bool) {
        self.fold_talk = fold;
        if let Some(th) = &mut self.thread {
            th.folds = Folds::every(fold);
            th.follow.set(true);
        }
    }

    /// `w l →` (`on`) or `b h ←` while choosing in a conversation: the
    /// cursor goes a word on or back, over the end of a line to the next
    /// or the one before; the choice turns word-wise, from the end it
    /// had (the start of its first line, the end of its last).
    pub(super) fn word_step(&mut self, on: bool) {
        let Some(th) = &mut self.thread else {
            return;
        };
        let Some(c) = &mut th.copy else {
            return;
        };
        let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
        let counts: Vec<usize> = lines.iter().map(|l| l.words.len()).collect();
        let last = |l: usize| counts.get(l).map_or(0, |n| n.saturating_sub(1));
        let (aw, cw) = th.words.unwrap_or(match c.anchor.cmp(&c.at) {
            std::cmp::Ordering::Less => (0, last(c.at)),
            std::cmp::Ordering::Greater => (last(c.anchor), 0),
            std::cmp::Ordering::Equal => (0, 0),
        });
        let (line, word) = ui::word_step(&counts, (c.at, cw), on);
        c.at = line;
        th.words = Some((aw, word));
        if let Some(l) = lines.get(line) {
            th.sel = l.entry;
        }
        th.follow.set(true);
    }

    /// `s` while choosing in a conversation: the choice starts again
    /// where the cursor is, the cursor's word alone when it is word-wise.
    pub(super) fn choose_from_here(&mut self) {
        if let Some(th) = &mut self.thread
            && let Some(c) = &mut th.copy
        {
            c.anchor = c.at;
            if let Some((aw, cw)) = &mut th.words {
                *aw = *cw;
            }
        }
    }

    /// `c` while choosing in a conversation: a note on the words chosen,
    /// written in the command line; one on the same words is written
    /// again. The choosing ends. Blank words take no note. The note goes
    /// to the agent of the line the words start in: whoever wrote it, or,
    /// the person's own line, the agent it went to (`Event::agent`).
    pub(super) fn note_words(&mut self) {
        let Some(th) = &mut self.thread else {
            return;
        };
        let Some(c) = th.copy else {
            return;
        };
        let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
        let quote = ui::chosen_text(&lines, th);
        if quote.trim().is_empty() {
            return self.flash(self.lang.text().note_blank);
        }
        let Some(&entry) = (lines.get(c.span().0)).and_then(|l| th.entries.get(l.entry)) else {
            return;
        };
        th.copy = None;
        let to = self.world.journal[entry].event.agent();
        let had = (self.talk_notes.iter())
            .find(|n| n.entry == entry && n.quote == quote)
            .map(|n| n.text.clone());
        self.input = had.unwrap_or_default();
        self.after = 0;
        self.typing = true;
        self.menu = None;
        self.noting = Some(Noting::Words(TalkNote {
            to,
            entry,
            quote,
            text: String::new(),
        }));
    }

    /// The notes on words of the open conversation, oldest line first.
    pub fn talk_notes_here(&self) -> Vec<&TalkNote> {
        let Some(th) = &self.thread else {
            return Vec::new();
        };
        let mut notes: Vec<&TalkNote> = (self.talk_notes.iter())
            .filter(|n| th.entries.contains(&n.entry))
            .collect();
        notes.sort_by_key(|n| n.entry);
        notes
    }

    /// The note written on `note`'s words is kept (in place of one there
    /// was); an empty one takes it away.
    pub(super) fn keep_talk_note(&mut self, mut note: TalkNote, text: String) {
        let t = self.lang.text();
        self.talk_notes
            .retain(|n| !(n.entry == note.entry && n.quote == note.quote));
        if text.is_empty() {
            return self.flash(t.note_dropped);
        }
        note.text = text;
        self.talk_notes.push(note);
        let n = self.talk_notes_here().len().to_string();
        self.flash(fill(t.talk_noted, &[&n]));
    }

    /// `n` in a conversation: its notes go to the agents they are for,
    /// one message each, every note under the words it is about. A note
    /// whose message could not go stays.
    fn send_talk_notes(&mut self) {
        let t = self.lang.text();
        let here: Vec<TalkNote> = self.talk_notes_here().into_iter().cloned().collect();
        if here.is_empty() {
            return self.flash(t.no_talk_notes);
        }
        let mut by: BTreeMap<AgentId, Vec<&TalkNote>> = BTreeMap::new();
        for n in &here {
            by.entry(n.to).or_default().push(n);
        }
        let (mut sent, mut names) = (0, Vec::new());
        for (to, notes) in by {
            let message = ui::talk_notes_message(&self.world, &notes);
            if self.tell(to, &message) {
                self.talk_notes.retain(|n| !notes.contains(&n));
                sent += notes.len();
                names.push(self.world.agents[to].name.clone());
            }
        }
        if sent > 0 {
            let t = self.lang.text();
            let names = names.join(", ");
            self.flash(match sent {
                1 => fill(t.talk_note_sent, &[&names]),
                n => fill(t.talk_notes_sent, &[&n.to_string(), &names]),
            });
        }
    }

    /// Opens the conversation that journal line `i` belongs to, with that
    /// line selected.
    pub fn open_thread(&mut self, i: usize) {
        let entries = ui::thread_of(&self.world, i);
        let sel = entries.iter().position(|&e| e == i).unwrap_or(0);
        self.thread = Some(Thread {
            entries,
            sel,
            scroll: Cell::new(0),
            follow: Cell::new(true),
            page: Cell::new(10),
            copy: None,
            words: None,
            folds: Folds::every(self.fold_talk),
            room: Cell::new(usize::MAX),
            about: None,
            passing: false,
        });
    }

    /// `m` or `:mark`: the line at hand (the open conversation's selected
    /// line, else the journal's) is marked to come back to, or loses its
    /// mark.
    pub(super) fn toggle_mark(&mut self) {
        // An archive's marks are as it left them.
        if self.viewing_archive() {
            return self.read_only();
        }
        let journal = self.window == Some(Window::Journal) && !self.story_on();
        let at = match &self.thread {
            Some(th) => th.entries.get(th.sel).copied(),
            None if journal => self.journal_sel,
            None => None,
        };
        let t = self.lang.text();
        let said = match at {
            Some(i) if self.world.mark(i) => t.marked,
            Some(_) => t.unmarked,
            None => t.mark_where,
        };
        self.flash(said);
    }

    /// `'`: the next marked line after the one at hand, round to the first
    /// again: among the open conversation's lines, else in the journal
    /// (the card's, on a card's screen) in the order it lists them, newest
    /// first; from anywhere else the journal is where it goes.
    pub(super) fn next_mark(&mut self) {
        let marks = self.world.marks.clone();
        let found = match &mut self.thread {
            Some(th) => {
                let marked: Vec<usize> = (0..th.entries.len())
                    .filter(|&n| marks.contains(&th.entries[n]))
                    .collect();
                let to = next_of(&marked, Some(th.sel));
                if let Some(n) = to {
                    th.sel = n;
                    th.follow.set(true);
                }
                to.is_some()
            }
            None => {
                // The journal's window, its lines shown even while the
                // story is on (`esc` brings the story back).
                self.open_window(Window::Journal);
                if self.story {
                    self.story_away = true;
                }
                let rows = self.journal_rows();
                let marked: Vec<usize> = (0..rows.len())
                    .filter(|&n| marks.contains(&rows[n].0))
                    .collect();
                let at = (self.journal_sel).and_then(|i| rows.iter().position(|&(r, _)| r == i));
                let to = next_of(&marked, at);
                if let Some(n) = to {
                    self.journal_sel = Some(rows[n].0);
                } else {
                    self.select_journal(None);
                }
                to.is_some()
            }
        };
        if !found {
            self.flash(self.lang.text().no_marks);
        }
    }

    /// `:export [PATH]`: the open conversation, else the card's screen,
    /// as Markdown in PATH (a file, or a directory to name one in), else
    /// in the directory imece was started from. Nothing is written over.
    pub(super) fn export(&mut self, arg: &str) {
        let t = self.lang.text();
        let (what, text) = match (&self.thread, self.card) {
            (Some(th), _) => (
                "conversation".to_string(),
                ui::export::thread(self.lang, &self.world, &th.entries),
            ),
            (None, Some(c)) => (format!("card-{}", c + 1), ui::export::card(self, c)),
            (None, None) => return self.flash(t.nothing_to_export),
        };
        let name = ui::export::file_name(&what, self.world.tick);
        let target = (!arg.is_empty()).then(|| expand_tilde(arg, self.home.as_deref()));
        let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match ui::export::write(target.as_deref(), &here, &name, &text) {
            Ok(path) => self.flash(fill(t.exported, &[&self.shown_path(&path)])),
            Err(e) => {
                let path = target.unwrap_or_else(|| here.join(&name));
                self.flash(fill(
                    t.export_failed,
                    &[&self.shown_path(&path), &e.to_string()],
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn page_up_and_page_down_scroll_a_conversation_by_its_page() {
        let (mut app, _) = with_thread();
        let th = app.thread.as_ref().unwrap();
        th.scroll.set(12);
        th.page.set(5);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 7);
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 12);
    }

    #[test]
    fn c_notes_the_words_chosen_and_n_sends_them_to_who_wrote_them() {
        let (mut app, worker) = talk_with_words();
        let answer = app.world.journal.len() - 1;
        keys(&mut app, "vjswc");
        assert!(app.typing && app.input.is_empty());
        assert!(app.thread.as_ref().unwrap().copy.is_none(), "c leaves");
        let note = TalkNote {
            to: worker,
            entry: answer,
            quote: "alpha beta".into(),
            text: String::new(),
        };
        assert_eq!(app.noting, Some(Noting::Words(note.clone())));
        app.input = "say why".into();
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.noting.is_none());
        let kept = TalkNote {
            text: "say why".into(),
            ..note.clone()
        };
        assert_eq!(app.talk_notes, std::slice::from_ref(&kept));
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_noted, &["1"])
        );
        // The same words again: the note to write over.
        keys(&mut app, "vjswc");
        assert_eq!(app.input, "say why");
        app.input = "say why, shortly".into();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes.len(), 1, "in place of the one there was");
        // Other words of the same message, or the same words of another
        // message: a note of their own.
        keys(&mut app, "vjjsc");
        assert!(app.input.is_empty(), "no note of the other words to reopen");
        app.input = "other words".into();
        app.on_key(key(KeyCode::Enter));
        app.talk_notes.push(TalkNote {
            entry: answer - 1,
            ..app.talk_notes[0].clone()
        });
        assert_eq!(app.talk_notes.len(), 3);
        keys(&mut app, "vjswc");
        app.input.clear();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes.len(), 2, "written empty, only that one goes");
        assert_eq!(app.talk_notes[0].quote, "delta epsilon");
        app.talk_notes.clear();
        keys(&mut app, "vjswc");
        app.input = "say why, shortly".into();
        app.on_key(key(KeyCode::Enter));
        // The person's own line: to the agent it went to.
        keys(&mut app, "ggvjsc");
        app.input = "and this".into();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes[1].to, worker);
        assert_eq!(app.talk_notes[1].quote, "first");
        let was = app.world.journal.len();
        keys(&mut app, "n");
        assert!(app.talk_notes.is_empty());
        let Event::YouSaid { to, text } = &app.world.journal[was].event else {
            panic!("{:?}", app.world.journal[was].event);
        };
        assert_eq!(*to, worker);
        let message = "Notes on words from our conversation, each under the words it is \
                       about.\n\n> first\nand this\n\n> alpha beta\nsay why, shortly";
        assert_eq!(text, message, "one message, the older line first");
        let name = app.world.agents[worker].name.clone();
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_notes_sent, &["2", &name])
        );
        keys(&mut app, "n");
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().no_talk_notes);
    }

    #[test]
    fn one_note_sent_is_said_as_one() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.journal.clear();
        let id = app.world.number();
        app.world.note(Event::Sent {
            from: 0,
            to: 1,
            kind: MsgKind::Inform,
            text: "do the parser".into(),
            id,
            re: None,
        });
        app.open_thread(0);
        keys(&mut app, "vjsc");
        app.input = "to the lead".into();
        app.on_key(key(KeyCode::Enter));
        keys(&mut app, "n");
        let name = app.world.agents[0].name.clone();
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            format!("1 note went to {name}")
        );
    }

    #[test]
    fn notes_on_a_talk_between_agents_go_each_to_its_writer() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.journal.clear();
        for (from, to, text) in [(0, 1, "do the parser"), (1, 0, "done the parser")] {
            let id = app.world.number();
            app.world.note(Event::Sent {
                from,
                to,
                kind: MsgKind::Inform,
                text: text.into(),
                id,
                re: None,
            });
        }
        app.open_thread(0);
        // The lead's line, then the worker's.
        keys(&mut app, "vjsc");
        app.input = "to the lead".into();
        app.on_key(key(KeyCode::Enter));
        keys(&mut app, "jvjsc");
        app.input = "to the worker".into();
        app.on_key(key(KeyCode::Enter));
        let to: Vec<AgentId> = app.talk_notes.iter().map(|n| n.to).collect();
        assert_eq!(to, [0, 1]);
        keys(&mut app, "n");
        let said: Vec<(AgentId, String)> = (app.world.journal.iter())
            .filter_map(|e| match &e.event {
                Event::YouSaid { to, text } => Some((*to, text.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(said.len(), 2, "{said:?}");
        assert_eq!(said[0].0, 0);
        assert!(
            said[0]
                .1
                .ends_with("On message 1:\n> do the parser\nto the lead"),
            "{said:?}"
        );
        assert_eq!(said[1].0, 1);
        assert!(
            said[1]
                .1
                .ends_with("On message 2:\n> done the parser\nto the worker")
        );
        let names = format!("{}, {}", app.world.agents[0].name, app.world.agents[1].name);
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_notes_sent, &["2", &names])
        );
    }

    #[test]
    fn a_note_that_could_not_go_stays_and_an_empty_one_goes_away() {
        let (mut app, _) = talk_with_words();
        keys(&mut app, "vjc");
        app.input = "keep me".into();
        app.on_key(key(KeyCode::Enter));
        app.world.budget = 0;
        keys(&mut app, "n");
        assert_eq!(app.talk_notes.len(), 1, "the budget is spent");
        assert!(!app.flash.as_ref().unwrap().0.contains("went to"));
        // Written empty, the note is taken away.
        keys(&mut app, "vjc");
        assert_eq!(app.input, "keep me");
        app.input.clear();
        app.on_key(key(KeyCode::Enter));
        assert!(app.talk_notes.is_empty());
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().note_dropped);
        // Blank words take none, and the choice stays.
        app.world.note(Event::ToYou {
            from: 1,
            kind: MsgKind::Answer,
            text: "a\n\nb".into(),
            options: Vec::new(),
        });
        keys(&mut app, "Gvjjsc");
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().note_blank);
        assert!(app.thread.as_ref().unwrap().copy.is_some());
        // Esc while writing one keeps nothing.
        keys(&mut app, "kc");
        app.on_key(key(KeyCode::Esc));
        assert!(app.noting.is_none() && app.talk_notes.is_empty());
    }

    #[test]
    fn h_l_and_z_fold_messages_of_a_conversation_and_the_choice_is_kept() {
        let (mut app, _) = talk_with_words();
        let collapsed = app.collapsed.clone();
        let folded = |app: &App| {
            let th = app.thread.as_ref().unwrap();
            (th.folds.folded(0), th.folds.folded(1))
        };
        let follows = |app: &App| app.thread.as_ref().unwrap().follow.take();
        follows(&app);
        keys(&mut app, "h");
        assert_eq!(folded(&app), (false, true));
        assert!(follows(&app), "the selected message stays in view");
        keys(&mut app, "l");
        assert_eq!(folded(&app), (false, false));
        assert!(follows(&app));
        app.on_key(key(KeyCode::Left));
        assert_eq!(folded(&app), (false, true));
        app.on_key(key(KeyCode::Right));
        assert_eq!(folded(&app), (false, false));
        keys(&mut app, "za");
        assert_eq!(folded(&app), (false, true));
        assert!(follows(&app));
        keys(&mut app, "za");
        assert_eq!(folded(&app), (false, false));
        assert_eq!(app.collapsed, collapsed, "not the salon's folds");
        keys(&mut app, "zM");
        assert_eq!(folded(&app), (true, true));
        assert!(follows(&app));
        keys(&mut app, "l");
        assert_eq!(folded(&app), (true, false), "one opened of all folded");
        // The next conversation opens folded too, until zR.
        app.on_key(key(KeyCode::Esc));
        app.open_thread(0);
        assert_eq!(folded(&app), (true, true));
        keys(&mut app, "zR");
        assert_eq!(folded(&app), (false, false));
        assert!(!app.fold_talk);
        app.on_key(key(KeyCode::Esc));
        app.open_thread(0);
        assert_eq!(folded(&app), (false, false));
        // `:fold` and `:unfold` with a conversation open fold it.
        app.command("fold");
        assert_eq!(folded(&app), (true, true));
        assert!(app.thread.is_some() && app.fold_talk);
        app.command("unfold");
        assert_eq!(folded(&app), (false, false));
        assert_eq!(app.collapsed, collapsed);
    }

    #[test]
    fn folds_say_which_messages_are_one_row() {
        let mut f = Folds::every(true);
        assert!(f.folded(0) && f.folded(7));
        f.set(7, false);
        assert!(f.folded(0) && !f.folded(7));
        f.set(7, true);
        assert_eq!(f, Folds::every(true), "back as it was");
        f.set(3, true);
        assert_eq!(f, Folds::every(true));
        let mut f = Folds::every(false);
        assert!(!f.folded(2));
        f.set(2, true);
        assert!(f.folded(2) && !f.folded(1));
        f.set(2, false);
        assert_eq!(f, Folds::default());
    }

    #[test]
    fn a_message_written_from_the_conversation_window_lands_in_it_selected() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        app.thread.as_mut().unwrap().follow.set(false);
        app.typing = true;
        app.input = format!("@{name} second");
        app.on_key(key(KeyCode::Enter));
        let i = app.world.journal.len() - 1;
        assert!(matches!(app.world.journal[i].event, Event::YouSaid { .. }));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 2);
        assert_eq!(th.entries[1], i);
        assert_eq!(th.sel, 1, "the line just written is selected");
        assert!(th.follow.get());
    }

    #[test]
    fn the_persons_own_line_is_selected_even_when_reading_further_up() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        app.world.said(worker, "second");
        app.follow_thread();
        let th = app.thread.as_mut().unwrap();
        th.sel = 0;
        th.follow.set(false);
        app.typing = true;
        app.input = format!("@{name} third");
        app.on_key(key(KeyCode::Enter));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 3);
        assert_eq!(th.sel, 2, "what the person wrote is selected");
        assert!(th.follow.get());
    }

    #[test]
    fn nothing_new_moves_nothing() {
        let (mut app, _) = with_thread();
        app.thread.as_mut().unwrap().follow.set(false);
        app.follow_thread();
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (1, 0));
        assert!(!th.follow.get(), "at the end with nothing new: no scroll");
    }

    #[test]
    fn a_reply_that_comes_while_the_last_line_is_selected_is_followed() {
        let (mut app, worker) = with_thread();
        app.thread.as_mut().unwrap().follow.set(false);
        app.world.note(Event::Replied {
            agent: worker,
            text: "yes".into(),
        });
        let i = app.world.journal.len() - 1;
        app.step();
        let th = app.thread.as_ref().unwrap();
        assert!(th.entries.contains(&i), "{:?}", th.entries);
        assert_eq!(th.entries[0], th.entries[0].min(i));
        assert_eq!(th.sel + 1, th.entries.len(), "still at the end");
        assert!(th.follow.get());
    }

    #[test]
    fn a_reply_that_comes_while_the_person_reads_further_up_leaves_the_selection() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        app.follow_thread();
        let th = app.thread.as_mut().unwrap();
        assert_eq!(th.entries.len(), 2);
        th.sel = 0;
        th.follow.set(false);
        app.world.note(Event::Replied {
            agent: worker,
            text: "yes".into(),
        });
        let i = app.world.journal.len() - 1;
        app.follow_thread();
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 3);
        assert_eq!(th.entries[2], i);
        assert_eq!(th.sel, 0, "reading up: the selection stays");
        assert!(!th.follow.get());
        // Followed once: a second pass adds nothing.
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries.len(), 3);
    }

    #[test]
    fn brackets_go_from_turn_to_turn_in_a_conversation() {
        // Speakers: you you · worker · you · worker worker.
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        let reply = |app: &mut App, text: &str| {
            app.world.note(Event::Replied {
                agent: worker,
                text: text.into(),
            })
        };
        reply(&mut app, "one");
        app.world.said(worker, "third");
        reply(&mut app, "two");
        reply(&mut app, "three");
        app.follow_thread();
        let entries = app.thread.as_ref().unwrap().entries.clone();
        assert_eq!(entries.len(), 6);
        assert_eq!(ui::turn_starts(&app.world, &entries), [0, 2, 3, 4]);
        let pane = app.pane;
        let press = |app: &mut App, c: char| {
            app.thread.as_mut().unwrap().follow.set(false);
            app.on_key(key(KeyCode::Char(c)));
            let th = app.thread.as_ref().unwrap();
            assert!(th.follow.get(), "{c}: the drawing scrolls to it");
            th.sel
        };
        app.thread.as_mut().unwrap().sel = 0;
        assert_eq!(press(&mut app, ']'), 2, "past the person's second line");
        assert_eq!(press(&mut app, ']'), 3);
        assert_eq!(press(&mut app, ']'), 4);
        app.thread.as_mut().unwrap().follow.set(false);
        app.on_key(key(KeyCode::Char(']')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 4, "no turn after the last one");
        assert!(!th.follow.get());
        assert_eq!(
            press(&mut app, '['),
            3,
            "from a turn's start: the one before"
        );
        app.thread.as_mut().unwrap().sel = 5;
        assert_eq!(press(&mut app, '['), 4, "from inside a turn: its start");
        assert_eq!(press(&mut app, '['), 3);
        assert_eq!(press(&mut app, '['), 2);
        assert_eq!(press(&mut app, '['), 0);
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 0, "none before the first");
        assert_eq!(app.pane, pane, "the conversation keeps [ ], no panel moves");
    }

    #[test]
    fn a_conversation_open_over_a_change_decides_nothing() {
        let mut app = quiet();
        app.world.change_ready(a_change(3));
        app.world.replied(3, "it is done");
        app.pane = Pane::Inbox;
        let reply = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Reply)
            .expect("its reply waits");
        let change = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Change)
            .expect("its change waits");
        app.open_thread(reply);
        app.inbox_sel = (app.inbox_rows().iter())
            .position(|&i| i == change)
            .expect("the change is on the list");
        for k in ['a', 'r'] {
            app.on_key(key(KeyCode::Char(k)));
            assert!(
                app.world.inbox[change].open,
                "{k} decided what is not shown"
            );
            assert!(app.world.change_of(3).is_some(), "{k}");
            assert!(!app.typing, "{k}");
            let said = app.flash.take().expect("it says why").0;
            assert_eq!(said, app.lang.text().talk_decides_nothing);
        }
        // With the conversation closed the same key decides.
        app.on_key(key(KeyCode::Esc));
        assert!(app.thread.is_none());
        app.on_key(key(KeyCode::Char('a')));
        let said = app.flash.as_ref().map(|f| f.0.as_str());
        assert_ne!(said, Some(app.lang.text().talk_decides_nothing));
    }

    #[test]
    fn a_conversation_of_one_speaker_is_one_turn_and_other_lines_have_none() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        app.follow_thread();
        let entries = app.thread.as_ref().unwrap().entries.clone();
        assert_eq!(ui::turn_starts(&app.world, &entries), [0]);
        app.world.note(Event::Forgotten { agent: worker });
        let lone = app.world.journal.len() - 1;
        assert_eq!(ui::turn_starts(&app.world, &[lone]), [0]);
        assert_eq!(ui::turn_starts(&app.world, &[lone, lone]), [0]);
        assert_eq!(ui::turn_starts(&app.world, &[lone, entries[0]]), [0, 1]);
        assert!(ui::turn_starts(&app.world, &[]).is_empty());
    }

    #[test]
    fn a_line_of_another_conversation_stays_out_of_the_open_one() {
        let (mut app, worker) = with_thread();
        let other = (0..app.world.agents.len())
            .find(|&id| id != worker && app.world.agents[id].role == Role::Worker)
            .unwrap();
        app.world.said(other, "elsewhere");
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries.len(), 1);
        // A thread opened on a line that is not a conversation follows nothing.
        app.world.note(Event::Forgotten { agent: worker });
        let i = app.world.journal.len() - 1;
        app.open_thread(i);
        app.world.said(worker, "third");
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries, [i]);
        app.thread = None;
        app.follow_thread();
        assert!(app.thread.is_none());
    }

    /// A demo whose journal is only `n` lines that stand alone, with the
    /// journal panel of the salon at hand.
    fn journal_of(n: usize) -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.journal.clear();
        for _ in 0..n {
            app.world.note(Event::Failed { agent: 0 });
        }
        app.open_window(Window::Journal);
        app
    }

    #[test]
    fn m_marks_the_line_at_hand_and_again_takes_the_mark_away() {
        let mut app = journal_of(3);
        let t = app.lang.text();
        app.journal_sel = Some(1);
        app.on_key(key(KeyCode::Char('m')));
        assert_eq!(app.world.marks, BTreeSet::from([1]));
        assert_eq!(flashed(&app).as_deref(), Some(t.marked));
        app.on_key(key(KeyCode::Char('m')));
        assert!(app.world.marks.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.unmarked));
        // `:mark` is the same.
        app.command("mark");
        assert_eq!(app.world.marks, BTreeSet::from([1]));
        // Nothing at hand: no journal window, or the story in it.
        app.world.marks.clear();
        for at in [|a: &mut App| a.close_window(), |a: &mut App| a.story = true] {
            let mut app = journal_of(3);
            app.journal_sel = Some(0);
            at(&mut app);
            app.on_key(key(KeyCode::Char('m')));
            assert!(app.world.marks.is_empty());
            assert_eq!(flashed(&app).as_deref(), Some(t.mark_where));
        }
        // In a conversation, its selected line, whatever the panel.
        let mut app = journal_of(3);
        app.pane = Pane::Salon;
        app.open_thread(2);
        app.on_key(key(KeyCode::Char('m')));
        assert_eq!(app.world.marks, BTreeSet::from([2]));
        assert!(app.thread.is_some(), "it stays open");
    }

    #[test]
    fn quote_goes_to_the_next_marked_row_of_the_journal_round_to_the_first() {
        let mut app = journal_of(5);
        let t = app.lang.text();
        // Rows newest first: journal lines 4 3 2 1 0.
        app.world.marks.extend([3, 1]);
        app.zoom = Zoom::Focus(0);
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('\'')));
        // The journal's window opens over what was there.
        assert_eq!(
            (app.zoom, app.pane, app.window),
            (Zoom::Focus(0), Pane::Journal, Some(Window::Journal))
        );
        assert_eq!(
            app.journal_sel,
            Some(3),
            "nothing selected: the first marked"
        );
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(1));
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(3), "round again");
        app.journal_sel = Some(2);
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(1), "from between two: the one after");
        assert!(flashed(&app).is_none());
        // None marked: the selection stays on a row, and it says so.
        app.world.marks.clear();
        app.journal_sel = None;
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(4), "the first row");
        assert_eq!(flashed(&app).as_deref(), Some(t.no_marks));
    }

    #[test]
    fn quote_in_a_conversation_goes_among_its_own_marked_lines() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.journal.clear();
        let said = |from, to| Event::Sent {
            from,
            to,
            kind: MsgKind::Ask,
            text: "x".into(),
            id: 0,
            re: None,
        };
        for _ in 0..2 {
            app.world.note(said(0, 1));
            app.world.note(Event::Failed { agent: 0 });
        }
        app.world.note(said(1, 0));
        // The conversation is lines 0 2 4; line 1 is marked but not in it.
        app.world.marks.extend([1, 4]);
        app.open_thread(0);
        assert_eq!(app.thread.as_ref().unwrap().entries, [0, 2, 4]);
        app.on_key(key(KeyCode::Char('\'')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 2);
        assert!(th.follow.get(), "the drawing scrolls to it");
        // At the only mark: it stays there.
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 2);
        // None in it: the selection stays, and it says so.
        app.world.marks.remove(&4);
        app.thread.as_mut().unwrap().sel = 1;
        app.thread.as_ref().unwrap().follow.set(false);
        app.on_key(key(KeyCode::Char('\'')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 1);
        assert!(!th.follow.get());
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_marks));
        assert_eq!(app.pane, Pane::Salon, "the journal is not where it went");
    }

    #[test]
    fn next_of_is_the_first_after_or_the_first_of_all() {
        assert_eq!(next_of(&[1, 3], None), Some(1));
        assert_eq!(next_of(&[1, 3], Some(0)), Some(1));
        assert_eq!(next_of(&[1, 3], Some(1)), Some(3));
        assert_eq!(next_of(&[1, 3], Some(2)), Some(3));
        assert_eq!(next_of(&[1, 3], Some(3)), Some(1));
        assert_eq!(next_of(&[], Some(3)), None);
        assert_eq!(next_of(&[], None), None);
    }

    fn said_to(app: &App) -> Vec<(AgentId, String)> {
        (app.world.journal.iter())
            .filter_map(|e| match &e.event {
                Event::YouSaid { to, text } => Some((*to, text.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn slash_opens_the_leaders_chat_and_its_line_stays_open_after_a_message() {
        let (mut app, _) = four_idle();
        app.world.journal.clear();
        let lead = app.world.projects[0].leader;
        for open in ['/', 'i'] {
            app.on_key(key(KeyCode::Char(open)));
            assert!(app.typing && app.input.is_empty(), "{open}");
            assert_eq!(app.chat_agent(), Some(lead), "{open}");
            let th = app.thread.as_ref().unwrap();
            assert!(th.entries.is_empty() && !th.passing, "nothing said yet");
            assert_eq!(app.typing_target(), app.world.agents[lead].name);
            app.on_key(key(KeyCode::Esc));
            assert!(!app.typing && app.thread.is_some(), "esc leaves the line");
            app.on_key(key(KeyCode::Esc));
            assert!(app.thread.is_none(), "esc again leaves the chat");
        }
        app.on_key(key(KeyCode::Char('/')));
        type_in(&mut app, "hello");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said_to(&app), [(lead, "hello".to_string())]);
        assert!(app.typing && app.input.is_empty(), "the line stays open");
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (1, 0), "it stands in the chat");
        type_in(&mut app, "again");
        app.on_key(key(KeyCode::Enter));
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (2, 1));
        // Reading further up, `/` writes in the chat as it stands.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 0);
        app.on_key(key(KeyCode::Char('/')));
        assert!(app.typing);
        assert_eq!(app.thread.as_ref().unwrap().sel, 0, "not opened anew");
        // A `:` command from the chat's line is a command: the line ends.
        type_in(&mut app, ":lang en");
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.thread.is_some());
    }

    #[test]
    fn a_line_written_in_a_chat_goes_to_its_agent_unless_it_names_another() {
        let (mut app, workers) = four_idle();
        app.world.journal.clear();
        let (one, two) = (workers[0], workers[1]);
        let lead = app.world.projects[0].leader;
        app.open_chat(one);
        assert_eq!(app.chat_agent(), Some(one));
        app.on_key(key(KeyCode::Char('/')));
        assert_eq!(app.typing_target(), app.world.agents[one].name);
        type_in(&mut app, "to you");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said_to(&app), [(one, "to you".to_string())]);
        // `@` in a chat writes in it too: no name is asked for.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('@')));
        assert!(app.typing && app.input.is_empty());
        // A name on its line goes there; the chat the person opened stays.
        let other = app.world.agents[two].name.clone();
        type_in(&mut app, &format!("@{other} and you"));
        assert_eq!(app.chat_agent(), Some(one), "the open chat is kept");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said_to(&app).last(), Some(&(two, "and you".to_string())));
        assert!(app.typing, "still the chat's line");
        // A conversation between two agents is no chat: the leader's opens.
        app.on_key(key(KeyCode::Esc));
        app.world.note(Event::Talk {
            from: one,
            to: two,
            text: "between us".into(),
        });
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.chat_agent(), None);
        app.on_key(key(KeyCode::Char('/')));
        assert_eq!(app.chat_agent(), Some(lead));
    }

    #[test]
    fn a_name_on_the_line_brings_its_conversation_which_goes_with_the_line() {
        let (mut app, workers) = four_idle();
        app.world.journal.clear();
        let (one, two) = (workers[0], workers[1]);
        let (a, b) = (
            app.world.agents[one].name.clone(),
            app.world.agents[two].name.clone(),
        );
        app.on_key(key(KeyCode::Char('@')));
        type_in(&mut app, &a);
        assert!(app.thread.is_none(), "the name is not whole yet");
        type_in(&mut app, " ");
        assert_eq!(app.chat_agent(), Some(one));
        assert!(app.thread.as_ref().unwrap().passing);
        // Another name: its conversation instead; no name: none.
        app.input = format!("@{b} ");
        type_in(&mut app, "x");
        assert_eq!(app.chat_agent(), Some(two));
        app.input = "@nobody ".into();
        type_in(&mut app, "x");
        assert!(app.thread.is_none());
        app.input = format!("@{a} ");
        type_in(&mut app, "hi");
        assert_eq!(app.chat_agent(), Some(one));
        // Sent: the conversation goes with the line, back where one was.
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said_to(&app), [(one, "hi".to_string())]);
        assert!(!app.typing && app.thread.is_none());
        // Dropped: the same.
        app.on_key(key(KeyCode::Char('@')));
        type_in(&mut app, &format!("{a} "));
        assert!(app.thread.is_some());
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert!(!app.typing && app.thread.is_none());
        // Not over a change: what is written there is about its lines.
        app.zoom = Zoom::Diff(one);
        app.on_key(key(KeyCode::Char('@')));
        assert_eq!(app.input, format!("@{a} "));
        type_in(&mut app, "x");
        assert!(app.thread.is_none());
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        // On its log the name is filled in, so the conversation is there
        // at once.
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('@')));
        assert_eq!(app.chat_agent(), Some(one));
        // Not while a note is written, whatever its text starts with.
        app.thread = None;
        app.zoom = Zoom::None;
        app.typing = true;
        app.input = format!("@{a} x");
        app.searching = true;
        app.chat_follows();
        assert!(app.thread.is_none());
    }

    #[test]
    fn a_chat_nothing_was_said_in_takes_its_keys_and_follows_the_first_line() {
        let (mut app, workers) = four_idle();
        app.world.journal.clear();
        let one = workers[0];
        app.open_chat(one);
        let sel = app.selected;
        for c in "jkG[]%cyCYvhln ".chars() {
            app.on_key(key(KeyCode::Char(c)));
            assert!(app.thread.is_some() && !app.copying(), "{c:?}");
        }
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Home,
            KeyCode::End,
        ] {
            app.on_key(key(code));
        }
        keys(&mut app, "ggzazMzRm'");
        assert!(app.thread.is_some() && app.copy.is_none());
        assert_eq!(app.selected, sel, "nothing under it moved");
        // The agent's line comes first: followed, selected.
        app.world.note(Event::Replied {
            agent: one,
            text: "hello".into(),
        });
        app.follow_thread();
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (1, 0));
        // A line between others is not its.
        app.world.note(Event::Replied {
            agent: workers[1],
            text: "not here".into(),
        });
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries.len(), 1);
        app.on_key(key(KeyCode::Esc));
        assert!(app.thread.is_none());
    }

    #[test]
    fn a_message_refused_closes_the_chats_line_so_the_reason_is_seen() {
        let (mut app, _) = four_idle();
        app.world.budget = 0;
        app.on_key(key(KeyCode::Char('/')));
        type_in(&mut app, "hello");
        app.on_key(key(KeyCode::Enter));
        assert!(said_to(&app).is_empty());
        assert!(!app.typing && app.thread.is_some());
        let said = app.flash.as_ref().map(|f| f.0.clone());
        assert_eq!(said, Some(ui::words::pot_spent(Lang::En, &app.world)));
    }
}
