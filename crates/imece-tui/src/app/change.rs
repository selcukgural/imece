//! A change (Δ) on the screen: its folds, notes and decision, its review and commit message.

use super::*;

/// The checks the person names after `:checks` (record 127): `none`, or
/// names between commas or spaces, each one Lonca takes, at most
/// [`CHECKS_MOST`](imece_agent::lonca::CHECKS_MOST); sorted, each once.
fn checks_named(t: &crate::i18n::Text, arg: &str) -> Result<Vec<String>, String> {
    if arg == "none" {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = (arg.split(|c: char| c == ',' || c.is_whitespace()))
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect();
    if let Some(bad) = names.iter().find(|n| !imece_agent::lonca::is_check_name(n)) {
        return Err(fill(t.checks_bad_name, &[bad]));
    }
    names.sort();
    names.dedup();
    let most = imece_agent::lonca::CHECKS_MOST;
    if names.len() > most {
        return Err(fill(t.checks_too_many, &[&most.to_string()]));
    }
    Ok(names)
}

/// Refuses those of `checks` that the policy's `known` checks lack (#390),
/// in the person's words: which, the names it has, the nearest when one
/// was lacking.
fn checks_known(t: &crate::i18n::Text, checks: &[String], known: &[String]) -> Result<(), String> {
    let lacking = crate::checks::unknown(checks, known);
    let [first, ..] = lacking.as_slice() else {
        return Ok(());
    };
    let names = crate::checks::listed(&lacking);
    if known.is_empty() {
        return Err(fill(t.checks_policy_none, &[&names]));
    }
    let mut why = fill(
        t.checks_not_policy,
        &[&names, &crate::checks::listed(known)],
    );
    if let (1, Some(near)) = (lacking.len(), crate::checks::nearest(first, known)) {
        why += &fill(t.checks_near, &[near]);
    }
    Err(why)
}

impl App {
    /// A key for the open change of worker `id`; `false` leaves it to the
    /// rest of the screen (typing, switching views, quitting).
    pub(super) fn diff_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let lines = self
            .world
            .change_of(id)
            .map_or(0, |c| c.patch.lines().count());
        let last = lines.saturating_sub(1);
        let page = usize::from(self.diff_page.get()).max(1);
        // The scroll goes over the lines shown: a folded file is its header.
        let shown = self.diff_shown(id);
        let step = |at: usize, by: usize, on: bool| shown.step(at, by, on);
        match k.code {
            KeyCode::Esc => {
                self.zoom = Zoom::None;
                self.pane = Pane::Inbox;
            }
            KeyCode::Up | KeyCode::Char('k') => self.diff_scroll = step(self.diff_scroll, 1, false),
            KeyCode::Down | KeyCode::Char('j') => {
                self.diff_scroll = step(self.diff_scroll, 1, true)
            }
            KeyCode::PageUp => self.diff_scroll = step(self.diff_scroll, page, false),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.diff_scroll = step(self.diff_scroll, page, true)
            }
            KeyCode::Home => self.diff_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.diff_scroll = step(last, 0, true),
            KeyCode::Left | KeyCode::Char('h') => self.fold_files(id, |f, n| {
                if let Some(n) = n {
                    f.set(n, true)
                }
            }),
            KeyCode::Right | KeyCode::Char('l') => self.fold_files(id, |f, n| {
                if let Some(n) = n {
                    f.set(n, false)
                }
            }),
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let starts = self
                    .world
                    .change_of(id)
                    .map(|c| ui::file_starts(&c.patch))
                    .unwrap_or_default();
                if let Some(to) = jump(&starts, self.diff_scroll, k.code == KeyCode::Char(']')) {
                    self.diff_scroll = to;
                }
            }
            KeyCode::Char('a') => {
                if self.decide_change(id, true) {
                    self.next_change(id);
                }
            }
            KeyCode::Char('r') => {
                self.decide_change(id, false);
            }
            KeyCode::Char('>') => self.quote_hunk(id),
            KeyCode::Char('c') => self.note_line(id),
            KeyCode::Char('x') => self.choose_hunk(id),
            KeyCode::Char('w') => self.diff_wrap = !self.diff_wrap,
            KeyCode::Char('R') => self.review_whole = !self.review_whole,
            KeyCode::Char('n') => self.send_notes(id),
            KeyCode::Char('b') => self.send_back(id),
            KeyCode::Char('v') => {
                // Lines are chosen from the whole patch: its files open.
                self.diff_folds = None;
                if let Some(c) = self.world.change_of(id) {
                    let at = Copying::new(self.diff_scroll.min(last));
                    self.diff_copy = Some((id, patch_key(&c.patch), at));
                }
            }
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// The lines of worker `id`'s change as it is now being chosen to
    /// copy, if they are.
    /// Which files of agent `id`'s change are folded: none unless they
    /// were folded on this very patch.
    pub fn diff_folds_of(&self, id: AgentId) -> Folds {
        let patch = self.world.change_of(id).map(|c| patch_key(&c.patch));
        (self.diff_folds.as_ref())
            .filter(|(a, p, _)| *a == id && Some(*p) == patch)
            .map(|(_, _, f)| f.clone())
            .unwrap_or_default()
    }

    /// The lines agent `id`'s change shows now.
    pub(super) fn diff_shown(&self, id: AgentId) -> ui::Shown {
        match self.world.change_of(id) {
            Some(c) => ui::Shown::of(&c.patch, &self.diff_folds_of(id)),
            None => ui::Shown::default(),
        }
    }

    /// The files of agent `id`'s change folded as `change` makes them
    /// from what they are; the line at hand, hidden, goes to its file's
    /// header.
    pub(super) fn fold_files(
        &mut self,
        id: AgentId,
        change: impl FnOnce(&mut Folds, Option<usize>),
    ) {
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        let (key, file) = (patch_key(&c.patch), ui::file_of(&c.patch, self.diff_scroll));
        let mut folds = self.diff_folds_of(id);
        change(&mut folds, file);
        self.diff_folds = Some((id, key, folds));
        let shown = self.diff_shown(id);
        if !shown.lines.contains(&self.diff_scroll) {
            self.diff_scroll = shown.step(self.diff_scroll, 0, true);
        }
    }

    pub fn diff_copy_of(&self, id: AgentId) -> Option<Copying> {
        let patch = self.world.change_of(id).map(|c| patch_key(&c.patch))?;
        (self.diff_copy)
            .filter(|&(a, p, _)| a == id && p == patch)
            .map(|(_, _, c)| c)
    }

    /// The change open, when no conversation is over it.
    pub(super) fn diff_zoom(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Diff(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// `za` in a conversation: the selected message folded, or open.
    pub(super) fn toggle_fold(&mut self) {
        if let Some(th) = &mut self.thread {
            let folded = th.folds.folded(th.sel);
            th.folds.set(th.sel, !folded);
            th.follow.set(true);
        }
    }

    /// `:commit`: a cheap model is asked for a commit message for what is
    /// staged in the project at hand's checkout; it comes to the command
    /// line when it answers (see [`App::show_suggestion`]).
    pub(super) fn suggest_commit(&mut self) {
        let t = self.lang.text();
        let p = self.current_project();
        let asked = match &mut self.engine {
            Engine::Live(l) => l.suggest(&self.world, p),
            Engine::Sim(s) => s.suggest(&self.world, p),
            Engine::View(_) => return self.read_only(),
        };
        match asked {
            Ok(model) => {
                let name = self.world.projects[p].name.clone();
                let asking = fill(t.suggesting, &[&model, &name]);
                self.asking = Some(asking.clone());
                self.flash(asking);
            }
            Err(why) => self.flash(fill(t.suggest_refused, &[&why])),
        }
    }

    /// Asks the engine for a review of worker `agent`'s change.
    fn ask_review(&mut self, agent: AgentId) -> Result<(), String> {
        match &mut self.engine {
            Engine::Live(l) => l.review(&self.world, agent),
            Engine::Sim(s) => s.review(&self.world, agent),
            Engine::View(_) => Err("an archive is read only".into()),
        }
    }

    /// Whether worker `agent`'s change is being reviewed, or is about to.
    pub fn reviewing(&self, agent: AgentId) -> bool {
        match &self.engine {
            Engine::Live(l) => l.review_of(agent),
            Engine::Sim(s) => s.review_of(agent),
            Engine::View(_) => false,
        }
    }

    /// `:review on`: each change that came to wait since the last step
    /// is asked a review; a refusal is journaled, as nobody asked by hand
    /// to be told at once.
    pub(super) fn watch_reviews(&mut self) {
        let new = self.review_items;
        self.review_items = self.world.inbox.len();
        if !self.review {
            return;
        }
        let came: Vec<AgentId> = (self.world.inbox.iter().enumerate())
            .filter(|(i, it)| *i >= new && it.open && it.kind == InboxKind::Change)
            .map(|(_, it)| it.agent)
            .collect();
        for agent in came {
            if let Err(why) = self.ask_review(agent) {
                self.world.note(Event::ReviewFailed {
                    agent,
                    why,
                    cost: 0,
                });
            }
        }
    }

    /// `:review [on|off]`: `on` and `off` set whether every change gets
    /// a review as it comes; bare, the change at hand (the one open, else
    /// the selected waiting item) is asked one now, and with no change at
    /// hand the setting is said.
    pub(super) fn review_command(&mut self, arg: &str) {
        let t = self.lang.text();
        match arg {
            "on" | "off" => {
                self.review = arg == "on";
                return self.flash(if self.review {
                    t.review_on
                } else {
                    t.review_off
                });
            }
            "" => {}
            _ => return self.flash(fill(t.review_refused, &["on | off"])),
        }
        let Some(agent) = self.change_at_hand() else {
            return self.flash(if self.review {
                t.review_on
            } else {
                t.review_off
            });
        };
        match self.ask_review(agent) {
            Ok(()) => {
                let name = self.world.agents[agent].name.clone();
                self.flash(fill(t.reviewing, &[CLAUDE, &name]));
            }
            Err(why) => self.flash(fill(t.review_refused, &[&why])),
        }
    }

    /// The worker whose change is at hand: the change open, else the one
    /// `W` opened, else Home's row.
    pub(super) fn change_at_hand(&self) -> Option<AgentId> {
        self.diff_zoom().or_else(|| {
            let i = self.waiting_at_hand().or_else(|| match self.home_at()? {
                HomeRow::Item(i) => Some(i),
                HomeRow::Agent(_) | HomeRow::Project(_) => None,
            })?;
            let it = &self.world.inbox[i];
            (it.open && it.kind == InboxKind::Change).then_some(it.agent)
        })
    }

    /// `:checks [NAMES | none]` (record 127): the checks of Lonca's policy
    /// the yes to the change at hand asks for, the card's, which `NAMES`
    /// (commas or spaces between) or `none` set first. The person signs
    /// the yes, so the person may change them at it: a name the policy
    /// lacks need not make the change a reject. Refused off Lonca (nothing
    /// would run them), for a change made for no card, and for a name the
    /// policy in force lacks once Lonca says what it has (#390).
    pub(super) fn checks_command(&mut self, arg: &str) {
        let t = self.lang.text();
        let refused = |app: &mut App, why: &str| app.flash(fill(t.checks_refused, &[why]));
        let Some(agent) = self.change_at_hand() else {
            return refused(self, t.checks_no_change);
        };
        let name = self.world.agents[agent].name.clone();
        let project = self.world.agents[agent].project;
        if self.world.projects[project].lonca.is_none() {
            let p = self.world.projects[project].name.clone();
            return refused(self, &fill(t.checks_not_lonca, &[&p]));
        }
        let card = self.world.change_card(agent);
        let Some(card) = card.filter(|&c| self.world.cards.get(c).is_some()) else {
            return refused(self, &fill(t.checks_no_card, &[&name]));
        };
        if !arg.is_empty() {
            let known = self.world.policy_checks(project);
            let named = checks_named(t, arg).and_then(|checks| {
                known.map_or(Ok(()), |known| checks_known(t, &checks, known))?;
                Ok(checks)
            });
            match named {
                Ok(checks) => self.world.set_checks(card, checks, None),
                Err(why) => return refused(self, &why),
            }
        }
        let checks = &self.world.cards[card].checks;
        self.flash(match checks.is_empty() {
            true => fill(t.checks_are_none, &[&name]),
            false => fill(t.checks_are, &[&name, &crate::checks::listed(checks)]),
        });
    }

    /// A suggested commit message, once the command line is free: it
    /// opens as a `!git commit` of the message to edit and run, in the
    /// project's checkout (`-C` when that is not the one at hand). The
    /// demo's commits nothing (`--dry-run`): its message was made up.
    pub(super) fn show_suggestion(&mut self) {
        if self.typing {
            return;
        }
        let Some(s) = self.world.suggested.take() else {
            return;
        };
        let dir = match &self.engine {
            Engine::Live(l) if s.project != self.current_project() => {
                Some(l.repo_of(s.project).to_path_buf())
            }
            _ => None,
        };
        self.input = commit_line(&s.text, dir.as_deref(), !self.live());
        self.after = 0;
        self.typing = true;
        self.menu = None;
    }

    /// `>` in worker `id`'s change: a message to it opens with the hunk at
    /// the top of the view quoted under the line the person writes on.
    fn quote_hunk(&mut self, id: AgentId) {
        let change = self.world.change_of(id);
        let Some(quote) = change.and_then(|c| ui::quote(&c.patch, self.diff_scroll)) else {
            return self.flash(self.lang.text().no_hunk);
        };
        let head = format!("@{} ", self.world.agents[id].name);
        self.input = format!("{head}\n{quote}");
        self.after = self.input.len() - head.len();
        self.typing = true;
        self.menu = None;
    }

    /// The notes on worker `id`'s change as it is now, by line.
    pub fn notes_of(&self, id: AgentId) -> Vec<&DiffNote> {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return Vec::new();
        };
        let mut notes: Vec<&DiffNote> = (self.diff_notes.iter())
            .filter(|n| n.agent == id && n.patch == patch)
            .collect();
        notes.sort_by_key(|n| n.line);
        notes
    }

    /// The hunks of worker `id`'s change as it is now that the person
    /// chose to apply alone, by their `@@` lines, in order.
    pub fn chosen_of(&self, id: AgentId) -> Vec<usize> {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return Vec::new();
        };
        let mut hunks: Vec<usize> = (self.chosen.iter())
            .filter(|&&(a, p, _)| a == id && p == patch)
            .map(|&(_, _, h)| h)
            .collect();
        hunks.sort_unstable();
        hunks
    }

    /// `x` in worker `id`'s change: the hunk at hand is chosen to be
    /// applied alone (with the others chosen), or no longer.
    fn choose_hunk(&mut self, id: AgentId) {
        let t = self.lang.text();
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        if self.world.proposal_of(id).is_some() {
            return self.flash(t.proposal_whole);
        }
        let Some(hunk) = ui::hunk_at(&c.patch, self.diff_scroll) else {
            return self.flash(t.no_hunk);
        };
        let all = c.patch.lines().filter(|l| l.starts_with("@@")).count();
        let key = (id, patch_key(&c.patch), hunk);
        let was = self.chosen.len();
        self.chosen.retain(|&k| k != key);
        if self.chosen.len() == was {
            self.chosen.push(key);
        }
        let n = self.chosen_of(id).len();
        let said = match n {
            0 => t.unchosen.to_string(),
            _ => fill(t.chosen, &[&n.to_string(), &all.to_string()]),
        };
        self.flash(said);
    }

    /// Worker `id`'s change is rejected: a proposal in Lonca with `reason`,
    /// which it must have. Whether it was; the flash says why not.
    pub(super) fn reject(&mut self, id: AgentId, reason: &str) -> bool {
        let t = self.lang.text();
        let proposed = self.world.proposal_of(id).is_some();
        let returned = match &mut self.engine {
            Engine::Live(l) if proposed => l.reject_proposal(&self.world, id, reason),
            Engine::Sim(s) if proposed => s.reject_proposal(&mut self.world, id, reason),
            Engine::Live(l) => l.reject_change(&mut self.world, id),
            Engine::Sim(s) => s.reject_change(&mut self.world, id),
            Engine::View(_) => {
                self.read_only();
                return false;
            }
        };
        if let Err(why) = &returned {
            let name = self.world.agents[id].name.clone();
            self.flash(fill(t.reject_failed, &[&name, why]));
        }
        returned.is_ok()
    }

    /// `c` in worker `id`'s change: a note on the line at hand (the top
    /// one), written in the command line; one there already is written
    /// again. Only a line of a hunk takes one.
    fn note_line(&mut self, id: AgentId) {
        let line = self.diff_scroll;
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        if ui::place(&c.patch, line).is_none() {
            return self.flash(self.lang.text().note_where);
        }
        let patch = patch_key(&c.patch);
        let had = (self.diff_notes.iter())
            .find(|n| n.agent == id && n.patch == patch && n.line == line)
            .map(|n| n.text.clone());
        self.input = had.unwrap_or_default();
        self.after = 0;
        self.typing = true;
        self.menu = None;
        self.noting = Some(Noting::Line(id, line));
    }

    /// The note written on `line` of worker `id`'s change is kept (in
    /// place of one there was); an empty one takes it away.
    pub(super) fn keep_note(&mut self, id: AgentId, line: usize, text: String) {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return;
        };
        let t = self.lang.text();
        self.diff_notes
            .retain(|n| !(n.agent == id && n.patch == patch && n.line == line));
        if text.is_empty() {
            return self.flash(t.note_dropped);
        }
        self.diff_notes.push(DiffNote {
            agent: id,
            patch,
            line,
            text,
        });
        let n = self.notes_of(id).len().to_string();
        self.flash(fill(t.noted, &[&n]));
    }

    /// `n` in worker `id`'s change: its notes go to it in one message and
    /// the change goes back, not applied, for it to change and send again:
    /// the third answer beside apply and reject.
    fn send_notes(&mut self, id: AgentId) {
        let t = self.lang.text();
        let notes: Vec<(usize, String)> = (self.notes_of(id).iter())
            .map(|n| (n.line, n.text.clone()))
            .collect();
        let Some(patch) = self.world.change_of(id).map(|c| c.patch.clone()) else {
            return;
        };
        if notes.is_empty() {
            return self.flash(t.no_notes);
        }
        let name = self.world.agents[id].name.clone();
        let refs: Vec<(usize, &str)> = notes.iter().map(|(l, n)| (*l, n.as_str())).collect();
        let message = ui::notes_message(&patch, &refs);
        let said = match notes.len() {
            1 => fill(t.note_sent, &[&name]),
            n => fill(t.notes_sent, &[&n.to_string(), &name]),
        };
        if self.goes_back(id, &message, said) {
            self.diff_notes.retain(|n| n.agent != id);
        }
    }

    /// `b` in worker `id`'s change: one git refused goes back to it with
    /// what clashes, for it to bring its worktree up and resolve (record
    /// 108). A worker that can not hear it leaves the change as it is.
    fn send_back(&mut self, id: AgentId) {
        let t = self.lang.text();
        let Some(clashes) = self.world.clashes_of(id) else {
            return self.flash(t.no_clash);
        };
        let a = &self.world.agents[id];
        let name = a.name.clone();
        if a.gone || a.status == Status::Stopped {
            return self.flash(fill(t.back_gone, &[&name]));
        }
        if self.world.spent() >= self.world.budget {
            return self.flash(fill(t.budget_spent, &[&name]));
        }
        let message = ui::clash_message(clashes, a.role == Role::Solo);
        self.goes_back(id, &message, fill(t.sent_back, &[&name]));
    }

    /// Worker `id`'s change goes back as a rejection does, `message` is
    /// its next word from the person, `said` the flash when it went, and
    /// the change's view closes. Whether the change went back.
    fn goes_back(&mut self, id: AgentId, message: &str, said: String) -> bool {
        if !self.reject(id, message) {
            return false;
        }
        if self.tell(id, message) {
            self.flash(said);
        }
        let open = self.inbox_rows().len();
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
        true
    }

    /// The person has worker `id`'s change applied to their checkout (git
    /// does it on a thread of its own; the journal says how it went), or
    /// rejects it; a rejection opens the composer to that worker, so they
    /// say why. The change's view closes either way.
    pub(super) fn decide_change(&mut self, id: AgentId, apply: bool) -> bool {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let open = self.inbox_rows().len();
        let hunks = self.chosen_of(id);
        let proposed = self.world.proposal_of(id).is_some();
        if proposed && !apply && !self.viewing_archive() {
            // Lonca takes no rejection without a reason, so the words come
            // first: the line that goes is both.
            self.rejecting = Some(id);
            self.typing = true;
            self.input = format!("@{name} ");
            self.after = self.input.chars().count();
            self.flash(fill(t.say_why, &[&name]));
            self.close_change(id);
            return false;
        }
        // Git applies one change at a time: one approved while another goes
        // in follows it, and the flash says so.
        let after = apply && !proposed && self.applying_besides(Some(id));
        let decided = match &mut self.engine {
            Engine::Live(l) if apply => l.apply_change(&self.world, id, hunks),
            Engine::Sim(s) if apply => s.apply_change(&self.world, id, hunks),
            Engine::Live(l) => l.reject_change(&mut self.world, id),
            Engine::Sim(s) => s.reject_change(&mut self.world, id),
            Engine::View(_) => {
                self.read_only();
                return false;
            }
        };
        let went = decided.is_ok();
        match decided {
            Ok(()) if proposed => self.flash(fill(t.approving, &[&name])),
            Ok(()) if after => self.flash(fill(t.applying_after, &[&name])),
            Ok(()) if apply => self.flash(fill(t.applying, &[&name])),
            Err(why) if apply => self.flash(fill(t.apply_failed, &[&name, &why])),
            Ok(()) => {
                self.flash(fill(t.rejected_change, &[&name]));
                self.typing = true;
                self.input = format!("@{name} ");
            }
            Err(why) => self.flash(fill(t.reject_failed, &[&name, &why])),
        }
        self.close_change(id);
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(2));
        went
    }

    /// Whether a change of anyone but `id` (none: of anyone) is being
    /// applied to the checkout now.
    pub(super) fn applying_besides(&self, id: Option<AgentId>) -> bool {
        match &self.engine {
            Engine::Live(l) => l.applying_besides(&self.world, id),
            Engine::Sim(s) => s.applying_besides(id),
            Engine::View(_) => false,
        }
    }

    /// Worker `from`'s change was decided from Home: the next change
    /// waiting opens, so a queue is read in one go; none, Home again.
    fn next_change(&mut self, from: AgentId) {
        let w = &self.world;
        let next = (self.home_rows().into_iter()).find_map(|(_, row)| match row {
            HomeRow::Item(i) if w.inbox[i].kind == InboxKind::Change => {
                Some((i, w.inbox[i].agent)).filter(|&(_, a)| a != from)
            }
            _ => None,
        });
        if let Some((i, agent)) = next {
            self.at_home.at = Some(HomeRow::Item(i));
            self.diff_scroll = 0;
            self.zoom = Zoom::Diff(agent);
        }
    }

    /// Worker `id`'s change's view closes, if it is open.
    fn close_change(&mut self, id: AgentId) {
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
    }

    /// The reason on `line`, a line that starts with `@name ` and says
    /// something to it.
    pub(super) fn reason_on<'a>(&self, id: AgentId, line: &'a str) -> Option<&'a str> {
        let head = format!("@{} ", self.world.agents[id].name);
        let reason = line.strip_prefix(&head)?.trim();
        (!reason.is_empty()).then_some(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn two_files() -> String {
        let mut p = String::from("diff --git a/x b/x\nnew file mode 1\n@@ -0 +1 @@\n+one\n");
        p.push_str("diff --git a/y b/y\nnew file mode 1\n@@ -0 +1 @@\n+two\n+three\n");
        p
    }

    #[test]
    fn a_change_s_files_fold_to_their_headers_and_the_scroll_skips_them() {
        // Lines 0-3 are x (header 0), 4-8 are y (header 4).
        let (mut app, worker) = with_change(&two_files());
        let k = |app: &mut App, c: char| app.on_key(key(KeyCode::Char(c)));
        k(&mut app, 'h');
        assert!(app.diff_folds_of(worker).folded(0), "the file at hand");
        assert!(!app.diff_folds_of(worker).folded(1));
        k(&mut app, 'j');
        assert_eq!(app.diff_scroll, 4, "over the folded file");
        k(&mut app, 'k');
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::Right));
        assert!(!app.diff_folds_of(worker).folded(0), "l → open it");
        k(&mut app, 'j');
        assert_eq!(app.diff_scroll, 1);
        // Folding the file of a hidden line puts the line at hand on its header.
        app.diff_scroll = 6;
        app.on_key(key(KeyCode::Left));
        assert_eq!(app.diff_scroll, 4);
        assert!(app.diff_folds_of(worker).folded(1));
        k(&mut app, 'l');
        // za toggles, zM folds them all, zR opens them.
        app.diff_scroll = 2;
        k(&mut app, 'z');
        k(&mut app, 'a');
        assert!(app.diff_folds_of(worker).folded(0));
        assert_eq!(app.diff_scroll, 0);
        k(&mut app, 'z');
        k(&mut app, 'a');
        assert!(!app.diff_folds_of(worker).folded(0));
        app.diff_scroll = 7;
        k(&mut app, 'z');
        k(&mut app, 'M');
        assert!(app.diff_folds_of(worker).folded(0) && app.diff_folds_of(worker).folded(1));
        assert_eq!(app.diff_scroll, 4);
        k(&mut app, 'G');
        assert_eq!(app.diff_scroll, 4, "the last line shown");
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(app.diff_scroll, 4);
        k(&mut app, 'g');
        k(&mut app, 'g');
        assert_eq!(app.diff_scroll, 0);
        k(&mut app, 'z');
        k(&mut app, 'R');
        assert!(!app.diff_folds_of(worker).folded(1));
        k(&mut app, 'G');
        assert_eq!(app.diff_scroll, 8);
        // :fold and :unfold do it for the change open.
        app.command("fold");
        assert!(app.diff_folds_of(worker).folded(1));
        assert_eq!(app.diff_scroll, 4);
        app.command("unfold");
        assert!(!app.diff_folds_of(worker).folded(1));
        // `v` chooses from the whole patch: its files open.
        k(&mut app, 'h');
        k(&mut app, 'v');
        assert_eq!(app.diff_folds_of(worker), Folds::default());
        assert!(app.diff_copy_of(worker).is_some());
        app.on_key(key(KeyCode::Esc));
        // Another patch starts open.
        k(&mut app, 'h');
        assert!(app.diff_folds_of(worker).folded(1));
        let c = app.world.change_of(worker).unwrap().clone();
        app.world.change_ready(Change {
            patch: format!("{}+four\n", c.patch),
            ..c
        });
        assert_eq!(app.diff_folds_of(worker), Folds::default());
        // A conversation over the change takes the fold keys.
        assert_eq!(app.diff_zoom(), Some(worker));
        app.thread = Some(Thread {
            entries: vec![0],
            sel: 0,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
            copy: None,
            words: None,
            folds: Folds::default(),
            room: Cell::new(usize::MAX),
            about: None,
        });
        assert_eq!(app.diff_zoom(), None);
        app.command("fold");
        assert!(app.thread.as_ref().unwrap().folds.all, "the conversation's");
        assert!(!app.diff_folds_of(worker).folded(0));
    }

    #[test]
    fn a_change_pages_by_whole_and_half_screens() {
        let patch: String = (0..30).map(|i| format!("+line {i}\n")).collect();
        let (mut app, _) = with_change(&patch);
        app.diff_page.set(10);
        app.on_key(ctrl('d'));
        assert_eq!(app.diff_scroll, 5, "half a page down");
        app.on_key(ctrl('f'));
        assert_eq!(app.diff_scroll, 15, "a whole page down");
        app.on_key(ctrl('u'));
        assert_eq!(app.diff_scroll, 10, "half a page up");
        app.on_key(ctrl('b'));
        assert_eq!(app.diff_scroll, 0, "a whole page up stops at the top");
        for _ in 0..5 {
            app.on_key(ctrl('f'));
        }
        assert_eq!(app.diff_scroll, 29, "paging stops at the last line");
    }

    #[test]
    fn bracket_keys_jump_between_file_headers() {
        let (mut app, _) = with_change(&two_files());
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(
            app.diff_scroll, 4,
            "from a header, ] goes to the next file, not itself"
        );
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.diff_scroll, 4, "no file after the last one");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(
            app.diff_scroll, 4,
            "[ goes back to the current file's header"
        );
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.diff_scroll, 0, "and then to the one before");
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.diff_scroll, 0, "no file before the first one");
    }

    #[test]
    fn a_and_r_decide_the_open_change_and_close_it() {
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        for c in ['a', 'r'] {
            let (mut app, worker) = with_change(&two_files());
            let name = app.world.agents[worker].name.clone();
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            assert_eq!(app.pane, Pane::Inbox, "{c} returns to the inbox");
            if c == 'a' {
                // The demo applies it as git's thread does: the next step
                // says how it went.
                assert_eq!(flash(&app), Some(fill(t.applying, &[&name])));
                assert!(app.world.change_of(worker).is_some() && !app.typing);
                app.on_key(key(KeyCode::Char('a')));
                assert_eq!(
                    flash(&app),
                    Some(fill(
                        t.apply_failed,
                        &[&name, "their change is being applied"]
                    )),
                    "once is enough"
                );
                app.step();
                assert!(app.world.journal.iter().any(|e| matches!(
                    e.event,
                    Event::Applied { agent, .. } | Event::ApplyFailed { agent, .. } if agent == worker
                )));
            } else {
                assert_eq!(flash(&app), Some(fill(t.rejected_change, &[&name])));
                assert!(app.world.change_of(worker).is_none());
                assert!(app.typing, "the composer opens for the reasons");
                assert_eq!(app.input, format!("@{name} "));
            }
        }
    }

    #[test]
    fn a_and_r_on_a_live_change_say_what_became_of_it() {
        use crate::live::testing::{as_if_applying, hold_change, idle_live, team};
        let t = Lang::En.text();
        let app_with = |live: crate::live::Live| {
            let mut w = team(1);
            w.change_ready(Change {
                agent: 1,
                title: "did it".into(),
                files: Vec::new(),
                patch: String::new(),
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            });
            let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
            app.zoom = Zoom::Diff(1);
            app.pane = Pane::Inbox;
            app
        };
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        // Nothing imece holds: refused, with why.
        let mut app = app_with(idle_live(2));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(
            flash(&app),
            Some(fill(
                t.apply_failed,
                &["claude-1", "no change of theirs waits"]
            ))
        );
        // A change it holds: git applies it, the journal will say.
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut app = app_with(live);
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.applying, &["claude-1"])));
        // Rejected while being applied: refused, the change still waits,
        // no composer.
        let mut live = idle_live(2);
        as_if_applying(&mut live, 1);
        let mut app = app_with(live);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(
            flash(&app),
            Some(fill(
                t.reject_failed,
                &["claude-1", "their change is being applied"]
            ))
        );
        assert!(app.world.change_of(1).is_some() && !app.typing);
        // Rejected: the composer opens to the worker for the reasons.
        let mut app = app_with(idle_live(2));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.rejected_change, &["claude-1"])));
        assert!(app.world.change_of(1).is_none());
        assert!(app.typing);
        assert_eq!(app.input, "@claude-1 ");
    }

    /// The inbox selection may have moved while the change was open (an
    /// earlier item closed, a new one came): `a` and `r` decide the change
    /// in view, not the item the selection landed on.
    #[test]
    fn a_and_r_decide_the_change_in_view_not_the_selected_inbox_item() {
        for c in ['a', 'r'] {
            let (mut app, worker) = with_change(&two_files());
            let leader = app.world.projects[0].leader;
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: leader,
                detail: "bump a dependency".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
            app.inbox_sel = 1;
            let name = app.world.agents[worker].name.clone();
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            let flash = app.flash.as_ref().map(|(m, _)| m.clone());
            let said = if c == 'a' {
                Lang::En.text().applying
            } else {
                Lang::En.text().rejected_change
            };
            assert_eq!(flash, Some(fill(said, &[&name])));
            let other = app
                .world
                .inbox
                .iter()
                .find(|i| i.kind == InboxKind::Sensitive);
            assert!(
                other.is_some_and(|i| i.open),
                "{c} must not decide the item the selection moved to"
            );
        }
    }

    #[test]
    fn esc_leaves_the_change_open_in_the_inbox() {
        let (mut app, worker) = with_change(&two_files());
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.diff_scroll, 8);
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(app.pane, Pane::Inbox);
        assert!(app.world.change_of(worker).is_some());
        assert!(app.flash.is_none());
    }

    #[test]
    fn a_commit_line_quotes_the_message_for_the_shell() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(sh_quote("$(rm -rf ~) `x`"), "'$(rm -rf ~) `x`'");
        assert_eq!(
            commit_line("Fix it\n\nWhy", None, false),
            "!git commit -m 'Fix it\n\nWhy'"
        );
        assert_eq!(
            commit_line("A", Some(Path::new("/w/it's")), true),
            r"!git -C '/w/it'\''s' commit --dry-run -m 'A'"
        );
    }

    #[test]
    fn a_suggestion_opens_the_command_line_once_it_is_free() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let suggestion = Suggestion {
            project: 0,
            text: "Fix it".into(),
        };
        app.world.suggested = Some(suggestion.clone());
        app.typing = true;
        app.input = "@lead hi".into();
        app.show_suggestion();
        assert_eq!(app.input, "@lead hi", "not over what the person writes");
        assert_eq!(app.world.suggested, Some(suggestion));
        app.typing = false;
        app.input.clear();
        app.after = 3;
        app.show_suggestion();
        // The demo's message is made up: it only tries.
        assert_eq!(app.input, "!git commit --dry-run -m 'Fix it'");
        assert!(app.typing);
        assert_eq!(app.after, 0, "the cursor at the end");
        assert!(app.menu.is_none());
        assert_eq!(app.world.suggested, None);
        // Nothing waits: nothing opens.
        app.typing = false;
        app.input.clear();
        app.show_suggestion();
        assert!(!app.typing);
        assert!(app.input.is_empty());
    }

    #[test]
    fn a_live_suggestion_commits_in_its_own_projects_checkout() {
        let one = crate::live::testing::checkout("sugg-one");
        let two = crate::live::testing::checkout("sugg-two");
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live_in(&one, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.command(&format!("open {} two", two.display()));
        app.selected = Sel::Agent(0);
        app.typing = false;
        app.world.suggested = Some(Suggestion {
            project: 0,
            text: "Here".into(),
        });
        app.show_suggestion();
        assert_eq!(app.input, "!git commit -m 'Here'");
        app.typing = false;
        app.world.suggested = Some(Suggestion {
            project: 1,
            text: "There".into(),
        });
        app.show_suggestion();
        let top = imece_agent::worktree::top(&two).unwrap();
        assert_eq!(
            app.input,
            format!(
                "!git -C {} commit -m 'There'",
                sh_quote(&top.to_string_lossy())
            )
        );
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn colon_checks_says_and_sets_what_the_yes_asks_lonca_for_on_lonca_and_for_a_card() {
        let (mut app, worker) = with_change("+x\n");
        let flash = |app: &App| {
            app.flash
                .as_ref()
                .map(|(m, _)| m.clone())
                .unwrap_or_default()
        };
        let name = app.world.agents[worker].name.clone();
        let project = app.world.projects[app.world.agents[worker].project]
            .name
            .clone();
        // Off Lonca: refused, nothing would run them.
        app.command("checks api");
        assert_eq!(
            flash(&app),
            format!(
                "the checks stay as they were: {project} is not on Lonca, and only Lonca's \
                 merge queue runs a card's checks"
            )
        );
        app.world.on_lonca(0, Some("demo".into()));
        // A change made for no card: refused.
        app.world.agents[worker].card = None;
        app.command("checks api");
        assert_eq!(
            flash(&app),
            format!("the checks stay as they were: {name}'s change was made for no card")
        );
        assert_eq!(app.command_note("checks"), "");
        // Its card: bare says them, names set them sorted, each once.
        let c = app.world.cards.len();
        app.world.cards.push(Card {
            id: c,
            project: app.world.agents[worker].project,
            title: "t".into(),
            owner: Some(worker),
            column: Column::Review,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        });
        app.world.agents[worker].card = Some(c);
        app.command("checks");
        assert_eq!(
            flash(&app),
            format!("the yes to {name}'s change asks Lonca for none of the card's own checks")
        );
        assert_eq!(app.command_note("checks"), "(none)");
        app.command("checks unit, api api");
        assert_eq!(app.world.cards[c].checks, ["api", "unit"]);
        assert_eq!(
            flash(&app),
            format!("the yes to {name}'s change also asks Lonca for api, unit")
        );
        assert_eq!(app.command_note("checks"), "(api, unit)");
        assert_eq!(app.world.asked_checks(worker), ["api", "unit"]);
        let you = Event::ChecksSet {
            agent: worker,
            card: c,
            checks: vec!["api".into(), "unit".into()],
            you: true,
        };
        assert!(app.world.journal.iter().any(|l| l.event == you));
        assert!(matches!(
            app.world.cards[c].story.last().map(|s| &s.event),
            Some(StoryEvent::ChecksSet { by: None, .. })
        ));
        // A name Lonca would not take: refused whole, the list kept.
        for bad in ["api Build", "api build", "a/b"] {
            app.command(&format!("checks {bad}"));
            assert!(
                flash(&app).starts_with("the checks stay as they were: "),
                "{bad}"
            );
            assert_eq!(app.world.cards[c].checks, ["api", "unit"], "{bad}");
        }
        assert!(flash(&app).ends_with(
            "a/b is not a check's name: a-z, 0-9, _ and -, at most 32, and not one of Lonca's own"
        ));
        let most = imece_agent::lonca::CHECKS_MOST;
        let many = |n: usize| {
            (0..n)
                .map(|i| format!("c{i:02}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        app.command(&format!("checks {}", many(most + 1)));
        assert_eq!(
            flash(&app),
            format!("the checks stay as they were: a card asks for at most {most} checks")
        );
        app.command(&format!("checks {}", many(most)));
        assert_eq!(app.world.cards[c].checks.len(), most);
        // `none` takes them all away.
        app.command("checks none");
        assert!(app.world.cards[c].checks.is_empty());
        // Once Lonca says what its policy has, a name it lacks is refused
        // with the names there are, and the nearest when one is lacking.
        let names = |n: &[&str]| Some(n.iter().map(|c| c.to_string()).collect());
        app.world.store_checks(0, "demo", names(&["api", "e2e"]));
        app.command("checks e2e apii");
        assert_eq!(
            flash(&app),
            "the checks stay as they were: Lonca's policy has no apii; it has api, e2e · did \
             you mean api?"
        );
        app.command("checks zzzz e2");
        assert_eq!(
            flash(&app),
            "the checks stay as they were: Lonca's policy has no e2, zzzz; it has api, e2e"
        );
        assert!(app.world.cards[c].checks.is_empty());
        app.command("checks e2e");
        assert_eq!(app.world.cards[c].checks, ["e2e"]);
        app.command("checks none");
        assert!(app.world.cards[c].checks.is_empty(), "none is no name");
        app.world.store_checks(0, "demo", names(&[]));
        app.command("checks api");
        assert_eq!(
            flash(&app),
            "the checks stay as they were: Lonca's policy has no api; it names no checks"
        );
        app.lang = Lang::Tr;
        app.world.store_checks(0, "demo", names(&["api"]));
        app.command("checks apx");
        assert_eq!(
            flash(&app),
            "denetimler olduğu gibi kaldı: Lonca'nın politikasında apx yok; olanlar: api · api \
             mi demek istedin?"
        );
        app.lang = Lang::En;
        // The menu after `:checks ` offers the policy's names.
        app.world.store_checks(0, "demo", names(&["api", "e2e"]));
        app.input = ":checks api ".into();
        let m = app.suggest().expect("a menu");
        assert_eq!(
            (m.items.as_slice(), m.start),
            (&["e2e".to_string()][..], 12)
        );
        app.input = ":checks ".into();
        assert_eq!(app.suggest().unwrap().items, ["none", "api", "e2e"]);
        app.world.store_checks(0, "demo", None);
        assert!(app.suggest().is_none(), "Lonca has not said");
        app.input.clear();
        // No change at hand: refused.
        app.zoom = Zoom::None;
        app.world.inbox.clear();
        app.command("checks api");
        assert_eq!(
            flash(&app),
            "the checks stay as they were: no change at hand: open one, or select it on Home"
        );
        assert!(app.world.cards[c].checks.is_empty());
    }

    #[test]
    fn colon_review_turns_on_and_off_and_asks_for_the_change_at_hand() {
        let (mut app, worker) = with_change("+x\n");
        let flash = |app: &App| {
            app.flash
                .as_ref()
                .map(|(m, _)| m.clone())
                .unwrap_or_default()
        };
        let t = Lang::En.text();
        let name = app.world.agents[worker].name.clone();
        assert!(!app.review, "off by default");
        assert_eq!(app.command_note("review"), "(off) $");
        app.command("review on");
        assert!(app.review);
        assert_eq!(flash(&app), t.review_on);
        assert_eq!(app.command_note("review"), "(on) $");
        app.command("review off");
        assert!(!app.review);
        assert_eq!(flash(&app), t.review_off);
        app.command("review maybe");
        assert!(!app.review);
        assert_eq!(flash(&app), "no review: on | off");
        // Bare, with the change open: asked now, whatever the setting.
        assert!(!app.reviewing(worker));
        app.command("review");
        assert!(app.reviewing(worker), "asked");
        assert_eq!(
            flash(&app),
            format!("claude reads {name}'s change for a review")
        );
        app.command("review");
        assert_eq!(
            flash(&app),
            format!("no review: a review of {name}'s change is under way")
        );
        // No change at hand: the setting is said.
        app.zoom = Zoom::None;
        app.world.inbox.clear();
        app.command("review on");
        app.command("review");
        assert_eq!(flash(&app), t.review_on);
        // The change selected in waiting for you is at hand too.
        let (mut app, worker) = with_change("+x\n");
        app.zoom = Zoom::None;
        app.at_home.at = Some(HomeRow::Item(app.inbox_rows()[0]));
        app.home_settle();
        app.command("review");
        assert!(app.reviewing(worker), "{}", flash(&app));
        // Not a change: nothing asked.
        let (mut app, worker) = with_change("+x\n");
        app.world.replied(worker, "hi");
        app.zoom = Zoom::None;
        let reply = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Reply)
            .unwrap();
        app.at_home.at = Some(HomeRow::Item(reply));
        app.home_settle();
        app.command("review");
        assert!(!app.reviewing(worker));
        assert_eq!(flash(&app), t.review_off);
    }

    #[test]
    fn review_on_asks_for_each_change_as_it_comes_and_journals_a_refusal() {
        let (mut app, worker) = with_change("+x\n");
        let name = app.world.agents[worker].name.clone();
        // What waited before it was on is not asked.
        app.step();
        app.review = true;
        app.step();
        assert!(!app.reviewing(worker));
        // A change that comes is.
        app.world.change_rejected(worker);
        app.world.change_ready(a_change(worker));
        app.step();
        assert!(app.reviewing(worker));
        // A newer one while that is asked: refused, journaled.
        app.world.change_ready(a_change(worker));
        app.step();
        assert!(
            app.world.journal.iter().any(|e| e.event
                == Event::ReviewFailed {
                    agent: worker,
                    why: format!("a review of {name}'s change is under way"),
                    cost: 0,
                }),
            "journaled"
        );
        // Off, a change that comes is not asked.
        let (mut app, worker) = with_change("+x\n");
        app.step();
        app.world.change_rejected(worker);
        app.world.change_ready(a_change(worker));
        app.step();
        assert!(!app.reviewing(worker));
        // A change gone before the step is not asked either.
        app.review = true;
        app.world.change_rejected(worker);
        app.world.change_ready(a_change(worker));
        app.world.change_rejected(worker);
        let lines = app.world.journal.len();
        app.watch_reviews();
        assert!(!app.reviewing(worker));
        assert_eq!(app.world.journal.len(), lines, "nothing refused");
    }

    #[test]
    fn capital_r_in_a_change_shows_its_review_whole_or_folded() {
        let (mut app, _) = with_change("+x\n");
        assert!(!app.review_whole);
        app.on_key(key(KeyCode::Char('R')));
        assert!(app.review_whole);
        app.on_key(key(KeyCode::Char('R')));
        assert!(!app.review_whole);
    }

    #[test]
    fn colon_commit_asks_and_says_so_or_why_not() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let name = app.world.projects[app.current_project()].name.clone();
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some(format!(
                "asking haiku for a commit message for what is staged in {name}"
            ))
        );
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some("no commit message: a suggestion is under way".into())
        );
        assert_eq!(app.command_note("commit"), "$");
        // Live without Claude Code: refused, said why.
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live(1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.world.budget = 0;
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some("no commit message: the pot has less than $0.05 left".into())
        );
    }

    #[test]
    fn c_writes_a_note_on_the_top_line_and_again_rewrites_or_takes_it_away() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let (mut app, worker) = with_change(patch);
        let t = app.lang.text();
        let press = |app: &mut App, c: char| app.on_key(key(KeyCode::Char(c)));
        // Off a hunk's lines: no note.
        app.diff_scroll = 3;
        press(&mut app, 'c');
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(flashed(&app).as_deref(), Some(t.note_where));
        // On one: written in the command line, kept on enter.
        app.diff_scroll = 5;
        press(&mut app, 'c');
        assert!(app.typing);
        assert_eq!(app.noting, Some(Noting::Line(worker, 5)));
        assert_eq!(app.menu, None);
        // A note is no command: `:` offers none.
        press(&mut app, ':');
        assert_eq!(app.menu, None, "no commands offered in a note");
        for c in "b is wrong".chars() {
            press(&mut app, c);
        }
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(
            flashed(&app).as_deref(),
            Some("note kept (1 on this change); n sends them")
        );
        let notes: Vec<(usize, String)> = (app.notes_of(worker).iter())
            .map(|n| (n.line, n.text.clone()))
            .collect();
        assert_eq!(notes, [(5, ":b is wrong".to_string())]);
        // Again on the same line: the note to write over.
        press(&mut app, 'c');
        assert_eq!(app.input, ":b is wrong");
        for _ in 0..11 {
            app.on_key(key(KeyCode::Backspace));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(app.notes_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.note_dropped));
        // Esc gives a note up.
        press(&mut app, 'c');
        press(&mut app, 'x');
        app.on_key(key(KeyCode::Esc));
        assert!(app.noting.is_none() && app.notes_of(worker).is_empty());
        // A note on another patch of the same worker, or on another
        // worker's, is not this one's; nor is one on another line.
        let here = patch_key(patch);
        let other = |agent, patch, line, text: &str| DiffNote {
            agent,
            patch,
            line,
            text: text.into(),
        };
        app.diff_notes.extend([
            other(worker, patch_key("older"), 5, "old"),
            other(0, here, 5, "not mine"),
            other(worker, here, 4, "above"),
        ]);
        let notes: Vec<&str> = (app.notes_of(worker).iter())
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(notes, ["above"]);
        press(&mut app, 'c');
        assert_eq!(app.input, "", "line 5 has none of its own");
        press(&mut app, 'y');
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.diff_notes.len(), 4, "kept beside the others");
        // Taking it away leaves the others.
        press(&mut app, 'c');
        assert_eq!(app.input, "y");
        app.on_key(key(KeyCode::Backspace));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.diff_notes.len(), 3);
        app.diff_notes.retain(|n| n.text != "above");
        press(&mut app, 'n');
        assert_eq!(flashed(&app).as_deref(), Some(t.no_notes));
        assert_eq!(app.zoom, Zoom::Diff(worker), "nothing sent, still there");
    }

    #[test]
    fn w_in_a_change_wraps_its_long_lines_and_cuts_them_again() {
        let (mut app, _) = with_change("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n");
        assert!(!app.diff_wrap);
        app.on_key(key(KeyCode::Char('w')));
        assert!(app.diff_wrap);
        assert_eq!(
            app.pane,
            Pane::Inbox,
            "w is the diff's here, not the panel's"
        );
        app.on_key(key(KeyCode::Char('w')));
        assert!(!app.diff_wrap);
    }

    #[test]
    fn x_chooses_the_hunk_at_hand_and_a_applies_only_the_chosen() {
        let patch =
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n@@ -9 +9 @@\n-c\n+d\n";
        let (mut app, worker) = with_change(patch);
        let t = app.lang.text();
        // On the header: the first hunk; inside the second: that one.
        app.diff_scroll = 0;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.chosen_of(worker), [3]);
        assert_eq!(
            flashed(&app).as_deref(),
            Some("1 of 2 hunks chosen; a applies only them, the rest waits")
        );
        app.diff_scroll = 8;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.chosen_of(worker), [3, 6]);
        // Again: no longer chosen.
        app.on_key(key(KeyCode::Char('x')));
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.chosen_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.unchosen));
        // Another change's or another worker's choice is not this one's.
        app.chosen
            .extend([(worker, patch_key("older"), 3), (0, patch_key(patch), 3)]);
        assert!(app.chosen_of(worker).is_empty());
        app.diff_scroll = 6;
        app.on_key(key(KeyCode::Char('x')));
        app.on_key(key(KeyCode::Char('a')));
        let Engine::Sim(s) = &app.engine else {
            panic!("the demo")
        };
        assert_eq!(s.applying_now(), [(worker, vec![6])]);
        // A change with no hunk: nothing to choose.
        let (mut app, worker) = with_change("diff --git a/b b/b\nBinary files differ\n");
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.chosen_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.no_hunk));
    }

    #[test]
    fn a_while_a_change_goes_in_says_so() {
        let mut app = App::new(1, 3, 0, 1, Lang::En);
        let workers: Vec<AgentId> = (app.world.agents.iter())
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect();
        let [one, two] = workers[..2] else {
            panic!("two workers")
        };
        for agent in [one, two] {
            app.world.change_ready(Change {
                agent,
                title: "did it".into(),
                files: Vec::new(),
                patch: "diff --git a/x b/x\n".into(),
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            });
        }
        let t = Lang::En.text();
        let names = |id: AgentId| app_name(&app, id);
        fn app_name(app: &App, id: AgentId) -> String {
            app.world.agents[id].name.clone()
        }
        let (first, second) = (names(one), names(two));
        assert!(!app.applying_besides(None));
        app.zoom = Zoom::Diff(one);
        app.pane = Pane::Inbox;
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app), Some(fill(t.applying, &[&first])));
        assert!(app.applying_besides(None) && app.applying_besides(Some(two)));
        assert!(!app.applying_besides(Some(one)), "its own is not another's");
        // The next one while the first goes in: after it.
        app.zoom = Zoom::Diff(two);
        app.pane = Pane::Inbox;
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app), Some(fill(t.applying_after, &[&second])));
        // Nothing at hand to approve meanwhile: said, not silent.
        for item in &mut app.world.inbox {
            item.open = false;
        }
        app.zoom = Zoom::None;
        app.thread = None;
        app.home_settle();
        app.flash = None;
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app).as_deref(), Some(t.apply_under_way));
    }

    #[test]
    fn one_note_sent_on_a_change_is_said_as_one() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let (mut app, worker) = with_change(patch);
        let name = app.world.agents[worker].name.clone();
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('c')));
        app.on_key(key(KeyCode::Char('y')));
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(
            flashed(&app),
            Some(format!(
                "1 note went to {name}; the change went back to them"
            ))
        );
    }

    #[test]
    fn n_sends_the_notes_in_one_message_and_the_change_goes_back() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let (mut app, worker) = with_change(patch);
        let name = app.world.agents[worker].name.clone();
        for (line, text) in [(6, "and here"), (4, "why")] {
            app.diff_scroll = line;
            app.on_key(key(KeyCode::Char('c')));
            for c in text.chars() {
                app.on_key(key(KeyCode::Char(c)));
            }
            app.on_key(key(KeyCode::Enter));
        }
        assert_eq!(app.notes_of(worker).len(), 2);
        app.on_key(key(KeyCode::Char('n')));
        assert!(app.world.change_of(worker).is_none(), "it went back");
        assert!(app.diff_notes.is_empty());
        assert_eq!((app.zoom, app.pane), (Zoom::None, Pane::Inbox));
        assert_eq!(
            flashed(&app),
            Some(format!(
                "2 notes went to {name}; the change went back to them"
            ))
        );
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == worker => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            said,
            Some(ui::notes_message(patch, &[(4, "why"), (6, "and here")]))
        );
    }

    /// `with_change` whose change git refused: `x` clashes with the checkout.
    fn with_clash() -> (App, AgentId, Clashes) {
        let (mut app, worker) = with_change("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n");
        let clashes = Clashes {
            head: "abc1234".into(),
            hunks: vec![Clash {
                file: "x".into(),
                hunk: Some("@@ -1 +1 @@".into()),
                why: "patch failed: x:1".into(),
            }],
            ..Clashes::default()
        };
        app.world.change_clashed(worker, clashes.clone());
        (app, worker, clashes)
    }

    #[test]
    fn b_sends_a_change_that_clashed_back_with_what_clashes() {
        let t = Lang::En.text();
        let (mut app, worker, clashes) = with_clash();
        let name = app.world.agents[worker].name.clone();
        app.on_key(key(KeyCode::Char('b')));
        assert!(app.world.change_of(worker).is_none(), "it went back");
        assert!(app.world.clashes.is_empty());
        assert_eq!((app.zoom, app.pane), (Zoom::None, Pane::Inbox));
        assert!(!app.typing, "the message is the reason: nothing to write");
        assert_eq!(flashed(&app), Some(fill(t.sent_back, &[&name])));
        assert!(
            app.world
                .journal
                .iter()
                .any(|e| matches!(e.event, Event::Rejected { agent } if agent == worker))
        );
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == worker => Some(text.clone()),
            _ => None,
        });
        assert_eq!(said, Some(ui::clash_message(&clashes, false)));
        // A solo is told as a solo.
        let (mut app, solo, clashes) = with_clash();
        app.world.agents[solo].role = Role::Solo;
        app.on_key(key(KeyCode::Char('b')));
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == solo => Some(text.clone()),
            _ => None,
        });
        assert_eq!(said, Some(ui::clash_message(&clashes, true)));
    }

    #[test]
    fn b_leaves_the_change_where_there_is_nothing_to_send_or_nobody_to_hear_it() {
        let t = Lang::En.text();
        let stays = |app: &App, worker: AgentId| {
            assert!(app.world.change_of(worker).is_some());
            assert_eq!(app.zoom, Zoom::Diff(worker));
            assert!(
                app.world
                    .journal
                    .iter()
                    .all(|e| !matches!(e.event, Event::Rejected { .. } | Event::YouSaid { .. }))
            );
        };
        // Git found no clash: nothing for a worker to resolve.
        let (mut app, worker) = with_change("+x\n");
        app.on_key(key(KeyCode::Char('b')));
        assert_eq!(flashed(&app).as_deref(), Some(t.no_clash));
        stays(&app, worker);
        // Stopped, or gone: said, and the change waits as it was.
        for gone in [false, true] {
            let (mut app, worker, _) = with_clash();
            let name = app.world.agents[worker].name.clone();
            match gone {
                true => app.world.agents[worker].gone = true,
                false => app.world.agents[worker].status = Status::Stopped,
            }
            app.on_key(key(KeyCode::Char('b')));
            assert_eq!(flashed(&app), Some(fill(t.back_gone, &[&name])));
            stays(&app, worker);
            assert!(app.world.clashes_of(worker).is_some());
        }
        // The pot is spent, to the last of it: no message would go.
        let (mut app, worker, _) = with_clash();
        let name = app.world.agents[worker].name.clone();
        app.world.budget = app.world.spent();
        app.on_key(key(KeyCode::Char('b')));
        assert_eq!(flashed(&app), Some(fill(t.budget_spent, &[&name])));
        stays(&app, worker);
        // One ten-thousandth left is enough.
        app.world.budget = app.world.spent() + 1;
        app.on_key(key(KeyCode::Char('b')));
        assert!(app.world.change_of(worker).is_none());
    }

    #[test]
    fn notes_stay_when_their_change_can_not_go_back_yet() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n";
        let (mut app, worker) = with_change(patch);
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('c')));
        app.on_key(key(KeyCode::Char('y')));
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('a')));
        app.zoom = Zoom::Diff(worker);
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.notes_of(worker).len(), 1, "being applied: it stays");
    }

    #[test]
    fn b_is_refused_while_the_change_is_being_applied_and_nothing_is_said() {
        let t = Lang::En.text();
        let (mut app, worker, _) = with_clash();
        let name = app.world.agents[worker].name.clone();
        app.on_key(key(KeyCode::Char('a')));
        app.zoom = Zoom::Diff(worker);
        app.on_key(key(KeyCode::Char('b')));
        assert_eq!(
            flashed(&app),
            Some(fill(
                t.reject_failed,
                &[&name, "their change is being applied"]
            ))
        );
        assert!(app.world.change_of(worker).is_some());
        assert!(
            !app.world
                .journal
                .iter()
                .any(|e| matches!(e.event, Event::YouSaid { .. }))
        );
    }

    #[test]
    fn quote_in_a_change_writes_to_its_worker_with_the_hunk_under_the_line() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n";
        let (mut app, worker) = with_change(patch);
        let name = app.world.agents[worker].name.clone();
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('>')));
        assert!(app.typing);
        let head = format!("@{name} ");
        let quoted = "```diff\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n```";
        assert_eq!(app.input, format!("{head}\n{quoted}"));
        assert_eq!(
            app.cursor(),
            head.len(),
            "on the first line, after the name"
        );
        assert_eq!(app.menu, None);
        for c in "why b".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing);
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == worker => Some(text.clone()),
            _ => None,
        });
        assert_eq!(said, Some(format!("why b\n{quoted}")));
        // No hunk to quote: it says so and nothing is written.
        let (mut app, _) = with_change("diff --git a/b b/b\nBinary files differ\n");
        app.on_key(key(KeyCode::Char('>')));
        assert!(!app.typing && app.input.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_hunk));
    }

    /// Home with the changes of workers 1 and 2 waiting, the first open.
    fn two_changes_at_home() -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        app.world.change_ready(a_change(2));
        app.zoom = Zoom::Diff(1);
        app.diff_scroll = 3;
        app
    }

    #[test]
    fn a_change_applied_from_home_opens_the_next_and_the_last_comes_home() {
        let mut app = two_changes_at_home();
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.zoom, Zoom::Diff(2));
        assert_eq!(app.diff_scroll, 0);
        let Some(HomeRow::Item(i)) = app.at_home.at else {
            panic!("an item at hand")
        };
        assert_eq!(app.world.inbox[i].agent, 2);
        // Nothing after it but what was just decided.
        app.world.inbox.retain(|i| i.agent == 2);
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!((app.zoom), (Zoom::None));
    }

    #[test]
    fn a_change_sent_back_opens_nothing_else() {
        let mut app = two_changes_at_home();
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(app.zoom, Zoom::None);
    }

    /// A demo with worker 1's proposal waiting and its view open.
    fn a_proposal_shown() -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        let proposal = StoreProposal {
            seq: 5,
            change: "c5".into(),
            why: Vec::new(),
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
        };
        app.world.change_proposed(a_change(1), proposal);
        app.zoom = Zoom::Diff(1);
        app
    }

    #[test]
    fn a_proposal_goes_whole() {
        let mut app = a_proposal_shown();
        app.world.changes[0].patch = "@@ -1 +1 @@\n+x\n".into();
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(
            flashed(&app),
            Some(app.lang.text().proposal_whole.to_string())
        );
        assert!(app.chosen.is_empty());
    }

    #[test]
    fn r_on_a_proposal_asks_why_and_rejects_nothing_yet() {
        let mut app = a_proposal_shown();
        let name = app.world.agents[1].name.clone();
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(app.rejecting, Some(1));
        assert!(app.typing);
        assert_eq!(app.input, format!("@{name} "));
        assert_eq!(app.after, app.input.chars().count());
        assert_eq!(app.zoom, Zoom::None);
        let t = app.lang.text();
        assert_eq!(flashed(&app), Some(fill(t.say_why, &[&name])));
        assert!(app.world.proposal_of(1).is_some());
    }

    #[test]
    fn leaving_the_line_rejects_nothing_and_says_so() {
        let mut app = a_proposal_shown();
        app.on_key(key(KeyCode::Char('r')));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.rejecting, None);
        assert_eq!(
            flashed(&app),
            Some(app.lang.text().not_rejected.to_string())
        );
        assert!(app.world.proposal_of(1).is_some());
        // Nothing to reject: the line's own esc says nothing.
        app.flash = None;
        app.typing = true;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(flashed(&app), None);
    }

    #[test]
    fn the_line_after_r_is_the_reason_and_the_message() {
        let mut app = a_proposal_shown();
        app.on_key(key(KeyCode::Char('r')));
        app.input.push_str("not this way");
        app.submit();
        assert_eq!(app.rejecting, None);
        assert!(app.world.proposal_of(1).is_none());
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Landed { how: Landed::Rejected(why), .. } if why == &["rejected: not this way"]
        )));
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::YouSaid { to: 1, text } if text == "not this way"
        )));
    }

    #[test]
    fn a_line_that_is_no_reason_for_it_rejects_nothing() {
        for line in ["@nobody why", ":salon", "hello"] {
            let mut app = a_proposal_shown();
            app.on_key(key(KeyCode::Char('r')));
            app.input = line.to_string();
            app.submit();
            assert_eq!(app.rejecting, None, "{line}");
            assert!(app.world.proposal_of(1).is_some(), "{line}");
        }
        let mut app = a_proposal_shown();
        app.on_key(key(KeyCode::Char('r')));
        app.submit();
        assert!(app.world.proposal_of(1).is_some(), "no words");
        assert_eq!(
            flashed(&app),
            Some(app.lang.text().not_rejected.to_string())
        );
    }

    #[test]
    fn a_rejection_refused_keeps_the_words_from_going() {
        let mut app = a_proposal_shown();
        app.on_key(key(KeyCode::Char('r')));
        app.input.push_str("not this way");
        let repo = "r".to_string();
        app.world.on_lonca(0, Some(repo.clone()));
        app.world
            .store_seen(0, &repo, StoreSeen::Down(StoreDown::NoDaemon));
        app.submit();
        assert!(app.world.proposal_of(1).is_some());
        let name = app.world.agents[1].name.clone();
        let t = app.lang.text();
        let why = StoreDown::NoDaemon.why();
        assert_eq!(flashed(&app), Some(fill(t.reject_failed, &[&name, why])));
        let said = (app.world.journal.iter()).any(|e| matches!(e.event, Event::YouSaid { .. }));
        assert!(!said);
    }

    #[test]
    fn a_is_the_approval_of_a_proposal() {
        let mut app = a_proposal_shown();
        let name = app.world.agents[1].name.clone();
        app.on_key(key(KeyCode::Char('a')));
        let t = app.lang.text();
        assert_eq!(flashed(&app), Some(fill(t.approving, &[&name])));
        let Engine::Sim(s) = &app.engine else {
            panic!("the demo");
        };
        assert_eq!(s.applying_now().len(), 1);
    }

    #[test]
    fn notes_sent_on_a_proposal_are_its_rejection() {
        let mut app = a_proposal_shown();
        assert!(app.goes_back(1, "see my notes", "sent".to_string()));
        assert!(app.world.proposal_of(1).is_none());
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Landed { how: Landed::Rejected(why), .. } if why == &["rejected: see my notes"]
        )));
    }

    #[test]
    fn a_live_rejection_goes_by_what_the_change_is() {
        use crate::live::testing::{idle_live, team};
        let proposal = StoreProposal {
            seq: 5,
            change: "c5".into(),
            why: Vec::new(),
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
        };
        let mut w = team(1);
        w.change_ready(a_change(1));
        let mut app = App::with(w, Engine::Live(Box::new(idle_live(2))), Lang::En);
        assert!(app.reject(1, ""), "a worktree's change needs no reason");
        assert!(app.world.change_of(1).is_none());

        let mut w = team(1);
        w.change_proposed(a_change(1), proposal);
        let mut app = App::with(w, Engine::Live(Box::new(idle_live(2))), Lang::En);
        // This project is on no repository, so only Lonca's way refuses it.
        assert!(!app.reject(1, "no"));
        assert!(app.world.proposal_of(1).is_some());
        let name = app.world.agents[1].name.clone();
        let why = "no proposal of theirs waits";
        let t = app.lang.text();
        assert_eq!(flashed(&app), Some(fill(t.reject_failed, &[&name, why])));
    }
}
