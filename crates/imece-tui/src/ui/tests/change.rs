use super::*;

#[test]
fn a_change_s_line_ends_with_what_it_cost_once_known() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.world.change_ready(a_change(1));
    let open = app.world.open_inbox();
    let line = inbox_line(&app, *open.last().unwrap());
    let last = line.spans.last().unwrap();
    assert!(!last.content.contains('$'), "unsettled: {}", last.content);
    app.world.changes[0].spend = Some(Spend {
        cost: 8_400,
        ticks: 144,
        rounds: 2,
        unpriced: false,
    });
    let line = inbox_line(&app, *open.last().unwrap());
    let last = line.spans.last().unwrap();
    assert_eq!(last.content, " · $0.84 · 12 min · 2 rounds");
    assert_eq!(last.style, dim());
}

#[test]
fn a_change_s_line_starts_with_how_the_check_stands() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    // The repository's check stands before the runs, in its colour;
    // none, or one that could not run, puts nothing there.
    let checked = |ok| {
        Check::Ran(Checked {
            ok,
            end: "exit 0, 1.0 s".into(),
            lines: Vec::new(),
        })
    };
    for (check, mark, style) in [
        (checked(true), "✓ check · ", fg(pal().ok)),
        (checked(false), "✗ check · ", fg(pal().bad)),
        (Check::Running, "● check · ", dim()),
        (Check::Asked, "▶ check · ", dim()),
    ] {
        app.world.change_ready(Change {
            check,
            ..a_change(1)
        });
        let open = app.world.open_inbox();
        let line = inbox_line(&app, *open.last().unwrap());
        let before_last = &line.spans[line.spans.len() - 2];
        assert_eq!(
            (before_last.content.as_ref(), before_last.style),
            (mark, style)
        );
    }
    for check in [Check::None, Check::Failed("x".into())] {
        app.world.change_ready(Change {
            check,
            ..a_change(1)
        });
        let open = app.world.open_inbox();
        let line = inbox_line(&app, *open.last().unwrap());
        let before_last = &line.spans[line.spans.len() - 2];
        assert!(!before_last.content.contains("check"), "{:?}", line.spans);
    }
}

#[test]
fn a_change_s_line_says_what_ran_on_its_tree_in_the_colour_of_how_it_went() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    let ran = |line: &str, code| Ran {
        line: line.into(),
        code: Some(code),
        timed_out: false,
    };
    for (runs, says, colour) in [
        (vec![], " · no run on this tree", pal().warn),
        (
            vec![ran("cargo test", 0), ran("cargo build", 0)],
            " · ✓ ran clean on this tree: cargo test, cargo build",
            pal().ok,
        ),
        (
            vec![
                ran("cargo test", 0),
                ran("cargo clippy", 101),
                ran("make", 2),
            ],
            " · ✗ failed on this tree: cargo clippy, make",
            pal().bad,
        ),
    ] {
        app.world.change_ready(Change {
            runs,
            check: Check::None,
            ..a_change(1)
        });
        let open = app.world.open_inbox();
        let line = inbox_line(&app, *open.last().unwrap());
        let last = line.spans.last().unwrap();
        assert_eq!((last.content.as_ref(), last.style), (says, fg(colour)));
    }
    // Only a change's line: the same worker's answer has none.
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Reply,
        agent: 1,
        detail: "an answer".into(),
        researcher: None,
        open: true,
        tick: 50,
        card: None,
    });
    let line = inbox_line(&app, app.world.inbox.len() - 1);
    assert!(
        line.spans.last().unwrap().content.ends_with("an answer"),
        "{line:?}"
    );
    app.lang = Lang::Tr;
    let open = app.world.open_inbox();
    let line = inbox_line(&app, open[0]);
    assert_eq!(
        line.spans.last().unwrap().content,
        " · ✗ bu ağaçta başarısız: cargo clippy, make"
    );
}

#[test]
fn a_change_of_several_files_lists_them_beside_its_patch_from_120_columns() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let id = app.world.agents[1].id;
    let file = |path: &str| ChangedFile {
        path: path.into(),
        added: 1,
        removed: 0,
        binary: false,
    };
    app.world.change_ready(Change {
        agent: id,
        title: "Paging".into(),
        files: vec![file("src/a.rs"), file("src/b.rs")],
        patch: "diff --git a/src/a.rs b/src/a.rs\n@@ -1 +1 @@\n+x\n".into(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    app.zoom = Zoom::Diff(id);
    // Width less the window's two borders.
    let wide = render(&app, 122, 30);
    assert!(wide.contains("▸ src/a.rs +1 −0"), "{wide}");
    assert!(!wide.contains("diff --git"), "{wide}");
    let narrow = render(&app, 121, 30);
    assert!(!narrow.contains('▸'), "{narrow}");
    assert!(narrow.contains(" src/b.rs +1 −0"), "{narrow}");
}

#[test]
fn a_change_says_first_what_its_worker_said_of_it() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let id = app.world.agents[1].id;
    app.world.note(Event::Sent {
        from: id,
        to: 0,
        kind: MsgKind::Done,
        text: "Paging\n\nKept the old shape; next is a new field.".into(),
        id: 1,
        re: None,
    });
    app.world.change_ready(Change {
        agent: id,
        title: "Paging".into(),
        files: Vec::new(),
        patch: "+x\n".into(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    app.zoom = Zoom::Diff(id);
    let s = render(&app, 120, 30);
    let rows: Vec<&str> = s.lines().collect();
    // The detail's head, then the tabs, then the change's own frame
    // with the verdict first and the worker's words under it.
    assert!(
        rows[1].contains("Paging") && rows[1].contains("· 0 files +0 −0"),
        "{s}"
    );
    assert!(rows[3].contains("diff · talk · log"), "{s}");
    let v = rows
        .iter()
        .position(|l| l.contains("! look first · nothing ran"))
        .unwrap();
    assert_eq!(v, 5, "{s}");
    assert!(
        rows[v + 1].contains("Kept the old shape; next is a new field."),
        "the worker's words under the verdict: {s}"
    );
}

#[test]
fn the_files_stand_beside_a_wide_change_only_when_there_are_two() {
    let file = |path: &str| ChangedFile {
        path: path.into(),
        added: 7,
        removed: 5,
        binary: false,
    };
    let shown = |files: Vec<ChangedFile>| {
        let mut app = app_after(1, 2, 0, Lang::En);
        let id = app.world.agents[1].id;
        app.world.change_ready(Change {
            agent: id,
            title: "Paging".into(),
            patch: "diff --git a/src/one.rs b/src/one.rs\n+x\n".into(),
            files,
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Diff(id);
        render(&app, 160, 30)
    };
    // The tree's row: the file at hand, its counts, then the rule.
    let beside = |s: &str| s.lines().any(|l| l.contains("▸ src/one.rs +7 −5  "));
    let two = shown(vec![file("src/one.rs"), file("src/two.rs")]);
    assert!(beside(&two), "{two}");
    let one = shown(vec![file("src/one.rs")]);
    assert!(!beside(&one), "{one}");
}

#[test]
fn a_proposal_says_so_in_its_view_and_its_row_says_its_keys() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.pane = Pane::Inbox;
    app.world.change_ready(a_change(1));
    let s = render(&app, 140, 40);
    assert!(s.contains(" a  approve   r  send back"), "{s}");
    let proposal = crate::model::StoreProposal {
        seq: 5,
        change: "c5".into(),
        why: vec!["unobserved_session".into()],
        redacted: Vec::new(),
        cut: false,
        verifies: Runs::default(),
        on_trunk: true,
        intent: String::new(),
    };
    app.world.change_proposed(a_change(1), proposal);
    let s = render(&app, 140, 40);
    assert!(s.contains(" a  approve   r  send back"), "{s}");
    app.zoom = Zoom::Diff(1);
    let s = render(&app, 140, 40);
    assert!(
        s.contains("nobody watched this session's tools · proposal 5"),
        "{s}"
    );
}

#[test]
fn a_change_of_no_proposal_says_nothing_of_one() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.change_ready(a_change(1));
    app.zoom = Zoom::Diff(1);
    assert!(!render(&app, 140, 40).contains("proposal"));
}

#[test]
fn a_change_shows_its_review_above_its_files_or_who_reads_it() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.world.change_ready(a_change(1));
    app.zoom = Zoom::Diff(1);
    let s = render(&app, 140, 40);
    assert!(!s.contains("✎"), "{s}");
    for c in ":review".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert!(app.reviewing(1), "asked");
    let s = render(&app, 140, 40);
    assert!(s.contains("✎ claude reads this change…"), "{s}");
    let patch = crate::model::patch_sum(&app.world.change_of(1).unwrap().patch);
    app.world.reviewed(
        1,
        Review {
            patch,
            model: "m1".into(),
            text: "1. parse.rs: no test".into(),
            cost: 420,
        },
    );
    let s = render(&app, 140, 40);
    let head = "✎ review · claude · m1 · $0.04";
    assert!(
        s.contains(head) && s.contains("1. parse.rs: no test"),
        "{s}"
    );
    let (review, files) = (s.find(head).unwrap(), s.find("2 files").unwrap_or(0));
    assert!(review > files, "under the title, above the list: {s}");

    // Whole, it takes up to half the screen and counts the rest.
    let long: Vec<String> = (1..=40).map(|i| format!("point-{i:02}")).collect();
    app.world.reviews.get_mut(&1).unwrap().text = long.join("\n");
    app.review_whole = true;
    let s = render(&app, 140, 40);
    assert!(s.contains("point-10") && !s.contains("point-40"), "{s}");
    assert!(s.contains("lines · R shows all"), "{s}");
}

#[test]
fn a_change_that_clashed_says_b_sends_it_back_in_its_journal_line_view_and_foot() {
    let t = Lang::En.text();
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.world.change_ready(a_change(1));
    app.world
        .change_failed(1, "conflicts with the checkout in x".into());
    let name = app.world.agents[1].name.clone();
    let line = |app: &App| {
        words::event(
            app.lang,
            &app.world,
            &app.world.journal.last().unwrap().event,
        )
    };
    let plain = fill(
        t.ev_apply_failed,
        &[&name, "conflicts with the checkout in x"],
    );
    assert_eq!(line(&app), plain);
    app.zoom = Zoom::Diff(1);
    let s = render(&app, 220, 40);
    assert!(
        !s.contains("send the clash back") && !s.contains("sends it back"),
        "{s}"
    );
    app.world.change_clashed(
        1,
        Clashes {
            head: "abc1234".into(),
            hunks: vec![Clash {
                file: "x.rs".into(),
                ..Clash::default()
            }],
            ..Clashes::default()
        },
    );
    assert_eq!(
        line(&app),
        format!("{plain} · b in the change sends it back")
    );
    let s = render(&app, 220, 40);
    assert!(
        s.contains(" b  send the clash back   a  approve   r  send back   esc  back"),
        "{s}"
    );
    assert!(
        s.contains(&format!(
            "✗ does not fit your checkout at abc1234: x.rs · b sends it back to {name}"
        )),
        "{s}"
    );
}

#[test]
fn a_change_waits_in_the_inbox_and_enter_shows_its_diff() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.world.change_ready(a_change(1));
    let name = app.world.agents[1].name.clone();
    let s = render(&app, 220, 40);
    assert!(
        s.contains("Fix the parser") && s.contains("2 files +3 −1"),
        "{s}"
    );
    app.pane = Pane::Inbox;
    app.inbox_sel = 0;
    let s = render(&app, 220, 40);
    assert!(s.contains(" a  approve   r  send back"), "{s}");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.zoom, Zoom::Diff(1));
    let s = render(&app, 118, 40);
    // The head names the card, the project and the worker, then the
    // files and lines; the tabs under it.
    let head = s.lines().nth(1).unwrap();
    assert!(
        head.contains(&format!(" · {name} · ")) && head.contains("2 files +3 −1"),
        "{s}"
    );
    assert!(
        s.lines().nth(3).unwrap().contains("diff · talk · log"),
        "{s}"
    );
    assert!(!s.contains("CHANGE ·"), "{s}");
    assert!(s.contains("-OLD-LINE") && s.contains("+NEW-LINE"), "{s}");
    // The verdict heads the diff, its files under that, then the
    // patch. The demo's card owns the files of its leases, which this
    // change is not of: the verdict says so (record 114).
    let row = |needle: &str| s.lines().position(|l| l.contains(needle)).unwrap();
    let card = app.world.agents[1].card.unwrap();
    assert!(!app.world.cards[card].paths.is_empty());
    let verdict = format!(
        "! look first · nothing ran · 2 files · 2 outside #{}: src/p.rs, src/q.rs",
        card + 1
    );
    let ran = row(&verdict);
    assert_eq!(ran + 1, row("│ src/p.rs +"), "{s}");
    assert_eq!(ran + 3, row("│src/p.rs "), "{s}");
    assert!(
        s.contains("+Q-LINE \n") || s.contains("+Q-LINE "),
        "control characters go: {s}"
    );
    assert!(
        s.contains(" a  approve   r  send back   c  note   esc  back"),
        "{s}"
    );
    let wide = render(&app, 200, 36);
    // Movement and the rest are in the help, however wide the screen.
    assert!(
        !wide.contains(" j k ") && !wide.contains(" >  quote to it"),
        "{wide}"
    );
    // The agent at hand is the change's: `@` writes to it.
    key(&mut app, KeyCode::Char('@'));
    assert_eq!(app.input, format!("@{name} "));
    key(&mut app, KeyCode::Esc);
    // Scrolling: lines, pages, ends, files.
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.diff_scroll, 4, "over git's lines");
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.diff_scroll, 3);
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.diff_scroll, 8, "the next file's header");
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.diff_scroll, 8, "no file after the last");
    key(&mut app, KeyCode::Char('['));
    assert_eq!(app.diff_scroll, 0);
    key(&mut app, KeyCode::Char('['));
    assert_eq!(app.diff_scroll, 0);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.diff_scroll, 13, "the last of 14 lines");
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.diff_scroll, 0);
    key(&mut app, KeyCode::PageDown);
    assert!(
        app.diff_scroll > 0 && app.diff_scroll <= 13,
        "{}",
        app.diff_scroll
    );
    key(&mut app, KeyCode::PageUp);
    assert_eq!(app.diff_scroll, 0);
    ctrl(&mut app, 'd');
    let half = app.diff_scroll;
    assert!(half > 0, "{half}");
    ctrl(&mut app, 'u');
    assert_eq!(app.diff_scroll, 0);
    ctrl(&mut app, 'f');
    assert!(app.diff_scroll >= half, "{}", app.diff_scroll);
    ctrl(&mut app, 'b');
    assert_eq!(app.diff_scroll, 0);
    key(&mut app, KeyCode::End);
    assert_eq!(app.diff_scroll, 13);
    key(&mut app, KeyCode::Home);
    assert_eq!(app.diff_scroll, 0);
    key(&mut app, KeyCode::Char(' '));
    assert!(app.diff_scroll > 0);
    // Esc: back to the inbox.
    app.pane = Pane::Salon;
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.zoom, Zoom::None);
    assert_eq!(app.pane, Pane::Inbox);
    // The demo applies it as live does: said at once, done on the
    // next step; the item stays until then.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('a'));
    assert_eq!(app.zoom, Zoom::None);
    assert!(render(&app, 140, 40).contains(&format!("applying {name}'s change")));
    assert_eq!(app.world.open_inbox().len(), 1);
    // A change that went away leaves an empty view, not a crash.
    app.zoom = Zoom::Diff(1);
    app.world.change_rejected(1);
    let s = render(&app, 140, 40);
    assert!(
        s.contains(&format!("nothing of {name}'s waits any more")),
        "{s}"
    );
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char(']'));
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.diff_scroll, 0);
}

#[test]
fn a_live_change_is_applied_or_rejected_from_the_inbox_and_the_change_view() {
    let w = crate::live::testing::team(1);
    let live = crate::live::testing::idle_live(2);
    let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
    app.world.change_ready(a_change(1));
    app.pane = Pane::Inbox;
    // Nothing of claude-1's is known to the engine: applying fails and
    // says so; the item stays.
    key(&mut app, KeyCode::Char('a'));
    let s = render(&app, 140, 40);
    assert!(
        s.contains("claude-1's change did not apply: no change of theirs waits"),
        "{s}"
    );
    assert_eq!(app.world.open_inbox().len(), 1);
    // Rejecting closes it and opens the composer to the worker.
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.zoom, Zoom::Diff(1));
    key(&mut app, KeyCode::Char('r'));
    assert_eq!(app.zoom, Zoom::None);
    assert!(app.typing);
    assert_eq!(app.input, "@claude-1 ");
    assert!(app.world.open_inbox().is_empty());
    let s = render(&app, 140, 40);
    assert!(s.contains("› to claude-1"), "{s}");
    assert!(matches!(
        app.world.journal.last().unwrap().event,
        Event::Rejected { agent: 1 }
    ));
}

#[test]
fn the_demo_decides_an_ask_as_live_does() {
    let mut app = app_after(1, 4, 0, Lang::En);
    let name = app.world.agents[1].name.clone();
    for (k, allowed) in [('a', true), ('r', false)] {
        app.world.asks_you(1, "Bash: make");
        let item = app.world.inbox.len() - 1;
        app.at_home.at = Some(HomeRow::Item(item));
        key(&mut app, KeyCode::Char(k));
        assert!(!app.world.inbox[item].open, "{k}");
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: a } if what == "Bash: make" && *a == allowed
        ));
        let how = if allowed { "allowed" } else { "refused" };
        let s = render(&app, 140, 40);
        assert!(s.contains(&format!("{how}: {name}")), "{s}");
    }
}
