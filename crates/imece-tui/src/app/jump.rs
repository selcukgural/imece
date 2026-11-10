//! Going somewhere: the jump line, the lines sent and recall, the places left.

use super::*;

impl App {
    /// `&`: the line that goes to an agent or a project by name, its menu
    /// the places as they stand now, what needs the person first.
    pub(super) fn open_jump(&mut self) {
        self.jumps = self.jump_list();
        self.typing = true;
        self.noting = None;
        self.input = "&".into();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// Where `&` may go, in the order its menu offers: the agents of open
    /// projects that need the person first (stuck, waiting, done and not
    /// seen), the rest by their rung (working before idle), the salon's
    /// order among equals; then the open projects. Each agent with its
    /// glyph and status words.
    fn jump_list(&self) -> Vec<Jump> {
        let w = &self.world;
        let needs = self.needs();
        let rungs = self.rungs();
        let order = w.salon_order();
        let place = |p: ProjectId| order.iter().position(|&o| o == p).unwrap_or(usize::MAX);
        let mut agents: Vec<&Agent> = w
            .agents
            .iter()
            .filter(|a| !a.gone && !w.projects[a.project].closed)
            .collect();
        agents.sort_by_key(|a| {
            (
                std::cmp::Reverse(needs[a.id]),
                std::cmp::Reverse(rungs[a.id]),
                place(a.project),
                a.id,
            )
        });
        let mut out: Vec<Jump> = agents
            .iter()
            .map(|a| {
                let glyph = ui::rung_glyph(a, rungs[a.id]).0;
                // As its row words it: done and not seen says so.
                let status: String = if self.done_unseen(a.id) {
                    self.lang.text().done_unseen.to_string()
                } else {
                    ui::status_spans(self.lang, w, a)
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                let status: String = status.chars().take(30).collect();
                Jump {
                    name: a.name.clone(),
                    note: format!("{glyph} {status}").trim_end().to_string(),
                    needs: needs[a.id] != Need::Nothing,
                }
            })
            .collect();
        let t = self.lang.text();
        out.extend(order.iter().map(|&p| Jump {
            name: w.projects[p].name.clone(),
            note: t.jump_project.to_string(),
            needs: false,
        }));
        out
    }

    /// `:goto NAME`, or what `&` was given: to the agent of that name, else
    /// the open project; a bare one opens the `&` line.
    pub(super) fn goto(&mut self, name: &str) {
        let name = name.strip_prefix('@').unwrap_or(name).trim();
        if name.is_empty() {
            return self.open_jump();
        }
        // As the menu matches: case and Turkish letters aside.
        let want = ui::fold(name);
        let agent = self
            .world
            .agents
            .iter()
            .find(|a| {
                !a.gone && !self.world.projects[a.project].closed && ui::fold(&a.name) == want
            })
            .map(|a| a.id);
        if let Some(id) = agent {
            return self.go_agent(id);
        }
        let project = self
            .world
            .open_projects()
            .find(|p| ui::fold(&p.name) == want)
            .map(|p| p.id);
        match project {
            Some(p) => {
                self.go_home();
                self.thread = None;
                self.settled_projects.remove(&p);
                self.selected = Sel::Project(p);
                // Its heading on Home, else its first row.
                let rows = self.home_rows();
                let w = &self.world;
                let mine = |r: &(HomeGroup, HomeRow)| match r.1 {
                    HomeRow::Project(q) => q == p,
                    HomeRow::Agent(id) => w.agents[id].project == p,
                    HomeRow::Item(i) => w.agents[w.inbox[i].agent].project == p,
                };
                let heading = rows.iter().find(|r| r.1 == HomeRow::Project(p));
                if let Some(row) = heading.or_else(|| rows.iter().find(|r| mine(r))) {
                    self.at_home.at = Some(row.1);
                    self.home_settle();
                }
            }
            None => self.flash(fill(self.lang.text().no_jump, &[name])),
        }
    }

    /// `↑` (`back`) and `↓` on the command line: the line sent before the
    /// one shown (the newest first), or the one after; past the newest,
    /// what was being written comes back; before the oldest it stays.
    pub(super) fn recall(&mut self, back: bool) {
        let n = self.world.sent.len();
        let shown = self.recall.as_ref().map(|(i, _)| *i);
        let next = match (shown, back) {
            (None, true) => n.checked_sub(1),
            (None, false) => None,
            (Some(i), true) => i.checked_sub(1),
            (Some(i), false) => (i + 1 < n).then_some(i + 1),
        };
        match next {
            Some(i) => {
                match &mut self.recall {
                    Some((at, _)) => *at = i,
                    None => self.recall = Some((i, self.input.clone())),
                }
                self.input = self.world.sent[i].text.clone();
            }
            None if !back => {
                let Some((_, mine)) = self.recall.take() else {
                    return;
                };
                self.input = mine;
            }
            None => return,
        }
        self.after = 0;
        self.menu = None;
        self.searching = false;
    }

    /// `^r`, `:history WORDS`: the line open with `words`, the menu listing
    /// the lines sent that have them; said when nothing was sent yet.
    pub(super) fn search_history(&mut self, words: &str) {
        if self.world.sent.is_empty() {
            return self.flash(self.lang.text().no_history.to_string());
        }
        self.typing = true;
        self.noting = None;
        self.searching = true;
        self.recalling = false;
        self.recall = None;
        self.input = words.trim().to_string();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// `H`, `:recall WORDS`: the line open with `words`, the menu listing
    /// the journal and trace lines of every project and archive that
    /// have them, the newest first (record 93). An archive's view only
    /// looks at its own.
    pub(super) fn search_all(&mut self, words: &str) {
        if matches!(self.engine, Engine::View(_)) {
            return self.read_only();
        }
        self.typing = true;
        self.noting = None;
        self.searching = true;
        self.recalling = true;
        self.recall = None;
        self.input = words.trim().to_string();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// The lines that have the terms of `words`: the store's (live), what
    /// the world holds (the demo). The store's name agents and projects
    /// by the world's ids: both are saved together and never shrink.
    fn recall_hits(&self, words: &str) -> Result<Vec<recall::Hit>, String> {
        match &self.engine {
            Engine::Live(l) => l.recall(&self.world, words, recall::SHOWN),
            Engine::Sim(_) => Ok(recall::in_world(&self.world, words, recall::SHOWN)),
            Engine::View(_) => Ok(Vec::new()),
        }
    }

    /// The `:recall` menu for what is typed: each line as the journal
    /// words it (a trace line cleaned), and who, where and when.
    pub(super) fn recall_menu(&self) -> Option<Menu> {
        let hits = self.recall_hits(&self.input).ok()?;
        let (w, t) = (&self.world, self.lang.text());
        let today = day(w.tick);
        complete::recall(
            &hits,
            |h| match &h.line {
                recall::Line::Journal { entry, .. } => ui::words::event(self.lang, w, &entry.event),
                recall::Line::Trace(text) => clean(text),
            },
            |h| {
                let (who, place) = (&w.agents[h.agent].name, &w.projects[h.project].name);
                match h.tick {
                    Some(tick) => fill(t.recall_note, &[who, place, &when(tick, today)]),
                    None => fill(t.recall_note_untimed, &[who, place]),
                }
            },
        )
    }

    /// Enter while recalling: to the line under the highlight; said when
    /// nothing has the words.
    pub(super) fn take_recall(&mut self) {
        let chosen = self.menu.as_ref().map(|m| m.chosen);
        let words = std::mem::take(&mut self.input);
        let hits = self.recall_hits(&words);
        self.typing = false;
        self.searching = false;
        self.recalling = false;
        self.menu = None;
        self.after = 0;
        let t = self.lang.text();
        match hits {
            Err(why) => self.flash(fill(t.recall_failed, &[&why])),
            Ok(hits) => match chosen.and_then(|c| hits.into_iter().nth(c)) {
                Some(hit) => self.go_recalled(hit),
                None => self.flash(fill(t.recall_none, &[words.trim()])),
            },
        }
    }

    /// To a line `:recall` found: a journal line's conversation (its
    /// line selected in the journal when it shows there), a trace line's
    /// agent's raw stream; in an archive's view when its project is put
    /// away. A line the screen does not hold (older than what was loaded,
    /// or of a closed project) is said instead.
    fn go_recalled(&mut self, hit: recall::Hit) {
        let p = hit.project;
        if self.world.projects[p].archived.is_some() {
            self.view_archive(p, Some(hit.agent));
            if let Some(v) = &mut self.viewing {
                match hit.line {
                    recall::Line::Journal { entry, .. } => {
                        if let Some(i) = v.world.journal.iter().rposition(|e| *e == entry) {
                            v.show_line(i);
                        }
                    }
                    recall::Line::Trace(_) => v.trace(hit.agent),
                }
            }
            return;
        }
        match hit.line {
            recall::Line::Journal { at: Some(i), .. } => self.show_line(i),
            recall::Line::Trace(_) if !self.world.projects[p].closed => {
                self.go_agent(hit.agent);
                self.trace(hit.agent);
            }
            _ => {
                let t = self.lang.text();
                self.flash(t.recall_away);
            }
        }
    }

    /// Journal line `i` in its conversation, selected in the salon's
    /// journal when it shows there.
    fn show_line(&mut self, i: usize) {
        self.leave_card();
        self.go_home();
        if self.journal_rows().iter().any(|&(r, _)| r == i) {
            self.pane = Pane::Journal;
            self.journal_sel = Some(i);
        }
        self.open_thread(i);
    }

    /// Enter while searching: the line under the highlight becomes the
    /// line written, whole (its own lines back), to send or go on with.
    pub(super) fn take_sent(&mut self) {
        if let Some(m) = &self.menu {
            let picks = complete::history_picks(&self.input, &self.world.sent);
            if let Some(&i) = picks.get(m.chosen) {
                self.input = self.world.sent[i].text.clone();
            }
        }
        self.searching = false;
        self.menu = None;
        self.after = 0;
    }

    /// Home with agent `id`'s row at hand: its project unfolded, and its
    /// row (or its worker's, for a helper) no longer settled away.
    pub(super) fn go_agent(&mut self, id: AgentId) {
        self.go_home();
        self.thread = None;
        let a = &self.world.agents[id];
        self.collapsed[a.project] = false;
        self.settled_projects.remove(&a.project);
        // Its own row, or the worker's row it hangs under.
        let rows: Vec<AgentId> = (self.settled.iter().copied())
            .filter(|&s| self.row_group(s).contains(&id))
            .collect();
        for s in rows {
            self.settled.remove(&s);
        }
        self.selected = Sel::Agent(id);
        // On Home: its row, else the item that stands for it.
        let rows = self.home_rows();
        let w = &self.world;
        let row = if rows.iter().any(|r| r.1 == HomeRow::Agent(id)) {
            Some(HomeRow::Agent(id))
        } else {
            (w.open_inbox().into_iter())
                .find(|&i| w.inbox[i].agent == id)
                .map(HomeRow::Item)
        };
        if let Some(row) = row {
            self.at_home.at = Some(row);
            self.home_settle();
        }
    }

    /// Where the person is now.
    pub fn place(&self) -> Place {
        Place {
            zoom: self.zoom,
            pane: self.pane,
            card: self.card,
        }
    }

    /// `^o`: back to the place the person was before, as vim's jump list
    /// goes; an open conversation closes first. A place that is not there
    /// any more (a gone agent's focus, a change applied since) is passed
    /// over, and with none left it says so.
    pub(super) fn go_back(&mut self) {
        self.went_back = true;
        if self.thread.take().is_some() {
            return;
        }
        while let Some(p) = self.been.pop() {
            let agent_ok = |id: AgentId| self.world.agents.get(id).is_some_and(|a| !a.gone);
            let zoom_ok = match p.zoom {
                Zoom::None => true,
                Zoom::Focus(id) | Zoom::Trace(id) => agent_ok(id),
                Zoom::Diff(id) => self.world.change_of(id).is_some(),
            };
            let card_ok = p.card.is_none_or(|c| c < self.world.cards.len());
            if zoom_ok && card_ok && p != self.place() {
                self.zoom = p.zoom;
                self.pane = p.pane;
                self.card = p.card;
                if self.pane == Pane::Journal {
                    self.select_journal(None);
                }
                return;
            }
        }
        self.flash(self.lang.text().no_back);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn a_line_sent_is_kept_for_recall_unless_it_only_went_somewhere() {
        assert!(recalled("@claude-1 fix the clock"));
        assert!(recalled(":budget 80"));
        assert!(recalled("!cargo test"));
        assert!(recalled("hello"));
        assert!(!recalled("#12"));
        assert!(!recalled("&3 claude-1"));
        assert!(!recalled(":goto claude-1"));
        assert!(!recalled(":history fix"));
        assert!(!recalled(":history"));
        assert!(recalled(":gotten"), "the word, not its start");
        let (mut app, w) = four_idle();
        let name = app.world.agents[w[0]].name.clone();
        let send = |app: &mut App, line: &str| {
            type_in(app, line);
            app.on_key(KeyEvent::from(KeyCode::Enter));
        };
        app.world.tick = 4;
        send(&mut app, &format!("@{name} fix the clock"));
        send(&mut app, "#1");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        send(&mut app, ":budget 80");
        send(&mut app, ":budget 80");
        app.command("goto nobody");
        app.flash = None;
        let texts = |app: &App| {
            app.world
                .sent
                .iter()
                .map(|s| s.text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            texts(&app),
            [format!("@{name} fix the clock"), ":budget 80".into()]
        );
        assert_eq!(app.world.sent[0].tick, 4);
        // The line as typed, trimmed; `!` lines too.
        send(&mut app, "  !cargo test  ");
        assert_eq!(texts(&app).last().unwrap(), "!cargo test");
        assert_eq!(app.world.sent.len(), 3);
    }

    #[test]
    fn up_and_down_bring_the_lines_sent_back_and_the_draft_after_the_newest() {
        let (mut app, _) = four_idle();
        let up = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Up));
        let down = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Down));
        // Nothing sent: up and down leave the line as it is.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "draft");
        up(&mut app);
        down(&mut app);
        assert_eq!(app.input, "draft");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        for line in ["first", "second", "third"] {
            app.world.recall_line(line);
        }
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "draft");
        up(&mut app);
        assert_eq!((app.input.as_str(), app.after), ("third", 0));
        assert!(app.menu.is_none());
        up(&mut app);
        assert_eq!(app.input, "second");
        up(&mut app);
        assert_eq!(app.input, "first");
        up(&mut app);
        assert_eq!(app.input, "first", "the oldest stays");
        down(&mut app);
        assert_eq!(app.input, "second");
        down(&mut app);
        down(&mut app);
        assert_eq!(app.input, "draft", "past the newest: the draft is back");
        assert!(app.recall.is_none());
        down(&mut app);
        assert_eq!(app.input, "draft");
        // Writing on a line brought back makes it the person's own: up
        // starts from the newest again, and down brings the written one.
        up(&mut app);
        up(&mut app);
        assert_eq!(app.input, "second");
        type_in(&mut app, "!");
        assert!(app.recall.is_none());
        up(&mut app);
        assert_eq!(app.input, "third");
        down(&mut app);
        assert_eq!(app.input, "second!");
        // Sent, it is the newest; the line is empty again.
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.world.sent.last().unwrap().text, "second!");
        // The chat's line stays open for the next message, empty.
        assert!(app.typing && app.recall.is_none() && app.input.is_empty());
        // Over several lines, up and down move between them first; at the
        // first line up brings a line back, at the last line down does.
        app.world.recall_line("a\nb");
        type_in(&mut app, "x\\");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        type_in(&mut app, "y");
        assert_eq!(app.input, "x\ny");
        up(&mut app);
        assert_eq!(app.input, "x\ny", "moved up a line, nothing brought back");
        assert_eq!(app.cursor(), 1);
        up(&mut app);
        assert_eq!(app.input, "a\nb", "at the first line: the newest sent");
        assert_eq!(app.cursor(), 3);
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.cursor(), 1, "up within the line brought back");
        down(&mut app);
        assert_eq!((app.input.as_str(), app.cursor()), ("a\nb", 3));
        down(&mut app);
        assert_eq!(app.input, "x\ny", "at the last line: the draft");
        // Esc drops it all.
        up(&mut app);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing && app.recall.is_none() && app.input.is_empty());
    }

    #[test]
    fn capital_h_recalls_every_journal_and_trace_line_and_enter_goes_there() {
        let (mut app, w) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let lead = app.world.projects[0].leader;
        let (worker, other) = (w[0], w[1]);
        app.world.tick = 20;
        app.world.journal.push(Entry {
            tick: 3,
            event: Event::Talk {
                from: worker,
                to: lead,
                text: "Deploy the site".into(),
            },
        });
        let line = app.world.journal.len() - 1;
        app.world.agents[other].trace(9, "run: deploy.sh");
        app.on_key(key(KeyCode::Char('H')));
        assert!(app.typing && app.searching && app.recalling);
        assert!(app.input.is_empty() && app.menu.is_none(), "nothing typed");
        type_in(&mut app, "DEPLO");
        let m = app.menu.clone().expect("two lines have it");
        let event = ui::words::event(Lang::En, &app.world, &app.world.journal[line].event);
        let trace = app.world.agents[other].trace.back().unwrap().clone();
        assert_eq!(m.items, [complete::one_line(&event), trace]);
        let (wn, on) = (
            &app.world.agents[worker].name,
            &app.world.agents[other].name,
        );
        let project = &app.world.projects[0].name;
        assert_eq!(
            m.notes,
            [
                format!("{wn} · {project} · {}", clock(3)),
                format!("{on} · {project}")
            ]
        );
        // Enter: the journal line's conversation, the line selected.
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && !app.searching && !app.recalling);
        assert!(app.input.is_empty() && app.menu.is_none());
        let thread = app.thread.as_ref().expect("its conversation");
        assert_eq!(thread.entries[thread.sel], line);
        assert_eq!((app.pane, app.journal_sel), (Pane::Journal, Some(line)));
        // A line `:clear` took off the journal: its conversation, nothing
        // selected in the journal that does not show it.
        app.thread = None;
        app.pane = Pane::Salon;
        app.journal_sel = None;
        app.world.journal.push(Entry {
            tick: 4,
            event: Event::Stopped {
                agent: other,
                why: "later".into(),
            },
        });
        app.world.cleared.journal = line + 1;
        app.command("recall deploy the site");
        app.on_key(key(KeyCode::Enter));
        let thread = app.thread.as_ref().expect("its conversation");
        assert_eq!(thread.entries[thread.sel], line);
        assert_eq!((app.pane, app.journal_sel), (Pane::Salon, None));
        app.world.cleared.journal = 0;
        // Tab to the trace line: its agent's raw stream.
        app.thread = None;
        app.on_key(key(KeyCode::Char('H')));
        type_in(&mut app, "deploy");
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Trace(other));
        assert_eq!(app.selected, Sel::Agent(other));
        // Of a closed project: said, not gone to.
        app.zoom = Zoom::None;
        app.world.projects[0].closed = true;
        app.command("recall deploy.sh");
        assert!(app.recalling && app.input == "deploy.sh");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(
            said(&mut app),
            "that line is not on the screen: its project is closed, or it is older than \
             what was loaded"
        );
        app.world.projects[0].closed = false;
        // Nothing has the words: said.
        app.on_key(key(KeyCode::Char('H')));
        type_in(&mut app, "zebra ");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said(&mut app), "no journal or trace line has zebra");
        assert!(!app.typing && !app.recalling);
        // ^r is the lines sent again; esc leaves.
        app.world.recall_line("deploy it");
        app.on_key(key(KeyCode::Char('H')));
        app.on_key(KeyCode::Esc.into());
        assert!(!app.typing && !app.searching);
        app.on_key(ctrl('r'));
        assert!(app.searching && !app.recalling);
        // `:recall` is no line to bring back.
        assert!(!recalled(":recall deploy") && recalled(":budget 80"));
    }

    #[test]
    fn a_line_recalled_of_an_archive_opens_in_its_view() {
        let mut app = App::new(2, 2, 0, 7, Lang::En);
        let worker = (app.world.agents.iter())
            .find(|a| a.project == 1 && a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        let lead = app.world.projects[1].leader;
        app.world.journal.push(Entry {
            tick: 1,
            event: Event::Talk {
                from: worker,
                to: lead,
                text: "the zebra crossing".into(),
            },
        });
        app.world.agents[worker].trace(1, "zebra stripes");
        app.selected = Sel::Agent(lead);
        app.command("archive epic");
        app.end_grace();
        assert!(app.world.projects[1].archived.is_some());
        app.command("recall zebra crossing");
        app.on_key(key(KeyCode::Enter));
        let v = view(&app);
        let thread = v.thread.as_ref().expect("its conversation, in the view");
        let at = thread.entries[thread.sel];
        assert!(
            matches!(&v.world.journal[at].event, Event::Talk { text, .. } if text == "the zebra crossing")
        );
        assert_eq!(app.thread.as_ref().map(|t| t.sel), None, "not behind it");
        // In the view, recall is the screen behind's.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('H')));
        assert!(!view(&app).typing);
        assert_eq!(
            view(&app).flash.as_ref().map(|f| f.0.as_str()),
            Some(Lang::En.text().read_only)
        );
        app.viewing = None;
        // A trace line of the archive: its agent's stream, in the view.
        app.command("recall zebra stripes");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(view(&app).zoom, Zoom::Trace(worker));
    }

    #[test]
    fn ctrl_r_searches_the_lines_sent_and_enter_takes_one_to_the_line() {
        let (mut app, _) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // Nothing sent yet: said, the line stays closed.
        app.on_key(ctrl('r'));
        assert!(!app.typing && !app.searching);
        assert_eq!(said(&mut app), "nothing sent yet to bring back");
        app.world.tick = 0;
        app.world.recall_line("@claude-1 fix the clock");
        app.world.recall_line(":budget 80");
        app.world.tick = 24;
        app.world.recall_line("@claude-1 İşler\nnasıl?");
        app.world.tick = 36;
        // From the salon: the whole list, newest first, each with its age.
        app.on_key(ctrl('r'));
        assert!(app.typing && app.searching && app.input.is_empty());
        let m = app.menu.clone().expect("the menu");
        assert_eq!(
            m.items,
            [
                "@claude-1 İşler ⏎ nasıl?",
                ":budget 80",
                "@claude-1 fix the clock"
            ]
        );
        assert_eq!(m.notes, ["1 min", "3 min", "3 min"]);
        assert_eq!(m.start, 0);
        // Typing filters; tab and the arrows step; enter takes the line
        // whole, its own lines back, and the search ends.
        type_in(&mut app, "İŞ");
        assert_eq!(app.menu.as_ref().unwrap().items.len(), 1);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "@claude-1 İşler\nnasıl?");
        assert!(app.typing && !app.searching && app.menu.is_none());
        assert_eq!(app.after, 0);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(ctrl('r'));
        app.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.menu.as_ref().unwrap().chosen, 1);
        assert_eq!(app.input, "", "tab steps, it does not insert");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, ":budget 80");
        // Enter again sends it, as any line.
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.typing);
        assert_eq!(app.world.sent.last().unwrap().text, ":budget 80");
        // While writing, ^r takes the words written as the search.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "fix clock");
        app.on_key(ctrl('r'));
        assert!(app.searching && app.input == "fix clock");
        assert_eq!(
            app.menu.as_ref().unwrap().items,
            ["@claude-1 fix the clock"]
        );
        // Nothing has the words: no menu; enter keeps the words, ends the
        // search, sends nothing.
        type_in(&mut app, " zzz");
        assert!(app.menu.is_none() && app.searching);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.typing && !app.searching);
        assert_eq!(app.input, "fix clock zzz");
        assert_eq!(app.world.sent.len(), 4);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // Esc closes the menu first, the search goes on with the next key;
        // esc again closes the line.
        app.on_key(ctrl('r'));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.menu.is_none() && app.searching && app.typing);
        type_in(&mut app, "bud");
        assert_eq!(app.menu.as_ref().unwrap().items, [":budget 80"]);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing && !app.searching);
        // Up while searching with nothing matched leaves the search for
        // the newest line.
        app.on_key(ctrl('r'));
        type_in(&mut app, "zzz");
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert!(!app.searching);
        assert_eq!(app.input, ":budget 80");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `:history WORDS` is the same by name, the words trimmed.
        app.command("history  fix ");
        assert!(app.typing && app.searching && app.input == "fix");
        assert_eq!(app.menu.as_ref().unwrap().items.len(), 1);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.world.sent.clear();
        app.command("history");
        assert_eq!(said(&mut app), "nothing sent yet to bring back");
        assert!(!app.typing);
        assert!(complete::COMMANDS.contains(&"history"));
    }

    #[test]
    fn the_jump_list_keeps_the_salons_project_order_among_equals() {
        let mut app = App::new(2, 1, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.unseen.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.talk = None;
            a.starting = false;
        }
        let order = app.world.salon_order();
        assert_eq!(order.len(), 2);
        // The second project pinned: its agents come first, then its row.
        app.world.projects[1].pinned = true;
        let order = app.world.salon_order();
        assert_eq!(order, [1, 0]);
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let projects: Vec<ProjectId> = app
            .jumps
            .iter()
            .filter_map(|j| app.world.agent_by_name(&j.name))
            .map(|id| app.world.agents[id].project)
            .collect();
        assert_eq!(projects.len(), app.world.agents.len());
        let first = projects.iter().filter(|&&p| p == 1).count();
        assert!(
            first > 0 && projects[..first].iter().all(|&p| p == 1),
            "{projects:?}"
        );
        assert!(projects[first..].iter().all(|&p| p == 0), "{projects:?}");
        let names: Vec<&str> = app.jumps.iter().map(|j| j.name.as_str()).collect();
        let n = names.len();
        assert_eq!(names[n - 2], app.world.projects[1].name);
        assert_eq!(names[n - 1], app.world.projects[0].name);
    }

    #[test]
    fn the_jump_line_takes_a_name_a_number_a_project_or_says_what_it_did_not_find() {
        let (mut app, w) = four_idle();
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let name = app.world.agents[w[3]].name.clone();
        // `&3 name` as the menu inserts it: the name is what counts.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let third = app.jumps[2].name.clone();
        type_in(&mut app, &format!("3 {third}"));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            app.selected,
            Sel::Agent(app.world.agent_by_name(&third).unwrap())
        );
        // A name typed in any case.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, &name.to_uppercase());
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected, Sel::Agent(w[3]));
        // Enter on the highlighted item takes it at once.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let second = app.world.agents[w[1]].name.clone();
        type_in(&mut app, &second);
        app.on_key(KeyEvent::from(KeyCode::Down));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected, Sel::Agent(w[1]));
        assert!(!app.typing);
        // A project: its header.
        let project = app.world.projects[0].name.clone();
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, &project);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        // One project in view: no heading on Home, its first row instead.
        assert_eq!(app.pane, Pane::Salon);
        assert!(
            matches!(app.at_home.at, Some(HomeRow::Agent(id)) if app.world.agents[id].project == 0),
            "{:?}",
            app.at_home.at
        );
        // Nothing of the name: said, nothing moves.
        let before = (app.selected, app.at_home.at);
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, "zzz");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            said(&mut app),
            "nothing called \"zzz\" to go to: an agent or a project"
        );
        assert_eq!((app.selected, app.at_home.at), before);
        // A gone agent is nobody.
        app.world.agents[w[2]].gone = true;
        app.command(&format!("goto {}", app.world.agents[w[2]].name.clone()));
        assert!(said(&mut app).starts_with("nothing called"));
        // `&#` turns to the cards.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        app.on_key(KeyEvent::from(KeyCode::Char('#')));
        assert_eq!(app.input, "#");
        assert!(app.jumps.is_empty());
        assert!(
            app.menu
                .as_ref()
                .is_some_and(|m| m.items[0].starts_with('#'))
        );
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing);
        // `#` anywhere else is a letter: a message keeps it.
        type_in(&mut app, "@");
        type_in(&mut app, &format!("{name} see #"));
        assert_eq!(app.input, format!("@{name} see #"));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `:goto` with a name goes, with `@` too; bare, it opens the line.
        app.command(&format!("goto @{name}"));
        assert_eq!(app.selected, Sel::Agent(w[3]));
        app.command("goto");
        assert!(app.typing && app.input == "&" && !app.jumps.is_empty());
        assert_eq!(t.typing_jump, " › go to: ");
        assert!(complete::COMMANDS.contains(&"goto"));
    }

    #[test]
    fn ctrl_o_goes_back_through_the_places_left_and_not_further() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        let salon = app.place();
        app.on_key(key(KeyCode::Enter));
        let focus = app.place();
        let Zoom::Focus(id) = focus.zoom else {
            panic!("Home's row opened: {:?}", focus.zoom)
        };
        app.on_key(key(KeyCode::Char('t')));
        assert_eq!(app.zoom, Zoom::Trace(id));
        assert_eq!(app.been, [salon, focus]);
        // A key that goes nowhere leaves nothing.
        app.on_key(key(KeyCode::Char('2')));
        assert_eq!(app.been.len(), 2);
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), focus);
        assert_eq!(app.been, [salon], "going back leaves nothing behind");
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), salon);
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), salon);
        assert_eq!(flashed(&app).as_deref(), Some(t.no_back));
        // The journal panel comes back with a row selected.
        app.pane = Pane::Journal;
        app.journal_sel = None;
        app.been.push(app.place());
        app.pane = Pane::Salon;
        app.on_key(ctrl('o'));
        assert_eq!(app.pane, Pane::Journal);
        assert!(app.journal_sel.is_some());
    }

    #[test]
    fn ctrl_o_closes_a_conversation_first_and_passes_over_places_gone() {
        let (mut app, worker) = with_change("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n");
        app.zoom = Zoom::None;
        app.pane = Pane::Salon;
        let here = app.place();
        app.open_thread(0);
        app.on_key(ctrl('o'));
        assert!(app.thread.is_none());
        assert_eq!(app.place(), here);
        let at = |zoom, card| Place {
            zoom,
            pane: Pane::Inbox,
            card,
        };
        let lead = app.world.projects[0].leader;
        let cards = app.world.cards.len();
        app.been = vec![
            at(Zoom::None, Some(0)),
            at(Zoom::Focus(lead), None),
            at(Zoom::Diff(worker), None),
            here,
            at(Zoom::None, Some(cards)),
            at(Zoom::Diff(lead), None),
            at(Zoom::Trace(worker), None),
        ];
        app.world.agents[worker].gone = true;
        // The gone worker's stream, a change that is not there, a card
        // that is not, where it is already: passed over.
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::Diff(worker), None));
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::Focus(lead), None));
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::None, Some(0)), "a card that is there");
        assert!(app.been.is_empty());
    }

    #[test]
    fn the_places_left_are_kept_up_to_fifty_the_oldest_going_first() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for n in 0..60 {
            let c = [KeyCode::Enter, KeyCode::Esc][n % 2];
            app.on_key(key(c));
        }
        assert_eq!(app.been.len(), 50);
        // The last place left was the focus, the one before Home.
        assert!(matches!(app.been[49].zoom, Zoom::Focus(_)));
        assert_eq!(app.been[48].zoom, Zoom::None);
        assert_eq!(app.been[0].zoom, Zoom::None);
    }
}
