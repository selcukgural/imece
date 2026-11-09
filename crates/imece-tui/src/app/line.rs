//! The command line: typing, its menus, and what a line sent does.

use super::*;

impl App {
    /// A key while writing. The menu of completions works as a shell's:
    /// it shows what matches as the text changes, `tab` puts the first
    /// match in and then steps on, `⇧tab` steps back, the arrows (and
    /// ctrl-n / ctrl-p) step too, `enter` takes the highlighted match
    /// when the word is not in yet, else sends, and `esc` hides the menu
    /// before it cancels the writing. The rest are readline's editing
    /// keys (`edit::key`); the menu is offered only with the cursor at
    /// the end, since a completion replaces the word the line ends with.
    /// Shift-enter, alt-enter and enter after a `\` start a new line;
    /// `↑` `↓` go between lines while no menu is shown.
    pub(super) fn type_key(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let newline =
            k.modifiers.contains(KeyModifiers::SHIFT) || k.modifiers.contains(KeyModifiers::ALT);
        // `^x ^e`, as bash and zsh have it: the line in the editor. Any
        // other key after `^x` does what it does alone.
        let after_x = std::mem::take(&mut self.ctrl_x);
        match k.code {
            KeyCode::Char('e') if ctrl && after_x => self.open_editor(self.input.clone()),
            KeyCode::Char('x') if ctrl => self.ctrl_x = true,
            KeyCode::Char('n') if ctrl => self.pick(true),
            KeyCode::Char('p') if ctrl => self.pick(false),
            KeyCode::Char('r') if ctrl => {
                let words = self.input.clone();
                self.search_history(&words);
            }
            KeyCode::Enter if newline => {
                self.edit(k);
            }
            // Searching: enter takes the line under the highlight (the words
            // typed stay when nothing has them), tab steps on.
            KeyCode::Enter if self.searching && self.recalling => self.take_recall(),
            KeyCode::Enter if self.searching => self.take_sent(),
            KeyCode::Tab if self.searching => {
                if let Some(m) = &mut self.menu {
                    m.step(true);
                }
            }
            // At the text's first or last line, up and down bring back the
            // lines sent, as a shell does; between lines they move.
            KeyCode::Up | KeyCode::Down if self.menu.is_none() => {
                let (back, at) = (k.code == KeyCode::Up, self.cursor());
                let at_end = if back {
                    edit::line_start(&self.input, at) == 0
                } else {
                    edit::line_end(&self.input, at) == self.input.len()
                };
                if at_end {
                    self.recall(back);
                } else {
                    self.edit(k);
                }
            }
            // A command's name typed whole is a command already, as a
            // short name is: `:news` runs at once.
            KeyCode::Enter if self.whole_command() => self.submit(),
            KeyCode::Enter => match &mut self.menu {
                Some(m) if !m.inserted => {
                    m.insert(&mut self.input);
                    // A card or a place to go is all there is to take: it
                    // opens at once.
                    if self.typing_card() || self.typing_jump() {
                        self.submit();
                    }
                }
                // Enter after a `\` is a new line, not a send.
                _ => {
                    if !self.edit(k) {
                        self.submit();
                    }
                }
            },
            KeyCode::Esc if self.menu.is_some() => self.menu = None,
            KeyCode::Esc => {
                if self.rejecting.take().is_some() {
                    self.flash(self.lang.text().not_rejected);
                }
                self.typing = false;
                self.noting = None;
                self.input.clear();
                self.after = 0;
                self.jumps.clear();
                self.recall = None;
                self.searching = false;
            }
            // Right after `&`: a digit goes to the nth at once, `#` turns
            // to the cards.
            KeyCode::Char(d) if ('1'..='9').contains(&d) && self.input == "&" => {
                self.input.push(d);
                self.submit();
            }
            KeyCode::Char('#') if self.input == "&" => {
                self.input = "#".into();
                self.jumps.clear();
                self.menu = self.suggest();
            }
            KeyCode::Tab => match &mut self.menu {
                None if self.after > 0 => {}
                None => {
                    self.menu = self.suggest_with(true);
                    if let Some(m) = &mut self.menu {
                        m.insert(&mut self.input);
                    }
                }
                Some(m) if !m.inserted => m.insert(&mut self.input),
                Some(_) => self.pick(true),
            },
            KeyCode::BackTab | KeyCode::Up => self.pick(false),
            KeyCode::Down => self.pick(true),
            _ => {
                self.edit(k);
            }
        }
    }

    /// Where the cursor is in `input`, as a byte offset on a character's
    /// edge (the next edge on, should `after` be stale).
    pub fn cursor(&self) -> usize {
        let end = self.input.len();
        let at = end.saturating_sub(self.after);
        (at..end)
            .find(|&i| self.input.is_char_boundary(i))
            .unwrap_or(end)
    }

    /// An editing key: the line or the cursor changes, and with them
    /// what the menu offers. False for a key that is not an editing key.
    fn edit(&mut self, k: KeyEvent) -> bool {
        let (was, at) = (self.input.clone(), self.cursor());
        let Some(to) = edit::key(k, &mut self.input, at, &mut self.cut) else {
            return false;
        };
        self.moved(&was, at, to);
        true
    }

    /// The line was `was` with the cursor at `at`; now the cursor is at
    /// `to`. A change or a move asks the menu again, at the end only.
    fn moved(&mut self, was: &str, at: usize, to: usize) {
        self.after = self.input.len() - to;
        if self.input != was {
            // Written on: the line is the person's own again.
            self.recall = None;
        }
        if to != at || self.input != was {
            self.menu = if self.after == 0 {
                self.suggest()
            } else {
                None
            };
        }
    }

    /// Text the terminal says was pasted (bracketed paste): it goes into
    /// the line at the cursor whole, its new lines kept, so a paste's
    /// line ends do not send it a line at a time. Only while writing;
    /// elsewhere its characters would be taken as keys, so it is left
    /// out and the person told how to write first.
    pub fn paste(&mut self, text: &str) {
        if self.too_small() {
            return;
        }
        if let Some(v) = &mut self.viewing {
            v.paste(text);
            return self.after_view();
        }
        self.paste_in(text);
        self.preview_theme();
    }

    fn paste_in(&mut self, text: &str) {
        // Into the help's filter while it is typed, on one line.
        if self.help && self.help_typing {
            let text = pasted(text).replace(['\n', '\t'], " ");
            return self.help_find.push_str(&text);
        }
        if let Some(b) = &mut self.archives {
            if !b.paste(&self.world, &pasted(text)) {
                self.flash(self.lang.text().paste_not_writing);
            }
            return;
        }
        let open = self.help || self.solo_pick.is_some() || self.window_open() || self.leaving;
        if !self.typing || open {
            return self.flash(self.lang.text().paste_not_writing);
        }
        let text = pasted(text);
        let (was, at) = (self.input.clone(), self.cursor());
        self.input.insert_str(at, &text);
        self.moved(&was, at, at + text.len());
    }

    /// Highlights the next (or previous) match and puts it in.
    fn pick(&mut self, forward: bool) {
        if let Some(m) = &mut self.menu {
            m.step(forward);
            m.insert(&mut self.input);
        }
    }

    /// What the composer could complete the input with, now. Nothing once
    /// the text runs over lines: a name or a command is on one.
    pub(super) fn suggest(&self) -> Option<Menu> {
        self.suggest_with(false)
    }

    /// [`suggest`](Self::suggest); `forced` when `tab` asked, which
    /// completes any word of a message to a file.
    fn suggest_with(&self, forced: bool) -> Option<Menu> {
        if self.input.contains('\n') || self.noting.is_some() {
            return None;
        }
        if self.searching && self.recalling {
            return self.recall_menu();
        }
        if self.searching {
            let w = &self.world;
            let lang = self.lang;
            return complete::history(&self.input, &w.sent, |s| {
                ui::words::ago(lang, w.tick.saturating_sub(s.tick) * SECONDS_PER_TICK)
            });
        }
        if let Some(rest) = self.input.strip_prefix('&') {
            return complete::jumps(rest, &self.jumps);
        }
        if let Some(m) = self.choice_menu() {
            return Some(m);
        }
        let Some(mut m) = complete::suggest(
            &self.input,
            &self.world,
            self.current_project(),
            self.home.as_deref(),
        ) else {
            return self.file_menu(forced);
        };
        // The command names: the demo's own left out live, each with its
        // value.
        if self.typing_command() && !self.input.contains(' ') {
            if self.live() {
                m.items
                    .retain(|i| !complete::DEMO_ONLY.contains(&i.as_str()));
                if m.items.is_empty() {
                    return None;
                }
            }
            m.notes = m.items.iter().map(|i| self.command_note(i)).collect();
        }
        Some(m)
    }

    /// What the `:` menu shows after command `name`: what it stands at
    /// now or would act on, in brackets, and `$` when using it raises the
    /// next bill (`:open` a new leader session, `:forget` a new session
    /// that pays the whole briefing again, without the cache).
    pub(super) fn command_note(&self, name: &str) -> String {
        let t = self.lang.text();
        let on_off = |on: bool| if on { t.value_on } else { t.value_off }.to_string();
        let value = match name {
            // The pot, its rate, how long it lasts at it, and the usage
            // window nearest to full: what the header said before record
            // 125 and says now only when pressing.
            "budget" => {
                let w = &self.world;
                let mut said = fill(t.budget, &[&w.spent_words(), &dollars(w.budget)]);
                said += " · ";
                said += &fill(t.per_hour, &[&w.money(w.rate_per_hour(), false)]);
                if let Some(minutes) = w.pot_lasts() {
                    said += ", ";
                    said += &fill(t.pot_lasts, &[&ui::words::ago(self.lang, minutes * 60)]);
                }
                if let Some(l) = w.tightest() {
                    said += " · ";
                    said += &ui::words::limit(self.lang, &l, w.now());
                }
                Some(said)
            }
            "edit" => Some(editor::of_env()),
            "theme" => Some(theme::pal().name.clone()),
            "lang" => Some(t.language.to_string()),
            "close" | "rename" => Some(self.world.projects[self.current_project()].name.clone()),
            // The project at hand, marked as the salon marks it when
            // pinned (then `:pin` unpins it).
            "pin" => {
                let p = &self.world.projects[self.current_project()];
                let mark = if p.pinned { "▴ " } else { "" };
                Some(format!("{mark}{}", p.name))
            }
            "forget" | "fork" | "talk" | "trace" | "peek" | "why" | "yes" => self
                .agent_at_hand()
                .map(|id| self.world.agents[id].name.clone()),
            "next" | "waiting" => {
                let open = self.world.open_decisions().to_string();
                Some(fill(t.value_waiting, &[&open]))
            }
            "story" => Some(on_off(self.story)),
            "review" => Some(on_off(self.review)),
            // What the yes to the change at hand asks Lonca for (record 127).
            "checks" => (self.change_at_hand())
                .and_then(|id| self.world.change_card(id))
                .and_then(|c| self.world.cards.get(c))
                .map(|c| ui::words::checks_list(t, &c.checks)),
            "plan" => Some(on_off(self.world.projects[self.current_project()].plan.on)),
            "lonca" => (self.world.projects[self.current_project()].lonca.as_ref())
                .map(|on| on.repo.clone()),
            "sync" => match self.world.projects[self.current_project()].lonca.as_ref() {
                Some(OnLonca {
                    seen: StoreSeen::Up { behind, .. },
                    ..
                }) => ui::words::behind(*behind),
                _ => None,
            },
            "news" => Some(welcome::VERSION.to_string()),
            "settle" => Some(match self.selected {
                Sel::Agent(id) => self.world.agents[id].name.clone(),
                Sel::Project(p) => self.world.projects[p].name.clone(),
            }),
            "unsettle" => Some((self.settled.len() + self.settled_projects.len()).to_string()),
            "awake" => Some(on_off(self.awake.on)),
            "notify" => Some(self.notify.name().to_string()),
            "motion" => Some(self.motion.name().to_string()),
            "away" => Some(self.away.name()),
            "health" => Some(self.world.health.name()),
            "keys" => Some(self.keys.len().to_string()),
            "pause" => Some(on_off(self.paused)),
            "faster" | "slower" => Some(format!("×{}", self.speed)),
            "export" if self.thread.is_some() => Some(t.value_thread.to_string()),
            "export" => self.card.map(|c| format!("#{}", c + 1)),
            // The command whose output `:output` quotes.
            "output" => (self.outputs.get(&self.current_project())).map(|o| o.command.clone()),
            "copy" => {
                let copies = &self.world.projects[self.current_project()].copies;
                (!copies.is_empty()).then(|| copies.join(", "))
            }
            "allow" => {
                let rules = &self.world.projects[self.current_project()].allowed;
                let lines: Vec<String> = rules.iter().map(Standing::line).collect();
                (!lines.is_empty()).then(|| lines.join(", "))
            }
            // What the solo at hand runs on, the part the command sets.
            "effort" | "model" => self.solo_at_hand().map(|id| {
                let c = Choice::parse(self.model_of(id).as_deref().unwrap_or_default());
                let part = if name == "effort" { c.effort } else { c.model };
                part.unwrap_or_else(|| complete::DEFAULT.to_string())
            }),
            _ => None,
        };
        let bill = matches!(
            name,
            "forget" | "fork" | "open" | "solo" | "commit" | "review"
        );
        match (value, bill) {
            (Some(v), true) => format!("({v}) $"),
            (Some(v), false) => format!("({v})"),
            (None, true) => "$".to_string(),
            (None, false) => String::new(),
        }
    }

    /// The last word of a message or a `!` line, completed to a file of
    /// the project at hand, or a symbol leased on its code map
    /// (`file::name`), the holder of the file as the note (see
    /// [`complete::files`]). Not on the lines that are something else
    /// (`:` `#` `&`, a search), nor for a name (`@`) or a command (`/`).
    fn file_menu(&self, forced: bool) -> Option<Menu> {
        if self.input.starts_with([':', '#', '&']) {
            return None;
        }
        let word = self
            .input
            .rsplit(char::is_whitespace)
            .next()
            .unwrap_or_default();
        let word = if word.len() == self.input.len() {
            word.strip_prefix('!').unwrap_or(word)
        } else {
            word
        };
        if word.is_empty() || word.starts_with(['@', '/']) {
            return None;
        }
        let start = self.input.len() - word.len();
        let w = &self.world;
        let p = self.current_project();
        let paths = w.files_of(p);
        let held: Vec<(&str, &str, AgentId)> = w
            .cards
            .iter()
            .filter(|c| c.project == p && c.holds_leases())
            .filter_map(|c| c.owner.map(|o| (c, o)))
            .flat_map(|(c, o)| c.symbols.iter().map(move |s| (s.file, s.name, o)))
            .collect();
        let note = |file: &str| -> String {
            held.iter()
                .find(|h| h.0 == file)
                .map(|h| {
                    let a = &w.agents[h.2];
                    format!("{} {}", ui::glyph(a).0, a.name)
                })
                .unwrap_or_default()
        };
        let symbols: Vec<(&str, &str)> = held.iter().map(|h| (h.0, h.1)).collect();
        complete::files(start, word, forced, &paths, &symbols, note)
    }

    /// Where the terminal's own cursor stands and what it looks like, while
    /// the person types: the window on top takes it (the help's or the
    /// archives' `/` filter), a window on top that takes no text hides it,
    /// else the command line has it while it is open. `None`: hidden.
    pub fn caret(&self) -> Option<(Field, Caret)> {
        if let Some(v) = &self.viewing {
            return v.caret();
        }
        if self.solo_pick.is_some() {
            return None;
        }
        if let Some(b) = &self.archives {
            return b.typing.then_some((Field::Archives, Caret::Find));
        }
        if self.help {
            return self.help_typing.then_some((Field::Help, Caret::Find));
        }
        let window = !self.news.is_empty()
            || self.welcome
            || self.digest.is_some()
            || self.left_off.is_some()
            || self.leaving;
        if window || !self.typing {
            return None;
        }
        // In the order the line's heading names what is written
        // (`ui::writing`).
        let caret = if self.noting.is_some() {
            Caret::Write
        } else if self.searching {
            Caret::Find
        } else if self.typing_shell() {
            Caret::Write
        } else if self.typing_card() || self.typing_jump() {
            Caret::Find
        } else if self.typing_command() {
            Caret::Command
        } else {
            Caret::Write
        };
        Some((Field::Line, caret))
    }

    /// Whether what is being typed is a command to imece (`:budget 80`),
    /// not a message to an agent.
    pub fn typing_command(&self) -> bool {
        self.input.starts_with(':')
    }

    /// Whether the line is `:` and a command's whole name with the menu's
    /// highlight on that name, not yet put in: `enter` runs it rather
    /// than taking the menu's entry (`:news`, `:commit`). A word only
    /// begun (`:ne`), or a highlight moved to another name, takes the
    /// entry as before.
    fn whole_command(&self) -> bool {
        let Some(m) = self.menu.as_ref().filter(|m| !m.inserted) else {
            return false;
        };
        let Some(word) = self.input.strip_prefix(':') else {
            return false;
        };
        m.start == 1 && m.items.get(m.chosen).is_some_and(|i| i == word)
    }

    /// Whether what is being typed names a card to open (`#323`), not a
    /// message to an agent.
    pub fn typing_card(&self) -> bool {
        self.input.starts_with('#')
    }

    /// Whether what is being typed names a place to go (`&claude-1`, `&3`).
    pub fn typing_jump(&self) -> bool {
        self.input.starts_with('&')
    }

    /// Whether what is being typed is a command for the person's own shell
    /// (`!cargo test`), not a message to an agent.
    pub fn typing_shell(&self) -> bool {
        self.input.starts_with('!')
    }

    /// Who a message being typed goes to: the named agent, or the leader of
    /// the current project.
    pub fn typing_target(&self) -> String {
        match self.input.strip_prefix('@') {
            Some(rest) => rest
                .split(char::is_whitespace)
                .next()
                .unwrap_or_default()
                .to_string(),
            None => {
                let leader = self.world.projects[self.current_project()].leader;
                self.world.agents[leader].name.clone()
            }
        }
    }

    pub(super) fn submit(&mut self) {
        self.end_preview();
        let mut text = self.input.trim().to_string();
        let mut target = self.typing_target();
        self.input.clear();
        self.after = 0;
        self.typing = false;
        self.menu = None;
        self.recall = None;
        self.searching = false;
        match self.noting.take() {
            Some(Noting::Line(id, line)) => return self.keep_note(id, line, text),
            Some(Noting::Words(note)) => return self.keep_talk_note(note, text),
            None => {}
        }
        if let Some(id) = self.rejecting.take() {
            match self.reason_on(id, &text).map(str::to_string) {
                Some(reason) => {
                    if !self.reject(id, &reason) {
                        return;
                    }
                    // The words go to the worker whose change it was,
                    // whatever became of the name put on the line.
                    target = self.world.agents[id].name.clone();
                    text = format!("@{target} {reason}");
                }
                None => {
                    self.flash(self.lang.text().not_rejected);
                    // A name alone: that is all there is to say.
                    if text.split_whitespace().nth(1).is_none() {
                        return;
                    }
                }
            }
        }
        if text.is_empty() {
            return;
        }
        if recalled(&text) {
            self.world.recall_line(&text);
        }
        let t = self.lang.text();
        if let Some(line) = text.strip_prefix(':') {
            return self.command(line.trim());
        }
        if text.starts_with('#') {
            return match complete::card_named(&text, &self.world) {
                Some(c) => self.open_card(c),
                None => self.flash(fill(t.no_card_found, &[&text])),
            };
        }
        if let Some(rest) = text.strip_prefix('&') {
            let list = std::mem::take(&mut self.jumps);
            return match complete::jump_named(&text, &list) {
                Some(name) => self.goto(&name),
                // A number past the list's end is nowhere; a bare `&` is
                // nothing to do.
                None => {
                    let rest = rest.trim_start_matches('!').trim();
                    if !rest.is_empty() {
                        self.flash(fill(self.lang.text().no_jump, &[rest]));
                    }
                }
            };
        }
        if let Some(command) = text.strip_prefix('!') {
            let command = command.trim();
            if command.is_empty() {
                return self.flash(t.shell_empty);
            }
            if self.viewing_archive() {
                return self.read_only();
            }
            self.shell = Some(command.to_string());
            return;
        }
        let Some(rest) = text.strip_prefix('@') else {
            let leader = self.world.projects[self.current_project()].leader;
            self.tell(leader, &text);
            return;
        };
        let msg = rest
            .split_once(char::is_whitespace)
            .map_or("", |(_, m)| m)
            .trim();
        match self.world.agent_by_name(&target) {
            Some(_) if msg.is_empty() => self.flash(t.empty_message),
            Some(id) => {
                self.tell(id, msg);
            }
            None => self.flash(fill(t.no_agent, &[&target])),
        }
    }

    /// The person's message to agent `id`: the engine gets it first, and only
    /// a message that reached the agent is recorded as said. A line that
    /// starts with `/` is a command of the agent's own command line
    /// (`/clear`, `/compact`, `/cost`), not a message. Whether it went.
    pub(super) fn tell(&mut self, id: AgentId, text: &str) -> bool {
        let command = text.starts_with('/');
        let refused = match &mut self.engine {
            Engine::Sim(s) if command => s.command(&mut self.world, id, text).err(),
            Engine::Sim(s) => s.told(&mut self.world, id, text).err(),
            Engine::Live(l) if command => l.command(&mut self.world, id, text).err(),
            Engine::Live(l) => l.told(&mut self.world, id, text).err(),
            Engine::View(_) => {
                self.read_only();
                return false;
            }
        };
        let went = refused.is_none();
        self.report(id, text, refused);
        went
    }

    /// Flashes how the person's `text` to agent `id` fared.
    fn report(&mut self, id: AgentId, text: &str, refused: Option<Refusal>) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        match refused {
            None => {
                self.world.said(id, text);
                self.flash(fill(t.sent, &[&name]));
            }
            Some(Refusal::Budget) => self.flash(ui::words::pot_spent(self.lang, &self.world)),
            Some(Refusal::Start(why)) => self.flash(fill(t.cannot_start, &[&name, &why])),
            Some(Refusal::Command(why)) => self.flash(fill(t.command_refused, &[&name, &why])),
        }
    }

    /// A command to imece itself, typed after `:`. `budget USD` (or `b`)
    /// sets what all agents together may spend; a running turn finishes
    /// on its old cap, the next one starts on the new remainder. `theme
    /// NAME` (or `t`) switches to a built-in theme or a colour file.
    ///
    /// Every key that acts has a name here too (`:salon`, `:lang`,
    /// `:quit` ...; `complete::COMMANDS`), doing what the key does from
    /// wherever the person is.
    pub(crate) fn command(&mut self, line: &str) {
        let t = self.lang.text();
        let (name, arg) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(n, a)| (n, a.trim()));
        let long = complete::long(name);
        if self.viewing_archive() && complete::COMMANDS.contains(&long) {
            if long == "restore" {
                self.restore_asked = true;
                return;
            }
            if !VIEW_COMMANDS.contains(&long) {
                return self.read_only();
            }
        }
        match long {
            "budget" => match parse_dollars(arg) {
                Some(to) => {
                    let from = self.world.budget;
                    self.world.budget = to;
                    // A new pot is worth a new warning.
                    self.world.spending.warned = false;
                    let leader = self.world.projects[self.current_project()].leader;
                    self.world.note(Event::Budget { leader, from, to });
                    self.flash(fill(t.budget_is, &[&dollars(to)]));
                }
                None => self.flash(fill(t.bad_budget, &[arg])),
            },
            "theme" if arg.is_empty() => self.next_theme(),
            "theme" => match theme::choose(arg) {
                Ok(theme) => self.flash(fill(t.theme_is, &[&theme.name])),
                Err(why) => self.flash(fill(t.bad_theme, &[arg, &why])),
            },
            "open" => self.open_project(arg),
            "solo" => self.solo_project(arg),
            "effort" => self.rechoose(Part::Effort, arg),
            "model" => self.rechoose(Part::Model, arg),
            "close" => self.close_project(arg),
            "archive" => self.archive_project(arg),
            "restore" => self.restore_project(arg),
            "archives" => self.open_archives(),
            "undo" => self.undo(),
            "now" => self.grace_now(),
            "rename" => self.rename_project(arg),
            "forget" => self.forget_agent(arg),
            "fork" => self.fork_agent(arg),
            "goto" => self.goto(arg),
            "history" => self.search_history(arg),
            "recall" => self.search_all(arg),
            "export" => self.export(arg),
            "commit" => self.suggest_commit(),
            "review" => self.review_command(arg),
            "checks" => self.checks_command(arg),
            "plan" => self.plan_command(arg),
            "lonca" => self.lonca_command(arg.trim()),
            "sync" => self.sync_command(),
            "copy" => self.copy_path(arg),
            "allow" => self.allow(arg),
            "pin" => self.pin(arg),
            "mark" => self.toggle_mark(),
            "output" => self.quote_output(),
            "edit" => self.open_editor(arg.to_string()),
            // The team is Home's lower half (record 125, step 5).
            "salon" => self.go_team(),
            "board" => self.open_window(Window::Board),
            "code" => self.open_window(Window::Code),
            "timeline" => self.open_window(Window::Timeline),
            "home" => self.go_home(),
            "lang" if arg.is_empty() => self.set_lang(self.lang.other()),
            "lang" => match Lang::parse(arg) {
                Some(lang) => self.set_lang(lang),
                None => self.flash(fill(t.bad_lang, &[arg])),
            },
            "help" => self.open_help(),
            "quit" => self.ask_quit(),
            "pending" => self.leaving = self.in_flight(),
            "next" => self.next_waiting(false),
            "defer" => self.next_waiting(true),
            "waiting" => self.go_waiting(),
            "changes" => self.go_changes(),
            "story" => {
                self.open_window(Window::Journal);
                self.toggle_story();
            }
            "journal" => self.open_window(Window::Journal),
            "ledger" => self.open_ledger(),
            which @ ("fold" | "unfold") if self.thread.is_none() && self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, _| *f = Folds::every(which == "fold"));
                }
            }
            which @ ("fold" | "unfold") if self.thread.is_some() => {
                self.fold_talk(which == "fold");
            }
            // The team is Home's lower half: the folding is Home's.
            which @ ("fold" | "unfold") => {
                self.go_home();
                self.fold_all(which == "fold");
            }
            "trace" => {
                if let Some(id) = self.agent_named(arg) {
                    self.trace(id);
                }
            }
            "peek" => {
                if let Some(id) = self.agent_named(arg) {
                    self.peek = Some(id);
                }
            }
            "yes" => {
                if let Some(id) = self.agent_named(arg) {
                    match self.focus_ask(id) {
                        Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                            self.answer_turn(item)
                        }
                        _ => {
                            let t = self.lang.text();
                            self.flash(fill(t.turn_yes_none, &[&self.world.agents[id].name]));
                        }
                    }
                }
            }
            "why" => {
                if let Some(id) = self.agent_named(arg) {
                    let said = ui::words::why(self.lang, &self.world, id, self.why(id));
                    self.flash(said);
                }
            }
            "tidy" => self.open_tidy(),
            "ps" => self.open_ps(),
            "welcome" => self.open_welcome(),
            "news" => self.open_news(),
            "settle" => {
                self.go_home();
                self.home_settle();
                self.home_hide();
            }
            "unsettle" => self.unsettle(),
            "clear" => self.clear(arg),
            "unclear" => self.unclear(arg),
            "talk" => {
                if let Some(id) = self.agent_named(arg) {
                    self.open_talk_with(id);
                }
            }
            "pause" | "faster" | "slower" if self.live() => self.flash(fill(t.demo_only, &[name])),
            "pause" => self.pause(),
            "faster" => self.faster(),
            "slower" => self.slower(),
            "awake" => {
                self.awake.on = !self.awake.on;
                let t = self.lang.text();
                self.flash(if self.awake.on {
                    t.awake_on
                } else {
                    t.awake_off
                });
                self.keep_awake();
            }
            "keys" => self.read_keys(),
            "notify" => match (arg, Notify::parse(arg)) {
                ("", _) => self.set_notify(self.notify.next()),
                (_, Some(how)) => self.set_notify(how),
                (_, None) => self.flash(fill(t.bad_notify, &[arg])),
            },
            "motion" => match (arg, Motion::parse(arg)) {
                ("", _) => self.set_motion(self.motion.next()),
                (_, Some(how)) => self.set_motion(how),
                (_, None) => self.flash(fill(t.bad_motion, &[arg])),
            },
            "away" => match (arg, Away::parse(arg)) {
                ("", _) => self.set_away(self.away.next()),
                (_, Some(away)) => self.set_away(away),
                (_, None) => self.flash(fill(t.bad_away, &[arg])),
            },
            "digest" => self.open_digest(),
            "health" => self.set_health(arg),
            _ => self.flash(fill(t.no_command, &[name])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn a_message_to_an_agent_with_no_text_is_not_sent() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        let was = app.world.journal.len();
        app.typing = true;
        app.input = format!("@{name}   ");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.world.journal.len(), was, "nothing said");
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().empty_message);
    }

    #[test]
    fn the_colon_menu_notes_each_commands_value_and_what_raises_the_bill() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.agents.iter_mut().for_each(|a| a.cost = 0);
        app.world.agents[0].cost = 31_000;
        app.world.budget = 200_000;
        // What the header no longer carries (record 125): the rate, how
        // long the pot lasts at it, the usage window nearest to full.
        assert_eq!(
            app.command_note("budget"),
            "($3.10 of $20.00 · $18.60/h, ~54 min left)"
        );
        app.world.limits = vec![crate::model::Limit {
            minutes: 300,
            used: 52,
            resets_at: None,
        }];
        assert_eq!(
            app.command_note("budget"),
            "($3.10 of $20.00 · $18.60/h, ~54 min left · claude 5h: 48% left)"
        );
        app.world.limits.clear();
        app.world.run_spent = 31_000;
        assert_eq!(
            app.command_note("budget"),
            "($3.10 of $20.00 · $0.00/h)",
            "nothing spent this window: no rate, no end in sight"
        );
        theme::choose("light").unwrap();
        assert_eq!(app.command_note("theme"), "(light)");
        assert_eq!(app.command_note("lang"), "(English)");
        app.lang = Lang::Tr;
        assert_eq!(app.command_note("lang"), "(Türkçe)");
        app.lang = Lang::En;
        let project = format!("({})", app.world.projects[0].name);
        assert_eq!(app.command_note("close"), project);
        assert_eq!(app.command_note("rename"), project);
        // The agent a bare command acts on; `$` on what raises the bill.
        app.selected = Sel::Agent(1);
        let name = app.world.agents[1].name.clone();
        assert_eq!(app.command_note("forget"), format!("({name}) $"));
        assert_eq!(app.command_note("talk"), format!("({name})"));
        assert_eq!(app.command_note("trace"), format!("({name})"));
        assert_eq!(app.command_note("open"), "$");
        app.pane = Pane::Inbox;
        assert_eq!(app.command_note("forget"), "$", "no agent at hand");
        assert_eq!(app.command_note("talk"), "");
        assert_eq!(app.command_note("next"), "(0 to decide)");
        app.world.asks_you(1, "Bash: make deploy");
        app.world.asks_you(1, "Bash: make test");
        assert_eq!(app.command_note("next"), "(2 to decide)");
        assert_eq!(app.command_note("waiting"), "(2 to decide)");
        // A reply is read, not decided.
        app.world.replied(1, "done");
        assert_eq!(app.command_note("next"), "(2 to decide)");
        assert_eq!(app.command_note("story"), "(off)");
        app.story = true;
        assert_eq!(app.command_note("story"), "(on)");
        app.story = false;
        assert_eq!(app.command_note("pause"), "(off)");
        app.paused = true;
        assert_eq!(app.command_note("pause"), "(on)");
        app.speed = 4;
        assert_eq!(app.command_note("faster"), "(×4)");
        assert_eq!(app.command_note("slower"), "(×4)");
        for plain in ["help", "quit", "salon", "fold", "defer"] {
            assert_eq!(app.command_note(plain), "", "{plain}");
        }
    }

    #[test]
    fn the_colon_menu_carries_the_notes_of_its_items() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.on_key(key(KeyCode::Char(':')));
        for c in "op".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        let m = app.menu.clone().unwrap();
        assert_eq!(m.items, ["open"]);
        assert_eq!(m.notes, ["$"]);
        // An argument's menu has no notes, nor do names after `@`.
        app.input = ":lang e".into();
        app.menu = app.suggest();
        assert_eq!(app.menu.as_ref().unwrap().items, ["en"]);
        assert!(app.menu.as_ref().unwrap().notes.is_empty());
        app.input = "@".into();
        app.menu = app.suggest();
        assert!(!app.menu.as_ref().unwrap().items.is_empty());
        assert!(app.menu.as_ref().unwrap().notes.is_empty());
    }

    #[test]
    fn a_command_s_whole_name_runs_on_the_first_enter() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let typed =
            |app: &mut App, s: &str| s.chars().for_each(|c| app.on_key(key(KeyCode::Char(c))));
        // A name nothing else begins with: no menu, enter runs it.
        for line in [":news", ":commit", ":welcome", ":help"] {
            typed(&mut app, line);
            assert_eq!(app.menu, None, "{line}");
            app.on_key(key(KeyCode::Esc));
        }
        typed(&mut app, ":help");
        app.on_key(key(KeyCode::Enter));
        assert!(app.help && !app.typing, "ran at once");
        assert!(app.input.is_empty());
        app.help = false;
        // A name that begins a longer one: the menu is shown, the
        // highlight on the name typed, and enter runs it.
        typed(&mut app, ":archive");
        assert_eq!(app.menu.as_ref().unwrap().items, ["archive", "archives"]);
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.input.is_empty(), "ran at once");
        assert_eq!(app.menu, None);
        app.end_grace();
        // A word only begun takes the entry first.
        typed(&mut app, ":hel");
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.input.as_str(), app.typing), (":help ", true));
        assert!(!app.help);
        app.on_key(key(KeyCode::Enter));
        assert!(app.help && !app.typing);
        app.help = false;
        // A name that begins a longer one: the highlight moved to the
        // longer one takes it.
        typed(&mut app, ":archive");
        assert!(app.whole_command());
        app.menu.as_mut().unwrap().chosen = 1;
        assert!(!app.whole_command());
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.input.as_str(), app.typing), (":archives ", true));
        assert!(!app.whole_command(), "put in already");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        // An argument's menu is not a command's name.
        typed(&mut app, ":lang e");
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.input.as_str(), app.typing), (":lang en", true));
    }

    #[test]
    fn writing_colon_theme_name_previews_it_and_only_sending_keeps_it() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        theme::choose("terminal").unwrap();
        let typed =
            |app: &mut App, s: &str| s.chars().for_each(|c| app.on_key(key(KeyCode::Char(c))));
        typed(&mut app, ":theme ");
        assert_eq!(
            (pal_name(), app.theme_was.clone()),
            ("terminal".into(), None)
        );
        typed(&mut app, "dark");
        assert_eq!(pal_name(), "dark", "previewed");
        assert_eq!(app.theme(), "terminal", "not chosen");
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(
            (pal_name(), app.theme_was.clone()),
            ("terminal".into(), None)
        );
        typed(&mut app, "k");
        assert_eq!(pal_name(), "dark");
        // Given up: back to what was.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert!(!app.typing);
        assert_eq!(
            (pal_name(), app.theme()),
            ("terminal".into(), "terminal".into())
        );
        // Stepping through the menu wears each one in turn.
        typed(&mut app, ":theme ");
        app.on_key(key(KeyCode::Tab));
        let first = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_eq!(pal_name(), first);
        app.on_key(key(KeyCode::Tab));
        let second = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_ne!(first, second);
        assert_eq!(pal_name(), second);
        app.on_key(key(KeyCode::Tab));
        let third = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_eq!(pal_name(), third);
        assert_eq!(app.theme(), "terminal", "what was before the first one");
        // Sent: kept.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(pal_name(), "terminal");
        typed(&mut app, ":theme light");
        assert_eq!(
            (pal_name(), app.theme()),
            ("light".into(), "terminal".into())
        );
        app.on_key(key(KeyCode::Enter));
        assert_eq!((pal_name(), app.theme()), ("light".into(), "light".into()));
        assert_eq!(app.theme_was, None);
        // A paste names one too.
        typed(&mut app, ":theme ");
        app.paste("dark");
        assert_eq!((pal_name(), app.theme()), ("dark".into(), "light".into()));
        // Only after `:theme `, and only while writing.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        app.input = ":theme dark".into();
        app.preview_theme();
        assert_eq!(pal_name(), "light", "not writing");
        app.typing = true;
        app.input = "theme dark".into();
        app.preview_theme();
        assert_eq!(pal_name(), "light");
        theme::choose("terminal").unwrap();
    }

    fn pal_name() -> String {
        theme::pal().name.clone()
    }

    #[test]
    fn every_key_that_acts_has_a_colon_name_doing_the_same() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("board");
        assert_eq!(app.window, Some(Window::Board));
        app.command("code");
        assert_eq!(app.window, Some(Window::Code));
        app.command("salon");
        assert_eq!((app.window), (None), "the team is Home's");
        // The language: turned over, or named.
        app.command("lang");
        assert_eq!(app.lang, Lang::Tr);
        assert_eq!(said(&mut app), "Türkçe");
        app.command("lang tr");
        assert_eq!(app.lang, Lang::Tr);
        app.command("lang en");
        assert_eq!(app.lang, Lang::En);
        app.command("lang xx");
        assert_eq!(app.lang, Lang::En);
        assert_eq!(said(&mut app), "xx is not a language: en or tr");
        // `:theme` alone is `T`.
        theme::choose("dark").unwrap();
        app.command("theme");
        let next = "light";
        assert_eq!(theme::pal().name, next);
        assert_eq!(said(&mut app), format!("theme: {next}"));
        app.help_find = "old".into();
        app.command("help");
        assert!(app.help && app.help_find.is_empty());
        app.help = false;
        // `:quit` and `:q` ask as `q` does.
        for line in ["quit", "q"] {
            app.pending = None;
            app.command(line);
            assert_eq!(app.pending, Some('q'), "{line}");
            assert_eq!(said(&mut app), app.quit_ask());
        }
        app.pending = None;
        app.leaving = true;
        app.command("pending");
        assert!(!app.leaving, "nothing in flight");
        app.world.asks_you(1, "Bash: one");
        app.world.asks_you(1, "Bash: two");
        app.command("next");
        assert_eq!(app.waiting_at, Some(0));
        app.command("defer");
        assert_eq!(app.waiting_at, Some(1));
        app.go_home();
        app.command("waiting");
        assert_eq!((app.pane), (Pane::Inbox), "Home's first row to decide");
        assert_eq!(app.at_home.at, Some(HomeRow::Item(1)), "item 0 was put off");
        // `:changes` as `gc`, from anywhere: none waits, a flash.
        app.zoom = Zoom::Focus(0);
        app.command("changes");
        assert_eq!((app.zoom), (Zoom::None), "back on Home");
        assert_eq!(said(&mut app), app.lang.text().no_change_waits);
        let mut change = app.world.inbox[1].clone();
        change.kind = InboxKind::Change;
        app.world.inbox.push(change);
        app.zoom = Zoom::Focus(0);
        app.command("changes");
        assert_eq!((app.zoom), (Zoom::None));
        assert_eq!(app.at_home.at, Some(HomeRow::Item(2)), "the change");
        assert_eq!(said(&mut app), "");
        app.go_home();
        app.command("fold");
        assert!(app.collapsed.iter().all(|&c| c));
        app.command("unfold");
        assert!(app.collapsed.iter().all(|&c| !c));
        app.zoom = Zoom::Focus(0);
        app.command("fold");
        assert_eq!((app.zoom), (Zoom::None), "the team is Home's");
        assert!(app.collapsed.iter().all(|&c| c));
        app.command("unfold");
        assert!(app.collapsed.iter().all(|&c| !c));
        // An agent: the one at hand, or named, `@` or not.
        let name = app.world.agents[1].name.clone();
        app.selected = Sel::Agent(0);
        app.command("trace");
        assert_eq!(app.zoom, Zoom::Trace(0));
        app.command(&format!("trace @{name}"));
        assert_eq!(app.zoom, Zoom::Trace(1));
        app.zoom = Zoom::None;
        app.command("trace nobody");
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        app.command(&format!("talk {name}"));
        let t = app.lang.text();
        assert_eq!(said(&mut app), fill(t.no_talk_yet, &[&name]));
        app.command("talk nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        // The demo's clock.
        app.command("pause");
        assert!(app.paused);
        app.command("faster");
        assert_eq!(app.speed, 2);
        app.command("faster");
        app.command("slower");
        assert_eq!(app.speed, 2);
        app.command("nope");
        assert_eq!(said(&mut app), "no command nope; : and tab list them");
    }

    #[test]
    fn a_word_of_a_message_completes_to_a_file_of_the_project_by_itself_with_a_slash_else_on_tab() {
        let (mut app, w) = four_idle();
        let name = app.world.agents[w[0]].name.clone();
        let tab = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Tab));
        let menu = |app: &App| {
            app.menu
                .as_ref()
                .map(|m| m.items.clone())
                .unwrap_or_default()
        };
        // The demo's files are its code map's.
        let files = app.world.files_of(0);
        assert!(files.contains(&"src/Auth/Session.cs"), "{files:?}");
        // A word with a `/` opens the menu by itself, at the word; enter
        // puts the file in its place, a space after it; enter sends.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "see src/Auth/Se");
        assert_eq!(menu(&app), ["src/Auth/Session.cs"]);
        assert_eq!(app.menu.as_ref().unwrap().start, 4);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "see src/Auth/Session.cs ");
        assert!(app.typing);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.typing);
        // Without a `/`, nothing until tab, which takes the first match.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "fix Rates");
        assert!(app.menu.is_none());
        tab(&mut app);
        assert_eq!(app.input, "fix src/Shipping/Rates.cs ");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // In a message to one agent, and on a `!` line (its `!` is not
        // part of the first word).
        type_in(&mut app, &format!("@{name} src/Orders/Mo"));
        assert_eq!(menu(&app), ["src/Orders/Money.cs"]);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        type_in(&mut app, "!src/Orders/Mo");
        assert_eq!(menu(&app), ["src/Orders/Money.cs"]);
        assert_eq!(app.menu.as_ref().unwrap().start, 1);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "!src/Orders/Money.cs ");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // Not a name, not a command, not on the lines that are something
        // else, not after a space, not over several lines.
        type_in(&mut app, "@cl");
        assert!(
            menu(&app).iter().all(|i| !i.contains('/')),
            "{:?}",
            menu(&app)
        );
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        for line in ["#src/", "&src/", "//src/"] {
            type_in(&mut app, line);
            assert!(
                menu(&app).iter().all(|i| !i.ends_with(".cs")),
                "{line}: {:?}",
                menu(&app)
            );
            app.on_key(KeyEvent::from(KeyCode::Esc));
            app.on_key(KeyEvent::from(KeyCode::Esc));
        }
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "src/Or ");
        assert!(app.menu.is_none());
        tab(&mut app);
        assert!(app.menu.is_none() && app.input == "src/Or ");
        type_in(&mut app, "x\\");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        type_in(&mut app, "src/Or");
        assert!(app.menu.is_none(), "over lines");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // A symbol leased on the code map: `file::name`, the holder as the
        // note of the file and of its symbols.
        let card = app
            .world
            .cards
            .iter()
            .position(|c| c.project == 0)
            .expect("a card");
        app.world.cards[card].owner = Some(w[1]);
        app.world.cards[card].column = Column::Working;
        app.world.cards[card].symbols = vec![Symbol {
            file: "src/Auth/Session.cs",
            name: "Session.Start",
        }];
        for c in app.world.cards.iter_mut().filter(|c| c.id != card) {
            c.symbols.clear();
        }
        // A card that holds no lease any more (done) offers nothing.
        let done = app.world.cards.iter().position(|c| c.id != card).unwrap();
        app.world.cards[done].project = 0;
        app.world.cards[done].owner = Some(w[2]);
        app.world.cards[done].column = Column::Done;
        app.world.cards[done].symbols = vec![Symbol {
            file: "src/Auth/Session.cs",
            name: "Session.End",
        }];
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "Session.");
        tab(&mut app);
        let m = app.menu.as_ref().expect("the menu");
        assert_eq!(
            m.items,
            ["src/Auth/Session.cs", "src/Auth/Session.cs::Session.Start"]
        );
        let holder = format!("○ {}", app.world.agents[w[1]].name);
        assert_eq!(m.notes, [holder.clone(), holder]);
        assert_eq!(app.input, "src/Auth/Session.cs ");
        tab(&mut app);
        assert_eq!(app.input, "src/Auth/Session.cs::Session.Start ");
    }

    #[test]
    fn ctrl_x_ctrl_e_hands_the_line_to_the_editor_and_its_text_comes_back_unsent() {
        let (mut app, _) = four_idle();
        app.on_key(key(KeyCode::Char('i')));
        type_in(&mut app, "@claude-1 a long");
        app.after = 5;
        app.on_key(ctrl('x'));
        assert!(app.editor.is_none(), "^x alone waits");
        app.on_key(ctrl('e'));
        assert_eq!(
            app.editor.as_deref(),
            Some("@claude-1 a long"),
            "the whole line"
        );
        assert!(app.menu.is_none());
        let draft = app.editor.take().unwrap();
        // What the editor left is the line, cleaned, the cursor at its end;
        // nothing is sent and nothing is kept as sent.
        let sent = app.world.sent.len();
        app.edited(&draft, Ok("@claude-1 a long\nmessage\t\u{1b}[2J".into()));
        assert!(app.typing && app.editor.is_none());
        assert_eq!(app.input, "@claude-1 a long\nmessage\t[2J");
        assert_eq!(app.after, 0);
        assert_eq!(app.world.sent.len(), sent);
        // An editor that failed gives the draft back as it was; why was
        // said on the terminal before the screen came back.
        app.input = "changed".into();
        app.after = 2;
        let why = editor::Why::Ended(Ended::Exit(1));
        assert_eq!(
            app.editor_back("vim", &why),
            "[imece] `vim` ended with 1: the draft is kept · any key: back to the screen\n"
        );
        app.edited("the draft", Err(why));
        assert_eq!((app.input.as_str(), app.after), ("the draft", 0));
        assert!(app.typing);
        app.lang = Lang::Tr;
        assert_eq!(
            app.editor_back("vim", &editor::Why::NotText),
            "[imece] `vim` UTF-8 metin olmayan bir şey bıraktı: taslak duruyor · herhangi bir tuş: ekrana dön\n"
        );
    }

    #[test]
    fn ctrl_x_then_another_key_does_what_that_key_does_alone() {
        let (mut app, _) = four_idle();
        app.on_key(key(KeyCode::Char('i')));
        type_in(&mut app, "ab");
        app.on_key(ctrl('x'));
        app.on_key(ctrl('a'));
        assert_eq!(app.cursor(), 0, "^a went to the start");
        // The chord is gone: a later ^e is only the end of the line.
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        assert_eq!(app.cursor(), 2);
        // A plain e after ^x is a letter, and the chord goes with it.
        app.on_key(ctrl('x'));
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.input, "abe");
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        // ^e without ^x before it never opens the editor, and x without
        // ctrl is a letter, no chord.
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.input, "abex");
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
    }

    #[test]
    fn edit_opens_the_editor_on_what_follows_it_and_its_menu_names_the_editor() {
        let (mut app, _) = four_idle();
        app.command("edit");
        assert_eq!(app.editor.as_deref(), Some(""));
        app.editor = None;
        app.command("edit dear leader");
        assert_eq!(app.editor.as_deref(), Some("dear leader"));
        assert!(complete::COMMANDS.contains(&"edit"));
        assert_eq!(app.command_note("edit"), format!("({})", editor::of_env()));
        assert!(!VIEW_COMMANDS.contains(&"edit"));
        // Every way it fails is worded, in both languages.
        let t = |lang, why| ui::words::editor_why(lang, "ed", &why);
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Exit(127))),
            "no editor `ed`: set $VISUAL or $EDITOR; the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Exit(126))),
            "`ed` ended with 126: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Signal(9))),
            "`ed` was stopped by signal 9: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Long),
            "`ed` left more than 1024 KiB: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::NotText),
            "`ed` left what is not UTF-8 text: the draft is kept"
        );
        let io = io::Error::other("disk full");
        assert_eq!(
            t(Lang::En, editor::Why::Io(io)),
            "could not open the draft in `ed`: disk full; it is kept"
        );
        assert_eq!(
            t(Lang::Tr, editor::Why::Ended(Ended::Exit(2))),
            "`ed` 2 ile bitti: taslak duruyor"
        );
    }

    #[test]
    fn the_cursor_stands_where_the_person_types_and_its_shape_says_what() {
        let typed = |input: &str, set: fn(&mut App)| {
            let mut a = App::new(1, 1, 0, 1, Lang::En);
            a.typing = true;
            a.input = input.into();
            set(&mut a);
            a.caret()
        };
        let line = |c| Some((Field::Line, c));
        let as_is = |_: &mut App| {};
        assert_eq!(typed("", |a| a.typing = false), None, "nothing typed");
        assert_eq!(typed("hello", as_is), line(Caret::Write));
        assert_eq!(typed("@lead hi", as_is), line(Caret::Write));
        assert_eq!(typed(":budget 80", as_is), line(Caret::Command));
        assert_eq!(typed("!cargo test", as_is), line(Caret::Write));
        assert_eq!(typed("#323", as_is), line(Caret::Find));
        assert_eq!(typed("&claude-1", as_is), line(Caret::Find));
        // The heading's order: a note, then `^r`, then `!`, `#` `&`, `:`.
        let note = |a: &mut App| a.noting = Some(Noting::Line(0, 0));
        let search = |a: &mut App| a.searching = true;
        assert_eq!(typed(":x", note), line(Caret::Write));
        assert_eq!(typed("#x", note), line(Caret::Write));
        assert_eq!(typed(":x", search), line(Caret::Find));
        assert_eq!(typed("!x", search), line(Caret::Find));
        assert_eq!(typed("x", search), line(Caret::Find));
        // A window on top that takes no text hides it, each on its own.
        let windows: [fn(&mut App); 7] = [
            |a| a.news = vec!["x"],
            |a| a.welcome = true,
            |a| {
                a.digest = Some(Digest {
                    minutes: 4,
                    merged: 0,
                    blocked: 0,
                    assumptions: 0,
                    talks: 0,
                    waiting: 0,
                    highlights: Vec::new(),
                })
            },
            |a| a.left_off = Some(LeftOff::default()),
            |a| a.leaving = true,
            |a| a.solo_pick = Some(SoloPick::new(None, None)),
            |a| a.help = true,
        ];
        for (i, window) in windows.iter().enumerate() {
            assert_eq!(typed("hello", *window), None, "{i}");
        }
        // The filters of the help and the archives take it while typed.
        let help = |a: &mut App| {
            a.help = true;
            a.help_typing = true;
        };
        assert_eq!(typed("hello", help), Some((Field::Help, Caret::Find)));
        let browsing = |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new()));
        assert_eq!(typed("hello", browsing), None, "browsing, not filtering");
        let filtering = |a: &mut App| {
            let mut b = Browser::new(&a.world, Vec::new());
            b.typing = true;
            a.archives = Some(b);
            a.help = true;
            a.help_typing = true;
        };
        assert_eq!(
            typed("", filtering),
            Some((Field::Archives, Caret::Find)),
            "the archives' window is over the help's"
        );
        // An archive's view has the cursor of its own line.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.typing = true;
        app.input = "hello".into();
        let mut v = App::new(1, 1, 0, 1, Lang::En);
        v.typing = true;
        v.input = ":quit".into();
        app.viewing = Some(Box::new(v));
        assert_eq!(app.caret(), line(Caret::Command));
        app.viewing.as_mut().unwrap().typing = false;
        assert_eq!(app.caret(), None, "the view types nothing");
    }
}
