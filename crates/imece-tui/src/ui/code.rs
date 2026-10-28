//! The code map of the current project: its files with the symbols leased
//! right now, so the person sees where work happens and where agents share a
//! file.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let project = &w.projects[app.current_project()];
    let block = Block::bordered()
        .title(fill(t.code_title, &[&project.name]))
        .border_style(dim());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list, foot] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    // (file, symbol, holder) for every lease held now in this project.
    let held: Vec<(&str, &str, AgentId)> = w
        .cards
        .iter()
        .filter(|c| c.project == project.id && c.holds_leases())
        .filter_map(|c| c.owner.map(|o| (c, o)))
        .flat_map(|(c, o)| c.symbols.iter().map(move |s| (s.file, s.name, o)))
        .collect();

    let mut lines = Vec::new();
    let mut dir = "";
    let mut shared = 0;
    for &(file, _) in project.repo {
        let (d, base) = file.rsplit_once('/').unwrap_or(("", file));
        if d != dir {
            lines.push(Line::from(Span::styled(format!("{d}/"), dim())));
            dir = d;
        }
        let mut spans = vec![Span::raw(format!("  {}", fit(base, 24)))];
        let here: Vec<&(&str, &str, AgentId)> = held.iter().filter(|h| h.0 == file).collect();
        if here.is_empty() {
            spans.push(Span::styled("·", dim()));
        }
        for &&(_, symbol, owner) in &here {
            let a = &w.agents[owner];
            let mark = match a.status {
                Status::Blocked => "✕",
                Status::WaitingHuman => "!",
                Status::Verifying { .. } => "◌",
                _ => "█",
            };
            let short = symbol.rsplit_once('.').map_or(symbol, |(_, m)| m);
            spans.push(Span::styled(
                format!("{mark}{mark} {}", clean(&a.name)),
                fg(vendor_color(a.vendor)),
            ));
            spans.push(Span::styled(format!(" {short}  "), dim()));
        }
        let mut owners: Vec<AgentId> = here.iter().map(|h| h.2).collect();
        owners.sort_unstable();
        owners.dedup();
        if owners.len() > 1 {
            shared += 1;
            spans.push(Span::styled(
                fill(t.same_file, &[&owners.len().to_string()]),
                fg(Color::Yellow),
            ));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), list);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                fill(t.leases, &[&held.len().to_string()]),
                bold(Color::White),
            ),
            Span::styled(fill(t.shared_files, &[&shared.to_string()]), dim()),
            Span::styled(t.code_legend, dim()),
        ])),
        foot,
    );
}
