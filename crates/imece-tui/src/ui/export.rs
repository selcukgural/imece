//! `:export`: what the person has open, a conversation or a card's
//! screen, as a Markdown file of their own. The text is what the screen
//! shows (cleaned: no control characters, secrets hidden), with the day
//! on every time, since a file leaves the screen where "today" means
//! nothing.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::*;
use crate::app::App;

/// Journal lines `entries` as Markdown, in that order: a blank line
/// between blocks, none between list items that follow each other.
fn lines(lang: Lang, w: &World, entries: &[usize]) -> String {
    let mut out = String::new();
    let mut listed = None;
    for &i in entries {
        let (text, list) = entry(lang, w, i);
        if listed.is_some_and(|before| !(before && list)) {
            out.push('\n');
        }
        out.push_str(&text);
        listed = Some(list);
    }
    out
}

/// Journal line `i` as Markdown, and whether it is a list item: a
/// message is a heading (when, who to whom, its kind) over its whole
/// text; anything else is a list item.
fn entry(lang: Lang, w: &World, i: usize) -> (String, bool) {
    let e = &w.journal[i];
    let party = |p: Party| match p {
        Party::Agent(id) => clean(&w.agents[id].name),
        Party::Person => lang.text().you.to_string(),
    };
    match words::said(&e.event) {
        Some(s) => {
            let kind = s
                .kind
                .map(|k| format!(" [{}]", lang.text().kinds[k as usize]))
                .unwrap_or_default();
            let text = format!(
                "### {} {} → {}{kind}\n\n{}\n",
                stamp(e.tick),
                party(s.from),
                party(s.to),
                body_lines(s.text).join("\n")
            );
            (text, false)
        }
        None => {
            let said = words::event(lang, w, &e.event);
            (format!("- {} {said}\n", stamp(e.tick)), true)
        }
    }
}

/// A conversation, journal lines `entries` oldest first, under a heading
/// that names who talked.
pub fn thread(lang: Lang, w: &World, entries: &[usize]) -> String {
    let t = lang.text();
    let who = entries
        .iter()
        .find_map(|&i| words::said(&w.journal[i].event))
        .map(|s| {
            let (a, b) = s.pair();
            let name = |p: Party| match p {
                Party::Agent(id) => clean(&w.agents[id].name),
                Party::Person => t.you.to_string(),
            };
            format!("{} ⇄ {}", name(a), name(b))
        })
        .unwrap_or_default();
    let mut out = format!("# {}\n\n", fill(t.export_thread, &[&who]));
    out.push_str(&lines(lang, w, entries));
    out
}

/// A card's screen: where it stands, its leases, who had a part in it,
/// its story, what waits for the person on it and its journal lines,
/// oldest first.
pub fn card(app: &App, c: CardId) -> String {
    let (lang, w) = (app.lang, &app.world);
    let t = lang.text();
    let card = &w.cards[c];
    let mut out = format!("# #{} {}\n\n", c + 1, clean(&card.title));
    out.push_str(&format!(
        "{} · {}{}\n\n",
        clean(&w.projects[card.project].name),
        t.columns[card::column_index(card.column)],
        fill(t.minutes, &[&card::minutes(w, card).to_string()])
    ));
    if !card.symbols.is_empty() {
        let leases: Vec<String> = (card.symbols.iter())
            .map(|s| clean(&format!("{}#{}", s.file, s.name)))
            .collect();
        out.push_str(&format!("{}{}\n\n", t.lease, leases.join(", ")));
    }
    let people = w.card_people(c);
    out.push_str(&format!("## {}\n\n", t.card_who));
    for &(id, part) in &people {
        let part = t.card_parts[card::part_index(part)];
        out.push_str(&format!("- {}: {}\n", part, clean(&w.agents[id].name)));
    }
    out.push_str(&format!("\n## {}\n\n", t.card_story));
    let me = card.owner.or(people.first().map(|&(id, _)| id));
    match me {
        Some(me) if !card.story.is_empty() => {
            for line in focus::story_lines(app, card, me) {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                // The screen's columns are spaces; one says it here.
                let words: Vec<&str> = text.split_whitespace().collect();
                out.push_str(&format!("- {}\n", words.join(" ")));
            }
        }
        _ => out.push_str(&format!("{}\n", t.card_no_story)),
    }
    let waiting = w.card_inbox(c);
    if !waiting.is_empty() {
        out.push_str(&format!("\n## {}\n\n", t.export_waiting));
        for i in waiting {
            let item = &w.inbox[i];
            let said = words::inbox(lang, w, item);
            out.push_str(&format!("- {} {}\n", stamp(item.tick), said));
        }
    }
    let journal = salon::card_lines(w, c);
    if !journal.is_empty() {
        out.push_str(&format!("\n## {}\n\n", t.export_journal));
        out.push_str(&lines(lang, w, &journal));
    }
    out
}

/// The file's name when the person names none: what it holds and when,
/// `imece-card-12-20261031-1400.md`.
pub fn file_name(what: &str, tick: u64) -> String {
    let when: String = stamp(tick).chars().filter(char::is_ascii_digit).collect();
    let (day, time) = when.split_at(when.len().min(8));
    format!("imece-{what}-{day}-{time}.md")
}

/// Writes `text` to `target`, or to `name` in it when it is a
/// directory, or to `name` in `dir` when there is no target. A file that
/// is there already is never written over.
pub fn write(target: Option<&Path>, dir: &Path, name: &str, text: &str) -> io::Result<PathBuf> {
    let path = match target {
        Some(t) if t.is_dir() => t.join(name),
        Some(t) => t.to_path_buf(),
        None => dir.join(name),
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    Ok(path)
}
