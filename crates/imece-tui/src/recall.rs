//! Recall (`:recall`, `H`): every journal line and trace line imece keeps,
//! of every project and archive, searched by words (record 93).
//!
//! Words are folded as the help filter folds them (`ui::fold`: case and
//! the Turkish letters), cut into terms at every character that is not a
//! letter or a digit, and a line has them when each term starts one of
//! its words: `deploy fix` finds `fixed the deploy script`. Live searches
//! the store's full-text index (`store::Store::recall`), which also holds
//! the lines an earlier run or an archive left; the demo, and live
//! without a store, search what the world holds (`in_world`).

use crate::model::{AgentId, Entry, Event, ProjectId, World};
use crate::ui::fold;

/// Lines a search shows at most, the newest.
pub const SHOWN: usize = 50;

/// A line a search found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// When it was written (a tick of the world); none for a trace line
    /// the world holds in memory, which keeps no time beside its clock.
    pub tick: Option<u64>,
    pub agent: AgentId,
    pub project: ProjectId,
    pub line: Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A journal line, and where it is in the world's journal when it is
    /// there (an old line a load did not bring is not).
    Journal { entry: Entry, at: Option<usize> },
    /// A line of the agent's trace, as written (its clock first).
    Trace(String),
}

/// The terms of what was typed: folded, cut at every character that is
/// neither a letter nor a digit, empty ones left out.
pub fn terms(words: &str) -> Vec<String> {
    fold(words)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// The full-text query of `terms`: each one a quoted prefix, all of them
/// needed. Terms hold only letters and digits, so nothing in them is the
/// query language's; the quotes are doubled all the same.
pub fn query(terms: &[String]) -> String {
    let quoted: Vec<String> = terms
        .iter()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    quoted.join(" ")
}

/// Whether `text` has every one of `terms` at the start of one of its
/// words, folded as the terms are.
pub fn has(text: &str, terms: &[String]) -> bool {
    let hay = fold(text);
    let words: Vec<&str> = hay
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    terms
        .iter()
        .all(|t| words.iter().any(|w| w.starts_with(t.as_str())))
}

/// What of a journal line is searched: the name of the agent it is
/// about (`name`) and every text the event carries (a message, a topic, a
/// command, a path, a reason), folded. Ids and numbers the event holds
/// are not words of it.
pub fn words_of(name: &str, e: &Event) -> String {
    let mut out = vec![name.to_string()];
    strings(&serde_json::to_value(e).unwrap_or_default(), &mut out);
    fold(&out.join(" "))
}

/// What of a trace line is searched: its agent's name and the line.
pub fn trace_words(name: &str, line: &str) -> String {
    fold(&format!("{name} {line}"))
}

/// The name of agent `id` in `w`, empty for none.
fn name(w: &World, id: AgentId) -> &str {
    w.agents.get(id).map_or("", |a| a.name.as_str())
}

/// The strings in `v`, the keys of its objects left out (they are the
/// event's field names).
fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
        serde_json::Value::Object(m) => m.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

/// The lines of `w` that have every one of the terms of `words`, the
/// newest first and at most `cap`: journal lines from the newest back,
/// then each agent's trace from its newest back. Nothing for no terms.
pub fn in_world(w: &World, words: &str, cap: usize) -> Vec<Hit> {
    let terms = terms(words);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, e) in w.journal.iter().enumerate().rev() {
        if hits.len() == cap {
            return hits;
        }
        if has(&words_of(name(w, e.event.agent()), &e.event), &terms) {
            let agent = e.event.agent();
            hits.push(Hit {
                tick: Some(e.tick),
                agent,
                project: w.agents[agent].project,
                line: Line::Journal {
                    entry: e.clone(),
                    at: Some(i),
                },
            });
        }
    }
    for a in &w.agents {
        for line in a.trace.iter().rev() {
            if hits.len() == cap {
                return hits;
            }
            if has(&trace_words(&a.name, line), &terms) {
                hits.push(Hit {
                    tick: None,
                    agent: a.id,
                    project: a.project,
                    line: Line::Trace(line.clone()),
                });
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MsgKind;

    #[test]
    fn terms_are_folded_and_cut_at_what_is_no_letter_or_digit() {
        assert_eq!(terms("  Çalış-ma  İŞ_2 "), ["calis", "ma", "is", "2"]);
        assert_eq!(terms("\"*:() -"), Vec::<String>::new());
        assert_eq!(query(&terms("Fix the dé")), "\"fix\"* \"the\"* \"dé\"*");
        assert_eq!(query(&["a\"b".to_string()]), "\"a\"\"b\"*");
        assert_eq!(query(&[]), "");
    }

    #[test]
    fn a_line_has_the_terms_that_start_its_words_every_one_of_them() {
        let t = terms("dep FIX");
        assert!(has("Fixed the deploy script", &t));
        assert!(!has("Fixed the script", &t), "every term");
        assert!(!has("redeploy fixed", &t), "the start of a word");
        assert!(has("ŞİMDİ", &terms("simdi")), "folded as the terms are");
        assert!(has("x-fix deploy", &t), "words cut at what is no letter");
        assert!(has("anything", &[]), "no terms: every line");
    }

    /// Agent 0 leads project 0 with berk (1); ayşe (2) is project 1's.
    fn world() -> World {
        let (mut w, _) = crate::sim::world(2, 1, 0, 1, crate::i18n::Lang::En);
        w.journal.clear();
        w.agents.truncate(3);
        for (id, p, name) in [(0, 0, "lead"), (1, 0, "berk"), (2, 1, "ayşe")] {
            let a = &mut w.agents[id];
            (a.project, a.name) = (p, name.to_string());
            a.trace.clear();
        }
        w
    }

    #[test]
    fn the_events_texts_and_its_agents_name_are_its_words_folded() {
        let w = world();
        let e = Event::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Ask,
            text: "Where is the Şema?".into(),
            id: 7,
            re: Some(3),
        };
        let words = words_of(name(&w, e.agent()), &e);
        assert!(words.starts_with("berk "), "{words}");
        assert!(words.contains("where is the sema?"), "{words}");
        assert!(!words.contains('7') && !words.contains("from"), "{words}");
        let e = Event::Stopped {
            agent: 2,
            why: "Kota".into(),
        };
        assert_eq!(words_of(name(&w, 2), &e), "ayse kota");
        assert_eq!(name(&w, 9), "", "no such agent");
        let e = Event::Standing {
            leader: 0,
            rules: vec!["cargo test".into(), "Git LOG".into()],
            added: true,
        };
        assert_eq!(words_of(name(&w, 0), &e), "lead cargo test git log");
        assert_eq!(trace_words("Ayşe", "12:00:01 RUN"), "ayse 12:00:01 run");
    }

    #[test]
    fn the_world_is_searched_newest_first_journal_then_traces_up_to_the_cap() {
        let mut w = world();
        let talk = |from, text: &str| Event::Talk {
            from,
            to: 0,
            text: text.into(),
        };
        w.journal.push(Entry {
            tick: 3,
            event: talk(1, "deploy the site"),
        });
        w.journal.push(Entry {
            tick: 5,
            event: talk(2, "nothing here"),
        });
        w.journal.push(Entry {
            tick: 9,
            event: talk(2, "Deploy again"),
        });
        w.agents[1].trace(4, "run: deploy.sh");
        w.agents[2].trace(6, "deployed");
        let hits = in_world(&w, "deplo", 10);
        let at: Vec<(Option<u64>, AgentId, ProjectId)> =
            hits.iter().map(|h| (h.tick, h.agent, h.project)).collect();
        assert_eq!(
            at,
            [(Some(9), 2, 1), (Some(3), 1, 0), (None, 1, 0), (None, 2, 1)]
        );
        assert_eq!(
            hits[0].line,
            Line::Journal {
                entry: w.journal[2].clone(),
                at: Some(2)
            }
        );
        assert!(matches!(&hits[2].line, Line::Trace(l) if l.ends_with("run: deploy.sh")));
        assert_eq!(in_world(&w, "deplo", 2).len(), 2, "the cap");
        assert_eq!(in_world(&w, "deplo", 3).len(), 3, "the cap in the traces");
        assert_eq!(in_world(&w, "berk", 10).len(), 2, "by the agent's name");
        assert_eq!(in_world(&w, " - ", 10), [], "no terms, no lines");
        assert_eq!(in_world(&w, "deploy nothing", 10), []);
    }
}
