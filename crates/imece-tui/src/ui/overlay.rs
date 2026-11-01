//! Windows over the screen: help, what happened while the person was away,
//! and a conversation read whole.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, Digest, LeftOff, PickStep, SoloPick, TalkNote, Thread};
use crate::welcome::{self, Seen};
use imece_agent::Login;

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn window(f: &mut Frame, title: String, lines: Vec<Line<'static>>, width: u16) {
    let area = centered(f.area(), width, lines.len() as u16 + 2);
    shade(f, area);
    clear(f, area);
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// `K`: agent `id`'s focus in a window over the screen, a little smaller
/// than it so the view under it, which does not move, shows around it.
pub fn peek(f: &mut Frame, app: &App, id: AgentId) {
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(6),
    );
    shade(f, area);
    clear(f, area);
    super::focus::draw(f, area, app, id);
}

/// The keys, two columns on a wide screen, then the legend. The line for
/// a new line in the writing names the key this terminal has for it
/// (`shift_enter`: it tells shift-enter from enter). Filtered (`/`), only
/// the keys and legend lines with every word of the filter stay, and the
/// window keeps its size, so it does not jump while the person types.
pub fn help(f: &mut Frame, app: &App) {
    let t = app.lang.text();
    let key = |(k, what): &(&str, &str)| {
        vec![
            Span::styled(format!(" {}", fit(k, 13)), bold(pal().strong)),
            Span::raw(fit(what, 44)),
        ]
    };
    // Two columns where the screen is wide enough, so the help fits a
    // short screen too.
    let wide = f.area().width >= 120;
    let rows = |n: usize| if wide { n.div_ceil(2) } else { n };
    let all = help_keys(app.lang, app.shift_enter);
    let keys = found(&all, &app.help_find, |k| format!("{} {}", k.0, k.1));
    let shown = rows(keys.len());
    let mut lines: Vec<Line> = (0..shown)
        .map(|r| {
            let mut spans = key(keys[r]);
            if wide && let Some(k) = keys.get(r + shown) {
                spans.extend(key(k));
            }
            Line::from(spans)
        })
        .collect();
    let legend = found(&t.help_legend, &app.help_find, |l| l.to_string());
    if keys.is_empty() && legend.is_empty() {
        lines.push(Line::from(Span::styled(t.help_none, dim())));
    }
    lines.resize(rows(all.len()), Line::raw(""));
    lines.push(Line::raw(""));
    lines.extend(legend.iter().map(|l| Line::from(Span::styled(**l, dim()))));
    lines.resize(
        lines.len() + t.help_legend.len() - legend.len(),
        Line::raw(""),
    );
    lines.push(Line::raw(""));
    lines.push(if app.help_typing || !app.help_find.is_empty() {
        let mut spans = vec![
            Span::styled(" /", fg(pal().accent)),
            Span::styled(app.help_find.clone(), bold(pal().strong)),
        ];
        if app.help_typing {
            spans.push(Span::styled("▏", fg(pal().accent)));
        }
        let hint = if app.help_typing {
            t.help_finding
        } else {
            t.help_found
        };
        spans.push(Span::styled(hint, dim()));
        Line::from(spans)
    } else {
        Line::from(Span::styled(t.help_close, dim()))
    });
    window(f, t.help_title.into(), lines, if wide { 118 } else { 60 });
}

/// The items whose `text` has every word of `filter`, case and Turkish
/// letters folded; all of them when the filter has no words.
pub(crate) fn found<'a, T>(
    items: &'a [T],
    filter: &str,
    text: impl Fn(&T) -> String,
) -> Vec<&'a T> {
    let words: Vec<String> = filter.split_whitespace().map(fold).collect();
    items
        .iter()
        .filter(|i| {
            let hay = fold(&text(i));
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

/// `s` in lower case with the Turkish letters as their plain Latin ones,
/// so `calis` finds `çalış` and `ISIM` finds `isim`.
pub(crate) fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let plain = match c {
                'ç' | 'Ç' => 'c',
                'ğ' | 'Ğ' => 'g',
                'ı' | 'İ' => 'i',
                'ö' | 'Ö' => 'o',
                'ş' | 'Ş' => 's',
                'ü' | 'Ü' => 'u',
                c => c,
            };
            plain.to_lowercase()
        })
        .collect()
}

/// How many of `help_keys` come before the help's new-line line: the
/// writing keys end there.
const WRITING_KEYS: usize = 19;

/// The keys the help lists, in its order: the new-line line, the one
/// this terminal has (`shift_enter`), after the writing keys.
pub(crate) fn help_keys(lang: Lang, shift_enter: bool) -> Vec<(&'static str, &'static str)> {
    let t = lang.text();
    let mut keys = t.help_keys.to_vec();
    keys.insert(WRITING_KEYS, t.help_newline[usize::from(!shift_enter)]);
    keys
}

/// The solo pick screen: the step's question, its rows with the chosen
/// one reversed, what was picked so far, and the keys.
pub fn solo_pick(f: &mut Frame, app: &App, p: &SoloPick) {
    let t = app.lang.text();
    let question = match p.step {
        PickStep::Vendor => t.solo_pick_vendor,
        PickStep::Model => t.solo_pick_model,
        PickStep::Effort => t.solo_pick_effort,
    };
    let mut lines = vec![Line::from(Span::raw(question)), Line::raw("")];
    for (n, item) in p.items().iter().enumerate() {
        let shown = if item == crate::complete::DEFAULT {
            format!("{item} ({})", t.solo_pick_default)
        } else {
            item.clone()
        };
        let style = match p.step {
            PickStep::Vendor => bold(vendor_color(p.vendors[n])),
            _ => bold(pal().strong),
        };
        let line = Line::from(vec![
            Span::styled(format!(" {} ", n + 1), dim()),
            Span::styled(shown, style),
        ]);
        lines.push(if n == p.sel() {
            line.style(Style::new().add_modifier(Modifier::REVERSED))
        } else {
            line
        });
    }
    lines.push(Line::raw(""));
    let so_far = match p.choice() {
        Some(c) => format!(" {} {c}", p.chosen_vendor().label()),
        None => format!(" {}", p.chosen_vendor().label()),
    };
    if p.step != PickStep::Vendor {
        lines.push(Line::from(Span::styled(so_far, dim())));
    }
    lines.push(Line::from(Span::styled(t.solo_pick_keys, dim())));
    window(f, t.solo_pick_title.into(), lines, 64);
}

pub fn digest(f: &mut Frame, app: &App, d: &Digest) {
    let t = app.lang.text();
    let counts = [
        ("✓", pal().ok, d.merged),
        ("!", pal().warn, d.waiting),
        ("✕", pal().bad, d.blocked),
        ("⚑", pal().warn, d.assumptions),
        ("⇄", pal().info, d.talks),
    ];
    let mut lines: Vec<Line> = counts
        .iter()
        .zip(t.away_counts)
        .map(|(&(icon, color, n), what)| {
            Line::from(vec![
                Span::styled(format!(" {icon} "), fg(color)),
                Span::raw(format!("{n} {what}")),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    for e in &d.highlights {
        let mut line = entry_line(app, e);
        line.spans.insert(0, Span::raw(" "));
        lines.push(line);
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.close, dim())));
    window(
        f,
        fill(t.away_title, &[&d.minutes.max(1).to_string()]),
        lines,
        84,
    );
}

/// Where the person left off, at the start of a run that remembers an
/// earlier one: the pot, each project in a line, what waits (the oldest
/// items as the inbox lists them), turns the earlier run cut short, and
/// its last journal lines, with a day row where they are not of today.
pub fn left_off(f: &mut Frame, app: &App, l: &LeftOff) {
    let t = app.lang.text();
    let w = &app.world;
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        format!(
            " {}",
            fill(t.left_spent, &[&dollars(l.spent), &dollars(l.budget)])
        ),
        dim(),
    ))];
    for (name, agents, cards) in &l.projects {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                fill(
                    t.left_project,
                    &[name, &agents.to_string(), &cards.to_string()],
                ),
                fg(pal().strong),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    if l.waiting.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" {}", t.nothing_needed),
            dim(),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled(" ! ", bold(pal().warn)),
            Span::styled(
                fill(t.left_waiting, &[&l.waiting_total.to_string()]),
                bold(pal().warn),
            ),
        ]));
        for &i in &l.waiting {
            let mut line = inbox_line(app, i);
            line.spans.insert(0, Span::raw("   "));
            lines.push(line);
        }
        let more = l.waiting_total.saturating_sub(l.waiting.len());
        if more > 0 {
            lines.push(Line::from(Span::styled(
                format!("   {}", fill(t.left_more, &[&more.to_string()])),
                dim(),
            )));
        }
    }
    if !l.interrupted.is_empty() {
        let (icon, style) = entry_icon(EntryKind::Failed);
        let names: Vec<&str> = l
            .interrupted
            .iter()
            .map(|&id| w.agents[id].name.as_str())
            .collect();
        lines.push(Line::from(vec![
            Span::styled(format!(" {icon} "), style),
            Span::raw(fill(t.left_interrupted, &[&names.join(", ")])),
        ]));
    }
    if !l.last.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(format!(" {}", t.left_last), dim())));
        let mut above = day(w.tick);
        for e in &l.last {
            let of = day(e.tick);
            if of != above {
                let mut row = date_row(of);
                row.spans.insert(0, Span::raw("   "));
                lines.push(row);
                above = of;
            }
            let mut line = entry_line(app, e);
            line.spans.insert(0, Span::raw("   "));
            lines.push(line);
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.close, dim())));
    let title = match l.ago {
        Some(ago) => fill(t.left_title_ago, &[&words::ago(app.lang, ago)]),
        None => t.left_title.to_string(),
    };
    window(f, title, lines, 84);
}

/// The first-run window (`:welcome`): what imece is, the coding agents on
/// this machine (version, signed in or how to sign in, `checking…` until
/// they answer), and the keys to start with.
pub fn welcome(f: &mut Frame, app: &App) {
    let t = app.lang.text();
    let mut lines: Vec<Line> = t
        .welcome_about
        .iter()
        .map(|l| Line::raw(format!(" {l}")))
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!(" {}", t.welcome_vendors),
        bold(pal().strong),
    )));
    for (v, seen) in &app.vendors {
        lines.push(vendor_line(app, *v, seen));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!(" {}", t.welcome_keys_title),
        bold(pal().strong),
    )));
    for (k, what) in &t.welcome_keys {
        lines.push(Line::from(vec![
            Span::styled(format!("   {}", fit(k, 16)), key_style()),
            Span::raw(*what),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.welcome_close, dim())));
    window(f, fill(t.welcome_title, &[welcome::VERSION]), lines, 84);
}

/// One vendor's line in the first-run window: a mark, its name, the
/// version it gave (cleaned: a CLI's words) and its sign-in.
fn vendor_line(app: &App, v: Vendor, seen: &Seen) -> Line<'static> {
    let t = app.lang.text();
    let (mark, style, version, what) = match seen {
        Seen::Checking => ("…", dim(), String::new(), t.seen_checking.to_string()),
        Seen::Missing => ("·", dim(), String::new(), t.seen_missing.to_string()),
        Seen::Here { version, login } => {
            let version = format!("{} · ", clean(version));
            match login {
                Login::Yes => ("✓", fg(pal().ok), version, t.login_yes.to_string()),
                Login::No => (
                    "✕",
                    bold(pal().warn),
                    version,
                    fill(t.login_no, &[welcome::sign_in(v)]),
                ),
                Login::Unknown => ("?", dim(), version, t.login_unknown.to_string()),
            }
        }
    };
    Line::from(vec![
        Span::styled(format!("   {mark} "), style),
        Span::styled(fit(v.label(), 8), bold(pal().strong)),
        Span::styled(version, dim()),
        Span::styled(what, style),
    ])
}

/// What is new (`:news`, and once at the first start of a new version):
/// each version's lines, newest first.
pub fn news(f: &mut Frame, app: &App) {
    let t = app.lang.text();
    let mut lines: Vec<Line> = Vec::new();
    for version in &app.news {
        let Some((_, said)) = t.news.iter().find(|(v, _)| v == version) else {
            continue;
        };
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(Span::styled(
            format!(" {version}"),
            bold(pal().strong),
        )));
        for l in *said {
            lines.push(Line::raw(format!("   · {l}")));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.news_close, dim())));
    window(f, fill(t.news_title, &[welcome::VERSION]), lines, 92);
}

/// What quitting now would leave behind (`q s`): the agents mid-turn, with
/// what each is doing, and the changes not reviewed, as the waiting panel
/// shows them.
pub fn leaving(f: &mut Frame, app: &App) {
    let t = app.lang.text();
    let w = &app.world;
    let heading = |words: &str| Line::from(Span::styled(format!(" {words}"), bold(pal().warn)));
    let mut lines = Vec::new();
    let busy = w.mid_turn();
    if !busy.is_empty() {
        lines.push(heading(t.leaving_mid_turn));
        for id in busy {
            let a = &w.agents[id];
            let mut spans = vec![
                Span::raw("   "),
                Span::styled(fit(&a.name, 16), bold(vendor_color(a.vendor))),
            ];
            if w.projects.len() > 1 {
                spans.push(Span::styled(fit(&w.projects[a.project].name, 12), dim()));
            }
            spans.push(Span::raw(" "));
            spans.extend(status_spans(app.lang, w, a));
            lines.push(Line::from(spans));
        }
    }
    let changes = w.open_changes();
    if !changes.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(heading(t.leaving_changes));
        for i in changes {
            let mut line = inbox_line(app, i);
            line.spans.insert(0, Span::raw("   "));
            lines.push(line);
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.leaving_keys, dim())));
    window(f, t.leaving_title.to_string(), lines, 84);
}

/// Where `scroll` must be for rows `start..end` to show in `height` rows,
/// moving as little as it can; a part longer than the screen shows from
/// its start.
pub(crate) fn in_view(start: usize, end: usize, scroll: usize, height: usize) -> usize {
    // No further down than `start`, no further up than what shows `end`.
    scroll.clamp(end.saturating_sub(height).min(start), start)
}

/// One line of a conversation as copy mode (`v`) moves over it: the
/// message it belongs to (an index into the conversation's entries) and
/// what copying it takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyLine {
    pub entry: usize,
    pub text: String,
    /// Its words as drawn ([`line_words`]): what the cursor steps over
    /// word by word, and what copying or noting part of it takes.
    pub words: Vec<String>,
}

/// The words of a drawn line: what stands between blanks, after the
/// spans that only lead it (indent, a list marker, a code rule, a quote
/// bar), so `**bold** text` is two words.
pub(crate) fn line_words(line: &Line) -> Vec<String> {
    let lead = line.spans.iter().take_while(|s| is_lead(s)).count();
    let text: String = (line.spans.iter().skip(lead))
        .map(|s| s.content.as_ref())
        .collect();
    text.split_whitespace().map(str::to_string).collect()
}

/// `line` with its words `from..=to` ([`line_words`]) drawn as chosen,
/// the blanks between two chosen words too, and word `cursor` (if any)
/// reversed. `to` may lie past the last word.
pub(crate) fn paint_words(
    line: Line<'static>,
    from: usize,
    to: usize,
    cursor: Option<usize>,
) -> Line<'static> {
    let lead = line.spans.iter().take_while(|s| is_lead(s)).count();
    let style = line.style;
    let mut spans: Vec<Span<'static>> = line.spans[..lead].to_vec();
    // The word the text is in or last passed; none before the first.
    let mut word: Option<usize> = None;
    let mut blank = true;
    for span in &line.spans[lead..] {
        let mut piece = String::new();
        let mut was: Option<Style> = None;
        for ch in span.content.chars() {
            let is_blank = ch.is_whitespace();
            if !is_blank && blank {
                word = Some(word.map_or(0, |w| w + 1));
            }
            blank = is_blank;
            let chosen = match word {
                // A blank is chosen between two chosen words.
                Some(w) if is_blank => from <= w && w < to,
                Some(w) => from <= w && w <= to,
                None => false,
            };
            let mut st = span.style;
            if chosen {
                st = st.patch(changed());
            }
            if !is_blank && word == cursor {
                st = st.patch(changed()).add_modifier(Modifier::REVERSED);
            }
            if was.is_some_and(|w| w != st) {
                let style = was.unwrap_or_default();
                spans.push(Span::styled(std::mem::take(&mut piece), style));
            }
            was = Some(st);
            piece.push(ch);
        }
        if let Some(st) = was {
            spans.push(Span::styled(piece, st));
        }
    }
    Line::from(spans).style(style)
}

/// `line` cut to `room` cells, `…` where it was cut (the blanks before
/// it dropped), and whether it was.
pub(crate) fn cut(line: Line<'static>, room: usize) -> (Line<'static>, bool) {
    if line.width() <= room {
        return (line, false);
    }
    let style = line.style;
    let mut left = room.saturating_sub(1);
    let mut spans = Vec::new();
    for span in line.spans {
        let mut kept = String::new();
        for ch in span.content.chars() {
            let w = ch.width().unwrap_or(0);
            if w > left {
                left = 0;
                break;
            }
            left -= w;
            kept.push(ch);
        }
        spans.push(Span::styled(kept, span.style));
        if left == 0 {
            break;
        }
    }
    if let Some(last) = spans.last_mut() {
        last.content = last.content.trim_end().to_string().into();
    }
    spans.push(Span::styled("…", dim()));
    (Line::from(spans).style(style), true)
}

/// Where the cursor goes from word `at.1` of line `at.0` one word on
/// (`on`) or back, over lines of `counts` words: past a line's last word
/// to the next line's first, before its first to the last of the line
/// before; nowhere past the first or the last line. A word past its
/// line's end counts as its last.
pub(crate) fn word_step(counts: &[usize], at: (usize, usize), on: bool) -> (usize, usize) {
    let (line, word) = at;
    let last = |l: usize| counts.get(l).map_or(0, |n| n.saturating_sub(1));
    let word = word.min(last(line));
    if on {
        if word < last(line) {
            (line, word + 1)
        } else if line + 1 < counts.len() {
            (line + 1, 0)
        } else {
            (line, word)
        }
    } else if word > 0 {
        (line, word - 1)
    } else if line > 0 {
        (line - 1, last(line - 1))
    } else {
        (line, word)
    }
}

/// What copying or noting the choice of conversation `th` takes, over
/// its `lines`: a line chosen whole as [`copy_lines`] reads it, part of
/// one as its words chosen, a blank line where one message ends and the
/// next begins.
pub(crate) fn chosen_text(lines: &[CopyLine], th: &Thread) -> String {
    let parts = lines.iter().enumerate().filter_map(|(i, l)| {
        let (from, to) = th.part(i)?;
        let last = l.words.len().saturating_sub(1);
        let text = if from == 0 && to >= last {
            l.text.clone()
        } else {
            let words = l.words.get(from..=to.min(last)).unwrap_or_default();
            words.join(" ")
        };
        Some((l.entry, text))
    });
    join_lines(parts)
}

/// The notes on words of a conversation as the message that takes them
/// to the agent: each note under the words it is about, quoted, and the
/// number of the message they are from when it has one. The words are
/// the screen's, cleaned already; the notes are the person's own, as a
/// message they write is.
pub(crate) fn talk_notes_message(w: &World, notes: &[&TalkNote]) -> String {
    let mut out =
        String::from("Notes on words from our conversation, each under the words it is about.");
    for n in notes {
        let id = (w.journal.get(n.entry))
            .and_then(|e| words::said(&e.event))
            .map_or(0, |s| s.id);
        out.push_str("\n\n");
        if id > 0 {
            out.push_str(&format!("On message {id}:\n"));
        }
        for line in n.quote.lines() {
            out.push_str(&format!("> {line}\n"));
        }
        out.push_str(n.text.trim());
    }
    out
}

/// What a drawn line reads as: its spans' text, trailing blanks dropped.
fn line_text(line: &Line) -> String {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    text.trim().to_string()
}

/// A message's lines before wrapping, each with what copying it takes:
/// its head (when, who to whom, the kind) and the line it answers as
/// read, its text as written ([`markdown_src`]).
fn message_lines(lang: Lang, w: &World, i: usize) -> Vec<(Line<'static>, String)> {
    let e = &w.journal[i];
    let mut head = Line::from(vec![Span::styled(format!("{} ", clock(e.tick)), dim())]);
    let mut rest = Vec::new();
    match words::said(&e.event) {
        Some(s) => {
            let mut said = said_spans(lang, w, &s);
            said.pop(); // the one-line text; it follows whole below
            head.spans.extend(said);
            rest.extend(re_row(lang, w, &s).map(|l| {
                let text = line_text(&l);
                (l, text)
            }));
            rest.extend(markdown_src(s.text, "  "));
        }
        None => head.spans.push(Span::raw(words::event(lang, w, &e.event))),
    }
    let text = line_text(&head);
    let mut lines = vec![(head, text)];
    lines.extend(rest);
    lines
}

/// The lines of a conversation copy mode moves over, in order.
pub(crate) fn thread_lines(lang: Lang, w: &World, entries: &[usize]) -> Vec<CopyLine> {
    let mut out = Vec::new();
    for (n, &i) in entries.iter().enumerate() {
        out.extend(
            message_lines(lang, w, i)
                .into_iter()
                .map(|(line, text)| CopyLine {
                    entry: n,
                    words: line_words(&line),
                    text,
                }),
        );
    }
    out
}

/// What copying lines `from..=to` of a conversation takes: their text,
/// a blank line where one message ends and the next begins (what
/// [`chosen_text`] takes for whole lines).
#[cfg(test)]
pub(crate) fn copy_lines(lines: &[CopyLine], from: usize, to: usize) -> String {
    let parts = (lines.iter().take(to + 1).skip(from)).map(|l| (l.entry, l.text.clone()));
    join_lines(parts)
}

/// Lines of a conversation, each with the message it is of, as one text:
/// a blank line where one message ends and the next begins.
fn join_lines(parts: impl Iterator<Item = (usize, String)>) -> String {
    let mut out = String::new();
    let mut entry = None;
    for (at, text) in parts {
        if entry.is_some_and(|e| e != at) {
            out.push('\n');
        }
        if entry.is_some() {
            out.push('\n');
        }
        out.push_str(&text);
        entry = Some(at);
    }
    out
}

/// The rows of a conversation `width` cells wide, oldest first: each
/// message is a head (when, who to whom, the kind) and its text as
/// Markdown, wrapped, then a blank row; a day row stands before the first
/// message of each day that is not the day of the message above it (today,
/// for the first). The selected message carries a mark in the gutter on
/// every row and the selection background. Returns the rows and each
/// message's row range (the day row and the blank row not included).
#[cfg(test)]
pub(crate) fn thread_rows(
    lang: Lang,
    w: &World,
    th: &Thread,
    width: u16,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let (rows, ranges, _) = thread_layout(lang, w, &[], th, width);
    (rows, ranges)
}

/// [`thread_rows`], and each line's row range as well (a line wraps to
/// several). While lines are being chosen to copy (`Thread::copy`), the
/// chosen ones carry the mark and the background instead of the selected
/// message, the cursor's with `▶`; chosen word by word, the words chosen
/// carry the background, the cursor's word reversed, and only a line
/// chosen whole the row's. A folded message (`Thread::folds`; none while
/// choosing) is one row: its head and its text on one line, cut with `…`
/// to fit, and, when opening it shows more, how many lines it has (its
/// lines have no row ranges: they are only asked for while choosing). Under
/// an open message, a row per note on its words (`✎ "words" note`); a
/// folded one counts them.
#[allow(clippy::type_complexity)]
pub(crate) fn thread_layout(
    lang: Lang,
    w: &World,
    talk_notes: &[TalkNote],
    th: &Thread,
    width: u16,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>, Vec<(usize, usize)>) {
    const GUTTER: usize = 2;
    let t = lang.text();
    let text_width = usize::from(width).saturating_sub(GUTTER);
    let mut rows: Vec<Line> = Vec::new();
    let mut ranges = Vec::new();
    let mut line_ranges = Vec::new();
    let mut above = day(w.tick);
    for (n, &i) in th.entries.iter().enumerate() {
        let e = &w.journal[i];
        let of = day(e.tick);
        if of != above {
            let mut row = date_row(of);
            row.spans.insert(0, Span::raw("  "));
            rows.push(row);
            above = of;
        }
        let start = rows.len();
        // The mark, when the line has one, in the gutter of its first row.
        let mut mark = w.marks.contains(&i);
        let gutter = |row: &mut Line<'static>, mark: &mut bool, bar: Option<&'static str>| {
            let second = match std::mem::take(mark) {
                true => Span::styled(MARK, bold(pal().accent)),
                false => Span::raw(" "),
            };
            let first = match bar {
                Some(bar) => Span::styled(bar, fg(pal().accent)),
                None => Span::raw(" "),
            };
            row.spans.insert(0, first);
            row.spans.insert(1, second);
        };
        let lines = message_lines(lang, w, i);
        let notes: Vec<&TalkNote> = talk_notes.iter().filter(|n| n.entry == i).collect();
        let said = words::said(&e.event);
        if th.copy.is_none()
            && th.folds.folded(n)
            && let Some(s) = &said
        {
            let mut row = Line::from(Span::styled(format!("{} ", clock(e.tick)), dim()));
            row.spans.extend(said_spans(lang, w, s));
            let body = lines.len() - 1;
            let noted = match notes.len() {
                0 => String::new(),
                k => format!(" ✎ {k}"),
            };
            let more = match body {
                1 => format!(" {}", t.folded_line),
                _ => format!(" {}", fill(t.folded_lines, &[&body.to_string()])),
            };
            // More to see: more lines than one, or the one cut to fit.
            let fits = row.width() + noted.width() <= text_width;
            let tail = match body > 1 || (body == 1 && !fits) {
                true => format!("{more}{noted}"),
                false => noted,
            };
            let (mut row, _) = cut(row, text_width.saturating_sub(tail.width()));
            row.spans.push(Span::styled(tail, dim()));
            let selected = n == th.sel;
            gutter(&mut row, &mut mark, selected.then_some("▌"));
            if selected {
                row = row.style(changed());
            }
            rows.push(row);
            ranges.push((start, rows.len()));
            rows.push(Line::raw(""));
            continue;
        }
        for (line, _) in lines {
            let at = line_ranges.len();
            let part = th.part(at);
            let (selected, cursor) = match th.copy {
                Some(c) => (part.is_some(), c.at == at),
                None => (n == th.sel, false),
            };
            // Chosen word by word, the row's background only when whole.
            let (line, whole) = match (th.words, part) {
                (Some((_, cw)), Some((from, to))) => {
                    let last = line_words(&line).len().saturating_sub(1);
                    let whole = from == 0 && to >= last;
                    let painted = paint_words(line, from, to, cursor.then_some(cw));
                    (painted, whole)
                }
                _ => (line, true),
            };
            let line_start = rows.len();
            for mut row in wrap(line, text_width) {
                let bar = selected.then_some(if cursor { "▶" } else { "▌" });
                gutter(&mut row, &mut mark, bar);
                if selected && whole {
                    row = row.style(changed());
                }
                rows.push(row);
            }
            line_ranges.push((line_start, rows.len()));
        }
        for note in notes {
            let quote = brief(&note.quote, usize::MAX);
            let row = Line::from(vec![
                Span::raw("    "),
                Span::styled("✎ ", fg(pal().accent)),
                Span::styled(format!("\"{quote}\" "), dim()),
                Span::raw(clean(&note.text)),
            ]);
            let (mut row, _) = cut(row, text_width);
            gutter(&mut row, &mut false, None);
            rows.push(row);
        }
        ranges.push((start, rows.len()));
        rows.push(Line::raw(""));
    }
    (rows, ranges, line_ranges)
}

/// A conversation read whole, oldest first; the selected message is
/// marked on every row and kept in view.
pub fn thread(f: &mut Frame, app: &App, th: &Thread) {
    let w = &app.world;
    let t = app.lang.text();
    let screen = f.area();
    let area = centered(
        screen,
        screen.width.saturating_sub(4).min(120),
        screen.height.saturating_sub(2),
    );
    shade(f, area);
    clear(f, area);
    let first = th.entries.first().map(|&i| &w.journal[i].event);
    let title = match first.and_then(words::said) {
        Some(s) => {
            let name = |p: Party| match p {
                Party::Agent(id) => w.agents[id].name.clone(),
                Party::Person => t.you.to_string(),
            };
            let (a, b) = s.pair();
            fill(
                t.thread_title,
                &[
                    &name(a),
                    &name(b),
                    &(th.sel + 1).to_string(),
                    &th.entries.len().to_string(),
                ],
            )
        }
        None => t.event_title.to_string(),
    };
    // Notes on its words not sent yet: counted after the title.
    let title = match app.talk_notes_here().len() {
        0 => title,
        n => format!("{title}✎ {n} "),
    };
    // Choosing lines to copy: the title counts them, the keys are the
    // choosing's.
    let (title, hints) = match th.copy {
        Some(c) => (
            fill(t.copy_title, &[&c.len().to_string()]),
            &t.choose_hints[..],
        ),
        None => (title, &t.thread_hints[..]),
    };
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(Line::from(hint_spans(hints))), keys);

    let (rows, ranges, lines) = thread_layout(app.lang, w, &app.talk_notes, th, body.width);
    let height = usize::from(body.height);
    let max = rows.len().saturating_sub(height);
    // The cursor's line is kept in view while copying, else the message.
    let shown = match th.copy {
        Some(c) => lines.get(c.at),
        None => ranges.get(th.sel),
    };
    if th.follow.take()
        && let Some(&(start, end)) = shown
    {
        let at = in_view(start, end, usize::from(th.scroll.get()), height);
        th.scroll.set(u16::try_from(at).unwrap_or(u16::MAX));
    }
    th.scroll
        .set(th.scroll.get().min(u16::try_from(max).unwrap_or(u16::MAX)));
    th.page.set(body.height.saturating_sub(1).max(1));
    let skip = usize::from(th.scroll.get());
    for (row, at) in rows.iter().skip(skip).zip(body.rows()) {
        f.render_widget(row, at);
    }
}

#[cfg(test)]
mod tests {
    use super::in_view;

    #[test]
    fn in_view_moves_the_scroll_as_little_as_it_can() {
        // Already shown: nothing moves.
        assert_eq!(in_view(5, 8, 3, 10), 3);
        assert_eq!(in_view(3, 8, 3, 10), 3, "starting on the first row counts");
        assert_eq!(in_view(5, 13, 3, 10), 3, "ending on the last row too");
        // Above: up to its start.
        assert_eq!(in_view(2, 4, 3, 10), 2);
        // Below: just far enough for its end.
        assert_eq!(in_view(5, 14, 3, 10), 4);
        // Longer than the screen: its start.
        assert_eq!(in_view(5, 16, 3, 10), 5);
        assert_eq!(in_view(5, 15, 3, 10), 5, "exactly the screen fits");
        assert_eq!(in_view(0, 0, 0, 0), 0);
    }
}
