//! Home (record 124): the person's turn across projects, what is stuck
//! and what is at work, in one list, with what is at hand read beside it.
//! On a narrow screen the list takes the width and the row at hand opens
//! under itself.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::*;
use crate::app::{App, HomeGroup, HomeRow};
use crate::i18n::Text;

/// From this wide the preview stands beside the list.
pub(crate) const WIDE: u16 = 100;
/// The list's narrowest beside the preview.
const LIST_MIN: u16 = 52;
/// Rows of the preview under the row at hand on a narrow screen.
const OPENED: usize = 5;
/// Files named in a preview.
const FILES: usize = 6;

/// How wide the list is on a screen `width` wide, the preview beside it.
fn list_width(width: u16) -> u16 {
    (width * 45 / 100).max(LIST_MIN)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let lonca = lonca_line(app);
    let tall = u16::from(lonca.is_some());
    let [bar, store, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(tall),
        Constraint::Min(0),
    ])
    .areas(area);
    f.render_widget(Paragraph::new(project_bar(app, bar.width)), bar);
    if let Some(line) = lonca {
        f.render_widget(Paragraph::new(line), store);
    }
    let at = app.home_at();
    if body.width < WIDE {
        let lines = list_lines(app, usize::from(body.width), at, true);
        f.render_widget(Paragraph::new(lines.0).scroll((lines.1, 0)), body);
        return;
    }
    let [list, gap, preview] = Layout::horizontal([
        Constraint::Length(list_width(body.width)),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(body);
    let (lines, scroll) = list_lines(app, usize::from(list.width), at, false);
    let scroll = scroll.min(lines.len().saturating_sub(1) as u16);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), list);
    let rule = vec![Line::from(Span::styled("│", dim())); usize::from(gap.height)];
    f.render_widget(Paragraph::new(rule), gap);
    let room = usize::from(preview.width).saturating_sub(1);
    let text = at.map_or_else(
        || {
            vec![Line::from(Span::styled(
                app.lang.text().home_nothing,
                dim(),
            ))]
        },
        |row| preview_lines(app, row, room, usize::from(preview.height)),
    );
    f.render_widget(Paragraph::new(text), preview);
}

/// The first of Home's rows: every project with what it holds, the one
/// it is narrowed to lit, and whom `/` writes to.
fn project_bar(app: &App, width: u16) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let lit = |on: bool| {
        if on {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            dim()
        }
    };
    let mut spans = vec![
        Span::styled(
            format!(" {} ", t.home_all),
            lit(app.at_home.scope.is_none()),
        ),
        Span::raw(" "),
    ];
    for p in w.open_projects() {
        let rows = app.home_rows_in(Some(p.id));
        let many = |g| rows.iter().filter(|r| r.0 == g).count();
        spans.push(Span::styled(
            format!(" {} ", clean(&p.name)),
            lit(app.at_home.scope == Some(p.id)),
        ));
        for (group, mark, colour) in [
            (HomeGroup::Turn, "!", pal().warn),
            (HomeGroup::Stuck, "■", pal().bad),
            (HomeGroup::Work, "●", pal().ok),
        ] {
            let n = many(group);
            if n > 0 {
                spans.push(Span::styled(format!("{mark}{n} "), fg(colour)));
            }
        }
        spans.push(Span::raw(" "));
    }
    let lead = w.projects[app.current_project()].leader;
    let writes = fill(t.home_writes, &[&w.agents[lead].name]);
    let used: usize = spans.iter().map(Span::width).sum();
    let free = usize::from(width).saturating_sub(used + writes.chars().count() + 1);
    if free > 0 {
        spans.push(Span::raw(" ".repeat(free)));
        spans.push(Span::styled(writes, dim()));
    }
    Line::from(spans)
}

/// How the projects in view stand on Lonca; none when none is on it.
fn lonca_line(app: &App) -> Option<Line<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let mut spans = Vec::new();
    for p in w.open_projects() {
        if app.at_home.scope.is_some_and(|s| s != p.id) {
            continue;
        }
        if let Some(on) = &p.lonca {
            spans.push(salon::store_span(on, t));
        }
    }
    (!spans.is_empty()).then(|| Line::from(spans))
}

/// The words of a group's heading.
fn group_word(t: &Text, group: HomeGroup) -> (&'static str, Style) {
    match group {
        HomeGroup::Turn => (t.home_turn, bold(pal().warn)),
        HomeGroup::Stuck => (t.home_stuck, bold(pal().bad)),
        HomeGroup::Work => (t.home_work, bold(pal().ok)),
        HomeGroup::Quiet => (t.home_quiet, dim()),
    }
}

/// The list in rows `width` wide and how far to scroll it for the row at
/// hand to show; with `open`, the row at hand has its preview under it.
fn list_lines(
    app: &App,
    width: usize,
    at: Option<HomeRow>,
    open: bool,
) -> (Vec<Line<'static>>, u16) {
    let t = app.lang.text();
    let rows = app.home_rows();
    let mut lines = Vec::new();
    let mut scroll = 0;
    let mut last = None;
    for &(group, row) in &rows {
        if last != Some(group) {
            if last.is_some() {
                lines.push(Line::default());
            }
            let (word, style) = group_word(t, group);
            let many = rows.iter().filter(|r| r.0 == group).count();
            lines.push(Line::from(vec![
                Span::styled(format!(" {word} "), style),
                Span::styled(many.to_string(), dim()),
            ]));
            last = Some(group);
        }
        let here = at == Some(row);
        if here {
            scroll = lines.len().saturating_sub(4) as u16;
        }
        lines.push(row_line(app, row, width, here));
        if open && here {
            let room = width.saturating_sub(4);
            for line in preview_lines(app, row, room, OPENED + 2)
                .into_iter()
                .skip(2)
            {
                let mut spans = vec![Span::raw("    ")];
                spans.extend(line.spans);
                lines.push(Line::from(spans));
            }
        }
    }
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(t.home_nothing, dim())));
    }
    (lines, scroll)
}

/// The card a row is about.
fn card_of(w: &World, row: HomeRow) -> Option<&Card> {
    let id = match row {
        HomeRow::Item(i) => (w.inbox[i].card).or(w.agents[w.inbox[i].agent].card),
        HomeRow::Agent(a) => w.agents[a].card,
    };
    id.and_then(|c| w.cards.get(c))
}

/// What a row is called: its change's title, else its card's, else the
/// words of its item or the agent's name.
fn title_of(lang: Lang, w: &World, row: HomeRow) -> String {
    let change = match row {
        HomeRow::Item(i) if w.inbox[i].kind == InboxKind::Change => w.change_of(w.inbox[i].agent),
        _ => None,
    };
    if let Some(c) = change {
        return clean(&c.title);
    }
    if let Some(c) = card_of(w, row) {
        return clean(&c.title);
    }
    match row {
        HomeRow::Item(i) => words::inbox(lang, w, &w.inbox[i]),
        HomeRow::Agent(a) => clean(&w.agents[a].name),
    }
}

/// The agent a row belongs to.
fn agent_of(w: &World, row: HomeRow) -> AgentId {
    match row {
        HomeRow::Item(i) => w.inbox[i].agent,
        HomeRow::Agent(a) => a,
    }
}

/// How an item is shown on the list, and what it is called at the row's end.
fn item_mark(t: &Text, kind: InboxKind) -> (Span<'static>, &'static str) {
    let tails = t.home_tails;
    let (glyph, colour, tail) = match kind {
        InboxKind::Change => ("Δ", pal().strong, ""),
        InboxKind::Question => ("?", pal().warn, tails[0]),
        InboxKind::Approval => ("⚑", pal().warn, tails[1]),
        InboxKind::Script | InboxKind::Check => ("▶", pal().warn, tails[2]),
        InboxKind::Plan => ("▤", pal().warn, tails[3]),
        InboxKind::Fault => ("■", pal().bad, tails[4]),
        InboxKind::Reply => ("»", pal().info, tails[5]),
        InboxKind::Sensitive => ("!", pal().warn, tails[6]),
        InboxKind::Research => ("?", pal().research, tails[6]),
    };
    (Span::styled(glyph, bold(colour)), tail)
}

/// A change's end of a row: its lines and how its runs went.
fn change_tail(c: &Change) -> Vec<Span<'static>> {
    let added: u32 = c.files.iter().map(|f| f.added).sum();
    let removed: u32 = c.files.iter().map(|f| f.removed).sum();
    let mut spans = vec![Span::styled(format!("+{added} −{removed}"), dim())];
    if !c.runs.is_empty() {
        let good = c.runs.iter().all(Ran::ok);
        let (mark, colour) = if good {
            ("✓", pal().ok)
        } else {
            ("✗", pal().bad)
        };
        spans.push(Span::styled(format!(" {mark}"), fg(colour)));
    }
    spans
}

/// One row of the list: its mark, project, card and title, and what it
/// comes to at the end, with how long.
fn row_line(app: &App, row: HomeRow, width: usize, here: bool) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let lang = app.lang;
    let agent = &w.agents[agent_of(w, row)];
    let (mark, tail) = match row {
        HomeRow::Item(i) => {
            let (mark, word) = item_mark(t, w.inbox[i].kind);
            let change = (w.inbox[i].kind == InboxKind::Change)
                .then(|| w.change_of(agent.id))
                .flatten();
            let tail = match change {
                Some(c) => change_tail(c),
                None => vec![Span::styled(word, fg(pal().warn))],
            };
            (mark, tail)
        }
        HomeRow::Agent(_) => {
            let (glyph, colour) = glyph(agent);
            let doing = on_step(w, agent).map_or_else(
                || {
                    let text: String = doing_status(lang, w, agent)
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect();
                    text
                },
                str::to_string,
            );
            (
                Span::styled(glyph, fg(colour)),
                vec![Span::styled(fit(&doing, 24).trim_end().to_string(), dim())],
            )
        }
    };
    let from = match row {
        HomeRow::Item(i) => w.inbox[i].tick,
        HomeRow::Agent(id) => app.since(id),
    };
    let since = w.tick.saturating_sub(from) * SECONDS_PER_TICK;
    let mut end = tail;
    if since >= 60 {
        end.push(Span::styled(format!(" {}", words::ago(lang, since)), dim()));
    }
    let end_width: usize = end.iter().map(Span::width).sum();
    let mut spans = vec![
        Span::styled(if here { "▸ " } else { "  " }, bold(pal().strong)),
        mark,
        Span::raw(" "),
    ];
    let many = w.open_projects().count() > 1;
    if many && app.at_home.scope.is_none() {
        spans.push(Span::styled(
            fit(&w.projects[agent.project].name, 11),
            dim(),
        ));
        spans.push(Span::raw(" "));
    }
    let card = card_of(w, row).map_or(String::new(), |c| format!("#{}", c.id + 1));
    spans.push(Span::styled(format!("{card:<5}"), dim()));
    let used: usize = spans.iter().map(Span::width).sum();
    let room = width.saturating_sub(used + end_width + 1);
    let title = fit(&title_of(lang, w, row), room);
    let style = if here {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    spans.push(Span::styled(title, style));
    spans.push(Span::raw(" "));
    spans.extend(end);
    let line = Line::from(spans);
    if here {
        line.style(Style::new().bg(pal().selection))
    } else {
        line
    }
}

/// A row of the preview: a dim label in its own column, then the text.
fn labelled(label: &str, text: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{label:<9}"), dim())];
    spans.extend(text);
    Line::from(spans)
}

/// `text` in rows of `room`, under `label`.
fn labelled_text(lines: &mut Vec<Line<'static>>, label: &str, text: &str, room: usize) {
    let wrapped = overlay::wrap_words(&clean(text), room.saturating_sub(9));
    for (n, row) in wrapped.into_iter().enumerate() {
        let label = if n == 0 { label } else { "" };
        lines.push(labelled(label, vec![Span::raw(row)]));
    }
}

/// What a change comes to: its why, its proof, cost and gate, its files
/// and the top of its patch.
fn change_lines(lang: Lang, w: &World, c: &Change, lines: &mut Vec<Line<'static>>, room: usize) {
    let t = lang.text();
    let [why, proof, cost, gate, files] = t.home_labels;
    labelled_text(lines, why, w.done_why(c.agent).unwrap_or(&c.title), room);
    let mut evidence = Vec::new();
    if let Some((mark, good)) = words::check_mark(lang, &c.check) {
        let style = match good {
            Some(true) => fg(pal().ok),
            Some(false) => fg(pal().bad),
            None => dim(),
        };
        evidence.push(Span::styled(mark, style));
    }
    let colour = if c.runs.is_empty() {
        pal().warn
    } else if c.runs.iter().all(Ran::ok) {
        pal().ok
    } else {
        pal().bad
    };
    let ran = words::evidence(lang, &c.runs);
    let ran = ran.strip_prefix(" · ").unwrap_or(&ran).to_string();
    evidence.push(Span::styled(ran, fg(colour)));
    lines.push(labelled(proof, evidence));
    if let Some(s) = c.spend {
        lines.push(labelled(
            cost,
            vec![Span::styled(words::spend(lang, &s), dim())],
        ));
    }
    if let Some(p) = w.proposal_of(c.agent) {
        for (n, line) in diff::proposal_lines(lang, p).into_iter().enumerate() {
            lines.push(labelled(if n == 0 { gate } else { "" }, line.spans));
        }
    }
    lines.push(Line::default());
    for (n, file) in c.files.iter().take(FILES).enumerate() {
        let path = fit(&file.path, room.saturating_sub(24).max(12));
        lines.push(labelled(
            if n == 0 { files } else { "" },
            vec![
                Span::raw(format!("{path} ")),
                Span::styled(format!("+{}", file.added), fg(pal().ok)),
                Span::styled(format!(" −{}", file.removed), fg(pal().bad)),
            ],
        ));
    }
    if c.files.len() > FILES {
        let rest = (c.files.len() - FILES).to_string();
        lines.push(labelled(
            "",
            vec![Span::styled(fill(t.home_more, &[&rest]), dim())],
        ));
    }
}

/// The patch's own lines, what git says around them left out.
fn patch_lines(patch: &str, room: usize, rows: usize) -> Vec<Line<'static>> {
    let git = |l: &str| {
        ["diff --git", "index ", "--- ", "+++ "]
            .iter()
            .any(|p| l.starts_with(p))
    };
    (patch.lines())
        .filter(|l| !git(l))
        .take(rows)
        .map(|l| {
            let style = match l.as_bytes().first() {
                Some(b'+') => fg(pal().ok),
                Some(b'-') => fg(pal().bad),
                Some(b'@') => dim(),
                _ => Style::new(),
            };
            Line::from(Span::styled(fit(l, room).trim_end().to_string(), style))
        })
        .collect()
}

/// What an agent is at: its status, its steps, its newest lines.
fn agent_lines(app: &App, id: AgentId, lines: &mut Vec<Line<'static>>, room: usize, rows: usize) {
    let w = &app.world;
    let a = &w.agents[id];
    let t = app.lang.text();
    lines.push(Line::from(status_spans(app.lang, w, a)));
    lines.push(Line::default());
    lines.extend(focus::plan_lines(t, &a.plan));
    if !a.plan.is_empty() {
        lines.push(Line::default());
    }
    let left = rows.saturating_sub(lines.len());
    let colour = speaker_color(id);
    let newest = a.trace.len().saturating_sub(left);
    for raw in a.trace.iter().skip(newest) {
        lines.extend(focus::trace_lines(raw, colour, room));
    }
}

/// What a row holds, read without a key: a heading, then what it is.
fn preview_lines(app: &App, row: HomeRow, room: usize, rows: usize) -> Vec<Line<'static>> {
    let w = &app.world;
    let id = agent_of(w, row);
    let a = &w.agents[id];
    let project = clean(&w.projects[a.project].name);
    let mut head = Vec::new();
    if let Some(c) = card_of(w, row) {
        head.push(Span::styled(format!("#{} ", c.id + 1), dim()));
    }
    head.push(Span::styled(
        fit(&title_of(app.lang, w, row), room.saturating_sub(30).max(10))
            .trim_end()
            .to_string(),
        bold(pal().strong),
    ));
    head.push(Span::styled(
        format!("  {project} · {} · {}", clean(&a.name), dollars(a.cost)),
        dim(),
    ));
    let mut lines = vec![Line::from(head), Line::default()];
    match row {
        HomeRow::Item(i) => {
            let item = &w.inbox[i];
            let change = (item.kind == InboxKind::Change)
                .then(|| w.change_of(item.agent))
                .flatten();
            if let Some(c) = change {
                change_lines(app.lang, w, c, &mut lines, room);
                lines.push(Line::default());
                let left = rows.saturating_sub(lines.len());
                lines.extend(patch_lines(&c.patch, room, left));
            } else {
                let said = words::inbox(app.lang, w, item);
                for row in overlay::wrap_words(&said, room) {
                    lines.push(Line::from(Span::raw(row)));
                }
                if !item.detail.is_empty() && !said.contains(item.detail.as_str()) {
                    lines.push(Line::default());
                    for row in overlay::wrap_words(&clean(&item.detail), room) {
                        lines.push(Line::from(Span::styled(row, bold(pal().strong))));
                    }
                }
            }
        }
        HomeRow::Agent(_) => agent_lines(app, id, &mut lines, room, rows),
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Engine, View};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| crate::ui::draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn home(lang: Lang) -> App {
        let (world, sim) = crate::sim::world(2, 4, 0, 1, lang);
        let mut app = App::with(world, Engine::Sim(sim), lang);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app.view = View::Home;
        app
    }

    fn worker(app: &App) -> AgentId {
        app.world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id
    }

    fn item(kind: InboxKind, agent: AgentId, detail: &str) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: detail.into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        }
    }

    fn change(agent: AgentId, runs: Vec<Ran>) -> Change {
        Change {
            agent,
            title: "Paging in product search".into(),
            files: vec![ChangedFile {
                path: "src/query.rs".into(),
                added: 19,
                removed: 6,
                binary: false,
            }],
            patch: "diff --git a/x b/x\nindex 1..2\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old line\n+new line\n"
                .into(),
            runs,
            check: Check::None,
            spend: None,
        }
    }

    #[test]
    fn a_question_is_read_whole_where_it_waits() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let long = "May paging change the shape of the API's response?";
        app.world.inbox.push(item(InboxKind::Question, a, long));
        let s = render(&app, 150, 40);
        assert!(s.contains(long), "{s}");
        assert!(s.contains("TO DECIDE 1"), "{s}");
    }

    #[test]
    fn a_change_says_what_it_comes_to_and_its_patch_without_git_s_lines() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.changes.push(change(a, Vec::new()));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        let s = render(&app, 150, 40);
        assert!(s.contains("Paging in product search"), "{s}");
        assert!(s.contains("+19 −6"), "{s}");
        assert!(s.contains("src/query.rs"), "{s}");
        assert!(s.contains("+new line") && s.contains("-old line"), "{s}");
        assert!(!s.contains("diff --git") && !s.contains("+++ b/x"), "{s}");
        assert!(!s.contains("index 1..2") && !s.contains("--- a/x"), "{s}");
    }

    #[test]
    fn a_narrow_screen_opens_the_row_at_hand_under_itself() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world
            .inbox
            .push(item(InboxKind::Question, a, "Which index?"));
        let wide = render(&app, WIDE, 30);
        let narrow = render(&app, WIDE - 1, 30);
        let rule = |s: &str| s.lines().skip(2).any(|l| l.contains('│'));
        assert!(rule(&wide), "{wide}");
        assert!(!rule(&narrow), "{narrow}");
        assert!(narrow.contains("Which index?"), "{narrow}");
    }

    #[test]
    fn the_bar_counts_each_project_and_says_whom_slash_writes_to() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let p = app.world.agents[a].project;
        app.world.agents[a].status = Status::Working;
        app.world.inbox.push(item(InboxKind::Reply, a, "x"));
        let lead = app.world.agents[app.world.projects[p].leader].name.clone();
        let line = |app: &App| render(app, 150, 40).lines().nth(1).unwrap().to_string();
        let row = line(&app);
        assert!(row.contains("!1 ●1"), "{row}");
        assert!(row.contains(&format!("/ writes to {lead}")), "{row}");
        assert!(row.starts_with(" all "), "{row}");
        app.world.agents[a].status = Status::Blocked;
        assert!(line(&app).contains("!1 ■1"), "{}", line(&app));
        app.lang = Lang::Tr;
        assert!(line(&app).contains("hepsi"), "{}", line(&app));
    }

    #[test]
    fn a_full_bar_leaves_out_whom_slash_writes_to() {
        let app = home(Lang::En);
        let s = render(&app, 40, 20);
        assert!(!s.lines().nth(1).unwrap().contains("writes"), "{s}");
    }

    #[test]
    fn nothing_listed_says_so() {
        let mut app = home(Lang::En);
        for a in &mut app.world.agents {
            a.gone = true;
        }
        assert!(render(&app, 150, 40).contains("Nothing needs you"));
        assert!(render(&app, 80, 24).contains("Nothing needs you"));
    }

    #[test]
    fn every_kind_of_item_has_its_mark_and_word() {
        let t = Lang::En.text();
        for (kind, glyph, word) in [
            (InboxKind::Question, "?", "question"),
            (InboxKind::Approval, "⚑", "allow?"),
            (InboxKind::Script, "▶", "script"),
            (InboxKind::Check, "▶", "script"),
            (InboxKind::Plan, "▤", "plan"),
            (InboxKind::Fault, "■", "API error"),
            (InboxKind::Reply, "»", "answered"),
            (InboxKind::Sensitive, "!", "decide"),
            (InboxKind::Research, "?", "decide"),
            (InboxKind::Change, "Δ", ""),
        ] {
            let (mark, tail) = item_mark(t, kind);
            assert_eq!((mark.content.as_ref(), tail), (glyph, word));
        }
    }

    #[test]
    fn a_change_s_end_says_how_its_runs_went() {
        let ran = |code| Ran {
            line: "t".into(),
            code,
            timed_out: false,
        };
        let text = |c: &Change| {
            let spans = change_tail(c);
            spans.iter().map(|s| s.content.as_ref()).collect::<String>()
        };
        assert_eq!(text(&change(0, Vec::new())), "+19 −6");
        assert_eq!(text(&change(0, vec![ran(Some(0))])), "+19 −6 ✓");
        assert_eq!(
            text(&change(0, vec![ran(Some(0)), ran(Some(1))])),
            "+19 −6 ✗"
        );
    }

    #[test]
    fn the_patch_keeps_its_own_lines_and_only_as_many_as_there_is_room_for() {
        let patch = "diff --git a/x b/x\nindex 1\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n x\n-a\n+b\n";
        let text = |rows| {
            patch_lines(patch, 40, rows)
                .iter()
                .map(|l| l.spans[0].content.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(text(10), ["@@ -1 +1 @@", " x", "-a", "+b"]);
        assert_eq!(text(2), ["@@ -1 +1 @@", " x"]);
    }

    #[test]
    fn an_item_s_time_is_its_own_and_the_proof_starts_clean() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.tick = 240;
        let mut first = item(InboxKind::Reply, a, "x");
        first.tick = 228;
        let mut second = item(InboxKind::Question, a, "y");
        second.tick = 120;
        app.world.inbox = vec![first, second];
        let s = render(&app, 150, 40);
        assert!(s.contains("1 min") && s.contains("10 min"), "{s}");
        let ran = Ran {
            line: "cargo test".into(),
            code: Some(0),
            timed_out: false,
        };
        let mut lines = Vec::new();
        let c = change(a, vec![ran]);
        change_lines(Lang::En, &app.world, &c, &mut lines, 80);
        let proof: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(proof.starts_with("proof    ✓"), "{proof}");
    }

    #[test]
    fn the_list_takes_under_half_and_never_less_than_its_narrowest() {
        assert_eq!(list_width(WIDE), LIST_MIN);
        assert_eq!(list_width(116), LIST_MIN);
        assert_eq!(list_width(120), 54);
        assert_eq!(list_width(150), 67);
        assert_eq!(list_width(200), 90);
    }

    #[test]
    fn the_bar_lights_the_project_it_is_narrowed_to_and_ends_a_cell_short() {
        let mut app = home(Lang::En);
        let name = clean(&app.world.projects[1].name);
        let reversed = |line: &Line, word: &str| {
            let span = line
                .spans
                .iter()
                .find(|s| s.content.contains(word))
                .unwrap();
            span.style.add_modifier.contains(Modifier::REVERSED)
        };
        let bar = project_bar(&app, 150);
        assert!(reversed(&bar, " all "));
        assert!(!reversed(&bar, &name));
        assert_eq!(bar.width(), 149);
        app.at_home.scope = Some(1);
        let bar = project_bar(&app, 150);
        assert!(!reversed(&bar, " all "));
        assert!(reversed(&bar, &name));
        // The words go when only their own width and the last cell are left.
        let plain: usize = home_bar_left(&bar);
        let writes = bar.spans.last().unwrap().width();
        let fits = u16::try_from(plain + writes + 2).unwrap();
        assert!(project_bar(&app, fits).width() > plain);
        assert_eq!(project_bar(&app, fits - 1).width(), plain);
    }

    /// The bar's width before the blank that ends it.
    fn home_bar_left(bar: &Line) -> usize {
        let n = bar.spans.len() - 2;
        bar.spans[..n].iter().map(Span::width).sum()
    }

    fn row_text(app: &App, row: HomeRow) -> String {
        let line = row_line(app, row, 100, false);
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn only_a_change_s_row_ends_with_its_lines() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.changes.push(change(a, Vec::new()));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        app.world.inbox.push(item(InboxKind::Question, a, "Which?"));
        assert!(row_text(&app, HomeRow::Item(0)).contains("+19 −6"));
        let question = row_text(&app, HomeRow::Item(1));
        assert!(question.contains("question"), "{question}");
        assert!(!question.contains("+19"), "{question}");
    }

    #[test]
    fn a_row_names_its_project_only_where_that_tells_it_apart() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let name = clean(&app.world.projects[app.world.agents[a].project].name);
        let row = HomeRow::Agent(a);
        assert!(row_text(&app, row).contains(&name));
        app.at_home.scope = Some(app.world.agents[a].project);
        assert!(!row_text(&app, row).contains(&name));
        app.at_home.scope = None;
        let other = 1 - app.world.agents[a].project;
        app.world.projects[other].closed = true;
        assert!(!row_text(&app, row).contains(&name));
    }

    #[test]
    fn a_row_says_how_long_from_a_minute_on() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.inbox.push(item(InboxKind::Reply, a, "x"));
        let tick = 12;
        app.world.tick = tick;
        let row = |app: &App| render(app, 150, 40);
        assert!(row(&app).contains("1 min"), "{}", row(&app));
        app.world.tick = tick - 1;
        assert!(!row(&app).contains(" 55 s"), "{}", row(&app));
    }
}
