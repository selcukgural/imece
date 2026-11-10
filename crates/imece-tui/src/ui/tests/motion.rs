use super::*;

/// The screen of `app` patched from `last` on a terminal of `width`
/// columns, if it could be.
fn patched(app: &App, last: &Buffer, width: u16) -> Option<(Buffer, Option<Position>)> {
    let mut t = Terminal::new(TestBackend::new(width, 40)).unwrap();
    let mut done = false;
    t.draw(|f| done = patch(f, app, last)).unwrap();
    let cursor = app.caret_at.get().map(|_| t.get_cursor_position().unwrap());
    done.then(|| (t.backend().buffer().clone(), cursor))
}

#[test]
fn a_frame_patched_for_the_breath_is_the_frame_drawn_whole() {
    use crate::motion::Motion;
    let ms = std::time::Duration::from_millis;
    let mut app = app_after(2, 10, 0, Lang::En);
    app.motion = Motion::Calm;
    let (mut checked, mut refused) = (0, 0);
    for round in 0..120 {
        app.step();
        // Folded projects, a selected row, the person writing, a narrow
        // screen: each in some rounds.
        app.collapsed[1] = round % 3 == 1;
        app.selected = match (app.world.agents.iter()).find(|a| breathes(a, app.rungs()[a.id])) {
            Some(a) if round % 4 == 2 => Sel::Agent(a.id),
            _ => Sel::Project(0),
        };
        if round == 30 {
            for c in ['/', 'h', 'i'] {
                app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
        }
        let width = if round % 5 == 4 { 70 } else { 160 };
        // Drawn whole in one tone; the next frame, in the other, is
        // patched where the main loop would patch it. Two seconds on,
        // when the glows the step lit are over.
        let since = std::time::Instant::now() + ms(2000) - app.after(ms(0));
        let beat = since.as_millis() as u64 / 600 * 600 + 600;
        for (from, to) in [(beat, beat + 600), (beat + 600, beat + 1200)] {
            app.frame_after(ms(from));
            let (first, _) = whole(&app, width);
            app.drawn(app.after(ms(from)));
            let glyphs = app.breathing.borrow().clone();
            let due = app.patch_due(app.after(ms(to)));
            app.frame_after(ms(to));
            let got = patched(&app, &first, width);
            let want = whole(&app, width);
            assert_eq!(*app.breathing.borrow(), glyphs, "the same glyphs");
            if due {
                assert_eq!(got, Some(want), "round {round}, {from} ms → {to} ms");
                checked += 1;
            } else {
                refused += 1;
            }
        }
    }
    // Enough of each: the demo lights rows and starts sessions often.
    assert!(checked > 40, "{checked} patched, {refused} not");
    assert!(refused > 10, "{checked} patched, {refused} not");
}

#[test]
fn a_working_owner_breathes_on_the_board_s_window_and_nothing_is_patched() {
    use crate::motion::Motion;
    let ms = std::time::Duration::from_millis;
    let (mut app, w, _) = calm_team();
    app.motion = Motion::Calm;
    app.window = Some(Window::Board);
    // The team under the window tells of cards the test replaces.
    app.world.journal.clear();
    app.world.inbox.clear();
    let card = |id, owner, column| Card {
        id,
        project: 0,
        title: format!("card {id}"),
        owner,
        column,
        story: Vec::new(),
        symbols: Vec::new(),
        paths: Vec::new(),
        checks: Vec::new(),
        started: 0,
        finished: 0,
        attempt: 1,
        breaker: Default::default(),
        after: Vec::new(),
    };
    app.world.cards = vec![
        card(0, Some(w[0]), Column::Working),
        card(1, Some(w[1]), Column::Working),
        card(2, None, Column::Waiting),
        card(3, Some(w[3]), Column::Done),
        card(4, None, Column::Waiting),
    ];
    app.world.agents[w[1]].status = Status::Idle;
    app.world.agents[w[3]].status = Status::Working;
    let width = 160;
    // The window starts four columns and two rows in, its border one
    // more; in progress is the second of five columns of 30, its
    // border at 35, its rows under its own border (y 4); the glyph
    // after four cells, two wide, under the card's title.
    let (x, y) = (40, 5);
    app.frame_after(ms(600));
    let (first, _) = whole(&app, width);
    assert_eq!(
        *app.breathing.borrow(),
        None,
        "a window over the screen: no patch"
    );
    assert!(!app.patchable());
    assert_eq!(first[(x, y)].symbol(), "●");
    assert!(
        first[(x, y)].modifier.contains(Modifier::DIM),
        "the second tone"
    );
    assert_eq!(first[(x, y + 2)].symbol(), "○", "the idle owner");
    // A waiting card nobody holds and that waits on nothing takes one
    // row: the next card's title right under it.
    assert_eq!(first[(6, 4)].symbol(), "#");
    assert_eq!(first[(7, 4)].symbol(), "3");
    assert_eq!(first[(6, 5)].symbol(), "#");
    assert_eq!(first[(7, 5)].symbol(), "5");
    assert!(!first[(x, y + 2)].modifier.contains(Modifier::DIM));
    // The next beat draws the first tone, whole.
    app.drawn(app.after(ms(600)));
    assert!(!app.patch_due(app.after(ms(1200))));
    app.frame_after(ms(1200));
    assert!(patched(&app, &first, width).is_none());
    let (second, _) = whole(&app, width);
    assert_eq!(second[(x, y)].symbol(), "●");
    assert!(
        !second[(x, y)].modifier.contains(Modifier::DIM),
        "the first tone"
    );
}

#[test]
fn a_working_row_breathes_in_its_colour_never_its_shape() {
    use crate::motion::Motion;
    let ms = std::time::Duration::from_millis;
    let (mut app, w, _) = calm_team();
    let lead = app.world.projects[0].leader;
    let glyph = |app: &App, id: AgentId| {
        let line = salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200);
        (line.spans[1].content.to_string(), line.spans[1].style)
    };
    let ok = fg(pal().ok);
    let out = ok.add_modifier(Modifier::DIM);
    assert_eq!(glyph(&app, w[0]), ("● ".into(), ok), "off: still");
    app.motion = Motion::Calm;
    app.frame_after(ms(600));
    assert_eq!(glyph(&app, w[0]), ("● ".into(), out), "the second tone");
    assert_eq!(glyph(&app, lead), ("● ".into(), ok), "a leader's does not");
    app.frame_after(ms(1200));
    assert_eq!(glyph(&app, w[0]), ("● ".into(), ok));
    app.frame_after(ms(600));
    app.world.agents[w[1]].status = Status::Idle;
    assert_eq!(glyph(&app, w[1]), ("○ ".into(), fg(pal().dim)), "idle");
    waiting(&mut app, w[2]);
    assert_eq!(glyph(&app, w[2]), ("! ".into(), fg(pal().warn)), "waiting");
    // A folded project's loudest member breathes with it.
    app.world.inbox.clear();
    app.world.agents[lead].status = Status::Idle;
    app.collapsed[0] = true;
    let header = salon::project_line(&app, 0, &app.rungs());
    let member = &header.spans[header.spans.len() - 2];
    assert_eq!((member.content.as_ref(), member.style), ("  ● ", out));
    app.frame_after(ms(0));
    let header = salon::project_line(&app, 0, &app.rungs());
    assert_eq!(header.spans[header.spans.len() - 2].style, ok);
}

#[test]
fn a_lit_row_blinks_or_settles_and_the_selection_stays_reversed() {
    use crate::motion::{Glow, GlowOn, Lit, Motion};
    let ms = std::time::Duration::from_millis;
    let reversed = Style::new().add_modifier(Modifier::REVERSED);
    assert_eq!(lit_style(Lit::Reversed), reversed);
    // Settling: the theme's `ok` toward its background, step by step.
    assert!(crate::theme::set("gruvbox-dark-medium"));
    let (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) = (pal().ok, pal().bg) else {
        panic!("an RGB theme");
    };
    let shade = |percent| {
        let c = crate::theme::Rgb(br, bg, bb).blend(crate::theme::Rgb(r, g, b), percent);
        Style::new().bg(Color::Rgb(c.0, c.1, c.2))
    };
    assert_eq!(lit_style(Lit::Settle(0)), shade(35));
    assert_eq!(lit_style(Lit::Settle(1)), shade(20));
    assert_eq!(lit_style(Lit::Settle(2)), shade(8));
    assert_eq!(lit_style(Lit::Settle(9)), shade(8), "no step past the last");
    assert_ne!(shade(35), shade(20));
    // Lines pointed at: the theme's `accent`, five steps.
    let (Color::Rgb(ar, ag, ab), true) = (pal().accent, pal().accent != pal().ok) else {
        panic!("an accent of its own");
    };
    let soft = |percent| {
        let c = crate::theme::Rgb(br, bg, bb).blend(crate::theme::Rgb(ar, ag, ab), percent);
        Style::new().bg(Color::Rgb(c.0, c.1, c.2))
    };
    for (step, percent) in [(0, 30), (1, 24), (2, 18), (3, 12), (4, 6), (9, 6)] {
        assert_eq!(lit_style(Lit::Point(step)), soft(percent), "step {step}");
    }
    // A theme of the terminal's own colours: reversed, then calmer.
    assert!(crate::theme::set("terminal"));
    assert_eq!(lit_style(Lit::Settle(0)), reversed);
    assert_eq!(lit_style(Lit::Settle(1)), changed());
    assert_eq!(lit_style(Lit::Settle(2)), changed());
    // Many rows are never reversed: lines pointed at stay as they are.
    assert_eq!(lit_style(Lit::Point(0)), Style::new());
    assert_eq!(lit_style(Lit::Point(3)), Style::new());
    let (mut app, w, _) = calm_team();
    let row = |app: &App, id: AgentId| {
        salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200).style
    };
    app.motion = Motion::Calm;
    app.frame_after(ms(0));
    app.light_now(GlowOn::Agent(w[0]), Glow::Arrive);
    app.light_now(GlowOn::Agent(w[1]), Glow::Settle);
    assert_eq!(row(&app, w[0]), reversed, "a blink");
    assert_eq!(row(&app, w[1]), lit_style(Lit::Settle(0)));
    assert_eq!(row(&app, w[2]), Style::default());
    app.frame_after(ms(300));
    assert_eq!(row(&app, w[0]), Style::default(), "between blinks");
    assert_eq!(row(&app, w[1]), lit_style(Lit::Settle(1)));
    app.world.agents[w[0]].changed_at = app.world.tick;
    assert_eq!(row(&app, w[0]), changed(), "a change shows as before");
    app.frame_after(ms(600));
    assert_eq!(row(&app, w[0]), reversed, "lit over a change");
    app.pane = Pane::Salon;
    app.selected = Sel::Agent(w[0]);
    app.frame_after(ms(300));
    assert_eq!(row(&app, w[0]), reversed, "the selection, not a blink");
    app.motion = Motion::Off;
    app.frame_after(ms(0));
    assert_eq!(row(&app, w[1]), Style::default(), "off: nothing lit");
}

#[test]
fn a_change_opened_from_its_log_line_marks_the_file_and_lights_it_while_motion_moves() {
    use crate::motion::{Glow, GlowOn, Lit, Motion};
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let ms = std::time::Duration::from_millis;
    assert!(crate::theme::set("gruvbox-dark-medium"));
    let mut app = app_after(1, 2, 0, Lang::En);
    let id = app.world.projects[0].leader + 1;
    let c = app.world.cards[0].id;
    app.world.agents[id].card = Some(c);
    app.world.cards[c].story.clear();
    app.world.story(Some(c), StoryEvent::Touched("b.rs".into()));
    app.zoom = Zoom::Focus(id);
    let foot = |app: &App| render(app, 100, 30).lines().last().unwrap().to_string();
    // The footer says what `enter` opens on a change line.
    let calm = foot(&app);
    assert!(
        calm.contains("   enter  its lines   @  write to this agent   esc  back"),
        "{calm}"
    );
    // Three keys for an ask leave it to the help.
    app.world.asks_you(id, "Bash: make");
    let asked = foot(&app);
    assert!(
        asked.contains("   a  allow once   r  refuse   e  yes this turn"),
        "{asked}"
    );
    assert!(!asked.contains("its lines"), "{asked}");
    let ask = app.world.open_inbox()[0];
    app.world.decided(ask, true);
    app.world.change_ready(crate::model::Change {
        agent: id,
        title: "did it".into(),
        files: Vec::new(),
        patch: "diff --git a/a.rs b/a.rs\n@@ -1 +1 @@\n-a\n+b\n\
                diff --git a/b.rs b/b.rs\n@@ -2 +2 @@\n-c\n+d\n"
            .into(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    let change = foot(&app);
    assert!(
        change.contains("   enter  its lines   a  approve   r  send back   esc  back"),
        "{change}"
    );
    app.lang = Lang::Tr;
    assert!(
        foot(&app).contains("   enter  satırları   a  "),
        "{}",
        foot(&app)
    );
    app.lang = Lang::En;
    app.motion = Motion::Calm;
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.zoom, Zoom::Diff(id));
    // The background of the first cell of the row that reads `text`.
    let back = |app: &App, text: &str| {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        let rows: Vec<String> = (buf.content.chunks(100))
            .map(|row| row.iter().map(|c| c.symbol()).collect())
            .collect();
        let y = (rows.iter().position(|r| r.contains(text))).unwrap_or_else(|| panic!("{rows:#?}"));
        let x = rows[y].find(text).unwrap();
        let x = rows[y][..x].chars().count();
        buf[(x as u16, y as u16)].bg
    };
    let shade = |step| lit_style(Lit::Point(step)).bg.unwrap();
    app.frame_after(ms(0));
    app.light_now(GlowOn::Lines, Glow::Point);
    // The header is the line at hand, and says where it came from;
    // the file's lines under it are lit, a step calmer each beat.
    assert_eq!(back(&app, "b.rs  ← the log's line"), pal().selection);
    assert_eq!(back(&app, "@@ -2 +2 @@"), shade(0));
    assert_eq!(back(&app, "+d"), shade(0));
    assert_ne!(
        back(&app, "cost ·"),
        shade(0),
        "what is above the patch is not lit"
    );
    app.frame_after(ms(300));
    assert_eq!(back(&app, "+d"), shade(1));
    app.frame_after(ms(1200));
    assert_eq!(back(&app, "+d"), shade(4));
    assert_ne!(shade(4), shade(0));
    app.frame_after(ms(1500));
    let plain = back(&app, "cost ·");
    assert_ne!(plain, shade(4));
    assert_eq!(back(&app, "+d"), plain, "over: the words alone");
    assert_eq!(back(&app, "b.rs  ← the log's line"), pal().selection);
    // Nothing moves: the words mark the file, nothing is lit.
    app.frame_after(ms(0));
    app.motion = Motion::Off;
    assert_eq!(back(&app, "+d"), plain);
    assert_eq!(back(&app, "b.rs  ← the log's line"), pal().selection);
    // Opened as any change is, it has no mark and no light.
    app.motion = Motion::Calm;
    app.pointed = None;
    assert_eq!(back(&app, "+d"), plain);
    assert!(!render(&app, 100, 30).contains("the log's line"));
    assert!(crate::theme::set("terminal"));
}

#[test]
fn starting_counts_its_dots_while_motion_moves() {
    use crate::motion::Motion;
    let ms = std::time::Duration::from_millis;
    let (mut app, w, _) = calm_team();
    app.world.agents[w[0]].starting = true;
    let row = |app: &App| {
        let line = salon::agent_line(app, &app.world.agents[w[0]], "", &app.rungs(), 200);
        text(&[line]).remove(0)
    };
    let name = app.world.agents[w[0]].name.clone();
    assert!(row(&app).contains(" starting…"), "{}", row(&app));
    app.motion = Motion::Calm;
    for (at, dots) in [(0, "   "), (600, ".  "), (1200, ".. "), (1800, "...")] {
        app.frame_after(ms(at));
        let r = row(&app);
        assert!(r.contains(&format!(" starting{dots}")), "{at}: {r}");
        assert!(!r.contains('…') && r.contains(&name), "{at}: {r}");
    }
    app.world.agents[w[0]].starting = false;
    assert!(!row(&app).contains("starting"));
}

#[test]
fn the_bottom_line_spins_while_an_answer_is_waited_for_and_full_spins_by_the_fleet() {
    use crate::motion::{Motion, SPIN};
    let ms = std::time::Duration::from_millis;
    let (mut app, _, _) = calm_team();
    let foot = |app: &App| render(app, 160, 30).lines().last().unwrap().to_string();
    let head = |app: &App| render(app, 160, 30).lines().next().unwrap().to_string();
    app.asking = Some("asking claude for a commit message".into());
    assert!(foot(&app).starts_with(" … asking claude"), "{}", foot(&app));
    app.motion = Motion::Calm;
    app.frame_after(ms(300));
    assert!(foot(&app).starts_with(&format!(" {} asking claude", SPIN[1])));
    app.flash = Some(("said".into(), std::time::Instant::now()));
    assert!(foot(&app).starts_with(" said"), "a flash first");
    app.flash = None;
    app.asking = None;
    assert!(!foot(&app).contains("asking"));
    // Full: by the fleet while anyone works.
    assert!(!head(&app).contains(SPIN[1]), "calm: none");
    app.motion = Motion::Full;
    app.frame_after(ms(100));
    assert!(
        head(&app).contains(&format!("{} ", SPIN[1])),
        "{}",
        head(&app)
    );
    let at = head(&app).find(SPIN[1]).unwrap();
    let name = app.world.projects[0].name.clone();
    assert!(head(&app)[at..].contains(&name), "before the projects");
}

#[test]
fn the_first_run_window_spins_while_claude_code_is_checked() {
    use crate::motion::{Motion, SPIN};
    use crate::welcome::Seen;
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    app.seen = Seen::Checking;
    app.welcome = true;
    assert!(render(&app, 120, 40).contains("… claude"));
    app.motion = Motion::Calm;
    app.frame_after(std::time::Duration::from_millis(600));
    let screen = render(&app, 120, 40);
    assert!(screen.contains(&format!("{} claude", SPIN[2])), "{screen}");
}

#[test]
fn a_starting_session_says_so_on_its_row_until_it_answers() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let text = |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
    let row = |app: &App| salon::agent_line(app, &app.world.agents[w[0]], "", &app.rungs(), 200);
    app.world.agents[w[0]].activity = Some(String::new());
    assert!(text(&row(&app)).contains("thinking…"));
    app.world.agents[w[0]].starting = true;
    let line = row(&app);
    assert_eq!(
        (line.spans[1].content.as_ref(), line.spans[1].style),
        ("● ", fg(pal().ok))
    );
    assert!(text(&line).contains(" starting…"), "{}", text(&line));
    assert!(!text(&line).contains("thinking…"));
    app.lang = Lang::Tr;
    assert!(text(&row(&app)).contains(" başlıyor…"));
    app.world.agents[w[0]].starting = false;
    assert!(text(&row(&app)).contains("düşünüyor…"));
}
