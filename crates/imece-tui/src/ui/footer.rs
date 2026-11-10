//! The footer: the completion menu's row, the line being written and its
//! cursor, the status line and its keys.

use super::*;

/// The completion menu's row: the items with their notes, the highlighted
/// one reversed, in a window that always holds it. When not all fit,
/// `‹ N` before says how many are out of view before the window and
/// `+N more ›` after how many after it, so the person knows `tab` has
/// more to show.
pub(super) fn completions(f: &mut Frame, area: Rect, m: &crate::complete::Menu, lang: Lang) {
    let width = area.width as usize;
    // An item with its note (a command's value, `$`), a space either side.
    let cells: Vec<String> = (m.items.iter().enumerate())
        .map(|(i, item)| match m.notes.get(i).filter(|n| !n.is_empty()) {
            Some(note) => format!(" {} {} ", clean(item), clean(note)),
            None => format!(" {} ", clean(item)),
        })
        .collect();
    let cell = |i: usize| cells[i].chars().count();
    let n = cells.len();
    let before = |k: usize| format!(" ‹ {k} ");
    let after = |k: usize| fill(lang.text().menu_more, &[&k.to_string()]);
    let marks = |first: usize, last: usize| {
        let b = if first > 0 {
            before(first).chars().count()
        } else {
            0
        };
        let a = if last < n {
            after(n - last).chars().count()
        } else {
            0
        };
        a + b
    };
    let sum = |from: usize, to: usize| (from..to).map(cell).sum::<usize>();
    // All when they fit; else the window starts at the highlighted item,
    // is pulled back while earlier items still fit, then filled on, the
    // marks for what stays out counted in.
    // Ranges searched, never a loop a changed operator could make endless.
    let fits = |first: usize, last: usize| sum(first, last) + marks(first, last) <= width;
    let (mut first, mut last) = (0, n);
    if sum(0, n) > width {
        let end = m.chosen + 1;
        first = (0..m.chosen)
            .rev()
            .take_while(|&f| fits(f, end))
            .last()
            .unwrap_or(m.chosen);
        last = (end + 1..=n)
            .take_while(|&l| fits(first, l))
            .last()
            .unwrap_or(end);
    }
    let mut spans = Vec::new();
    if first > 0 {
        spans.push(Span::styled(before(first), bold(pal().info)));
    }
    for (i, text) in cells.iter().enumerate().take(last).skip(first) {
        let style = if i == m.chosen {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            dim()
        };
        spans.push(Span::styled(text.clone(), style));
    }
    if last < n {
        spans.push(Span::styled(after(n - last), bold(pal().info)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The line being written as it is drawn, and the byte of it the cursor
/// stands before. A line `clean` would change (a pasted token) is drawn
/// cleaned whole with the cursor at its end, so moving into it does not
/// split the token past hiding.
pub(super) fn caret_line(input: &str, at: usize) -> (String, usize) {
    let whole = clean(input);
    let before = if whole == input { at } else { whole.len() };
    (whole, before)
}

/// `text` cut into rows of at most `room` cells, cell by cell (a wide
/// character that does not fit goes on to the next row), with the row and
/// column the cursor stands at when it is before byte `caret`. A cursor
/// at the end of a full row stands at the start of the next, which is
/// there for it; an empty text is one empty row.
pub(super) fn wrap_cells(
    text: &str,
    room: usize,
    caret: Option<usize>,
) -> (Vec<String>, (usize, usize)) {
    let room = room.max(1);
    let mut rows = vec![String::new()];
    let (mut used, mut at) = (0, (0, 0));
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > room && used > 0 {
            rows.push(String::new());
            used = 0;
        }
        if caret == Some(i) {
            at = (rows.len() - 1, used);
        }
        rows.last_mut().expect("a row").push(c);
        used += w;
    }
    if caret.is_some_and(|c| c >= text.len()) {
        if used >= room {
            rows.push(String::new());
            used = 0;
        }
        at = (rows.len() - 1, used);
    }
    (rows, at)
}

/// Who the text being written goes to, before its first row; the rows
/// after it start under it.
pub(super) fn writing_to(app: &App) -> Span<'static> {
    let t = app.lang.text();
    // A command for the person's own shell stands out from a message or
    // a `:` command: its lead on the theme's shell colour.
    if app.noting.is_none() && !app.searching && app.typing_shell() {
        return Span::styled(t.typing_shell, shell());
    }
    Span::styled(
        if let Some(Noting::Line(id, line)) = app.noting {
            let patch = app.world.change_of(id).map(|c| c.patch.as_str());
            let place = patch.and_then(|p| place(p, line)).unwrap_or_default();
            fill(t.typing_note, &[&place])
        } else if let Some(Noting::Words(note)) = &app.noting {
            fill(t.typing_words_note, &[&brief(&note.quote, NOTE_QUOTE)])
        } else if app.searching && app.recalling {
            t.typing_recall.to_string()
        } else if app.searching {
            t.typing_history.to_string()
        } else if app.typing_card() {
            t.typing_card.to_string()
        } else if app.typing_jump() {
            t.typing_jump.to_string()
        } else if app.typing_command() {
            t.typing_command.to_string()
        } else {
            fill(t.typing_to, &[&app.typing_target()])
        },
        bold(pal().strong),
    )
}

/// The text being written in rows `width` cells wide after its lead
/// ([`writing_to`]): each line cut where it reaches the edge
/// ([`wrap_cells`]), cleaned. With the row and column (in the text) of
/// the cursor.
fn written_rows(app: &App, width: usize) -> (Vec<String>, (usize, usize)) {
    let room = width.saturating_sub(writing_to(app).width());
    let at = app.cursor();
    let mut start = 0;
    let mut rows = Vec::new();
    let mut caret = (0, 0);
    for line in app.input.split('\n') {
        let from = start;
        start += line.len() + 1;
        let (text, here) = if (from..start).contains(&at) {
            let (text, before) = caret_line(line, at - from);
            (text, Some(before))
        } else {
            (clean(line), None)
        };
        let (cut, (row, column)) = wrap_cells(&text, room, here);
        if here.is_some() {
            caret = (rows.len() + row, column);
        }
        rows.extend(cut);
    }
    (rows, caret)
}

/// The most rows the line being written takes.
const INPUT_ROWS: u16 = 8;

/// How many rows the footer takes: one, or while writing one per row of
/// the text in a footer `width` wide ([`written_rows`]), at most
/// [`INPUT_ROWS`] and a third of the screen.
pub(super) fn input_rows(app: &App, width: u16, height: u16) -> u16 {
    if !app.typing {
        return 1;
    }
    let rows = written_rows(app, usize::from(width)).0.len();
    let most = (height / 3).clamp(1, INPUT_ROWS);
    u16::try_from(rows).unwrap_or(u16::MAX).min(most)
}

/// The text being written, `rows` rows of a footer `width` wide under who
/// it goes to: a line too long for a row goes on in the next, the
/// cursor's row always shown (the rows above it go out of view first),
/// where the cursor is (`3/12`) when not all rows fit, and the keys to
/// send or cancel only while nothing is written yet. With the rows, the
/// cursor's row among them and its column.
fn writing(app: &App, rows: usize, width: usize) -> (Vec<Line<'static>>, (usize, usize)) {
    let t = app.lang.text();
    let to = writing_to(app);
    let indent = Span::raw(" ".repeat(to.width()));
    let (all, (row, column)) = written_rows(app, width);
    let first = (row + 1).saturating_sub(rows);
    let mut out: Vec<Line<'static>> = (all.iter().enumerate().skip(first).take(rows.max(1)))
        .map(|(n, text)| {
            let lead = if n == first {
                to.clone()
            } else {
                indent.clone()
            };
            Line::from(vec![lead, Span::raw(text.clone())])
        })
        .collect();
    let caret = (row - first, to.width() + column);
    let mut hint = Vec::new();
    if all.len() > out.len() {
        hint.push(Span::styled(format!("   {}/{}", row + 1, all.len()), dim()));
    }
    let newline = if app.shift_enter {
        t.newline_shift
    } else {
        t.newline_plain
    };
    // The menu's keys while it is open; the keys to send only until
    // something is written.
    let keys = match (&app.menu, &app.noting) {
        (Some(m), _) if m.items.len() > 1 => Some(format!(
            "{}{}",
            t.pick_cancel,
            fill(
                t.menu_at,
                &[&(m.chosen + 1).to_string(), &m.items.len().to_string()]
            )
        )),
        (Some(_), _) => Some(t.pick_cancel.to_string()),
        _ if !app.input.is_empty() => None,
        (None, Some(_)) => Some(fill(t.keep_cancel, &[newline])),
        (None, None) => Some(fill(t.send_cancel, &[newline])),
    };
    hint.extend(keys.map(|k| Span::styled(k, dim())));
    if let Some(what) = command_hint(app) {
        hint.push(Span::styled(format!("   {what}"), fg(pal().info)));
    }
    if let Some(last) = out.last_mut() {
        last.spans.extend(hint);
    }
    (out, caret)
}

/// What the `:` command `tab` just put on the line does, in a line: shown
/// after the menu's keys until the person writes on (the menu is made
/// again then, with nothing put in).
fn command_hint(app: &App) -> Option<&'static str> {
    let m = app.menu.as_ref().filter(|m| m.inserted && m.start == 1)?;
    if !app.input.starts_with(':') {
        return None;
    }
    let name = m.items.get(m.chosen)?;
    let hints = app.lang.text().command_hints;
    hints.iter().find(|(c, _)| c == name).map(|(_, what)| *what)
}

/// The footer: what is being written, else the keys or a word for a
/// moment. While writing, where the cursor goes.
pub(super) fn footer(f: &mut Frame, area: Rect, app: &App) -> Option<Position> {
    let t = app.lang.text();
    if app.typing {
        let (lines, (row, column)) =
            writing(app, usize::from(area.height), usize::from(area.width));
        f.render_widget(Paragraph::new(lines), area);
        return caret_in(area, row, column);
    }
    let line = if let Some((msg, at)) = &app.flash
        && at.elapsed() < FLASH
    {
        Line::from(Span::styled(format!(" {msg}"), fg(pal().warn)))
    } else if let Some((what, left)) = app.grace_left(std::time::Instant::now()) {
        let left = left.to_string();
        let said = match what {
            Undoable::Close(p) => fill(t.grace_close, &[&app.world.projects[p].name, &left]),
            Undoable::Forget(id) => fill(t.grace_forget, &[&app.world.agents[id].name, &left]),
            Undoable::Archive(p) => {
                let name = &app.world.projects[p].name;
                let cut = (app.world.mid_turn().into_iter())
                    .filter(|&a| app.world.agents[a].project == p)
                    .count();
                match cut {
                    0 => fill(t.grace_archive, &[name, &app.archive_title, &left]),
                    n => fill(
                        t.grace_archive_cut,
                        &[name, &app.archive_title, &left, &counted(n, t.n_turns)],
                    ),
                }
            }
            Undoable::Clear(what) => fill(t.grace_clear[what.index()], &[&left]),
        };
        Line::from(Span::styled(format!(" {said}"), fg(pal().warn)))
    } else if let Some(asking) = app.asking() {
        // Waiting for an answer: the spinner turns while motion moves.
        Line::from(vec![
            Span::styled(format!(" {} ", app.spin().unwrap_or("…")), fg(pal().info)),
            Span::styled(asking.to_string(), dim()),
        ])
    } else if let Some(chord) = which_key(app) {
        chord
    } else {
        // The status line (record 125): where the person is, then at most
        // three keys for the selected thing, the way back and the help.
        let tail = |keys: &mut Vec<(&'static str, &'static str)>, back: bool| {
            if back {
                keys.push(("esc", t.back));
            }
            keys.push(("?", t.help));
        };
        let hints: Vec<(&str, &str)> = match app.zoom {
            _ if app.script.is_some() => t.setup_hints.to_vec(),
            _ if app.plan_window.is_some() => t.plan_hints.to_vec(),
            // A window over the screen: its own keys, no place.
            _ if app.window == Some(Window::Timeline) => {
                let mut keys = vec![
                    ("[ ]", t.timeline_scale),
                    ("enter", t.focus),
                    ("@", t.to_agent),
                ];
                tail(&mut keys, true);
                keys
            }
            _ if app.window.is_some() => vec![("esc", t.back), ("?", t.help)],
            // The conversation has its keys on its own last row; a chat
            // is written in.
            _ if app.chat_agent().is_some() => {
                vec![("/", t.write_this), ("esc", t.back), ("?", t.help)]
            }
            _ if app.thread.is_some() => vec![("esc", t.back), ("?", t.help)],
            _ if app.peek.is_some() => {
                let mut keys = vec![("enter", t.focus), ("@", t.write_this)];
                tail(&mut keys, true);
                keys
            }
            // A card's screen: the log tab of its detail (record 125).
            Zoom::None | Zoom::Focus(_) if app.card.is_some() => {
                let mut keys = vec![("enter", t.trace), ("n p", t.next_card)];
                tail(&mut keys, true);
                keys
            }
            Zoom::None => home_hints(app),
            Zoom::Focus(id) => {
                // On a change line `enter` opens that file's lines, said
                // beside what decides the change while three keys fit.
                let lines = app.change_line(id).is_some();
                let enter = ("enter", if lines { t.line_opens } else { t.trace });
                let mut keys = match ask_hints(app) {
                    Some(mut keys) if lines && keys.len() < 3 => {
                        keys.insert(0, enter);
                        keys
                    }
                    Some(keys) => keys,
                    None => vec![enter, ("@", t.write_this)],
                };
                tail(&mut keys, true);
                keys
            }
            Zoom::Trace(_) => {
                let mut keys = ask_hints(app).unwrap_or_else(|| vec![("@", t.write_this)]);
                tail(&mut keys, true);
                keys
            }
            // Choosing lines to copy: only copy mode's keys work.
            Zoom::Diff(id) if app.diff_copy_of(id).is_some() => {
                let mut keys = t.copy_hints.to_vec();
                keys.push(("Y", t.new_side));
                keys.push(("?", t.help));
                keys
            }
            Zoom::Diff(id) => {
                let mut keys = vec![("a", t.apply), ("r", t.reject), ("c", t.note_key)];
                if app.world.clashes_of(id).is_some() {
                    keys.pop();
                    keys.insert(0, ("b", t.send_back));
                }
                tail(&mut keys, true);
                keys
            }
        };
        let mut spans = vec![Span::raw(" ")];
        let (place, hints) = status_parts(place_spans(app), hints, usize::from(area.width));
        if let Some(place) = place {
            spans.push(place);
            spans.push(Span::raw("   "));
        }
        spans.extend(hint_spans(&hints));
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
    None
}

/// The keys of Home: what the row at hand comes to first, a row's own
/// keys never out of sight.
pub(crate) fn home_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    let t = app.lang.text();
    let w = &app.world;
    let kind = match app.home_at() {
        Some(crate::app::HomeRow::Item(i)) => Some(w.inbox[i].kind),
        _ => None,
    };
    let mut keys = match kind {
        Some(InboxKind::Change) => vec![("enter", t.home_read), ("a", t.apply), ("r", t.reject)],
        Some(InboxKind::Question | InboxKind::Fault | InboxKind::Blocked) => vec![
            ("a", t.home_answer),
            ("enter", t.home_open),
            ("s", t.home_later),
        ],
        Some(InboxKind::Approval) => vec![("a", t.allow_once), ("r", t.refuse), ("e", t.this_turn)],
        Some(InboxKind::Reply) => vec![
            ("enter", t.home_open),
            ("r", t.home_dismiss),
            ("s", t.home_later),
        ],
        Some(InboxKind::Tries | InboxKind::Rounds) => vec![
            ("enter", t.home_open),
            ("a", t.home_once_more),
            ("r", t.home_fail),
        ],
        Some(InboxKind::Close) => vec![("a", t.home_close), ("r", t.home_leave)],
        Some(_) => vec![("enter", t.home_open), ("a", t.home_yes), ("r", t.home_no)],
        None => match app.home_at() {
            // A stop of a whole team: its choices, then what stopped.
            Some(crate::app::HomeRow::Stop(_, halt)) => vec![
                (super::halt::digits(halt), t.home_pick),
                ("enter", t.home_open),
                ("s", t.home_later),
            ],
            Some(crate::app::HomeRow::Project(_)) => {
                vec![
                    ("enter", t.home_open),
                    ("h l", t.home_fold),
                    ("s", t.home_hide),
                ]
            }
            Some(_) => vec![
                ("enter", t.home_open),
                ("s", t.home_hide),
                ("@", t.write_this),
            ],
            // Nothing listed: no agent to write to, the lead to write to.
            None => vec![("/", t.write_lead)],
        },
    };
    keys.push(("?", t.help));
    keys
}

/// The keys that decide what the agent of the open focus or raw stream
/// waits for, first in the footer: `a A r` on its ask, `a r` on its
/// change; none when it waits for neither or no focus is open.
fn ask_hints(app: &App) -> Option<Vec<(&'static str, &'static str)>> {
    let t = app.lang.text();
    let item = app.focus_ask(app.zoomed_agent()?)?;
    Some(match app.world.inbox[item].kind {
        InboxKind::Approval => vec![("a", t.allow_once), ("r", t.refuse), ("e", t.this_turn)],
        _ => vec![("a", t.apply), ("r", t.reject)],
    })
}

/// While the first key of a chord waits for its second (`g…`, `z…`,
/// `q…`, or a key the person's file pairs), the footer lists what the
/// second key can be and what each does, as which-key does in vim:
/// imece's own pairs, then the file's, each as what it acts as. Nothing
/// while no chord waits.
pub(super) fn which_key(app: &App) -> Option<Line<'static>> {
    let t = app.lang.text();
    let first = match (&app.keys_held, app.pending) {
        (Some((k, _)), _) => crate::keys::Key::of(k),
        (None, Some(c)) => crate::keys::Key::parse(&c.to_string())?,
        (None, None) => return None,
    };
    let own: &[(&str, &str)] = match app.pending {
        Some('g') => &t.chord_g,
        Some('z') => &t.chord_z,
        Some('q') => &t.chord_q,
        _ => &[],
    };
    let mut keys: Vec<(String, String)> = own
        .iter()
        .map(|(k, w)| (k.to_string(), w.to_string()))
        .collect();
    // `gd` `gw` act on Home's list alone: said there alone.
    if app.pending == Some('g') && app.home_keys() {
        keys.extend((t.chord_home.iter()).map(|(k, w)| (k.to_string(), w.to_string())));
    }
    for (from, to) in app.keys.given() {
        let from = from.keys();
        if from.len() == 2 && from[0] == first {
            keys.push((from[1].name(), fill(t.chord_as, &[&to.name()])));
        }
    }
    let mut spans = vec![Span::styled(
        format!(" {}…  ", first.name()),
        bold(pal().warn),
    )];
    for (k, what) in keys {
        spans.push(Span::styled(format!(" {k} "), key_style()));
        spans.push(Span::raw(format!(" {what}  ")));
    }
    Some(Line::from(spans))
}

/// Where the person is, dim, before the footer's keys: the view or the
/// screen over it, the project at hand, and the card (else the agent)
/// the open screen or the selected row is about. Nothing in a window
/// that has a title of its own, or under hint letters.
fn place_spans(app: &App) -> Vec<Span<'static>> {
    let t = app.lang.text();
    let w = &app.world;
    let copying = matches!(app.zoom, Zoom::Diff(id) if app.diff_copy_of(id).is_some());
    if app.script.is_some() || app.plan_window.is_some() || app.window.is_some() || copying {
        return Vec::new();
    }
    let place = if app.thread.is_some() {
        t.places[3]
    } else if app.peek.is_some() {
        t.places[5]
    } else {
        match app.zoom {
            Zoom::None | Zoom::Focus(_) if app.card.is_some() => t.places[4],
            Zoom::None => t.home_place,
            Zoom::Focus(_) => t.places[0],
            Zoom::Trace(_) => t.places[1],
            Zoom::Diff(_) => t.places[2],
        }
    };
    let mut words = vec![place.to_string()];
    let p = app.project_at_hand();
    if let Some(project) = w.projects.get(p) {
        words.push(project.name.clone());
    }
    // On Home's list the row at hand; anywhere else (a screen over Home
    // too) the agent at hand.
    // An item's row is about the card it came on, not the one its agent
    // has since taken.
    let (about, card) = if app.zoom == Zoom::None {
        match app.home_at() {
            Some(crate::app::HomeRow::Item(i)) => {
                let item = w.inbox.get(i);
                (item.map(|item| item.agent), item.and_then(|item| item.card))
            }
            Some(crate::app::HomeRow::Agent(id)) => (Some(id), None),
            // A project's heading is about no card: the project is named.
            Some(crate::app::HomeRow::Project(_) | crate::app::HomeRow::Stop(..)) | None => {
                (None, None)
            }
        }
    } else {
        // A detail is about the card its head names: a change's or an
        // item's, else the agent's (`App::detail_card`).
        let id = app.agent_at_hand();
        (id, id.and_then(|id| app.detail_card(id)))
    };
    if let Some(id) = about.filter(|&id| id < w.agents.len()) {
        match card
            .or_else(|| w.card_of(id))
            .and_then(|c| w.cards.iter().find(|card| card.id == c))
        {
            Some(card) => words.push(format!(
                "{} {}",
                words::card_ref(card.id),
                cut_title(&card.title, PLACE_TITLE)
            )),
            None => words.push(w.agents[id].name.clone()),
        }
    }
    vec![Span::styled(clean(&words.join(" · ")), dim())]
}

/// The place's narrowest on the status line: under it the place goes.
pub(super) const PLACE_MIN: usize = 12;

/// The status line's parts on a line `width` wide (record 131): every
/// key first, the place in what they leave, cut with `…` while it keeps
/// `PLACE_MIN` cells, else gone; only then keys go, from the end, never
/// `esc` or `?`. A blank leads the line and three stand between the two.
pub(super) fn status_parts<'a>(
    place: Vec<Span<'static>>,
    hints: Vec<(&'a str, &'a str)>,
    width: usize,
) -> (Option<Span<'static>>, Vec<(&'a str, &'a str)>) {
    let room = width.saturating_sub(1);
    let left = room.saturating_sub(spans_width(&hint_spans(&hints)) + 3);
    let place = place.into_iter().next().filter(|p| p.width() > 0);
    match place {
        Some(p) if left >= PLACE_MIN => {
            let words = cut_title(&p.content, left);
            (Some(Span::styled(words, p.style)), hints)
        }
        _ => (None, fit_hints(hints, room)),
    }
}

/// How much of a card's title the status line carries.
const PLACE_TITLE: usize = 30;

/// `title` whole when it fits `room` cells, else its start and `…`.
fn cut_title(title: &str, room: usize) -> String {
    if title.width() <= room {
        return title.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in title.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > room.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.trim_end().to_string() + "…"
}

/// The cell `column` columns into row `row` of `area`, where the cursor
/// goes: on the area's last column when the text runs past its edge (the
/// line is cut there), none on a row the area does not have.
pub(crate) fn caret_in(area: Rect, row: usize, column: usize) -> Option<Position> {
    let row = u16::try_from(row).ok().filter(|&r| r < area.height)?;
    let room = usize::from(area.width.checked_sub(1)?);
    let column = u16::try_from(column.min(room)).ok()?;
    Some(Position::new(area.x + column, area.y + row))
}
