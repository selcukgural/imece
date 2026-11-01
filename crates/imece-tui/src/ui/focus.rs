//! One agent up close: its card's story (intent, decisions, assumptions,
//! conversations, evidence) and, one level deeper, its raw stream.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

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
    tail(f, body, story);
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

/// Card `c`'s story, a line per event, oldest first; conversation lines
/// are told as agent `me` (the card's owner) had them.
pub(super) fn story_lines(app: &App, c: &Card, me: AgentId) -> Vec<Line<'static>> {
    let t = app.lang.text();
    c.story
        .iter()
        .map(|s| {
            let (icon, label, color) = story_icon(s.event.kind());
            let mut spans = vec![
                Span::styled(format!("{} ", clock(s.tick)), dim()),
                Span::styled(
                    format!("{icon} {} ", fit(t.story_labels[label], 10)),
                    fg(color),
                ),
            ];
            match words::said_in_story(me, &s.event) {
                Some(said) => spans.extend(said_spans(app.lang, &app.world, &said)),
                None => spans.push(Span::raw(words::story(app.lang, &app.world, c, &s.event))),
            }
            Line::from(spans)
        })
        .collect()
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
    tail(f, inner, lines);
}

/// A trace line drawn so the kinds tell apart at a glance: the clock dim;
/// the agent's own words (`text "…"`, the line the person must not miss)
/// in its vendor's colour, unquoted, one row per line of them, with the
/// inline marks of Markdown; `tool_use NAME TARGET` with the tool's name
/// in the info colour; `run:` lines with their end coloured by the exit
/// code; `stopped` in the colour of what is wrong; the bus (`send`,
/// `mail`, `woken`, `hire`, `hired`, `post`, `claim`) with its first word
/// in the accent. Anything else as written. The vocabulary is the trace
/// producers' in `live/`, read back here by the first word; a line that
/// matches none is still shown whole. A line that runs over rows (a
/// command with a heredoc) keeps them: the first [`TRACE_ROWS`] under it,
/// indented, then how many more.
pub(crate) fn trace_lines(raw: &str, vendor: Color) -> Vec<Line<'static>> {
    let mut rows = raw.lines();
    let mut out = trace_row(rows.next().unwrap_or_default(), vendor);
    let more: Vec<&str> = rows.collect();
    let indent = || Span::raw(" ".repeat(9));
    for row in more.iter().take(TRACE_ROWS) {
        out.push(Line::from(vec![indent(), Span::raw(clean(row))]));
    }
    if more.len() > TRACE_ROWS {
        let left = format!("… +{}", more.len() - TRACE_ROWS);
        out.push(Line::from(vec![indent(), Span::styled(left, dim())]));
    }
    out
}

/// How many rows of a trace line are drawn after its first.
const TRACE_ROWS: usize = 6;

/// The first row of a trace line, as [`trace_lines`] tells.
fn trace_row(raw: &str, vendor: Color) -> Vec<Line<'static>> {
    let line = clean(raw);
    let (clock, rest) = line.split_once(' ').unwrap_or((&line, ""));
    let clock = Span::styled(format!("{clock} "), dim());
    let (word, arg) = rest.split_once(' ').unwrap_or((rest, ""));
    let mut spans = vec![clock];
    match word {
        "text" => {
            let text = unquote(arg).unwrap_or_else(|| arg.to_string());
            let base = fg(vendor);
            let mut rows = text.lines();
            spans.extend(inline(rows.next().unwrap_or_default(), base));
            let mut out = vec![Line::from(spans)];
            for row in rows {
                let mut more = vec![Span::raw(" ".repeat(9))];
                more.extend(inline(row, base));
                out.push(Line::from(more));
            }
            return out;
        }
        "tool_use" => {
            let (name, target) = arg.split_once(' ').unwrap_or((arg, ""));
            spans.push(Span::styled("tool_use ", dim()));
            spans.push(Span::styled(name.to_string(), fg(pal().info)));
            spans.push(Span::raw(format!(" {target}")));
        }
        "run:" => {
            spans.push(Span::styled("run: ", fg(pal().info)));
            match arg.rsplit_once(": exit ") {
                Some((cmd, end)) => {
                    let ok = end.starts_with("0,") || end == "0";
                    let colour = if ok { pal().ok } else { pal().bad };
                    spans.push(Span::raw(cmd.to_string()));
                    spans.push(Span::styled(format!(": exit {end}"), fg(colour)));
                }
                None => spans.push(Span::raw(arg.to_string())),
            }
        }
        "stopped" | "run" => {
            spans.push(Span::styled(rest.to_string(), fg(pal().bad)));
        }
        "send" | "mail" | "woken" | "hire" | "hired" | "post" | "claim" => {
            spans.push(Span::styled(format!("{word} "), fg(pal().accent)));
            spans.push(Span::raw(arg.to_string()));
        }
        _ => spans.push(Span::raw(rest.to_string())),
    }
    vec![Line::from(spans)]
}

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

    #[test]
    fn the_agents_own_words_in_the_trace_are_unquoted_and_in_its_colour() {
        let v = Color::Indexed(9);
        let lines = trace_lines(
            "12:00:01 text \"Four pass.\\n**All** green \\\"now\\\"\"",
            v,
        );
        assert_eq!(
            texts(&lines),
            ["12:00:01 Four pass.", "         All green \"now\""]
        );
        assert_eq!(lines[0].spans[0].style, dim());
        assert_eq!(colour_of(&lines[0], "Four pass."), Some(v));
        let all = lines[1].spans.iter().find(|s| s.content == "All").unwrap();
        assert_eq!(all.style.fg, Some(v));
        assert!(all.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(
            texts(&trace_lines("12:00:01 text not quoted", v)),
            ["12:00:01 not quoted"],
            "a text that is not one quoted string shows as written"
        );
    }

    #[test]
    fn a_command_over_rows_keeps_them_up_to_a_few_and_counts_the_rest() {
        let v = Color::Indexed(9);
        let body = |n: usize| (1..=n).map(|i| format!("\nline {i}")).collect::<String>();
        let six = format!("12:00:02 tool_use Bash cat > a.txt <<EOF{}", body(6));
        assert_eq!(
            texts(&trace_lines(&six, v)),
            [
                "12:00:02 tool_use Bash cat > a.txt <<EOF",
                "         line 1",
                "         line 2",
                "         line 3",
                "         line 4",
                "         line 5",
                "         line 6",
            ]
        );
        let nine = format!("12:00:02 tool_use Bash cat > a.txt <<EOF{}\n", body(9));
        let lines = trace_lines(&nine, v);
        assert_eq!(lines.len(), 8, "{:?}", texts(&lines));
        assert_eq!(texts(&lines[6..]), ["         line 6", "         … +3"]);
        assert_eq!(colour_of(&lines[7], "… +3"), Some(pal().dim));
        assert_eq!(colour_of(&lines[0], "Bash"), Some(pal().info));
        // Each row is cleaned on its own.
        assert_eq!(
            texts(&trace_lines(
                "12:00:02 tool_use Bash x\nTOKEN=abc \u{1b}[2J",
                v
            ))[1],
            "         [***]  [2J"
        );
    }

    #[test]
    fn tool_runs_and_the_bus_are_told_apart_in_the_trace() {
        let v = Color::Indexed(9);
        let p = pal();
        let one = |raw: &str| trace_lines(raw, v).remove(0);
        let l = one("12:00:02 tool_use Edit src/a.rs b");
        assert_eq!(
            texts(std::slice::from_ref(&l)),
            ["12:00:02 tool_use Edit src/a.rs b"]
        );
        assert_eq!(colour_of(&l, "Edit"), Some(p.info));
        assert_eq!(colour_of(&l, "tool_use "), Some(p.dim));
        let l = one("12:00:03 run: cargo test: exit 0, 3.5 s, 10 bytes");
        assert_eq!(colour_of(&l, ": exit 0, 3.5 s, 10 bytes"), Some(p.ok));
        assert_eq!(colour_of(&l, "run: "), Some(p.info));
        let l = one("12:00:03 run: cargo test: exit 101, 3.5 s, 10 bytes");
        assert_eq!(colour_of(&l, ": exit 101, 3.5 s, 10 bytes"), Some(p.bad));
        let l = one("12:00:03 run: cargo test");
        assert_eq!(
            texts(std::slice::from_ref(&l)),
            ["12:00:03 run: cargo test"]
        );
        assert_eq!(colour_of(&l, "cargo test"), None, "a start line is plain");
        let l = one("12:00:04 stopped \"tools differ\"");
        assert_eq!(colour_of(&l, "stopped \"tools differ\""), Some(p.bad));
        let l = one("12:00:04 run refused: rm");
        assert_eq!(colour_of(&l, "run refused: rm"), Some(p.bad));
        for word in ["send", "mail", "woken", "hire", "hired", "post", "claim"] {
            let l = one(&format!("12:00:05 {word} to=lead"));
            assert_eq!(colour_of(&l, &format!("{word} ")), Some(p.accent), "{word}");
            assert_eq!(colour_of(&l, "to=lead"), None, "{word}");
        }
        let l = one("12:00:06 session hit its cap");
        assert_eq!(
            texts(std::slice::from_ref(&l)),
            ["12:00:06 session hit its cap"]
        );
        assert_eq!(colour_of(&l, "session hit its cap"), None);
        assert_eq!(texts(&[one("12:00:07")]), ["12:00:07 "]);
        assert_eq!(
            texts(&[one("12:00:08 text \"\u{1b}[2Jx\"")]),
            ["12:00:08  [2Jx"],
            "cleaned like everything drawn"
        );
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
