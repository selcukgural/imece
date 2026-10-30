//! The tools agents call on the bus, and the two that act for them:
//! `run` (a command from an allow-list) and the approval of what a
//! session asks.

use super::*;

/// Cargo commands nobody runs through imece: they change what the project
/// depends on or reach outside it, which is the person's call.
pub(super) const CARGO_DENIED: &[&str] = &[
    "add",
    "install",
    "login",
    "logout",
    "owner",
    "publish",
    "remove",
    "rm",
    "uninstall",
    "yank",
];

/// Cargo commands a leader may not run either: they change files.
pub(super) const CARGO_WRITES: &[&str] = &["clean", "fix", "fmt", "init", "new"];

/// Git commands that only read.
pub(super) const GIT_READ: &[&str] = &[
    "blame",
    "diff",
    "grep",
    "log",
    "ls-files",
    "rev-parse",
    "show",
    "status",
];

/// git options that run a program (a pager: `-O`, `--open-files-in-pager`)
/// or write a file (`--output`), by their start: `-Ovim` and
/// `--output=x` too. `-O<orderfile>` of diff and log goes with them.
pub(super) const GIT_DENIED_OPTIONS: &[&str] = &["-O", "--open-files-in-pager", "--output"];

/// How long a tool call waits for imece's answer: a run is answered when
/// the program ends, not by the screen's loop, so it waits for the run's
/// deadline and a little more; an ask waits for the person; the rest are
/// answered within a step.
pub(super) fn call_wait(tool: &str, run_timeout: Duration) -> Duration {
    match tool {
        "run" => run_timeout + CALL_WAIT,
        APPROVE_TOOL => APPROVE_WAIT,
        _ => CALL_WAIT,
    }
}

/// The tool a Claude Code session asks through before a tool call that
/// needs a yes. Claude Code calls it itself and hides it from the model.
pub(super) fn approve_tool() -> Tool {
    Tool {
        name: APPROVE_TOOL.into(),
        description: "Asks the person whether a tool call may run.".into(),
        input: json!({
            "type": "object",
            "properties": {
                "tool_name": { "type": "string" },
                "input": { "type": "object" },
                "tool_use_id": { "type": "string" },
            },
            "required": ["tool_name", "input"],
        }),
    }
}

/// Whether what a session asks is routine, allowed without the person:
/// one line that is one program with its arguments (nothing a shell would
/// read otherwise, no second line), which the run tool would run for
/// `role`: the build, the tests, git's reading commands, the programs
/// the person named.
pub(super) fn routine(role: Role, what: &Ask, programs: &[String]) -> bool {
    match what {
        Ask::Command { command, .. } => {
            let line = command.trim();
            // A shell runs each line as a command of its own.
            !line.contains(['\n', '\r'])
                && run::split(line).is_ok_and(|argv| allowed(role, &argv, programs).is_ok())
        }
        Ask::Other { .. } => false,
    }
}

/// imece's tools for an agent of `role`: workers alone take cards, the
/// leader hires workers and a worker hires helpers (from `vendors`);
/// helpers and researchers hire nobody.
pub(super) fn tools_of(
    role: Role,
    vendors: &[&str],
    programs: &[String],
    timeout: Duration,
) -> Vec<Tool> {
    let mut tools = vec![
        send_tool(),
        post_tool(),
        board_tool(),
        run_tool(role, programs, timeout),
    ];
    match role {
        Role::Worker => {
            tools.push(claim_tool());
            tools.push(hire_tool(vendors, HIRE_HELPER));
        }
        Role::Leader => tools.push(hire_tool(vendors, HIRE_WORKER)),
        Role::Helper | Role::Researcher => {}
    }
    tools
}

/// How one run ended, for the trace.
pub(super) fn run_ended(o: &run::Outcome) -> String {
    let how = if o.timed_out {
        "killed at the deadline".to_string()
    } else {
        match o.code {
            Some(c) => format!("exit {c}"),
            None => "killed".to_string(),
        }
    };
    format!("{how}, {:.1} s, {} bytes", o.took.as_secs_f64(), o.bytes)
}

/// What the agent reads back from a run. A run killed at the deadline
/// says the deadline, not the time it took: under load that is a second
/// or more later, and the deadline is what the agent can plan with.
pub(super) fn run_answer(o: &run::Outcome, deadline: Duration) -> String {
    let first = if o.timed_out {
        format!(
            "killed after {} s: it did not finish in time",
            deadline.as_secs()
        )
    } else {
        match o.code {
            Some(0) => "exit code 0".to_string(),
            Some(c) => format!("exit code {c} (failed)"),
            None => "killed by a signal".to_string(),
        }
    };
    if o.output.is_empty() {
        format!("{first}, no output")
    } else {
        format!("{first}\n{}", o.output)
    }
}

/// Whether an agent in `role` may run `argv`: cargo without the commands
/// that change dependencies (a leader: without the ones that change
/// files), git's reading commands, and for workers and helpers the
/// programs the person named. Nothing else.
pub(super) fn allowed(role: Role, argv: &[String], programs: &[String]) -> Result<(), String> {
    let program = Path::new(&argv[0])
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    if argv[0] != program {
        return Err(format!(
            "{}: name the program alone, on PATH; not a path",
            argv[0]
        ));
    }
    // The first word that is not an option (or a toolchain, `+nightly`).
    let sub = argv[1..]
        .iter()
        .find(|a| !a.starts_with('-') && !a.starts_with('+'))
        .map_or("", String::as_str);
    let leader = role == Role::Leader;
    match program.as_str() {
        "cargo" => {
            if CARGO_DENIED.contains(&sub) {
                return Err(format!(
                    "cargo {sub} is for the person: dependencies and what is outside the \
                     project are not changed through imece"
                ));
            }
            if leader && CARGO_WRITES.contains(&sub) && !argv.iter().any(|a| a == "--check") {
                return Err(format!(
                    "cargo {sub} changes files, and a leader changes none; a worker can"
                ));
            }
            Ok(())
        }
        "git" => {
            // git's options before the command (`-c`, `-C`, `--git-dir`)
            // change what it runs and where; a pager or an output file
            // runs a program or writes outside the directory.
            if argv[1..].first().is_some_and(|a| a.starts_with('-')) {
                return Err(
                    "git takes no options before its command here; name the command first"
                        .to_string(),
                );
            }
            if let Some(a) = argv
                .iter()
                .find(|a| GIT_DENIED_OPTIONS.iter().any(|o| a.starts_with(o)))
            {
                return Err(format!(
                    "git {sub} {a}: a pager or an output file is not run through imece"
                ));
            }
            if GIT_READ.contains(&sub) {
                Ok(())
            } else {
                Err(format!(
                    "git {sub} is not run through imece; git only reads here: {}",
                    GIT_READ.join(", ")
                ))
            }
        }
        _ if !leader && programs.contains(&program) => Ok(()),
        _ => {
            let mut can = vec!["cargo".to_string(), "git".to_string()];
            if !leader {
                can.extend(programs.iter().cloned());
            }
            Err(format!(
                "{program} is not a program imece runs; these are: {} (the person adds \
                 one with --run NAME)",
                can.join(", ")
            ))
        }
    }
}

pub(super) fn run_tool(role: Role, programs: &[String], timeout: Duration) -> Tool {
    let mut can = vec![
        "cargo (not add, remove, install or publish)".to_string(),
        format!("git ({} only)", GIT_READ.join(", ")),
    ];
    if role == Role::Leader {
        can[0] = "cargo (check, clippy, test, build, fmt --check; not the commands that \
                  change files)"
            .to_string();
    } else {
        can.extend(programs.iter().cloned());
    }
    Tool {
        name: "run".into(),
        description: format!(
            "Runs one program in your directory and gives back its exit code and \
             output (stdout and stderr together; lines of passing tests are left \
             out, very long output is cut in the middle). No shell: one program and \
             its arguments, no pipes, redirections or variables; quote an argument \
             with spaces. Programs: {}. A run is killed after {} s. Use it to check \
             your work (build, lint, test) before you say it is done.",
            can.join("; "),
            timeout.as_secs()
        ),
        input: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The program and its arguments, as one line: \
                                    cargo test --workspace",
                },
            },
            "required": ["command"],
            "additionalProperties": false,
        }),
    }
}

/// The hire tool as the leader sees it.
pub(super) const HIRE_WORKER: &str = "Adds a worker to your team in imece: a coding agent in its own git \
                           worktree of the repository. The answer names it; then give it work \
                           with send (type assign). Hire for work that can run at once, not in \
                           advance: one worker takes several tasks in turn.";
/// The hire tool as a worker sees it.
const HIRE_HELPER: &str = "Adds a helper of your own in imece: a coding agent in your worktree, \
                           changing the same files as you. The answer names it; then give it a \
                           part of your task with send (type assign): what to do, which files, \
                           when it is done. It sends you done and leaves. Hire for a part that \
                           can run beside your own work, in files you are not changing.";

fn hire_tool(vendors: &[&str], description: &str) -> Tool {
    let tiers: Vec<&str> = Tier::ALL.iter().map(|t| t.wire()).collect();
    Tool {
        name: "hire".into(),
        description: description.into(),
        input: json!({
            "type": "object",
            "properties": {
                "tier": {
                    "type": "string",
                    "enum": tiers,
                    "description": "cheap: small, well-specified mechanical work (renames, \
                                    tests of known behaviour, docs). standard (default): \
                                    ordinary fixes and features. strong: design that cuts \
                                    across the code, hard debugging, review.",
                },
                "vendor": {
                    "type": "string",
                    "enum": vendors,
                    "description": "Which coding agent (default: your own).",
                },
            },
            "additionalProperties": false,
        }),
    }
}

pub(super) fn post_tool() -> Tool {
    Tool {
        name: "post".into(),
        description: "Puts a card on imece's board for any worker to take. Use it for \
                      work nobody has yet; for work one teammate should do, send assign."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "One line; the screen shows its first 20 characters \
                                    next to the worker, so start with what tells this \
                                    card from the others (an issue number, a file, a \
                                    symbol), not with a verb every card shares.",
                    "maxLength": MAX_TITLE,
                },
                "body": {
                    "type": "string",
                    "description": "The task: what to do, which files, when it is done.",
                    "maxLength": MAX_BODY,
                },
            },
            "required": ["title", "body"],
            "additionalProperties": false,
        }),
    }
}

pub(super) fn board_tool() -> Tool {
    Tool {
        name: "board".into(),
        description: "Lists the cards on imece's board: open ones, and who took the others.".into(),
        input: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    }
}

pub(super) fn claim_tool() -> Tool {
    Tool {
        name: "claim".into(),
        description: "Takes an open card from imece's board; the answer is its task. \
                      One card at a time."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "card": { "type": "integer", "minimum": 1, "description": "The card's number, without #." },
            },
            "required": ["card"],
            "additionalProperties": false,
        }),
    }
}

/// imece's `send` tool, as agents see it.
pub(super) fn send_tool() -> Tool {
    let kinds: Vec<&str> = MsgKind::ALL.iter().map(|k| k.wire()).collect();
    Tool {
        name: "send".into(),
        description: "Sends a message to a teammate in imece, or to the person (to: you). \
                      To a teammate it arrives as the start of their next turn: assign, \
                      ask, answer, handoff, done, blocked and cancel start that turn as \
                      soon as they are free; inform, progress and notice wait until \
                      something else does. To the person, an ask waits on their screen \
                      until they answer (which starts your next turn); the rest only \
                      informs them."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "The teammate's name, or you for the person." },
                "type": { "type": "string", "enum": kinds },
                "body": {
                    "type": "string",
                    "description": "The message. For assign: the first line becomes \
                                    the card's title (the screen shows its first 20 \
                                    characters next to the worker, so start with what \
                                    tells it from the other cards: an issue number, a \
                                    file, a symbol, not a verb every task shares); then \
                                    the task, the files it touches and when it is done.",
                    "maxLength": MAX_BODY,
                },
            },
            "required": ["to", "type", "body"],
            "additionalProperties": false,
        }),
    }
}

impl Live {
    /// Agent `from` asked to run a command: checked here, run on a thread
    /// of its own in the agent's directory, answered when it ends. The
    /// trace gets the command and how it ended. A worker's or helper's run
    /// is kept with the tree its worktree made before and after it (see
    /// [`Change::runs`]); one the tree moved under counts for none.
    pub(super) fn run(
        &mut self,
        w: &mut World,
        from: AgentId,
        args: &Value,
        reply: Sender<Answer>,
    ) {
        let line = args["command"].as_str().unwrap_or_default().trim();
        let checked = (|| {
            if line.is_empty() {
                return Err("command: what to run, a program and its arguments".to_string());
            }
            if line.chars().count() > MAX_COMMAND {
                return Err(format!(
                    "the command is longer than {MAX_COMMAND} characters"
                ));
            }
            let argv = run::split(line)?;
            allowed(w.agents[from].role, &argv, &self.run_programs)?;
            let cwd = self.cwds[from]
                .clone()
                .ok_or_else(|| "the session has no directory yet".to_string())?;
            Ok((argv, cwd))
        })();
        let (argv, cwd) = match checked {
            Ok(c) => c,
            Err(e) => {
                w.agents[from].trace(w.tick, format!("run refused: {e}"));
                let _ = reply.send(Err(e));
                return;
            }
        };
        let line = line.to_string();
        w.agents[from].trace(w.tick, format!("run: {line}"));
        let (tx, generation) = (self.tx.clone(), self.generation[from]);
        let (running, timeout) = (self.running[from].clone(), self.run_timeout);
        let stamped = matches!(w.agents[from].role, Role::Worker | Role::Helper);
        let ran_tx = self.ran_tx.clone();
        thread::spawn(move || {
            let before = if stamped {
                worktree::tree(&cwd).ok()
            } else {
                None
            };
            let mut pgid = None;
            let outcome = run::run(&argv, &cwd, timeout, MAX_RUN_OUTPUT, |p| {
                running.add(p);
                pgid = Some(p);
            });
            if let Some(p) = pgid {
                running.remove(p);
            }
            if let (Some(tree), Ok(o)) = (before, &outcome)
                && worktree::tree(&cwd).is_ok_and(|after| after == tree)
            {
                let ran = Ran {
                    line: line.clone(),
                    code: o.code,
                    timed_out: o.timed_out,
                };
                let _ = ran_tx.send((tree, ran));
            }
            let (note, answer) = match outcome {
                Ok(o) => (
                    format!("run: {line}: {}", run_ended(&o)),
                    Ok(run_answer(&o, timeout)),
                ),
                Err(e) => (
                    format!("run: {line}: could not start: {e}"),
                    Err(format!("could not start {}: {e}", argv[0])),
                ),
            };
            let _ = tx.send((from, generation, AgentEvent::Note(note)));
            let _ = reply.send(answer);
        });
    }

    /// Agent `from`'s Claude Code session asks before a tool call (its
    /// permission prompt, through imece's socket): a routine command is
    /// allowed at once, anything else waits in the person's inbox and is
    /// answered when they decide. The trace says which.
    pub(super) fn approve(
        &mut self,
        w: &mut World,
        from: AgentId,
        args: &Value,
        reply: Sender<Answer>,
    ) {
        let reply = Reply::Socket {
            input: args["input"].clone(),
            to: reply,
        };
        self.ask(w, from, claude::ask(args), reply);
    }

    /// Agent `from`'s session asks before it does `what`: a routine
    /// command is allowed at once, anything else waits in the person's
    /// inbox and is answered when they decide. The trace says which.
    pub(super) fn ask(&mut self, w: &mut World, from: AgentId, what: Ask, reply: Reply) {
        if routine(w.agents[from].role, &what, &self.run_programs) {
            w.agents[from].trace(w.tick, format!("allowed on its own: {}", what.line()));
            self.reply(from, reply, true);
            return;
        }
        w.agents[from].trace(w.tick, format!("asks the person: {}", what.line()));
        w.asks_you(from, &what.line());
        self.pending[from].push(Pending { what, reply });
    }

    /// Gives agent `id`'s session the answer to what it asked.
    pub(super) fn reply(&mut self, id: AgentId, reply: Reply, allow: bool) {
        match reply {
            Reply::Socket { input, to } => {
                let _ = to.send(Ok(claude::decision(allow, &input)));
            }
            Reply::Session(request) => {
                if let Slot::Running(s) = &mut self.slots[id] {
                    let _ = s.decide(&request, allow);
                }
            }
        }
    }

    /// What agent `id`'s session asked with `request` waits no more (its
    /// vendor settled it): its item closes without a decision.
    pub(super) fn withdrawn(&mut self, w: &mut World, id: AgentId, request: &str) {
        let Some(nth) = self.pending[id]
            .iter()
            .position(|p| matches!(&p.reply, Reply::Session(r) if r == request))
        else {
            return;
        };
        let p = self.pending[id].remove(nth);
        w.agents[id].trace(w.tick, format!("no longer asks: {}", p.what.line()));
        w.withdrew(id, nth);
    }

    /// The person allowed or refused what inbox item `item` asks: the
    /// session waiting on it goes on. An item that is not an open
    /// approval is left alone.
    pub fn decide(&mut self, w: &mut World, item: usize, allow: bool) {
        let Some(id) = w
            .inbox
            .get(item)
            .filter(|i| i.open && i.kind == InboxKind::Approval)
            .map(|i| i.agent)
        else {
            return;
        };
        // An agent's open approvals and its waiting calls are in the same
        // order: an ask adds one of each, a decision or the session's end
        // takes them away together.
        let nth = w.inbox[..item]
            .iter()
            .filter(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
            .count();
        w.decided(item, allow);
        if nth < self.pending[id].len() {
            let p = self.pending[id].remove(nth);
            let how = if allow { "allowed" } else { "refused" };
            w.agents[id].trace(w.tick, format!("the person {how}: {}", p.what.line()));
            self.reply(id, p.reply, allow);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::live::testing::*;

    #[test]
    fn what_may_run_is_cargo_git_s_reads_and_the_person_s_programs_nothing_else() {
        let extra = vec!["npm".to_string(), "make".to_string()];
        for role in [Role::Leader, Role::Worker, Role::Helper] {
            let ok = |line: &str| allowed(role, &argv(line), &extra).unwrap();
            let no = |line: &str| allowed(role, &argv(line), &extra).unwrap_err();
            ok("cargo test --workspace");
            ok("cargo +nightly clippy -- -D warnings");
            ok("cargo fmt --all --check");
            ok("cargo --locked check");
            ok("git status --short");
            ok("git diff HEAD~1");
            ok("git log --oneline -5");
            for denied in [
                "add serde",
                "remove serde",
                "rm serde",
                "install x",
                "publish",
                "yank",
                "login",
                "logout",
                "owner --add x",
                "uninstall x",
            ] {
                let e = no(&format!("cargo {denied}"));
                assert!(e.contains("for the person"), "{e}");
            }
            for denied in [
                "push",
                "commit -m x",
                "checkout main",
                "reset --hard",
                "branch x",
                "stash",
                "fetch",
                "pull",
                "clean -fd",
            ] {
                let e = no(&format!("git {denied}"));
                assert!(e.contains("only reads"), "{e}");
            }
            assert!(no("git").contains("only reads"));
            assert!(no("/usr/bin/git status").contains("not a path"));
            assert!(no("./cargo test").contains("not a path"));
            for other in [
                "sh -c ls",
                "bash",
                "python3 x.py",
                "rm -rf x",
                "curl http://x",
                "ls",
            ] {
                let e = no(other);
                assert!(
                    e.contains("not a program imece runs") && e.contains("--run"),
                    "{e}"
                );
            }
        }
        // Leaders change nothing: cargo's writing commands and the person's
        // programs are for workers and helpers.
        for writes in [
            "fmt",
            "fmt --all",
            "+nightly fmt",
            "--locked +nightly fmt",
            "fix",
            "clean",
            "new x",
            "init",
        ] {
            let e = allowed(Role::Leader, &argv(&format!("cargo {writes}")), &extra).unwrap_err();
            assert!(e.contains("changes files"), "{e}");
            allowed(Role::Worker, &argv(&format!("cargo {writes}")), &extra).unwrap();
            allowed(Role::Helper, &argv(&format!("cargo {writes}")), &[]).unwrap();
        }
        for program in ["npm test", "make check"] {
            let e = allowed(Role::Leader, &argv(program), &extra).unwrap_err();
            assert!(e.contains("these are: cargo, git ("), "{e}");
            allowed(Role::Worker, &argv(program), &extra).unwrap();
            let e = allowed(Role::Worker, &argv(program), &[]).unwrap_err();
            assert!(e.contains("these are: cargo, git ("), "{e}");
        }
        let e = allowed(Role::Worker, &argv("yarn"), &extra).unwrap_err();
        assert!(e.contains("cargo, git, npm, make"), "{e}");
    }

    #[test]
    fn a_run_s_call_waits_past_its_deadline_and_the_rest_a_short_while() {
        let t = Duration::from_secs(600);
        assert_eq!(call_wait("run", t), Duration::from_secs(630));
        for tool in ["send", "post", "board", "claim", "hire"] {
            assert_eq!(call_wait(tool, t), CALL_WAIT, "{tool}");
        }
        // An ask waits for the person, who may be away for a day.
        assert_eq!(call_wait("approve", t), Duration::from_secs(86_400));
    }

    #[test]
    fn git_through_imece_takes_no_options_before_its_command_no_pager_and_no_output_file() {
        let argv = |s: &str| run::split(s).unwrap();
        for before in [
            "git -c diff.external=x diff",
            "git -cdiff.external=x diff",
            "git -C /elsewhere log",
            "git --git-dir=/x status",
            "git --no-pager log",
        ] {
            let e = allowed(Role::Worker, &argv(before), &[]).unwrap_err();
            assert!(e.contains("no options before its command"), "{before}: {e}");
        }
        for runs in [
            "git grep -O x",
            "git grep -Ovim x",
            "git grep --open-files-in-pager=vim x",
            "git diff --output=/tmp/x",
            "git log --output /tmp/x",
            "git show HEAD --output=x",
        ] {
            let e = allowed(Role::Leader, &argv(runs), &[]).unwrap_err();
            assert!(
                e.contains("a pager or an output file is not run through imece"),
                "{runs}: {e}"
            );
        }
        let e = allowed(Role::Worker, &argv("git diff -Oorder"), &[]).unwrap_err();
        assert!(e.starts_with("git diff -Oorder:"), "{e}");
        for reads in [
            "git log --oneline -5",
            "git diff --stat HEAD~1",
            "git grep -n foo",
            "git show HEAD:src/x.rs",
        ] {
            allowed(Role::Leader, &argv(reads), &[]).unwrap();
        }
    }

    fn bash(command: &str) -> Ask {
        Ask::Command {
            tool: "Bash".into(),
            command: command.into(),
        }
    }

    #[test]
    fn a_routine_command_is_one_line_the_run_tool_would_run() {
        let extra = vec!["npm".to_string()];
        for yes in [
            "cargo test",
            "  cargo build --release\n",
            "git log --oneline -5",
            "npm test",
        ] {
            assert!(routine(Role::Worker, &bash(yes), &extra), "{yes:?}");
        }
        for no in [
            "cargo test | tail",
            "cargo test 2>&1",
            "cargo test && rm -rf x",
            "cargo test\nrm -rf x",
            "cargo test\rrm -rf x",
            "echo $HOME",
            "cargo publish",
            "git push",
            "git -c x=y log",
            "ls",
            "",
        ] {
            assert!(!routine(Role::Worker, &bash(no), &extra), "{no:?}");
        }
        // The run tool's rules, role and programs included.
        assert!(!routine(Role::Worker, &bash("npm test"), &[]));
        assert!(routine(Role::Worker, &bash("cargo fmt"), &extra));
        assert!(!routine(Role::Leader, &bash("cargo fmt"), &extra));
        // Only a command is ever routine.
        let other = Ask::Other {
            tool: "Bash".into(),
            detail: "cargo test".into(),
        };
        assert!(!routine(Role::Worker, &other, &extra));
    }

    /// Agent `from`'s session asks to run `command`, through the bus;
    /// where its answer comes.
    fn ask_to_run(live: &Live, from: AgentId, command: &str) -> Receiver<Answer> {
        let (tx, rx) = mpsc::channel();
        let args = json!({
            "tool_name": "Bash",
            "input": { "command": command, "description": "d" },
            "tool_use_id": "toolu_1",
        });
        live.calls_tx
            .send((from, APPROVE_TOOL.into(), args, tx))
            .unwrap();
        rx
    }

    /// What the session reads back from an ask.
    fn verdict(rx: &Receiver<Answer>) -> Value {
        serde_json::from_str(&rx.try_recv().unwrap().unwrap()).unwrap()
    }

    #[test]
    fn a_routine_ask_is_allowed_at_once_and_the_rest_wait_for_the_person_in_order() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let routine = ask_to_run(&live, 1, "cargo test");
        live.step(&mut w);
        let yes = verdict(&routine);
        assert_eq!(yes["behavior"], "allow");
        assert_eq!(yes["updatedInput"]["command"], "cargo test");
        assert!(w.open_inbox().is_empty());
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" allowed on its own: Bash: cargo test"))
        );

        // An older question of the worker and the leader's ask come first:
        // neither counts among the worker's asks.
        w.asked_you(1, "Which API?");
        let lead = ask_to_run(&live, 0, "make lead");
        let asks: Vec<_> = ["make a", "make b", "make c"]
            .iter()
            .map(|c| ask_to_run(&live, 1, c))
            .collect();
        live.step(&mut w);
        assert!(lead.try_recv().is_err() && asks.iter().all(|a| a.try_recv().is_err()));
        let open = w.open_inbox();
        let shown: Vec<_> = open
            .iter()
            .map(|&i| {
                (
                    w.inbox[i].kind,
                    w.inbox[i].agent,
                    w.inbox[i].detail.as_str(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (InboxKind::Question, 1, "Which API?"),
                (InboxKind::Approval, 0, "Bash: make lead"),
                (InboxKind::Approval, 1, "Bash: make a"),
                (InboxKind::Approval, 1, "Bash: make b"),
                (InboxKind::Approval, 1, "Bash: make c"),
            ]
        );
        assert!(w.has_approval(0) && w.has_approval(1));
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" asks the person: Bash: make b"))
        );
        // Another kind of item is not an approval, whoever's it is.
        live.decide(&mut w, open[0], true);
        assert!(w.inbox[open[0]].open);
        assert!(asks.iter().all(|a| a.try_recv().is_err()));
        live.decide(&mut w, 999, true);

        // The middle one first: refused, and only that call hears it.
        live.decide(&mut w, open[3], false);
        let no = verdict(&asks[1]);
        assert_eq!(no["behavior"], "deny");
        assert_eq!(no["message"], claude::REFUSED);
        assert!(asks[0].try_recv().is_err() && asks[2].try_recv().is_err());
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: false } if what == "Bash: make b"
        ));
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" the person refused: Bash: make b"))
        );
        // Decided once: the closed item is left alone, and so are the
        // calls still waiting.
        let journal = w.journal.len();
        live.decide(&mut w, open[3], true);
        assert_eq!(w.journal.len(), journal);
        assert!(asks[0].try_recv().is_err() && asks[2].try_recv().is_err());
        // Then the first and the last, past the decided one.
        live.decide(&mut w, open[2], true);
        assert_eq!(verdict(&asks[0])["updatedInput"]["command"], "make a");
        live.decide(&mut w, open[4], true);
        assert_eq!(verdict(&asks[2])["updatedInput"]["command"], "make c");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: true } if what == "Bash: make c"
        ));
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" the person allowed: Bash: make c"))
        );
        assert!(!w.has_approval(1) && w.has_approval(0));
        assert!(live.pending[1].is_empty());
        // The leader's waits on.
        assert!(lead.try_recv().is_err());
        assert_eq!(live.pending[0].len(), 1);
    }

    #[test]
    fn an_approval_whose_session_ended_closes_and_its_call_hears_so() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let ask = ask_to_run(&live, 1, "make deploy");
        let lead = ask_to_run(&live, 0, "make lead");
        live.step(&mut w);
        live.end(&mut w, 1);
        assert_eq!(
            ask.try_recv().unwrap(),
            Err("the session ended".to_string())
        );
        assert!(!w.has_approval(1) && w.has_approval(0));
        assert!(live.pending[1].is_empty());
        assert!(lead.try_recv().is_err());
        // An item with no call behind it (none waits any more) is only
        // closed, and the journal still says what the person decided.
        w.asks_you(1, "Bash: make again");
        let item = w.inbox.len() - 1;
        live.decide(&mut w, item, false);
        assert!(!w.inbox[item].open);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Decided {
                agent: 1,
                allowed: false,
                ..
            }
        ));
    }

    /// The answers a session was given: request, and whether it runs.
    type Answers = Arc<Mutex<Vec<(String, bool)>>>;

    /// A session that keeps the answers it was given.
    struct Decides(Answers);

    impl imece_agent::Agent for Decides {
        fn prompt(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn command(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn terminate(&mut self) {}
        fn decide(&mut self, request: &str, allow: bool) -> io::Result<()> {
            self.0.lock().unwrap().push((request.to_string(), allow));
            Ok(())
        }
    }

    /// A live screen whose agent 1 runs a [`Decides`] session; what it
    /// was answered.
    fn deciding(w: &mut World) -> (Live, Answers) {
        let mut live = idle_live(2);
        let answers = Arc::new(Mutex::new(Vec::new()));
        live.ready(
            w,
            1,
            Ok((
                Box::new(Decides(answers.clone())),
                PathBuf::from("/w"),
                None,
                None,
            )),
        );
        (live, answers)
    }

    /// Agent `id`'s session asks on its own wire to run `command`.
    fn codex_asks(live: &Live, id: AgentId, request: &str, command: &str) {
        let what = Ask::Command {
            tool: "shell".into(),
            command: command.into(),
        };
        live.tx
            .send((
                id,
                live.generation[id],
                AgentEvent::Asks {
                    request: request.into(),
                    what,
                },
            ))
            .unwrap();
    }

    #[test]
    fn what_a_session_asks_on_its_own_wire_is_answered_through_it() {
        let mut w = team(1);
        let (mut live, answers) = deciding(&mut w);
        codex_asks(&live, 1, "5", "cargo test");
        codex_asks(&live, 1, "6", "make a");
        codex_asks(&live, 1, "7", "make b");
        live.step(&mut w);
        // The routine one at once, the rest when the person says.
        assert_eq!(*answers.lock().unwrap(), [("5".to_string(), true)]);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" allowed on its own: shell: cargo test"))
        );
        let open = w.open_inbox();
        let shown: Vec<_> = open.iter().map(|&i| w.inbox[i].detail.as_str()).collect();
        assert_eq!(shown, ["shell: make a", "shell: make b"]);
        live.decide(&mut w, open[1], false);
        live.decide(&mut w, open[0], true);
        assert_eq!(
            *answers.lock().unwrap(),
            [
                ("5".to_string(), true),
                ("7".to_string(), false),
                ("6".to_string(), true)
            ]
        );
        assert!(live.pending[1].is_empty() && !w.has_approval(1));
        // The session's end answers nobody: the session is gone.
        codex_asks(&live, 1, "8", "make c");
        live.step(&mut w);
        assert!(w.has_approval(1));
        live.end(&mut w, 1);
        assert!(!w.has_approval(1) && live.pending[1].is_empty());
        assert_eq!(answers.lock().unwrap().len(), 3);
        // An ask of an earlier session is dropped with its events.
        live.tx
            .send((
                1,
                live.generation[1] - 1,
                AgentEvent::Asks {
                    request: "9".into(),
                    what: Ask::Command {
                        tool: "shell".into(),
                        command: "make d".into(),
                    },
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert!(!w.has_approval(1));
    }

    #[test]
    fn an_ask_its_session_settled_closes_without_a_decision() {
        let mut w = team(1);
        let (mut live, answers) = deciding(&mut w);
        for (r, c) in [("1", "make a"), ("2", "make b"), ("3", "make c")] {
            codex_asks(&live, 1, r, c);
        }
        // The leader's ask stays whatever the worker's session settles.
        let lead = ask_to_run(&live, 0, "make lead");
        live.step(&mut w);
        let journal = w.journal.len();
        let withdraw = |live: &Live, request: &str| {
            live.tx
                .send((
                    1,
                    live.generation[1],
                    AgentEvent::Withdrawn {
                        request: request.into(),
                    },
                ))
                .unwrap();
        };
        withdraw(&live, "2");
        withdraw(&live, "never asked");
        live.step(&mut w);
        let open: Vec<_> = w
            .open_inbox()
            .iter()
            .map(|&i| w.inbox[i].detail.clone())
            .collect();
        assert_eq!(open, ["shell: make a", "shell: make c", "Bash: make lead"]);
        assert_eq!(w.journal.len(), journal, "nobody decided");
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" no longer asks: shell: make b"))
        );
        assert_eq!(live.pending[1].len(), 2);
        // The rest still pair up with their items.
        let items = w.open_inbox();
        live.decide(&mut w, items[1], false);
        assert_eq!(*answers.lock().unwrap(), [("3".to_string(), false)]);
        withdraw(&live, "1");
        live.step(&mut w);
        assert!(!w.has_approval(1) && w.has_approval(0));
        assert!(live.pending[1].is_empty());
        assert!(lead.try_recv().is_err());
        assert!(answers.lock().unwrap().len() == 1);
    }

    #[test]
    fn dropping_imece_ends_every_session_and_kills_every_run() {
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        let mut w = team(1);
        let mut live = idle_live(2);
        let ended = Arc::new(AtomicBool::new(false));
        live.ready(
            &mut w,
            1,
            Ok((
                Box::new(Ends(ended.clone())),
                PathBuf::from("/w"),
                None,
                None,
            )),
        );
        let mut sleeper = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[0].add(sleeper.id() as i32);
        let mut other = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[1].add(other.id() as i32);
        drop(live);
        assert!(ended.load(Ordering::SeqCst), "the session was not ended");
        for s in [&mut sleeper, &mut other] {
            let status = s.wait().unwrap();
            assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
        }
    }

    #[test]
    fn the_run_tool_names_what_each_role_may_run_and_the_deadline() {
        let extra = vec!["npm".to_string()];
        let t = Duration::from_secs(120);
        let lead = run_tool(Role::Leader, &extra, t);
        assert_eq!(lead.name, "run");
        assert!(
            lead.description.contains("fmt --check"),
            "{}",
            lead.description
        );
        assert!(!lead.description.contains("npm"));
        assert!(lead.description.contains("killed after 120 s"));
        assert_eq!(lead.input["required"], json!(["command"]));
        let worker = run_tool(Role::Worker, &extra, t);
        assert!(
            worker.description.contains("; npm."),
            "{}",
            worker.description
        );
        assert!(
            worker
                .description
                .contains("not add, remove, install or publish")
        );
        assert!(
            worker
                .description
                .contains("git (blame, diff, grep, log, ls-files, rev-parse, show, status only)")
        );
    }

    #[test]
    fn a_run_is_checked_on_the_screen_s_loop_and_answered_from_its_own_thread() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.run_programs = vec!["sh".into()];
        live.run_timeout = Duration::from_secs(5);
        // No session, no directory.
        let e = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "git status" }),
        )
        .unwrap_err();
        assert!(e.contains("no directory"), "{e}");
        // A command may be MAX_COMMAND characters long, not one more.
        let long = |n: usize| json!({ "command": format!("git log {}", "x".repeat(n - 8)) });
        let e = call(&mut live, &mut w, 1, "run", long(MAX_COMMAND)).unwrap_err();
        assert!(e.contains("no directory"), "{e}");
        let e = call(&mut live, &mut w, 1, "run", long(MAX_COMMAND + 1)).unwrap_err();
        assert!(e.contains("longer than 1000"), "{e}");
        let dir = std::env::temp_dir().join(format!("imece-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        live.cwds[1] = Some(dir.clone());
        live.cwds[0] = Some(dir.clone());
        for (args, says) in [
            (json!({}), "what to run"),
            (json!({ "command": "   " }), "what to run"),
            (json!({ "command": "a".repeat(1001) }), "longer than 1000"),
            (json!({ "command": "sh -c 'ls' | head" }), "no shell"),
            (json!({ "command": "git push" }), "only reads"),
            (json!({ "command": "ls" }), "not a program imece runs"),
        ] {
            let e = call(&mut live, &mut w, 1, "run", args).unwrap_err();
            assert!(e.contains(says), "{e}");
        }
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("run refused: git push")),
            "{:?}",
            w.agents[1].trace
        );
        // The leader may not run the person's programs.
        let e = call(
            &mut live,
            &mut w,
            0,
            "run",
            json!({ "command": "sh -c 'pwd'" }),
        )
        .unwrap_err();
        assert!(e.contains("not a program imece runs"), "{e}");
        // A worker's run happens in its directory, and the answer carries
        // the exit code and the output.
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'pwd; echo err >&2; exit 2'" }),
        )
        .unwrap();
        assert_eq!(
            got,
            format!(
                "exit code 2 (failed)\n{}\nerr",
                dir.canonicalize().unwrap().display()
            )
        );
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'exit 0'" }),
        )
        .unwrap();
        assert_eq!(got, "exit code 0, no output");
        // The trace says what ran and how it ended, once the loop ran again.
        live.step(&mut w);
        let trace = w.agents[1]
            .trace
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(trace.contains("run: sh -c 'exit 0'\n"), "{trace}");
        assert!(trace.contains("run: sh -c 'exit 0': exit 0, "), "{trace}");
        assert!(trace.contains(" s, 0 bytes"), "{trace}");
        assert!(live.running.iter().all(run::Running::is_empty));
        // A program that can not start is an error, not an exit code.
        live.run_programs.push("imece-no-such-program".into());
        let e = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "imece-no-such-program" }),
        )
        .unwrap_err();
        assert!(e.contains("could not start imece-no-such-program"), "{e}");
        // Past the deadline the run is killed and says so, with the
        // deadline (the time it took may be a second more under load).
        live.run_timeout = Duration::from_secs(1);
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'echo a; sleep 20'" }),
        )
        .unwrap();
        assert!(
            got.starts_with("killed after 1 s: it did not finish in time\na"),
            "{got}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_repository_s_notes_for_agents_go_into_the_prompt() {
        let dir = std::env::temp_dir().join(format!("imece-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(repo_notes(&dir, Vendor::Claude), None);
        std::fs::write(dir.join("CLAUDE.md"), "  \n").unwrap();
        assert_eq!(
            repo_notes(&dir, Vendor::Claude),
            None,
            "an empty file is none"
        );
        std::fs::write(dir.join("CLAUDE.md"), "# notes\n\nRun cargo fmt.\n").unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert_eq!(
            (notes.file, notes.size, notes.cut),
            ("CLAUDE.md", 23, false)
        );
        let notes = notes.text;
        assert!(notes.starts_with("\n\nThe repository's notes"), "{notes}");
        assert!(notes.contains("from its CLAUDE.md:\n\n# notes\n\nRun cargo fmt."));
        assert!(notes.ends_with("cargo fmt."), "{notes:?}");
        // Codex reads AGENTS.md on its own; the others get it from imece.
        assert_eq!(repo_notes(&dir, Vendor::Codex).unwrap().file, "CLAUDE.md");
        std::fs::write(dir.join("AGENTS.md"), "agents first").unwrap();
        for v in [Vendor::Claude, Vendor::Gemini] {
            let notes = repo_notes(&dir, v).unwrap();
            assert_eq!((notes.file, notes.size), ("AGENTS.md", 12));
            assert!(
                notes.text.contains("from its AGENTS.md:\n\nagents first"),
                "{}",
                notes.text
            );
        }
        assert_eq!(repo_notes(&dir, Vendor::Codex), None);
        // A file of exactly the cap is whole; one byte more is cut at a
        // character boundary, and says so (the size is the file's, not
        // what went in).
        std::fs::write(dir.join("AGENTS.md"), "x".repeat(MAX_NOTES)).unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert_eq!((notes.size, notes.cut), (MAX_NOTES, false));
        let notes = notes.text;
        assert!(!notes.contains("[cut"), "{}", &notes[notes.len() - 40..]);
        assert_eq!(notes.matches('x').count(), MAX_NOTES);
        let long = format!("{}é", "x".repeat(MAX_NOTES - 1));
        std::fs::write(dir.join("AGENTS.md"), &long).unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert_eq!((notes.size, notes.cut), (MAX_NOTES + 1, true));
        let notes = notes.text;
        assert!(
            notes.ends_with("x\n[cut: the file goes on]"),
            "{}",
            &notes[notes.len() - 40..]
        );
        assert!(!notes.contains('é'));
        assert_eq!(notes.matches('x').count(), MAX_NOTES - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
