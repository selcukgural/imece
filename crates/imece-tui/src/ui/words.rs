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
        Event::Talk { from, to, text } => fill(t.ev_talk, &[name(w, *from), name(w, *to), text]),
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
        Event::YouSaid { to, text } => fill(t.ev_you_said, &[name(w, *to), text]),
        Event::Replied { agent, text } => fill(t.ev_replied, &[name(w, *agent), text]),
        Event::Stopped { agent, why } => fill(t.ev_stopped, &[name(w, *agent), why]),
    }
}

pub fn story(lang: Lang, w: &World, card: &Card, e: &StoryEvent) -> String {
    let t = lang.text();
    match e {
        StoryEvent::Took { agent } => fill(t.st_took, &[name(w, *agent), &card.title]),
        StoryEvent::Decision(s) | StoryEvent::Assumption(s) => fill("{}", &[s]),
        StoryEvent::WritingTests => t.st_writing_tests.to_string(),
        StoryEvent::Asked { from, to, text } => {
            fill(t.st_asked, &[name(w, *from), name(w, *to), text])
        }
        StoryEvent::Answered { from, text } => fill(t.st_answered, &[name(w, *from), text]),
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
        StoryEvent::YouSaid(text) => fill(t.st_you_said, &[text]),
        StoryEvent::Replied(text) => fill(t.st_replied, &[text]),
        StoryEvent::Touched(file) => fill(t.st_touched, &[file]),
        StoryEvent::Stopped(why) => fill(t.st_stopped, &[why]),
    }
}

pub fn inbox(lang: Lang, w: &World, item: &InboxItem) -> String {
    let t = lang.text();
    match (item.kind, item.researcher) {
        (InboxKind::Research, Some(r)) => fill(
            t.in_research,
            &[name(w, r), &item.detail, name(w, item.agent)],
        ),
        _ => fill("{} {}", &[name(w, item.agent), &item.detail]),
    }
}
