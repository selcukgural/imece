//! What an agent is told: its system prompt, the repository's notes,
//! what it can do, and its mail and offers as a prompt.

use super::*;

/// The prompt that offers a worker the open `cards`, each with who posted
/// it.
pub(super) fn board_prompt(w: &World, cards: &[(CardId, AgentId)]) -> String {
    let mut out = String::from(
        "Open cards on imece's board (posted by your teammates, not by the person):\n",
    );
    for &(c, by) in cards {
        let by = &w.agents[by].name;
        out.push_str(&format!(
            "\n#{} {} (posted by {by})",
            c + 1,
            w.cards[c].title
        ));
    }
    out.push_str(
        "\n\nIf one fits you, take it with claim and do it. If none does, end your \
         turn with one line saying why.",
    );
    out
}

/// The prompt that hands an agent its mail.
pub(super) fn mail_prompt(w: &World, mail: &[Mail]) -> String {
    let mut out = String::from(
        "Messages from your teammates in imece (written by those agents, not by the person):",
    );
    for m in mail {
        out.push_str(&format!(
            "\n\n[{}] from {}:\n{}",
            m.kind.wire(),
            w.agents[m.from].name,
            m.body
        ));
    }
    out
}

/// The system prompt of agent `id`; a leader may hire up to `max_workers`,
/// a worker may have up to `max_helpers` at once; `confine` says what
/// the session can do to the files and the world outside.
pub(super) fn system_prompt(
    w: &World,
    id: AgentId,
    max_workers: usize,
    max_helpers: usize,
    confine: Confine,
) -> String {
    let a = &w.agents[id];
    let workers: Vec<&str> = w
        .agents
        .iter()
        .filter(|o| o.role == Role::Worker && !o.gone && o.project == a.project)
        .map(|o| o.name.as_str())
        .collect();
    let leader = &w.agents[w.projects[a.project].leader].name;
    match a.role {
        Role::Leader => {
            let team = if max_workers == 0 {
                "The person runs you without workers: answer from what you read, and \
                 say so when the work would need one."
                    .to_string()
            } else {
                let so_far = if workers.is_empty() {
                    "You have no workers yet.".to_string()
                } else {
                    format!("The workers so far are {}.", workers.join(", "))
                };
                format!(
                    "Workers are hired by you with the hire tool, up to {max_workers}; \
                     each is a coding agent in its own git worktree of the repository. \
                     {so_far} Hire for the work at hand, not in advance: one worker \
                     takes several tasks in turn, so hire a second only when two tasks \
                     can run at once. Pick a tier, not a model: cheap for small, \
                     well-specified mechanical work (renames, tests of known behaviour, \
                     docs), standard (the default) for ordinary fixes and features, \
                     strong for design that cuts across the code, hard debugging and \
                     review. Give each worker its part with the send tool (type \
                     assign: what to do, which files, when it is done); give different \
                     workers different files where you can. A file most tasks touch (a \
                     changelog, a version, a shared list) belongs to one worker at a \
                     time: tell the others to leave it out and to say in their done what \
                     it should get, since two changes to the same lines cannot both be \
                     applied. Work that any worker could \
                     pick up may go on the board instead (post); idle workers take it \
                     themselves. A worker's done shows the person its change (its \
                     worktree's diff); they apply it to their checkout or reject it, and \
                     only an applied change is in the checkout you read. Workers answer \
                     with done, blocked or ask; their \
                     messages start your next turn."
                )
            };
            format!(
                "You are {}, the leader of a team of coding agents in imece, on the \
                 repository in your current directory. {} The person talks to you. \
                 {team} You never change files yourself, so do not offer to; offer \
                 only work a worker will do. Answer the person briefly: what you \
                 found or assigned, and when all is done, what was done and what is \
                 left. When you need the person to decide, end with one clear \
                 question; mid-task, send it to them (send to you, type ask) and \
                 their answer starts your next turn.",
                a.name,
                abilities(a.vendor, Role::Leader, confine)
            )
        }
        Role::Helper => {
            let parent = &w.agents[a.parent.expect("a helper has a worker")].name;
            format!(
                "You are {}, a helper of {parent}, a coding agent in imece, a team led by \
                 {leader}. You work in {parent}'s git worktree of the repository (your \
                 current directory), on the files {parent} gave you; it changes others \
                 at the same time, so stay in yours. {} Do not commit. Your task comes \
                 from {parent}. When it is done, send {parent} a done message with a \
                 short summary of what you changed and what is left open; you leave \
                 then. If you are stuck, send {parent} blocked or ask instead of \
                 guessing; only what no teammate can answer goes to the person (send \
                 to you, type ask). Use these tools only for that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Helper, confine)
            )
        }
        _ => {
            let helpers = if max_helpers == 0 {
                String::new()
            } else {
                format!(
                    " You may hire helpers with the hire tool, up to {max_helpers} at once: \
                     each is a coding agent in your worktree, changing the same files as \
                     you. Give one a part of your task with send (type assign: what to do, \
                     which files, when it is done), in files you are not changing yourself; \
                     it sends you done and leaves. Hire for a part that can run beside your \
                     own work, not for the whole task and not in advance."
                )
            };
            format!(
                "You are {}, a coding agent in imece, a team led by {leader}. You work \
                 in your own git worktree of the repository (your current \
                 directory); the others work in theirs. {} Do not commit. Tasks come \
                 from {leader} or from the person. When you finish a task from \
                 {leader}, send it a done message with a short summary of what \
                 you changed and what is left open; your done also shows the person \
                 your worktree's diff, which they apply to their checkout or reject, \
                 so finish before you send it and check it with the run tool (build, \
                 test) after your last edit: the person sees which runs passed on exactly \
                 the diff you hand in, and a check in your own shell does not show there. \
                 If you are stuck, send blocked \
                 or ask instead of guessing: {leader} first, it knows the plan; what only \
                 the person can decide goes to them (send to you, type ask), and their \
                 answer starts your next turn. imece may offer you open cards from \
                 the board; take one that fits with claim, and send whoever posted \
                 it a done message when it is done. Work you find outside your \
                 task goes on the board (post), not into your change; so do files \
                 your task does not name that every task touches (a changelog): \
                 say in your done what they should get.{helpers} Use \
                 these tools only for that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Worker, confine)
            )
        }
    }
}

/// The files a repository keeps its notes for coding agents in, in the
/// order they are looked for; the first one there is used.
pub(super) const NOTES_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md"];

/// The most of a notes file that goes into a prompt, in KiB.
pub(crate) const MAX_NOTES_KIB: usize = 16;

/// [`MAX_NOTES_KIB`] in bytes.
pub(super) const MAX_NOTES: usize = MAX_NOTES_KIB << 10;

/// The repository's notes for coding agents (its `AGENTS.md` or
/// `CLAUDE.md`), to append to an agent's instructions. The vendors' CLIs
/// are started so that they load nothing from the repository (no hooks,
/// no settings), which drops these notes too; imece passes them itself.
/// Codex loads `AGENTS.md` from its directory on its own and gets it only
/// when the notes are in another file.
pub(super) fn repo_notes(repo: &Path, vendor: Vendor) -> Option<Notes> {
    let (file, text) = NOTES_FILES
        .iter()
        .find_map(|f| std::fs::read_to_string(repo.join(f)).ok().map(|t| (*f, t)))?;
    if text.trim().is_empty() || (vendor == Vendor::Codex && file == "AGENTS.md") {
        return None;
    }
    let mut body = text.trim().to_string();
    let size = body.len();
    let cut = size > MAX_NOTES;
    if cut {
        // The last character boundary at or before the cap.
        let at = (0..=MAX_NOTES)
            .rev()
            .find(|&i| body.is_char_boundary(i))
            .unwrap_or(0);
        body.truncate(at);
        body.push_str("\n[cut: the file goes on]");
    }
    Some(Notes {
        text: format!("\n\nThe repository's notes for coding agents, from its {file}:\n\n{body}"),
        file,
        size,
        cut,
    })
}

/// What an agent of `vendor` in `role` can do to the files and the world
/// outside, as its session is set up at `confine`. Under strict Claude
/// Code and Gemini get file tools and no shell, and Codex (which always
/// has a shell) a sandbox without the network; from standard on a worker
/// has its vendor's shell and the web, a leader the web and reading
/// commands. Codex's sandbox is read-only for a leader and confined to
/// the agent's directory for the rest at every level.
pub(super) fn abilities(vendor: Vendor, role: Role, confine: Confine) -> &'static str {
    match (vendor, role, confine.full()) {
        (Vendor::Codex, Role::Leader, false) => {
            "You can read files and run commands that only read; the sandbox refuses \
             writes and the network."
        }
        (Vendor::Codex, Role::Leader, true) => {
            "You can read files, search the web and run commands that only read; the \
             sandbox refuses writes and the network."
        }
        (Vendor::Codex, _, false) => {
            "You can read and change files here and run commands; the sandbox keeps \
             writes inside this directory and refuses the network."
        }
        (Vendor::Codex, _, true) => {
            "You can read and change files here, run commands and reach the web; the \
             sandbox keeps writes inside this directory. What needs more than the \
             sandbox gives waits for the person's yes, unless it is one program with its \
             arguments (no pipes, redirections, && or $, one line) that runs the build, \
             the tests or git's reading commands. A refusal is the person's decision, \
             not an error: do not try the same another way."
        }
        (_, Role::Leader, false) => {
            "You can read files but not change them; the run tool runs checks (cargo \
             check, clippy, test; git diff, log) in the checkout."
        }
        (_, Role::Leader, true) => {
            "You can read files and the web but not change files; the run tool runs \
             checks (cargo check, clippy, test; git diff, log) in the checkout."
        }
        (_, _, false) => {
            "You can read and change files here; the run tool runs the build, the \
             tests and git's reading commands, nothing else."
        }
        (Vendor::Claude, _, true) => {
            "You can read and change files here, run commands in your shell and reach \
             the web. A command that is one program with its arguments (no pipes, \
             redirections, && or $, one line) and runs the build, the tests or git's \
             reading commands runs at once; most others wait for the person's yes, so \
             prefer the first kind. A refusal is the person's decision, not an error: \
             do not try the same another way. The run tool runs the build and the \
             tests with their output tidied."
        }
        (Vendor::Gemini, _, true) => {
            "You can read and change files here, run commands in your shell and reach \
             the web. Give each command your files' directory as its dir_path: one that \
             runs there and is one program with its arguments (no pipes, redirections, \
             && or $, one line) running the build, the tests or git's reading commands \
             runs at once; most others wait for the person's yes, so prefer the first \
             kind. A refusal is the person's decision, not an error: do not try the same \
             another way. The run tool runs the build and the tests with their output \
             tidied."
        }
        (_, _, true) => {
            "You can read and change files here, run commands in your shell and reach \
             the web; the run tool runs the build and the tests with their output \
             tidied."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn a_cut_notes_file_goes_into_the_journal_when_the_session_starts() {
        let dir = std::env::temp_dir().join(format!("imece-notes-cut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(1);
        let mut live = idle_live_in(&dir, 2);
        // A file within the cap: nothing in the journal about it.
        std::fs::write(dir.join("CLAUDE.md"), "x".repeat(MAX_NOTES)).unwrap();
        live.told(&mut w, 0, "go").unwrap();
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::NotesCut { .. })),
            "{:?}",
            w.journal
        );
        // One over: the start journals the file and its whole size, for
        // the agent that starts.
        std::fs::write(dir.join("CLAUDE.md"), "x".repeat(MAX_NOTES + 1)).unwrap();
        live.told(&mut w, 1, "go").unwrap();
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::NotesCut { agent: 1, file, size } if file == "CLAUDE.md" && *size == MAX_NOTES + 1
        ));
        assert_eq!(
            w.journal.last().unwrap().event.kind(),
            EntryKind::Assumption
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn where_a_worktree_stands_goes_into_the_journal() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let cwd = PathBuf::from("/work/claude-1");
        let placed = |p| Ok((Box::new(Stub) as Box<dyn Session>, cwd.clone(), p, None));
        live.ready(&mut w, 1, placed(Some(worktree::Placed::Fresh)));
        live.ready(&mut w, 1, placed(Some(worktree::Placed::AtHead)));
        live.ready(&mut w, 0, placed(None));
        assert!(w.journal.is_empty(), "{:?}", w.journal);
        assert!(matches!(live.slots[1], Slot::Running(_)));
        live.ready(
            &mut w,
            1,
            placed(Some(worktree::Placed::Moved {
                from: "aaaaaaa".into(),
                to: "bbbbbbb".into(),
            })),
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::WorktreeMoved { agent: 1, from, to } if from == "aaaaaaa" && to == "bbbbbbb"
        ));
        for (why, commits) in [
            (worktree::Why::Changes, false),
            (worktree::Why::Commits, true),
        ] {
            live.ready(
                &mut w,
                2,
                placed(Some(worktree::Placed::Kept {
                    at: "aaaaaaa".into(),
                    head: "bbbbbbb".into(),
                    why,
                })),
            );
            assert!(matches!(
                &w.journal.last().unwrap().event,
                Event::WorktreeKept { agent: 2, at, head, commits: c }
                    if at == "aaaaaaa" && head == "bbbbbbb" && *c == commits
            ));
        }
        assert_eq!(w.journal.len(), 3);
    }

    #[test]
    fn the_leader_hires_a_worker_named_after_its_vendor_with_the_tier_s_model() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.max_workers = 2;
        let got = live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        assert!(
            got.starts_with("claude-1 joined (claude, haiku).")
                && got.contains("1 more may be hired"),
            "{got}"
        );
        let a = &w.agents[1];
        assert_eq!(
            (a.name.as_str(), a.vendor, a.role),
            ("claude-1", Vendor::Claude, Role::Worker)
        );
        assert_eq!(a.status, Status::Idle);
        assert_eq!(a.project, 0);
        assert_eq!(live.models[1].as_deref(), Some("haiku"));
        assert!(live.tiered[1]);
        assert_eq!(live.slots.len(), 2, "every per-agent list grew");
        assert_eq!(live.mail.len(), 2);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Hired { by: 0, agent: 1, model: Some(m) } if m == "haiku"
        ));
        assert!(
            w.agents[0]
                .trace
                .iter()
                .any(|l| l.contains("hire claude-1 cheap haiku"))
        );
        // The default tier is the vendor's own model, which has nothing to
        // fall back to.
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(
            got.starts_with("claude-2 joined (claude, the default model)."),
            "{got}"
        );
        assert!(got.contains("0 more may be hired"), "{got}");
        assert_eq!(live.models[2], None);
        assert!(!live.tiered[2]);
        // Exactly the cap, not one more.
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(
            err.contains("full: 2 workers") && err.contains("--agents 2"),
            "{err}"
        );
        assert_eq!(w.agents.len(), 3);
        // A worker is given to the leader by send; it is told so.
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-2", "assign", "Add a test"),
        )
        .unwrap();
    }

    #[test]
    fn fnv_is_fnv_1a() {
        assert_eq!(fnv(b""), 0x811c_9dc5);
        assert_eq!(fnv(b"a"), 0xe40c_292c);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }
}
