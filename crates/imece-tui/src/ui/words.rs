//! Events, card stories and inbox items worded in the person's language.
//! Every argument goes through `fill`, which cleans agent-written text.

use crate::i18n::{Lang, fill};
use crate::model::*;

fn name(w: &World, id: AgentId) -> &str {
    &w.agents[id].name
}

fn card_ref(c: CardId) -> String {
    format!("#{}", c + 1)
}

pub fn event(lang: Lang, w: &World, e: &Event) -> String {
    let t = lang.text();
    match e {
        Event::Plan { leader, request } => fill(t.ev_plan, &[name(w, *leader), request]),
        Event::Assign { agent, card } => {
            let leader = w.projects[w.agents[*agent].project].leader;
            fill(
                t.ev_assign,
                &[
                    name(w, leader),
                    name(w, *agent),
                    &card_ref(*card),
                    &w.cards[*card].title,
                ],
            )
        }
        Event::Assumption { agent, text } => fill(t.ev_assumption, &[name(w, *agent), text]),
        Event::Blocked {
            agent,
            topic,
            joined: None,
        } => fill(t.ev_blocked, &[name(w, *agent), topic]),
        Event::Blocked {
            agent,
            topic,
            joined: Some(r),
        } => fill(t.ev_blocked_joined, &[name(w, *agent), topic, name(w, *r)]),
        Event::ResearchOpened { researcher, topic } => {
            fill(t.ev_research_opened, &[name(w, *researcher), topic])
        }
        Event::ResearchDelivered { researcher, topic } => {
            fill(t.ev_research_delivered, &[name(w, *researcher), topic])
        }
        Event::ResearchNeedsYou { researcher, topic } => {
            fill(t.ev_research_needs_you, &[name(w, *researcher), topic])
        }
        Event::FromCache { agent, topic } => fill(t.ev_from_cache, &[topic, name(w, *agent)]),
        Event::Sensitive { agent, what } => fill(t.ev_sensitive, &[name(w, *agent), what]),
        Event::Merged { agent, card } => fill(
            t.ev_merged,
            &[&card_ref(*card), &w.cards[*card].title, name(w, *agent)],
        ),
        Event::Failed { agent } => fill(t.ev_failed, &[name(w, *agent)]),
        Event::Approved {
            agent,
            research: false,
        } => fill(t.ev_approved, &[name(w, *agent)]),
        Event::Approved {
            agent,
            research: true,
        } => fill(t.ev_approved_research, &[name(w, *agent)]),
        Event::Rejected { agent } => fill(t.ev_rejected, &[name(w, *agent)]),
        Event::Stopped { agent, why } => fill(t.ev_stopped, &[name(w, *agent), why]),
        Event::Posted { agent, card } => fill(
            t.ev_posted,
            &[name(w, *agent), &card_ref(*card), &w.cards[*card].title],
        ),
        Event::Claimed { agent, card } => fill(
            t.ev_claimed,
            &[name(w, *agent), &card_ref(*card), &w.cards[*card].title],
        ),
        Event::Hired { by, agent, model } => fill(
            t.ev_hired,
            &[
                name(w, *by),
                name(w, *agent),
                model.as_deref().unwrap_or(t.default_model),
            ],
        ),
        Event::Left { agent } => fill(t.ev_left, &[name(w, *agent)]),
        Event::Fallback { agent, model } => fill(t.ev_fallback, &[name(w, *agent), model]),
        Event::Budget { from, to, .. } => fill(t.ev_budget, &[&dollars(*to), &dollars(*from)]),
        Event::Shell { command, ended, .. } => shell(lang, command, *ended),
        Event::WorktreeMoved { agent, from, to } => {
            fill(t.ev_worktree_moved, &[name(w, *agent), from, to])
        }
        Event::WorktreeKept {
            agent,
            at,
            head,
            commits,
        } => {
            let how = if *commits {
                t.ev_worktree_commits
            } else {
                t.ev_worktree_changes
            };
            fill(how, &[name(w, *agent), at, head])
        }
        Event::ChangeReady {
            agent,
            files,
            added,
            removed,
        } => fill(
            t.ev_change_ready,
            &[
                name(w, *agent),
                &files.to_string(),
                &added.to_string(),
                &removed.to_string(),
            ],
        ),
        Event::Applied { agent, files, mark } => {
            fill(t.ev_applied, &[name(w, *agent), &files.to_string(), mark])
        }
        Event::ApplyFailed { agent, why } => fill(t.ev_apply_failed, &[name(w, *agent), why]),
        // Conversations are drawn from `said`, with their speakers coloured.
        Event::Talk { .. } | Event::YouSaid { .. } | Event::Replied { .. } | Event::Sent { .. } => {
            String::new()
        }
    }
}

/// How a command of the person's own went: the journal line, and the
/// flash after it.
pub fn shell(lang: Lang, command: &str, ended: Ended) -> String {
    let t = lang.text();
    match ended {
        Ended::Exit(code) => fill(t.ev_shell, &[command, &code.to_string()]),
        Ended::Signal(sig) => fill(t.ev_shell_signal, &[command, &sig.to_string()]),
    }
}

/// The line under a command's output, before the screen comes back.
pub fn shell_back(lang: Lang, ended: Ended) -> String {
    let t = lang.text();
    match ended {
        Ended::Exit(code) => fill(t.shell_back, &[&code.to_string()]),
        Ended::Signal(sig) => fill(t.shell_back_signal, &[&sig.to_string()]),
    }
}

pub fn story(lang: Lang, w: &World, card: &Card, e: &StoryEvent) -> String {
    let t = lang.text();
    match e {
        StoryEvent::Took { agent } => fill(t.st_took, &[name(w, *agent), &card.title]),
        StoryEvent::Decision(s) | StoryEvent::Assumption(s) => fill("{}", &[s]),
        StoryEvent::WritingTests => t.st_writing_tests.to_string(),
        StoryEvent::BlockedOn(topic) => fill(t.st_blocked_on, &[topic]),
        StoryEvent::ResearchArrived { researcher, topic } => {
            fill(t.st_research_arrived, &[name(w, *researcher), topic])
        }
        StoryEvent::ResearchNeedsYou { researcher } => {
            fill(t.st_research_needs_you, &[name(w, *researcher)])
        }
        StoryEvent::FromCache(topic) => fill(t.st_from_cache, &[topic]),
        StoryEvent::Change { files, plus, minus } => fill(
            t.st_change,
            &[&files.to_string(), &plus.to_string(), &minus.to_string()],
        ),
        StoryEvent::Sensitive(what) => fill(t.st_sensitive, &[what]),
        StoryEvent::Verified { tests, trunk } => {
            fill(t.st_verified, &[&tests.to_string(), &trunk.to_string()])
        }
        StoryEvent::QueueFailed => t.st_queue_failed.to_string(),
        StoryEvent::YouApproved { research: false } => t.st_you_approved.to_string(),
        StoryEvent::YouApproved { research: true } => t.st_you_approved_research.to_string(),
        StoryEvent::YouRejected { research: false } => t.st_you_rejected.to_string(),
        StoryEvent::YouRejected { research: true } => t.st_you_rejected_research.to_string(),
        StoryEvent::Touched(file) => fill(t.st_touched, &[file]),
        StoryEvent::Stopped(why) => fill(t.st_stopped, &[why]),
        StoryEvent::Posted { by, text } => fill(t.st_posted, &[name(w, *by), text]),
        StoryEvent::Asked { .. }
        | StoryEvent::Answered { .. }
        | StoryEvent::YouSaid(_)
        | StoryEvent::Replied(_)
        | StoryEvent::Sent { .. } => String::new(),
    }
}

/// One line of a conversation, uncleaned: the drawing cleans it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said<'a> {
    pub from: Party,
    pub to: Party,
    pub kind: Option<MsgKind>,
    pub text: &'a str,
}

impl Said<'_> {
    /// The two parties in a fixed order, so A → B and B → A are one thread.
    pub fn pair(&self) -> (Party, Party) {
        (self.from.min(self.to), self.from.max(self.to))
    }
}

/// The conversation line in a journal event, if it is one.
pub fn said(e: &Event) -> Option<Said<'_>> {
    use Party::{Agent, Person};
    let s = |from, to, kind, text| {
        Some(Said {
            from,
            to,
            kind,
            text,
        })
    };
    match e {
        Event::Talk { from, to, text } => s(Agent(*from), Agent(*to), None, text),
        Event::YouSaid { to, text } => s(Person, Agent(*to), None, text),
        Event::Replied { agent, text } => s(Agent(*agent), Person, None, text),
        Event::Sent {
            from,
            to,
            kind,
            text,
        } => s(Agent(*from), Agent(*to), Some(*kind), text),
        _ => None,
    }
}

/// The conversation line in agent `me`'s card story, if it is one.
pub fn said_in_story(me: AgentId, e: &StoryEvent) -> Option<Said<'_>> {
    use Party::{Agent, Person};
    let s = |from, to, kind, text| {
        Some(Said {
            from,
            to,
            kind,
            text,
        })
    };
    match e {
        StoryEvent::Asked { from, to, text } => s(Agent(*from), Agent(*to), None, text),
        StoryEvent::Answered { from, text } => s(Agent(*from), Agent(me), None, text),
        StoryEvent::YouSaid(text) => s(Person, Agent(me), None, text),
        StoryEvent::Replied(text) => s(Agent(me), Person, None, text),
        StoryEvent::Sent {
            from,
            to,
            kind,
            text,
        } => s(Agent(*from), Agent(*to), Some(*kind), text),
        _ => None,
    }
}

pub fn inbox(lang: Lang, w: &World, item: &InboxItem) -> String {
    let t = lang.text();
    match (item.kind, item.researcher) {
        (InboxKind::Research, Some(r)) => fill(
            t.in_research,
            &[name(w, r), &item.detail, name(w, item.agent)],
        ),
        (InboxKind::Reply, _) => fill(t.in_reply, &[name(w, item.agent), &item.detail]),
        (InboxKind::Change, _) => {
            let (files, added, removed) = w.change_of(item.agent).map_or((0, 0, 0), |c| {
                let (a, r) = c.counts();
                (c.files.len(), a, r)
            });
            fill(
                t.in_change,
                &[
                    name(w, item.agent),
                    &item.detail,
                    &files.to_string(),
                    &added.to_string(),
                    &removed.to_string(),
                ],
            )
        }
        _ => fill("{} {}", &[name(w, item.agent), &item.detail]),
    }
}
