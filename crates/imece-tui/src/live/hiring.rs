//! Hiring: the vendors a team may use, their tier ladders, `hire`,
//! and the fall-back to a default model.

use super::*;

impl Ladder {
    pub(super) fn model(&self, tier: Tier) -> Option<&str> {
        match tier {
            Tier::Cheap => &self.cheap,
            Tier::Standard => &self.standard,
            Tier::Strong => &self.strong,
        }
        .as_deref()
    }

    pub(super) fn set(&mut self, tier: Tier, model: String) {
        *match tier {
            Tier::Cheap => &mut self.cheap,
            Tier::Standard => &mut self.standard,
            Tier::Strong => &mut self.strong,
        } = Some(model);
    }
}

impl Hireable {
    /// `vendor` at `program`, on its own ladder, with the person's
    /// stand-ins `tiers` (for this vendor) on top.
    pub(super) fn new(vendor: Vendor, program: &str, tiers: &[(Tier, String)]) -> Hireable {
        let own = |tier| {
            match vendor {
                Vendor::Codex => codex::model(tier),
                Vendor::Gemini => gemini::model(tier),
                _ => claude::model(tier),
            }
            .map(String::from)
        };
        let mut ladder = Ladder {
            cheap: own(Tier::Cheap),
            standard: own(Tier::Standard),
            strong: own(Tier::Strong),
        };
        for (tier, model) in tiers {
            ladder.set(*tier, model.clone());
        }
        Hireable {
            vendor,
            program: program.to_string(),
            ladder,
        }
    }

    /// Claude Code at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    pub(super) fn claude(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Claude, program, tiers)
    }

    /// Codex at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    pub(super) fn codex(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Codex, program, tiers)
    }

    /// Gemini CLI at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    pub(super) fn gemini(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Gemini, program, tiers)
    }
}

/// The hireable `vendors`, each at its program from [`KNOWN`] on its own
/// ladder, with the person's stand-ins `tiers` on top: the ones named for
/// the vendor, and the bare ones for the leader's vendor `lead`.
pub(super) fn team_vendors(
    vendors: Vec<Vendor>,
    lead: Vendor,
    tiers: &[(Option<Vendor>, Tier, String)],
) -> Vec<Hireable> {
    vendors
        .into_iter()
        .map(|v| {
            let program = KNOWN.iter().find(|k| k.0 == v).map_or("", |k| k.1);
            let own: Vec<(Tier, String)> = tiers
                .iter()
                .filter(|(for_, _, _)| for_.map_or(v == lead, |f| f == v))
                .map(|(_, t, m)| (*t, m.clone()))
                .collect();
            Hireable::new(v, program, &own)
        })
        .collect()
}

/// Whether `program` runs from `repo` with a cleared environment.
pub(super) fn on_path(program: &str, repo: &Path) -> bool {
    child::command(program, repo, &[])
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Which of the vendors `present` on PATH (with whether imece drives them)
/// may be hired, given the leader must be `lead`; or why the team can not
/// start.
pub(super) fn hireable(present: &[(Vendor, bool)], lead: Vendor) -> Result<Vec<Vendor>, String> {
    let names = |v: &[(Vendor, bool)]| {
        v.iter()
            .map(|(v, _)| v.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if present.is_empty() {
        let known: Vec<&str> = KNOWN.iter().map(|k| k.1).collect();
        return Err(format!(
            "no coding agent on PATH (looked for {}); install Claude Code \
             (https://claude.com/claude-code) and log in",
            known.join(", ")
        ));
    }
    match present.iter().find(|(v, _)| *v == lead) {
        None => Err(format!(
            "{} is not on PATH (found {}); install it, or pick the leader with --lead",
            lead.label(),
            names(present)
        )),
        Some((_, false)) => {
            let could: Vec<&str> = present
                .iter()
                .filter(|(_, drives)| *drives)
                .map(|(v, _)| v.label())
                .collect();
            Err(format!(
                "{} is on PATH, but imece can not drive it yet; start with --lead {}",
                lead.label(),
                if could.is_empty() {
                    "claude".to_string()
                } else {
                    could.join(" or --lead ")
                }
            ))
        }
        Some((_, true)) => Ok(present
            .iter()
            .filter(|(_, drives)| *drives)
            .map(|(v, _)| *v)
            .collect()),
    }
}

impl Live {
    /// Makes room for one more agent, on `model` (`tiered`: picked by
    /// tier, so it may fall back).
    pub(super) fn grow(&mut self, model: Option<String>, tiered: bool) {
        self.remembered.push(false);
        self.interrupted.push(Vec::new());
        self.sessions.push(None);
        self.floor.push(0);
        self.resuming.push(None);
        self.slots.push(Slot::None);
        self.generation.push(0);
        self.spent_before.push(0);
        self.models.push(model);
        self.tiered.push(tiered);
        self.turned.push(false);
        self.inflight.push(Vec::new());
        self.renewed.push(false);
        self.leaving.push(false);
        self.notes.push(Vec::new());
        self.mail.push(Vec::new());
        self.turns.push(0);
        self.asked.push(false);
        self.woken.push(0);
        self.posted.push(0);
        self.cwds.push(None);
        self.running.push(run::Running::default());
        self.pending.push(Vec::new());
        self.reading.push(0);
        self.applying.push(false);
        self.placing.push(None);
    }

    /// The vendor `v` as imece drives it. Every agent's vendor is one.
    pub(super) fn of(&self, v: Vendor) -> &Hireable {
        self.vendors
            .iter()
            .find(|h| h.vendor == v)
            .unwrap_or_else(|| panic!("{} is not a vendor imece drives", v.label()))
    }

    /// The vendors' names, for the hire tool.
    pub(super) fn vendor_names(&self) -> Vec<&str> {
        self.vendors.iter().map(|h| h.vendor.label()).collect()
    }

    /// Agent `id`'s tier model is not available here: its session ends and
    /// a new one on its vendor's standard model gets the prompts the old
    /// one had.
    pub(super) fn fall_back(&mut self, w: &mut World, id: AgentId) {
        let model = self.models[id].take().unwrap_or_default();
        self.models[id] = self.of(w.agents[id].vendor).ladder.standard.clone();
        self.tiered[id] = false;
        let texts = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.forget(w, id, false, true);
        w.agents[id].trace(
            w.tick,
            format!("model {model} not available here; falling back to the default"),
        );
        w.note(Event::Fallback { agent: id, model });
        for text in texts {
            if let Err(why) = self.prompt(w, id, text, None) {
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// Agent `id`'s session hit its cap, but the budget has grown since the
    /// session started: a new session, capped at what is left now, gets the
    /// prompts the old one had.
    pub(super) fn renew(&mut self, w: &mut World, id: AgentId) {
        let texts = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.forget(w, id, false, true);
        self.renewed[id] = true;
        w.agents[id].trace(
            w.tick,
            "session hit its cap; a new one takes what is left of the budget".to_string(),
        );
        for text in texts {
            if let Err(why) = self.prompt(w, id, text, None) {
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// The leader `from` adds a worker, or the worker `from` adds a helper
    /// (the `hire` tool's arguments). The answer names it.
    pub(super) fn hire(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let hirer = &w.agents[from];
        let (project, own) = (hirer.project, hirer.vendor);
        let role = match hirer.role {
            Role::Leader => Role::Worker,
            Role::Worker => Role::Helper,
            _ => return Err("only the leader and workers hire; ask yours with send".into()),
        };
        // How many there are and how many there may be.
        let (so_far, cap) = if role == Role::Worker {
            let workers = w
                .agents
                .iter()
                .filter(|o| o.project == project && o.role == Role::Worker && !o.gone)
                .count();
            if self.max_workers == 0 {
                return Err(
                    "the person runs you without workers (--agents 0); answer from what you read"
                        .into(),
                );
            }
            if workers >= self.max_workers {
                return Err(format!(
                    "the team is full: {workers} workers, the most the person allowed \
                     (--agents {}); give the work to one of them",
                    self.max_workers
                ));
            }
            (workers, self.max_workers)
        } else {
            let helpers = w.helpers_of(from).count();
            if self.max_helpers == 0 {
                return Err(
                    "the person runs you without helpers (--helpers 0); do it yourself".into(),
                );
            }
            if helpers >= self.max_helpers {
                return Err(format!(
                    "you have {helpers} helpers, the most the person allowed (--helpers {}); \
                     wait for one to send done",
                    self.max_helpers
                ));
            }
            (helpers, self.max_helpers)
        };
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let vendor = match arg("vendor") {
            "" => self.of(own).clone(),
            v => self
                .vendors
                .iter()
                .find(|h| h.vendor.label() == v)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "there is no vendor {v:?} here; you can hire {}",
                        self.vendor_names().join(", ")
                    )
                })?,
        };
        let tier = match arg("tier") {
            "" => Tier::Standard,
            t => Tier::from_wire(t).ok_or("tier must be cheap, standard or strong")?,
        };
        let model = vendor.ladder.model(tier).map(str::to_string);
        // Only a model other than the standard one has something to fall
        // back to.
        let tiered = model != vendor.ladder.standard;
        // The next number, skipping names already taken: a worker's after
        // its vendor (`claude-2`), a helper's after its worker
        // (`claude-2.1`), counting the ones that left.
        let (stem, next) = if role == Role::Worker {
            let n = w
                .agents
                .iter()
                .filter(|o| o.vendor == vendor.vendor && o.role == Role::Worker)
                .count();
            (format!("{}-", vendor.vendor.label()), n + 1)
        } else {
            let n = w
                .agents
                .iter()
                .filter(|o| o.role == Role::Helper && o.parent == Some(from))
                .count();
            (format!("{}.", w.agents[from].name), n + 1)
        };
        let name = (next..)
            .map(|n| format!("{stem}{n}"))
            .find(|name| w.agent_by_name(name).is_none())
            .expect("some number is free");
        let id = w.agents.len();
        // The socket first: an agent without one could never start.
        self.bind(id, role, vendor.vendor)
            .map_err(|e| format!("{name} could not get a socket: {e}"))?;
        self.grow(model.clone(), tiered);
        let t = w.tick;
        let mut agent = Agent::new(
            id,
            project,
            name.clone(),
            vendor.vendor,
            role,
            Status::Idle,
            t,
        );
        agent.parent = (role == Role::Helper).then_some(from);
        w.agents.push(agent);
        let shown = model.clone().unwrap_or_else(|| "the default model".into());
        let by = w.agents[from].name.clone();
        w.agents[from].trace(t, format!("hire {name} {} {shown}", tier.wire()));
        w.agents[id].trace(t, format!("hired by {by} · {shown}"));
        w.note(Event::Hired {
            by: from,
            agent: id,
            model,
        });
        Ok(format!(
            "{name} joined ({}, {shown}). Give it work with send (type assign). \
             {} more may be hired.",
            vendor.vendor.label(),
            cap - so_far - 1
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn a_new_worker_s_name_skips_the_ones_taken() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        assert!(got.starts_with("claude-3 joined (claude, opus)."), "{got}");
        // A name that is not the count: the count is skipped over it.
        w.agents[3].name = "claude-4".into();
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(got.starts_with("claude-5 joined"), "{got}");
        assert_eq!(w.agent_by_name("claude-5"), Some(4));
    }

    #[test]
    fn bad_hires_are_refused_with_a_reason() {
        let mut w = team(1);
        let helper = with_helper(&mut w, 1);
        let mut live = idle_live(3);
        let err = live.hire(&mut w, helper, &json!({})).unwrap_err();
        assert!(err.contains("only the leader and workers"), "{err}");
        live.max_helpers = 0;
        let err = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(err.contains("without helpers"), "{err}");
        live.max_helpers = usize::MAX;
        let err = live
            .hire(&mut w, 0, &json!({ "tier": "huge" }))
            .unwrap_err();
        assert!(err.contains("cheap, standard or strong"), "{err}");
        let err = live
            .hire(&mut w, 0, &json!({ "vendor": "codex" }))
            .unwrap_err();
        assert!(
            err.contains("no vendor \"codex\"") && err.contains("hire claude"),
            "{err}"
        );
        live.max_workers = 0;
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.contains("without workers"), "{err}");
        assert_eq!(w.agents.len(), 3);
        assert!(w.journal.is_empty());
    }

    #[test]
    fn a_worker_hires_helpers_named_after_it_up_to_its_cap_and_they_hire_nobody() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.max_workers = 2;
        live.max_helpers = 2;
        let got = live.hire(&mut w, 1, &json!({ "tier": "cheap" })).unwrap();
        assert!(
            got.starts_with("claude-1.1 joined (claude, haiku)")
                && got.contains("1 more may be hired"),
            "{got}"
        );
        let h = &w.agents[3];
        assert_eq!(
            (h.name.as_str(), h.role, h.parent, h.project, h.card),
            ("claude-1.1", Role::Helper, Some(1), 0, None)
        );
        assert_eq!(live.models[3].as_deref(), Some("haiku"));
        assert!(matches!(
            w.journal.last().map(|e| &e.event),
            Some(Event::Hired { by: 1, agent: 3, model: Some(m) }) if m == "haiku"
        ));
        let got = live.hire(&mut w, 1, &json!({})).unwrap();
        assert!(
            got.starts_with("claude-1.2 joined") && got.contains("0 more"),
            "{got}"
        );
        // Exactly the cap, per worker: the other worker still may.
        let err = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(
            err.contains("you have 2 helpers") && err.contains("--helpers 2"),
            "{err}"
        );
        let got = live.hire(&mut w, 2, &json!({})).unwrap();
        assert!(got.starts_with("claude-2.1 joined"), "{got}");
        // Helpers are not workers: the leader's cap is untouched by them,
        // and they are not offered cards.
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.contains("the team is full: 2 workers"), "{err}");
        assert_eq!(w.helpers_of(1).count(), 2);
        assert_eq!(w.helpers_of(2).count(), 1);
        assert_eq!(w.helpers_of(0).count(), 0);
        let err = live.hire(&mut w, 3, &json!({})).unwrap_err();
        assert!(err.contains("only the leader and workers"), "{err}");
    }

    #[test]
    fn a_helper_that_sent_done_to_its_worker_leaves_when_its_turn_ends() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.hire(&mut w, 1, &json!({})).unwrap();
        // The worker is busy, so the mail waits and nothing starts.
        live.turns[1] = 1;
        live.turns[2] = 1;
        // done to someone else is a message like any other.
        sent(&mut live, &mut w, 2, &msg("lead", "done", "fyi")).unwrap();
        assert!(!live.leaving[2]);
        let got = sent(
            &mut live,
            &mut w,
            2,
            &msg("claude-1", "done", "Tests written."),
        )
        .unwrap();
        assert!(got.starts_with("queued; claude-1 is busy"), "{got}");
        assert!(live.leaving[2], "leaves when the turn ends, not now");
        assert!(!w.agents[2].gone);
        // Mail that came for it meanwhile goes nowhere.
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Inform,
            body: "x".into(),
        });
        finish(&mut live, &mut w, 2, "done");
        let h = &w.agents[2];
        assert!(h.gone && h.status == Status::Idle && h.talk.is_none());
        assert!(!live.leaving[2]);
        assert!(live.mail[2].is_empty());
        assert_eq!(live.generation[2], 1, "its session ended");
        assert!(
            w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Left { agent: 2 }))
        );
        assert!(
            h.trace.iter().any(|l| l.ends_with(" left")),
            "{:?}",
            h.trace
        );
        assert_eq!(w.agent_by_name("claude-1.1"), None);
        // A gone helper takes no mail and no turn.
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Assign,
            body: "more".into(),
        });
        live.step(&mut w);
        assert_eq!(live.turns[2], 0);
        assert!(matches!(live.slots[2], Slot::None));
        // The worker still got the done, and the next helper is numbered on.
        assert_eq!(live.mail[1].len(), 1);
        assert_eq!(live.mail[1][0].kind, MsgKind::Done);
        let got = live.hire(&mut w, 1, &json!({})).unwrap();
        assert!(got.starts_with("claude-1.2 joined"), "{got}");
    }

    #[test]
    fn a_worker_that_stops_takes_its_helpers_with_it() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.hire(&mut w, 1, &json!({})).unwrap();
        live.hire(&mut w, 2, &json!({})).unwrap();
        live.tx
            .send((1, 0, AgentEvent::Stopped("gone".into())))
            .unwrap();
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert!(w.agents[3].gone, "claude-1's helper");
        assert!(!w.agents[4].gone, "claude-2's helper stays");
        assert!(
            w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Left { agent: 3 }))
        );
    }

    #[test]
    fn a_hired_worker_gets_a_socket_with_the_worker_s_tools() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let mut w = team(0);
        let mut live = idle_live(1);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        live.hire(&mut w, 0, &json!({})).unwrap();
        let path = live.bus.as_ref().unwrap().servers[1].path().to_path_buf();
        assert!(path.ends_with("a1.sock"), "{}", path.display());
        let mut sock = UnixStream::connect(path).unwrap();
        writeln!(sock, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).unwrap();
        let mut line = String::new();
        BufReader::new(&sock).read_line(&mut line).unwrap();
        let listed: Value = serde_json::from_str(&line).unwrap();
        let names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["send", "post", "board", "run", "claim", "hire", "approve"]
        );
    }

    #[test]
    fn a_tier_model_that_fails_its_first_turn_gives_way_to_the_standard_one() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.vendors[0].ladder.standard = Some("sonnet".into());
        live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        assert_eq!(live.models[1].as_deref(), Some("opus"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
        live.step(&mut w);
        assert_eq!(live.inflight[1].len(), 1);
        let card = w.agents[1].card.expect("a card");
        let failed = |cost| AgentEvent::TurnDone {
            ok: false,
            text: "error_api".into(),
            cost,
            denied: 0,
        };
        live.tx.send((1, 0, failed(0))).unwrap();
        live.step(&mut w);
        assert_eq!(live.models[1].as_deref(), Some("sonnet"));
        assert!(!live.tiered[1]);
        assert_eq!(live.generation[1], 1, "a new session");
        assert_eq!(w.agents[1].status, Status::Working, "still on it");
        assert_eq!(w.agents[1].card, Some(card), "the same card");
        assert!(
            matches!(&live.slots[1], Slot::Starting(q) if q.len() == 1 && q[0].contains("Fix it"))
        );
        assert_eq!(live.turns[1], 1);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Fallback { agent: 1, model } if model == "opus"
        ));
        assert!(
            !w.journal
                .iter()
                .any(|j| matches!(j.event, Event::Stopped { .. }))
        );
        // Once is enough: the standard model failing is a failure.
        live.tx.send((1, 1, failed(0))).unwrap();
        live.step(&mut w);
        assert_eq!(live.generation[1], 1);
        assert_eq!(w.agents[1].status, Status::Idle);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 1, .. }
        ));
    }

    #[test]
    fn only_a_first_turn_that_cost_nothing_falls_back() {
        for (turned, cost) in [(true, 0), (false, 5)] {
            let mut w = team(0);
            let mut live = idle_live(1);
            live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
            live.turns[1] = 1;
            live.turned[1] = turned;
            live.claude_home = trees(&format!("fallback-home-{turned}"));
            live.sessions[1] = Some(Resumable {
                session: UUID.into(),
                spent: 0,
            });
            let file = transcript_of(&live, &w, 1, UUID);
            live.tx
                .send((
                    1,
                    0,
                    AgentEvent::TurnDone {
                        ok: false,
                        text: "error".into(),
                        cost,
                        denied: 0,
                    },
                ))
                .unwrap();
            live.step(&mut w);
            assert_eq!(
                live.models[1].as_deref(),
                Some("haiku"),
                "turned {turned} cost {cost}"
            );
            assert!(live.tiered[1]);
            assert!(live.turned[1]);
            assert!(matches!(
                &w.journal.last().unwrap().event,
                Event::Stopped { agent: 1, .. }
            ));
            assert!(file.is_file(), "no fallback, the session stays remembered");
            assert_eq!(
                live.sessions[1].as_ref().map(|s| s.session.as_str()),
                Some(UUID)
            );
            let _ = std::fs::remove_dir_all(&live.claude_home);
        }
        // A first turn that cost nothing: the model is not here, the
        // session on it is forgotten, transcript and all.
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.turns[1] = 1;
        live.claude_home = trees("fallback-home");
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
        });
        let file = transcript_of(&live, &w, 1, UUID);
        live.tx
            .send((
                1,
                0,
                AgentEvent::TurnDone {
                    ok: false,
                    text: "error".into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.models[1], None, "the standard model");
        assert_eq!(live.sessions[1], None);
        assert!(
            gone_soon(&file),
            "the transcript of the failed session goes"
        );
        let _ = std::fs::remove_dir_all(&live.claude_home);
        // A turn that went well is a turn too: nothing to fall back from later.
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.turns[1] = 1;
        finish(&mut live, &mut w, 1, "done");
        assert!(live.turned[1]);
        assert!(live.inflight[1].is_empty());
    }

    #[test]
    fn the_ladder_is_the_vendor_s_unless_the_person_says_otherwise() {
        let h = Hireable::claude("claude", &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus"));
        assert_eq!(h.program, "claude");
        assert_eq!(h.vendor, Vendor::Claude);
        let h = Hireable::claude(
            "claude",
            &[
                (Tier::Strong, "opus[1m]".into()),
                (Tier::Standard, "sonnet".into()),
                (Tier::Cheap, "sonnet".into()),
            ],
        );
        assert_eq!(h.ladder.model(Tier::Cheap), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Standard), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus[1m]"));
        let h = Hireable::claude("claude", &[(Tier::Standard, "opus".into())]);
        assert_eq!(h.ladder.model(Tier::Standard), Some("opus"));
        // Codex's ladder is efforts on its default model.
        let h = Hireable::codex("codex", &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("@low"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("@high"));
        assert_eq!((h.program.as_str(), h.vendor), ("codex", Vendor::Codex));
        // Gemini's ladder is its own model aliases.
        let h = Hireable::gemini("gemini", &[(Tier::Strong, "gemini-2.5-pro".into())]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("flash"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("gemini-2.5-pro"));
        assert_eq!((h.program.as_str(), h.vendor), ("gemini", Vendor::Gemini));
    }

    #[test]
    fn the_person_s_stand_ins_go_to_their_vendor_and_bare_ones_to_the_leader_s() {
        let tiers = vec![
            (None, Tier::Strong, "opus[1m]".to_string()),
            (
                Some(Vendor::Codex),
                Tier::Cheap,
                "gpt-5.6-luna@low".to_string(),
            ),
            (Some(Vendor::Claude), Tier::Standard, "sonnet".to_string()),
        ];
        let team = team_vendors(vec![Vendor::Claude, Vendor::Codex], Vendor::Claude, &tiers);
        assert_eq!(team.len(), 2);
        let (claude, codex) = (&team[0], &team[1]);
        assert_eq!(
            (claude.vendor, claude.program.as_str()),
            (Vendor::Claude, "claude")
        );
        assert_eq!(
            claude.ladder.model(Tier::Strong),
            Some("opus[1m]"),
            "bare: the leader's"
        );
        assert_eq!(claude.ladder.model(Tier::Standard), Some("sonnet"));
        assert_eq!(claude.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(
            (codex.vendor, codex.program.as_str()),
            (Vendor::Codex, "codex")
        );
        assert_eq!(codex.ladder.model(Tier::Cheap), Some("gpt-5.6-luna@low"));
        assert_eq!(
            codex.ladder.model(Tier::Strong),
            Some("@high"),
            "not the bare one"
        );
        // With codex leading, the bare stand-in is codex's.
        let team = team_vendors(vec![Vendor::Claude, Vendor::Codex], Vendor::Codex, &tiers);
        assert_eq!(team[1].ladder.model(Tier::Strong), Some("opus[1m]"));
        assert_eq!(team[0].ladder.model(Tier::Strong), Some("opus"));
        assert!(team_vendors(vec![], Vendor::Claude, &tiers).is_empty());
    }

    #[test]
    fn only_vendors_imece_drives_could_lead() {
        assert_eq!(
            leaders(&[
                (Vendor::Codex, false),
                (Vendor::Claude, true),
                (Vendor::Gemini, true)
            ]),
            [Vendor::Claude, Vendor::Gemini]
        );
        assert!(leaders(&[]).is_empty());
        assert!(leaders(&[(Vendor::Codex, false)]).is_empty());
    }

    #[test]
    fn the_team_needs_a_leader_imece_can_drive() {
        let err = hireable(&[], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("no coding agent") && err.contains("claude, codex, gemini"),
            "{err}"
        );
        let err = hireable(&[(Vendor::Gemini, false)], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("claude is not on PATH (found gemini)") && err.contains("--lead"),
            "{err}"
        );
        let err = hireable(
            &[(Vendor::Claude, true), (Vendor::Gemini, false)],
            Vendor::Gemini,
        )
        .unwrap_err();
        assert!(
            err.contains("gemini is on PATH, but") && err.contains("--lead claude"),
            "{err}"
        );
        let err = hireable(
            &[
                (Vendor::Claude, true),
                (Vendor::Codex, true),
                (Vendor::Gemini, false),
            ],
            Vendor::Gemini,
        )
        .unwrap_err();
        assert!(err.contains("--lead claude or --lead codex"), "{err}");
        let err = hireable(&[(Vendor::Gemini, false)], Vendor::Gemini).unwrap_err();
        assert!(err.contains("--lead claude"), "{err}");
        let ok = hireable(
            &[
                (Vendor::Claude, true),
                (Vendor::Codex, true),
                (Vendor::Gemini, false),
            ],
            Vendor::Codex,
        )
        .unwrap();
        assert_eq!(
            ok,
            [Vendor::Claude, Vendor::Codex],
            "only vendors imece drives are hireable"
        );
    }

    #[test]
    fn the_leader_hires_a_codex_worker_named_after_codex_on_an_effort() {
        let mut w = team(0);
        let mut live = Live::new(
            1,
            two_vendors(&std::env::temp_dir()),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
        );
        let got = live
            .hire(&mut w, 0, &json!({ "vendor": "codex", "tier": "cheap" }))
            .unwrap();
        assert!(got.starts_with("codex-1 joined (codex, @low)."), "{got}");
        assert_eq!(w.agents[1].vendor, Vendor::Codex);
        assert_eq!(live.models[1].as_deref(), Some("@low"));
        assert!(live.tiered[1]);
        let got = live.hire(&mut w, 0, &json!({ "vendor": "codex" })).unwrap();
        assert!(
            got.starts_with("codex-2 joined (codex, the default model)."),
            "{got}"
        );
        assert!(!live.tiered[2]);
        // A claude worker counts its own.
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(got.starts_with("claude-1 joined"), "{got}");
        // The hire tool offers both.
        let hire = &tools_of(Role::Leader, &live.vendor_names(), &[], DEFAULT_RUN_TIMEOUT)[4];
        assert_eq!(
            hire.input["properties"]["vendor"]["enum"],
            json!(["claude", "codex"])
        );
        // A codex worker's prompt says what it can do.
        let worker = system_prompt(&w, 1, 4, 2, Confine::Strict);
        assert!(
            worker.contains("run commands; the sandbox keeps writes inside"),
            "{worker}"
        );
        let claude = system_prompt(&w, 3, 4, 2, Confine::Strict);
        assert!(
            claude.contains("the run tool runs the build, the tests"),
            "{claude}"
        );
    }

    #[test]
    fn a_codex_leader_s_session_reads_only_and_a_worker_s_writes_its_worktree() {
        let dir = std::env::temp_dir().join(format!("imece-codex-lead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("codex-repo");
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Codex;
        let mut live = Live::new(1, two_vendors(&dir), repo.clone(), dir.join("worktrees"));
        live.models[0] = Some("gpt-5.6-terra@high".into());
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(repo.join("args.txt")).unwrap();
        assert!(args.starts_with("app-server\n"), "{args}");
        assert!(args.contains("sandbox_mode=\"read-only\""), "{args}");
        assert!(args.contains("approval_policy=\"never\""), "{args}");
        assert!(
            !args.contains("hello") && !args.contains("You are"),
            "no prompt in argv"
        );
        assert!(
            !args.contains("--model"),
            "the model goes to the thread, not argv"
        );
        let _ = std::fs::remove_file(repo.join("args.txt"));

        live.hire(&mut w, 0, &json!({ "vendor": "codex" })).unwrap();
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let tree = dir.join("worktrees").join("codex-1");
        let args = std::fs::read_to_string(tree.join("args.txt")).unwrap();
        assert!(args.contains("sandbox_mode=\"workspace-write\""), "{args}");
        // The standard level: a worker's sandbox reaches the network and
        // web search is live.
        assert!(
            args.contains("sandbox_workspace_write.network_access=true"),
            "{args}"
        );
        assert!(args.contains("web_search=\"live\""), "{args}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    fn fake_gemini(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-gemini");
        std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_gemini_session_runs_in_a_private_directory_with_the_agent_s_files_included() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-lead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("gemini-repo");
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Gemini;
        let vendors = vec![
            Hireable::claude(&fake_claude(&dir), &[]),
            Hireable::gemini(&fake_gemini(&dir), &[]),
        ];
        let mut live = Live::new(1, vendors, repo.clone(), dir.join("worktrees"));
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        live.models[0] = Some("pro".into());
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        // Gemini ran in the private directory, not in the checkout.
        assert!(!repo.join("args.txt").exists());
        let home = dir.join("worktrees-gemini").join("lead");
        // Seen missing once under load; the trace says why it stopped.
        let args = std::fs::read_to_string(home.join("args.txt"))
            .unwrap_or_else(|e| panic!("{e}: {:?}", w.agents[0].trace));
        assert!(args.starts_with("--acp\n"), "{args}");
        assert!(
            args.contains(&format!("--include-directories\n{}\n", repo.display())),
            "{args}"
        );
        assert!(
            args.contains(&format!("--policy\n{}\n", home.join("policy").display())),
            "{args}"
        );
        assert!(args.contains("-m\npro\n"), "{args}");
        assert!(
            !args.contains("hello") && !args.contains("You are"),
            "no prompt in argv"
        );
        let policy = std::fs::read_to_string(home.join("policy/imece.toml")).unwrap();
        assert!(
            policy.contains("\"read_file\"") && !policy.contains("write_file"),
            "{policy}"
        );
        // The standard level: the web for a leader, never a shell.
        assert!(
            policy.contains("\"google_web_search\", \"web_fetch\"")
                && !policy.contains("run_shell_command"),
            "{policy}"
        );
        assert!(
            policy.contains("toolName = \"*\"\ndecision = \"deny\""),
            "{policy}"
        );
        assert!(
            policy.contains("\"mcp_imece_hire\"") && !policy.contains("claim"),
            "{policy}"
        );
        let instructions = std::fs::read_to_string(home.join("GEMINI.md")).unwrap();
        assert!(
            instructions.contains("You are lead")
                && instructions.contains(&repo.display().to_string()),
            "{instructions}"
        );

        live.hire(&mut w, 0, &json!({ "vendor": "gemini" }))
            .unwrap();
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let tree = dir.join("worktrees").join("gemini-1");
        let home = dir.join("worktrees-gemini").join("gemini-1");
        let args = std::fs::read_to_string(home.join("args.txt"))
            .unwrap_or_else(|e| panic!("{e}: {:?}", w.agents[1].trace));
        assert!(
            args.contains(&format!(
                "--include-directories\n{}\n",
                tree.canonicalize().unwrap().display()
            )),
            "{args}"
        );
        let policy = std::fs::read_to_string(home.join("policy/imece.toml")).unwrap();
        assert!(
            policy.contains("\"write_file\"") && policy.contains("\"mcp_imece_claim\""),
            "{policy}"
        );
        assert!(
            policy.contains("\"run_shell_command\""),
            "a worker's shell: {policy}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn hire_reaches_the_bus_through_the_call_channel() {
        let mut w = team(0);
        let mut live = idle_live(1);
        let got = call(&mut live, &mut w, 0, "hire", json!({ "tier": "strong" })).unwrap();
        assert!(got.starts_with("claude-1 joined (claude, opus)"), "{got}");
        assert_eq!(w.agents.len(), 2);
    }

    #[test]
    fn hiring_and_falling_back_are_worded_in_both_languages() {
        use crate::i18n::Lang;
        use crate::ui::words::event;
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.hire(&mut w, 0, &json!({})).unwrap();
        let hired: Vec<&Event> = w.journal.iter().map(|j| &j.event).collect();
        assert_eq!(event(Lang::En, &w, hired[0]), "lead hired claude-1 (haiku)");
        assert_eq!(
            event(Lang::Tr, &w, hired[0]),
            "lead işe aldı: claude-1 (haiku)"
        );
        assert_eq!(
            event(Lang::En, &w, hired[1]),
            "lead hired claude-2 (default model)"
        );
        assert_eq!(
            event(Lang::Tr, &w, hired[1]),
            "lead işe aldı: claude-2 (varsayılan model)"
        );
        let fell = Event::Fallback {
            agent: 1,
            model: "haiku".into(),
        };
        assert_eq!(
            event(Lang::En, &w, &fell),
            "claude-1: model haiku is not available here, using the default"
        );
        assert_eq!(
            event(Lang::Tr, &w, &fell),
            "claude-1: haiku modeli burada yok, varsayılanla devam"
        );
        assert_eq!(hired[0].kind(), EntryKind::Plan);
        assert_eq!(fell.kind(), EntryKind::Assumption);
        assert_eq!((hired[0].agent(), fell.agent()), (1, 1));
    }

    #[test]
    fn a_session_starts_with_the_agent_s_model_what_is_left_of_the_budget_and_prompt() {
        let dir = std::env::temp_dir().join(format!("imece-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "Run cargo fmt before you finish.").unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            claude_at(&fake_claude(&dir)),
            dir.clone(),
            dir.join("worktrees"),
        );
        live.max_workers = 3;
        live.models[0] = Some("sonnet".into());
        w.agents[0].cost = 1_000;
        live.told(&mut w, 0, "hello").unwrap();
        // The stand-in exits at once, which ends the session.
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        let after = |f: &str| args[args.iter().position(|a| *a == f).unwrap() + 1];
        assert_eq!(after("--model"), "sonnet");
        // What is left of the budget, in dollars: (10000 - 1000) ten-thousandths.
        assert_eq!(after("--max-budget-usd"), "0.900000");
        assert_eq!(after("--tools"), "Glob,Grep,Read,WebFetch,WebSearch");
        assert_eq!(after("--allowedTools"), "WebFetch,WebSearch");
        assert!(args.contains(&"--restricted"), "standard still restricts");
        assert_eq!(after("--name"), "imece lead");
        assert!(after("--append-system-prompt").contains("hire tool, up to 3;"));
        // The prompt spans lines; the stand-in wrote one argument per line.
        let raw = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert!(
            raw.contains("from its CLAUDE.md:\n\nRun cargo fmt before you finish.\n--model\n"),
            "the repository's notes end the prompt: {raw}"
        );
        assert!(!args.contains(&"--mcp-config"), "no bus, no MCP server");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
