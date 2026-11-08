//! Windows over the screen: help, what happened while the person was away,
//! and a conversation read whole.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use super::*;
use crate::app::{
    App, Digest, LeftOff, LoncaWin, PickStep, SoloPick, TalkNote, TeardownAsk, Thread, TidyWin,
    Window,
};
use crate::archives::{self, Browser, Row};
use crate::keys::{Keymap, Scope};
use crate::model::ImportAsk;
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

fn window(f: &mut Frame, title: String, lines: Vec<Line<'static>>, width: u16) -> Rect {
    let area = centered(f.area(), width, lines.len() as u16 + 2);
    shade(f, area);
    clear(f, area);
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    f.render_widget(Paragraph::new(lines).block(block), area);
    area
}

/// `K`: agent `id`'s focus in a window over the screen, a little smaller
/// than it so the view under it, which does not move, shows around it.
pub fn peek(f: &mut Frame, app: &App, id: AgentId) -> Rect {
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(6),
    );
    shade(f, area);
    clear(f, area);
    super::focus::draw(f, area, app, id, false);
    area
}

/// A worktree setup script's ask (record 92): what runs and how, its
/// file, then its bytes as read and hashed, a numbered line each (each
/// cleaned, in the code colour; never read as Markdown), from line `at`
/// on, and the keys. What the person reads here is what runs.
pub fn script(f: &mut Frame, app: &App, item: usize, at: usize) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let agent = &app.world.agents[app.world.inbox[item].agent];
    let Some(s) = app.script_text(item) else {
        let block = Block::bordered().border_style(fg(pal().accent));
        let gone = Line::from(Span::styled(t.setup_gone, dim()));
        f.render_widget(Paragraph::new(gone).block(block), area);
        return area;
    };
    let sha: String = s.sha.chars().take(16).collect();
    let check = s.kind == crate::live::setup::Kind::Check;
    let title = fill(
        if check { t.check_title } else { t.setup_title },
        &[&agent.name, &sha],
    );
    let shown = Shown {
        title,
        head: if check { t.check_head } else { t.setup_head },
        file: t.setup_file,
        hints: &t.setup_hints,
    };
    script_body(f, area, shown, &s, at);
    area
}

/// The window that asks before a checkout goes into Lonca (record 117):
/// where it comes from, what it becomes, what can not be taken back.
pub fn import(f: &mut Frame, app: &App, ask: &ImportAsk) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(full, full.width.saturating_sub(8).min(72), 13);
    shade(f, area);
    clear(f, area);
    let block = Block::bordered()
        .title(t.import_title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let checkout = ask.checkout.display().to_string();
    let lines = vec![
        Line::from(fill(t.import_from, &[&checkout])),
        Line::from(fill(
            t.import_branch,
            &[&ask.branch, words::short_commit(&ask.head)],
        )),
        Line::from(fill(t.import_into, &[&ask.repo])),
        Line::default(),
        Line::from(Span::styled(t.import_note, dim())),
        Line::from(Span::styled(t.import_critical, dim())),
        Line::from(Span::styled(t.import_final, fg(pal().warn))),
        Line::default(),
        Line::from(super::hint_spans(&t.import_hints)),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    area
}

/// A teardown script a `:tidy` removal asks about (record 96), in a
/// window over the screen as a setup script is: what runs and how, its
/// file, its bytes numbered, scrolled to `ask.scroll`.
pub fn teardown(f: &mut Frame, app: &App, ask: &TeardownAsk) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let s = &ask.script;
    let sha: String = s.sha.chars().take(16).collect();
    // The checkout's directory, the script's file in it.
    let checkout = (s.real.parent().and_then(std::path::Path::parent))
        .and_then(std::path::Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = fill(t.teardown_title, &[&checkout, &sha]);
    let shown = Shown {
        title,
        head: t.teardown_head,
        file: t.setup_file,
        hints: &t.teardown_hints,
    };
    script_body(f, area, shown, s, ask.scroll);
    area
}

/// The words of a script's window: its title, what runs and how, how
/// its file is named, its keys.
struct Shown<'a> {
    title: String,
    head: &'a str,
    file: &'a str,
    hints: &'a [(&'a str, &'a str)],
}

/// A script's window in `area`: its words, the script's file and its
/// bytes numbered from line `at` (the rest cut at the foot).
fn script_body(f: &mut Frame, area: Rect, shown: Shown, s: &crate::live::setup::Script, at: usize) {
    let block = Block::bordered()
        .title(shown.title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = vec![
        Line::from(Span::styled(shown.head.to_string(), fg(pal().warn))),
        Line::from(Span::styled(
            fill(shown.file, &[&s.real.display().to_string()]),
            dim(),
        )),
        Line::default(),
    ];
    let text = s.text();
    let numbers = text.lines().count().to_string().len();
    // As many as the window holds: the rest is cut at its foot.
    for (n, line) in text.lines().enumerate().skip(at) {
        lines.push(Line::from(vec![
            Span::styled(format!("{:>numbers$} ", n + 1), dim()),
            Span::styled(clean(line), fg(pal().code)),
        ]));
    }
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(lines), body);
    f.render_widget(Paragraph::new(Line::from(hint_spans(shown.hints))), keys);
}

/// The leader's plan (record 114) as the person reads it before saying
/// yes: what it means and the pot beside the expected cost, then each
/// card with its number, title and tier, the paths it owns, and its
/// body, from line `at` on; the keys at the foot. Every word is the
/// leader's, cleaned, never read as Markdown.
pub fn plan(f: &mut Frame, app: &App, item: usize, at: usize) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let w = &app.world;
    let leader = &w.agents[w.inbox[item].agent];
    let Some(p) = w.projects[leader.project].plan.proposal.as_ref() else {
        let block = Block::bordered().border_style(fg(pal().accent));
        let gone = Line::from(Span::styled(t.plan_gone, dim()));
        f.render_widget(Paragraph::new(gone).block(block), area);
        return area;
    };
    let title = fill(
        t.plan_win_title,
        &[&leader.name, &p.cards.len().to_string(), &dollars(p.cost)],
    );
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let left = w.budget.saturating_sub(w.spent());
    let mut lines = vec![
        Line::from(Span::styled(t.plan_head.to_string(), fg(pal().warn))),
        Line::from(Span::styled(
            fill(t.plan_pot, &[&dollars(p.cost), &dollars(left)]),
            dim(),
        )),
        Line::default(),
    ];
    lines.extend(plan_lines(app, p).into_iter().skip(at));
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(lines), body);
    f.render_widget(Paragraph::new(Line::from(hint_spans(&t.plan_hints))), keys);
    area
}

/// The cards of plan `p`, as the window lists them: a line with number,
/// title and tier, one with the paths it owns when it has any, one with
/// the checks it asks for (record 127), its body
/// line by line, and a blank line after each card. What `j k` scroll.
pub fn plan_lines(app: &App, p: &Proposal) -> Vec<Line<'static>> {
    let t = app.lang.text();
    let mut lines = Vec::new();
    for (n, c) in p.cards.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            fill(
                t.plan_card,
                &[&(n + 1).to_string(), &clean(&c.title), &clean(&c.tier)],
            ),
            bold(pal().strong),
        )));
        if !c.paths.is_empty() {
            lines.push(Line::from(Span::styled(
                fill(t.plan_owns, &[&clean(&crate::paths::listed(&c.paths))]),
                dim(),
            )));
        }
        if !c.checks.is_empty() {
            lines.push(Line::from(Span::styled(
                fill(t.plan_checks, &[&clean(&crate::checks::listed(&c.checks))]),
                dim(),
            )));
        }
        if !c.after.is_empty() {
            let places: Vec<String> = c.after.iter().map(usize::to_string).collect();
            lines.push(Line::from(Span::styled(
                fill(t.plan_after, &[&places.join(", ")]),
                dim(),
            )));
        }
        for line in c.body.lines() {
            lines.push(Line::raw(format!("   {}", clean(line))));
        }
        lines.push(Line::default());
    }
    lines
}

/// The `:tidy` window (record 96): the worktrees no agent owns, a row
/// each (its name, its checkout, what it holds), the one at hand marked;
/// while a look or a removal is under way, a line says so. At its foot
/// its keys, or the `y` it asks for removing one that holds work.
pub fn tidy(f: &mut Frame, app: &App, win: TidyWin) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let o = &app.world.orphans;
    let found = o.found.as_deref().unwrap_or_default();
    let block = Block::bordered()
        .title(fill(t.tidy_title, &[&found.len().to_string()]))
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = vec![
        Line::from(Span::styled(t.tidy_head, dim())),
        Line::default(),
    ];
    if o.busy {
        let busy = if o.found.is_some() {
            t.tidy_removing
        } else {
            t.tidy_looking
        };
        lines.push(Line::from(Span::styled(busy, fg(pal().warn))));
    } else if found.is_empty() {
        lines.push(Line::from(Span::styled(t.tidy_empty, dim())));
    }
    let width = found
        .iter()
        .map(|o| o.name.chars().count())
        .max()
        .unwrap_or(0);
    for (i, orphan) in found.iter().enumerate() {
        let at = i == win.at;
        let held = words::held(app.lang, orphan);
        let style = if orphan.clean() {
            fg(pal().ok)
        } else {
            fg(pal().warn)
        };
        let mark = if at { "▶ " } else { "  " };
        let name = clean(&orphan.name);
        let name = if at {
            Span::styled(format!("{name:<width$}"), bold(pal().fg))
        } else {
            Span::raw(format!("{name:<width$}"))
        };
        lines.push(Line::from(vec![
            Span::raw(mark),
            name,
            Span::styled(
                format!("  {}  ", clean(&orphan.checkout.display().to_string())),
                dim(),
            ),
            Span::styled(held, style),
        ]));
    }
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    // The row at hand stays on the window: the rows above it scroll away.
    let room = usize::from(body.height).max(1);
    let row = 2 + usize::from(o.busy || found.is_empty()) + win.at;
    let skip = (row + 1).saturating_sub(room);
    f.render_widget(Paragraph::new(lines).scroll((skip as u16, 0)), body);
    let foot = match found.get(win.at) {
        Some(orphan) if win.sure => Line::from(Span::styled(
            fill(t.tidy_sure, &[&orphan.name, &words::held(app.lang, orphan)]),
            fg(pal().warn),
        )),
        _ => Line::from(hint_spans(&t.tidy_hints)),
    };
    f.render_widget(Paragraph::new(foot), keys);
    area
}

/// A view in a window over the screen (record 125, step 5): the board
/// under a title that counts its cards, the code map or the timeline as
/// they drew themselves, each on the shaded screen.
pub fn view(f: &mut Frame, app: &App, win: Window) -> Rect {
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    match win {
        Window::Board => {
            let t = app.lang.text();
            let w = &app.world;
            let cards = w.shown_cards().count();
            // The ready filter says what it hides and how to undo it
            // (record 126, F10).
            let filter = match app.board_ready {
                true => {
                    let hidden = super::board::hidden(w);
                    fill(t.board_ready, &[&hidden.to_string()])
                }
                false => t.board_all.to_string(),
            };
            let block = Block::bordered()
                .title(format!(
                    "{}{filter}",
                    fill(t.board_title, &[&cards.to_string()])
                ))
                .border_style(fg(pal().accent));
            let inner = block.inner(area);
            f.render_widget(block, area);
            super::board::draw(f, inner, app);
        }
        Window::Code => super::code::draw(f, area, app),
        Window::Timeline => super::timeline::draw(f, area, app),
        Window::Journal => super::salon::journal_pane(f, area, app),
    }
    area
}

/// The `:lonca` window (record 125, step 4): how the project at hand's
/// repository stands, then every proposal waiting for a human in it, a
/// row each (its number, who made it: the agent's name here when it is
/// one of the project's, the paths it changes, `Δ shown` when it is the
/// change waiting for the person, `based before trunk`), the one at hand
/// marked, then what is held (file, symbol, holder). At its foot its
/// keys.
pub fn lonca(f: &mut Frame, app: &App, win: LoncaWin) -> Rect {
    let t = app.lang.text();
    let full = f.area();
    let area = centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let w = &app.world;
    let p = app.current_project();
    let repo = (w.projects[p].lonca.as_ref()).map_or(String::new(), |on| clean(&on.repo));
    let block = Block::bordered()
        .title(fill(t.lonca_title, &[&repo]))
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = vec![
        Line::from(Span::styled(app.lonca_state(p), dim())),
        Line::default(),
    ];
    let rows = app.lonca_rows();
    let head = fill(t.lonca_pending_head, &[&rows.len().to_string()]);
    lines.push(Line::from(Span::styled(head, bold(pal().fg))));
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(t.lonca_pending_none, dim())));
    }
    let name_of = |row: &crate::model::StorePending| match row.holder {
        Some(id) => w.agents[id].name.clone(),
        None => clean(&row.name),
    };
    let width = rows
        .iter()
        .map(|r| name_of(r).chars().count())
        .max()
        .unwrap_or(0);
    for (i, row) in rows.iter().enumerate() {
        let at = i == win.at;
        let mark = if at { "▶ " } else { "  " };
        let name = name_of(row);
        let name = if at {
            Span::styled(format!("{name:<width$}"), bold(pal().fg))
        } else {
            Span::raw(format!("{name:<width$}"))
        };
        let mut spans = vec![
            Span::raw(mark),
            Span::styled(format!("{:>4}  ", row.seq), dim()),
            name,
            Span::styled(format!("  {}", diff::named(&row.paths)), dim()),
        ];
        if app.lonca_row_shown(row).is_some() {
            spans.push(Span::styled(format!("  {}", t.lonca_shown), fg(pal().ok)));
        }
        if !row.on_trunk {
            spans.push(Span::styled(
                format!("  {}", t.lonca_before_trunk),
                fg(pal().warn),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    let leases = (w.projects[p].lonca.as_ref()).map_or(&[][..], |on| &on.leases[..]);
    let head = fill(t.lonca_leases_head, &[&leases.len().to_string()]);
    lines.push(Line::from(Span::styled(head, bold(pal().fg))));
    if leases.is_empty() {
        lines.push(Line::from(Span::styled(t.lonca_leases_none, dim())));
    }
    for lease in leases {
        let holder = match lease.holder {
            Some(id) => w.agents[id].name.clone(),
            None => clean(&lease.name),
        };
        lines.push(Line::from(vec![
            Span::raw(format!("  {}", clean(&lease.file))),
            Span::styled(format!(" · {}", clean(&lease.symbol)), dim()),
            Span::styled(format!(" · {holder}"), fg(pal().agent)),
        ]));
    }
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    // The row at hand stays on the window: the rows above it scroll away.
    let room = usize::from(body.height).max(1);
    let row = 3 + win.at.min(rows.len().saturating_sub(1));
    let skip = (row + 1).saturating_sub(room);
    f.render_widget(Paragraph::new(lines).scroll((skip as u16, 0)), body);
    f.render_widget(Paragraph::new(Line::from(hint_spans(&t.lonca_hints))), keys);
    area
}

/// The keys, two columns on a wide screen, then the legend. The line for
/// a new line in the writing names the key this terminal has for it
/// (`shift_enter`: it tells shift-enter from enter). Filtered (`/`), only
/// the keys and legend lines with every word of the filter stay, and the
/// window keeps its size, so it does not jump while the person types.
/// On a screen too short for it, its lines scroll (`App::help_scroll`)
/// above the last line, which says how many are below. While the filter
/// is typed, where the cursor goes: after it.
pub fn help(f: &mut Frame, app: &App) -> Option<Position> {
    let t = app.lang.text();
    // Two columns where the screen is wide enough, so the help fits a
    // short screen too.
    let wide = f.area().width >= 120;
    let place = app.scope_below();
    let scoped = app.help_scoped() && place.is_some();
    let all = help_rows(app.lang, app.shift_enter, &app.keys, place, scoped);
    let keys = found(&all, &app.help_find, |k| format!("{} {}", k.0, k.1));
    let widths = help_widths(f.area().width, wide);
    let mut lines = help_columns(&keys, wide, widths);
    let legend = found(&t.help_legend, &app.help_find, |l| l.to_string());
    if keys.is_empty() && legend.is_empty() {
        lines.push(Line::from(Span::styled(t.help_none, dim())));
    }
    // Filtered, the window keeps the height of the whole list.
    let whole = help_columns(&all.iter().collect::<Vec<_>>(), wide, widths).len();
    lines.resize(lines.len().max(whole), Line::raw(""));
    lines.push(Line::raw(""));
    lines.extend(legend.iter().map(|l| Line::from(Span::styled(**l, dim()))));
    lines.resize(
        lines.len() + t.help_legend.len() - legend.len(),
        Line::raw(""),
    );
    lines.push(Line::raw(""));
    // The frame and the last line take three rows; what does not fit the
    // rest scrolls, and the last line says how much is below.
    let room = usize::from(f.area().height).saturating_sub(3).max(1);
    let last = lines.len().saturating_sub(room);
    app.help_room.set((room, last));
    let from = app.help_scroll.min(last);
    let below = lines.len().saturating_sub(from + room);
    let mut lines: Vec<Line> = lines.into_iter().skip(from).take(room).collect();
    let more = if below > 0 {
        fill(t.help_more, &[&below.to_string()])
    } else if from > 0 {
        t.help_top.to_string()
    } else {
        String::new()
    };
    let mut caret = None;
    let mut foot = if app.help_typing || !app.help_find.is_empty() {
        let mut spans = vec![
            Span::styled(" /", fg(pal().accent)),
            Span::styled(clean(&app.help_find), bold(pal().strong)),
        ];
        let hint = if app.help_typing {
            // The cursor after the filter, on the hint's first blank.
            caret = Some(Line::from(spans.clone()).width());
            t.help_finding
        } else {
            t.help_found
        };
        spans.push(Span::styled(hint, dim()));
        Line::from(spans)
    } else {
        Line::from(Span::styled(t.help_close, dim()))
    };
    if !more.is_empty() {
        caret = caret.map(|c| c + more.width());
        foot.spans.insert(0, Span::styled(more, fg(pal().accent)));
    }
    lines.push(foot);
    let row = lines.len() - 1;
    let columns = if wide { 2 } else { 1 };
    let width = columns * (2 + widths.0 + widths.1) + 2;
    let title = match place {
        None => t.help_title,
        Some(_) if scoped => t.help_here,
        Some(_) => t.help_every,
    };
    let area = window(
        f,
        title.into(),
        lines,
        u16::try_from(width).unwrap_or(u16::MAX),
    );
    caret_in(area.inner(Margin::new(1, 1)), row, caret?)
}

/// The narrowest and the widest key column of the help: a key longer than
/// the column (`/cmd  @name /cmd`, 16) takes a line of its own above its
/// text; with room to spare the column grows to the longest.
pub(crate) const HELP_KEY: (usize, usize) = (13, 16);

/// The narrowest and the widest text column of the help: 44 in the narrow
/// window (60) and in the wide one on a screen of 120; wider with the
/// screen until the longest text (74, `:copy YOL`) is on one line.
pub(crate) const HELP_TEXT: (usize, usize) = (44, 74);

/// The help's key and text columns on a screen `width` wide, in two
/// columns (`wide`) or one. A column is a blank, the key, a blank and the
/// text; two of them leave a cell to spare on either side and the frame.
/// The text gets the room first, the key what is left past its widest.
pub(crate) fn help_widths(width: u16, wide: bool) -> (usize, usize) {
    if !wide {
        return (HELP_KEY.0, HELP_TEXT.0);
    }
    let column = (usize::from(width).saturating_sub(4) / 2).saturating_sub(2);
    let key = column
        .saturating_sub(HELP_TEXT.1)
        .clamp(HELP_KEY.0, HELP_KEY.1);
    let text = column.saturating_sub(key).clamp(HELP_TEXT.0, HELP_TEXT.1);
    (key, text)
}

/// The lines of the help's `rows` in columns `(key, text)` wide: each
/// row its key and its text, the text wrapped onto lines of its own under
/// it rather than cut. `wide`, in two columns, the first taking the rows
/// until it is at least as long as the second, so they end together.
pub(crate) fn help_columns(
    rows: &[&(String, String)],
    wide: bool,
    widths: (usize, usize),
) -> Vec<Line<'static>> {
    let cells: Vec<Vec<Vec<Span<'static>>>> = rows.iter().map(|r| help_cell(r, widths)).collect();
    if !wide {
        return cells.into_iter().flatten().map(Line::from).collect();
    }
    let total: usize = cells.iter().map(Vec::len).sum();
    let mut left = 0;
    let split = cells
        .iter()
        .position(|c| {
            let before = left;
            left += c.len();
            before * 2 >= total
        })
        .unwrap_or(cells.len());
    let (first, second) = cells.split_at(split);
    let second: Vec<_> = second.iter().flatten().collect();
    first
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, l)| {
            let mut spans = l.clone();
            if let Some(r) = second.get(i) {
                spans.extend(r.iter().cloned());
            }
            Line::from(spans)
        })
        .collect()
}

/// One row of the help as its lines, each `2 + key + text` wide: the key
/// before the text's first line (on a line of its own when it is longer
/// than `key`), the text wrapped by words.
fn help_cell((k, what): &(String, String), (key, text): (usize, usize)) -> Vec<Vec<Span<'static>>> {
    let name = clean(k);
    let alone = name.chars().count() > key;
    let line = |name: &str, words: &str| {
        vec![
            Span::styled(format!(" {name:<key$} "), bold(pal().strong)),
            Span::raw(format!("{words:<text$}")),
        ]
    };
    let mut lines = Vec::new();
    if alone {
        let rest = (1 + key + text).saturating_sub(name.chars().count());
        lines.push(vec![
            Span::styled(format!(" {name}"), bold(pal().strong)),
            Span::raw(" ".repeat(rest)),
        ]);
    }
    for (i, words) in wrap_words(&clean(what), text).iter().enumerate() {
        lines.push(line(if i == 0 && !alone { &name } else { "" }, words));
    }
    lines
}

/// `s` in lines of at most `width` characters, broken between words; a
/// word longer than a line is cut where the line ends. Always one line,
/// an empty one for an empty `s`.
pub(crate) fn wrap_words(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![String::new()];
    let words: Vec<Vec<char>> = s.split_whitespace().map(|w| w.chars().collect()).collect();
    for piece in words.iter().flat_map(|w| w.chunks(width)) {
        let line = lines.last_mut().expect("one line at least");
        let used = line.chars().count();
        if used == 0 {
            line.extend(piece);
        } else if used + 1 + piece.len() <= width {
            line.push(' ');
            line.extend(piece);
        } else {
            lines.push(piece.iter().collect());
        }
    }
    lines
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
const WRITING_KEYS: usize = 18;

/// The rows of `help_keys` (with the new-line line) whose keys are
/// written, or none of imece's: the person's key file does not turn
/// them, so they are shown as they are.
pub(crate) const TYPED: [usize; 5] = [4, 15, 16, 17, WRITING_KEYS];

/// Where each row of `help_keys` (without the new-line line) belongs: the
/// places whose help lists it, `&[]` for every place. `?` opens the help
/// filtered to the screen it was pressed on (record 125, step 5): the
/// rows of that place and of every place; `?` again lists them all.
const HELP_PLACES: [&[Scope]; 57] = {
    use Scope::{Board, Change, Choose, Journal, Peek, Salon, Talk, Timeline};
    const ALL: &[Scope] = &[];
    [
        ALL,                        // 0  /  i
        ALL,                        // 1  @name text
        ALL,                        // 2  /cmd
        ALL,                        // 3  :
        ALL,                        // 4  $
        ALL,                        // 5  :budget
        ALL,                        // 6  :open :solo
        ALL,                        // 7  :close
        ALL,                        // 8  :rename
        ALL,                        // 9  :forget
        &[Salon, Talk],             // 10 :export
        ALL,                        // 11 :commit
        ALL,                        // 12 :copy
        &[Salon],                   // 13 :pin
        ALL,                        // 14 !cmd O ^x^e
        ALL,                        // 15 tab ↑ ↓ ^r
        ALL,                        // 16 ^a ^e ⌥b ⌥f
        ALL,                        // 17 ^w ^u ^k ^y
        &[Salon],                   // 18 #N #word n p
        ALL,                        // 19 & :goto
        ALL,                        // 20 ↑ ↓ j k
        ALL,                        // 21 gg G gd gw
        ALL,                        // 22 ^d ^u ^f ^b
        ALL,                        // 23 ^o
        &[Salon, Talk, Change],     // 24 ← → h l
        &[Salon, Talk, Change],     // 25 za zM zR
        &[Salon],                   // 26 s :settle
        &[Salon, Peek],             // 27 enter K
        &[Salon, Talk, Change],     // 28 tab
        &[Salon, Peek, Talk],       // 29 t :trace
        &[Salon, Peek],             // 30 v :talk
        ALL,                        // 31 esc u U
        ALL,                        // 32 A H :recall
        &[Salon, Change, Timeline], // 33 [ ]
        ALL,                        // 34 :waiting
        ALL,                        // 35 W S
        &[Talk, Journal],           // 36 enter %
        &[Talk, Journal],           // 37 c y C Y
        &[Talk, Change, Choose],    // 38 v w b s
        &[Salon, Talk, Journal],    // 39 m ' :mark
        &[Salon, Journal],          // 40 J :journal
        &[Salon, Change],           // 41 a A e r :allow
        &[Change, Talk],            // 42 [ ] file / turn
        &[Change],                  // 43 >
        &[Change, Choose],          // 44 c n
        &[Change],                  // 45 x b
        &[Change],                  // 46 w R
        &[Board],                   // 47 r
        ALL,                        // 48 space
        ALL,                        // 49 + -
        ALL,                        // 50 L :lang
        ALL,                        // 51 T :theme
        ALL,                        // 52 :awake :notify
        ALL,                        // 53 :motion :away
        ALL,                        // 54 q q :quit
        ALL,                        // 55 q s :pending
        ALL,                        // 56 ? :help
    ]
};

/// Whether row `i` of the help's list (with the new-line line in it) is
/// shown with the help filtered to `place`: the new-line line and the
/// rows of every place are, the rest where `HELP_PLACES` names the place.
pub(crate) fn help_shown(i: usize, place: Scope) -> bool {
    let j = match i.cmp(&WRITING_KEYS) {
        std::cmp::Ordering::Less => i,
        std::cmp::Ordering::Equal => return true,
        std::cmp::Ordering::Greater => i - 1,
    };
    let places = HELP_PLACES[j];
    places.is_empty() || places.contains(&place)
}

/// The help's rows with the person's own keys: one for each key the key
/// file gives first (those of a place say where), then `help_keys` with
/// every key the file moved written as the keys that do it now at
/// `place`, where the help was opened (`Keymap::shown`). `scoped`, only
/// the rows of that place (`help_shown`) and the key file's lines of it.
pub(crate) fn help_rows(
    lang: Lang,
    shift_enter: bool,
    keys: &Keymap,
    place: Option<Scope>,
    scoped: bool,
) -> Vec<(String, String)> {
    let t = lang.text();
    let only = place.filter(|_| scoped);
    let everywhere =
        (keys.given().iter()).map(|(from, to)| (from.name(), fill(t.keys_as, &[&to.name()])));
    let placed = (keys.placed().iter())
        .filter(|(p, ..)| only.is_none_or(|o| *p == o))
        .map(|(p, from, to)| {
            let said = fill(t.keys_as_in, &[p.words(lang), &to.name()]);
            (from.name(), said)
        });
    let keys = keys.at(place);
    let mine = everywhere.chain(placed);
    let rows = help_keys(lang, shift_enter)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| only.is_none_or(|o| help_shown(*i, o)))
        .map(|(i, (k, what))| {
            let k = if TYPED.contains(&i) {
                k.to_string()
            } else {
                keys.shown(k)
            };
            (k, what.to_string())
        });
    mine.chain(rows).collect()
}

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
        let line = Line::from(vec![
            Span::styled(format!(" {} ", n + 1), dim()),
            Span::styled(shown, bold(pal().strong)),
        ]);
        lines.push(if n == p.sel() {
            line.style(Style::new().add_modifier(Modifier::REVERSED))
        } else {
            line
        });
    }
    lines.push(Line::raw(""));
    let so_far = match p.choice() {
        Some(c) => format!(" {CLAUDE} {c}"),
        None => format!(" {CLAUDE}"),
    };
    lines.push(Line::from(Span::styled(so_far, dim())));
    lines.push(Line::from(Span::styled(t.solo_pick_keys, dim())));
    window(f, t.solo_pick_title.into(), lines, 64);
}

/// The archives browser: the filter while there is one, the rows that fit
/// with the selected one reversed and in sight, and the keys. While the
/// filter is typed, where the cursor goes: after it.
pub fn archives(f: &mut Frame, app: &App, b: &Browser) -> Option<Position> {
    let t = app.lang.text();
    let w = &app.world;
    let mut lines: Vec<Line> = Vec::new();
    let mut caret = None;
    if b.typing || !b.find.is_empty() {
        let line = Line::from(vec![
            Span::styled(" /", fg(pal().accent)),
            Span::styled(clean(&b.find), bold(pal().strong)),
        ]);
        if b.typing {
            caret = Some(line.width());
        }
        lines.push(line);
    }
    lines.push(Line::raw(""));
    let rows = b.rows(w);
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(t.help_none, dim())));
    }
    // The frame, the lines around the rows and the keys take their part
    // of the screen; the rows scroll in the rest.
    let room = usize::from(f.area().height)
        .saturating_sub(lines.len() + 4)
        .max(1);
    let from = (b.sel + 1).saturating_sub(room);
    for (n, &row) in rows.iter().enumerate().skip(from).take(room) {
        let line = archive_row(app, b, row);
        lines.push(if n == b.sel {
            line.style(Style::new().add_modifier(Modifier::REVERSED))
        } else {
            line
        });
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.archives_keys, dim())));
    let area = window(f, t.archives_title.into(), lines, 96);
    caret_in(area.inner(Margin::new(1, 1)), 0, caret?)
}

/// One row of the archives browser: a checkout and how many archives it
/// has; a project open on it now; an archive with its cost, cards done,
/// agents and day; an agent with what it spent.
fn archive_row(app: &App, b: &Browser, row: Row) -> Line<'static> {
    let t = app.lang.text();
    let w = &app.world;
    let mark = |open: bool| if open { "▾" } else { "▸" };
    let agents = |p: ProjectId| archives::agents_of(w, p).count().to_string();
    match row {
        Row::Checkout(i) => {
            let g = &b.groups[i];
            Line::from(vec![
                Span::styled(format!(" {} ", mark(!b.folded.contains(&i))), dim()),
                Span::styled(clean(&g.checkout), bold(pal().strong)),
                Span::styled(
                    format!(
                        " · {}",
                        fill(t.archives_count, &[&g.archives.len().to_string()])
                    ),
                    dim(),
                ),
            ])
        }
        Row::Open(p) => Line::from(vec![
            Span::styled("   ● ", fg(pal().ok)),
            Span::styled(clean(&w.projects[p].name), fg(pal().strong)),
            Span::styled(
                format!(" · {}", fill(t.archives_open, &[&agents(p)])),
                dim(),
            ),
        ]),
        Row::Archive(p) => {
            let cost: u64 = (w.agents.iter().filter(|a| a.project == p))
                .map(|a| a.cost)
                .sum();
            let done = (w.cards.iter())
                .filter(|c| c.project == p && c.column == Column::Done)
                .count();
            let at = w.projects[p].archived.as_ref().map_or(0, |a| a.at);
            let what = fill(
                t.archives_row,
                &[
                    &dollars(cost),
                    &done.to_string(),
                    &agents(p),
                    &date(day(at)),
                ],
            );
            Line::from(vec![
                Span::styled(format!("   {} ", mark(b.unfolded.contains(&p))), dim()),
                Span::styled(clean(archives::title(w, p)), bold(pal().strong)),
                Span::styled(format!(" · {what}"), dim()),
            ])
        }
        Row::Agent(id) => {
            let a = &w.agents[id];
            let lead = matches!(a.role, Role::Leader | Role::Solo);
            let indent = if a.role == Role::Helper {
                "         "
            } else {
                "       "
            };
            let name = if lead {
                bold(pal().strong)
            } else {
                fg(pal().strong)
            };
            Line::from(vec![
                Span::raw(indent),
                Span::styled(clean(&a.name), name),
                Span::styled(format!(" · {}", dollars(a.cost)), dim()),
            ])
        }
    }
}

pub fn digest(f: &mut Frame, app: &App, d: &Digest) {
    let t = app.lang.text();
    let counts = [
        ("✓", pal().ok, d.merged),
        ("!", pal().warn, d.waiting),
        ("■", pal().bad, d.blocked),
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
    let mut lines: Vec<Line> = (t.welcome_about.iter())
        .map(|l| Line::raw(format!(" {l}")))
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!(" {}", t.welcome_claude),
        bold(pal().strong),
    )));
    lines.push(seen_line(app));
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
    let lines = branded(f, app, 84, lines);
    window(f, fill(t.welcome_title, &[welcome::VERSION]), lines, 84);
}

/// A window's `lines` under imece's mark and words, a blank row above and
/// below them, when the screen has the rows for all of it; else the
/// lines alone. The window is `width` wide, borders inside.
fn branded(f: &Frame, app: &App, width: u16, lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let full = f.area();
    if lines.len() + super::brand::ROWS + 2 + 2 > usize::from(full.height) {
        return lines;
    }
    let inside = usize::from(width.min(full.width).saturating_sub(2));
    let mut out = vec![Line::raw("")];
    out.extend(super::brand::lines(app, inside, 2));
    out.push(Line::raw(""));
    out.extend(lines);
    out
}

/// Claude Code's line in the first-run window: a mark, its name, the
/// version it gave (cleaned: a CLI's words) and its sign-in.
fn seen_line(app: &App) -> Line<'static> {
    let t = app.lang.text();
    let (mark, style, version, what) = match &app.seen {
        Seen::Checking => (
            app.spin().unwrap_or("…"),
            dim(),
            String::new(),
            t.seen_checking.to_string(),
        ),
        Seen::Missing => ("·", dim(), String::new(), t.seen_missing.to_string()),
        Seen::Here { version, login } => {
            let version = format!("{} · ", clean(version));
            match login {
                Login::Yes => ("✓", fg(pal().ok), version, t.login_yes.to_string()),
                Login::No => (
                    "✗",
                    bold(pal().warn),
                    version,
                    fill(t.login_no, &[welcome::SIGN_IN]),
                ),
                Login::Unknown => ("?", dim(), version, t.login_unknown.to_string()),
            }
        }
    };
    Line::from(vec![
        Span::styled(format!("   {mark} "), style),
        Span::styled(fit(CLAUDE, 8), bold(pal().strong)),
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
    let lines = branded(f, app, 92, lines);
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
                Span::styled(fit(&a.name, 16), bold(pal().agent)),
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

/// A message's lines before wrapping, each with what copying it takes:
/// its head (when, who to whom, the kind) and the line it answers as
/// read, its text as written ([`markdown_src`]), `room` cells wide.
fn message_lines(lang: Lang, w: &World, i: usize, room: usize) -> Vec<(Line<'static>, String)> {
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
            rest.extend(markdown_src(s.text, "  ", room));
        }
        None => head.spans.push(Span::raw(words::event(lang, w, &e.event))),
    }
    let text = line_text(&head);
    let mut lines = vec![(head, text)];
    lines.extend(rest);
    lines
}

/// The lines of a conversation copy mode moves over, in order, `room`
/// cells wide as it was drawn (`Thread::room`).
pub(crate) fn thread_lines(lang: Lang, w: &World, entries: &[usize], room: usize) -> Vec<CopyLine> {
    let mut out = Vec::new();
    for (n, &i) in entries.iter().enumerate() {
        out.extend(
            message_lines(lang, w, i, room)
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

/// The cells left of a conversation's text: its mark and the bar.
const GUTTER: usize = 2;

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
        let lines = message_lines(lang, w, i, text_width);
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
pub fn thread(f: &mut Frame, app: &App, th: &Thread, within: Rect) -> Rect {
    let w = &app.world;
    let t = app.lang.text();
    let area = centered(
        within,
        within.width.saturating_sub(4).min(120),
        within.height,
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
    th.room.set(usize::from(body.width).saturating_sub(GUTTER));
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
    area
}

#[cfg(test)]
mod tests {
    use super::{help_columns, help_rows, help_widths, in_view, wrap_words};
    use crate::i18n::Lang;
    use ratatui::text::Line;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn row(k: &str, what: &str) -> (String, String) {
        (k.to_string(), what.to_string())
    }

    #[test]
    fn words_wrap_between_words_and_a_long_one_is_cut_where_the_line_ends() {
        assert_eq!(wrap_words("", 5), [""], "always a line");
        assert_eq!(wrap_words("  ", 5), [""]);
        assert_eq!(wrap_words("a bb ccc", 4), ["a bb", "ccc"]);
        assert_eq!(wrap_words("ab cd", 5), ["ab cd"], "a line just full");
        assert_eq!(wrap_words("ab cd", 4), ["ab", "cd"]);
        assert_eq!(wrap_words("a  b\tc", 9), ["a b c"], "one blank between");
        assert_eq!(wrap_words("abcdefgh", 3), ["abc", "def", "gh"]);
        assert_eq!(wrap_words("x abcdef", 3), ["x", "abc", "def"]);
        assert_eq!(
            wrap_words("çğı şöü", 3),
            ["çğı", "şöü"],
            "letters, not bytes"
        );
        assert_eq!(wrap_words("ab", 0), ["a", "b"], "a line holds one at least");
    }

    #[test]
    fn the_help_s_columns_grow_with_the_screen_the_text_first() {
        assert_eq!(help_widths(80, false), (13, 44), "one column");
        assert_eq!(
            help_widths(300, false),
            (13, 44),
            "one column whatever the width"
        );
        assert_eq!(help_widths(120, true), (13, 44));
        assert_eq!(help_widths(140, true), (13, 53));
        assert_eq!(help_widths(178, true), (13, 72));
        assert_eq!(
            help_widths(184, true),
            (14, 74),
            "the text whole, the key next"
        );
        assert_eq!(help_widths(188, true), (16, 74));
        assert_eq!(
            help_widths(240, true),
            (16, 74),
            "no wider than the longest"
        );
    }

    #[test]
    fn a_help_row_wraps_its_text_and_a_long_key_has_a_line_of_its_own() {
        let widths = (13, 10);
        let lines = |r: &(String, String)| -> Vec<String> {
            help_columns(&[r], false, widths).iter().map(text).collect()
        };
        assert_eq!(
            lines(&row("gg  G", "first / last line")),
            [" gg  G         first /   ", "               last line "]
        );
        // Thirteen is a key's whole column; fourteen is past it.
        assert_eq!(
            lines(&row(":forget @NAME", "forgets")),
            [" :forget @NAME forgets   "]
        );
        assert_eq!(
            lines(&row(":forget @NAMES", "forgets")),
            [" :forget @NAMES          ", "               forgets   "]
        );
    }

    #[test]
    fn two_columns_end_together_the_first_never_the_shorter() {
        let widths = (2, 3);
        let one = |k: &str| row(k, "one");
        let two = |k: &str| row(k, "two two");
        let columns = |rows: &[(String, String)]| -> Vec<String> {
            let rows: Vec<_> = rows.iter().collect();
            help_columns(&rows, true, widths).iter().map(text).collect()
        };
        // Four of one line: two and two, side by side.
        let four = [one("a"), one("b"), one("c"), one("d")];
        assert_eq!(columns(&four), [" a  one c  one", " b  one d  one"]);
        // Six: three and three.
        let six = [one("a"), one("b"), one("c"), one("d"), one("e"), one("f")];
        let got = columns(&six);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], " a  one d  one");
        // One of two lines first: five lines, three on the left.
        let rows = [two("a"), one("b"), one("c"), one("d")];
        let got = columns(&rows);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], " a  two c  one");
        assert_eq!(got[1], "    two d  one");
        assert_eq!(got[2], " b  one");
        assert!(columns(&[]).is_empty());
        assert_eq!(columns(&[one("a")]), [" a  one"]);
    }

    #[test]
    fn every_help_row_is_drawn_whole_in_both_languages_and_on_every_screen() {
        let keys = crate::keys::Keymap::default();
        for lang in [Lang::En, Lang::Tr] {
            for shift_enter in [false, true] {
                let rows = help_rows(lang, shift_enter, &keys, None, false);
                for (width, wide) in [(80, false), (120, true), (140, true), (188, true)] {
                    let widths = help_widths(width, wide);
                    for r in &rows {
                        let lines = help_columns(&[r], false, widths);
                        let drawn: Vec<String> = lines.iter().map(text).collect();
                        for l in &drawn {
                            assert_eq!(
                                l.chars().count(),
                                2 + widths.0 + widths.1,
                                "{lang:?} {width}: every line as wide: {l:?}"
                            );
                        }
                        let words = drawn.join(" ");
                        let words: Vec<&str> = words.split_whitespace().collect();
                        let whole = format!("{} {}", r.0, r.1);
                        let want: Vec<&str> = whole.split_whitespace().collect();
                        assert_eq!(words, want, "{lang:?} {width}: {r:?}");
                    }
                }
                // Wide enough, every row is one line.
                let widths = help_widths(188, true);
                for r in &rows {
                    assert_eq!(help_columns(&[r], false, widths).len(), 1, "{r:?}");
                }
            }
        }
    }

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
