//! Fake events for the screen prototype: projects, each with a leader and
//! workers from several vendors, a capped researcher pool shared by all
//! projects, cards, messages and a merge queue. A seed replays the same
//! scenario.

use std::collections::VecDeque;
use std::path::Path;

use crate::i18n::Lang;
use crate::model::*;
use crate::rng::Rng;

type Repo = &'static [(&'static str, &'static [&'static str])];

const SHOP: Repo = &[
    (
        "src/Orders/OrderService.cs",
        &[
            "OrderService.Calculate",
            "OrderService.Validate",
            "OrderService.Submit",
        ],
    ),
    ("src/Orders/Money.cs", &["Money", "Money.Round"]),
    (
        "src/Payments/RetryPolicy.cs",
        &["RetryPolicy.Execute", "RetryPolicy.Backoff"],
    ),
    (
        "src/Payments/Gateway.cs",
        &["Gateway.Charge", "Gateway.Refund", "Gateway.MapError"],
    ),
    (
        "src/Auth/TokenService.cs",
        &["TokenService.Refresh", "TokenService.Revoke"],
    ),
    ("src/Auth/Session.cs", &["Session.Start", "Session.End"]),
    (
        "src/Catalog/ProductQuery.cs",
        &[
            "ProductQuery.Search",
            "ProductQuery.ByCategory",
            "ProductQuery.Page",
        ],
    ),
    (
        "src/Catalog/PriceList.cs",
        &["PriceList.For", "PriceList.Cache"],
    ),
    ("src/Shipping/Rates.cs", &["Rates.Quote", "Rates.Zones"]),
    (
        "src/Notifications/Mailer.cs",
        &["Mailer.Send", "Mailer.Render", "Mailer.Queue"],
    ),
    (
        "tests/Orders/OrderServiceTests.cs",
        &["OrderServiceTests.Calculate", "OrderServiceTests.Validate"],
    ),
    (
        "tests/Payments/RetryPolicyTests.cs",
        &["RetryPolicyTests.Execute"],
    ),
    ("docs/payments.md", &["Retry", "Refunds"]),
];

const MOBILE: Repo = &[
    (
        "app/cart/CartViewModel.kt",
        &[
            "CartViewModel.add",
            "CartViewModel.total",
            "CartViewModel.clear",
        ],
    ),
    (
        "app/checkout/CheckoutScreen.kt",
        &["CheckoutScreen", "CheckoutScreen.PayButton"],
    ),
    (
        "app/net/ApiClient.kt",
        &["ApiClient.orders", "ApiClient.retry"],
    ),
    ("app/settings/Prefs.kt", &["Prefs.token", "Prefs.locale"]),
    (
        "app/test/CartViewModelTest.kt",
        &["CartViewModelTest.total"],
    ),
];

const WEB: Repo = &[
    ("src/pages/Checkout.tsx", &["Checkout", "Checkout.onPay"]),
    ("src/components/Cart.tsx", &["Cart", "Cart.total"]),
    ("src/api/client.ts", &["client.get", "client.retry"]),
    ("src/i18n/tr.json", &["checkout", "cart"]),
    ("tests/cart.test.ts", &["cart.total"]),
];

const BILLING: Repo = &[
    (
        "internal/invoice/invoice.go",
        &["Invoice.Total", "Invoice.Tax"],
    ),
    (
        "internal/ledger/ledger.go",
        &["Ledger.Post", "Ledger.Reverse"],
    ),
    ("cmd/billing/main.go", &["main"]),
    ("internal/invoice/invoice_test.go", &["TestTotal"]),
];

const PROJECTS: &[(&str, Repo)] = &[
    ("shop-api", SHOP),
    ("mobile-app", MOBILE),
    ("web", WEB),
    ("billing", BILLING),
];

pub const MAX_PROJECTS: usize = 4;

/// What fake agents say, in the person's language.
struct Content {
    titles: &'static [&'static str],
    questions: &'static [&'static str],
    answers: &'static [&'static str],
    assumptions: &'static [&'static str],
    decisions: &'static [&'static str],
    topics: &'static [&'static str],
    sensitive: &'static [&'static str],
    tests: &'static str,
    docs: &'static str,
}

const EN: Content = Content {
    titles: &[
        "Currency support",
        "Payment retry policy",
        "Refresh token rotation",
        "Paging in product search",
        "Shipping zones",
        "E-mail templates",
        "Order validation rules",
        "Refund flow",
        "Price list cache",
        "Session timeout",
        "Payments docs",
        "Order tests",
        "Gateway error mapping",
        "Category filter",
        "Bulk e-mail queue",
        "Token revocation",
        "Shipping cost",
        "Money rounding rules",
        "Search result ranking",
        "Notification settings",
    ],
    questions: &[
        "What is Calculate's new signature?",
        "Who adds the Money type?",
        "Does the retry count come from config?",
        "Does Refund use the same queue?",
        "Are you writing this test?",
        "Does Session.End revoke the token too?",
        "Do prices include VAT?",
        "Should zone codes be an enum?",
    ],
    answers: &[
        "Calculate(Money amount)",
        "I added it, in Money.cs",
        "Yes, from PaymentOptions",
        "No, a separate queue",
        "Yes, in OrderServiceTests",
        "Yes, it calls Revoke",
        "No, VAT excluded",
        "An enum: ShippingZone",
    ],
    assumptions: &[
        "a missing currency means TRY",
        "retry at most 3 times",
        "page size 50",
        "tokens live 15 min",
        "old templates stay",
        "shipping cost in cents",
        "refunds may be partial",
    ],
    decisions: &[
        "Money type instead of decimal, because of rounding",
        "hand-written backoff instead of Polly: no new dependency",
        "cursor-based paging, for stable results",
        "rotation in one conditional UPDATE",
        "plain-text templates instead of Razor",
        "cache for 5 min",
    ],
    topics: &[
        "Polly v8 retry API",
        "Npgsql timeout error",
        "System.Text.Json source generator",
        "EF Core 10 bulk update",
        "Razor template cache",
        "ISO 4217 currency list",
        "JWT clock skew",
    ],
    sensitive: &[
        "adds a package to the csproj",
        "adds a step to CI",
        "updates the lockfile",
        "adds a host to appsettings",
        "changes the build script",
    ],
    tests: "tests",
    docs: "docs",
};

const TR: Content = Content {
    titles: &[
        "Para birimi desteği",
        "Ödeme retry politikası",
        "Refresh token rotasyonu",
        "Ürün aramasında sayfalama",
        "Kargo bölgeleri",
        "E-posta şablonları",
        "Sipariş doğrulama kuralları",
        "İade akışı",
        "Fiyat listesi önbelleği",
        "Oturum zaman aşımı",
        "Ödeme dokümanı",
        "Sipariş testleri",
        "Gateway hata eşlemesi",
        "Kategori filtresi",
        "Toplu e-posta kuyruğu",
        "Token iptali",
        "Kargo ücret hesabı",
        "Para yuvarlama kuralları",
        "Arama sonuç sıralaması",
        "Bildirim tercihleri",
    ],
    questions: &[
        "Calculate'in yeni imzası ne?",
        "Money tipini kim ekliyor?",
        "Retry sayısı config'den mi gelecek?",
        "Refund aynı kuyruğu mu kullanıyor?",
        "Bu testi sen mi yazıyorsun?",
        "Session.End token'ı da iptal ediyor mu?",
        "Fiyatlar KDV dahil mi?",
        "Bölge kodları enum mu olsun?",
    ],
    answers: &[
        "Calculate(Money amount)",
        "Ben ekledim, Money.cs'de",
        "Evet, PaymentOptions'tan",
        "Hayır, ayrı kuyruk",
        "Evet, OrderServiceTests'te",
        "Evet, Revoke'u çağırıyor",
        "Hayır, KDV hariç",
        "Enum: ShippingZone",
    ],
    assumptions: &[
        "null para birimi → TRY",
        "retry en fazla 3 kez",
        "sayfa boyutu 50",
        "token ömrü 15 dk",
        "eski şablonlar korunuyor",
        "kargo ücreti kuruş cinsinden",
        "iade kısmi olabilir",
    ],
    decisions: &[
        "decimal yerine Money tipi, sebep: yuvarlama",
        "Polly yerine elle backoff, sebep: yeni bağımlılık yok",
        "cursor tabanlı sayfalama, sebep: kararlılık",
        "rotasyon tek bir koşullu UPDATE ile",
        "şablonlar Razor yerine düz metin",
        "önbellek süresi 5 dk",
    ],
    topics: &[
        "Polly v8 retry API",
        "Npgsql timeout hatası",
        "System.Text.Json kaynak üretici",
        "EF Core 10 toplu güncelleme",
        "Razor şablon önbelleği",
        "ISO 4217 para birimi listesi",
        "JWT saat kayması",
    ],
    sensitive: &[
        "csproj'a yeni paket ekliyor",
        "CI'a yeni bir adım ekliyor",
        "lockfile'ı güncelliyor",
        "appsettings'e yeni bir host ekliyor",
        "build script'ini değiştiriyor",
    ],
    tests: "testler",
    docs: "doküman",
};

const HOSTS: &[&str] = &[
    "learn.microsoft.com",
    "github.com/App-vNext/Polly/issues",
    "stackoverflow.com",
    "www.npgsql.org",
];

fn content(lang: Lang) -> &'static Content {
    match lang {
        Lang::En => &EN,
        Lang::Tr => &TR,
    }
}

pub struct Sim {
    rng: Rng,
    content: &'static Content,
    researchers: usize,
    /// Topics already researched; the next question about them is free.
    cache: Vec<String>,
}

pub fn world(
    projects: usize,
    workers: usize,
    max_researchers: usize,
    seed: u64,
    lang: Lang,
) -> (World, Sim) {
    let projects = projects.clamp(1, MAX_PROJECTS);
    let mut sim = Sim {
        rng: Rng::new(seed),
        content: content(lang),
        researchers: 0,
        cache: Vec::new(),
    };
    let mut w = World {
        tick: 0,
        started_at: 0,
        run_tick: 0,
        run_spent: 0,
        projects: Vec::new(),
        agents: Vec::new(),
        cards: Vec::new(),
        journal: Vec::new(),
        inbox: Vec::new(),
        budget: 200 * 10_000,
        max_researchers,
        research_queue: VecDeque::new(),
        changes: Vec::new(),
        plans: Vec::new(),
    };
    for (p, &(name, repo)) in PROJECTS.iter().take(projects).enumerate() {
        let leader = w.agents.len();
        let lead_name = if projects == 1 {
            "lead".to_string()
        } else {
            format!("lead-{name}")
        };
        w.agents.push(Agent::new(
            leader,
            p,
            lead_name,
            Vendor::Claude,
            Role::Leader,
            Status::Idle,
            0,
        ));
        w.projects.push(Project {
            id: p,
            name: name.into(),
            leader,
            trunk: Some(140 + 37 * p as u32),
            repo,
            closed: false,
        });
    }
    for i in 0..workers {
        // Every project gets a worker first; the rest are spread at random.
        let project = if i < projects {
            i
        } else {
            sim.rng.below(projects as u64) as usize
        };
        sim.hire(&mut w, project);
    }
    for p in 0..projects {
        let n = w
            .agents
            .iter()
            .filter(|a| a.project == p && a.role == Role::Worker)
            .count();
        for _ in 0..(n + n / 2).max(1) {
            let title = (*sim.rng.pick(sim.content.titles)).to_string();
            sim.new_card(&mut w, p, title);
        }
    }
    (w, sim)
}

/// Symbols of `project` leased by cards that are not merged yet.
pub fn held_symbols(w: &World, project: ProjectId) -> Vec<Symbol> {
    w.cards
        .iter()
        .filter(|c| c.project == project && c.holds_leases())
        .flat_map(|c| c.symbols.iter().copied())
        .collect()
}

impl Sim {
    /// New fake content comes in this language from now on.
    pub fn set_lang(&mut self, lang: Lang) {
        self.content = content(lang);
    }

    /// A leader the person talked to breaks the request into cards; what
    /// the person tells a worker only lands in its story.
    pub fn told(&mut self, w: &mut World, id: AgentId, text: &str) {
        if w.agents[id].role != Role::Leader {
            return;
        }
        let t = w.tick;
        w.plans.push(Plan {
            project: w.agents[id].project,
            text: text.into(),
            ready: t + 4,
        });
        w.agents[id].set_status(Status::Planning { until: t + 4 }, t);
    }

    pub fn step(&mut self, w: &mut World) {
        w.tick += 1;
        let t = w.tick;
        for a in &mut w.agents {
            if a.talk.as_ref().is_some_and(|k| k.until <= t) {
                a.talk = None;
            }
            if !a.gone && a.status.busy() {
                a.cost += a.vendor.cost_per_tick();
            }
        }
        for p in 0..w.projects.len() {
            if !w.projects[p].closed {
                self.leader(w, p);
            }
        }
        // Researchers spawned during this tick start on the next one.
        for id in 0..w.agents.len() {
            if w.agents[id].gone {
                continue;
            }
            match w.agents[id].role {
                Role::Worker => self.worker(w, id),
                Role::Researcher => self.researcher(w, id),
                Role::Leader | Role::Helper => {}
            }
        }
    }

    /// A worker of a random vendor joins `project`, named `<vendor>-N`
    /// with the next free N, idle.
    fn hire(&mut self, w: &mut World, project: ProjectId) -> AgentId {
        let v = *self.rng.pick(&[
            Vendor::Claude,
            Vendor::Claude,
            Vendor::Codex,
            Vendor::Codex,
            Vendor::Gemini,
            Vendor::Qwen,
            Vendor::Local,
        ]);
        let so_far = w
            .agents
            .iter()
            .filter(|a| a.vendor == v && a.role == Role::Worker && !a.gone)
            .count();
        let name = (so_far + 1..)
            .map(|n| format!("{}-{n}", v.label()))
            .find(|name| w.agent_by_name(name).is_none())
            .expect("some number is free");
        let id = w.agents.len();
        w.agents.push(Agent::new(
            id,
            project,
            name,
            v,
            Role::Worker,
            Status::Idle,
            w.tick,
        ));
        id
    }

    /// `:open PATH [NAME]` in the demo: a fake project named `name` (else
    /// after the last part of `path`), with a leader, two workers and
    /// three cards; a closed project of that name opens again in place.
    pub fn open_project(
        &mut self,
        w: &mut World,
        path: &str,
        name: Option<&str>,
    ) -> Result<ProjectId, ProjectRefusal> {
        let name = match name {
            Some(n) if !valid_project_name(n) => {
                return Err(ProjectRefusal::BadName(n.to_string()));
            }
            Some(n) => n.to_string(),
            None => project_name_of(Path::new(path)),
        };
        if w.project_by_name(&name).is_some() {
            return Err(ProjectRefusal::NameTaken(name));
        }
        let reuse = w
            .projects
            .iter()
            .find(|q| q.closed && q.name == name)
            .map(|q| q.id);
        let p = w.open_project(reuse, name, path.to_string(), Vendor::Claude);
        if reuse.is_none() {
            w.projects[p].repo = PROJECTS[p % PROJECTS.len()].1;
            w.projects[p].trunk = Some(140 + 37 * p as u32);
        }
        for _ in 0..2 {
            self.hire(w, p);
        }
        for _ in 0..3 {
            let title = (*self.rng.pick(self.content.titles)).to_string();
            self.new_card(w, p, title);
        }
        Ok(p)
    }

    /// `:close` in the demo: the project's agents leave and its plans and
    /// research requests go; the last open project stays.
    pub fn close_project(&mut self, w: &mut World, p: ProjectId) -> Result<(), ProjectRefusal> {
        if w.open_projects().count() <= 1 {
            return Err(ProjectRefusal::Last);
        }
        w.plans.retain(|plan| plan.project != p);
        let agents = &w.agents;
        w.research_queue.retain(|&id| agents[id].project != p);
        w.close_project(p);
        Ok(())
    }

    /// `:rename` in the demo.
    pub fn rename_project(
        &mut self,
        w: &mut World,
        p: ProjectId,
        name: &str,
    ) -> Result<(), ProjectRefusal> {
        if !valid_project_name(name) {
            return Err(ProjectRefusal::BadName(name.to_string()));
        }
        if w.project_by_name(name).is_some_and(|q| q != p) {
            return Err(ProjectRefusal::NameTaken(name.to_string()));
        }
        w.rename_project(p, name.to_string());
        Ok(())
    }

    fn new_card(&mut self, w: &mut World, project: ProjectId, title: String) -> CardId {
        let id = w.cards.len();
        w.cards.push(Card {
            id,
            project,
            title,
            owner: None,
            column: Column::Waiting,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 0,
            finished: 0,
        });
        id
    }

    fn leader(&mut self, w: &mut World, project: ProjectId) {
        let t = w.tick;
        let leader = w.projects[project].leader;
        if let Status::Planning { until } = w.agents[leader].status
            && t >= until
        {
            w.agents[leader].set_status(Status::Idle, t);
        }

        let mut ready = Vec::new();
        w.plans.retain(|p| {
            if p.project == project && p.ready <= t {
                ready.push(p.clone());
                false
            } else {
                true
            }
        });
        for p in ready {
            let titles = [
                p.text.clone(),
                format!("{}: {}", p.text, self.content.tests),
                format!("{}: {}", p.text, self.content.docs),
            ];
            for title in titles {
                self.new_card(w, project, title);
            }
            w.note(Event::Plan {
                leader,
                request: p.text,
            });
            w.agents[leader].trace(t, "tool_use lonca.board_post ×3");
        }

        // Keep a card waiting for every idle worker, so the guild stays busy.
        // Routine planning goes to the leader's trace, not the journal.
        let in_project = |a: &&Agent| a.project == project && a.role == Role::Worker;
        let idle = w
            .agents
            .iter()
            .filter(in_project)
            .filter(|a| a.status == Status::Idle)
            .count();
        let waiting = w
            .cards
            .iter()
            .filter(|c| c.project == project && c.column == Column::Waiting)
            .count();
        if waiting < idle && self.rng.one_in(3) {
            let title = (*self.rng.pick(self.content.titles)).to_string();
            let id = self.new_card(w, project, title.clone());
            w.agents[leader].set_status(Status::Planning { until: t + 3 }, t);
            w.agents[leader].trace(
                t,
                format!("tool_use lonca.board_post #{} {title:?}", id + 1),
            );
        }

        // At most two assignments a tick, so the screen stays calm.
        for _ in 0..2 {
            let Some(agent) = w
                .agents
                .iter()
                .filter(in_project)
                .find(|a| a.status == Status::Idle)
                .map(|a| a.id)
            else {
                break;
            };
            let Some(card) = w
                .cards
                .iter()
                .find(|c| c.project == project && c.column == Column::Waiting)
                .map(|c| c.id)
            else {
                break;
            };
            self.assign(w, agent, card);
        }
    }

    fn assign(&mut self, w: &mut World, agent: AgentId, card: CardId) {
        let t = w.tick;
        let project = w.cards[card].project;
        let held = held_symbols(w, project);
        let mut free: Vec<Symbol> = w.projects[project]
            .repo
            .iter()
            .flat_map(|&(file, names)| names.iter().map(move |&name| Symbol { file, name }))
            .filter(|s| !held.contains(s))
            .collect();
        let mut symbols = Vec::new();
        for _ in 0..self.rng.range(1, 3) {
            if free.is_empty() {
                break;
            }
            let i = self.rng.below(free.len() as u64) as usize;
            symbols.push(free.swap_remove(i));
        }
        let lease = symbols
            .iter()
            .map(|s| format!("{}#{}", s.file, s.name))
            .collect::<Vec<_>>()
            .join(", ");

        let c = &mut w.cards[card];
        c.owner = Some(agent);
        c.column = Column::Working;
        c.started = t;
        c.symbols = symbols;
        let title = c.title.clone();

        let a = &mut w.agents[agent];
        a.card = Some(card);
        a.progress = 0;
        a.set_status(Status::Working, t);
        a.trace(t, format!("session/prompt \"#{} {title}\"", card + 1));
        if !lease.is_empty() {
            a.trace(t, format!("tool_use lonca.lease_acquire [{lease}]"));
        }
        let name = a.name.clone();
        w.note(Event::Assign { agent, card });
        w.story(Some(card), StoryEvent::Took { agent });
        let leader = w.projects[project].leader;
        w.agents[leader].trace(
            t,
            format!("tool_use lonca.send type=assign #{} → {name}", card + 1),
        );
    }

    fn worker(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        match w.agents[id].status {
            Status::Working => self.work(w, id),
            Status::Asking { to, until } if t >= until => {
                let answer = *self.rng.pick(self.content.answers);
                let other = w.agents[to].name.clone();
                let a = &mut w.agents[id];
                a.talk = Some(Talk {
                    with: Party::Agent(to),
                    text: answer.into(),
                    until: t + 4,
                });
                a.set_status(Status::Working, t);
                a.trace(t, format!("inbox ← {other} type=answer {answer:?}"));
                let card = a.card;
                w.story(
                    card,
                    StoryEvent::Answered {
                        from: to,
                        text: answer.into(),
                    },
                );
            }
            Status::Verifying { until } if t >= until => self.verified(w, id),
            _ => {}
        }
    }

    fn work(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let card = w.agents[id].card;
        let before = Phase::of(w.agents[id].progress);
        let progress = (w.agents[id].progress + self.rng.range(1, 2) as u8).min(100);
        w.agents[id].progress = progress;
        let after = Phase::of(progress);
        if before != after {
            match after {
                Phase::Impl => {
                    let d = *self.rng.pick(self.content.decisions);
                    w.story(card, StoryEvent::Decision(d.into()));
                }
                Phase::Test => w.story(card, StoryEvent::WritingTests),
                Phase::Plan => {}
            }
        }

        if progress >= 100 {
            let files = card.map_or(1, |c| w.cards[c].symbols.len().max(1));
            let (plus, minus) = (self.rng.range(20, 140), self.rng.range(0, 40));
            if let Some(c) = card {
                w.cards[c].column = Column::Verifying;
            }
            w.story(card, StoryEvent::Change { files, plus, minus });
            let until = t + self.rng.range(5, 12);
            let a = &mut w.agents[id];
            a.set_status(Status::Verifying { until }, t);
            a.trace(t, "tool_use lonca.propose");
            return;
        }

        match self.rng.below(1000) {
            0..4 => self.ask(w, id),
            4..6 => self.block(w, id),
            6..9 => {
                let s = *self.rng.pick(self.content.assumptions);
                w.story(card, StoryEvent::Assumption(s.into()));
                w.note(Event::Assumption {
                    agent: id,
                    text: s.into(),
                });
            }
            9..10 => {
                let what = *self.rng.pick(self.content.sensitive);
                w.inbox.push(InboxItem {
                    kind: InboxKind::Sensitive,
                    agent: id,
                    detail: what.into(),
                    researcher: None,
                    open: true,
                    tick: t,
                });
                w.agents[id].set_status(Status::WaitingHuman, t);
                w.note(Event::Sensitive {
                    agent: id,
                    what: what.into(),
                });
                w.story(card, StoryEvent::Sensitive(what.into()));
            }
            10..60 => {
                let syms = card.map(|c| w.cards[c].symbols.clone()).unwrap_or_default();
                let line = match (syms.first(), self.rng.below(5)) {
                    (Some(s), 0) => format!("tool_use Read {}", s.file),
                    (Some(s), 1) => format!("tool_use Edit {}", s.file),
                    (Some(s), 2) => format!("tool_use Grep {:?}", s.name),
                    (_, 3) => "tool_use Bash build".to_string(),
                    _ => "tool_use lonca.preview".to_string(),
                };
                w.agents[id].trace(t, line);
            }
            _ => {}
        }
    }

    fn ask(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let project = w.agents[id].project;
        let others: Vec<AgentId> = w
            .agents
            .iter()
            .filter(|a| {
                a.id != id
                    && a.project == project
                    && a.role == Role::Worker
                    && a.status == Status::Working
            })
            .map(|a| a.id)
            .collect();
        if others.is_empty() {
            return;
        }
        let to = *self.rng.pick(&others);
        let q = *self.rng.pick(self.content.questions);
        let until = t + self.rng.range(3, 8);
        let other = w.agents[to].name.clone();
        let a = &mut w.agents[id];
        a.set_status(Status::Asking { to, until }, t);
        a.talk = Some(Talk {
            with: Party::Agent(to),
            text: q.into(),
            until,
        });
        a.trace(t, format!("tool_use lonca.send to={other} type=ask"));
        w.agents[to].talk = Some(Talk {
            with: Party::Agent(id),
            text: q.into(),
            until,
        });
        w.note(Event::Talk {
            from: id,
            to,
            text: q.into(),
        });
        let card = w.agents[id].card;
        w.story(
            card,
            StoryEvent::Asked {
                from: id,
                to,
                text: q.into(),
            },
        );
    }

    fn block(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let topic = *self.rng.pick(self.content.topics);
        let card = w.agents[id].card;
        let a = &mut w.agents[id];
        a.set_status(Status::Blocked, t);
        a.topic = topic.into();
        a.trace(
            t,
            format!("tool_use lonca.send to=researcher type=blocked {topic:?}"),
        );

        if self.cache.iter().any(|c| c == topic) {
            w.agents[id].set_status(Status::Working, t);
            w.note(Event::FromCache {
                agent: id,
                topic: topic.into(),
            });
            w.story(card, StoryEvent::FromCache(topic.into()));
            return;
        }
        w.story(card, StoryEvent::BlockedOn(topic.into()));

        // The same question is never researched twice at once.
        let in_flight = w
            .agents
            .iter()
            .find(|r| r.role == Role::Researcher && !r.gone && r.topic == topic)
            .map(|r| r.id);
        if let Some(r) = in_flight {
            w.agents[id].researcher = Some(r);
            w.note(Event::Blocked {
                agent: id,
                topic: topic.into(),
                joined: Some(r),
            });
            return;
        }
        w.note(Event::Blocked {
            agent: id,
            topic: topic.into(),
            joined: None,
        });
        if w.active_researchers() < w.max_researchers {
            self.spawn_researcher(w, id);
        } else {
            w.research_queue.push_back(id);
        }
    }

    fn spawn_researcher(&mut self, w: &mut World, requester: AgentId) {
        let t = w.tick;
        self.researchers += 1;
        let id = w.agents.len();
        let topic = w.agents[requester].topic.clone();
        let until = t + self.rng.range(8, 18);
        let mut r = Agent::new(
            id,
            w.agents[requester].project,
            format!("researcher-{}", self.researchers),
            Vendor::Gemini,
            Role::Researcher,
            Status::Researching {
                for_agent: requester,
                until,
            },
            t,
        );
        // It reads the outside world: everything it writes needs care.
        r.taint = Taint::Full;
        r.topic = topic.clone();
        r.trace(t, format!("session/prompt {topic:?}"));
        w.agents.push(r);
        w.agents[requester].researcher = Some(id);
        w.note(Event::ResearchOpened {
            researcher: id,
            topic,
        });
    }

    fn researcher(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let Status::Researching { until, .. } = w.agents[id].status else {
            return;
        };
        if t < until {
            if self.rng.one_in(3) {
                let host = *self.rng.pick(HOSTS);
                w.agents[id].trace(t, format!("tool_use WebFetch https://{host}/…"));
            }
            return;
        }

        w.agents[id].gone = true;
        let topic = w.agents[id].topic.clone();
        // One report in four proposes something sensitive (a new package):
        // that one waits for the person; the rest go straight to the reader.
        let sensitive = self.rng.one_in(4);
        let waiting: Vec<AgentId> = w
            .agents
            .iter()
            .filter(|a| a.researcher == Some(id))
            .map(|a| a.id)
            .collect();
        for a in waiting {
            w.agents[a].researcher = None;
            let card = w.agents[a].card;
            if sensitive {
                w.inbox.push(InboxItem {
                    kind: InboxKind::Research,
                    agent: a,
                    detail: topic.clone(),
                    researcher: Some(id),
                    open: true,
                    tick: t,
                });
                w.agents[a].set_status(Status::WaitingHuman, t);
                w.story(card, StoryEvent::ResearchNeedsYou { researcher: id });
            } else {
                let agent = &mut w.agents[a];
                agent.set_status(Status::Working, t);
                agent.taint = agent.taint.max(Taint::Soft);
                w.story(
                    card,
                    StoryEvent::ResearchArrived {
                        researcher: id,
                        topic: topic.clone(),
                    },
                );
            }
        }
        if sensitive {
            w.note(Event::ResearchNeedsYou {
                researcher: id,
                topic: topic.clone(),
            });
        } else {
            w.note(Event::ResearchDelivered {
                researcher: id,
                topic: topic.clone(),
            });
        }
        self.cache.push(topic);

        // A slot is free: the next queued question.
        while w.active_researchers() < w.max_researchers {
            let Some(next) = w.research_queue.pop_front() else {
                break;
            };
            if w.agents[next].status != Status::Blocked || w.agents[next].researcher.is_some() {
                continue;
            }
            let topic = w.agents[next].topic.clone();
            if self.cache.contains(&topic) {
                let card = w.agents[next].card;
                w.agents[next].set_status(Status::Working, t);
                w.note(Event::FromCache {
                    agent: next,
                    topic: topic.clone(),
                });
                w.story(card, StoryEvent::FromCache(topic));
                continue;
            }
            self.spawn_researcher(w, next);
        }
    }

    fn verified(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let card = w.agents[id].card;
        if self.rng.one_in(12) {
            w.story(card, StoryEvent::QueueFailed);
            if let Some(c) = card {
                w.cards[c].column = Column::Working;
            }
            w.note(Event::Failed { agent: id });
            let a = &mut w.agents[id];
            a.progress = 80;
            a.set_status(Status::Working, t);
            return;
        }
        let project = w.agents[id].project;
        let trunk = w.projects[project].trunk.map_or(1, |n| n + 1);
        w.projects[project].trunk = Some(trunk);
        let tests = self.rng.range(3, 40);
        w.story(card, StoryEvent::Verified { tests, trunk });
        if let Some(c) = card {
            w.cards[c].column = Column::Done;
            w.cards[c].finished = t;
            w.note(Event::Merged { agent: id, card: c });
        }
        let a = &mut w.agents[id];
        a.card = None;
        a.progress = 0;
        a.set_status(Status::Idle, t);
        a.trace(t, "change.integrate ✓");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(w: &mut World, s: &mut Sim, ticks: usize, approve: bool) {
        for _ in 0..ticks {
            s.step(w);
            if approve {
                for i in w.open_inbox() {
                    w.approve(i);
                }
            }
        }
    }

    fn journal(w: &World) -> Vec<String> {
        w.journal
            .iter()
            .map(|e| format!("{} {:?}", e.tick, e.event))
            .collect()
    }

    #[test]
    fn a_seed_replays_the_same_scenario() {
        let (mut a, mut sa) = world(2, 12, 4, 7, Lang::En);
        let (mut b, mut sb) = world(2, 12, 4, 7, Lang::En);
        run(&mut a, &mut sa, 600, false);
        run(&mut b, &mut sb, 600, false);
        assert!(!a.journal.is_empty());
        assert_eq!(journal(&a), journal(&b));
    }

    #[test]
    fn researchers_never_exceed_the_cap() {
        let (mut w, mut s) = world(3, 60, 3, 11, Lang::En);
        for _ in 0..3000 {
            s.step(&mut w);
            assert!(w.active_researchers() <= 3);
            for i in w.open_inbox() {
                w.approve(i);
            }
        }
        assert!(
            w.agents.iter().any(|a| a.role == Role::Researcher),
            "no researcher was ever opened"
        );
    }

    #[test]
    fn a_symbol_is_leased_by_one_card_at_a_time() {
        let (mut w, mut s) = world(2, 40, 4, 3, Lang::En);
        for _ in 0..2000 {
            s.step(&mut w);
            for p in 0..w.projects.len() {
                let held = held_symbols(&w, p);
                for (i, a) in held.iter().enumerate() {
                    assert!(!held[i + 1..].contains(a), "{a:?} is leased twice");
                }
            }
            for i in w.open_inbox() {
                w.approve(i);
            }
        }
    }

    #[test]
    fn agents_stay_in_their_project() {
        let (mut w, mut s) = world(4, 30, 4, 2, Lang::En);
        run(&mut w, &mut s, 1500, true);
        for a in &w.agents {
            if let Some(c) = a.card {
                assert_eq!(
                    w.cards[c].project, a.project,
                    "{} works on another project's card",
                    a.name
                );
            }
            if let Status::Asking { to, .. } = a.status {
                assert_eq!(w.agents[to].project, a.project);
            }
        }
        for p in 0..4 {
            assert!(
                w.agents
                    .iter()
                    .any(|a| a.project == p && a.role == Role::Worker),
                "project {p} has no worker"
            );
        }
    }

    #[test]
    fn work_reaches_every_trunk() {
        let (mut w, mut s) = world(2, 12, 4, 5, Lang::En);
        let before: Vec<u32> = w.projects.iter().map(|p| p.trunk.unwrap()).collect();
        run(&mut w, &mut s, 1500, true);
        for (p, b) in w.projects.iter().zip(before) {
            let trunk = p.trunk.unwrap();
            assert!(trunk >= b + 5, "{} only reached #{trunk}", p.name);
        }
    }

    #[test]
    fn approving_resumes_the_waiting_agent() {
        let (mut w, mut s) = world(1, 24, 4, 9, Lang::En);
        let mut item = None;
        for _ in 0..5000 {
            s.step(&mut w);
            if let Some(&i) = w.open_inbox().first() {
                item = Some(i);
                break;
            }
        }
        let i = item.expect("no inbox item in 5000 ticks");
        let agent = w.inbox[i].agent;
        assert_eq!(w.agents[agent].status, Status::WaitingHuman);
        w.approve(i);
        assert_eq!(w.agents[agent].status, Status::Working);
        assert!(!w.inbox[i].open);
    }

    #[test]
    fn a_leader_breaks_a_request_into_cards_of_its_project() {
        let (mut w, mut s) = world(2, 4, 2, 1, Lang::Tr);
        let leader = w.projects[1].leader;
        s.told(&mut w, leader, "Kargo takibi");
        w.said(leader, "Kargo takibi");
        run(&mut w, &mut s, 10, false);
        let card = w
            .cards
            .iter()
            .find(|c| c.title == "Kargo takibi: testler")
            .expect("no card");
        assert_eq!(card.project, 1);
    }

    #[test]
    fn the_demo_opens_a_project_with_a_lead_two_workers_and_cards_and_reopens_it_by_name() {
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        assert_eq!(s.open_project(&mut w, "~/work/shop", None), Ok(1));
        let p = &w.projects[1];
        assert_eq!(
            (p.name.as_str(), p.trunk, p.closed),
            ("shop", Some(177), false)
        );
        assert_eq!(p.repo, PROJECTS[1].1, "a code map of its own");
        assert_eq!(w.agents[p.leader].name, "lead-shop");
        let workers: Vec<&Agent> = w
            .agents
            .iter()
            .filter(|a| a.project == 1 && a.role == Role::Worker)
            .collect();
        assert_eq!(workers.len(), 2);
        assert!(workers.iter().all(|a| a.status == Status::Idle && !a.gone));
        let mut names: Vec<&str> = w.agents.iter().map(|a| a.name.as_str()).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "names are unique");
        assert_eq!(w.cards.iter().filter(|c| c.project == 1).count(), 3);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Opened { leader, path } if *leader == w.projects[1].leader && path == "~/work/shop"
        ));
        assert_eq!(s.open_project(&mut w, "/x", Some("api")), Ok(2));
        assert_eq!(w.projects[2].repo, PROJECTS[2].1);
        assert_eq!(w.projects[2].trunk, Some(214));
        assert_eq!(
            s.open_project(&mut w, "/y", Some("a b")),
            Err(ProjectRefusal::BadName("a b".into()))
        );
        assert_eq!(
            s.open_project(&mut w, "/y", Some("shop")),
            Err(ProjectRefusal::NameTaken("shop".into()))
        );
        assert_eq!(
            s.open_project(&mut w, "/y/shop", None),
            Err(ProjectRefusal::NameTaken("shop".into())),
            "named after the path's last part"
        );
        assert_eq!(w.projects.len(), 3);

        // Work goes on in it.
        run(&mut w, &mut s, 100, true);
        assert!(
            w.agents
                .iter()
                .any(|a| a.project == 1 && a.role == Role::Worker && a.cost > 0)
        );
        // Closed: its people leave, its plans and research requests go,
        // nothing of it moves while the rest goes on.
        let lead = w.projects[1].leader;
        s.told(&mut w, lead, "add a thing");
        let worker = workers_of(&w, 1)[0];
        let other = workers_of(&w, 0)[0];
        w.research_queue.push_back(worker);
        w.research_queue.push_back(other);
        assert_eq!(s.close_project(&mut w, 1), Ok(()));
        assert!(w.projects[1].closed);
        assert!(w.agents.iter().filter(|a| a.project == 1).all(|a| a.gone));
        assert!(w.plans.iter().all(|p| p.project != 1));
        assert_eq!(w.research_queue, [other]);
        let costs: Vec<u64> = w
            .agents
            .iter()
            .filter(|a| a.project == 1)
            .map(|a| a.cost)
            .collect();
        let cards = w.cards.iter().filter(|c| c.project == 1).count();
        run(&mut w, &mut s, 100, true);
        let after: Vec<u64> = w
            .agents
            .iter()
            .filter(|a| a.project == 1)
            .map(|a| a.cost)
            .collect();
        assert_eq!(after, costs, "nobody of a closed project works");
        assert_eq!(
            w.cards.iter().filter(|c| c.project == 1).count(),
            cards,
            "no new cards for a closed project"
        );
        assert!(w.projects[0].closed || w.agents.iter().any(|a| a.project == 0 && a.cost > 0));

        // Opened again by name, in place: a new lead, new workers, same code map.
        assert_eq!(s.open_project(&mut w, "/z", Some("shop")), Ok(1));
        assert_eq!(w.projects.len(), 3);
        assert_ne!(w.projects[1].leader, lead);
        assert!(w.agents[lead].gone, "the old lead stays gone");
        assert_eq!(w.projects[1].repo, PROJECTS[1].1);
        assert_eq!(workers_of(&w, 1).len(), 2);
    }

    fn workers_of(w: &World, p: ProjectId) -> Vec<AgentId> {
        w.agents
            .iter()
            .filter(|a| a.project == p && a.role == Role::Worker && !a.gone)
            .map(|a| a.id)
            .collect()
    }

    #[test]
    fn the_demo_keeps_its_last_project_and_renames_one() {
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        assert_eq!(s.close_project(&mut w, 0), Err(ProjectRefusal::Last));
        assert!(!w.projects[0].closed);
        s.open_project(&mut w, "/x", Some("api")).unwrap();
        assert_eq!(
            s.rename_project(&mut w, 1, "a b"),
            Err(ProjectRefusal::BadName("a b".into()))
        );
        assert_eq!(
            s.rename_project(&mut w, 1, "shop-api"),
            Err(ProjectRefusal::NameTaken("shop-api".into()))
        );
        assert_eq!(w.projects[1].name, "api", "refused: unchanged");
        assert_eq!(s.rename_project(&mut w, 1, "api"), Ok(()), "its own name");
        assert_eq!(s.rename_project(&mut w, 1, "store"), Ok(()));
        assert_eq!(w.projects[1].name, "store");
        assert_eq!(w.agents[w.projects[1].leader].name, "lead-store");
        assert_eq!(s.close_project(&mut w, 0), Ok(()));
        assert_eq!(s.close_project(&mut w, 1), Err(ProjectRefusal::Last));
        assert!(!w.projects[1].closed);
        // A closed project is reused for its own name only: another name
        // is a new project.
        assert_eq!(s.open_project(&mut w, "/n", Some("new")), Ok(2));
        assert_eq!(w.projects.len(), 3);
        assert!(w.projects[0].closed, "not reused for another name");
    }

    #[test]
    fn a_hired_worker_takes_the_next_free_number_of_its_vendor() {
        let (mut w, mut s) = world(1, 0, 4, 7, Lang::En);
        // Only workers count: the lead is a Claude too, and the first
        // Claude worker is still claude-1.
        assert_eq!(w.agents[0].vendor, Vendor::Claude);
        let mut claude = None;
        for _ in 0..64 {
            let id = s.hire(&mut w, 0);
            if w.agents[id].vendor == Vendor::Claude {
                claude = Some(id);
                break;
            }
        }
        assert_eq!(w.agents[claude.expect("a claude")].name, "claude-1");
        let (mut w, mut s) = world(1, 0, 4, 7, Lang::En);
        let first = s.hire(&mut w, 0);
        let v = w.agents[first].vendor;
        assert_eq!(w.agents[first].name, format!("{}-1", v.label()));
        assert_eq!(w.agents[first].project, 0);
        w.agents[first].gone = true;
        // The number of one that left is free again; a bounded search for
        // the same vendor (the pick is random).
        let mut same = None;
        for _ in 0..64 {
            let id = s.hire(&mut w, 0);
            if w.agents[id].vendor == v {
                same = Some(id);
                break;
            }
        }
        let same = same.expect("the vendor comes up again");
        assert_eq!(w.agents[same].name, format!("{}-1", v.label()));
        let mut again = None;
        for _ in 0..64 {
            let id = s.hire(&mut w, 0);
            if w.agents[id].vendor == v {
                again = Some(id);
                break;
            }
        }
        let again = again.expect("and again");
        assert_eq!(w.agents[again].name, format!("{}-2", v.label()));
    }
}
