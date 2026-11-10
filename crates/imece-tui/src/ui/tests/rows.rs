use super::*;

#[test]
fn the_ledger_says_what_each_model_spent_on_a_project_on_the_row_at_hand_and_on_a_card() {
    use crate::ledger::{Attempt, Outcome};
    let mut app = app_after(1, 1, 0, Lang::En);
    let w = &mut app.world;
    let p = w.salon_order()[0];
    let lead = w.projects[p].leader;
    let c = crate::live::testing::with_card(w, lead);
    w.cards[c].project = p;
    w.ledger = vec![
        Attempt {
            project: p,
            card: Some(c),
            attempt: 1,
            agent: lead,
            cost: 8_400,
            outcome: Outcome::Failed,
            // $0.70 of b, $0.13 of a, $0.01 said by none.
            models: [("a".into(), 1_300), ("b".into(), 7_000)].into(),
            ..Attempt::default()
        },
        Attempt {
            project: p,
            agent: lead,
            cost: 299,
            // Under a cent said by none is not said.
            models: [("a".into(), 200)].into(),
            ..Attempt::default()
        },
    ];
    app.command("ledger");
    let s = render(&app, 140, 30);
    let lines: Vec<&str> = s.lines().collect();
    let at = |word: &str| lines.iter().position(|l| l.contains(word));
    // The project's: both rows, the most first; 100 + 99 not split.
    let head = at("applied").unwrap();
    assert!(
        lines[head + 1].contains("  by model: b $0.70 · a $0.15 · not split $0.01"),
        "{s}"
    );
    // The row at hand (the newest), under it, without what is not split.
    let here = at("▶ no card").unwrap();
    assert!(lines[here + 1].contains("    by model: a $0.02"), "{s}");
    assert!(!lines[here + 1].contains("not split"), "{s}");
    // The row not at hand says none.
    let failed = at("failed").unwrap();
    assert!(!lines[failed + 1].contains("by model"), "{s}");
    // Down: the row at hand is the other, and says its own.
    app.on_key(KeyEvent::from(KeyCode::Char('j')));
    let s = render(&app, 140, 30);
    let lines: Vec<&str> = s.lines().collect();
    let failed = lines.iter().position(|l| l.contains("▶ #")).unwrap();
    assert!(
        lines[failed + 1].contains("    by model: b $0.70 · a $0.13 · not split $0.01"),
        "{s}"
    );
    assert_eq!(s.matches("by model").count(), 2, "{s}");
    // A row nothing said by model of: no line; nor a project.
    app.world.ledger.iter_mut().for_each(|r| r.models.clear());
    let s = render(&app, 140, 30);
    assert!(!s.contains("by model"), "{s}");
    // The card's screen: under each of its tries.
    app.world.ledger[0].models = [("b".into(), 7_000)].into();
    app.ledger_win = None;
    app.open_card(c);
    let s = render(&app, 140, 40);
    let lines: Vec<&str> = s.lines().collect();
    let tried = lines.iter().position(|l| l.contains("try 1")).unwrap();
    assert!(
        lines[tried + 1].contains("by model: b $0.70 · not split $0.14"),
        "{s}"
    );
    // In Turkish.
    app.lang = Lang::Tr;
    let s = render(&app, 140, 40);
    assert!(s.contains("modele göre: b $0.70 · ayrılmamış $0.14"), "{s}");
}

#[test]
fn how_long_an_attempt_took_is_seconds_under_a_minute() {
    use crate::ui::ledger::lasted;
    assert_eq!(lasted(Lang::En, 11), "55 s");
    assert_eq!(lasted(Lang::En, 12), "1 min");
    assert_eq!(lasted(Lang::Tr, 0), "0 sn");
    // Every outcome has its own words.
    let t = Lang::En.text();
    let words: std::collections::HashSet<&str> = [
        crate::ledger::Outcome::Open,
        crate::ledger::Outcome::Applied,
        crate::ledger::Outcome::Done,
        crate::ledger::Outcome::Failed,
        crate::ledger::Outcome::Withdrawn,
        crate::ledger::Outcome::HandedOn,
        crate::ledger::Outcome::Late,
    ]
    .map(|o| crate::ui::ledger::outcome(t, o))
    .into_iter()
    .collect();
    assert_eq!(words.len(), 7);
    assert_eq!(
        crate::ui::ledger::outcome(t, crate::ledger::Outcome::HandedOn),
        "handed on"
    );
    assert_eq!(
        crate::ui::ledger::outcome(t, crate::ledger::Outcome::Withdrawn),
        "withdrawn"
    );
    assert_eq!(
        crate::ui::ledger::outcome(t, crate::ledger::Outcome::Failed),
        "failed"
    );
    assert_eq!(
        crate::ui::ledger::outcome(t, crate::ledger::Outcome::Done),
        "done, no change"
    );
    assert_eq!(
        crate::ui::ledger::outcome(t, crate::ledger::Outcome::Open),
        "at work"
    );
}

#[test]
fn a_folded_project_still_shows_what_waits_for_the_person() {
    let mut app = app_after(2, 24, 0, Lang::En);
    for _ in 0..5000 {
        app.step();
        if app
            .world
            .open_inbox()
            .iter()
            .any(|&i| app.world.agents[app.world.inbox[i].agent].project == 1)
        {
            break;
        }
    }
    app.collapsed[1] = true;
    let s = render(&app, 160, 45);
    let header = s
        .lines()
        .find(|l| l.contains("▸ mobile-app"))
        .expect("no folded header");
    assert!(header.contains("waiting"), "{header}");
}

#[test]
fn the_focus_of_a_helper_names_its_worker_and_a_helper_that_left_is_journaled() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let (worker, [h1, _]) = with_helpers(&mut app);
    app.world.note(Event::Left { agent: h1 });
    let name = app.world.agents[worker].name.clone();
    app.open_window(Window::Journal);
    let s = render(&app, 140, 45);
    assert!(s.contains(&format!("{name}.1 finished and left")), "{s}");
    app.close_window();
    app.zoom = Zoom::Focus(h1);
    let s = render(&app, 140, 45);
    assert!(s.contains(&format!("helper of {name}")), "{s}");
}

#[test]
fn mail_that_waits_with_an_agent_shows_on_its_row_in_both_languages() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let id = app.world.projects[0].leader + 1;
    let text = |app: &App| {
        status_spans(app.lang, &app.world, &app.world.agents[id])
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>()
    };
    let plain = text(&app);
    app.world.agents[id].held_mail = 2;
    assert_eq!(text(&app), format!("✉ 2 queued {plain}"));
    let spans = status_spans(app.lang, &app.world, &app.world.agents[id]);
    assert_eq!(spans[0].style, fg(pal().warn));
    app.world.agents[id].turn_yes = Some(0);
    assert_eq!(text(&app), format!("✓ turn ✉ 2 queued {plain}"));
    app.world.agents[id].turn_yes = None;
    app.lang = Lang::Tr;
    assert!(text(&app).starts_with("✉ 2 kuyrukta "));
}

#[test]
fn a_yes_for_the_turn_shows_first_on_the_agents_status_while_it_holds() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let id = app.world.projects[0].leader + 1;
    let text = |app: &App| {
        status_spans(app.lang, &app.world, &app.world.agents[id])
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>()
    };
    let plain = text(&app);
    assert!(!plain.contains("✓ turn"), "{plain}");
    app.world.agents[id].turn_yes = Some(3);
    let spans = status_spans(app.lang, &app.world, &app.world.agents[id]);
    assert_eq!(spans[0].content, "✓ turn ");
    assert_eq!(spans[0].style, bold(pal().warn));
    assert_eq!(text(&app), format!("✓ turn {plain}"));
    app.zoom = Zoom::Focus(id);
    assert!(render(&app, 160, 30).contains("status ✓ turn "));
    app.lang = Lang::Tr;
    assert!(text(&app).starts_with("✓ tur "));
}

#[test]
fn the_salon_and_the_focus_say_the_folder_they_work_in() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let name = app.world.projects[0].name.clone();
    let header = |app: &App| {
        let l = salon::project_line(app, 0, &app.rungs());
        l.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    };
    assert!(
        header(&app).ends_with(&format!("  ~/src/{name}")),
        "{}",
        header(&app)
    );
    // Folded, it ends with its loudest member instead.
    app.collapsed[0] = true;
    assert!(!header(&app).contains("~/src"), "{}", header(&app));
    app.collapsed[0] = false;
    let lead = app.world.projects[0].leader;
    app.zoom = Zoom::Focus(lead);
    let s = render(&app, 160, 30);
    assert!(s.contains(&format!("folder ~/src/{name} ")), "{s}");
    assert!(!s.contains("worktree of"), "{s}");
    assert!(!s.contains("scratch "), "a leader writes nothing: {s}");
    app.zoom = Zoom::Focus(lead + 1);
    let s = render(&app, 160, 30);
    let worker = app.world.agents[lead + 1].name.clone();
    assert!(
        s.contains(&format!(
            "folder ~/.local/state/imece/worktrees/{name}-5e1f0a2c/{worker} · worktree of ~/src/{name}"
        )),
        "{s}"
    );
    assert!(
        s.contains(&format!(
            "scratch ~/.local/state/imece/worktrees/{name}-5e1f0a2c-scratch/{worker} "
        )),
        "{s}"
    );
    // A worker on Lonca: the folder is where it was, the word is not.
    app.world.agents[lead + 1].workspace = Some("kule".into());
    let s = render(&app, 160, 30);
    assert!(s.contains("· workspace of lonca kule"), "{s}");
    assert!(!s.contains("worktree of"), "{s}");
    app.lang = Lang::Tr;
    let s = render(&app, 160, 30);
    assert!(s.contains("· lonca kule için workspace"), "{s}");
    app.world.agents[lead + 1].workspace = None;
    let s = render(&app, 160, 30);
    assert!(s.contains(&format!("· ~/src/{name} için worktree")), "{s}");
    assert!(s.contains("klasör ~/.local"), "{s}");
    assert!(s.contains("karalama ~/.local"), "{s}");
    // A solo's goes by its project, as live's does.
    app.world.agents[lead + 1].role = Role::Solo;
    assert_eq!(
        app.scratch_of(lead + 1),
        Some(format!(
            "~/.local/state/imece/worktrees/{name}-5e1f0a2c-scratch/solo-0"
        ))
    );
}

#[test]
fn the_step_is_the_first_in_progress_and_none_without_one() {
    let (mut app, w, _) = calm_team();
    let a = w[0];
    assert_eq!(app.world.agents[a].step(), None);
    plan_of(&mut app, a);
    assert_eq!(app.world.agents[a].step(), Some("Run the tests"));
    app.world.agents[a].plan[1].text = "  ".into();
    app.world.agents[a].plan.remove(2);
    assert_eq!(
        app.world.agents[a].step(),
        None,
        "a blank step says nothing"
    );
    for s in &mut app.world.agents[a].plan {
        s.state = StepState::Done;
    }
    assert_eq!(app.world.agents[a].step(), None);
}

#[test]
fn a_salon_s_rows_stand_in_columns_whatever_their_depth_and_parts() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let lead = app.world.projects[app.world.agents[w[0]].project].leader;
    let rungs = app.rungs();
    for &id in &[lead, w[0], w[1]] {
        let a = &mut app.world.agents[id];
        a.status = Status::Idle;
        a.talk = None;
        a.activity = None;
        a.plan.clear();
        a.context = None;
        a.cost = 0;
        a.stand = Stand::default();
    }
    app.world.agents[lead].cost = 550_000;
    app.world.agents[w[0]].cost = 12_290_000;
    app.world.agents[w[0]].stand.added = 727;
    app.world.agents[w[1]].stand.added = 1090;
    app.world.agents[w[1]].stand.removed = 134;
    plan_of(&mut app, w[0]);
    for id in [lead, w[0], w[1]] {
        app.world.agents[id].status = Status::Idle;
        app.world.agents[id].changed_at = 0;
    }
    app.world.tick = 240;
    app.world.agents[w[1]].changed_at = 228;
    let row = |app: &App, id: AgentId, prefix: &'static str, width: usize| -> String {
        let line = salon::agent_line(app, &app.world.agents[id], prefix, &rungs, width);
        (line.spans.iter()).map(|s| s.content.as_ref()).collect()
    };
    let at = |r: &str, what: &str| {
        let cut = r.find(what).unwrap_or_else(|| panic!("{what} in {r:?}"));
        r[..cut].chars().count()
    };
    let lead_row = row(&app, lead, "  ", 200);
    let one = row(&app, w[0], "  ├ ", 200);
    let two = row(&app, w[1], "  └ ", 200);
    let helper = row(&app, w[1], "    └ ", 200);
    for r in [&one, &two, &helper] {
        assert_eq!(at(r, "idle"), at(&one, "idle"), "the status: {r:?}");
    }
    assert_eq!(at(&lead_row, "coordinating"), at(&one, "idle"));
    // The columns stand under one another, a part a row lacks kept blank.
    assert_eq!(at(&lead_row, "·"), at(&one, "·"), "{lead_row:?}\n{one:?}");
    assert_eq!(at(&one, "·"), at(&two, "·"), "{one:?}\n{two:?}");
    assert_eq!(at(&one, "+727"), at(&two, "+1090"), "{one:?}\n{two:?}");
    assert_eq!(
        at(&lead_row, "$") + dollars(550_000).len(),
        at(&one, "$") + dollars(12_290_000).len(),
        "the money is right aligned: {lead_row:?}\n{one:?}"
    );
    // A narrow screen gives up the blanks, never what the row says.
    for width in 80..130 {
        for (id, prefix) in [(lead, "  "), (w[0], "  ├ "), (w[1], "  └ ")] {
            let r = row(&app, id, prefix, width);
            assert!(r.chars().count() <= width, "{width}: {r:?}");
        }
        let r = row(&app, w[0], "  ├ ", width);
        assert!(
            r.contains(&dollars(12_290_000)) && r.contains("+727"),
            "{r:?}"
        );
    }
    // Who has gone away widens nothing, and a row alone is not padded.
    app.world.agents[lead].cost = 1_000_000_000;
    let wider = row(&app, w[0], "  ├ ", 200);
    assert!(at(&wider, "$") > at(&one, "$"), "{wider:?}");
    app.world.agents[lead].gone = true;
    assert_eq!(row(&app, w[0], "  ├ ", 200), one, "gone");
    app.world.agents[lead].gone = false;
    app.world.projects[app.world.agents[lead].project].closed = true;
    assert_eq!(row(&app, w[0], "  ├ ", 200).len(), one.len(), "closed");
    app.world.projects[app.world.agents[lead].project].closed = false;
    app.world.agents[lead].cost = 550_000;
    let alone = row(&app, w[1], "", 200);
    assert!(alone.chars().count() < two.chars().count(), "{alone:?}");
    assert!(!alone.contains("  ·"), "{alone:?}");
    // A helper's name gives way, a shallower one takes the room.
    app.world.agents[w[1]].name = "a-helper-with-a-long-name".into();
    assert!(row(&app, w[1], "    └ ", 200).contains("a-helper-with…"));
    assert!(row(&app, w[1], "  └ ", 200).contains("a-helper-with-a…"));
    assert!(row(&app, w[1], "  ", 200).contains("a-helper-with-a-l…"));
}

#[test]
fn a_step_is_cut_to_what_the_row_leaves_it_between_its_bounds() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let a = w[0];
    plan_of(&mut app, a);
    app.world.agents[a].plan[1].text = "Run the tests with coverage on".into();
    app.world.agents[a].cost = 20_000;
    app.world.agents[a].stand = Stand::default();
    let rungs = app.rungs();
    let line =
        |app: &App, width: usize| salon::agent_line(app, &app.world.agents[a], "", &rungs, width);
    let full = line(&app, 1000);
    let at = (full.spans.iter())
        .position(|s| s.content.starts_with("▸ "))
        .expect("a step");
    let base: usize = full.spans[..at].iter().map(Span::width).sum();
    let tail = 2 + dollars(20_000).chars().count();
    let step = |app: &App, width: usize| {
        line(app, width)
            .spans
            .iter()
            .find(|s| s.content.starts_with("▸ "))
            .map(|s| s.content.to_string())
            .unwrap_or_default()
    };
    assert_eq!(step(&app, 1000), "▸ Run the tests with cove…");
    assert_eq!(step(&app, base + tail + 2 + 12), "▸ Run the tes…");
    assert_eq!(step(&app, base + tail + 2 + 3), "▸ Run the…");
    assert_eq!(
        status_spans(Lang::En, &app.world, &app.world.agents[a])[0].content,
        "▸ Run the tests with cove…"
    );
}

#[test]
fn what_a_step_does_takes_what_the_status_and_row_leave_or_nothing() {
    let mut a = Agent::new(0, 0, "w".into(), Role::Worker, Status::Working, 0);
    let text = |s: Vec<Span>| s.iter().map(|s| s.content.to_string()).collect::<String>();
    a.activity = Some("x".repeat(50));
    let x = |n: usize| "x".repeat(n);
    assert_eq!(
        text(doing_spans(Lang::En, &a, 0, usize::MAX)),
        format!("  {}…", x(37))
    );
    assert_eq!(
        text(doing_spans(Lang::En, &a, 30, usize::MAX)),
        format!("  {}…", x(7))
    );
    assert!(doing_spans(Lang::En, &a, 31, usize::MAX).is_empty());
    assert_eq!(
        text(doing_spans(Lang::En, &a, 0, 10)),
        format!("  {}…", x(7))
    );
    assert!(doing_spans(Lang::En, &a, 0, 9).is_empty());
    a.activity = Some(String::new());
    assert_eq!(text(doing_spans(Lang::Tr, &a, 0, 80)), "  düşünüyor…");
    a.activity = None;
    assert!(doing_spans(Lang::En, &a, 0, 80).is_empty());
}

#[test]
fn a_folded_project_s_header_ends_with_its_loudest_member_and_counts_the_stopped() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let text = |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
    let header = |app: &App| text(&salon::project_line(app, 0, &app.rungs()));
    let name = |app: &App, id: AgentId| app.world.agents[id].name.clone();
    let lead = app.world.projects[0].leader;
    // Open: the counts, no member.
    let open = header(&app);
    assert!(
        open.contains("▾ ") && !open.contains(&name(&app, w[0])),
        "{open}"
    );
    assert!(!open.contains("■"), "{open}");
    // Folded: the loudest member last; the lead counts as a member,
    // with its status's glyph like anyone (record 125).
    app.collapsed[0] = true;
    let h = header(&app);
    assert!(h.ends_with(&format!("  ● {}", name(&app, lead))), "{h}");
    app.world.agents[lead].status = Status::Idle;
    let h = header(&app);
    assert!(h.ends_with(&format!("  ● {}", name(&app, w[0]))), "{h}");
    app.world.agents[w[3]].status = Status::Stopped;
    app.world.agents[w[1]].status = Status::Blocked;
    let h = header(&app);
    // Blocked and stopped are one count of stuck.
    assert!(h.contains("  ■2"), "{h}");
    assert!(!h.contains("■1"), "{h}");
    assert!(h.ends_with(&format!("  ■ {}", name(&app, w[1]))), "{h}");
    app.world.agents[w[1]].status = Status::Working;
    let h = header(&app);
    assert!(!h.contains("■2") && h.contains("  ■1"), "{h}");
    assert!(h.ends_with(&format!("  ■ {}", name(&app, w[3]))), "{h}");
    waiting(&mut app, w[2]);
    let h = header(&app);
    assert!(h.contains("! 1 waiting"), "{h}");
    assert!(
        h.ends_with(&format!("  ■ {}", name(&app, w[3]))),
        "stuck over waiting: {h}"
    );
    app.world.agents[w[3]].status = Status::Working;
    let h = header(&app);
    assert!(h.ends_with(&format!("  ! {}", name(&app, w[2]))), "{h}");
}

#[test]
fn a_row_whose_item_waits_shows_the_waiting_mark_whatever_it_does() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let row = |app: &App, id: AgentId| {
        salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200)
    };
    app.world.agents[w[0]].status = Status::Idle;
    assert_eq!(row(&app, w[0]).spans[1].content, "○ ");
    waiting(&mut app, w[0]);
    let line = row(&app, w[0]);
    assert_eq!(
        (line.spans[1].content.as_ref(), line.spans[1].style),
        ("! ", fg(pal().warn))
    );
    assert_eq!(row(&app, w[1]).spans[1].content, "● ");
    waiting(&mut app, w[1]);
    assert_eq!(row(&app, w[1]).spans[1].content, "! ", "working too");
    // Stuck shows as stuck even with an item waiting.
    app.world.agents[w[1]].status = Status::Stopped;
    assert_eq!(row(&app, w[1]).spans[1].content, "■ ");
    // A leader's row waits like anyone's (record 125: no glyph of its own).
    let lead = app.world.projects[0].leader;
    assert_eq!(row(&app, lead).spans[1].content, "● ");
    waiting(&mut app, lead);
    assert_eq!(row(&app, lead).spans[1].content, "! ");
}

/// A demo team at its start, everyone idle and nothing waiting.
fn idle_team(lang: Lang) -> App {
    let mut app = app_after(1, 4, 0, lang);
    app.world.inbox.clear();
    for a in &mut app.world.agents {
        a.status = Status::Idle;
    }
    app
}

#[test]
fn a_live_agent_at_work_with_nothing_said_thinks_and_only_the_demo_has_phases() {
    let mut app = app_after(1, 2, 0, Lang::En);
    assert!(app.world.demo, "the simulation's world");
    assert!(!crate::live::testing::team(1).demo, "a live one");
    let id = (app.world.agents.iter())
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    let said = |app: &App| spans_text(&status_spans(app.lang, &app.world, &app.world.agents[id]));
    let a = &mut app.world.agents[id];
    (a.status, a.starting, a.activity, a.progress) = (Status::Working, false, None, 50);
    a.plan.clear();
    assert_eq!(said(&app), format!("{} impl", bar(50)));
    app.world.demo = false;
    assert_eq!(said(&app), "thinking…");
    app.lang = Lang::Tr;
    assert_eq!(said(&app), "düşünüyor…");
    // Asking someone, or the person, a live agent has no bar either.
    let leader = app.world.projects[0].leader;
    app.lang = Lang::En;
    app.world.agents[id].status = Status::Asking {
        to: leader,
        until: 9,
    };
    let lead = app.world.agents[leader].name.clone();
    assert_eq!(said(&app), format!("? → {lead}"));
    app.world.demo = true;
    assert_eq!(said(&app), format!("{} ? → {lead}", bar(50)));
}

#[test]
fn home_shows_imece_s_mark_while_no_project_has_a_worker() {
    let mut app = App::new(1, 0, 4, 7, Lang::En);
    let screen = render(&app, 120, 30);
    assert!(app.mark_drawn.get());
    let at = screen.lines().position(|l| l.contains("imece v")).unwrap();
    let rows: Vec<&str> = screen.lines().skip(at).take(4).collect();
    assert!(rows[0].contains("     ██        imece v"), "{screen}");
    assert!(rows[1].contains(" ██  ▄▄  ██    one terminal, many agents, one leader"));
    assert!(rows[2].contains("   ▄▄▀▀▄▄      shop-api · claude leads"));
    assert!(rows[3].contains("   ▀▀  ▀▀   "));
    // Under Home's list, mid the rows it leaves free.
    assert!(at > 2 && at < 20, "{screen}");
    // Drawn again, later: it goes on coming from where it began.
    let from = app.mark_from.get();
    assert!(from.is_some());
    render(&app, 120, 30);
    assert_eq!(app.mark_from.get(), from);
    // A worker on the list: the rows are its, the mark goes.
    let lead = app.world.projects[0].leader;
    let mut worker = app.world.agents[lead].clone();
    worker.id = app.world.agents.len();
    worker.name = "claude-1".into();
    worker.role = Role::Worker;
    worker.status = Status::Idle;
    app.world.agents.push(worker);
    let screen = render(&app, 120, 30);
    assert!(!screen.contains("imece v"), "{screen}");
    assert!(!app.mark_drawn.get());
    // Idle with a change waiting it has no row under AT WORK, yet the
    // project has a worker: still no mark (the field, 2026-11-09).
    let id = app.world.agents.len() - 1;
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Change,
        agent: id,
        detail: String::new(),
        researcher: None,
        open: true,
        tick: 0,
        card: None,
    });
    assert!(
        !(app.home_rows().iter()).any(|r| r.1 == crate::app::HomeRow::Agent(id)),
        "listed only as its change"
    );
    let screen = render(&app, 120, 30);
    assert!(!screen.contains("imece v"), "{screen}");
    // Gone, or its project put away: the mark is back.
    app.world.inbox.clear();
    app.world.agents[id].gone = true;
    assert!(render(&app, 120, 30).contains("imece v"));
    app.world.agents[id].gone = false;
    app.world.agents[id].role = Role::Leader;
    assert!(
        render(&app, 120, 30).contains("imece v"),
        "a leader is no worker"
    );
}

#[test]
fn a_project_header_shows_how_far_its_own_cards_are() {
    assert!(salon::cards_spans(0, 0).is_empty());
    let bar = |done, all| spans_text(&salon::cards_spans(done, all));
    assert_eq!(bar(3, 8), "  ▓▓▓░░░░░ 3/8");
    assert_eq!(bar(1, 9), "  ░░░░░░░░ 1/9", "down to the cell");
    assert_eq!(bar(7, 7), "  ▓▓▓▓▓▓▓▓ 7/7");
    let mut app = app_after(2, 2, 0, Lang::En);
    let card = |project, column| Card {
        id: 0,
        project,
        title: "c".into(),
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
    app.world.cards = vec![
        card(0, Column::Done),
        card(0, Column::Working),
        card(0, Column::Waiting),
        card(1, Column::Done),
    ];
    let rungs = app.rungs();
    let line = |app: &App, p| spans_text(&salon::project_line(app, p, &rungs).spans);
    assert!(
        line(&app, 0).contains("  ▓▓░░░░░░ 1/3  "),
        "{}",
        line(&app, 0)
    );
    assert!(
        line(&app, 1).contains("  ▓▓▓▓▓▓▓▓ 1/1  "),
        "{}",
        line(&app, 1)
    );
    app.world.cards.clear();
    assert!(!line(&app, 0).contains('░'), "{}", line(&app, 0));
}

#[test]
fn a_row_done_and_not_seen_says_so_for_a_leader_too() {
    let mut app = idle_team(Lang::En);
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    let row = |app: &App, id: AgentId| {
        salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200)
    };
    let text = |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
    assert_eq!(row(&app, worker).spans[1].content, "○ ");
    app.unseen.insert(worker);
    let line = row(&app, worker);
    assert_eq!(
        (line.spans[1].content.as_ref(), line.spans[1].style),
        ("✓ ", fg(pal().ok))
    );
    assert!(
        text(&line).contains("done, not seen yet"),
        "{}",
        text(&line)
    );
    assert!(!text(&line).contains(" idle"), "instead of idle");
    app.unseen.insert(0);
    let line = row(&app, 0);
    assert_eq!(line.spans[1].content, "✓ ");
    assert!(text(&line).contains("done, not seen yet"));
    app.lang = Lang::Tr;
    assert!(text(&row(&app, worker)).contains("bitti, henüz bakılmadı"));
}

#[test]
fn a_worker_s_row_says_where_its_worktree_stands_and_only_what_there_is() {
    let text = |spans: &[Span]| -> String { spans.iter().map(|s| s.content.as_ref()).collect() };
    let stand = |branch: Option<&str>, ahead, behind, added, removed| Stand {
        branch: branch.map(String::from),
        ahead,
        behind,
        added,
        removed,
    };
    assert!(salon::stand_spans(&Stand::default(), None).is_empty());
    let all = salon::stand_spans(&stand(Some("fix-parser"), 2, 1, 12, 3), None);
    assert_eq!(text(&all), "  ⎇ fix-parser ↑2 ↓1 +12 −3");
    let styles: Vec<Style> = all.iter().skip(1).step_by(2).map(|s| s.style).collect();
    assert_eq!(
        styles,
        [
            dim(),
            fg(pal().info),
            fg(pal().warn),
            fg(pal().ok),
            fg(pal().bad)
        ]
    );
    // Each part alone, the first after two spaces.
    assert_eq!(
        text(&salon::stand_spans(&stand(None, 1, 0, 0, 0), None)),
        "  ↑1"
    );
    assert_eq!(
        text(&salon::stand_spans(&stand(None, 0, 4, 0, 0), None)),
        "  ↓4"
    );
    assert_eq!(
        text(&salon::stand_spans(&stand(None, 0, 0, 5, 0), None)),
        "  +5"
    );
    assert_eq!(
        text(&salon::stand_spans(&stand(None, 0, 0, 0, 6), None)),
        "  −6"
    );
    assert_eq!(
        text(&salon::stand_spans(&stand(None, 0, 0, 5, 6), None)),
        "  +5 −6"
    );
    // An agent's branch name is cleaned and cut.
    let long = salon::stand_spans(
        &stand(Some("a-very-long-branch-name\u{1b}[2J"), 0, 0, 0, 0),
        None,
    );
    assert_eq!(text(&long), "  ⎇ a-very-long-bra…");
    // The start point it was hired from, when no branch is named; a
    // branch the agent made wins.
    let from = salon::stand_spans(&stand(None, 0, 3, 0, 0), Some("#41"));
    assert_eq!(text(&from), "  ⎇ #41 ↓3");
    let both = salon::stand_spans(&stand(Some("wip"), 0, 0, 0, 0), Some("#41"));
    assert_eq!(text(&both), "  ⎇ wip");

    // On a worker's row, after its cost; never on a leader's.
    let mut app = idle_team(Lang::En);
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    let row = |app: &App, id: AgentId| {
        let line = salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200);
        text(&line.spans)
    };
    app.world.agents[worker].stand = stand(None, 0, 0, 7, 2);
    app.world.agents[worker].cost = 150;
    let want = format!("{}  +7 −2", dollars(150));
    assert!(row(&app, worker).ends_with(&want), "{}", row(&app, worker));
    app.world.agents[0].stand = stand(None, 0, 0, 7, 2);
    assert!(!row(&app, 0).contains("+7"), "{}", row(&app, 0));
}

#[test]
fn the_turkish_help_lists_the_keys_the_english_help_lists() {
    // A key is the same in both languages; only the words that stand
    // for what the person types are translated.
    let words = [
        ("text", "mesaj"),
        ("name", "ad"),
        ("NAME", "AD"),
        ("PATH", "YOL"),
        ("cmd", "kmt"),
        ("word", "kelime"),
    ];
    let turkish = |key: &str| {
        key.split(' ')
            .map(|token| {
                (words.iter())
                    .find_map(|(en, tr)| {
                        let head = token.trim_end_matches(en);
                        (head.len() < token.len() && head.chars().all(|c| "@/!#".contains(c)))
                            .then(|| format!("{head}{tr}"))
                    })
                    .unwrap_or_else(|| token.to_string())
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let en = overlay::help_keys(Lang::En, false);
    let tr = overlay::help_keys(Lang::Tr, false);
    assert_eq!(en.len(), tr.len());
    for ((e, _), (t, _)) in en.iter().zip(&tr) {
        assert_eq!(turkish(e), *t, "the Turkish row of {e:?}");
    }
    assert_eq!(turkish("@name text"), "@ad mesaj", "the words, translated");
    assert_eq!(turkish("J  :journal"), "J  :journal", "a key stays");
}

fn told(a: &mut Agent, with: Party, mine: bool, text: &str) {
    a.talk = Some(Talk {
        with,
        mine,
        text: text.into(),
    });
}

#[test]
fn quoted_cleans_cuts_to_the_room_and_says_nothing_under_the_minimum() {
    assert_eq!(quoted("abc", 8).as_deref(), Some("\"abc\""));
    assert_eq!(quoted("abc", 7), None, "under QUOTE_MIN");
    assert_eq!(
        quoted("abcdefgh", 8).as_deref(),
        Some("\"abcde…\""),
        "cut, its quotes inside the room"
    );
    assert_eq!(
        quoted("abcdef", 8).as_deref(),
        Some("\"abcdef\""),
        "six letters fit in eight"
    );
    assert_eq!(quoted("", 20), None);
    assert_eq!(quoted("   ", 20), None);
    assert_eq!(quoted("  x  ", 20).as_deref(), Some("\"x\""), "trimmed");
    let q = quoted("a\x1b[31mb", 20).unwrap();
    assert!(!q.contains('\x1b'), "{q:?}");
}

#[test]
fn a_status_carries_the_text_that_belongs_to_it() {
    let mut app = quiet_rows();
    let w = 1;
    let words = |app: &App, lang: Lang| -> String {
        status_spans(lang, &app.world, &app.world.agents[w])
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    };
    // An answer not read yet: its first line.
    app.world.replied(w, "\nUse the enum.\nmore");
    assert_eq!(words(&app, Lang::En), "answered you \"Use the enum.\"");
    app.world.inbox.clear();
    // A tool call waiting for a yes: what it would do, on one line.
    app.world.agents[w].status = Status::Working;
    app.world.asks_you(w, "cargo test\n-q");
    assert_eq!(words(&app, Lang::En), "⚑ asks to run \"cargo test ↵ -q\"");
    // The forty columns: the mark and the words take 13, a space and
    // the quotes 3, so 24 letters show whole and 25 are cut.
    app.world.inbox.clear();
    app.world.asks_you(w, &"c".repeat(24));
    assert!(words(&app, Lang::En).ends_with(&format!("\"{}\"", "c".repeat(24))));
    app.world.inbox.clear();
    app.world.asks_you(w, &"c".repeat(25));
    assert!(words(&app, Lang::En).ends_with(&format!("\"{}…\"", "c".repeat(23))));
    app.world.inbox.clear();
    // Blocked: what it waits for, then on whom.
    app.world.agents[w].status = Status::Blocked;
    app.world.agents[w].topic = "JWT clock skew".into();
    // The queue's words leave eight of the forty.
    assert_eq!(
        words(&app, Lang::En),
        "BLOCKED \"JWT c…\" in the researcher queue"
    );
    let r = app.world.agents.len();
    app.world.agents.push(Agent::new(
        r,
        0,
        "researcher-1".into(),
        Role::Researcher,
        Status::Idle,
        0,
    ));
    app.world.agents[w].researcher = Some(r);
    let name = app.world.agents[r].name.clone();
    assert_eq!(
        words(&app, Lang::En),
        format!("BLOCKED \"JWT clock skew\" ← {name}")
    );
    // The topic gets what the wait leaves of the forty columns, eight
    // at the least: BLOCKED, a space, ` ← ` and 21 letters leave 8.
    app.world.agents[r].name = "r".repeat(21);
    assert_eq!(
        words(&app, Lang::En),
        format!("BLOCKED \"JWT c…\" ← {}", "r".repeat(21))
    );
    app.world.agents[r].name = "r".repeat(22);
    assert_eq!(
        words(&app, Lang::En),
        format!("BLOCKED ← {}", "r".repeat(22)),
        "seven would be left: no quote"
    );
    // Stopped: why, within the forty columns; without a why, what to
    // do about it.
    app.world.agents[w].status = Status::Stopped;
    app.world.agents[w].topic = "exited with code 1".into();
    assert_eq!(words(&app, Lang::En), "■ session ended: exited with code 1");
    assert_eq!(words(&app, Lang::Tr), "■ oturum bitti: exited with code 1");
    app.world.agents[w].topic = "x".repeat(50);
    let said = words(&app, Lang::En);
    assert_eq!(said.chars().count(), 40, "{said}");
    assert!(
        said.starts_with("■ session ended: xxx") && said.ends_with('…'),
        "{said}"
    );
    app.world.agents[w].topic.clear();
    assert_eq!(
        words(&app, Lang::En),
        "■ session ended · write to it to restart"
    );
}

#[test]
fn a_row_quotes_its_newest_exchange_only_when_the_status_says_no_text() {
    let mut app = quiet_rows();
    let (w, other) = (1, 2);
    let lead = app.world.projects[0].leader;
    app.world.agents[other].name = "bob".into();
    let other_name = app.world.agents[other].name.clone();
    let lead_name = app.world.agents[lead].name.clone();
    // Idle: what it last said.
    told(
        &mut app.world.agents[w],
        Party::Person,
        true,
        "parser fixed",
    );
    let r = row(&app, w, 200);
    assert!(r.contains("idle → you \"parser fixed\""), "{r}");
    // Told by the lead: the lead's words, marked as told.
    told(
        &mut app.world.agents[w],
        Party::Agent(lead),
        false,
        "fix the parser",
    );
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("idle ← {lead_name} \"fix the parser\"")),
        "{r}"
    );
    // Working: while it thinks, the quote; what it does says enough.
    app.world.agents[w].status = Status::Working;
    app.world.agents[w].activity = Some(String::new());
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("thinking… ← {lead_name} \"fix the parser\"")),
        "{r}"
    );
    app.world.agents[w].activity = Some("Edit src/p.rs".into());
    let r = row(&app, w, 200);
    assert!(
        r.contains("Edit src/p.rs") && !r.contains("fix the parser"),
        "{r}"
    );
    app.world.agents[w].activity = None;
    let r = row(&app, w, 200);
    assert!(
        r.contains("\"fix the parser\""),
        "the bar says nothing: {r}"
    );
    app.world.agents[w].starting = true;
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("starting… ← {lead_name} \"fix the parser\"")),
        "{r}"
    );
    app.world.agents[w].starting = false;
    // Asking: its own question to the one it waits for is quoted
    // alone, the status names them; another's word, or a word to
    // someone else, says whose.
    app.world.agents[w].status = Status::Asking {
        to: other,
        until: 0,
    };
    told(
        &mut app.world.agents[w],
        Party::Agent(other),
        true,
        "which parser?",
    );
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("? → {other_name} \"which parser?\"")),
        "{r}"
    );
    assert!(!r.contains(&format!("{other_name} → ")), "{r}");
    told(
        &mut app.world.agents[w],
        Party::Agent(other),
        false,
        "the fast one",
    );
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("? → {other_name} ← {other_name} \"the fast one\"")),
        "{r}"
    );
    told(
        &mut app.world.agents[w],
        Party::Agent(lead),
        true,
        "which parser?",
    );
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("? → {other_name} → {lead_name} \"which parser?\"")),
        "{r}"
    );
    // A status with a text of its own quotes nothing more.
    told(
        &mut app.world.agents[w],
        Party::Person,
        true,
        "parser fixed",
    );
    app.world.agents[w].topic = "a topic".into();
    for status in [
        Status::Blocked,
        Status::Researching {
            for_agent: other,
            until: 0,
        },
        Status::Stopped,
    ] {
        app.world.agents[w].status = status;
        assert!(!row(&app, w, 200).contains("parser fixed"), "{status:?}");
    }
    app.world.agents[w].topic.clear();
    let r = row(&app, w, 200);
    assert!(
        r.contains("■ session ended · write to it to restart") && !r.contains('"'),
        "stopped without a why: what to do fills the column: {r}"
    );
    app.world.agents[w].status = Status::Idle;
    app.world.replied(w, "done");
    assert!(
        !row(&app, w, 200).contains("parser fixed"),
        "the answer says more"
    );
    app.world.inbox.clear();
    app.world.asked_you(w, "may I?");
    assert!(
        !row(&app, w, 200).contains("parser fixed"),
        "the question says more"
    );
    app.world.inbox.clear();
    app.world.agents[w].status = Status::Working;
    app.world.asks_you(w, "rm -r build");
    assert!(
        !row(&app, w, 200).contains("parser fixed"),
        "the ask says more"
    );
    app.world.inbox.clear();
    app.world.agents[w].status = Status::WaitingHuman;
    let r = row(&app, w, 200);
    assert!(r.contains("needs you → you \"parser fixed\""), "{r}");
    app.world.agents[w].status = Status::Asleep;
    let r = row(&app, w, 200);
    assert!(
        r.contains("write to it to wake it") && !r.contains('"'),
        "asleep: the hint fills the column: {r}"
    );
    // Done, not seen: what it said at the end of its turn.
    app.world.agents[w].status = Status::Idle;
    app.unseen.insert(w);
    let r = row(&app, w, 200);
    assert!(
        r.contains("done, not seen yet → you \"parser fixed\""),
        "{r}"
    );
    // The quote keeps to the status column: `idle`, a space and
    // `→ you ` leave 29 of its forty.
    app.unseen.clear();
    told(
        &mut app.world.agents[w],
        Party::Person,
        true,
        &"a".repeat(40),
    );
    let r = row(&app, w, 200);
    assert!(
        r.contains(&format!("idle → you \"{}…\"", "a".repeat(26))),
        "{r}"
    );
    assert!(!r.contains(&"a".repeat(27)), "{r}");
}

fn steps(n: usize, done: usize) -> Vec<Step> {
    (1..=n)
        .map(|i| Step {
            text: format!("s{i}"),
            state: if i <= done {
                StepState::Done
            } else if i == done + 1 {
                StepState::Doing
            } else {
                StepState::Todo
            },
        })
        .collect()
}

#[test]
fn a_row_marks_how_far_its_to_do_list_is_and_the_focus_lists_the_steps() {
    let mut app = quiet_rows();
    let w = 1;
    assert!(!row(&app, w, 200).contains('≡'));
    let a = &mut app.world.agents[w];
    a.plan = vec![
        Step {
            text: "Read the parser".into(),
            state: StepState::Done,
        },
        Step {
            text: "Change it".into(),
            state: StepState::Doing,
        },
        Step {
            text: "Test <it>".into(),
            state: StepState::Todo,
        },
    ];
    a.cost = 12_345;
    a.changed_at = 64;
    let r = row(&app, w, 200);
    assert!(r.contains("idle · 3 min  ≡ 1/3  $1.23"), "{r}");
    app.zoom = Zoom::Focus(w);
    let s = render(&app, 140, 40);
    for line in [
        "TO DO 1/3",
        "✓ Read the parser",
        "▸ Change it",
        "○ Test <it>",
    ] {
        assert!(s.contains(line), "{line}\n{s}");
    }
    app.lang = Lang::Tr;
    assert!(render(&app, 140, 40).contains("YAPILACAKLAR 1/3"));
    // No list, no title.
    app.world.agents[w].plan.clear();
    assert!(!render(&app, 140, 40).contains("YAPILACAKLAR"));
}

#[test]
fn a_long_to_do_list_folds_its_done_steps_and_counts_what_it_leaves_off() {
    let t = Lang::En.text();
    let text = |lines: Vec<Line>| -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    };
    // Eight fit whole.
    assert_eq!(
        text(focus::plan_lines(t, &steps(8, 2))),
        [
            "✓ s1", "✓ s2", "▸ s3", "○ s4", "○ s5", "○ s6", "○ s7", "○ s8"
        ]
    );
    // Twelve, four done: the done fold, the rest to the cap, the count.
    assert_eq!(
        text(focus::plan_lines(t, &steps(12, 4))),
        [
            "✓ 4 done",
            "▸ s5",
            "○ s6",
            "○ s7",
            "○ s8",
            "○ s9",
            "○ s10",
            "… 2 more"
        ]
    );
    // Nine with none done: nothing to fold, seven and the count.
    assert_eq!(
        text(focus::plan_lines(t, &steps(9, 0))),
        [
            "▸ s1",
            "○ s2",
            "○ s3",
            "○ s4",
            "○ s5",
            "○ s6",
            "○ s7",
            "… 2 more"
        ]
    );
    // All done: the fold alone.
    assert_eq!(text(focus::plan_lines(t, &steps(9, 9))), ["✓ 9 done"]);
    let tr = text(focus::plan_lines(Lang::Tr.text(), &steps(12, 4)));
    assert_eq!((tr[0].as_str(), tr[7].as_str()), ("✓ 4 bitti", "… 2 daha"));
    // The legend names the mark in both languages.
    assert!(
        t.help_legend[4].ends_with("  ≡ 3/7 its to-do steps done, ▸ the one in progress"),
        "{}",
        t.help_legend[4]
    );
    assert!(
        Lang::Tr.text().help_legend[4]
            .ends_with("  ≡ 3/7 yapılacaklarından biten, ▸ şu an yaptığı"),
        "{}",
        Lang::Tr.text().help_legend[4]
    );
}

#[test]
fn the_to_do_mark_gives_way_after_the_quote_and_the_time() {
    let mut app = quiet_rows();
    let (worker, _) = with_helpers(&mut app);
    let a = &mut app.world.agents[worker];
    a.status = Status::Idle;
    a.changed_at = 64;
    a.local = Some("http://localhost:5173/".into());
    a.plan = steps(3, 1);
    told(a, Party::Person, true, "parser fixed");
    // The 111 of the row without the mark, and the mark's 7.
    let whole = row(&app, worker, 200);
    assert_eq!(whole.chars().count(), 118, "{whole}");
    assert!(
        whole.contains("idle → you \"parser fixed\" · 3 min  ≡ 1/3  2 helpers  ↗ localhost:5173/"),
        "{whole}"
    );
    assert_eq!(row(&app, worker, 118), whole, "fits exactly");
    // One short: the quote shrinks first.
    let r = row(&app, worker, 117);
    assert!(
        r.contains("\"parser fix…\" · 3 min  ≡ 1/3  2 helpers"),
        "{r}"
    );
    // Then the quote goes, then the time; the mark stays longest.
    let r = row(&app, worker, 98);
    assert!(r.contains("idle · 3 min  ≡ 1/3  2 helpers"), "{r}");
    // The time needs its eight after the mark: at 97 exactly, at 96 not.
    let r = row(&app, worker, 97);
    assert!(r.contains("idle · 3 min  ≡ 1/3  2 helpers"), "{r}");
    let r = row(&app, worker, 96);
    assert!(
        r.contains("idle  ≡ 1/3  2 helpers") && !r.contains("min"),
        "{r}"
    );
    // The mark needs its seven: at 89 exactly, at 88 not.
    let r = row(&app, worker, 89);
    assert!(
        r.contains("idle  ≡ 1/3  2 helpers") && !r.contains("min"),
        "{r}"
    );
    let r = row(&app, worker, 88);
    assert!(r.contains("idle  2 helpers") && !r.contains('≡'), "{r}");
}

#[test]
fn the_activity_bar_shows_the_last_ticks_and_gives_way_first() {
    use crate::motion::Motion;
    let mut app = quiet_rows();
    app.motion = Motion::Calm;
    let (worker, _) = with_helpers(&mut app);
    for a in &mut app.world.agents {
        a.stirs = Stirs::default();
    }
    let a = &mut app.world.agents[worker];
    a.status = Status::Idle;
    a.changed_at = 64;
    a.local = Some("http://localhost:5173/".into());
    a.plan = steps(3, 1);
    told(a, Party::Person, true, "parser fixed");
    // Nothing written lately: no bar.
    let without = row(&app, worker, 200);
    assert!(!without.contains('▁'), "{without}");
    // One line four ticks ago, three now.
    let a = &mut app.world.agents[worker];
    a.trace(96, "tool_use Read");
    for _ in 0..3 {
        a.trace(100, "tool_use Edit");
    }
    let whole = row(&app, worker, 200);
    assert!(
        whole.contains("idle → you \"parser fixed\"  ▂▁▁▁▄ · 3 min  ≡ 1/3  2 helpers"),
        "{whole}"
    );
    // The bar's seven: at the row's width exactly it stays.
    let n = whole.chars().count();
    assert_eq!(n, without.chars().count() + 7, "{whole}");
    assert_eq!(row(&app, worker, n), whole, "fits exactly");
    // One short: the bar goes, the quote keeps all of its words.
    let r = row(&app, worker, n - 1);
    assert_eq!(r, without, "the bar gives way before the quote");
    // A quote cut short leaves no room for the bar either.
    let r = row(&app, worker, n - 9);
    assert!(
        r.contains("\"parser fi…\" · 3 min") && !r.contains('▁'),
        "{r}"
    );
    assert!(r.chars().count() <= n - 9, "{r}");
    // A tick on, the columns move left; five on, it is gone.
    app.world.tick = 101;
    assert!(row(&app, worker, 200).contains("\"  ▁▁▁▄▁ · 3 min"));
    app.world.tick = 104;
    assert!(row(&app, worker, 200).contains("\"  ▄▁▁▁▁ · "));
    app.world.tick = 105;
    assert_eq!(row(&app, worker, 200), without);
    // Motion off: no bar, whatever was written.
    app.world.tick = 100;
    app.motion = Motion::Off;
    assert_eq!(row(&app, worker, 200), without);
    app.motion = Motion::Full;
    assert!(row(&app, worker, 200).contains("  ▂▁▁▁▄ · "));
    let spans = salon::stir_spans(&app, &app.world.agents[worker]);
    assert_eq!(spans[0].style, dim(), "quiet");
    // One scale for every row, a column per count.
    let bar: String = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 14, 15, 200, u16::MAX]
        .into_iter()
        .map(salon::stir_glyph)
        .collect();
    assert_eq!(bar, "▁▂▃▄▄▅▅▆▆▇▇███");
    // The legend names the bar in both languages.
    assert!(
        Lang::En.text().help_legend[5].ends_with("  ▁▃▅ what it did, 5 s a column"),
        "{}",
        Lang::En.text().help_legend[5]
    );
    assert!(
        Lang::Tr.text().help_legend[5].ends_with("  ▁▃▅ yaptıkları, sütun başı 5 sn"),
        "{}",
        Lang::Tr.text().help_legend[5]
    );
}

#[test]
fn the_demo_s_working_agents_wear_the_activity_bar() {
    use crate::motion::Motion;
    let mut app = app_after(1, 4, 60, Lang::En);
    app.motion = Motion::Calm;
    let tick = app.world.tick;
    let busy: Vec<AgentId> = (app.world.agents.iter())
        .filter(|a| a.stirs.at(tick).iter().any(|&n| n > 0))
        .map(|a| a.id)
        .collect();
    assert!(!busy.is_empty(), "the demo writes traces");
    for id in busy {
        let r = row(&app, id, 300);
        assert!(r.chars().any(|c| ('▁'..='█').contains(&c)), "{r}");
    }
    app.motion = Motion::Off;
    assert!(!render(&app, 200, 45).contains("▁"));
}

#[test]
fn a_row_warns_of_a_full_context_from_seventy_percent_and_the_focus_has_the_figure() {
    let mut app = quiet_rows();
    let w = 1;
    let fill = |used, size| {
        Some(Fill {
            used,
            size: Some(size),
        })
    };
    app.world.agents[w].context = fill(139_999, 200_000);
    assert!(!row(&app, w, 200).contains('◔'), "69%: not on the row");
    app.world.agents[w].context = fill(140_000, 200_000);
    let r = row(&app, w, 200);
    assert!(r.contains("idle  ◔ 70%"), "{r}");
    let spans = salon::context_spans(&app.world.agents[w]);
    assert_eq!(spans[0].style.fg, dim().fg, "quiet under ninety");
    app.world.agents[w].context = fill(179_999, 200_000);
    assert_eq!(
        salon::context_spans(&app.world.agents[w])[0].style.fg,
        dim().fg
    );
    app.world.agents[w].context = fill(180_000, 200_000);
    let spans = salon::context_spans(&app.world.agents[w]);
    assert_eq!(spans[0].content.as_ref(), "  ◔ 90%");
    assert_eq!(spans[0].style.fg, Some(pal().warn), "loud from ninety");
    // No size: nothing on the row, the tokens on the focus.
    app.world.agents[w].context = Some(Fill {
        used: 45_506,
        size: None,
    });
    assert!(salon::context_spans(&app.world.agents[w]).is_empty());
    app.zoom = Zoom::Focus(w);
    let s = render(&app, 140, 40);
    assert!(s.contains("context:  45k tokens"), "{s}");
    app.world.agents[w].context = fill(45_506, 200_000);
    let s = render(&app, 140, 40);
    assert!(s.contains("context:  22% · 45k of 200k"), "{s}");
    // The focus line is loud from ninety too, by the exact token.
    let colour = |app: &App| {
        let buf = buffer(app, 140, 40);
        let y = (0..40)
            .find(|&y| {
                (0..140)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("context:")
            })
            .unwrap();
        let x = (0..140).find(|&x| buf[(x, y)].symbol() == "c").unwrap();
        buf[(x, y)].style().fg
    };
    app.world.agents[w].context = fill(179_999, 200_000);
    assert_eq!(colour(&app), dim().fg, "89%: quiet");
    app.world.agents[w].context = fill(180_000, 200_000);
    assert_eq!(colour(&app), Some(pal().warn), "90%: loud");
    app.world.agents[w].context = fill(45_506, 200_000);
    app.lang = Lang::Tr;
    let s = render(&app, 140, 40);
    assert!(s.contains("bağlam:   %22 · 45k / 200k"), "{s}");
    app.world.agents[w].context = None;
    assert!(!render(&app, 140, 40).contains("bağlam:"));
    // Token counts in a few characters.
    for (n, s) in [
        (980, "980"),
        (1_000, "1k"),
        (45_506, "45k"),
        (999_999, "999k"),
        (1_000_000, "1M"),
        (1_250_000, "1.2M"),
        (2_000_000, "2M"),
    ] {
        assert_eq!(words::tokens(n), s, "{n}");
    }
    // The legend names the mark in both languages.
    assert!(
        Lang::En.text().help_legend[2].ends_with("  ◔ 72% of its context window used (from 70%)")
    );
    assert!(Lang::Tr.text().help_legend[2].ends_with("  ◔ %72 bağlam penceresi dolu (%70'ten)"));
}

#[test]
fn the_context_warning_stays_on_the_row_longer_than_the_to_do_mark() {
    let mut app = quiet_rows();
    let (worker, _) = with_helpers(&mut app);
    let a = &mut app.world.agents[worker];
    a.status = Status::Idle;
    a.changed_at = 64;
    a.local = Some("http://localhost:5173/".into());
    a.plan = steps(3, 1);
    a.context = Some(Fill {
        used: 150_000,
        size: Some(200_000),
    });
    told(a, Party::Person, true, "parser fixed");
    // The 118 of the row with the mark, and the warning's 7.
    let whole = row(&app, worker, 200);
    assert_eq!(whole.chars().count(), 125, "{whole}");
    assert!(
        whole.contains("\"parser fixed\" · 3 min  ≡ 1/3  ◔ 75%  2 helpers"),
        "{whole}"
    );
    // Head 52, tail 32: the warning needs its seven, the mark seven
    // more, the time eight more.
    let r = row(&app, worker, 104);
    assert!(r.contains("idle · 3 min  ≡ 1/3  ◔ 75%  2 helpers"), "{r}");
    let r = row(&app, worker, 103);
    assert!(
        r.contains("idle  ≡ 1/3  ◔ 75%  2 helpers") && !r.contains("min"),
        "{r}"
    );
    let r = row(&app, worker, 96);
    assert!(r.contains("idle  ≡ 1/3  ◔ 75%  2 helpers"), "{r}");
    let r = row(&app, worker, 95);
    assert!(
        r.contains("idle  ◔ 75%  2 helpers") && !r.contains('≡'),
        "{r}"
    );
    let r = row(&app, worker, 89);
    assert!(r.contains("idle  ◔ 75%  2 helpers"), "{r}");
    let r = row(&app, worker, 88);
    assert!(r.contains("idle  2 helpers") && !r.contains('◔'), "{r}");
}

#[test]
fn going_to_a_helper_unfolds_its_worker_and_project_and_shows_everyone_again() {
    let mut app = quiet_rows();
    let (worker, [h1, _]) = with_helpers(&mut app);
    let helper = app.world.agents[h1].name.clone();
    app.world.agents[h1].status = Status::Idle;
    app.world.agents[h1].activity = None;
    app.collapsed[0] = true;
    app.zoom = Zoom::Focus(worker);
    key(&mut app, KeyCode::Char('&'));
    let s = render(&app, 160, 40);
    assert!(s.contains(" › go to: &"), "{s}");
    for c in helper.chars() {
        key(&mut app, KeyCode::Char(c));
    }
    let m = app.menu.as_ref().expect("the menu");
    assert!(m.items[0].ends_with(&format!(" {helper}")), "{:?}", m.items);
    assert_eq!(m.notes[0], "○ idle", "the helper with its glyph and status");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.selected, Sel::Agent(h1));
    assert!(!app.collapsed[0], "the project unfolded");
    assert_eq!((app.zoom, app.pane), (Zoom::None, Pane::Salon));
    assert_eq!(app.at_home.at, Some(crate::app::HomeRow::Agent(h1)));
    let s = render(&app, 160, 40);
    assert!(
        s.contains(&format!("└ ○ {helper}")),
        "under its worker: {s}"
    );
    // In Turkish the line says where it goes too; the help names the key.
    app.lang = Lang::Tr;
    key(&mut app, KeyCode::Char('&'));
    assert!(render(&app, 160, 40).contains(" › git: &"));
    for lang in [Lang::En, Lang::Tr] {
        assert!(
            lang.text().help_keys.iter().any(|(k, _)| *k == "&  :goto"),
            "{lang:?}"
        );
    }
}

#[test]
fn a_row_says_how_long_it_has_been_in_its_state_from_a_minute_on() {
    let mut app = quiet_rows();
    let w = 1;
    // Eleven ticks are 55 seconds; twelve, a minute.
    app.world.agents[w].changed_at = 89;
    let r = row(&app, w, 200);
    assert!(!r.contains(" · "), "{r}");
    app.world.agents[w].changed_at = 88;
    let r = row(&app, w, 200);
    assert!(r.contains("idle · 1 min"), "{r}");
    app.world.agents[w].changed_at = 76;
    let r = row(&app, w, 200);
    assert!(r.contains("idle · 2 min"), "{r}");
    // Something waiting for the person: since it came, not since the
    // status changed.
    app.world.agents[w].changed_at = 100;
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Question,
        agent: w,
        detail: "q?".into(),
        researcher: None,
        open: true,
        tick: 64,
        card: None,
    });
    let r = row(&app, w, 200);
    assert!(r.contains("? → you \"q?\" · 3 min"), "{r}");
    app.lang = Lang::Tr;
    let r = row(&app, w, 200);
    assert!(r.contains("? → sen \"q?\" · 3 dk"), "{r}");
}

#[test]
fn the_time_and_the_quote_give_way_to_the_row_s_fixed_parts() {
    let mut app = quiet_rows();
    let (worker, _) = with_helpers(&mut app);
    let a = &mut app.world.agents[worker];
    a.status = Status::Idle;
    a.changed_at = 64;
    a.local = Some("http://localhost:5173/".into());
    a.plan.clear();
    told(a, Party::Person, true, "parser fixed");
    let tail = "  2 helpers  ↗ localhost:5173/";
    // Glyph, mark, name, taint, card and `idle` take 52; the quote 21,
    // the time 8, the tail 32: 113 in all.
    let whole = row(&app, worker, 200);
    assert_eq!(whole.chars().count(), 111, "{whole}");
    assert!(
        whole.contains("idle → you \"parser fixed\" · 3 min") && whole.ends_with(tail),
        "{whole}"
    );
    assert_eq!(row(&app, worker, 111), whole, "fits exactly");
    // One short: the quote shrinks first, to what is left.
    let r = row(&app, worker, 110);
    assert!(r.contains("idle → you \"parser fix…\" · 3 min"), "{r}");
    assert!(r.ends_with(tail) && r.chars().count() <= 112, "{r}");
    // The shortest quote needs 15 free after the time: 107 shows one,
    // 106 keeps the time alone.
    let r = row(&app, worker, 105);
    assert!(r.contains("\"parse…\" · 3 min"), "{r}");
    let r = row(&app, worker, 104);
    assert!(r.contains("idle · 3 min") && !r.contains('"'), "{r}");
    assert!(r.ends_with(tail) && r.chars().count() <= 106, "{r}");
    // Eight columns free take the time, seven do not: the fixed parts
    // alone.
    let r = row(&app, worker, 90);
    assert!(r.contains("idle · 3 min") && r.ends_with(tail), "{r}");
    let r = row(&app, worker, 89);
    assert!(!r.contains(" · ") && !r.contains('"'), "{r}");
    assert!(r.ends_with(tail) && r.chars().count() <= 91, "{r}");
}

#[test]
fn the_floor_is_forty_columns_by_twelve_rows() {
    assert!(fits_floor((MIN_COLUMNS, MIN_ROWS)));
    assert!(!fits_floor((MIN_COLUMNS - 1, MIN_ROWS)));
    assert!(!fits_floor((MIN_COLUMNS, MIN_ROWS - 1)));
    assert!(fits_floor((u16::MAX, MIN_ROWS)));
    assert_eq!((MIN_COLUMNS, MIN_ROWS, NARROW), (40, 12, 80));
}

#[test]
fn under_the_floor_one_line_says_so_in_the_middle_and_nothing_else() {
    let app = app_after(1, 4, 0, Lang::En);
    assert!(!app.too_small(), "not drawn yet: keys act");
    let buf = buffer(&app, 39, 20);
    let row = |y: u16| -> String { (0..39).map(|x| buf[(x, y)].symbol()).collect() };
    let all: Vec<String> = (0..20).map(row).collect();
    // Two rows of words on twenty: from the tenth.
    assert_eq!(
        all[9].trim(),
        "this terminal is 39×20; imece needs",
        "{all:#?}"
    );
    assert_eq!(all[10].trim(), "40×12");
    let words = all.iter().filter(|r| !r.trim().is_empty());
    assert_eq!(words.count(), 2, "nothing else: {all:#?}");
    assert!(app.too_small());
    // One row short is under it too; the floor itself is not.
    assert!(render(&app, 120, MIN_ROWS - 1).contains("120×11"));
    assert!(app.too_small());
    let s = render(&app, MIN_COLUMNS, MIN_ROWS);
    assert!(s.starts_with(" imece ") || s.starts_with("  "), "{s}");
    assert!(!s.contains("this terminal"), "{s}");
    assert!(!app.too_small());
    // In Turkish too.
    let tr = app_after(1, 4, 0, Lang::Tr);
    assert!(render(&tr, 30, 30).contains("bu terminal 30×30; imece en az"));
    // A screen of no rows draws nothing and holds the keys.
    render(&app, 80, 0);
    assert!(app.too_small());
}

#[test]
fn under_the_floor_no_key_acts_but_ctrl_c_and_no_paste_lands() {
    let mut app = app_after(1, 4, 0, Lang::En);
    render(&app, MIN_COLUMNS - 1, 30);
    key(&mut app, KeyCode::Char(':'));
    assert!(!app.typing, "what was not shown is not decided");
    key(&mut app, KeyCode::Char('?'));
    assert!(!app.help);
    render(&app, MIN_COLUMNS, MIN_ROWS);
    key(&mut app, KeyCode::Char(':'));
    assert!(app.typing, "at the floor keys act");
    app.paste("ab");
    assert_eq!(app.input, ":ab");
    render(&app, 80, MIN_ROWS - 1);
    app.paste("cd");
    assert_eq!(app.input, ":ab", "a paste neither");
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.input, ":ab");
    ctrl(&mut app, 'c');
    assert!(app.quit, "ctrl-c always");
}
