//! Drawing. One rule above all: the screen is calm unless something needs
//! the person. Motion and colour are signals, never decoration, and a status
//! never relies on colour alone (every one has its own glyph or word).

mod board;
mod code;
mod focus;
mod overlay;
mod salon;
mod words;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, FLASH, View, Zoom};
use crate::i18n::{Lang, clean, fill};
use crate::model::*;

pub fn draw(f: &mut Frame, app: &App) {
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(f.area());
    header(f, head, app);
    match app.zoom {
        Zoom::Focus(id) => focus::draw(f, body, app, id),
        Zoom::Trace(id) => focus::trace(f, body, app, id),
        Zoom::None => match app.view {
            View::Salon => salon::draw(f, body, app),
            View::Board => board::draw(f, body, app),
            View::Code => code::draw(f, body, app),
        },
    }
    footer(f, foot, app);
    if app.help {
        overlay::help(f, app.lang);
    } else if let Some(d) = &app.digest {
        overlay::digest(f, app, d);
    }
}

pub(crate) fn dim() -> Style {
    Style::new().fg(Color::Indexed(244))
}

pub(crate) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub(crate) fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

/// 256-colour indexes, so terminals without true colour show them too.
pub(crate) fn vendor_color(v: Vendor) -> Color {
    match v {
        Vendor::Claude => Color::Indexed(173),
        Vendor::Codex => Color::Indexed(36),
        Vendor::Gemini => Color::Indexed(69),
        Vendor::Qwen => Color::Indexed(99),
        Vendor::Local => Color::Indexed(250),
    }
}

pub(crate) fn glyph(a: &Agent) -> (&'static str, Color) {
    if a.role == Role::Leader {
        return ("◆", Color::Magenta);
    }
    match a.status {
        Status::Idle => ("○", Color::Indexed(244)),
        Status::Planning { .. } => ("◆", Color::Magenta),
        Status::Working => ("●", Color::Green),
        Status::Asking { .. } => ("◐", Color::Yellow),
        Status::Blocked => ("✕", Color::Red),
        Status::WaitingHuman => ("!", Color::Yellow),
        Status::Verifying { .. } => ("◌", Color::Cyan),
        Status::Researching { .. } => ("◇", Color::Blue),
        Status::Stopped => ("■", Color::LightRed),
    }
}

pub(crate) fn taint_span(a: &Agent) -> Span<'static> {
    match a.taint {
        Taint::Clean => Span::raw(" "),
        Taint::Soft => Span::styled("~", fg(Color::Yellow)),
        Taint::Full => Span::styled("≈", fg(Color::Red)),
    }
}

pub(crate) fn entry_icon(k: EntryKind) -> (&'static str, Style) {
    match k {
        EntryKind::Assign => ("→", dim()),
        EntryKind::Plan => ("◆", fg(Color::Magenta)),
        EntryKind::Talk => ("⇄", fg(Color::Cyan)),
        EntryKind::Assumption => ("⚑", fg(Color::Yellow)),
        EntryKind::Blocked => ("✕", fg(Color::Red)),
        EntryKind::Research => ("◇", fg(Color::Blue)),
        EntryKind::Sensitive => ("!", bold(Color::Yellow)),
        EntryKind::Merged => ("✓", fg(Color::Green)),
        EntryKind::Failed => ("✕", fg(Color::LightRed)),
        EntryKind::Person => ("›", bold(Color::White)),
    }
}

/// One journal line: time, icon, the event in words.
pub(crate) fn entry_line(app: &App, e: &Entry) -> Line<'static> {
    let w = &app.world;
    let (icon, style) = entry_icon(e.event.kind());
    let mut spans = vec![
        Span::styled(format!("{} ", clock(e.tick)), dim()),
        Span::styled(format!("{icon} "), style),
    ];
    if w.projects.len() > 1 {
        let p = w.agents[e.event.agent()].project;
        spans.push(Span::styled(format!("{} ", w.projects[p].name), dim()));
    }
    spans.push(Span::raw(words::event(app.lang, w, &e.event)));
    Line::from(spans)
}

/// `s` cleaned, then cut or padded to exactly `n` characters.
pub(crate) fn fit(s: &str, n: usize) -> String {
    let s = clean(s);
    if s.chars().count() <= n {
        format!("{s:<n$}")
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// `lines` wrapped to `area`, scrolled so the newest lines are in view.
pub(crate) fn tail(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    let rows = p.line_count(area.width);
    let skip = rows.saturating_sub(usize::from(area.height));
    let skip = u16::try_from(skip).unwrap_or(u16::MAX);
    f.render_widget(p.scroll((skip, 0)), area);
}

pub(crate) fn bar(progress: u8) -> String {
    let full = ((usize::from(progress) * 5 + 50) / 100).min(5);
    format!("{}{}", "▓".repeat(full), "░".repeat(5 - full))
}

/// What an agent is doing, in a few words.
pub(crate) fn status_spans(lang: Lang, w: &World, a: &Agent) -> Vec<Span<'static>> {
    let t = lang.text();
    match a.status {
        Status::Idle if a.role == Role::Leader => vec![Span::styled(t.coordinating, dim())],
        Status::Idle => vec![Span::styled(t.idle, dim())],
        Status::Planning { .. } => vec![Span::styled(t.planning, fg(Color::Magenta))],
        Status::Working => match a.activity.as_deref() {
            Some("") => vec![Span::styled(t.thinking, fg(Color::Green))],
            Some(doing) => vec![Span::styled(
                fit(doing, 40).trim_end().to_string(),
                fg(Color::Green),
            )],
            None => vec![
                Span::styled(bar(a.progress), fg(Color::Green)),
                Span::raw(format!(" {}", Phase::of(a.progress).label())),
            ],
        },
        Status::Asking { to, .. } => vec![
            Span::styled(bar(a.progress), dim()),
            Span::styled(
                format!(" ? → {}", clean(&w.agents[to].name)),
                fg(Color::Yellow),
            ),
        ],
        Status::Blocked => {
            let wait = match a.researcher {
                Some(r) => format!(" ← {}", clean(&w.agents[r].name)),
                None => t.research_queue.into(),
            };
            vec![
                Span::styled(t.blocked, bold(Color::Red)),
                Span::styled(wait, dim()),
            ]
        }
        Status::WaitingHuman => vec![Span::styled(t.waiting_human, bold(Color::Yellow))],
        Status::Verifying { .. } => vec![Span::styled(t.verifying, fg(Color::Cyan))],
        Status::Researching { .. } => vec![Span::styled(
            fill(t.researching, &[&a.topic]),
            fg(Color::Blue),
        )],
        Status::Stopped => vec![Span::styled(t.stopped, bold(Color::LightRed))],
    }
}

fn header(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let sep = || Span::styled(" │ ", dim());
    let open = w.open_inbox().len();
    let budget = w.spent() * 100 / w.daily_budget.max(1);
    let mut spans = vec![
        Span::styled(
            " imece ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Indexed(173))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(fill(
            t.cards,
            &[&w.cards_done().to_string(), &w.cards.len().to_string()],
        )),
        sep(),
        Span::raw(fill(t.agents_working, &[&w.active_agents().to_string()])),
        sep(),
        Span::raw(fill(t.per_hour, &[&dollars(w.rate_per_hour())])),
        Span::styled(fill(t.budget, &[&budget.to_string()]), dim()),
        sep(),
        Span::raw(fill(
            t.researchers,
            &[
                &w.active_researchers().to_string(),
                &w.max_researchers.to_string(),
            ],
        )),
    ];
    if open > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            fill(t.waiting_for_you, &[&open.to_string()]),
            Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(format!("  {}", clock(w.tick)), dim()));
    if app.paused {
        spans.push(Span::styled(
            t.paused,
            Style::new().add_modifier(Modifier::REVERSED),
        ));
    } else if app.speed > 1 {
        spans.push(Span::styled(format!(" ×{}", app.speed), dim()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn footer(f: &mut Frame, area: Rect, app: &App) {
    let t = app.lang.text();
    let key = || Style::new().fg(Color::Black).bg(Color::Indexed(250));
    let line = if app.typing {
        Line::from(vec![
            Span::styled(
                fill(t.typing_to, &[&app.typing_target()]),
                bold(Color::White),
            ),
            Span::raw(clean(&app.input)),
            Span::styled("█", dim()),
            Span::styled(t.send_cancel, dim()),
        ])
    } else if let Some((msg, at)) = &app.flash
        && at.elapsed() < FLASH
    {
        Line::from(Span::styled(format!(" {msg}"), fg(Color::Yellow)))
    } else {
        let hints: Vec<(&str, &str)> = match app.zoom {
            Zoom::None => vec![
                ("/", t.write_lead),
                ("@", t.to_agent),
                ("tab", t.panel),
                ("← →", t.fold),
                ("enter", t.focus),
                ("space", t.pause),
                ("?", t.help),
            ],
            Zoom::Focus(_) => vec![
                ("enter", t.trace),
                ("esc", t.back),
                ("@", t.write_this),
                ("?", t.help),
            ],
            Zoom::Trace(_) => vec![("esc", t.back), ("@", t.write_this), ("?", t.help)],
        };
        let mut spans = vec![Span::raw(" ")];
        for (k, label) in hints {
            spans.push(Span::styled(format!(" {k} "), key()));
            spans.push(Span::raw(format!(" {label}  ")));
        }
        for (i, view) in [View::Salon, View::Board, View::Code]
            .into_iter()
            .enumerate()
        {
            let style = if app.view == view {
                bold(Color::White)
            } else {
                dim()
            };
            spans.push(Span::styled(format!(" {} {}", i + 1, t.views[i]), style));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{App, Pane, Sel};
    use crate::i18n::Lang;

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app_after(projects: usize, workers: usize, ticks: usize, lang: Lang) -> App {
        let mut app = App::new(projects, workers, 4, 7, lang);
        for _ in 0..ticks {
            app.step();
        }
        app
    }

    #[test]
    fn the_salon_shows_projects_the_inbox_and_the_journal() {
        let app = app_after(2, 12, 300, Lang::En);
        let s = render(&app, 140, 45);
        for want in [
            "SALON",
            "▾ shop-api",
            "▾ mobile-app",
            "lead-shop-api",
            "WAITING FOR YOU",
            "JOURNAL",
            "QUEUE",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
    }

    #[test]
    fn the_screen_speaks_turkish_when_asked() {
        let app = app_after(1, 12, 300, Lang::Tr);
        let s = render(&app, 140, 45);
        for want in ["SENİ BEKLEYEN", "DEFTER", "KUYRUK", "lidere yaz"] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
        assert!(!s.contains("WAITING FOR YOU"));
    }

    #[test]
    fn a_folded_project_shows_only_its_header() {
        let mut app = app_after(2, 12, 300, Lang::En);
        let shown = |app: &App, p: ProjectId| {
            salon::rows(&app.world, &app.collapsed, app.selected, 100)
                .iter()
                .filter(|r| matches!(r, salon::Row::Agent { id, .. } if app.world.agents[*id].project == p))
                .count()
        };
        assert!(shown(&app, 1) > 1);
        app.collapsed[1] = true;
        assert_eq!(shown(&app, 1), 0);
        assert!(
            shown(&app, 0) > 1,
            "folding one project must not fold the other"
        );
        assert!(render(&app, 140, 45).contains("▸ mobile-app"));
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
    fn many_agents_fold_the_calm_ones() {
        let app = app_after(1, 80, 200, Lang::En);
        let s = render(&app, 120, 30);
        assert!(s.contains("agents doing fine"), "no folded line:\n{s}");
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
        let s = render(&app, 140, 40);
        assert!(!s.contains('\u{1b}'));
        assert!(!s.contains("sk-secret123"), "{s}");
        assert!(s.contains("hello"));
    }

    #[test]
    fn a_long_answer_wraps_and_its_end_stays_in_view() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let id = app
            .world
            .agents
            .iter()
            .find(|a| a.card.is_some())
            .unwrap()
            .id;
        let card = app.world.agents[id].card;
        let long = format!("{} THE-END", "word ".repeat(400));
        for _ in 0..30 {
            app.world.story(card, StoryEvent::Decision("filler".into()));
        }
        app.world.story(card, StoryEvent::Replied(long.clone()));
        app.zoom = Zoom::Focus(id);
        assert!(render(&app, 100, 30).contains("THE-END"));
        app.world.agents[id].trace(0, long);
        app.zoom = Zoom::Trace(id);
        assert!(render(&app, 100, 30).contains("THE-END"));
    }

    #[test]
    fn every_view_draws_at_every_size() {
        for lang in [Lang::En, Lang::Tr] {
            let mut app = app_after(3, 30, 400, lang);
            let views = [
                (View::Salon, Zoom::None),
                (View::Board, Zoom::None),
                (View::Code, Zoom::None),
                (View::Salon, Zoom::Focus(5)),
                (View::Salon, Zoom::Trace(5)),
                (View::Salon, Zoom::Focus(0)),
            ];
            for (view, zoom) in views {
                app.view = view;
                app.zoom = zoom;
                app.pane = Pane::Inbox;
                app.selected = Sel::Project(1);
                for (w, h) in [(1, 1), (10, 3), (40, 12), (80, 24), (200, 60)] {
                    render(&app, w, h);
                }
            }
            app.help = true;
            render(&app, 30, 10);
        }
    }

    #[test]
    fn the_views_name_themselves() {
        let mut app = app_after(2, 12, 300, Lang::En);
        app.view = View::Board;
        assert!(render(&app, 140, 40).contains("IN PROGRESS"));
        app.view = View::Code;
        assert!(render(&app, 140, 40).contains("CODE MAP"));
        app.zoom = Zoom::Focus(3);
        assert!(render(&app, 140, 40).contains("FOCUS"));
    }
}
