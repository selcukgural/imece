//! Drawing. One rule above all: the screen is calm unless something needs
//! the person. Motion and colour are signals, never decoration, and a status
//! never relies on colour alone (every one has its own glyph or word).

mod board;
mod code;
mod focus;
mod overlay;
mod salon;
pub(crate) mod words;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, FLASH, View, Zoom};
use crate::i18n::{Lang, clean, fill};
use crate::model::*;
pub(crate) use crate::theme::pal;
pub(crate) use salon::{journal_rows, thread_of};

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
    if let Some(th) = &app.thread {
        overlay::thread(f, app, th);
    }
    if app.help {
        overlay::help(f, app.lang);
    } else if let Some(d) = &app.digest {
        overlay::digest(f, app, d);
    }
}

pub(crate) fn dim() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::DIM)
    } else {
        Style::new().fg(p.dim)
    }
}

/// Bold text on a coloured badge: the brand, or what waits for the person.
fn badge(bg: Color) -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::new()
            .fg(p.on_badge)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn brand() -> Style {
    badge(pal().accent)
}

pub(crate) fn attention() -> Style {
    badge(pal().warn)
}

/// A key in a hint line.
pub(crate) fn key_style() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new().fg(p.on_key).bg(p.key)
    }
}

/// The row that just changed.
pub(crate) fn changed() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(p.selection)
    }
}

pub(crate) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub(crate) fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

/// 256-colour indexes, so terminals without true colour show them too.
pub(crate) fn vendor_color(v: Vendor) -> Color {
    pal().vendors[v as usize]
}

/// An agent's own colour in conversations, fixed by its id so a speaker
/// keeps it for the whole run; none of them a status colour.
pub(crate) fn speaker_color(id: AgentId) -> Color {
    let s = &pal().speakers;
    s[id % s.len()]
}

fn party_span(lang: Lang, w: &World, p: Party) -> Span<'static> {
    match p {
        Party::Agent(id) => Span::styled(clean(&w.agents[id].name), bold(speaker_color(id))),
        Party::Person => Span::styled(lang.text().you, bold(pal().strong)),
    }
}

/// A conversation line: `from → to [kind]: text`, each speaker in its own
/// colour.
pub(crate) fn said_spans(lang: Lang, w: &World, s: &words::Said) -> Vec<Span<'static>> {
    let mut spans = vec![
        party_span(lang, w, s.from),
        Span::styled(" → ", dim()),
        party_span(lang, w, s.to),
    ];
    if let Some(k) = s.kind {
        spans.push(Span::styled(
            format!(" [{}]", lang.text().kinds[k as usize]),
            dim(),
        ));
    }
    spans.push(Span::raw(format!(": {}", clean(s.text))));
    spans
}

pub(crate) fn glyph(a: &Agent) -> (&'static str, Color) {
    if a.role == Role::Leader {
        return ("◆", pal().plan);
    }
    match a.status {
        Status::Idle => ("○", pal().dim),
        Status::Planning { .. } => ("◆", pal().plan),
        Status::Working => ("●", pal().ok),
        Status::Asking { .. } => ("◐", pal().warn),
        Status::Blocked => ("✕", pal().bad),
        Status::WaitingHuman => ("!", pal().warn),
        Status::Verifying { .. } => ("◌", pal().info),
        Status::Researching { .. } => ("◇", pal().research),
        Status::Stopped => ("■", pal().stopped),
    }
}

pub(crate) fn taint_span(a: &Agent) -> Span<'static> {
    match a.taint {
        Taint::Clean => Span::raw(" "),
        Taint::Soft => Span::styled("~", fg(pal().warn)),
        Taint::Full => Span::styled("≈", fg(pal().bad)),
    }
}

pub(crate) fn entry_icon(k: EntryKind) -> (&'static str, Style) {
    match k {
        EntryKind::Assign => ("→", dim()),
        EntryKind::Plan => ("◆", fg(pal().plan)),
        EntryKind::Talk => ("⇄", fg(pal().info)),
        EntryKind::Assumption => ("⚑", fg(pal().warn)),
        EntryKind::Blocked => ("✕", fg(pal().bad)),
        EntryKind::Research => ("◇", fg(pal().research)),
        EntryKind::Sensitive => ("!", bold(pal().warn)),
        EntryKind::Merged => ("✓", fg(pal().ok)),
        EntryKind::Failed => ("✕", fg(pal().stopped)),
        EntryKind::Person => ("›", bold(pal().strong)),
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
    match words::said(&e.event) {
        Some(said) => spans.extend(said_spans(app.lang, w, &said)),
        None => spans.push(Span::raw(words::event(app.lang, w, &e.event))),
    }
    Line::from(spans)
}

/// Journal lines `entries` as text to paste elsewhere: who said what, when,
/// whole. Cleaned like everything drawn, so what is copied is what the
/// screen shows (no control characters, secrets hidden).
pub(crate) fn copy_text(lang: Lang, w: &World, entries: &[usize]) -> String {
    let party = |p: Party| match p {
        Party::Agent(id) => clean(&w.agents[id].name),
        Party::Person => lang.text().you.to_string(),
    };
    let one = |i: usize| {
        let e = &w.journal[i];
        match words::said(&e.event) {
            Some(s) if entries.len() == 1 => body_lines(s.text).join("\n"),
            Some(s) => {
                let kind = s
                    .kind
                    .map(|k| format!(" [{}]", lang.text().kinds[k as usize]))
                    .unwrap_or_default();
                format!(
                    "{} {} → {}{kind}\n{}",
                    clock(e.tick),
                    party(s.from),
                    party(s.to),
                    body_lines(s.text).join("\n")
                )
            }
            None => format!("{} {}", clock(e.tick), words::event(lang, w, &e.event)),
        }
    };
    entries
        .iter()
        .map(|&i| one(i))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// An agent's text as cleaned lines, trailing blank lines dropped.
pub(crate) fn body_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| clean(l).trim_end().to_string())
        .collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// An agent's text drawn lightly as the Markdown it usually is: headings,
/// lists, `code` and fenced blocks. Every line is cleaned first.
pub(crate) fn markdown(text: &str, indent: &str) -> Vec<Line<'static>> {
    let code = fg(pal().code);
    let mut fenced = false;
    let mut out = Vec::new();
    for line in body_lines(text) {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let mut spans = vec![Span::raw(indent.to_string())];
        if fenced {
            spans.push(Span::styled("│ ", dim()));
            spans.push(Span::styled(line, code));
        } else if let Some(h) = line.strip_prefix('#') {
            spans.push(Span::styled(
                h.trim_start_matches('#').trim().to_string(),
                bold(pal().strong),
            ));
        } else {
            let lead = line.len() - line.trim_start().len();
            let rest = &line[lead..];
            let rest = match rest.strip_prefix("- ").or_else(|| rest.strip_prefix("* ")) {
                Some(item) => {
                    spans.push(Span::raw(format!("{}• ", &line[..lead])));
                    item
                }
                None => {
                    spans.push(Span::raw(line[..lead].to_string()));
                    rest
                }
            };
            // Backticks alternate between prose and code.
            for (n, part) in rest.split('`').enumerate() {
                if part.is_empty() {
                    continue;
                }
                spans.push(if n % 2 == 1 {
                    Span::styled(part.to_string(), code)
                } else {
                    Span::raw(part.to_string())
                });
            }
        }
        out.push(Line::from(spans));
    }
    out
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
        Status::Idle
            if w.open_inbox()
                .iter()
                .any(|&i| w.inbox[i].kind == InboxKind::Reply && w.inbox[i].agent == a.id) =>
        {
            vec![Span::styled(t.answered, fg(pal().warn))]
        }
        Status::Idle
            if a.role == Role::Leader
                && w.agents.iter().any(|o| {
                    o.project == a.project
                        && o.role == Role::Worker
                        && !o.gone
                        && o.status != Status::Idle
                }) =>
        {
            vec![Span::styled(t.coordinating, dim())]
        }
        Status::Idle => vec![Span::styled(t.idle, dim())],
        Status::Planning { .. } => vec![Span::styled(t.planning, fg(pal().plan))],
        Status::Working => match a.activity.as_deref() {
            Some("") => vec![Span::styled(t.thinking, fg(pal().ok))],
            Some(doing) => vec![Span::styled(
                fit(doing, 40).trim_end().to_string(),
                fg(pal().ok),
            )],
            None => vec![
                Span::styled(bar(a.progress), fg(pal().ok)),
                Span::raw(format!(" {}", Phase::of(a.progress).label())),
            ],
        },
        Status::Asking { to, .. } => vec![
            Span::styled(bar(a.progress), dim()),
            Span::styled(
                format!(" ? → {}", clean(&w.agents[to].name)),
                fg(pal().warn),
            ),
        ],
        Status::Blocked => {
            let wait = match a.researcher {
                Some(r) => format!(" ← {}", clean(&w.agents[r].name)),
                None => t.research_queue.into(),
            };
            vec![
                Span::styled(t.blocked, bold(pal().bad)),
                Span::styled(wait, dim()),
            ]
        }
        Status::WaitingHuman => vec![Span::styled(t.waiting_human, bold(pal().warn))],
        Status::Verifying { .. } => vec![Span::styled(t.verifying, fg(pal().info))],
        Status::Researching { .. } => vec![Span::styled(
            fill(t.researching, &[&a.topic]),
            fg(pal().research),
        )],
        Status::Stopped => vec![Span::styled(t.stopped, bold(pal().stopped))],
    }
}

fn header(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let sep = || Span::styled(" │ ", dim());
    let open = w.open_inbox().len();
    let budget = w.spent() * 100 / w.daily_budget.max(1);
    let mut spans = vec![Span::styled(" imece ", brand()), Span::raw(" ")];
    // Live work starts without cards and without researchers: no "0/0".
    if !w.cards.is_empty() {
        spans.push(Span::raw(fill(
            t.cards,
            &[&w.cards_done().to_string(), &w.cards.len().to_string()],
        )));
        spans.push(sep());
    }
    spans.extend([
        Span::raw(fill(t.agents_working, &[&w.active_agents().to_string()])),
        sep(),
        Span::raw(fill(t.per_hour, &[&dollars(w.rate_per_hour())])),
        Span::styled(fill(t.budget, &[&budget.to_string()]), dim()),
    ]);
    if w.max_researchers > 0 {
        spans.push(sep());
        spans.push(Span::raw(fill(
            t.researchers,
            &[
                &w.active_researchers().to_string(),
                &w.max_researchers.to_string(),
            ],
        )));
    }
    if open > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            fill(t.waiting_for_you, &[&open.to_string()]),
            attention(),
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
    let key = key_style;
    let line = if app.typing {
        Line::from(vec![
            Span::styled(
                fill(t.typing_to, &[&app.typing_target()]),
                bold(pal().strong),
            ),
            Span::raw(clean(&app.input)),
            Span::styled("█", dim()),
            Span::styled(t.send_cancel, dim()),
        ])
    } else if let Some((msg, at)) = &app.flash
        && at.elapsed() < FLASH
    {
        Line::from(Span::styled(format!(" {msg}"), fg(pal().warn)))
    } else {
        let hints: Vec<(&str, &str)> = match app.zoom {
            Zoom::None => vec![
                ("/", t.write_lead),
                ("@", t.to_agent),
                ("tab", t.panel),
                ("← →", t.fold),
                ("enter", t.focus),
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
        if app.zoom == Zoom::None && !app.live() {
            spans.push(Span::styled(" space ", key()));
            spans.push(Span::raw(format!(" {}  ", t.pause)));
        }
        for (i, view) in [View::Salon, View::Board, View::Code]
            .into_iter()
            .enumerate()
        {
            let style = if app.view == view {
                bold(pal().strong)
            } else {
                dim()
            };
            spans.push(Span::styled(format!(" {} {}", i + 1, t.views[i]), style));
        }
        if let Some(c) = app.pending {
            // The first key of a chord, as vim shows it.
            spans.push(Span::styled(format!("   {c}…"), bold(pal().warn)));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use ratatui::crossterm::event::{KeyCode, KeyEvent};

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
    fn a_conversation_shows_its_speakers_in_their_colours_and_folds() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let lead = app.world.projects[0].leader;
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        for (from, to, text) in [
            (lead, worker, "take the parser"),
            (worker, lead, "on it"),
            (lead, worker, "newest-line"),
        ] {
            app.world.note(Event::Sent {
                from,
                to,
                kind: MsgKind::Inform,
                text: text.into(),
            });
        }
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| draw(f, &app)).unwrap();
        let buf = t.backend().buffer();
        let row = (0..buf.area.height)
            .find(|&y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("newest-line")
            })
            .expect("the newest line is in the journal");
        let text: String = (0..buf.area.width)
            .map(|x| buf[(x, row)].symbol())
            .collect();
        assert!(
            text.contains("+2"),
            "the older two lines fold under it: {text}"
        );
        let full = render(&app, 140, 40);
        assert!(!full.contains("on it"), "folded lines stay folded:\n{full}");
        let name = &app.world.agents[lead].name;
        let at = text.find(name.as_str()).unwrap();
        let x = u16::try_from(text[..at].chars().count()).unwrap();
        assert_eq!(buf[(x, row)].fg, speaker_color(lead), "{text}");
    }

    #[test]
    fn a_salon_row_carries_its_speaker_colour_before_the_name() {
        let app = app_after(1, 4, 50, Lang::En);
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| draw(f, &app)).unwrap();
        let buf = t.backend().buffer();
        for a in app.world.agents.iter().filter(|a| !a.gone) {
            let Some((x, y)) = (0..buf.area.height).find_map(|y| {
                let text: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                let at = text.find(&format!("{}{}", salon::SPEAKER_MARK, a.name))?;
                Some((u16::try_from(text[..at].chars().count()).unwrap(), y))
            }) else {
                continue; // folded away
            };
            assert_eq!(buf[(x, y)].fg, speaker_color(a.id), "{}", a.name);
            assert_eq!(buf[(x + 1, y)].fg, vendor_color(a.vendor), "{}", a.name);
        }
        let render = render(&app, 140, 40);
        assert!(
            render.contains(&format!(
                "{}{}",
                salon::SPEAKER_MARK,
                app.world.agents[0].name
            )),
            "{render}"
        );
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
    fn every_view_draws_at_every_size_in_every_theme() {
        for (theme, lang) in crate::theme::ALL
            .iter()
            .zip([Lang::En, Lang::Tr].into_iter().cycle())
        {
            crate::theme::set(theme.name);
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
        crate::theme::set("dark");
    }

    #[test]
    fn capital_t_cycles_the_theme_and_says_which() {
        let mut app = app_after(1, 4, 50, Lang::En);
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "light");
        assert!(render(&app, 140, 40).contains("theme: light"));
        key(&mut app, KeyCode::Char('T'));
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "dark");
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

    fn key(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::from(code));
    }

    #[test]
    fn an_answer_is_in_the_inbox_and_enter_reads_it_whole() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let answer = "Short first line.\nTHE-REST";
        app.world.note(Event::Replied {
            agent: 0,
            text: answer.into(),
        });
        app.world.replied(0, answer);
        let screen = render(&app, 140, 40);
        assert!(
            screen.contains("answered you: Short first line."),
            "{screen}"
        );
        app.pane = Pane::Inbox;
        app.inbox_sel = app.world.open_inbox().len() - 1;
        key(&mut app, KeyCode::Enter);
        assert!(app.thread.is_some(), "the conversation opens");
        assert!(render(&app, 140, 40).contains("THE-REST"));
        assert!(
            app.world
                .open_inbox()
                .iter()
                .all(|&i| app.world.inbox[i].kind != InboxKind::Reply)
        );
    }

    /// A world whose journal is only a conversation of the person and the
    /// leader, with another pair talking in between.
    fn talking() -> App {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        let you = |text: &str| Event::YouSaid {
            to: 0,
            text: text.into(),
        };
        let lead = |text: &str| Event::Replied {
            agent: 0,
            text: text.into(),
        };
        app.world.note(you("check the readme"));
        app.world.note(lead(
            "# Readme\nIt is one line: `# demo`.\n- no usage\n```\ncargo run\n```",
        ));
        app.world.note(Event::Talk {
            from: 1,
            to: 2,
            text: "between workers".into(),
        });
        app.world.note(you("assign it"));
        app.world
            .note(lead(&format!("{} THE-END", "long ".repeat(300))));
        app
    }

    #[test]
    fn tab_reaches_the_journal_and_enter_opens_the_whole_conversation() {
        let mut app = talking();
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Journal);
        // The newest line is selected; its conversation folds the older one.
        assert_eq!(app.journal_sel, Some(4));
        assert!(render(&app, 140, 40).contains("enter open the conversation"));
        key(&mut app, KeyCode::Enter);
        let th = app.thread.as_ref().unwrap();
        // Every line of the pair, not only the neighbouring ones; not the
        // workers' talk.
        assert_eq!(th.entries, [0, 1, 3, 4]);
        assert_eq!(th.sel, 3);
        let screen = render(&app, 120, 30);
        assert!(screen.contains("CONVERSATION"), "{screen}");
        assert!(screen.contains("THE-END"), "the selected line is in view");
        key(&mut app, KeyCode::Home);
        let screen = render(&app, 120, 30);
        assert!(screen.contains("check the readme"), "{screen}");
        assert!(!screen.contains("# Readme"), "headings drop their marks");
        assert!(screen.contains("• no usage") && screen.contains("│ cargo run"));
        key(&mut app, KeyCode::Esc);
        assert!(app.thread.is_none());
        assert_eq!(app.pane, Pane::Journal, "esc only closes the window");
    }

    #[test]
    fn c_copies_a_line_and_capital_c_the_conversation_as_shown() {
        let mut app = talking();
        app.world.note(Event::Replied {
            agent: 0,
            text: "token sk-abc123 \u{1b}[31mred".into(),
        });
        app.pane = Pane::Journal;
        app.journal_sel = Some(5);
        key(&mut app, KeyCode::Char('c'));
        let one = app.copy.take().unwrap();
        assert!(
            !one.contains("sk-abc123") && !one.contains('\u{1b}'),
            "{one}"
        );
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('C'));
        let all = app.copy.take().unwrap();
        assert!(all.contains("you → lead\ncheck the readme"), "{all}");
        assert!(all.contains("lead → you\n# Readme"), "copied as written");
        assert!(!all.contains("between workers"));
    }

    #[test]
    fn typing_works_with_the_conversation_open() {
        let mut app = talking();
        app.open_thread(1);
        key(&mut app, KeyCode::Char('/'));
        assert!(app.typing);
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.input, "x");
        assert!(app.thread.is_some());
    }

    fn ctrl(app: &mut App, c: char) {
        app.on_key(KeyEvent::new(
            KeyCode::Char(c),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        ));
    }

    #[test]
    fn gg_and_capital_g_jump_in_every_list_and_the_chord_shows() {
        let mut app = app_after(2, 12, 300, Lang::En);
        render(&app, 140, 40);
        key(&mut app, KeyCode::Char('g'));
        assert!(
            render(&app, 140, 40).contains("g…"),
            "the pending key shows"
        );
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.selected, Sel::Project(0));
        key(&mut app, KeyCode::Char('G'));
        let last = *app.shown.borrow().last().unwrap();
        assert_eq!(app.selected, last);
        app.pending = None;
        // The journal, newest first: gg is the newest line.
        app.pane = Pane::Journal;
        key(&mut app, KeyCode::Char('G'));
        let rows = journal_rows(&app.world);
        assert_eq!(app.journal_sel, Some(rows.last().unwrap().0));
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        // Inside a conversation too.
        let mut app = talking();
        app.open_thread(4);
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.thread.as_ref().unwrap().sel, 0);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.thread.as_ref().unwrap().sel, 3);
    }

    #[test]
    fn z_chords_fold_one_project_all_or_none() {
        let mut app = app_after(3, 12, 100, Lang::En);
        render(&app, 140, 40);
        app.selected = Sel::Agent(app.world.projects[1].leader);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.collapsed, [false, true, false]);
        assert_eq!(app.selected, Sel::Project(1), "the selection stays visible");
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.collapsed, [false, false, false]);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('M'));
        assert_eq!(app.collapsed, [true, true, true]);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [false, false, false]);
        // A chord that means nothing is dropped, not kept pending.
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.pending, None);
        key(&mut app, KeyCode::Char('j'));
        assert_ne!(app.selected, Sel::Project(0), "j moved as usual");
    }

    #[test]
    fn t_opens_the_trace_and_v_the_conversation_of_the_agent_at_hand() {
        let mut app = talking();
        app.selected = Sel::Agent(0);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.zoom, Zoom::Trace(0));
        key(&mut app, KeyCode::Char('v'));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries, [0, 1, 3, 4], "you ⇄ lead, from the trace too");
        app.thread = None;
        app.zoom = Zoom::None;
        app.selected = Sel::Agent(2);
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_none());
        let screen = render(&app, 140, 40);
        assert!(screen.contains("no conversation with"), "{screen}");
        // On a project line, the leader is at hand.
        app.selected = Sel::Project(0);
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_some());
    }

    #[test]
    fn panels_cycle_both_ways_and_bang_goes_to_the_inbox() {
        let mut app = app_after(1, 4, 50, Lang::En);
        key(&mut app, KeyCode::BackTab);
        assert_eq!(app.pane, Pane::Journal);
        assert!(app.journal_sel.is_some());
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.pane, Pane::Inbox);
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.pane, Pane::Journal);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Salon);
        key(&mut app, KeyCode::Char('!'));
        assert_eq!(app.pane, Pane::Inbox);
        key(&mut app, KeyCode::Char('i'));
        assert!(app.typing, "i writes, as in vim");
    }

    #[test]
    fn ctrl_d_and_u_page_the_conversation_and_the_lists() {
        let mut app = talking();
        app.open_thread(4);
        render(&app, 100, 20);
        let th = app.thread.as_ref().unwrap();
        th.scroll.set(0);
        ctrl(&mut app, 'd');
        let after = app.thread.as_ref().unwrap().scroll.get();
        assert!(after > 0, "scrolled half a page");
        ctrl(&mut app, 'u');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 0);
        app.thread = None;
        let mut app = app_after(1, 20, 100, Lang::En);
        render(&app, 140, 60);
        app.selected = Sel::Project(0);
        ctrl(&mut app, 'd');
        let pos = app
            .shown
            .borrow()
            .iter()
            .position(|&s| s == app.selected)
            .unwrap();
        assert_eq!(pos, 5);
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
        // Two columns: a line of the first half carries one of the second.
        let line = wide.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(line.contains("side panel"), "two columns: {line}");
        let narrow = render(&app, 80, 40);
        let line = narrow.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(!line.contains("side panel"), "one column: {line}");
    }

    fn buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn styles_follow_the_theme_and_mono_keeps_the_signal_without_colour() {
        let p = pal();
        assert_eq!(p.name, "dark");
        assert_eq!(dim(), Style::new().fg(p.dim));
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
        assert_eq!(vendor_color(Vendor::Claude), p.vendors[0]);
        assert_eq!(vendor_color(Vendor::Codex), p.vendors[1]);
        assert_eq!(speaker_color(0), p.speakers[0]);
        assert_eq!(speaker_color(13), speaker_color(1), "colours wrap by id");
        crate::theme::set("mono");
        assert_eq!(dim(), Style::new().add_modifier(Modifier::DIM));
        assert_eq!(
            brand(),
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        );
        assert_eq!(attention(), brand());
        assert_eq!(key_style(), Style::new().add_modifier(Modifier::REVERSED));
        assert_eq!(changed(), Style::new().add_modifier(Modifier::BOLD));
        crate::theme::set("dark");
    }

    #[test]
    fn every_glyph_icon_and_status_has_its_own_mark() {
        let mut a = Agent::new(
            1,
            0,
            "w".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Working,
            0,
        );
        assert_eq!(glyph(&a).0, "●");
        a.role = Role::Leader;
        assert_eq!(glyph(&a), ("◆", pal().plan), "a leader is always a leader");
        a.role = Role::Worker;
        let marks: Vec<&str> = [
            Status::Idle,
            Status::Planning { until: 1 },
            Status::Working,
            Status::Asking { to: 0, until: 1 },
            Status::Blocked,
            Status::WaitingHuman,
            Status::Verifying { until: 1 },
            Status::Researching {
                for_agent: 0,
                until: 1,
            },
            Status::Stopped,
        ]
        .into_iter()
        .map(|s| {
            a.status = s;
            glyph(&a).0
        })
        .collect();
        assert_eq!(marks, ["○", "◆", "●", "◐", "✕", "!", "◌", "◇", "■"]);
        a.taint = Taint::Clean;
        assert_eq!(taint_span(&a).content, " ");
        a.taint = Taint::Soft;
        assert_eq!(taint_span(&a).content, "~");
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
        // Every status says something, in words.
        let w = app_after(1, 4, 50, Lang::En).world;
        let mut idle = Agent::new(
            9,
            0,
            "w".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        );
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
    fn the_header_shows_the_budget_share_and_the_footer_hides_pause_where_it_means_nothing() {
        let mut app = app_after(1, 2, 0, Lang::En);
        app.world.daily_budget = 10_000;
        for a in &mut app.world.agents {
            a.cost = 0;
        }
        app.world.agents[0].cost = 2_500;
        app.world.agents[1].cost = 2_500;
        let screen = render(&app, 140, 40);
        assert!(screen.contains("imece"));
        assert!(screen.contains("(budget 50%)"), "{screen}");
        let footer = screen.lines().last().unwrap().to_string();
        assert!(footer.contains("space"), "{footer}");
        // The current view's name is the bold one.
        let buf = buffer(&app, 140, 40);
        let bold_runs: String = (0..140)
            .map(|x| {
                let c = &buf[(x, 39)];
                if c.style().add_modifier.contains(Modifier::BOLD) {
                    c.symbol().to_string()
                } else {
                    " ".into()
                }
            })
            .collect();
        assert!(bold_runs.contains("salon"), "{bold_runs:?}");
        assert!(!bold_runs.contains("board"), "{bold_runs:?}");
        app.zoom = Zoom::Focus(1);
        let footer = render(&app, 140, 40).lines().last().unwrap().to_string();
        assert!(!footer.contains("space"), "{footer}");
    }

    #[test]
    fn a_project_line_counts_its_blocked_workers_only_when_there_are_some() {
        let mut app = app_after(1, 4, 50, Lang::En);
        for a in &mut app.world.agents {
            if a.role == Role::Worker {
                a.status = Status::Working;
            }
        }
        app.world.inbox.clear();
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("✕0"), "{screen}");
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.agents[worker].status = Status::Blocked;
        let screen = render(&app, 140, 40);
        assert!(screen.contains("✕1"), "{screen}");
    }

    #[test]
    fn a_talk_row_names_the_partner_agent() {
        let mut app = app_after(1, 4, 50, Lang::En);
        for a in &mut app.world.agents {
            a.talk = None;
        }
        let (me, other) = (1, 2);
        let partner = app.world.agents[other].name.clone();
        app.world.agents[me].talk = Some(Talk {
            with: Party::Agent(other),
            text: "shall we split it".into(),
            until: 1_000_000,
        });
        let screen = render(&app, 140, 40);
        let row = screen
            .lines()
            .find(|l| l.contains("shall we split it"))
            .expect("the talk row");
        // The salon's part of the row, before the side panel.
        let salon = row.split("││").next().unwrap();
        assert!(salon.contains(&partner), "{salon}");
        assert!(!salon.contains("you"), "{salon}");
    }

    #[test]
    fn the_inbox_border_warns_only_while_something_waits_and_it_is_not_focused() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        // The inbox block's top-left corner: the second `┌` of the top row.
        let corner = |app: &App| {
            let buf = buffer(app, 140, 40);
            let x = (0..140)
                .filter(|&x| buf[(x, 1)].symbol() == "┌")
                .nth(1)
                .expect("two panes");
            buf[(x, 1)].style().fg
        };
        assert_eq!(corner(&app), Some(pal().dim), "nothing waits");
        app.world.replied(0, "an answer");
        assert_eq!(corner(&app), Some(pal().warn), "something waits");
        app.pane = Pane::Inbox;
        assert_eq!(
            corner(&app),
            Some(pal().accent),
            "focused: the focus colour"
        );
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
    }

    #[test]
    fn keys_do_only_what_they_do_where_they_do_it() {
        let mut app = app_after(2, 8, 100, Lang::En);
        render(&app, 140, 40);
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        key(&mut app, KeyCode::Esc);
        assert!(!app.help, "any key closes the help");
        // Panels cycle only on the salon, outside a focus.
        app.view = View::Board;
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('['),
            KeyCode::Char(']'),
            KeyCode::Char('!'),
        ] {
            key(&mut app, code);
            assert_eq!(app.pane, Pane::Salon, "{code:?} on the board");
        }
        app.view = View::Salon;
        app.zoom = Zoom::Focus(1);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Salon, "tab in a focus");
        app.zoom = Zoom::None;
        // Copying is the journal's; chords that fold are the salon's. A
        // stale journal selection must not make c copy elsewhere.
        app.journal_sel = Some(0);
        key(&mut app, KeyCode::Char('c'));
        assert!(app.copy.is_none(), "c in the salon copies nothing");
        app.pane = Pane::Journal;
        app.zoom = Zoom::Focus(1);
        key(&mut app, KeyCode::Char('c'));
        assert!(app.copy.is_none(), "c in a focus copies nothing");
        app.zoom = Zoom::None;
        app.pane = Pane::Inbox;
        for second in ['a', 'M'] {
            key(&mut app, KeyCode::Char('z'));
            key(&mut app, KeyCode::Char(second));
            assert_eq!(app.collapsed, [false, false], "z{second} outside the salon");
        }
        app.view = View::Board;
        app.collapsed = vec![true, true];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [true, true], "zR on the board");
        app.view = View::Salon;
        app.pane = Pane::Salon;
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [false, false], "zR in the salon");
        key(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
    }

    #[test]
    fn ctrl_f_and_b_page_whole_pages_and_the_conversation_takes_half_and_whole() {
        let mut app = app_after(1, 20, 100, Lang::En);
        render(&app, 140, 60);
        app.selected = Sel::Project(0);
        ctrl(&mut app, 'f');
        let pos = |app: &App| {
            app.shown
                .borrow()
                .iter()
                .position(|&s| s == app.selected)
                .unwrap()
        };
        assert_eq!(pos(&app), 10);
        ctrl(&mut app, 'b');
        assert_eq!(pos(&app), 0);
        let mut app = talking();
        app.open_thread(4);
        render(&app, 100, 20);
        let th = app.thread.as_ref().unwrap();
        th.scroll.set(0);
        let page = th.page.get();
        assert!(page > 2, "{page}");
        ctrl(&mut app, 'd');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page / 2);
        ctrl(&mut app, 'u');
        ctrl(&mut app, 'f');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page);
        key(&mut app, KeyCode::PageDown);
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 2 * page);
        key(&mut app, KeyCode::Home);
        assert_eq!(app.thread.as_ref().unwrap().sel, 0);
        key(&mut app, KeyCode::End);
        assert_eq!(app.thread.as_ref().unwrap().sel, 3);
        key(&mut app, KeyCode::Char('c'));
        assert!(
            app.copy.take().is_some(),
            "c copies the line in a conversation"
        );
    }

    #[test]
    fn the_side_panel_resizes_within_bounds() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let start = app.side;
        key(&mut app, KeyCode::Char('<'));
        assert_eq!(app.side, start + 4);
        for _ in 0..30 {
            key(&mut app, KeyCode::Char('<'));
        }
        assert_eq!(app.side, crate::prefs::SIDE_MAX);
        render(&app, 140, 40);
        for _ in 0..30 {
            key(&mut app, KeyCode::Char('>'));
        }
        assert_eq!(app.side, crate::prefs::SIDE_MIN);
        render(&app, 140, 40);
        render(&app, 60, 20);
    }

    #[test]
    fn without_a_merge_queue_there_is_no_trunk_or_lease() {
        let mut app = app_after(1, 4, 300, Lang::En);
        app.world.projects[0].trunk = None;
        app.world.projects[0].repo = &[];
        for c in &mut app.world.cards {
            c.symbols.clear();
        }
        app.world.max_researchers = 0;
        let salon = render(&app, 140, 40);
        assert!(!salon.contains("trunk #"), "{salon}");
        assert!(!salon.contains("researchers"), "{salon}");
        app.zoom = Zoom::Focus(1);
        assert!(!render(&app, 140, 40).contains("lease"));
        app.zoom = Zoom::None;
        app.view = View::Code;
        assert!(render(&app, 140, 40).contains("no shared trunk yet"));
        // Before any card exists there is no "0/0 cards" either.
        app.view = View::Salon;
        app.world.cards.clear();
        app.world.journal.clear();
        app.world.inbox.clear();
        app.world.plans.clear();
        for a in &mut app.world.agents {
            a.card = None;
        }
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("cards"), "{screen}");
    }
}
