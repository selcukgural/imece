use super::*;

const OPEN_1: &str = "\x1b]8;id=imece-3;http://localhost:1/\x1b\\";

const CLOSE: &str = "\x1b]8;;\x1b\\";

#[test]
fn a_link_cell_opens_and_closes_its_own_link_and_keeps_its_width() {
    let url = "http://localhost:1/";
    let mut buf = Buffer::with_lines(["abcd"]);
    hyperlink(&mut buf, Rect::new(1, 0, 2, 1), url, 3);
    let symbols: Vec<&str> = (0..4).map(|x| buf[(x, 0)].symbol()).collect();
    assert_eq!(
        symbols,
        [
            "a",
            &format!("{OPEN_1}b{CLOSE}"),
            &format!("{OPEN_1}c{CLOSE}"),
            "d"
        ]
    );
    let one = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
    assert_eq!(buf[(1, 0)].diff_option, one);
    assert_eq!(buf[(2, 0)].diff_option, one);
    assert_eq!(buf[(0, 0)].diff_option, CellDiffOption::None);
    // Cut to the buffer.
    let mut buf = Buffer::with_lines(["abcd", "efgh"]);
    hyperlink(&mut buf, Rect::new(3, 1, 5, 2), url, 3);
    assert_eq!(buf[(3, 1)].symbol(), format!("{OPEN_1}h{CLOSE}"));
    assert_eq!(buf[(2, 1)].symbol(), "g");
    assert_eq!(buf[(3, 0)].symbol(), "d");
    // A control character, which could end the sequence, links nothing.
    let mut buf = Buffer::with_lines(["abcd"]);
    let before = buf.clone();
    hyperlink(
        &mut buf,
        Rect::new(0, 0, 4, 1),
        "http://localhost:1/\x07",
        3,
    );
    assert_eq!(buf, before);
}

#[test]
fn web_addresses_shown_whole_are_links_and_no_others() {
    let url = "https://a.b/c";
    let n = url.len() as u16;
    // At 3 on the first row; up to the area's last cell.
    let rows = vec![
        Line::from(vec![Span::raw("ab "), Span::styled(url, link_style())]),
        Line::from(vec![Span::raw("abcd"), Span::styled(url, link_style())]),
        Line::from(vec![Span::raw("x"), Span::styled(url, fg(pal().info))]),
        Line::from(vec![Span::styled("not a url", link_style())]),
        Line::from(vec![Span::styled(url, link_style())]),
    ];
    let area = Rect::new(2, 1, 3 + n, 4);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
    ratatui::widgets::Widget::render(Paragraph::new(rows.clone()), area, &mut buf);
    link_rows(&mut buf, area, &rows);
    assert!(!links_to(&buf, 4, 1, url), "before it");
    assert!(links_to(&buf, 5, 1, url), "its first cell");
    assert!(
        links_to(&buf, 4 + n, 1, url),
        "its last cell, the area's last"
    );
    assert!(!links_to(&buf, 6, 2, url), "one cell past the area: none");
    assert!(!links_to(&buf, 3, 3, url), "not in a link's style");
    assert!(!buf[(2, 4)].symbol().starts_with("\x1b"), "not an address");
    // A row below the area is not drawn there, nor linked.
    assert!(!links_to(&buf, 2, 5, url));
    // Drawn from the area's top row on: the second row on the area's
    // second.
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
    link_rows(&mut buf, Rect::new(0, 1, 40, 2), &rows[3..]);
    assert!(links_to(&buf, 0, 2, url) && links_to(&buf, n - 1, 2, url));
    assert!(!links_to(&buf, n, 2, url));
    assert!(!links_to(&buf, 0, 1, url) && !links_to(&buf, 0, 0, url));
}

#[test]
fn an_address_wider_than_a_row_is_cut_and_never_a_link() {
    let url = "https://a.b/cdef"; // 16 cells
    let line = || Line::from(vec![Span::raw("  "), Span::styled(url, link_style())]);
    // Room for it after the indent: one row, still a link.
    let rows = rows_of(vec![line()], 18);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].spans[1].style, link_style());
    // One cell short: cut, in the link's colour without the underline.
    let rows = rows_of(vec![line()], 17);
    assert!(rows.len() > 1);
    assert!(
        rows.iter()
            .flat_map(|r| &r.spans)
            .all(|s| s.style != link_style())
    );
    assert!(rows[0].spans.iter().any(|s| s.style == fg(pal().info)));
}

#[test]
fn linked_takes_each_address_that_starts_a_word() {
    let base = fg(pal().ok);
    assert_eq!(
        parts(&linked("a http://x.y b https://z.w", base)),
        [
            ("a ".into(), base),
            ("http://x.y".into(), link_style()),
            (" b ".into(), base),
            ("https://z.w".into(), link_style()),
        ]
    );
    assert_eq!(
        parts(&linked("xhttp://x.y httpd http://", base)),
        [("xhttp://x.y httpd http://".into(), base)],
        "inside a word, no address, nothing after the scheme"
    );
    assert_eq!(
        parts(&linked("(http://x.y)", base)),
        [
            ("(".into(), base),
            ("http://x.y".into(), link_style()),
            (")".into(), base),
        ]
    );
    assert_eq!(
        parts(&linked("http://x.y", base)),
        [("http://x.y".into(), link_style())]
    );
    assert_eq!(
        parts(&linked("http://a.b/?u=http://c.d z", base)),
        [
            ("http://a.b/?u=http://c.d".into(), link_style()),
            (" z".into(), base),
        ],
        "an address inside one is part of it"
    );
    assert!(linked("", base).is_empty());
}

#[test]
fn a_link_is_written_in_one_run_and_a_cell_redrawn_alone_closes_its_own() {
    use ratatui::backend::{Backend, CrosstermBackend};
    let url = "http://localhost:1/";
    let written = |prev: &Buffer, next: &Buffer| {
        let mut out = Vec::new();
        CrosstermBackend::new(&mut out)
            .draw(prev.diff(next).into_iter())
            .unwrap();
        String::from_utf8(out).unwrap()
    };
    let mut next = Buffer::with_lines(["abcd"]);
    hyperlink(&mut next, Rect::new(1, 0, 2, 1), url, 3);
    let all = written(&Buffer::empty(next.area), &next);
    // One cursor move: each cell is one column wide, as ratatui counts.
    assert!(
        all.contains(&format!("a{OPEN_1}b{CLOSE}{OPEN_1}c{CLOSE}d")),
        "{all:?}"
    );
    assert_eq!(all.matches('H').count(), 1, "{all:?}");
    let mut later = Buffer::with_lines(["abxd"]);
    hyperlink(&mut later, Rect::new(1, 0, 2, 1), url, 3);
    assert_eq!(next.diff(&later).len(), 1);
    let one = written(&next, &later);
    assert!(one.contains(&format!("{OPEN_1}x{CLOSE}")), "{one:?}");
    assert_eq!(one.matches(CLOSE).count(), 1);
    // Gone again: the plain cells are written over the linked ones.
    let plain = Buffer::with_lines(["abcd"]);
    assert_eq!(later.diff(&plain).len(), 2);
}

#[test]
fn a_worker_s_or_helper_s_loopback_url_is_shown_as_it_is_and_no_one_else_s() {
    let text = |spans: &[Span]| -> String { spans.iter().map(|s| s.content.as_ref()).collect() };
    let agent = |role, url: &str| {
        let mut a = Agent::new(3, 0, "w".into(), role, Status::Idle, 0);
        a.local = Some(url.into());
        a
    };
    let url = "http://localhost:5173/";
    for role in [Role::Worker, Role::Helper, Role::Solo] {
        assert_eq!(salon::linked(&agent(role, url)), Some(url));
        let spans = salon::local_spans(&agent(role, url));
        assert_eq!(text(&spans), "  ↗ localhost:5173/");
        assert_eq!(spans[0].style, dim());
        assert_eq!(
            spans[1].style,
            fg(pal().info).add_modifier(Modifier::UNDERLINED)
        );
    }
    for role in [Role::Leader, Role::Researcher] {
        assert_eq!(salon::linked(&agent(role, url)), None);
        assert!(salon::local_spans(&agent(role, url)).is_empty());
    }
    let mut none = agent(Role::Worker, url);
    none.local = None;
    assert!(salon::local_spans(&none).is_empty());
    // Only `http://` is left out of the text.
    let https = agent(Role::Worker, "https://127.0.0.1:8443/");
    assert_eq!(
        text(&salon::local_spans(&https)),
        "  ↗ https://127.0.0.1:8443/"
    );
    // A path cleaning would hide is not shown: the text is the link.
    let secret = agent(Role::Worker, "http://localhost:1/TOKEN=abc");
    assert_eq!(salon::linked(&secret), None);
    assert!(salon::local_spans(&secret).is_empty());
}
