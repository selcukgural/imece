//! A worker's change, whole: the patch as git wrote it, file by file,
//! for the person to read before they apply it to their checkout.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;

/// Line offsets of the file headers (`diff --git`) in `patch`, for
/// jumping from file to file.
pub fn file_starts(patch: &str) -> Vec<usize> {
    patch
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("diff --git "))
        .map(|(i, _)| i)
        .collect()
}

fn line_style(l: &str) -> Style {
    if l.starts_with("diff --git ") {
        bold(pal().strong)
    } else if l.starts_with("+++ ") || l.starts_with("--- ") || l.starts_with("index ") {
        dim()
    } else if l.starts_with("@@") {
        fg(pal().info)
    } else if l.starts_with('+') {
        fg(pal().ok)
    } else if l.starts_with('-') {
        fg(pal().bad)
    } else {
        Style::new()
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let w = &app.world;
    let t = app.lang.text();
    let a = &w.agents[id];
    let Some(c) = w.change_of(id) else {
        let block = Block::bordered()
            .title(fill(t.diff_title, &[&a.name, "", "", "", ""]))
            .border_style(fg(vendor_color(a.vendor)));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                fill(t.diff_gone, &[&a.name]),
                dim(),
            ))),
            inner,
        );
        return;
    };
    let (added, removed) = c.counts();
    let block = Block::bordered()
        .title(fill(
            t.diff_title,
            &[
                &a.name,
                &c.files.len().to_string(),
                &added.to_string(),
                &removed.to_string(),
                &c.title,
            ],
        ))
        .border_style(fg(vendor_color(a.vendor)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.diff_page.set(inner.height);
    let height = usize::from(inner.height);
    let lines: Vec<Line> = c
        .patch
        .lines()
        .skip(app.diff_scroll)
        .take(height)
        .map(|l| Line::from(Span::styled(clean(l), line_style(l))))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_headers_are_where_the_files_start() {
        let patch =
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/y b/y\n";
        assert_eq!(file_starts(patch), [0, 6]);
        assert_eq!(file_starts(""), Vec::<usize>::new());
    }

    #[test]
    fn added_and_removed_lines_have_their_own_colours_and_headers_stand_out() {
        assert_eq!(line_style("+new"), fg(pal().ok));
        assert_eq!(line_style("-old"), fg(pal().bad));
        assert_eq!(line_style("@@ -1 +1 @@"), fg(pal().info));
        assert_eq!(line_style("diff --git a/x b/x"), bold(pal().strong));
        assert_eq!(line_style("+++ b/x"), dim());
        assert_eq!(line_style("--- a/x"), dim());
        assert_eq!(line_style("index 1..2 100644"), dim());
        assert_eq!(line_style(" same"), Style::new());
    }
}
