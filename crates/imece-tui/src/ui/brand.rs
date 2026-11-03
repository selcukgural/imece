//! imece's mark on the screen (record 101): the site's mark
//! (`website/static/img/imece-mark.svg`, five hands round one job) drawn
//! in half blocks, twelve cells by four rows, beside imece's words: the
//! welcome and news windows, and the salon while no project has a worker.
//! Where it comes on the screen its dots come one a beat
//! (`App::mark_dots`); a key, motion off, SSH or another window shows it
//! whole at once.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{bold, dim, fg, fit};
use crate::app::App;
use crate::i18n::fill;
use crate::model::Role;
use crate::theme::pal;
use crate::welcome;

/// The mark's dots in the order they come: the five hands clockwise from
/// the top, then the job they are round.
pub(crate) const DOTS: usize = 6;

/// The mark's columns and rows on the screen.
pub(crate) const WIDTH: usize = 12;
pub(crate) const ROWS: usize = 4;

/// Columns between the mark and its words.
const GAP: usize = 3;

/// What a pixel of the mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dot {
    Hand,
    Job,
}

/// Each dot's top-left pixel (column, pixel row) on a 12 × 8 grid, a
/// pixel half a cell high; each dot is 2 × 2 pixels. From the SVG's
/// circles scaled: top (32,14), upper right (49,26), lower right
/// (43,47), lower left (21,47), upper left (15,26), the job (32,33).
const AT: [(usize, usize, Dot); DOTS] = [
    (5, 0, Dot::Hand),
    (9, 2, Dot::Hand),
    (7, 5, Dot::Hand),
    (3, 5, Dot::Hand),
    (1, 2, Dot::Hand),
    (5, 3, Dot::Job),
];

/// The mark with its first `shown` dots, as cells: a row of `WIDTH`
/// (symbol, what it shows) per screen row, two pixels to a cell (`▀` the
/// top one, `▄` the bottom one, `█` both). No two dots share a cell.
pub(crate) fn cells(shown: usize) -> [[(char, Option<Dot>); WIDTH]; ROWS] {
    let mut pixels = [[None; WIDTH]; ROWS * 2];
    for &(x, y, dot) in AT.iter().take(shown) {
        for row in &mut pixels[y..y + 2] {
            row[x..x + 2].fill(Some(dot));
        }
    }
    let mut out = [[(' ', None); WIDTH]; ROWS];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = match (pixels[2 * r][c], pixels[2 * r + 1][c]) {
                (None, None) => (' ', None),
                (Some(d), None) => ('▀', Some(d)),
                (None, Some(d)) => ('▄', Some(d)),
                (Some(d), Some(_)) => ('█', Some(d)),
            };
        }
    }
    out
}

/// The mark's row `r` with its first `shown` dots: the hands in imece's
/// own colour, the job in the strong one (mono: both the text's).
fn mark_spans(r: usize, shown: usize) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut look = None;
    for (ch, dot) in cells(shown)[r] {
        if !run.is_empty() && look != dot {
            spans.push(Span::styled(std::mem::take(&mut run), dot_style(look)));
        }
        look = dot;
        run.push(ch);
    }
    spans.push(Span::styled(run, dot_style(look)));
    spans
}

fn dot_style(dot: Option<Dot>) -> ratatui::style::Style {
    match dot {
        Some(Dot::Hand) => fg(pal().accent),
        Some(Dot::Job) => fg(pal().strong),
        None => ratatui::style::Style::new(),
    }
}

/// imece's three lines beside the mark: name and version, what it is, and
/// the project at hand with who heads it.
pub(crate) fn words(app: &App) -> [Line<'static>; 3] {
    let t = app.lang.text();
    let w = &app.world;
    let p = app.project_at_hand();
    let head = &w.agents[w.projects[p].leader];
    let says = if head.role == Role::Solo {
        t.brand_solo
    } else {
        t.brand_head
    };
    let project = fit(&w.projects[p].name, 24);
    [
        Line::from(vec![
            Span::styled("imece", bold(pal().strong)),
            Span::styled(format!(" v{}", welcome::VERSION), dim()),
        ]),
        Line::from(Span::styled(t.brand_tagline, dim())),
        Line::from(Span::styled(
            fill(says, &[project.trim_end(), head.vendor.label()]),
            dim(),
        )),
    ]
}

/// The mark and its words in `width` columns, `indent` columns in: four
/// rows with the mark when it fits beside them, else the three lines of
/// words alone, each cut to the width.
pub(crate) fn lines(app: &App, width: usize, indent: usize) -> Vec<Line<'static>> {
    let words = words(app);
    let longest = words.iter().map(Line::width).max().unwrap_or(0);
    let pad = " ".repeat(indent);
    if indent + WIDTH + GAP + longest <= width {
        app.mark_shown();
        let shown = app.mark_dots();
        let mut rows: Vec<Line<'static>> = Vec::with_capacity(ROWS);
        let mut words = words.into_iter();
        for r in 0..ROWS {
            let mut spans = vec![Span::raw(pad.clone())];
            spans.extend(mark_spans(r, shown));
            if let Some(line) = words.next() {
                spans.push(Span::raw(" ".repeat(GAP)));
                spans.extend(line.spans);
            }
            rows.push(Line::from(spans));
        }
        return rows;
    }
    let room = width.saturating_sub(indent);
    (words.into_iter())
        .map(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let style = line.spans.first().map(|s| s.style).unwrap_or_default();
            let cut = fit(&text, room);
            Line::from(vec![
                Span::raw(pad.clone()),
                Span::styled(cut.trim_end().to_string(), style),
            ])
        })
        .collect()
}

/// The mark and its words in the middle of the salon's free rows `area`,
/// when they fit there with a row to spare above and below.
pub(crate) fn salon(f: &mut Frame, area: Rect, app: &App) {
    if usize::from(area.height) < ROWS + 2 {
        return;
    }
    let width = usize::from(area.width);
    // `lines` keeps to the width.
    let all = lines(app, width, 0);
    let wide = all.iter().map(Line::width).max().unwrap_or(0);
    let tall = all.len() as u16;
    let x = area.x + (width.saturating_sub(wide) / 2) as u16;
    let y = area.y + (area.height - tall) / 2;
    f.render_widget(Paragraph::new(all), Rect::new(x, y, wide as u16, tall));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(shown: usize) -> Vec<String> {
        (cells(shown).iter())
            .map(|r| r.iter().map(|c| c.0).collect())
            .collect()
    }

    #[test]
    fn the_mark_is_five_hands_round_one_job_in_half_blocks_a_dot_at_a_time() {
        assert_eq!(
            rows(DOTS),
            [
                "     ██     ",
                " ██  ▄▄  ██ ",
                "   ▄▄▀▀▄▄   ",
                "   ▀▀  ▀▀   "
            ]
        );
        assert!(
            rows(0)
                .iter()
                .all(|r| r.trim().is_empty() && r.chars().count() == WIDTH)
        );
        assert_eq!(
            rows(1),
            [
                "     ██     ",
                "            ",
                "            ",
                "            "
            ]
        );
        assert_eq!(rows(2)[1], "         ██ ", "the upper right hand");
        assert_eq!(rows(3)[2], "       ▄▄   ", "the lower right one");
        assert_eq!(rows(3)[3], "       ▀▀   ");
        assert_eq!(rows(4)[2], "   ▄▄  ▄▄   ", "the lower left one");
        assert_eq!(rows(5)[1], " ██      ██ ", "the upper left one, no job yet");
        assert_eq!(rows(7), rows(DOTS), "no more than there are");
        let c = cells(DOTS);
        assert_eq!(c[0][5], ('█', Some(Dot::Hand)));
        assert_eq!(c[1][1], ('█', Some(Dot::Hand)));
        assert_eq!(c[1][5], ('▄', Some(Dot::Job)));
        assert_eq!(c[2][6], ('▀', Some(Dot::Job)));
        assert_eq!(c[2][3], ('▄', Some(Dot::Hand)));
        assert_eq!(c[3][8], ('▀', Some(Dot::Hand)));
        assert_eq!(c[3][0], (' ', None));
    }

    #[test]
    fn the_mark_goes_beside_the_words_only_where_both_fit_else_the_words_alone_cut() {
        let app = App::new(1, 0, 4, 7, crate::i18n::Lang::En);
        let longest = "one terminal, many agents, one leader".len();
        let fits = WIDTH + GAP + longest;
        assert_eq!(fits, 52);
        assert_eq!(lines(&app, fits, 0).len(), ROWS, "exactly");
        assert!(app.mark_drawn.get());
        app.mark_drawn.set(false);
        assert_eq!(lines(&app, fits - 1, 0).len(), 3, "a column short");
        assert!(!app.mark_drawn.get());
        assert_eq!(lines(&app, fits + 2, 2).len(), ROWS, "the indent counts");
        assert_eq!(lines(&app, fits + 1, 2).len(), 3);
        let all = lines(&app, fits + 2, 2);
        assert_eq!(
            all[0].width(),
            2 + WIDTH + GAP + "imece v".len() + crate::welcome::VERSION.len()
        );
        assert_eq!(all[3].width(), 2 + WIDTH, "no fourth line of words");
        let cut = lines(&app, 22, 2);
        let text: Vec<String> = (cut.iter())
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(text[1], "  one terminal, many …");
        assert_eq!(text[2], "  shop-api · claude l…");
        assert!(cut.iter().all(|l| l.width() <= 22));
        assert_eq!(cut[1].spans[1].style, dim(), "its look kept");
    }

    #[test]
    fn the_hands_are_in_imece_s_colour_and_the_job_in_the_strong_one() {
        let spans = mark_spans(1, DOTS);
        let text: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, [" ", "██", "  ", "▄▄", "  ", "██", " "]);
        assert_eq!(spans[1].style, fg(pal().accent));
        assert_eq!(spans[3].style, fg(pal().strong));
        assert_eq!(spans[5].style, fg(pal().accent));
        assert_eq!(spans[0].style, ratatui::style::Style::new());
        assert_ne!(pal().accent, pal().strong, "told apart");
    }
}
