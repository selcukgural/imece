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

/// git's long options that run a program (a pager) or write a file.
/// git takes a long option by any start that names one alone, so a
/// name that starts one of these is refused (`--open=vim`, `--o`).
pub(super) const GIT_DENIED_LONG: &[&str] = &["open-files-in-pager", "output"];

/// git's short option that runs a pager (`-O`, grep). Short options go
/// together (`-iOvim`), so a group with an `O` anywhere is refused;
/// `-O<orderfile>` of diff and log, and a value with an `O` in it
/// (`-SFOO`), go with them: spelled apart (`-S FOO`) they run.
pub(super) const GIT_DENIED_SHORT: char = 'O';

/// Whether `arg` is, or may be read by git as, an option that runs a
/// program or writes a file.
pub(super) fn git_denied(arg: &str) -> bool {
    if let Some(long) = arg.strip_prefix("--") {
        let name = long.split('=').next().unwrap_or_default();
        !name.is_empty() && GIT_DENIED_LONG.iter().any(|o| o.starts_with(name))
    } else {
        arg.strip_prefix('-')
            .is_some_and(|short| short.contains(GIT_DENIED_SHORT))
    }
}

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
/// leader hires workers and a worker hires helpers; helpers and
/// researchers hire nobody.
pub(super) fn tools_of(role: Role, programs: &[String], timeout: Duration) -> Vec<Tool> {
    // The board is the team's: a solo has none.
    let mut tools = match role {
        Role::Solo => vec![send_tool(), run_tool(role, programs, timeout)],
        _ => vec![
            send_tool(),
            post_tool(),
            board_tool(),
            run_tool(role, programs, timeout),
        ],
    };
    tools.push(whoami_tool());
    match role {
        Role::Worker => {
            tools.push(claim_tool());
            tools.push(hire_tool(HIRE_HELPER, false));
        }
        Role::Solo => tools.push(hire_tool(HIRE_HELPER, false)),
        Role::Leader => {
            tools.push(plan_tool());
            tools.push(hire_tool(HIRE_WORKER, true));
        }
        Role::Helper | Role::Researcher => {}
    }
    tools
}

/// imece's `plan` tool (record 114), the leader's: the cards it means to
/// assign or post before it hires a second worker, for the person to read.
pub(super) fn plan_tool() -> Tool {
    let tiers: Vec<&str> = Tier::ALL.iter().map(|t| t.wire()).collect();
    Tool {
        name: "plan".into(),
        description: "Puts your plan before the person: the cards you mean to assign or \
                      post, each with a tier and the paths it owns, and what you expect \
                      them to cost. Needed before you hire a second worker: the person \
                      reads it in WAITING FOR YOU and their yes opens hiring; end your \
                      turn after it. A new plan replaces the one waiting."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "cards": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_PLANNED,
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "maxLength": MAX_TITLE,
                                       "description": "One line, as a card's title." },
                            "tier": { "type": "string", "enum": tiers,
                                      "description": "The worker's tier for it; standard when left out." },
                            "paths": paths_schema(),
                            "body": { "type": "string", "maxLength": MAX_BODY,
                                      "description": "The task: what to do, when it is done." },
                        },
                        "required": ["title", "body"],
                        "additionalProperties": false,
                    },
                },
                "cost": {
                    "type": "number",
                    "minimum": 0,
                    "description": "What you expect the cards to cost in dollars, all \
                                    together: tiers times a rough count of turns.",
                },
            },
            "required": ["cards", "cost"],
            "additionalProperties": false,
        }),
    }
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

/// `git` for an agent in a Lonca workspace, refused. The `.git` there is the
/// agent's own to write (record 119), unlike a worktree's, so git run for it
/// would run what its config says, past the session's restrictions and out
/// of `lonca hook`'s sight.
pub(super) fn no_git_in_workspace(agent: &Agent, argv: &[String]) -> Result<(), String> {
    if agent.workspace.is_some() && argv[0] == "git" {
        return Err("git is not run through imece in a Lonca workspace; \
                    its preview and status tools say what git would"
            .to_string());
    }
    Ok(())
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
            if let Some(a) = argv.iter().find(|a| git_denied(a)) {
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

/// What `hire`'s `from` says, for a leader: a worker's start point.
const HIRE_FROM: &str = "Where the worker's worktree starts (default: the checkout's HEAD): a \
                         branch, tag or commit of the checkout, or #N for pull request N of \
                         origin. Only when the work is on that point (the person asked for \
                         it): the worker's change is against it, and its worktree never moves \
                         to HEAD.";

/// The hire tool, with `from` for a leader (`start`): a helper works in
/// its worker's worktree, wherever that started.
fn hire_tool(description: &str, start: bool) -> Tool {
    let tiers: Vec<&str> = Tier::ALL.iter().map(|t| t.wire()).collect();
    let mut input = json!({
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
        },
        "additionalProperties": false,
    });
    if start {
        input["properties"]["from"] = json!({
            "type": "string",
            "maxLength": worktree::START_MAX,
            "description": HIRE_FROM,
        });
    }
    Tool {
        name: "hire".into(),
        description: description.into(),
        input,
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
                "paths": paths_schema(),
            },
            "required": ["title", "body"],
            "additionalProperties": false,
        }),
    }
}

/// The `paths` of an assign or a post: the paths the card owns.
fn paths_schema() -> Value {
    json!({
        "type": "array",
        "maxItems": paths::MAX_PATHS,
        "items": { "type": "string", "maxLength": paths::MAX_PATH },
        "description": "The paths the card owns, relative to the repository, as git \
                        pathspecs (a file, a directory, or a pattern with *): the \
                        worker is told when it writes outside them, and no other open \
                        card may own one of them; a path every card touches ends with \
                        the word shared (CHANGELOG.md shared). Give them on every card.",
    })
}

pub(super) fn whoami_tool() -> Tool {
    Tool {
        name: "whoami".into(),
        description: "Says who you are in imece: your name, role and project, who you \
                      answer to, what is left of the budget, and whether you may hire."
            .into(),
        input: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
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
                      soon as they are free (while they wait on the person's answer, \
                      only cancel does; the rest waits for it); inform, progress and \
                      notice wait until something else does. To the person, an ask waits on their screen \
                      until they answer (which starts your next turn); the rest only \
                      informs them."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "The teammate's name, or you for the person. The \
                                    leader may also write an inform or a notice to a group \
                                    of its workers: workers (all) or idle; each gets it as \
                                    its own message.",
                },
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
                "re": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "The number of the message this replies to: each \
                                    message you get is headed [type number]. An answer \
                                    to an ask, and a done for an assign, carry it. Only \
                                    a message sent to you, and the reply goes to its \
                                    sender.",
                },
                "paths": paths_schema(),
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
            no_git_in_workspace(&w.agents[from], &argv)?;
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
        // A workspace's run is tied to no tree: that would be imece's git in
        // it, and its Δ is a proposal.
        let stamped = matches!(w.agents[from].role, Role::Worker | Role::Helper)
            && w.agents[from].workspace.is_none();
        let ran_tx = self.ran_tx.clone();
        thread::spawn(move || {
            let before = if stamped {
                worktree::tree(&cwd).ok()
            } else {
                None
            };
            let outcome = run::run(&argv, &cwd, timeout, MAX_RUN_OUTPUT, &running);
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
        let reply = Reply {
            input: args["input"].clone(),
            to: reply,
        };
        self.ask(w, from, claude::ask(args), reply);
    }

    /// Agent `from`'s session asks before it does `what`: a routine
    /// command is allowed at once, and so is one whose commands the
    /// person said yes to from now on for its project
    /// ([`standing::covered`]), and anything while the person's yes for
    /// the agent's turn holds ([`World::allowed_this_turn`], counted);
    /// anything else waits in the person's inbox and is answered when
    /// they decide. The trace says which.
    pub(super) fn ask(&mut self, w: &mut World, from: AgentId, what: Ask, reply: Reply) {
        if routine(w.agents[from].role, &what, &self.run_programs) {
            w.agents[from].trace(w.tick, format!("allowed on its own: {}", what.line()));
            self.reply(reply, true);
            return;
        }
        if self.standing_yes(w, from, &what) {
            w.agents[from].trace(
                w.tick,
                format!("allowed by your yes from now on: {}", what.line()),
            );
            self.reply(reply, true);
            return;
        }
        if w.allowed_this_turn(from) {
            w.agents[from].trace(
                w.tick,
                format!("allowed by your yes for this turn: {}", what.line()),
            );
            self.reply(reply, true);
            return;
        }
        w.agents[from].trace(w.tick, format!("asks the person: {}", what.line()));
        w.asks_you(from, &what.line());
        self.pending[from].push(Pending { what, reply });
    }

    /// Gives the session that asked the answer.
    pub(super) fn reply(&self, reply: Reply, allow: bool) {
        let _ = reply.to.send(Ok(claude::decision(allow, &reply.input)));
    }

    /// Whether agent `from`'s project has a standing yes for every
    /// command `what` runs that is not routine.
    fn standing_yes(&self, w: &World, from: AgentId, what: &Ask) -> bool {
        let Ask::Command { command, .. } = what else {
            return false;
        };
        let role = w.agents[from].role;
        let rules = &w.projects[w.agents[from].project].allowed;
        standing::covered(
            command,
            |argv| allowed(role, argv, &self.run_programs).is_ok(),
            rules,
        )
    }

    /// The person says yes from now on to what inbox item `item` asks: a
    /// rule for each of its commands not yet allowed goes into its
    /// project's standing yes ([`standing::needed`]), the ask is allowed,
    /// and so is every other open ask of the project that the rules now
    /// cover. The rules added are given back; nothing changes for what is
    /// not a command.
    pub fn always(&mut self, w: &mut World, item: usize) -> Result<Vec<Standing>, standing::Why> {
        let Some((id, nth)) = Self::ask_at(w, item) else {
            return Err(standing::Why::NotACommand);
        };
        let Some(Ask::Command { command, .. }) = self.pending[id].get(nth).map(|p| &p.what) else {
            return Err(standing::Why::NotACommand);
        };
        let role = w.agents[id].role;
        let p = w.agents[id].project;
        let rules = standing::needed(
            command,
            |argv| allowed(role, argv, &self.run_programs).is_ok(),
            &w.projects[p].allowed,
        )?;
        let added = w.stand(p, &rules);
        self.decide(w, item, true);
        self.go_on_allowed(w, p);
        Ok(added)
    }

    /// The person says yes to everything the agent of approval `item`
    /// asks until its turn ends: the ask is allowed, so is every other
    /// open ask of that agent (counted), and its next asks go on in
    /// [`ask`](Self::ask) until the turn or the session ends. `false`,
    /// and nothing changes, when `item` is not an open approval.
    pub fn yes_turn(&mut self, w: &mut World, item: usize) -> bool {
        let Some((id, _)) = Self::ask_at(w, item) else {
            return false;
        };
        w.yes_this_turn(id);
        self.decide(w, item, true);
        for other in 0..w.inbox.len() {
            if Self::ask_at(w, other).is_some_and(|(a, _)| a == id) && w.allowed_this_turn(id) {
                self.decide(w, other, true);
            }
        }
        true
    }

    /// Every open ask of project `p` its standing yes now covers is
    /// allowed: the person said yes from now on to what it runs.
    pub fn go_on_allowed(&mut self, w: &mut World, p: ProjectId) {
        for other in 0..w.inbox.len() {
            let Some((a, n)) = Self::ask_at(w, other) else {
                continue;
            };
            if w.agents[a].project == p
                && self.pending[a]
                    .get(n)
                    .is_some_and(|pending| self.standing_yes(w, a, &pending.what))
            {
                self.decide(w, other, true);
            }
        }
    }

    /// The agent inbox item `item` waits for and which of its waiting
    /// calls it is, when it is an open approval: an agent's open
    /// approvals and its waiting calls are in the same order.
    fn ask_at(w: &World, item: usize) -> Option<(AgentId, usize)> {
        let id = w
            .inbox
            .get(item)
            .filter(|i| i.open && i.kind == InboxKind::Approval)
            .map(|i| i.agent)?;
        let nth = w.inbox[..item]
            .iter()
            .filter(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
            .count();
        Some((id, nth))
    }

    /// The person allowed or refused what inbox item `item` asks: the
    /// session waiting on it goes on. An item that is not an open
    /// approval is left alone.
    pub fn decide(&mut self, w: &mut World, item: usize, allow: bool) {
        // An agent's open approvals and its waiting calls are in the same
        // order: an ask adds one of each, a decision or the session's end
        // takes them away together.
        let Some((id, nth)) = Self::ask_at(w, item) else {
            return;
        };
        w.decided(item, allow);
        if nth < self.pending[id].len() {
            let p = self.pending[id].remove(nth);
            let how = if allow { "allowed" } else { "refused" };
            w.agents[id].trace(w.tick, format!("the person {how}: {}", p.what.line()));
            self.reply(p.reply, allow);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

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
            // Together with other short options, and by a start of the
            // long name: git takes both.
            "git grep -iOvim x",
            "git grep -lO vim x",
            "git grep --open=vim x",
            "git grep --open-f vim x",
            "git grep --o=vim x",
            "git log --outp=/tmp/x",
            "git diff --out /tmp/x",
            "git diff -RO/tmp/order",
        ] {
            let e = allowed(Role::Leader, &argv(runs), &[]).unwrap_err();
            assert!(
                e.contains("a pager or an output file is not run through imece"),
                "{runs}: {e}"
            );
        }
        let e = allowed(Role::Worker, &argv("git diff -Oorder"), &[]).unwrap_err();
        assert!(e.starts_with("git diff -Oorder:"), "{e}");
        // A value with an O in it is refused with them; apart it runs.
        assert!(allowed(Role::Leader, &argv("git log -SFOO"), &[]).is_err());
        allowed(Role::Leader, &argv("git log -S FOO"), &[]).unwrap();
        for reads in [
            "git log --oneline -5",
            "git log --no-merges --",
            "git grep -o -n foo",
            "git grep -e FOO -- src",
            "git diff HEAD -- OLD.md",
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
            "git grep -lOsh x",
            "git grep --open=sh x",
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
            .send((from, APPROVE_TOOL.into(), args, tx, Default::default()))
            .unwrap();
        rx
    }

    /// What the session reads back from an ask.
    fn verdict(rx: &Receiver<Answer>) -> Value {
        serde_json::from_str(&rx.try_recv().unwrap().unwrap()).unwrap()
    }

    #[test]
    fn a_yes_from_now_on_allows_the_ask_its_project_s_others_and_the_next_ones() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let first = ask_to_run(&live, 1, "npm run build && cargo test");
        let same = ask_to_run(&live, 2, "npm run lint");
        let other = ask_to_run(&live, 2, "make clean");
        live.step(&mut w);
        let open = w.open_inbox();
        assert_eq!(open.len(), 3);
        // Not a command, or not plain, or an option first: nothing changes.
        assert_eq!(live.always(&mut w, 999), Err(standing::Why::NotACommand));
        w.asked_you(1, "Which API?");
        let question = *w.open_inbox().last().unwrap();
        assert_eq!(
            live.always(&mut w, question),
            Err(standing::Why::NotACommand)
        );
        let added = live.always(&mut w, open[0]).unwrap();
        let npm = Standing {
            program: "npm".into(),
            sub: Some("run".into()),
            whole: false,
        };
        assert_eq!(
            added,
            std::slice::from_ref(&npm),
            "cargo test is routine: no rule"
        );
        assert_eq!(w.projects[0].allowed, std::slice::from_ref(&npm));
        assert_eq!(verdict(&first)["behavior"], "allow");
        assert_eq!(verdict(&same)["behavior"], "allow", "covered now");
        assert!(other.try_recv().is_err(), "make clean still waits");
        assert!(!w.inbox[open[0]].open && !w.inbox[open[1]].open && w.inbox[open[2]].open);
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Standing { rules, added: true, .. } if rules == &["npm run …".to_string()]
        )));
        // The next ask it covers is not asked at all.
        let next = ask_to_run(&live, 1, "npm run test -- --watch=false");
        live.step(&mut w);
        assert_eq!(verdict(&next)["behavior"], "allow");
        assert!(w.agents[1].trace.iter().any(|l| {
            l.ends_with(" allowed by your yes from now on: Bash: npm run test -- --watch=false")
        }));
        // One it does not cover still asks; one with an option first gets
        // a yes to that line alone: it goes on, the same again too, another
        // still asks.
        let rm = ask_to_run(&live, 1, "rm -rf target");
        live.step(&mut w);
        assert!(rm.try_recv().is_err());
        let item = *w.open_inbox().last().unwrap();
        assert_eq!(
            live.always(&mut w, item),
            Ok(vec![Standing::exactly("rm -rf target")])
        );
        assert!(!w.inbox[item].open, "decided");
        assert_eq!(verdict(&rm)["behavior"], "allow");
        let again = ask_to_run(&live, 1, "rm -rf target");
        live.step(&mut w);
        assert_eq!(verdict(&again)["behavior"], "allow");
        let wider = ask_to_run(&live, 1, "rm -rf /");
        live.step(&mut w);
        assert!(wider.try_recv().is_err());
        let item = *w.open_inbox().last().unwrap();
        live.decide(&mut w, item, false);
        // With a substitution: a rule for it and for what it runs, so the
        // next one with other words goes on, and one that runs more asks.
        let gh = ask_to_run(&live, 1, "gh pr view \"$(git branch --show-current)\"");
        live.step(&mut w);
        assert!(gh.try_recv().is_err());
        let item = *w.open_inbox().last().unwrap();
        let added = live.always(&mut w, item).unwrap();
        assert_eq!(
            added.iter().map(Standing::line).collect::<Vec<_>>(),
            ["gh pr …", "git branch …"],
            "what it runs asks too"
        );
        assert_eq!(verdict(&gh)["behavior"], "allow");
        let next = ask_to_run(&live, 1, "gh pr checks \"$(git branch)\" 2>&1");
        live.step(&mut w);
        assert_eq!(verdict(&next)["behavior"], "allow");
        let more = ask_to_run(&live, 1, "gh pr view \"$(rm x)\"");
        live.step(&mut w);
        assert!(more.try_recv().is_err());
        let item = *w.open_inbox().last().unwrap();
        live.decide(&mut w, item, false);
        // A rule added by name lets the open ask it covers go on.
        w.stand(
            0,
            &[Standing {
                program: "make".into(),
                sub: Some("clean".into()),
                whole: false,
            }],
        );
        live.go_on_allowed(&mut w, 0);
        assert_eq!(verdict(&other)["behavior"], "allow");
        // Another project's rules are not this one's.
        let mut w2 = team(1);
        w2.projects[0].allowed.clear();
        let mut live2 = idle_live(2);
        let asked = ask_to_run(&live2, 1, "npm run build");
        live2.step(&mut w2);
        assert!(asked.try_recv().is_err(), "no rule in this world");
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

    /// A session that does nothing.
    struct Idle;

    impl imece_agent::Agent for Idle {
        fn prompt(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn command(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn terminate(&mut self) {}
    }

    /// A live screen whose agent 1 runs an [`Idle`] session.
    fn running_one(w: &mut World) -> Live {
        let mut live = idle_live(w.agents.len());
        live.ready(
            w,
            1,
            Ok((
                Box::new(Idle),
                PathBuf::from("/w"),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
            )),
        );
        live
    }

    /// Whether the session was let go on.
    fn allowed_to(rx: &Receiver<Answer>) -> bool {
        verdict(rx)["behavior"] == "allow"
    }

    #[test]
    fn a_yes_for_the_turn_allows_the_agents_asks_until_its_turn_or_session_ends() {
        let mut w = team(2);
        let mut live = running_one(&mut w);
        let first = ask_to_run(&live, 1, "make a");
        let second = ask_to_run(&live, 1, "make b");
        let other = ask_to_run(&live, 2, "make other");
        let lead = ask_to_run(&live, 0, "make lead");
        live.step(&mut w);
        let open = w.open_inbox();
        assert_eq!(open.len(), 4);
        // Not an open approval: nothing changes.
        assert!(!live.yes_turn(&mut w, 999));
        w.asked_you(1, "Which API?");
        let question = w.inbox.len() - 1;
        assert!(!live.yes_turn(&mut w, question));
        assert_eq!(w.agents[1].turn_yes, None);
        assert!(first.try_recv().is_err() && second.try_recv().is_err());
        // The first ask of worker 1: it and worker 1's other open one go
        // on; the others' asks wait.
        assert!(live.yes_turn(&mut w, open[0]));
        assert!(allowed_to(&first) && allowed_to(&second));
        assert_eq!(w.agents[1].turn_yes, Some(1), "the other open one counted");
        assert!(live.pending[1].is_empty());
        let still: Vec<_> = (w.open_inbox().iter())
            .map(|&i| w.inbox[i].detail.clone())
            .collect();
        assert_eq!(still, ["Bash: make other", "Bash: make lead", "Which API?"]);
        assert!(other.try_recv().is_err() && lead.try_recv().is_err());
        assert!(
            w.journal
                .iter()
                .any(|e| e.event == Event::TurnYes { agent: 1 })
        );
        // Its next asks go on at once, counted; a routine one is no yes of
        // the person's and is not.
        let third = ask_to_run(&live, 1, "rm -rf build");
        let routine = ask_to_run(&live, 1, "cargo test");
        live.step(&mut w);
        assert!(allowed_to(&third) && allowed_to(&routine));
        assert_eq!(w.agents[1].turn_yes, Some(2));
        assert!(!w.has_approval(1));
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(" allowed by your yes for this turn: Bash: rm -rf build"))
        );
        // Another agent's ask still waits for the person.
        let again = ask_to_run(&live, 2, "make again");
        live.step(&mut w);
        assert!(again.try_recv().is_err());
        assert_eq!(w.agents[2].turn_yes, None);
        // The turn ends: so does the yes, said with its count; the next
        // ask waits again.
        let done = AgentEvent::TurnDone {
            ok: true,
            text: "done".into(),
            cost: 0,
            denied: 0,
        };
        live.tx.send((1, live.generation[1], done)).unwrap();
        live.step(&mut w);
        assert_eq!(w.agents[1].turn_yes, None);
        let ended = |w: &World| {
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::TurnYesEnded { agent: 1, .. }))
                .map(|e| e.event.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ended(&w),
            [Event::TurnYesEnded {
                agent: 1,
                allowed: 2
            }]
        );
        let fifth = ask_to_run(&live, 1, "make c");
        live.step(&mut w);
        assert!(fifth.try_recv().is_err(), "asked again");
        assert!(w.has_approval(1));
        // Given again, the session's end takes it.
        let item = (w.open_inbox().into_iter())
            .find(|&i| w.inbox[i].agent == 1 && w.inbox[i].kind == InboxKind::Approval);
        assert!(live.yes_turn(&mut w, item.unwrap()));
        assert_eq!(w.agents[1].turn_yes, Some(0));
        live.end(&mut w, 1);
        assert_eq!(w.agents[1].turn_yes, None);
        assert_eq!(ended(&w).len(), 2);
        assert_eq!(
            ended(&w)[1],
            Event::TurnYesEnded {
                agent: 1,
                allowed: 0
            }
        );
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
                imece_agent::copy::Brought::default(),
                setup::Done::None,
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
    fn each_role_gets_its_tools_and_a_solo_has_no_board() {
        let names = |role| -> Vec<String> {
            tools_of(role, &[], Duration::from_secs(5))
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(names(Role::Solo), ["send", "run", "whoami", "hire"]);
        assert_eq!(
            names(Role::Worker),
            ["send", "post", "board", "run", "whoami", "claim", "hire"]
        );
        assert_eq!(
            names(Role::Leader),
            ["send", "post", "board", "run", "whoami", "plan", "hire"]
        );
        assert_eq!(
            names(Role::Helper),
            ["send", "post", "board", "run", "whoami"]
        );
        assert_eq!(
            names(Role::Researcher),
            ["send", "post", "board", "run", "whoami"]
        );
        let solo_hire = &tools_of(Role::Solo, &[], Duration::from_secs(5))[3];
        assert_eq!(solo_hire.description, HIRE_HELPER, "a solo hires helpers");
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
    fn git_goes_through_imece_only_outside_a_workspace() {
        let mut w = team(2);
        let line = |l: &str| run::split(l).unwrap();
        let (git, other) = (line("git status"), line("cargo test"));
        assert_eq!(no_git_in_workspace(&w.agents[1], &git), Ok(()));
        assert_eq!(no_git_in_workspace(&w.agents[1], &other), Ok(()));
        w.agents[1].workspace = Some("demo".into());
        let e = no_git_in_workspace(&w.agents[1], &git).unwrap_err();
        assert!(e.contains("Lonca workspace"), "{e}");
        assert_eq!(no_git_in_workspace(&w.agents[1], &other), Ok(()));
    }

    #[test]
    fn a_run_in_a_workspace_reads_no_tree_and_runs_no_git() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.run_programs = vec!["sh".into()];
        live.run_timeout = Duration::from_secs(5);
        let dir = crate::live::testing::checkout("run-workspace");
        live.cwds[1] = Some(dir.clone());
        let ok = json!({ "command": "sh -c 'exit 0'" });
        let git = json!({ "command": "git status" });
        // In a worktree a run is tied to its tree, and git reads.
        call(&mut live, &mut w, 1, "run", ok.clone()).unwrap();
        live.step(&mut w);
        assert_eq!(live.ran.len(), 1);
        call(&mut live, &mut w, 1, "run", git.clone()).unwrap();
        live.step(&mut w);
        assert_eq!(live.ran.len(), 2);
        // In a workspace neither, and the program still runs.
        live.ran.clear();
        w.agents[1].workspace = Some("demo".into());
        let got = call(&mut live, &mut w, 1, "run", ok).unwrap();
        assert_eq!(got, "exit code 0, no output");
        live.step(&mut w);
        assert_eq!(live.ran.len(), 0, "{:?}", live.ran);
        let e = call(&mut live, &mut w, 1, "run", git).unwrap_err();
        assert!(e.contains("Lonca workspace"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
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
    fn an_assign_and_a_post_take_the_paths_the_card_owns_capped() {
        for tool in [send_tool(), post_tool()] {
            let paths = &tool.input["properties"]["paths"];
            assert_eq!(paths["type"], "array", "{}", tool.name);
            assert_eq!(paths["maxItems"], 20);
            assert_eq!(paths["items"]["type"], "string");
            assert_eq!(paths["items"]["maxLength"], 200);
            assert!(
                paths["description"].as_str().unwrap().contains("shared"),
                "{}",
                tool.name
            );
            assert!(
                !tool.input["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("paths"))
            );
        }
        assert!(claim_tool().input["properties"]["paths"].is_null());
    }
    #[test]
    fn the_plan_tool_takes_cards_with_a_tier_and_paths_and_a_cost() {
        let tool = plan_tool();
        assert_eq!(tool.name, "plan");
        let input = &tool.input;
        assert_eq!(input["required"], json!(["cards", "cost"]));
        let cards = &input["properties"]["cards"];
        assert_eq!(
            (cards["minItems"].as_u64(), cards["maxItems"].as_u64()),
            (Some(1), Some(20))
        );
        let card = &cards["items"];
        assert_eq!(card["required"], json!(["title", "body"]));
        assert_eq!(card["properties"]["title"]["maxLength"], 120);
        assert_eq!(card["properties"]["body"]["maxLength"], 4000);
        assert_eq!(
            card["properties"]["tier"]["enum"],
            json!(["cheap", "standard", "strong"])
        );
        assert_eq!(card["properties"]["paths"], paths_schema());
        assert_eq!(card["additionalProperties"], false);
        assert_eq!(input["properties"]["cost"]["minimum"], 0);
        assert_eq!(input["properties"]["cost"]["type"], "number");
        assert!(tool.description.contains("second worker"));
    }
}
