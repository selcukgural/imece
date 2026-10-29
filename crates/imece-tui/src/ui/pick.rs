//! The question before the salon when several coding agents are on PATH
//! and none is the person's primary: who leads this time? Enter answers
//! for this run, `p` answers for good (the primary vendor, kept in the
//! preferences until `:lead` or the chooser changes it).

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::*;

/// What the person chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Quit,
    Lead {
        vendor: Vendor,
        /// Make it the primary vendor: no question from now on.
        remember: bool,
    },
}

pub struct Pick {
    pub candidates: Vec<Vendor>,
    pub sel: usize,
    /// The primary vendor that was not found, if any: said on screen.
    pub missing: Option<Vendor>,
    pub lang: Lang,
}

impl Pick {
    pub fn new(candidates: Vec<Vendor>, missing: Option<Vendor>, lang: Lang) -> Pick {
        assert!(!candidates.is_empty(), "nothing to choose from");
        Pick {
            candidates,
            sel: 0,
            missing,
            lang,
        }
    }

    /// A key; `Some` once the question is answered.
    pub fn on_key(&mut self, code: KeyCode) -> Option<Picked> {
        let last = self.candidates.len() - 1;
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.sel = self.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.sel = (self.sel + 1).min(last),
            KeyCode::Char('g') | KeyCode::Home => self.sel = 0,
            KeyCode::Char('G') | KeyCode::End => self.sel = last,
            KeyCode::Char(c @ '1'..='9') => {
                let n = usize::from(u8::try_from(c).unwrap_or(b'1') - b'1');
                if n <= last {
                    self.sel = n;
                }
            }
            KeyCode::Enter => {
                return Some(Picked::Lead {
                    vendor: self.candidates[self.sel],
                    remember: false,
                });
            }
            KeyCode::Char('p') => {
                return Some(Picked::Lead {
                    vendor: self.candidates[self.sel],
                    remember: true,
                });
            }
            KeyCode::Esc | KeyCode::Char('q') => return Some(Picked::Quit),
            _ => {}
        }
        None
    }
}

/// Shown while the vendors on PATH are being found, which takes a moment.
pub fn probing(f: &mut Frame, lang: Lang) {
    let area = f.area();
    let line = Line::from(Span::styled(lang.text().probing, dim())).centered();
    let row = Rect {
        y: area.y + area.height / 2,
        height: 1.min(area.height),
        ..area
    };
    f.render_widget(Paragraph::new(line), row);
}

pub fn draw(f: &mut Frame, pick: &Pick) {
    let t = pick.lang.text();
    let mut lines = vec![Line::from(Span::raw(t.pick_text)), Line::raw("")];
    for (n, v) in pick.candidates.iter().enumerate() {
        let line = Line::from(vec![
            Span::styled(format!(" {} ", n + 1), dim()),
            Span::styled(fit(v.label(), 12), bold(vendor_color(*v))),
        ]);
        lines.push(if n == pick.sel {
            line.style(Style::new().add_modifier(Modifier::REVERSED))
        } else {
            line
        });
    }
    lines.push(Line::raw(""));
    if let Some(v) = pick.missing {
        lines.push(Line::from(Span::styled(
            fill(t.pick_missing, &[v.label()]),
            fg(pal().warn),
        )));
    }
    lines.push(Line::from(Span::styled(t.pick_keys, dim())));
    let width = 64.min(f.area().width);
    let height = (lines.len() as u16 + 2).min(f.area().height);
    let area = Rect {
        x: (f.area().width - width) / 2,
        y: (f.area().height - height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .title(t.pick_title)
        .border_style(fg(pal().accent));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(pick: &Pick, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, pick)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_keys_choose_enter_answers_once_and_p_for_good() {
        let mut pick = Pick::new(vec![Vendor::Claude, Vendor::Codex], None, Lang::En);
        assert_eq!(pick.on_key(KeyCode::Char('k')), None);
        assert_eq!(pick.sel, 0, "stays at the top");
        assert_eq!(pick.on_key(KeyCode::Char('j')), None);
        assert_eq!(pick.sel, 1);
        pick.on_key(KeyCode::Char('j'));
        assert_eq!(pick.sel, 1, "stays at the bottom");
        pick.on_key(KeyCode::Char('g'));
        assert_eq!(pick.sel, 0);
        pick.on_key(KeyCode::Char('G'));
        assert_eq!(pick.sel, 1);
        pick.on_key(KeyCode::Char('1'));
        assert_eq!(pick.sel, 0);
        pick.on_key(KeyCode::Char('2'));
        assert_eq!(pick.sel, 1);
        pick.on_key(KeyCode::Char('3'));
        assert_eq!(pick.sel, 1, "no third one");
        assert_eq!(pick.on_key(KeyCode::Char('x')), None);
        assert_eq!(
            pick.on_key(KeyCode::Enter),
            Some(Picked::Lead {
                vendor: Vendor::Codex,
                remember: false
            })
        );
        pick.on_key(KeyCode::Up);
        assert_eq!(
            pick.on_key(KeyCode::Char('p')),
            Some(Picked::Lead {
                vendor: Vendor::Claude,
                remember: true
            })
        );
        assert_eq!(pick.on_key(KeyCode::Esc), Some(Picked::Quit));
        assert_eq!(pick.on_key(KeyCode::Char('q')), Some(Picked::Quit));
    }

    #[test]
    fn the_search_for_vendors_is_said_while_it_runs() {
        let mut t = Terminal::new(TestBackend::new(60, 9)).unwrap();
        t.draw(|f| probing(f, Lang::Tr)).unwrap();
        let buf = t.backend().buffer();
        let rows: Vec<String> = buf
            .content
            .chunks(60)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect();
        assert!(rows[4].contains("kodlama ajanları aranıyor"), "{rows:?}");
        assert!(rows[4].starts_with("          "), "centered: {:?}", rows[4]);
    }

    #[test]
    fn the_question_lists_the_vendors_and_says_whose_primary_is_missing() {
        let mut pick = Pick::new(
            vec![Vendor::Claude, Vendor::Codex],
            Some(Vendor::Gemini),
            Lang::En,
        );
        pick.sel = 1;
        let s = render(&pick, 80, 20);
        for want in [
            "WHO LEADS",
            " 1 claude",
            " 2 codex",
            "gemini is your primary, but not on PATH",
            "p always",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
        pick.lang = Lang::Tr;
        pick.missing = None;
        let s = render(&pick, 80, 20);
        assert!(s.contains("KİM LİDER") && !s.contains("primary"), "{s}");
        // A narrow screen still shows it whole.
        let s = render(&pick, 40, 10);
        assert!(s.contains("codex"), "{s}");
    }
}
