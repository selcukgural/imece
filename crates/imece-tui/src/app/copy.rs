//! Choosing lines and words to copy.

use super::*;

impl App {
    /// Whether lines are being chosen to copy: in the open conversation,
    /// else in the open change.
    pub(super) fn copying(&self) -> bool {
        match &self.thread {
            Some(th) => th.copy.is_some(),
            None => matches!(self.zoom, Zoom::Diff(id) if self.diff_copy_of(id).is_some()),
        }
    }

    /// A key while lines are being chosen to copy (`v`): the cursor moves
    /// (`j k`, a page, `gg G`), `y` copies, `Y` in a change copies the
    /// new side, `v` and esc leave. In a conversation the cursor also
    /// moves word by word (`w b h l`, the choice turns word-wise), `s`
    /// starts the choice where it is and `c` notes the words chosen. `?` and `q` work as anywhere; any
    /// other key does nothing, so one meant for the change (`a` applies)
    /// is not taken by mistake. `false` when nothing is being chosen.
    pub(super) fn copy_key(&mut self, k: KeyEvent) -> bool {
        if !self.copying() || k.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.copy_move(|at, _, _| at.saturating_sub(1));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.copy_move(|at, _, _| at + 1);
            }
            KeyCode::PageUp => self.page(false, false),
            KeyCode::PageDown | KeyCode::Char(' ') => self.page(true, false),
            KeyCode::Home => self.jump(true),
            KeyCode::End | KeyCode::Char('G') => self.jump(false),
            KeyCode::Char('g') => self.pending = Some('g'),
            KeyCode::Char('y') => self.copy_chosen(false),
            KeyCode::Char('Y') => self.copy_chosen(true),
            KeyCode::Char('w') | KeyCode::Char('l') | KeyCode::Right => self.word_step(true),
            KeyCode::Char('b') | KeyCode::Char('h') | KeyCode::Left => self.word_step(false),
            KeyCode::Char('s') => self.choose_from_here(),
            KeyCode::Char('c') => self.note_words(),
            KeyCode::Esc | KeyCode::Char('v') => {
                if let Some(th) = &mut self.thread {
                    th.copy = None;
                }
                self.diff_copy = None;
            }
            KeyCode::Char('?') | KeyCode::Char('q') => return false,
            _ => {}
        }
        true
    }

    /// Moves the cursor of the lines being chosen to copy: `to` gets the
    /// cursor, the last line and a page of lines and says where it goes
    /// (no further than the last). The conversation selects the message
    /// the cursor is in and keeps it in view; the change scrolls as
    /// little as it can. `false` when nothing is being chosen.
    pub(super) fn copy_move(&mut self, to: impl FnOnce(usize, usize, usize) -> usize) -> bool {
        if let Some(th) = &mut self.thread {
            let Some(c) = &mut th.copy else {
                return false;
            };
            let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
            let last = lines.len().saturating_sub(1);
            c.at = to(c.at, last, usize::from(th.page.get()).max(1)).min(last);
            if let Some(l) = lines.get(c.at) {
                th.sel = l.entry;
                if let Some((_, word)) = &mut th.words {
                    *word = (*word).min(l.words.len().saturating_sub(1));
                }
            }
            th.follow.set(true);
            return true;
        }
        let Zoom::Diff(id) = self.zoom else {
            return false;
        };
        let (Some(mut c), Some(change)) = (self.diff_copy_of(id), self.world.change_of(id)) else {
            return false;
        };
        let last = change.patch.lines().count().saturating_sub(1);
        let page = usize::from(self.diff_page.get()).max(1);
        c.at = to(c.at, last, page).min(last);
        self.diff_scroll = ui::in_view(c.at, c.at + 1, self.diff_scroll, page);
        self.diff_copy = Some((id, patch_key(&change.patch), c));
        true
    }

    /// Copies the lines chosen and leaves the choosing: a conversation's
    /// as [`ui::copy_lines`] reads them, a change's as written or, with
    /// `new_side`, as the code reads after it. A new side with nothing
    /// on it is said, and the choice stays to be changed.
    fn copy_chosen(&mut self, new_side: bool) {
        if let Some(th) = &mut self.thread {
            if th.copy.is_none() {
                return;
            }
            let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
            self.copy = Some(ui::chosen_text(&lines, th));
            th.copy = None;
            return;
        }
        let Zoom::Diff(id) = self.zoom else {
            return;
        };
        let (Some(c), Some(change)) = (self.diff_copy_of(id), self.world.change_of(id)) else {
            return;
        };
        let (from, to) = c.span();
        match ui::copy_patch(&change.patch, from, to, new_side) {
            Some(text) => {
                self.copy = Some(text);
                self.diff_copy = None;
            }
            None => {
                let t = self.lang.text();
                self.flash(t.copy_no_new.to_string());
            }
        }
    }

    /// `:copy [PATH]`: the paths git ignores that are copied from the
    /// project at hand's checkout into its workers' worktrees at their
    /// start (said when none is given, with a word on its
    /// `.worktreeinclude` when it has one), one more (checked: literal,
    /// inside, ignored; the demo checks only the first), or one fewer
    /// when it is listed already. A trailing `/` is the same path.
    pub(super) fn copy_path(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        if arg.is_empty() {
            let includes = match &self.engine {
                Engine::Live(l) => l.includes(p),
                Engine::Sim(s) => s.includes(p),
                Engine::View(_) => false,
            };
            let copies = &self.world.projects[p].copies;
            let said = match (copies.is_empty(), includes) {
                (true, false) => fill(t.copy_none, &[&name]),
                (true, true) => fill(t.copy_file, &[&name]),
                (false, false) => fill(t.copy_list, &[&name, &copies.join(", ")]),
                (false, true) => fill(
                    t.copy_also,
                    &[&fill(t.copy_list, &[&name, &copies.join(", ")])],
                ),
            };
            return self.flash(said);
        }
        let path = arg.trim_end_matches('/');
        let copies = &mut self.world.projects[p].copies;
        if let Some(i) = copies.iter().position(|c| c == path) {
            copies.remove(i);
            return self.flash(fill(t.copy_removed, &[path]));
        }
        let checked = match &self.engine {
            Engine::Live(l) => l.may_copy(p, path),
            Engine::Sim(_) => imece_agent::copy::literal(path),
            Engine::View(_) => return self.read_only(),
        };
        match checked {
            Ok(()) => {
                self.world.projects[p].copies.push(path.to_string());
                self.flash(fill(t.copy_added, &[path, &name]));
            }
            Err(why) => self.flash(fill(t.copy_refused, &[path, &why])),
        }
    }

    /// The person's copy request went through, or did not.
    pub fn copied(&mut self, how: Result<&str, String>) {
        let t = self.lang.text();
        let msg = match how {
            Ok(how) => fill(t.copied, &[how]),
            Err(why) => fill(t.copy_failed, &[&why]),
        };
        self.flash(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn a_copy_choice_spans_its_anchor_and_cursor_either_way() {
        let down = Copying { anchor: 3, at: 5 };
        let up = Copying { anchor: 5, at: 3 };
        for c in [down, up] {
            assert_eq!(c.span(), (3, 5));
            assert_eq!(c.len(), 3);
            assert!(c.holds(3) && c.holds(4) && c.holds(5), "{c:?}");
            assert!(!c.holds(2) && !c.holds(6), "{c:?}");
        }
        assert_eq!(Copying::new(7), Copying { anchor: 7, at: 7 });
        assert_eq!(Copying::new(7).len(), 1);
    }

    #[test]
    fn v_in_a_change_chooses_lines_from_the_top_one_and_moves_like_the_view() {
        let patch: String = (0..30).map(|i| format!("+line {i}\n")).collect();
        let (mut app, worker) = with_change(&patch);
        app.diff_page.set(10);
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(4)));
        let at = |app: &App| (app.diff_copy_of(worker).unwrap().at, app.diff_scroll);
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Down));
        assert_eq!(at(&app), (6, 4), "the view stays while the cursor is in it");
        app.on_key(key(KeyCode::Char('k')));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Up));
        assert_eq!(at(&app), (3, 3), "above the view: it scrolls up to it");
        app.on_key(ctrl('d'));
        assert_eq!(at(&app), (8, 3), "half a page");
        app.on_key(ctrl('f'));
        assert_eq!(at(&app), (18, 9), "a page, the view just far enough");
        app.on_key(ctrl('u'));
        assert_eq!(at(&app), (13, 9));
        app.on_key(ctrl('b'));
        assert_eq!(at(&app), (3, 3));
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(at(&app).0, 13);
        app.on_key(key(KeyCode::Char(' ')));
        assert_eq!(at(&app).0, 23);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(at(&app).0, 13);
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(at(&app), (29, 20), "the last line");
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(at(&app).0, 29, "no further");
        app.on_key(key(KeyCode::Home));
        assert_eq!(at(&app), (0, 0));
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(at(&app).0, 0, "no further up");
        app.on_key(key(KeyCode::End));
        assert_eq!(at(&app).0, 29);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), (0, 0), "gg");
        assert_eq!(
            app.diff_copy_of(worker).unwrap().anchor,
            4,
            "the anchor stays"
        );
        // `y` copies the lines as written and leaves.
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            app.copy.as_deref(),
            Some("+line 0\n+line 1\n+line 2\n+line 3\n+line 4")
        );
        assert_eq!(app.diff_copy_of(worker), None);
        assert_eq!(app.zoom, Zoom::Diff(worker), "the change stays open");
    }

    #[test]
    fn v_in_a_change_starts_on_its_last_line_at_most() {
        let (mut app, worker) = with_change("+a\n+b\n+c\n");
        app.diff_scroll = 9;
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(2)));
    }

    #[test]
    fn big_y_in_a_change_copies_the_new_side_or_says_there_is_none() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n";
        let (mut app, worker) = with_change(patch);
        app.diff_scroll = 5;
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy, None, "the line chosen was removed");
        assert_eq!(flashed(&app).as_deref(), Some(Lang::En.text().copy_no_new));
        assert_eq!(
            app.diff_copy_of(worker),
            Some(Copying::new(5)),
            "still choosing"
        );
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy.take().as_deref(), Some("keep"), "heads left out");
        assert_eq!(app.diff_copy_of(worker), None);
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('G')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy.as_deref(), Some("keep\nnew"));
    }

    #[test]
    fn while_choosing_lines_other_keys_do_nothing_but_help_and_quit() {
        let (mut app, worker) = with_change("+a\n+b\n");
        app.on_key(key(KeyCode::Char('v')));
        for c in ['a', 'r', 'c', 'x', 'n', '>', 'w', 't'] {
            app.on_key(key(KeyCode::Char(c)));
        }
        assert!(app.world.change_of(worker).is_some(), "not applied");
        assert!(!app.typing && !app.diff_wrap && app.diff_notes.is_empty());
        assert_eq!(app.zoom, Zoom::Diff(worker));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(0)));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help, "? opens the help");
        app.help = false;
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.pending, Some('q'), "q asks to quit");
        app.pending = None;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.diff_copy_of(worker), None, "esc leaves the choosing");
        assert_eq!(app.zoom, Zoom::Diff(worker), "and only that");
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), None, "v again leaves too");
    }

    #[test]
    fn a_copy_choice_goes_with_its_change_and_its_view() {
        let (mut app, worker) = with_change("+a\n+b\n");
        app.on_key(key(KeyCode::Char('v')));
        // A newer change from the worker has other lines.
        let mut newer = a_change(worker);
        newer.patch = "+c\n".into();
        app.world.change_ready(newer);
        assert_eq!(app.diff_copy_of(worker), None);
        app.on_key(key(KeyCode::Char('v')));
        assert!(app.diff_copy_of(worker).is_some());
        // Another view: gone with the next key.
        app.zoom = Zoom::None;
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.diff_copy, None);
        // The patch's hash alone does not stand for another worker.
        let other = (app.world.agents.iter())
            .find(|a| a.id != worker)
            .map(|a| a.id)
            .unwrap();
        let key_of = patch_key(&app.world.change_of(worker).unwrap().patch);
        app.diff_copy = Some((other, key_of, Copying::new(0)));
        assert_eq!(app.diff_copy_of(worker), None);
    }

    #[test]
    fn v_in_a_conversation_chooses_lines_across_messages_and_y_copies_them() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "one\ntwo");
        app.world.said(worker, "```\nthree\n```");
        app.on_key(key(KeyCode::Char('G')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (3, 2));
        let lines = ui::thread_lines(app.lang, &app.world, &th.entries, th.room.get());
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts.len(), 7, "{texts:?}");
        assert_eq!(
            [texts[1], texts[3], texts[4], texts[6]],
            ["first", "one", "two", "three"],
            "a fence is not a line"
        );
        let head = texts[5].to_string();
        assert!(head.contains(&app.world.agents[worker].name), "{head}");
        let th = |app: &App| {
            let th = app.thread.as_ref().unwrap();
            (th.copy, th.sel)
        };
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(th(&app), (Some(Copying::new(5)), 2), "the message's head");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(th(&app).0.unwrap().at, 6, "no further than the last");
        for _ in 0..3 {
            app.on_key(key(KeyCode::Char('k')));
        }
        assert_eq!(
            th(&app),
            (Some(Copying { anchor: 5, at: 3 }), 1),
            "the message follows"
        );
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(app.copy, None, "r is not taken while choosing");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(app.copy, Some(format!("two\n\n{head}")));
        assert_eq!(th(&app).0, None, "y leaves the choosing");
        assert!(app.thread.is_some(), "not the conversation");
    }

    #[test]
    fn choosing_word_by_word_steps_over_words_and_lines_and_copies_them() {
        let (mut app, _) = talk_with_words();
        // From one line chosen whole: the cursor on its first word, then on.
        keys(&mut app, "vjs");
        assert_eq!(choice(&app), (3, 3, None));
        keys(&mut app, "w");
        assert_eq!(choice(&app), (3, 3, Some((0, 1))));
        keys(&mut app, "y");
        assert_eq!(
            app.copy.as_deref(),
            Some("alpha beta"),
            "the words as drawn"
        );
        assert!(app.thread.as_ref().unwrap().copy.is_none(), "y leaves");
        // The whole line is its source; over its end, the next line.
        keys(&mut app, "vjswl");
        assert_eq!(choice(&app), (3, 3, Some((0, 2))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma"));
        keys(&mut app, "vjsww");
        app.on_key(key(KeyCode::Right));
        assert_eq!(choice(&app), (4, 3, Some((0, 0))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma\ndelta"));
        // Back over the start of a line, to the last word of the one above.
        keys(&mut app, "vjjs");
        assert_eq!(choice(&app), (4, 4, None));
        keys(&mut app, "b");
        assert_eq!(choice(&app), (3, 4, Some((0, 2))));
        keys(&mut app, "h");
        app.on_key(key(KeyCode::Left));
        assert_eq!(choice(&app), (3, 4, Some((0, 0))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma\ndelta"));
        // Into the message above: it is the one selected, kept in view.
        keys(&mut app, "vjs");
        app.thread.as_ref().unwrap().follow.set(false);
        keys(&mut app, "bbbbbb");
        let th = app.thread.as_ref().unwrap();
        assert_eq!((choice(&app), th.sel), ((1, 3, Some((0, 0))), 0));
        assert!(th.follow.get());
    }

    #[test]
    fn a_choice_of_lines_turns_word_wise_from_the_ends_it_had() {
        let (mut app, worker) = talk_with_words();
        let name = app.world.agents[worker].name.clone();
        // Downward: from the start of its first line; the cursor at the
        // end of its last, so `b` gives that word back.
        keys(&mut app, "vjj");
        assert_eq!(choice(&app), (4, 2, None));
        keys(&mut app, "b");
        assert_eq!(choice(&app), (4, 2, Some((0, 0))));
        keys(&mut app, "y");
        let head = format!("{name} → you [answer]");
        let copied = app.copy.clone().unwrap();
        assert!(
            copied.ends_with(&format!("{head}\nalpha **beta** gamma\ndelta")),
            "{copied}"
        );
        // Upward: from the end of its last line; the cursor at the start
        // of its first.
        keys(&mut app, "vjjjsk");
        assert_eq!(choice(&app), (3, 4, None));
        keys(&mut app, "w");
        assert_eq!(choice(&app), (3, 4, Some((1, 1))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("beta gamma\ndelta epsilon"));
        // Word-wise, j and k keep the cursor on a word of the line it goes to.
        keys(&mut app, "vjswwj");
        assert_eq!(choice(&app), (4, 3, Some((0, 1))), "two words, not three");
        keys(&mut app, "k");
        assert_eq!(choice(&app), (3, 3, Some((0, 1))));
        // `s` starts again at the cursor's word.
        keys(&mut app, "ws");
        assert_eq!(choice(&app), (3, 3, Some((2, 2))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("gamma"));
        // `v` starts whole lines again.
        keys(&mut app, "v");
        assert_eq!(choice(&app), (2, 2, None));
    }

    #[test]
    fn a_thread_s_part_is_the_words_chosen_on_a_line() {
        let (mut app, _) = with_thread();
        let th = app.thread.as_mut().unwrap();
        assert_eq!(th.part(0), None, "nothing chosen");
        th.copy = Some(Copying { anchor: 4, at: 2 });
        assert_eq!(th.part(1), None);
        assert_eq!(th.part(2), Some((0, usize::MAX)));
        assert_eq!(th.part(3), Some((0, usize::MAX)));
        assert_eq!(th.part(4), Some((0, usize::MAX)));
        assert_eq!(th.part(5), None);
        // The anchor's word at 4, the cursor's at 2: from 2.1 to 4.3.
        th.words = Some((3, 1));
        assert_eq!(th.part(1), None);
        assert_eq!(th.part(2), Some((1, usize::MAX)));
        assert_eq!(th.part(3), Some((0, usize::MAX)));
        assert_eq!(th.part(4), Some((0, 3)));
        assert_eq!(th.part(5), None);
        // On one line, either way round.
        th.copy = Some(Copying { anchor: 2, at: 2 });
        assert_eq!(th.part(2), Some((1, 3)));
        th.words = Some((1, 3));
        assert_eq!(th.part(2), Some((1, 3)));
    }

    #[test]
    fn choosing_lines_of_a_conversation_pages_and_leaves_with_esc() {
        let (mut app, worker) = with_thread();
        let long: Vec<String> = (0..20).map(|i| format!("l{i}")).collect();
        app.world.said(worker, &long.join("\n"));
        app.on_key(key(KeyCode::Char('G')));
        app.thread.as_ref().unwrap().page.set(4);
        app.on_key(key(KeyCode::Char('v')));
        let at = |app: &App| app.thread.as_ref().unwrap().copy.unwrap().at;
        assert_eq!(at(&app), 2, "head and line of the first, then this head");
        app.on_key(ctrl('d'));
        assert_eq!(at(&app), 4, "half a page of lines");
        app.on_key(ctrl('f'));
        assert_eq!(at(&app), 8);
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(at(&app), 12);
        app.on_key(ctrl('u'));
        assert_eq!(at(&app), 10);
        app.on_key(ctrl('b'));
        assert_eq!(at(&app), 6);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(at(&app), 2);
        app.on_key(key(KeyCode::End));
        assert_eq!(at(&app), 22);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), 0);
        assert_eq!(app.thread.as_ref().unwrap().sel, 0, "the first message");
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.thread.as_ref().unwrap().copy, None);
        assert!(app.thread.is_some(), "esc leaves the choosing first");
        app.on_key(key(KeyCode::Esc));
        assert!(app.thread.is_none());
    }
}
