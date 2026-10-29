//! The code map of the current project: its files with the symbols leased
//! right now, so the person sees where work happens and where agents share a
//! file. One row per file, one cell per lease, in columns that stay put:
//! the cell is the holder's status glyph (the salon's), its name in its
//! vendor's colour and the symbol, each padded to the widest in the
//! project, so names and symbols line up down the screen. A file several
//! agents hold ends with `⇆ N`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use super::*;
use crate::app::App;

/// Every status glyph the map can show (the salon's), in the order the
/// legend lists them; `Text::code_glyphs` words them in the same order.
pub(crate) const GLYPHS: [&str; 10] = ["○", "◆", "●", "◐", "✕", "!", "◌", "◇", "■", "◦"];

/// Where `mark` sits in `GLYPHS`.
fn kind(mark: &str) -> Option<usize> {
    GLYPHS.iter().position(|g| *g == mark)
}

/// The widest a column gets: a longer name is cut with `…`.
const FILE_MAX: usize = 28;
const NAME_MAX: usize = 14;
const SYMBOL_MAX: usize = 18;

/// Column widths in characters: file names, holder names, symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Widths {
    pub file: usize,
    pub name: usize,
    pub symbol: usize,
}

impl Widths {
    /// From everything that can be shown, not only what is held now, so
    /// the columns do not jump when a lease comes or goes.
    pub(crate) fn of<'a>(
        files: impl Iterator<Item = &'a str>,
        names: impl Iterator<Item = &'a str>,
        symbols: impl Iterator<Item = &'a str>,
    ) -> Self {
        Self {
            file: widest(files.map(base), FILE_MAX),
            name: widest(names, NAME_MAX),
            symbol: widest(symbols.map(short), SYMBOL_MAX),
        }
    }

    /// How wide one lease cell is: glyph, name, symbol and the gaps.
    #[cfg(test)]
    fn cell(&self) -> usize {
        2 + self.name + 2 + self.symbol + 3
    }
}

/// The longest of `names` in characters, at most `cap`.
fn widest<'a>(names: impl Iterator<Item = &'a str>, cap: usize) -> usize {
    names.map(|s| s.chars().count()).max().unwrap_or(0).min(cap)
}

/// The file's own name: what follows the last `/`.
fn base(file: &str) -> &str {
    file.rsplit_once('/').map_or(file, |(_, b)| b)
}

/// A symbol without its type: `Calculate` for `OrderService.Calculate`.
fn short(symbol: &str) -> &str {
    symbol.rsplit_once('.').map_or(symbol, |(_, m)| m)
}

/// How many agents hold a file, when more than one does.
pub(crate) fn shared(owners: &[AgentId]) -> Option<usize> {
    let mut owners = owners.to_vec();
    owners.sort_unstable();
    owners.dedup();
    (owners.len() > 1).then_some(owners.len())
}

/// The text of one lease cell, padded to the columns.
pub(crate) fn cell(glyph: &str, name: &str, symbol: &str, w: &Widths) -> (String, String) {
    (
        format!("{glyph} {}", fit(name, w.name)),
        format!("  {}   ", fit(short(symbol), w.symbol)),
    )
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let project = &w.projects[app.current_project()];
    let block = Block::bordered()
        .title(fill(t.code_title, &[&project.name]))
        .border_style(border());
    let inner = block.inner(area);
    f.render_widget(block, area);
    if project.repo.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(t.no_code_map, dim())).wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    let [list, foot] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let ours = w.cards.iter().filter(|c| c.project == project.id);
    let widths = Widths::of(
        project.repo.iter().map(|(file, _)| *file),
        w.agents
            .iter()
            .filter(|a| a.project == project.id)
            .map(|a| a.name.as_str()),
        ours.clone().flat_map(|c| c.symbols.iter().map(|s| s.name)),
    );

    // (file, symbol, holder) for every lease held now in this project.
    let held: Vec<(&str, &str, AgentId)> = ours
        .filter(|c| c.holds_leases())
        .filter_map(|c| c.owner.map(|o| (c, o)))
        .flat_map(|(c, o)| c.symbols.iter().map(move |s| (s.file, s.name, o)))
        .collect();

    let mut lines = Vec::new();
    let mut dir = "";
    let mut shared_files = 0;
    // The glyphs on the map, with their colour, for the legend.
    let mut seen: [Option<Color>; GLYPHS.len()] = [None; GLYPHS.len()];
    for &(file, _) in project.repo {
        let (d, name) = file.rsplit_once('/').unwrap_or(("", file));
        if d != dir {
            lines.push(Line::from(Span::styled(format!("{d}/"), dim())));
            dir = d;
        }
        let here: Vec<&(&str, &str, AgentId)> = held.iter().filter(|h| h.0 == file).collect();
        let file_style = if here.is_empty() {
            dim()
        } else {
            Style::default()
        };
        let mut spans = vec![Span::styled(
            format!("  {}  ", fit(name, widths.file)),
            file_style,
        )];
        for &&(_, symbol, owner) in &here {
            let a = &w.agents[owner];
            let (mark, colour) = glyph(a);
            if let Some(k) = kind(mark) {
                seen[k] = Some(colour);
            }
            let (head, tail) = cell(mark, &a.name, symbol, &widths);
            spans.push(Span::styled(mark.to_string(), fg(colour)));
            spans.push(Span::styled(
                head[mark.len()..].to_string(),
                fg(vendor_color(a.vendor)),
            ));
            spans.push(Span::styled(tail, dim()));
        }
        let owners: Vec<AgentId> = here.iter().map(|h| h.2).collect();
        if let Some(n) = shared(&owners) {
            shared_files += 1;
            spans.push(Span::styled(
                fill(t.same_file, &[&n.to_string()]),
                fg(pal().warn),
            ));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), list);
    let mut legend = vec![
        Span::styled(
            fill(t.leases, &[&held.len().to_string()]),
            bold(pal().strong),
        ),
        Span::styled(fill(t.shared_files, &[&shared_files.to_string()]), dim()),
        Span::raw(" "),
    ];
    for (k, colour) in seen.iter().enumerate() {
        if let Some(colour) = colour {
            legend.push(Span::styled(format!("  {}", GLYPHS[k]), fg(*colour)));
            legend.push(Span::styled(format!(" {}", t.code_glyphs[k]), dim()));
        }
    }
    if shared_files > 0 {
        legend.push(Span::styled(format!("  {}", t.code_shared), dim()));
    }
    f.render_widget(Paragraph::new(Line::from(legend)), foot);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::View;
    use crate::i18n::Lang;

    #[test]
    fn columns_are_as_wide_as_the_widest_name_and_no_wider_than_their_cap() {
        let w = Widths::of(
            ["src/a/LongerName.cs", "b.cs"].into_iter(),
            ["claude-3", "qwen-4"].into_iter(),
            ["OrderService.Calculate", "Money.Round"].into_iter(),
        );
        assert_eq!(
            w,
            Widths {
                file: 13,
                name: 8,
                symbol: 9
            }
        );
        assert_eq!(w.cell(), 2 + 8 + 2 + 9 + 3);
        let long = "x".repeat(40);
        let capped = Widths::of(
            [long.as_str()].into_iter(),
            [long.as_str()].into_iter(),
            [long.as_str()].into_iter(),
        );
        assert_eq!(
            capped,
            Widths {
                file: FILE_MAX,
                name: NAME_MAX,
                symbol: SYMBOL_MAX
            }
        );
        let none = Widths::of([].into_iter(), [].into_iter(), [].into_iter());
        assert_eq!(
            none,
            Widths {
                file: 0,
                name: 0,
                symbol: 0
            }
        );
    }

    #[test]
    fn a_cell_is_the_glyph_the_padded_name_and_the_padded_short_symbol() {
        let w = Widths {
            file: 10,
            name: 8,
            symbol: 9,
        };
        let (head, tail) = cell("●", "qwen-4", "OrderService.Submit", &w);
        assert_eq!(head, "● qwen-4  ");
        assert_eq!(tail, "  Submit      ");
        assert_eq!(head.chars().count() + tail.chars().count(), w.cell());
        let (_, plain) = cell("!", "claude-3", "Round", &w);
        assert_eq!(plain, "  Round       ", "a symbol without a type stays");
    }

    #[test]
    fn a_file_is_shared_when_two_different_agents_hold_it() {
        assert_eq!(shared(&[]), None);
        assert_eq!(shared(&[4]), None);
        assert_eq!(shared(&[4, 4]), None, "two leases of one agent");
        assert_eq!(shared(&[4, 2]), Some(2));
        assert_eq!(shared(&[4, 2, 2, 7]), Some(3));
    }

    fn render(app: &App, width: u16, height: u16) -> Vec<String> {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| super::super::draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect()
    }

    fn demo() -> App {
        let mut app = App::new(2, 12, 4, 7, Lang::En);
        for _ in 0..300 {
            app.step();
        }
        app.view = View::Code;
        app
    }

    /// Where the lease cells start on a row: the status glyphs.
    fn cell_starts(row: &str) -> Vec<usize> {
        let chars: Vec<char> = row.chars().collect();
        (0..chars.len().saturating_sub(2))
            .filter(|&i| {
                "●◌✕!◐○◆◇■".contains(chars[i])
                    && chars[i + 1] == ' '
                    && chars[i + 2].is_ascii_alphabetic()
            })
            .collect()
    }

    #[test]
    fn leases_sit_in_columns_that_line_up_down_the_map() {
        let mut app = demo();
        // The other project's names and symbols must not widen the columns.
        let here = app.world.projects[app.current_project()].id;
        let other = app
            .world
            .agents
            .iter_mut()
            .find(|a| a.project != here)
            .expect("two projects");
        other.name = "an-agent-with-a-long-name".into();
        let card = app
            .world
            .cards
            .iter_mut()
            .find(|c| c.project != here)
            .expect("the other project has cards");
        card.symbols.push(crate::model::Symbol {
            file: "elsewhere/Far.cs",
            name: "Far.AVeryLongSymbolNameIndeed",
        });
        let w = &app.world;
        let project = &w.projects[app.current_project()];
        let widths = Widths::of(
            project.repo.iter().map(|(file, _)| *file),
            w.agents
                .iter()
                .filter(|a| a.project == project.id)
                .map(|a| a.name.as_str()),
            w.cards
                .iter()
                .filter(|c| c.project == project.id)
                .flat_map(|c| c.symbols.iter().map(|s| s.name)),
        );
        // The border, two spaces, the file column, two spaces.
        let first = 1 + 2 + widths.file + 2;
        let rows = render(&app, 160, 50);
        let mut cells = 0;
        // The legend's glyphs are not cells.
        for row in rows.iter().filter(|r| !r.contains(" leases ·")) {
            let starts = cell_starts(row);
            for (i, &at) in starts.iter().enumerate() {
                assert_eq!(at, first + i * widths.cell(), "{row:?}");
            }
            cells += starts.len();
        }
        assert!(cells > 3, "the demo has leases to show after 300 ticks");
        let shared: Vec<&String> = rows
            .iter()
            .filter(|r| r.contains('⇆') && !r.contains(" leases ·"))
            .collect();
        assert!(!shared.is_empty(), "the demo has files held by several");
        for row in &shared {
            let starts = cell_starts(row);
            assert!(starts.len() >= 2, "{row:?}");
            let badge = row.chars().position(|c| c == '⇆').unwrap();
            assert_eq!(badge, first + starts.len() * widths.cell(), "{row:?}");
            assert!(
                row.contains(&format!("⇆ {}", starts.len())) || row.contains("⇆ 2"),
                "{row:?}"
            );
            assert!(!row.contains("agents in one file"), "{row:?}");
        }
        let legend = rows.iter().find(|r| r.contains(" leases ·")).unwrap();
        assert!(
            legend.contains(&format!("· {} files with several", shared.len())),
            "{legend:?}"
        );
        assert!(
            legend.contains("⇆ several agents in one file"),
            "{legend:?}"
        );
    }

    #[test]
    fn the_legend_words_exactly_the_glyphs_on_the_map() {
        let app = demo();
        let rows = render(&app, 160, 50);
        let t = app.lang.text();
        let on_map: std::collections::BTreeSet<char> = rows
            .iter()
            .filter(|r| !r.contains(" leases ·"))
            .flat_map(|r| {
                let chars: Vec<char> = r.chars().collect();
                cell_starts(r).into_iter().map(move |i| chars[i])
            })
            .collect();
        assert!(on_map.len() >= 2, "{on_map:?}");
        let legend = rows.iter().find(|r| r.contains(" leases ·")).unwrap();
        for (k, g) in GLYPHS.iter().enumerate() {
            let entry = format!("{g} {}", t.code_glyphs[k]);
            assert_eq!(
                legend.contains(&entry),
                on_map.contains(&g.chars().next().unwrap()),
                "{entry:?} in {legend:?}"
            );
        }
    }

    #[test]
    fn with_nothing_held_the_legend_has_nothing_to_explain() {
        let mut app = demo();
        for c in &mut app.world.cards {
            c.symbols.clear();
        }
        let rows = render(&app, 160, 50);
        let legend = rows.iter().find(|r| r.contains(" leases ·")).unwrap();
        assert!(legend.contains("0 leases · 0 files"), "{legend:?}");
        assert!(!legend.contains('⇆'), "{legend:?}");
        for g in GLYPHS {
            assert!(!legend.contains(g), "{legend:?} explains {g}");
        }
    }

    #[test]
    fn the_glyph_table_is_the_salons_in_order() {
        let mut a = crate::model::Agent::new(
            1,
            0,
            "w".into(),
            crate::model::Vendor::Claude,
            Role::Worker,
            Status::Working,
            0,
        );
        let statuses = [
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
            Status::Asleep,
        ];
        for (k, s) in statuses.into_iter().enumerate() {
            a.status = s;
            assert_eq!(kind(glyph(&a).0), Some(k), "{:?}", a.status);
        }
        assert_eq!(kind("?"), None);
        for lang in [Lang::En, Lang::Tr] {
            let words = lang.text().code_glyphs;
            let distinct: std::collections::BTreeSet<&str> = words.iter().copied().collect();
            assert_eq!(distinct.len(), GLYPHS.len(), "{lang:?}: a word per glyph");
        }
    }

    #[test]
    fn every_directory_heads_its_files_once() {
        let app = demo();
        let rows = render(&app, 160, 50);
        let project = &app.world.projects[app.current_project()];
        let mut dirs: Vec<&str> = project
            .repo
            .iter()
            .filter_map(|(file, _)| file.rsplit_once('/').map(|(d, _)| d))
            .collect();
        dirs.dedup();
        assert!(dirs.len() > 1);
        for d in &dirs {
            let heads = rows
                .iter()
                .filter(|r| r.trim_end_matches([' ', '│']) == format!("│{d}/"))
                .count();
            assert_eq!(heads, 1, "{d}");
        }
    }

    #[test]
    fn a_file_nobody_holds_is_only_its_name() {
        let app = demo();
        let rows = render(&app, 160, 50);
        let project = &app.world.projects[app.current_project()];
        let held: Vec<&str> = app
            .world
            .cards
            .iter()
            .filter(|c| c.project == project.id && c.holds_leases() && c.owner.is_some())
            .flat_map(|c| c.symbols.iter().map(|s| s.file))
            .collect();
        let quiet = project
            .repo
            .iter()
            .map(|(file, _)| *file)
            .find(|f| !held.contains(f))
            .expect("a file nobody holds");
        let name = base(quiet);
        let row = rows
            .iter()
            .find(|r| r.contains(&format!("  {name}")))
            .expect("the file is on the map");
        let after = row.split_once(name).unwrap().1;
        assert_eq!(after.trim_end_matches([' ', '│']), "", "{row:?}");
    }
}
