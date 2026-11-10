use super::*;

#[test]
fn the_doctor_window_shows_the_report_and_says_while_it_looks() {
    use crate::app::DoctorWin;
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    app.command("doctor");
    let s = render(&app, 120, 40);
    let p = app.current_project();
    let folder = app.project_folder(p).unwrap();
    assert!(s.contains(&format!(" DOCTOR · {folder} ")), "{s}");
    // The category with an issue is open, what to do under its finding;
    // the others are a line each.
    for want in [
        " [!] imece",
        "     ✓ this imece holds the store",
        "     ! 800 MB free where the state is; a worker's worktree is a copy of the",
        "       free some space; :tidy in imece removes worktrees no agent owns",
        " [✓] Claude Code: ",
        " [✓] Lonca: ",
        " [✓] Terminal: 100×40",
        " ! Doctor found issues in 1 category.",
        " j k  scroll   v  every finding   r  look again   esc  back",
    ] {
        assert!(s.contains(want), "{want} missing:\n{s}");
    }
    assert!(!s.contains("Doctor summary"), "the title says it: {s}");
    assert!(
        !s.contains("24-bit colour"),
        "a category with no issue is one line"
    );
    assert_eq!(
        app.doctor_room.get().1,
        33,
        "40 rows less the frame and the keys"
    );
    // Every finding.
    app.doctor = Some(DoctorWin {
        top: 0,
        verbose: true,
    });
    let s = render(&app, 120, 40);
    assert!(s.contains("     ✓ 24-bit colour"), "{s}");
    assert!(s.contains("     • no ~/.config/imece/push: "), "{s}");
    let (lines, room) = app.doctor_room.get();
    assert!(lines > 25 && room == 33, "{lines} {room}");
    // A short screen shows from the line it is at, never past the end.
    let short = |app: &App| render(app, 120, 16);
    let s = short(&app);
    assert!(
        s.contains("[!] imece") && !s.contains("Doctor found issues"),
        "{s}"
    );
    let (lines, room) = app.doctor_room.get();
    assert_eq!(room, 9);
    app.doctor = Some(DoctorWin {
        top: 3,
        verbose: true,
    });
    assert!(!short(&app).contains("[!] imece"), "scrolled away");
    app.doctor = Some(DoctorWin {
        top: lines + 50,
        verbose: true,
    });
    let s = short(&app);
    assert!(
        s.contains("Doctor found issues in 1 category."),
        "the end stays: {s}"
    );
    // A narrow window wraps a long finding under its own words.
    app.doctor = Some(DoctorWin::default());
    let s = render(&app, 60, 40);
    assert!(
        s.contains("     ! 800 MB free where the state is; a worker's"),
        "{s}"
    );
    assert!(
        s.contains("       worktree is a copy of the checkout's files"),
        "{s}"
    );
    // In Turkish, from the same facts.
    app.lang = Lang::Tr;
    let s = render(&app, 120, 40);
    assert!(
        s.contains(" DOKTOR · ") && s.contains("Doktor 1 başlıkta sorun buldu."),
        "{s}"
    );
    assert!(
        s.contains(" j k  kaydır   v  her bulgu   r  yeniden bak   esc  geri"),
        "{s}"
    );
    app.lang = Lang::En;
    // Nothing known yet: it says whom it asks; a newer look is said at
    // the keys, what was found staying.
    app.doctor_facts = None;
    let s = render(&app, 120, 40);
    assert!(s.contains(" DOCTOR · … "), "{s}");
    assert!(s.contains(" asking Claude Code, git and Lonca…"), "{s}");
    assert_eq!(app.doctor_room.get(), (1, 33));
}

#[test]
fn a_program_s_words_on_the_doctor_window_can_not_drive_the_terminal() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    app.command("doctor");
    let facts = app.doctor_facts.as_mut().unwrap();
    facts.checkout = "~/x\u{1b}]0;owned\u{7}".into();
    facts.git.version = Some("git \u{1b}[2Jversion 2.47.0".into());
    app.doctor = Some(crate::app::DoctorWin {
        top: 0,
        verbose: true,
    });
    let s = render(&app, 120, 40);
    assert!(!s.contains('\u{1b}') && !s.contains('\u{7}'), "{s:?}");
    assert!(s.contains("version 2.47.0"), "{s}");
}

#[test]
fn the_ps_window_sums_each_session_and_asks_before_one_ends() {
    use crate::procs::{Proc, Procs};
    let mut app = app_after(1, 2, 0, Lang::En);
    app.command("ps");
    let p = |pid, ppid, cpu, kib, name: &str| Proc {
        pid,
        ppid,
        cpu,
        rss: kib,
        name: name.into(),
    };
    app.world.procs = Procs::default();
    let s = render(&app, 120, 30);
    assert!(s.contains("PROCESSES · 0"), "{s}");
    assert!(s.contains("no agent has a session running"), "{s}");
    app.world.procs.looking = true;
    assert!(render(&app, 120, 30).contains("looking…"));
    app.world.procs = Procs {
        agents: vec![(
            1,
            vec![
                p(500, 1, 123, 300 * 1024, "claude"),
                p(501, 500, 0, 9 * 1024, "imece"),
                p(502, 500, 950, 400 * 1024, "cargo\u{1b}[2J"),
            ],
        )],
        looking: true,
        failed: None,
        left: Vec::new(),
    };
    app.ps = Some(crate::app::PsWin { at: 2, sure: None });
    let s = render(&app, 120, 30);
    assert!(s.contains("PROCESSES · 3"), "{s}");
    assert!(!s.contains("looking…"), "what was read shows meanwhile");
    let row = |s: &str, name: &str| {
        s.lines()
            .find(|l| l.contains(name))
            .unwrap_or_default()
            .to_string()
    };
    assert!(s.contains("│claude-1 · 107.3% · 709 MiB "), "{s}");
    let root = row(&s, "  claude ");
    assert!(
        root.contains("  claude           500   12.3%  300 MiB"),
        "{root:?}"
    );
    assert!(!root.contains('▶'));
    let relay = row(&s, "  imece   ");
    assert!(
        relay.contains("    imece          501    0.0%  9 MiB"),
        "{relay:?}"
    );
    let cmd = row(&s, "▶");
    assert!(
        cmd.contains("▶   cargo [2J      502   95.0%  400 MiB"),
        "{cmd:?}"
    );
    let column = |l: &str, pid: &str| l[..l.find(pid).unwrap()].chars().count();
    assert_eq!(
        column(&root, "500"),
        column(&cmd, "502"),
        "the columns line up"
    );
    assert!(s.contains("end this one"), "{s}");
    app.ps = Some(crate::app::PsWin {
        at: 2,
        sure: Some(crate::app::Sure::One(crate::app::Whose::Agent(1), 502)),
    });
    let s = render(&app, 120, 30);
    assert!(
        s.contains("end cargo [2J (502) under claude-1? y ends it, any other key keeps it"),
        "{s}"
    );
    assert!(!s.contains("end this one"), "{s}");
    app.world.procs.failed = Some("ps did not answer".into());
    let s = render(&app, 120, 30);
    assert!(
        s.contains("the processes could not be read: ps did not answer"),
        "{s}"
    );
    // On a short screen the row at hand stays in view.
    app.ps = Some(crate::app::PsWin { at: 2, sure: None });
    let s = render(&app, 120, MIN_ROWS);
    assert!(s.contains("▶"), "{s}");
    // What an earlier run left, under its own heading, `a` named.
    app.world.procs.failed = None;
    assert!(!render(&app, 120, 30).contains("end all left over"));
    app.world.procs.left = vec![
        crate::procs::Left {
            place: "imece-b6-notes/claude-3".into(),
            procs: vec![
                p(700, 1, 40, 200 * 1024, "claude"),
                p(701, 700, 900, 100 * 1024, "node"),
            ],
        },
        crate::procs::Left {
            place: String::new(),
            procs: vec![p(800, 1, 0, 1024, "sh")],
        },
    ];
    app.ps = Some(crate::app::PsWin { at: 4, sure: None });
    let s = render(&app, 120, 30);
    assert!(s.contains("PROCESSES · 6"), "{s}");
    let at = |s: &str, text: &str| s.lines().position(|l| l.contains(text));
    let head = at(&s, "LEFT FROM AN EARLIER RUN · 3").expect("its heading");
    assert!(at(&s, "  cargo").unwrap() < head, "{s}");
    assert!(
        s.contains("│imece-b6-notes/claude-3 · 94.0% · 300 MiB"),
        "{s}"
    );
    assert!(s.contains("│? · 0.0% · 1 MiB"), "{s}");
    let node = row(&s, "node");
    assert!(node.contains("▶   node"), "{node:?}");
    assert!(at(&s, "▶").unwrap() > head);
    assert!(s.contains("end all left over"), "{s}");
    app.ps = Some(crate::app::PsWin {
        at: 4,
        sure: Some(crate::app::Sure::One(crate::app::Whose::Left(0), 701)),
    });
    let s = render(&app, 120, 30);
    assert!(
        s.contains("end node (701), left from an earlier run? y ends it"),
        "{s}"
    );
    app.ps = Some(crate::app::PsWin {
        at: 4,
        sure: Some(crate::app::Sure::AllLeft),
    });
    let s = render(&app, 120, 30);
    assert!(
        s.contains("end all 3 processes left from an earlier run? y ends them"),
        "{s}"
    );
    // No agent's session, and what was left: both said.
    app.world.procs.agents.clear();
    app.world.procs.looking = false;
    let s = render(&app, 120, 30);
    assert!(s.contains("no agent has a session running"), "{s}");
    assert!(s.contains("PROCESSES · 3"), "{s}");
    assert_eq!(overlay::cpu(0), "0.0%");
    assert_eq!(overlay::cpu(1009), "100.9%");
}

#[test]
fn the_tidy_window_lists_the_worktrees_no_agent_owns_and_asks_before_work_goes() {
    let mut app = app_after(1, 1, 0, Lang::En);
    app.open_tidy();
    let s = render(&app, 120, 30);
    assert!(s.contains("WORKTREES NO AGENT OWNS · 0"), "{s}");
    assert!(s.contains("looking…"), "{s}");
    app.step();
    let s = render(&app, 120, 30);
    assert!(s.contains("WORKTREES NO AGENT OWNS · 2"), "{s}");
    assert!(!s.contains("looking…"), "{s}");
    let row = |s: &str, name: &str| {
        s.lines()
            .find(|l| l.contains(name))
            .map(str::to_string)
            .unwrap_or_default()
    };
    let first = row(&s, "claude-31");
    assert!(first.contains("▶ claude-31"), "{first}");
    assert!(first.contains("~/src/"), "{first}");
    assert!(first.contains("clean"), "{first}");
    let second = row(&s, "claude-32");
    assert!(second.contains("  claude-32 "), "{second}");
    assert!(!second.contains('▶'), "{second}");
    assert!(
        second.contains("files changed: 3 · commits of its own: 1"),
        "{second}"
    );
    assert!(s.contains("remove the clean ones"), "{s}");
    app.tidy = Some(crate::app::TidyWin { at: 1, sure: true });
    let s = render(&app, 140, 30);
    assert!(row(&s, "▶").contains("claude-32"), "{s}");
    assert!(
        s.contains("remove claude-32 with what it holds (files changed: 3 · commits of its own: 1)? y removes it"),
        "{s}"
    );
    assert!(!s.contains("remove the clean ones"), "{s}");
    // Going: said; all gone: said so.
    app.tidy = Some(crate::app::TidyWin::default());
    app.world.orphans.busy = true;
    assert!(render(&app, 120, 30).contains("removing…"));
    app.world.orphans = crate::model::Orphans {
        found: Some(Vec::new()),
        ..Default::default()
    };
    let s = render(&app, 120, 30);
    assert!(
        s.contains("none: every worktree under imece's state has its agent"),
        "{s}"
    );
    assert!(!s.contains("removing…"), "{s}");
}

#[test]
fn the_lonca_window_lists_what_waits_for_a_human_and_what_is_held() {
    use crate::app::LoncaWin;
    use crate::model::{Role, StoreLease, StorePending, StoreProposal};
    let mut app = app_after(2, 2, 0, Lang::En);
    let (name, repo) = (
        app.world.projects[1].name.clone(),
        app.world.projects[1].name.clone(),
    );
    let id = (app.world.agents.iter())
        .find(|a| a.project == 1 && a.role == Role::Worker)
        .map(|a| a.id)
        .unwrap();
    let worker = app.world.agents[id].name.clone();
    app.world.agents[id].workspace = Some(repo.clone());
    let mut change = a_change(id);
    change.files = vec![ChangedFile {
        path: "x.kt".into(),
        added: 1,
        removed: 0,
        binary: false,
    }];
    app.world.change_proposed(
        change,
        StoreProposal {
            seq: 4,
            change: "demo2".into(),
            why: Vec::new(),
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
            intent: String::new(),
        },
    );
    app.selected = crate::app::Sel::Project(1);
    app.step();
    let lease = |symbol: &str, holder, who: &str| StoreLease {
        file: "app/cart/CartViewModel.kt".into(),
        symbol: symbol.into(),
        holder,
        name: who.into(),
        shared: false,
    };
    app.world.store_leases(
        1,
        &repo,
        vec![
            lease("CartViewModel.add", Some(id), "x"),
            lease("CartViewModel.total\u{1b}[31m", None, "someone"),
        ],
    );
    app.lonca_win = Some(LoncaWin::default());
    let s = render(&app, 120, 40);
    assert!(s.contains(&format!("LONCA · {repo}")), "{s}");
    assert!(
        s.contains(&format!(
            "{name} is on Lonca repository {repo}: trunk 9d3f1c2, 2 waiting for a human"
        )),
        "{s}"
    );
    assert!(s.contains("WAITING FOR A HUMAN · 2"), "{s}");
    let row = |s: &str, word: &str| {
        (s.lines()
            .find(|l| l.contains(word))
            .map(|l| l.trim_end().to_string()))
        .unwrap_or_else(|| panic!("no row with {word}: {s}"))
    };
    let shown = row(&s, "Δ shown");
    assert!(
        shown.contains(&format!("▶    4  {worker}  x.kt  Δ shown")),
        "{shown}"
    );
    assert!(!shown.contains("based before trunk"));
    let stranger = row(&s, "ayse");
    let pad = " ".repeat(worker.chars().count() - 4);
    assert!(
        stranger.contains(&format!(
            "     1  ayse{pad}  app/cart/CartViewModel.kt  based before trunk"
        )),
        "{stranger}"
    );
    assert!(!stranger.contains('▶'));
    assert!(s.contains("HELD · 2"), "{s}");
    assert!(
        s.contains(&format!(
            "  app/cart/CartViewModel.kt · CartViewModel.add · {worker}"
        )),
        "{s}"
    );
    let total = clean("CartViewModel.total\u{1b}[31m");
    assert!(
        s.contains(&format!("  app/cart/CartViewModel.kt · {total} · someone")),
        "cleaned: {s}"
    );
    assert!(s.contains("move") && s.contains("open the change") && s.contains("back"));
    // The row at hand marked.
    app.lonca_win = Some(LoncaWin { at: 1 });
    let s = render(&app, 120, 40);
    assert!(
        row(&s, "ayse").starts_with('▶') || row(&s, "ayse").contains("▶    1"),
        "{s}"
    );
    assert!(!row(&s, "Δ shown").contains('▶'));
    // In Turkish.
    app.lang = Lang::Tr;
    let s = render(&app, 120, 40);
    for word in [
        "İNSAN BEKLEYEN · 2",
        "TUTULAN · 2",
        "Δ gösteriliyor",
        "temeli gövdenin gerisinde",
        "değişikliği aç",
    ] {
        assert!(s.contains(word), "{word}: {s}");
    }
    // Nothing waiting, nothing held: said.
    app.lang = Lang::En;
    app.world.store_pending(1, &repo, Vec::new());
    app.world.store_leases(1, &repo, Vec::new());
    app.lonca_win = Some(LoncaWin::default());
    let s = render(&app, 120, 40);
    assert!(
        s.contains("WAITING FOR A HUMAN · 0") && s.contains("nothing waits for a human"),
        "{s}"
    );
    assert!(
        s.contains("HELD · 0") && s.contains("nothing is held"),
        "{s}"
    );
    // On a short screen the row at hand stays in view.
    let many = (10..=15)
        .map(|seq| StorePending {
            seq,
            change: format!("c{seq}"),
            name: format!("p{seq}"),
            holder: None,
            paths: Vec::new(),
            on_trunk: true,
        })
        .collect();
    app.world.store_pending(1, &repo, many);
    app.lonca_win = Some(LoncaWin { at: 5 });
    let s = render(&app, 100, 12);
    assert!(s.contains("▶   15  p15"), "{s}");
    assert!(!s.contains("  10  p10"), "scrolled away: {s}");
    app.lonca_win = Some(LoncaWin { at: 0 });
    let s = render(&app, 100, 12);
    assert!(s.contains("▶   10  p10") && !s.contains("p15"), "{s}");
}

#[test]
fn the_ledger_window_says_each_project_s_figures_and_its_attempts_newest_first() {
    use crate::ledger::{Attempt, Outcome};
    let mut app = app_after(2, 1, 0, Lang::En);
    let w = &mut app.world;
    let p = w.salon_order()[0];
    let other = w.salon_order()[1];
    let lead = w.projects[p].leader;
    let name = w.agents[lead].name.clone();
    w.agents[lead].cost = 2_500;
    let c = crate::live::testing::with_card(w, lead);
    w.cards[c].project = p;
    w.ledger = vec![
        Attempt {
            project: p,
            card: Some(c),
            attempt: 2,
            agent: lead,
            model: "claude-opus-5-5".into(),
            cost: 8_400,
            rounds: 3,
            sent_back: 2,
            outcome: Outcome::Late,
            began: 0,
            ended: 12,
            version: Some("2.1.4 (Claude Code)".into()),
            ..Attempt::default()
        },
        Attempt {
            project: p,
            agent: lead,
            cost: 700,
            unpriced: true,
            rounds: 1,
            outcome: Outcome::Applied,
            began: 0,
            ended: 11,
            ..Attempt::default()
        },
    ];
    app.command("ledger");
    assert_eq!(app.ledger_win, Some(0));
    let s = render(&app, 140, 30);
    assert!(s.contains("LEDGER · 2 attempts"), "{s}");
    assert!(!s.contains("API-equivalent"), "{s}");
    assert!(
        s.contains("applied $0.07+? · wasted $0.84 · at work $0.00 · lead $0.25"),
        "{s}"
    );
    let row = |s: &str, word: &str| {
        s.lines()
            .find(|l| l.contains(word))
            .unwrap_or("")
            .to_string()
    };
    let late = row(&s, "late done");
    assert!(
        late.contains(&format!(
            "  #{} try 2  {name}  claude-opus-5-5  3 Δ · 2 sent back  $0.84  1 min  late done",
            c + 1
        )),
        "{s}"
    );
    assert!(
        late.ends_with(" · 2.1.4") || late.contains(" · 2.1.4 "),
        "{late}"
    );
    assert!(!late.contains("(Claude Code)"), "{late}");
    let applied = row(&s, "  applied  ");
    assert!(
        applied.contains(&format!("▶ no card  {name}  1 Δ  $0.07+?  55 s  applied")),
        "{s}"
    );
    assert!(
        !late.contains('▶') && !applied.contains("sent back"),
        "{applied}"
    );
    // Newest first: the row with no card is above.
    assert!(
        s.find("no card").unwrap() < s.find("late done").unwrap(),
        "{s}"
    );
    // A project nothing was spent in says so.
    let other_name = app.world.projects[other].name.clone();
    assert!(s.contains(&format!("{other_name}  applied $0.00")), "{s}");
    assert!(s.contains("nothing spent on a card yet"), "{s}");
    assert!(s.contains("open the card"), "{s}");
    // On a plan: API-equivalent, every figure ≈.
    app.world.estimated = true;
    let s = render(&app, 140, 30);
    assert!(s.contains("LEDGER · 2 attempts · API-equivalent"), "{s}");
    assert!(
        s.contains("≈$0.84") && s.contains("applied ≈$0.07+?"),
        "{s}"
    );
    // In Turkish.
    app.lang = Lang::Tr;
    let s = render(&app, 140, 30);
    for word in [
        "HARCAMALAR · 2 deneme · API karşılığı",
        "uygulanan ≈$0.07+? · boşa ≈$0.84 · süren ≈$0.00 · lider ≈$0.25",
        "deneme 2",
        "kartsız",
        "2 geri döndü",
        "geç done",
        "uygulandı",
        "henüz bir karta harcama yok",
        "kartı aç",
    ] {
        assert!(s.contains(word), "{word}: {s}");
    }
    // On a short screen the row at hand stays in view.
    app.lang = Lang::En;
    app.world.estimated = false;
    let older = (0..8).map(|i| Attempt {
        project: p,
        agent: lead,
        model: format!("m-{i}"),
        ..Attempt::default()
    });
    app.world.ledger.splice(0..0, older);
    app.ledger_win = Some(9);
    let s = render(&app, 140, 12);
    assert!(row(&s, "m-0").contains('▶'), "the oldest, at hand: {s}");
    assert!(!s.contains("late done"), "scrolled away: {s}");
    app.ledger_win = Some(0);
    let s = render(&app, 140, 12);
    assert!(s.contains("lead $0.25") && !s.contains("m-0"), "{s}");
    app.world.ledger.drain(0..8);
    // How it ended says it in its colour; the row at hand in bold;
    // a blank line between projects.
    app.ledger_win = Some(1);
    let mut term = ratatui::Terminal::new(TestBackend::new(140, 30)).unwrap();
    term.draw(|f| draw(f, &app)).unwrap();
    let buf = term.backend().buffer().clone();
    let s = render(&app, 140, 30);
    let lines: Vec<&str> = s.lines().collect();
    let cell = |word: &str| {
        let y = lines.iter().position(|l| l.contains(word)).unwrap();
        let x = lines[y].split(word).next().unwrap().chars().count();
        buf[(x as u16, y as u16)].clone()
    };
    assert_eq!(cell("late done").fg, pal().warn);
    let applied = lines.iter().position(|l| l.contains("no card")).unwrap();
    let x = (lines[applied].find("55 s  applied"))
        .map(|b| lines[applied][..b].chars().count() + "55 s  ".len())
        .unwrap();
    assert_eq!(buf[(x as u16, applied as u16)].fg, pal().ok);
    assert!(
        cell("#3 try 2").modifier.contains(Modifier::BOLD),
        "at hand"
    );
    assert!(!cell("no card").modifier.contains(Modifier::BOLD));
    let other_at = lines
        .iter()
        .position(|l| l.contains(&format!("{other_name}  applied")))
        .unwrap();
    assert!(
        lines[other_at - 1].contains("│ ")
            && lines[other_at - 1]
                .trim_matches(|c| c == ' ' || c == '│')
                .is_empty(),
        "{s}"
    );
    app.world.ledger.push(Attempt {
        project: other,
        agent: lead,
        model: "m-open".into(),
        ..Attempt::default()
    });
    let s = render(&app, 140, 30);
    let lines: Vec<&str> = s.lines().collect();
    let mut term = ratatui::Terminal::new(TestBackend::new(140, 30)).unwrap();
    term.draw(|f| draw(f, &app)).unwrap();
    let buf = term.backend().buffer().clone();
    let y = lines.iter().position(|l| l.contains("m-open")).unwrap();
    let x = lines[y]
        .find("at work")
        .map(|b| lines[y][..b].chars().count())
        .unwrap();
    assert_eq!(buf[(x as u16, y as u16)].fg, pal().info);
    app.world.ledger.pop();
    // The card's screen lists its tries.
    app.ledger_win = None;
    app.open_card(c);
    let s = render(&app, 140, 40);
    assert!(s.contains("ATTEMPTS"), "{s}");
    assert!(row(&s, "late done").contains("try 2"), "{s}");
    assert!(!s.contains("no card"), "only the card's: {s}");
    // A card no try spent on lists none.
    let d = crate::live::testing::with_card(&mut app.world, lead);
    app.world.cards[d].project = p;
    app.open_card(d);
    let s = render(&app, 140, 40);
    assert!(!s.contains("ATTEMPTS"), "{s}");
}

#[test]
fn a_teardown_script_asked_about_shows_its_bytes_over_the_tidy_window() {
    let mut app = app_after(1, 1, 0, Lang::En);
    app.open_tidy();
    app.step();
    let checkout = std::path::PathBuf::from("/src/shop");
    let script = crate::sim::teardown_script(&checkout);
    let sha: String = script.sha.chars().take(16).collect();
    app.teardown = Some(crate::app::TeardownAsk {
        script: script.clone(),
        scroll: 0,
        at: vec![0],
        force: false,
    });
    let s = render(&app, 160, 30);
    assert!(
        s.contains(&format!("TEARDOWN SCRIPT · shop · sha256 {sha} ")),
        "{s}"
    );
    assert!(s.contains("before :tidy removes it"), "{s}");
    assert!(
        s.contains("file: /src/shop/.imece/worktree-teardown.sh"),
        "{s}"
    );
    assert!(s.contains("1 #!/bin/sh"), "{s}");
    assert!(s.contains("5 npm run db:drop"), "{s}");
    assert!(s.contains("yes, then remove"), "{s}");
    assert!(s.contains("remove nothing"), "{s}");
    app.teardown.as_mut().unwrap().scroll = 4;
    let s = render(&app, 160, 30);
    assert!(!s.contains("1 #!/bin/sh"), "{s}");
    assert!(s.contains("5 npm run db:drop"), "{s}");
    app.lang = Lang::Tr;
    let s = render(&app, 160, 30);
    assert!(s.contains("SÖKÜM BETİĞİ · shop"), "{s}");
    assert!(s.contains("dosya: /src/shop/"), "{s}");
    assert!(s.contains("evet, sonra sil"), "{s}");
}

#[test]
fn the_tidy_window_keeps_the_row_at_hand_on_a_short_screen() {
    let mut app = app_after(1, 1, 0, Lang::En);
    app.open_tidy();
    app.step();
    let mut found = app.world.orphans.found.clone().unwrap();
    for i in 0..30 {
        let mut o = found[0].clone();
        o.name = format!("w-{i:02}");
        found.push(o);
    }
    app.world.orphans.found = Some(found);
    app.tidy = Some(crate::app::TidyWin {
        at: 31,
        sure: false,
    });
    let s = render(&app, 120, 16);
    assert!(s.contains("▶ w-29"), "{s}");
    assert!(!s.contains("claude-31"), "{s}");
    app.tidy = Some(crate::app::TidyWin { at: 0, sure: false });
    let s = render(&app, 120, 16);
    assert!(s.contains("▶ claude-31"), "{s}");
    assert!(s.contains("under imece's state"), "{s}");
    // While a removal runs, its line is above the rows: the row at
    // hand, the last, still shows, the one above it too.
    app.world.orphans.busy = true;
    app.tidy = Some(crate::app::TidyWin {
        at: 31,
        sure: false,
    });
    let s = render(&app, 120, 16);
    assert!(s.contains("▶ w-29"), "{s}");
    assert!(s.contains("  w-28 "), "{s}");
    assert!(!s.contains("removing…"), "scrolled away: {s}");
    // Scrolled exactly: the row at hand is the last the window shows.
    let shown: Vec<&str> = s.lines().filter(|l| l.contains("w-2")).collect();
    assert!(shown.last().unwrap().contains("▶ w-29"), "{s}");
    assert!(!s.contains(" w-19 "), "one row too few scrolled: {s}");
}

#[test]
fn a_bare_colon_solo_opens_the_pick_screen_which_asks_model_and_effort() {
    let mut app = app_after(1, 1, 0, Lang::En);
    app.home = None;
    let t = Lang::En.text();
    for c in ":solo".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    let s = render(&app, 140, 40);
    assert!(s.contains(t.solo_pick_title.trim()), "{s}");
    assert!(
        s.contains("Which model?") && s.contains("default (Claude Code's own)"),
        "{s}"
    );
    assert!(s.contains("esc back"), "{s}");
    // Only the chosen row is reversed.
    let buf = buffer(&app, 140, 40);
    let row = |y: u16| -> String { (0..140).map(|x| buf[(x, y)].symbol()).collect() };
    let cell = |needle: &str| {
        let y = (0..40).find(|&y| row(y).contains(needle)).expect(needle);
        let line = row(y);
        // Cells, not bytes: the frame's box characters are wide in bytes.
        let at = line.find(needle).unwrap();
        let x = line[..at].chars().count() + needle.chars().count() - 1;
        buf[(u16::try_from(x).unwrap(), y)].clone()
    };
    let default = cell(" 1 default");
    assert!(
        default.modifier.contains(Modifier::REVERSED),
        "the chosen row"
    );
    let fable = cell(" 2 fable");
    assert_eq!(fable.fg, pal().strong);
    assert!(!fable.modifier.contains(Modifier::REVERSED), "not chosen");
    // The keys go to the screen, not the salon: j moves its row.
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.solo_pick.as_ref().unwrap().sel(), 1);
    key(&mut app, KeyCode::Char('k'));
    assert!(s.contains(" claude"), "what was picked so far: {s}");
    key(&mut app, KeyCode::Enter);
    let s = render(&app, 140, 40);
    assert!(
        s.contains("How deep should it think?") && s.contains(" 3 medium"),
        "{s}"
    );
    key(&mut app, KeyCode::Char('4'));
    key(&mut app, KeyCode::Enter);
    assert!(app.solo_pick.is_none());
    let head = app.world.projects[1].leader;
    assert_eq!(app.model_of(head).as_deref(), Some("@high"));
    // Turkish words too.
    app.lang = Lang::Tr;
    for c in ":solo".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    let s = render(&app, 140, 40);
    assert!(s.contains("Hangi model?"), "{s}");
    key(&mut app, KeyCode::Esc);
    assert!(app.solo_pick.is_none());
}

#[test]
fn the_welcome_and_news_windows_open_under_imece_s_mark_when_it_fits() {
    let mut app = App::new(1, 2, 4, 7, Lang::En);
    app.welcome = true;
    let screen = render(&app, 120, 40);
    let at = screen
        .lines()
        .position(|l| l.contains("WELCOME TO IMECE"))
        .unwrap();
    let rows: Vec<&str> = screen.lines().collect();
    assert!(
        rows[at + 2].contains("│       ██        imece v"),
        "{screen}"
    );
    assert!(rows[at + 5].contains("│     ▀▀  ▀▀   "), "{screen}");
    assert!(
        rows[at + 7].contains("One terminal for a team of Claude Code agents."),
        "{screen}"
    );
    assert!(app.mark_drawn.get());
    // The rows the window needs and the mark's six: 21 + 6 + 2.
    let lines = 2 + 2 + 1 + 1 + 1 + 9 + 2;
    assert!(render(&app, 120, lines + 6 + 2).contains("imece v"));
    let short = render(&app, 120, lines + 6 + 1);
    assert!(
        !short.contains("imece v") && short.contains("WELCOME TO IMECE"),
        "{short}"
    );
    // A narrow screen narrows the window: 53 inside, a column short of
    // the mark's 2 + 52, the words alone; 54, the mark.
    let narrow = render(&app, 55, 40);
    assert!(
        narrow.contains("imece v") && !narrow.contains("▀▀"),
        "{narrow}"
    );
    assert!(render(&app, 56, 40).contains("▀▀"));
    // What is new, under it too.
    app.welcome = false;
    app.news = vec![crate::welcome::VERSION];
    let screen = render(&app, 120, 40);
    assert!(
        screen.contains("imece v") && screen.contains("one terminal, many agents"),
        "{screen}"
    );
}

#[test]
fn the_board_lists_what_ended_newest_first_and_only_work_shows_its_owner_s_state() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let owner = app.world.agents[1].id;
    let name = app.world.agents[1].name.clone();
    app.world.agents[owner].status = Status::Working;
    let mut card = app.world.cards[0].clone();
    card.owner = Some(owner);
    let of = |id, column, finished, title: &str| {
        let mut c = card.clone();
        (c.id, c.column, c.finished) = (id, column, finished);
        c.title = title.into();
        c
    };
    app.world.cards = vec![
        of(0, Column::Done, 1, "older done"),
        of(1, Column::Done, 2, "newer done"),
        of(2, Column::Done, 3, "older review"),
        of(3, Column::Done, 4, "newer review"),
        of(4, Column::Verifying, 0, "checked"),
        of(5, Column::Working, 0, "at work"),
    ];
    app.world.inbox.clear();
    for c in [2, 3] {
        let mut change = item(InboxKind::Change, owner);
        change.card = Some(c);
        app.world.inbox.push(change);
    }
    app.window = Some(Window::Board);
    let s = render(&app, 160, 40);
    // Inside the window: Home's preview behind it names the cards too.
    let from = s.find("BOARD · cards").unwrap();
    let at = |word: &str| {
        s[from..]
            .find(word)
            .unwrap_or_else(|| panic!("{word}: {s}"))
    };
    assert!(s.contains("TO REVIEW (2)") && s.contains("DONE (2)"), "{s}");
    assert!(at("newer done") < at("older done"), "{s}");
    assert!(at("newer review") < at("older review"), "{s}");
    // At work on a card its owner has a state; past it, a name.
    let glyph = |word: &str| {
        let row = s.lines().find(|l| l.contains(word)).unwrap();
        let line = s.lines().skip_while(|l| !l.contains(word)).nth(1).unwrap();
        let cell = |l: &str, w: &str| l[..l.find(w).unwrap()].chars().count();
        let at = cell(row, word);
        line.chars().skip(at).take(16).collect::<String>()
    };
    assert!(glyph("checked").contains(&format!("● {name}")), "{s}");
    // Only a card at work has its progress.
    let bar = |word: &str| {
        let row = s.lines().skip_while(|l| !l.contains(word)).nth(1).unwrap();
        let at = s.lines().find(|l| l.contains(word)).unwrap();
        let from = at[..at.find(word).unwrap()].chars().count();
        let cell: String = row.chars().skip(from).take(28).collect();
        cell.contains('░') || cell.contains('▓')
    };
    assert!(bar("at work"), "{s}");
    assert!(!bar("checked"), "{s}");
    assert!(!bar("newer done"), "{s}");
    // The bar is the demo's made-up progress: a live run has none.
    app.world.demo = false;
    let s = render(&app, 160, 40);
    let row = s
        .lines()
        .skip_while(|l| !l.contains("at work"))
        .nth(1)
        .unwrap();
    assert!(
        row.contains(&name) && !row.contains('░') && !row.contains('▓'),
        "{s}"
    );
    app.world.demo = true;
    let s = render(&app, 160, 40);
    assert!(!glyph("newer review").contains('●'), "{s}");
    assert!(!glyph("newer done").contains('●'), "{s}");
    // Record 126: a card waiting for review stands there by its own
    // column, a failed one with the done ones marked, a later try says
    // which.
    app.world.inbox.clear();
    app.world.cards[2].column = Column::Review;
    app.world.cards[1].column = Column::Failed;
    app.world.cards[1].attempt = 2;
    let s = render(&app, 160, 40);
    assert!(s.contains("TO REVIEW (1)") && s.contains("DONE (3)"), "{s}");
    let row = s.lines().find(|l| l.contains("newer done")).unwrap();
    let cell = &row[row.find("#2").unwrap()..];
    let cell = cell.split('│').next().unwrap();
    assert!(cell.starts_with("#2  ✕ newer done "), "{cell}");
    assert!(
        cell.ends_with(" · try 2"),
        "the try at the column's end: {cell}"
    );
    let row = s.lines().find(|l| l.contains("older done")).unwrap();
    assert!(!row.contains('✕') && !row.contains("try"), "{row}");
}

#[test]
fn a_setup_ask_s_window_shows_how_it_runs_and_its_bytes_from_the_line_scrolled_to() {
    let (mut app, item, id) = crate::app::tests::asking_setup();
    let text = app.script_text(item).unwrap().text();
    let sha = crate::sha::hex(text.as_bytes());
    app.pane = crate::app::Pane::Inbox;
    let row = render(&app, 240, 30);
    let name = app.world.agents[id].name.clone();
    assert!(
        row.contains("run .imece/worktree-setup.sh in its new worktrees?"),
        "{row}"
    );
    assert!(row.contains(" a  yes   r  no"), "{row}");
    app.script = Some((item, 0));
    let s = render(&app, 140, 30);
    assert!(
        s.contains(&format!("SETUP SCRIPT · {name} · sha256 {} ", &sha[..16])),
        "{s}"
    );
    assert!(!s.contains(&sha[..17]), "the hash's head only");
    assert!(s.contains("runs as /bin/sh in each new worktree"), "{s}");
    assert!(
        s.contains("file: /demo/shop-api/.imece/worktree-setup.sh"),
        "{s}"
    );
    assert!(s.contains("1 #!/bin/sh"), "{s}");
    assert!(s.contains("5 npm ci --no-audit"), "{s}");
    assert!(s.contains(" a  yes "), "{s}");
    // Scrolled: from that line on.
    app.script = Some((item, 4));
    let s = render(&app, 140, 30);
    assert!(!s.contains("1 #!/bin/sh"), "{s}");
    assert!(s.contains("5 npm ci --no-audit"), "{s}");
    // Answered: the window says it is no longer asked about.
    app.script = None;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    app.script = Some((item, 0));
    assert!(render(&app, 140, 30).contains("that setup script is no longer asked about"));
}

#[test]
fn before_you_go_lists_who_is_mid_turn_and_the_changes_not_reviewed() {
    let quiet = |projects| {
        let mut app = app_after(projects, 4, 0, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app.leaving = true;
        app
    };
    let rows =
        |app: &App| -> Vec<String> { render(app, 140, 40).lines().map(str::to_string).collect() };
    let at = |rows: &[String], words: &str| rows.iter().position(|r| r.contains(words));
    let mut app = quiet(1);
    app.world.agents[2].status = Status::Working;
    let name = app.world.agents[2].name.clone();
    let s = rows(&app);
    let title = at(&s, "BEFORE YOU GO").expect("the window");
    assert_eq!(
        at(&s, "Mid-turn: quitting cuts these turns short"),
        Some(title + 1)
    );
    assert!(s[title + 2].contains(&name), "{}", s[title + 2]);
    let project = app.world.projects[0].name.clone();
    assert!(!s[title + 2].contains(&project), "one project: no name");
    assert!(
        s[title + 3].trim_matches(['│', ' ']).is_empty(),
        "a blank row between"
    );
    assert_eq!(at(&s, "Changes not reviewed"), Some(title + 4));
    assert!(s[title + 5].contains("Fix the parser"), "{}", s[title + 5]);
    assert!(s[title + 7].contains("q quits · W opens the next waiting"));
    // Only the change.
    app.world.agents[2].status = Status::Idle;
    let s = rows(&app);
    let title = at(&s, "BEFORE YOU GO").unwrap();
    assert_eq!(at(&s, "Changes not reviewed"), Some(title + 1));
    assert_eq!(at(&s, "Mid-turn"), None);
    // Only the turn: the change was reviewed.
    app.world.agents[2].status = Status::Working;
    app.world.inbox.iter_mut().for_each(|i| i.open = false);
    let s = rows(&app);
    assert!(at(&s, "Mid-turn").is_some());
    assert_eq!(at(&s, "Changes not reviewed"), None);
    // Two projects: each agent's project is named.
    let mut app = quiet(2);
    app.world.agents[2].status = Status::Working;
    let project = app.world.projects[app.world.agents[2].project].name.clone();
    let s = rows(&app);
    let title = at(&s, "BEFORE YOU GO").unwrap();
    assert!(s[title + 2].contains(&project), "{}", s[title + 2]);
    app.lang = Lang::Tr;
    let s = rows(&app);
    assert!(at(&s, "ÇIKMADAN ÖNCE").is_some() && at(&s, "İncelenmemiş değişiklikler").is_some());
}

#[test]
fn the_help_names_the_new_line_key_this_terminal_has_after_the_writing_keys() {
    for lang in [Lang::En, Lang::Tr] {
        let t = lang.text();
        for shift_enter in [true, false] {
            let keys = overlay::help_keys(lang, shift_enter);
            assert_eq!(keys[17].0, "^w ^u ^k ^y", "{lang:?}");
            let want = t.help_newline[usize::from(!shift_enter)];
            assert_eq!(keys[18], want, "{lang:?} {shift_enter}");
            assert_eq!(keys.len(), t.help_keys.len() + 1);
        }
    }
    let mut app = app_after(1, 4, 0, Lang::En);
    app.help = true;
    let plain = render(&app, 140, 40);
    assert!(plain.contains("here ⇧enter is enter"), "{plain}");
    app.shift_enter = true;
    let kitty = render(&app, 140, 40);
    assert!(kitty.contains("a new line; ↑ ↓ between lines"), "{kitty}");
    assert!(!kitty.contains("here ⇧enter is enter"));
}

#[test]
fn a_opens_the_archives_as_a_tree_and_r_brings_the_one_at_hand_back() {
    use crate::archives::Row;
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        app.menu = None;
        key(app, KeyCode::Enter);
    };
    let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
    for (lang, none, title, checkout, open, row, keys, back) in [
        (
            Lang::En,
            "nothing is archived yet; :archive puts the project at hand away",
            " ARCHIVES ",
            " ▾ shop · 1 archived",
            "   ● shop · open now · 1 agent",
            "   ▸ first epic · $4.00 · 2 cards done · 2 agents · archived {}",
            " j k move · h l za fold · enter view · / filter · r restore · esc close",
            "brought shop “first epic” back",
        ),
        (
            Lang::Tr,
            "henüz arşiv yok; :archive elindeki projeyi kaldırır",
            " ARŞİVLER ",
            " ▾ shop · 1 arşiv",
            "   ● shop · şu an açık · 1 agent",
            "   ▸ first epic · $4.00 · 2 kart bitti · 2 agent · {} arşivlendi",
            " j k gez · h l za katla · enter bak · / süz · r geri getir · esc kapat",
            "shop “first epic” geri geldi",
        ),
    ] {
        let mut app = app_after(1, 1, 0, lang);
        app.home = None;
        key(&mut app, KeyCode::Char('A'));
        assert!(app.archives.is_none());
        assert_eq!(flashed(&app), none, "{lang:?}");
        typed(&mut app, ":open ~/work/shop");
        let ours: Vec<AgentId> = (app.world.agents.iter())
            .filter(|a| a.project == 1)
            .map(|a| a.id)
            .collect();
        let (lead, worker) = (ours[0], ours[1]);
        // What it spent counts its gone agents; its agents do not.
        app.world.agents[lead].cost = 15_000;
        app.world.agents[worker].cost = 25_000;
        for &id in &ours[2..] {
            app.world.agents[id].gone = true;
        }
        // A helper's row goes in further than a leader's.
        app.world.agents[worker].role = Role::Helper;
        // Its cards done count, not the others nor another's.
        let card = |id, project, column| Card {
            id,
            project,
            title: "t".into(),
            owner: None,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            closes: None,
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            item: Default::default(),
            after: Vec::new(),
        };
        app.world.cards.clear();
        app.world.cards.push(card(0, 1, Column::Done));
        app.world.cards.push(card(1, 1, Column::Working));
        app.world.cards.push(card(2, 0, Column::Done));
        app.world.cards.push(card(3, 1, Column::Done));
        app.at_home.scope = Some(1);
        typed(&mut app, ":archive first epic");
        // The `:archive` waiting acts first, and shows.
        key(&mut app, KeyCode::Char('A'));
        assert!(app.grace.is_none() && app.world.projects[1].archived.is_some());
        let b = app.archives.as_ref().expect("open");
        assert_eq!(b.groups.len(), 1);
        assert_eq!(b.at(&app.world), Some(Row::Archive(1)));
        let at = app.world.projects[1].archived.as_ref().unwrap().at;
        let s = render(&app, 160, 40);
        let row = row.replace("{}", &date(day(at)));
        for want in [title, checkout, open, row.as_str(), keys] {
            assert!(s.contains(want), "{lang:?} {want}\n{s}");
        }
        // `l` shows its agents: the leader first in bold, a helper two
        // cells in, each with what it spent.
        key(&mut app, KeyCode::Char('l'));
        let s = render(&app, 160, 40);
        let name = &app.world.agents[lead].name;
        assert!(s.contains(&format!("│       {name} · $1.50")), "{s}");
        let helper = &app.world.agents[worker];
        let name = &helper.name;
        assert!(s.contains(&format!("│         {name} · $2.50")), "{s}");
        let (buf, _) = whole(&app, 160);
        let bold_at = |name: &str| {
            // From the bottom: the browser's window stands under
            // Home's preview, which names the head too.
            let y = (0..40)
                .rev()
                .find(|&y| {
                    let row: String = (0..160).map(|x| buf[(x, y)].symbol()).collect();
                    row.contains(&format!(" {name} · "))
                })
                .unwrap_or_else(|| panic!("{name}: {s}"));
            let row: String = (0..160).map(|x| buf[(x, y)].symbol()).collect();
            // The cell, not the byte: glyphs before the name are wide.
            let byte = row.find(name).unwrap();
            let x = row[..byte].chars().count() as u16;
            buf[(x, y)].modifier.contains(Modifier::BOLD)
        };
        assert!(bold_at(&app.world.agents[lead].name), "the head in bold");
        assert!(!bold_at(name), "the rest plain");
        assert!(!s.contains(&app.world.agents[ours[2]].name), "gone: {s}");
        // `?` is the browser's to keep; esc closes it, `:archives` opens it.
        key(&mut app, KeyCode::Char('?'));
        assert!(!app.help && app.archives.is_some());
        key(&mut app, KeyCode::Esc);
        assert!(app.archives.is_none());
        typed(&mut app, ":archives");
        assert!(app.archives.is_some());
        // A paste goes into the filter only while it is typed.
        app.flash = None;
        app.paste("first");
        assert_eq!(flashed(&app), lang.text().paste_not_writing);
        assert_eq!(app.archives.as_ref().unwrap().find, "");
        assert!(!render(&app, 160, 40).contains("│ /"), "no filter line");
        key(&mut app, KeyCode::Char('/'));
        app.paste("first");
        assert_eq!(app.archives.as_ref().unwrap().find, "first");
        assert_eq!(app.scope_below(), None, "no place's keys in it");
        assert!(render(&app, 160, 40).contains("│ /first "));
        assert!(caret_of(&app, 160, 40).is_some(), "the cursor after it");
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 160, 40);
        assert!(s.contains("│ /first ") && !s.contains("▏"), "kept: {s}");
        assert_eq!(caret_of(&app, 160, 40), None, "kept, not typed");
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Char('%'));
        let s = render(&app, 160, 40);
        assert!(s.contains(lang.text().help_none), "{s}");
        key(&mut app, KeyCode::Backspace);
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('r'));
        assert!(app.archives.is_none());
        assert!(app.world.projects[1].archived.is_none());
        assert_eq!(app.selected, Sel::Agent(app.world.projects[1].leader));
        assert!(flashed(&app).starts_with(back), "{lang:?}");
    }
}

#[test]
fn the_archives_scroll_to_keep_the_selected_row_in_sight() {
    let mut app = app_after(1, 1, 0, Lang::En);
    for n in 0..8 {
        app.end_grace();
        app.selected = Sel::Agent(app.world.projects[app.world.projects.len() - 1].leader);
        app.on_key(KeyEvent::from(KeyCode::Char(':')));
        for c in format!("archive epic{n}").chars() {
            key(&mut app, KeyCode::Char(c));
        }
        app.menu = None;
        key(&mut app, KeyCode::Enter);
    }
    key(&mut app, KeyCode::Char('A'));
    // A checkout, the project open on it, eight archives newest first.
    let rows = app.archives.as_ref().unwrap().rows(&app.world).len();
    assert_eq!(rows, 10);
    key(&mut app, KeyCode::Char('G'));
    // Twelve rows: the frame, a blank line above and below the rows and
    // the keys leave seven for the rows.
    let buf = buffer(&app, 120, 12);
    let row = |y: u16| -> String { (0..120).map(|x| buf[(x, y)].symbol()).collect() };
    let all: Vec<String> = (0..12).map(row).collect();
    let at = |title: &str| all.iter().position(|r| r.contains(&format!("▸ {title} ")));
    assert_eq!(at("epic0"), Some(8), "the last row, at the foot\n{all:#?}");
    assert_eq!(at("epic6"), Some(2), "seven rows up from it");
    assert_eq!(at("epic7"), None, "the one before is out of sight");
    let x = all[8]
        .find("▸ epic0")
        .map(|b| all[8][..b].chars().count())
        .unwrap();
    let x = u16::try_from(x).unwrap();
    assert!(
        buf[(x, 8)].modifier.contains(Modifier::REVERSED),
        "selected"
    );
    assert!(!buf[(x, 7)].modifier.contains(Modifier::REVERSED));
    // However short the screen, down to the floor, the selected row
    // is drawn; under it nothing is (record 131).
    assert!(!render(&app, 120, MIN_ROWS - 1).contains("epic0"));
    assert!(render(&app, 120, MIN_ROWS).contains("▸ epic0 "));
    // At the top again, the first rows are in sight.
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('g'));
    let s = render(&app, 120, 12);
    assert!(s.contains("· 8 archived") && !s.contains("epic1 "), "{s}");
}

#[test]
fn the_help_shows_the_persons_own_keys_first_and_in_place() {
    let none = crate::keys::Keymap::default();
    for lang in [Lang::En, Lang::Tr] {
        for shift_enter in [true, false] {
            let plain = overlay::help_keys(lang, shift_enter);
            let rows = overlay::help_rows(lang, shift_enter, &none, None, false, true);
            let want: Vec<(String, String)> = (plain.iter())
                .map(|(k, w)| (k.to_string(), w.to_string()))
                .collect();
            assert_eq!(rows, want, "no key file: the help as it is");
        }
        // The rows whose keys are written stay as they are.
        let plain = overlay::help_keys(lang, false);
        let typed: Vec<&str> = overlay::TYPED.iter().map(|&i| plain[i].0).collect();
        assert_eq!(
            typed,
            [
                "$",
                "tab ↑ ↓ ^r",
                "^a ^e ⌥b ⌥f",
                "^w ^u ^k ^y",
                "\\enter ⌥enter"
            ],
            "{lang:?}"
        );
    }
    let keys = crate::keys::Keymap::read("^n = j\nj = G\n↑ = k\n^w = q").unwrap();
    let rows = overlay::help_rows(Lang::En, false, &keys, None, false, true);
    let row = |k: &str| -> Vec<&str> {
        (rows.iter().filter(|r| r.1 == k))
            .map(|r| r.0.as_str())
            .collect()
    };
    assert_eq!(
        rows[..4],
        [
            ("^n".to_string(), "your key: as j".to_string()),
            ("j".into(), "your key: as G".into()),
            ("↑".into(), "your key: as k".into()),
            ("^w".into(), "your key: as q".into()),
        ]
    );
    assert_eq!(row("select"), ["– ↓  ^n ↑/k"]);
    assert_eq!(
        row("first / last line · Home: to decide · at work"),
        ["gg  j/G  gd  gw"]
    );
    assert_eq!(row("quit (q or :quit asks first)"), ["^w/q ^w/q  :quit"]);
    assert_eq!(
        row("writing: complete · sent lines · :history"),
        ["tab ↑ ↓ ^r"],
        "written keys are not turned"
    );
    assert_eq!(
        row("writing: cut a word / before / after; paste"),
        ["^w ^u ^k ^y"]
    );
    let tr = overlay::help_rows(Lang::Tr, false, &keys, None, false, true);
    assert_eq!(tr[0], ("^n".to_string(), "senin tuşun: j gibi".to_string()));
    assert_eq!(tr.len(), rows.len());
    // Drawn, and found by the filter.
    let mut app = app_after(1, 4, 0, Lang::En);
    app.keys = keys;
    app.help = true;
    let shown = render(&app, 140, 50);
    assert!(shown.contains("your key: as G"), "{shown}");
    app.help_find = "your key".into();
    let found = render(&app, 140, 50);
    assert!(found.contains("your key: as q"), "{found}");
    assert!(!found.contains("first / last line"), "{found}");
    // Keys of two, on either side.
    let two = crate::keys::Keymap::read("gt = G\nx = gg\n^t = z M").unwrap();
    let rows = overlay::help_rows(Lang::En, false, &two, None, false, true);
    let row = |k: &str| -> Vec<&str> {
        (rows.iter().filter(|r| r.1 == k))
            .map(|r| r.0.as_str())
            .collect()
    };
    assert_eq!(
        rows[..3],
        [
            ("gt".to_string(), "your key: as G".to_string()),
            ("x".into(), "your key: as gg".into()),
            ("^t".into(), "your key: as zM".into()),
        ]
    );
    assert_eq!(
        row("first / last line · Home: to decide · at work"),
        ["x/gg  gt/G  gd  gw"]
    );
    // A place's keys: said where, and the rows as they are there.
    let placed = crate::keys::Keymap::read("J = G\n[change]\n^n = j\nJ = gg").unwrap();
    let rows = |lang, place| overlay::help_rows(lang, false, &placed, place, false, true);
    let change = rows(Lang::En, Some(crate::keys::Scope::Change));
    assert_eq!(
        change[..3],
        [
            ("J".to_string(), "your key: as G".to_string()),
            ("^n".into(), "your key in a change: as j".into()),
            ("J".into(), "your key in a change: as gg".into()),
        ]
    );
    let row = |rows: &[(String, String)], k: &str| -> Vec<String> {
        (rows.iter().filter(|r| r.1 == k))
            .map(|r| r.0.clone())
            .collect()
    };
    let first_last = "first / last line · Home: to decide · at work";
    assert_eq!(row(&change, first_last), ["J/gg  G  gd  gw"]);
    assert_eq!(row(&change, "select"), ["↑ ↓  ^n/j k"]);
    let salon = rows(Lang::En, Some(crate::keys::Scope::Salon));
    assert_eq!(row(&salon, first_last), ["gg  J/G  gd  gw"]);
    assert_eq!(row(&salon, "select"), ["↑ ↓  j k"]);
    assert_eq!(rows(Lang::En, None), salon);
    let tr = rows(Lang::Tr, Some(crate::keys::Scope::Change));
    assert_eq!(tr[1].1, "senin tuşun, değişiklikte: j gibi");
    // Filtered to a screen, the file's lines of another place go.
    let scoped = |place| overlay::help_rows(Lang::En, false, &placed, Some(place), true, true);
    let salon = scoped(crate::keys::Scope::Salon);
    assert_eq!(salon[0], ("J".to_string(), "your key: as G".to_string()));
    assert!(
        !salon.iter().any(|r| r.1.contains("in a change")),
        "{salon:?}"
    );
    let change = scoped(crate::keys::Scope::Change);
    assert_eq!(change[1].1, "your key in a change: as j");
}

#[test]
fn the_help_lists_the_vim_keys_in_two_columns_when_wide() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.help = true;
    let wide = render(&app, 140, 40);
    assert!(
        wide.contains("gg  G") && wide.contains("za  zM  zR"),
        "{wide}"
    );
    // Two columns: a line of the first half carries one of the second
    // (the split is by rows drawn, so which one depends on wrapping).
    let keys = overlay::help_keys(Lang::En, false);
    // A long text wraps under its key, so the key is what the line
    // surely carries.
    // A key of one letter (r) is in every line's words: left out.
    let second: Vec<&str> = (keys[keys.len() / 2..].iter())
        .map(|k| k.0)
        .filter(|k| k.chars().count() > 1)
        .collect();
    let line = wide.lines().find(|l| l.contains("gg  G")).unwrap();
    assert!(
        second.iter().any(|o| line.contains(o)),
        "two columns: {line}"
    );
    let narrow = render(&app, 80, 40);
    let line = narrow.lines().find(|l| l.contains("gg  G")).unwrap();
    assert!(
        !second.iter().any(|o| line.contains(o)),
        "one column: {line}"
    );
}

#[test]
fn the_help_filter_keeps_what_has_every_word_in_either_column() {
    let text = |k: &(&str, &str)| format!("{} {}", k.0, k.1);
    let keys = overlay::help_keys(Lang::En, false);
    assert_eq!(overlay::found(&keys, "", text).len(), keys.len(), "all");
    assert_eq!(overlay::found(&keys, "  ", text).len(), keys.len());
    let got = overlay::found(&keys, "FIRST  last", text);
    let first_last = (
        "gg  G  gd  gw",
        "first / last line · Home: to decide · at work",
    );
    assert_eq!(got, [&first_last], "every word");
    assert!(overlay::found(&keys, "first zzz", text).is_empty());
    let got = overlay::found(&keys, "zM", text);
    let fold = ("za  zM  zR", "fold: toggle · all (:fold) · none (:unfold)");
    assert_eq!(got, [&fold], "keys");
    // Turkish letters typed or not.
    let tr = overlay::help_keys(Lang::Tr, false);
    for filter in ["butceyi artir", "bütçeyi artır", "BÜTÇEYİ"] {
        let got = overlay::found(&tr, filter, text);
        assert_eq!(got.len(), 1, "{filter}");
        assert_eq!(got[0].0, ":budget USD");
    }
}

#[test]
fn slash_in_the_help_filters_it_until_enter_keeps_or_esc_drops_it() {
    let mut app = app_after(1, 4, 0, Lang::En);
    let typed = |app: &mut App, s: &str| s.chars().for_each(|c| key(app, KeyCode::Char(c)));
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help && !app.help_typing);
    key(&mut app, KeyCode::Char('/'));
    assert!(app.help && app.help_typing);
    typed(&mut app, "f?/qu");
    assert_eq!(app.help_find, "f?/qu", "every key is typed, ? q u too");
    ctrl(&mut app, 'u');
    assert_eq!(app.help_find, "", "^u empties it");
    ctrl(&mut app, 'x');
    alt(&mut app, KeyCode::Char('x'));
    key(&mut app, KeyCode::Up);
    assert_eq!((app.help_find.as_str(), app.help_typing), ("", true));
    typed(&mut app, "folds");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.help_find, "fold");
    let s = render(&app, 140, 40);
    assert!(s.contains(" /fold ") && s.contains("enter keep"), "{s}");
    assert!(caret_of(&app, 140, 40).is_some(), "the cursor after it");
    // Enter keeps it; / goes back to it; any other key closes.
    key(&mut app, KeyCode::Enter);
    assert!(app.help && !app.help_typing);
    assert_eq!(app.help_find, "fold");
    let s = render(&app, 140, 40);
    assert!(s.contains(" /fold ") && !s.contains("▏"), "{s}");
    assert_eq!(caret_of(&app, 140, 40), None, "kept, not typed");
    assert!(s.contains("/ change · any other key: close"), "{s}");
    key(&mut app, KeyCode::Char('/'));
    typed(&mut app, "s");
    assert_eq!((app.help_find.as_str(), app.help_typing), ("folds", true));
    // Esc drops it and the whole help is back.
    key(&mut app, KeyCode::Esc);
    assert!(app.help && !app.help_typing);
    assert_eq!(app.help_find, "");
    let s = render(&app, 140, 40);
    assert!(s.contains("/ filter · any other key: close"), "{s}");
    // Backspace past the start leaves the filter, not the help.
    key(&mut app, KeyCode::Char('/'));
    typed(&mut app, "x");
    key(&mut app, KeyCode::Backspace);
    assert!(app.help_typing, "x went, the filter stays");
    key(&mut app, KeyCode::Backspace);
    assert!(app.help && !app.help_typing);
    // A kept filter closes with the help and is gone when it opens.
    key(&mut app, KeyCode::Char('/'));
    typed(&mut app, "gg");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('x'));
    assert!(!app.help, "closed");
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help && app.help_find.is_empty(), "{:?}", app.help_find);
}

#[test]
fn the_help_opens_with_this_screens_keys_and_question_mark_again_with_every_key() {
    use crate::keys::Scope;
    let settle = "hide the row at hand until it needs you";
    let hunk = "the hunk at the top, quoted in a message to its worker";
    let wrap = "in a change: wrap long lines";
    let mut app = app_after(1, 2, 0, Lang::En);
    let t = Lang::En.text();
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help && app.help_scoped());
    let s = render(&app, 188, 60);
    assert!(s.contains(t.help_here.trim()), "{s}");
    assert!(s.contains(settle) && s.contains(":budget USD"), "{s}");
    assert!(
        !s.contains(hunk) && !s.contains(wrap),
        "a change's keys: {s}"
    );
    // `?` again: every key; again: this screen's.
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help && !app.help_scoped());
    let s = render(&app, 188, 60);
    assert!(s.contains(t.help_every.trim()), "{s}");
    assert!(s.contains(hunk) && s.contains(settle), "{s}");
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help && app.help_scoped());
    assert!(!render(&app, 188, 60).contains(hunk));
    // Opened anew, this screen's again; Turkish words the title.
    key(&mut app, KeyCode::Esc);
    app.help_all = true;
    app.lang = Lang::Tr;
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help_scoped());
    assert!(render(&app, 188, 60).contains(Lang::Tr.text().help_here.trim()));
    // Over a window that is no place (the first run's), every key.
    app.lang = Lang::En;
    app.welcome = true;
    app.help = true;
    app.help_all = false;
    assert_eq!(app.scope_below(), None);
    let s = render(&app, 188, 60);
    assert!(s.contains(t.help_title.trim()) && !s.contains(t.help_here.trim()));
    assert!(s.contains(hunk) && s.contains(settle), "{s}");
    // The rows as the table places them.
    let keys = overlay::help_keys(Lang::En, false);
    assert_eq!(keys[13].0, ":pin NAME");
    assert_eq!(keys[27].0, "s  :settle");
    assert_eq!(keys[44].0, ">");
    assert_eq!(keys[47].0, "w  R");
    assert!(
        keys[18].1.starts_with("writing: a new line"),
        "{:?}",
        keys[18]
    );
    assert!(
        overlay::help_shown(18, Scope::Board),
        "the new-line line, everywhere"
    );
    assert!(overlay::help_shown(0, Scope::Board) && overlay::help_shown(13, Scope::Salon));
    assert!(!overlay::help_shown(13, Scope::Board), ":pin is the team's");
    assert!(overlay::help_shown(44, Scope::Change) && !overlay::help_shown(44, Scope::Salon));
    assert!(overlay::help_shown(27, Scope::Salon) && !overlay::help_shown(27, Scope::Change));
}

#[test]
fn the_help_in_a_change_and_in_the_windows_lists_their_keys() {
    let settle = "hide the row at hand until it needs you";
    let hunk = "the hunk at the top, quoted in a message to its worker";
    let copy = "copy the message · the conversation";
    let scale = "home: one project · change: file · timeline: scale";
    let mut app = app_after(1, 2, 0, Lang::En);
    app.zoom = Zoom::Diff(app.world.agents[1].id);
    key(&mut app, KeyCode::Char('?'));
    let s = render(&app, 188, 60);
    assert!(s.contains(hunk) && s.contains(scale), "{s}");
    assert!(
        !s.contains(settle) && !s.contains(copy) && !s.contains(":pin NAME"),
        "{s}"
    );
    key(&mut app, KeyCode::Esc);
    app.zoom = Zoom::None;
    app.command("journal");
    key(&mut app, KeyCode::Char('?'));
    let s = render(&app, 188, 60);
    assert!(
        s.contains(copy) && s.contains("mark the line at hand"),
        "{s}"
    );
    assert!(
        !s.contains(hunk) && !s.contains(scale) && !s.contains(settle),
        "{s}"
    );
    key(&mut app, KeyCode::Esc);
    app.command("timeline");
    key(&mut app, KeyCode::Char('?'));
    let s = render(&app, 188, 60);
    assert!(s.contains(scale) && s.contains(":budget USD"), "{s}");
    assert!(!s.contains(settle) && !s.contains(copy), "{s}");
}

#[test]
fn the_help_s_filter_searches_every_key_and_esc_narrows_it_again() {
    let hunk = "the hunk at the top, quoted in a message to its worker";
    let mut app = app_after(1, 2, 0, Lang::En);
    let t = Lang::En.text();
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Char('/'));
    assert!(
        !app.help_scoped(),
        "typing a filter is a search over every key"
    );
    for c in "hunk".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    let s = render(&app, 188, 60);
    assert!(s.contains(hunk) && s.contains(t.help_every.trim()), "{s}");
    key(&mut app, KeyCode::Enter);
    assert!(!app.help_scoped() && !app.help_find.is_empty());
    assert!(render(&app, 188, 60).contains(hunk));
    // Back to the filter, dropped: this screen's keys again.
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Esc);
    assert!(
        app.help && app.help_scoped(),
        "the filter dropped, this screen's keys"
    );
    let s = render(&app, 188, 60);
    assert!(!s.contains(hunk) && s.contains(t.help_here.trim()), "{s}");
}

#[test]
fn a_filtered_help_shows_what_matches_and_keeps_its_size() {
    let t = Lang::En.text();
    let mut app = app_after(1, 4, 50, Lang::En);
    app.help = true;
    app.help_all = true;
    // The rows of the frame's top and bottom corners, on a screen the
    // whole help fits.
    let frame = |app: &App, width: u16| {
        let buf = buffer(app, width, 50);
        let ys: Vec<u16> = (0..50)
            .filter(|&y| (0..width).any(|x| matches!(buf[(x, y)].symbol(), "┌" | "└")))
            .collect();
        (ys[0], ys[ys.len() - 1])
    };
    let (wide, narrow) = (frame(&app, 140), frame(&app, 80));
    app.help_typing = true;
    for (find, keys, legend, none) in [
        ("fold", &["za  zM  zR", "← →  h l"][..], false, false),
        ("tainted", &[][..], true, false),
        ("zzz", &[][..], false, true),
        ("", &["gg  G", "za  zM  zR"][..], true, false),
    ] {
        app.help_find = find.into();
        let s = render(&app, 140, 50);
        for k in keys {
            assert!(s.contains(k), "{find}: {k} {s}");
        }
        assert_eq!(s.contains("gg  G"), find.is_empty(), "{find}: {s}");
        assert_eq!(s.contains("≈ tainted"), legend, "{find}: {s}");
        assert_eq!(s.contains("▴ pinned"), find.is_empty(), "{find}: {s}");
        assert_eq!(s.contains(t.help_none.trim()), none, "{find}: {s}");
        assert_eq!(frame(&app, 140), wide, "{find}: the same size");
        assert_eq!(frame(&app, 80), narrow, "{find}: narrow too");
    }
}

#[test]
fn a_paste_goes_into_the_help_filter_while_it_is_typed() {
    let mut app = app_after(1, 4, 0, Lang::En);
    (app.help, app.help_typing) = (true, true);
    app.help_find = "a".into();
    app.paste("b\nc\td\u{1b}");
    assert_eq!(app.help_find, "ab c d", "one line, no control characters");
    assert!(app.flash.is_none() && app.input.is_empty());
    app.help_typing = false;
    app.paste("x");
    assert_eq!(app.help_find, "ab c d");
    assert!(app.flash.is_some(), "with the filter not typed, as before");
    // Not into a filter left from a help that closed.
    (app.help, app.help_typing) = (false, true);
    app.flash = None;
    app.paste("x");
    assert_eq!(app.help_find, "ab c d");
}

#[test]
fn the_help_scrolls_where_the_screen_is_too_short_for_it() {
    let t = Lang::En.text();
    let mut app = app_after(1, 4, 50, Lang::En);
    app.on_key(KeyEvent::from(KeyCode::Char('?')));
    app.help_all = true;
    // One column: the keys (a long text on two lines), a blank, the
    // legend, a blank: more lines than twenty rows less the frame and
    // the last line.
    let rows = overlay::help_rows(Lang::En, false, &app.keys, None, false, true);
    let rows: Vec<_> = rows.iter().collect();
    let keys = overlay::help_columns(&rows, false, overlay::help_widths(80, false)).len();
    assert!(keys > rows.len(), "some wrap");
    let (key_cells, text_cells) = overlay::help_widths(80, false);
    let legend: usize = (t.help_legend.iter())
        .map(|l| overlay::legend_wrap(l, 1 + key_cells + text_cells).len())
        .sum();
    assert!(legend > t.help_legend.len(), "the legend wraps");
    let lines = keys + legend + 2;
    let room = 20 - 3;
    let first = |app: &App| {
        let s = render(app, 80, 20);
        assert!(s.contains(t.help_close.trim()), "the last line stays: {s}");
        s
    };
    let s = first(&app);
    assert!(s.contains(" /  i "), "the top: {s}");
    let below = lines - room;
    assert!(
        s.contains(&format!(" ↓ {below} more · j k scroll · / filter")),
        "{s}"
    );
    assert_eq!(app.help_room.get(), (room, below));
    // `j` scrolls a line and keeps the help open; `k` back.
    key(&mut app, KeyCode::Char('j'));
    assert!(app.help);
    assert_eq!(app.help_scroll, 1);
    let s = first(&app);
    assert!(!s.contains(" /  i ") && s.contains(" @name text"), "{s}");
    assert!(s.contains(&format!(" ↓ {} more", below - 1)), "{s}");
    key(&mut app, KeyCode::Up);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help_scroll, 0, "not above the top");
    // Half a page, a page, back; never past the last.
    let ctrl =
        |app: &mut App, c| app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    ctrl(&mut app, 'd');
    assert_eq!(app.help_scroll, room / 2);
    ctrl(&mut app, 'u');
    assert_eq!(app.help_scroll, 0);
    ctrl(&mut app, 'f');
    assert_eq!(app.help_scroll, room);
    ctrl(&mut app, 'b');
    assert_eq!(app.help_scroll, 0);
    key(&mut app, KeyCode::PageDown);
    assert_eq!(app.help_scroll, room);
    key(&mut app, KeyCode::PageUp);
    assert_eq!(app.help_scroll, 0);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.help_scroll, below);
    let s = first(&app);
    let foot: String = (t.help_legend[t.help_legend.len() - 1].chars())
        .take(24)
        .collect();
    assert!(s.contains(&foot), "the foot: {s}");
    assert!(
        s.contains(" ↑ j k scroll · / filter") && !s.contains("more · j k"),
        "{s}"
    );
    for _ in 0..3 {
        key(&mut app, KeyCode::Char('j'));
        ctrl(&mut app, 'd');
        ctrl(&mut app, 'f');
        key(&mut app, KeyCode::PageDown);
        key(&mut app, KeyCode::End);
    }
    assert_eq!(app.help_scroll, below, "not past the last");
    // `g` waits for the second; `gg` and home go to the top.
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.help_scroll, below);
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.help_scroll, 0);
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Home);
    assert_eq!(app.help_scroll, 0);
    // A key after `g` that is not `g` acts as itself.
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, 1);
    // The filter shows from its top; what it finds fits.
    key(&mut app, KeyCode::Char('/'));
    assert_eq!(app.help_scroll, 1, "not before a letter");
    key(&mut app, KeyCode::Char('z'));
    assert_eq!(app.help_scroll, 0);
    key(&mut app, KeyCode::Enter);
    // Any other key closes it, ctrl ones too; it opens at the top.
    ctrl(&mut app, 'x');
    assert!(!app.help);
    // The page keys are ctrl's: the letters alone close it, and ctrl-j
    // is not j.
    for close in [
        KeyEvent::from(KeyCode::Char('d')),
        KeyEvent::from(KeyCode::Char('u')),
        KeyEvent::from(KeyCode::Char('f')),
        KeyEvent::from(KeyCode::Char('b')),
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
    ] {
        key(&mut app, KeyCode::Char('?'));
        app.help_all = true;
        app.help_scroll = 2;
        app.on_key(close);
        assert!(!app.help, "{close:?}");
    }
    app.help_scroll = 4;
    key(&mut app, KeyCode::Char('?'));
    app.help_all = true;
    assert!(app.help && app.help_scroll == 0);
    // However short the screen, down to the floor, its lines less the
    // frame and the last line are in sight and ^d moves half of them.
    render(&app, 80, MIN_ROWS);
    let least = usize::from(MIN_ROWS) - 3;
    assert_eq!(app.help_room.get(), (least, lines - least));
    ctrl(&mut app, 'd');
    assert_eq!(app.help_scroll, least / 2);
    // Under the floor nothing is drawn, so no key moves it (record 131).
    render(&app, 80, MIN_ROWS - 1);
    ctrl(&mut app, 'd');
    assert_eq!(app.help_scroll, least / 2);
    app.help_scroll = 0;
    // A screen it fits: nothing to say, nothing to scroll.
    let s = render(&app, 80, 100);
    assert!(!s.contains("more · j k") && !s.contains("↑ j k"), "{s}");
    assert_eq!(app.help_room.get().1, 0);
    key(&mut app, KeyCode::Char('j'));
    assert!(app.help && app.help_scroll == 0);
    key(&mut app, KeyCode::Char('x'));
    assert!(!app.help);
}

#[test]
fn the_help_window_is_as_tall_as_its_lines_and_their_frame() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.help = true;
    app.help_all = true;
    let t = Lang::En.text();
    // Forty rows: the help fits a screen that tall whole where it is
    // wide enough for every row on one line (188: keys 16, texts 74).
    let buf = buffer(&app, 188, 40);
    let row = |y: u16| -> String { (0..188).map(|x| buf[(x, y)].symbol()).collect() };
    let top = (0..40).find(|&y| row(y).contains(t.help_title)).unwrap();
    // Two columns of keys, a blank line, the legend, a blank line, close.
    let keys = overlay::help_keys(Lang::En, false).len();
    assert_eq!(keys, t.help_keys.len() + 1, "and the new-line line");
    let lines = u16::try_from(keys.div_ceil(2) + t.help_legend.len() + 3).unwrap();
    let x = (188 - 186) / 2;
    assert_eq!(buf[(x, top)].symbol(), "┌");
    assert!(
        row(top + lines).contains(t.help_close),
        "{}",
        row(top + lines)
    );
    assert_eq!(
        buf[(x, top + lines + 1)].symbol(),
        "└",
        "the frame closes under the last line"
    );
    // Whole on forty rows, with no row to spare.
    assert_eq!(top + lines + 2, 40, "it fills the forty rows");
    assert!(
        (top..top + lines).any(|y| row(y).contains("⎇ branch  ↑ own commits")),
        "the legend says what a worker's row counts"
    );
    assert!(
        (top..top + lines).any(|y| row(y).contains("↗ a local address the worker wrote")),
        "and what its link is"
    );
    // Narrower, its long texts take two lines and it scrolls.
    let s = render(&app, 140, 40);
    assert!(s.contains(" more · j k scroll"), "{s}");
    // One column: a blank, the key, a blank, the text and the frame.
    let buf = buffer(&app, 80, 100);
    let line = |y: u16| -> String { (0..80).map(|x| buf[(x, y)].symbol()).collect() };
    let top = (0..100).find(|&y| line(y).contains(t.help_title)).unwrap();
    let corners: Vec<u16> = (0..80)
        .filter(|&x| matches!(buf[(x, top)].symbol(), "┌" | "┐"))
        .collect();
    assert_eq!(corners, [9, 9 + (2 + 13 + 44 + 2) - 1], "61 wide");
}

#[test]
fn the_digest_window_shows_what_happened() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.digest = Some(crate::app::Digest {
        minutes: 7,
        merged: 2,
        blocked: 1,
        assumptions: 0,
        talks: 3,
        waiting: 0,
        highlights: Vec::new(),
    });
    let screen = render(&app, 140, 40);
    assert!(screen.contains("WHILE YOU WERE AWAY"), "{screen}");
    assert!(screen.contains("last 7 min"), "{screen}");
    assert!(screen.contains(" ■ 1 blocks"), "{screen}");
    assert!(screen.contains(" ✓ 2 cards merged"), "{screen}");
}

#[test]
fn the_welcome_window_says_what_claude_code_answered_and_the_keys() {
    use crate::welcome::Seen;
    use imece_agent::Login;
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    let here = |version: &str, login| Seen::Here {
        version: version.into(),
        login,
    };
    app.seen = here("2.1.3 (Claude Code)", Login::Yes { subscription: true });
    app.welcome = true;
    let screen = render(&app, 120, 40);
    for want in [
        &format!("WELCOME TO IMECE {}", crate::welcome::VERSION),
        "One terminal for a team of Claude Code agents.",
        "Claude Code on this machine:",
        "✓ claude  2.1.3 (Claude Code) · signed in, a plan · ≈ API prices",
        "Keys to start with:",
        "   /  i            write to the leader",
        "   ?               every key; / in it filters",
        "any key: close · :welcome shows this again",
    ] {
        assert!(screen.contains(want), "{want} missing:\n{screen}");
    }
    // Signed in: nothing more to look for.
    let doctor = ":doctor (imece doctor in a shell) says what else a start needs";
    assert!(!screen.contains(doctor), "{screen}");
    // An API key's figures are what it pays.
    app.seen = here(
        "2.1.3 (Claude Code)",
        Login::Yes {
            subscription: false,
        },
    );
    let screen = render(&app, 120, 40);
    assert!(screen.contains("✓ claude  2.1.3 (Claude Code) · signed in "));
    assert!(!screen.contains("a plan"), "{screen}");
    // Not signed in, in a CLI's words, cleaned.
    app.seen = here("2.1.3 \u{1b}[31mred", Login::No);
    let screen = render(&app, 120, 40);
    assert!(!screen.contains('\u{1b}'), "a CLI's words are cleaned");
    assert!(
        screen.contains("✗ claude  2.1.3")
            && screen.contains("red · not signed in: run claude auth login"),
        "{screen}"
    );
    assert!(
        screen.contains(doctor),
        "not signed in points at it: {screen}"
    );
    app.seen = here("0.65.0", Login::Unknown);
    let screen = render(&app, 120, 40);
    assert!(
        screen.contains("? claude  0.65.0 · its sign-in is checked when it first starts"),
        "{screen}"
    );
    assert!(screen.contains(doctor), "{screen}");
    // Before the answer, and not there.
    app.seen = Seen::Checking;
    let screen = render(&app, 120, 40);
    assert!(screen.contains("… claude  checking…"), "{screen}");
    assert!(!screen.contains(doctor), "not known yet: {screen}");
    app.seen = Seen::Missing;
    let screen = render(&app, 120, 40);
    assert!(screen.contains("· claude  not on PATH"), "{screen}");
    assert!(screen.contains(doctor), "{screen}");
    // In Turkish.
    app.lang = Lang::Tr;
    let screen = render(&app, 120, 40);
    assert!(screen.contains("Bu makinedeki Claude Code:"), "{screen}");
    assert!(screen.contains("PATH'te yok"), "{screen}");
    assert!(
        screen.contains(":doctor (kabukta imece doctor)"),
        "{screen}"
    );
}

#[test]
fn the_news_window_lists_each_version_newest_first() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    app.news = vec!["0.1.0"];
    let screen = render(&app, 120, 40);
    for want in [
        &format!("WHAT'S NEW IN IMECE {}", crate::welcome::VERSION),
        " 0.1.0",
        "· The first release, for macOS and Linux",
        "any key: close · :news shows this again",
    ] {
        assert!(screen.contains(want), "{want} missing:\n{screen}");
    }
    // The first version right under imece's mark (a blank row, four,
    // a blank row), no blank line more.
    let lines: Vec<&str> = screen.lines().collect();
    let top = lines.iter().position(|l| l.contains("WHAT'S NEW")).unwrap();
    assert!(lines[top + 2].contains("imece v"), "{screen}");
    assert!(lines[top + 7].contains("│ 0.1.0"), "{screen}");
    // The news come before the welcome, which comes before where the
    // person left off.
    app.welcome = true;
    app.left_off = Some(Default::default());
    assert!(render(&app, 120, 40).contains("WHAT'S NEW"));
    app.news.clear();
    let screen = render(&app, 120, 40);
    assert!(screen.contains("WELCOME TO IMECE") && !screen.contains("WHAT'S NEW"));
    app.welcome = false;
    assert!(render(&app, 120, 40).contains("WHERE YOU LEFT OFF"));
}

#[test]
fn the_where_you_left_off_window_tells_what_was_left() {
    let mut app = talking_over_two_days();
    let leader = app.world.projects[0].leader;
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .map(|a| a.id)
        .unwrap();
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Reply,
        agent: leader,
        detail: "the answer's first line".into(),
        researcher: None,
        open: true,
        tick: 7300,
        card: None,
    });
    let item = app.world.inbox.len() - 1;
    // The last lines: one of yesterday, then today's.
    let last: Vec<Entry> = app.world.journal[2..5].to_vec();
    assert_eq!((last[0].tick, last[1].tick), (0, 7200));
    app.left_off = Some(crate::app::LeftOff {
        ago: Some(3 * 3600 + 5 * 60),
        spent: 1_234,
        budget: 500_000,
        projects: vec![("shop-api".into(), 3, 2), ("other".into(), 1, 0)],
        waiting: vec![item],
        waiting_total: 2,
        replies: 1,
        interrupted: vec![leader, worker],
        last,
    });
    let screen = render(&app, 140, 40);
    for want in [
        "WHERE YOU LEFT OFF · 3 h 5 min ago",
        "$0.12 of $50.00 spent",
        "shop-api: 3 agents, 2 open cards",
        "other: 1 agent, 0 open cards",
        "! 2 to decide:",
        "» 1 reply to read on Home",
        "» 00:08 ",
        "the answer's first line",
        "… and 1 more",
        &format!(
            "turns cut short when imece closed: {}, {}",
            app.world.agents[leader].name, app.world.agents[worker].name
        ),
        "the last lines before:",
        "─ 1970-01-01",
        "any key: close",
    ] {
        assert!(screen.contains(want), "{want} missing:\n{screen}");
    }
    // Oldest first, like a conversation: each day's lines under its
    // date, the window's own rows (the journal pane has a day row too).
    let lines: Vec<&str> = screen.lines().collect();
    let top = lines
        .iter()
        .position(|l| l.contains("WHERE YOU LEFT OFF"))
        .unwrap();
    let window: Vec<&str> = lines[top..]
        .iter()
        .take_while(|l| !l.contains("any key: close"))
        .copied()
        .collect();
    let at = |needle: &str| {
        window
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle} not in the window\n{screen}"))
    };
    assert_eq!(at("14:00 ⇄ claude-1"), at("─ 1970-01-01") + 1, "{screen}");
    assert_eq!(at("00:00 › you"), at("─ 1970-01-02") + 1, "{screen}");
    assert_eq!(
        window.iter().filter(|l| l.contains("─ 1970-0")).count(),
        2,
        "two days, two rows; the second day's second line needs none\n{screen}"
    );
    // Every waiting item shown: no "more" line.
    app.left_off.as_mut().unwrap().waiting_total = 1;
    let screen = render(&app, 140, 40);
    assert!(!screen.contains("… and"), "{screen}");
    // Without a time, without waiting items, without cut turns.
    let l = app.left_off.as_mut().unwrap();
    l.ago = None;
    l.waiting.clear();
    l.waiting_total = 0;
    l.replies = 0;
    l.interrupted.clear();
    l.last.clear();
    let screen = render(&app, 140, 40);
    assert!(screen.contains("WHERE YOU LEFT OFF "), "{screen}");
    assert!(!screen.contains(" ago "), "{screen}");
    assert!(screen.contains("Nothing needs you right now."), "{screen}");
    for gone in ["wait for you", "cut short", "the last lines"] {
        assert!(!screen.contains(gone), "{gone} still on\n{screen}");
    }
    app.lang = Lang::Tr;
    app.left_off.as_mut().unwrap().ago = Some(60);
    let screen = render(&app, 140, 40);
    assert!(screen.contains("NEREDE KALMIŞTIN · 1 dk önce"), "{screen}");
    // The help wins over it, the key closes one at a time.
    app.help = true;
    let screen = render(&app, 140, 40);
    assert!(!screen.contains("NEREDE KALMIŞTIN"), "{screen}");
}

#[test]
fn the_help_lists_the_project_commands_in_both_languages() {
    let mut app = app_after(1, 2, 0, Lang::En);
    app.help = true;
    for (lang, keys) in [
        (
            Lang::En,
            [
                ":open :solo",
                ":close NAME",
                ":rename NAME",
                "a solo you drive",
            ],
        ),
        (
            Lang::Tr,
            [":open :solo", ":close AD", ":rename AD", "sürdüğün solo"],
        ),
    ] {
        app.lang = lang;
        let s = render(&app, 140, 40);
        for k in keys {
            assert!(s.contains(k), "{lang:?} {k}: {s}");
        }
    }
}

#[test]
fn a_window_over_the_salon_dims_what_shows_around_it() {
    let mut app = app_after(1, 2, 200, Lang::En);
    let before = buffer(&app, 200, 50);
    assert!(
        !before[(2, 3)].modifier.contains(Modifier::DIM),
        "no window, nothing dim"
    );
    app.help = true;
    let buf = buffer(&app, 200, 50);
    // The salon's top-left corner lies outside every window.
    assert!(
        buf[(2, 3)].modifier.contains(Modifier::DIM),
        "around the help"
    );
    let mid = buf[(100, 25)].modifier;
    assert!(
        !mid.contains(Modifier::DIM),
        "inside the help window: {mid:?}"
    );
}

/// A demo team, everyone idle and nothing waiting: the lead (0) and
/// workers 1 to 4, agent 3 a helper of worker 1 and agent 4
/// researching for it.
fn row_of_three() -> App {
    let mut app = App::new(1, 4, 0, 1, Lang::En);
    app.world.inbox.clear();
    for a in &mut app.world.agents {
        a.status = Status::Idle;
    }
    let a = &mut app.world.agents;
    a[3].role = Role::Helper;
    a[3].parent = Some(1);
    a[4].role = Role::Researcher;
    a[4].status = Status::Researching {
        for_agent: 1,
        until: 9,
    };
    app
}

#[test]
fn a_peek_is_the_focus_in_a_window_over_the_view() {
    let mut app = row_of_three();
    app.peek = Some(2);
    let t = Lang::En.text();
    let s = render(&app, 100, 40);
    let rows: Vec<Vec<char>> = s.lines().map(|l| l.chars().collect()).collect();
    // Four columns and three rows of the view show around it.
    assert_eq!(rows[3][4], '┌', "{s}");
    assert_ne!(rows[3][3], '┌', "{s}");
    assert_ne!(rows[2][4], '┌', "{s}");
    assert_eq!(rows[36][95], '┘', "{s}");
    assert_eq!(rows[36].get(96), Some(&' '), "{s}");
    let name = app.world.agents[2].name.clone();
    assert!(
        s.lines()
            .nth(3)
            .unwrap()
            .contains(&fill(t.focus_title, &[&name]))
    );
    // The footer has the peek's keys.
    let foot = s.lines().last().unwrap();
    assert!(
        foot.starts_with(&format!(" {} · ", t.places[5]))
            && foot.contains(t.focus)
            && foot.contains(t.write_this),
        "{foot}"
    );
    app.peek = None;
    let foot = render(&app, 100, 40).lines().last().unwrap().to_string();
    assert!(!foot.contains(t.places[5]), "{foot}");
}

#[test]
fn a_plan_s_window_lists_its_cards_with_tier_paths_and_body_and_the_pot_beside_the_cost() {
    let (mut app, item, leader) = crate::app::tests::asking_plan();
    let name = app.world.agents[leader].name.clone();
    let row = render(&app, 240, 30);
    assert!(row.contains("plans 2 cards for $1.25"), "{row}");
    assert!(row.contains(" a  yes   r  no"), "{row}");
    app.plan_window = Some((item, 0));
    let s = render(&app, 140, 30);
    assert!(
        s.contains(&format!(" PLAN · {name} · 2 cards · $1.25 ")),
        "{s}"
    );
    assert!(
        s.contains("the leader hires a second worker only once you say yes"),
        "{s}"
    );
    let left = dollars(app.world.budget.saturating_sub(app.world.spent()));
    assert!(
        s.contains(&format!("expected $1.25 · left in the pot {left}")),
        "{s}"
    );
    assert!(s.contains("1. Parser · cheap"), "{s}");
    assert!(s.contains("   owns src/p.rs, docs (shared)"), "{s}");
    assert!(s.contains("   Fix it."), "{s}");
    assert!(s.contains("   Then test."), "{s}");
    assert!(s.contains("2. Docs · standard"), "{s}");
    assert!(s.contains("   Write them."), "{s}");
    assert!(s.contains(" a  yes ") && s.contains(" n  notes "), "{s}");
    // The footer says the window's keys too, not the pane's.
    let foot = s.lines().last().unwrap();
    assert!(
        foot.contains("a  yes") && foot.contains("j k  scroll"),
        "{foot}"
    );
    assert!(!foot.contains("enter shows the plan"), "{foot}");
    // Scrolled past the first card.
    app.plan_window = Some((item, 5));
    let s = render(&app, 140, 30);
    assert!(!s.contains("1. Parser"), "{s}");
    assert!(s.contains("2. Docs · standard"), "{s}");
    // The plan gone (answered, then the item reopened by hand): said so.
    app.world.projects[0].plan.proposal = None;
    let s = render(&app, 140, 30);
    assert!(s.contains("that plan is no longer waiting"), "{s}");
    // The lines a card takes.
    let p = Proposal {
        cards: vec![
            Planned {
                title: "a".into(),
                tier: "cheap".into(),
                paths: Vec::new(),
                checks: Vec::new(),
                closes: None,
                body: "one".into(),
                after: Vec::new(),
            },
            Planned {
                title: "b".into(),
                tier: "strong".into(),
                paths: vec![Owned::new("x")],
                checks: vec!["api".into(), "e2e".into()],
                closes: None,
                body: "one\ntwo\nthree".into(),
                after: vec![1],
            },
        ],
        cost: 0,
        tick: 0,
    };
    let lines = overlay::plan_lines(&app, &p, usize::MAX);
    let texts: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(
        texts,
        [
            "1. a · cheap",
            "   one",
            "",
            "2. b · strong",
            "   owns x",
            "   checks api, e2e",
            "   after 1 above",
            "   one",
            "   two",
            "   three",
            ""
        ]
    );
    assert_eq!(lines[0].spans[0].style, bold(pal().strong));
    assert_eq!(lines[4].spans[0].style, dim());
    assert_eq!(lines[5].spans[0].style, dim());
    assert_eq!(lines[6].spans[0].style, dim());
    // Narrow: every line wraps under the card's indent, none cut.
    let mut p = p;
    p.cards[0].title = "a title of six words".into();
    p.cards[0].body = "one two three four".into();
    let lines = overlay::plan_lines(&app, &p, 14);
    let texts: Vec<String> = lines
        .iter()
        .map(|l| l.to_string().trim_end().to_string())
        .collect();
    assert_eq!(
        texts[..6],
        [
            "1. a title",
            "   of six",
            "   words ·",
            "   cheap",
            "   one two",
            "   three four",
        ]
    );
    assert!(texts.iter().all(|l| l.chars().count() <= 14), "{texts:?}");
}

#[test]
fn the_plan_window_wraps_its_lines_and_j_k_scroll_the_wrapped_ones() {
    let (mut app, item, _) = crate::app::tests::asking_plan();
    let p = app.world.projects[0].plan.proposal.as_mut().unwrap();
    p.cards[0].body = "word ".repeat(40).trim_end().to_string();
    app.plan_window = Some((item, 0));
    let s = render(&app, 60, 40);
    // The window is 52 wide, 50 inside: the head's last words show,
    // and the body's forty words, each whole.
    assert!(s.contains("has your yes)"), "{s}");
    assert_eq!(s.matches("word").count(), 40, "{s}");
    assert!(!s.contains("wor│") && !s.contains("wo│"), "{s}");
    // The drawing told the keys how wide: they scroll its lines.
    assert_eq!(app.plan_width.get(), 50);
    let wrapped = overlay::plan_lines(
        &app,
        app.world.projects[0].plan.proposal.as_ref().unwrap(),
        50,
    )
    .len();
    assert!(wrapped > 8, "{wrapped}");
    app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
    assert_eq!(app.plan_window, Some((item, wrapped - 1)));
}

#[test]
fn the_board_says_what_a_card_waits_on_and_r_leaves_out_what_is_not_ready() {
    let (mut app, w, _) = calm_team();
    app.window = Some(Window::Board);
    let name = |app: &App, id: AgentId| app.world.agents[id].name.clone();
    let card = |id, owner, column, after: Vec<CardId>| Card {
        id,
        project: 0,
        title: format!("card {id}"),
        owner,
        column,
        story: Vec::new(),
        symbols: Vec::new(),
        paths: Vec::new(),
        checks: Vec::new(),
        closes: None,
        started: 0,
        finished: 0,
        attempt: 1,
        breaker: Default::default(),
        item: Default::default(),
        after,
    };
    app.world.cards = vec![
        card(0, Some(w[0]), Column::Working, vec![]),
        card(1, None, Column::Waiting, vec![0]),
        card(2, Some(w[1]), Column::Waiting, vec![0]),
        card(3, Some(w[2]), Column::Failed, vec![]),
        card(4, None, Column::Waiting, vec![0, 3]),
        card(5, None, Column::Waiting, vec![]),
        card(6, Some(w[3]), Column::Waiting, vec![]),
    ];
    let s = render(&app, 200, 40);
    assert!(s.contains(" BOARD · cards: 7 · r ready only "), "{s}");
    assert!(s.contains("⧗ after #1 "), "{s}");
    assert!(
        s.contains(&format!("⧗ after #1 → {}", name(&app, w[1]))),
        "{s}"
    );
    assert!(s.contains("✕ #4 failed "), "{s}");
    assert!(s.contains(&format!("    → {}", name(&app, w[3]))), "{s}");
    assert!(s.contains("#6  card 5"), "{s}");
    // Ready only: the three that wait are gone, and the title says so.
    app.board_ready = true;
    let s = render(&app, 200, 40);
    assert!(
        s.contains(" BOARD · cards: 7 · ready only: 3 waiting hidden · r all "),
        "{s}"
    );
    for gone in ["card 1", "card 2", "card 4", "⧗", "✕ #4"] {
        assert!(!s.contains(gone), "{gone}:\n{s}");
    }
    for kept in ["card 0", "card 3", "card 5", "card 6"] {
        assert!(s.contains(kept), "{kept}:\n{s}");
    }
    assert_eq!(board::hidden(&app.world), 3);
    // Their marks in colour: failed in the bad one, waiting dim.
    let t = Lang::En.text();
    let spans = board_waiting_spans(t, &app.world, 4);
    assert_eq!(spans[0].style, fg(pal().bad));
    let spans = board_waiting_spans(t, &app.world, 1);
    assert_eq!(spans[0].style, dim());
    assert!(board_waiting_spans(t, &app.world, 5).is_empty());
    // A withdrawn card stands with the done ones, marked dim; one that
    // waits on it, through another, is held back for good.
    app.board_ready = false;
    app.world.cards[3].column = Column::Withdrawn;
    app.world.cards[5].after = vec![4];
    let s = render(&app, 200, 40);
    assert!(s.contains("#4  – card 3"), "{s}");
    assert!(s.contains("✕ #4 withdrawn"), "{s}");
    let spans = board_waiting_spans(t, &app.world, 5);
    assert_eq!(spans[0].content, "✕ #4 withdrawn");
    assert_eq!(spans[0].style, fg(pal().bad));
    let line = |s: &str| {
        s.lines()
            .find(|l| l.contains("– card 3"))
            .unwrap()
            .to_string()
    };
    assert!(!line(&s).contains("✕"), "{}", line(&s));
    // The mark is drawn in the withdrawn style, not the failed one.
    let (buf, _) = whole(&app, 200);
    let at = (buf.content.iter())
        .position(|c| c.symbol() == "–")
        .expect("the mark");
    let style = Style::default().patch(dim());
    assert_eq!(Some(buf.content[at].fg), style.fg.or(Some(Color::Reset)));
    assert_ne!(Some(buf.content[at].fg), fg(pal().bad).fg);
    assert_eq!(
        crate::ui::card::state(t, &app.world, &app.world.cards[3]),
        "WITHDRAWN"
    );
    let spans = card::after_spans(t, &app.world, &app.world.cards[4]);
    assert_eq!(spans.last().unwrap().style, fg(pal().bad));
    assert_eq!(spans.last().unwrap().content, "✕");
}

fn board_waiting_spans(t: &crate::i18n::Text, w: &World, c: CardId) -> Vec<Span<'static>> {
    board::waiting_spans(t, w, &w.cards[c])
}

#[test]
fn the_import_window_says_what_goes_and_what_can_not_be_taken_back() {
    for (lang, word) in [
        (Lang::En, "imece can not take this back"),
        (Lang::Tr, "geri alamaz"),
    ] {
        let mut app = app_after(1, 1, 0, lang);
        app.importing = Some(crate::model::ImportAsk {
            project: 0,
            repo: "shop".into(),
            checkout: "/src/shop".into(),
            branch: "main".into(),
            head: "693d62952cd8".into(),
        });
        let s = render(&app, 120, 30);
        for shown in ["/src/shop", "main", "693d629", "shop", word] {
            assert!(s.contains(shown), "{shown}: {s}");
        }
        assert!(!s.contains("693d62952cd8"), "{s}");
        // Under the home: `~`, as every folder on the screen.
        app.home = Some("/Users/me".into());
        if let Some(ask) = app.importing.as_mut() {
            ask.checkout = "/Users/me/code/shelf".into();
        }
        let s = render(&app, 120, 30);
        assert!(s.contains("~/code/shelf"), "{s}");
        assert!(!s.contains("/Users/me"), "{s}");
        // Its workers hired before: what becomes of their worktrees.
        let words = match lang {
            Lang::En => "workers hired before it move onto a Lonca workspace",
            Lang::Tr => "daha önce alınan işçiler",
        };
        assert!(s.contains(words), "{s}");
        let s = render(&app, 120, 30);
        assert!(
            s.contains(word) && s.contains("esc"),
            "the window grew: {s}"
        );
        for a in &mut app.world.agents {
            if a.role == Role::Worker {
                a.gone = true;
            }
        }
        assert!(!render(&app, 120, 30).contains(words), "nobody hired");
    }
}
