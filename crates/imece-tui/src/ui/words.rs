//! Events, card stories and inbox items worded in the person's language.
//! Every argument goes through `fill`, which cleans agent-written text.

use crate::app::Rung;
use crate::editor::{self, Why};
use crate::health::Sign;
use crate::i18n::{Lang, Text, fill};
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

/// What leader `leader`'s plan holds: its cards and what it expects them
/// to cost; nothing when no plan is there.
pub(crate) fn plan_figures(w: &World, leader: AgentId) -> (usize, u64) {
    let p = w.agents[leader].project;
    (w.projects[p].plan.proposal.as_ref()).map_or((0, 0), |p| (p.cards.len(), p.cost))
}

pub(crate) fn card_ref(c: CardId) -> String {
    format!("#{}", c + 1)
}

/// Cards as the board names them, in a row: `#2, #5`.
pub(crate) fn card_refs(cs: &[CardId]) -> String {
    let refs: Vec<String> = cs.iter().map(|&c| card_ref(c)).collect();
    refs.join(", ")
}

/// imece's health notice on agent `id` (record 128), and who heard it:
/// its leader, woken or at its next turn; nobody else when it leads.
fn health(t: &Text, w: &World, id: AgentId, sign: &Sign) -> String {
    let who = name(w, id);
    let title = |c: CardId| w.cards[c].title.as_str();
    let said = match sign {
        Sign::Stalled { card, minutes } => fill(
            t.ev_health_stalled,
            &[who, &minutes.to_string(), &card_ref(*card), title(*card)],
        ),
        Sign::Waiting { card, to, minutes } => fill(
            t.ev_health_waiting,
            &[
                who,
                &minutes.to_string(),
                name(w, *to),
                &card_ref(*card),
                title(*card),
            ],
        ),
        Sign::Silent { minutes, doing, .. } if doing.is_empty() => {
            fill(t.ev_health_silent, &[who, &minutes.to_string()])
        }
        Sign::Silent { minutes, doing, .. } => fill(
            t.ev_health_silent_doing,
            &[who, &minutes.to_string(), doing],
        ),
        Sign::Free { minutes } => fill(t.ev_health_free, &[who, &minutes.to_string()]),
        Sign::Looping { call, times } => {
            fill(t.ev_health_looping, &[who, &times.to_string(), call])
        }
    };
    match (w.told_of(id), sign.wakes()) {
        (None, _) => said,
        (Some(l), true) => fill(t.ev_health_told, &[&said, name(w, l)]),
        (Some(l), false) => fill(t.ev_health_hears, &[&said, name(w, l)]),
    }
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
        Event::Handed {
            agent,
            card,
            from,
            attempt,
        } => {
            let (card, title, n) = (card_ref(*card), &w.cards[*card].title, attempt.to_string());
            match from {
                Some(f) => fill(
                    t.ev_handed_from,
                    &[name(w, *agent), &card, title, name(w, *f), &n],
                ),
                None => fill(t.ev_handed, &[name(w, *agent), &card, title, &n]),
            }
        }
        Event::Held { agent, card, on } => fill(
            t.ev_held,
            &[
                name(w, *agent),
                &card_ref(*card),
                &w.cards[*card].title,
                &card_refs(on),
            ],
        ),
        Event::Released { agent, card } => fill(
            t.ev_released,
            &[name(w, *agent), &card_ref(*card), &w.cards[*card].title],
        ),
        Event::CardFailed { agent, card, why } => fill(
            t.ev_card_failed,
            &[
                name(w, *agent),
                &card_ref(*card),
                &w.cards[*card].title,
                why,
            ],
        ),
        Event::LateDone { agent, card } => {
            // A card withdrawn went to no other try (record 126).
            let text = match w.cards[*card].column {
                Column::Withdrawn => t.ev_late_withdrawn,
                _ => t.ev_late_done,
            };
            fill(text, &[name(w, *agent), &card_ref(*card)])
        }
        Event::ChecksSet {
            agent,
            card,
            checks,
            you,
        } => {
            let list = checks_list(t, checks);
            match you {
                true => fill(t.ev_checks_you, &[&card_ref(*card), &list]),
                false => fill(t.ev_checks_set, &[name(w, *agent), &card_ref(*card), &list]),
            }
        }
        Event::Killed { agent, pid, name } => fill(
            t.ev_killed,
            &[name, &pid.to_string(), &w.agents[*agent].name],
        ),
        Event::Withdrawn { agent, card, why } => fill(
            t.ev_withdrawn,
            &[
                name(w, *agent),
                &card_ref(*card),
                &w.cards[*card].title,
                why,
            ],
        ),
        Event::Health { agent, sign } => health(t, w, *agent, sign),
        Event::Capped {
            card, cap, count, ..
        } => {
            let text = match cap {
                Cap::Tries => t.ev_capped_tries,
                Cap::Rounds => t.ev_capped_rounds,
            };
            let title = &w.cards[*card].title;
            fill(text, &[&card_ref(*card), title, &count.to_string()])
        }
        Event::CapAnswered { card, cap, yes, .. } => {
            let text = match (cap, yes) {
                (Cap::Tries, true) => t.ev_cap_more_try,
                (Cap::Rounds, true) => t.ev_cap_more_round,
                (_, false) => t.ev_cap_failed,
            };
            fill(text, &[&card_ref(*card)])
        }
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
        Event::PotLow { rate, minutes, .. } => fill(
            t.ev_pot_low,
            &[&w.money(*rate, false), &ago(lang, minutes * 60)],
        ),
        Event::Shell {
            command,
            ended,
            took,
            ..
        } => shell(lang, command, *ended, *took),
        Event::Opened { leader, path } => fill(t.ev_opened, &[project(w, *leader), path]),
        Event::NoGit { leader, path } => fill(t.ev_no_git, &[name(w, *leader), path]),
        Event::StoreDown { leader, repo, why } => fill(
            t.ev_store_down,
            &[project(w, *leader), repo, store_down(t, *why)],
        ),
        Event::StoreBack { leader, repo } => fill(t.ev_store_back, &[project(w, *leader), repo]),
        Event::StoreSignedIn { human, .. } if human.is_empty() => t.ev_store_signed_in.to_string(),
        Event::StoreSignedIn { human, .. } => fill(t.ev_store_signed_in_as, &[human]),
        Event::StoreImported {
            leader,
            repo,
            warnings,
            hardened,
        } => {
            let mut line = fill(t.ev_store_imported, &[project(w, *leader), repo]);
            if *hardened {
                line = fill(t.ev_store_imported_hardened, &[&line]);
            }
            match warnings.first() {
                None => line,
                Some(first) => fill(
                    t.ev_store_imported_warned,
                    &[&line, first, &(warnings.len() - 1).to_string()],
                ),
            }
        }
        Event::StoreCriticalKept {
            leader,
            repo,
            why: None,
        } => fill(t.ev_store_critical_kept, &[project(w, *leader), repo]),
        Event::StoreCriticalKept {
            leader,
            repo,
            why: Some(why),
        } => {
            let line = fill(t.ev_store_critical_kept, &[project(w, *leader), repo]);
            fill(t.ev_store_failed, &[&line, why])
        }
        Event::StoreSynced { leader, repo, how } => fill(
            match how {
                Synced::Even => t.ev_store_synced_even,
                Synced::Moved => t.ev_store_synced,
                Synced::Apart => t.ev_store_synced_apart,
            },
            &[project(w, *leader), repo],
        ),
        Event::StoreFailed { act, why, .. } => {
            let did = match act {
                StoreAct::Login => t.store_act_login,
                StoreAct::Import => t.store_act_import,
                StoreAct::Sync => t.store_act_sync,
                StoreAct::Decide => t.store_act_decide,
            };
            let why = why.as_deref().map(|w| denied_why(t, w));
            fill(
                t.ev_store_failed,
                &[did, why.as_deref().unwrap_or(t.store_no_answer)],
            )
        }
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
        Event::CommandsMissing { agent } => fill(t.ev_commands_missing, &[name(w, *agent), CLAUDE]),
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
        Event::Checked {
            agent,
            ok,
            end,
            lines,
        } => {
            if *ok {
                fill(t.ev_check_ok, &[name(w, *agent), end])
            } else {
                let last = lines.last().map_or(t.setup_no_output, String::as_str);
                fill(t.ev_check_bad, &[name(w, *agent), end, last])
            }
        }
        Event::CheckFailed { agent, why } => fill(t.ev_check_failed, &[name(w, *agent), why]),
        Event::Strayed { agent, card, path } => {
            fill(t.ev_strayed, &[name(w, *agent), path, &card_ref(*card)])
        }
        Event::Planned {
            leader,
            cards,
            cost,
        } => fill(
            t.ev_planned,
            &[name(w, *leader), &cards.to_string(), &dollars(*cost)],
        ),
        Event::PlanAnswered { leader, yes } => {
            let text = if *yes { t.ev_plan_yes } else { t.ev_plan_no };
            fill(text, &[name(w, *leader)])
        }
        Event::CheckAnswered { agent, yes } => {
            let text = if *yes { t.ev_check_yes } else { t.ev_check_no };
            fill(text, &[name(w, *agent)])
        }
        Event::Pushed { words, sent, .. } => {
            fill(if *sent { t.ev_pushed } else { t.ev_pushed_demo }, &[words])
        }
        Event::PushFailed { why, .. } => fill(t.ev_push_failed, &[why]),
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
            spend: None,
        } => fill(
            t.ev_change_ready,
            &[
                name(w, *agent),
                &files.to_string(),
                &added.to_string(),
                &removed.to_string(),
            ],
        ),
        Event::ChangeReady {
            agent,
            files,
            added,
            removed,
            spend: Some(s),
        } => fill(
            t.ev_change_ready_spent,
            &[
                name(w, *agent),
                &files.to_string(),
                &added.to_string(),
                &removed.to_string(),
                &spend(lang, w, s),
            ],
        ),
        Event::ChangeCost { agent, spend: s } => {
            fill(t.ev_change_cost, &[name(w, *agent), &spend(lang, w, s)])
        }
        Event::Landed { agent, how } => landed(t, name(w, *agent), how),
        Event::Verified { agent, checks, .. } => {
            fill(t.ev_verified, &[name(w, *agent), &attested(checks)])
        }
        Event::Applied { agent, files, mark } => {
            fill(t.ev_applied, &[name(w, *agent), &files.to_string(), mark])
        }
        Event::ApplyFailed { agent, why } => {
            let said = match w.clashes_of(*agent) {
                Some(_) => t.ev_apply_failed_back,
                None => t.ev_apply_failed,
            };
            fill(said, &[name(w, *agent), why])
        }
        Event::Suggested { model, cost, .. } => {
            fill(t.ev_suggested, &[model, &w.money(*cost, false)])
        }
        Event::SuggestFailed { why, cost, .. } => {
            fill(t.ev_suggest_failed, &[&w.money(*cost, false), why])
        }
        Event::Reviewed {
            agent,
            model,
            text,
            cost,
            ..
        } => fill(
            t.ev_reviewed,
            &[
                CLAUDE,
                model,
                name(w, *agent),
                &w.money(*cost, false),
                text.lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or_default(),
            ],
        ),
        Event::ReviewFailed { agent, why, cost } => fill(
            t.ev_review_failed,
            &[name(w, *agent), &w.money(*cost, false), why],
        ),
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
        StoryEvent::Strayed(file) => fill(t.st_strayed, &[file]),
        StoryEvent::Handed { from, to, attempt } => match from {
            Some(f) => fill(
                t.st_handed_from,
                &[name(w, *f), name(w, *to), &attempt.to_string()],
            ),
            None => fill(t.st_handed, &[name(w, *to), &attempt.to_string()]),
        },
        StoryEvent::Held { to, on } => fill(t.st_held, &[name(w, *to), &card_refs(on)]),
        StoryEvent::Failed(why) => fill(t.st_failed, &[why]),
        StoryEvent::Withdrawn(why) => fill(t.st_withdrawn, &[why]),
        StoryEvent::ChecksSet { by, checks } => {
            let list = checks_list(t, checks);
            match by {
                Some(by) => fill(t.st_checks_set, &[name(w, *by), &list]),
                None => fill(t.st_checks_you, &[&list]),
            }
        }
        StoryEvent::LateDone { from } => fill(t.st_late_done, &[name(w, *from)]),
        StoryEvent::Capped { cap, count } => {
            let text = match cap {
                Cap::Tries => t.st_capped_tries,
                Cap::Rounds => t.st_capped_rounds,
            };
            fill(text, &[&count.to_string()])
        }
        StoryEvent::CapAnswered { cap, yes } => match (cap, yes) {
            (Cap::Tries, true) => t.st_cap_more_try,
            (Cap::Rounds, true) => t.st_cap_more_round,
            (_, false) => t.st_cap_failed,
        }
        .to_string(),
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
        Event::ToYou {
            from, kind, text, ..
        } => s(Agent(*from), Person, Some(*kind), text),
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
        Progress::Stuck => "■ ",
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
pub fn farewell(lang: Lang, w: &World, (spent, unpriced): (u64, bool), demo: bool) -> String {
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
    if spent > 0 || unpriced {
        parts.push(fill(t.bye_spent, &[&w.money(spent, unpriced)]));
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
    if o.unread {
        parts.push(t.tidy_unread.to_string());
    }
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

/// The plan's usage window `l` as the header says it, `now` being the
/// world's seconds: `claude`, the window's length, what is left of
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
    match l.resets_at.filter(|&r| r > now) {
        Some(r) => fill(
            t.limit_resets,
            &[CLAUDE, &window, &left, &ago(lang, r - now)],
        ),
        None => fill(t.limit, &[CLAUDE, &window, &left]),
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
                InboxKind::Check => t.notice_check,
                InboxKind::Plan => t.notice_plan,
                InboxKind::Tries | InboxKind::Rounds => t.notice_cap,
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

/// What cap item `item` counts (record 126, F9): its card's tries, or
/// the times its change came back.
pub(crate) fn cap_count(w: &World, item: &InboxItem) -> u32 {
    let Some(card) = item.card.and_then(|c| w.cards.get(c)) else {
        return 0;
    };
    match item.kind {
        InboxKind::Tries => card.attempt,
        _ => card.breaker.sent_back,
    }
}

pub fn inbox(lang: Lang, w: &World, item: &InboxItem) -> String {
    let t = lang.text();
    match (item.kind, item.researcher) {
        (InboxKind::Tries | InboxKind::Rounds, _) => {
            let text = match item.kind {
                InboxKind::Tries => t.in_tries,
                _ => t.in_rounds,
            };
            let card = item.card.map_or(String::new(), card_ref);
            let count = cap_count(w, item).to_string();
            fill(text, &[name(w, item.agent), &card, &count])
        }
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
        (InboxKind::Check, _) => fill(t.in_check, &[name(w, item.agent), &item.detail]),
        (InboxKind::Plan, _) => {
            let (cards, cost) = plan_figures(w, item.agent);
            fill(
                t.in_plan,
                &[name(w, item.agent), &cards.to_string(), &dollars(cost)],
            )
        }
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

/// Where the repository's check stands on a change, for its row in
/// TO DECIDE: the mark and whether it is good, bad or neither;
/// nothing when there is no check or it could not run (the diff says).
pub fn check_mark(lang: Lang, check: &Check) -> Option<(&'static str, Option<bool>)> {
    let t = lang.text();
    match check {
        Check::Ran(d) if d.ok => Some((t.check_row_ok, Some(true))),
        Check::Ran(_) => Some((t.check_row_bad, Some(false))),
        Check::Running => Some((t.check_row_running, None)),
        Check::Asked => Some((t.check_row_asked, None)),
        Check::None | Check::Failed(_) => None,
    }
}

/// What a change cost (record 113): the dollars, the time worked (whole
/// seconds under a minute, else minutes, hours and minutes), and the
/// rounds when there was more than one: `$0.84 · 12 min`, `$1.20 · 25 min
/// · 3 rounds`; the dollars as [`World::money`] says them (record 132).
pub fn spend(lang: Lang, w: &World, s: &Spend) -> String {
    let t = lang.text();
    let time = super::ledger::lasted(lang, s.ticks);
    let mut out = format!("{} · {time}", w.money(s.cost, s.unpriced));
    if s.rounds > 1 {
        out.push_str(&fill(t.spend_rounds, &[&s.rounds.to_string()]));
    }
    out
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

/// How many digits of a commit a line shows.
const SHORT_COMMIT: usize = 7;

/// What became of `name`'s proposal, as the journal says it.
fn landed(t: &Text, name: &str, how: &Landed) -> String {
    match how {
        Landed::Queued => fill(t.ev_landed_queued, &[name]),
        Landed::Integrated(commit) => {
            let short: String = commit.chars().take(SHORT_COMMIT).collect();
            fill(t.ev_landed_integrated, &[name, &short])
        }
        Landed::Conflicted(symbols) => fill(t.ev_landed_conflicted, &[name, &symbols.join(", ")]),
        Landed::Rejected(reasons) => {
            let said: Vec<String> = reasons.iter().map(|r| refused_why(t, r)).collect();
            fill(t.ev_landed_rejected, &[name, &said.join("; ")])
        }
        Landed::Withdrawn => fill(t.ev_landed_withdrawn, &[name]),
    }
}

/// Lonca's word for why its queue refused a change, as a person says it;
/// a word imece does not know (a person's own reason among them), as Lonca
/// said it. Uncleaned: it goes through `fill` as every argument does.
fn refused_why(t: &Text, why: &str) -> String {
    let (word, rest) = why.split_once(':').unwrap_or((why, ""));
    let [
        failed,
        error,
        session,
        day,
        merge,
        clash,
        stale,
        frozen,
        missing,
        unknown,
        invalid,
        not_a_change,
    ] = t.landed_whys;
    match (word, rest) {
        ("verification_failed", check) => fill(failed, &[check]),
        ("verification_error", check) => fill(error, &[check]),
        ("verification_missing", check) => fill(missing, &[check]),
        ("verification_quota", "session") => session.to_string(),
        ("verification_quota", "day") => day.to_string(),
        ("merge_conflict", "") => merge.to_string(),
        ("conflict", symbol) => fill(clash, &[symbol]),
        ("stale_trunk" | "unknown_base", "") => stale.to_string(),
        ("frozen", why) => fill(frozen, &[why]),
        // What an approval asking for a card's checks may be refused for
        // (record 127).
        ("unknown_check", check) => fill(unknown, &[check]),
        ("invalid_checks", "") => invalid.to_string(),
        ("checks_not_a_change", "") => not_a_change.to_string(),
        _ => why.to_string(),
    }
}

/// What the `lonca` program said when the daemon refused (`denied:
/// unknown_check:api, invalid_checks`): each reason as a person says it;
/// anything else as it came. A refusal of the card's checks ends with
/// how to change them (`:checks`), so a yes refused for a name the policy
/// lacks need not become a reject.
fn denied_why(t: &Text, said: &str) -> String {
    match said.strip_prefix("denied: ") {
        Some(reasons) => {
            let mut said = (reasons.split(", "))
                .map(|r| refused_why(t, r))
                .collect::<Vec<_>>()
                .join(", ");
            if reasons.split(", ").any(of_the_checks) {
                said += t.denied_checks;
            }
            said
        }
        None => said.to_string(),
    }
}

/// Whether Lonca refused the yes for the checks the card asks for, which
/// `:checks` changes (record 127): not for one given with no change.
fn of_the_checks(reason: &str) -> bool {
    reason == "invalid_checks" || reason.starts_with("unknown_check:")
}

/// A card's checks as a list is read, `none` when it asks for none.
pub fn checks_list(t: &Text, checks: &[String]) -> String {
    match checks.is_empty() {
        true => t.checks_none.to_string(),
        false => crate::checks::listed(checks),
    }
}

/// What Lonca's queue checked, on one line: `✓ build exit 0, 1.1 s · ✗ tests
/// exit 1, 2.0 s · its last line`.
fn attested(checks: &[Attested]) -> String {
    let one = |c: &Attested| {
        let mark = if c.ok { "✓" } else { "✗" };
        let head = format!("{mark} {} {}", c.check, c.end);
        match c.lines.last() {
            Some(last) => format!("{} · {last}", head.trim_end()),
            None => head.trim_end().to_string(),
        }
    };
    checks.iter().map(one).collect::<Vec<_>>().join(" · ")
}

/// Why a Lonca repository can not be read, in the screen's language.
pub fn store_down(t: &Text, why: StoreDown) -> &'static str {
    match why {
        StoreDown::NoDaemon => t.store_no_daemon,
        StoreDown::NotTheDaemon => t.store_not_the_daemon,
        StoreDown::Protocol => t.store_protocol,
        StoreDown::SignedOut => t.store_signed_out,
        StoreDown::Forbidden => t.store_forbidden,
        StoreDown::NoRepo => t.store_no_repo,
        StoreDown::Refused => t.store_refused,
        StoreDown::Unguarded => t.store_unguarded,
        StoreDown::Busy => t.store_busy,
        StoreDown::Frozen => t.store_frozen,
    }
}

/// How far behind trunk a checkout is, as a line shows it; nothing when it
/// is level or not known.
pub fn behind(behind: Behind) -> Option<String> {
    match behind {
        Behind::By(0) | Behind::Unknown => None,
        Behind::By(n) => Some(format!("↓{n}")),
        Behind::Never => Some("↓?".to_string()),
    }
}

/// The first digits of a commit, as a person reads it.
pub fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
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
            "imece · claude-1 asks to run a worktree setup script"
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
        assert_eq!(title(Lang::En, 1, Progress::Calm), "⁙ imece · 1 to decide");
        assert_eq!(
            title(Lang::En, 12, Progress::Calm),
            "⁙ imece · 12 to decide"
        );
        assert_eq!(
            title(Lang::Tr, 3, Progress::Calm),
            "⁙ imece · 3 karar bekliyor"
        );
    }

    #[test]
    fn the_farewell_says_what_waits_who_sleeps_what_the_run_spent_and_how_to_go_on() {
        let mut w = crate::live::testing::team(2);
        assert_eq!(
            farewell(Lang::En, &w, (0, false), false),
            "⁙ imece · 3 agents asleep · resume: imece"
        );
        assert_eq!(
            farewell(Lang::Tr, &w, (24_000, false), false),
            "⁙ imece · 3 agent uykuda · bu oturumda $2.40 · devam: imece"
        );
        w.agents[1].gone = true;
        w.agents[2].gone = true;
        assert_eq!(
            farewell(Lang::En, &w, (1, false), false),
            "⁙ imece · 1 agent asleep · <$0.01 spent this run · resume: imece"
        );
        // Never $0 for what is not known (record 132).
        assert_eq!(
            farewell(Lang::En, &w, (0, true), false),
            "⁙ imece · 1 agent asleep · ? spent this run · resume: imece"
        );
        w.estimated = true;
        assert_eq!(
            farewell(Lang::En, &w, (24_000, true), false),
            "⁙ imece · 1 agent asleep · ≈$2.40+? spent this run · resume: imece"
        );
        w.estimated = false;
        w.agents[0].gone = true;
        assert_eq!(
            farewell(Lang::En, &w, (0, false), false),
            "⁙ imece · resume: imece"
        );
        w.agents[0].gone = false;
        w.projects[0].closed = true;
        assert_eq!(
            farewell(Lang::En, &w, (0, false), false),
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
            farewell(Lang::Tr, &w, (0, false), false),
            "⁙ imece · 2 karar bekliyor · 1 agent uykuda · devam: imece"
        );
        assert_eq!(
            farewell(Lang::En, &w, (8_500, false), true),
            "⁙ imece · 2 to decide · $0.85 spent this run · the demo keeps nothing"
        );
        assert_eq!(
            farewell(Lang::Tr, &w, (0, false), true),
            "⁙ imece · 2 karar bekliyor · demo hiçbir şey saklamaz"
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
            model: "default".into(),
            patch: "p".into(),
            text: "\n  \n1. x: no test\n2. y".into(),
            cost: 420,
        };
        assert_eq!(
            event(Lang::En, &w, &ok),
            "claude (default) reviewed claude-1's change, for $0.04: 1. x: no test"
        );
        assert_eq!(
            event(Lang::Tr, &w, &ok),
            "claude (default) claude-1 değişikliğini inceledi, $0.04 tuttu: 1. x: no test"
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
            minutes: 10_080,
            used: 100,
            resets_at: Some(10_000),
        };
        assert_eq!(limit(Lang::En, &week, 10_000), "claude 7d: 0% left");
        assert_eq!(limit(Lang::Tr, &week, 10_000), "claude 7 g: %0 kaldı");
        let day = Limit {
            minutes: 24 * 60,
            resets_at: None,
            ..week
        };
        assert_eq!(limit(Lang::En, &day, 0), "claude 1d: 0% left");
        let hour = Limit {
            minutes: 23 * 60,
            used: 0,
            ..day
        };
        assert_eq!(limit(Lang::En, &hour, 0), "claude 23h: 100% left");
        assert_eq!(
            limit(
                Lang::En,
                &Limit {
                    resets_at: Some(1),
                    ..hour
                },
                0
            ),
            "claude 23h: 100% left, resets in 0 min"
        );
    }

    #[test]
    fn a_command_line_never_listed_is_worded_with_the_agent() {
        let w = crate::live::testing::team(1);
        let e = Event::CommandsMissing { agent: 1 };
        let who = &w.agents[1].name;
        assert_eq!(
            event(Lang::En, &w, &e),
            format!(
                "{who}'s session listed no commands of its own; claude's format may have changed, so only /clear completes"
            )
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            format!(
                "{who} oturumu kendi komutlarını listelemedi; claude biçimi değişmiş olabilir, yalnızca /clear tamamlanır"
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
            options: Vec::new(),
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
            whole: false,
        };
        let make = Standing {
            program: "make".into(),
            sub: None,
            whole: false,
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
            format!("{who} asks to run Bash: rm *.txt **x** ↵ cat <<EOF")
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &item),
            format!("{who} çalıştırmak istiyor: Bash: rm *.txt **x** ↵ cat <<EOF")
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
            "claude-1 hit a API error: API Error: ↵ overloaded · imece tries again in 2 min 0 s (2/3)"
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            "claude-1 API hatası aldı: API Error: ↵ overloaded · imece 2 dk 0 sn sonra yeniden deniyor (2/3)"
        );
        w.turn_failed(1, "Invalid API key", Some(Fault::Account));
        let i = w.open_inbox()[0];
        assert_eq!(
            notice(Lang::En, &w, crate::notify::Note::Item(i)),
            "imece · claude-1 hit a API error that needs you"
        );
        assert_eq!(
            notice(Lang::Tr, &w, crate::notify::Note::Item(i)),
            "imece · claude-1 senin çözmen gereken bir API hatası aldı"
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &w.inbox[i]),
            "claude-1 API hatası, giderince ona yaz: Invalid API key"
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
            unread: false,
        };
        let unread = Orphan {
            unread: true,
            ..o(0, 0)
        };
        assert!(!unread.clean());
        assert_eq!(
            held(Lang::En, &unread),
            "a workspace that may hold work: its session is over"
        );
        assert_eq!(
            held(Lang::Tr, &unread),
            "iş tutuyor olabilecek workspace: oturumu bitmiş"
        );
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
            removed: vec!["claude-3".into(), "claude-4".into()],
            kept: Vec::new(),
            torn: Vec::new(),
        };
        assert_eq!(tidied(Lang::En, &done), "removed claude-3, claude-4");
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

    #[test]
    fn a_card_at_its_breaker_and_the_persons_answer_are_said_in_both_languages() {
        let mut w = crate::live::testing::team(2);
        let c = crate::live::testing::with_card(&mut w, 1);
        w.cards[c].attempt = 3;
        w.cards[c].breaker.sent_back = 4;
        let say = |lang, e: Event| event(lang, &w, &e);
        let capped = |cap| Event::Capped {
            agent: 0,
            card: c,
            cap,
            count: 3,
        };
        assert_eq!(
            say(Lang::En, capped(Cap::Tries)),
            "#1 fix it failed 3 tries: another waits for your yes"
        );
        assert_eq!(
            say(Lang::En, capped(Cap::Rounds)),
            "#1 fix it came back 3 times: another round waits for your yes"
        );
        assert_eq!(
            say(Lang::Tr, capped(Cap::Tries)),
            "#1 fix it 3 denemede başarısız: bir deneme daha onayınızı bekliyor"
        );
        assert_eq!(
            say(Lang::Tr, capped(Cap::Rounds)),
            "#1 fix it 3 kez geri döndü: bir tur daha onayınızı bekliyor"
        );
        let answered = |cap, yes| Event::CapAnswered {
            agent: 0,
            card: c,
            cap,
            yes,
        };
        for (cap, yes, en, tr) in [
            (
                Cap::Tries,
                true,
                "you allowed #1 one more try",
                "#1 için bir deneme daha verdiniz",
            ),
            (
                Cap::Rounds,
                true,
                "you allowed #1 one more round",
                "#1 için bir tur daha verdiniz",
            ),
            (
                Cap::Tries,
                false,
                "you failed #1: nobody hands it again",
                "#1 kartını başarısız saydınız: kimse onu yeniden vermez",
            ),
            (
                Cap::Rounds,
                false,
                "you failed #1: nobody hands it again",
                "#1 kartını başarısız saydınız: kimse onu yeniden vermez",
            ),
        ] {
            assert_eq!(say(Lang::En, answered(cap, yes)), en);
            assert_eq!(say(Lang::Tr, answered(cap, yes)), tr);
        }
        let card = w.cards[c].clone();
        let tell = |lang, e: StoryEvent| story(lang, &w, &card, &e);
        assert_eq!(
            tell(
                Lang::En,
                StoryEvent::Capped {
                    cap: Cap::Tries,
                    count: 3
                }
            ),
            "failed 3 tries: the person decides"
        );
        assert_eq!(
            tell(
                Lang::Tr,
                StoryEvent::Capped {
                    cap: Cap::Rounds,
                    count: 4
                }
            ),
            "4 kez geri döndü: karar kişinin"
        );
        assert_eq!(
            tell(
                Lang::En,
                StoryEvent::Capped {
                    cap: Cap::Rounds,
                    count: 4
                }
            ),
            "came back 4 times: the person decides"
        );
        for (cap, yes, en) in [
            (Cap::Tries, true, "the person allowed one more try"),
            (Cap::Rounds, true, "the person allowed one more round"),
            (Cap::Tries, false, "the person failed it"),
            (Cap::Rounds, false, "the person failed it"),
        ] {
            assert_eq!(tell(Lang::En, StoryEvent::CapAnswered { cap, yes }), en);
        }
        assert_eq!(
            tell(
                Lang::Tr,
                StoryEvent::CapAnswered {
                    cap: Cap::Rounds,
                    yes: true
                }
            ),
            "kişi bir tur daha verdi"
        );
        // The items: who waits, the card, and what it counts.
        let item = |kind, agent| InboxItem {
            kind,
            agent,
            detail: "fix it".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: Some(c),
        };
        assert_eq!(
            inbox(Lang::En, &w, &item(InboxKind::Tries, 0)),
            "lead: #1 failed 3 tries; another needs your yes"
        );
        assert_eq!(
            inbox(Lang::En, &w, &item(InboxKind::Rounds, 1)),
            "claude-1: #1's change came back 4 times; another round needs your yes"
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &item(InboxKind::Rounds, 1)),
            "claude-1: #1 değişikliği 4 kez geri döndü; bir tur daha onayınızı bekliyor"
        );
        let cardless = InboxItem {
            card: None,
            ..item(InboxKind::Tries, 0)
        };
        assert_eq!(cap_count(&w, &cardless), 0);
        let gone = InboxItem {
            card: Some(9),
            ..item(InboxKind::Rounds, 0)
        };
        assert_eq!(cap_count(&w, &gone), 0);
        w.inbox.push(item(InboxKind::Rounds, 1));
        assert_eq!(
            notice(Lang::En, &w, crate::notify::Note::Item(0)),
            "imece · claude-1: a card waits for your word"
        );
        w.inbox[0].kind = InboxKind::Tries;
        assert_eq!(
            notice(Lang::Tr, &w, crate::notify::Note::Item(0)),
            "imece · claude-1: bir kart kararınızı bekliyor"
        );
    }

    #[test]
    fn health_is_said_in_both_languages_with_who_heard_it() {
        let mut w = crate::live::testing::team(2);
        let c = crate::live::testing::with_card(&mut w, 1);
        let say = |w: &World, lang, agent, sign| event(lang, w, &Event::Health { agent, sign });
        let both = |w: &World, agent, sign: Sign, en: &str, tr: &str| {
            assert_eq!(say(w, Lang::En, agent, sign.clone()), en);
            assert_eq!(say(w, Lang::Tr, agent, sign), tr);
        };
        both(
            &w,
            1,
            Sign::Stalled {
                card: c,
                minutes: 10,
            },
            "claude-1 has been idle 10 min on #1 fix it without a done · lead told",
            "claude-1 10 dk'dır #1 fix it üzerinde boşta, done demedi · lead uyarıldı",
        );
        both(
            &w,
            1,
            Sign::Waiting {
                card: c,
                to: 2,
                minutes: 11,
            },
            "claude-1 has waited 11 min on claude-2's answer for #1 fix it · lead told",
            "claude-1 11 dk'dır claude-2 yanıtını bekliyor (#1 fix it) · lead uyarıldı",
        );
        both(
            &w,
            2,
            Sign::Silent {
                card: None,
                minutes: 12,
                doing: String::new(),
            },
            "claude-2 has said nothing for 12 min in its turn · lead told",
            "claude-2 12 dk'dır turunda hiçbir şey demedi · lead uyarıldı",
        );
        both(
            &w,
            2,
            Sign::Silent {
                card: Some(c),
                minutes: 13,
                doing: "Bash cargo test".into(),
            },
            "claude-2 has said nothing for 13 min in its turn; last: Bash cargo test · lead told",
            "claude-2 13 dk'dır turunda hiçbir şey demedi; son: Bash cargo test · lead uyarıldı",
        );
        both(
            &w,
            2,
            Sign::Free { minutes: 14 },
            "claude-2 has been idle 14 min with no card · lead hears at its next turn",
            "claude-2 14 dk'dır kartsız boşta · lead sonraki turunda duyacak",
        );
        // The leader's own is the person's alone; an agent's text is
        // cleaned.
        both(
            &w,
            0,
            Sign::Looping {
                call: "Bash make\u{1b}[2J".into(),
                times: 3,
            },
            "lead failed 3 times this turn on Bash make [2J",
            "lead bu turda 3 kez başarısız oldu: Bash make [2J",
        );
    }

    #[test]
    fn a_process_the_person_ended_is_said_in_both_languages() {
        let w = crate::live::testing::team(1);
        let e = Event::Killed {
            agent: 1,
            pid: 502,
            name: "cargo".into(),
        };
        assert_eq!(
            event(Lang::En, &w, &e),
            "you ended cargo (502) under claude-1"
        );
        assert_eq!(
            event(Lang::Tr, &w, &e),
            "cargo (502) sonlandırıldı, claude-1 altında"
        );
        assert_eq!(e.kind(), EntryKind::Person);
        assert_eq!(e.agent(), 1);
    }

    #[test]
    fn a_card_held_and_released_is_said_in_both_languages() {
        let mut w = crate::live::testing::team(2);
        let c = crate::live::testing::with_card(&mut w, 1);
        let d = crate::live::testing::with_card(&mut w, 2);
        let say = |lang, e: Event| event(lang, &w, &e);
        let held = Event::Held {
            agent: 2,
            card: d,
            on: vec![c, d],
        };
        assert_eq!(
            say(Lang::En, held.clone()),
            "claude-2 gets #2 fix it after #1, #2"
        );
        assert_eq!(
            say(Lang::Tr, held),
            "claude-2 alacak: #2 fix it, #1, #2 bitince"
        );
        let released = Event::Released { agent: 2, card: d };
        assert_eq!(
            say(Lang::En, released.clone()),
            "claude-2 begins #2 fix it: what it waited on is done"
        );
        assert_eq!(
            say(Lang::Tr, released),
            "claude-2 başladı: #2 fix it, beklediği bitti"
        );
        let withdrawn = Event::Withdrawn {
            agent: 0,
            card: d,
            why: "covered".into(),
        };
        assert_eq!(
            say(Lang::En, withdrawn.clone()),
            "lead withdrew #2 fix it: covered"
        );
        assert_eq!(
            say(Lang::Tr, withdrawn),
            "lead geri çekti: #2 fix it: covered"
        );
        // A card's checks set (record 127): by the leader or by you,
        // `none` when it asks for none.
        let set = |checks: &[&str], you| Event::ChecksSet {
            agent: if you { 2 } else { 0 },
            card: d,
            checks: checks.iter().map(|c| c.to_string()).collect(),
            you,
        };
        assert_eq!(
            say(Lang::En, set(&["api", "e2e"], false)),
            "lead set #2's checks: api, e2e"
        );
        assert_eq!(
            say(Lang::Tr, set(&["api"], false)),
            "lead, #2 kartının denetimlerini değiştirdi: api"
        );
        assert_eq!(say(Lang::En, set(&[], true)), "you set #2's checks: none");
        assert_eq!(
            say(Lang::Tr, set(&["api"], true)),
            "#2 kartının denetimlerini değiştirdin: api"
        );
        let st = |by, checks: &[&str]| StoryEvent::ChecksSet {
            by,
            checks: checks.iter().map(|c| c.to_string()).collect(),
        };
        let card = &w.cards[d];
        assert_eq!(
            story(Lang::En, &w, card, &st(Some(0), &["api"])),
            "lead set the checks: api"
        );
        assert_eq!(
            story(Lang::Tr, &w, card, &st(Some(0), &[])),
            "lead denetimleri değiştirdi: hiçbiri"
        );
        assert_eq!(
            story(Lang::En, &w, card, &st(None, &[])),
            "you set the checks: none"
        );
        assert_eq!(
            story(Lang::Tr, &w, card, &st(None, &["api", "e2e"])),
            "denetimleri değiştirdin: api, e2e"
        );
        assert_eq!(st(None, &[]).kind(), StoryKind::Decision);
        assert_eq!(set(&[], true).kind(), EntryKind::Assign);
        assert_eq!(set(&[], true).card(), Some(d));
        assert_eq!(set(&[], true).agent(), 2);
        let st = StoryEvent::Withdrawn("covered".into());
        assert_eq!(story(Lang::En, &w, &w.cards[d], &st), "withdrawn: covered");
        assert_eq!(
            story(Lang::Tr, &w, &w.cards[d], &st),
            "geri çekildi: covered"
        );
        let late = Event::LateDone { agent: 2, card: d };
        assert_eq!(
            say(Lang::En, late.clone()),
            "claude-2: a late done for #2, which went to another try; it closes nothing"
        );
        w.cards[d].column = Column::Withdrawn;
        assert_eq!(
            event(Lang::En, &w, &late),
            "claude-2: a late done for #2, which was withdrawn; it closes nothing"
        );
        assert_eq!(
            event(Lang::Tr, &w, &late),
            "claude-2: #2 için geç bir done, kart geri çekilmişti; hiçbir şeyi kapatmaz"
        );
        let st = StoryEvent::Held { to: 2, on: vec![c] };
        assert_eq!(
            story(Lang::En, &w, &w.cards[d], &st),
            "for claude-2, after #1"
        );
        assert_eq!(
            story(Lang::Tr, &w, &w.cards[d], &st),
            "claude-2 için, #1 bitince"
        );
    }

    #[test]
    fn a_card_handed_failed_or_done_late_is_said_in_both_languages() {
        let mut w = crate::live::testing::team(2);
        let c = crate::live::testing::with_card(&mut w, 1);
        let say = |lang, e: Event| event(lang, &w, &e);
        let handed = |from| Event::Handed {
            agent: 2,
            card: c,
            from,
            attempt: 2,
        };
        assert_eq!(say(Lang::En, handed(None)), "claude-2 got #1 fix it, try 2");
        assert_eq!(
            say(Lang::En, handed(Some(1))),
            "claude-2 got #1 fix it from claude-1, try 2"
        );
        assert_eq!(
            say(Lang::Tr, handed(None)),
            "claude-2 aldı: #1 fix it, 2. deneme"
        );
        assert_eq!(
            say(Lang::Tr, handed(Some(1))),
            "claude-2 aldı: #1 fix it, claude-1 elinden, 2. deneme"
        );
        let failed = Event::CardFailed {
            agent: 1,
            card: c,
            why: "Cannot".into(),
        };
        assert_eq!(
            say(Lang::En, failed.clone()),
            "claude-1: #1 fix it failed: Cannot"
        );
        assert_eq!(
            say(Lang::Tr, failed),
            "claude-1: #1 fix it başarısız: Cannot"
        );
        let late = Event::LateDone { agent: 1, card: c };
        assert_eq!(
            say(Lang::En, late.clone()),
            "claude-1: a late done for #1, which went to another try; it closes nothing"
        );
        assert!(say(Lang::Tr, late).starts_with("claude-1: #1 için geç bir done"));
        let card = w.cards[c].clone();
        let tell = |lang, e: StoryEvent| story(lang, &w, &card, &e);
        let handed = |from| StoryEvent::Handed {
            from,
            to: 2,
            attempt: 3,
        };
        assert_eq!(tell(Lang::En, handed(None)), "handed to claude-2, try 3");
        assert_eq!(
            tell(Lang::En, handed(Some(1))),
            "handed from claude-1 to claude-2, try 3"
        );
        assert_eq!(tell(Lang::Tr, handed(None)), "claude-2 aldı, 3. deneme");
        assert_eq!(
            tell(Lang::Tr, handed(Some(1))),
            "claude-1 elinden claude-2 aldı, 3. deneme"
        );
        assert_eq!(
            tell(Lang::En, StoryEvent::Failed("Cannot".into())),
            "failed: Cannot"
        );
        assert_eq!(
            tell(Lang::Tr, StoryEvent::Failed("Cannot".into())),
            "başarısız: Cannot"
        );
        assert_eq!(
            tell(Lang::En, StoryEvent::LateDone { from: 1 }),
            "a late done from claude-1: it closes nothing"
        );
        assert_eq!(
            tell(Lang::Tr, StoryEvent::LateDone { from: 1 }),
            "claude-1 geç bir done gönderdi: hiçbir şeyi kapatmıyor"
        );
    }

    #[test]
    fn what_a_change_cost_is_said_in_dollars_time_and_rounds() {
        let w = crate::live::testing::team(1);
        let s = |cost, ticks, rounds| Spend {
            cost,
            ticks,
            rounds,
            unpriced: false,
        };
        assert_eq!(spend(Lang::En, &w, &s(8_400, 144, 1)), "$0.84 · 12 min");
        assert_eq!(
            spend(Lang::En, &w, &s(12_000, 300, 3)),
            "$1.20 · 25 min · 3 rounds"
        );
        assert_eq!(
            spend(Lang::En, &w, &s(100, 5, 1)),
            "$0.01 · 25 s",
            "seconds under a minute"
        );
        assert_eq!(spend(Lang::En, &w, &s(100, 11, 1)), "$0.01 · 55 s");
        assert_eq!(
            spend(Lang::En, &w, &s(100, 12, 1)),
            "$0.01 · 1 min",
            "from a minute"
        );
        assert_eq!(
            spend(Lang::En, &w, &s(0, 1_500, 2)),
            "$0.00 · 2 h 5 min · 2 rounds"
        );
        assert_eq!(
            spend(Lang::Tr, &w, &s(12_000, 300, 3)),
            "$1.20 · 25 dk · 3 tur"
        );
        assert_eq!(spend(Lang::Tr, &w, &s(100, 5, 1)), "$0.01 · 25 sn");
        // Never $0.00 for what is not known (record 132).
        let unknown = Spend {
            unpriced: true,
            ..s(0, 1_500, 2)
        };
        assert_eq!(spend(Lang::En, &w, &unknown), "? · 2 h 5 min · 2 rounds");
        let say = |e: Event| event(Lang::En, &w, &e);
        assert_eq!(
            say(Event::ChangeReady {
                agent: 1,
                files: 2,
                added: 3,
                removed: 1,
                spend: Some(s(8_400, 144, 1)),
            }),
            "claude-1 finished: change ready · 2 files +3 −1 · $0.84 · 12 min"
        );
        assert_eq!(
            say(Event::ChangeReady {
                agent: 1,
                files: 2,
                added: 3,
                removed: 1,
                spend: None,
            }),
            "claude-1 finished: change ready · 2 files +3 −1"
        );
        assert_eq!(
            say(Event::ChangeCost {
                agent: 1,
                spend: s(8_400, 144, 2),
            }),
            "claude-1's change cost $0.84 · 12 min · 2 rounds"
        );
        assert_eq!(
            event(
                Lang::Tr,
                &w,
                &Event::ChangeCost {
                    agent: 1,
                    spend: s(8_400, 144, 1),
                }
            ),
            "claude-1 değişikliği $0.84 · 12 dk tuttu"
        );
    }

    #[test]
    fn the_check_s_lines_say_how_it_went_and_mark_the_change_s_row() {
        let w = crate::live::testing::team(1);
        let say = |e: Event| event(Lang::En, &w, &e);
        let ran = |ok, lines: &[&str]| Event::Checked {
            agent: 1,
            ok,
            end: "exit 1, 1.5 s".into(),
            lines: lines.iter().map(|l| l.to_string()).collect(),
        };
        assert_eq!(
            say(ran(true, &["a"])),
            "claude-1: check passed on its change, exit 1, 1.5 s"
        );
        assert_eq!(
            say(ran(false, &["a", "b"])),
            "claude-1: check failed on its change, exit 1, 1.5 s · b"
        );
        assert_eq!(
            say(ran(false, &[])),
            "claude-1: check failed on its change, exit 1, 1.5 s · no output"
        );
        assert_eq!(
            say(Event::CheckFailed {
                agent: 1,
                why: "x".into()
            }),
            "claude-1: check did not run: x"
        );
        assert!(
            say(Event::CheckAnswered {
                agent: 1,
                yes: true
            })
            .starts_with("you said yes to the repository's check (claude-1 asked)")
        );
        assert!(
            say(Event::CheckAnswered {
                agent: 1,
                yes: false
            })
            .starts_with("you said no to the repository's check (claude-1 asked)")
        );
        let item = InboxItem {
            kind: InboxKind::Check,
            agent: 1,
            detail: ".imece/check".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        assert_eq!(
            inbox(Lang::En, &w, &item),
            "claude-1: run the repository's check .imece/check on its changes? enter shows it"
        );
        let good = |ok| Checked {
            ok,
            end: String::new(),
            lines: Vec::new(),
        };
        let mark = |c: &Check| check_mark(Lang::En, c);
        assert_eq!(
            mark(&Check::Ran(good(true))),
            Some(("✓ check · ", Some(true)))
        );
        assert_eq!(
            mark(&Check::Ran(good(false))),
            Some(("✗ check · ", Some(false)))
        );
        assert_eq!(mark(&Check::Running), Some(("● check · ", None)));
        assert_eq!(mark(&Check::Asked), Some(("▶ check · ", None)));
        assert_eq!(mark(&Check::None), None);
        assert_eq!(mark(&Check::Failed("x".into())), None);
        assert_eq!(
            check_mark(Lang::Tr, &Check::Running),
            Some(("● kontrol · ", None))
        );
    }
    #[test]
    fn a_plan_is_worded_in_the_journal_and_on_its_row_in_both_languages() {
        let mut w = crate::live::testing::team(1);
        let say = |lang, e: &Event| event(lang, &w, e);
        let planned = Event::Planned {
            leader: 0,
            cards: 3,
            cost: 12_000,
        };
        assert_eq!(
            say(Lang::En, &planned),
            "lead planned 3 cards for $1.20; waits for your yes"
        );
        assert_eq!(
            say(Lang::Tr, &planned),
            "lead 3 kart planladı, $1.20; onayınızı bekliyor"
        );
        let yes = Event::PlanAnswered {
            leader: 0,
            yes: true,
        };
        let no = Event::PlanAnswered {
            leader: 0,
            yes: false,
        };
        assert_eq!(say(Lang::En, &yes), "you said yes to lead's plan");
        assert_eq!(say(Lang::En, &no), "you said no to lead's plan");
        assert_eq!(say(Lang::Tr, &yes), "lead planına evet dediniz");
        assert_eq!(say(Lang::Tr, &no), "lead planına hayır dediniz");
        // The row reads the plan itself: nothing there, nothing counted.
        assert_eq!(plan_figures(&w, 0), (0, 0));
        let item = w.propose(
            0,
            Proposal {
                cards: vec![
                    Planned {
                        title: "Parser".into(),
                        tier: "cheap".into(),
                        paths: Vec::new(),
                        checks: Vec::new(),
                        body: "b".into(),
                        after: Vec::new(),
                    };
                    2
                ],
                cost: 12_500,
                tick: 0,
            },
        );
        assert_eq!(plan_figures(&w, 0), (2, 12_500));
        assert_eq!(
            inbox(Lang::En, &w, &w.inbox[item]),
            "lead: plans 2 cards for $1.25; enter reads it"
        );
        assert_eq!(
            inbox(Lang::Tr, &w, &w.inbox[item]),
            "lead: 2 kart planladı, $1.25; enter okur"
        );
        let note = crate::notify::Note::Item(item);
        assert_eq!(
            notice(Lang::En, &w, note),
            "imece · lead: a plan waits for your yes"
        );
    }

    #[test]
    fn what_became_of_a_proposal_is_said_in_both_languages() {
        for lang in [Lang::En, Lang::Tr] {
            let t = lang.text();
            let on_trunk = Landed::Integrated("0123456789".into());
            assert_eq!(
                landed(t, "w", &on_trunk),
                fill(t.ev_landed_integrated, &["w", "0123456"])
            );
            assert_eq!(
                landed(t, "w", &Landed::Queued),
                fill(t.ev_landed_queued, &["w"])
            );
            assert_eq!(
                landed(t, "w", &Landed::Withdrawn),
                fill(t.ev_landed_withdrawn, &["w"])
            );
            let symbols = Landed::Conflicted(vec!["a".into(), "b".into()]);
            assert_eq!(
                landed(t, "w", &symbols),
                fill(t.ev_landed_conflicted, &["w", "a, b"])
            );
            let reasons = Landed::Rejected(vec!["a".into(), "b".into()]);
            assert_eq!(
                landed(t, "w", &reasons),
                fill(t.ev_landed_rejected, &["w", "a; b"])
            );
            let reasons = Landed::Rejected(vec!["verification_quota:day".into(), "x:y".into()]);
            assert_eq!(
                landed(t, "w", &reasons),
                fill(
                    t.ev_landed_rejected,
                    &["w", &format!("{}; x:y", t.landed_whys[3])]
                )
            );
        }
    }

    #[test]
    fn lonca_s_words_for_why_its_queue_refused_are_said_as_a_person_says_them() {
        let t = Lang::En.text();
        for (word, said) in [
            ("verification_failed:build", "its build check failed"),
            ("verification_error:test", "its test check could not run"),
            ("verification_missing:api", "its api check never ran"),
            (
                "verification_quota:session",
                "this session's verification time is used up",
            ),
            (
                "verification_quota:day",
                "the day's verification time is used up until midnight UTC",
            ),
            ("verification_quota:week", "verification_quota:week"),
            ("merge_conflict", "it does not merge with trunk"),
            ("merge_conflict:x", "merge_conflict:x"),
            ("conflict:a.rs#f", "it clashes with trunk at a.rs#f"),
            ("stale_trunk", "it is based on a trunk that moved on"),
            ("unknown_base", "it is based on a trunk that moved on"),
            ("stale_trunk:x", "stale_trunk:x"),
            ("frozen:audit", "the repository is frozen: audit"),
            ("rejected: not this", "rejected: not this"),
            ("unknown_check:api", "Lonca's policy has no api check"),
            (
                "invalid_checks",
                "the card's checks are not a list Lonca takes",
            ),
            ("invalid_checks:x", "invalid_checks:x"),
            (
                "checks_not_a_change",
                "checks go only with a proposed change",
            ),
            ("checks_not_a_change:x", "checks_not_a_change:x"),
        ] {
            assert_eq!(refused_why(t, word), said);
        }
        // Every word has a Turkish one of its own.
        let tr = Lang::Tr.text();
        let words: std::collections::HashSet<&str> = tr.landed_whys.iter().copied().collect();
        assert_eq!(words.len(), tr.landed_whys.len());
        assert_eq!(refused_why(tr, "frozen:audit"), "depo donduruldu: audit");
    }

    #[test]
    fn a_repository_that_lets_changes_in_unseen_says_so() {
        let t = Lang::En.text();
        assert_eq!(store_down(t, StoreDown::Unguarded), t.store_unguarded);
    }
}
