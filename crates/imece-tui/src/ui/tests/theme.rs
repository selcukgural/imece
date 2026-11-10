use super::*;

#[test]
fn the_screen_speaks_turkish_when_asked() {
    let app = app_after(1, 12, 300, Lang::Tr);
    let s = render(&app, 140, 80);
    assert!(s.contains("KARAR BEKLEYEN"), "{s}");
    assert!(!s.contains("TO DECIDE") && !s.contains("AT WORK"));
}

#[test]
fn agent_text_reaches_the_screen_clean() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let id = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    app.world.said(id, "\u{1b}[2Jhello sk-secret123");
    app.open_window(Window::Journal);
    let s = render(&app, 140, 40);
    assert!(!s.contains('\u{1b}'));
    assert!(!s.contains("sk-secret123"), "{s}");
    assert!(s.contains("hello"));
}

#[test]
fn a_scheme_paints_the_whole_screen_and_the_terminal_theme_leaves_it() {
    let app = app_after(2, 6, 60, Lang::En);
    assert!(crate::theme::set("dracula"));
    let p = pal();
    let buf = buffer(&app, 80, 24);
    assert!(
        buf.content().iter().all(|c| c.bg == p.bg
            || c.bg == p.selection
            || c.bg == p.key
            || c.bg == p.accent
            || c.bg == p.warn),
        "every cell has the scheme's background or a badge's"
    );
    assert!(
        buf.content().iter().all(|c| c.fg != Color::Reset),
        "and its text colour"
    );
    let mut app = app;
    app.help = true;
    let buf = buffer(&app, 80, 24);
    assert!(
        buf.content().iter().all(|c| c.bg != Color::Reset),
        "a window over the screen keeps it"
    );
    crate::theme::set("terminal");
    let buf = buffer(&app, 80, 24);
    assert!(buf.content().iter().any(|c| c.bg == Color::Reset));
}

#[test]
fn the_backdrop_is_laid_when_the_theme_has_a_background_or_a_text_colour() {
    assert_eq!(backdrop(Color::Reset, Color::Reset), None);
    let (bg, fg) = (Color::Rgb(1, 2, 3), Color::Indexed(7));
    assert_eq!(backdrop(bg, fg), Some(Style::new().bg(bg).fg(fg)));
    assert_eq!(
        backdrop(bg, Color::Reset),
        Some(Style::new().bg(bg).fg(Color::Reset)),
        "a background alone"
    );
    assert_eq!(
        backdrop(Color::Reset, fg),
        Some(Style::new().bg(Color::Reset).fg(fg)),
        "a text colour alone"
    );
}

#[test]
fn fold_lowers_the_case_and_the_turkish_letters() {
    assert_eq!(overlay::fold("ÇçĞğİıÖöŞşÜü"), "ccggiioossuu");
    assert_eq!(overlay::fold("ISIM Abc ⇧enter"), "isim abc ⇧enter");
}

#[test]
fn a_colon_theme_command_switches_the_theme_and_t_is_short_for_it() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        key(app, KeyCode::Enter);
    };
    typed(&mut app, ":theme dracula");
    assert_eq!(crate::theme::pal().name, "dracula");
    assert!(render(&app, 140, 40).contains("theme: dracula"));
    typed(&mut app, ":t mono");
    assert_eq!(crate::theme::pal().name, "mono");
    typed(&mut app, ":theme nope");
    assert_eq!(crate::theme::pal().name, "mono", "unchanged");
    let s = render(&app, 140, 40);
    assert!(s.contains("theme nope: not one of"), "{s}");
    crate::theme::set("terminal");
}

#[test]
fn styles_follow_the_theme_and_mono_keeps_the_signal_without_colour() {
    let p = pal();
    assert_eq!(p.name, "terminal");
    assert_eq!(dim(), Style::new().fg(p.dim));
    assert_eq!(border(), Style::new().fg(p.border));
    assert_eq!(
        brand(),
        Style::new()
            .fg(p.on_badge)
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(
        attention(),
        Style::new()
            .fg(p.on_badge)
            .bg(p.warn)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(key_style(), Style::new().fg(p.on_key).bg(p.key));
    assert_eq!(changed(), Style::new().bg(p.selection));
    assert_eq!(
        shell(),
        Style::new()
            .fg(p.on_badge)
            .bg(p.shell)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(speaker_color(0), p.speakers[0]);
    assert_eq!(speaker_color(13), speaker_color(1), "colours wrap by id");
    crate::theme::set("mono");
    assert_eq!(dim(), Style::new().add_modifier(Modifier::DIM));
    assert_eq!(
        brand(),
        Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    );
    assert_eq!(attention(), brand());
    assert_eq!(shell(), brand(), "reversed: plain without colour");
    assert_eq!(key_style(), Style::new().add_modifier(Modifier::REVERSED));
    assert_eq!(changed(), Style::new().add_modifier(Modifier::BOLD));
    assert_eq!(border(), dim());
    crate::theme::set("terminal");
}

#[test]
fn every_glyph_icon_and_status_has_its_own_mark() {
    let mut a = Agent::new(1, 0, "w".into(), Role::Worker, Status::Working, 0);
    assert_eq!(glyph(&a).0, "●");
    a.role = Role::Leader;
    assert_eq!(glyph(&a), ("●", pal().ok), "a leader's is its status's");
    a.role = Role::Worker;
    let marks: Vec<&str> = [
        Status::Idle,
        Status::Working,
        Status::Asking { to: 0, until: 1 },
        Status::Blocked,
        Status::WaitingHuman,
        Status::Researching {
            for_agent: 0,
            until: 1,
        },
        Status::Stopped,
        Status::Asleep,
    ]
    .into_iter()
    .map(|s| {
        a.status = s;
        glyph(&a).0
    })
    .collect();
    // Five glyphs (record 125): the colour tells asking from working
    // and blocked from stopped, the status words do without it.
    assert_eq!(marks, ["○", "●", "●", "■", "!", "●", "■", "○"]);
    let colours: Vec<Color> = [
        Status::Asking { to: 0, until: 1 },
        Status::Blocked,
        Status::Researching {
            for_agent: 0,
            until: 1,
        },
        Status::Stopped,
        Status::Asleep,
    ]
    .into_iter()
    .map(|s| {
        a.status = s;
        glyph(&a).1
    })
    .collect();
    let p = pal();
    assert_eq!(colours, [p.warn, p.bad, p.research, p.stopped, p.dim]);
    // On the ladder: a rung's own glyph for waiting, done and
    // starting, the status's for the rest.
    assert!(!a.starting, "new: not starting");
    a.status = Status::Working;
    assert_eq!(rung_glyph(&a, Rung::Waiting), ("!", pal().warn));
    assert_eq!(rung_glyph(&a, Rung::Done), ("✓", pal().ok));
    assert_eq!(rung_glyph(&a, Rung::Starting), ("●", pal().ok));
    assert_eq!(rung_glyph(&a, Rung::Working), ("●", pal().ok));
    a.status = Status::Stopped;
    assert_eq!(rung_glyph(&a, Rung::Stuck), ("■", pal().stopped));
    a.status = Status::Blocked;
    assert_eq!(rung_glyph(&a, Rung::Stuck), ("■", pal().bad));
    a.status = Status::Asleep;
    assert_eq!(rung_glyph(&a, Rung::Idle), ("○", pal().dim));
    // A leader's row is on the ladder like anyone's.
    a.role = Role::Leader;
    a.status = Status::Working;
    assert_eq!(rung_glyph(&a, Rung::Waiting), ("!", pal().warn));
    assert_eq!(rung_glyph(&a, Rung::Done), ("✓", pal().ok));
    assert_eq!(rung_glyph(&a, Rung::Working), ("●", pal().ok));
    a.role = Role::Worker;
    for lang in [Lang::En, Lang::Tr] {
        let legend = lang.text().help_legend.join(" ");
        for g in ["!", "●", "○", "✓", "■"] {
            assert!(legend.contains(&format!(" {g} ")), "{lang:?} {g}");
        }
        for g in ["◆", "◌", "◐", "◇", "✕", "◦", "~"] {
            assert!(!legend.contains(g), "{lang:?} {g}: gone from the legend");
        }
    }
    a.taint = Taint::Clean;
    assert_eq!(taint_span(&a).content, " ");
    a.taint = Taint::Soft;
    assert_eq!(taint_span(&a).content, " ", "soft-tainted says nothing");
    a.taint = Taint::Full;
    assert_eq!(taint_span(&a).content, "≈");
    for k in [
        EntryKind::Assign,
        EntryKind::Plan,
        EntryKind::Talk,
        EntryKind::Assumption,
        EntryKind::Blocked,
        EntryKind::Research,
        EntryKind::Sensitive,
        EntryKind::Merged,
        EntryKind::Failed,
        EntryKind::Person,
    ] {
        assert!(!entry_icon(k).0.is_empty(), "{k:?}");
    }
    assert_eq!(entry_icon(EntryKind::Merged).0, "✓");
    assert_eq!(entry_icon(EntryKind::Person).0, "›");
    // The leader's acts are an arrow like an assignment, in the plan
    // colour; blocked and failed are stuck, told apart by colour.
    assert_eq!(entry_icon(EntryKind::Plan), ("→", fg(pal().plan)));
    assert_eq!(entry_icon(EntryKind::Assign), ("→", dim()));
    assert_eq!(entry_icon(EntryKind::Blocked), ("■", fg(pal().bad)));
    assert_eq!(entry_icon(EntryKind::Failed), ("■", fg(pal().stopped)));
    // Every status says something, in words.
    let w = app_after(1, 4, 50, Lang::En).world;
    let mut idle = Agent::new(9, 0, "w".into(), Role::Worker, Status::Idle, 0);
    let words = |a: &Agent| {
        status_spans(Lang::En, &w, a)
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>()
    };
    assert_eq!(words(&idle), "idle");
    let mut seen = vec!["idle".to_string()];
    for status in [
        Status::Blocked,
        Status::Stopped,
        Status::Asleep,
        Status::WaitingHuman,
        Status::Working,
    ] {
        idle.status = status;
        let w = words(&idle);
        assert!(!w.is_empty() && !seen.contains(&w), "{status:?}: {w:?}");
        seen.push(w);
    }
}

#[test]
fn shade_dims_every_cell_but_the_kept_ones() {
    let mut t = Terminal::new(TestBackend::new(3, 2)).unwrap();
    t.draw(|f| {
        f.render_widget(Paragraph::new("abc\ndef"), f.area());
        shade(
            f,
            Rect {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            },
        );
    })
    .unwrap();
    let buf = t.backend().buffer();
    let dimmed = |x: u16, y: u16| buf[(x, y)].modifier.contains(Modifier::DIM);
    assert!(!dimmed(1, 0), "the kept cell");
    for (x, y) in [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
        assert!(dimmed(x, y), "({x}, {y})");
    }
    assert_eq!(buf[(1, 0)].symbol(), "b", "the text stays");
}
