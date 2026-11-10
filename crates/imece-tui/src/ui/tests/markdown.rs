use super::*;

#[test]
fn wrap_cuts_at_spaces_and_keeps_the_lead_on_every_row() {
    let bullet = Line::from(vec![
        Span::raw("  "),
        Span::raw("• "),
        Span::raw("alpha beta gamma"),
    ]);
    assert_eq!(
        text(&wrap(bullet, 12)),
        ["  • alpha ", "    beta ", "    gamma"],
        "a bullet's width, not the bullet, leads the rows after it"
    );
    let code = fg(pal().code);
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled("│ ", dim()),
        Span::styled("cargo test --workspace", code),
    ]);
    let rows = wrap(rule, 15);
    assert_eq!(text(&rows), ["  │ cargo test ", "  │ --workspace"]);
    assert_eq!(rows[1].spans[1].style, dim(), "the rule keeps its style");
    assert_eq!(rows[1].spans[2].style, code, "the code too");
    let short = Line::from(vec![Span::raw("  "), Span::raw("fits")]);
    assert_eq!(text(&wrap(short, 10)), ["  fits"]);
    let numbered = Line::from(vec![Span::raw("3. "), Span::raw("alpha beta")]);
    assert_eq!(
        text(&wrap(numbered, 8)),
        ["3. alpha ", "   beta"],
        "a number with a dot leads"
    );
    let lettered = Line::from(vec![Span::raw("x. "), Span::raw("alpha beta")]);
    assert_eq!(
        text(&wrap(lettered, 8)),
        ["x. alpha ", "beta"],
        "letters with a dot do not, and a word that just fills the row stays on it"
    );
    let dot = Line::from(vec![Span::raw(". "), Span::raw("alpha beta")]);
    assert_eq!(text(&wrap(dot, 8)), [". alpha ", "beta"], "nor a dot alone");
    let exact = Line::from(Span::raw("abcde fghij"));
    assert_eq!(
        text(&wrap(exact, 5)),
        ["abcde ", "fghij"],
        "a word that just fits stays whole; its space may hang over"
    );
}

#[test]
fn wrap_cuts_a_word_wider_than_a_row_by_cell_width() {
    let long = Line::from(vec![Span::raw("  "), Span::raw("abcdefghij end")]);
    assert_eq!(text(&wrap(long, 6)), ["  abcd", "  efgh", "  ij ", "  end"]);
    let wide = Line::from(Span::raw("日本語 çok"));
    assert_eq!(
        text(&wrap(wide, 4)),
        ["日本", "語 ", "çok"],
        "a wide character takes two cells"
    );
    let narrow = Line::from(vec![Span::raw("    "), Span::raw("ab")]);
    assert_eq!(
        text(&wrap(narrow, 2)),
        ["    a", "    b"],
        "a width under the lead still leaves one cell"
    );
    assert_eq!(text(&wrap(Line::raw(""), 10)), [""]);
    assert_eq!(
        text(&wrap(Line::from(Span::raw("日本")), 1)),
        ["日", "本"],
        "a character wider than the row is a row of its own, no empty row first"
    );
    assert_eq!(pieces("日本", 1), ["日", "本"]);
    assert_eq!(pieces("abc", 1), ["a", "b", "c"]);
    assert_eq!(pieces("abc", 2), ["ab", "c"]);
    assert_eq!(pieces("", 3), [""]);
}

#[test]
fn cutting_a_line_ends_it_with_an_ellipsis_where_it_was_cut() {
    let line = Line::from(vec![Span::raw("ab"), Span::styled("cdef", dim())]);
    let (same, cut) = overlay::cut(line.clone(), 6);
    assert_eq!((same, cut), (line.clone(), false), "it fits exactly");
    let (short, cut) = overlay::cut(line.clone(), 5);
    assert!(cut);
    assert_eq!(text(std::slice::from_ref(&short)), ["abcd…"]);
    assert_eq!(short.spans.len(), 3, "{:?}", short.spans);
    let (short, _) = overlay::cut(line.clone(), 3);
    assert_eq!(text(std::slice::from_ref(&short)), ["ab…"]);
    assert_eq!(short.spans.len(), 2, "no empty span after the cut");
    // A wide character that does not fit is left out whole.
    let (short, _) = overlay::cut(Line::raw("a界bc"), 3);
    assert_eq!(text(&[short]), ["a…"]);
    let (short, _) = overlay::cut(Line::raw("a界bc"), 4);
    assert_eq!(text(&[short]), ["a界…"]);
    // A cut after a blank drops it.
    let (short, _) = overlay::cut(Line::raw("ab cd"), 4);
    assert_eq!(text(&[short]), ["ab…"]);
}

#[test]
fn brief_puts_words_on_one_line_and_cuts_them() {
    assert_eq!(brief("a  b\n c", 10), "a b c");
    assert_eq!(brief("abcde", 5), "abcde", "exactly as long");
    assert_eq!(brief("abcdef", 5), "abcd…");
}

#[test]
fn inline_marks_code_bold_italic_and_links_and_leaves_lone_marks_alone() {
    let base = Style::new();
    let code = fg(pal().code);
    let bold_ = base.add_modifier(Modifier::BOLD);
    let italic = base.add_modifier(Modifier::ITALIC);
    let under = base.add_modifier(Modifier::UNDERLINED);
    assert_eq!(
        parts(&inline("a **b** `c **d**` *e* [f](http://x) g", base)),
        [
            ("a ".into(), base),
            ("b".into(), bold_),
            (" ".into(), base),
            ("c **d**".into(), code),
            (" ".into(), base),
            ("e".into(), italic),
            (" ".into(), base),
            ("f".into(), under),
            (" (".into(), dim()),
            ("http://x".into(), link_style()),
            (")".into(), dim()),
            (" g".into(), base),
        ]
    );
    // A link without its words is text, its address still an address.
    assert_eq!(
        parts(&inline("[](http://x)", base)),
        [
            ("[](".into(), base),
            ("http://x".into(), link_style()),
            (")".into(), base),
        ]
    );
    // A link's address that is no web address stays dim.
    assert_eq!(
        parts(&inline("[f](ftp://x)", base)),
        [
            ("f".into(), under),
            (" (".into(), dim()),
            ("ftp://x".into(), dim()),
            (")".into(), dim()),
        ]
    );
    // A bare address is a span of its own; in code it is code.
    assert_eq!(
        parts(&inline("see https://a.b/c. `http://d.e`", base)),
        [
            ("see ".into(), base),
            ("https://a.b/c".into(), link_style()),
            (". ".into(), base),
            ("http://d.e".into(), code),
        ]
    );
    for plain in [
        "**x",
        "x**",
        "****",
        "a * b * c",
        "snake_case_name",
        "`open",
        "[x](",
        "[x]()",
        "*",
        "* x",
        "*x**",
    ] {
        assert_eq!(
            parts(&inline(plain, base)),
            [(plain.into(), base)],
            "{plain}"
        );
    }
    assert_eq!(
        parts(&inline("çok **önemli**", bold_)),
        [("çok ".into(), bold_), ("önemli".into(), bold_)],
        "a base style carries into the marks"
    );
    assert!(inline("", base).is_empty());
    assert_eq!(
        parts(&inline("[the docs](http://x/y) tail", base)),
        [
            ("the docs".into(), under),
            (" (".into(), dim()),
            ("http://x/y".into(), link_style()),
            (")".into(), dim()),
            (" tail".into(), base),
        ],
        "a label longer than one character"
    );
}

#[test]
fn a_table_becomes_aligned_columns_with_the_header_bold() {
    let md = "before
| Command | Result |
|---|---|
| `cargo test` | 171 passed |
| short |

after | not a table";
    let lines = markdown(md, "  ");
    let shown = text(&lines);
    assert_eq!(
        shown,
        [
            "  before",
            "  Command    │ Result",
            "  ───────────┼───────────",
            "  cargo test │ 171 passed",
            "  short      │ ",
            "  ",
            "  after | not a table",
        ]
    );
    assert_eq!(lines[1].spans[1].style, bold(pal().strong));
    assert_eq!(
        lines[3].spans[1].style,
        fg(pal().code),
        "cells take inline marks"
    );
    assert_eq!(lines[2].spans[1].style, dim());
    let rows: Vec<String> = ["a|b", "-|-", "c|d", "", "e|f"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(table_len(&rows, 0), 3, "a blank line ends the table");
    let rows: Vec<String> = ["a|b", "-|-", "c|d", "prose", "e|f"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(table_len(&rows, 0), 3, "a line without a column too");
    assert_eq!(table_len(&rows, 4), 0, "no separator, no table");
    assert_eq!(table_len(&rows, 2), 0);
    assert!(is_separator("| :---: | --- |"));
    assert!(!is_separator("---"), "a rule has no column");
    assert!(!is_separator("| | |"));
    assert_eq!(cells("| a | b |"), ["a", "b"]);
    assert_eq!(cells("a|b"), ["a", "b"]);
    assert_eq!(cells("|"), [""]);
}

#[test]
fn a_table_s_columns_give_way_widest_first_to_fit() {
    // They fit, the equal case too: as they are.
    assert_eq!(column_widths(&[3, 10, 5], 18), [3, 10, 5]);
    assert_eq!(column_widths(&[3, 10, 5], 100), [3, 10, 5]);
    // One cell short: the widest gives one.
    assert_eq!(column_widths(&[3, 10, 5], 17), [3, 9, 5]);
    // The widest down to the next, then both.
    assert_eq!(column_widths(&[3, 10, 5], 13), [3, 5, 5]);
    assert_eq!(column_widths(&[3, 10, 5], 11), [3, 4, 4]);
    assert_eq!(column_widths(&[2, 20, 30], 26), [2, 12, 12]);
    // No room: a cell each, an empty column too.
    assert_eq!(column_widths(&[3, 10, 0], 2), [1, 1, 1]);
    assert_eq!(column_widths(&[], 0), Vec::<usize>::new());
}

#[test]
fn a_table_wider_than_its_room_wraps_its_cells_in_their_columns() {
    use unicode_width::UnicodeWidthStr;
    let line = |l: &Line| {
        l.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    };
    let md = "| # | What | Priority |
|---|---|---|
| #28 | Config file and a baseline run on both | high |
| #29 | see https://example.com/a/long/path/here | medium |";
    // Room for it all: one line a row, copied as written.
    let wide = markdown_src(md, "  ", 200);
    assert_eq!(wide.len(), 4);
    assert_eq!(
        wide[1].1, "|---|---|---|",
        "the rule copies as the separator"
    );
    assert_eq!(
        wide[2].1,
        "| #28 | Config file and a baseline run on both | high |"
    );
    // 40 cells, the indent's 2 and the gaps' 6 counted: 32 for the
    // cells, the narrow columns kept (3 + 8), the widest the 21 left,
    // wrapped in its own.
    let lines = markdown_src(md, "  ", 40);
    let shown: Vec<String> = lines.iter().map(|(l, _)| line(l)).collect();
    assert_eq!(
        shown,
        [
            "  #   │ What                  │ Priority",
            "  ────┼───────────────────────┼─────────",
            "  #28 │ Config file and a     │ high",
            "      │ baseline run on both  │ ",
            "  #29 │ see                   │ medium",
            "      │ https://example.com/a │ ",
            "      │ /long/path/here       │ ",
        ]
    );
    assert_eq!(shown[0].width(), 40, "just full");
    // One cell less: the widest gives it.
    let narrower = markdown_src(md, "  ", 39);
    assert_eq!(
        line(&narrower[0].0),
        "  #   │ What                 │ Priority"
    );
    // Copied as read, so a wrapped row copies its lines, not twice.
    assert_eq!(lines[3].1, "│ baseline run on both  │");
    assert_eq!(lines[1].1, "────┼───────────────────────┼─────────");
    assert_eq!(lines[2].1, "#28 │ Config file and a     │ high");
    // The header still bold, the address too long for its column no
    // link: a link of part of it would open somewhere else.
    assert_eq!(lines[0].0.spans[1].style, bold(pal().strong));
    let parts: Vec<Style> = (lines[5..7].iter())
        .flat_map(|(l, _)| l.spans.iter())
        .filter(|s| s.content.contains("example") || s.content.contains("/long"))
        .map(|s| s.style)
        .collect();
    assert_eq!(parts, [fg(pal().info), fg(pal().info)]);
    // An address that fits its column stays a link.
    let md = "| a | b |\n|---|---|\n| https://a.b | x y z w v u t |";
    let lines = markdown_src(md, "", 26);
    let link = (lines.iter())
        .flat_map(|(l, _)| l.spans.iter())
        .find(|s| s.content == "https://a.b")
        .map(|s| s.style);
    assert_eq!(link, Some(link_style()));
    assert_eq!(line(&lines[2].0), "https://a.b │ x y z w v u ");
    assert_eq!(line(&lines[3].0), "            │ t");
}

#[test]
fn lists_quotes_and_rules_draw_and_wrap_with_their_marks() {
    let md = "1. one
2. two
> said
>quoted
---
+ plus
  * nested
10. ten";
    assert_eq!(
        text(&markdown(md, "  ")),
        [
            "  1. one",
            "  2. two",
            "  ▎ said",
            "  ▎ quoted",
            &format!("  {}", "─".repeat(24)),
            "  • plus",
            "    • nested",
            "  10. ten",
        ]
    );
    let numbered = markdown("1. alpha beta gamma", "  ").remove(0);
    assert_eq!(
        text(&wrap(numbered, 12)),
        ["  1. alpha ", "     beta ", "     gamma"],
        "the number's width leads the rows after it"
    );
    let quoted = markdown("> alpha beta gamma", "  ").remove(0);
    assert_eq!(
        text(&wrap(quoted, 12)),
        ["  ▎ alpha ", "  ▎ beta ", "  ▎ gamma"],
        "the quote bar goes on"
    );
    assert_eq!(list_marker("- a"), Some(("• ".into(), "a")));
    assert_eq!(list_marker("3. a"), Some(("3. ".into(), "a")));
    assert_eq!(list_marker("3.a"), None);
    assert_eq!(list_marker("-a"), None);
    assert_eq!(list_marker(". a"), None);
}

#[test]
fn plain_makes_one_row_of_words_out_of_an_agents_markdown() {
    assert_eq!(
        plain("## Head\n\n- **bold** `code` *it* [the docs](http://u)\n2. two\n   \n"),
        "Head bold code it the docs two"
    );
    assert_eq!(
        plain("**x\n\u{1b}[2Jy"),
        "**x [2Jy",
        "a lone mark and a control code"
    );
    assert_eq!(plain("* bullet\n+ plus\n# \n"), "bullet plus");
    assert_eq!(plain(""), "");
    assert_eq!(plain("\n\n"), "");
}
