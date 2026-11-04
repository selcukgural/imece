//! Events, card stories and inbox items worded in the person's language.
//! Every argument goes through `fill`, which cleans agent-written text.

use crate::app::Rung;
use crate::editor::{self, Why};
use crate::i18n::{Lang, fill};
use crate::model::*;
use crate::screen::Progress;
use crate::shell::Output;

/// The name of the project agent `id` belongs to.
fn project(w: &World, id: AgentId) -> &str {
    &w.projects[w.agents[id].project].name
}

fn name(w: &World, id: AgentId) -> &str {
    &w.agents[id].name
}

fn card_ref(c: CardId) -> String {
    format!("#{}", c + 1)
}

/// A size in bytes in the largest unit that keeps it whole: `512 B`,
/// `3 KiB`, `40 MiB`, `2 GiB`.
pub fn size(bytes: u64) -> String {
    match bytes {
        b if b < 1 << 10 => format!("{b} B"),
        b if b < 1 << 20 => format!("{} KiB", b >> 10),
        b if b < 1 << 30 => format!("{} MiB", b >> 20),
        b => format!("{} GiB", b >> 30),
    }
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
        Event::Decided {
            agent,
            what,
            allowed,
        } => fill(
            if *allowed { t.ev_allowed } else { t.ev_refused },
            &[name(w, *agent), &one_line(what)],
        ),
        Event::TurnYes { agent } => fill(t.ev_turn_yes, &[name(w, *agent)]),
        Event::TurnYesEnded { agent, allowed } => fill(
            t.ev_turn_yes_ended,
            &[name(w, *agent), &allowed.to_string()],
        ),
        Event::Standing {
            leader,
            rules,
            added,
        } => fill(
            if *added {
                t.ev_standing
            } else {
                t.ev_unstanding
            },
            &[
                &w.projects[w.agents[*leader].project].name,
                &rules.join(", "),
            ],
        ),
        Event::Stopped { agent, why } => fill(t.ev_stopped, &[name(w, *agent), why]),
        Event::Interrupted { agent } => fill(t.ev_interrupted, &[name(w, *agent)]),
        Event::Retrying {
            agent,
            why,
            attempt,
            seconds,
        } => fill(
            t.ev_retrying,
            &[
                name(w, *agent),
                &one_line(why),
                &took_words(lang, seconds * 1000),
                &attempt.to_string(),
                &RETRY_TICKS.len().to_string(),
            ],
        ),
        Event::Posted { agent, card } => fill(
            t.ev_posted,
            &[name(w, *agent), &card_ref(*card), &w.cards[*card].title],
        ),
        Event::Claimed { agent, card } => fill(
            t.ev_claimed,
            &[name(w, *agent), &card_ref(*card), &w.cards[*card].title],
        ),
        Event::Hired {
            by,
            agent,
            model,
            from,
        } => {
            let model = model.as_deref().unwrap_or(t.default_model);
            match from {
                Some(f) => fill(t.ev_hired_from, &[name(w, *by), name(w, *agent), model, f]),
                None => fill(t.ev_hired, &[name(w, *by), name(w, *agent), model]),
            }
        }
        Event::Forked { from, agent } => fill(t.ev_forked, &[name(w, *agent), name(w, *from)]),
        Event::Left { agent } => fill(t.ev_left, &[name(w, *agent)]),
        Event::Fallback { agent, model } => fill(t.ev_fallback, &[name(w, *agent), model]),
        Event::Resumed { agent } => fill(t.ev_resumed, &[name(w, *agent)]),
        Event::Restarted { agent, why } => match why {
            Fresh::NoSession => fill(t.ev_restarted, &[name(w, *agent)]),
            Fresh::Missing => fill(t.ev_restarted_missing, &[name(w, *agent)]),
            Fresh::Refused(said) => fill(t.ev_restarted_refused, &[name(w, *agent), said]),
        },
        Event::Budget { from, to, .. } => fill(t.ev_budget, &[&dollars(*to), &dollars(*from)]),
        Event::PotLow { rate, minutes, .. } => {
            fill(t.ev_pot_low, &[&dollars(*rate), &ago(lang, minutes * 60)])
        }
        Event::Shell {
            command,
            ended,
            took,
            ..
        } => shell(lang, command, *ended, *took),
        Event::Opened { leader, path } => fill(t.ev_opened, &[project(w, *leader), path]),
        Event::NoGit { leader, path } => fill(t.ev_no_git, &[name(w, *leader), path]),
        Event::Closed { leader } => fill(t.ev_closed, &[project(w, *leader)]),
        Event::Archived { leader, title, .. } => fill(
            t.ev_archived,
            &[project(w, *leader), title, project(w, *leader)],
        ),
        Event::Restored {
            leader,
            title,
            swapped: None,
        } => fill(t.ev_restored, &[project(w, *leader), title]),
        Event::Restored {
            leader,
            title,
            swapped: Some(was),
        } => fill(t.ev_restored_swapped, &[project(w, *leader), title, was]),
        Event::Renamed { from, to, .. } => fill(t.ev_renamed, &[from, to]),
        Event::Forgotten { agent } => fill(t.ev_forgotten, &[name(w, *agent)]),
        Event::Chose { agent, choice } => fill(t.ev_chose, &[name(w, *agent), choice]),
        Event::Confine { level, .. } => {
            if level == "open" {
                t.ev_confine_open.to_string()
            } else {
                t.ev_confine_standard.to_string()
            }
        }
        Event::CommandsMissing { agent } => {
            let vendor = w.agents[*agent].vendor.label();
            fill(t.ev_commands_missing, &[name(w, *agent), vendor])
        }
        Event::SetupRan {
            agent,
            ok,
            end,
            lines,
        } => {
            let last = lines.last().map_or(t.setup_no_output, String::as_str);
            let text = if *ok { t.ev_setup_ran } else { t.ev_setup_bad };
            fill(text, &[name(w, *agent), end, last])
        }
        Event::SetupFailed { agent, why } => fill(t.ev_setup_failed, &[name(w, *agent), why]),
        Event::SetupAnswered { agent, yes } => {
            let text = if *yes { t.ev_setup_yes } else { t.ev_setup_no };
            fill(text, &[name(w, *agent)])
        }
        Event::Pushed { words, sent, .. } => {
            fill(if *sent { t.ev_pushed } else { t.ev_pushed_demo }, &[words])
        }
        Event::PushFailed { why, .. } => fill(t.ev_push_failed, &[why]),
        Event::InstructionsCut { agent, path, size } => fill(
            t.ev_instructions_cut,
            &[
                name(w, *agent),
                path,
                &size.div_ceil(1024).to_string(),
                &crate::live::MAX_NOTES_KIB.to_string(),
            ],
        ),
        Event::NotesCut { agent, file, size } => fill(
            t.ev_notes_cut,
            &[
                name(w, *agent),
                file,
                &size.div_ceil(1024).to_string(),
                &crate::live::MAX_NOTES_KIB.to_string(),
            ],
        ),
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
        Event::Suggested { model, cost, .. } => fill(t.ev_suggested, &[model, &dollars(*cost)]),
        Event::SuggestFailed { why, cost, .. } => {
            fill(t.ev_suggest_failed, &[&dollars(*cost), why])
        }
        Event::Reviewed {
            agent,
            by,
            model,
            text,
            cost,
            ..
        } => fill(
            t.ev_reviewed,
            &[
                by.label(),
                model,
                name(w, *agent),
                &dollars(*cost),
                text.lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or_default(),
            ],
        ),
        Event::ReviewFailed { agent, why, cost } => {
            fill(t.ev_review_failed, &[name(w, *agent), &dollars(*cost), why])
        }
        Event::Copied {
            agent,
            paths,
            bytes,
            included,
        } => fill(
            if *included {
                t.ev_included
            } else {
                t.ev_copied
            },
            &[&paths.join(", "), name(w, *agent), &size(*bytes)],
        ),
        Event::FilesCut { leader, kept } => fill(
            t.ev_files_cut,
            &[
                &w.projects[w.agents[*leader].project].name,
                &kept.to_string(),
                &kept.to_string(),
            ],
        ),
        Event::CopySkipped {
            agent,
            path,
            why,
            included,
        } => fill(
            if *included {
                t.ev_include_skipped
            } else {
                t.ev_copy_skipped
            },
            &[path, name(w, *agent), why],
        ),
        Event::IncludeRefused { agent, line, why } => match line {
            Some((n, text)) => fill(
                t.ev_include_line_refused,
                &[&n.to_string(), text, name(w, *agent), why],
            ),
            None => fill(t.ev_include_refused, &[name(w, *agent), why]),
        },
        // Conversations are drawn from `said`, with their speakers coloured.
        Event::Talk { .. }
        | Event::YouSaid { .. }
        | Event::Replied { .. }
        | Event::Sent { .. }
        | Event::ToYou { .. } => String::new(),
    }
}

/// How a command of the person's own went: the journal line, and the
/// flash after it.
pub fn shell(lang: Lang, command: &str, ended: Ended, took: Option<u64>) -> String {
    let t = lang.text();
    let said = match ended {
        Ended::Exit(code) => fill(t.ev_shell, &[command, &code.to_string()]),
        Ended::Signal(sig) => fill(t.ev_shell_signal, &[command, &sig.to_string()]),
    };
    match took {
        Some(ms) => fill(t.ev_shell_took, &[&said, &took_words(lang, ms)]),
        None => said,
    }
}

/// Why the editor gave the draft back as it was (`^x ^e`, `:edit`): an
/// exit, with 127 (the shell found no such program) said as a missing
/// editor; a signal; too much or not text; what could not be read or
/// written.
pub fn editor_why(lang: Lang, editor: &str, why: &Why) -> String {
    let t = lang.text();
    match why {
        Why::Ended(Ended::Exit(127)) => fill(t.editor_missing, &[editor]),
        Why::Ended(Ended::Exit(code)) => fill(t.editor_ended, &[editor, &code.to_string()]),
        Why::Ended(Ended::Signal(sig)) => fill(t.editor_signal, &[editor, &sig.to_string()]),
        Why::Long => fill(t.editor_long, &[editor, &(editor::MAX / 1024).to_string()]),
        Why::NotText => fill(t.editor_not_text, &[editor]),
        Why::Io(e) => fill(t.editor_failed, &[editor, &e.to_string()]),
    }
}

/// A `!` command's output quoted in a message (`O`), from the person to
/// an agent: how it ran, then its last lines in a `text` fence longer
/// than any run of backticks in them (so none closes it early), the
/// lines above them counted first. The lines are cleaned already; the
/// command is cleaned here.
pub fn output_quote(lang: Lang, o: &Output) -> String {
    let t = lang.text();
    let said = match o.ended {
        Ended::Exit(code) => fill(t.output_ran, &[&o.command, &code.to_string()]),
        Ended::Signal(sig) => fill(t.output_ran_signal, &[&o.command, &sig.to_string()]),
    };
    let mut out = fill(t.ev_shell_took, &[&said, &took_words(lang, o.took)]);
    if o.lines.is_empty() {
        return out;
    }
    let ticks = (o.lines.iter())
        .map(|l| l.split(|c| c != '`').map(str::len).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(ticks.max(2) + 1);
    out.push('\n');
    out.push_str(&fence);
    out.push_str("text\n");
    if o.above > 0 {
        out.push_str(&fill(t.output_above, &[&o.above.to_string()]));
        out.push('\n');
    }
    for l in &o.lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&fence);
    out
}

/// The line under a command's output, before the screen comes back:
/// how it ended and how long it took.
pub fn shell_back(lang: Lang, ended: Ended, took: u64) -> String {
    let t = lang.text();
    let took = took_words(lang, took);
    match ended {
        Ended::Exit(code) => fill(t.shell_back, &[&code.to_string(), &took]),
        Ended::Signal(sig) => fill(t.shell_back_signal, &[&sig.to_string(), &took]),
    }
}

/// How long something ran, `ms` milliseconds, in the largest units that
/// say it: milliseconds under a second, whole seconds under a minute,
/// minutes and seconds under an hour, else hours and minutes.
pub fn took_words(lang: Lang, ms: u64) -> String {
    let t = lang.text();
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    if seconds == 0 {
        fill(t.took_ms, &[&ms.to_string()])
    } else if minutes == 0 {
        fill(t.took_s, &[&seconds.to_string()])
    } else if minutes < 60 {
        fill(
            t.took_min,
            &[&minutes.to_string(), &(seconds % 60).to_string()],
        )
    } else {
        fill(
            t.ago_hours,
            &[&(minutes / 60).to_string(), &(minutes % 60).to_string()],
        )
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
        StoryEvent::Interrupted => t.st_interrupted.to_string(),
        StoryEvent::Posted { by, text } => fill(t.st_posted, &[name(w, *by), text]),
        StoryEvent::Asked { .. }
        | StoryEvent::Answered { .. }
        | StoryEvent::YouSaid(_)
        | StoryEvent::Replied(_)
        | StoryEvent::Sent { .. }
        | StoryEvent::ToYou { .. } => String::new(),
    }
}

/// One line of a conversation, uncleaned: the drawing cleans it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said<'a> {
    pub from: Party,
    pub to: Party,
    pub kind: Option<MsgKind>,
    pub text: &'a str,
    /// A message's number on the bus (0: none) and the number of the one
    /// it replies to.
    pub id: MsgId,
    pub re: Option<MsgId>,
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
            id: 0,
            re: None,
        })
    };
    match e {
        Event::Talk { from, to, text } => s(Agent(*from), Agent(*to), None, text),
        Event::YouSaid { to, text } => s(Person, Agent(*to), None, text),
        Event::Replied { agent, text } => s(Agent(*agent), Person, None, text),
        Event::ToYou { from, kind, text } => s(Agent(*from), Person, Some(*kind), text),
        Event::Sent {
            from,
            to,
            kind,
            text,
            id,
            re,
        } => Some(Said {
            id: *id,
            re: *re,
            ..s(Agent(*from), Agent(*to), Some(*kind), text)?
        }),
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
            id: 0,
            re: None,
        })
    };
    match e {
        StoryEvent::Asked { from, to, text } => s(Agent(*from), Agent(*to), None, text),
        StoryEvent::Answered { from, text } => s(Agent(*from), Agent(me), None, text),
        StoryEvent::YouSaid(text) => s(Person, Agent(me), None, text),
        StoryEvent::Replied(text) => s(Agent(me), Person, None, text),
        StoryEvent::ToYou { kind, text } => s(Agent(me), Person, Some(*kind), text),
        StoryEvent::Sent {
            from,
            to,
            kind,
            text,
            id,
            re,
        } => Some(Said {
            id: *id,
            re: *re,
            ..s(Agent(*from), Agent(*to), Some(*kind), text)?
        }),
        _ => None,
    }
}

/// A span of `seconds` as the person reads it: minutes under an hour,
/// hours and minutes under a day, days and hours from then on.
/// The terminal's title: what a tab in the background shows, `waiting`
/// being the count of open inbox items (the header's).
pub fn title(lang: Lang, waiting: usize, progress: Progress) -> String {
    let words = if waiting == 0 {
        "imece".to_string()
    } else {
        fill(lang.text().title_waiting, &[&waiting.to_string()])
    };
    // The ladder's top first, in the glyphs of the header's fleet and the
    // rows, so a tab in the background says it; calm, imece's mark
    // (record 101: five dots, as the mark's hands).
    let glyph = match progress {
        Progress::Stuck => "✕ ",
        Progress::Waiting => "! ",
        Progress::Working => "● ",
        Progress::Calm => "⁙ ",
    };
    format!("{glyph}{words}")
}

/// The line imece leaves in the terminal when it quits (record 101):
/// imece's mark and name, what waits for the person, the agents that
/// sleep until the next start (the demo keeps none), what this run spent,
/// and how to go on.
pub fn farewell(lang: Lang, w: &World, spent: u64, demo: bool) -> String {
    let t = lang.text();
    let mut parts = vec!["⁙ imece".to_string()];
    let waiting = w.open_inbox().len();
    if waiting > 0 {
        parts.push(fill(t.bye_waiting, &[&waiting.to_string()]));
    }
    let asleep = (w.agents.iter())
        .filter(|a| !a.gone && !w.put_away(a.id))
        .count();
    if !demo && asleep > 0 {
        parts.push(match asleep {
            1 => t.bye_asleep_one.to_string(),
            n => fill(t.bye_asleep, &[&n.to_string()]),
        });
    }
    if spent > 0 {
        parts.push(fill(t.bye_spent, &[&crate::model::dollars(spent)]));
    }
    parts.push(if demo { t.bye_demo } else { t.bye_resume }.to_string());
    parts.join(" · ")
}

/// What a worktree no agent owns holds, as its row in `:tidy` says it:
/// clean, else its files changed and its own commits, the ones it has.
pub fn held(lang: Lang, o: &Orphan) -> String {
    let t = lang.text();
    if o.clean() {
        return t.tidy_clean.to_string();
    }
    let mut parts = Vec::new();
    if o.changes > 0 {
        parts.push(fill(t.tidy_changes, &[&o.changes.to_string()]));
    }
    if o.commits > 0 {
        parts.push(fill(t.tidy_commits, &[&o.commits.to_string()]));
    }
    parts.join(" · ")
}

/// A removal's end in one line: the names removed, then those kept with
/// why (git's words, or that an agent works in it now).
pub fn tidied(lang: Lang, done: &Tidied) -> String {
    let t = lang.text();
    let mut parts = Vec::new();
    if !done.removed.is_empty() {
        parts.push(fill(t.tidy_removed, &[&done.removed.join(", ")]));
    }
    if !done.kept.is_empty() {
        let kept: Vec<String> = (done.kept.iter())
            .map(|(name, why)| format!("{name} ({})", why.as_deref().unwrap_or(t.tidy_owned)))
            .collect();
        parts.push(fill(t.tidy_kept, &[&kept.join(", ")]));
    }
    for (name, how) in &done.torn {
        parts.push(match how {
            TornDown::Ran { ok, end, last } => {
                let said = match last {
                    Some(l) => format!("{end} · {l}"),
                    None => end.clone(),
                };
                let words = if *ok { t.torn_ran } else { t.torn_bad };
                fill(words, &[name, &said])
            }
            TornDown::Failed(why) => fill(t.torn_failed, &[name, why]),
            TornDown::Refused => fill(t.torn_refused, &[name]),
        });
    }
    parts.join(" · ")
}

/// `:why`'s line about agent `id`: its rung, since when and the
/// journal's line that put it there (`App::why`), in the journal's words.
pub fn why(
    lang: Lang,
    w: &World,
    id: AgentId,
    (rung, since, line): (Rung, u64, Option<usize>),
) -> String {
    let t = lang.text();
    let state = t.rungs[rung as usize];
    let at = crate::model::when(since, crate::model::day(w.tick));
    let cause = line.map_or_else(
        || t.why_unsaid.to_string(),
        |i| event(lang, w, &w.journal[i].event),
    );
    let said = fill(t.why, &[&w.agents[id].name, state, &at, &cause]);
    match w.agents[id].held_mail {
        0 => said,
        n => format!("{said} · {}", fill(t.why_held, &[&n.to_string()])),
    }
}

pub fn ago(lang: Lang, seconds: u64) -> String {
    let t = lang.text();
    let minutes = seconds / 60;
    let hours = minutes / 60;
    if hours == 0 {
        fill(t.ago_minutes, &[&minutes.to_string()])
    } else if hours < 24 {
        fill(
            t.ago_hours,
            &[&hours.to_string(), &(minutes % 60).to_string()],
        )
    } else {
        fill(
            t.ago_days,
            &[&(hours / 24).to_string(), &(hours % 24).to_string()],
        )
    }
}

/// A vendor plan's usage window `l` as the header says it, `now` being
/// the world's seconds: the vendor, the window's length, what is left of
/// it, and when it starts again (when said and still ahead).
pub fn limit(lang: Lang, l: &Limit, now: u64) -> String {
    let t = lang.text();
    let hours = l.minutes / 60;
    let window = if hours < 24 {
        fill(t.window_hours, &[&hours.to_string()])
    } else {
        fill(t.window_days, &[&(hours / 24).to_string()])
    };
    let left = (100 - l.used.min(100)).to_string();
    let vendor = l.vendor.label();
    match l.resets_at.filter(|&r| r > now) {
        Some(r) => fill(
            t.limit_resets,
            &[vendor, &window, &left, &ago(lang, r - now)],
        ),
        None => fill(t.limit, &[vendor, &window, &left]),
    }
}

/// A command on one row, its lines joined with a mark so none is hidden:
/// what the person is asked to allow is shown as it is, never as
/// markdown.
pub(crate) fn one_line(text: &str) -> String {
    text.trim().lines().collect::<Vec<_>>().join(" ↵ ")
}

/// What rings in another window for `note`: the kind of what waits and
/// the agent's name, never the agent's words.
pub fn notice(lang: Lang, w: &World, note: crate::notify::Note) -> String {
    let t = lang.text();
    let (text, agent) = match note {
        crate::notify::Note::Stuck(id) => (t.notice_stuck, id),
        crate::notify::Note::Item(i) => {
            let item = &w.inbox[i];
            let text = match item.kind {
                InboxKind::Change => t.notice_change,
                InboxKind::Question => t.notice_question,
                InboxKind::Reply => t.notice_reply,
                InboxKind::Approval => t.notice_approval,
                InboxKind::Fault => t.notice_fault,
                InboxKind::Script => t.notice_script,
                InboxKind::Sensitive | InboxKind::Research => t.notice_sensitive,
            };
            (text, item.agent)
        }
    };
    fill(text, &[name(w, agent)])
}

/// What a push to the person's phone says of `note`: [`notice`]'s words
/// and the project, nothing an agent wrote.
pub fn pushed(lang: Lang, w: &World, note: crate::notify::Note) -> String {
    let agent = match note {
        crate::notify::Note::Stuck(id) => id,
        crate::notify::Note::Item(i) => w.inbox[i].agent,
    };
    let project = &w.projects[w.agents[agent].project].name;
    fill(lang.text().push_one, &[&notice(lang, w, note), project])
}

/// Why `:notify push` was refused, in the person's language.
pub fn push_why(lang: Lang, why: &crate::push::Why) -> String {
    use crate::push::Why;
    let t = lang.text();
    match why {
        Why::NoFile(path) => fill(t.push_no_file, &[path]),
        Why::Unreadable(e) => fill(t.push_unreadable, &[e]),
        Why::Line(n) => fill(t.push_line, &[&n.to_string()]),
        Why::NoTopic => t.push_no_topic.to_string(),
        Why::Server(s) => fill(t.push_server, &[s]),
        Why::Topic => fill(t.push_topic, &[&crate::push::TOPIC_MAX.to_string()]),
        Why::Guessable => fill(t.push_guessable, &[&crate::push::TOPIC_MIN.to_string()]),
        Why::Token(e) => fill(t.push_token, &[e]),
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
        (InboxKind::Question, _) => fill(
            t.in_question,
            &[name(w, item.agent), &super::plain(&item.detail)],
        ),
        (InboxKind::Approval, _) => fill(
            t.in_approval,
            &[name(w, item.agent), &one_line(&item.detail)],
        ),
        (InboxKind::Fault, _) => fill(t.in_fault, &[name(w, item.agent), &item.detail]),
        (InboxKind::Script, _) => fill(t.in_script, &[name(w, item.agent), &item.detail]),
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

/// What the runs on a change's tree say, after its inbox line: that all
/// ran clean (which), which failed, or that none ran on it.
pub fn evidence(lang: Lang, runs: &[Ran]) -> String {
    let t = lang.text();
    let lines = |ok: bool| {
        runs.iter()
            .filter(|r| r.ok() == ok)
            .map(|r| r.line.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if runs.is_empty() {
        t.ran_none.to_string()
    } else if runs.iter().all(Ran::ok) {
        fill(t.ran_clean, &[&lines(true)])
    } else {
        fill(t.ran_failed, &[&lines(false)])
    }
}

/// One run on a change's tree and how it ended.
pub fn ran(lang: Lang, r: &Ran) -> String {
    let t = lang.text();
    match r.code {
        Some(0) => fill(t.ran_ok, &[&r.line]),
        Some(c) => fill(t.ran_exit, &[&r.line, &c.to_string()]),
        None if r.timed_out => fill(t.ran_deadline, &[&r.line]),
        None => fill(t.ran_killed, &[&r.line]),
    }
}

/// A token count in a few characters: `980`, `144k`, `1.2M`.
pub(crate) fn tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let tenths = n / 100_000;
        if tenths.is_multiple_of(10) {
            format!("{}M", tenths / 10)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// How full a context is, for the trace: `72% · 144k of 200k`, or the
/// tokens alone when the window's size is not known.
pub(crate) fn fill_words(f: &Fill) -> String {
    match (f.percent(), f.size) {
        (Some(p), Some(size)) => format!("{p}% · {} of {}", tokens(f.used), tokens(size)),
        _ => format!("{} tokens", tokens(f.used)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setup_script_s_lines_say_who_how_it_ended_and_its_last_words() {
        let w = crate::live::testing::team(1);
        let say = |e: Event| event(Lang::En, &w, &e);
        let ran = |ok, lines: &[&str]| Event::SetupRan {
            agent: 1,
            ok,
            end: "exit 0, 1.5 s".into(),
            lines: lines.iter().map(|l| l.to_string()).collect(),
        };
        assert_eq!(
            say(ran(true, &["a", "b"])),
            "claude-1: worktree setup ran, exit 0, 1.5 s · b"
        );
        assert_eq!(
            say(ran(false, &[])),
            "claude-1: worktree setup failed, exit 0, 1.5 s · no output"
        );
        assert_eq!(
            say(Event::SetupFailed {
                agent: 1,
                why: "x".into()
            }),
            "claude-1: worktree setup did not run: x"
        );
        assert!(
            say(Event::SetupAnswered {
                agent: 1,
                yes: true
            })
            .starts_with("you said yes")
        );
        assert!(
            say(Event::SetupAnswered {
                agent: 1,
                yes: false
            })
            .starts_with("you said no")
        );
        assert_eq!(ran(true, &[]).kind(), EntryKind::Assumption);
        assert_eq!(ran(false, &[]).kind(), EntryKind::Failed);
        assert_eq!(
            event(Lang::Tr, &w, &ran(true, &[])),
            "claude-1: worktree kurulumu çalıştı, exit 0, 1.5 s · çıktı yok"
        );
        let mut w = w;
        w.inbox.push(InboxItem {
            kind: InboxKind::Script,
            agent: 1,
            detail: ".imece/worktree-setup.sh".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(
            notice(Lang::En, &w, crate::notify::Note::Item(0)),
            "imece · claude-1: a worktree setup script waits for your yes"
        );
    }

    #[test]
    fn a_runs_time_is_said_in_the_largest_units_that_say_it() {
        let en = |ms| took_words(Lang::En, ms);
        assert_eq!(en(0), "0 ms");
        assert_eq!(en(999), "999 ms");
        assert_eq!(en(1_000), "1 s");
        assert_eq!(en(59_999), "59 s");
        assert_eq!(en(60_000), "1 min 0 s");
        assert_eq!(en(125_000), "2 min 5 s");
        assert_eq!(en(3_599_999), "59 min 59 s");
        assert_eq!(en(3_600_000), "1 h 0 min");
        assert_eq!(en(3_725_000), "1 h 2 min");
        let tr = |ms| took_words(Lang::Tr, ms);
        assert_eq!(tr(40), "40 ms");
        assert_eq!(tr(2_000), "2 sn");
        assert_eq!(tr(125_000), "2 dk 5 sn");
        assert_eq!(tr(7_380_000), "2 sa 3 dk");
    }

    #[test]
    fn the_title_names_imece_and_counts_what_waits_in_both_languages() {
        assert_eq!(title(Lang::En, 0, Progress::Calm), "⁙ imece");
        assert_eq!(title(Lang::Tr, 0, Progress::Calm), "⁙ imece");
        assert_eq!(title(Lang::En, 1, Progress::Calm), "⁙ imece · 1 waiting");
        assert_eq!(title(Lang::En, 12, Progress::Calm), "⁙ imece · 12 waiting");
        assert_eq!(
            title(Lang::Tr, 3, Progress::Calm),
            "⁙ imece · 3 seni bekliyor"
        );
    }

    #[test]
    fn the_farewell_says_what_waits_who_sleeps_what_the_run_spent_and_how_to_go_on() {
        let mut w = crate::live::testing::team(2);
        assert_eq!(
            farewell(Lang::En, &w, 0, false),
            "⁙ imece · 3 agents asleep · resume: imece"
        );
        assert_eq!(
            farewell(Lang::Tr, &w, 24_000, false),
            "⁙ imece · 3 agent uykuda · bu oturumda $2.40 · devam: imece"
        );
        w.agents[1].gone = true;
        w.agents[2].gone = true;
        assert_eq!(
            farewell(Lang::En, &w, 1, false),
            "⁙ imece · 1 agent asleep · <$0.01 spent this run · resume: imece"
        );
        w.agents[0].gone = true;
        assert_eq!(farewell(Lang::En, &w, 0, false), "⁙ imece · resume: imece");
        w.agents[0].gone = false;
        w.projects[0].closed = true;
        assert_eq!(
            farewell(Lang::En, &w, 0, false),
            "⁙ imece · resume: imece",
            "a closed project's agents are not this run's"
        );
        w.projects[0].closed = false;
        for detail in ["which?", "and?"] {
            w.inbox.push(InboxItem {
                kind: InboxKind::Question,
                agent: 0,
                detail: detail.into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
        }
        assert_eq!(
            farewell(Lang::Tr, &w, 0, false),
            "⁙ imece · 2 seni bekliyor · 1 agent uykuda · devam: imece"
        );
        assert_eq!(
            farewell(Lang::En, &w, 8_500, true),
            "⁙ imece · 2 waiting for you · $0.85 spent this run · the demo keeps nothing"
        );
        assert_eq!(
            farewell(Lang::Tr, &w, 0, true),
            "⁙ imece · 2 seni bekliyor · demo hiçbir şey saklamaz"
        );
    }

    #[test]
    fn a_new_session_says_why_the_old_one_was_not_resumed() {
        let w = crate::live::testing::team(1);
        let said = |lang, why| event(lang, &w, &Event::Restarted { agent: 1, why });
        assert_eq!(
            said(Lang::En, Fresh::NoSession),
            "claude-1 starts a new session, told where it left off"
        );
        assert_eq!(
            said(Lang::En, Fresh::Missing),
            "claude-1 starts a new session, told where it left off: its last one was not found"
        );
        assert_eq!(
            said(Lang::En, Fresh::Refused("no rollout\x1b[2J found".into())),
            "claude-1 starts a new session, told where it left off: its last one could not \
             be resumed (no rollout [2J found)"
        );
        assert_eq!(
            said(Lang::Tr, Fresh::Missing),
            "claude-1 yeni bir oturumla başlıyor, kaldığı yer anlatıldı: son oturumu bulunamadı"
        );
        assert_eq!(
            said(Lang::Tr, Fresh::Refused("x".into())),
            "claude-1 yeni bir oturumla başlıyor, kaldığı yer anlatıldı: son oturumu \
             sürdürülemedi (x)"
        );
    }

    #[test]
    fn a_suggestion_is_said_with_its_model_and_cost_and_a_failure_with_why() {
        let w = crate::live::testing::team(1);
        let ok = Event::Suggested {
            leader: 0,
            model: "haiku".into(),
            cost: 420,
        };
        assert_eq!(
            event(Lang::En, &w, &ok),
            "haiku suggested a commit message for what is staged, for $0.04"
        );
        assert_eq!(
            event(Lang::Tr, &w, &ok),
            "haiku stage'dekiler için bir commit mesajı önerdi, $0.04 tuttu"
        );
        let failed = Event::SuggestFailed {
            leader: 0,
            why: "nothing is staged\u{1b}[2J".into(),
            cost: 0,
        };
        assert_eq!(
            event(Lang::En, &w, &failed),
            "no commit message came ($0.00): nothing is staged [2J"
        );
        assert_eq!(
            event(Lang::Tr, &w, &failed),
            "commit mesajı gelmedi ($0.00): nothing is staged [2J"
        );
        assert_eq!(ok.kind(), EntryKind::Sensitive);
        assert_eq!(failed.kind(), EntryKind::Failed);
        assert_eq!((ok.agent(), failed.agent()), (0, 0));
    }

    #[test]
    fn a_review_is_said_with_who_on_what_its_cost_and_its_first_line() {
        let w = crate::live::testing::team(1);
        let ok = Event::Reviewed {
            agent: 1,
            by: Vendor::Codex,
            model: "default".into(),
            patch: "p".into(),
            text: "\n  \n1. x: no test\n2. y".into(),
            cost: 420,
        };
        assert_eq!(
            event(Lang::En, &w, &ok),
            "codex (default) reviewed claude-1's change, for $0.04: 1. x: no test"
        );
        assert_eq!(
            event(Lang::Tr, &w, &ok),
            "codex (default) claude-1 değişikliğini inceledi, $0.04 tuttu: 1. x: no test"
        );
        let failed = Event::ReviewFailed {
            agent: 1,
            why: "capped\u{1b}[2J".into(),
            cost: 120,
        };
        assert_eq!(
            event(Lang::En, &w, &failed),
            "no review of claude-1's change came ($0.01): capped [2J"
        );
        assert_eq!(
            event(Lang::Tr, &w, &failed),
            "claude-1 değişikliği için inceleme gelmedi ($0.01): capped [2J"
        );
        assert_eq!(ok.kind(), EntryKind::Sensitive);
        assert_eq!(failed.kind(), EntryKind::Failed);
        assert_eq!((ok.agent(), failed.agent()), (1, 1));
    }

    #[test]
    fn a_size_is_said_in_the_largest_unit_that_keeps_it_whole() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1 KiB");
        assert_eq!(size((1 << 20) - 1), "1023 KiB");
        assert_eq!(size(1 << 20), "1 MiB");
        assert_eq!(size(5 << 20 | 7), "5 MiB");
        assert_eq!(size((1 << 30) - 1), "1023 MiB");
        assert_eq!(size(1 << 30), "1 GiB");
        assert_eq!(size(3 << 30), "3 GiB");
    }

    #[test]
    fn copies_are_said_with_their_paths_and_size_and_a_skip_with_why() {
        let w = crate::live::testing::team(1);
        let copied = Event::Copied {
            agent: 1,
            paths: vec![".env".into(), "target".into()],
            bytes: 3 << 20,
            included: false,
        };
        assert_eq!(
            event(Lang::En, &w, &copied),
            "copied .env, target into claude-1's worktree (3 MiB)"
        );
        assert_eq!(
            event(Lang::Tr, &w, &copied),
            ".env, target → claude-1 çalışma ağacına kopyalandı (3 MiB)"
        );
        let skipped = Event::CopySkipped {
            agent: 1,
            path: "node_modules".into(),
            why: "not in the checkout".into(),
            included: false,
        };
        assert_eq!(
            event(Lang::En, &w, &skipped),
            "node_modules was not copied into claude-1's worktree: not in the checkout"
        );
        assert_eq!(
            event(Lang::Tr, &w, &skipped),
            "node_modules → claude-1 çalışma ağacına kopyalanmadı: not in the checkout"
        );
        assert_eq!(copied.kind(), EntryKind::Assumption);
        assert_eq!(skipped.kind(), EntryKind::Failed);
        assert_eq!((copied.agent(), skipped.agent()), (1, 1));
    }

    #[test]
    fn what_a_worktreeinclude_names_is_said_as_its_own() {
        let w = crate::live::testing::team(1);
        let copied = Event::Copied {
            agent: 1,
            paths: vec!["vendor".into()],
            bytes: 2 << 20,
            included: true,
        };
        assert_eq!(
            event(Lang::En, &w, &copied),
            "copied vendor into claude-1's worktree (2 MiB), as .worktreeinclude names"
        );
        assert_eq!(
            event(Lang::Tr, &w, &copied),
            "vendor → claude-1 çalışma ağacına kopyalandı (2 MiB), .worktreeinclude'un saydığı"
        );
        let skipped = Event::CopySkipped {
            agent: 1,
            path: "cache".into(),
            why: "not in the checkout".into(),
            included: true,
        };
        assert_eq!(
            event(Lang::En, &w, &skipped),
            "cache (from .worktreeinclude) was not copied into claude-1's worktree: not in the checkout"
        );
        assert_eq!(
            event(Lang::Tr, &w, &skipped),
            "cache (.worktreeinclude'dan) → claude-1 çalışma ağacına kopyalanmadı: not in the checkout"
        );
        let file = Event::IncludeRefused {
            agent: 1,
            line: None,
            why: "a link: not followed".into(),
        };
        assert_eq!(
            event(Lang::En, &w, &file),
            ".worktreeinclude was not followed for claude-1's worktree: a link: not followed"
        );
        assert_eq!(
            event(Lang::Tr, &w, &file),
            ".worktreeinclude, claude-1 çalışma ağacı için uygulanmadı: a link: not followed"
        );
        let line = Event::IncludeRefused {
            agent: 1,
            line: Some((3, "../x\u{1b}[2J".into())),
            why: "reaches outside the checkout".into(),
        };
        assert_eq!(
            event(Lang::En, &w, &line),
            "line 3 of .worktreeinclude (../x [2J) was not followed for claude-1's worktree: reaches outside the checkout"
        );
        assert_eq!(
            event(Lang::Tr, &w, &line),
            ".worktreeinclude'un 3. satırı (../x [2J), claude-1 çalışma ağacı için uygulanmadı: reaches outside the checkout"
        );
        assert_eq!(line.kind(), EntryKind::Failed);
        assert_eq!(line.agent(), 1);
    }

    #[test]
    fn a_low_pot_is_said_with_its_rate_and_time_left() {
        let w = crate::live::testing::team(1);
        let low = Event::PotLow {
            leader: 0,
            rate: 150_000,
            minutes: 75,
        };
        assert_eq!(
            event(Lang::En, &w, &low),
            "at $15.00/h the budget lasts about 1 h 15 min more; :budget USD raises it"
        );
        assert_eq!(
            event(Lang::Tr, &w, &low),
            "$15.00/sa hızla bütçe yaklaşık 1 sa 15 dk daha yeter; :budget USD ile artır"
        );
    }

    #[test]
    fn a_span_of_time_is_minutes_then_hours_then_days() {
        let en = |s| ago(Lang::En, s);
        assert_eq!(en(0), "0 min");
        assert_eq!(en(59), "0 min");
        assert_eq!(en(60), "1 min");
        assert_eq!(en(3599), "59 min");
        assert_eq!(en(3600), "1 h 0 min");
        assert_eq!(en(3 * 3600 + 5 * 60 + 59), "3 h 5 min");
        assert_eq!(en(86_399), "23 h 59 min");
        assert_eq!(en(86_400), "1 d 0 h");
        assert_eq!(en(2 * 86_400 + 5 * 3600 + 59 * 60), "2 d 5 h");
        assert_eq!(ago(Lang::Tr, 3 * 3600 + 5 * 60), "3 sa 5 dk");
        assert_eq!(ago(Lang::Tr, 60), "1 dk");
        assert_eq!(ago(Lang::Tr, 86_400), "1 g 0 sa");
    }

    #[test]
    fn a_usage_window_is_worded_with_what_is_left_and_when_it_starts_again() {
        let l = Limit {
            vendor: Vendor::Claude,
            minutes: 300,
            used: 52,
            resets_at: Some(10_000 + 2 * 3600 + 59 * 60),
        };
        assert_eq!(
            limit(Lang::En, &l, 10_000),
            "claude 5h: 48% left, resets in 2 h 59 min"
        );
        assert_eq!(
            limit(Lang::Tr, &l, 10_000),
            "claude 5 sa: %48 kaldı, 2 sa 59 dk sonra yenilenir"
        );
        // A week, already past its reset or without one: no time.
        let week = Limit {
            vendor: Vendor::Codex,
            minutes: 10_080,
            used: 100,
            resets_at: Some(10_000),
        };
        assert_eq!(limit(Lang::En, &week, 10_000), "codex 7d: 0% left");
        assert_eq!(limit(Lang::Tr, &week, 10_000), "codex 7 g: %0 kaldı");
        let day = Limit {
            minutes: 24 * 60,
            resets_at: None,
            ..week
        };
        assert_eq!(limit(Lang::En, &day, 0), "codex 1d: 0% left");
        let hour = Limit {
            minutes: 23 * 60,
            used: 0,
            ..day
        };
        assert_eq!(limit(Lang::En, &hour, 0), "codex 23h: 100% left");
        assert_eq!(
            limit(
                Lang::En,
                &Limit {
                    resets_at: Some(1),
                    ..hour
                },
                0
            ),
            "codex 23h: 100% left, resets in 0 min"
        );
    }

    #[test]
    fn a_command_line_never_listed_is_worded_with_the_agent_and_its_vendor() {
        let mut w = crate::live::testing::team(1);
        w.agents[1].vendor = Vendor::Gemini;
        let e = Event::CommandsMissing { agent: 1 };
        let who = &w.agents[1].name;
        assert_eq!(
            event(Lang::En, &w, &e),
            format!(
                "{who}'s session listed no commands of its own; gemini's format may have changed, so only /clear completes"
            )
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            format!(
                "{who} oturumu kendi komutlarını listelemedi; gemini biçimi değişmiş olabilir, yalnızca /clear tamamlanır"
            )
        );
    }

    #[test]
    fn a_cut_notes_file_is_worded_in_kib_rounded_up() {
        let w = crate::live::testing::team(1);
        let e = Event::NotesCut {
            agent: 1,
            file: "CLAUDE.md".into(),
            size: 50_649,
        };
        let who = &w.agents[1].name;
        assert_eq!(
            event(Lang::En, &w, &e),
            format!(
                "{who}: the repository's CLAUDE.md is 50 KiB; only its first 16 KiB went into the instructions"
            )
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            format!(
                "{who}: deponun CLAUDE.md dosyası 50 KiB; talimatlara yalnızca ilk 16 KiB girdi"
            )
        );
        let exact = Event::NotesCut {
            agent: 1,
            file: "AGENTS.md".into(),
            size: 17 * 1024,
        };
        assert!(event(Lang::En, &w, &exact).contains("AGENTS.md is 17 KiB;"));
        let mine = Event::InstructionsCut {
            agent: 1,
            path: "/home/me/.config/imece/instructions.md".into(),
            size: 17 * 1024 + 1,
        };
        assert_eq!(
            event(Lang::En, &w, &mine),
            format!(
                "{who}: your instructions file /home/me/.config/imece/instructions.md is 18 \
                 KiB; only its first 16 KiB went into the instructions"
            )
        );
        assert_eq!(
            event(Lang::Tr, &w, &mine),
            format!(
                "{who}: talimat dosyan /home/me/.config/imece/instructions.md 18 KiB; \
                 talimatlara yalnızca ilk 16 KiB girdi"
            )
        );
    }

    #[test]
    fn a_message_on_the_bus_is_a_conversation_line_with_its_numbers() {
        let line = Said {
            from: Party::Agent(1),
            to: Party::Agent(0),
            kind: Some(MsgKind::Answer),
            text: "Yes.",
            id: 3,
            re: Some(2),
        };
        let e = Event::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Answer,
            text: "Yes.".into(),
            id: 3,
            re: Some(2),
        };
        assert_eq!(said(&e), Some(line.clone()));
        let s = StoryEvent::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Answer,
            text: "Yes.".into(),
            id: 3,
            re: Some(2),
        };
        assert_eq!(said_in_story(1, &s), Some(line));
    }

    #[test]
    fn a_message_to_the_person_is_a_conversation_line_and_a_question_an_inbox_line() {
        let w = crate::live::testing::team(1);
        let e = Event::ToYou {
            from: 1,
            kind: MsgKind::Ask,
            text: "Which?\nmore".into(),
        };
        assert_eq!(
            said(&e),
            Some(Said {
                from: Party::Agent(1),
                to: Party::Person,
                kind: Some(MsgKind::Ask),
                text: "Which?\nmore",
                id: 0,
                re: None
            })
        );
        assert_eq!(event(Lang::En, &w, &e), "", "drawn as a conversation line");
        let s = StoryEvent::ToYou {
            kind: MsgKind::Inform,
            text: "fyi".into(),
        };
        assert_eq!(
            said_in_story(1, &s),
            Some(Said {
                from: Party::Agent(1),
                to: Party::Person,
                kind: Some(MsgKind::Inform),
                text: "fyi",
                id: 0,
                re: None
            })
        );
        let item = InboxItem {
            kind: InboxKind::Question,
            agent: 1,
            detail: "Which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        let who = &w.agents[1].name;
        assert_eq!(
            inbox(Lang::En, &w, &item),
            format!("{who} asks you: Which?")
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &item),
            format!("{who} sana soruyor: Which?")
        );
    }

    #[test]
    fn a_standing_yes_is_worded_in_both_languages_and_said_once_a_rule() {
        let mut w = crate::live::testing::team(1);
        let npm = Standing {
            program: "npm".into(),
            sub: Some("run".into()),
        };
        let make = Standing {
            program: "make".into(),
            sub: None,
        };
        let added = w.stand(0, &[npm.clone(), make.clone(), npm.clone()]);
        assert_eq!(added, [npm.clone(), make.clone()], "once each");
        assert_eq!(w.projects[0].allowed, [make.clone(), npm.clone()], "sorted");
        let n = w.journal.len();
        assert!(w.stand(0, std::slice::from_ref(&npm)).is_empty());
        assert_eq!(w.journal.len(), n, "nothing new, nothing said");
        let name = w.projects[0].name.clone();
        let said = &w.journal[n - 1].event;
        assert_eq!(
            event(Lang::En, &w, said),
            format!("{name}: you say yes from now on to npm run …, make")
        );
        assert_eq!(
            event(Lang::Tr, &w, said),
            format!("{name}: artık hep evet diyorsun: npm run …, make")
        );
        assert!(w.unstand(0, &npm));
        assert!(!w.unstand(0, &npm), "not there any more");
        let gone = &w.journal.last().unwrap().event;
        assert_eq!(
            event(Lang::En, &w, gone),
            format!("{name}: no standing yes any more to npm run …")
        );
        assert_eq!(
            event(Lang::Tr, &w, gone),
            format!("{name}: artık hep evet yok: npm run …")
        );
        assert_eq!(w.projects[0].allowed, [make]);
    }

    #[test]
    fn a_yes_for_a_turn_is_journaled_in_both_languages() {
        let w = crate::live::testing::team(1);
        let given = Event::TurnYes { agent: 1 };
        let ended = Event::TurnYesEnded {
            agent: 1,
            allowed: 7,
        };
        assert_eq!(
            event(Lang::En, &w, &given),
            "you say yes to everything claude-1 asks until its turn ends"
        );
        assert_eq!(
            event(Lang::En, &w, &ended),
            "claude-1's turn ended, and your yes for it: it let 7 asks go on"
        );
        assert_eq!(
            event(Lang::Tr, &w, &ended),
            "claude-1 turunu bitirdi, onun için evetin de bitti: 7 isteğe sormadan izin verdi"
        );
        assert!(event(Lang::Tr, &w, &given).starts_with("claude-1 turu bitene dek"));
    }

    #[test]
    fn an_ask_is_worded_as_it_is_on_one_row_and_so_is_the_decision() {
        let w = crate::live::testing::team(1);
        let who = &w.agents[1].name;
        let item = InboxItem {
            kind: InboxKind::Approval,
            agent: 1,
            detail: "Bash: rm *.txt **x**\ncat <<EOF\n".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        // Never as markdown: what runs is what the person reads.
        assert_eq!(
            inbox(Lang::En, &w, &item),
            format!("{who} waits for your yes: Bash: rm *.txt **x** ↵ cat <<EOF")
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &item),
            format!("{who} onayını bekliyor: Bash: rm *.txt **x** ↵ cat <<EOF")
        );
        let yes = Event::Decided {
            agent: 1,
            what: "Bash: make\nmake check".into(),
            allowed: true,
        };
        assert_eq!(
            event(Lang::En, &w, &yes),
            format!("you allowed {who}: Bash: make ↵ make check")
        );
        assert_eq!(
            event(Lang::Tr, &w, &yes),
            format!("{who} için izin verdin: Bash: make ↵ make check")
        );
        let no = Event::Decided {
            agent: 1,
            what: "Bash: curl x".into(),
            allowed: false,
        };
        assert_eq!(
            event(Lang::En, &w, &no),
            format!("you refused {who}: Bash: curl x")
        );
        assert_eq!(
            event(Lang::Tr, &w, &no),
            format!("{who} için reddettin: Bash: curl x")
        );
    }

    #[test]
    fn a_retry_and_a_vendor_error_are_worded_in_the_journal_and_in_another_window() {
        let mut w = crate::live::testing::team(1);
        let e = Event::Retrying {
            agent: 1,
            why: "API Error:\noverloaded".into(),
            attempt: 2,
            seconds: 120,
        };
        assert_eq!(
            event(Lang::En, &w, &e),
            "claude-1 hit a vendor error: API Error: ↵ overloaded · imece tries again in 2 min 0 s (2/3)"
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            "claude-1 satıcı hatası aldı: API Error: ↵ overloaded · imece 2 dk 0 sn sonra yeniden deniyor (2/3)"
        );
        w.turn_failed(1, "Invalid API key", Some(Fault::Account));
        let i = w.open_inbox()[0];
        assert_eq!(
            notice(Lang::En, &w, crate::notify::Note::Item(i)),
            "imece · claude-1 hit a vendor error that needs you"
        );
        assert_eq!(
            notice(Lang::Tr, &w, crate::notify::Note::Item(i)),
            "imece · claude-1 senin çözmen gereken bir satıcı hatası aldı"
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &w.inbox[i]),
            "claude-1 satıcı hatası, giderince ona yaz: Invalid API key"
        );
    }

    #[test]
    fn a_question_in_the_inbox_is_worded_without_its_markdown() {
        let w = crate::live::testing::team(1);
        let item = InboxItem {
            kind: InboxKind::Question,
            agent: 1,
            detail: "**Which** `one`?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        assert_eq!(
            inbox(Lang::En, &w, &item),
            format!("{} asks you: Which one?", w.agents[1].name)
        );
    }

    #[test]
    fn a_worktree_no_agent_owns_says_what_it_holds_and_a_removal_how_it_ended() {
        let o = |changes, commits| Orphan {
            name: "claude-7".into(),
            path: "/s/claude-7".into(),
            checkout: "/r".into(),
            changes,
            commits,
        };
        assert_eq!(held(Lang::En, &o(0, 0)), "clean");
        assert_eq!(held(Lang::En, &o(3, 0)), "files changed: 3");
        assert_eq!(held(Lang::En, &o(0, 2)), "commits of its own: 2");
        assert_eq!(
            held(Lang::En, &o(3, 2)),
            "files changed: 3 · commits of its own: 2"
        );
        assert_eq!(
            held(Lang::Tr, &o(1, 1)),
            "değişen dosya: 1 · kendi commit'i: 1"
        );
        assert_eq!(held(Lang::Tr, &o(0, 0)), "temiz");
        let done = Tidied {
            removed: vec!["claude-3".into(), "codex-4".into()],
            kept: Vec::new(),
            torn: Vec::new(),
        };
        assert_eq!(tidied(Lang::En, &done), "removed claude-3, codex-4");
        let done = Tidied {
            removed: Vec::new(),
            kept: vec![
                ("claude-1".into(), None),
                ("x\u{1b}[2J".into(), Some("holds work".into())),
            ],
            torn: Vec::new(),
        };
        // A name from the disk is cleaned.
        assert_eq!(
            tidied(Lang::En, &done),
            "not removed: claude-1 (an agent works in it now), x [2J (holds work)"
        );
        let done = Tidied {
            removed: vec!["a".into()],
            kept: vec![("b".into(), None)],
            torn: Vec::new(),
        };
        assert_eq!(
            tidied(Lang::Tr, &done),
            "silindi: a · silinmedi: b (şimdi bir ajan içinde çalışıyor)"
        );
        let ran = |ok, last: Option<&str>| TornDown::Ran {
            ok,
            end: "exit 0, 0.4 s".into(),
            last: last.map(str::to_string),
        };
        let done = Tidied {
            removed: vec!["a".into()],
            kept: Vec::new(),
            torn: vec![
                ("a".into(), ran(true, Some("dropped"))),
                ("b".into(), ran(true, None)),
                ("c".into(), ran(false, Some("no db"))),
                ("d".into(), TornDown::Failed("not a plain file".into())),
                ("e".into(), TornDown::Refused),
            ],
        };
        assert_eq!(
            tidied(Lang::En, &done),
            "removed a · teardown in a: exit 0, 0.4 s · dropped · teardown in b: exit 0, 0.4 s · \
             teardown failed in c: exit 0, 0.4 s · no db · teardown did not run in d: not a plain \
             file · teardown not run in e: you said no"
        );
        assert_eq!(
            tidied(Lang::Tr, &done),
            "silindi: a · a söküldü: exit 0, 0.4 s · dropped · b söküldü: exit 0, 0.4 s · \
             c sökümü başarısız: exit 0, 0.4 s · no db · d sökümü çalışmadı: not a plain file · \
             e sökümü çalışmadı: hayır dedin"
        );
    }
}
