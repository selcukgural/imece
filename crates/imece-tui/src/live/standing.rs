//! A standing yes (#229): the person says yes from now on to a command a
//! project's agents ask about (`A` on the ask, `:allow`), so the next ask
//! that only runs such commands is allowed without them. A rule is a
//! program and its first argument (`npm run`, `dotnet ef`), as Claude
//! Code's `Bash(npm run:*)`; it never matches a command with an option
//! before that argument, and git's options that start a program
//! (`tools::git_denied`) never match at all. A line of several commands
//! (`&&`, `||`, `;`, `|`, a new line) is allowed when each of them is
//! routine or has its rule.
//!
//! A rule already takes whatever follows its two words, so what a shell
//! makes of those words adds nothing to it: `$NAME`, `${NAME}`, a glob and
//! `$(…)` may stand there, and the commands a `$(…)` runs count as commands
//! of the line. Not in the two words themselves, and not for what is
//! routine or for git: their lists of what is denied read the words, which
//! an expansion hides (`cargo $X`, `git grep $X`, a file named `-Ovim`). A
//! redirection that changes nothing on the disk (`2>&1`, `>/dev/null`) is
//! dropped. Anything else a shell reads (a backtick, another redirection)
//! or a command with no rule (an option first) has one yes from now on left,
//! and `A` gives it: that line, byte for byte ([`Standing::exactly`]).

use imece_agent::run;

use super::tools::git_denied;
use crate::model::Standing;

/// Why a yes from now on cannot be made for what was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    /// It is not a command (a file edit, a fetch): it asks each time.
    NotACommand,
    /// A shell would read more than commands and their words in it.
    NotPlain,
    /// This command has an option before what it works on.
    OptionFirst(String),
}

/// Stands in a word for what a shell will make of it. The agent may write
/// this character too: it only makes a word count as unknown.
const EXPANDED: char = '\u{F0000}';

/// How deep `$(…)`s may nest.
const MAX_NESTED: usize = 4;

/// The redirections that leave the disk as it was.
const NO_EFFECT: [&str; 3] = ["2>&1", "1>&2", ">&2"];
const TO_NOTHING: [&str; 8] = ["&>>", "&>", "1>>", "1>", "2>>", "2>", ">>", ">"];

/// Whether a shell decides what `word` is.
fn expanded(word: &str) -> bool {
    word.contains(EXPANDED)
}

/// `rest` when nothing joins it to what came before.
fn ended(rest: &str) -> Option<&str> {
    rest.chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || matches!(c, ';' | '|' | '&'))
        .then_some(rest)
}

/// What follows the redirection `here` starts with, when it is one of
/// those that change nothing.
fn past_no_effect(here: &str) -> Option<&str> {
    let dup = NO_EFFECT.iter().find_map(|r| here.strip_prefix(r));
    let null = || {
        TO_NOTHING.iter().find_map(|r| {
            let path = here.strip_prefix(r)?.trim_start_matches([' ', '\t']);
            path.strip_prefix("/dev/null")
        })
    };
    dup.or_else(null).and_then(ended)
}

/// What follows a plain `NAME` or `{NAME}`, `rest` coming after its `$`;
/// none for anything else (`$?`, `${X:-$(id)}`, `$'…'`).
fn past_name(rest: &str) -> Option<&str> {
    let braced = rest.strip_prefix('{');
    let body = braced.unwrap_or(rest);
    let name = |c: char| c == '_' || c.is_ascii_alphanumeric();
    let after = body.trim_start_matches(name);
    let first = body.chars().next().filter(|c| !c.is_ascii_digit());
    first.filter(|&c| name(c))?;
    match braced {
        Some(_) => after.strip_prefix('}'),
        None => Some(after),
    }
}

/// `rest`, coming after a `$(`, as what is inside and what follows its
/// closing bracket.
fn inside(rest: &str) -> Option<(&str, &str)> {
    let mut open = 1;
    let mut quote: Option<char> = None;
    let mut chars = rest.char_indices();
    while let Some((at, c)) = chars.next() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some('"') | None, '\\') => {
                chars.next();
            }
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(') => open += 1,
            (None, ')') if open == 1 => return Some((&rest[..at], &rest[at + 1..])),
            (None, ')') => open -= 1,
            (None, _) => {}
        }
    }
    None
}

/// The commands `line` runs, split where a shell runs one after or into
/// another (`&&`, `||`, `;`, `|`, a new line) outside quotes, each as its
/// words ([`run::split`]), those a `$(…)` runs after them; a word a shell
/// decides holds [`EXPANDED`]. None when a part is empty or has anything
/// else a shell would read.
pub(crate) fn commands(line: &str) -> Option<Vec<Vec<String>>> {
    commands_in(line, 0)
}

fn commands_in(line: &str, nested: usize) -> Option<Vec<Vec<String>>> {
    let mut parts = Vec::new();
    let mut inner = Vec::new();
    let mut part = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.trim().chars();
    loop {
        let here = chars.as_str();
        let Some(c) = chars.next() else { break };
        let word_starts = !part.ends_with(|c: char| !c.is_whitespace());
        if let Some(rest) = (quote.is_none() && word_starts)
            .then(|| past_no_effect(here))
            .flatten()
        {
            chars = rest.chars();
            continue;
        }
        match (quote, c) {
            (Some(q), _) if c == q => {
                quote = None;
                part.push(c);
            }
            (Some('"'), '\\') | (None, '\\') => {
                part.push(c);
                part.extend(chars.next());
            }
            (Some('"') | None, '$') => {
                let rest = match chars.as_str().strip_prefix('(') {
                    Some(rest) if nested < MAX_NESTED => {
                        let (body, rest) = inside(rest)?;
                        inner.extend(commands_in(body, nested + 1)?);
                        rest
                    }
                    Some(_) => return None,
                    None => past_name(chars.as_str())?,
                };
                part.push(EXPANDED);
                chars = rest.chars();
            }
            (Some(_), _) => part.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                part.push(c);
            }
            (None, '&') if chars.as_str().starts_with('&') => {
                chars.next();
                parts.push(std::mem::take(&mut part));
            }
            (None, '|') => {
                if chars.as_str().starts_with('|') {
                    chars.next();
                }
                parts.push(std::mem::take(&mut part));
            }
            (None, ';' | '\n') => parts.push(std::mem::take(&mut part)),
            (None, '*' | '?' | '[') => part.push(EXPANDED),
            (None, _) => part.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    parts.push(part);
    let mut all = parts
        .iter()
        .map(|p| {
            Some(p.trim())
                .filter(|p| !p.is_empty())
                .and_then(|p| run::split(p).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    all.extend(inner);
    Some(all)
}

/// The rule a yes from now on to the command `argv` makes: its program
/// and first argument, or the program alone when it has none. Those two
/// are words a person can read, never what a shell makes of them.
pub(crate) fn rule_of(argv: &[String]) -> Result<Standing, Why> {
    if argv.iter().take(2).any(|w| expanded(w)) {
        return Err(Why::NotPlain);
    }
    match argv {
        [program] => Ok(Standing {
            program: program.clone(),
            sub: None,
            whole: false,
        }),
        [program, sub, ..] if !sub.starts_with('-') => Ok(Standing {
            program: program.clone(),
            sub: Some(sub.clone()),
            whole: false,
        }),
        _ => Err(Why::OptionFirst(argv.join(" "))),
    }
}

/// Whether the command `argv` is one `rule` says yes to: the same
/// program and the same first argument (none: the program alone), and
/// for git none of the options that start a program, nor a word that could
/// be one. A rule for one whole line matches no command: only that line
/// ([`covered`]).
pub(crate) fn matches(rule: &Standing, argv: &[String]) -> bool {
    let denied = |a: &String| git_denied(a) || expanded(a);
    !rule.whole
        && argv.first() == Some(&rule.program)
        && argv.get(1) == rule.sub.as_ref()
        && !(rule.program == "git" && argv.iter().any(denied))
}

/// Whether the command `argv` is routine as written: a list of what is
/// denied cannot read a word a shell decides.
fn known(routine: &impl Fn(&[String]) -> bool, argv: &[String]) -> bool {
    !argv.iter().any(|w| expanded(w)) && routine(argv)
}

/// Whether `rules` say yes to exactly `line`.
fn said_whole(line: &str, rules: &[Standing]) -> bool {
    let line = line.trim();
    rules.iter().any(|r| r.whole && r.program == line)
}

/// Whether one of `rules` says yes to `line` as it is, or every command
/// it runs is `routine` or has its rule.
pub(crate) fn covered(line: &str, routine: impl Fn(&[String]) -> bool, rules: &[Standing]) -> bool {
    said_whole(line, rules)
        || commands(line).is_some_and(|all| {
            all.iter()
                .all(|c| known(&routine, c) || rules.iter().any(|r| matches(r, c)))
        })
}

/// The rules a yes from now on to `line` adds to `rules`: one for each of
/// its commands that is not `routine` or said yes to already, or, when one
/// of them can have none that would hold, the rule for that line alone.
/// An empty line can have none.
pub(crate) fn needed(
    line: &str,
    routine: impl Fn(&[String]) -> bool,
    rules: &[Standing],
) -> Result<Vec<Standing>, Why> {
    if line.trim().is_empty() {
        return Err(Why::NotPlain);
    }
    if said_whole(line, rules) {
        return Ok(Vec::new());
    }
    let alone = || Ok(vec![Standing::exactly(line)]);
    let Some(all) = commands(line) else {
        return alone();
    };
    let mut out: Vec<Standing> = Vec::new();
    for c in &all {
        let said = |r: &Standing| matches(r, c);
        if known(&routine, c) || rules.iter().any(said) || out.iter().any(said) {
            continue;
        }
        match rule_of(c) {
            Ok(rule) if matches(&rule, c) => out.push(rule),
            _ => return alone(),
        }
    }
    Ok(out)
}

/// The rule `:allow` names with `words` (`npm run`, `make`).
pub(crate) fn named(words: &str) -> Result<Standing, Why> {
    match commands(words).as_deref() {
        Some(all) if all.iter().flatten().any(|w| expanded(w)) => Err(Why::NotPlain),
        Some([argv]) if argv.len() <= 2 => rule_of(argv),
        Some([argv]) => Err(Why::OptionFirst(argv.join(" "))),
        _ => Err(Why::NotPlain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_string).collect()
    }

    fn rule(program: &str, sub: Option<&str>) -> Standing {
        Standing {
            program: program.into(),
            sub: sub.map(str::to_string),
            whole: false,
        }
    }

    #[test]
    fn a_line_is_split_into_its_commands_outside_quotes() {
        let got =
            |l: &str| commands(l).map(|all| all.iter().map(|c| c.join(" ")).collect::<Vec<_>>());
        assert_eq!(got("npm run build"), Some(vec!["npm run build".into()]));
        assert_eq!(
            got("cargo build && npm test || make; ls | wc -l\nmake x"),
            Some(vec![
                "cargo build".into(),
                "npm test".into(),
                "make".into(),
                "ls".into(),
                "wc -l".into(),
                "make x".into(),
            ])
        );
        assert_eq!(
            got("echo 'a && b' \"c; d\" e\\;f"),
            Some(vec!["echo a && b c; d e;f".into()]),
            "quoted and escaped operators are words"
        );
        assert_eq!(got("  npm test  "), Some(vec!["npm test".into()]));
        for refused in [
            "",
            "npm test &&",
            "&& npm test",
            "a ;; b",
            "a & b",
            "echo `id`",
            "echo \"`id`\"",
            "ls > out",
            "ls >> out",
            "ls 2> out",
            "ls &> out",
            "cat < in",
            "cat <<EOF",
            "diff <(a) b",
            "ls >/dev/nullx",
            "ls 2>&12",
            "ls 2>&1x",
            "echo $?",
            "echo $$",
            "echo $1",
            "echo $",
            "echo \"cost $5\"",
            "echo ${X:-$(id)}",
            "echo ${X",
            "echo ${}",
            "echo $'a'",
            "echo $((1 + 2))",
            "echo $(a",
            "echo $()",
            "echo $(a # b)",
            "echo 'open",
            "echo \"open",
            "a |",
            "(cd x)",
        ] {
            assert_eq!(got(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn what_a_shell_decides_in_a_word_is_marked_and_quotes_are_kept_apart() {
        let shown = |l: &str| {
            commands(l).map(|all| {
                all.iter()
                    .map(|c| c.join(" ").replace(EXPANDED, "?"))
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(shown("echo $HOME"), Some(vec!["echo ?".into()]));
        assert_eq!(shown("echo ${HOME}/x"), Some(vec!["echo ?/x".into()]));
        assert_eq!(shown("echo $_a1b"), Some(vec!["echo ?".into()]));
        assert_eq!(shown("echo \"$HOME x\""), Some(vec!["echo ? x".into()]));
        assert_eq!(
            shown("rm src/*.o a? [ab]"),
            Some(vec!["rm src/?.o a? ?ab]".into()])
        );
        // Inside single quotes, or escaped, nothing is decided.
        assert_eq!(
            commands("echo '$(id) * ? [' \\$HOME \"\\$(id)\""),
            Some(vec![vec![
                "echo".to_string(),
                "$(id) * ? [".into(),
                "$HOME".into(),
                "\\$(id)".into()
            ]])
        );
        assert_eq!(
            commands("grep \"a*b?[\" x"),
            Some(vec![vec!["grep".to_string(), "a*b?[".into(), "x".into()]])
        );
    }

    #[test]
    fn what_a_substitution_runs_are_commands_of_the_line() {
        let shown = |l: &str| {
            commands(l).map(|all| {
                all.iter()
                    .map(|c| c.join(" ").replace(EXPANDED, "?"))
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            shown("gh pr view \"$(git branch --show-current)\""),
            Some(vec![
                "gh pr view ?".into(),
                "git branch --show-current".into()
            ])
        );
        assert_eq!(
            shown("a $(b | c) d && e $(f $(g))"),
            Some(vec![
                "a ? d".into(),
                "e ?".into(),
                "b".into(),
                "c".into(),
                "f ?".into(),
                "g".into()
            ]),
            "nested, each after the line it stands in"
        );
        assert_eq!(
            shown("a \"$(b \"c)\" 'd)' e\\) )\" f"),
            Some(vec!["a ? f".into(), "b c) d) e)".into()]),
            "a bracket quoted or escaped closes nothing"
        );
        assert_eq!(
            shown("a $(b)c"),
            Some(vec!["a ?c".into(), "b".into()]),
            "the word goes on after it"
        );
        // How deep they nest is capped, and what is past the cap refused.
        let nested = |n: usize| "$(".repeat(n) + "a" + &")".repeat(n);
        assert_eq!(
            commands(&nested(MAX_NESTED)).map(|c| c.len()),
            Some(MAX_NESTED + 1)
        );
        assert_eq!(commands(&nested(MAX_NESTED + 1)), None);
        assert_eq!(inside("a(b)c) d"), Some(("a(b)c", " d")));
        assert_eq!(inside("a(b)"), None);
    }

    #[test]
    fn a_redirection_that_changes_nothing_on_the_disk_is_dropped() {
        for quiet in [
            "2>&1",
            "1>&2",
            ">&2",
            ">/dev/null",
            "> /dev/null",
            ">  \t/dev/null",
            ">>/dev/null",
            "1>/dev/null",
            "1>>/dev/null",
            "2>/dev/null",
            "2>>/dev/null",
            "&>/dev/null",
            "&>>/dev/null",
        ] {
            let got = commands(&format!("make {quiet} x")).unwrap();
            assert_eq!(got, [["make", "x"]], "{quiet}");
            let got = commands(&format!("make {quiet}")).unwrap();
            assert_eq!(got, [["make"]], "{quiet} at the end");
            let got = commands(&format!("make {quiet}|wc")).unwrap();
            assert_eq!(got, [["make"], ["wc"]], "{quiet} into a pipe");
            let got = commands(&format!("make {quiet};wc")).unwrap();
            assert_eq!(got, [["make"], ["wc"]], "{quiet} before ;");
            let got = commands(&format!("make {quiet}&&wc")).unwrap();
            assert_eq!(got, [["make"], ["wc"]], "{quiet} before &&");
            let got = commands(&format!("make {quiet}\nwc")).unwrap();
            assert_eq!(got, [["make"], ["wc"]], "{quiet} before a new line");
            // Quoted, it is a word; joined to one, a shell's to read.
            let got = commands(&format!("echo '{quiet}'")).unwrap();
            assert_eq!(got, [["echo", quiet]]);
            assert_eq!(commands(&format!("echo x{quiet}")), None, "{quiet}");
            assert_eq!(commands(&format!("echo {quiet}x")), None, "{quiet}");
        }
        assert_eq!(commands("2>&1"), None, "no command");
    }

    #[test]
    fn a_rule_is_the_program_and_its_first_argument() {
        assert_eq!(
            rule_of(&words("npm run build")),
            Ok(rule("npm", Some("run")))
        );
        assert_eq!(rule_of(&words("make")), Ok(rule("make", None)));
        assert_eq!(rule_of(&words("rm target")), Ok(rule("rm", Some("target"))));
        assert_eq!(
            rule_of(&words("rm -rf target")),
            Err(Why::OptionFirst("rm -rf target".into()))
        );
        assert_eq!(rule_of(&[]), Err(Why::OptionFirst(String::new())));
        // What a shell decides is never part of a rule; after them it can be.
        let open = |w: &[&str]| {
            let words: Vec<String> = w
                .iter()
                .map(|w| w.replace('?', &EXPANDED.to_string()))
                .collect();
            rule_of(&words)
        };
        assert_eq!(open(&["?"]), Err(Why::NotPlain));
        assert_eq!(open(&["?", "run"]), Err(Why::NotPlain));
        assert_eq!(open(&["npm", "?"]), Err(Why::NotPlain));
        assert_eq!(open(&["npm", "run", "?"]), Ok(rule("npm", Some("run"))));
        assert_eq!(rule("npm", Some("run")).line(), "npm run …");
        assert_eq!(rule("make", None).line(), "make");
    }

    #[test]
    fn a_rule_matches_its_program_and_first_argument_only() {
        let npm = rule("npm", Some("run"));
        assert!(matches(&npm, &words("npm run build")));
        assert!(matches(&npm, &words("npm run")));
        assert!(!matches(&npm, &words("npm install x")));
        assert!(
            !matches(&npm, &words("npm --prefix /x run build")),
            "an option first"
        );
        assert!(!matches(&npm, &words("npx run")));
        assert!(!matches(&npm, &[]));
        let make = rule("make", None);
        assert!(matches(&make, &words("make")));
        assert!(!matches(&make, &words("make clean")), "the program alone");
        let git = rule("git", Some("grep"));
        assert!(matches(&git, &words("git grep x")));
        assert!(
            !matches(&git, &words("git grep -Ovim x")),
            "git's pager option"
        );
        assert!(!matches(&git, &words("git grep --open=vim x")));
        // Only git's options are git's.
        assert!(matches(&rule("tool", Some("x")), &words("tool x -O")));
    }

    #[test]
    fn a_line_is_covered_when_each_command_is_routine_or_said_yes_to() {
        let routine = |c: &[String]| c.first().is_some_and(|p| p == "cargo");
        let rules = [rule("npm", Some("run")), rule("make", None)];
        assert!(covered("npm run build", routine, &rules));
        assert!(covered(
            "cargo build && npm run test && make",
            routine,
            &rules
        ));
        assert!(!covered("cargo build && npm install", routine, &rules));
        assert!(
            !covered("npm run build > log", routine, &rules),
            "a redirection asks"
        );
        assert!(!covered("rm x", routine, &[]));
        assert!(covered("npm run build 2>&1", routine, &rules));
        assert!(covered("npm run build $X src/*.rs", routine, &rules));
        assert!(!covered("npm run build `id`", routine, &rules));
        assert!(covered("cargo test", routine, &[]));
        assert!(!covered("cargo test; rm -rf /", routine, &rules));
    }

    #[test]
    fn a_yes_from_now_on_adds_a_rule_for_each_command_not_yet_allowed() {
        let routine = |c: &[String]| c.first().is_some_and(|p| p == "cargo");
        let rules = [rule("make", None)];
        assert_eq!(
            needed(
                "cargo build && npm run x && make && npm run y && dotnet ef up",
                routine,
                &rules
            ),
            Ok(vec![rule("npm", Some("run")), rule("dotnet", Some("ef"))]),
            "once each, routine and allowed ones left out"
        );
        assert_eq!(needed("make", routine, &rules), Ok(Vec::new()));
    }

    #[test]
    fn a_shell_cannot_hide_what_is_routine_or_git_s_denied_options() {
        let routine = |c: &[String]| c.first().is_some_and(|p| p == "cargo" || p == "git");
        // Routine as written; with a word a shell decides, not (`$X` may
        // be `publish`, a denied command).
        assert!(covered("cargo build", routine, &[]));
        for hidden in [
            "cargo $X",
            "cargo build \"$(cat x)\"",
            "cargo build *",
            "git log [-]O",
        ] {
            assert!(!covered(hidden, routine, &[]), "{hidden}");
        }
        // A rule for git does not take one: `$X` may be `-Ovim`.
        let git = [rule("git", Some("grep"))];
        assert!(covered("git grep x", |_| false, &git));
        assert!(!covered("git grep $X", |_| false, &git));
        assert!(!covered("git grep *", |_| false, &git));
        assert!(!covered("git grep \"$(cat x)\"", |_| false, &git));
        // Nor does what a substitution runs get by.
        let rules = [rule("gh", Some("pr"))];
        assert!(covered("gh pr view \"$(git branch)\"", routine, &rules));
        assert!(!covered("gh pr view \"$(rm x)\"", routine, &rules));
        assert!(!covered("gh pr view $(git $X)", routine, &rules));
        // Nor one in a rule's own two words.
        assert!(!covered("npm $X", routine, &[rule("npm", Some("run"))]));
        assert!(!covered("$X run", routine, &[rule("npm", Some("run"))]));
    }

    #[test]
    fn a_line_no_rule_can_hold_gets_a_yes_to_itself_alone() {
        let routine = |c: &[String]| c.first().is_some_and(|p| p == "cargo");
        let whole = |l: &str| Standing::exactly(l);
        for line in [
            "rm -rf target",
            "sqlite3 -readonly x.db \"select 1\"",
            "echo `id`",
            "ls > out",
            "npm run x && rm -rf y",
            "npm $X",
            "$X run",
            "git grep $X",
            "git grep -Ovim x",
        ] {
            assert_eq!(needed(line, routine, &[]), Ok(vec![whole(line)]), "{line}");
            let rules = [whole(line)];
            assert!(covered(line, routine, &rules), "{line}");
            assert!(covered(&format!("  {line}\n"), routine, &rules), "{line}");
            assert_eq!(
                needed(line, routine, &rules),
                Ok(Vec::new()),
                "said already"
            );
            assert!(!covered(&format!("{line} x"), routine, &rules), "{line}");
        }
        assert_eq!(whole(" rm -rf x ").program, "rm -rf x");
        assert!(whole("x").whole);
        assert_eq!(whole("rm -rf target").line(), "`rm -rf target`");
        // It goes with one word's rules for a line that can have them.
        assert_eq!(
            needed("gh pr view \"$(git branch) $X\" 2>&1", |_| false, &[]),
            Ok(vec![rule("gh", Some("pr")), rule("git", Some("branch"))])
        );
        assert_eq!(needed("", routine, &[]), Err(Why::NotPlain));
        assert_eq!(needed("  ", routine, &[]), Err(Why::NotPlain));
        // A word that happens to be such a line runs another program.
        let rule = whole("foo bar");
        assert!(!covered("'foo bar'", routine, std::slice::from_ref(&rule)));
        assert!(!matches(&rule, &["foo bar".to_string()]));
        assert!(!covered("cargo", |_| false, &[whole("cargo x")]));
    }

    #[test]
    fn allow_names_a_rule_by_its_words() {
        assert_eq!(named("npm run"), Ok(rule("npm", Some("run"))));
        assert_eq!(named("make"), Ok(rule("make", None)));
        assert_eq!(
            named("npm run build"),
            Err(Why::OptionFirst("npm run build".into()))
        );
        assert_eq!(named("rm -rf"), Err(Why::OptionFirst("rm -rf".into())));
        assert_eq!(named("a && b"), Err(Why::NotPlain));
        assert_eq!(named(""), Err(Why::NotPlain));
        assert_eq!(named("npm $X"), Err(Why::NotPlain));
        assert_eq!(named("npm run $X"), Err(Why::NotPlain), "not said back");
        assert_eq!(named("npm run $(a)"), Err(Why::NotPlain));
    }
}
