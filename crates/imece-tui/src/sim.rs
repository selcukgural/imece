//! Fake events for the screen prototype, shaped as live's: projects, each
//! with a leader and workers from several vendors that post, claim and
//! assign cards, talk over the bus in typed messages, ask the person and
//! wait for their yes, hire workers and helpers, and finish with a change
//! that waits for the person in WAITING FOR YOU; a capped researcher pool
//! shared by all projects, and symbol leases, show what is still to come.
//! A seed replays the same scenario.

use std::collections::VecDeque;
use std::path::Path;

use crate::i18n::Lang;
use crate::live::Refusal;
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

/// What a project builds and tests with (imece's `run`), and what its
/// agents' sessions ask the person's yes for.
struct Kit {
    build: &'static str,
    test: &'static str,
    /// Where a worker says the project runs once it is built, as a live
    /// worker writes a dev server's address; nothing for an app that
    /// serves nothing.
    serves: Option<Serves>,
    asks: &'static [&'static str],
}

/// A dev server's address: each worker of a project runs its own copy in
/// its worktree, so each takes the next port after the one before.
struct Serves {
    /// Scheme and host.
    at: &'static str,
    /// The first worker's port.
    port: u16,
    path: &'static str,
}

impl Serves {
    /// The address the `n`th worker or helper of the project (from 0)
    /// serves at.
    fn url(&self, n: usize) -> String {
        let port = usize::from(self.port) + n;
        format!("{}:{port}{}", self.at, self.path)
    }
}

const SHOP_KIT: Kit = Kit {
    build: "dotnet build",
    test: "dotnet test",
    serves: Some(Serves {
        at: "http://localhost",
        port: 5080,
        path: "/swagger",
    }),
    asks: &[
        "Bash: dotnet add package Polly --version 8.5.0",
        "Bash: dotnet ef database update",
        "WebFetch: https://learn.microsoft.com/dotnet/api/system.text.json",
        "Edit: .github/workflows/ci.yml",
    ],
};

const MOBILE_KIT: Kit = Kit {
    build: "./gradlew assembleDebug",
    test: "./gradlew testDebugUnitTest",
    serves: None,
    asks: &[
        "Edit: app/build.gradle.kts",
        "Bash: ./gradlew dependencies --refresh-dependencies",
        "WebFetch: https://developer.android.com/jetpack/androidx/releases/compose",
    ],
};

const WEB_KIT: Kit = Kit {
    build: "npm run build",
    test: "npm test",
    serves: Some(Serves {
        at: "http://localhost",
        port: 5173,
        path: "/",
    }),
    asks: &[
        "Bash: npm install zod@3",
        "Edit: package.json",
        "WebFetch: https://react.dev/reference/react/useTransition",
    ],
};

const BILLING_KIT: Kit = Kit {
    build: "go build ./...",
    test: "go test ./...",
    serves: Some(Serves {
        at: "http://127.0.0.1",
        port: 8080,
        path: "/healthz",
    }),
    asks: &[
        "Bash: go get github.com/shopspring/decimal",
        "Edit: go.mod",
        "WebFetch: https://pkg.go.dev/github.com/shopspring/decimal",
    ],
};

const PROJECTS: &[(&str, Repo, &Kit)] = &[
    ("shop-api", SHOP, &SHOP_KIT),
    ("mobile-app", MOBILE, &MOBILE_KIT),
    ("web", WEB, &WEB_KIT),
    ("billing", BILLING, &BILLING_KIT),
];

pub const MAX_PROJECTS: usize = 4;

/// The most workers a demo leader has in a project.
const MAX_WORKERS: usize = 6;
/// Ticks an approval waits before the session stops waiting, as Claude's
/// permission prompt does after 100 seconds.
const APPROVAL_WAIT: u64 = 100 / SECONDS_PER_TICK;
/// Ticks a worker that asked the person waits before it goes on with what
/// does not depend on the answer.
const QUESTION_WAIT: u64 = 40;
/// Why a demo session stops, in the words of a vendor's process.
const STOPPED: &str = "the session ended: the process exited with code 1";

/// What fake agents say, in the person's language.
struct Content {
    titles: &'static [&'static str],
    questions: &'static [&'static str],
    answers: &'static [&'static str],
    assumptions: &'static [&'static str],
    decisions: &'static [&'static str],
    topics: &'static [&'static str],
    /// What a worker asks the person.
    asks_you: &'static [&'static str],
    /// What an agent answers the person who told it something.
    acks: &'static [&'static str],
    /// A leader's answer to a request: how many cards, and which.
    posted: &'static str,
    /// What a worker tells the person it assumes.
    assume: &'static str,
    /// A helper's done to its worker.
    helper_done: &'static str,
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
    asks_you: &[
        "Should a refund go back to the card or to store credit?",
        "May I drop the old v1 endpoint, or do clients still call it?",
        "Half-up rounding for Money, or banker's rounding?",
        "May paging change the shape of the API's response?",
    ],
    acks: &[
        "Understood; I go on that way.",
        "Done, changed as you said.",
        "Noted for this card.",
        "Good point; I add a test for it.",
    ],
    posted: "Posted {} cards for it: {}. The workers take them from the board.",
    assume: "Assuming {}.",
    helper_done: "Tests written; they pass.",
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
    asks_you: &[
        "İade karta mı dönsün, mağaza kredisine mi?",
        "Eski v1 uç noktasını kaldırabilir miyim, çağıran istemci var mı?",
        "Money için yarıya yukarı yuvarlama mı, banker yuvarlaması mı?",
        "Sayfalama API cevabının şeklini değiştirebilir mi?",
    ],
    acks: &[
        "Anlaşıldı, o şekilde devam ediyorum.",
        "Tamam, dediğin gibi değiştirdim.",
        "Bu kart için not aldım.",
        "Haklısın, bunun için de test ekliyorum.",
    ],
    posted: "Bunun için {} kart açtım: {}. İşçiler panodan alıyor.",
    assume: "Varsayım: {}.",
    helper_done: "Testler yazıldı, geçiyor.",
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

/// The build and the tests of project `p`, which has the code map of the
/// same place in [`PROJECTS`].
fn kit(p: ProjectId) -> &'static Kit {
    PROJECTS[p % PROJECTS.len()].2
}

/// How many of agent `id`'s project's workers and helpers came before it,
/// gone ones too: its dev server takes the port after theirs, and keeps
/// it when one of them leaves.
fn serving_before(w: &World, id: AgentId) -> usize {
    let project = w.agents[id].project;
    w.agents[..id]
        .iter()
        .filter(|a| a.project == project && matches!(a.role, Role::Worker | Role::Helper))
        .count()
}

/// The agent's own command line, as a session of its vendor reports it
/// (imece's `/clear` where the vendor has none).
fn commands_of(v: Vendor) -> Vec<String> {
    let words: &[&str] = match v {
        Vendor::Claude => &["/compact", "/cost", "/clear"],
        Vendor::Codex => &["/compact", "/clear"],
        Vendor::Gemini => &["/clear", "/stats"],
        Vendor::Qwen | Vendor::Local => &["/clear"],
    };
    words.iter().map(|w| w.to_string()).collect()
}

/// What the simulation keeps of an agent beyond the world's view of it.
#[derive(Clone, Debug, Default)]
struct Mind {
    /// It has a session: it took a turn.
    started: bool,
    /// The person made it forget its session: the next one is new.
    forgot: bool,
    /// When its tests end, while they run.
    testing: Option<u64>,
    /// When a helper's task ends.
    until: u64,
    /// When it last asked the person.
    asked: u64,
    /// imece's runs on its tree since its last change, the last of each
    /// command.
    runs: Vec<Ran>,
}

/// An answer to the person, due at a tick.
struct Due {
    agent: AgentId,
    at: u64,
    text: String,
}

/// How a worker comes to its card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Taken {
    /// The leader sent it an assign.
    Assigned,
    /// It took the card from the board.
    Claimed,
    /// The person told it what to do.
    Told,
}

pub struct Sim {
    rng: Rng,
    content: &'static Content,
    researchers: usize,
    /// Topics already researched; the next question about them is free.
    cache: Vec<String>,
    /// By agent id.
    minds: Vec<Mind>,
    /// Answers to the person still to come.
    replies: Vec<Due>,
    /// Workers whose change the person applies: git's answer comes on the
    /// next step, as it does from live's git thread.
    applying: Vec<AgentId>,
    /// A session started with the head of long repository notes; said once
    /// a run.
    notes_cut: bool,
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
        minds: Vec::new(),
        replies: Vec::new(),
        applying: Vec::new(),
        notes_cut: false,
    };
    let mut w = World {
        marks: Default::default(),
        tick: 0,
        started_at: 0,
        run_spent: 0,
        spending: Spending::default(),
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
    for (p, &(name, repo, _)) in PROJECTS.iter().take(projects).enumerate() {
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
        // Each agent works in a worktree of its own: no shared trunk.
        w.projects.push(Project {
            id: p,
            name: name.into(),
            leader,
            trunk: None,
            repo,
            closed: false,
            pinned: false,
        });
    }
    // The confine dial's default, said once a start as live does.
    w.note(Event::Confine {
        leader: w.projects[0].leader,
        level: "standard".into(),
    });
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
            sim.post(&mut w, p, title);
        }
    }
    (w, sim)
}

/// Symbols of `project` leased by cards that are not done yet.
pub fn held_symbols(w: &World, project: ProjectId) -> Vec<Symbol> {
    w.cards
        .iter()
        .filter(|c| c.project == project && c.holds_leases())
        .flat_map(|c| c.symbols.iter().copied())
        .collect()
}

/// A short commit id.
/// Where a worktree kept at `at` next to the checkout's `head` stands: with
/// `commits` of its own on a branch the agent made, or with changes; the
/// numbers come from the ids, so the scenario stays the seed's.
fn kept(name: &str, at: &str, head: &str, commits: bool) -> Stand {
    let digit = |s: &str| s.chars().next().and_then(|c| c.to_digit(16)).unwrap_or(0);
    let behind = 1 + digit(head) % 4;
    if commits {
        Stand {
            branch: Some(format!("wip-{name}")),
            ahead: 1 + digit(at) % 3,
            behind,
            ..Stand::default()
        }
    } else {
        Stand {
            behind,
            added: 5 + digit(at) * 3,
            removed: digit(at) % 5,
            ..Stand::default()
        }
    }
}

fn sha(rng: &mut Rng) -> String {
    format!("{:07x}", rng.below(1 << 28))
}

impl Sim {
    /// New fake content comes in this language from now on.
    pub fn set_lang(&mut self, lang: Lang) {
        self.content = content(lang);
    }

    fn mind(&mut self, id: AgentId) -> &mut Mind {
        if self.minds.len() <= id {
            self.minds.resize_with(id + 1, Mind::default);
        }
        &mut self.minds[id]
    }

    /// Whether agent `id` has a card it has not finished.
    fn unfinished(w: &World, id: AgentId) -> bool {
        w.agents[id]
            .card
            .is_some_and(|c| w.cards[c].column != Column::Done)
    }

    /// Whether worker `id` can take a card: idle and between cards.
    fn free(w: &World, id: AgentId) -> bool {
        let a = &w.agents[id];
        a.role == Role::Worker && !a.gone && a.status == Status::Idle && !Self::unfinished(w, id)
    }

    /// The person told agent `id` something, as live's sessions take it:
    /// a leader posts cards for it and says which; a worker between cards
    /// gets a card for it, one in the middle of one goes on (a stopped
    /// one in the session it had, a forgotten one in a new one), and
    /// answers. Refused when the pot is spent.
    pub fn told(&mut self, w: &mut World, id: AgentId, text: &str) -> Result<(), Refusal> {
        if w.spent() >= w.budget {
            return Err(Refusal::Budget);
        }
        let t = w.tick;
        let stopped = w.agents[id].status == Status::Stopped;
        let had = self.mind(id).started;
        self.start(w, id);
        if stopped && had {
            w.agents[id].trace(t, "session resumed");
            w.note(Event::Resumed { agent: id });
        }
        if w.agents[id].role == Role::Leader {
            w.plans.push(Plan {
                project: w.agents[id].project,
                text: text.into(),
                ready: t + 4,
            });
            let a = &mut w.agents[id];
            a.set_status(Status::Working, t);
            a.activity = Some(String::new());
            return Ok(());
        }
        match w.agents[id].role {
            Role::Worker if !Self::unfinished(w, id) => {
                let title = text.lines().next().unwrap_or_default().to_string();
                let card = self.new_card(w, w.agents[id].project, title);
                self.take(w, id, card, Taken::Told);
            }
            Role::Worker | Role::Helper => {
                let a = &mut w.agents[id];
                a.set_status(Status::Working, t);
                if a.activity.is_none() {
                    a.activity = Some(String::new());
                }
            }
            // A researcher answers and goes on with its research.
            _ => {}
        }
        let text = (*self.rng.pick(self.content.acks)).to_string();
        let at = t + self.rng.range(2, 5);
        self.replies.push(Due {
            agent: id,
            at,
            text,
        });
        Ok(())
    }

    /// A line of the agent's own command line (`/compact`), as its
    /// session takes it: only a session has one, `/clear` forgets the
    /// session, and the rest answer like a turn.
    pub fn command(&mut self, w: &mut World, id: AgentId, line: &str) -> Result<(), Refusal> {
        if w.spent() >= w.budget {
            return Err(Refusal::Budget);
        }
        let line = line.trim();
        if !self.mind(id).started {
            return Err(Refusal::Command(
                "it has no session yet; say something to it first".into(),
            ));
        }
        if !w.agents[id].commands.iter().any(|c| c == line) {
            return Err(Refusal::Command(format!("{line}: no such command")));
        }
        let t = w.tick;
        w.agents[id].trace(t, format!("command: {line}"));
        if line == "/clear" {
            self.forget(w, id)?;
            w.agents[id].trace(
                t,
                "session ended on /clear; the next message starts a new one",
            );
            return Ok(());
        }
        let text = match line {
            "/cost" => format!("Total cost: {}", dollars(w.agents[id].cost)),
            "/compact" => "Compacted the conversation.".to_string(),
            _ => format!("{line}: done."),
        };
        self.replies.push(Due {
            agent: id,
            at: t + 1,
            text,
        });
        Ok(())
    }

    /// `:forget` in the demo: agent `id`'s session goes, and it sleeps
    /// until a message starts a new one, told where it left off.
    pub fn forget(&mut self, w: &mut World, id: AgentId) -> Result<(), Refusal> {
        if w.agents[id].gone || !self.mind(id).started {
            return Err(Refusal::Command("it has no session to forget".into()));
        }
        let m = self.mind(id);
        m.started = false;
        m.forgot = true;
        m.testing = None;
        self.replies.retain(|r| r.agent != id);
        w.close_approvals(id);
        let t = w.tick;
        let a = &mut w.agents[id];
        a.activity = None;
        a.set_status(Status::Asleep, t);
        a.trace(
            t,
            "the person made it forget its session; the next message starts a new one, told \
             where it left off",
        );
        w.note(Event::Forgotten { agent: id });
        Ok(())
    }

    /// The person has worker `id`'s change applied: the next step says
    /// how it went. Refused while it is being applied, or when none waits.
    pub fn apply_change(&mut self, w: &World, id: AgentId) -> Result<(), String> {
        if self.applying.contains(&id) {
            return Err("their change is being applied".into());
        }
        if w.change_of(id).is_none() {
            return Err("no change of theirs waits".into());
        }
        self.applying.push(id);
        Ok(())
    }

    /// The person rejects worker `id`'s change; not while it is being
    /// applied.
    pub fn reject_change(&mut self, w: &mut World, id: AgentId) -> Result<(), String> {
        if self.applying.contains(&id) {
            return Err("their change is being applied".into());
        }
        w.change_rejected(id);
        Ok(())
    }

    /// How applying worker `id`'s change went: it `fits` the checkout and
    /// is applied with a mark, or git refuses it and it still waits.
    fn applied(&mut self, w: &mut World, id: AgentId, fits: bool) {
        let Some(file) = w
            .change_of(id)
            .and_then(|c| c.files.first())
            .map(|f| f.path.clone())
        else {
            return;
        };
        if !fits {
            w.change_failed(id, format!("error: {file}: patch does not apply"));
            return;
        }
        let mark = sha(&mut self.rng);
        w.change_applied(id, mark);
        // The worktree is marked at the change: nothing written since.
        let s = &mut w.agents[id].stand;
        s.added = 0;
        s.removed = 0;
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
        for id in std::mem::take(&mut self.applying) {
            let fits = !self.rng.one_in(8);
            self.applied(w, id, fits);
        }
        let (due, later): (Vec<Due>, Vec<Due>) = std::mem::take(&mut self.replies)
            .into_iter()
            .partition(|r| r.at <= t);
        self.replies = later;
        for r in due {
            self.answer(w, r.agent, &r.text);
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
                Role::Helper => self.helper(w, id),
                Role::Researcher => self.researcher(w, id),
                Role::Leader => {}
            }
        }
    }

    /// Agent `id` answers the person: the journal and their inbox get it,
    /// and a leader's turn ends with it.
    fn answer(&mut self, w: &mut World, id: AgentId, text: &str) {
        if w.agents[id].gone {
            return;
        }
        let t = w.tick;
        let card = w.agents[id].card;
        w.story(card, StoryEvent::Replied(text.into()));
        w.note(Event::Replied {
            agent: id,
            text: text.into(),
        });
        w.replied(id, text);
        let a = &mut w.agents[id];
        a.trace(t, format!("result ok {}", dollars(a.cost)));
        a.talk = Some(Talk {
            with: Party::Person,
            text: text.lines().next().unwrap_or_default().to_string(),
            until: t + 12,
        });
        if a.role == Role::Leader {
            a.activity = None;
            a.set_status(Status::Idle, t);
        }
    }

    /// Agent `id`'s session starts, if it has none: its command line comes
    /// with it, and a worker's worktree may say something.
    fn start(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let m = self.mind(id);
        if m.started {
            return;
        }
        m.started = true;
        let forgot = std::mem::take(&mut m.forgot);
        let a = &mut w.agents[id];
        a.commands = commands_of(a.vendor);
        a.trace(t, "session/new");
        if forgot {
            w.note(Event::Restarted { agent: id });
        }
        if w.agents[id].role != Role::Worker {
            return;
        }
        let roll = self.rng.below(12);
        self.worktree(w, id, roll);
    }

    /// What a worker's worktree says as its session starts, by `roll`: it
    /// was moved to the checkout's HEAD, or kept where its own work is, or
    /// the repository's notes were too long (said once a run); mostly
    /// nothing.
    fn worktree(&mut self, w: &mut World, id: AgentId, roll: u64) {
        match roll {
            0 => {
                let (from, to) = (sha(&mut self.rng), sha(&mut self.rng));
                w.agents[id].stand = Stand::default();
                w.note(Event::WorktreeMoved {
                    agent: id,
                    from,
                    to,
                });
            }
            1 => {
                let (at, head) = (sha(&mut self.rng), sha(&mut self.rng));
                let commits = self.rng.one_in(2);
                w.agents[id].stand = kept(&w.agents[id].name, &at, &head, commits);
                w.note(Event::WorktreeKept {
                    agent: id,
                    at,
                    head,
                    commits,
                });
            }
            2 if !self.notes_cut => {
                self.notes_cut = true;
                let file = match w.agents[id].vendor {
                    Vendor::Claude => "CLAUDE.md",
                    _ => "AGENTS.md",
                };
                let size = self.rng.range(20_000, 60_000) as usize;
                w.note(Event::NotesCut {
                    agent: id,
                    file: file.into(),
                    size,
                });
            }
            _ => {}
        }
    }

    /// A worker of a random vendor joins `project`, named `<vendor>-N`
    /// with the next free N, idle; as `--agents` starts them, nobody hired
    /// it.
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
        let name = free_name(w, &format!("{}-", v.label()), so_far + 1);
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

    /// Agent `by` hired agent `id` on a tier of its vendor; a model this
    /// account lacks falls back to the vendor's default, now and then.
    fn hired(&mut self, w: &mut World, by: AgentId, id: AgentId) {
        let t = w.tick;
        let models: [Option<&str>; 3] = match w.agents[id].vendor {
            Vendor::Claude => [Some("haiku"), None, Some("opus")],
            Vendor::Codex => [Some("@low"), None, Some("@high")],
            Vendor::Gemini => [Some("flash"), None, Some("pro")],
            Vendor::Qwen | Vendor::Local => [None; 3],
        };
        let model = models[self.rng.below(3) as usize].map(str::to_string);
        let shown = model.clone().unwrap_or_else(|| "the default model".into());
        let (hirer, name) = (w.agents[by].name.clone(), w.agents[id].name.clone());
        w.agents[by].trace(t, format!("hire {name} {shown}"));
        w.agents[id].trace(t, format!("hired by {hirer} · {shown}"));
        w.note(Event::Hired {
            by,
            agent: id,
            model: model.clone(),
        });
        if let Some(model) = model
            && self.rng.one_in(5)
        {
            w.agents[id].trace(t, format!("model {model} is not available here"));
            w.note(Event::Fallback { agent: id, model });
        }
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
        }
        for _ in 0..2 {
            self.hire(w, p);
        }
        for _ in 0..3 {
            let title = (*self.rng.pick(self.content.titles)).to_string();
            self.post(w, p, title);
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
        self.replies.retain(|r| agents[r.agent].project != p);
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

    /// The leader of `project` puts a card on the board.
    fn post(&mut self, w: &mut World, project: ProjectId, title: String) -> CardId {
        let t = w.tick;
        let leader = w.projects[project].leader;
        let card = self.new_card(w, project, title.clone());
        w.story(
            Some(card),
            StoryEvent::Posted {
                by: leader,
                text: title.clone(),
            },
        );
        w.note(Event::Posted {
            agent: leader,
            card,
        });
        w.agents[leader].trace(t, format!("post #{} {title:?}", card + 1));
        card
    }

    fn leader(&mut self, w: &mut World, project: ProjectId) {
        let t = w.tick;
        let leader = w.projects[project].leader;
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
            let cards: Vec<String> = titles
                .into_iter()
                .map(|title| format!("#{}", self.post(w, project, title) + 1))
                .collect();
            let n = cards.len().to_string();
            let said = crate::i18n::fill(self.content.posted, &[&n, &cards.join(", ")]);
            self.answer(w, leader, &said);
        }
        // A spent pot starts no new turns.
        if w.spent() >= w.budget {
            return;
        }

        // Keep a card waiting for every free worker, so the guild stays busy.
        let workers: Vec<AgentId> = w
            .agents
            .iter()
            .filter(|a| a.project == project && a.role == Role::Worker && !a.gone)
            .map(|a| a.id)
            .collect();
        let free = workers.iter().filter(|&&a| Self::free(w, a)).count();
        let waiting = w
            .cards
            .iter()
            .filter(|c| c.project == project && c.column == Column::Waiting)
            .count();
        if waiting < free && self.rng.one_in(3) {
            let title = (*self.rng.pick(self.content.titles)).to_string();
            self.post(w, project, title);
        }
        // Work waits and nobody is free: one more worker.
        if waiting >= 2 && free == 0 && workers.len() < MAX_WORKERS && self.rng.one_in(40) {
            let id = self.hire(w, project);
            self.hired(w, leader, id);
        }

        // At most two cards a tick, so the screen stays calm: the leader
        // assigns one, or a worker takes one from the board.
        for _ in 0..2 {
            let Some(agent) = workers.iter().copied().find(|&a| Self::free(w, a)) else {
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
            let how = if self.rng.one_in(2) {
                Taken::Assigned
            } else {
                Taken::Claimed
            };
            self.take(w, agent, card, how);
        }
    }

    /// Worker `agent` takes `card` (`how` says who chose it), leasing a
    /// symbol or two nobody else holds, and starts on it.
    fn take(&mut self, w: &mut World, agent: AgentId, card: CardId, how: Taken) {
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
        a.activity = Some(String::new());
        a.set_status(Status::Working, t);
        if !lease.is_empty() {
            a.trace(t, format!("tool_use lonca.lease_acquire [{lease}]"));
        }
        w.story(Some(card), StoryEvent::Took { agent });
        match how {
            Taken::Assigned => {
                let leader = w.projects[project].leader;
                self.send(
                    w,
                    leader,
                    agent,
                    MsgKind::Assign,
                    &format!("#{} {title}", card + 1),
                );
            }
            Taken::Claimed => {
                w.note(Event::Claimed { agent, card });
                w.agents[agent].trace(t, format!("claim #{}", card + 1));
            }
            Taken::Told => {}
        }
        self.start(w, agent);
    }

    /// Agent `from` sends `to` a typed message over the bus.
    fn send(&mut self, w: &mut World, from: AgentId, to: AgentId, kind: MsgKind, text: &str) {
        let t = w.tick;
        let story = StoryEvent::Sent {
            from,
            to,
            kind,
            text: text.into(),
        };
        let (mine, theirs) = (w.agents[from].card, w.agents[to].card);
        w.story(mine, story.clone());
        if theirs != mine {
            w.story(theirs, story);
        }
        w.note(Event::Sent {
            from,
            to,
            kind,
            text: text.into(),
        });
        let (name, sender) = (w.agents[to].name.clone(), w.agents[from].name.clone());
        w.agents[from].trace(t, format!("send to={name} type={}", kind.wire()));
        w.agents[to].trace(t, format!("mail from={sender} type={}", kind.wire()));
        for (a, other) in [(from, to), (to, from)] {
            w.agents[a].talk = Some(Talk {
                with: Party::Agent(other),
                text: text.into(),
                until: t + 8,
            });
        }
    }

    /// Agent `from` sends the person a message; an `ask` waits in their
    /// inbox.
    fn tell_you(&mut self, w: &mut World, from: AgentId, kind: MsgKind, text: &str) {
        let t = w.tick;
        let card = w.agents[from].card;
        w.story(
            card,
            StoryEvent::ToYou {
                kind,
                text: text.into(),
            },
        );
        w.note(Event::ToYou {
            from,
            kind,
            text: text.into(),
        });
        if kind == MsgKind::Ask {
            w.asked_you(from, text);
        }
        let a = &mut w.agents[from];
        a.trace(t, format!("send to=you type={}", kind.wire()));
        a.talk = Some(Talk {
            with: Party::Person,
            text: text.into(),
            until: t + 8,
        });
    }

    fn worker(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        match w.agents[id].status {
            Status::Working => self.work(w, id),
            Status::Asking { to, until } if t >= until => {
                let answer = *self.rng.pick(self.content.answers);
                self.send(w, to, id, MsgKind::Answer, answer);
                w.agents[id].set_status(Status::Working, t);
            }
            Status::Idle
                if w.has_question(id)
                    && Self::unfinished(w, id)
                    && t >= self.mind(id).asked + QUESTION_WAIT =>
            {
                let a = &mut w.agents[id];
                a.set_status(Status::Working, t);
                a.trace(t, "goes on with what does not depend on the answer");
            }
            _ => {}
        }
    }

    fn work(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        // A session that waits for the person's yes does nothing else,
        // until it stops waiting.
        if let Some(since) = w
            .inbox
            .iter()
            .find(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
            .map(|i| i.tick)
        {
            if t >= since + APPROVAL_WAIT {
                w.withdrew(id, 0);
                w.agents[id].trace(t, "no answer in time; the tool call was refused");
            }
            return;
        }
        let Some(card) = w.agents[id].card.filter(|_| Self::unfinished(w, id)) else {
            return;
        };
        if let Some(until) = self.mind(id).testing {
            if t >= until {
                // One run in six fails.
                let code = if self.rng.one_in(6) { 1 } else { 0 };
                self.tested(w, id, code);
            }
            return;
        }
        let progress = (w.agents[id].progress + self.rng.range(1, 2) as u8).min(100);
        w.agents[id].progress = progress;
        if progress >= 100 {
            self.test(w, id);
            return;
        }

        match self.rng.below(1000) {
            0..4 => self.ask(w, id),
            4..6 if w.max_researchers > 0 => self.block(w, id),
            6..9 => {
                let s = *self.rng.pick(self.content.assumptions);
                let said = crate::i18n::fill(self.content.assume, &[s]);
                self.tell_you(w, id, MsgKind::Inform, &said);
            }
            9..11 => self.approval(w, id),
            11..12 => self.ask_you(w, id),
            12..15 => {
                let d = *self.rng.pick(self.content.decisions);
                let leader = w.projects[w.agents[id].project].leader;
                self.send(w, id, leader, MsgKind::Progress, d);
            }
            15..16 if w.helpers_of(id).next().is_none() => {
                self.helper_for(w, id, card);
            }
            16..17 if self.rng.one_in(4) => self.stop(w, id),
            20..70 => self.tool(w, id, card),
            _ => {}
        }
    }

    /// Worker `id` uses a tool on its card's files: what it does shows
    /// in the salon, and a file it edits goes into the card's story once.
    fn tool(&mut self, w: &mut World, id: AgentId, card: CardId) {
        let t = w.tick;
        let first = w.cards[card].symbols.first().copied();
        let kit = kit(w.agents[id].project);
        let (doing, edits, serves) = match (first, self.rng.below(5)) {
            (Some(s), 0) => (format!("Read {}", s.file), None, None),
            (Some(s), 1) => (format!("Edit {}", s.file), Some(s.file), None),
            (Some(s), 2) => (format!("Grep {:?}", s.name), None, None),
            (_, 3) => (
                format!("run {}", kit.build),
                None,
                kit.serves.as_ref().map(|s| s.url(serving_before(w, id))),
            ),
            _ => (String::new(), None, None),
        };
        let a = &mut w.agents[id];
        if !doing.is_empty() {
            a.trace(t, format!("tool_use {doing}"));
        }
        a.activity = Some(doing);
        if let Some(url) = serves {
            // Built, it runs: its address goes on the row as a link.
            let text = format!("It builds; the app runs at {url}.");
            a.wrote(&text);
            a.trace(t, format!("text {text:?}"));
        }
        if edits.is_some() {
            // An edit writes lines, and the row counts them as git would
            // read them. From the clock, so the scenario stays the seed's.
            a.stand.added += 2 + (t % 9) as u32;
            a.stand.removed += (t % 4) as u32;
        }
        if let Some(file) = edits {
            let seen = w.cards[card]
                .story
                .iter()
                .any(|s| matches!(&s.event, StoryEvent::Touched(f) if f == file));
            if !seen {
                w.story(Some(card), StoryEvent::Touched(file.into()));
            }
        }
    }

    /// Worker `id`'s session asks the person's yes for one of its
    /// project's sensitive steps.
    fn approval(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let what = *self.rng.pick(kit(w.agents[id].project).asks);
        w.asks_you(id, what);
        let a = &mut w.agents[id];
        a.activity = Some(what.into());
        a.trace(t, format!("approve? {what}"));
    }

    /// Worker `id` asks the person and ends its turn; it waits for the
    /// answer a while.
    fn ask_you(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let q = *self.rng.pick(self.content.asks_you);
        self.tell_you(w, id, MsgKind::Ask, q);
        self.mind(id).asked = t;
        let a = &mut w.agents[id];
        a.activity = None;
        a.set_status(Status::Idle, t);
    }

    /// Worker `id`'s session ends under it; the person's next message
    /// resumes it.
    fn stop(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let card = w.agents[id].card;
        let a = &mut w.agents[id];
        a.activity = None;
        // What it served went with its session.
        a.local = None;
        a.set_status(Status::Stopped, t);
        a.trace(t, format!("stopped {STOPPED:?}"));
        w.story(card, StoryEvent::Stopped(STOPPED.into()));
        w.note(Event::Stopped {
            agent: id,
            why: STOPPED.into(),
        });
    }

    /// Worker `id` hires a helper of its vendor in its worktree, named
    /// after it (`claude-1.1`), for its card's tests.
    fn helper_for(&mut self, w: &mut World, id: AgentId, card: CardId) -> AgentId {
        let t = w.tick;
        let ever = w
            .agents
            .iter()
            .filter(|a| a.role == Role::Helper && a.parent == Some(id))
            .count();
        let name = free_name(w, &format!("{}.", w.agents[id].name), ever + 1);
        let helper = w.agents.len();
        let (project, vendor) = (w.agents[id].project, w.agents[id].vendor);
        let mut a = Agent::new(helper, project, name, vendor, Role::Helper, Status::Idle, t);
        a.parent = Some(id);
        w.agents.push(a);
        self.hired(w, id, helper);
        let task = format!("{}: {}", w.cards[card].title, self.content.tests);
        self.send(w, id, helper, MsgKind::Assign, &task);
        self.start(w, helper);
        let until = t + self.rng.range(10, 25);
        self.mind(helper).until = until;
        let a = &mut w.agents[helper];
        a.set_status(Status::Working, t);
        a.activity = Some(format!("run {}", kit(project).test));
        helper
    }

    /// A helper works until its task is done, then tells its worker and
    /// leaves.
    fn helper(&mut self, w: &mut World, id: AgentId) {
        if w.agents[id].status != Status::Working || w.tick < self.mind(id).until {
            return;
        }
        let worker = w.agents[id].parent.expect("a helper has a worker");
        self.send(w, id, worker, MsgKind::Done, self.content.helper_done);
        w.leave(id);
        w.note(Event::Left { agent: id });
    }

    /// Worker `id` is through with its card: imece's `run` builds it and
    /// starts its tests.
    fn test(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let kit = kit(w.agents[id].project);
        self.ran(id, kit.build, 0);
        let until = t + self.rng.range(3, 8);
        self.mind(id).testing = Some(until);
        let a = &mut w.agents[id];
        a.trace(t, format!("run {} → exit 0", kit.build));
        a.trace(t, format!("tool_use run {}", kit.test));
        a.activity = Some(format!("run {}", kit.test));
    }

    /// Worker `id`'s tests ended with `code`: when they failed it goes
    /// back to work, else it is done.
    fn tested(&mut self, w: &mut World, id: AgentId, code: i32) {
        let t = w.tick;
        self.mind(id).testing = None;
        let test = kit(w.agents[id].project).test;
        self.ran(id, test, code);
        w.agents[id].trace(t, format!("run {test} → exit {code}"));
        if code != 0 {
            let a = &mut w.agents[id];
            a.progress = 85;
            a.activity = Some(String::new());
            return;
        }
        self.done(w, id);
    }

    /// imece ran `line` for agent `id`, which ended with `code`: the last
    /// run of each command counts.
    fn ran(&mut self, id: AgentId, line: &str, code: i32) {
        let runs = &mut self.mind(id).runs;
        runs.retain(|r| r.line != line);
        runs.push(Ran {
            line: line.into(),
            code: Some(code),
            timed_out: false,
        });
    }

    /// Worker `id` sends its leader done: the change in its worktree, with
    /// the runs on it, waits for the person, and the worker is free.
    fn done(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let Some(card) = w.agents[id].card else {
            return;
        };
        let title = w.cards[card].title.clone();
        let leader = w.projects[w.agents[id].project].leader;
        self.send(w, id, leader, MsgKind::Done, &title);
        let (files, patch) = self.patch(&w.cards[card].symbols);
        let runs = std::mem::take(&mut self.mind(id).runs);
        let change = Change {
            agent: id,
            title,
            files,
            patch,
            runs,
        };
        let (added, removed) = change.counts();
        let n = change.files.len();
        w.change_ready(change);
        let c = &mut w.cards[card];
        c.column = Column::Done;
        c.finished = t;
        let a = &mut w.agents[id];
        a.progress = 0;
        a.activity = None;
        a.set_status(Status::Idle, t);
        a.trace(t, format!("change: {n} files +{added} −{removed}"));
        // What the person reviews is what the worktree holds.
        a.stand.added = added;
        a.stand.removed = removed;
    }

    /// A patch that changes each of `symbols` in its file, as `git diff`
    /// writes one, with its files; a card that leased nothing changes the
    /// README.
    fn patch(&mut self, symbols: &[Symbol]) -> (Vec<ChangedFile>, String) {
        let mut by_file: Vec<(&str, Vec<&str>)> = Vec::new();
        for s in symbols {
            match by_file.iter_mut().find(|(f, _)| *f == s.file) {
                Some((_, names)) => names.push(s.name),
                None => by_file.push((s.file, vec![s.name])),
            }
        }
        if by_file.is_empty() {
            by_file.push(("README.md", vec!["Usage"]));
        }
        let mut files = Vec::new();
        let mut patch = String::new();
        for (file, names) in by_file {
            patch.push_str(&format!(
                "diff --git a/{file} b/{file}\n--- a/{file}\n+++ b/{file}\n"
            ));
            let mut at = self.rng.range(8, 40);
            for name in &names {
                let short = name.rsplit('.').next().unwrap_or(name);
                patch.push_str(&format!(
                    "@@ -{at},3 +{at},4 @@\n {name}\n-    return {short}(input)\n\
                     +    let checked = validate(input)\n+    return {short}(checked)\n }}\n"
                ));
                at += 40;
            }
            let n = names.len() as u32;
            files.push(ChangedFile {
                path: file.into(),
                added: 2 * n,
                removed: n,
                binary: false,
            });
        }
        (files, patch)
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
        self.send(w, id, to, MsgKind::Ask, q);
        w.agents[id].set_status(Status::Asking { to, until }, t);
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
                    card,
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
}

/// `stem` followed by the first number from `from` on that no agent
/// here goes by. The search is bounded: there are fewer agents than the
/// numbers it tries.
fn free_name(w: &World, stem: &str, from: usize) -> String {
    (from..=from + w.agents.len())
        .map(|n| format!("{stem}{n}"))
        .find(|name| w.agent_by_name(name).is_none())
        .expect("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The person answers everything that waits for them: changes are
    /// applied, asks allowed, questions answered, answers read, reports
    /// approved.
    fn answer_all(w: &mut World, s: &mut Sim) {
        for i in w.open_inbox() {
            let a = w.inbox[i].agent;
            match w.inbox[i].kind {
                InboxKind::Change => {
                    let _ = s.apply_change(w, a);
                }
                InboxKind::Approval => w.decided(i, true),
                InboxKind::Question => {
                    if s.told(w, a, "yes").is_ok() {
                        w.said(a, "yes");
                    }
                }
                InboxKind::Reply => w.close_replies(a),
                InboxKind::Sensitive | InboxKind::Research => w.approve(i),
            }
        }
    }

    fn run(w: &mut World, s: &mut Sim, ticks: usize, answer: bool) {
        for _ in 0..ticks {
            s.step(w);
            if answer {
                answer_all(w, s);
            }
        }
    }

    fn journal(w: &World) -> Vec<String> {
        w.journal
            .iter()
            .map(|e| format!("{} {:?}", e.tick, e.event))
            .collect()
    }

    /// An event's name, with the kind of message or who hired.
    fn name(w: &World, e: &Event) -> String {
        let debug = format!("{e:?}");
        let head = debug.split([' ', '{', '(']).next().unwrap_or_default();
        match e {
            Event::Sent { kind, .. } | Event::ToYou { kind, .. } => format!("{head}:{kind:?}"),
            Event::Hired { by, .. } => format!("{head}:{:?}", w.agents[*by].role),
            _ => head.to_string(),
        }
    }

    fn workers_of(w: &World, p: ProjectId) -> Vec<AgentId> {
        w.agents
            .iter()
            .filter(|a| a.project == p && a.role == Role::Worker && !a.gone)
            .map(|a| a.id)
            .collect()
    }

    /// A demo of one project and one worker, at work on the first card.
    fn one_at_work() -> (World, Sim, AgentId, CardId) {
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::En);
        let worker = workers_of(&w, 0)[0];
        let card = w
            .cards
            .iter()
            .position(|c| c.column == Column::Waiting)
            .unwrap();
        s.take(&mut w, worker, card, Taken::Claimed);
        (w, s, worker, card)
    }

    #[test]
    fn a_seed_replays_the_same_scenario() {
        let (mut a, mut sa) = world(2, 12, 4, 7, Lang::En);
        let (mut b, mut sb) = world(2, 12, 4, 7, Lang::En);
        run(&mut a, &mut sa, 600, true);
        run(&mut b, &mut sb, 600, true);
        assert!(!a.journal.is_empty());
        assert_eq!(journal(&a), journal(&b));
    }

    #[test]
    fn researchers_never_exceed_the_cap() {
        let (mut w, mut s) = world(3, 60, 3, 11, Lang::En);
        for _ in 0..3000 {
            s.step(&mut w);
            assert!(w.active_researchers() <= 3);
            answer_all(&mut w, &mut s);
        }
        assert!(
            w.agents.iter().any(|a| a.role == Role::Researcher),
            "no researcher was ever opened"
        );
    }

    #[test]
    fn without_researchers_nobody_blocks_on_a_question() {
        let (mut w, mut s) = world(1, 8, 0, 11, Lang::En);
        run(&mut w, &mut s, 2000, true);
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Blocked { .. })),
            "nobody would ever answer it"
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
            answer_all(&mut w, &mut s);
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
            if let Some(p) = a.parent {
                assert_eq!(w.agents[p].project, a.project, "{}", a.name);
            }
        }
        for p in 0..4 {
            assert!(!workers_of(&w, p).is_empty(), "project {p} has no worker");
        }
    }

    /// Everything live shows comes up in a long demo run where the person
    /// answers, asks for things and talks to stopped agents; nothing of
    /// the old merge queue does.
    #[test]
    fn a_long_run_shows_what_live_shows_and_nothing_live_does_not() {
        let (mut w, mut s) = world(2, 6, 2, 7, Lang::En);
        for tick in 0..4000 {
            s.step(&mut w);
            answer_all(&mut w, &mut s);
            for id in 0..w.agents.len() {
                assert!(w.helpers_of(id).count() <= 1, "one helper at a time");
            }
            if tick % 150 == 0 {
                for p in 0..2 {
                    let lead = w.projects[p].leader;
                    s.told(&mut w, lead, "Shipment tracking").unwrap();
                    w.said(lead, "Shipment tracking");
                }
            }
            let stopped: Vec<AgentId> = (w.agents.iter())
                .filter(|a| a.status == Status::Stopped)
                .map(|a| a.id)
                .collect();
            for id in stopped {
                s.told(&mut w, id, "go on").unwrap();
                w.said(id, "go on");
            }
        }
        let seen: std::collections::BTreeSet<String> =
            w.journal.iter().map(|e| name(&w, &e.event)).collect();
        for want in [
            "Confine",
            "Posted",
            "Claimed",
            "Sent:Assign",
            "Sent:Ask",
            "Sent:Answer",
            "Sent:Progress",
            "Sent:Done",
            "ToYou:Inform",
            "ToYou:Ask",
            "Decided",
            "ChangeReady",
            "Applied",
            "ApplyFailed",
            "Replied",
            "Hired:Leader",
            "Hired:Worker",
            "Fallback",
            "Left",
            "Stopped",
            "Resumed",
            "YouSaid",
            "Blocked",
            "ResearchOpened",
        ] {
            assert!(seen.contains(want), "{want} never came: {seen:?}");
        }
        for old in [
            "Plan",
            "Talk",
            "Assign",
            "Assumption",
            "Merged",
            "Failed",
            "Sensitive",
        ] {
            assert!(!seen.contains(old), "{old} is not live's");
        }
        let kinds: std::collections::BTreeSet<String> =
            w.inbox.iter().map(|i| format!("{:?}", i.kind)).collect();
        for want in ["Change", "Approval", "Question", "Reply"] {
            assert!(kinds.contains(want), "no {want} item: {kinds:?}");
        }
        assert!(w.projects.iter().all(|p| p.trunk.is_none()));
        assert!(
            w.cards.iter().any(|c| c
                .story
                .iter()
                .any(|s| matches!(s.event, StoryEvent::Touched(_)))),
            "nobody edited a file"
        );
    }

    #[test]
    fn the_demo_starts_as_live_does_with_the_confine_dial_and_posted_cards() {
        let (w, _) = world(2, 3, 0, 1, Lang::En);
        assert_eq!(
            w.journal[0].event,
            Event::Confine {
                leader: w.projects[0].leader,
                level: "standard".into()
            }
        );
        let posted: Vec<CardId> = w
            .journal
            .iter()
            .filter_map(|e| match e.event {
                Event::Posted { agent, card } => {
                    assert_eq!(agent, w.projects[w.cards[card].project].leader);
                    Some(card)
                }
                _ => None,
            })
            .collect();
        assert_eq!(posted, (0..w.cards.len()).collect::<Vec<_>>());
        for c in &w.cards {
            assert!(matches!(
                &c.story[0].event,
                StoryEvent::Posted { by, text } if *by == w.projects[c.project].leader && *text == c.title
            ));
        }
        assert_eq!(
            w.journal.len(),
            1 + w.cards.len(),
            "nobody hired at the start"
        );
        assert!(w.projects.iter().all(|p| p.trunk.is_none()));
    }

    #[test]
    fn free_means_an_idle_worker_between_cards() {
        let (mut w, mut s, worker, card) = one_at_work();
        let lead = w.projects[0].leader;
        assert!(!Sim::free(&w, lead), "a leader takes no card");
        assert!(!Sim::free(&w, worker), "working");
        w.agents[worker].status = Status::Idle;
        assert!(!Sim::free(&w, worker), "its card is not done");
        w.cards[card].column = Column::Done;
        assert!(Sim::free(&w, worker));
        w.agents[worker].gone = true;
        assert!(!Sim::free(&w, worker));
        w.agents[worker].gone = false;
        w.agents[worker].card = None;
        assert!(Sim::free(&w, worker));
        assert!(s.mind(worker).started, "taking a card starts a session");
    }

    #[test]
    fn a_finished_card_waits_as_a_change_with_its_patch_and_its_runs() {
        let (mut w, mut s, worker, card) = one_at_work();
        let lead = w.projects[0].leader;
        let title = w.cards[card].title.clone();
        w.agents[worker].progress = 99;
        s.step(&mut w);
        let t = w.tick;
        let until = s.mind(worker).testing.expect("its tests run");
        assert!((t + 3..=t + 8).contains(&until), "{until}");
        assert_eq!(
            w.agents[worker].activity.as_deref(),
            Some("run dotnet test")
        );
        assert_eq!(
            w.cards[card].column,
            Column::Working,
            "not done while they run"
        );
        // A range: a step that does not move the clock cannot hang it.
        for _ in w.tick + 1..until {
            s.step(&mut w);
        }
        assert!(w.change_of(worker).is_none(), "not before they end");
        assert!(s.mind(worker).testing.is_some());
        s.step(&mut w);
        assert_eq!(s.mind(worker).testing, None, "they ended");
        if w.change_of(worker).is_none() {
            // They failed: once more, passing.
            s.test(&mut w, worker);
            s.tested(&mut w, worker, 0);
        }
        let change = w.change_of(worker).expect("a change waits").clone();
        assert_eq!(change.title, title);
        let n = w.journal.len();
        assert!(matches!(
            &w.journal[n - 2].event,
            Event::Sent { from, to, kind: MsgKind::Done, text } if *from == worker && *to == lead && *text == title
        ));
        assert!(matches!(
            w.journal[n - 1].event,
            Event::ChangeReady { agent, .. } if agent == worker
        ));
        // The files are the card's leases, their counts the patch's.
        let mut files: Vec<&str> = w.cards[card].symbols.iter().map(|s| s.file).collect();
        files.dedup();
        let paths: Vec<&str> = change.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, files);
        let plus = change
            .patch
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count();
        let minus = change
            .patch
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .count();
        assert_eq!(change.counts(), (plus as u32, minus as u32));
        let stand = &w.agents[worker].stand;
        assert_eq!((stand.added, stand.removed), change.counts(), "the row");
        assert_eq!(
            change.runs,
            [
                Ran {
                    line: "dotnet build".into(),
                    code: Some(0),
                    timed_out: false
                },
                Ran {
                    line: "dotnet test".into(),
                    code: Some(0),
                    timed_out: false
                },
            ]
        );
        assert!(
            s.mind(worker).runs.is_empty(),
            "the next change has its own"
        );
        assert_eq!(w.cards[card].column, Column::Done);
        assert_eq!(w.cards[card].finished, w.tick);
        let a = &w.agents[worker];
        assert_eq!(
            (a.status, a.progress, a.card),
            (Status::Idle, 0, Some(card))
        );
        assert_eq!(a.activity, None);
        assert!(Sim::free(&w, worker), "it takes the next card");
    }

    #[test]
    fn failed_tests_send_the_worker_back_and_the_last_run_counts() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 1);
        assert!(w.change_of(worker).is_none());
        assert_eq!(s.mind(worker).testing, None);
        let a = &w.agents[worker];
        assert_eq!((a.progress, a.activity.as_deref()), (85, Some("")));
        assert_eq!(s.mind(worker).runs[1].code, Some(1));
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        let runs = &w.change_of(worker).unwrap().runs;
        assert_eq!(runs.len(), 2, "one of each command: {runs:?}");
        assert!(runs.iter().all(Ran::ok));
    }

    #[test]
    fn a_patch_groups_symbols_by_file_and_a_card_without_leases_changes_the_readme() {
        let (_, mut s) = world(1, 0, 0, 1, Lang::En);
        let (files, patch) = s.patch(&[]);
        assert_eq!(files.len(), 1);
        assert_eq!(
            (files[0].path.as_str(), files[0].added, files[0].removed),
            ("README.md", 2, 1)
        );
        assert!(patch.starts_with("diff --git a/README.md b/README.md\n"));
        let syms = [
            Symbol {
                file: "a.cs",
                name: "A.One",
            },
            Symbol {
                file: "b.cs",
                name: "B",
            },
            Symbol {
                file: "a.cs",
                name: "A.Two",
            },
        ];
        let (files, patch) = s.patch(&syms);
        let counts: Vec<(&str, u32, u32)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.added, f.removed))
            .collect();
        assert_eq!(counts, [("a.cs", 4, 2), ("b.cs", 2, 1)]);
        assert!(patch.contains(
            "-    return One(input)\n+    let checked = validate(input)\n+    return One(checked)\n"
        ));
        let at: Vec<u64> = patch
            .lines()
            .filter_map(|l| l.strip_prefix("@@ -"))
            .map(|l| l.split(',').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(at.len(), 3);
        assert_eq!(at[1], at[0] + 40, "the next hunk further down");
        assert!(files.iter().all(|f| !f.binary));
    }

    #[test]
    fn applying_answers_on_the_next_step_and_refuses_twice() {
        let (mut w, mut s, worker, _) = one_at_work();
        assert_eq!(
            s.apply_change(&w, worker),
            Err("no change of theirs waits".into())
        );
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        assert_eq!(s.apply_change(&w, worker), Ok(()));
        assert_eq!(
            s.apply_change(&w, worker),
            Err("their change is being applied".into())
        );
        assert_eq!(
            s.reject_change(&mut w, worker),
            Err("their change is being applied".into())
        );
        assert!(w.change_of(worker).is_some(), "not before the next step");
        s.step(&mut w);
        assert!(s.applying.is_empty());
        assert!(w.journal.iter().any(|e| matches!(
            e.event,
            Event::Applied { agent, .. } | Event::ApplyFailed { agent, .. } if agent == worker
        )));
    }

    #[test]
    fn a_change_that_fits_is_applied_with_a_mark_one_that_does_not_still_waits() {
        let (mut w, mut s, worker, _) = one_at_work();
        let lines = |w: &World| (w.agents[worker].stand.added, w.agents[worker].stand.removed);
        s.applied(&mut w, worker, true);
        assert!(
            w.journal
                .iter()
                .all(|e| !matches!(e.event, Event::Applied { .. })),
            "none waits"
        );
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        let held = lines(&w);
        assert_ne!(held, (0, 0));
        let file = w.change_of(worker).unwrap().files[0].path.clone();
        s.applied(&mut w, worker, false);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::ApplyFailed {
                agent: worker,
                why: format!("error: {file}: patch does not apply")
            }
        );
        assert!(w.change_of(worker).is_some());
        assert_eq!(lines(&w), held, "not applied: still in the worktree");
        w.agents[worker].stand.ahead = 2;
        s.applied(&mut w, worker, true);
        assert!(w.change_of(worker).is_none());
        assert_eq!(lines(&w), (0, 0), "marked");
        assert_eq!(w.agents[worker].stand.ahead, 2, "its own commits stay");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Applied { agent, mark, .. } if *agent == worker && mark.len() == 7
                && mark.chars().all(|c| c.is_ascii_hexdigit())
        ));
    }

    #[test]
    fn most_changes_fit() {
        let (mut w, mut s, worker, _) = one_at_work();
        let (mut fit, mut not) = (0, 0);
        for _ in 0..64 {
            s.test(&mut w, worker);
            s.tested(&mut w, worker, 0);
            s.apply_change(&w, worker).unwrap();
            let n = w.journal.len();
            s.step(&mut w);
            let how = w.journal[n..].iter().find_map(|e| match e.event {
                Event::Applied { agent, .. } if agent == worker => Some(true),
                Event::ApplyFailed { agent, .. } if agent == worker => Some(false),
                _ => None,
            });
            match how.expect("it says how it went") {
                true => fit += 1,
                false => {
                    not += 1;
                    w.change_rejected(worker);
                }
            }
            w.agents[worker].status = Status::Working;
            let c = w.agents[worker].card.unwrap();
            w.cards[c].column = Column::Working;
        }
        assert!(not > 0 && fit > 2 * not, "{fit} fit, {not} not");
    }

    #[test]
    fn rejecting_a_change_drops_it_and_the_worker_takes_the_reasons_as_its_next_card() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        assert_eq!(s.reject_change(&mut w, worker), Ok(()));
        assert!(w.change_of(worker).is_none());
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Rejected { agent: worker }
        );
        let cards = w.cards.len();
        s.told(&mut w, worker, "Keep the old rounding\nIt breaks invoices.")
            .unwrap();
        assert_eq!(w.cards.len(), cards + 1, "a card for what it was told");
        let c = &w.cards[cards];
        assert_eq!(c.title, "Keep the old rounding");
        assert_eq!((c.owner, c.column), (Some(worker), Column::Working));
        assert!(matches!(c.story[0].event, StoryEvent::Took { agent } if agent == worker));
        assert_eq!(w.agents[worker].card, Some(cards));
        assert_eq!(w.agents[worker].status, Status::Working);
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Claimed { card, .. } if card == cards)),
            "nobody claimed it"
        );
        // Told again in the middle of it: no other card.
        s.told(&mut w, worker, "and the tests").unwrap();
        assert_eq!(w.cards.len(), cards + 1);
    }

    #[test]
    fn a_told_agent_answers_a_few_steps_later() {
        let (mut w, mut s, worker, _) = one_at_work();
        let t = w.tick;
        s.told(&mut w, worker, "Use cents").unwrap();
        let due = s.replies[0].at;
        assert!((t + 2..=t + 5).contains(&due), "{due}");
        for _ in w.tick + 1..due {
            s.step(&mut w);
        }
        let replied = |w: &World| {
            w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Replied { agent, .. } if agent == worker))
        };
        assert!(!replied(&w));
        s.step(&mut w);
        let text = match &w
            .journal
            .iter()
            .rev()
            .find(|e| matches!(e.event, Event::Replied { .. }))
            .unwrap()
            .event
        {
            Event::Replied { text, .. } => text.clone(),
            _ => unreachable!(),
        };
        assert!(EN.acks.contains(&text.as_str()), "{text}");
        let open = w.open_inbox();
        assert!(open.iter().any(|&i| w.inbox[i].kind == InboxKind::Reply
            && w.inbox[i].agent == worker
            && w.inbox[i].detail == text));
        let a = &w.agents[worker];
        assert_eq!(a.status, Status::Working, "a worker goes on");
        assert!(
            matches!(&a.talk, Some(Talk { with: Party::Person, text: said, until }) if *said == text && *until == w.tick + 12)
        );
        assert!(s.replies.is_empty());
    }

    #[test]
    fn a_researcher_told_something_answers_and_goes_on_researching() {
        let (mut w, mut s) = world(1, 1, 1, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        w.agents[worker].topic = "JWT clock skew".into();
        s.spawn_researcher(&mut w, worker);
        let r = w.agents.len() - 1;
        let before = w.agents[r].status;
        s.told(&mut w, r, "hurry").unwrap();
        assert_eq!(w.agents[r].status, before);
        assert_eq!(w.cards.iter().filter(|c| c.owner == Some(r)).count(), 0);
    }

    #[test]
    fn a_leader_posts_cards_for_a_request_and_says_which() {
        let (mut w, mut s) = world(2, 4, 2, 1, Lang::Tr);
        let leader = w.projects[1].leader;
        s.told(&mut w, leader, "Kargo takibi").unwrap();
        assert_eq!(w.agents[leader].status, Status::Working);
        assert_eq!(w.agents[leader].activity.as_deref(), Some(""));
        let cards = w.cards.len();
        for _ in 0..3 {
            s.step(&mut w);
        }
        assert!(
            !w.cards[cards..].iter().any(|c| c.title == "Kargo takibi"),
            "not yet"
        );
        s.step(&mut w);
        let mine: Vec<&Card> = w.cards[cards..]
            .iter()
            .filter(|c| c.title.starts_with("Kargo takibi"))
            .collect();
        let titles: Vec<&str> = mine.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Kargo takibi",
                "Kargo takibi: testler",
                "Kargo takibi: doküman"
            ]
        );
        assert!(mine.iter().all(|c| c.project == 1));
        let numbers: Vec<String> = mine.iter().map(|c| format!("#{}", c.id + 1)).collect();
        let want = crate::i18n::fill(TR.posted, &["3", &numbers.join(", ")]);
        assert!(w.journal.iter().any(|e| e.event
            == Event::Replied {
                agent: leader,
                text: want.clone()
            }));
        assert!(
            w.open_inbox()
                .iter()
                .any(|&i| w.inbox[i].kind == InboxKind::Reply && w.inbox[i].agent == leader)
        );
        let a = &w.agents[leader];
        assert_eq!((a.status, a.activity.as_deref()), (Status::Idle, None));
    }

    #[test]
    fn a_spent_pot_refuses_words_and_commands_and_starts_no_work() {
        let (mut w, mut s) = world(1, 2, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        w.budget = w.spent() + 1;
        assert_eq!(s.told(&mut w, lead, "hi"), Ok(()));
        w.budget = w.spent();
        assert_eq!(s.told(&mut w, lead, "hi"), Err(Refusal::Budget));
        assert_eq!(s.command(&mut w, lead, "/cost"), Err(Refusal::Budget));
        let waiting = |w: &World| {
            w.cards
                .iter()
                .filter(|c| c.column == Column::Waiting)
                .count()
        };
        let before = waiting(&w);
        s.leader(&mut w, 0);
        assert_eq!(waiting(&w), before, "nobody takes a card");
    }

    #[test]
    fn the_leader_hires_when_two_cards_wait_and_nobody_is_free_up_to_six_workers() {
        // `n` workers, all at work, and `waiting` cards on the board.
        let setup = |n: usize, waiting: usize| {
            let (mut w, mut s) = world(1, n, 0, 5, Lang::En);
            for c in &mut w.cards {
                c.column = Column::Done;
            }
            for id in workers_of(&w, 0) {
                let c = s.new_card(&mut w, 0, "busy".into());
                s.take(&mut w, id, c, Taken::Claimed);
            }
            for _ in 0..waiting {
                s.new_card(&mut w, 0, "waits".into());
            }
            (w, s)
        };
        let hired = |w: &World| {
            w.journal
                .iter()
                .filter(|e| matches!(e.event, Event::Hired { by: 0, .. }))
                .count()
        };
        let (mut w, mut s) = setup(5, 2);
        for _ in 0..400 {
            s.leader(&mut w, 0);
            if hired(&w) > 0 {
                break;
            }
        }
        assert_eq!(hired(&w), 1, "five busy, two waiting: one more");
        let new = w.agents.len() - 1;
        assert_eq!(w.agents[new].role, Role::Worker);
        assert_eq!(w.agents[new].project, 0);
        for (n, waiting) in [(6, 2), (5, 1)] {
            let (mut w, mut s) = setup(n, waiting);
            for _ in 0..400 {
                s.leader(&mut w, 0);
            }
            assert_eq!(hired(&w), 0, "{n} workers, {waiting} waiting");
        }
    }

    #[test]
    fn a_hire_gets_a_model_of_its_vendor_and_some_fall_back() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        let mut models = std::collections::BTreeSet::new();
        let mut fell = 0;
        for v in [Vendor::Claude, Vendor::Codex, Vendor::Gemini, Vendor::Qwen] {
            w.agents[worker].vendor = v;
            for _ in 0..60 {
                let n = w.journal.len();
                s.hired(&mut w, 0, worker);
                let Event::Hired { by, agent, model } = &w.journal[n].event else {
                    panic!("{:?}", w.journal[n].event);
                };
                assert_eq!((*by, *agent), (0, worker));
                models.insert((v.label(), model.clone()));
                if let Some(Event::Fallback { agent, model: m }) =
                    w.journal.get(n + 1).map(|e| &e.event)
                {
                    assert_eq!(*agent, worker);
                    assert_eq!(Some(m), model.as_ref(), "the model it was given");
                    fell += 1;
                }
            }
        }
        let want: std::collections::BTreeSet<(&str, Option<String>)> = [
            ("claude", Some("haiku")),
            ("claude", None),
            ("claude", Some("opus")),
            ("codex", Some("@low")),
            ("codex", None),
            ("codex", Some("@high")),
            ("gemini", Some("flash")),
            ("gemini", None),
            ("gemini", Some("pro")),
            ("qwen", None),
        ]
        .into_iter()
        .map(|(v, m)| (v, m.map(str::to_string)))
        .collect();
        assert_eq!(models, want);
        assert!(fell > 0 && fell < 60, "{fell}");
    }

    #[test]
    fn a_worker_hires_a_helper_that_says_done_and_leaves() {
        let (mut w, mut s, worker, card) = one_at_work();
        let name = w.agents[worker].name.clone();
        let title = w.cards[card].title.clone();
        let h = s.helper_for(&mut w, worker, card);
        let a = &w.agents[h];
        assert_eq!(a.name, format!("{name}.1"));
        assert_eq!(
            (a.role, a.parent, a.project),
            (Role::Helper, Some(worker), 0)
        );
        assert_eq!(a.vendor, w.agents[worker].vendor);
        assert_eq!(a.status, Status::Working);
        assert!(s.mind(h).started);
        assert!(w.journal.iter().any(
            |e| matches!(e.event, Event::Hired { by, agent, .. } if by == worker && agent == h)
        ));
        assert!(w.journal.iter().any(|e| e.event
            == Event::Sent {
                from: worker,
                to: h,
                kind: MsgKind::Assign,
                text: format!("{title}: tests")
            }));
        let until = s.mind(h).until;
        let t = w.tick;
        assert!((t + 10..=t + 25).contains(&until), "{until}");
        while w.tick + 1 < until {
            w.tick += 1;
            s.helper(&mut w, h);
        }
        assert!(!w.agents[h].gone, "not before its task is done");
        w.tick += 1;
        s.helper(&mut w, h);
        assert!(w.agents[h].gone);
        let n = w.journal.len();
        assert_eq!(
            w.journal[n - 2].event,
            Event::Sent {
                from: h,
                to: worker,
                kind: MsgKind::Done,
                text: EN.helper_done.into()
            }
        );
        assert_eq!(w.journal[n - 1].event, Event::Left { agent: h });
        // A helper that left is still counted: the next is the second.
        let h2 = s.helper_for(&mut w, worker, card);
        assert_eq!(w.agents[h2].name, format!("{name}.2"));
        // One that does not work does not leave.
        w.agents[h2].status = Status::Idle;
        w.tick = s.mind(h2).until;
        s.helper(&mut w, h2);
        assert!(!w.agents[h2].gone);
    }

    #[test]
    fn a_helpers_number_counts_only_its_own_workers_helpers() {
        let (mut w, mut s) = world(1, 2, 0, 4, Lang::En);
        let ws = workers_of(&w, 0);
        let card = s.new_card(&mut w, 0, "x".into());
        let theirs = s.helper_for(&mut w, ws[1], card);
        assert_eq!(w.agents[theirs].name, format!("{}.1", w.agents[ws[1]].name));
        let mine = s.helper_for(&mut w, ws[0], card);
        assert_eq!(
            w.agents[mine].name,
            format!("{}.1", w.agents[ws[0]].name),
            "another worker's helper is not mine"
        );
    }

    #[test]
    fn a_card_taken_says_its_leases_and_one_without_says_none() {
        let lease_lines = |w: &World, id: AgentId| {
            (w.agents[id].trace.iter())
                .filter(|l| l.contains("lonca.lease_acquire"))
                .cloned()
                .collect::<Vec<_>>()
        };
        let (w, _, worker, card) = one_at_work();
        let lines = lease_lines(&w, worker);
        assert_eq!(lines.len(), 1, "{lines:?}");
        let s0 = w.cards[card].symbols[0];
        assert!(
            lines[0].contains(&format!("{}#{}", s0.file, s0.name)),
            "{lines:?}"
        );
        // A project without a code map leases nothing, and says nothing.
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::En);
        w.projects[0].repo = &[];
        let worker = workers_of(&w, 0)[0];
        let card = s.new_card(&mut w, 0, "x".into());
        s.take(&mut w, worker, card, Taken::Claimed);
        assert!(w.cards[card].symbols.is_empty());
        assert!(lease_lines(&w, worker).is_empty());
    }

    #[test]
    fn free_names_skip_the_taken_and_stay_bounded() {
        let (w, _) = world(1, 0, 0, 1, Lang::En);
        assert_eq!(free_name(&w, "lea", 1), "lea1");
        assert_eq!(free_name(&w, "x-", 3), "x-3");
        let mut w = w;
        w.agents[0].name = "x-3".into();
        assert_eq!(free_name(&w, "x-", 3), "x-4");
        assert_eq!(free_name(&w, "x-", 2), "x-2");
    }

    #[test]
    fn an_approval_holds_the_worker_until_decided_or_given_up_on() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.approval(&mut w, worker);
        let item = w.inbox.len() - 1;
        let it = &w.inbox[item];
        assert_eq!(
            (it.kind, it.agent, it.open),
            (InboxKind::Approval, worker, true)
        );
        assert!(SHOP_KIT.asks.contains(&it.detail.as_str()));
        assert_eq!(
            w.agents[worker].activity.as_deref(),
            Some(it.detail.as_str())
        );
        let progress = w.agents[worker].progress;
        for _ in 0..5 {
            s.step(&mut w);
        }
        assert_eq!(w.agents[worker].progress, progress, "it waits");
        w.decided(item, true);
        s.step(&mut w);
        assert!(w.agents[worker].progress > progress, "and goes on");

        // Unanswered: the session gives up after 100 seconds, nobody decided.
        s.approval(&mut w, worker);
        let item = w.inbox.len() - 1;
        let asked = w.inbox[item].tick;
        let decided = |w: &World| {
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::Decided { .. }))
                .count()
        };
        let before = decided(&w);
        for _ in w.tick + 1..asked + APPROVAL_WAIT {
            s.step(&mut w);
        }
        assert!(w.inbox[item].open);
        s.step(&mut w);
        assert!(!w.inbox[item].open);
        assert_eq!(decided(&w), before, "nobody decided");
        assert_eq!(APPROVAL_WAIT, 20);
    }

    #[test]
    fn a_worker_that_asked_the_person_waits_then_goes_on_and_an_answer_wakes_it() {
        let (mut w, mut s, worker, card) = one_at_work();
        s.ask_you(&mut w, worker);
        let asked = w.tick;
        let q = match &w.journal.last().unwrap().event {
            Event::ToYou {
                from,
                kind: MsgKind::Ask,
                text,
            } if *from == worker => text.clone(),
            e => panic!("{e:?}"),
        };
        assert!(EN.asks_you.contains(&q.as_str()));
        assert!(w.has_question(worker));
        assert!(matches!(
            &w.cards[card].story.last().unwrap().event,
            StoryEvent::ToYou {
                kind: MsgKind::Ask,
                ..
            }
        ));
        assert_eq!(w.agents[worker].status, Status::Idle);
        assert!(!Sim::free(&w, worker), "its card waits for it");
        for _ in w.tick + 1..asked + QUESTION_WAIT {
            s.step(&mut w);
        }
        assert_eq!(w.agents[worker].status, Status::Idle);
        s.step(&mut w);
        assert_eq!(w.agents[worker].status, Status::Working, "goes on");
        assert!(w.has_question(worker), "the question still waits");

        // Asked again; the answer wakes it at once.
        s.ask_you(&mut w, worker);
        assert_eq!(w.agents[worker].status, Status::Idle);
        s.told(&mut w, worker, "store credit").unwrap();
        w.said(worker, "store credit");
        assert_eq!(w.agents[worker].status, Status::Working);
        assert!(!w.has_question(worker));
        // Between cards, a question still open does not hold it back.
        s.ask_you(&mut w, worker);
        w.cards[card].column = Column::Done;
        assert!(w.has_question(worker) && Sim::free(&w, worker));
    }

    #[test]
    fn a_stopped_worker_resumes_in_its_session_when_told() {
        let (mut w, mut s, worker, card) = one_at_work();
        s.stop(&mut w, worker);
        assert_eq!(w.agents[worker].status, Status::Stopped);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Stopped {
                agent: worker,
                why: STOPPED.into()
            }
        );
        assert!(
            matches!(&w.cards[card].story.last().unwrap().event, StoryEvent::Stopped(why) if why == STOPPED)
        );
        s.told(&mut w, worker, "go on").unwrap();
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Resumed { agent: worker }
        );
        assert_eq!(w.agents[worker].status, Status::Working);
        assert_eq!(w.agents[worker].card, Some(card), "on the same card");
        // Told while it runs: nothing to resume.
        let n = w.journal.len();
        s.told(&mut w, worker, "more").unwrap();
        assert_eq!(w.journal.len(), n);
        // Stopped without a session (it never took a turn): a new one.
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        w.agents[worker].status = Status::Stopped;
        let n = w.journal.len();
        s.told(&mut w, worker, "start").unwrap();
        assert!(
            w.journal[n..]
                .iter()
                .all(|e| !matches!(e.event, Event::Resumed { .. }))
        );
    }

    #[test]
    fn a_forgotten_agent_sleeps_and_wakes_in_a_new_session() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        let none = Err(Refusal::Command("it has no session to forget".into()));
        assert_eq!(s.forget(&mut w, lead), none);
        s.told(&mut w, lead, "hello").unwrap();
        w.asks_you(lead, "Bash: ls");
        assert_eq!(s.forget(&mut w, lead), Ok(()));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Forgotten { agent: lead }
        );
        assert_eq!(w.agents[lead].status, Status::Asleep);
        assert_eq!(w.agents[lead].activity, None);
        assert!(!w.has_approval(lead), "what it asked waits for nobody");
        assert_eq!(s.forget(&mut w, lead), none, "nothing left to forget");
        // Its plan goes on; its answer does not come.
        s.replies.push(Due {
            agent: lead,
            at: w.tick + 1,
            text: "x".into(),
        });
        s.forget(&mut w, lead).unwrap_err();
        s.told(&mut w, lead, "again").unwrap();
        assert!(
            w.journal
                .iter()
                .any(|e| e.event == Event::Restarted { agent: lead })
        );
        assert!(!s.mind(lead).forgot);
        assert_eq!(s.forget(&mut w, lead), Ok(()));
        assert!(s.replies.iter().all(|r| r.agent != lead));
        w.agents[lead].gone = true;
        s.told(&mut w, lead, "back").unwrap();
        assert_eq!(s.forget(&mut w, lead), none, "gone");
    }

    #[test]
    fn commands_need_a_session_and_answer_as_a_turn() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        assert_eq!(
            s.command(&mut w, lead, "/cost"),
            Err(Refusal::Command(
                "it has no session yet; say something to it first".into()
            ))
        );
        s.told(&mut w, lead, "hello").unwrap();
        assert_eq!(w.agents[lead].commands, ["/compact", "/cost", "/clear"]);
        assert_eq!(
            s.command(&mut w, lead, "/stats"),
            Err(Refusal::Command("/stats: no such command".into()))
        );
        w.agents[lead].cost = 12_345;
        s.replies.clear();
        assert_eq!(s.command(&mut w, lead, " /cost "), Ok(()));
        assert_eq!(s.replies[0].at, w.tick + 1);
        assert_eq!(s.replies[0].text, "Total cost: $1.23");
        assert_eq!(s.command(&mut w, lead, "/compact"), Ok(()));
        assert_eq!(s.replies[1].text, "Compacted the conversation.");
        w.agents[lead].commands.push("/stats".into());
        assert_eq!(s.command(&mut w, lead, "/stats"), Ok(()));
        assert_eq!(s.replies[2].text, "/stats: done.");
        assert_eq!(s.command(&mut w, lead, "/clear"), Ok(()));
        assert_eq!(w.agents[lead].status, Status::Asleep);
        assert!(
            w.journal
                .iter()
                .any(|e| e.event == Event::Forgotten { agent: lead })
        );
        assert!(s.replies.is_empty(), "a forgotten session answers nothing");
    }

    #[test]
    fn each_vendor_has_its_command_line() {
        let words = |v| commands_of(v);
        assert_eq!(words(Vendor::Claude), ["/compact", "/cost", "/clear"]);
        assert_eq!(words(Vendor::Codex), ["/compact", "/clear"]);
        assert_eq!(words(Vendor::Gemini), ["/clear", "/stats"]);
        for v in [Vendor::Qwen, Vendor::Local] {
            assert_eq!(words(v), ["/clear"], "{v:?}");
        }
    }

    #[test]
    fn a_workers_first_session_may_say_where_its_worktree_is() {
        let (mut w, mut s, worker, _) = one_at_work();
        let n = w.journal.len();
        s.worktree(&mut w, worker, 0);
        assert!(
            matches!(&w.journal[n].event, Event::WorktreeMoved { agent, from, to } if *agent == worker && from.len() == 7 && to.len() == 7)
        );
        s.worktree(&mut w, worker, 1);
        assert!(
            matches!(&w.journal[n + 1].event, Event::WorktreeKept { agent, at, head, .. } if *agent == worker && at.len() == 7 && head.len() == 7)
        );
        w.agents[worker].vendor = Vendor::Codex;
        s.worktree(&mut w, worker, 2);
        assert!(
            matches!(&w.journal[n + 2].event, Event::NotesCut { agent, file, size } if *agent == worker && file == "AGENTS.md" && (20_000..=60_000).contains(size))
        );
        s.worktree(&mut w, worker, 2);
        s.worktree(&mut w, worker, 3);
        assert_eq!(
            w.journal.len(),
            n + 3,
            "notes are cut once a run; 3 says nothing"
        );
        let (mut w, mut s, worker, _) = one_at_work();
        w.agents[worker].vendor = Vendor::Claude;
        s.worktree(&mut w, worker, 2);
        assert!(
            matches!(&w.journal.last().unwrap().event, Event::NotesCut { file, .. } if file == "CLAUDE.md")
        );
        // Kept with commits, or with changes only: both come up, and the
        // row says which; moved, it says nothing.
        let mut both = std::collections::BTreeSet::new();
        for _ in 0..32 {
            s.worktree(&mut w, worker, 1);
            let Event::WorktreeKept {
                at, head, commits, ..
            } = &w.journal.last().unwrap().event
            else {
                panic!("kept");
            };
            assert_eq!(
                w.agents[worker].stand,
                kept(&w.agents[worker].name, at, head, *commits)
            );
            both.insert(*commits);
        }
        assert_eq!(both.len(), 2);
        s.worktree(&mut w, worker, 0);
        assert_eq!(w.agents[worker].stand, Stand::default());
    }

    #[test]
    fn a_kept_worktree_stands_by_its_ids() {
        assert_eq!(
            kept("codex-1", "5ab", "6cd", true),
            Stand {
                branch: Some("wip-codex-1".into()),
                ahead: 3,
                behind: 3,
                ..Stand::default()
            }
        );
        assert_eq!(
            kept("codex-1", "7ab", "f00", false),
            Stand {
                behind: 4,
                added: 26,
                removed: 2,
                ..Stand::default()
            }
        );
        assert_eq!(
            kept("x", "0", "0", true),
            Stand {
                branch: Some("wip-x".into()),
                ahead: 1,
                behind: 1,
                ..Stand::default()
            }
        );
    }

    #[test]
    fn a_session_starts_once_and_only_a_worker_hears_of_its_worktree() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        let n = w.journal.len();
        s.start(&mut w, lead);
        assert!(s.mind(lead).started);
        assert_eq!(w.agents[lead].commands, commands_of(Vendor::Claude));
        assert_eq!(w.journal.len(), n, "a leader has no worktree");
        w.agents[lead].commands.clear();
        s.start(&mut w, lead);
        assert!(w.agents[lead].commands.is_empty(), "once");
    }

    #[test]
    fn each_worker_serves_on_its_own_port_after_the_ones_before_it() {
        let (mut w, mut s) = world(2, 2, 0, 3, Lang::En);
        // The second shop worker comes after the mobile one.
        s.hire(&mut w, 0);
        let shop = workers_of(&w, 0);
        let mobile = workers_of(&w, 1);
        assert!(shop[0] < mobile[0] && mobile[0] < shop[1]);
        assert_eq!(serving_before(&w, shop[0]), 0);
        assert_eq!(serving_before(&w, shop[1]), 1);
        assert_eq!(
            serving_before(&w, mobile[0]),
            0,
            "another project's workers and leaders do not count"
        );
        let card = w
            .cards
            .iter()
            .position(|c| c.column == Column::Waiting && c.project == 0)
            .unwrap();
        let helper = s.helper_for(&mut w, shop[0], card);
        assert_eq!(serving_before(&w, helper), 2, "a helper counts");
        w.agents[shop[0]].gone = true;
        assert_eq!(serving_before(&w, shop[1]), 1, "a gone one keeps its port");
        let shop_kit = kit(0).serves.as_ref().unwrap();
        assert_eq!(shop_kit.url(0), "http://localhost:5080/swagger");
        assert_eq!(shop_kit.url(3), "http://localhost:5083/swagger");
        let billing = PROJECTS.iter().find(|p| p.2.build.starts_with("go"));
        let billing = billing.unwrap().2.serves.as_ref().unwrap();
        assert_eq!(billing.url(2), "http://127.0.0.1:8082/healthz");
    }

    #[test]
    fn a_built_project_that_serves_puts_its_address_on_the_row_until_the_session_stops() {
        let (mut w, mut s, worker, card) = one_at_work();
        // A range: a roll that never builds cannot hang the test.
        for _ in 0..200 {
            s.tool(&mut w, worker, card);
            let built = w.agents[worker].activity.as_deref() == Some("run dotnet build");
            if built {
                break;
            }
            assert_eq!(w.agents[worker].local, None, "not before it is built");
        }
        assert_eq!(
            w.agents[worker].local.as_deref(),
            Some("http://localhost:5080/swagger")
        );
        let said = w.agents[worker].trace.back().unwrap();
        assert!(
            said.ends_with(r#"text "It builds; the app runs at http://localhost:5080/swagger.""#),
            "{said}"
        );
        s.stop(&mut w, worker);
        assert_eq!(w.agents[worker].local, None, "it went with the session");

        // An app that serves nothing puts nothing there.
        let (mut w, mut s) = world(2, 2, 0, 3, Lang::En);
        let worker = workers_of(&w, 1)[0];
        let card = w
            .cards
            .iter()
            .position(|c| c.column == Column::Waiting && c.project == 1)
            .unwrap();
        s.take(&mut w, worker, card, Taken::Claimed);
        for _ in 0..200 {
            s.tool(&mut w, worker, card);
        }
        let ran = w.agents[worker]
            .trace
            .iter()
            .any(|l| l.contains("run ./gradlew assembleDebug"));
        assert!(ran, "it built");
        assert_eq!(w.agents[worker].local, None);
    }

    #[test]
    fn tool_use_shows_what_the_worker_does_and_a_file_it_edits_once() {
        let (mut w, mut s, worker, card) = one_at_work();
        let file = w.cards[card].symbols[0].file;
        let mut doing = std::collections::BTreeSet::new();
        for _ in 0..200 {
            s.tool(&mut w, worker, card);
            doing.insert(w.agents[worker].activity.clone().unwrap());
        }
        let sym = w.cards[card].symbols[0].name;
        let want: std::collections::BTreeSet<String> = [
            format!("Read {file}"),
            format!("Edit {file}"),
            format!("Grep {sym:?}"),
            "run dotnet build".to_string(),
            String::new(),
        ]
        .into_iter()
        .collect();
        assert_eq!(doing, want);
        // Each edit adds to the row's lines, by the clock; nothing else
        // does.
        for t in 100..140 {
            w.tick = t;
            let before = w.agents[worker].stand.clone();
            s.tool(&mut w, worker, card);
            let after = &w.agents[worker].stand;
            let edit = w.agents[worker].activity.as_deref() == Some(&format!("Edit {file}"));
            let (added, removed) = if edit {
                (2 + (t % 9) as u32, (t % 4) as u32)
            } else {
                (0, 0)
            };
            assert_eq!(
                (after.added - before.added, after.removed - before.removed),
                (added, removed),
                "at {t}"
            );
        }
        let touched = w.cards[card]
            .story
            .iter()
            .filter(|s| matches!(&s.event, StoryEvent::Touched(f) if f == file))
            .count();
        assert_eq!(touched, 1);
        assert!(
            w.agents[worker]
                .trace
                .iter()
                .any(|l| l.ends_with(&format!("tool_use Edit {file}")))
        );
        assert!(
            !w.agents[worker]
                .trace
                .iter()
                .any(|l| l.ends_with("tool_use "))
        );
    }

    #[test]
    fn closing_a_project_drops_its_answers_still_to_come() {
        let (mut w, mut s) = world(2, 2, 0, 1, Lang::En);
        let (a, b) = (w.projects[0].leader, w.projects[1].leader);
        s.told(&mut w, a, "x").unwrap();
        s.told(&mut w, b, "y").unwrap();
        s.replies = vec![
            Due {
                agent: a,
                at: 99,
                text: "a".into(),
            },
            Due {
                agent: b,
                at: 99,
                text: "b".into(),
            },
        ];
        s.close_project(&mut w, 1).unwrap();
        assert_eq!(s.replies.len(), 1);
        assert_eq!(s.replies[0].agent, a);
    }

    #[test]
    fn a_gone_agent_answers_nothing_and_a_leader_ends_its_turn_with_its_answer() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        let worker = workers_of(&w, 0)[0];
        w.agents[worker].gone = true;
        let n = w.journal.len();
        s.answer(&mut w, worker, "late");
        assert_eq!(w.journal.len(), n);
        w.agents[lead].status = Status::Working;
        w.agents[lead].activity = Some(String::new());
        s.answer(&mut w, lead, "done");
        assert_eq!(w.agents[lead].status, Status::Idle);
        assert_eq!(w.agents[lead].activity, None);
    }

    #[test]
    fn the_demo_opens_a_project_with_a_lead_two_workers_and_cards_and_reopens_it_by_name() {
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        assert_eq!(s.open_project(&mut w, "~/work/shop", None), Ok(1));
        let p = &w.projects[1];
        assert_eq!((p.name.as_str(), p.trunk, p.closed), ("shop", None, false));
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
        let posted = w
            .journal
            .iter()
            .filter(
                |e| matches!(e.event, Event::Posted { agent, .. } if agent == w.projects[1].leader),
            )
            .count();
        assert_eq!(posted, 3, "the new leader posts them");
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Opened { leader, path } if *leader == w.projects[1].leader && path == "~/work/shop"
        )));
        assert_eq!(s.open_project(&mut w, "/x", Some("api")), Ok(2));
        assert_eq!(w.projects[2].repo, PROJECTS[2].1);
        assert_eq!(kit(2).test, "npm test");
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
        s.told(&mut w, lead, "add a thing").unwrap();
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

    #[test]
    fn a_short_commit_id_is_seven_hex_digits() {
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let s = sha(&mut rng);
            assert_eq!(s.len(), 7, "{s}");
            assert!(s.chars().all(|c| c.is_ascii_hexdigit()), "{s}");
        }
    }

    #[test]
    fn the_leader_posts_a_card_only_when_fewer_wait_than_workers_are_free() {
        // Two free workers and `waiting` cards: did the leader post one?
        let posts = |waiting: usize| {
            let mut any = false;
            for seed in 0..60 {
                let (mut w, mut s) = world(1, 2, 0, seed, Lang::En);
                for c in &mut w.cards {
                    c.column = Column::Done;
                }
                for _ in 0..waiting {
                    s.new_card(&mut w, 0, "waits".into());
                }
                let n = w.journal.len();
                s.leader(&mut w, 0);
                any |= w.journal[n..]
                    .iter()
                    .any(|e| matches!(e.event, Event::Posted { .. }));
            }
            any
        };
        assert!(posts(1), "one waits, two are free");
        assert!(!posts(2), "as many wait as are free");
    }

    #[test]
    fn a_teammates_question_lands_on_both_cards_and_its_answer_comes_in_time() {
        let (mut w, mut s) = world(1, 2, 0, 4, Lang::En);
        let ws: Vec<AgentId> = (w.agents.iter())
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect();
        for &id in &ws {
            let c = s.new_card(&mut w, 0, "x".into());
            s.take(&mut w, id, c, Taken::Claimed);
        }
        let (asker, other) = (ws[0], ws[1]);
        s.ask(&mut w, asker);
        let Status::Asking { to, until } = w.agents[asker].status else {
            panic!("{:?}", w.agents[asker].status);
        };
        assert_eq!(to, other);
        let t = w.tick;
        for id in [asker, other] {
            let c = w.agents[id].card.unwrap();
            assert!(
                matches!(&w.cards[c].story.last().unwrap().event, StoryEvent::Sent { from, to, kind: MsgKind::Ask, .. } if *from == asker && *to == other),
                "{id}"
            );
            assert!(matches!(&w.agents[id].talk, Some(Talk { until, .. }) if *until == t + 8));
        }
        assert!((t + 3..=t + 8).contains(&until), "{until}");
        while w.tick + 1 < until {
            w.tick += 1;
            s.worker(&mut w, asker);
        }
        assert!(matches!(w.agents[asker].status, Status::Asking { .. }));
        w.tick += 1;
        s.worker(&mut w, asker);
        assert_eq!(w.agents[asker].status, Status::Working);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Sent { from, to, kind: MsgKind::Answer, text } if *from == other && *to == asker && EN.answers.contains(&text.as_str())
        ));
    }

    #[test]
    fn a_message_to_the_person_shows_for_a_while() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let worker = w.agents.iter().find(|a| a.role == Role::Worker).unwrap().id;
        w.tick = 10;
        s.tell_you(&mut w, worker, MsgKind::Inform, "Assuming x.");
        assert!(!w.has_question(worker), "an inform asks nothing");
        assert!(matches!(
            &w.agents[worker].talk,
            Some(Talk {
                with: Party::Person,
                until: 18,
                ..
            })
        ));
        assert!(
            w.agents[worker]
                .trace
                .back()
                .unwrap()
                .ends_with("send to=you type=inform")
        );
    }
}
