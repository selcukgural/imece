//! The tests a repository's own files say it has (record 136): a Python,
//! JavaScript or Go checkout's test commands run without the person's yes
//! (routine, record 30) and through the run tool (record 10), as cargo's
//! do, so a run of them counts on the change the person reads.
//!
//! What the files can unlock is only a fixed list of test shapes for each
//! language, never an interpreter: `python3 -m pytest …`, `python3 -m
//! unittest …` and `pytest …` (Python), `npm test` and `npm run test` with
//! pnpm or yarn in npm's place when the lockfile says so, anything for the
//! script after `--` (JavaScript), `go test …` (Go). The options each takes
//! are an allow-list of those that only choose, show or stop tests; one
//! that loads a plugin, reads arguments or a configuration from a file,
//! writes a file or runs another program is refused (`pytest -p`, `-c`,
//! `@args`, `--basetemp`; `go test -exec`, `-o`, `-coverprofile`,
//! `-toolexec`; anything of npm's own before `--`). No word may name a path
//! outside the directory (`/…`, `~…`, a `..` part, also after `=`, `:` or
//! `,`), and `go test` takes packages as `./…` only. Pure: what a list of
//! files and a command mean, nothing of where they come from.

/// The JavaScript package manager a checkout's lockfile names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    Npm,
    Pnpm,
    Yarn,
}

impl Node {
    pub fn program(self) -> &'static str {
        match self {
            Node::Npm => "npm",
            Node::Pnpm => "pnpm",
            Node::Yarn => "yarn",
        }
    }
}

/// The test commands a checkout's files call for. None by default: what
/// is not found runs nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Runners {
    pub python: bool,
    pub node: Option<Node>,
    pub go: bool,
}

/// The files at the top of a checkout that make it a Python one; so does
/// a `requirements*.txt` there, and a test file anywhere ([`python_file`]).
const PYTHON_MARKS: [&str; 5] = [
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "tox.ini",
    "pytest.ini",
];

/// The interpreters `-m pytest` and `-m unittest` run with.
const PYTHONS: [&str; 2] = ["python3", "python"];

/// Whether `path` (relative to the checkout, `/` between its parts) says
/// the checkout is a Python one: a mark at its top, or a file named as
/// unittest and pytest find tests (`test_*.py`, `*_test.py`) anywhere.
fn python_file(path: &str) -> bool {
    let top = !path.contains('/');
    let name = path.rsplit('/').next().unwrap_or(path);
    let requirements = name.starts_with("requirements") && name.ends_with(".txt");
    let test = name.ends_with(".py") && (name.starts_with("test_") || name.ends_with("_test.py"));
    (top && (PYTHON_MARKS.contains(&name) || requirements)) || test
}

/// What the checkout whose files are `files` (as git lists them, relative
/// to its top) runs its tests with.
pub fn of(files: &[String]) -> Runners {
    let has = |name: &str| files.iter().any(|f| f == name);
    let node = has("package.json").then(|| {
        if has("pnpm-lock.yaml") {
            Node::Pnpm
        } else if has("yarn.lock") {
            Node::Yarn
        } else {
            Node::Npm
        }
    });
    Runners {
        python: files.iter().any(|f| python_file(f)),
        node,
        go: has("go.mod"),
    }
}

/// The options one test program takes here; every other is refused.
struct Options {
    /// Options that take no value.
    flags: &'static [&'static str],
    /// Letters of such options that may go together after one `-`
    /// (`-xvs`); none for a program that does not take them so.
    letters: &'static str,
    /// Options that take a value, as `NAME VALUE` or `NAME=VALUE`; a one
    /// letter one also as `-kVALUE` when [`letters`](Self::letters) go
    /// together.
    values: &'static [&'static str],
    /// Whether a word that is no option may stand: a test, a file, a
    /// directory, a package.
    plain: fn(&str) -> bool,
}

const PYTEST: Options = Options {
    flags: &[
        "--co",
        "--collect-only",
        "--disable-warnings",
        "--exitfirst",
        "--failed-first",
        "--ff",
        "--full-trace",
        "--last-failed",
        "--lf",
        "--new-first",
        "--nf",
        "--no-header",
        "--no-summary",
        "--quiet",
        "--runxfail",
        "--showlocals",
        "--stepwise",
        "--strict-markers",
        "--sw",
        "--verbose",
    ],
    letters: "lqsvx",
    values: &[
        "-k",
        "-m",
        "-n",
        "-r",
        "--color",
        "--durations",
        "--maxfail",
        "--tb",
    ],
    plain: |_| true,
};

const UNITTEST: Options = Options {
    flags: &[
        "--buffer",
        "--catch",
        "--failfast",
        "--locals",
        "--quiet",
        "--verbose",
    ],
    letters: "bcfqv",
    values: &[
        "-k",
        "-p",
        "-s",
        "-t",
        "--durations",
        "--pattern",
        "--start-directory",
        "--top-level-directory",
    ],
    plain: |_| true,
};

const GO_TEST: Options = Options {
    flags: &[
        "-benchmem",
        "-cover",
        "-failfast",
        "-json",
        "-race",
        "-short",
        "-v",
    ],
    letters: "",
    values: &[
        "-bench",
        "-benchtime",
        "-count",
        "-covermode",
        "-cpu",
        "-list",
        "-p",
        "-parallel",
        "-run",
        "-shuffle",
        "-skip",
        "-tags",
        "-timeout",
    ],
    plain: |w| w == "." || w.starts_with("./"),
};

/// Whether `word` names nothing outside the directory a test runs in: no
/// path from the root or a home (`/…`, `~…`, also after `=`, `:` or `,`),
/// no `..` part, and no `@file` to read more arguments from.
pub fn inside(word: &str) -> bool {
    let away = word
        .split(['=', ':', ','])
        .any(|p| p.starts_with(['/', '~', '\\']));
    let up = word.split(['/', '\\', '=', ':', ',']).any(|p| p == "..");
    !word.starts_with('@') && !away && !up
}

/// Why `word` is not run through imece: it names a path outside.
fn outside(word: &str) -> String {
    format!(
        "{word}: a path outside your directory, or a file of arguments, is not run \
         through imece"
    )
}

/// Whether `args`, the words after a test program, are ones it runs
/// here with: each [`inside`], each option `opts` allows, a value after
/// each option that takes one.
fn words(args: &[String], opts: &Options) -> Result<(), String> {
    if let Some(w) = args.iter().find(|w| !inside(w)) {
        return Err(outside(w));
    }
    let mut rest = args.iter();
    while let Some(w) = rest.next() {
        if !w.starts_with('-') {
            if !(opts.plain)(w) {
                return Err(format!("{w}: name packages as ./… here"));
            }
            continue;
        }
        // Go takes `--name` as `-name`.
        let name = match (opts.letters.is_empty(), w.strip_prefix("--")) {
            (true, Some(n)) => format!("-{n}"),
            _ => w.clone(),
        };
        let (name, given) = match name.split_once('=') {
            Some((n, _)) => (n.to_string(), true),
            None => (name, false),
        };
        if opts.flags.contains(&name.as_str()) && !given {
            continue;
        }
        if opts.values.contains(&name.as_str()) {
            if !given && rest.next().is_none() {
                return Err(format!("{name} takes a value"));
            }
            continue;
        }
        if !opts.letters.is_empty() && !given {
            if together(&name, opts) {
                continue;
            }
            if let Some(value) = joined(&name, opts) {
                if !inside(value) {
                    return Err(outside(value));
                }
                continue;
            }
        }
        return Err(format!(
            "{w}: not an option imece runs this repository's tests with"
        ));
    }
    Ok(())
}

/// Whether `word` is letters of options that take no value, together
/// after one `-` (`-xvs`).
fn together(word: &str, opts: &Options) -> bool {
    word.strip_prefix('-')
        .is_some_and(|l| !l.is_empty() && l.chars().all(|c| opts.letters.contains(c)))
}

/// The value joined to a one letter option that takes one (`-ra`,
/// `-kcart`).
fn joined<'a>(word: &'a str, opts: &Options) -> Option<&'a str> {
    let letters = word.strip_prefix('-')?;
    (opts.values.iter())
        .filter_map(|v| v.strip_prefix('-').filter(|l| l.len() == 1))
        .find_map(|l| letters.strip_prefix(l).filter(|value| !value.is_empty()))
}

/// Whether `args`, the words after `npm test`, are none, or `--` and what
/// goes to the test script.
fn script(args: &[String]) -> Result<(), String> {
    match args {
        [] => Ok(()),
        [dashes, rest @ ..] if dashes == "--" => match rest.iter().find(|w| !inside(w)) {
            Some(w) => Err(outside(w)),
            None => Ok(()),
        },
        _ => Err(
            "words for the test script go after --; the package manager's own \
                  options are not run through imece"
                .into(),
        ),
    }
}

impl Runners {
    /// Whether there are any.
    pub fn any(&self) -> bool {
        self.python || self.node.is_some() || self.go
    }

    /// The commands they are, as the person and an agent read them.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.python {
            out.extend(
                ["python3 -m pytest …", "python3 -m unittest …", "pytest …"].map(String::from),
            );
        }
        if let Some(n) = self.node {
            let p = n.program();
            out.push(format!("{p} test"));
            out.push(format!("{p} run test"));
        }
        if self.go {
            out.push("go test …".to_string());
        }
        out
    }

    /// Whether `argv` is one of these tests: none when its program is
    /// none of theirs, else why not when it is not.
    pub fn check(&self, argv: &[String]) -> Option<Result<(), String>> {
        let (program, rest) = argv.split_first()?;
        let p = program.as_str();
        if self.python && PYTHONS.contains(&p) {
            return Some(match rest {
                [m, module, args @ ..] if m == "-m" && module == "pytest" => words(args, &PYTEST),
                [m, module, args @ ..] if m == "-m" && module == "unittest" => {
                    words(args, &UNITTEST)
                }
                _ => Err(format!(
                    "{p} runs here only as {p} -m pytest or {p} -m unittest, this \
                     repository's tests"
                )),
            });
        }
        if self.python && p == "pytest" {
            return Some(words(rest, &PYTEST));
        }
        if let Some(n) = self.node.filter(|n| n.program() == p) {
            return Some(match rest {
                [t, args @ ..] if t == "test" => script(args),
                [r, t, args @ ..] if r == "run" && t == "test" => script(args),
                _ => Err(format!(
                    "{p} runs here only as {p} test or {p} run test, this repository's \
                     tests",
                    p = n.program()
                )),
            });
        }
        if self.go && p == "go" {
            return Some(match rest {
                [t, args @ ..] if t == "test" => words(args, &GO_TEST),
                _ => Err("go runs here only as go test, this repository's tests".into()),
            });
        }
        None
    }

    /// Whether `argv` is one of these tests.
    pub fn runs(&self, argv: &[String]) -> bool {
        self.check(argv) == Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn argv(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_string).collect()
    }

    const ALL: Runners = Runners {
        python: true,
        node: Some(Node::Npm),
        go: true,
    };

    #[test]
    fn a_checkout_s_files_say_which_tests_it_has() {
        assert_eq!(of(&[]), Runners::default());
        assert!(!Runners::default().any());
        let py = Runners {
            python: true,
            ..Runners::default()
        };
        for mark in [
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "tox.ini",
            "pytest.ini",
            "requirements.txt",
            "requirements-dev.txt",
            "tests/test_cart.py",
            "test_cart.py",
            "pkg/cart_test.py",
        ] {
            assert_eq!(of(&files(&["README.md", mark])), py, "{mark}");
            assert!(py.any());
        }
        for not in [
            "docs/pyproject.toml",
            "docs/setup.py",
            "tools/requirements.txt",
            "requirements.md",
            "requirements",
            "my-requirements.txt",
            "cart.py",
            "tests/cart.py",
            "tests/test_cart.pyc",
            "test_cart.txt",
            "testcart.py",
            "pyproject.toml.bak",
        ] {
            assert_eq!(of(&files(&[not])), Runners::default(), "{not}");
        }
        let node = |n| Runners {
            node: Some(n),
            ..Runners::default()
        };
        assert_eq!(of(&files(&["package.json"])), node(Node::Npm));
        assert_eq!(
            of(&files(&["package.json", "package-lock.json"])),
            node(Node::Npm)
        );
        assert_eq!(of(&files(&["package.json", "yarn.lock"])), node(Node::Yarn));
        assert_eq!(
            of(&files(&["package.json", "pnpm-lock.yaml"])),
            node(Node::Pnpm)
        );
        assert_eq!(
            of(&files(&["yarn.lock", "package.json", "pnpm-lock.yaml"])),
            node(Node::Pnpm),
            "pnpm's lockfile first"
        );
        assert_eq!(
            of(&files(&["web/package.json", "yarn.lock"])),
            Runners::default()
        );
        assert!(node(Node::Yarn).any());
        let go = Runners {
            go: true,
            ..Runners::default()
        };
        assert_eq!(of(&files(&["go.mod", "main.go"])), go);
        assert_eq!(of(&files(&["api/go.mod"])), Runners::default());
        assert!(go.any());
        assert_eq!(
            of(&files(&["go.mod", "package.json", "setup.py"])),
            ALL,
            "all three"
        );
    }

    #[test]
    fn the_tests_are_named_as_an_agent_runs_them() {
        assert_eq!(Runners::default().lines(), Vec::<String>::new());
        assert_eq!(
            ALL.lines(),
            [
                "python3 -m pytest …",
                "python3 -m unittest …",
                "pytest …",
                "npm test",
                "npm run test",
                "go test …"
            ]
        );
        let yarn = Runners {
            node: Some(Node::Yarn),
            ..Runners::default()
        };
        assert_eq!(yarn.lines(), ["yarn test", "yarn run test"]);
        assert_eq!(Node::Pnpm.program(), "pnpm");
        assert_eq!(Node::Npm.program(), "npm");
        assert_eq!(Node::Yarn.program(), "yarn");
    }

    #[test]
    fn nothing_runs_for_a_language_the_files_do_not_name() {
        let none = Runners::default();
        for line in [
            "python3 -m pytest",
            "python -m unittest",
            "pytest",
            "npm test",
            "go test ./...",
            "cargo test",
        ] {
            assert_eq!(none.check(&argv(line)), None, "{line}");
            assert!(!none.runs(&argv(line)), "{line}");
        }
        let py = Runners {
            python: true,
            ..Runners::default()
        };
        assert_eq!(py.check(&argv("npm test")), None);
        assert_eq!(py.check(&argv("go test")), None);
        assert_eq!(py.check(&[]), None);
        let npm = Runners {
            node: Some(Node::Npm),
            ..Runners::default()
        };
        assert_eq!(npm.check(&argv("yarn test")), None, "not its lockfile's");
        assert_eq!(npm.check(&argv("pnpm test")), None);
        assert_eq!(npm.check(&argv("pytest")), None);
    }

    #[test]
    fn python_runs_its_test_runners_never_the_interpreter() {
        for line in [
            "python3 -m pytest",
            "python -m pytest",
            "pytest",
            "python3 -m unittest",
            "python -m unittest",
            "python3 -m pytest tests/test_cart.py::test_total -x -q",
            "pytest -xvs tests",
            "pytest -k cart -m slow -n 4 --maxfail 2 --tb=short --durations=5",
            "pytest -ra -rA -kcart --color=no --lf --sw",
            "pytest --tb short --co",
            "python3 -m unittest -v tests.test_cart",
            "python3 -m unittest discover -s tests -p test_*.py -t .",
            "python3 -m unittest -vfb -k cart --durations 3 --locals",
            "python3 -m unittest discover --start-directory=tests --pattern=x",
        ] {
            assert_eq!(ALL.check(&argv(line)), Some(Ok(())), "{line}");
            assert!(ALL.runs(&argv(line)), "{line}");
        }
        for line in [
            "python3",
            "python3 -c print(1)",
            "python3 script.py",
            "python3 -m",
            "python3 -m pip install x",
            "python3 -m http.server",
            "python3 -I -m pytest",
            "python3 -m pytest_x",
            "python3.12 -m pytest",
        ] {
            let got = ALL.check(&argv(line));
            if line.starts_with("python3.12") {
                assert_eq!(got, None, "another interpreter is not one of these");
                continue;
            }
            let e = got.expect(line).unwrap_err();
            assert!(
                e.contains("only as python3 -m pytest or python3 -m unittest"),
                "{e}"
            );
        }
        let e = ALL.check(&argv("python x.py")).unwrap().unwrap_err();
        assert!(e.contains("only as python -m pytest"), "{e}");
    }

    #[test]
    fn an_option_that_loads_reads_or_writes_more_is_refused() {
        for line in [
            "pytest -p evil",
            "pytest -pevil",
            "pytest -c other.ini",
            "pytest -o addopts=-pevil",
            "pytest -W error::evil.Warning",
            "pytest --basetemp=tmp",
            "pytest --rootdir=x",
            "pytest --junitxml=out.xml",
            "pytest --confcutdir=x",
            "pytest --pyargs os",
            "pytest --override-ini=x",
            "pytest --pdb",
            "pytest -xp evil",
            "pytest --",
            "pytest --quiet=1",
            "pytest --lf=x",
            "pytest -",
            "pytest --x",
            "python3 -m unittest -h",
            "python3 -m unittest --catch=x",
            "go test -exec evil ./...",
            "go test -o bin ./...",
            "go test -c ./...",
            "go test -toolexec evil",
            "go test -ldflags=-extld=evil",
            "go test -coverprofile=c.out",
            "go test -args -test.v",
            "go test -vx",
            "go test -v=true",
            "go test -modfile=x.mod",
            "go test -C sub",
            "go test -p4",
            "pytest -xv=1",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.contains("not an option imece runs"), "{line}: {e}");
        }
        for line in [
            "pytest -k",
            "pytest --tb",
            "go test -run",
            "python3 -m unittest -s",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.ends_with("takes a value"), "{line}: {e}");
        }
        // Go takes either dash, one or two; a value given with `=` or after.
        for line in [
            "go test",
            "go test ./...",
            "go test .",
            "go test --run TestX ./cmd/...",
            "go test -run=TestX -count 1 -race -v -timeout 30s ./...",
            "go test --v -short -json -failfast -cover -benchmem -p 2 ./internal/...",
        ] {
            assert_eq!(ALL.check(&argv(line)), Some(Ok(())), "{line}");
        }
        assert_eq!(
            ALL.check(&argv("pytest -x")),
            Some(Ok(())),
            "one letter alone"
        );
    }

    #[test]
    fn no_word_reaches_outside_the_directory() {
        for word in [
            "/etc",
            "~/x",
            "~",
            "\\x",
            "..",
            "../x",
            "a/../../b",
            "a\\..\\b",
            "@args.txt",
            "--rootdir=/x",
            "--x=../y",
            "x::..",
            "a,/b",
            "a:~/b",
        ] {
            assert!(!inside(word), "{word}");
        }
        for word in [
            "tests",
            "./tests/a.py",
            "tests/a.py::T::t",
            "a..b",
            "...",
            "./...",
            "x=y",
            "a@b",
            "-k",
            "",
            "HEAD~1",
        ] {
            assert!(inside(word), "{word}");
        }
        for line in [
            "pytest /etc",
            "pytest ../other",
            "pytest @args",
            "python3 -m unittest discover -s ../x",
            "python3 -m unittest discover -t /",
            "python3 -m unittest discover -s../x",
            "python3 -m unittest discover -s/etc",
            "pytest -k~",
            "pytest --tb=../x",
            "go test ../...",
            "go test -run /x",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.contains("outside your directory"), "{line}: {e}");
        }
        for line in [
            "go test pkg",
            "go test example.com/m/x",
            "go test ...",
            "go test .x",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.contains("as ./… here"), "{line}: {e}");
        }
    }

    #[test]
    fn a_package_manager_runs_the_test_script_and_words_after_dashes_only() {
        for line in [
            "npm test",
            "npm run test",
            "npm test -- --watch=false",
            "npm run test -- -t cart src/cart.test.ts",
        ] {
            assert_eq!(ALL.check(&argv(line)), Some(Ok(())), "{line}");
        }
        for line in [
            "npm",
            "npm install",
            "npm run build",
            "npm run",
            "npm t",
            "npm run-script test",
            "npm exec x",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(
                e.contains("only as npm test or npm run test"),
                "{line}: {e}"
            );
        }
        for line in [
            "npm test --script-shell=/x",
            "npm test --prefix x",
            "npm test cart",
            "npm run test --workspace x",
        ] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.contains("after --"), "{line}: {e}");
        }
        for line in ["npm test -- /etc", "npm test -- --config=../x"] {
            let e = ALL.check(&argv(line)).expect(line).unwrap_err();
            assert!(e.contains("outside your directory"), "{line}: {e}");
        }
        let yarn = Runners {
            node: Some(Node::Yarn),
            ..Runners::default()
        };
        assert!(yarn.runs(&argv("yarn test")));
        assert!(yarn.runs(&argv("yarn run test -- x")));
        let e = yarn.check(&argv("yarn add x")).unwrap().unwrap_err();
        assert!(e.contains("only as yarn test or yarn run test"), "{e}");
        let pnpm = Runners {
            node: Some(Node::Pnpm),
            ..Runners::default()
        };
        assert!(pnpm.runs(&argv("pnpm test")));
        assert_eq!(pnpm.check(&argv("npm test")), None);
        let e = ALL.check(&argv("go build ./...")).unwrap().unwrap_err();
        assert!(e.contains("only as go test"), "{e}");
        assert!(ALL.check(&argv("go")).unwrap().is_err());
    }
}
