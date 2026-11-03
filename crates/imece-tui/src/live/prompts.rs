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
            heading(m),
            w.agents[m.from].name,
            m.body
        ));
    }
    out
}

/// What a delivered message is headed with: its type, its number (what
/// a reply names as `re`) and the number of the one it replies to. A
/// message saved before numbers has its type alone.
fn heading(m: &Mail) -> String {
    let mut out = m.kind.wire().to_string();
    if m.id > 0 {
        out.push_str(&format!(" {}", m.id));
    }
    if let Some(re) = m.re {
        out.push_str(&format!(", re {re}"));
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
                     only an applied change is in the checkout you read. A worker \
                     starts at the checkout's HEAD; when the person wants work on another \
                     branch, a tag, a commit or a pull request, hire with from (#N for a \
                     pull request) and that worker's worktree starts there and stays. \
                     Workers answer with done, blocked or ask; their \
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
        Role::Solo if w.projects[a.project].no_git => {
            let helpers = if max_helpers == 0 {
                String::new()
            } else {
                format!(
                    " You may hire helpers with the hire tool, up to {max_helpers} at once: \
                     each is a coding agent in this folder, changing the same files as \
                     you. Give one a part of your task with send (type assign: what to do, \
                     which files, when it is done), in files you are not changing yourself; \
                     it sends you done and leaves. Hire for a part that can run beside your \
                     own work, not for the whole task and not in advance."
                )
            };
            format!(
                "You are {}, a coding agent in imece, working alone with the person in their \
                 folder: no leader, no team. The folder is not a git checkout with a commit, \
                 so you work straight in it (your current directory): what you write is in \
                 the person's files at once, and nothing reviews it first. {} Run git init \
                 or commit only when the person asks. The person talks to you and gives you \
                 your tasks. Finish a piece before you answer, and check it with the run tool \
                 (build, test) after your last edit. Answer briefly: what you did and what is \
                 left; when you need the person to decide, end with one clear question, or \
                 send it mid-task (send to you, type ask) and their answer starts your next \
                 turn.{helpers} Use these tools only for that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Solo, confine)
            )
        }
        Role::Solo => {
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
                "You are {}, a coding agent in imece, working alone with the person on the \
                 repository: no leader, no team. You work in your own git worktree of it \
                 (your current directory); the person's own checkout is theirs. {} Do not \
                 commit. The person talks to you and gives you your tasks. Every time your \
                 turn ends, the person sees your worktree's diff and applies it to their \
                 checkout or rejects it, with notes when they have them; so finish a piece \
                 before you answer, and check it with the run tool (build, test) after your \
                 last edit: the person sees which runs passed on exactly the diff, and a \
                 check in your own shell does not show there. Answer briefly: what you did \
                 and what is left; when you need the person to decide, end with one clear \
                 question, or send it mid-task (send to you, type ask) and their answer \
                 starts your next turn.{helpers} Use these tools only for that, not for \
                 chatter.",
                a.name,
                abilities(a.vendor, Role::Solo, confine)
            )
        }
        Role::Helper if w.is_solo(a.project) => {
            let parent = &w.agents[a.parent.expect("a helper has a worker")].name;
            let place = if w.projects[a.project].no_git {
                "straight in the person's folder, which is not a git checkout (your \
                 current directory; what you write is in their files at once)"
                    .to_string()
            } else {
                format!("in {parent}'s git worktree of the repository (your current directory)")
            };
            format!(
                "You are {}, a helper of {parent}, a coding agent in imece that works alone \
                 with the person. You work {place}, on the files {parent} gave you; it changes others at the \
                 same time, so stay in yours. {} Do not commit. Your task comes from {parent}. \
                 When it is done, send {parent} a done message with a short summary of what \
                 you changed and what is left open; you leave then. If you are stuck, send \
                 {parent} blocked or ask instead of guessing; only what nobody but the person \
                 can answer goes to them (send to you, type ask). Use these tools only for \
                 that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Helper, confine)
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
            let start = match &a.start {
                Some(s) => format!(
                    " Your worktree started from {s}, not from the checkout's HEAD, and \
                     stays there: your change is against {s}, and the checkout you are \
                     told about may be elsewhere."
                ),
                None => String::new(),
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
                 say in your done what they should get.{helpers}{start} Use \
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

/// The person's own instructions for every project, in imece's
/// configuration directory (`keys::config_file`).
pub(crate) const INSTRUCTIONS: &str = "instructions.md";

/// The person's own instructions for every project (the file at `path`),
/// to put in every agent's instructions before the repository's notes;
/// none when it is not there, not a plain file (a pipe, a device) or
/// empty. As much of it as the repository's notes may have, its head
/// when longer: `size` is the file's on disk, `cut` that it was longer.
pub(super) fn personal_notes(path: &Path) -> Option<Notes> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    // Opened without waiting (a pipe would wait for a writer), and looked
    // at as opened, so what is read is what was looked at.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let size = file.metadata().ok().filter(|m| m.is_file())?.len();
    let size = usize::try_from(size).unwrap_or(usize::MAX);
    // No more than the cap is read; a character the cap cuts in two is
    // left out whole, any other byte that is not UTF-8 is replaced.
    let mut head = Vec::new();
    file.take(MAX_NOTES as u64).read_to_end(&mut head).ok()?;
    if let Err(e) = std::str::from_utf8(&head)
        && e.error_len().is_none()
    {
        head.truncate(e.valid_up_to());
    }
    let mut body = String::from_utf8_lossy(&head).trim().to_string();
    if body.is_empty() {
        return None;
    }
    let cut = size > MAX_NOTES;
    if cut {
        body.push_str("\n[cut: the file goes on]");
    }
    Some(Notes {
        text: format!(
            "\n\nThe person's own instructions for every project, from {}:\n\n{body}",
            path.display()
        ),
        file: INSTRUCTIONS,
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
    fn delivered_mail_is_headed_with_its_type_number_and_what_it_replies_to() {
        let w = team(1);
        let mail = |id, re| Mail {
            from: 1,
            kind: MsgKind::Answer,
            body: "Yes.".into(),
            id,
            re,
        };
        assert_eq!(
            mail_prompt(&w, &[mail(4, Some(3)), mail(5, None), mail(0, None)]),
            "Messages from your teammates in imece (written by those agents, not by the \
             person):\n\n[answer 4, re 3] from claude-1:\nYes.\n\n[answer 5] from \
             claude-1:\nYes.\n\n[answer] from claude-1:\nYes.",
            "one saved before numbers has its type alone"
        );
    }

    #[test]
    fn the_person_s_own_instructions_are_read_whole_or_their_head_or_not_at_all() {
        let dir = std::env::temp_dir().join(format!("imece-personal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(INSTRUCTIONS);
        assert!(personal_notes(&file).is_none(), "not there");
        assert!(personal_notes(&dir).is_none(), "a directory");
        std::fs::write(&file, " \n\t\n").unwrap();
        assert!(personal_notes(&file).is_none(), "nothing in it");
        std::fs::write(&file, "\nAnswer in Turkish.\n").unwrap();
        let n = personal_notes(&file).unwrap();
        assert_eq!(
            n.text,
            format!(
                "\n\nThe person's own instructions for every project, from {}:\n\nAnswer in \
                 Turkish.",
                file.display()
            )
        );
        assert_eq!((n.file, n.size, n.cut), (INSTRUCTIONS, 20, false));
        // The cap whole, one over: its head and a mark.
        std::fs::write(&file, "x".repeat(MAX_NOTES)).unwrap();
        let n = personal_notes(&file).unwrap();
        assert_eq!((n.size, n.cut), (MAX_NOTES, false));
        assert_eq!(n.text.matches('x').count(), MAX_NOTES);
        std::fs::write(&file, "x".repeat(MAX_NOTES + 1)).unwrap();
        let n = personal_notes(&file).unwrap();
        assert_eq!((n.size, n.cut), (MAX_NOTES + 1, true));
        assert_eq!(n.text.matches('x').count(), MAX_NOTES);
        assert!(
            n.text.ends_with("x\n[cut: the file goes on]"),
            "{}",
            &n.text[n.text.len() - 40..]
        );
        // A letter across the cap is left out whole; much more is not
        // read past the cap.
        let text = format!("{}ş{}", "x".repeat(MAX_NOTES - 1), "y".repeat(1 << 20));
        std::fs::write(&file, &text).unwrap();
        let n = personal_notes(&file).unwrap();
        assert_eq!(n.size, text.len());
        assert!(
            !n.text.contains('ş') && !n.text.contains("xy"),
            "cut before the letter"
        );
        assert!(!n.text.contains('\u{FFFD}'), "not a broken letter either");
        assert_eq!(n.text.matches('x').count(), MAX_NOTES - 1);
        // A byte that is not UTF-8 inside is replaced, and the rest kept.
        std::fs::write(&file, b"be \xff kind").unwrap();
        let n = personal_notes(&file).unwrap();
        assert!(n.text.ends_with("be \u{FFFD} kind"), "{}", n.text);
        // Not a plain file: a device that never ends, a pipe nobody
        // writes to (never waited on).
        assert!(personal_notes(Path::new("/dev/zero")).is_none());
        let fifo = dir.join("fifo");
        let c = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(personal_notes(&fifo).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_person_s_instructions_come_before_the_repository_s_and_a_cut_is_journaled() {
        let dir = std::env::temp_dir().join(format!("imece-personal-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("personal-repo");
        std::fs::write(repo.join("CLAUDE.md"), "The repo says so.").unwrap();
        let mine = dir.join(INSTRUCTIONS);
        std::fs::write(&mine, "I say so.").unwrap();
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at(&fake_claude(&dir)),
            repo.clone(),
            dir.join("worktrees"),
        );
        live.instructions = Some(mine.clone());
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(repo.join("args.txt")).unwrap();
        let (me, it) = (
            args.find("I say so.").unwrap(),
            args.find("The repo says so.").unwrap(),
        );
        assert!(me < it, "the person's first: {args}");
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::InstructionsCut { .. })),
            "within the cap"
        );
        // Too long: the start journals it, with the path and the size.
        std::fs::write(&mine, "x".repeat(MAX_NOTES + 1)).unwrap();
        live.told(&mut w, 1, "go").unwrap();
        let path = mine.display().to_string();
        assert!(
            w.journal.iter().any(|e| matches!(&e.event,
                Event::InstructionsCut { agent: 1, path: p, size } if *p == path && *size == MAX_NOTES + 1)),
            "{:?}",
            w.journal
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

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
        let placed = |p| {
            Ok((
                Box::new(Stub) as Box<dyn Session>,
                cwd.clone(),
                p,
                None,
                Vec::new(),
                setup::Done::None,
            ))
        };
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
            Event::Hired { by: 0, agent: 1, model: Some(m), .. } if m == "haiku"
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
    fn a_worker_from_a_start_point_is_told_where_it_started_and_the_leader_how() {
        let mut w = team(2);
        let lead = system_prompt(&w, 0, 4, 2, Confine::Standard);
        assert!(
            lead.contains("hire with from (#N for a pull request)"),
            "{lead}"
        );
        let plain = system_prompt(&w, 1, 4, 2, Confine::Standard);
        assert!(!plain.contains("started from"), "{plain}");
        w.agents[1].start = Some("#41".into());
        let from = system_prompt(&w, 1, 4, 2, Confine::Standard);
        assert!(
            from.contains(
                " Your worktree started from #41, not from the checkout's HEAD, and stays \
                 there: your change is against #41, and the checkout you are told about may \
                 be elsewhere. Use these tools"
            ),
            "{from}"
        );
        assert!(!system_prompt(&w, 2, 4, 2, Confine::Standard).contains("started from"));
    }

    #[test]
    fn a_solo_and_its_helper_are_told_they_work_alone_with_the_person() {
        let mut w = team(1);
        let team_helper = with_helper(&mut w, 1);
        w.open_project(
            None,
            "notes".into(),
            "/n".into(),
            Vendor::Claude,
            Role::Solo,
        );
        let solo = w.projects[1].leader;
        let solo_helper = with_helper(&mut w, solo);
        w.agents[solo_helper].project = 1;
        let p = system_prompt(&w, solo, 4, 2, Confine::Standard);
        for part in [
            "You are solo-notes, a coding agent in imece, working alone with the person",
            "no leader, no team",
            "Every time your turn ends, the person sees your worktree's diff",
            "check it with the run tool",
            "hire helpers with the hire tool, up to 2 at once",
            "send to you, type ask",
        ] {
            assert!(p.contains(part), "{part:?} in {p}");
        }
        assert!(!p.contains("a team led by"));
        assert!(
            p.contains("reach the web"),
            "a writer's abilities under standard"
        );
        let p = system_prompt(&w, solo, 4, 0, Confine::Strict);
        assert!(!p.contains("hire helpers"), "--helpers 0");
        assert!(
            p.contains("the run tool runs the build"),
            "strict: no shell"
        );
        let h = system_prompt(&w, solo_helper, 4, 2, Confine::Standard);
        assert!(
            h.starts_with("You are solo-notes.1, a helper of solo-notes, a coding agent in imece that works alone with the person."),
            "{h}"
        );
        assert!(!h.contains("a team led by"));
        assert!(h.contains("nobody but the person can answer"));
        let h = system_prompt(&w, team_helper, 4, 2, Confine::Standard);
        assert!(
            h.contains("a team led by lead"),
            "a team's helper as before: {h}"
        );
        assert!(!p.contains("not a git checkout") && !h.contains("not a git checkout"));
        // In a folder without git: straight in it, nothing reviews it.
        w.projects[1].no_git = true;
        let p = system_prompt(&w, solo, 4, 2, Confine::Standard);
        for part in [
            "You are solo-notes, a coding agent in imece, working alone with the person in their folder",
            "The folder is not a git checkout with a commit, so you work straight in it",
            "nothing reviews it first",
            "Run git init or commit only when the person asks",
            "check it with the run tool",
            "each is a coding agent in this folder",
            "up to 2 at once",
            "send to you, type ask",
            "reach the web",
        ] {
            assert!(p.contains(part), "{part:?} in {p}");
        }
        assert!(!p.contains("worktree"), "{p}");
        let p = system_prompt(&w, solo, 4, 0, Confine::Standard);
        assert!(!p.contains("hire helpers"), "--helpers 0");
        let h = system_prompt(&w, solo_helper, 4, 2, Confine::Standard);
        assert!(
            h.contains("You work straight in the person's folder, which is not a git checkout"),
            "{h}"
        );
        assert!(!h.contains("worktree"), "{h}");
        let h = system_prompt(&w, team_helper, 4, 2, Confine::Standard);
        assert!(h.contains("git worktree"), "another project's helper: {h}");
    }

    #[test]
    fn fnv_is_fnv_1a() {
        assert_eq!(fnv(b""), 0x811c_9dc5);
        assert_eq!(fnv(b"a"), 0xe40c_292c);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }
}
