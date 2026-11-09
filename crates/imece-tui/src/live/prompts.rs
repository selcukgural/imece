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
        // A card handed again keeps the paths it owns and the checks it
        // asks for (records 126, 127).
        let card = m.card.map(|c| &w.cards[c]);
        let paths = card.map_or(&m.paths, |c| &c.paths);
        let checks = card.map_or(&m.checks, |c| &c.checks);
        out.push_str(&format!(
            "\n\n[{}] from {}:\n{}{}{}",
            heading(w, m),
            w.agents[m.from].name,
            m.body,
            owns(paths),
            asks(checks)
        ));
    }
    out
}

/// What a delivered message is headed with: its type, its number (what
/// a reply names as `re`) and the number of the one it replies to; a
/// worker's `done` adds what its change cost (record 113: `[done 12 ·
/// $0.84]`, the newest change of the worker's settled), so a leader
/// learns what its tiers cost. A done that failed or came late says so
/// instead, and an assign that hands a card again names it and its try
/// (record 126: `[done 12 · failed]`, `[assign 14 · #3, try 2]`). A
/// message saved before numbers has its type alone.
fn heading(w: &World, m: &Mail) -> String {
    let mut out = m.kind.wire().to_string();
    if m.id > 0 {
        out.push_str(&format!(" {}", m.id));
    }
    if let Some(re) = m.re {
        out.push_str(&format!(", re {re}"));
    }
    let from = &w.agents[m.from];
    match m.outcome {
        Outcome::Failed(Some(c)) => out.push_str(&format!(" · #{} failed", c + 1)),
        Outcome::Failed(None) => out.push_str(" · failed"),
        Outcome::Late(c) if w.cards[c].column == Column::Withdrawn => out.push_str(&format!(
            " · late: #{} was withdrawn, it closes nothing",
            c + 1
        )),
        Outcome::Late(c) => out.push_str(&format!(
            " · late: #{} is {}'s now, it closes nothing",
            c + 1,
            w.cards[c]
                .owner
                .map_or("nobody", |o| w.agents[o].name.as_str())
        )),
        Outcome::Ok => {
            if m.kind == MsgKind::Done
                && from.role == Role::Worker
                && let Some(spend) = from.task.last
            {
                out.push_str(&format!(" · {}", dollars(spend.cost)));
            }
        }
    }
    if let Some(c) = m.card {
        out.push_str(&format!(" · #{}, try {}", c + 1, w.cards[c].attempt));
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
    let tests = tests_note(&w.projects[a.project].runners);
    let text = match a.role {
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
                     can run at once. Before a second worker, put your plan before the \
                     person with the plan tool (the cards you mean to assign or post, \
                     each with a tier and the paths it owns, and what you expect them to \
                     cost) and end your turn: they read it and their yes opens hiring; \
                     a first worker needs no plan. Pick a tier, not a model: cheap for small, \
                     well-specified mechanical work (renames, tests of known behaviour, \
                     docs), standard (the default) for ordinary fixes and features, \
                     strong for design that cuts across the code, hard debugging and \
                     review. Give each worker its part with the send tool (type \
                     assign: what to do, which files, when it is done), and name the \
                     paths the card owns (paths: files, directories or patterns \
                     relative to the repository); give different workers different \
                     paths, since no two open cards may own the same one: the person \
                     sees a worker's writes outside its card's paths and the worker is \
                     told of them. A file most tasks touch (a changelog, a version, a \
                     shared list) belongs to one worker at a time: give it to one card \
                     and tell the others to leave it out and to say in their done what \
                     it should get, since two changes to the same lines cannot both be \
                     applied; a file every card must touch ends with the word shared \
                     (CHANGELOG.md shared). Work that any worker could \
                     pick up may go on the board instead (post); idle workers take it \
                     themselves. A card that needs another's work first names it (after: \
                     the card numbers, on an assign or a post, and in a plan by place): \
                     it is not offered until they are done, and an assign with it waits \
                     on the card and goes then, so assign the whole order at once; one \
                     that waits on a failed card waits until that card is handed again \
                     and done. A card nobody is to do any more (its plan changed, \
                     another card did it) you withdraw with the withdraw tool and a line \
                     why; whoever has it stops, and the cards waiting on it never begin. \
                     A worker's done shows the person its change (its \
                     worktree's diff); they apply it to their checkout or reject it, and \
                     only an applied change is in the checkout you read. A card is \
                     done once its change is in; one sent back is at work again. A \
                     done headed failed made no change: hand that card again (assign \
                     with card: its number), to the same worker or another, with what \
                     to do differently; so for a card whose worker stopped. A done \
                     headed late came from a worker the card was taken from: it closes \
                     nothing. A card has three tries and its change three rounds; past \
                     them the person decides whether it gets another, and you hear \
                     their answer. A worker \
                     starts at the checkout's HEAD; when the person wants work on another \
                     branch, a tag, a commit or a pull request, hire with from (#N for a \
                     pull request) and that worker's worktree starts there and stays. \
                     Workers answer with done, blocked or ask; their \
                     messages start your next turn.{}",
                    lonca_checks(w, a.project)
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
                abilities(Role::Leader, confine)
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
                abilities(Role::Solo, confine)
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
                abilities(Role::Solo, confine)
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
                abilities(Role::Helper, confine)
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
                abilities(Role::Helper, confine)
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
                 A card that owns paths is done inside them: a file you change outside \
                 them is noted on the change the person reads, and you are told; say in \
                 your done what else needs changing, or ask {leader} to widen the card. \
                 When the task cannot be done as asked, say so: send the done \
                 with outcome failed and why, and no change is shown; never hand in \
                 half a change as done. If you are stuck, send blocked \
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
                abilities(Role::Worker, confine)
            )
        }
    };
    text + &tests
}

/// What an agent is told of the tests its project's files call for
/// (record 136): which run at once, and to run them as one program so a
/// worker's run shows on its change. Nothing when there are none (cargo's
/// are said in [`abilities`]).
pub(super) fn tests_note(tests: &Runners) -> String {
    if !tests.any() {
        return String::new();
    }
    format!(
        "\n\nThis repository's tests run at once, through the run tool or your shell: {} \
         (python as well as python3; for npm, pnpm or yarn, words for the test script go \
         after --). Run them as one program with its arguments, with no pipe (| tail) \
         and no redirection: anything more waits for the person's yes. Prefer the run \
         tool: it tidies the output, and a worker's run through it shows on the change \
         the person reads.",
        tests.lines().join(", ")
    )
}

/// The file Claude Code's own loader reads in a directory it is given
/// (record 110).
pub(super) const NOTES_FILE: &str = "CLAUDE.md";

/// The file of a repository's notes Claude Code does not read by itself.
const AGENTS_FILE: &str = "AGENTS.md";

/// What is in the file at `path`, whole; none when it is not there, not a
/// plain file (a pipe, a device) or empty.
fn text_of(path: &Path) -> Option<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    // Opened without waiting (a pipe would wait for a writer), and looked
    // at as opened, so what is read is what was looked at.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    file.metadata().ok().filter(|m| m.is_file())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// The repository's notes for coding agents that Claude Code's loader
/// will not find in the agent's directory `cwd`: the checkout's
/// `CLAUDE.md` (git ignores it, or a worker started where it is not),
/// else its `AGENTS.md`. Only a file inside the checkout: a link out of
/// it is the repository asking for a file of the person's.
fn repo_notes(cwd: &Path, checkout: &Path) -> Option<String> {
    let found = [cwd.join(NOTES_FILE), cwd.join(".claude").join(NOTES_FILE)];
    if found.iter().any(|f| text_of(f).is_some()) {
        return None;
    }
    let top = checkout.canonicalize().ok()?;
    [NOTES_FILE, AGENTS_FILE].iter().find_map(|f| {
        let file = checkout.join(f).canonicalize().ok()?;
        let text = text_of(&file).filter(|_| file.starts_with(&top))?;
        Some(format!(
            "The repository's notes for coding agents, from its {f}:\n\n{text}"
        ))
    })
}

/// What goes in an agent's notes directory (record 110): the person's own
/// `CLAUDE.md` (Claude Code reads that itself only without `--restricted`,
/// so not under open), their instructions for imece, then what
/// [`repo_notes`] finds. None when none of them has anything to say.
pub(super) fn notes_text(
    own: Option<&Path>,
    instructions: Option<&Path>,
    cwd: &Path,
    checkout: &Path,
    confine: Confine,
) -> Option<String> {
    let mine = |path: &Path| {
        Some(format!(
            "The person's own instructions for every project, from {}:\n\n{}",
            path.display(),
            text_of(path)?
        ))
    };
    let own = own.filter(|_| confine != Confine::Open).and_then(mine);
    let parts = [own, instructions.and_then(mine), repo_notes(cwd, checkout)];
    let parts: Vec<String> = parts.into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// What an agent with a scratch directory (record 105) is told of it.
pub(super) fn scratch_note(dir: &Path) -> String {
    format!(
        "\n\nYour scratch directory is {}: yours alone, outside the files you change. \
         Put notes, logs, downloads and whatever else is no part of your change there, \
         never among your files. It stays from session to session until the person makes \
         you forget your session.",
        dir.display()
    )
}

/// What a leader of a project on Lonca is told of a card's checks (record
/// 127); nothing off Lonca, where nothing runs them.
fn lonca_checks(w: &World, project: ProjectId) -> &'static str {
    match w.projects[project].lonca {
        Some(_) => {
            " This project is on Lonca: a card may also ask for checks the repository's \
             Lonca policy defines (checks: names from verification.checks, on an assign, \
             a post or a plan card); the person's yes to its change has Lonca's merge \
             queue run them too, and a failure goes back to its worker. Lonca's own \
             build and tests run anyway, so name only the extra ones the card needs."
        }
        None => "",
    }
}

/// What an agent in a Lonca workspace (record 119) is told of it, on top of
/// what its prompt says of a worktree. Lonca's own rules come by its
/// `SessionStart` hook: this says what they mean for a `done`.
pub(super) fn lonca_note(repo: &str) -> String {
    format!(
        "\n\nYour directory is a Lonca workspace of repository {repo}, not a git worktree: \
         where this text says worktree, it means this workspace. It holds the repository's \
         files and a `.git` of its own, for the tools that expect one: git reads it and a \
         commit stays here, but nothing reaches the repository through git, the run tool \
         runs no git here, and `git clean -x` would end the workspace. A \
         change reaches the repository as a proposal: lease what you edit, check with the \
         preview tool, submit with the propose tool. Propose before you say done: what is \
         not proposed nobody sees, and a done with changes no proposal carries is refused. A \
         proposal waits for the person's approval, and then \
         the sync tool brings trunk to you. When a proposal is denied and you can not put \
         it right, say blocked with what it said."
    )
}

/// The person's own instructions for every project, in imece's
/// configuration directory (`keys::config_file`).
pub(crate) const INSTRUCTIONS: &str = "instructions.md";

/// What an agent in `role` can do to the files and the world outside, as
/// its session is set up at `confine`. Under strict it gets file tools and
/// no shell; from standard on a worker has Claude Code's shell and the
/// web, a leader the web and reading commands.
pub(super) fn abilities(role: Role, confine: Confine) -> &'static str {
    match (role, confine.full()) {
        (Role::Leader, false) => {
            "You can read files but not change them; the run tool runs checks (cargo \
             check, clippy, test; git diff, log) in the checkout."
        }
        (Role::Leader, true) => {
            "You can read files and the web but not change files; the run tool runs \
             checks (cargo check, clippy, test; git diff, log) in the checkout."
        }
        (_, false) => {
            "You can read and change files here; the run tool runs the build, the \
             tests and git's reading commands, nothing else."
        }
        (_, true) => {
            "You can read and change files here, run commands in your shell and reach \
             the web. A command that is one program with its arguments (no pipes, \
             redirections, && or $, one line) and runs the build, the tests or git's \
             reading commands runs at once; most others wait for the person's yes, so \
             prefer the first kind. A refusal is the person's decision, not an error: \
             do not try the same another way. The run tool runs the build and the \
             tests with their output tidied."
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
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
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
    fn a_done_that_failed_or_came_late_and_a_card_handed_again_are_headed_so() {
        let mut w = team(2);
        let c = with_card(&mut w, 2);
        w.cards[c].attempt = 2;
        w.cards[c].paths = vec![Owned::new("src/a.rs")];
        w.agents[1].task.last = Some(Spend {
            cost: 8_400,
            ticks: 1,
            rounds: 1,
            unpriced: false,
        });
        let mail = |from, kind, outcome, card| Mail {
            from,
            kind,
            body: "x".into(),
            id: 12,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card,
            outcome,
        };
        let head = |w: &World, m: Mail| {
            let p = mail_prompt(w, &[m]);
            p.lines().nth(2).unwrap().to_string()
        };
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Ok, None)),
            "[done 12 · $0.84] from claude-1:"
        );
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Failed(Some(c)), None)),
            "[done 12 · #1 failed] from claude-1:",
            "no cost: nothing was settled"
        );
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Failed(None), None)),
            "[done 12 · failed] from claude-1:"
        );
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Late(c), None)),
            "[done 12 · late: #1 is claude-2's now, it closes nothing] from claude-1:"
        );
        w.cards[c].owner = None;
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Late(c), None)),
            "[done 12 · late: #1 is nobody's now, it closes nothing] from claude-1:"
        );
        // Withdrawn, it went to no other try (record 126).
        w.cards[c].column = Column::Withdrawn;
        assert_eq!(
            head(&w, mail(1, MsgKind::Done, Outcome::Late(c), None)),
            "[done 12 · late: #1 was withdrawn, it closes nothing] from claude-1:"
        );
        // An assign handing a card names it and its try, and its paths.
        let p = mail_prompt(&w, &[mail(0, MsgKind::Assign, Outcome::Ok, Some(c))]);
        assert!(
            p.ends_with(
                "[assign 12 · #1, try 2] from lead:\nx\n\nThis card owns src/a.rs: stay in \
                 them; what else needs changing goes in your done, or ask for the card to be \
                 widened."
            ),
            "{p}"
        );
        // The leader and the worker are told what failed and handed mean.
        let leader = system_prompt(&w, 0, 4, 0, Confine::Standard);
        assert!(
            leader.contains("hand that card again (assign with card: its number)"),
            "{leader}"
        );
        assert!(
            leader.contains("A card has three tries and its change three rounds; past them"),
            "{leader}"
        );
        assert!(
            leader.contains("A done headed late came from a worker"),
            "{leader}"
        );
        let worker = system_prompt(&w, 1, 4, 0, Confine::Standard);
        assert!(
            worker.contains("send the done with outcome failed and why"),
            "{worker}"
        );
    }

    #[test]
    fn a_file_of_notes_is_read_whole_or_not_at_all() {
        let dir = std::env::temp_dir().join(format!("imece-text-of-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(INSTRUCTIONS);
        assert_eq!(text_of(&file), None, "not there");
        assert_eq!(text_of(&dir), None, "a directory");
        std::fs::write(&file, " \n\t\n").unwrap();
        assert_eq!(text_of(&file), None, "nothing in it");
        std::fs::write(&file, "\nAnswer in Turkish.\n").unwrap();
        assert_eq!(text_of(&file).as_deref(), Some("Answer in Turkish."));
        // No cap: the last of a long file is there.
        let long = format!("{}the end", "x".repeat(1 << 20));
        std::fs::write(&file, &long).unwrap();
        assert_eq!(text_of(&file).as_deref(), Some(long.as_str()));
        // A byte that is not UTF-8 inside is replaced, and the rest kept.
        std::fs::write(&file, b"be \xff kind").unwrap();
        assert_eq!(text_of(&file).as_deref(), Some("be \u{FFFD} kind"));
        // Not a plain file: a device that never ends, a pipe nobody
        // writes to (never waited on).
        assert_eq!(text_of(Path::new("/dev/zero")), None);
        let fifo = dir.join("fifo");
        let c = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(text_of(&fifo), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_agent_s_notes_are_the_person_s_then_imece_s_then_the_repository_s() {
        let dir = std::env::temp_dir().join(format!("imece-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (tree, repo) = (dir.join("tree"), dir.join("repo"));
        std::fs::create_dir_all(tree.join(".claude")).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        let (own, mine) = (dir.join("own.md"), dir.join("mine.md"));
        let (o, m) = (Some(own.as_path()), Some(mine.as_path()));
        let text = |c| notes_text(o, m, &tree, &repo, c);
        assert_eq!(text(Confine::Standard), None, "nothing says anything");
        std::fs::write(repo.join("AGENTS.md"), "agents").unwrap();
        let agents = "The repository's notes for coding agents, from its AGENTS.md:\n\nagents";
        assert_eq!(text(Confine::Strict).as_deref(), Some(agents));
        std::fs::write(repo.join("CLAUDE.md"), "claude").unwrap();
        let claude = "The repository's notes for coding agents, from its CLAUDE.md:\n\nclaude";
        assert_eq!(
            text(Confine::Standard).as_deref(),
            Some(claude),
            "a CLAUDE.md first, as the tree lacks it"
        );
        std::fs::write(&own, "own").unwrap();
        std::fs::write(&mine, "mine").unwrap();
        let head = "The person's own instructions for every project, from";
        let first = format!("{head} {}:\n\nown", own.display());
        let second = format!("{head} {}:\n\nmine", mine.display());
        assert_eq!(
            text(Confine::Standard).unwrap(),
            format!("{first}\n\n{second}\n\n{claude}")
        );
        // Open: Claude Code reads the person's file itself.
        assert_eq!(
            text(Confine::Open).unwrap(),
            format!("{second}\n\n{claude}")
        );
        // What the tree holds its loader finds; said twice it costs twice.
        std::fs::write(tree.join("CLAUDE.md"), "the tree's").unwrap();
        assert_eq!(
            text(Confine::Standard).unwrap(),
            format!("{first}\n\n{second}")
        );
        std::fs::remove_file(tree.join("CLAUDE.md")).unwrap();
        std::fs::write(tree.join(".claude/CLAUDE.md"), "the tree's").unwrap();
        assert_eq!(
            text(Confine::Standard).unwrap(),
            format!("{first}\n\n{second}")
        );
        // An empty one is none, as Claude Code finds nothing in it.
        std::fs::write(tree.join(".claude/CLAUDE.md"), " \n").unwrap();
        assert!(text(Confine::Standard).unwrap().ends_with(claude));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_link_out_of_the_checkout_is_no_notes_of_its() {
        let dir = std::env::temp_dir().join(format!("imece-notes-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (tree, repo) = (dir.join("tree"), dir.join("repo"));
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        let secret = dir.join("secret");
        std::fs::write(&secret, "a key").unwrap();
        std::os::unix::fs::symlink(&secret, repo.join("AGENTS.md")).unwrap();
        assert_eq!(repo_notes(&tree, &repo), None);
        // One inside it is the repository's own way of keeping them.
        std::fs::remove_file(repo.join("AGENTS.md")).unwrap();
        std::fs::write(repo.join("docs/agents.md"), "inside").unwrap();
        std::os::unix::fs::symlink("docs/agents.md", repo.join("AGENTS.md")).unwrap();
        assert!(repo_notes(&tree, &repo).unwrap().ends_with("inside"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_starts_with_its_notes_where_claude_code_s_loader_finds_them() {
        let dir = std::env::temp_dir().join(format!("imece-notes-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("notes-repo");
        std::fs::write(repo.join("AGENTS.md"), "The repo says so.").unwrap();
        let (own, mine) = (dir.join("CLAUDE.md"), dir.join(INSTRUCTIONS));
        std::fs::write(&own, "I always say so.").unwrap();
        std::fs::write(&mine, "I say so here.").unwrap();
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at(&fake_claude(&dir)),
            repo.clone(),
            dir.join("worktrees"),
        );
        (live.own_notes, live.instructions) = (Some(own), Some(mine));
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(repo.join("args.txt")).unwrap();
        let notes = live.notes_of(&w, 0);
        let text = std::fs::read_to_string(notes.join(NOTES_FILE)).unwrap();
        for said in ["I always say so.", "I say so here.", "The repo says so."] {
            assert!(text.contains(said), "{text}");
            assert!(!args.contains(said), "{args}");
        }
        let notes = notes.canonicalize().unwrap();
        assert!(args.contains(&notes.display().to_string()), "{args}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_worker_on_lonca_hears_that_a_proposal_comes_before_a_done() {
        let note = lonca_note("kule");
        assert!(
            note.contains("Lonca workspace of repository kule"),
            "{note}"
        );
        assert!(note.contains("a `.git` of its own"), "{note}");
        assert!(note.contains("nothing reaches the repository"), "{note}");
        assert!(note.contains("`git clean -x`"), "{note}");
        assert!(note.contains("Propose before you say done"), "{note}");
        assert!(note.contains("say blocked"), "{note}");
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
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
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
    fn the_leader_hires_a_worker_with_the_tier_s_model() {
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
        assert_eq!((a.name.as_str(), a.role), ("claude-1", Role::Worker));
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
        // The default tier is Claude Code's own model, which has nothing to
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
    fn every_agent_is_told_the_tests_its_project_s_files_call_for_last() {
        let mut w = team(2);
        let helper = with_helper(&mut w, 1);
        for id in [0, 1, helper] {
            let p = system_prompt(&w, id, 4, 2, Confine::Standard);
            assert!(!p.contains("This repository's tests"), "{p}");
        }
        assert_eq!(tests_note(&crate::runners::Runners::default()), "");
        w.projects[0].runners = crate::runners::Runners {
            python: true,
            node: Some(crate::runners::Node::Yarn),
            go: false,
        };
        let note = "\n\nThis repository's tests run at once, through the run tool or your \
                    shell: python3 -m pytest …, python3 -m unittest …, pytest …, yarn test, \
                    yarn run test (python as well as python3; for npm, pnpm or yarn, words for \
                    the test script go after --). Run them as one program with its arguments, \
                    with no pipe (| tail) and no redirection: anything more waits for the \
                    person's yes. Prefer the run tool: it tidies the output, and a worker's \
                    run through it shows on the change the person reads.";
        assert_eq!(tests_note(&w.projects[0].runners), note);
        for id in [0, 1, helper] {
            let p = system_prompt(&w, id, 4, 2, Confine::Standard);
            assert!(p.ends_with(note), "{p}");
        }
    }

    #[test]
    fn a_solo_and_its_helper_are_told_they_work_alone_with_the_person() {
        let mut w = team(1);
        let team_helper = with_helper(&mut w, 1);
        w.open_project(None, "notes".into(), "/n".into(), Role::Solo);
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
    #[test]
    fn an_assign_s_paths_follow_its_body_and_the_roles_are_told_of_them() {
        let w = team(1);
        let mail = Mail {
            from: 0,
            kind: MsgKind::Assign,
            body: "Fix it\nin src".into(),
            id: 2,
            re: None,
            paths: vec![Owned::new("src/a.rs"), Owned::shared("docs")],
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
        };
        let prompt = mail_prompt(&w, &[mail]);
        assert!(
            prompt.ends_with(
                "[assign 2] from lead:\nFix it\nin src\n\nThis card owns src/a.rs, docs \
                 (shared): stay in them; what else needs changing goes in your done, or ask \
                 for the card to be widened."
            ),
            "{prompt}"
        );
        let leader = system_prompt(&w, 0, 4, 0, Confine::Standard);
        assert!(
            leader.contains("name the paths the card owns (paths:"),
            "{leader}"
        );
        assert!(leader.contains("(CHANGELOG.md shared)"), "{leader}");
        let worker = system_prompt(&w, 1, 4, 2, Confine::Standard);
        assert!(
            worker.contains("A card that owns paths is done inside them"),
            "{worker}"
        );
        assert!(worker.contains("ask lead to widen the card"), "{worker}");
    }

    #[test]
    fn a_leader_on_lonca_hears_that_a_card_may_ask_for_checks() {
        let mut w = team(1);
        let off = system_prompt(&w, 0, 4, 0, Confine::Standard);
        assert!(!off.contains("Lonca"), "{off}");
        w.on_lonca(0, Some("demo".into()));
        let on = system_prompt(&w, 0, 4, 0, Confine::Standard);
        assert!(
            on.contains(
                "messages start your next turn. This project is on Lonca: a card may also ask \
                 for checks the repository's Lonca policy defines (checks: names from \
                 verification.checks, on an assign, a post or a plan card); the person's yes \
                 to its change has Lonca's merge queue run them too, and a failure goes back \
                 to its worker. Lonca's own build and tests run anyway, so name only the extra \
                 ones the card needs. You never change files yourself"
            ),
            "{on}"
        );
    }
}
