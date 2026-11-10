//! What stops a whole team, as Home says it (record 146, part 2): a
//! `■ STOPPED` line above everything else, a row first in TO DECIDE that
//! ends with imece's choices as digits, and a preview that names who
//! waits behind it and what each choice does.

use super::*;
use crate::app::App;
use crate::halt::{Halt, Stop};

/// The stops in Home's view: the project it is narrowed to, else every
/// open one, in the world's order; rested ones too (the line stays).
fn in_view(app: &App) -> Vec<&Stop> {
    let w = &app.world;
    (w.halts.stops.iter())
        .filter(|s| app.at_home.scope.is_none_or(|p| p == s.project))
        .filter(|s| !w.projects[s.project].closed)
        .collect()
}

/// What a stop says after its words, dim: the pot's figures, else the
/// words of what stopped; none for the turns.
fn detail(app: &App, s: &Stop) -> Option<String> {
    let w = &app.world;
    match s.halt {
        Halt::Pot => Some(fill(
            app.lang.text().budget,
            &[&w.spent_words(), &dollars(w.budget)],
        )),
        Halt::Store | Halt::Lead if !s.why.is_empty() => Some(clean(&words::one_line(&s.why))),
        Halt::Store | Halt::Lead | Halt::Woken => None,
    }
}

/// `3 wait behind it`, none while nobody does.
fn behind(app: &App, s: &Stop) -> Option<String> {
    (s.waiting() > 0).then(|| words::behind_words(app.lang, s.waiting()))
}

/// Home's first lines while a stop holds in its view, one a stop:
/// `■ STOPPED  shop-api · the pot is spent · 3 wait behind it · 2 min`,
/// cut to `width`, then a blank line; nothing while none holds.
pub(crate) fn banner(app: &App, width: usize) -> Vec<Line<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let mut lines = Vec::new();
    for s in in_view(app) {
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(format!("■ {}", t.halt_word), bold(pal().bad)),
            Span::raw("  "),
            Span::raw(clean(&w.projects[s.project].name)),
            Span::styled(" · ", dim()),
            Span::raw(clean(&words::halt_words(app.lang, w, s.project, s.halt))),
        ];
        if let Some(more) = behind(app, s) {
            spans.push(Span::styled(" · ", dim()));
            spans.push(Span::styled(more, fg(pal().bad)));
        }
        let seconds = w.tick.saturating_sub(s.since) * SECONDS_PER_TICK;
        if seconds >= 60 {
            spans.push(Span::styled(
                format!(" · {}", words::ago(app.lang, seconds)),
                dim(),
            ));
        }
        lines.push(overlay::cut(Line::from(spans), width.saturating_sub(1)).0);
    }
    if !lines.is_empty() {
        lines.push(Line::default());
    }
    lines
}

/// The choices of a stop as the row's keys: `1 raise by $50.00  2 …`.
fn choice_spans(app: &App, p: ProjectId, halt: Halt) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (n, &c) in halt.choices().iter().enumerate() {
        spans.push(Span::styled(format!(" {} ", n + 1), key_style()));
        let said = words::choice_words(app.lang, &app.world, p, c);
        spans.push(Span::styled(format!(" {said}  "), dim()));
    }
    spans
}

/// The digits a stop's choices take, for the status line: `1`, `1-3`.
pub(crate) fn digits(halt: Halt) -> &'static str {
    match halt.choices().len() {
        1 => "1",
        2 => "1-2",
        _ => "1-3",
    }
}

/// A stop's row in TO DECIDE: the glyph, the project in the title's
/// column, what stopped, its detail and who waits behind it, the words
/// wrapping under their column; the row at hand ends with its choices,
/// on their own line when they do not fit beside the words (never on a
/// narrow screen: the status line has them).
pub(crate) fn row(
    app: &App,
    p: ProjectId,
    halt: Halt,
    width: usize,
    narrow: bool,
    here: bool,
    title_width: usize,
) -> Vec<Line<'static>> {
    let w = &app.world;
    let Some(s) = w.halt_of(p, halt) else {
        return Vec::new();
    };
    let selection = |line: Line<'static>| {
        if here {
            line.style(Style::new().bg(pal().selection))
        } else {
            line
        }
    };
    let title_style = if here {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    let mut head = vec![
        Span::styled(if here { "▸" } else { " " }, bold(pal().strong)),
        Span::styled("■", fg(pal().bad)),
        Span::raw(" "),
        Span::raw("     "),
    ];
    let name = clean(&w.projects[p].name);
    head.push(Span::styled(
        format!("{:<title_width$}  ", fit(&name, title_width)),
        title_style,
    ));
    let used: usize = head.iter().map(Span::width).sum();
    let mut said = words::halt_words(app.lang, w, p, halt);
    if let Some(d) = detail(app, s) {
        said.push_str(&format!(" · {d}"));
    }
    if let Some(more) = behind(app, s) {
        said.push_str(&format!(" · {more}"));
    }
    let room = width.saturating_sub(used + 1).max(1);
    let mut rows = overlay::wrap_words(&said, room).into_iter();
    let mut first = head;
    first.push(Span::raw(rows.next().unwrap_or_default()));
    let keys = if here && !narrow {
        choice_spans(app, p, halt)
    } else {
        Vec::new()
    };
    let keys_width: usize = keys.iter().map(Span::width).sum();
    let mut lines = Vec::new();
    let filled: usize = first.iter().map(Span::width).sum();
    let beside = !keys.is_empty() && filled + 2 + keys_width <= width;
    if beside {
        first.push(Span::raw(" ".repeat(width - filled - keys_width)));
        first.extend(keys.iter().cloned());
    }
    lines.push(selection(Line::from(first)));
    for row in rows {
        lines.push(selection(Line::from(vec![
            Span::raw(" ".repeat(used)),
            Span::raw(row),
        ])));
    }
    if !keys.is_empty() && !beside {
        let mut line = vec![Span::raw(" ".repeat(used))];
        line.extend(keys);
        lines.push(selection(overlay::cut(Line::from(line), width).0));
    }
    lines
}

/// The preview of a stop: what stopped since when, who waits behind it
/// and with what, and each choice with what it does.
pub(crate) fn preview(app: &App, p: ProjectId, halt: Halt, room: usize) -> Vec<Line<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let Some(s) = w.halt_of(p, halt) else {
        return Vec::new();
    };
    let mut head = vec![
        Span::styled(
            format!("■ {}", words::halt_words(app.lang, w, p, halt)),
            bold(pal().bad),
        ),
        Span::styled(format!("  {}", clean(&w.projects[p].name)), dim()),
    ];
    head.push(Span::styled(format!(" · {}", clock(s.since)), dim()));
    let mut lines = vec![overlay::cut(Line::from(head), room).0];
    if let Some(d) = detail(app, s) {
        for row in overlay::wrap_words(&d, room.max(1)) {
            lines.push(Line::from(Span::styled(row, dim())));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(t.halt_behind_label, dim())));
    if s.behind.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  {}", t.halt_nobody),
            dim(),
        )));
    }
    for b in &s.behind {
        let a = &w.agents[b.agent];
        let named = match a.card.and_then(|c| w.cards.get(c)) {
            Some(c) => format!("{} {}", words::card_ref(c.id), clean(&c.title)),
            None => clean(&a.name),
        };
        let waits: Vec<String> = (b.waits.iter())
            .map(|&x| words::waits_words(app.lang, w, p, halt, x))
            .collect();
        let line = Line::from(vec![
            Span::raw(format!("  {} ", fit(&named, 28))),
            Span::styled(waits.join(" · "), dim()),
        ]);
        lines.push(overlay::cut(line, room).0);
    }
    lines.push(Line::default());
    for (n, &c) in halt.choices().iter().enumerate() {
        let does = words::does_words(app.lang, c);
        let line = Line::from(vec![
            Span::styled(format!(" {} ", n + 1), key_style()),
            Span::raw(format!(" {}", words::choice_words(app.lang, w, p, c))),
            Span::styled(format!(" · {does}"), dim()),
        ]);
        lines.push(overlay::cut(line, room).0);
    }
    let more = match halt {
        Halt::Pot => Some((":", format!("budget USD · {}", t.halt_other_amount))),
        Halt::Woken | Halt::Lead => Some(("/", t.write_lead.to_string())),
        Halt::Store => None,
    };
    if let Some((key, said)) = more {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key} "), key_style()),
            Span::styled(format!(" {said}"), dim()),
        ]));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::halt::{Held, Waits};

    fn text(lines: &[Line]) -> Vec<String> {
        (lines.iter())
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// A demo app of one project whose pot is spent with two behind it.
    fn spent() -> App {
        let mut app = App::new(1, 2, 0, 1, crate::i18n::Lang::En);
        app.world.inbox.clear();
        let w = &mut app.world;
        w.budget = w.spent();
        w.tick = 30;
        w.look_halts(&[
            Held {
                agent: 1,
                by: Halt::Pot,
                waits: Waits::Mail(2),
            },
            Held {
                agent: 2,
                by: Halt::Pot,
                waits: Waits::Retry,
            },
        ]);
        app
    }

    fn name(app: &App) -> String {
        app.world.projects[0].name.clone()
    }

    fn pot(app: &App) -> String {
        let w = &app.world;
        format!("{} of {}", w.spent_words(), dollars(w.budget))
    }

    #[test]
    fn the_banner_says_what_stopped_whose_how_many_wait_and_since_when() {
        let mut app = spent();
        let got = text(&banner(&app, 100));
        assert_eq!(got.len(), 2, "{got:?}");
        let head = format!(
            " ■ STOPPED  {} · the pot is spent · 2 wait behind it",
            name(&app)
        );
        assert_eq!(got[0], head);
        assert_eq!(got[1], "");
        app.world.tick += 12 * 3;
        let got = text(&banner(&app, 100));
        assert!(got[0].ends_with("wait behind it · 3 min"), "{got:?}");
        app.world.halts.stops.clear();
        assert!(banner(&app, 100).is_empty());
    }

    #[test]
    fn a_stop_s_row_at_hand_ends_with_its_choices_and_one_not_at_hand_does_not() {
        let app = spent();
        let at = text(&row(&app, 0, Halt::Pot, 160, false, true, 24));
        assert_eq!(at.len(), 1, "{at:?}");
        assert!(
            at[0].starts_with(&format!("▸■      {}", name(&app))),
            "{:?}",
            at[0]
        );
        let said = format!("the pot is spent · {} · 2 wait behind it", pot(&app));
        assert!(at[0].contains(&said), "{:?}", at[0]);
        assert!(
            at[0].ends_with(" 1  raise by $50.00   2  raise by $20.00   3  rest here  "),
            "{:?}",
            at[0]
        );
        let not = text(&row(&app, 0, Halt::Pot, 160, false, false, 24));
        assert!(!not[0].contains("raise by"), "{not:?}");
        // A narrow screen: the status line has the keys.
        let narrow = text(&row(&app, 0, Halt::Pot, 140, true, true, 24));
        assert!(!narrow.concat().contains("raise by"), "{narrow:?}");
        // Too little room beside the words: the choices go under them.
        let tight = text(&row(&app, 0, Halt::Pot, 80, false, true, 24));
        assert!(tight.len() >= 2, "{tight:?}");
        assert!(
            tight.last().unwrap().contains(" 1  raise by $50.00"),
            "{tight:?}"
        );
        // A stop that is over has no row.
        assert!(row(&app, 0, Halt::Lead, 140, false, true, 24).is_empty());
    }

    #[test]
    fn the_preview_names_who_waits_with_what_and_what_each_choice_does() {
        let app = spent();
        let got = text(&preview(&app, 0, Halt::Pot, 90)).join("\n");
        let head = format!("■ the pot is spent  {} · {}", name(&app), clock(30));
        for said in [
            head.as_str(),
            "waits behind it",
            "  claude-1",
            "2 messages for it, not delivered",
            "its retry after an API error",
            " 1  raise by $50.00 · the ones behind it go on at once",
            " 3  rest here · nothing new starts; the row goes after the rest",
            " :  budget USD · another amount",
        ] {
            assert!(got.contains(said), "{said:?} in\n{got}");
        }
    }

    #[test]
    fn the_header_marks_the_project_and_counts_the_stop_among_what_is_to_decide() {
        let app = spent();
        let spans: String = (super::super::header::projects_spans(&app).iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(spans.ends_with(" ■"), "{spans:?}");
        let head: String = (super::super::header::header_parts(&app).iter())
            .flat_map(|p| p.spans.iter().map(|s| s.content.to_string()))
            .collect();
        let decide = fill(app.lang.text().waiting_for_you, &["1"]);
        assert!(head.contains(&decide), "{head:?}");
    }

    #[test]
    fn a_stop_rings_in_imece_s_words_with_its_project_and_loud() {
        let app = spent();
        let note = crate::notify::Note::Halt(0, Halt::Pot);
        let said = format!("imece · {} stopped: the pot is spent", name(&app));
        assert_eq!(words::notice(crate::i18n::Lang::En, &app.world, note), said);
        assert_eq!(words::pushed(crate::i18n::Lang::En, &app.world, note), said);
        let tr = words::notice(crate::i18n::Lang::Tr, &app.world, note);
        assert_eq!(tr, format!("imece · {} durdu: bütçe bitti", name(&app)));
    }

    #[test]
    fn the_status_line_names_the_digits_a_stop_takes() {
        assert_eq!(digits(Halt::Lead), "1");
        assert_eq!(digits(Halt::Store), "1-2");
        assert_eq!(digits(Halt::Pot), "1-3");
    }
}
