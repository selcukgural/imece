//! The detail's frame (record 125): the piece of work's head line, a
//! question's block with the answers it offers, and the tab bar; the
//! body under them is the tab's own screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::*;
use crate::app::{App, Tab};

/// Draws the frame at the top of `area` and gives back what is left for
/// the body.
pub(crate) fn frame(f: &mut Frame, area: Rect, app: &App, id: AgentId, tab: Tab) -> Rect {
    let width = usize::from(area.width);
    let mut lines = head_lines(app, id, tab, width);
    if let Some(q) = app.world.question_of(id) {
        lines.push(Line::default());
        lines.extend(question_lines(app, id, &q, width));
    }
    lines.push(Line::default());
    lines.push(tab_line(app, id, tab, width));
    let rows = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let [top, body] = Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).areas(area);
    f.render_widget(Paragraph::new(lines), top);
    body
}

/// What the piece of work is called: its card's title, else its change's
/// on the diff, else what the focus said first: a researcher's topic, a
/// helper's worker, the agent's name.
fn title_of(app: &App, id: AgentId, tab: Tab) -> String {
    let w = &app.world;
    let t = app.lang.text();
    let a = &w.agents[id];
    if let Some(c) = app.detail_card(id).and_then(|c| w.cards.get(c)) {
        return clean(&c.title);
    }
    if tab == Tab::Diff
        && let Some(c) = w.change_of(id)
    {
        return clean(&c.title);
    }
    if a.role == Role::Researcher {
        return fill(t.research, &[&a.topic]);
    }
    if let Some(p) = a.parent {
        return fill(t.helper_of, &[&w.agents[p].name]);
    }
    clean(&a.name)
}

/// The head line: the card and its title, the project, the agent, how
/// long it has been as it is, what it cost; on the diff its files and
/// lines, and which change of those waiting it is, at the right.
pub(crate) fn head_line(app: &App, id: AgentId, tab: Tab, width: usize) -> Line<'static> {
    let w = &app.world;
    let a = &w.agents[id];
    let mut spans = Vec::new();
    if let Some(c) = app.detail_card(id) {
        spans.push(Span::styled(format!("{} ", words::card_ref(c)), dim()));
    }
    spans.push(Span::styled(title_of(app, id, tab), bold(pal().strong)));
    spans.push(Span::styled(
        format!(" · {}", clean(&w.projects[a.project].name)),
        dim(),
    ));
    spans.push(Span::styled(
        format!(" · {}", clean(&a.name)),
        fg(pal().agent),
    ));
    let seconds = w.tick.saturating_sub(app.since(id)) * SECONDS_PER_TICK;
    if seconds >= 60 {
        spans.push(Span::styled(
            format!(" · {}", words::ago(app.lang, seconds)),
            dim(),
        ));
    }
    spans.push(Span::styled(format!(" · {}", w.spent_by(a.id)), dim()));
    if tab == Tab::Diff
        && let Some(c) = w.change_of(id)
    {
        let (added, removed) = c.counts();
        let t = app.lang.text();
        let files = match c.files.len() {
            1 => t.row_file.to_string(),
            n => fill(t.row_files, &[&n.to_string()]),
        };
        spans.push(Span::styled(
            format!(" · {files} +{added} −{removed}"),
            dim(),
        ));
    }
    let right = match tab {
        Tab::Diff => diff::place_among(app.lang, w, id),
        _ => None,
    };
    if let Some(place) = right {
        let used: usize = spans.iter().map(Span::width).sum();
        let place = place.trim().to_string();
        let gap = width.saturating_sub(used + place.chars().count());
        if gap > 1 {
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(place, dim()));
        }
    }
    Line::from(spans)
}

/// The head line, or where it is wider than `width` (record 131: a
/// narrow screen) its title on a line and the rest under it, each
/// wrapped at its blanks rather than cut by the edge.
pub(crate) fn head_lines(app: &App, id: AgentId, tab: Tab, width: usize) -> Vec<Line<'static>> {
    let line = head_line(app, id, tab, width);
    if line.width() <= width {
        return vec![line];
    }
    let title = 1 + usize::from(app.detail_card(id).is_some());
    let mut spans = line.spans;
    let mut rest = spans.split_off(title.min(spans.len()));
    if let Some(first) = rest.first_mut() {
        first.content = first.content.trim_start_matches(" · ").to_string().into();
    }
    let mut lines = wrap(Line::from(spans), width.max(1));
    if !rest.is_empty() {
        lines.extend(wrap(Line::from(rest), width.max(1)));
    }
    lines
}

/// A question's block: who asks, the question whole, then the answers
/// it offers with the key that picks each, and the key that writes
/// one's own.
pub(crate) fn question_lines(
    app: &App,
    id: AgentId,
    q: &Question,
    width: usize,
) -> Vec<Line<'static>> {
    let t = app.lang.text();
    let name = clean(&app.world.agents[id].name);
    let mut head = vec![Span::styled(
        fill(t.asks_you_head, &[&name]),
        bold(pal().warn),
    )];
    // A leader's question about a worker's block (record 143) says why
    // it asks, and reads the worker's words and what the leader tried
    // before the question.
    let block = app.world.asked_block(id).map(|(_, b)| b);
    let asked = match block.map(|b| &b.stage) {
        Some(crate::blocked::Stage::Asked { tried, risk }) => Some((tried, risk)),
        _ => None,
    };
    if let Some((_, risk)) = asked {
        let why = match risk {
            Some(risk) => fill(t.block_risky, &[&super::plain(risk)]),
            None => t.block_could_not.to_string(),
        };
        head.push(Span::styled(format!(" · {why}"), dim()));
    }
    let mut lines = super::wrap(Line::from(head), width.max(1));
    let lead = if asked.is_some() { "    " } else { "  " };
    let room = width.saturating_sub(lead.len()).max(1);
    // Cleaned line by line: a newline is a control character too.
    let said = |text: &str, lines: &mut Vec<Line<'static>>| {
        for text in text.lines().map(|l| clean(l.trim_end())) {
            if text.trim().is_empty() {
                continue;
            }
            for row in overlay::wrap_words(&text, room) {
                lines.push(Line::from(vec![Span::raw(lead), Span::raw(row)]));
            }
        }
    };
    if let (Some(b), Some((tried, _))) = (block, asked) {
        let label = |text: String| Line::from(Span::styled(format!("  {text}"), dim()));
        lines.push(label(t.blocked_word.to_string()));
        said(&b.said, &mut lines);
        if !tried.trim().is_empty() {
            lines.push(label(fill(t.block_tried, &[&name])));
            said(tried, &mut lines);
        }
        lines.push(label(t.block_asks.to_string()));
    }
    said(q.text, &mut lines);
    lines.push(Line::default());
    let mut keys = vec![Span::raw("  ")];
    for (n, option) in q.options.iter().enumerate() {
        keys.push(Span::styled(format!(" {} ", n + 1), key_style()));
        keys.push(Span::raw(format!(" {}     ", clean(option))));
    }
    keys.push(Span::styled(" / ", key_style()));
    keys.push(Span::styled(format!(" {}", t.write_own), dim()));
    lines.extend(super::wrap(Line::from(keys), width.max(1)));
    lines
}

/// The tab bar: `diff · talk · log`, the open one bold, one with nothing
/// to show dim, and the key that switches when another has something.
pub(crate) fn tab_line(app: &App, id: AgentId, tab: Tab, width: usize) -> Line<'static> {
    let t = app.lang.text();
    let mut spans = Vec::new();
    let mut others = 0;
    for (n, each) in Tab::ALL.iter().enumerate() {
        if n > 0 {
            spans.push(Span::styled(" · ", dim()));
        }
        let has = app.tab_has(id, *each);
        if *each != tab && has {
            others += 1;
        }
        let style = if *each == tab {
            bold(pal().strong)
        } else if has {
            Style::new()
        } else {
            dim()
        };
        spans.push(Span::styled(t.tabs[n].to_string(), style));
    }
    if others > 0 {
        let used: usize = spans.iter().map(Span::width).sum();
        let hint = fill(t.tab_switches, &[]);
        let gap = width.saturating_sub(used + hint.chars().count());
        if gap > 1 {
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(hint, dim()));
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Engine;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn team() -> App {
        let (world, sim) = crate::sim::world(1, 2, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app
    }

    fn worker(app: &App) -> AgentId {
        app.world.projects[0].leader + 1
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| crate::ui::draw(f, app)).unwrap();
        let buf = t.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn change(agent: AgentId, files: usize) -> Change {
        Change {
            agent,
            title: "Paging".into(),
            files: (0..files)
                .map(|n| ChangedFile {
                    path: format!("src/{n}.rs"),
                    added: 2,
                    removed: 1,
                    binary: false,
                })
                .collect(),
            patch: "diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        }
    }

    #[test]
    fn a_head_line_wider_than_the_screen_breaks_after_its_title() {
        let mut app = team();
        let a = worker(&app);
        let w = &mut app.world;
        let (project, name) = (w.projects[0].name.clone(), w.agents[a].name.clone());
        w.agents[a].card = Some(0);
        w.cards[0].title = "Refund flow".into();
        w.agents[a].cost = 600;
        let whole = format!("#1 Refund flow · {project} · {name} · $0.06");
        let lines = |app: &App, width| -> Vec<String> {
            head_lines(app, a, Tab::Log, width)
                .iter()
                .map(text)
                .collect()
        };
        let n = whole.chars().count();
        assert_eq!(lines(&app, n), [whole], "just fits: one line");
        let got = lines(&app, n - 1);
        assert_eq!(got[0].trim_end(), "#1 Refund flow", "{got:?}");
        assert_eq!(got[1].trim_end(), format!("{project} · {name} · $0.06"));
        // Narrower still, each part wraps at its blanks.
        let got = lines(&app, 12);
        assert!(
            got.iter().all(|l| l.trim_end().chars().count() <= 12),
            "{got:?}"
        );
        assert_eq!(got[0].trim_end(), "#1 Refund", "{got:?}");
        // Without a card the title alone goes first.
        app.world.agents[a].card = None;
        let got = lines(&app, 20);
        assert_eq!(got[0].trim_end(), name, "{got:?}");
        assert!(got[1].starts_with(&project), "{got:?}");
    }

    #[test]
    fn the_head_names_the_card_the_project_the_agent_the_time_and_the_cost() {
        let mut app = team();
        let a = worker(&app);
        let w = &mut app.world;
        let (project, name) = (w.projects[0].name.clone(), w.agents[a].name.clone());
        w.agents[a].card = Some(0);
        w.cards[0].title = "Refund \u{1b}[31mflow".into();
        w.agents[a].cost = 600;
        w.tick = 100;
        w.agents[a].changed_at = 100;
        let title = clean("Refund \u{1b}[31mflow");
        let head = |app: &App, tab| text(&head_line(app, a, tab, 120));
        assert_eq!(
            head(&app, Tab::Log),
            format!("#1 {title} · {project} · {name} · $0.06")
        );
        // From a minute on, how long it has been as it is.
        app.world.agents[a].changed_at = 100 - 11;
        assert!(
            !head(&app, Tab::Log).contains(&words::ago(Lang::En, 55)),
            "55 s"
        );
        app.world.agents[a].changed_at = 100 - 12;
        assert!(
            head(&app, Tab::Log).ends_with(&format!("{} · $0.06", words::ago(Lang::En, 60))),
            "{}",
            head(&app, Tab::Log)
        );
        // On the diff: the files and lines; which change of those
        // waiting, at the right edge.
        app.world.change_ready(change(a, 1));
        assert!(
            head(&app, Tab::Diff).ends_with(" · 1 file +2 −1"),
            "{}",
            head(&app, Tab::Diff)
        );
        assert!(
            !head(&app, Tab::Log).contains("+2 −1"),
            "the log says no counts"
        );
        app.world.changes[0].files = change(a, 3).files;
        let line = head(&app, Tab::Diff);
        assert!(line.ends_with(" · 3 files +6 −3"), "{line}");
        app.world.change_ready(change(a + 1, 1));
        let line = head(&app, Tab::Diff);
        assert!(line.ends_with("change 1 of 2"), "{line}");
        assert_eq!(line.chars().count(), 120, "at the right edge");
        assert!(
            !head(&app, Tab::Log).contains("change 1 of 2"),
            "only the diff says which change"
        );
        let spans = head_line(&app, a, Tab::Diff, 120);
        assert_eq!(spans.spans[1].style, bold(pal().strong), "the title");
        assert_eq!(spans.spans[3].style, fg(pal().agent), "the agent");
        // The right part wants a gap of two cells at least.
        let whole = head(&app, Tab::Diff);
        let left = whole.trim_end_matches("change 1 of 2").trim_end();
        let used = left.chars().count();
        let tight = head_line(&app, a, Tab::Diff, used + 13 + 1);
        assert!(!text(&tight).contains("change 1 of 2"), "{}", text(&tight));
        let loose = head_line(&app, a, Tab::Diff, used + 13 + 2);
        assert!(
            text(&loose).ends_with("  change 1 of 2"),
            "{}",
            text(&loose)
        );
    }

    #[test]
    fn the_head_s_title_is_the_card_s_else_the_change_s_on_the_diff_else_who_it_is() {
        let mut app = team();
        let a = worker(&app);
        app.world.agents[a].card = None;
        let name = app.world.agents[a].name.clone();
        let head = |app: &App, tab| text(&head_line(app, a, tab, 120));
        assert!(
            head(&app, Tab::Log).starts_with(&format!("{name} · ")),
            "{}",
            head(&app, Tab::Log)
        );
        app.world.change_ready(change(a, 1));
        assert!(
            head(&app, Tab::Diff).starts_with("Paging · "),
            "{}",
            head(&app, Tab::Diff)
        );
        assert!(
            head(&app, Tab::Log).starts_with(&format!("{name} · ")),
            "not on the log"
        );
        // The item's card over the agent's: a change stays with the card
        // it was made for.
        let i = app.world.inbox.len() - 1;
        app.world.inbox[i].card = Some(1);
        let title = clean(&app.world.cards[1].title);
        assert!(
            head(&app, Tab::Diff).starts_with(&format!("#2 {title} · ")),
            "{}",
            head(&app, Tab::Diff)
        );
        app.world.inbox.clear();
        app.world.changes.clear();
        // A researcher's topic; a helper's worker.
        app.world.agents[a].role = Role::Researcher;
        app.world.agents[a].topic = "rounding".into();
        let t = Lang::En.text();
        assert!(head(&app, Tab::Log).starts_with(&fill(t.research, &["rounding"])));
        app.world.agents[a].role = Role::Helper;
        app.world.agents[a].parent = Some(a + 1);
        let of = app.world.agents[a + 1].name.clone();
        assert!(head(&app, Tab::Log).starts_with(&fill(t.helper_of, &[&of])));
    }

    #[test]
    fn a_question_s_block_says_who_asks_the_question_whole_and_the_keys() {
        let app = team();
        let a = worker(&app);
        let name = app.world.agents[a].name.clone();
        let q = Question {
            text: "Which card?\n\nThe \u{1b}[31mred one?\n",
            options: &["the card".into(), "credit".into()],
        };
        let lines = question_lines(&app, a, &q, 100);
        let texts: Vec<String> = lines.iter().map(text).collect();
        assert_eq!(texts[0], format!("{name} asks you"));
        assert_eq!(lines[0].spans[0].style, bold(pal().warn));
        assert_eq!(texts[1], "  Which card?");
        assert_eq!(texts[2], "  The [31mred one?", "cleaned, blanks left out");
        assert_eq!(texts[3], "");
        assert_eq!(
            texts[4],
            "   1  the card      2  credit      /  write your own answer"
        );
        assert_eq!(lines[4].spans[1].style, key_style());
        assert_eq!(texts.len(), 5);
        // Without answers: the key for one's own alone; a long question
        // and a long row of keys wrap.
        let q = Question {
            text: &"word ".repeat(30),
            options: &[],
        };
        let lines = question_lines(&app, a, &q, 40);
        let texts: Vec<String> = lines.iter().map(text).collect();
        assert!(texts[1].starts_with("  word word"), "{}", texts[1]);
        assert!(texts.iter().all(|l| l.chars().count() <= 40), "{texts:?}");
        assert!(texts.last().unwrap().contains("write your own answer"));
        assert!(!texts.iter().any(|l| l.contains(" 1 ")), "no first answer");
        let tr = {
            let mut app = team();
            app.lang = Lang::Tr;
            question_lines(&app, a, &q, 100)
        };
        assert!(text(&tr[0]).ends_with("sana soruyor"));
        assert!(text(tr.last().unwrap()).contains("kendi yanıtını yaz"));
    }

    #[test]
    fn the_tab_bar_marks_the_open_tab_dims_an_empty_one_and_offers_the_key_when_another_has() {
        let mut app = team();
        let a = worker(&app);
        app.world.inbox.clear();
        // The log alone: the two others dim, nothing to switch to.
        let line = tab_line(&app, a, Tab::Log, 60);
        assert_eq!(text(&line), "diff · talk · log");
        assert_eq!(line.spans[0].style, dim(), "no change");
        assert_eq!(line.spans[2].style, dim(), "nothing said");
        assert_eq!(line.spans[4].style, bold(pal().strong), "open");
        app.world.change_ready(change(a, 1));
        let line = tab_line(&app, a, Tab::Log, 60);
        assert!(text(&line).ends_with("tab switches"), "{}", text(&line));
        assert_eq!(text(&line).chars().count(), 60);
        // The hint wants a gap of two cells at least: 17 + 12 + 2.
        let tight = tab_line(&app, a, Tab::Log, 30);
        assert_eq!(text(&tight), "diff · talk · log");
        let loose = tab_line(&app, a, Tab::Log, 31);
        assert_eq!(text(&loose), "diff · talk · log  tab switches");
        assert_eq!(line.spans[0].style, Style::new(), "a change to show");
        let line = tab_line(&app, a, Tab::Diff, 60);
        assert_eq!(line.spans[0].style, bold(pal().strong));
        assert_eq!(line.spans[4].style, Style::new(), "the log always has");
        let mut app = team();
        app.lang = Lang::Tr;
        assert_eq!(
            text(&tab_line(&app, a, Tab::Log, 60)),
            "diff · konuşma · log"
        );
    }

    #[test]
    fn the_frame_heads_a_change_a_focus_and_a_conversation_and_leaves_the_rest_to_them() {
        let mut app = team();
        let a = worker(&app);
        app.world.agents[a].card = Some(0);
        app.world.change_ready(change(a, 1));
        app.zoom = Zoom::Diff(a);
        let s = render(&app, 100, 30);
        let rows: Vec<&str> = s.lines().collect();
        assert!(rows[1].starts_with("#1 "), "{s}");
        assert!(rows[1].contains("1 file +2 −1"), "{s}");
        assert_eq!(rows[2].trim(), "");
        assert!(rows[3].starts_with("diff · talk · log"), "{s}");
        assert!(
            rows[4].starts_with('┌'),
            "the change's own frame under it: {s}"
        );
        // A question of the agent stands between, whatever the tab.
        app.world.note(Event::ToYou {
            from: a,
            kind: MsgKind::Ask,
            text: "Which card?".into(),
            options: vec!["the card".into()],
        });
        app.world.asked_you(a, "Which card?");
        for zoom in [Zoom::Diff(a), Zoom::Focus(a), Zoom::Trace(a)] {
            app.zoom = zoom;
            let s = render(&app, 100, 30);
            let rows: Vec<&str> = s.lines().collect();
            assert!(rows[3].contains("asks you"), "{zoom:?}: {s}");
            assert!(rows[4].contains("Which card?"), "{s}");
            assert!(
                rows[6].contains(" 1  the card") && rows[6].contains("/  write"),
                "{s}"
            );
            assert!(rows[8].starts_with("diff · talk · log"), "{s}");
            assert!(rows[9].starts_with('┌'), "{s}");
        }
        // The focus framed leaves its own first line out; the peek keeps it.
        app.world.inbox.clear();
        app.zoom = Zoom::Focus(a);
        let s = render(&app, 100, 30);
        let rows: Vec<&str> = s.lines().collect();
        assert!(rows[3].starts_with("diff · talk · log"), "{s}");
        assert!(
            rows[5].contains("status"),
            "the status comes first inside: {s}"
        );
        app.zoom = Zoom::None;
        app.peek = Some(a);
        let s = render(&app, 100, 30);
        assert!(
            s.contains("#1 ") && s.contains(" · $"),
            "the peek's own title line: {s}"
        );
        assert!(!s.contains("diff · talk · log"), "no frame on a peek: {s}");
        // A conversation: the window under the frame, never over the head.
        app.peek = None;
        app.world.said(a, "go on");
        app.open_thread(app.world.journal.len() - 1);
        let s = render(&app, 100, 30);
        let rows: Vec<&str> = s.lines().collect();
        assert!(rows[1].starts_with("#1 "), "{s}");
        assert!(
            rows[3].contains("diff · ") && rows[3].contains("talk"),
            "{s}"
        );
        // The person's conversation with it is a chat: a rule, no box.
        assert!(
            rows[4].contains("CONVERSATION · claude-1 ⇄ you · 2/2 ──") && !rows[4].contains('┌'),
            "the conversation starts under the tabs: {s}"
        );
    }

    #[test]
    fn a_leader_s_question_about_a_block_reads_the_worker_s_words_and_what_it_tried() {
        let mut app = team();
        let a = worker(&app);
        let lead = app.world.projects[app.world.agents[a].project].leader;
        let leader = app.world.agents[lead].name.clone();
        app.world
            .blocked(a, lead, 4, "No clock.\nI own only model.rs.");
        app.world.asked_you(lead, "Let it in?");
        let q = Question {
            text: "Let it in?",
            options: &[],
        };
        let text = |app: &App| -> Vec<String> {
            (question_lines(app, lead, &q, 100).iter())
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
                .collect()
        };
        // About no block yet: as any question.
        assert_eq!(
            text(&app)[..2],
            [format!("{leader} asks you"), "  Let it in?".to_string()]
        );
        app.world
            .block_asked(lead, Some(4), "Looked.", Some("it widens the card"));
        assert_eq!(
            text(&app)[..8],
            [
                format!("{leader} asks you · risky: it widens the card"),
                "  blocked".to_string(),
                "    No clock.".to_string(),
                "    I own only model.rs.".to_string(),
                format!("  {leader} tried"),
                "    Looked.".to_string(),
                "  asks".to_string(),
                "    Let it in?".to_string(),
            ]
        );
        app.world.block_asked(lead, Some(4), "", None);
        assert_eq!(
            text(&app)[..5],
            [
                format!("{leader} asks you · could not solve it"),
                "  blocked".to_string(),
                "    No clock.".to_string(),
                "    I own only model.rs.".to_string(),
                "  asks".to_string(),
            ]
        );
    }
}
