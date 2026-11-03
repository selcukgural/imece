//! One agent up close: its card's story (intent, decisions, assumptions,
//! conversations, evidence) and, one level deeper, its raw stream.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use unicode_width::UnicodeWidthStr;

use super::*;
use crate::app::App;
use crate::i18n::Text;

fn story_icon(k: StoryKind) -> (&'static str, usize, Color) {
    match k {
        StoryKind::Intent => ("◎", 0, pal().info),
        StoryKind::Assumption => ("⚑", 1, pal().warn),
        StoryKind::Decision => ("⚖", 2, pal().plan),
        StoryKind::Talk => ("⇄", 3, pal().info),
        StoryKind::Evidence => ("◌", 4, pal().ok),
        StoryKind::Change => ("Δ", 5, pal().strong),
        StoryKind::Person => ("›", 6, pal().strong),
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let w = &app.world;
    let t = app.lang.text();
    let a = &w.agents[id];
    let block = Block::bordered()
        .title(fill(t.focus_title, &[&a.name]))
        .border_style(fg(vendor_color(a.vendor)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut head = Vec::new();
    let card = a.card.map(|c| &w.cards[c]);
    let mut title = vec![Span::styled(
        format!("{} · ", w.projects[a.project].name),
        dim(),
    )];
    if let Some(c) = card {
        let minutes = w.tick.saturating_sub(c.started) * SECONDS_PER_TICK / 60;
        title.push(Span::styled(
            format!("#{} {}", c.id + 1, clean(&c.title)),
            bold(pal().strong),
        ));
        title.push(Span::styled(
            fill(t.minutes, &[&minutes.to_string()]),
            dim(),
        ));
    } else if a.role == Role::Researcher {
        title.push(Span::styled(
            fill(t.research, &[&a.topic]),
            bold(pal().strong),
        ));
    } else if let Some(p) = a.parent {
        title.push(Span::styled(
            fill(t.helper_of, &[&w.agents[p].name]),
            bold(pal().strong),
        ));
    } else {
        title.push(Span::styled(t.no_card, dim()));
    }
    title.push(Span::styled(
        format!(" · {} ({})", clean(&a.name), a.vendor.label()),
        fg(vendor_color(a.vendor)),
    ));
    title.push(Span::styled(format!(" · {}", dollars(a.cost)), dim()));
    head.push(Line::from(title));

    let mut status = vec![Span::styled(t.status, dim())];
    status.extend(status_spans(app.lang, w, a));
    head.push(Line::from(status));
    let taint = match a.taint {
        Taint::Clean => Span::styled(t.clean, fg(pal().ok)),
        Taint::Soft => Span::styled(t.soft, fg(pal().warn)),
        Taint::Full => Span::styled(t.full, fg(pal().bad)),
    };
    head.push(Line::from(vec![Span::styled(t.origin, dim()), taint]));
    if let Some(c) = card.filter(|c| !c.symbols.is_empty()) {
        let leases = c
            .symbols
            .iter()
            .map(|s| format!("{}#{}", s.file, s.name))
            .collect::<Vec<_>>()
            .join(", ");
        head.push(Line::from(vec![
            Span::styled(t.lease, dim()),
            Span::raw(leases),
        ]));
    }
    if let Some(f) = a.context {
        let line = match (f.percent(), f.size) {
            (Some(p), Some(size)) => fill(
                t.context_line,
                &[&p.to_string(), &words::tokens(f.used), &words::tokens(size)],
            ),
            _ => fill(t.context_tokens, &[&words::tokens(f.used)]),
        };
        let style = match f.percent() {
            Some(p) if p >= CONTEXT_LOUD => fg(pal().warn),
            _ => dim(),
        };
        head.push(Line::from(Span::styled(line, style)));
    }
    head.extend(activity_line(app, a));
    if !a.plan.is_empty() {
        let (done, all) = a.plan_count();
        head.push(Line::from(Span::styled(
            fill(t.plan_title, &[&done.to_string(), &all.to_string()]),
            bold(pal().strong),
        )));
        head.extend(plan_lines(t, &a.plan));
    }

    let [top, body] = Layout::vertical([
        Constraint::Length(head.len() as u16 + 1),
        Constraint::Min(0),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(head), top);

    let story: Vec<Line> = match card {
        Some(c) => story_lines(app, c, id),
        None => vec![Line::from(Span::styled(t.no_story, dim()))],
    };
    tail_linked(f, body, story);
}

/// The focus's activity bar: the agent's trace lines in each of the last
/// [`STIR_LONG`] ticks on the row's scale, and how many in all; none
/// while they are all empty or motion is off, as on the row.
pub(super) fn activity_line(app: &App, a: &Agent) -> Option<Line<'static>> {
    if app.motion == crate::motion::Motion::Off {
        return None;
    }
    let counts = a.stirs.long(app.world.tick);
    let all: u32 = counts.iter().map(|&n| u32::from(n)).sum();
    if all == 0 {
        return None;
    }
    let bar: String = counts
        .iter()
        .map(|&n| super::salon::stir_glyph(n))
        .collect();
    let text = fill(app.lang.text().activity_line, &[&bar, &all.to_string()]);
    Some(Line::from(Span::styled(text, dim())))
}

/// Rows the agent's to-do list may take on its focus, the title not
/// counted.
pub(crate) const PLAN_ROWS: usize = 8;

/// The agent's to-do list, a step a line in its order: `✓` done, `▸` in
/// hand, `○` not begun. A list longer than [`PLAN_ROWS`] folds the done
/// steps at its top into one line and counts what it leaves off, so what
/// is in hand stays in view.
pub(crate) fn plan_lines(t: &Text, plan: &[Step]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut rest = plan;
    if plan.len() > PLAN_ROWS {
        let done = plan
            .iter()
            .take_while(|s| s.state == StepState::Done)
            .count();
        if done > 0 {
            lines.push(Line::from(Span::styled(
                fill(t.plan_done, &[&done.to_string()]),
                fg(pal().ok),
            )));
            rest = &plan[done..];
        }
    }
    let room = PLAN_ROWS - lines.len();
    let shown = if rest.len() > room {
        room - 1
    } else {
        rest.len()
    };
    for s in &rest[..shown] {
        let (glyph, style) = match s.state {
            StepState::Done => ("✓", fg(pal().ok)),
            StepState::Doing => ("▸", bold(pal().info)),
            StepState::Todo => ("○", dim()),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{glyph} "), style),
            Span::styled(fit(&s.text, 100).trim_end().to_string(), style),
        ]));
    }
    if rest.len() > shown {
        lines.push(Line::from(Span::styled(
            fill(t.plan_more, &[&(rest.len() - shown).to_string()]),
            dim(),
        )));
    }
    lines
}

/// Card `c`'s story, an event a line, oldest first; conversation lines
/// are told as agent `me` (the card's owner) had them. What an agent
/// wrote is drawn as the conversation draws it: a one-line message and a
/// decision or an assumption with their inline marks on their line; a
/// message of more lines (or a list, a heading, code) under its head, as
/// Markdown, its first [`STORY_ROWS`] lines and how many more. imece's own
/// lines are plain.
pub(super) fn story_lines(app: &App, c: &Card, me: AgentId) -> Vec<Line<'static>> {
    let t = app.lang.text();
    let mut out = Vec::new();
    for s in &c.story {
        let (icon, label, color) = story_icon(s.event.kind());
        let when = clock(s.tick);
        let mut spans = vec![
            Span::styled(format!("{when} "), dim()),
            Span::styled(
                format!("{icon} {} ", fit(t.story_labels[label], 10)),
                fg(color),
            ),
        ];
        let said = words::said_in_story(me, &s.event);
        match (&said, &s.event) {
            (Some(said), _) => {
                spans.extend(said_spans(app.lang, &app.world, said));
                spans.pop(); // the one-line text, drawn below
                let body = body_lines(said.text);
                if let [line] = body.as_slice()
                    && !is_block(line)
                {
                    spans.push(Span::raw(": "));
                    spans.extend(inline(line, Style::new()));
                    out.push(Line::from(spans));
                    continue;
                }
                spans.push(Span::raw(":"));
                out.push(Line::from(spans));
                let indent = " ".repeat(when.width() + 1);
                out.extend(capped(markdown_src(said.text, &indent), &indent));
                continue;
            }
            (None, StoryEvent::Decision(text) | StoryEvent::Assumption(text)) => {
                spans.extend(inline(&body_lines(text).join(" "), Style::new()));
            }
            (None, e) => spans.push(Span::raw(words::story(app.lang, &app.world, c, e))),
        }
        out.push(Line::from(spans));
    }
    out
}

/// Lines of a message the focus shows under its head; the rest counted.
pub(crate) const STORY_ROWS: usize = 8;

/// `lines` drawn, up to [`STORY_ROWS`] of them, then `… +N` for the rest.
fn capped(lines: Vec<(Line<'static>, String)>, indent: &str) -> Vec<Line<'static>> {
    let more = lines.len().saturating_sub(STORY_ROWS);
    let mut out: Vec<Line> = lines.into_iter().take(STORY_ROWS).map(|(l, _)| l).collect();
    if more > 0 {
        out.push(Line::from(vec![
            Span::raw(indent.to_string()),
            Span::styled(format!("… +{more}"), dim()),
        ]));
    }
    out
}

/// Whether a line of Markdown is more than words: a fence, a heading, a
/// list item, a quote or a rule, which the focus draws under the head.
pub(crate) fn is_block(line: &str) -> bool {
    let l = line.trim_start();
    l.starts_with("```")
        || l.starts_with('>')
        || heading(l).is_some()
        || list_marker(l).is_some()
        || matches!(l, "---" | "***" | "___")
}

pub fn trace(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let a = &app.world.agents[id];
    let block = Block::bordered()
        .title(fill(app.lang.text().trace_title, &[&a.name]))
        .border_style(border());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = a
        .trace
        .iter()
        .flat_map(|l| trace_lines(l, vendor_color(a.vendor)))
        .collect();
    tail_linked(f, inner, lines);
}

/// What a trace line is, read from its first word: the trace producers'
/// vocabulary in `live/` and `imece_agent`, told apart by a glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TraceKind {
    /// The agent's own words (`text "…"`): the line not to miss.
    Words,
    /// A tool it called (`tool_use`, `tool_result`, `run:`).
    Tool,
    /// The bus: a message, a wake, a hire, the board.
    Bus,
    /// The vendor's own note (`note …`): a retry, tokens, a refusal.
    Note,
    /// Something went wrong: stopped, a run refused, a turn failed.
    Wrong,
    /// imece's own line: the session, the worktree, the change, the plan.
    Own,
}

impl TraceKind {
    pub(crate) fn of(word: &str, arg: &str) -> TraceKind {
        match word {
            "text" => TraceKind::Words,
            "tool_use" | "tool_result" | "run:" => TraceKind::Tool,
            "send" | "mail" | "woken" | "hire" | "hired" | "post" | "claim" | "told:" => {
                TraceKind::Bus
            }
            "note" => TraceKind::Note,
            "stopped" => TraceKind::Wrong,
            "run" if arg.starts_with("refused") => TraceKind::Wrong,
            "result" if arg.starts_with('✕') => TraceKind::Wrong,
            _ => TraceKind::Own,
        }
    }

    /// Its glyph and the glyph's style, `vendor` the agent's colour.
    pub(crate) fn glyph(self, vendor: Color) -> (&'static str, Style) {
        match self {
            TraceKind::Words => ("›", bold(vendor)),
            TraceKind::Tool => ("▸", fg(pal().info)),
            TraceKind::Bus => ("⇄", fg(pal().accent)),
            TraceKind::Note => ("•", fg(pal().warn)),
            TraceKind::Wrong => ("✕", fg(pal().bad)),
            TraceKind::Own => ("·", dim()),
        }
    }
}

/// A trace line drawn so the kinds tell apart at a glance: the clock dim,
/// then the kind's glyph ([`TraceKind`]); the agent's own words
/// (`text "…"`) unquoted, in its vendor's colour, as Markdown (lists,
/// headings, code in the code style, fences never drawn), one row per
/// line of them; a tool's name in the info colour and its target (a
/// command, a path) in the code style; `run:` lines with their end
/// coloured by the exit code; what went wrong in the colour of it; the
/// bus with its first word in the accent; a vendor's note and imece's own
/// lines as written. Web addresses in any of them are drawn as links
/// ([`linked`]). A line that matches no word is still shown whole. A line
/// that runs over rows (a command with a heredoc) keeps them: the first
/// [`TRACE_ROWS`] under it, indented, then how many more.
pub(crate) fn trace_lines(raw: &str, vendor: Color) -> Vec<Line<'static>> {
    let mut raw_rows = raw.lines();
    let line = clean(raw_rows.next().unwrap_or_default());
    let (clock, rest) = line.split_once(' ').unwrap_or((&line, ""));
    let (word, arg) = rest.split_once(' ').unwrap_or((rest, ""));
    let kind = TraceKind::of(word, arg);
    let (glyph, style) = kind.glyph(vendor);
    let indent = " ".repeat(clock.width() + 3);
    let mut spans = vec![
        Span::styled(format!("{clock} "), dim()),
        Span::styled(format!("{glyph} "), style),
    ];
    if kind == TraceKind::Words {
        let text = unquote(arg).unwrap_or_else(|| arg.to_string());
        let mut rows = markdown_in(&text, &indent, fg(vendor)).into_iter();
        if let Some((first, _)) = rows.next() {
            spans.extend(first.spans.into_iter().skip(1));
        }
        let mut out = vec![Line::from(spans)];
        out.extend(rows.map(|(l, _)| l));
        return out;
    }
    let first = rest;
    let code = fg(pal().code);
    match word {
        "tool_use" | "tool_result" => {
            let (name, target) = arg.split_once(' ').unwrap_or((arg, ""));
            spans.push(Span::styled(name.to_string(), bold(pal().info)));
            if target == "✕" {
                spans.push(Span::styled(" ✕", fg(pal().bad)));
            } else if !target.is_empty() {
                spans.push(Span::raw(" "));
                spans.extend(linked(target, code));
            }
        }
        "run:" => {
            spans.push(Span::styled("run ", bold(pal().info)));
            match arg.rsplit_once(": exit ") {
                Some((cmd, end)) => {
                    let ok = end.starts_with("0,") || end == "0";
                    let colour = if ok { pal().ok } else { pal().bad };
                    spans.push(Span::styled(cmd.to_string(), code));
                    spans.push(Span::styled(format!(": exit {end}"), fg(colour)));
                }
                None => spans.push(Span::styled(arg.to_string(), code)),
            }
        }
        _ => match kind {
            TraceKind::Wrong => spans.push(Span::styled(first.to_string(), fg(pal().bad))),
            TraceKind::Bus => {
                spans.push(Span::styled(format!("{word} "), fg(pal().accent)));
                spans.extend(linked(arg, Style::new()));
            }
            TraceKind::Note => spans.extend(linked(arg, Style::new())),
            _ => spans.extend(linked(first, Style::new())),
        },
    }
    let mut out = vec![Line::from(spans)];
    // The rows of a command are code; of anything else, as written.
    let more_style = if kind == TraceKind::Tool {
        code
    } else {
        Style::new()
    };
    let more: Vec<&str> = raw_rows.collect();
    for row in more.iter().take(TRACE_ROWS) {
        out.push(Line::from(vec![
            Span::raw(indent.clone()),
            Span::styled(clean(row), more_style),
        ]));
    }
    if more.len() > TRACE_ROWS {
        let left = format!("… +{}", more.len() - TRACE_ROWS);
        out.push(Line::from(vec![
            Span::raw(indent.clone()),
            Span::styled(left, dim()),
        ]));
    }
    out
}

/// How many rows of a trace line are drawn after its first.
const TRACE_ROWS: usize = 6;

/// The string a `{:?}` wrote, back: the quotes off and `\"`, `\\`, `\n`,
/// `\t` unescaped (other escapes stay as written). None when `s` is not
/// one quoted string.
pub(crate) fn unquote(s: &str) -> Option<String> {
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_kinds_have_their_own_icon_and_order() {
        let all = [
            StoryKind::Intent,
            StoryKind::Assumption,
            StoryKind::Decision,
            StoryKind::Talk,
            StoryKind::Evidence,
            StoryKind::Change,
            StoryKind::Person,
        ];
        let icons: Vec<(&str, usize)> = all
            .iter()
            .map(|&k| {
                let (i, n, _) = story_icon(k);
                (i, n)
            })
            .collect();
        assert_eq!(
            icons,
            [
                ("◎", 0),
                ("⚑", 1),
                ("⚖", 2),
                ("⇄", 3),
                ("◌", 4),
                ("Δ", 5),
                ("›", 6)
            ]
        );
    }

    #[test]
    fn the_focus_shows_two_minutes_of_activity_on_the_rows_scale() {
        use crate::i18n::Lang;
        use crate::motion::Motion;
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.motion = Motion::Calm;
        let id = (app.world.agents.iter())
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.tick = 100;
        app.world.agents[id].stirs = Stirs::default();
        assert_eq!(
            activity_line(&app, &app.world.agents[id]),
            None,
            "nothing done"
        );
        // Two minutes, the row's five ticks at its end.
        assert_eq!(STIR_LONG as u64 * SECONDS_PER_TICK, 120);
        let a = &mut app.world.agents[id];
        for (tick, n) in [(77, 1), (80, 3), (99, 15), (100, 2)] {
            for _ in 0..n {
                a.stirs.add(tick);
            }
        }
        let text = |app: &App| {
            let line = activity_line(app, &app.world.agents[id]).expect("shown");
            assert_eq!(line.spans[0].style, dim());
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };
        assert_eq!(
            text(&app),
            "activity: ▂▁▁▄▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁█▃ · trace lines in 2 min: 21"
        );
        app.lang = Lang::Tr;
        assert_eq!(
            text(&app),
            "etkinlik: ▂▁▁▄▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁█▃ · 2 dakikada iz satırı: 21"
        );
        app.lang = Lang::En;
        // A tick on, the oldest line has gone off its start.
        app.world.tick = 101;
        assert!(text(&app).starts_with("activity: ▁▁▄▁"), "{}", text(&app));
        assert!(text(&app).ends_with(": 20"));
        // Columns past what a u16 holds still add up.
        app.world.agents[id].stirs = Stirs::default();
        for tick in [100, 101] {
            for _ in 0..u16::MAX {
                app.world.agents[id].stirs.add(tick);
            }
        }
        assert!(text(&app).ends_with(&format!(": {}", 2 * u32::from(u16::MAX))));
        // Off: the screen as it was before motion, on the focus too.
        app.motion = Motion::Off;
        assert_eq!(activity_line(&app, &app.world.agents[id]), None);
        app.motion = Motion::Calm;
        // Drawn on the focus, between the status lines and the story.
        let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
        t.draw(|f| draw(f, f.area(), &app, id)).unwrap();
        let rows: Vec<String> = (0..20)
            .map(|y| {
                (0..100)
                    .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect();
        assert!(
            rows.iter()
                .any(|r| r.contains("activity: ▁") && r.contains(": 131070")),
            "{rows:#?}"
        );
    }

    fn texts(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    fn colour_of(line: &Line<'static>, text: &str) -> Option<Color> {
        line.spans
            .iter()
            .find(|s| s.content == text)
            .and_then(|s| s.style.fg)
    }

    fn style_of(line: &Line<'static>, text: &str) -> Option<Style> {
        line.spans
            .iter()
            .find(|s| s.content == text)
            .map(|s| s.style)
    }

    #[test]
    fn the_agents_own_words_in_the_trace_are_unquoted_markdown_in_its_colour() {
        let v = Color::Indexed(9);
        let lines = trace_lines(
            "12:00:01 text \"Four pass.\\n- **All** green \\\"now\\\"\\n```\\ncode\\n```\"",
            v,
        );
        assert_eq!(
            texts(&lines),
            [
                "12:00:01 › Four pass.",
                "           • All green \"now\"",
                "           │ code",
            ]
        );
        assert_eq!(lines[0].spans[0].style, dim());
        assert_eq!(style_of(&lines[0], "› "), Some(bold(v)));
        assert_eq!(colour_of(&lines[0], "Four pass."), Some(v));
        let all = lines[1].spans.iter().find(|s| s.content == "All").unwrap();
        assert_eq!(all.style.fg, Some(v));
        assert!(all.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(colour_of(&lines[2], "code"), Some(pal().code));
        assert_eq!(
            texts(&trace_lines("12:00:01 text not quoted", v)),
            ["12:00:01 › not quoted"],
            "a text that is not one quoted string shows as written"
        );
        assert_eq!(
            texts(&trace_lines("12:00:01 text \"\"", v)),
            ["12:00:01 › "]
        );
        // A web address in its words is a link's span.
        let l = trace_lines("12:00:01 text \"see https://a.b/c.\"", v).remove(0);
        assert_eq!(style_of(&l, "https://a.b/c"), Some(link_style()));
        assert_eq!(colour_of(&l, "."), Some(v));
    }

    #[test]
    fn a_command_over_rows_keeps_them_up_to_a_few_and_counts_the_rest() {
        let v = Color::Indexed(9);
        let body = |n: usize| (1..=n).map(|i| format!("\nline {i}")).collect::<String>();
        let six = format!("12:00:02 tool_use Bash cat > a.txt <<EOF{}", body(6));
        let lines = trace_lines(&six, v);
        assert_eq!(
            texts(&lines),
            [
                "12:00:02 ▸ Bash cat > a.txt <<EOF",
                "           line 1",
                "           line 2",
                "           line 3",
                "           line 4",
                "           line 5",
                "           line 6",
            ]
        );
        assert_eq!(colour_of(&lines[1], "line 1"), Some(pal().code));
        let nine = format!("12:00:02 tool_use Bash cat > a.txt <<EOF{}\n", body(9));
        let lines = trace_lines(&nine, v);
        assert_eq!(lines.len(), 8, "{:?}", texts(&lines));
        assert_eq!(texts(&lines[6..]), ["           line 6", "           … +3"]);
        assert_eq!(colour_of(&lines[7], "… +3"), Some(pal().dim));
        assert_eq!(style_of(&lines[0], "Bash"), Some(bold(pal().info)));
        assert_eq!(colour_of(&lines[0], "cat > a.txt <<EOF"), Some(pal().code));
        // Each row is cleaned on its own.
        assert_eq!(
            texts(&trace_lines(
                "12:00:02 tool_use Bash x\nTOKEN=abc \u{1b}[2J",
                v
            ))[1],
            "           [***]  [2J"
        );
        // The rows of anything but a tool are as written.
        let lines = trace_lines("12:00:02 worktree: x\nmore", v);
        assert_eq!(texts(&lines), ["12:00:02 · worktree: x", "           more"]);
        assert_eq!(style_of(&lines[1], "more"), Some(Style::new()));
    }

    #[test]
    fn each_kind_of_trace_line_has_a_glyph_of_its_own() {
        let v = Color::Indexed(9);
        let p = pal();
        for (word, arg, kind) in [
            ("text", "", TraceKind::Words),
            ("tool_use", "", TraceKind::Tool),
            ("tool_result", "", TraceKind::Tool),
            ("run:", "", TraceKind::Tool),
            ("send", "", TraceKind::Bus),
            ("mail", "", TraceKind::Bus),
            ("woken", "", TraceKind::Bus),
            ("hire", "", TraceKind::Bus),
            ("hired", "", TraceKind::Bus),
            ("post", "", TraceKind::Bus),
            ("claim", "", TraceKind::Bus),
            ("told:", "", TraceKind::Bus),
            ("note", "", TraceKind::Note),
            ("stopped", "", TraceKind::Wrong),
            ("run", "refused: rm", TraceKind::Wrong),
            ("run", "make", TraceKind::Own),
            ("result", "✕ $0.10 denied 0", TraceKind::Wrong),
            ("result", "ok $0.10 denied 0", TraceKind::Own),
            ("session", "s · m", TraceKind::Own),
        ] {
            assert_eq!(TraceKind::of(word, arg), kind, "{word} {arg}");
        }
        let glyphs = [
            (TraceKind::Words, "›", bold(v)),
            (TraceKind::Tool, "▸", fg(p.info)),
            (TraceKind::Bus, "⇄", fg(p.accent)),
            (TraceKind::Note, "•", fg(p.warn)),
            (TraceKind::Wrong, "✕", fg(p.bad)),
            (TraceKind::Own, "·", dim()),
        ];
        for (kind, glyph, style) in glyphs {
            assert_eq!(kind.glyph(v), (glyph, style), "{kind:?}");
        }
    }

    #[test]
    fn tool_runs_and_the_bus_are_told_apart_in_the_trace() {
        let v = Color::Indexed(9);
        let p = pal();
        let code = fg(p.code);
        let one = |raw: &str| trace_lines(raw, v).remove(0);
        let text = |l: &Line<'static>| texts(std::slice::from_ref(l)).remove(0);
        let l = one("12:00:02 tool_use Edit src/a.rs b");
        assert_eq!(text(&l), "12:00:02 ▸ Edit src/a.rs b");
        assert_eq!(style_of(&l, "▸ "), Some(fg(p.info)));
        assert_eq!(style_of(&l, "src/a.rs b"), Some(code));
        let l = one("12:00:02 tool_use Read");
        assert_eq!(text(&l), "12:00:02 ▸ Read", "no target, no space");
        let l = one("12:00:02 tool_use WebFetch https://a.b/x");
        assert_eq!(style_of(&l, "https://a.b/x"), Some(link_style()));
        let l = one("12:00:02 tool_result Bash ✕");
        assert_eq!(text(&l), "12:00:02 ▸ Bash ✕");
        assert_eq!(style_of(&l, " ✕"), Some(fg(p.bad)));
        let l = one("12:00:03 run: cargo test: exit 0, 3.5 s, 10 bytes");
        assert_eq!(
            text(&l),
            "12:00:03 ▸ run cargo test: exit 0, 3.5 s, 10 bytes"
        );
        assert_eq!(colour_of(&l, ": exit 0, 3.5 s, 10 bytes"), Some(p.ok));
        assert_eq!(style_of(&l, "run "), Some(bold(p.info)));
        assert_eq!(style_of(&l, "cargo test"), Some(code));
        let l = one("12:00:03 run: cargo test: exit 0");
        assert_eq!(colour_of(&l, ": exit 0"), Some(p.ok));
        let l = one("12:00:03 run: cargo test: exit 10, 1 s, 0 bytes");
        assert_eq!(colour_of(&l, ": exit 10, 1 s, 0 bytes"), Some(p.bad));
        let l = one("12:00:03 run: cargo test: exit 101, 3.5 s, 10 bytes");
        assert_eq!(colour_of(&l, ": exit 101, 3.5 s, 10 bytes"), Some(p.bad));
        let l = one("12:00:03 run: cargo test");
        assert_eq!(text(&l), "12:00:03 ▸ run cargo test");
        assert_eq!(style_of(&l, "cargo test"), Some(code), "a start line too");
        let l = one("12:00:04 stopped \"tools differ\"");
        assert_eq!(text(&l), "12:00:04 ✕ stopped \"tools differ\"");
        assert_eq!(colour_of(&l, "stopped \"tools differ\""), Some(p.bad));
        let l = one("12:00:04 run refused: rm");
        assert_eq!(colour_of(&l, "run refused: rm"), Some(p.bad));
        for word in [
            "send", "mail", "woken", "hire", "hired", "post", "claim", "told:",
        ] {
            let l = one(&format!("12:00:05 {word} to=lead"));
            assert_eq!(text(&l), format!("12:00:05 ⇄ {word} to=lead"));
            assert_eq!(colour_of(&l, &format!("{word} ")), Some(p.accent), "{word}");
            assert_eq!(style_of(&l, "to=lead"), Some(Style::new()), "{word}");
        }
        let l = one("12:00:05 note api error: see https://status.x.com");
        assert_eq!(text(&l), "12:00:05 • api error: see https://status.x.com");
        assert_eq!(style_of(&l, "https://status.x.com"), Some(link_style()));
        let l = one("12:00:06 session hit its cap");
        assert_eq!(text(&l), "12:00:06 · session hit its cap");
        assert_eq!(style_of(&l, "session hit its cap"), Some(Style::new()));
        assert_eq!(text(&one("12:00:07")), "12:00:07 · ");
        assert_eq!(
            text(&one("12:00:08 text \"\u{1b}[2Jx\"")),
            "12:00:08 ›  [2Jx",
            "cleaned like everything drawn"
        );
    }

    #[test]
    fn the_story_draws_what_agents_wrote_as_markdown_and_its_own_lines_plain() {
        use crate::i18n::Lang;
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let c = app.world.cards[0].id;
        let me = app.world.cards[0].owner.unwrap_or(0);
        app.world.cards[c].story.clear();
        let card = Some(c);
        app.world.story(
            card,
            StoryEvent::Replied("All **green**, see https://a.b".into()),
        );
        let long: String = (1..=10).map(|i| format!("- step {i}\n")).collect();
        app.world
            .story(card, StoryEvent::Replied(format!("Done:\n{long}")));
        app.world
            .story(card, StoryEvent::Replied("- one item".into()));
        app.world
            .story(card, StoryEvent::Decision("use `serde`\nnot *json*".into()));
        app.world
            .story(card, StoryEvent::Touched("src/*a*.rs".into()));
        let lines = story_lines(&app, &app.world.cards[c], me);
        let all = texts(&lines);
        let when = clock(app.world.tick);
        let indent = " ".repeat(when.len() + 1);
        // One line: on the head's line, its marks drawn.
        assert!(
            all[0].ends_with(": All green, see https://a.b"),
            "{}",
            all[0]
        );
        assert!(
            style_of(&lines[0], "green")
                .unwrap()
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(style_of(&lines[0], "https://a.b"), Some(link_style()));
        // Lines more than the room: under the head, the rest counted.
        assert!(all[1].ends_with(":"), "{}", all[1]);
        assert_eq!(all[2], format!("{indent}Done:"));
        assert_eq!(all[3], format!("{indent}• step 1"));
        assert_eq!(all[9], format!("{indent}• step 7"));
        assert_eq!(all[10], format!("{indent}… +3"));
        assert_eq!(style_of(&lines[10], "… +3"), Some(dim()));
        // One line that is a list: under the head as well.
        assert!(all[11].ends_with(":"), "{}", all[11]);
        assert_eq!(all[12], format!("{indent}• one item"));
        // A decision: its lines joined, its marks drawn.
        assert!(all[13].ends_with("use serde not json"), "{}", all[13]);
        assert_eq!(style_of(&lines[13], "serde"), Some(fg(pal().code)));
        // imece's own line: as written, no marks read.
        assert!(all[14].ends_with("src/*a*.rs"), "{}", all[14]);
        assert_eq!(lines.len(), 15);
    }

    #[test]
    fn a_block_is_what_the_focus_draws_under_the_head() {
        for line in [
            "```rust", "  ```", "> quoted", "# Title", "- item", "* item", "2. two", "---", "***",
            "___",
        ] {
            assert!(is_block(line), "{line}");
        }
        for line in [
            "plain words",
            "#323 is a card",
            "2.5 is a number",
            "--",
            "**bold**",
        ] {
            assert!(!is_block(line), "{line}");
        }
    }

    #[test]
    fn capped_draws_up_to_the_room_and_counts_the_rest() {
        let lines = |n: usize| -> Vec<(Line<'static>, String)> {
            (0..n)
                .map(|i| (Line::from(i.to_string()), i.to_string()))
                .collect()
        };
        assert_eq!(capped(lines(STORY_ROWS), "  ").len(), STORY_ROWS);
        let out = capped(lines(STORY_ROWS + 1), "  ");
        assert_eq!(out.len(), STORY_ROWS + 1);
        assert_eq!(texts(&out[STORY_ROWS..]), ["  … +1"]);
        assert_eq!(STORY_ROWS, 8);
    }

    #[test]
    fn unquote_undoes_what_debug_wrote() {
        let text = "a\"b\\c\nd\te";
        assert_eq!(unquote(&format!("{text:?}")).as_deref(), Some(text));
        assert_eq!(
            unquote("\"\\u{1}\\\"").as_deref(),
            Some("\\u{1}\\"),
            "other escapes stay, a last backslash too"
        );
        assert_eq!(unquote("\"\""), Some(String::new()));
        assert_eq!(unquote("x"), None);
        assert_eq!(unquote("\"x"), None);
        assert_eq!(unquote("x\""), None);
    }
}
