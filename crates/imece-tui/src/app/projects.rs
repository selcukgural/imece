//! Projects and their agents: open, solo, close, archive, restore, fork, forget, rename.

use super::*;

impl App {
    /// The project the person is looking at: of the selected line, the
    /// focused agent or the selected inbox item.
    pub fn current_project(&self) -> ProjectId {
        let p = self.project_at_hand();
        if self.world.projects[p].closed {
            // A closed project's journal lines still lead to it.
            self.world.open_projects().next().map_or(p, |q| q.id)
        } else {
            p
        }
    }

    /// The project the zoom, the inbox or journal selection, or the salon
    /// selection points at.
    pub(crate) fn project_at_hand(&self) -> ProjectId {
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return self.world.agents[id].project;
        }
        // Home narrowed to a project: that one, rows or none.
        if let Some(p) = self.at_home.scope {
            return p;
        }
        if self.pane == Pane::Inbox
            && let Some(&i) = self.inbox_rows().get(self.inbox_sel)
        {
            return self.world.agents[self.world.inbox[i].agent].project;
        }
        if self.pane == Pane::Journal
            && let Some(e) = self.journal_sel.and_then(|i| self.world.journal.get(i))
        {
            return self.world.agents[e.event.agent()].project;
        }
        match self.selected {
            Sel::Project(p) => p,
            Sel::Agent(a) => self.world.agents[a].project,
        }
    }

    /// `:pin [NAME]`: the named project (else the one at hand) is pinned
    /// to the top of the salon, or unpinned when it was.
    pub(super) fn pin(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = if arg.is_empty() {
            self.current_project()
        } else {
            match self.world.project_by_name(arg) {
                Some(p) => p,
                None => return self.flash(fill(t.no_project, &[arg])),
            }
        };
        let project = &mut self.world.projects[p];
        project.pinned = !project.pinned;
        let said = if project.pinned { t.pinned } else { t.unpinned };
        let said = fill(said, &[&project.name]);
        self.flash(said);
    }

    /// The agent a command's argument names, `@` or not; none named is
    /// the agent at hand. Nobody by that name is said.
    pub(super) fn agent_named(&mut self, arg: &str) -> Option<AgentId> {
        let word = arg.strip_prefix('@').unwrap_or(arg).trim();
        let id = if word.is_empty() {
            self.agent_at_hand()
        } else {
            self.world.agent_by_name(word)
        };
        if id.is_none() {
            let t = self.lang.text();
            self.flash(fill(t.no_agent, &[word]));
        }
        id
    }

    /// `:open PATH [NAME]`: the checkout at PATH becomes a project of its
    /// own, with its own leader, which is selected.
    pub(super) fn open_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let (path, name) = arg
            .split_once(char::is_whitespace)
            .map_or((arg, None), |(p, n)| (p, Some(n.trim())));
        if path.is_empty() {
            return self.flash(t.open_usage);
        }
        let full = expand_tilde(path, self.home.as_deref());
        let opened = match &mut self.engine {
            Engine::Live(live) => live.open_project(&mut self.world, &full, name, Open::Team),
            Engine::Sim(sim) => sim.open_project(&mut self.world, path, name),
            Engine::View(_) => return self.read_only(),
        };
        match opened {
            Ok(p) => {
                if p == self.collapsed.len() {
                    self.collapsed.push(false);
                } else {
                    self.collapsed[p] = false;
                }
                let project = &self.world.projects[p];
                let leader = project.leader;
                let said = fill(
                    t.opened,
                    &[
                        &project.name,
                        &self.shown_path(&full),
                        &self.world.agents[leader].name,
                    ],
                );
                self.zoom = Zoom::None;
                self.selected = Sel::Agent(leader);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(ProjectRefusal::PathOpen(n)) => self.flash(fill(t.project_open, &[path, &n])),
            Err(ProjectRefusal::BadName(n)) => self.flash(fill(t.bad_project_name, &[&n])),
            Err(e) => self.flash(fill(t.bad_open, &[&e.describe()])),
        }
    }

    /// `:solo [PATH] [claude[:MODEL][@EFFORT]] [NAME]`, the words in any
    /// order: a solo session on the checkout at PATH (else the project at
    /// hand's), on the choice, as a project named NAME (else after the
    /// directory, numbered when taken). A word with a `/`, or starting
    /// with `~` or `.`, is the path; one that starts with `claude` is the
    /// choice; the one left is the name. Without a choice, a screen asks
    /// the model and the effort first.
    /// Selected when opened.
    pub(super) fn solo_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let (mut path, mut choice, mut name) = (None, None, None);
        for word in arg.split_whitespace() {
            let slot = if word.contains('/') || word.starts_with(['~', '.']) {
                &mut path
            } else if let Some(c) = solo_choice(word) {
                if choice.replace(c).is_some() {
                    return self.flash(t.solo_usage);
                }
                continue;
            } else {
                &mut name
            };
            if slot.replace(word.to_string()).is_some() {
                return self.flash(t.solo_usage);
            }
        }
        match choice {
            Some(choice) => self.open_solo(path, name, choice),
            None => self.solo_pick = Some(SoloPick::new(path, name)),
        }
    }

    /// A key on the solo pick screen: a step answered opens the solo
    /// with what was picked; `q` or `esc` at the first step drops it.
    pub(super) fn solo_pick_key(&mut self, code: KeyCode) {
        let Some(pick) = &mut self.solo_pick else {
            return;
        };
        match pick.on_key(code) {
            None => {}
            Some(Picked::Nothing) => self.solo_pick = None,
            Some(Picked::Choice(choice)) => {
                let pick = self.solo_pick.take().expect("open");
                self.open_solo(pick.path, pick.name, choice);
            }
        }
    }

    /// Opens a solo on the checkout at `path` (none: the project at hand's)
    /// named `name`, on `choice`; selected when opened, the flash says
    /// what it runs on.
    fn open_solo(&mut self, path: Option<String>, name: Option<String>, choice: Option<String>) {
        let t = self.lang.text();
        let here = self.current_project();
        let full = match &path {
            Some(p) => expand_tilde(p, self.home.as_deref()),
            None => match &self.engine {
                Engine::Live(live) => live.repo_of(here).to_path_buf(),
                Engine::Sim(_) => PathBuf::from(&self.world.projects[here].name),
                Engine::View(_) => return self.read_only(),
            },
        };
        let shown = full.display().to_string();
        let name = name.as_deref();
        let opened = match &mut self.engine {
            Engine::Live(live) => live.open_project(
                &mut self.world,
                &full,
                name,
                Open::Solo {
                    choice: choice.clone(),
                },
            ),
            Engine::Sim(sim) => sim.open_solo(&mut self.world, &shown, choice, name),
            Engine::View(_) => return self.read_only(),
        };
        match opened {
            Ok(p) => {
                if p == self.collapsed.len() {
                    self.collapsed.push(false);
                } else {
                    self.collapsed[p] = false;
                }
                let project = &self.world.projects[p];
                let head = project.leader;
                let a = &self.world.agents[head];
                let runs = match self.model_of(head) {
                    Some(m) => format!("{CLAUDE} {m}"),
                    None => CLAUDE.to_string(),
                };
                let said = fill(t.solo_opened, &[&project.name, &shown, &a.name, &runs]);
                self.zoom = Zoom::None;
                self.selected = Sel::Agent(head);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(ProjectRefusal::BadName(n)) => self.flash(fill(t.bad_project_name, &[&n])),
            Err(e) => self.flash(fill(t.bad_open, &[&e.describe()])),
        }
    }

    /// The solo session the person looks at: the agent at hand when it is
    /// one, else the project at hand's head when that is one.
    pub(super) fn solo_at_hand(&self) -> Option<AgentId> {
        let w = &self.world;
        let at_hand = self
            .agent_at_hand()
            .filter(|&id| w.agents[id].role == Role::Solo);
        at_hand.or_else(|| {
            let p = self.current_project();
            w.is_solo(p).then(|| w.projects[p].leader)
        })
    }

    /// `:effort LEVEL` / `:model NAME`: the solo at hand runs on it from
    /// its next turn; `default` is Claude Code's own. The other part of its
    /// choice stays.
    pub(super) fn rechoose(&mut self, part: Part, arg: &str) {
        let t = self.lang.text();
        let word = arg.trim();
        if word.is_empty() {
            return self.flash(match part {
                Part::Effort => t.effort_usage,
                Part::Model => t.model_usage,
            });
        }
        let Some(id) = self.solo_at_hand() else {
            return self.flash(t.no_solo);
        };
        let name = self.world.agents[id].name.clone();
        let value = (word != complete::DEFAULT).then(|| word.to_string());
        if value
            .as_deref()
            .is_some_and(|v| !valid_model(v) || v.contains('@'))
        {
            return self.flash(fill(
                t.chose_refused,
                &[
                    &name,
                    &format!(
                        "{word:?} is not {}",
                        match part {
                            Part::Effort => "an effort level",
                            Part::Model => "a model name",
                        }
                    ),
                ],
            ));
        }
        let mut choice = Choice::parse(self.model_of(id).as_deref().unwrap_or_default());
        match part {
            Part::Effort => choice.effort = value,
            Part::Model => choice.model = value,
        }
        let choice = Some(choice.wire()).filter(|c| !c.is_empty());
        let changed = match &mut self.engine {
            Engine::Live(live) => live.rechoose(&mut self.world, id, choice),
            Engine::Sim(sim) => sim.rechoose(&mut self.world, id, choice),
            Engine::View(_) => return self.read_only(),
        };
        match changed {
            Ok(runs) => self.flash(fill(t.chose, &[&name, &runs])),
            Err(why) => self.flash(fill(t.chose_refused, &[&name, &why])),
        }
    }

    /// The menus of `:solo`, `:effort`, `:model` and `:checks`.
    pub(super) fn choice_menu(&self) -> Option<Menu> {
        let rest = self.input.strip_prefix(':')?;
        let (cmd, arg) = rest.split_once(' ')?;
        let word = arg.rsplit(' ').next().unwrap_or_default();
        let start = self.input.len() - word.len();
        match complete::long(cmd) {
            "solo" => complete::solo_words(
                start,
                word,
                self.home.as_deref(),
                live::efforts(),
                &complete::list_dirs,
            ),
            "effort" => {
                self.solo_at_hand()?;
                complete::choice_words(start, arg, live::efforts())
            }
            "model" => {
                self.solo_at_hand()?;
                complete::choice_words(start, arg, &live::aliases())
            }
            // The policy's checks, once Lonca says them (#390).
            "checks" => {
                let agent = self.change_at_hand()?;
                let known = self.world.policy_checks(self.world.agents[agent].project)?;
                complete::check_words(self.input.len() - arg.len(), arg, known)
            }
            _ => None,
        }
    }

    /// The `[MODEL][@EFFORT]` choice agent `id` runs on, none for its
    /// Claude Code's own default: a solo's row shows it.
    pub fn model_of(&self, id: AgentId) -> Option<String> {
        match &self.engine {
            Engine::Live(live) => live.model_of(id).map(str::to_string),
            Engine::Sim(sim) => sim.model_of(id).map(str::to_string),
            // What it ran on is not kept with an archive.
            Engine::View(_) => None,
        }
    }

    /// `:close [NAME]`: the named project (else the one at hand) closes:
    /// its agents leave, their worktrees stay. The last one stays open.
    pub(super) fn close_project(&mut self, arg: &str) {
        let t = self.lang.text();
        // One waits at a time: the one before acts now.
        self.end_grace();
        let p = if arg.is_empty() {
            self.current_project()
        } else {
            match self.world.project_by_name(arg) {
                Some(p) => p,
                None => return self.flash(fill(t.no_project, &[arg])),
            }
        };
        if self.world.open_projects().count() < 2 {
            let name = &self.world.projects[p].name;
            return self.flash(fill(t.last_project, &[name]));
        }
        self.wait_grace(Undoable::Close(p));
    }

    /// Project `p` closes: its agents leave, what pointed into it moves.
    pub(super) fn close_now(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let name = self.world.projects[p].name.clone();
        let closed = match &mut self.engine {
            Engine::Live(live) => live.close_project(&mut self.world, p),
            Engine::Sim(sim) => sim.close_project(&mut self.world, p),
            Engine::View(_) => return self.read_only(),
        };
        if closed.is_err() {
            return self.flash(fill(t.last_project, &[&name]));
        }
        if let Zoom::Focus(a) | Zoom::Trace(a) | Zoom::Diff(a) = self.zoom
            && self.world.agents[a].project == p
        {
            self.zoom = Zoom::None;
        }
        let was_in = match self.selected {
            Sel::Project(q) => q == p,
            Sel::Agent(a) => self.world.agents[a].project == p,
        };
        if was_in && let Some(home) = self.world.open_projects().next() {
            self.selected = Sel::Agent(home.leader);
        }
        if self.card.is_some_and(|c| self.world.cards[c].project == p) {
            self.card = None;
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
        self.flash(fill(t.closed, &[&name]));
    }

    /// `:archive [TITLE]`: the project at hand is put away as TITLE (else
    /// its name and days) after the grace, and a fresh one opens in its
    /// place.
    pub(super) fn archive_project(&mut self, arg: &str) {
        // One waits at a time: the one before acts now.
        self.end_grace();
        let p = self.current_project();
        let title: String = (arg.lines().next().unwrap_or_default().trim())
            .chars()
            .take(ARCHIVE_TITLE_MAX)
            .collect();
        self.archive_title = match title.is_empty() {
            true => self.world.archive_title(p),
            false => title,
        };
        self.wait_grace(Undoable::Archive(p));
    }

    /// Project `p` is archived now and a fresh one takes its place: what
    /// pointed into it moves to the fresh one.
    pub(super) fn archive_now(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let name = self.world.projects[p].name.clone();
        let title = std::mem::take(&mut self.archive_title);
        let archived = match &mut self.engine {
            Engine::Live(live) => live.archive_project(&mut self.world, p, title.clone()),
            Engine::Sim(sim) => sim.archive_project(&mut self.world, p, title.clone()),
            Engine::View(_) => return self.read_only(),
        };
        let fresh = match archived {
            Ok(fresh) => fresh,
            Err(why) => return self.flash(why.describe()),
        };
        self.collapsed
            .push(self.collapsed.get(p).copied().unwrap_or(false));
        self.put_away(p, fresh);
        self.flash(fill(t.archived, &[&name, &title]));
    }

    /// Project `p` went out of sight, archived or closed, and `to` is in
    /// its place: what the screen held of `p` moves to `to`'s head.
    fn put_away(&mut self, p: ProjectId, to: ProjectId) {
        // What the archive's agents left unseen waits in it.
        let w = &self.world;
        self.unseen.retain(|&a| w.agents[a].project != p);
        self.settled.retain(|&a| w.agents[a].project != p);
        if let Zoom::Focus(a) | Zoom::Trace(a) | Zoom::Diff(a) = self.zoom
            && self.world.agents[a].project == p
        {
            self.zoom = Zoom::None;
        }
        let was_in = match self.selected {
            Sel::Project(q) => q == p,
            Sel::Agent(a) => self.world.agents[a].project == p,
        };
        if was_in {
            self.selected = Sel::Agent(self.world.projects[to].leader);
        }
        // Home narrowed to it, or its row at hand: the fresh one's.
        if self.at_home.scope == Some(p) {
            self.at_home.scope = Some(to);
        }
        if self.at_home.at.is_some_and(|row| match row {
            HomeRow::Project(q) => q == p,
            HomeRow::Agent(a) => self.world.agents[a].project == p,
            HomeRow::Item(i) => self.world.agents[self.world.inbox[i].agent].project == p,
            HomeRow::Stop(q, _) => q == p,
            HomeRow::Card(c) => self.world.cards[c].project == p,
        }) {
            self.at_home.at = None;
        }
        if self.card.is_some_and(|c| self.world.cards[c].project == p) {
            self.card = None;
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// `:restore [NAME]`: the archive titled NAME (else the newest of the
    /// project called NAME; with no NAME, the newest of the project at
    /// hand's name) comes back, its head selected; the project open in
    /// its place gives way.
    pub(super) fn restore_project(&mut self, arg: &str) {
        let t = self.lang.text();
        // One waits at a time: the one before acts now.
        self.end_grace();
        let arg = arg.trim();
        let at = self.current_project();
        let Some(p) = self.world.archive_named(arg, at) else {
            return self.flash(if arg.is_empty() {
                fill(t.no_archive_of, &[&self.world.projects[at].name])
            } else {
                fill(t.no_archive, &[arg])
            });
        };
        self.restore_at(p);
    }

    /// Archive `p` comes back, its head selected; the project open in its
    /// place gives way.
    pub(super) fn restore_at(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let title = (self.world.projects[p].archived.as_ref())
            .map(|a| a.title.clone())
            .unwrap_or_default();
        let restored = match &mut self.engine {
            Engine::Live(live) => live.restore_project(&mut self.world, p),
            Engine::Sim(sim) => sim.restore_project(&mut self.world, p),
            Engine::View(_) => return self.read_only(),
        };
        let gave_way = match restored {
            Ok(q) => q,
            Err(why) => return self.flash(why.describe()),
        };
        if let Some(q) = gave_way {
            self.put_away(q, p);
        }
        self.selected = Sel::Agent(self.world.projects[p].leader);
        let name = &self.world.projects[p].name;
        self.flash(fill(t.restored, &[name, &title]));
    }

    /// `A`, `:archives`: the archives as a tree, the checkout at hand's
    /// first; nothing archived says so.
    pub(super) fn open_archives(&mut self) {
        // The view's other projects are put away only to hide them.
        if self.viewing_archive() {
            return self.read_only();
        }
        self.end_grace();
        if self.world.archives().is_empty() {
            return self.flash(self.lang.text().no_archives);
        }
        let at = self.current_project();
        let groups = archives::groups(&self.world, &|p| self.checkout_shown(p), Some(at));
        self.archives = Some(Browser::new(&self.world, groups));
    }

    /// Project `p`'s checkout as the archives browser writes it: live, its
    /// folder with `~` for the home; the demo's, the project's name (its
    /// stand-in for a checkout).
    fn checkout_shown(&self, p: ProjectId) -> String {
        match &self.engine {
            Engine::Live(live) => crate::model::tilde(live.repo_of(p), self.home.as_deref()),
            Engine::Sim(_) | Engine::View(_) => self.world.projects[p].name.clone(),
        }
    }

    /// `path` as the screen shows it: `~` for the home, cleaned.
    pub(crate) fn shown_path(&self, path: &Path) -> String {
        clean(&crate::model::tilde(path, self.home.as_deref()))
    }

    /// The folder project `p` is, as the salon's project line ends: live,
    /// its checkout (the folder itself without git); the demo's, the one
    /// `:open` or `:solo` was given, else `~/src/NAME`. None in an
    /// archive's view.
    pub fn project_folder(&self, p: ProjectId) -> Option<String> {
        match &self.engine {
            Engine::Live(live) => Some(self.shown_path(live.repo_of(p))),
            Engine::Sim(sim) => Some(match sim.folder_of(p) {
                Some(path) => clean(path),
                None => format!("~/src/{}", clean(&self.world.projects[p].name)),
            }),
            Engine::View(_) => None,
        }
    }

    /// The folder agent `id` works in, as its focus says it, and the
    /// checkout when that is a worktree of it: a leader works in the
    /// checkout, a worker, its helpers and a solo in their worktree, and
    /// all of them in the folder itself without git.
    pub fn folder_of(&self, id: AgentId) -> Option<(String, Option<String>)> {
        match &self.engine {
            Engine::Live(live) => {
                let (folder, of) = live.folder_of(&self.world, id);
                Some((self.shown_path(&folder), of.map(|c| self.shown_path(c))))
            }
            Engine::Sim(_) => {
                let a = &self.world.agents[id];
                let checkout = self.project_folder(a.project)?;
                if a.role == Role::Leader || self.world.projects[a.project].no_git {
                    return Some((checkout, None));
                }
                let owner = a.parent.map_or(a, |p| &self.world.agents[p]);
                let tree = match owner.role {
                    Role::Solo => format!("solo-{}", owner.project),
                    _ => clean(&owner.name),
                };
                let name = clean(&self.world.projects[a.project].name);
                let folder = format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c/{tree}");
                Some((folder, Some(checkout)))
            }
            Engine::View(_) => None,
        }
    }

    /// Agent `id`'s scratch directory (record 105) as the focus shows it;
    /// none for a leader. The demo's is where live would put it.
    pub fn scratch_of(&self, id: AgentId) -> Option<String> {
        match &self.engine {
            Engine::Live(live) => live
                .scratch_of(&self.world, id)
                .map(|d| self.shown_path(&d)),
            Engine::Sim(_) => {
                let a = &self.world.agents[id];
                let own = match a.role {
                    Role::Leader => return None,
                    Role::Solo => format!("solo-{}", a.project),
                    _ => clean(&a.name),
                };
                let name = clean(&self.world.projects[a.project].name);
                Some(format!(
                    "~/.local/state/imece/worktrees/{name}-5e1f0a2c-scratch/{own}"
                ))
            }
            Engine::View(_) => None,
        }
    }

    /// A key in the archives browser.
    pub(super) fn archives_key(&mut self, k: KeyEvent) {
        let Some(b) = &mut self.archives else {
            return;
        };
        match b.on_key(&self.world, k) {
            None => {}
            Some(archives::Act::Close) => self.archives = None,
            Some(archives::Act::View(p, agent)) => {
                self.archives = None;
                self.view_archive(p, agent);
            }
            Some(archives::Act::Restore(p)) => {
                self.archives = None;
                self.restore_at(p);
            }
        }
    }

    /// `:forget [@NAME]`: the agent (named, else the one at hand) forgets
    /// its session; its next message starts a new one, told where it left
    /// off.
    pub(super) fn forget_agent(&mut self, arg: &str) {
        // One waits at a time: the one before acts now.
        self.end_grace();
        let Some(id) = self.agent_named(arg) else {
            return;
        };
        let can = match &self.engine {
            Engine::Live(live) => live.can_forget(&self.world, id),
            Engine::Sim(sim) => sim.can_forget(&self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        match can {
            Ok(()) => self.wait_grace(Undoable::Forget(id)),
            Err(why) => self.forget_refused(id, why),
        }
    }

    /// `:fork [@NAME]` (record 94): a new worker goes on from the one
    /// named, else the one at hand, to try another way; its row selected
    /// and the line opened to it, to say what. Refused with why.
    pub(super) fn fork_agent(&mut self, arg: &str) {
        let Some(id) = self.agent_named(arg) else {
            return;
        };
        let forked = match &mut self.engine {
            Engine::Live(live) => live.fork(&mut self.world, id),
            Engine::Sim(sim) => sim.fork(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        let t = self.lang.text();
        let was = self.world.agents[id].name.clone();
        match forked {
            Ok(new) => {
                let name = self.world.agents[new].name.clone();
                self.go_agent(new);
                self.typing = true;
                self.noting = None;
                self.input = format!("@{name} ");
                self.after = 0;
                self.menu = None;
                self.flash(fill(t.forked, &[&name, &was]));
            }
            Err(why) => self.flash(fill(t.fork_refused, &[&was, &why])),
        }
    }

    /// Agent `id` forgets its session now.
    pub(super) fn forget_now(&mut self, id: AgentId) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let forgot = match &mut self.engine {
            Engine::Live(live) => live.forget_agent(&mut self.world, id),
            Engine::Sim(sim) => sim.forget(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        match forgot {
            Ok(()) => self.flash(fill(t.forgot, &[&name])),
            Err(why) => self.forget_refused(id, why),
        }
    }

    /// Why agent `id` cannot forget its session, said.
    fn forget_refused(&mut self, id: AgentId, why: Refusal) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        match why {
            Refusal::Command(why) => self.flash(fill(t.forget_refused, &[&name, &why])),
            why => self.flash(fill(t.forget_refused, &[&name, &format!("{why:?}")])),
        }
    }

    /// `:rename NAME`: the project at hand is called NAME from now on, and
    /// its leader `lead-NAME`.
    pub(super) fn rename_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let was = self.world.projects[p].name.clone();
        let renamed = match &mut self.engine {
            Engine::Live(live) => live.rename_project(&mut self.world, p, arg),
            Engine::Sim(sim) => sim.rename_project(&mut self.world, p, arg),
            Engine::View(_) => return self.read_only(),
        };
        match renamed {
            Ok(()) => {
                let leader = &self.world.agents[self.world.projects[p].leader].name;
                let said = fill(t.renamed, &[&was, arg, leader]);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(_) => self.flash(fill(t.bad_project_name, &[arg])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn the_journal_line_says_the_project_only_while_the_journal_is_at_hand() {
        let (mut app, [a, _, c]) = two_quiet_projects();
        app.world.said(c, "from the other project");
        app.journal_sel = Some(app.world.journal.len() - 1);
        app.selected = Sel::Agent(a);
        app.pane = Pane::Journal;
        assert_eq!(app.project_at_hand(), app.world.agents[c].project);
        app.pane = Pane::Salon;
        assert_eq!(app.project_at_hand(), app.world.agents[a].project);
    }

    #[test]
    fn the_folder_an_agent_works_in_is_its_checkout_or_its_worktree_with_home_as_tilde() {
        use crate::live::testing::{idle_live_in, team};
        let mut w = team(1);
        w.agents.push(Agent::new(
            2,
            0,
            "claude-1.1".into(),
            Role::Helper,
            Status::Working,
            0,
        ));
        w.agents[2].parent = Some(1);
        let live = idle_live_in(Path::new("/home/me/src/repo"), 3);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.home = Some(PathBuf::from("/home/me"));
        assert_eq!(app.project_folder(0).as_deref(), Some("~/src/repo"));
        assert_eq!(
            app.folder_of(0),
            Some(("~/src/repo".into(), None)),
            "the lead"
        );
        let tree = Some(("/nonexistent/claude-1".into(), Some("~/src/repo".into())));
        assert_eq!(app.folder_of(1), tree);
        assert_eq!(app.folder_of(2), tree, "a helper works in its worker's");
        // Not under the home: as it is; the home itself is `~`.
        app.home = Some(PathBuf::from("/home/other"));
        assert_eq!(app.project_folder(0).as_deref(), Some("/home/me/src/repo"));
        app.home = Some(PathBuf::from("/home/me/src/repo"));
        assert_eq!(app.project_folder(0).as_deref(), Some("~"));
        app.home = None;
        assert_eq!(app.project_folder(0).as_deref(), Some("/home/me/src/repo"));
        // Without git, everyone works in the folder itself.
        if let Engine::Live(l) = &mut app.engine {
            l.set_git(0, false);
        }
        assert_eq!(app.folder_of(1), Some(("/home/me/src/repo".into(), None)));
        // A control character in a path is never drawn.
        let live = idle_live_in(Path::new("/x/\x1b[31mred"), 1);
        let app = App::with(team(0), Engine::Live(Box::new(live)), Lang::En);
        assert!(!app.project_folder(0).unwrap().contains('\x1b'));
    }

    #[test]
    fn the_demo_says_the_folders_as_live_does() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let lead = app.world.projects[0].leader;
        let name = app.world.projects[0].name.clone();
        let checkout = format!("~/src/{name}");
        assert_eq!(app.project_folder(0), Some(checkout.clone()));
        assert_eq!(app.folder_of(lead), Some((checkout.clone(), None)));
        let worker = lead + 1;
        let worker_name = app.world.agents[worker].name.clone();
        assert_eq!(
            app.folder_of(worker),
            Some((
                format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c/{worker_name}"),
                Some(checkout.clone())
            ))
        );
        // A helper works in its worker's tree.
        app.world.agents[worker + 1].parent = Some(worker);
        assert_eq!(app.folder_of(worker + 1), app.folder_of(worker));
        // A project opened at a path is there.
        app.home = None;
        app.command("open ~/work/shop");
        let p = app.world.project_by_name("shop").unwrap();
        assert_eq!(app.project_folder(p).as_deref(), Some("~/work/shop"));
        // A solo's own tree; without git, the folder itself.
        app.command("solo ~/work/alone claude");
        let solo = app
            .world
            .agents
            .iter()
            .rfind(|a| a.role == Role::Solo)
            .unwrap()
            .id;
        let sp = app.world.agents[solo].project;
        assert_eq!(
            app.folder_of(solo),
            Some((
                format!(
                    "~/.local/state/imece/worktrees/{}-5e1f0a2c/solo-{sp}",
                    app.world.projects[sp].name
                ),
                Some("~/work/alone".into())
            ))
        );
        app.world.projects[sp].no_git = true;
        assert_eq!(app.folder_of(solo), Some(("~/work/alone".into(), None)));
    }

    #[test]
    fn fork_makes_a_new_worker_from_the_one_at_hand_and_opens_the_line_to_it() {
        let (mut app, w) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let id = w[0];
        // Numbered past their count: a fork is claude-5, counted, not
        // claude-1, the first name free; the rest are not workers.
        for (i, &o) in w.iter().enumerate() {
            app.world.agents[o].name = format!("claude-{}", i + 7);
        }
        let name = app.world.agents[id].name.clone();
        let lead = app.world.projects[0].leader;
        app.command(&format!("fork @{}", app.world.agents[lead].name));
        assert_eq!(
            said(&mut app),
            format!(
                "{} does not fork: only a worker forks: it has a conversation and a worktree \
                 of its own",
                app.world.agents[lead].name
            )
        );
        app.selected = Sel::Agent(id);
        assert_eq!(app.command_note("fork"), format!("({name}) $"));
        app.command("fork");
        assert_eq!(
            said(&mut app),
            format!("{name} does not fork: it has no session to fork yet: tell it something first")
        );
        let Engine::Sim(sim) = &mut app.engine else {
            unreachable!()
        };
        sim.told(&mut app.world, id, "make it fast").unwrap();
        app.command("fork");
        assert_eq!(
            said(&mut app),
            format!("{name} does not fork: it is mid-turn: fork it when its turn ends")
        );
        app.world.agents[id].status = Status::Idle;
        let before = app.world.agents.len();
        app.command("fork");
        assert_eq!(app.world.agents.len(), before + 1);
        let new = before;
        let fork = app.world.agents[new].name.clone();
        assert_eq!(fork, "claude-5");
        let a = &app.world.agents[new];
        assert_eq!(
            (a.role, a.project, a.status),
            (Role::Worker, 0, Status::Idle)
        );
        assert_eq!(
            said(&mut app),
            format!("{fork} is a fork of {name}: write what it should try")
        );
        assert!(app.typing && app.menu.is_none());
        assert_eq!(app.input, format!("@{fork} "));
        assert_eq!(app.selected, Sel::Agent(new));
        assert!(app.world.journal.iter().any(|e| e.event
            == Event::Forked {
                from: id,
                agent: new
            }));
        assert!(
            app.world.agents[new]
                .trace
                .back()
                .unwrap()
                .contains(&format!("forked from {name}"))
        );
        // What it is told goes on in its own session, as a copy.
        type_in(&mut app, "use a cache");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.world.agents[new].status, Status::Working);
        assert!(!app.world.journal.iter().any(|e| e.event
            == Event::Restarted {
                agent: new,
                why: Fresh::NoSession
            }));
    }

    #[test]
    fn colon_solo_places_its_words_opens_a_solo_and_selects_it() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // The words in any order: a path, a choice, a name.
        app.command("solo notes claude@high ~/x");
        assert_eq!(app.world.projects.len(), 2);
        let p = &app.world.projects[1];
        let head = p.leader;
        let a = &app.world.agents[head];
        assert_eq!(p.name, "notes");
        assert_eq!((a.name.as_str(), a.role), ("solo-notes", Role::Solo));
        assert_eq!(app.model_of(head).as_deref(), Some("@high"));
        assert_eq!(app.selected, Sel::Agent(head));
        assert_eq!(app.collapsed, [false, false]);
        assert_eq!(
            said(&mut app),
            "opened notes at ~/x as a solo session; write to solo-notes (claude @high)"
        );
        assert!(app.world.is_solo(1));
        // Typing goes to the solo now, as to a leader.
        assert_eq!(app.typing_target(), "solo-notes");
        // Bare: the pick screen asks the model and the effort;
        // the project at hand's checkout, named after it with a number (the
        // demo names its path after the project).
        app.command("solo");
        assert_eq!(app.world.projects.len(), 2, "nothing yet");
        let pick = app.solo_pick.as_ref().expect("the pick screen");
        assert_eq!((pick.path.as_deref(), pick.name.as_deref()), (None, None));
        for _ in 0..2 {
            app.on_key(key(KeyCode::Enter));
        }
        assert!(app.solo_pick.is_none());
        assert_eq!(app.world.projects.len(), 3);
        assert_eq!(app.world.projects[2].name, "notes-2");
        assert_eq!(app.model_of(app.world.projects[2].leader), None);
        assert_eq!(
            said(&mut app),
            "opened notes-2 at notes as a solo session; write to solo-notes-2 (claude)"
        );
        // A path is told by a `/` alone, a `~` alone or a `.` alone.
        for (n, (word, at)) in [("a/b", "a/b"), ("~", "~"), (".", ".")]
            .into_iter()
            .enumerate()
        {
            app.command(&format!("solo claude {word} p-{n}"));
            let flash = said(&mut app);
            assert!(
                flash.contains(&format!(" at {at} as a solo session")),
                "{word}: {flash}"
            );
        }
        assert_eq!(app.world.projects.len(), 6);
        // With a path and a name but no choice, they wait for the pick.
        app.command("solo ~/y ideas");
        let pick = app.solo_pick.as_ref().expect("the pick screen");
        assert_eq!(
            (pick.path.as_deref(), pick.name.as_deref()),
            (Some("~/y"), Some("ideas"))
        );
        app.on_key(key(KeyCode::Char('q')));
        assert!(
            app.solo_pick.is_none() && app.world.projects.len() == 6,
            "given up"
        );
        // Two names, two paths or two choices: the usage line, nothing opened.
        for bad in ["solo a b", "solo ~/x ./y", "solo claude claude@high"] {
            app.command(bad);
            assert_eq!(said(&mut app), t.solo_usage, "{bad}");
            assert_eq!(app.world.projects.len(), 6, "{bad}");
        }
        // A name that is taken, a name that can not be one.
        app.command("solo claude notes");
        assert_eq!(said(&mut app), fill(t.project_exists, &["notes"]));
        app.command("solo claude a:b");
        assert_eq!(said(&mut app), fill(t.bad_project_name, &["a:b"]));
        // `:solo` raises the next bill, as `:open` does.
        assert_eq!(app.command_note("solo"), "$");
    }

    #[test]
    fn the_menus_after_solo_effort_and_model_offer_what_the_engine_can_run() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let items = |app: &mut App, line: &str| -> Vec<String> {
            app.input = line.to_string();
            app.typing = true;
            app.suggest().map(|m| m.items).unwrap_or_default()
        };
        assert_eq!(items(&mut app, ":solo "), ["claude"]);
        assert_eq!(items(&mut app, ":solo cl"), ["claude"]);
        // The menu replaces the word being typed, where it starts.
        app.input = ":solo notes cl".into();
        assert_eq!(app.suggest().unwrap().start, 12);
        app.input = ":solo cl".into();
        assert_eq!(app.suggest().unwrap().start, 6);
        assert_eq!(
            items(&mut app, ":solo notes claude:opus@x"),
            ["claude:opus@xhigh"]
        );
        assert_eq!(
            items(&mut app, ":solo other@"),
            Vec::<String>::new(),
            "no such word"
        );
        // No solo at hand: the effort and model menus have nothing.
        assert!(items(&mut app, ":effort ").is_empty());
        assert!(items(&mut app, ":model ").is_empty());
        app.command("solo claude notes");
        assert_eq!(
            items(&mut app, ":effort "),
            ["default", "low", "medium", "high", "xhigh", "max"]
        );
        assert_eq!(items(&mut app, ":effort x"), ["xhigh"]);
        assert_eq!(
            items(&mut app, ":model "),
            ["default", "fable", "opus", "sonnet", "haiku"]
        );
        assert_eq!(items(&mut app, ":model s"), ["sonnet"]);
    }

    #[test]
    fn the_solo_pick_screen_walks_model_effort_and_composes_the_choice() {
        let mut p = SoloPick::new(Some("~/x".into()), None);
        assert_eq!((p.step, p.sel()), (PickStep::Model, 0));
        assert_eq!(p.items(), ["default", "fable", "opus", "sonnet", "haiku"]);
        assert_eq!(p.choice(), None, "default: Claude Code's own");
        // j k g G and digits move within the step; nothing past its ends.
        assert_eq!(p.on_key(KeyCode::Char('j')), None);
        assert_eq!(p.sel(), 1);
        p.on_key(KeyCode::Char('G'));
        assert_eq!(p.sel(), 4);
        p.on_key(KeyCode::Char('j'));
        assert_eq!(p.sel(), 4, "the last");
        p.on_key(KeyCode::Char('k'));
        assert_eq!(p.sel(), 3, "k goes up one");
        p.on_key(KeyCode::Up);
        assert_eq!(p.sel(), 2);
        p.on_key(KeyCode::Down);
        assert_eq!(p.sel(), 3);
        p.on_key(KeyCode::Char('g'));
        assert_eq!(p.sel(), 0, "g: the first");
        p.on_key(KeyCode::End);
        assert_eq!(p.sel(), 4);
        p.on_key(KeyCode::Home);
        assert_eq!(p.sel(), 0, "home: the first");
        p.on_key(KeyCode::Char('k'));
        assert_eq!(p.sel(), 0, "not past the first");
        p.on_key(KeyCode::Char('9'));
        assert_eq!(p.sel(), 0, "no ninth");
        p.on_key(KeyCode::Char('5'));
        assert_eq!(p.sel(), 4, "the fifth, the last");
        p.on_key(KeyCode::Char('3'));
        assert_eq!(p.choice().as_deref(), Some("opus"));
        // Then the effort step; the choice carries both.
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(p.step, PickStep::Effort);
        assert_eq!(
            p.items(),
            ["default", "low", "medium", "high", "xhigh", "max"]
        );
        p.on_key(KeyCode::Char('5'));
        assert_eq!(p.choice().as_deref(), Some("opus@xhigh"));
        // Esc goes back a step and keeps what was picked there.
        assert_eq!(p.on_key(KeyCode::Esc), None);
        assert_eq!((p.step, p.sel()), (PickStep::Model, 2));
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(
            p.on_key(KeyCode::Enter),
            Some(Picked::Choice(Some("opus@xhigh".into())))
        );
        // An effort alone, and neither.
        let mut p = SoloPick::new(None, None);
        p.on_key(KeyCode::Enter);
        p.on_key(KeyCode::Char('2'));
        assert_eq!(
            p.on_key(KeyCode::Enter),
            Some(Picked::Choice(Some("@low".into())))
        );
        let mut p = SoloPick::new(None, None);
        p.on_key(KeyCode::Enter);
        assert_eq!(p.on_key(KeyCode::Enter), Some(Picked::Choice(None)));
        // Given up: esc at the first step, q anywhere.
        let mut p = SoloPick::new(None, None);
        assert_eq!(p.on_key(KeyCode::Esc), Some(Picked::Nothing));
        let mut p = SoloPick::new(None, None);
        p.on_key(KeyCode::Enter);
        assert_eq!(p.on_key(KeyCode::Char('q')), Some(Picked::Nothing));
        assert_eq!(
            p.on_key(KeyCode::Char('x')),
            None,
            "an unknown key does nothing"
        );
    }

    #[test]
    fn colon_effort_and_colon_model_retune_the_solo_at_hand() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // No solo at hand: said so; nothing to do without a value.
        app.command("effort high");
        assert_eq!(said(&mut app), t.no_solo);
        app.command("solo claude:opus@high notes");
        said(&mut app);
        let head = app.world.projects[1].leader;
        assert_eq!(app.selected, Sel::Agent(head));
        app.command("effort");
        assert_eq!(said(&mut app), t.effort_usage);
        app.command("model");
        assert_eq!(said(&mut app), t.model_usage);
        // One part changes, the other stays.
        app.command("effort max");
        assert_eq!(
            said(&mut app),
            fill(t.chose, &["solo-notes", "claude opus@max"])
        );
        assert_eq!(app.model_of(head).as_deref(), Some("opus@max"));
        app.command("model sonnet");
        assert_eq!(app.model_of(head).as_deref(), Some("sonnet@max"));
        app.command("effort default");
        assert_eq!(app.model_of(head).as_deref(), Some("sonnet"));
        app.command("model default");
        assert_eq!(app.model_of(head), None);
        assert_eq!(said(&mut app), fill(t.chose, &["solo-notes", "claude"]));
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event, Event::Chose { agent, choice } if *agent == head && choice == "claude"
        )));
        // Refused: an effort Claude Code lacks, a word that is no model name,
        // a solo mid-turn.
        app.command("effort turbo");
        assert!(
            said(&mut app)
                .starts_with("could not change solo-notes: claude has no effort \"turbo\""),
        );
        app.command("model -x");
        assert!(said(&mut app).contains("is not a model name"));
        app.command("effort a@b");
        assert!(said(&mut app).contains("is not an effort level"));
        app.tell(head, "work");
        app.command("effort low");
        assert_eq!(
            said(&mut app),
            fill(
                t.chose_refused,
                &["solo-notes", "solo-notes is mid-turn; wait for its answer"]
            )
        );
        // The project at hand's solo counts when a line of its is at hand;
        // a team's agent never does.
        app.selected = Sel::Project(1);
        assert_eq!(app.solo_at_hand(), Some(head));
        app.selected = Sel::Project(0);
        assert_eq!(app.solo_at_hand(), None);
        app.command("model opus");
        assert_eq!(said(&mut app), t.no_solo);
        // The menu notes the part as it stands, each its own.
        app.selected = Sel::Agent(head);
        assert_eq!(app.command_note("effort"), "(default)");
        assert_eq!(app.command_note("model"), "(default)");
        app.command("solo claude:opus@xhigh other");
        assert_eq!(app.command_note("effort"), "(xhigh)");
        assert_eq!(app.command_note("model"), "(opus)");
        app.selected = Sel::Project(0);
        assert_eq!(app.command_note("effort"), "", "no solo at hand");
    }

    #[test]
    fn the_demo_opens_closes_and_renames_projects_too() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("open ~/work/shop");
        assert_eq!(app.world.projects.len(), 2);
        let leader = app.world.projects[1].leader;
        assert_eq!(app.world.agents[leader].name, "lead-shop");
        assert_eq!(app.selected, Sel::Agent(leader));
        assert_eq!(app.collapsed, [false, false]);
        assert_eq!(
            said(&mut app),
            "opened shop at ~/work/shop; write to lead-shop"
        );
        assert_eq!(
            app.cwd(),
            PathBuf::from("."),
            "the demo runs where it started"
        );
        app.command("rename store");
        assert_eq!(app.world.projects[1].name, "store");
        assert_eq!(said(&mut app), "shop is now store; write to lead-store");
        app.command("close store");
        assert!(!app.world.projects[1].closed, "not before its grace");
        app.end_grace();
        assert!(app.world.projects[1].closed);
        assert_eq!(app.selected, Sel::Agent(0));
        assert_eq!(
            said(&mut app),
            "closed store: its agents left; their worktrees stay on disk"
        );
        app.command("close");
        assert_eq!(
            said(&mut app),
            "shop-api is the last open project; q q quits"
        );
        assert!(!app.world.projects[0].closed);
        // :forget names an agent, @ or not, else takes the one at hand;
        // a demo agent has a session once it took a turn, as a live one.
        let t = Lang::En.text();
        app.command("forget nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        app.command("forget @nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        let lead = app.world.agents[0].name.clone();
        let none = fill(t.forget_refused, &[&lead, "it has no session to forget"]);
        app.command(&format!("forget @{lead}"));
        assert_eq!(said(&mut app), none);
        app.tell(0, "hello");
        app.command(&format!("f {lead}"));
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Forget(0)));
        app.end_grace();
        assert_eq!(said(&mut app), fill(t.forgot, &[&lead]));
        assert_eq!(app.world.agents[0].status, Status::Asleep);
        app.selected = Sel::Agent(0);
        app.command("forget");
        assert_eq!(said(&mut app), none, "forgotten already");
    }

    #[test]
    fn the_project_at_hand_falls_back_to_an_open_one_when_its_project_closed() {
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        app.selected = Sel::Project(1);
        assert_eq!(app.current_project(), 1);
        app.world.projects[1].closed = true;
        assert_eq!(app.current_project(), 0);
        app.selected = Sel::Agent(app.world.projects[1].leader);
        assert_eq!(app.current_project(), 0);
        app.world.projects[1].closed = false;
        app.world.projects[0].closed = true;
        app.selected = Sel::Project(0);
        assert_eq!(app.current_project(), 1, "the first open one");
        app.selected = Sel::Project(1);
        assert_eq!(app.current_project(), 1);
        // What waits in the inbox leads only while the inbox has the keys.
        app.world.projects[0].closed = false;
        app.world.replied(app.world.projects[0].leader, "an answer");
        assert_eq!(app.current_project(), 1, "the salon's selection");
        app.pane = Pane::Inbox;
        assert_eq!(app.current_project(), 0, "the inbox's selection");
    }

    #[test]
    fn colon_copy_lists_adds_and_drops_paths_of_the_project_at_hand() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        let p = app.current_project();
        let name = app.world.projects[p].name.clone();
        assert_eq!(app.command_note("copy"), "");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "nothing is copied into {name}'s worktrees; :copy PATH adds a path git ignores"
            )
        );
        app.command("copy .env");
        assert_eq!(
            flash(&app),
            format!(".env is copied into {name}'s worktrees that lack it, at a worker's start")
        );
        app.command("copy target/");
        assert_eq!(app.world.projects[p].copies, [".env", "target"]);
        assert_eq!(app.command_note("copy"), "(.env, target)");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!("copied into {name}'s worktrees: .env, target")
        );
        app.command("copy ../secret");
        assert!(flash(&app).starts_with("../secret can not be copied: a relative path"));
        app.command("copy .env");
        assert_eq!(flash(&app), ".env is no longer copied");
        assert_eq!(app.world.projects[p].copies, ["target"]);
    }

    #[test]
    fn colon_copy_says_so_when_the_checkout_has_a_worktreeinclude() {
        // The second demo project's checkout acts as if it had one.
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        app.selected = Sel::Agent(app.world.projects[1].leader);
        assert_eq!(app.current_project(), 1);
        let name = app.world.projects[1].name.clone();
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "{name}'s .worktreeinclude names what is copied into its worktrees; :copy PATH adds a path git ignores"
            )
        );
        app.command("copy .env");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!("copied into {name}'s worktrees: .env; and what its .worktreeinclude names")
        );
        app.lang = Lang::Tr;
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "{name} çalışma ağaçlarına kopyalananlar: .env; bir de .worktreeinclude'un saydıkları"
            )
        );
    }

    #[test]
    fn colon_copy_live_asks_git_whether_it_ignores_the_path() {
        let repo = crate::live::testing::checkout("app-copy");
        std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(repo.join(".env"), "KEY=1").unwrap();
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live_in(&repo, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.command("copy a.txt");
        let said = app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        assert!(said.contains("git does not ignore it"), "{said}");
        app.command("copy .env");
        assert_eq!(app.world.projects[0].copies, [".env"]);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_live_app_starts_on_the_leader_of_the_checkout_at_hand() {
        let one = crate::live::testing::checkout("home-one");
        let two = crate::live::testing::checkout("home-two");
        let mut w = crate::live::testing::team(0);
        let mut live = crate::live::testing::idle_live_in(&one, 1);
        live.open_project(&mut w, &two, Some("two"), Open::Team)
            .unwrap();
        live.set_home(1);
        let app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert_eq!(app.selected, Sel::Agent(1), "the second project's leader");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_live_archive_is_viewed_from_the_store_and_a_store_that_fails_says_so() {
        let mut w = crate::live::testing::team(1);
        w.note(Event::Failed { agent: 1 });
        w.projects[0].archived = Some(Archived {
            at: 3,
            title: "t".into(),
        });
        let mut live = crate::live::testing::idle_live(2);
        live.store_in_memory();
        live.persist(&w).unwrap();
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        // A line the store does not have is not the archive's.
        app.world.note(Event::Failed { agent: 0 });
        app.view_archive(0, None);
        let (lines, _) = app.world.journal_of(0);
        assert_eq!(view(&app).world.journal[..], lines[..lines.len() - 1]);
        app.viewing = None;
        let Engine::Live(l) = &mut app.engine else {
            unreachable!()
        };
        l.break_store();
        app.view_archive(0, None);
        assert!(app.viewing.is_none());
        assert!(
            said(&app).starts_with("the archive's journal could not be read: no such table"),
            "{}",
            said(&app)
        );
    }

    #[test]
    fn a_tilde_stands_for_home_in_a_typed_path() {
        let home = Some(Path::new("/home/me"));
        assert_eq!(expand_tilde("~", home), PathBuf::from("/home/me"));
        assert_eq!(expand_tilde("~/x/y", home), PathBuf::from("/home/me/x/y"));
        assert_eq!(expand_tilde("~x", home), PathBuf::from("~x"));
        assert_eq!(expand_tilde("/a/~", home), PathBuf::from("/a/~"));
        assert_eq!(expand_tilde("~", None), PathBuf::from("~"));
        assert_eq!(expand_tilde("~/x", None), PathBuf::from("~/x"));
    }

    #[test]
    fn enter_in_the_archives_opens_its_view_of_only_that_archive() {
        let app = viewing();
        let v = view(&app);
        let at = app.world.projects[1].archived.as_ref().unwrap().at;
        assert_eq!(
            v.viewed(),
            Some(&Viewed {
                project: 1,
                title: "epic".into(),
                at,
            })
        );
        assert!(app.archives.is_none(), "the browser gave way");
        let (lines, _) = app.world.journal_of(1);
        assert!(!lines.is_empty());
        assert_eq!(v.world.journal, lines, "its lines, and only those");
        assert!(!v.world.projects[1].closed && v.world.projects[1].archived.is_none());
        assert_eq!(v.selected, Sel::Agent(v.world.projects[1].leader));
        assert_eq!(v.lang, app.lang);
        // The screen behind goes on; the view does not move.
        let tick = v.world.tick;
        let mut app = app;
        app.step();
        app.step();
        assert!(app.world.tick > tick);
        assert_eq!(view(&app).world.tick, tick);
        // `enter` on one of its agents opens it with that agent selected.
        app.viewing = None;
        let worker = (app.world.agents.iter())
            .find(|a| a.project == 1 && a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        app.view_archive(1, Some(worker));
        assert_eq!(view(&app).selected, Sel::Agent(worker));
        // What is not an archive opens nothing.
        app.viewing = None;
        app.view_archive(0, None);
        assert!(app.viewing.is_none());
    }

    #[test]
    fn live_s_focus_says_the_scratch_directory_as_live_makes_it() {
        let mut w = crate::live::testing::team(1);
        w.projects[0].name = "repo".into();
        let live = crate::live::testing::idle_live(2);
        let app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert_eq!(app.scratch_of(0), None);
        let Engine::Live(l) = &app.engine else {
            unreachable!()
        };
        let made = l.scratch_of(&app.world, 1).unwrap();
        assert!(made.ends_with("claude-1"), "{}", made.display());
        assert_eq!(app.scratch_of(1), Some(made.display().to_string()));
    }

    #[test]
    fn closing_a_project_closes_its_card_s_screen() {
        let mut app = with_cards(2);
        let theirs = app.world.cards.iter().find(|c| c.project == 1).unwrap().id;
        let ours = app.world.cards.iter().find(|c| c.project == 0).unwrap().id;
        let other = app.world.projects[1].name.clone();
        app.open_card(theirs);
        app.command(&format!("close {other}"));
        app.end_grace();
        assert_eq!(app.card, None);
        // Another project closing leaves it open.
        app.home = None;
        app.command("open ~/x more");
        app.open_card(ours);
        app.command("close more");
        app.end_grace();
        assert_eq!(app.card, Some(ours));
    }
}
