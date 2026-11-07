//! The bus: messages between agents and to the person (`send`), and
//! the board (`post`, `board`, `claim`, the offers to idle workers).

use super::*;

impl Live {
    /// Agent `from` sends a message (the `send` tool's arguments). A
    /// worker's done from a Lonca workspace waits until what it holds is
    /// seen to be proposed (record 120); anything else goes on at once.
    pub(super) fn send(
        &mut self,
        w: &mut World,
        from: AgentId,
        args: &Value,
        reply: Sender<Answer>,
    ) {
        match self.gate_of(w, from, args) {
            Some((repo, tree)) => self.gate(from, args, reply, repo, tree),
            None => self.sent_on(w, from, args, reply),
        }
    }

    /// What became of the message goes back on `reply`. A worker's done
    /// shows the person what its worktree holds: that answer waits until
    /// git has read it.
    pub(super) fn sent_on(
        &mut self,
        w: &mut World,
        from: AgentId,
        args: &Value,
        reply: Sender<Answer>,
    ) {
        match self.message(w, from, args) {
            Ok((said, Some(title))) => self.snapshot(w, from, title, said, reply),
            said => {
                let _ = reply.send(said.map(|(said, _)| said));
            }
        }
    }

    /// What [`send`](Live::send) does before git: the answer, and the
    /// title of a worker's done whose change is to be read.
    fn message(
        &mut self,
        w: &mut World,
        from: AgentId,
        args: &Value,
    ) -> Result<(String, Option<String>), String> {
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let kind = MsgKind::from_wire(arg("type")).ok_or_else(|| {
            let kinds: Vec<&str> = MsgKind::ALL.iter().map(|k| k.wire()).collect();
            format!("type must be one of {}", kinds.join(", "))
        })?;
        let body = arg("body");
        if body.is_empty() {
            return Err("the body is empty".into());
        }
        if body.chars().count() > MAX_BODY {
            return Err(format!("the body is longer than {MAX_BODY} characters"));
        }
        let re = reply_to(&args["re"])?;
        let paths = paths::parse(&args["paths"])?;
        if !paths.is_empty() && kind != MsgKind::Assign {
            return Err("paths go with an assign: the card it opens owns them".into());
        }
        let options = options_of(&args["options"])?;
        if !options.is_empty() && (arg("to") != YOU || kind != MsgKind::Ask) {
            return Err("options go with an ask to the person (to: you)".into());
        }
        if arg("to") == YOU {
            if re.is_some() {
                return Err("re is for a teammate's message: the person's have no number".into());
            }
            return self.send_you(w, from, kind, body, options);
        }
        let project = w.agents[from].project;
        if w.agent_by_name(arg("to")).is_none()
            && let Some(group) = Group::parse(arg("to"))
        {
            if re.is_some() {
                return Err("re is for one teammate's message, not a group's".into());
            }
            return self.send_group(w, from, group, kind, body);
        }
        let to = w
            .agent_by_name(arg("to"))
            .filter(|&t| t != from && w.agents[t].project == project)
            .ok_or_else(|| {
                let team: Vec<&str> = w
                    .agents
                    .iter()
                    .filter(|a| a.id != from && a.project == project)
                    .map(|a| a.name.as_str())
                    .collect();
                format!(
                    "no teammate is called {:?}; the team is {}",
                    arg("to"),
                    team.join(", ")
                )
            })?;
        let name = w.agents[to].name.clone();
        if let Some(re) = re {
            replies(w, from, to, re)?;
        }
        self.owned_elsewhere(w, project, Some(to), &paths)?;
        if w.agents[to].status == Status::Stopped {
            return Err(format!(
                "{name} has stopped; it runs again when the person talks to it"
            ));
        }
        if kind.wakes() && self.over_budget(w) {
            return Err("the budget is spent; nobody takes new turns".into());
        }
        if kind.wakes() && self.woken[to] >= MAX_WOKEN {
            return Err(format!(
                "{name} took {MAX_WOKEN} turns on messages since the person last spoke; \
                 stop and tell the person what you need"
            ));
        }
        if self.mail[to].len() >= MAX_MAIL {
            return Err(format!(
                "{name} has {MAX_MAIL} unread messages; wait for it"
            ));
        }
        let id = w.number();
        self.mail[to].push(Mail {
            from,
            kind,
            body: body.to_string(),
            id,
            re,
            paths,
        });
        if kind.wakes() && w.agents[to].status == Status::Asleep {
            // Something new since the start: a sleeper takes it like any
            // idle agent (its older mail rides along).
            let t = w.tick;
            w.agents[to].set_status(Status::Idle, t);
            w.agents[to].trace(t, "woken by mail");
        }
        if kind == MsgKind::Done && w.agents[from].parent == Some(to) {
            // A helper's work is its worker's: done to it, and it leaves
            // when this turn ends.
            self.leaving[from] = true;
        }

        let t = w.tick;
        let first = body.lines().next().unwrap_or_default().to_string();
        let read =
            (kind == MsgKind::Done && w.agents[from].role == Role::Worker).then(|| first.clone());
        let story = StoryEvent::Sent {
            from,
            to,
            kind,
            text: body.to_string(),
            id,
            re,
        };
        let (mine, theirs) = (w.agents[from].card, w.agents[to].card);
        w.story(mine, story.clone());
        if theirs != mine {
            w.story(theirs, story);
        }
        w.note(Event::Sent {
            from,
            to,
            kind,
            text: first.clone(),
            id,
            re,
        });
        let sender = w.agents[from].name.clone();
        let numbers = numbers(id, re);
        w.agents[from].trace(t, format!("send to={name} type={}{numbers}", kind.wire()));
        w.agents[to].trace(
            t,
            format!("mail from={sender} type={}{numbers}", kind.wire()),
        );
        for (a, other) in [(from, to), (to, from)] {
            w.agents[a].talk = Some(Talk {
                with: Party::Agent(other),
                mine: a == from,
                text: first.clone(),
                until: t + 8,
            });
        }
        let held = kind != MsgKind::Cancel && w.waits_on_person(to);
        if held {
            w.agents[to].trace(t, format!("held until the person answers: message {id}"));
        }
        let delivered = match (kind.wakes(), held, self.turns[to] > 0) {
            (true, true, _) => format!(
                "held as message {id}; {name} waits on the person and gets it once they \
                 have answered, it starts nothing until then"
            ),
            (true, false, false) if read.is_some() => format!(
                "delivered as message {id}; {name} gets it when this turn of yours ends, \
                 headed with what it cost"
            ),
            (true, false, false) => {
                format!("delivered as message {id}; {name} starts on it now")
            }
            (true, false, true) => {
                format!("queued as message {id}; {name} is busy and gets it when its turn ends")
            }
            (false, _, _) => format!(
                "queued as message {id} for {name}'s next turn ({} does not wake it)",
                kind.wire()
            ),
        };
        Ok((delivered, read))
    }

    /// A message to the person (`to: you`). An `ask` waits in their inbox
    /// until they write back to the agent (which starts its next turn);
    /// the rest only goes into the journal and the conversation, and
    /// wakes nobody. A worker's `done` shows its change as any done does.
    /// Nothing here counts against the mail caps or the budget: the
    /// person reads for free.
    fn send_you(
        &mut self,
        w: &mut World,
        from: AgentId,
        kind: MsgKind,
        body: &str,
        options: Vec<String>,
    ) -> Result<(String, Option<String>), String> {
        if matches!(kind, MsgKind::Assign | MsgKind::Handoff | MsgKind::Cancel) {
            return Err(format!(
                "{} is not for the person; ask them, or tell them (inform, done, blocked)",
                kind.wire()
            ));
        }
        let t = w.tick;
        let first = body.lines().next().unwrap_or_default().to_string();
        let read =
            (kind == MsgKind::Done && w.agents[from].role == Role::Worker).then(|| first.clone());
        let card = w.agents[from].card;
        w.story(
            card,
            StoryEvent::ToYou {
                kind,
                text: body.to_string(),
            },
        );
        let offered = !options.is_empty();
        w.note(Event::ToYou {
            from,
            kind,
            text: body.to_string(),
            options,
        });
        if kind == MsgKind::Ask {
            w.asked_you(from, body);
        }
        let a = &mut w.agents[from];
        a.trace(t, format!("send to=you type={}", kind.wire()));
        a.talk = Some(Talk {
            with: Party::Person,
            mine: true,
            text: first,
            until: t + 8,
        });
        let answer = if kind == MsgKind::Ask && offered {
            "delivered; the person sees it under TO DECIDE with your options and \
             answers when they can, with one of them or in their own words, which \
             starts your next turn. Until then, end your turn or go on with what does \
             not depend on the answer; your teammates' messages wait for that answer \
             too, but for a cancel."
        } else if kind == MsgKind::Ask {
            "delivered; the person sees it under TO DECIDE and answers when they \
             can, which starts your next turn. Until then, end your turn or go on with \
             what does not depend on the answer; your teammates' messages wait for that \
             answer too, but for a cancel."
        } else {
            "noted; the person sees it in the journal. Nothing wakes them: a question \
             goes as type ask."
        };
        Ok((answer.to_string(), read))
    }

    /// Agent `from` puts a card on the board (the `post` tool's arguments).
    pub(super) fn post(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let (title, body) = (arg("title"), arg("body"));
        if title.is_empty() || body.is_empty() {
            return Err("a card needs a title and a body".into());
        }
        if title.contains('\n') || title.chars().count() > MAX_TITLE {
            return Err(format!(
                "the title must be one line of at most {MAX_TITLE} characters"
            ));
        }
        if body.chars().count() > MAX_BODY {
            return Err(format!("the body is longer than {MAX_BODY} characters"));
        }
        let paths = paths::parse(&args["paths"])?;
        let project = w.agents[from].project;
        self.owned_elsewhere(w, project, None, &paths)?;
        if self.open(w, project).count() >= MAX_OPEN {
            return Err(format!(
                "the board has {MAX_OPEN} open cards; wait until workers take some"
            ));
        }
        if self.posted[from] >= MAX_POSTS {
            return Err(format!(
                "you posted {MAX_POSTS} cards since the person last spoke; \
                 stop and tell the person what is left"
            ));
        }
        self.posted[from] += 1;
        let (t, card) = (w.tick, w.cards.len());
        w.cards.push(Card {
            id: card,
            project,
            title: title.to_string(),
            owner: None,
            column: Column::Waiting,
            story: Vec::new(),
            symbols: Vec::new(),
            paths,
            started: t,
            finished: 0,
        });
        self.posts.insert(
            card,
            Post {
                by: from,
                body: body.to_string(),
            },
        );
        w.story(
            Some(card),
            StoryEvent::Posted {
                by: from,
                text: body.to_string(),
            },
        );
        w.note(Event::Posted { agent: from, card });
        w.agents[from].trace(t, format!("post #{} {title:?}", card + 1));
        Ok(format!(
            "posted as #{}; an idle worker will be offered it",
            card + 1
        ))
    }

    /// The board of agent `from`'s project, for the `board` tool.
    pub(super) fn board(&self, w: &World, from: AgentId) -> String {
        let project = w.agents[from].project;
        let mut open = Vec::new();
        let mut taken = Vec::new();
        for c in w.cards.iter().filter(|c| c.project == project) {
            let Some(post) = self.posts.get(&c.id) else {
                continue;
            };
            let by = &w.agents[post.by].name;
            match (c.column, c.owner) {
                (Column::Waiting, _) => {
                    open.push(format!("#{} {} (posted by {by})", c.id + 1, c.title))
                }
                (Column::Done, Some(o)) => taken.push(format!(
                    "#{} {} (done by {})",
                    c.id + 1,
                    c.title,
                    w.agents[o].name
                )),
                (_, Some(o)) => taken.push(format!(
                    "#{} {} (taken by {})",
                    c.id + 1,
                    c.title,
                    w.agents[o].name
                )),
                (_, None) => {}
            }
        }
        if open.is_empty() && taken.is_empty() {
            return "the board is empty".into();
        }
        let mut out = String::new();
        if !open.is_empty() {
            out.push_str(&format!("Open:\n{}", open.join("\n")));
        }
        if !taken.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("Taken:\n{}", taken.join("\n")));
        }
        out
    }

    /// Worker `from` takes an open card (the `claim` tool's arguments). The
    /// answer is the card's body: the task.
    pub(super) fn claim(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        if w.agents[from].role != Role::Worker {
            return Err("only workers take cards; assign or post instead".into());
        }
        if let Some(c) = w.agents[from]
            .card
            .filter(|&c| w.cards[c].column != Column::Done)
        {
            return Err(format!(
                "finish #{} {} first; take one card at a time",
                c + 1,
                w.cards[c].title
            ));
        }
        let project = w.agents[from].project;
        let card = args["card"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok()?.checked_sub(1))
            .filter(|&c| {
                c < w.cards.len() && w.cards[c].project == project && self.posts.contains_key(&c)
            })
            .ok_or("there is no such card on the board; call board to see it")?;
        let c = &w.cards[card];
        if c.column != Column::Waiting {
            let who = c.owner.map_or("someone", |o| w.agents[o].name.as_str());
            return Err(format!("#{} was taken by {who}; pick another", card + 1));
        }
        let t = w.tick;
        let c = &mut w.cards[card];
        c.owner = Some(from);
        c.column = Column::Working;
        c.started = t;
        w.agents[from].card = Some(card);
        w.story(Some(card), StoryEvent::Took { agent: from });
        w.note(Event::Claimed { agent: from, card });
        w.agents[from].trace(t, format!("claim #{}", card + 1));
        self.offered.remove(&card);
        let post = &self.posts[&card];
        let by = &w.agents[post.by].name;
        Ok(format!(
            "#{} {} is yours. {by} posted it (written by that agent, not by the person):\n\n{}\n\n\
             When it is done, send {by} a done message.{}",
            card + 1,
            w.cards[card].title,
            post.body,
            owns(&w.cards[card].paths)
        ))
    }

    /// Refuses `paths` when an open card of `project` owns one of them
    /// (record 114): the one `to` holds now is not counted, since a card
    /// for `to` opens once that one is done.
    fn owned_elsewhere(
        &self,
        w: &World,
        project: ProjectId,
        to: Option<AgentId>,
        paths: &[Owned],
    ) -> Result<(), String> {
        for c in w.cards.iter().filter(|c| c.project == project) {
            if c.column == Column::Done || (c.owner.is_some() && c.owner == to) {
                continue;
            }
            if let Some((theirs, mine)) = paths::overlap(&c.paths, paths) {
                return Err(format!(
                    "#{} {} owns {theirs:?}, which {mine:?} names too; give this card \
                     other paths, or end one with the word shared ({mine} shared)",
                    c.id + 1,
                    c.title
                ));
            }
        }
        Ok(())
    }

    /// The open cards of `project`, oldest first.
    pub(super) fn open<'a>(
        &'a self,
        w: &'a World,
        project: ProjectId,
    ) -> impl Iterator<Item = CardId> + 'a {
        w.cards
            .iter()
            .filter(move |c| {
                c.project == project
                    && c.column == Column::Waiting
                    && self.posts.contains_key(&c.id)
            })
            .map(|c| c.id)
    }

    /// Offers open cards to idle workers: each card to one worker at a time,
    /// and to each worker at most once; an archived project's sleep on.
    pub(super) fn offer(&mut self, w: &mut World) {
        for id in 0..w.agents.len() {
            let a = &w.agents[id];
            if a.role != Role::Worker
                || a.gone
                || w.put_away(id)
                || a.status == Status::Stopped
                || self.turns[id] > 0
                || self.woken[id] >= MAX_WOKEN
                || self.over_budget(w)
                || w.waits_on_person(id)
            {
                continue;
            }
            // A sleeper is woken by a card posted since the start, not by
            // one that waited through it.
            let asleep = a.status == Status::Asleep;
            let cards: Vec<CardId> = self
                .open(w, a.project)
                .filter(|&c| !asleep || c >= self.loaded_cards)
                .filter(|c| {
                    let seen = self.offered.get(c).map_or(&[][..], Vec::as_slice);
                    // Not yet seen by this worker, and not in front of
                    // another one right now.
                    !seen.contains(&id) && !seen.iter().any(|&o| self.turns[o] > 0)
                })
                .collect();
            if cards.is_empty() {
                continue;
            }
            for &c in &cards {
                self.offered.entry(c).or_default().push(id);
            }
            self.woken[id] += 1;
            // The turn is about choosing; the card it takes is its card.
            w.agents[id].card = None;
            let listed: Vec<(CardId, AgentId)> =
                cards.iter().map(|c| (*c, self.posts[c].by)).collect();
            if let Err(why) = self.prompt(w, id, board_prompt(w, &listed), None) {
                apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// Starts the turns the mail is due: an agent free for one takes all
    /// its mail on a waking message; one that waits on the person only on
    /// a cancel, and the rest stays queued. What stays is written on its
    /// row.
    pub(super) fn post_office(&mut self, w: &mut World) {
        for id in 0..self.mail.len() {
            let waits = !self.mail[id].is_empty() && w.waits_on_person(id);
            let wakes = |m: &Mail| {
                if waits {
                    m.kind == MsgKind::Cancel
                } else {
                    m.kind.wakes()
                }
            };
            let due = self.mail[id].iter().any(wakes);
            // A worker's done says what its change cost once its turn
            // ended (record 113): the leader gets it then.
            let settling = (self.mail[id].iter()).any(|m| self.settling(w, m));
            let a = &w.agents[id];
            if due
                && !settling
                && self.turns[id] == 0
                && !matches!(a.status, Status::Stopped | Status::Asleep)
                && !a.gone
            {
                self.deliver(w, id);
            }
            w.agents[id].held_mail = if waits { self.mail[id].len() } else { 0 };
        }
    }

    /// Whether `m` is a worker's `done` whose turn still runs: its change
    /// is settled, and headed with its cost, at the turn's end.
    fn settling(&self, w: &World, m: &Mail) -> bool {
        let from = &w.agents[m.from];
        m.kind == MsgKind::Done && from.role == Role::Worker && !from.gone && self.turns[m.from] > 0
    }

    /// Starts agent `id`'s next turn with all its mail.
    pub(super) fn deliver(&mut self, w: &mut World, id: AgentId) {
        if self.over_budget(w) {
            return;
        }
        let mail = std::mem::take(&mut self.mail[id]);
        self.woken[id] += 1;
        let first = mail
            .iter()
            .find(|m| m.kind == MsgKind::Assign)
            .unwrap_or(&mail[0]);
        let title = first.body.lines().next().unwrap_or_default().to_string();
        let opening = Opening {
            title: &title,
            paths: &first.paths,
        };
        if let Err(why) = self.prompt(w, id, mail_prompt(w, &mail), Some(opening)) {
            apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
        }
    }
}

/// What a card's paths add to the task a worker is handed: nothing when
/// it owns none.
pub(super) fn owns(paths: &[Owned]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    format!(
        "\n\nThis card owns {}: stay in them; what else needs changing goes in your \
         done, or ask for the card to be widened.",
        paths::listed(paths)
    )
}

/// Whom a leader's message to a group goes to: every worker of its
/// project, or the idle ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Workers,
    Idle,
}

impl Group {
    /// The group `to` names: `workers` or `idle`; none for anything else.
    pub(super) fn parse(to: &str) -> Option<Group> {
        match to {
            "workers" => Some(Group::Workers),
            "idle" => Some(Group::Idle),
            _ => None,
        }
    }
}

impl Live {
    /// The leader `from` writes `body` (an inform or a notice) to every
    /// worker of `group` in its project, one message each, through the
    /// checks a message to one of them has; the answer names who got it
    /// and who did not, and why.
    fn send_group(
        &mut self,
        w: &mut World,
        from: AgentId,
        group: Group,
        kind: MsgKind,
        body: &str,
    ) -> Result<(String, Option<String>), String> {
        if w.agents[from].role != Role::Leader {
            return Err("only the leader writes to a group; write to one teammate".into());
        }
        if !matches!(kind, MsgKind::Inform | MsgKind::Notice) {
            return Err(
                "a group gets inform or notice only: give work to one worker with \
                        assign, or put it on the board with post"
                    .into(),
            );
        }
        let project = w.agents[from].project;
        let names: Vec<String> = (w.agents.iter())
            .filter(|a| a.project == project && a.role == Role::Worker && !a.gone)
            .filter(|a| match group {
                Group::Workers => true,
                Group::Idle => a.status == Status::Idle && self.turns[a.id] == 0,
            })
            .map(|a| a.name.clone())
            .collect();
        if names.is_empty() {
            return Err("no worker of yours is in that group".into());
        }
        let (mut got, mut not) = (Vec::new(), Vec::new());
        for name in names {
            let one = json!({ "to": name, "type": kind.wire(), "body": body });
            match self.message(w, from, &one) {
                Ok(_) => got.push(format!("{name} (message {})", w.messages)),
                Err(why) => not.push(format!("{name}: {why}")),
            }
        }
        if got.is_empty() {
            return Err(format!("sent to nobody: {}", not.join("; ")));
        }
        let mut said = format!("sent to {}", got.join(", "));
        if !not.is_empty() {
            said.push_str(&format!("; not to {}", not.join("; ")));
        }
        Ok((said, None))
    }
}

/// The answers an ask to the person offers (record 125): an array of
/// at most [`OPTIONS_MAX`] strings, each a line of at most [`OPTION_MAX`]
/// characters; none when absent.
fn options_of(v: &Value) -> Result<Vec<String>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(list) = v.as_array() else {
        return Err("options must be an array of strings".into());
    };
    if list.len() > OPTIONS_MAX {
        return Err(format!("an ask offers at most {OPTIONS_MAX} options"));
    }
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let Some(text) = item.as_str() else {
            return Err("options must be an array of strings".into());
        };
        let text = text.trim();
        if text.is_empty() {
            return Err("an option is empty".into());
        }
        if text.chars().count() > OPTION_MAX {
            return Err(format!("an option is longer than {OPTION_MAX} characters"));
        }
        if text.chars().any(|c| c.is_control()) {
            return Err(format!("{text:?} has a control character"));
        }
        out.push(text.to_string());
    }
    Ok(out)
}

/// The `send` tool's `re`: none when it is not given, else a message's
/// number.
fn reply_to(re: &Value) -> Result<Option<MsgId>, String> {
    if re.is_null() {
        return Ok(None);
    }
    re.as_u64()
        .and_then(|n| MsgId::try_from(n).ok())
        .filter(|&n| n > 0)
        .map(Some)
        .ok_or_else(|| "re must be the number of a message you got, as its heading shows".into())
}

/// Whether `from` may send `to` a reply to message `re`: one sent to
/// `from`, by `to`. A reply goes back to whoever wrote what it answers.
fn replies(w: &World, from: AgentId, to: AgentId, re: MsgId) -> Result<(), String> {
    let Some((asker, asked, _)) = w.message(re) else {
        return Err(format!("there is no message {re}"));
    };
    if asked != from {
        return Err(format!("message {re} was not sent to you"));
    }
    if asker != to {
        return Err(format!(
            "message {re} came from {}; send the reply to it",
            w.agents[asker].name
        ));
    }
    Ok(())
}

/// A message's numbers as a trace line ends with them (the demo's too).
pub(crate) fn numbers(id: MsgId, re: Option<MsgId>) -> String {
    match re {
        Some(re) => format!(" id={id} re={re}"),
        None => format!(" id={id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn the_leader_writes_an_inform_to_a_group_each_worker_its_own_message() {
        let mut w = team(3);
        let helper = with_helper(&mut w, 1);
        let mut live = idle_live(5);
        let send = |live: &mut Live, w: &mut World, from, to: &str, kind: &str| {
            call(
                live,
                w,
                from,
                "send",
                json!({ "to": to, "type": kind, "body": "main moved" }),
            )
        };
        let got = send(&mut live, &mut w, 0, "workers", "inform").unwrap();
        assert_eq!(
            got,
            "sent to claude-1 (message 1), claude-2 (message 2), claude-3 (message 3)"
        );
        for id in 1..=3 {
            assert_eq!(live.mail[id].len(), 1, "{id}");
            assert_eq!(live.mail[id][0].kind, MsgKind::Inform);
        }
        assert!(
            live.mail[0].is_empty() && live.mail[helper].is_empty(),
            "workers only"
        );
        let sent = (w.journal.iter())
            .filter(|e| matches!(e.event, Event::Sent { from: 0, .. }))
            .count();
        assert_eq!(sent, 3);
        // The idle ones: idle with no turn under way.
        w.agents[2].status = Status::Idle;
        w.agents[3].status = Status::Idle;
        live.turns[3] = 1;
        let got = send(&mut live, &mut w, 0, "idle", "notice").unwrap();
        assert_eq!(got, "sent to claude-2 (message 4)");
        // One that may not get it is said, the rest get it; none, an error.
        w.agents[1].status = Status::Stopped;
        w.agents[2].gone = true;
        let got = send(&mut live, &mut w, 0, "workers", "inform").unwrap();
        assert_eq!(
            got,
            "sent to claude-3 (message 5); not to claude-1: claude-1 has stopped; it runs \
             again when the person talks to it"
        );
        w.agents[3].status = Status::Stopped;
        let err = send(&mut live, &mut w, 0, "workers", "inform").unwrap_err();
        assert!(
            err.starts_with("sent to nobody: claude-1: claude-1 has stopped"),
            "{err}"
        );
    }

    #[test]
    fn a_group_is_the_leader_s_for_informs_and_notices_and_a_name_comes_first() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let send =
            |live: &mut Live, w: &mut World, from, args: Value| call(live, w, from, "send", args);
        let err = send(
            &mut live,
            &mut w,
            1,
            json!({ "to": "workers", "type": "inform", "body": "x" }),
        );
        assert_eq!(
            err.unwrap_err(),
            "only the leader writes to a group; write to one teammate"
        );
        for kind in ["assign", "ask", "done", "cancel", "progress"] {
            let err = send(
                &mut live,
                &mut w,
                0,
                json!({ "to": "workers", "type": kind, "body": "x" }),
            );
            assert!(
                err.unwrap_err()
                    .starts_with("a group gets inform or notice only"),
                "{kind}"
            );
        }
        let err = send(
            &mut live,
            &mut w,
            0,
            json!({ "to": "idle", "type": "inform", "body": "x", "re": 1 }),
        );
        assert_eq!(
            err.unwrap_err(),
            "re is for one teammate's message, not a group's"
        );
        assert!(live.mail.iter().all(Vec::is_empty));
        // A teammate of that name is written to, not the group.
        w.agents[2].name = "workers".into();
        let got = send(
            &mut live,
            &mut w,
            0,
            json!({ "to": "workers", "type": "assign", "body": "x" }),
        );
        assert!(got.is_ok(), "{got:?}");
        let to: Vec<AgentId> = (w.journal.iter())
            .filter_map(|e| match e.event {
                Event::Sent { to, .. } => Some(to),
                _ => None,
            })
            .collect();
        assert_eq!(to, [2], "the one called so");
        assert_eq!(Group::parse("claude"), None);
        assert_eq!(Group::parse("Workers"), None);
    }

    #[test]
    fn an_assign_to_an_idle_worker_starts_its_turn_with_a_card() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        let got = sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "assign", "Fix the parser\nin src/p.rs"),
        );
        assert_eq!(
            got.unwrap(),
            "delivered as message 1; claude-1 starts on it now"
        );
        live.step(&mut w);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(
            q[0].contains("[assign 1] from lead:\nFix the parser\nin src/p.rs"),
            "{q:?}"
        );
        assert!(q[0].contains("not by the person"));
        let card = w.agents[1].card.expect("a card");
        assert_eq!(w.cards[card].title, "Fix the parser");
        assert!(live.mail[1].is_empty());
        // Both rows quote the first line, each knowing who said it.
        let talk = |id: AgentId| w.agents[id].talk.clone().expect("a talk");
        assert_eq!(
            (talk(0).with, talk(0).mine, talk(0).text.as_str()),
            (Party::Agent(1), true, "Fix the parser")
        );
        assert_eq!(
            (talk(1).with, talk(1).mine, talk(1).text.as_str()),
            (Party::Agent(0), false, "Fix the parser")
        );
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Sent {
                from: 0,
                to: 1,
                kind: MsgKind::Assign,
                ..
            }
        ));
    }

    #[test]
    fn an_answer_to_the_person_waits_for_them_until_they_speak() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.said(0, "check the readme");
        live.asked[0] = true;
        live.turns[0] = 1;
        finish(
            &mut live,
            &mut w,
            0,
            "\nThe readme is one line.\nShall I assign more?",
        );
        assert_eq!(answers(&w), [(0, "The readme is one line.".to_string())]);
        // Speaking to the agent answers it.
        w.said(0, "yes");
        assert!(answers(&w).is_empty());
    }

    fn journaled_replies(w: &World) -> Vec<(AgentId, String)> {
        w.journal
            .iter()
            .filter_map(|e| match &e.event {
                Event::Replied { agent, text } => Some((*agent, text.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn only_a_text_that_reaches_the_person_is_journaled_as_a_reply() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The worker's turn on its mail: it sent done (journaled as such)
        // and its closing text restates it.
        live.turns[1] = 1;
        sent(&mut live, &mut w, 1, &msg("lead", "done", "Parser fixed.")).unwrap();
        finish(&mut live, &mut w, 1, "Fixed the parser and told lead.");
        assert_eq!(answers(&w), []);
        assert_eq!(journaled_replies(&w), []);
        assert!(
            matches!(
                &w.journal.last().unwrap().event,
                Event::Sent { from: 1, to: 0, .. }
            ),
            "{:?}",
            w.journal.last()
        );
        // The person asked the leader: its answer is a reply, once.
        w.said(0, "how far?");
        live.asked[0] = true;
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "Nearly there.");
        assert_eq!(answers(&w), [(0, "Nearly there.".to_string())]);
        assert_eq!(journaled_replies(&w), [(0, "Nearly there.".to_string())]);
    }

    #[test]
    fn a_leader_reports_once_the_team_is_quiet() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The worker still works: the leader's turn on its mail is no report.
        live.turns[1] = 1;
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "claude-1 is on it.");
        assert!(answers(&w).is_empty());
        // A worker's own turn is not an answer to the person either.
        finish(&mut live, &mut w, 1, "Done.");
        assert!(answers(&w).is_empty());
        live.turns[0] = 1;
        finish(
            &mut live,
            &mut w,
            0,
            "All done: README has a Usage section.",
        );
        assert_eq!(answers(&w).len(), 1);
    }

    #[test]
    fn a_busy_agent_gets_its_mail_when_its_turn_ends() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.turns[0] = 1;
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "Parser fixed."));
        assert!(got.unwrap().contains("busy"));
        live.step(&mut w);
        assert!(queued(&live, 0).is_empty(), "not while it works");
        live.tx
            .send((
                0,
                0,
                AgentEvent::TurnDone {
                    ok: true,
                    text: "ok".into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.turns[0], 1);
        assert!(queued(&live, 0)[0].contains("[done 1] from claude-1:\nParser fixed."));
    }

    #[test]
    fn a_worker_s_done_reaches_the_leader_when_its_turn_ends_headed_with_what_it_cost() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // A done comes mid-turn.
        live.turns[1] = 1;
        w.agents[1].task.round = Some((0, 3));
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "Parser fixed.")).unwrap();
        assert_eq!(
            got,
            "delivered as message 1; lead gets it when this turn of yours ends, headed with \
             what it cost"
        );
        live.step(&mut w);
        assert!(
            queued(&live, 0).is_empty(),
            "not while the worker's turn runs"
        );
        // Its change, read mid-turn, is settled at the turn's end, and the
        // leader's mail is headed with the figure.
        w.change_ready(Change {
            agent: 1,
            title: "Parser fixed.".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        w.started_at = crate::live::now() - (3 + 144) * SECONDS_PER_TICK;
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "Done.".into(),
                cost: 840_000,
                denied: 0,
            },
        );
        live.step(&mut w);
        let s = w.change_of(1).unwrap().spend.unwrap();
        assert_eq!((s.cost, s.rounds), (8_400, 1));
        assert!((144..=145).contains(&s.ticks), "{s:?}");
        assert_eq!(live.turns[0], 1);
        assert!(
            queued(&live, 0)[0].contains("[done 1 · $0.84] from claude-1:\nParser fixed."),
            "{}",
            queued(&live, 0)[0]
        );
        // What holds a delivery: a worker's done while its turn runs, and
        // nothing else.
        let mail = |from, kind| Mail {
            from,
            kind,
            body: "x".into(),
            id: 2,
            re: None,
            paths: Vec::new(),
        };
        let helper = with_helper(&mut w, 1);
        live.turns[1] = 1;
        assert!(live.settling(&w, &mail(1, MsgKind::Done)));
        assert!(!live.settling(&w, &mail(1, MsgKind::Ask)), "only a done");
        assert!(
            !live.settling(&w, &mail(helper, MsgKind::Done)),
            "a helper's is its worker's"
        );
        w.agents[1].gone = true;
        assert!(
            !live.settling(&w, &mail(1, MsgKind::Done)),
            "gone: nothing to wait for"
        );
        w.agents[1].gone = false;
        live.turns[1] = 0;
        assert!(
            !live.settling(&w, &mail(1, MsgKind::Done)),
            "its turn is over"
        );
    }

    #[test]
    fn an_inform_waits_for_a_turn_something_else_starts() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = sent(&mut live, &mut w, 2, &msg("lead", "progress", "Half done."));
        assert_eq!(
            got.unwrap(),
            "queued as message 1 for lead's next turn (progress does not wake it)"
        );
        live.step(&mut w);
        assert!(queued(&live, 0).is_empty());
        sent(&mut live, &mut w, 1, &msg("lead", "ask", "Which file?")).unwrap();
        live.step(&mut w);
        let q = queued(&live, 0);
        assert_eq!(q.len(), 1, "both in one turn");
        assert!(
            q[0].contains("[progress 1] from claude-2") && q[0].contains("[ask 2] from claude-1"),
            "{q:?}"
        );
    }

    /// [`msg`] with `re`, the number of the message it replies to.
    fn reply(to: &str, kind: &str, body: &str, re: Value) -> Value {
        let mut m = msg(to, kind, body);
        m["re"] = re;
        m
    }

    #[test]
    fn a_message_is_numbered_and_a_reply_names_the_one_it_answers() {
        let mut w = team(2);
        let mut live = idle_live(3);
        // Numbers go on from those an earlier run gave.
        w.messages = 6;
        w.cards.push(Card {
            id: 0,
            project: 0,
            title: "parser".into(),
            owner: Some(1),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            started: 0,
            finished: 0,
        });
        w.agents[1].card = Some(0);
        live.turns[1] = 1;
        let got = sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "ask", "Which parser?\nThe old one?"),
        );
        assert_eq!(
            got.unwrap(),
            "queued as message 7; claude-1 is busy and gets it when its turn ends"
        );
        assert_eq!((live.mail[1][0].id, live.mail[1][0].re), (7, None));
        live.turns[0] = 1;
        let got = sent(
            &mut live,
            &mut w,
            1,
            &reply("lead", "answer", "The new one.", json!(7)),
        );
        assert_eq!(
            got.unwrap(),
            "queued as message 8; lead is busy and gets it when its turn ends"
        );
        assert_eq!(w.messages, 8);
        assert_eq!(
            live.mail[0],
            [Mail {
                from: 1,
                kind: MsgKind::Answer,
                body: "The new one.".into(),
                id: 8,
                re: Some(7),
                paths: Vec::new(),
            }]
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Sent {
                from: 1,
                to: 0,
                kind: MsgKind::Answer,
                text: "The new one.".into(),
                id: 8,
                re: Some(7),
            }
        );
        assert_eq!(
            w.cards[0].story.last().unwrap().event,
            StoryEvent::Sent {
                from: 1,
                to: 0,
                kind: MsgKind::Answer,
                text: "The new one.".into(),
                id: 8,
                re: Some(7),
            }
        );
        assert_eq!(w.message(7), Some((0, 1, "Which parser?")));
        let last = |w: &World, a: AgentId| w.agents[a].trace.back().cloned().unwrap();
        assert!(last(&w, 1).ends_with(" send to=lead type=answer id=8 re=7"));
        assert!(last(&w, 0).ends_with(" mail from=claude-1 type=answer id=8 re=7"));
        // What lead reads when its turn ends: the number, and what it answers.
        live.turns[0] = 0;
        live.step(&mut w);
        let q = queued(&live, 0);
        assert!(
            q[0].contains("\n\n[answer 8, re 7] from claude-1:\nThe new one."),
            "{q:?}"
        );
    }

    #[test]
    fn a_reply_answers_a_message_sent_to_it_and_goes_back_to_its_sender() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.turns = vec![1, 1, 1];
        sent(&mut live, &mut w, 0, &msg("claude-1", "ask", "Which?")).unwrap();
        let refused = |live: &mut Live, w: &mut World, from: AgentId, args: Value| {
            sent(live, w, from, &args).expect_err("refused")
        };
        for bad in [
            json!("1"),
            json!(0),
            json!(-1),
            json!(1.5),
            json!(u64::from(u32::MAX) + 1),
        ] {
            let why = refused(
                &mut live,
                &mut w,
                1,
                reply("lead", "answer", "x", bad.clone()),
            );
            assert!(why.starts_with("re must be the number"), "{bad}: {why}");
        }
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                1,
                reply("lead", "answer", "x", json!(u32::MAX))
            ),
            format!("there is no message {}", u32::MAX)
        );
        assert_eq!(
            refused(&mut live, &mut w, 1, reply("lead", "answer", "x", json!(2))),
            "there is no message 2"
        );
        assert_eq!(
            refused(&mut live, &mut w, 2, reply("lead", "answer", "x", json!(1))),
            "message 1 was not sent to you"
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                1,
                reply("claude-2", "answer", "x", json!(1))
            ),
            "message 1 came from lead; send the reply to it"
        );
        assert_eq!(
            refused(&mut live, &mut w, 1, reply("you", "answer", "x", json!(1))),
            "re is for a teammate's message: the person's have no number"
        );
        assert_eq!(w.messages, 1, "a refused message takes no number");
        assert!(live.mail[0].is_empty() && live.mail[2].is_empty());
        assert_eq!(w.journal.len(), 1);
        sent(
            &mut live,
            &mut w,
            1,
            &reply("lead", "answer", "x", json!(1)),
        )
        .unwrap();
        sent(
            &mut live,
            &mut w,
            1,
            &reply("lead", "inform", "y", Value::Null),
        )
        .unwrap();
        let got: Vec<(MsgId, Option<MsgId>)> = live.mail[0].iter().map(|m| (m.id, m.re)).collect();
        assert_eq!(got, [(2, Some(1)), (3, None)], "null is no re");
        assert!(
            w.agents[1]
                .trace
                .back()
                .unwrap()
                .ends_with(" type=inform id=3")
        );
    }

    #[test]
    fn bad_messages_are_refused_with_a_reason() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            sent(live, w, 1, &args).expect_err("refused")
        };
        assert!(refused(&mut live, &mut w, msg("ghost", "ask", "hi")).contains("the team is lead"));
        assert!(refused(&mut live, &mut w, msg("claude-1", "ask", "me")).contains("no teammate"));
        assert!(refused(&mut live, &mut w, msg("lead", "shout", "hi")).contains("one of assign"));
        assert!(refused(&mut live, &mut w, msg("lead", "ask", "  ")).contains("empty"));
        let long = "x".repeat(MAX_BODY + 1);
        assert!(refused(&mut live, &mut w, msg("lead", "ask", &long)).contains("longer"));
        assert!(refused(&mut live, &mut w, json!({ "to": "lead" })).contains("type"));
        w.agents[0].status = Status::Stopped;
        assert!(refused(&mut live, &mut w, msg("lead", "ask", "hi")).contains("stopped"));
        assert!(live.mail[0].is_empty());
        assert!(w.journal.is_empty());
    }

    #[test]
    fn a_question_to_the_person_waits_in_their_inbox_until_they_write_back() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "assign", "Fix the parser"),
        )
        .unwrap();
        live.step(&mut w);
        let card = w.agents[1].card.expect("a card");
        let t = w.tick;
        let ok = sent(
            &mut live,
            &mut w,
            1,
            &msg("you", "ask", "Which API?\nDetails."),
        )
        .unwrap();
        assert!(ok.contains("TO DECIDE"), "{ok}");
        assert!(live.mail.iter().all(|m| m.is_empty()), "nobody's mail");
        let open = w.open_inbox();
        assert_eq!(open.len(), 1);
        let it = &w.inbox[open[0]];
        assert_eq!(
            (it.kind, it.agent, it.detail.as_str(), it.open, it.tick),
            (InboxKind::Question, 1, "Which API?", true, t)
        );
        assert!(w.has_question(1) && !w.has_question(2));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::ToYou { from: 1, kind: MsgKind::Ask, text, .. } if text == "Which API?\nDetails."
        ));
        assert!(matches!(
            &w.cards[card].story.last().unwrap().event,
            StoryEvent::ToYou { kind: MsgKind::Ask, text } if text == "Which API?\nDetails."
        ));
        let a = &w.agents[1];
        let talk = a.talk.as_ref().unwrap();
        assert_eq!(
            (talk.with, talk.mine, talk.text.as_str(), talk.until),
            (Party::Person, true, "Which API?", t + 8)
        );
        assert!(a.trace.back().unwrap().contains("send to=you type=ask"));
        // A second question takes the first's place in the inbox.
        sent(&mut live, &mut w, 1, &msg("you", "ask", "Or this?")).unwrap();
        let open = w.open_inbox();
        assert_eq!((open.len(), w.inbox.len()), (1, 2));
        assert_eq!(w.inbox[open[0]].detail, "Or this?");
        // The person's answer closes it (the screen records the answer).
        w.said(1, "The first.");
        assert!(!w.has_question(1) && w.open_inbox().is_empty());
        // The person reads for free: a spent budget refuses nothing here.
        w.agents[1].cost = w.budget;
        assert!(sent(&mut live, &mut w, 1, &msg("you", "ask", "Still?")).is_ok());
    }

    #[test]
    fn what_is_not_a_question_only_informs_the_person() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let ok = sent(&mut live, &mut w, 1, &msg("you", "inform", "Halfway.")).unwrap();
        assert!(ok.starts_with("noted"), "{ok}");
        assert!(w.open_inbox().is_empty() && !w.has_question(1));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::ToYou { from: 1, kind: MsgKind::Inform, text, .. } if text == "Halfway."
        ));
        assert_eq!(w.journal.last().unwrap().event.kind(), EntryKind::Talk);
        // A worker's done to the person shows its change like any done:
        // the worktree is read (here one that is not there, so the
        // reading fails and says so); nothing else reads it.
        live.cwds[1] = Some(PathBuf::from("/nonexistent/tree"));
        let ok = sent(&mut live, &mut w, 1, &msg("you", "done", "Done.")).unwrap();
        assert!(
            ok.starts_with("noted") && ok.contains("could not read your change"),
            "{ok}"
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::ToYou {
                from: 1,
                kind: MsgKind::Done,
                ..
            }
        ));
        let ok = sent(&mut live, &mut w, 1, &msg("you", "blocked", "Stuck.")).unwrap();
        assert!(!ok.contains("change"), "{ok}");
        w.agents[0].role = Role::Worker;
        w.agents[1].role = Role::Leader;
        let ok = sent(&mut live, &mut w, 1, &msg("you", "done", "Done.")).unwrap();
        assert!(!ok.contains("change"), "a leader has no change: {ok}");
        // Work is not handed to the person.
        let before = w.journal.len();
        for kind in ["assign", "handoff", "cancel"] {
            let err = sent(&mut live, &mut w, 1, &msg("you", kind, "x")).unwrap_err();
            assert!(err.contains("not for the person"), "{kind}: {err}");
        }
        assert_eq!(w.journal.len(), before);
        // Only the exact name reaches the person.
        let err = sent(&mut live, &mut w, 1, &msg("You", "ask", "x")).unwrap_err();
        assert!(err.contains("no teammate is called"), "{err}");
    }

    #[test]
    fn agents_can_not_keep_each_other_busy_without_the_person() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.woken[0] = MAX_WOKEN;
        let err = sent(&mut live, &mut w, 1, &msg("lead", "ask", "again?")).unwrap_err();
        assert!(err.contains("tell the person"), "{err}");
        // What does not wake still goes through.
        sent(&mut live, &mut w, 1, &msg("lead", "inform", "fyi")).unwrap();
        // The person speaking resets the count.
        live.told(&mut w, 1, "carry on").unwrap();
        assert_eq!(live.woken[0], 0);
        sent(&mut live, &mut w, 1, &msg("lead", "ask", "again?")).unwrap();
        live.mail[0].clear();
        for _ in 0..MAX_MAIL {
            sent(&mut live, &mut w, 1, &msg("lead", "inform", "x")).unwrap();
        }
        assert!(
            sent(&mut live, &mut w, 1, &msg("lead", "inform", "x"))
                .unwrap_err()
                .contains("unread")
        );
    }

    #[test]
    fn nobody_gets_waking_mail_once_the_budget_is_spent() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.agents[0].cost = w.budget;
        assert!(
            sent(&mut live, &mut w, 1, &msg("lead", "done", "x"))
                .unwrap_err()
                .contains("budget")
        );
    }

    #[test]
    fn the_bus_answers_through_the_socket_of_the_caller() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let mut w = team(2);
        let mut live = idle_live(3);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        let dir = live.bus.as_ref().unwrap().dir.clone();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // Agent 1's session runs in this test's process: its socket takes
        // the test's calls. Agent 2's runs in another process, and agent
        // 0 has none: theirs refuse them, and say so in their trace.
        let other = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        live.slots[1] = Slot::Running(Box::new(Runs(std::process::id())));
        live.slots[2] = Slot::Running(Box::new(Runs(other.id())));
        let call = |live: &Live, n: usize| {
            let path = live.bus.as_ref().unwrap().servers[n].path().to_path_buf();
            thread::spawn(move || {
                let mut sock = UnixStream::connect(path).unwrap();
                sock.set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let call = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"send",
                    "arguments":{"to":"lead","type":"inform","body":"from the socket"}}});
                writeln!(sock, "{call}").unwrap();
                let mut line = String::new();
                BufReader::new(&sock).read_line(&mut line).unwrap();
                serde_json::from_str::<Value>(&line).unwrap()
            })
        };
        live.step(&mut w);
        for n in [0, 2] {
            let reply = call(&live, n).join().unwrap();
            assert_eq!(reply["result"]["isError"], true, "{n}: {reply}");
            assert_eq!(
                reply["result"]["content"][0]["text"], NOT_ITS_SESSION_SAID,
                "{n}"
            );
        }
        let caller = call(&live, 1);
        wait_for(&mut live, &mut w, |w| !w.journal.is_empty());
        let reply = caller.join().unwrap();
        assert_eq!(reply["result"]["isError"], false, "{reply}");
        let mut other = other;
        other.kill().unwrap();
        other.wait().unwrap();
        for n in [0, 2] {
            assert!(
                w.agents[n].trace.iter().any(|l| l
                    .ends_with("send: called on this socket from outside its session, refused")),
                "{n}: {:?}",
                w.agents[n].trace
            );
        }
        assert_eq!(w.journal.len(), 1, "only agent 1's message went");
        // The sender is whose socket it was.
        assert!(matches!(
            w.journal[0].event,
            Event::Sent { from: 1, to: 0, .. }
        ));
        // The leader's socket does not even offer claim; it alone offers
        // hire. Every socket offers approve, which Claude Code asks through.
        let list = |n: usize| {
            let path = live.bus.as_ref().unwrap().servers[n].path().to_path_buf();
            let mut sock = UnixStream::connect(path).unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            writeln!(sock, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).unwrap();
            let mut line = String::new();
            BufReader::new(&sock).read_line(&mut line).unwrap();
            let listed: Value = serde_json::from_str(&line).unwrap();
            listed["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["name"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            list(0),
            [
                "send", "post", "board", "run", "whoami", "plan", "hire", "approve"
            ]
        );
        assert_eq!(
            list(2),
            [
                "send", "post", "board", "run", "whoami", "claim", "hire", "approve"
            ]
        );
        drop(live);
        assert!(!dir.exists());
    }

    #[test]
    fn a_posted_card_is_offered_to_one_idle_worker_without_a_card_of_its_own() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = live.post(&mut w, 0, &card("Add a test", "Cover sub() in src/m.rs."));
        assert!(got.unwrap().starts_with("posted as #1"));
        assert_eq!(w.cards[0].column, Column::Waiting);
        assert_eq!(w.cards[0].owner, None);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Posted { agent: 0, card: 0 }
        ));
        live.step(&mut w);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(q[0].contains("#1 Add a test (posted by lead)"), "{q:?}");
        assert!(q[0].contains("not by the person"));
        assert!(
            !q[0].contains("Cover sub()"),
            "the body comes with the claim"
        );
        assert_eq!(w.cards.len(), 1, "choosing is not a card");
        assert_eq!(w.agents[1].card, None);
        // Not in front of two workers at once.
        assert!(queued(&live, 2).is_empty());
        // The session never starts here and may be gone by the next step,
        // so what counts is the bookkeeping, not the queue.
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1], "offered once");
        assert_eq!(live.woken[1], 1);
    }

    #[test]
    fn a_card_goes_to_the_next_worker_when_the_first_lets_it_be() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("Add a test", "x")).unwrap();
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1]);
        // claude-1's turn ended without taking it.
        live.turns[1] = 0;
        live.step(&mut w);
        assert!(queued(&live, 2)[0].contains("#1 Add a test"));
        assert_eq!(live.offered[&0], [1, 2]);
        live.turns[2] = 0;
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1, 2], "each worker sees a card once");
    }

    #[test]
    fn a_claim_makes_the_card_the_workers_and_hands_it_the_task() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("Add a test", "Cover sub()."))
            .unwrap();
        let got = live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap();
        assert!(
            got.contains("Cover sub().") && got.contains("send lead a done"),
            "{got}"
        );
        assert_eq!(w.cards[0].owner, Some(1));
        assert_eq!(w.cards[0].column, Column::Working);
        assert_eq!(w.agents[1].card, Some(0));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Claimed { agent: 1, card: 0 }
        ));
        let err = live.claim(&mut w, 2, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains("taken by claude-1"), "{err}");
        assert!(
            live.board(&w, 2)
                .contains("#1 Add a test (taken by claude-1)")
        );
        // The turn ends: the card is done, and stays off the open list.
        live.step(&mut w);
        assert!(
            queued(&live, 2).is_empty(),
            "a taken card is offered to nobody"
        );
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "Added.".into(),
                cost: 0,
                denied: 0,
            },
            0,
        );
        assert_eq!(w.cards[0].column, Column::Done);
        assert!(live.board(&w, 0).contains("(done by claude-1)"));
    }

    #[test]
    fn bad_claims_are_refused_with_a_reason() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.post(&mut w, 1, &card("Split p.rs", "x")).unwrap();
        let err = live.claim(&mut w, 0, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains("only workers"), "{err}");
        for bad in [
            json!({ "card": 0 }),
            json!({ "card": 9 }),
            json!({ "card": "1" }),
            json!({}),
        ] {
            let err = live.claim(&mut w, 1, &bad).unwrap_err();
            assert!(err.contains("no such card"), "{bad} {err}");
        }
        // A card that did not come from the board is not one to take.
        with_card(&mut w, 0);
        w.cards[1].column = Column::Waiting;
        assert!(live.claim(&mut w, 1, &json!({ "card": 2 })).is_err());
        // One card at a time.
        let mine = with_card(&mut w, 1);
        let err = live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains(&format!("finish #{}", mine + 1)), "{err}");
        assert_eq!(w.cards[0].column, Column::Waiting);
    }

    #[test]
    fn bad_posts_are_refused_and_the_board_is_capped() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            live.post(w, 0, &args).expect_err("refused")
        };
        assert!(refused(&mut live, &mut w, card("", "x")).contains("title and a body"));
        assert!(refused(&mut live, &mut w, card("t", " ")).contains("title and a body"));
        assert!(refused(&mut live, &mut w, card("two\nlines", "x")).contains("one line"));
        let long = "x".repeat(MAX_TITLE + 1);
        assert!(refused(&mut live, &mut w, card(&long, "x")).contains("one line"));
        let body = "x".repeat(MAX_BODY + 1);
        assert!(refused(&mut live, &mut w, card("t", &body)).contains("longer"));
        assert!(w.cards.is_empty());

        for i in 0..MAX_POSTS {
            live.post(&mut w, 0, &card(&format!("t{i}"), "x")).unwrap();
        }
        assert!(refused(&mut live, &mut w, card("t", "x")).contains("tell the person"));
        // The person speaking resets what an agent may post, not the board.
        live.told(&mut w, 0, "go on").unwrap();
        for i in 0..MAX_OPEN as u32 - MAX_POSTS {
            live.post(&mut w, 0, &card(&format!("u{i}"), "x")).unwrap();
        }
        assert!(refused(&mut live, &mut w, card("t", "x")).contains("open cards"));
    }

    #[test]
    fn only_workers_get_the_claim_tool_and_only_the_leader_and_workers_hire() {
        let names = |r: Role| -> Vec<String> {
            tools_of(r, &[], DEFAULT_RUN_TIMEOUT)
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(
            names(Role::Leader),
            ["send", "post", "board", "run", "whoami", "plan", "hire"]
        );
        assert_eq!(
            names(Role::Worker),
            ["send", "post", "board", "run", "whoami", "claim", "hire"]
        );
        assert_eq!(
            names(Role::Helper),
            ["send", "post", "board", "run", "whoami"]
        );
        assert_eq!(
            names(Role::Researcher),
            ["send", "post", "board", "run", "whoami"]
        );
        // The same tool, told differently: a worker hires helpers.
        let worker_hire = &tools_of(Role::Worker, &[], DEFAULT_RUN_TIMEOUT)[6];
        assert!(worker_hire.description.starts_with("Adds a helper"));
        let hire = &tools_of(Role::Leader, &[], DEFAULT_RUN_TIMEOUT)[6];
        assert!(hire.description.starts_with("Adds a worker"));
        assert!(hire.input["properties"]["vendor"].is_null());
        assert_eq!(
            hire.input["properties"]["tier"]["enum"],
            json!(["cheap", "standard", "strong"])
        );
        // A start point for a worker; a helper works in its worker's tree.
        assert_eq!(hire.input["properties"]["from"]["maxLength"], 200);
        assert!(
            hire.input["properties"]["from"]["description"]
                .as_str()
                .unwrap()
                .starts_with("Where the worker's worktree starts")
        );
        assert!(worker_hire.input["properties"].get("from").is_none());
        let solo_hire = tools_of(Role::Solo, &[], DEFAULT_RUN_TIMEOUT);
        let solo_hire = solo_hire.iter().find(|t| t.name == "hire").unwrap();
        assert!(solo_hire.input["properties"].get("from").is_none());
    }

    #[test]
    fn every_tool_reaches_the_bus_through_the_call_channel() {
        let mut w = team(2);
        let mut live = idle_live(3);
        assert_eq!(
            call(&mut live, &mut w, 1, "board", json!({})),
            Ok("the board is empty".into())
        );
        call(
            &mut live,
            &mut w,
            0,
            "post",
            card("Add a test", "for the parser"),
        )
        .expect("posted");
        let open = call(&mut live, &mut w, 1, "board", json!({})).unwrap();
        assert!(open.contains("Open:") && !open.contains("Taken:"), "{open}");
        // Off the board, no panic: 0, and one past the last card.
        for bad in [0, w.cards.len() as u64 + 1] {
            assert!(call(&mut live, &mut w, 1, "claim", json!({ "card": bad })).is_err());
        }
        call(&mut live, &mut w, 1, "claim", json!({ "card": 1 })).expect("claimed");
        let taken = call(&mut live, &mut w, 2, "board", json!({})).unwrap();
        assert!(
            taken.contains("Taken:") && !taken.contains("Open:"),
            "{taken}"
        );
        let unknown = call(&mut live, &mut w, 1, "nothing", json!({}));
        assert!(
            matches!(&unknown, Err(e) if e.contains("no tool")),
            "{unknown:?}"
        );
    }

    #[test]
    fn the_caps_allow_exactly_the_cap_and_not_one_more() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let body = "x".repeat(MAX_BODY);
        assert!(sent(&mut live, &mut w, 1, &msg("lead", "inform", &body)).is_ok());
        let over = format!("{body}x");
        assert!(sent(&mut live, &mut w, 1, &msg("lead", "inform", &over)).is_err());
        let title = "t".repeat(MAX_TITLE);
        assert!(live.post(&mut w, 0, &card(&title, &body)).is_ok());
        assert!(
            live.post(&mut w, 0, &card(&format!("{title}t"), "b"))
                .is_err()
        );
        assert!(live.post(&mut w, 0, &card("t", &over)).is_err());
    }

    #[test]
    fn a_message_to_oneself_is_refused_and_the_team_listed_leaves_oneself_out() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = sent(&mut live, &mut w, 1, &msg("claude-1", "inform", "hello me"));
        let Err(why) = got else {
            panic!("sent to itself: {got:?}");
        };
        let (_, team) = why.split_once("the team is ").expect("lists the team");
        assert_eq!(team, "lead, claude-2", "{why}");
    }

    #[test]
    fn the_board_shows_open_and_taken_cards_apart() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("First", "one")).unwrap();
        live.step(&mut w);
        live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap();
        live.post(&mut w, 0, &card("Second", "two")).unwrap();
        let both = live.board(&w, 2);
        assert!(
            both.starts_with(
                "Open:
"
            ),
            "{both}"
        );
        assert!(both.contains("Second"));
        assert!(
            both.contains(
                "

Taken:
"
            ),
            "{both}"
        );
        assert!(both.contains("First"));
    }

    /// A leader, one idle worker and a teammate of it; no git, so nothing
    /// moves a worktree between turns.
    fn waiting_team() -> (World, Live) {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        live.places[0].git = false;
        (w, live)
    }

    const HELD: &str = "held as message 1; claude-1 waits on the person and gets it once \
                        they have answered, it starts nothing until then";

    #[test]
    fn mail_waits_while_its_agent_waits_on_the_person_and_goes_when_that_ends() {
        for kind in [
            InboxKind::Sensitive,
            InboxKind::Research,
            InboxKind::Question,
            InboxKind::Approval,
            InboxKind::Fault,
            InboxKind::Script,
        ] {
            let (mut w, mut live) = waiting_team();
            w.inbox.push(item(kind, 1, "what waits"));
            let got = sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it"));
            assert_eq!(got.unwrap(), HELD, "{kind:?}");
            assert!(
                w.agents[1]
                    .trace
                    .back()
                    .unwrap()
                    .ends_with("held until the person answers: message 1")
            );
            live.step(&mut w);
            live.step(&mut w);
            assert!(queued(&live, 1).is_empty(), "{kind:?} starts no turn");
            assert_eq!(
                (live.turns[1], live.woken[1], live.mail[1].len()),
                (0, 0, 1),
                "{kind:?}"
            );
            assert_eq!(w.agents[1].held_mail, 1, "{kind:?}");
            // The person's action started nothing: the mail starts a turn.
            w.inbox[0].open = false;
            live.step(&mut w);
            assert_eq!(queued(&live, 1).len(), 1, "{kind:?}");
            assert!(queued(&live, 1)[0].contains("Fix it"), "{kind:?}");
            assert_eq!(
                (live.turns[1], live.woken[1], live.mail[1].len()),
                (1, 1, 0),
                "{kind:?}"
            );
            assert_eq!(w.agents[1].held_mail, 0, "{kind:?}");
        }
    }

    #[test]
    fn the_status_that_waits_for_the_person_holds_mail_as_an_item_does() {
        let (mut w, mut live) = waiting_team();
        w.agents[1].status = Status::WaitingHuman;
        let got = sent(&mut live, &mut w, 0, &msg("claude-1", "ask", "Which?"));
        assert_eq!(got.unwrap(), HELD);
        live.step(&mut w);
        assert_eq!((live.turns[1], w.agents[1].held_mail), (0, 1));
        w.agents[1].status = Status::Idle;
        live.step(&mut w);
        assert_eq!((live.turns[1], w.agents[1].held_mail), (1, 0));
    }

    #[test]
    fn an_answer_or_a_change_of_its_own_and_someone_elses_item_hold_nothing() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Reply, 1, "said"));
        w.inbox.push(item(InboxKind::Change, 1, "Docs"));
        w.inbox.push(item(InboxKind::Question, 2, "the other's"));
        let mut closed = item(InboxKind::Question, 1, "answered");
        closed.open = false;
        w.inbox.push(closed);
        let got = sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it"));
        assert_eq!(
            got.unwrap(),
            "delivered as message 1; claude-1 starts on it now"
        );
        live.step(&mut w);
        assert_eq!((live.turns[1], w.agents[1].held_mail), (1, 0));
    }

    #[test]
    fn a_cancel_is_never_held_and_takes_what_waited_along_in_order() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
        live.step(&mut w);
        assert_eq!(live.turns[1], 0);
        let traced = w.agents[1].traced;
        let got = sent(&mut live, &mut w, 0, &msg("claude-1", "cancel", "Drop it"));
        assert_eq!(
            got.unwrap(),
            "delivered as message 2; claude-1 starts on it now"
        );
        assert_eq!(w.agents[1].traced, traced + 1, "the mail line alone");
        live.step(&mut w);
        assert_eq!((live.turns[1], live.woken[1]), (1, 1));
        let prompt = &queued(&live, 1)[0];
        let (fix, drop) = (prompt.find("Fix it"), prompt.find("Drop it"));
        assert!(fix.unwrap() < drop.unwrap(), "{prompt}");
        assert_eq!(w.agents[1].held_mail, 0);
        assert!(w.has_question(1), "the question still waits");
    }

    #[test]
    fn what_does_not_wake_is_counted_on_the_row_too_and_starts_nothing() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        let got = sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "inform", "main moved"),
        );
        assert_eq!(
            got.unwrap(),
            "queued as message 1 for claude-1's next turn (inform does not wake it)"
        );
        live.step(&mut w);
        assert_eq!((live.turns[1], w.agents[1].held_mail), (0, 1));
        // Nothing waits any more, and an inform still wakes nobody.
        w.inbox[0].open = false;
        live.step(&mut w);
        assert_eq!((live.turns[1], w.agents[1].held_mail), (0, 0));
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn a_busy_agent_with_a_question_open_is_said_to_hold_the_mail_too() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        live.turns[1] = 1;
        let got = sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it"));
        assert_eq!(got.unwrap(), HELD);
    }

    #[test]
    fn the_caps_hold_while_mail_waits_on_the_person() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        for n in 0..MAX_MAIL {
            sent(
                &mut live,
                &mut w,
                0,
                &msg("claude-1", "ask", &n.to_string()),
            )
            .unwrap();
        }
        live.step(&mut w);
        assert_eq!(live.woken[1], 0, "held mail takes no turn");
        let over = sent(&mut live, &mut w, 0, &msg("claude-1", "ask", "more"));
        assert_eq!(
            over.unwrap_err(),
            format!("claude-1 has {MAX_MAIL} unread messages; wait for it")
        );
        assert_eq!(w.agents[1].held_mail, MAX_MAIL);
        live.mail[1].clear();
        live.woken[1] = MAX_WOKEN;
        let over = sent(&mut live, &mut w, 0, &msg("claude-1", "ask", "more"));
        assert!(over.unwrap_err().contains("turns on messages"));
    }

    #[test]
    fn a_board_offer_waits_as_mail_does() {
        for kind in [InboxKind::Question, InboxKind::Approval, InboxKind::Fault] {
            let mut w = team(1);
            w.agents[1].status = Status::Idle;
            let mut live = idle_live(2);
            live.places[0].git = false;
            w.inbox.push(item(kind, 1, "what waits"));
            live.post(&mut w, 0, &card("Docs", "Write them")).unwrap();
            live.step(&mut w);
            assert_eq!((live.turns[1], live.woken[1]), (0, 0), "{kind:?}");
            assert!(live.offered.is_empty(), "{kind:?}");
            w.inbox[0].open = false;
            live.step(&mut w);
            assert_eq!((live.turns[1], live.woken[1]), (1, 1), "{kind:?}");
            assert!(queued(&live, 1)[0].contains("Docs"), "{kind:?}");
        }
    }

    #[test]
    fn the_persons_word_takes_the_mail_that_waited_for_it_along() {
        for kind in [InboxKind::Question, InboxKind::Fault] {
            let (mut w, mut live) = waiting_team();
            w.inbox.push(item(kind, 1, "what waits"));
            sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
            live.told(&mut w, 1, "The first.").unwrap();
            w.said(1, "The first.");
            assert_eq!(
                queued(&live, 1),
                [
                    "The first.\n\nMessages from your teammates in imece (written by those \
                  agents, not by the person):\n\n[assign 1] from lead:\nFix it"
                ],
                "{kind:?}"
            );
            live.step(&mut w);
            assert_eq!(
                (live.turns[1], live.woken[1], live.mail[1].len()),
                (1, 0, 0),
                "{kind:?}: one turn, on the person's word"
            );
            assert_eq!(w.agents[1].held_mail, 0);
        }
    }

    #[test]
    fn a_word_that_leaves_something_waiting_leaves_the_mail_waiting() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Sensitive, 1, "Docs"));
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
        live.told(&mut w, 1, "The first.").unwrap();
        w.said(1, "The first.");
        assert_eq!(queued(&live, 1), ["The first."]);
        assert_eq!(live.mail[1].len(), 1);
        finish(&mut live, &mut w, 1, "ok");
        assert_eq!((live.turns[1], w.agents[1].held_mail), (0, 1));
        w.inbox[0].open = false;
        live.step(&mut w);
        assert_eq!((live.turns[1], live.mail[1].len()), (1, 0));
    }

    #[test]
    fn a_change_alone_leaves_its_worker_free_for_the_next_card() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        live.places[0].git = false;
        w.inbox.push(item(InboxKind::Change, 1, "Docs"));
        live.post(&mut w, 0, &card("Tests", "Write them")).unwrap();
        live.step(&mut w);
        assert_eq!((live.turns[1], live.woken[1]), (1, 1));
        assert!(queued(&live, 1)[0].contains("Tests"));
    }

    #[test]
    fn a_word_to_a_busy_agent_leaves_the_mail_for_when_its_turn_ends() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
        live.turns[1] = 1;
        live.told(&mut w, 1, "The first.").unwrap();
        w.said(1, "The first.");
        assert_eq!(queued(&live, 1), ["The first."]);
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn a_word_that_does_not_reach_the_session_loses_no_mail() {
        let (mut w, mut live) = waiting_team();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
        let session = Box::new(Heard(Default::default(), true));
        live.ready(
            &mut w,
            1,
            Ok((
                session,
                std::env::temp_dir(),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
            )),
        );
        assert!(live.told(&mut w, 1, "The first.").is_err());
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn a_word_with_nothing_waiting_leaves_quiet_mail_where_it_is() {
        let (mut w, mut live) = waiting_team();
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "inform", "main moved"),
        )
        .unwrap();
        live.told(&mut w, 1, "Go on.").unwrap();
        assert_eq!(queued(&live, 1), ["Go on."]);
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn a_leader_reports_past_mail_that_waits_on_the_person() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.places[0].git = false;
        w.agents[1].status = Status::Stopped;
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Inform,
            body: "main moved".into(),
            id: 1,
            re: None,
            paths: Vec::new(),
        });
        // Mail that may still move: no report yet.
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "claude-1 has mail.");
        assert!(answers(&w).is_empty());
        // Mail that waits on the person: nothing moves until they come.
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "claude-1's question waits for you.");
        assert_eq!(answers(&w).len(), 1);
    }

    #[test]
    fn a_question_to_the_person_says_the_mail_waits_for_the_answer() {
        let (mut w, mut live) = waiting_team();
        let got = sent(&mut live, &mut w, 1, &msg("you", "ask", "Which?")).unwrap();
        assert!(got.ends_with("teammates' messages wait for that answer too, but for a cancel."));
    }

    #[test]
    fn a_delivery_takes_its_title_from_the_assignment_not_the_first_mail() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "inform", "a note first"),
        )
        .unwrap();
        sent(
            &mut live,
            &mut w,
            0,
            &msg(
                "claude-1",
                "assign",
                "Fix the parser
in p.rs",
            ),
        )
        .unwrap();
        live.step(&mut w);
        let card = w.agents[1].card.expect("a card");
        assert_eq!(w.cards[card].title, "Fix the parser");
    }

    #[test]
    fn the_tick_is_the_time_since_the_world_began_in_whole_ticks() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The world began 13 seconds ago (in an earlier run, maybe).
        w.started_at = now() - 13;
        live.step(&mut w);
        assert_eq!(w.tick, 13 / SECONDS_PER_TICK);
        w.started_at = now() - 7 * 86_400;
        live.step(&mut w);
        assert_eq!(w.tick, 7 * 86_400 / SECONDS_PER_TICK);
        w.started_at = now() + 60;
        live.step(&mut w);
        assert_eq!(w.tick, 0, "a clock set back does not underflow");
    }
    fn with_paths(mut args: Value, paths: Value) -> Value {
        args["paths"] = paths;
        args
    }

    fn with_options(mut args: Value, options: Value) -> Value {
        args["options"] = options;
        args
    }

    #[test]
    fn an_ask_to_the_person_may_offer_answers_and_nothing_else_may() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        let ask = |options: Value| with_options(msg("you", "ask", "Which?"), options);
        let ok = sent(
            &mut live,
            &mut w,
            1,
            &ask(json!([" the card ", "store credit"])),
        )
        .unwrap();
        assert!(ok.contains("with your options"), "{ok}");
        assert!(
            matches!(
                &w.journal.last().unwrap().event,
                Event::ToYou { from: 1, kind: MsgKind::Ask, options, .. }
                    if options == &["the card", "store credit"]
            ),
            "trimmed: {:?}",
            w.journal.last()
        );
        assert_eq!(
            w.question_of(1).unwrap().options,
            ["the card", "store credit"]
        );
        // Without options the answer promises none.
        let ok = sent(&mut live, &mut w, 1, &msg("you", "ask", "Again?")).unwrap();
        assert!(!ok.contains("options") && ok.contains("TO DECIDE"), "{ok}");
        assert!(w.question_of(1).unwrap().options.is_empty());
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            sent(live, w, 1, &args).expect_err("refused")
        };
        let inform = with_options(msg("you", "inform", "x"), json!(["a"]));
        assert!(refused(&mut live, &mut w, inform).contains("ask to the person"));
        let teammate = with_options(msg("lead", "ask", "x"), json!(["a"]));
        assert!(refused(&mut live, &mut w, teammate).contains("ask to the person"));
        assert!(refused(&mut live, &mut w, ask(json!("a"))).contains("array of strings"));
        assert!(refused(&mut live, &mut w, ask(json!([1]))).contains("array of strings"));
        let five = ask(json!(["a", "b", "c", "d", "e"]));
        assert!(refused(&mut live, &mut w, five).contains("at most 4"));
        assert!(refused(&mut live, &mut w, ask(json!(["a", " "]))).contains("empty"));
        let long = "x".repeat(OPTION_MAX + 1);
        assert!(refused(&mut live, &mut w, ask(json!([long]))).contains("longer than 60"));
        assert!(refused(&mut live, &mut w, ask(json!(["a\tb"]))).contains("control"));
        assert_eq!(
            w.question_of(1).unwrap().text,
            "Again?",
            "a refusal asks nothing"
        );
        // Four, one at the cap, pass; an empty list offers none.
        let four = json!(["x".repeat(OPTION_MAX), "b", "c", "d"]);
        sent(&mut live, &mut w, 1, &ask(four)).unwrap();
        assert_eq!(w.question_of(1).unwrap().options.len(), 4);
        sent(&mut live, &mut w, 1, &ask(json!([]))).unwrap();
        assert!(w.question_of(1).unwrap().options.is_empty());
        assert_eq!(
            send_tool().input["properties"]["options"]["maxItems"],
            OPTIONS_MAX
        );
    }

    #[test]
    fn an_assign_with_paths_opens_a_card_that_owns_them_and_the_prompt_names_them() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        let args = with_paths(
            msg("claude-1", "assign", "Fix the parser\nin src/p.rs"),
            json!(["src/p.rs", "docs/ shared"]),
        );
        let got = sent(&mut live, &mut w, 0, &args).unwrap();
        assert_eq!(got, "delivered as message 1; claude-1 starts on it now");
        live.step(&mut w);
        let q = queued(&live, 1);
        assert!(
            q[0].ends_with(
                "in src/p.rs\n\nThis card owns src/p.rs, docs (shared): stay in them; what \
                 else needs changing goes in your done, or ask for the card to be widened."
            ),
            "{q:?}"
        );
        let card = w.agents[1].card.expect("a card");
        assert_eq!(
            w.cards[card].paths,
            [Owned::new("src/p.rs"), Owned::shared("docs")]
        );
        // One without paths owns none, and the prompt says nothing of it.
        w.agents[2].status = Status::Idle;
        sent(&mut live, &mut w, 0, &msg("claude-2", "assign", "Docs")).unwrap();
        live.step(&mut w);
        let q = queued(&live, 2);
        assert!(q[0].ends_with("from lead:\nDocs"), "{q:?}");
        assert!(w.cards[w.agents[2].card.unwrap()].paths.is_empty());
        assert_eq!(owns(&[]), "");
    }

    #[test]
    fn paths_go_with_an_assign_only_and_are_checked_before_anything_is_sent() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            sent(live, w, 0, &args).expect_err("refused")
        };
        let err = refused(
            &mut live,
            &mut w,
            with_paths(msg("claude-1", "ask", "Which?"), json!(["src"])),
        );
        assert_eq!(err, "paths go with an assign: the card it opens owns them");
        let err = refused(
            &mut live,
            &mut w,
            with_paths(msg("claude-1", "assign", "Fix"), json!(["/etc"])),
        );
        assert_eq!(
            err,
            "paths are relative to the repository; \"/etc\" starts with /"
        );
        let err = refused(
            &mut live,
            &mut w,
            with_paths(msg("you", "inform", "Fix"), json!(["src"])),
        );
        assert_eq!(err, "paths go with an assign: the card it opens owns them");
        assert!(live.mail[1].is_empty() && w.messages == 0, "nothing went");
        // An empty list is no paths.
        let got = sent(
            &mut live,
            &mut w,
            0,
            &with_paths(msg("claude-1", "inform", "Hi"), json!([])),
        );
        assert!(got.is_ok(), "{got:?}");
    }

    #[test]
    fn a_path_an_open_card_owns_is_refused_to_another_unless_shared_or_the_same_worker_s() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        w.agents[2].status = Status::Idle;
        let mut live = idle_live(3);
        let assign = |to: &str, paths: Value| {
            with_paths(msg(to, "assign", &format!("Work for {to}")), paths)
        };
        sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-1", json!(["src/a.rs"])),
        )
        .unwrap();
        live.step(&mut w);
        let err = sent(&mut live, &mut w, 0, &assign("claude-2", json!(["src"]))).unwrap_err();
        assert_eq!(
            err,
            "#1 Work for claude-1 owns \"src/a.rs\", which \"src\" names too; give this \
             card other paths, or end one with the word shared (src shared)"
        );
        let err = live
            .post(
                &mut w,
                0,
                &with_paths(card("More", "x"), json!(["src/a.rs"])),
            )
            .unwrap_err();
        assert!(
            err.starts_with("#1 Work for claude-1 owns \"src/a.rs\""),
            "{err}"
        );
        assert_eq!(w.cards.len(), 1, "nothing posted");
        // Shared on either side passes; so does a path of its own.
        let got = sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-2", json!(["src shared"])),
        );
        assert!(got.is_ok(), "{got:?}");
        live.step(&mut w);
        assert_eq!(w.cards[1].paths, [Owned::shared("src")]);
        // A posted card, waiting, owns its paths too; a done card no more.
        live.post(&mut w, 0, &with_paths(card("Lib", "x"), json!(["lib/*"])))
            .unwrap();
        let err = sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-1", json!(["lib/x.rs"])),
        )
        .unwrap_err();
        assert!(
            err.starts_with("#3 Lib owns \"lib/*\", which \"lib/x.rs\" names too"),
            "{err}"
        );
        let err = live
            .post(
                &mut w,
                0,
                &with_paths(card("Lib too", "x"), json!(["lib/y.rs"])),
            )
            .unwrap_err();
        assert!(err.starts_with("#3 Lib owns \"lib/*\""), "{err}");
        w.cards[2].column = Column::Done;
        let got = sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-1", json!(["lib/x.rs"])),
        );
        assert!(got.is_ok(), "{got:?}");
        // The worker's own open card is not counted: its next card may
        // go on in the same files.
        let got = sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-1", json!(["src/a.rs"])),
        );
        assert!(got.is_ok(), "{got:?}");
        // Another project's card is not counted either.
        w.cards[0].project = 1;
        let got = sent(
            &mut live,
            &mut w,
            0,
            &assign("claude-2", json!(["src/a.rs"])),
        );
        assert!(got.is_ok(), "{got:?}");
    }

    #[test]
    fn a_posted_card_owns_its_paths_and_a_claim_hands_them_over() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.post(
            &mut w,
            0,
            &with_paths(
                card("Add a test", "Cover sub()."),
                json!(["tests/", "lib shared"]),
            ),
        )
        .unwrap();
        assert_eq!(
            w.cards[0].paths,
            [Owned::new("tests"), Owned::shared("lib")]
        );
        let got = live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap();
        assert!(
            got.ends_with(
                "send lead a done message.\n\nThis card owns tests, lib (shared): stay in \
                 them; what else needs changing goes in your done, or ask for the card to \
                 be widened."
            ),
            "{got}"
        );
        let err = live
            .post(&mut w, 0, &with_paths(card("Bad", "x"), json!(["a", "a"])))
            .unwrap_err();
        assert_eq!(err, "paths lists \"a\" twice");
    }
}
