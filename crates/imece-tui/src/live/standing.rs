//! A standing yes (#229): the person says yes from now on to a command a
//! project's agents ask about (`A` on the ask, `:allow`), so the next ask
//! that only runs such commands is allowed without them. A rule is a
//! program and its first argument (`npm run`, `dotnet ef`), as Claude
//! Code's `Bash(npm run:*)`; it never matches a command with an option
//! before that argument, and git's options that start a program
//! (`tools::git_denied`) never match at all. A line of several commands
//! (`&&`, `||`, `;`, `|`, a new line) is allowed when each of them is
//! routine or has its rule; a line with anything else a shell reads (`$`,
//! a backtick, a redirection, a glob) always asks.

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

/// The commands `line` runs, split where a shell runs one after or into
/// another (`&&`, `||`, `;`, `|`, a new line) outside quotes, each as its
/// words ([`run::split`]); none when a part is empty or has anything else
/// a shell would read.
pub(crate) fn commands(line: &str) -> Option<Vec<Vec<String>>> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.trim().chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), _) if c == q => {
                quote = None;
                part.push(c);
            }
            (Some('"'), '\\') | (None, '\\') => {
                part.push(c);
                part.extend(chars.next());
            }
            (Some(_), _) => part.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                part.push(c);
            }
            (None, '&') if chars.peek() == Some(&'&') => {
                chars.next();
                parts.push(std::mem::take(&mut part));
            }
            (None, '|') => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                parts.push(std::mem::take(&mut part));
            }
            (None, ';' | '\n') => parts.push(std::mem::take(&mut part)),
            (None, _) => part.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    parts.push(part);
    parts
        .iter()
        .map(|p| {
            Some(p.trim())
                .filter(|p| !p.is_empty())
                .and_then(|p| run::split(p).ok())
        })
        .collect()
}

/// The rule a yes from now on to the command `argv` makes: its program
/// and first argument, or the program alone when it has none.
pub(crate) fn rule_of(argv: &[String]) -> Result<Standing, Why> {
    match argv {
        [program] => Ok(Standing {
            program: program.clone(),
            sub: None,
        }),
        [program, sub, ..] if !sub.starts_with('-') => Ok(Standing {
            program: program.clone(),
            sub: Some(sub.clone()),
        }),
        _ => Err(Why::OptionFirst(argv.join(" "))),
    }
}

/// Whether the command `argv` is one `rule` says yes to: the same
/// program and the same first argument (none: the program alone), and
/// for git none of the options that start a program.
pub(crate) fn matches(rule: &Standing, argv: &[String]) -> bool {
    argv.first() == Some(&rule.program)
        && argv.get(1) == rule.sub.as_ref()
        && !(rule.program == "git" && argv.iter().any(|a| git_denied(a)))
}

/// Whether every command `line` runs is `routine` or one of `rules` says
/// yes to it.
pub(crate) fn covered(line: &str, routine: impl Fn(&[String]) -> bool, rules: &[Standing]) -> bool {
    commands(line).is_some_and(|all| {
        all.iter()
            .all(|c| routine(c) || rules.iter().any(|r| matches(r, c)))
    })
}

/// The rules a yes from now on to `line` adds to `rules`: one for each of
/// its commands that is not `routine` or said yes to already.
pub(crate) fn needed(
    line: &str,
    routine: impl Fn(&[String]) -> bool,
    rules: &[Standing],
) -> Result<Vec<Standing>, Why> {
    let all = commands(line).ok_or(Why::NotPlain)?;
    let mut out: Vec<Standing> = Vec::new();
    for c in &all {
        let said = |r: &Standing| matches(r, c);
        if routine(c) || rules.iter().any(said) || out.iter().any(said) {
            continue;
        }
        out.push(rule_of(c)?);
    }
    Ok(out)
}

/// The rule `:allow` names with `words` (`npm run`, `make`).
pub(crate) fn named(words: &str) -> Result<Standing, Why> {
    match commands(words).as_deref() {
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
            "echo $HOME",
            "echo `id`",
            "echo $(id)",
            "ls > out",
            "cat < in",
            "rm *.o",
            "echo 'open",
            "echo \"open",
            "a |",
            "(cd x)",
        ] {
            assert_eq!(got(refused), None, "{refused:?}");
        }
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
        assert_eq!(
            needed("npm run x && rm -rf y", routine, &rules),
            Err(Why::OptionFirst("rm -rf y".into()))
        );
        assert_eq!(needed("echo $X", routine, &rules), Err(Why::NotPlain));
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
    }
}
