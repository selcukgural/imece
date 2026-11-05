//! Fake events for the screen prototype, shaped as live's: projects, each
//! with a leader and workers that post, claim and assign cards, talk over the bus in typed messages, ask the person and
//! wait for their yes, hire workers and helpers, and finish with a change
//! that waits for the person in WAITING FOR YOU; a capped researcher pool
//! shared by all projects, and symbol leases, show what is still to come.
//! A seed replays the same scenario.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::i18n::Lang;
use crate::live::setup;
use crate::live::{
    REVIEW_CAP, REVIEW_DAY_CALLS, REVIEW_DAY_COST, Refusal, SUGGEST_CAP, SUGGEST_DAY_CALLS,
    SUGGEST_DAY_COST, TidyRefusal, numbers, standing,
};
use crate::model::*;
use crate::rng::Rng;
use imece_agent::Tier;

type Repo = &'static [(&'static str, &'static [&'static str])];

/// The start point the demo's second worker of a project is hired from.
const DEMO_START: &str = "#41";

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
        "Bash: dotnet ef migrations script > migrate.sql",
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
        "Bash: gh pr view \"$(git branch --show-current)\" 2>&1",
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

/// What the agents spend in the demo for each percent of the plan's
/// five-hour window, in ten-thousandths of a dollar.
const LIMIT_COST: u64 = 200;
const FIVE_HOURS: u32 = imece_agent::FIVE_HOURS;
const WEEK: u32 = imece_agent::WEEK;

/// The most workers a demo leader has in a project.
const MAX_WORKERS: usize = 6;
/// Fake cost of one busy tick, in ten-thousandths of a dollar.
const COST_PER_TICK: u64 = 8;
/// Ticks an approval waits before the demo's session gives up on it.
const APPROVAL_WAIT: u64 = 100 / SECONDS_PER_TICK;
/// Ticks a worker that asked the person waits before it goes on with what
/// does not depend on the answer.
const QUESTION_WAIT: u64 = 40;
/// Ticks a session takes to start (live: the process, a first worktree's
/// checkout), its row saying `◌ starting…` meanwhile; the work under it
/// goes on as before, so the demo's course is unchanged.
const STARTING: u64 = 3;
/// Why a demo session stops, in the words of Claude Code's process.
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
    /// The leader's question to the person after a no to its plan.
    plan_asked: &'static str,
    /// What a worker tells the person it assumes.
    assume: &'static str,
    /// A helper's done to its worker.
    helper_done: &'static str,
    /// The leader's inform to its other workers when the person applied
    /// a change, sent to the group `workers` as live's leader may.
    moved: &'static str,
    tests: &'static str,
    docs: &'static str,
    /// A worker's own to-do list for a card, as its planning tool would
    /// hold it; filled in as its progress grows.
    steps: [&'static str; STEPS],
}

/// Steps on a demo worker's to-do list.
const STEPS: usize = 5;

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
    plan_asked: "You said no to the plan: what should change before I hire?",
    assume: "Assuming {}.",
    helper_done: "Tests written; they pass.",
    moved: "A change was applied: the checkout moved; your worktree follows it when clean.",
    tests: "tests",
    docs: "docs",
    steps: [
        "Read the code around it",
        "Make the change",
        "Write the tests",
        "Run the tests",
        "Write it up for the leader",
    ],
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
    plan_asked: "Plana hayır dediniz: işe almadan önce ne değişmeli?",
    assume: "Varsayım: {}.",
    helper_done: "Testler yazıldı, geçiyor.",
    moved: "Bir değişiklik uygulandı: checkout ilerledi; worktree'n temizken onu izler.",
    tests: "testler",
    docs: "doküman",
    steps: [
        "Çevresindeki kodu oku",
        "Değişikliği yap",
        "Testleri yaz",
        "Testleri çalıştır",
        "Lidere yaz",
    ],
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

/// The files a patch changes and its lines added and removed in each,
/// as `--numstat` counts them.
fn files_of(patch: &str) -> Vec<ChangedFile> {
    let mut files: Vec<ChangedFile> = Vec::new();
    let mut in_hunk = false;
    for l in patch.lines() {
        if let Some(names) = l.strip_prefix("diff --git ") {
            let path = names.rsplit_once(" b/").map_or(names, |(_, p)| p);
            files.push(ChangedFile {
                path: path.into(),
                added: 0,
                removed: 0,
                binary: false,
            });
            in_hunk = false;
        } else if l.starts_with("@@") {
            in_hunk = true;
        } else if let Some(f) = files.last_mut().filter(|_| in_hunk) {
            match l.chars().next() {
                Some('+') => f.added += 1,
                Some('-') => f.removed += 1,
                _ => {}
            }
        }
    }
    files
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

/// The agent's own command line, as a session reports it.
fn commands_of() -> Vec<String> {
    ["/compact", "/cost", "/clear"].map(String::from).into()
}

/// What the simulation keeps of an agent beyond the world's view of it.
#[derive(Clone, Debug, Default)]
struct Mind {
    /// It has a session: it took a turn.
    started: bool,
    /// The person made it forget its session: the next one is new.
    forgot: bool,
    /// Claude Code would not resume its session: the next one is new.
    refused: bool,
    /// When its tests end, while they run.
    testing: Option<u64>,
    /// When a helper's task ends.
    until: u64,
    /// When it last asked the person.
    asked: u64,
    /// imece's runs on its tree since its last change, the last of each
    /// command.
    runs: Vec<Ran>,
    /// The question it sent a teammate, until the answer, which replies
    /// to it.
    question: Option<MsgId>,
    /// The assign that gave it its card or task, which its done replies
    /// to.
    assigned: Option<MsgId>,
    /// Tokens in its context window: its work fills it, a compaction
    /// empties it, its session's end forgets it.
    filled: u64,
}

/// A demo session's context window, what a request and a compaction leave
/// in it, and the fill at which the session compacts on its own, as the
/// Claude Code's sessions do near their limit.
const WINDOW: u64 = 200_000;
const FRESH_CONTEXT: u64 = 20_000;
const PER_TICK: u64 = 1_600;
const AUTO_COMPACT: u64 = WINDOW * 9 / 10;

/// An answer to the person, due at a tick.
struct Due {
    agent: AgentId,
    at: u64,
    text: String,
    /// The turn failed, `text` says why.
    failed: bool,
    /// The fault it failed on, as Claude Code says one.
    fault: Option<Fault>,
}

/// Of the person's messages a demo agent answers with a turn, every
/// `PASSING_EVERY`th fails on an overloaded API (and is tried again),
/// every `ACCOUNT_EVERY`th on a refused key (and waits for the person).
const PASSING_EVERY: u32 = 3;
const ACCOUNT_EVERY: u32 = 7;
/// What the demo says when a turn fails, as Claude Code does.
const OVERLOADED: &str = "API Error: Repeated 529 Overloaded errors. The API is at capacity.";
const BAD_KEY: &str = "Invalid API key · Fix external API key";

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
    /// The `[MODEL][@EFFORT]` choice each solo agent runs on, when the
    /// person named one.
    models: HashMap<AgentId, String>,
    /// Workers whose change the person applies: git's answer comes on the
    /// next step, as it does from live's git thread.
    /// Changes being applied, with the hunks chosen (none: all).
    applying: Vec<(AgentId, Vec<usize>)>,
    /// A helper's session listed no command line this run (said once).
    unlisted: bool,
    /// What the person's rarer commands keep, boxed: the engine stays
    /// small.
    extras: Box<Extras>,
}

/// The fault the `n`th message of the person's that a demo agent answers
/// fails on, if any: an account's before a passing one.
fn fault_of(n: u32) -> Option<Fault> {
    if n.is_multiple_of(ACCOUNT_EVERY) {
        Some(Fault::Account)
    } else if n.is_multiple_of(PASSING_EVERY) {
        Some(Fault::Passing)
    } else {
        None
    }
}

/// What the demo keeps for `:commit` and `:copy`.
#[derive(Default)]
struct Extras {
    /// The demo's Lonca store, a table (record 116): each repository and
    /// how many changes it has integrated.
    store: Vec<(String, u32)>,
    /// The `lonca` command under way and the tick it comes back on.
    job: Option<(StoreJob, u64)>,
    /// How many integrations each project's checkout has of its
    /// repository, by repository, and whether it has commits of its own.
    synced: HashMap<String, (u32, bool)>,
    /// How many times `:sync` ran: every third finds commits of the
    /// person's own, so the demo shows that too.
    syncs: u32,
    /// The listed paths each worker's worktree has, copied at a start.
    copied: HashMap<AgentId, Vec<String>>,
    /// The titles of the changes applied to each project's checkout, as
    /// if staged there; the newest last.
    staged: HashMap<ProjectId, Vec<String>>,
    /// The commit message being suggested: for which project, and the
    /// tick it answers at.
    suggesting: Option<(ProjectId, u64)>,
    /// The last suggestion of each project and what was staged for it:
    /// asked again for the same, it comes again without a call.
    suggestions: HashMap<ProjectId, (Vec<String>, String)>,
    /// The suggestions of this run and what they cost, for the day's
    /// ceilings (the demo's day is its run).
    paid_calls: (u32, u64),
    /// The reviews asked for (`:review`), each with the
    /// tick it answers on, oldest first: one at a time, as live's.
    reviews: VecDeque<(AgentId, u64)>,
    /// The reviews of this run and what they cost, for the day's
    /// ceilings.
    review_calls: (u32, u64),
    /// The person's messages answered with a turn this run.
    told: u32,
    /// No API errors: a test that counts answers.
    calm: bool,
    /// Sessions resumed this run, refused ones included.
    resumes: u32,
    /// The workers whose worktree the setup script was run in or asked
    /// about (once each, at its first start).
    set_up: std::collections::HashSet<AgentId>,
    /// Workers hired this run: their worktrees are new, as live's made
    /// at a hire (the demo's first workers' are as old as the demo).
    hired: std::collections::HashSet<AgentId>,
    /// The setup asks open: the item, the script's key, the workers that
    /// went without it meanwhile.
    setup_asks: Vec<(usize, String, Vec<AgentId>)>,
    /// The setup scripts and the check the person said no to this run.
    setup_refused: Vec<String>,
    /// The demo's checks under way (record 112): the worker whose change
    /// it runs on and the tick it ends at.
    checks_due: Vec<(AgentId, u64)>,
    /// Checks that ended this run: every third fails.
    checks_ran: u32,
    /// The demo's worktrees no agent owns (`:tidy`), made up at the first
    /// look; a look or a removal under way, answered at the next step,
    /// with the removal's end.
    orphans: Option<Vec<Orphan>>,
    tidy_due: Option<Option<Tidied>>,
    /// The teardown scripts the person said no to this run.
    teardown_refused: Vec<String>,
    /// The folder `:open` or `:solo` was given, by project: what the
    /// screen says it works in.
    folders: HashMap<ProjectId, String>,
}

/// The demo's worktree teardown script, the same in every checkout.
const DEMO_TEARDOWN: &str = "#!/bin/sh\n\
# Tears down a worktree before it goes: its database.\n\
set -e\n\
cd \"$IMECE_WORKTREE\"\n\
npm run db:drop\n";

/// The demo's teardown script as live reads it from `checkout`.
pub(crate) fn teardown_script(checkout: &Path) -> setup::Script {
    let bytes = DEMO_TEARDOWN.as_bytes().to_vec();
    setup::Script {
        kind: setup::Kind::Teardown,
        real: checkout.join(setup::TEARDOWN_FILE),
        sha: crate::sha::hex(&bytes),
        bytes,
    }
}

/// The demo's worktrees no agent owns, of its first project's checkout:
/// one clean, one with work of its own, as a crash would leave them; and
/// of a project on Lonca a workspace whose session is over, which nobody
/// can read.
fn demo_orphans(w: &World) -> Vec<Orphan> {
    let name = &w.projects[0].name;
    let checkout = PathBuf::from(format!("~/src/{name}"));
    let root = PathBuf::from(format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c"));
    let tree = |agent: &str, changes, commits| Orphan {
        name: agent.to_string(),
        path: root.join(agent),
        checkout: checkout.clone(),
        changes,
        commits,
        unread: false,
    };
    let worktrees = [tree("claude-31", 0, 0), tree("claude-32", 3, 1)].into_iter();
    let workspace = w.open_projects().find(|p| p.lonca.is_some()).map(|p| {
        let name = &p.name;
        let root = format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c");
        Orphan {
            name: "claude-33".to_string(),
            path: PathBuf::from(root).join("claude-33"),
            checkout: PathBuf::from(format!("~/src/{name}")),
            changes: 0,
            commits: 0,
            unread: true,
        }
    });
    worktrees.chain(workspace).collect()
}

/// The demo's worktree setup script, the same in every project.
const DEMO_SETUP: &str = "#!/bin/sh\n\
# Sets up a new worktree: its dependencies and its database.\n\
set -e\n\
cd \"$IMECE_WORKTREE\"\n\
npm ci --no-audit\n\
npm run db:migrate\n";

/// What the demo's setup script says as it ends.
const DEMO_SETUP_SAYS: [&str; 2] = ["added 412 packages in 3s", "migrations: 7 applied"];

/// The repository's check every demo checkout has (`.imece/check`,
/// record 112), run on each change as live runs it.
const DEMO_CHECK: &str = "#!/bin/sh\n\
# Builds and tests the change in the worktree.\n\
set -e\n\
cd \"$IMECE_WORKTREE\"\n\
npm run build\n\
npm test -- --silent\n";

/// What a failed demo check says last.
const DEMO_CHECK_SAYS: [&str; 3] = [
    "FAIL src/parser.test.ts",
    "  ● parses a nested list › keeps the indentation",
    "Tests: 1 failed, 211 passed",
];

/// How long the demo's check takes, in ticks.
const CHECK_TICKS: u64 = 2;

/// What the demo says when it would not resume a session.
const REFUSED: &str = "the session could not resume session 5f1c… (error_during_execution)";

/// What the first-run window (`:welcome`) says of the demo's Claude Code:
/// there and signed in.
pub fn seen() -> crate::welcome::Seen {
    crate::welcome::Seen::Here {
        version: "2.1.0 (Claude Code) · demo".into(),
        login: imece_agent::Login::Yes,
    }
}

/// The size the demo gives a copied path, from `roll`: a dotfile is a
/// few KiB at most (100 to 3 999 bytes), anything else a directory of
/// 1 to 400 MiB.
/// What the second demo project's `.worktreeinclude` names.
const INCLUDED: &str = "node_modules";

/// The line of that file that reaches outside its checkout.
const REFUSED_LINE: (usize, &str) = (2, "../shared/.env");

fn made_up_size(path: &str, roll: u64) -> u64 {
    match path.starts_with('.') {
        true => 100 + roll % 3_900,
        false => (1 + roll % 400) << 20,
    }
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
        models: HashMap::new(),
        applying: Vec::new(),
        unlisted: false,
        extras: Box::default(),
    };
    let mut w = World {
        marks: Default::default(),
        cleared: Default::default(),
        setup_approved: Vec::new(),
        sent: Vec::new(),
        limits: Vec::new(),
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
        clashes: Default::default(),
        paid: 0,
        suggested: None,
        reviews: Default::default(),
        orphans: crate::model::Orphans::default(),
        messages: 0,
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
            files: Vec::new(),
            repo,
            closed: false,
            pinned: false,
            copies: Vec::new(),
            allowed: Vec::new(),
            plan: PlanGate::default(),
            no_git: false,
            lonca: None,
            archived: None,
        });
    }
    // The demo's store knows every project; the second one is on it.
    sim.extras.store = (w.projects.iter()).map(|q| (q.name.clone(), 0)).collect();
    if let Some(q) = w.projects.get(1) {
        let repo = q.name.clone();
        w.on_lonca(1, Some(repo));
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

/// What the demo's leader expects a planned card to cost (record 114),
/// in ten-thousandths of a dollar.
const PLANNED_COST: u64 = 4_000;
/// Every this many cards, one's worker strays once (record 114).
const STRAY_EVERY: usize = 4;
/// From this far into its work.
const STRAY_FROM: u8 = 40;

/// Whether a leader wants one more worker: work waits, nobody is free, and
/// the team is not at its most.
fn short_of_hands(waiting: usize, free: usize, workers: usize) -> bool {
    waiting >= 2 && free == 0 && workers < MAX_WORKERS
}

/// How often the demo's daemon stops, and for how long: long enough for
/// `⌀ no lonca` to be read, in ticks.
const STORE_EVERY: u64 = 60;
const STORE_STOPS: u64 = 6;

/// How many ticks a `lonca` command of the person's takes in the demo.
const STORE_JOB_TICKS: u64 = 3;
/// Every how many `:sync`s the demo's checkout has commits of its own.
const SYNC_APART_EVERY: u32 = 3;

/// Whether the demo's daemon is stopped at `tick`.
fn store_stopped(tick: u64) -> bool {
    tick % STORE_EVERY >= STORE_EVERY - STORE_STOPS
}

/// The commit trunk is at after `integrated` changes: looks like one.
fn demo_trunk(integrated: u32) -> String {
    let head = 0x9d3f_1c2a_u32.wrapping_add(integrated.wrapping_mul(0x51_7cc1));
    format!("{head:08x}{:032}", 0)
}

/// Whether a worker `progress` percent into its card is far enough to
/// stray ([`STRAY_FROM`]).
fn strays_now(progress: u8) -> bool {
    progress >= STRAY_FROM
}

/// A file of `card`'s project that the card's paths do not name, for a
/// worker that strays: the first in the code map.
fn outside_file(w: &World, card: CardId) -> Option<&'static str> {
    let c = &w.cards[card];
    (w.projects[c.project].repo.iter())
        .map(|(file, _)| *file)
        .find(|file| crate::paths::strays(&c.paths, file))
}

/// Symbols of `project` leased by cards that are not done yet.
pub fn held_symbols(w: &World, project: ProjectId) -> Vec<Symbol> {
    w.cards
        .iter()
        .filter(|c| c.project == project && c.holds_leases())
        .flat_map(|c| c.symbols.iter().copied())
        .collect()
}

/// The command a demo ask runs (`Bash: npm install zod@3`); none for one
/// that is not a command (an edit, a fetch).
fn demo_command(what: &str) -> Option<&str> {
    what.strip_prefix("Bash: ")
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

    /// Whether agent `id` has a session (without making it a mind).
    fn started(&self, id: AgentId) -> bool {
        self.minds.get(id).is_some_and(|m| m.started)
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
        a.role == Role::Worker
            && !a.gone
            && a.status == Status::Idle
            && !Self::unfinished(w, id)
            && !w.waits_on_person(id)
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
        // Every second resume of a run is refused, as Claude Code that lost
        // the session says: a new one starts, and the journal says why.
        let refused = stopped && had && self.refuses();
        if refused {
            let m = self.mind(id);
            m.started = false;
            m.refused = true;
        }
        self.start(w, id);
        if stopped && had && !refused {
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
            Role::Worker | Role::Helper | Role::Solo => {
                let a = &mut w.agents[id];
                a.set_status(Status::Working, t);
                if a.activity.is_none() {
                    a.activity = Some(String::new());
                }
            }
            // A researcher answers and goes on with its research.
            _ => {}
        }
        self.extras.told += 1;
        let fault = fault_of(self.extras.told).filter(|_| !self.extras.calm);
        let text = match fault {
            Some(Fault::Passing) => OVERLOADED.to_string(),
            Some(Fault::Account) => BAD_KEY.to_string(),
            None => (*self.rng.pick(self.content.acks)).to_string(),
        };
        let at = t + self.rng.range(2, 5);
        self.replies.push(Due {
            agent: id,
            at,
            text,
            failed: fault.is_some(),
            fault,
        });
        Ok(())
    }

    /// A failed turn of agent `id` is tried again, as live tells its
    /// session to go on: it works again and answers.
    fn retry(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let Some(r) = w.agents[id].retry.take() else {
            return;
        };
        let a = &mut w.agents[id];
        a.trace(
            t,
            format!(
                "retry {}/{} after a API error",
                r.attempt,
                RETRY_TICKS.len()
            ),
        );
        a.set_status(Status::Working, t);
        if a.activity.is_none() {
            a.activity = Some(String::new());
        }
        let text = (*self.rng.pick(self.content.acks)).to_string();
        let at = t + self.rng.range(2, 5);
        self.replies.push(Due {
            agent: id,
            at,
            text,
            failed: false,
            fault: None,
        });
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
            "/compact" => {
                // A compacted context holds its summary alone.
                self.mind(id).filled = 0;
                self.fill(w, id, 0);
                "Compacted the conversation.".to_string()
            }
            _ => format!("{line}: done."),
        };
        self.replies.push(Due {
            agent: id,
            at: t + 1,
            text,
            failed: false,
            fault: None,
        });
        Ok(())
    }

    /// `:effort` / `:model` in the demo, as live does it: the solo's choice
    /// changes for its next turn; refused mid-turn and for anyone but a
    /// solo. Says what it runs on now.
    pub fn rechoose(
        &mut self,
        w: &mut World,
        id: AgentId,
        choice: Option<String>,
    ) -> Result<String, String> {
        let a = &w.agents[id];
        if a.role != Role::Solo {
            return Err(format!("{} is not a solo session", a.name));
        }
        if let Some(c) = &choice {
            crate::live::valid_choice(c)?;
        }
        if a.status == Status::Working {
            return Err(format!("{} is mid-turn; wait for its answer", a.name));
        }
        let runs = match &choice {
            Some(c) => format!("{CLAUDE} {c}"),
            None => CLAUDE.to_string(),
        };
        match choice {
            Some(c) => self.models.insert(id, c),
            None => self.models.remove(&id),
        };
        let t = w.tick;
        w.agents[id].trace(
            t,
            format!("the person chose {runs}; the session starts again on it"),
        );
        w.note(Event::Chose {
            agent: id,
            choice: runs.clone(),
        });
        Ok(runs)
    }

    /// Whether demo agent `id` has a session to forget: it took a turn.
    pub fn can_forget(&self, w: &World, id: AgentId) -> Result<(), Refusal> {
        if w.agents[id].gone || !self.started(id) {
            return Err(Refusal::Command("it has no session to forget".into()));
        }
        Ok(())
    }

    /// `:forget` in the demo: agent `id`'s session goes, and it sleeps
    /// until a message starts a new one, told where it left off.
    pub fn forget(&mut self, w: &mut World, id: AgentId) -> Result<(), Refusal> {
        self.can_forget(w, id)?;
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

    /// What the demo's daemon says of each project on Lonca, as live's
    /// looks do; it stops now and then, so the demo shows a project closed
    /// and coming back.
    fn store_answers(&self, w: &mut World) {
        for (p, repo) in w.stores() {
            let row = self.extras.store.iter().find(|(r, _)| *r == repo);
            let waiting = (w.open_inbox().into_iter())
                .filter(|&i| w.inbox[i].kind == InboxKind::Change)
                .filter(|&i| w.agents[w.inbox[i].agent].project == p)
                .count();
            let seen = match row {
                _ if store_stopped(w.tick) => StoreSeen::Down(StoreDown::NoDaemon),
                // As a real daemon says it to a human session.
                None => StoreSeen::Down(StoreDown::Forbidden),
                Some((_, integrated)) => StoreSeen::Up {
                    trunk: demo_trunk(*integrated),
                    pending: waiting,
                    behind: match self.extras.synced.get(&repo) {
                        Some((_, true)) => Behind::Never,
                        Some((has, false)) => Behind::By(u64::from(integrated - has)),
                        None => Behind::By(u64::from(*integrated)),
                    },
                },
            };
            w.store_seen(p, &repo, seen);
        }
    }

    /// A change landed in project `p`: the demo's table moves its trunk.
    fn store_integrated(&mut self, w: &World, p: ProjectId) {
        let repo = w.projects[p].lonca.as_ref().map(|on| on.repo.as_str());
        let table = self.extras.store.iter_mut();
        if let Some(row) = table.into_iter().find(|(r, _)| Some(r.as_str()) == repo) {
            row.1 += 1;
        }
    }

    /// The person has worker `id`'s change applied: the next step says
    /// how it went. Refused while it is being applied, when none waits,
    /// or while its project's store does not answer.
    pub fn apply_change(
        &mut self,
        w: &World,
        id: AgentId,
        hunks: Vec<usize>,
    ) -> Result<(), String> {
        if let Some(why) = w.store_closed(w.agents[id].project) {
            return Err(why.into());
        }
        if self.is_applying(id) {
            return Err("their change is being applied".into());
        }
        if w.change_of(id).is_none() {
            return Err("no change of theirs waits".into());
        }
        self.applying.push((id, hunks));
        Ok(())
    }

    /// The changes being applied, with their chosen hunks.
    #[cfg(test)]
    pub fn applying_now(&self) -> &[(AgentId, Vec<usize>)] {
        &self.applying
    }

    fn is_applying(&self, id: AgentId) -> bool {
        self.applying.iter().any(|(a, _)| *a == id)
    }

    /// The person rejects worker `id`'s change; not while it is being
    /// applied.
    pub fn reject_change(&mut self, w: &mut World, id: AgentId) -> Result<(), String> {
        if let Some(why) = w.store_closed(w.agents[id].project) {
            return Err(why.into());
        }
        if self.is_applying(id) {
            return Err("their change is being applied".into());
        }
        w.change_rejected(id);
        Ok(())
    }

    /// How applying worker `id`'s change went: it `fits` the checkout and
    /// is applied with a mark, or git refuses it and it still waits, its
    /// first hunk the one that clashes.
    fn applied(&mut self, w: &mut World, id: AgentId, fits: bool, hunks: &[usize]) {
        let Some(change) = w.change_of(id).cloned() else {
            return;
        };
        let Some(file) = change.files.first().map(|f| f.path.clone()) else {
            return;
        };
        if !fits {
            let hunk = change.patch.lines().find(|l| l.starts_with("@@"));
            let clash = Clash {
                file: file.clone(),
                hunk: hunk.map(str::to_string),
                why: format!("patch failed: {file}:1; {file}: patch does not apply"),
            };
            let clashes = Clashes {
                head: sha(&mut self.rng),
                hunks: vec![clash],
                ..Clashes::default()
            };
            w.change_clashed(id, clashes);
            w.change_failed(id, format!("error: {file}: patch does not apply"));
            return;
        }
        let mark = sha(&mut self.rng);
        w.change_applied(id, mark, hunks.is_empty());
        self.store_integrated(w, w.agents[id].project);
        let p = w.agents[id].project;
        self.tell_the_others(w, id);
        self.extras
            .staged
            .entry(p)
            .or_default()
            .push(change.title.clone());
        let stand = &mut w.agents[id].stand;
        if hunks.is_empty() {
            // The worktree is marked at the change: nothing written since.
            (stand.added, stand.removed) = (0, 0);
            return;
        }
        // Some hunks only: the rest waits as a change again, as live
        // reads it from the worktree; it is what the worktree holds.
        let rest: Vec<usize> = (change.patch.lines().enumerate())
            .filter(|(_, l)| l.starts_with("@@"))
            .map(|(i, _)| i)
            .filter(|i| !hunks.contains(i))
            .collect();
        let patch = imece_agent::worktree::subset(&change.patch, &rest);
        let files = files_of(&patch);
        let s = &mut w.agents[id].stand;
        (s.added, s.removed) = files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
        if files.is_empty() {
            w.change_none(id);
        } else {
            w.change_ready(Change {
                files,
                patch,
                runs: Vec::new(),
                check: Check::None,
                spend: None,
                ..change
            });
        }
    }

    /// `:tidy` in the demo, as live does it: the next step answers with
    /// what is left of the made-up worktrees no agent owns.
    pub fn look_orphans(&mut self, w: &mut World) {
        if w.orphans.busy {
            return;
        }
        w.orphans.busy = true;
        self.extras.tidy_due = Some(None);
    }

    /// The demo's worktrees at `at` of the list go on the next step, as
    /// live's thread removes them: one that holds work only when `force`,
    /// else it stays with git's words for why. Refused as live refuses.
    pub fn tidy(&mut self, w: &mut World, at: &[usize], force: bool) -> Result<(), TidyRefusal> {
        if w.orphans.busy {
            return Err(TidyRefusal::Busy);
        }
        let found = w.orphans.found.as_deref().unwrap_or_default();
        let asked: Vec<&Orphan> = at.iter().filter_map(|&i| found.get(i)).collect();
        if asked.is_empty() {
            return Err(TidyRefusal::Nothing);
        }
        // Every demo checkout has the teardown script: asked about first.
        let script = teardown_script(&asked[0].checkout);
        let key = script.key();
        let refused = self.extras.teardown_refused.contains(&key);
        if !refused && !w.setup_approved.contains(&key) {
            return Err(TidyRefusal::Asks(Box::new(script)));
        }
        let mut tidied = Tidied::default();
        let mut gone = Vec::new();
        for o in asked {
            let torn = if refused {
                TornDown::Refused
            } else {
                TornDown::Ran {
                    ok: true,
                    end: "exit 0, 0.4 s".into(),
                    last: Some(format!("dropped database {}", o.name)),
                }
            };
            tidied.torn.push((o.name.clone(), torn));
            if force || o.clean() {
                tidied.removed.push(o.name.clone());
                gone.push(o.path.clone());
            } else if o.unread {
                tidied
                    .kept
                    .push((o.name.clone(), Some(WORKSPACE_MAY_HOLD.into())));
            } else {
                let why = format!(
                    "{} holds work: {} files changed, {} commits of its own",
                    o.path.display(),
                    o.changes,
                    o.commits
                );
                tidied.kept.push((o.name.clone(), Some(why)));
            }
        }
        if let Some(list) = &mut self.extras.orphans {
            list.retain(|o| !gone.contains(&o.path));
        }
        w.orphans.busy = true;
        self.extras.tidy_due = Some(Some(tidied));
        Ok(())
    }

    /// No made-up worktrees no agent owns: a test of none found.
    #[cfg(test)]
    pub fn forget_orphans(&mut self) {
        self.extras.orphans = Some(Vec::new());
    }

    /// The person's answer to the demo's teardown script, kept as live
    /// keeps it.
    pub fn decide_teardown(&mut self, w: &mut World, script: &setup::Script, yes: bool) {
        if yes {
            w.approve_setup(script.key());
        } else {
            self.extras.teardown_refused.push(script.key());
        }
    }

    /// A look or a removal of the demo's under way answers.
    fn tidy_answers(&mut self, w: &mut World) {
        let Some(tidied) = self.extras.tidy_due.take() else {
            return;
        };
        let found = self.extras.orphans.get_or_insert_with(|| demo_orphans(w));
        w.orphans = Orphans {
            found: Some(found.clone()),
            busy: false,
            tidied,
        };
    }

    /// Whether something the person asked for has not come yet: a commit
    /// message, or a `lonca` command.
    pub fn asking(&self) -> bool {
        self.extras.suggesting.is_some() || self.store_busy()
    }

    /// What the person is asked before project `p`'s checkout goes into
    /// the demo's Lonca as `repo`, as live asks it.
    pub fn import_ask(&self, w: &World, p: ProjectId, repo: &str) -> Option<ImportAsk> {
        let name = &w.projects[p].name;
        let checkout = self
            .folder_of(p)
            .map_or_else(|| PathBuf::from(format!("~/src/{name}")), PathBuf::from);
        Some(ImportAsk {
            project: p,
            repo: repo.to_string(),
            checkout,
            branch: "main".to_string(),
            head: demo_trunk(0),
        })
    }

    /// Whether a `lonca` command of the person's is under way.
    pub fn store_busy(&self) -> bool {
        self.extras.job.is_some()
    }

    /// `job` starts, unless one is under way, and comes back
    /// `STORE_JOB_TICKS` later.
    pub fn store_job(&mut self, w: &World, job: StoreJob) -> bool {
        if self.extras.job.is_some() {
            return false;
        }
        self.extras.job = Some((job, w.tick + STORE_JOB_TICKS));
        true
    }

    /// `:sync` of `repo` on the demo's table, as live's fast-forward: level
    /// is level, every third finds the person's own commits and moves
    /// nothing, the next takes them as resolved.
    fn demo_sync(&mut self, repo: &str) -> Synced {
        let row = self.extras.store.iter().find(|(r, _)| r == repo);
        let integrated = row.map_or(0, |(_, n)| *n);
        self.extras.syncs += 1;
        let third = self.extras.syncs.is_multiple_of(SYNC_APART_EVERY);
        let kept = self.extras.synced.entry(repo.to_string()).or_default();
        let how = match (third, kept.0 == integrated) {
            (true, _) => Synced::Apart,
            (false, true) => Synced::Even,
            (false, false) => Synced::Moved,
        };
        kept.1 = third;
        if !third {
            kept.0 = integrated;
        }
        how
    }

    /// The job under way comes back when its tick has come: what is in the
    /// table is refused as a real daemon refuses it, and a new row warns
    /// of a large file, so the journal's lines can be seen.
    fn store_job_due(&mut self, w: &mut World) {
        let due = self
            .extras
            .job
            .as_ref()
            .is_some_and(|(_, at)| w.tick >= *at);
        if !due {
            return;
        }
        let Some((job, _)) = self.extras.job.take() else {
            return;
        };
        match &job {
            StoreJob::Login(p) => w.store_signed_in(*p, "demo".to_string()),
            StoreJob::Sync { project, repo } => {
                let how = self.demo_sync(repo);
                w.store_synced(*project, repo, how);
            }
            StoreJob::Import(ask) if self.extras.store.iter().any(|(r, _)| *r == ask.repo) => {
                let why = format!(
                    "repo.prepare_init: conflict: repository {} exists",
                    ask.repo
                );
                w.store_failed(&job, Some(why));
            }
            StoreJob::Import(ask) => {
                self.extras.store.push((ask.repo.clone(), 0));
                let warned = "tour.mp4 is 3145728 bytes, above the change cap of 2097152";
                w.store_imported(ask, vec![warned.to_string()]);
            }
        }
    }

    /// `:commit` in the demo, as live does it: one at a time, refused
    /// when the pot or the run (the demo's day) has no room, reserved at
    /// its cap; two ticks later a made-up message for what was applied to
    /// project `p` comes, or the journal says nothing is staged. The
    /// model it asks.
    pub fn suggest(&mut self, w: &World, p: ProjectId) -> Result<String, String> {
        if self.extras.suggesting.is_some() {
            return Err("a suggestion is under way".into());
        }
        if w.budget.saturating_sub(w.spent()) < SUGGEST_CAP {
            return Err(format!(
                "the pot has less than {} left",
                dollars(SUGGEST_CAP)
            ));
        }
        let (calls, cost) = self.extras.paid_calls;
        if calls >= SUGGEST_DAY_CALLS {
            return Err(format!("today's {SUGGEST_DAY_CALLS} suggestions were made"));
        }
        if cost + SUGGEST_CAP > SUGGEST_DAY_COST {
            return Err(format!(
                "today's {} for suggestions is spent",
                dollars(SUGGEST_DAY_COST)
            ));
        }
        self.extras.suggesting = Some((p, w.tick + 2));
        Ok("haiku".into())
    }

    /// The suggestion asked two ticks ago answers: nothing staged, the
    /// same as last time (no call), or a new message, counted against the
    /// run's ceilings and paid from the pot (live reserves its cap and
    /// settles at once to what it cost; the demo's call costs at once).
    fn suggested(&mut self, w: &mut World) {
        let Some((p, at)) = self.extras.suggesting else {
            return;
        };
        if w.tick < at {
            return;
        }
        self.extras.suggesting = None;
        let leader = w.projects[p].leader;
        let staged = self.extras.staged.get(&p).cloned().unwrap_or_default();
        let Some(first) = staged.first() else {
            w.note(Event::SuggestFailed {
                leader,
                why: "nothing is staged".into(),
                cost: 0,
            });
            return;
        };
        if let Some((was, text)) = self.extras.suggestions.get(&p)
            && *was == staged
        {
            w.suggested = Some(Suggestion {
                project: p,
                text: text.clone(),
            });
            return;
        }
        let cost = self.rng.range(20, 60);
        self.extras.paid_calls.0 += 1;
        self.extras.paid_calls.1 += cost;
        w.paid += cost;
        let mut text = first.clone();
        if staged.len() > 1 {
            text.push_str("\n\n");
            let rest: Vec<String> = staged[1..].iter().map(|t| format!("- {t}")).collect();
            text.push_str(&rest.join("\n"));
        }
        self.extras.suggestions.insert(p, (staged, text.clone()));
        w.suggested = Some(Suggestion { project: p, text });
        w.note(Event::Suggested {
            leader,
            model: "haiku".into(),
            cost,
        });
    }

    /// `:review` of worker `agent`'s change, as live takes it: refused
    /// when none waits, it has its review, one is asked for, or the pot or
    /// the day has no room; else it answers three ticks after the one
    /// before it.
    pub fn review(&mut self, w: &World, agent: AgentId) -> Result<(), String> {
        let name = &w.agents[agent].name;
        if w.change_of(agent).is_none() {
            return Err(format!("no change of {name} waits"));
        }
        if w.review_of(agent).is_some() {
            return Err(format!("{name}'s change has its review already"));
        }
        if self.review_of(agent) {
            return Err(format!("a review of {name}'s change is under way"));
        }
        let cap = REVIEW_CAP;
        if w.budget.saturating_sub(w.spent()) < cap {
            return Err(format!("the pot has less than {} left", dollars(cap)));
        }
        let (calls, cost) = self.extras.review_calls;
        let asked = u32::try_from(self.extras.reviews.len()).unwrap_or(u32::MAX);
        if calls + asked >= REVIEW_DAY_CALLS {
            return Err(format!("today's {REVIEW_DAY_CALLS} reviews were made"));
        }
        if cost + cap > REVIEW_DAY_COST {
            return Err(format!(
                "today's {} for reviews is spent",
                dollars(REVIEW_DAY_COST)
            ));
        }
        let after = self.extras.reviews.back().map_or(w.tick, |r| r.1);
        self.extras.reviews.push_back((agent, after + 3));
        Ok(())
    }

    /// Whether worker `agent`'s change is being reviewed, or is about to.
    pub fn review_of(&self, agent: AgentId) -> bool {
        self.extras.reviews.iter().any(|r| r.0 == agent)
    }

    /// The review due now answers, made up from the change's files: what
    /// it costs is paid from the pot, counted against
    /// the run's ceilings, and the review is kept on the change; a change
    /// no longer waiting takes none.
    fn reviewed(&mut self, w: &mut World) {
        let Some(&(agent, at)) = self.extras.reviews.front() else {
            return;
        };
        if w.tick < at {
            return;
        }
        self.extras.reviews.pop_front();
        let Some(c) = w.change_of(agent).cloned() else {
            return;
        };
        let cost = self.rng.range(30, 90);
        self.extras.review_calls.0 += 1;
        self.extras.review_calls.1 += cost;
        w.paid += cost;
        let file = c.files.first().map_or("the patch", |f| f.path.as_str());
        let text = if self.rng.one_in(3) {
            "Nothing worth saying: the change does what its title says.".to_string()
        } else {
            format!(
                "1. {file}: the new branch has no test that reaches it.\n                 2. {file}: an empty input is not handled; say what it should do.\n                 3. The title says more than the patch does ({} files).",
                c.files.len()
            )
        };
        let model = "sonnet";
        let patch = patch_sum(&c.patch);
        w.reviewed(
            agent,
            Review {
                patch,
                model: model.into(),
                text,
                cost,
            },
        );
    }

    pub fn step(&mut self, w: &mut World) {
        self.checks_due(w);
        w.tick += 1;
        let t = w.tick;
        // A busy agent's round opens at its first busy tick, as live's
        // does at its prompt (record 113).
        let busy: Vec<AgentId> = (w.agents.iter())
            .filter(|a| !a.gone && a.status.busy())
            .map(|a| a.id)
            .collect();
        for &id in &busy {
            w.round_opens(id);
            w.agents[id].cost += COST_PER_TICK;
        }
        for (id, hunks) in std::mem::take(&mut self.applying) {
            let fits = !self.rng.one_in(8);
            self.applied(w, id, fits, &hunks);
        }
        self.suggested(w);
        self.reviewed(w);
        self.tidy_answers(w);
        self.store_job_due(w);
        self.store_answers(w);
        // Live sessions report their plan's windows now and then.
        if t.is_multiple_of(12) {
            self.limits(w);
        }
        let (due, later): (Vec<Due>, Vec<Due>) = std::mem::take(&mut self.replies)
            .into_iter()
            .partition(|r| r.at <= t);
        self.replies = later;
        for r in due {
            match r.failed {
                true => self.failed(w, r.agent, &r.text, r.fault),
                false => self.answer(w, r.agent, &r.text),
            }
        }
        for id in w.retries_due() {
            self.retry(w, id);
        }
        for p in 0..w.projects.len() {
            if !w.projects[p].closed {
                self.leader(w, p);
            }
        }
        // Researchers spawned during this tick start on the next one.
        for id in 0..w.agents.len() {
            if w.agents[id].gone || w.put_away(id) {
                continue;
            }
            match w.agents[id].role {
                Role::Worker => self.worker(w, id),
                Role::Helper => self.helper(w, id),
                Role::Researcher => self.researcher(w, id),
                Role::Leader | Role::Solo => {}
            }
        }
        // A yes for a turn ends when the agent's work stops, as live's
        // ends with the turn.
        for id in 0..w.agents.len() {
            let a = &w.agents[id];
            if a.turn_yes.is_some() && (a.gone || !a.status.busy()) {
                w.turn_over(id);
            }
            if w.agents[id].held_mail > 0 && !w.waits_on_person(id) {
                let a = &mut w.agents[id];
                a.held_mail = 0;
                a.trace(t, "takes the mail that waited on the person");
            }
        }
    }

    /// The plan's usage windows, as live sessions report them, once a
    /// session runs: what the agents spent fills its five-hour window, a
    /// percent for every [`LIMIT_COST`], and a quarter as much of its week;
    /// each starts again at the next whole window of the world's clock.
    fn limits(&mut self, w: &mut World) {
        let now = w.now();
        if !w.agents.iter().any(|a| self.started(a.id)) {
            return;
        }
        let spent: u64 = w.agents.iter().map(|a| a.cost).sum();
        let five = (spent / LIMIT_COST).min(100) as u8;
        for (minutes, used) in [(FIVE_HOURS, five), (WEEK, five / 4)] {
            let length = u64::from(minutes) * 60;
            w.limit(Limit {
                minutes,
                used,
                resets_at: Some((now / length + 1) * length),
            });
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
        a.tries = 0;
        a.talk = Some(Talk {
            with: Party::Person,
            mine: true,
            text: text.lines().next().unwrap_or_default().to_string(),
            until: t + 12,
        });
        if matches!(a.role, Role::Leader | Role::Solo) {
            a.activity = None;
            a.set_status(Status::Idle, t);
        }
        // A solo's turn end shows what its worktree holds, as live reads
        // it: a change about every other answer.
        if w.agents[id].role == Role::Solo
            && !w.projects[w.agents[id].project].no_git
            && self.rng.one_in(2)
        {
            let repo = w.projects[w.agents[id].project].repo;
            let (file, names) = *self.rng.pick(repo);
            let symbols: Vec<Symbol> = names
                .iter()
                .take(2)
                .map(|n| Symbol { file, name: n })
                .collect();
            let (files, patch) = self.patch(&symbols);
            let change = Change {
                agent: id,
                title: text.lines().next().unwrap_or_default().to_string(),
                files,
                patch,
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            };
            let (added, removed) = change.counts();
            let n = change.files.len();
            w.change_ready(change);
            w.agents[id].trace(t, format!("change: {n} files +{added} −{removed}"));
            self.check(w, id);
        }
    }

    /// Whether the resume asked for now is refused: every second one.
    fn refuses(&mut self) -> bool {
        self.extras.resumes += 1;
        self.extras.resumes.is_multiple_of(2)
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
        let refused = std::mem::take(&mut m.refused);
        // The first helper's session of a run lists no command line, as a
        // changed format would leave it (#53): only imece's /clear.
        let helper = w.agents[id].role == Role::Helper;
        let unlisted = helper && !std::mem::replace(&mut self.unlisted, true);
        let a = &mut w.agents[id];
        a.trace(t, "session/new");
        if unlisted {
            a.commands = vec!["/clear".into()];
            a.trace(
                t,
                "commands not listed: Claude Code's format may have changed",
            );
            w.note(Event::CommandsMissing { agent: id });
        } else {
            a.commands = commands_of();
        }
        if refused {
            let line = format!("session could not be resumed ({REFUSED})");
            w.agents[id].trace(t, line);
            w.note(Event::Restarted {
                agent: id,
                why: Fresh::Refused(REFUSED.into()),
            });
        } else if forgot {
            w.note(Event::Restarted {
                agent: id,
                why: Fresh::NoSession,
            });
        }
        if w.agents[id].role != Role::Worker {
            return;
        }
        self.copy_in(w, id);
        self.set_up(w, id);
        let roll = self.rng.below(12);
        self.worktree(w, id, roll);
    }

    /// A worker hired into project `p` and started, as the demo's
    /// leader hires one.
    #[cfg(test)]
    pub fn hire_for_test(&mut self, w: &mut World, p: ProjectId) -> AgentId {
        let id = self.hire(w, p);
        self.hired(w, w.projects[p].leader, id);
        self.start(w, id);
        id
    }

    /// The demo's setup script as live reads it from project `p`'s
    /// checkout.
    fn setup_script(w: &World, p: ProjectId) -> setup::Script {
        let bytes = DEMO_SETUP.as_bytes().to_vec();
        setup::Script {
            kind: setup::Kind::Setup,
            real: std::path::Path::new("/demo")
                .join(&w.projects[p].name)
                .join(setup::FILE),
            sha: crate::sha::hex(&bytes),
            bytes,
        }
    }

    /// The demo's check as live reads it from project `p`'s checkout.
    pub(crate) fn check_script(w: &World, p: ProjectId) -> setup::Script {
        let bytes = DEMO_CHECK.as_bytes().to_vec();
        setup::Script {
            kind: setup::Kind::Check,
            real: std::path::Path::new("/demo")
                .join(&w.projects[p].name)
                .join(setup::CHECK_FILE),
            sha: crate::sha::hex(&bytes),
            bytes,
        }
    }

    /// The repository's check on the change worker `id` just handed in,
    /// as live runs it (record 112): it runs when the person said yes to
    /// it (two ticks, every third fails), is asked about when not (one ask
    /// per script, the changes that wait for it counted on it), nothing
    /// after a no this run.
    pub(crate) fn check(&mut self, w: &mut World, id: AgentId) {
        let key = Self::check_script(w, w.agents[id].project).key();
        if self.extras.setup_refused.contains(&key) {
            return;
        }
        if w.setup_approved.contains(&key) {
            return self.check_runs(w, id);
        }
        w.checked(id, Check::Asked);
        let open =
            |(item, k, _): &&mut (usize, String, Vec<AgentId>)| *k == key && w.inbox[*item].open;
        if let Some((_, _, agents)) = self.extras.setup_asks.iter_mut().find(open) {
            agents.push(id);
            return;
        }
        self.extras.setup_asks.push((w.inbox.len(), key, vec![id]));
        w.inbox.push(InboxItem {
            kind: InboxKind::Check,
            agent: id,
            detail: setup::CHECK_FILE.to_string(),
            researcher: None,
            open: true,
            tick: w.tick,
            card: None,
        });
    }

    /// The demo's check starts on worker `id`'s waiting change.
    fn check_runs(&mut self, w: &mut World, id: AgentId) {
        if w.checked(id, Check::Running) {
            w.agents[id].trace(w.tick, "check: runs on the change");
            self.extras.checks_due.push((id, w.tick + CHECK_TICKS));
        }
    }

    /// The demo's checks that end at this tick: on the change when it
    /// still waits, in the journal; every third fails.
    fn checks_due(&mut self, w: &mut World) {
        let due: Vec<AgentId> = (self.extras.checks_due.iter())
            .filter(|(_, at)| *at <= w.tick)
            .map(|(id, _)| *id)
            .collect();
        self.extras.checks_due.retain(|(_, at)| *at > w.tick);
        for id in due {
            if !matches!(w.change_of(id).map(|c| &c.check), Some(Check::Running)) {
                continue;
            }
            self.extras.checks_ran += 1;
            let ok = !self.extras.checks_ran.is_multiple_of(3);
            let end = if ok {
                "exit 0, 41.2 s"
            } else {
                "exit 1, 38.7 s"
            };
            let lines = if ok {
                Vec::new()
            } else {
                DEMO_CHECK_SAYS.map(String::from).to_vec()
            };
            w.agents[id].trace(w.tick, format!("check: {end}"));
            w.checked(
                id,
                Check::Ran(Checked {
                    ok,
                    end: end.into(),
                    lines: lines.clone(),
                }),
            );
            w.note(Event::Checked {
                agent: id,
                ok,
                end: end.into(),
                lines,
            });
        }
    }

    /// Worker `id`'s worktree, new at its first start, set up as live
    /// does: the script runs when the person said yes to it, else it is
    /// asked about (one ask per script, the workers that go without it
    /// counted on it); nothing after a no this run.
    fn set_up(&mut self, w: &mut World, id: AgentId) {
        if !self.extras.hired.contains(&id) || !self.extras.set_up.insert(id) {
            return;
        }
        let key = Self::setup_script(w, w.agents[id].project).key();
        if self.extras.setup_refused.contains(&key) {
            return;
        }
        if w.setup_approved.contains(&key) {
            return Self::setup_ran(w, id);
        }
        let open =
            |(item, k, _): &&mut (usize, String, Vec<AgentId>)| *k == key && w.inbox[*item].open;
        if let Some((_, _, agents)) = self.extras.setup_asks.iter_mut().find(open) {
            agents.push(id);
            return;
        }
        self.extras.setup_asks.push((w.inbox.len(), key, vec![id]));
        w.inbox.push(InboxItem {
            kind: InboxKind::Script,
            agent: id,
            detail: setup::FILE.to_string(),
            researcher: None,
            open: true,
            tick: w.tick,
            card: None,
        });
    }

    /// The demo's setup script ran in worker `id`'s worktree.
    fn setup_ran(w: &mut World, id: AgentId) {
        w.agents[id].trace(w.tick, "worktree setup: exit 0, 4.2 s");
        w.note(Event::SetupRan {
            agent: id,
            ok: true,
            end: "exit 0, 4.2 s".into(),
            lines: DEMO_SETUP_SAYS.map(String::from).to_vec(),
        });
    }

    /// The script the ask `item` is about (a setup script or the check),
    /// for the window that shows it.
    pub fn setup_asked(&self, w: &World, item: usize) -> Option<setup::Script> {
        self.extras.setup_asks.iter().find(|(i, _, _)| *i == item)?;
        let p = w.agents[w.inbox[item].agent].project;
        Some(match w.inbox[item].kind {
            InboxKind::Check => Self::check_script(w, p),
            _ => Self::setup_script(w, p),
        })
    }

    /// The person's answer to setup ask `item`, as live's
    /// (`Live::decide_setup`): yes keeps it and runs it in the worktrees
    /// that went without; no holds for the run. False when nothing is
    /// asked there.
    pub fn decide_setup(&mut self, w: &mut World, item: usize, yes: bool) -> bool {
        let asks = &mut self.extras.setup_asks;
        let Some(at) = asks
            .iter()
            .position(|(i, _, _)| *i == item && w.inbox[item].open)
        else {
            return false;
        };
        let (_, key, agents) = asks.remove(at);
        w.inbox[item].open = false;
        let agent = w.inbox[item].agent;
        let check = w.inbox[item].kind == InboxKind::Check;
        w.note(match check {
            true => Event::CheckAnswered { agent, yes },
            false => Event::SetupAnswered { agent, yes },
        });
        if !yes {
            self.extras.setup_refused.push(key);
            if check {
                for id in agents {
                    w.checked(id, Check::None);
                }
            }
            return true;
        }
        w.approve_setup(key);
        for id in agents {
            match check {
                true => self.check_runs(w, id),
                false => Self::setup_ran(w, id),
            }
        }
        true
    }

    /// The paths the person listed (`:copy`), and then what the
    /// checkout's `.worktreeinclude` names ([`INCLUDED`]; the second
    /// project's checkout acts as if it had one, see [`Sim::includes`]),
    /// that worker `id`'s worktree lacks, copied as its session starts, as
    /// live does: made-up sizes, small for a dotfile and large for a
    /// directory, and now and then a listed one gone from the checkout
    /// since. What the file names takes nothing of the demo's chance (its
    /// size is made up from `id`), so a second project plays as before.
    /// That file has a line that reaches outside too, refused at every
    /// start.
    fn copy_in(&mut self, w: &mut World, id: AgentId) {
        let p = w.agents[id].project;
        let includes = self.includes(p);
        if includes {
            w.note(Event::IncludeRefused {
                agent: id,
                line: Some((REFUSED_LINE.0, REFUSED_LINE.1.into())),
                why: "reaches outside the checkout".into(),
            });
        }
        let mut wanted = w.projects[p].copies.clone();
        let listed = wanted.len();
        if includes && !wanted.iter().any(|c| c == INCLUDED) {
            wanted.push(INCLUDED.into());
        }
        let had = self.extras.copied.entry(id).or_default();
        let lacks: Vec<(String, bool)> = (wanted.into_iter().enumerate())
            .filter(|(_, p)| !had.contains(p))
            .map(|(i, p)| (p, i >= listed))
            .collect();
        // The list's, then the file's.
        let mut copied = [(Vec::new(), 0), (Vec::new(), 0)];
        for (path, included) in lacks {
            if included {
                copied[1].1 += made_up_size(&path, id as u64);
                copied[1].0.push(path);
                continue;
            }
            if self.rng.one_in(8) {
                w.note(Event::CopySkipped {
                    agent: id,
                    path,
                    why: "not in the checkout".into(),
                    included,
                });
                continue;
            }
            copied[0].1 += made_up_size(&path, self.rng.below(u64::MAX));
            copied[0].0.push(path);
        }
        for (included, (paths, bytes)) in [false, true].into_iter().zip(copied) {
            if paths.is_empty() {
                continue;
            }
            let had = self.extras.copied.entry(id).or_default();
            had.extend(paths.iter().cloned());
            w.note(Event::Copied {
                agent: id,
                paths,
                bytes,
                included,
            });
        }
    }

    /// Whether project `p`'s checkout has a `.worktreeinclude`: the second
    /// demo project's acts as if it had one.
    pub fn includes(&self, p: ProjectId) -> bool {
        p == 1
    }

    /// What a worker's worktree says as its session starts, by `roll`: it
    /// was moved to the checkout's HEAD, or kept where its own work is;
    /// mostly nothing.
    fn worktree(&mut self, w: &mut World, id: AgentId, roll: u64) {
        match roll {
            // One started from a point of its own stays there, as live's.
            0 if w.agents[id].start.is_none() => {
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
            _ => {}
        }
    }

    /// A worker joins `project`, named `claude-N` with the next free N,
    /// idle; as `--agents` starts them, nobody hired it.
    fn hire(&mut self, w: &mut World, project: ProjectId) -> AgentId {
        let so_far = w
            .agents
            .iter()
            .filter(|a| a.role == Role::Worker && !a.gone)
            .count();
        let name = free_name(w, &format!("{CLAUDE}-"), so_far + 1);
        let id = w.agents.len();
        w.agents.push(Agent::new(
            id,
            project,
            name,
            Role::Worker,
            Status::Idle,
            w.tick,
        ));
        Self::seat(w, id);
        id
    }

    /// A new worker of a project on Lonca works in a workspace of it, as
    /// live: the focus says so, and `:fork` refuses it (record 119).
    fn seat(w: &mut World, id: AgentId) {
        let on = w.projects[w.agents[id].project].lonca.as_ref();
        let Some(repo) = on.map(|on| on.repo.clone()) else {
            return;
        };
        let t = w.tick;
        w.agents[id].trace(t, format!("on Lonca: a workspace of {repo}"));
        w.agents[id].workspace = Some(repo);
    }

    /// `:fork` in the demo, as live does it (record 94): a new worker goes
    /// on from worker `from`'s conversation and its change; refused as live
    /// refuses: not a worker, no session yet, mid-turn. The demo has no
    /// `--agents` cap.
    pub fn fork(&mut self, w: &mut World, from: AgentId) -> Result<AgentId, String> {
        let a = &w.agents[from];
        if a.role != Role::Worker || a.gone {
            return Err(
                "only a worker forks: it has a conversation and a worktree of its own".into(),
            );
        }
        if let Some(why) = w.store_closed(a.project) {
            return Err(why.into());
        }
        if !self.started(from) {
            return Err("it has no session to fork yet: tell it something first".into());
        }
        if a.status == Status::Working {
            return Err("it is mid-turn: fork it when its turn ends".into());
        }
        if a.workspace.is_some() {
            return Err(ON_LONCA_FORK.into());
        }
        let (project, was) = (a.project, a.name.clone());
        let so_far = (w.agents.iter()).filter(|o| o.role == Role::Worker).count();
        let name = free_name(w, &format!("{CLAUDE}-"), so_far + 1);
        let id = w.agents.len();
        let t = w.tick;
        w.agents.push(Agent::new(
            id,
            project,
            name.clone(),
            Role::Worker,
            Status::Idle,
            t,
        ));
        // Its session is a copy of the one it came from.
        self.mind(id).started = true;
        w.agents[from].trace(t, format!("forked into {name}"));
        w.agents[id].trace(
            t,
            format!("forked from {was} · worktree from its HEAD, with its change"),
        );
        w.note(Event::Forked { from, agent: id });
        Ok(id)
    }

    /// Agent `by` hired agent `id` on a tier; a model this account lacks
    /// falls back to the default, now and then.
    fn hired(&mut self, w: &mut World, by: AgentId, id: AgentId) {
        let t = w.tick;
        let models = [Some("haiku"), None, Some("opus")];
        let model = models[self.rng.below(3) as usize].map(str::to_string);
        let shown = model.clone().unwrap_or_else(|| "the default model".into());
        let (hirer, name) = (w.agents[by].name.clone(), w.agents[id].name.clone());
        // A project's second worker starts from a pull request, as a
        // leader asked with `hire`'s `from`: its worktree stays there.
        let a = &w.agents[id];
        let second = a.role == Role::Worker
            && (w.agents.iter())
                .filter(|o| o.project == a.project && o.role == Role::Worker)
                .count()
                == 2;
        let from = second.then(|| DEMO_START.to_string());
        w.agents[by].trace(t, format!("hire {name} {shown}"));
        w.agents[id].trace(t, format!("hired by {hirer} · {shown}"));
        if let Some(f) = &from {
            w.agents[id].trace(t, format!("worktree from {f}"));
        }
        w.agents[id].start.clone_from(&from);
        self.extras.hired.insert(id);
        w.note(Event::Hired {
            by,
            agent: id,
            model: model.clone(),
            from,
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
            .find(|q| q.closed && q.archived.is_none() && q.name == name)
            .map(|q| q.id);
        let p = w.open_project(reuse, name, path.to_string(), Role::Leader);
        self.extras.folders.insert(p, path.to_string());
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

    /// `:solo [PATH] [CHOICE] [NAME]` in the demo: a fake solo session on
    /// `choice`, named `name` (else after the
    /// last part of `path`, numbered when taken), alone in a project of
    /// its own on one of the demo's code maps; it answers what the person
    /// writes and, now and then, shows a change. A closed solo of that
    /// name opens again in place.
    pub fn open_solo(
        &mut self,
        w: &mut World,
        path: &str,
        choice: Option<String>,
        name: Option<&str>,
    ) -> Result<ProjectId, ProjectRefusal> {
        let name = match name {
            Some(n) if !valid_project_name(n) => {
                return Err(ProjectRefusal::BadName(n.to_string()));
            }
            Some(n) => n.to_string(),
            None => free_project_name(w, &project_name_of(Path::new(path))),
        };
        if w.project_by_name(&name).is_some() {
            return Err(ProjectRefusal::NameTaken(name));
        }
        let reuse = w
            .projects
            .iter()
            .find(|q| q.closed && q.archived.is_none() && q.name == name && w.is_solo(q.id))
            .map(|q| q.id);
        let p = w.open_project(reuse, name, path.to_string(), Role::Solo);
        self.extras.folders.insert(p, path.to_string());
        if reuse.is_none() {
            w.projects[p].repo = PROJECTS[p % PROJECTS.len()].1;
        }
        // A real folder without git, as live finds it: the solo works
        // straight in it and shows no change. The demo's own made-up
        // paths are checkouts.
        let folder = match crate::live::checkout_of(Path::new(path)) {
            Ok((folder, false)) => Some(folder.display().to_string()),
            _ => None,
        };
        w.set_no_git(p, folder.as_deref());
        if let Some(c) = choice {
            self.models.insert(w.projects[p].leader, c);
        }
        Ok(p)
    }

    /// The folder project `p` was opened at with `:open` or `:solo`; none
    /// for the demo's own projects.
    pub fn folder_of(&self, p: ProjectId) -> Option<&str> {
        self.extras.folders.get(&p).map(String::as_str)
    }

    /// The `[MODEL][@EFFORT]` choice agent `id` runs on, when the person
    /// named one.
    pub fn model_of(&self, id: AgentId) -> Option<&str> {
        self.models.get(&id).map(String::as_str)
    }

    /// `:close` in the demo: the project's agents leave and its plans and
    /// research requests go; the last open project stays.
    pub fn close_project(&mut self, w: &mut World, p: ProjectId) -> Result<(), ProjectRefusal> {
        if w.open_projects().count() <= 1 {
            return Err(ProjectRefusal::Last);
        }
        self.leave_be(w, p);
        w.close_project(p);
        Ok(())
    }

    /// What was to come of project `p` goes: its plans, research
    /// requests, replies and hunks being applied.
    fn leave_be(&mut self, w: &mut World, p: ProjectId) {
        w.plans.retain(|plan| plan.project != p);
        let agents = &w.agents;
        w.research_queue.retain(|&id| agents[id].project != p);
        self.replies.retain(|r| agents[r.agent].project != p);
        self.applying.retain(|(id, _)| agents[*id].project != p);
    }

    /// `:archive` in the demo: the project's plans, research requests,
    /// replies and hunks being applied go, its agents sleep, and a fresh
    /// project on the same code map takes its place.
    pub fn archive_project(
        &mut self,
        w: &mut World,
        p: ProjectId,
        title: String,
    ) -> Result<ProjectId, ProjectRefusal> {
        self.leave_be(w, p);
        let model = self.models.get(&w.projects[p].leader).cloned();
        let (repo, no_git) = (w.projects[p].repo, w.projects[p].no_git);
        let fresh = w.archive_project(p, title);
        w.projects[fresh].repo = repo;
        w.projects[fresh].no_git = no_git;
        if let Some(m) = model {
            self.models.insert(w.projects[fresh].leader, m);
        }
        Ok(fresh)
    }

    /// `:restore` in the demo: the archive `p` comes back, and the open
    /// project of its name and kind (the demo's stand-in for its
    /// checkout's team) gives way: archived under its name and days, or
    /// closed when it did nothing. Refused when an open project of
    /// another kind has its name. Its folder is the demo's own, never
    /// gone nor without git. Returns the project that gave way.
    pub fn restore_project(
        &mut self,
        w: &mut World,
        p: ProjectId,
    ) -> Result<Option<ProjectId>, ProjectRefusal> {
        let name = w.projects[p].name.clone();
        let solo = w.is_solo(p);
        let open = w.project_by_name(&name);
        if open.is_some_and(|q| w.is_solo(q) != solo) {
            return Err(ProjectRefusal::NameTaken(name));
        }
        let mut swapped = None;
        if let Some(q) = open {
            self.leave_be(w, q);
            if w.did_anything(q) {
                let title = w.archive_title(q);
                w.shelve(q, title.clone());
                swapped = Some(title);
            } else {
                w.close_project(q);
            }
        }
        w.restore_project(p, swapped);
        Ok(open)
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
            paths: Vec::new(),
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

    /// The leader of `project` puts its plan before the person (record
    /// 114), as a live leader calls the `plan` tool before its second
    /// worker: a card per waiting card of the board, each on a tier, each
    /// owning one file of the code map (no two the same), the cost a
    /// flat figure a card.
    fn propose(&mut self, w: &mut World, project: ProjectId) {
        let t = w.tick;
        let leader = w.projects[project].leader;
        let repo = w.projects[project].repo;
        let test = kit(project).test;
        let titles: Vec<String> = (w.cards.iter())
            .filter(|c| c.project == project && c.column == Column::Waiting)
            .map(|c| c.title.clone())
            .collect();
        let cards: Vec<Planned> = (titles.into_iter().enumerate())
            .map(|(i, title)| Planned {
                body: format!("{title}. Done when {test} passes."),
                title,
                tier: self.rng.pick(&Tier::ALL).wire().to_string(),
                paths: repo
                    .get(i)
                    .map(|(file, _)| vec![Owned::new(file)])
                    .unwrap_or_default(),
            })
            .collect();
        let n = cards.len();
        let cost = n as u64 * PLANNED_COST;
        let item = w.propose(
            project,
            Proposal {
                cards,
                cost,
                tick: t,
            },
        );
        w.agents[leader].trace(t, format!("plan {n} cards {} (item {item})", dollars(cost)));
    }

    /// The person's answer to plan item `item`, as live's
    /// (`Live::decide_plan`): yes opens hiring until the next plan; no
    /// drops the plan and the leader asks what they want. False when no
    /// plan waits there.
    pub fn decide_plan(&mut self, w: &mut World, item: usize, yes: bool) -> bool {
        let Some(leader) = w.decide_plan(item, yes) else {
            return false;
        };
        let t = w.tick;
        w.agents[leader].trace(t, format!("plan {}", if yes { "yes" } else { "no" }));
        if !yes {
            self.tell_you(w, leader, MsgKind::Ask, self.content.plan_asked);
        }
        true
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
        // Work waits and nobody is free: one more worker; a second one
        // only once the person said yes to a plan (record 114), as live.
        if short_of_hands(waiting, free, workers.len()) && self.rng.one_in(40) {
            self.more_hands(w, project, leader);
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
        // The card owns the files of its leases (record 114), as a live
        // assign names paths.
        c.paths = Vec::new();
        for s in &symbols {
            if !c.paths.iter().any(|o| o.path == s.file) {
                c.paths.push(Owned::new(s.file));
            }
        }
        c.symbols = symbols;
        let title = c.title.clone();

        let a = &mut w.agents[agent];
        a.card = Some(card);
        a.progress = 0;
        a.activity = Some(String::new());
        // Its list for the card, nothing begun; its progress fills it in.
        a.plan = self
            .content
            .steps
            .iter()
            .map(|&text| Step {
                text: text.into(),
                state: StepState::Todo,
            })
            .collect();
        // Its first task starts its session, as live does.
        a.starting = a.cost == 0;
        a.set_status(Status::Working, t);
        if !lease.is_empty() {
            a.trace(t, format!("tool_use lonca.lease_acquire [{lease}]"));
        }
        w.story(Some(card), StoryEvent::Took { agent });
        match how {
            Taken::Assigned => {
                let leader = w.projects[project].leader;
                let n = self.send(
                    w,
                    leader,
                    agent,
                    MsgKind::Assign,
                    &format!("#{} {title}", card + 1),
                    None,
                );
                self.mind(agent).assigned = Some(n);
            }
            Taken::Claimed => {
                w.note(Event::Claimed { agent, card });
                w.agents[agent].trace(t, format!("claim #{}", card + 1));
            }
            Taken::Told => {}
        }
        self.start(w, agent);
    }

    /// Worker `id`'s change was applied: its leader tells its other
    /// workers, an inform to the group `workers` (each its own message,
    /// as live sends one to a group), but those that stopped.
    fn tell_the_others(&mut self, w: &mut World, id: AgentId) {
        let p = w.agents[id].project;
        let others: Vec<AgentId> = (w.agents.iter())
            .filter(|a| a.project == p && a.role == Role::Worker && !a.gone)
            .filter(|a| a.id != id && a.status != Status::Stopped)
            .map(|a| a.id)
            .collect();
        let leader = w.projects[p].leader;
        let moved = self.content.moved;
        for o in others {
            self.send(w, leader, o, MsgKind::Inform, moved, None);
        }
    }

    /// Agent `from` sends `to` a typed message over the bus, a reply to
    /// message `re` when it is one, as live's `send` does; its number.
    fn send(
        &mut self,
        w: &mut World,
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: &str,
        re: Option<MsgId>,
    ) -> MsgId {
        let t = w.tick;
        let id = w.number();
        let story = StoryEvent::Sent {
            from,
            to,
            kind,
            text: text.into(),
            id,
            re,
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
            id,
            re,
        });
        let (name, sender) = (w.agents[to].name.clone(), w.agents[from].name.clone());
        let numbers = numbers(id, re);
        w.agents[from].trace(t, format!("send to={name} type={}{numbers}", kind.wire()));
        w.agents[to].trace(
            t,
            format!("mail from={sender} type={}{numbers}", kind.wire()),
        );
        // As live's: what comes for an agent that waits on the person
        // waits with it, but for a cancel.
        if kind != MsgKind::Cancel && w.waits_on_person(to) {
            w.agents[to].held_mail += 1;
            w.agents[to].trace(t, format!("held until the person answers: message {id}"));
        }
        for (a, other) in [(from, to), (to, from)] {
            w.agents[a].talk = Some(Talk {
                with: Party::Agent(other),
                mine: a == from,
                text: text.into(),
                until: t + 8,
            });
        }
        id
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
            mine: true,
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
                let re = self.mind(id).question.take();
                self.send(w, to, id, MsgKind::Answer, answer, re);
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
        if w.agents[id].starting && t >= w.agents[id].changed_at + STARTING {
            w.agents[id].starting = false;
        }
        // A session that waits for the person's yes does nothing else,
        // until it stops waiting.
        if let Some(i) =
            (w.inbox.iter_mut()).find(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
        {
            // The demo does not wait as live does, up to a day: nobody may
            // be there, and the show goes on.
            if t >= i.tick + APPROVAL_WAIT {
                i.open = false;
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
        Self::plan_at(w, id, progress);
        Self::stray(w, id, card);
        self.fill(w, id, PER_TICK);
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
                self.send(w, id, leader, MsgKind::Progress, d, None);
            }
            15..16 => self.helper_if_it_may(w, id, card),
            16..17 if self.rng.one_in(4) => self.stop(w, id),
            20..70 => self.tool(w, id, card),
            _ => {}
        }
    }

    /// Agent `id`'s context grows by `tokens`, as a live request would
    /// leave it: its row and focus say how full it is. Past
    /// `AUTO_COMPACT` the session compacts on its own, as live ones do,
    /// and the trace says so.
    fn fill(&mut self, w: &mut World, id: AgentId, tokens: u64) {
        let t = w.tick;
        let m = self.mind(id);
        m.filled = m.filled.max(FRESH_CONTEXT) + tokens;
        if m.filled > AUTO_COMPACT {
            m.filled = FRESH_CONTEXT;
            w.agents[id].trace(t, "note context compacted by the session");
        }
        let used = self.mind(id).filled;
        w.agents[id].context = Some(Fill {
            used,
            size: Some(WINDOW),
        });
    }

    /// Worker `id`'s to-do list at `progress` percent: a step is done per
    /// fifth of the way, the next one in hand, as a live worker's planning
    /// tool would mark them; the trace says so when a step completes.
    fn plan_at(w: &mut World, id: AgentId, progress: u8) {
        let t = w.tick;
        let a = &mut w.agents[id];
        let done = usize::from(progress) * a.plan.len() / 100;
        let was = a.plan_count().0;
        for (i, step) in a.plan.iter_mut().enumerate() {
            step.state = if i < done {
                StepState::Done
            } else if i == done {
                StepState::Doing
            } else {
                StepState::Todo
            };
        }
        if done != was {
            let doing = a
                .plan
                .get(done)
                .map_or(String::new(), |s| format!(" {:?}", s.text));
            a.trace(t, format!("plan {done}/{}{doing}", a.plan.len()));
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
            let text = format!("It **builds**; the app runs at {url}.");
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
            Self::edited(w, card, file);
        }
    }

    /// Worker `id` wrote `file` on `card`: the card's story has the file
    /// once.
    fn edited(w: &mut World, card: CardId, file: &'static str) {
        let seen = w.cards[card]
            .story
            .iter()
            .any(|s| matches!(&s.event, StoryEvent::Touched(f) if f == file));
        if !seen {
            w.story(Some(card), StoryEvent::Touched(file.into()));
        }
    }

    /// Every [`STRAY_EVERY`]th card's worker, [`STRAY_FROM`] percent into
    /// its work, edits a file its card does not own, once (record 114),
    /// as a live worker may: the write shows as any edit, the card's
    /// story and the journal say it strayed, and the trace carries the
    /// note live puts in the worker's next prompt.
    fn stray(w: &mut World, id: AgentId, card: CardId) {
        let t = w.tick;
        if card % STRAY_EVERY != 1 || !strays_now(w.agents[id].progress) {
            return;
        }
        let Some(file) = outside_file(w, card) else {
            return;
        };
        let Some(c) = w.strayed(id, file) else {
            return;
        };
        let a = &mut w.agents[id];
        a.trace(t, format!("tool_use Edit {file}"));
        a.activity = Some(format!("Edit {file}"));
        a.trace(
            t,
            format!(
                "note you changed {file}, which #{} does not own; say so in your done",
                c + 1
            ),
        );
        Self::edited(w, card, file);
    }

    /// Worker `id`'s session asks the person's yes for one of its
    /// project's sensitive steps.
    /// One the person said yes to from now on for the project goes on
    /// without asking, as live's does.
    fn approval(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let what = *self.rng.pick(kit(w.agents[id].project).asks);
        let rules = &w.projects[w.agents[id].project].allowed;
        let said = demo_command(what).is_some_and(|c| standing::covered(c, |_| false, rules));
        let a = &mut w.agents[id];
        a.activity = Some(what.into());
        if said {
            a.trace(t, format!("allowed by your yes from now on: {what}"));
            return;
        }
        if w.allowed_this_turn(id) {
            let a = &mut w.agents[id];
            a.trace(t, format!("allowed by your yes for this turn: {what}"));
            return;
        }
        w.asks_you(id, what);
        w.agents[id].trace(t, format!("approve? {what}"));
    }

    /// The person says yes from now on to what open item `item` asks, as
    /// live's [`crate::live::Live::always`]: its commands' rules go into
    /// the project's standing yes, it is allowed, and so is every other
    /// open ask of the project they cover.
    pub fn always(&mut self, w: &mut World, item: usize) -> Result<Vec<Standing>, standing::Why> {
        let Some(it) = (w.inbox.get(item)).filter(|i| i.open && i.kind == InboxKind::Approval)
        else {
            return Err(standing::Why::NotACommand);
        };
        let command = demo_command(&it.detail).ok_or(standing::Why::NotACommand)?;
        let p = w.agents[it.agent].project;
        let rules = standing::needed(command, |_| false, &w.projects[p].allowed)?;
        let added = w.stand(p, &rules);
        w.decided(item, true);
        self.go_on_allowed(w, p);
        Ok(added)
    }

    /// The person says yes to everything the agent of approval `item`
    /// asks until its turn ends, as live's [`crate::live::Live::yes_turn`]:
    /// the ask is allowed, the agent's other open asks too (counted).
    pub fn yes_turn(&mut self, w: &mut World, item: usize) -> bool {
        let Some(id) = (w.inbox.get(item))
            .filter(|i| i.open && i.kind == InboxKind::Approval)
            .map(|i| i.agent)
        else {
            return false;
        };
        w.yes_this_turn(id);
        w.decided(item, true);
        for other in 0..w.inbox.len() {
            let i = &w.inbox[other];
            if i.open && i.kind == InboxKind::Approval && i.agent == id && w.allowed_this_turn(id) {
                w.decided(other, true);
            }
        }
        true
    }

    /// Every open ask of project `p` its standing yes now covers is
    /// allowed, as live's.
    pub fn go_on_allowed(&mut self, w: &mut World, p: ProjectId) {
        for other in 0..w.inbox.len() {
            // `decided` leaves alone what is not an open approval.
            let i = &w.inbox[other];
            let covered = w.agents[i.agent].project == p
                && demo_command(&i.detail)
                    .is_some_and(|c| standing::covered(c, |_| false, &w.projects[p].allowed));
            if covered {
                w.decided(other, true);
            }
        }
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

    /// A turn of agent `id` failed, `why` says why, `fault` Claude Code's
    /// kind of it; its session goes on. As live takes a failed turn: idle
    /// again, the story says why, and the journal, a retry or the inbox
    /// as `World::turn_failed` decides.
    fn failed(&mut self, w: &mut World, id: AgentId, why: &str, fault: Option<Fault>) {
        if w.agents[id].gone {
            return;
        }
        let t = w.tick;
        let card = w.agents[id].card;
        let a = &mut w.agents[id];
        a.activity = None;
        a.trace(t, format!("result ✕ {} denied 0", dollars(a.cost)));
        a.set_status(Status::Idle, t);
        w.story(card, StoryEvent::Stopped(why.into()));
        w.turn_failed(id, why, fault);
        if w.agents[id].retry.is_none() {
            Self::helper_lost(w, id);
        }
    }

    /// As live's `helper_lost`: helper `id` stopped, or its turn failed
    /// for good, before it sent done, so its worker is told (its trace)
    /// and an idle one goes back to work for the part. Anyone else's end
    /// tells nobody.
    fn helper_lost(w: &mut World, id: AgentId) {
        let (Role::Helper, Some(p)) = (w.agents[id].role, w.agents[id].parent) else {
            return;
        };
        if w.agents[p].gone {
            return;
        }
        let (t, name) = (w.tick, w.agents[id].name.clone());
        let worker = &mut w.agents[p];
        worker.trace(t, format!("told: {name} stopped"));
        if worker.status == Status::Idle {
            worker.set_status(Status::Working, t);
            worker.activity.get_or_insert_with(String::new);
        }
    }

    /// Worker `id`'s session ends under it; the person's next message
    /// resumes it.
    fn stop(&mut self, w: &mut World, id: AgentId) {
        let t = w.tick;
        let card = w.agents[id].card;
        let a = &mut w.agents[id];
        a.activity = None;
        // What it served, its to-do list, its context and a retry went
        // with its session.
        a.retry = None;
        a.tries = 0;
        a.local = None;
        a.plan.clear();
        a.context = None;
        self.mind(id).filled = 0;
        a.topic = STOPPED.into();
        a.set_status(Status::Stopped, t);
        a.trace(t, format!("stopped {STOPPED:?}"));
        w.story(card, StoryEvent::Stopped(STOPPED.into()));
        w.note(Event::Stopped {
            agent: id,
            why: STOPPED.into(),
        });
        Self::helper_lost(w, id);
    }

    /// A leader short of hands takes one more worker, unless its project's
    /// store does not answer; a second one only once the person said yes
    /// to a plan (record 114), as live.
    fn more_hands(&mut self, w: &mut World, project: ProjectId, leader: AgentId) {
        if w.store_closed(project).is_some() {
            return;
        }
        match w.plan_hold(project) {
            None => {
                let id = self.hire(w, project);
                self.hired(w, leader, id);
            }
            Some(PlanHold::NoPlan | PlanHold::Unanswered) => self.propose(w, project),
            Some(PlanHold::Reading) => {}
        }
    }

    /// Worker `id` takes a helper, unless it has one or its project's
    /// store does not answer.
    fn helper_if_it_may(&mut self, w: &mut World, id: AgentId, card: CardId) {
        let free = w.helpers_of(id).next().is_none();
        if free && w.store_closed(w.agents[id].project).is_none() {
            self.helper_for(w, id, card);
        }
    }

    /// Worker `id` hires a helper in its worktree, named
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
        let project = w.agents[id].project;
        let mut a = Agent::new(helper, project, name, Role::Helper, Status::Idle, t);
        a.parent = Some(id);
        a.workspace = w.agents[id].workspace.clone();
        w.agents.push(a);
        self.hired(w, id, helper);
        let task = format!("{}: {}", w.cards[card].title, self.content.tests);
        let n = self.send(w, id, helper, MsgKind::Assign, &task, None);
        self.mind(helper).assigned = Some(n);
        self.start(w, helper);
        let until = t + self.rng.range(10, 25);
        self.mind(helper).until = until;
        let a = &mut w.agents[helper];
        a.set_status(Status::Working, t);
        a.starting = true;
        a.activity = Some(format!("run {}", kit(project).test));
        helper
    }

    /// A helper works until its task is done, then tells its worker and
    /// leaves.
    fn helper(&mut self, w: &mut World, id: AgentId) {
        let a = &mut w.agents[id];
        if a.starting && w.tick >= a.changed_at + STARTING {
            a.starting = false;
        }
        if a.status != Status::Working || w.tick < self.mind(id).until {
            return;
        }
        let worker = w.agents[id].parent.expect("a helper has a worker");
        let re = self.mind(id).assigned.take();
        self.send(w, id, worker, MsgKind::Done, self.content.helper_done, re);
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
        a.trace(t, format!("run: {}: exit 0", kit.build));
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
        w.agents[id].trace(t, format!("run: {test}: exit {code}"));
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
        let re = self.mind(id).assigned.take();
        self.send(w, id, leader, MsgKind::Done, &title, re);
        // The change holds what the card leased, and the file its worker
        // strayed into (record 114), so the Δ says what is outside.
        let mut symbols = w.cards[card].symbols.clone();
        let repo = w.projects[w.agents[id].project].repo;
        for s in &w.cards[card].story {
            if let StoryEvent::Strayed(file) = &s.event
                && let Some((file, names)) = repo.iter().find(|(f, _)| f == file)
                && let Some(&name) = names.first()
            {
                symbols.push(Symbol { file, name });
            }
        }
        let (files, patch) = self.patch(&symbols);
        let runs = std::mem::take(&mut self.mind(id).runs);
        let change = Change {
            agent: id,
            title,
            files,
            patch,
            runs,
            check: Check::None,
            spend: None,
        };
        let (added, removed) = change.counts();
        let n = change.files.len();
        let c = &mut w.cards[card];
        c.column = Column::Done;
        c.finished = t;
        let a = &mut w.agents[id];
        a.progress = 0;
        a.activity = None;
        // Its turn is over as the change is read: what it cost goes with it.
        a.set_status(Status::Idle, t);
        a.trace(t, format!("change: {n} files +{added} −{removed}"));
        // What the person reviews is what the worktree holds.
        a.stand.added = added;
        a.stand.removed = removed;
        w.change_ready(change);
        self.check(w, id);
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
        let n = self.send(w, id, to, MsgKind::Ask, q, None);
        self.mind(id).question = Some(n);
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
        .find(|name| !w.name_taken(name))
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
                    let _ = s.apply_change(w, a, Vec::new());
                }
                InboxKind::Approval => w.decided(i, true),
                InboxKind::Script | InboxKind::Check => {
                    s.decide_setup(w, i, true);
                }
                InboxKind::Plan => {
                    s.decide_plan(w, i, true);
                }
                InboxKind::Question | InboxKind::Fault => {
                    // Closed here, once: a run's length never hangs on
                    // `said` closing it.
                    w.inbox[i].open = false;
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
    fn a_helper_lost_before_done_is_told_to_its_worker_who_goes_back_to_work() {
        let (mut w, mut s, worker, card) = one_at_work();
        let h = s.helper_for(&mut w, worker, card);
        let name = w.agents[h].name.clone();
        let told = |w: &World| {
            (w.agents[worker].trace.iter())
                .filter(|l| l.ends_with(&format!("told: {name} stopped")))
                .count()
        };
        w.agents[worker].status = Status::Idle;
        w.agents[worker].activity = None;
        s.failed(&mut w, h, BAD_KEY, Some(Fault::Account));
        assert_eq!(told(&w), 1);
        assert_eq!(w.agents[worker].status, Status::Working, "back to work");
        assert!(w.agents[worker].activity.is_some());
        // A passing error is tried again: nothing yet.
        s.failed(&mut w, h, OVERLOADED, Some(Fault::Passing));
        assert_eq!(told(&w), 1);
        // Stopped: told again; a busy worker stays as it is.
        w.agents[worker].status = Status::Asking { to: 0, until: 9 };
        s.stop(&mut w, h);
        assert_eq!(told(&w), 2);
        assert!(matches!(w.agents[worker].status, Status::Asking { .. }));
        // A worker's own end, or a worker gone: nobody is told.
        let lead = w.projects[0].leader;
        let before = w.agents[lead].trace.len();
        s.stop(&mut w, worker);
        assert_eq!(w.agents[lead].trace.len(), before);
        w.agents[worker].gone = true;
        s.stop(&mut w, h);
        assert_eq!(told(&w), 2);
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
                assert_eq!(w.agents[id].topic, STOPPED, "a stopped row says why");
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
    fn a_worker_s_first_card_starts_its_session_for_three_ticks_a_helper_s_too() {
        let (mut w, mut s, worker, card) = one_at_work();
        let began = w.tick;
        assert_eq!(w.agents[worker].changed_at, began);
        assert!(
            w.agents[worker].starting,
            "its first card, nothing spent yet"
        );
        assert!(w.agents[worker].progress == 0);
        s.step(&mut w);
        s.step(&mut w);
        assert_eq!(w.tick, began + 2);
        assert!(w.agents[worker].starting, "two ticks: still starting");
        assert!(
            w.agents[worker].progress > 0,
            "the work under it goes on: the demo's course is unchanged"
        );
        s.step(&mut w);
        assert!(!w.agents[worker].starting, "three ticks: up");
        // A helper's session starts with it, for as long.
        let h = s.helper_for(&mut w, worker, card);
        assert!(w.agents[h].starting && w.agents[h].status == Status::Working);
        s.step(&mut w);
        s.step(&mut w);
        assert!(w.agents[h].starting);
        s.step(&mut w);
        assert!(!w.agents[h].starting);
        // A later card of a worker that spent something starts no session.
        w.cards[card].column = Column::Done;
        w.agents[worker].status = Status::Idle;
        w.agents[worker].card = None;
        assert!(w.agents[worker].cost > 0);
        let next = w.cards.len();
        s.post(&mut w, 0, "next".into());
        s.take(&mut w, worker, next, Taken::Claimed);
        assert!(!w.agents[worker].starting);
        // One that never spent (started with `--agents`, idle) does.
        w.agents[worker].cost = 0;
        w.cards[next].column = Column::Done;
        w.agents[worker].status = Status::Idle;
        w.agents[worker].card = None;
        let again = w.cards.len();
        s.post(&mut w, 0, "again".into());
        s.take(&mut w, worker, again, Taken::Claimed);
        assert!(w.agents[worker].starting);
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
            Event::Sent { from, to, kind: MsgKind::Done, text, .. } if *from == worker && *to == lead && *text == title
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
            s.apply_change(&w, worker, Vec::new()),
            Err("no change of theirs waits".into())
        );
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        assert_eq!(s.apply_change(&w, worker, Vec::new()), Ok(()));
        assert_eq!(
            s.apply_change(&w, worker, Vec::new()),
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
    fn an_applied_change_is_told_by_the_leader_to_the_other_workers_one_message_each() {
        let (mut w, mut s) = world(1, 4, 0, 3, Lang::En);
        let ws = workers_of(&w, 0);
        let (worker, idle, stopped) = (ws[0], ws[1], ws[2]);
        w.agents[ws[3]].gone = true;
        let card = (w.cards.iter())
            .position(|c| c.column == Column::Waiting)
            .unwrap();
        s.take(&mut w, worker, card, Taken::Claimed);
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        w.agents[stopped].status = Status::Stopped;
        let lead = w.projects[0].leader;
        let n = w.journal.len();
        s.applied(&mut w, worker, false, &[]);
        let informs = |w: &World| -> Vec<AgentId> {
            (w.journal[n..].iter())
                .filter_map(|e| match &e.event {
                    Event::Sent {
                        from,
                        to,
                        kind: MsgKind::Inform,
                        text,
                        ..
                    } if *from == lead && text == EN.moved => Some(*to),
                    _ => None,
                })
                .collect()
        };
        assert!(informs(&w).is_empty(), "nothing applied, nothing told");
        s.applied(&mut w, worker, true, &[]);
        assert_eq!(informs(&w), [idle], "not its own, not one stopped or gone");
    }

    #[test]
    fn an_applied_change_is_staged_in_its_projects_checkout() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        let title = w.change_of(worker).unwrap().title.clone();
        s.applied(&mut w, worker, false, &[]);
        assert!(
            s.extras.staged.is_empty(),
            "one that did not fit is not staged"
        );
        s.applied(&mut w, worker, true, &[]);
        let p = w.agents[worker].project;
        assert_eq!(s.extras.staged.get(&p), Some(&vec![title]));
    }

    #[test]
    fn a_made_up_size_is_small_for_a_dotfile_and_large_for_a_directory() {
        assert_eq!(made_up_size(".env", 0), 100);
        assert_eq!(made_up_size(".env", 3_899), 3_999);
        assert_eq!(made_up_size(".env", 3_900), 100);
        assert_eq!(made_up_size("target", 0), 1 << 20);
        assert_eq!(made_up_size("target", 7), 8 << 20);
        assert_eq!(made_up_size("target", 399), 400 << 20);
        assert_eq!(made_up_size("target", 400), 1 << 20);
    }

    #[test]
    fn a_demo_worker_gets_the_listed_paths_it_lacks_at_its_start() {
        let (mut w, mut s, worker, _) = one_at_work();
        let p = w.agents[worker].project;
        w.projects[p].copies = vec![".env".into(), "target".into()];
        let mut had: Vec<String> = Vec::new();
        for _ in 0..20 {
            let lines = w.journal.len();
            s.copy_in(&mut w, worker);
            for e in &w.journal[lines..] {
                match &e.event {
                    Event::Copied {
                        agent,
                        paths,
                        bytes,
                        included: false,
                    } => {
                        assert_eq!(*agent, worker);
                        assert!(paths.iter().all(|p| !had.contains(p)), "copied once");
                        assert!(*bytes >= 100, "{paths:?} {bytes}");
                        had.extend(paths.iter().cloned());
                    }
                    Event::CopySkipped {
                        agent,
                        why,
                        included: false,
                        ..
                    } => {
                        assert_eq!((*agent, why.as_str()), (worker, "not in the checkout"));
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        had.sort();
        assert_eq!(had, [".env", "target"], "each in the end, once");
        assert_eq!(s.extras.copied[&worker].len(), 2);
        // All there: nothing said.
        let lines = w.journal.len();
        s.copy_in(&mut w, worker);
        assert_eq!(w.journal.len(), lines);
        assert!(!s.includes(p));
    }

    #[test]
    fn the_second_demo_projects_include_file_names_a_path_and_refuses_a_line() {
        let (mut w, mut s) = world(2, 12, 0, 3, Lang::En);
        let worker = workers_of(&w, 1)[0];
        w.projects[1].copies = vec![".env".into()];
        assert!(s.includes(1));
        let mut had: Vec<String> = Vec::new();
        for _ in 0..30 {
            let lines = w.journal.len();
            s.copy_in(&mut w, worker);
            // Said at every start, first.
            assert_eq!(
                w.journal[lines].event,
                Event::IncludeRefused {
                    agent: worker,
                    line: Some((2, "../shared/.env".into())),
                    why: "reaches outside the checkout".into()
                }
            );
            for e in &w.journal[lines + 1..] {
                match &e.event {
                    Event::Copied {
                        paths, included, ..
                    } => {
                        // Only what the list lacks comes from the file.
                        assert_eq!(*included, paths == &["node_modules"], "{paths:?}");
                        had.extend(paths.iter().cloned());
                    }
                    Event::CopySkipped { path, included, .. } => {
                        assert_eq!((*included, path.as_str()), (false, ".env"));
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        had.sort();
        assert_eq!(had, [".env", "node_modules"]);
        let size = |e: &crate::model::Entry| match &e.event {
            Event::Copied {
                included: true,
                bytes,
                ..
            } => Some(*bytes),
            _ => None,
        };
        let sizes: Vec<u64> = w.journal.iter().filter_map(size).collect();
        assert_eq!(sizes, [made_up_size("node_modules", worker as u64)]);
        // Listed by the person, it is the list's.
        let (mut w, mut s) = world(2, 12, 0, 3, Lang::En);
        let worker = workers_of(&w, 1)[0];
        w.projects[1].copies = vec!["node_modules".into()];
        for _ in 0..50 {
            s.copy_in(&mut w, worker);
            if s.extras.copied.get(&worker).is_some_and(|c| !c.is_empty()) {
                break;
            }
        }
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Copied { included: false, paths, .. } if paths == &["node_modules"]
        ));
        // The first project has no such file.
        let first = workers_of(&w, 0)[0];
        let lines = w.journal.len();
        s.copy_in(&mut w, first);
        assert_eq!(w.journal.len(), lines);
    }

    #[test]
    fn a_demo_suggestion_is_refused_as_live_refuses_one() {
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::En);
        let spent = w.spent();
        w.budget = spent + SUGGEST_CAP;
        assert_eq!(s.suggest(&w, 0), Ok("haiku".into()), "just the cap left");
        assert_eq!(s.suggest(&w, 0), Err("a suggestion is under way".into()));
        s.extras.suggesting = None;
        w.budget -= 1;
        assert_eq!(
            s.suggest(&w, 0),
            Err("the pot has less than $0.05 left".into())
        );
        w.budget = spent + 100 * SUGGEST_CAP;
        s.extras.paid_calls = (SUGGEST_DAY_CALLS - 1, SUGGEST_DAY_COST - SUGGEST_CAP);
        assert_eq!(s.suggest(&w, 0), Ok("haiku".into()));
        s.extras.suggesting = None;
        s.extras.paid_calls = (SUGGEST_DAY_CALLS, 0);
        assert_eq!(
            s.suggest(&w, 0),
            Err("today's 20 suggestions were made".into())
        );
        s.extras.paid_calls = (0, SUGGEST_DAY_COST - SUGGEST_CAP + 1);
        assert_eq!(
            s.suggest(&w, 0),
            Err("today's $1.00 for suggestions is spent".into())
        );
        assert_eq!(s.extras.suggesting, None);
    }

    #[test]
    fn a_demo_suggestion_answers_two_ticks_later_from_what_was_applied() {
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::En);
        let leader = w.projects[0].leader;
        let at = w.tick;
        assert_eq!(s.suggest(&w, 0), Ok("haiku".into()));
        assert_eq!(s.extras.suggesting, Some((0, at + 2)));
        w.tick = at + 1;
        s.suggested(&mut w);
        assert!(s.extras.suggesting.is_some(), "not yet");
        w.tick = at + 2;
        s.suggested(&mut w);
        assert_eq!(s.extras.suggesting, None);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::SuggestFailed {
                leader,
                why: "nothing is staged".into(),
                cost: 0
            }
        );
        // Two changes applied: the first is the subject, the rest the body.
        s.extras
            .staged
            .insert(0, vec!["Add the parser".into(), "Fix the lexer".into()]);
        let paid = w.paid;
        s.suggest(&w, 0).unwrap();
        w.tick += 2;
        s.suggested(&mut w);
        let text = "Add the parser\n\n- Fix the lexer".to_string();
        assert_eq!(w.suggested.take().map(|g| g.text), Some(text.clone()));
        let cost = w.paid - paid;
        assert!((20..=60).contains(&cost), "{cost}");
        assert_eq!(s.extras.paid_calls, (1, cost));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Suggested {
                leader,
                model: "haiku".into(),
                cost
            }
        );
        // The same again: the same message, no call.
        let lines = w.journal.len();
        s.suggest(&w, 0).unwrap();
        w.tick += 2;
        s.suggested(&mut w);
        assert_eq!(w.suggested.take().map(|g| g.text), Some(text));
        assert_eq!((w.paid - paid, s.extras.paid_calls.0), (cost, 1));
        assert_eq!(w.journal.len(), lines);
        // One change alone has no body.
        s.extras.staged.insert(0, vec!["Only".into()]);
        s.suggest(&w, 0).unwrap();
        w.tick += 2;
        s.suggested(&mut w);
        assert_eq!(w.suggested.take().map(|g| g.text), Some("Only".into()));
    }

    /// A demo of one project and its worker, whose change waits.
    fn a_change_waits() -> (World, Sim, AgentId) {
        let (mut w, s) = world(1, 1, 0, 3, Lang::En);
        let id = workers_of(&w, 0)[0];
        w.change_ready(Change {
            agent: id,
            title: "Add the parser".into(),
            files: vec![ChangedFile {
                path: "src/parse.rs".into(),
                added: 3,
                removed: 1,
                binary: false,
            }],
            patch: TWO_FILES.into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        (w, s, id)
    }

    #[test]
    fn a_demo_review_is_refused_as_live_refuses_one() {
        let (mut w, mut s, id) = a_change_waits();
        let name = w.agents[id].name.clone();
        let lead = w.projects[0].leader;
        assert_eq!(
            s.review(&w, lead),
            Err(format!("no change of {} waits", w.agents[lead].name))
        );
        let spent = w.spent();
        w.budget = spent + REVIEW_CAP;
        assert_eq!(s.review(&w, id), Ok(()), "just the cap left");
        assert!(s.review_of(id));
        assert_eq!(
            s.review(&w, id),
            Err(format!("a review of {name}'s change is under way"))
        );
        s.extras.reviews.clear();
        assert!(!s.review_of(id));
        w.budget -= 1;
        assert_eq!(
            s.review(&w, id),
            Err("the pot has less than $0.10 left".into())
        );
        w.budget = spent + 1_000_000;
        s.extras.review_calls = (REVIEW_DAY_CALLS - 1, REVIEW_DAY_COST - REVIEW_CAP);
        assert_eq!(s.review(&w, id), Ok(()), "the last of the day");
        s.extras.review_calls = (REVIEW_DAY_CALLS - 2, 0);
        let (mut w2, _, other) = a_change_waits();
        w2.budget = w.budget;
        assert_eq!(
            s.review(&w2, other),
            Err(format!(
                "a review of {}'s change is under way",
                w2.agents[other].name
            ))
        );
        s.extras.reviews.clear();
        s.extras.review_calls = (REVIEW_DAY_CALLS, 0);
        assert_eq!(s.review(&w, id), Err("today's 20 reviews were made".into()));
        s.extras.review_calls = (0, REVIEW_DAY_COST - REVIEW_CAP + 1);
        assert_eq!(
            s.review(&w, id),
            Err("today's $2.00 for reviews is spent".into())
        );
        // The asked ones count against the day too.
        s.extras.review_calls = (REVIEW_DAY_CALLS - 1, 0);
        s.extras.reviews.push_back((lead, 0));
        assert_eq!(s.review(&w, id), Err("today's 20 reviews were made".into()));
        // The pot's cap is the review's: without it, none.
        let (mut w, mut s, id) = a_change_waits();
        w.budget = w.spent() + REVIEW_CAP - 1;
        assert_eq!(
            s.review(&w, id),
            Err(format!(
                "the pot has less than {} left",
                dollars(REVIEW_CAP)
            ))
        );
        let (mut w, mut s, id) = a_change_waits();
        let sum = patch_sum(TWO_FILES);
        w.reviewed(
            id,
            Review {
                patch: sum,
                model: "sonnet".into(),
                text: "fine".into(),
                cost: 4,
            },
        );
        assert_eq!(
            s.review(&w, id),
            Err(format!(
                "{}'s change has its review already",
                w.agents[id].name
            ))
        );
    }

    #[test]
    fn a_demo_review_answers_three_ticks_after_the_one_before_it() {
        let (mut w, mut s, id) = a_change_waits();
        let at = w.tick;
        assert_eq!(s.review(&w, id), Ok(()));
        let other = w.projects[0].leader;
        assert!(!s.review_of(other), "only the one asked");
        assert_eq!(s.extras.reviews.back(), Some(&(id, at + 3)));
        // One asked after it waits for it.
        let mut second = w.change_of(id).unwrap().clone();
        second.agent = other;
        w.change_ready(second);
        assert_eq!(s.review(&w, other), Ok(()));
        assert_eq!(s.extras.reviews.back(), Some(&(other, at + 6)));
        w.tick = at + 2;
        s.reviewed(&mut w);
        assert_eq!(w.review_of(id), None, "not yet");
        let paid = w.paid;
        w.tick = at + 3;
        s.reviewed(&mut w);
        let r = w.review_of(id).expect("kept on the change").clone();
        assert_eq!(r.model, "sonnet");
        assert_eq!(r.patch, patch_sum(TWO_FILES));
        assert!((30..=90).contains(&r.cost), "{}", r.cost);
        assert_eq!(w.paid - paid, r.cost);
        assert_eq!(s.extras.review_calls, (1, r.cost));
        assert!(
            r.text.contains("src/parse.rs") || r.text.starts_with("Nothing"),
            "{}",
            r.text
        );
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Reviewed { agent, cost, .. } if agent == id && cost == r.cost
        ));
        // The next one is of no waiting change: dropped, nothing paid.
        assert_eq!(s.extras.reviews.len(), 1);
        w.change_rejected(other);
        w.tick = at + 6;
        let lines = w.journal.len();
        s.reviewed(&mut w);
        assert!(s.extras.reviews.is_empty());
        assert_eq!(w.journal.len(), lines);
        assert_eq!(s.extras.review_calls.0, 1);
    }

    const TWO_FILES: &str = "\
diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1 +1,2 @@
-x
+y
+z
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -4 +4 @@
-p
+q
";

    #[test]
    fn a_patch_s_files_count_their_hunks_lines_only() {
        let files = files_of(TWO_FILES);
        let got: Vec<(&str, u32, u32)> = (files.iter())
            .map(|f| (f.path.as_str(), f.added, f.removed))
            .collect();
        assert_eq!(got, [("a.rs", 2, 1), ("b.rs", 1, 1)]);
        assert!(files_of("").is_empty());
        let header_only = "diff --git a/c b/c\n--- a/c\n+++ b/c\n";
        assert_eq!(files_of(header_only)[0].added, 0, "headers are no lines");
    }

    #[test]
    fn chosen_hunks_are_applied_and_the_rest_waits_as_a_change_again() {
        let (mut w, mut s, worker, _) = one_at_work();
        let ready = |w: &mut World| {
            w.change_ready(Change {
                agent: worker,
                title: "two files".into(),
                files: files_of(TWO_FILES),
                patch: TWO_FILES.into(),
                // A run on the whole change's tree.
                runs: vec![Ran {
                    line: "cargo test".into(),
                    code: Some(0),
                    timed_out: false,
                }],
                check: Check::Failed("no shell".into()),
                spend: Some(Spend {
                    cost: 1,
                    ticks: 1,
                    rounds: 1,
                }),
            })
        };
        ready(&mut w);
        assert_eq!(s.apply_change(&w, worker, vec![3]), Ok(()));
        assert_eq!(s.applying, [(worker, vec![3])]);
        s.applying.clear();
        s.applied(&mut w, worker, true, &[3]);
        assert!(matches!(
            w.journal.iter().rev().find(|e| matches!(e.event, Event::Applied { .. })).unwrap().event,
            Event::Applied { agent, files: 2, .. } if agent == worker
        ));
        let rest = w.change_of(worker).expect("the rest waits");
        assert_eq!(rest.title, "two files");
        assert!(
            rest.runs.is_empty(),
            "the rest is another tree: nothing ran on it"
        );
        assert_eq!(rest.check, Check::None, "nor was it checked");
        assert_eq!(
            rest.spend, None,
            "settled when its turn is over, not copied from the whole"
        );
        assert_eq!(
            rest.patch,
            "diff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -4 +4 @@\n-p\n+q\n"
        );
        assert_eq!(rest.files.len(), 1);
        let stand = &w.agents[worker].stand;
        assert_eq!(
            (stand.added, stand.removed),
            (1, 1),
            "what the worktree still holds"
        );
        assert_eq!(w.open_inbox().len(), 1);
        // Every hunk chosen: nothing waits after.
        ready(&mut w);
        s.applied(&mut w, worker, true, &[3, 10]);
        assert!(w.change_of(worker).is_none());
        let stand = &w.agents[worker].stand;
        assert_eq!((stand.added, stand.removed), (0, 0));
    }

    #[test]
    fn a_change_that_fits_is_applied_with_a_mark_one_that_does_not_still_waits() {
        let (mut w, mut s, worker, _) = one_at_work();
        let lines = |w: &World| (w.agents[worker].stand.added, w.agents[worker].stand.removed);
        s.applied(&mut w, worker, true, &[]);
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
        s.applied(&mut w, worker, false, &[]);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::ApplyFailed {
                agent: worker,
                why: format!("error: {file}: patch does not apply")
            }
        );
        assert!(w.change_of(worker).is_some());
        assert_eq!(lines(&w), held, "not applied: still in the worktree");
        // What clashes is at hand, for the change to go back with it.
        let hunk = (w.change_of(worker).unwrap().patch.lines())
            .find(|l| l.starts_with("@@"))
            .map(str::to_string);
        let clashes = w.clashes_of(worker).unwrap();
        assert_eq!(clashes.hunks.len(), 1);
        assert_eq!(clashes.hunks[0].file, file);
        assert_eq!(clashes.hunks[0].hunk, hunk);
        assert_eq!(
            clashes.hunks[0].why,
            format!("patch failed: {file}:1; {file}: patch does not apply")
        );
        assert_eq!(clashes.head.len(), 7);
        w.agents[worker].stand.ahead = 2;
        s.applied(&mut w, worker, true, &[]);
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
    fn a_change_that_clashed_goes_back_and_the_worker_takes_it_as_its_next_card() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        s.applied(&mut w, worker, false, &[]);
        let message = crate::ui::clash_message(w.clashes_of(worker).unwrap(), false);
        assert_eq!(s.reject_change(&mut w, worker), Ok(()));
        assert!(w.clashes.is_empty());
        let cards = w.cards.len();
        s.told(&mut w, worker, &message).unwrap();
        assert_eq!(w.cards.len(), cards + 1, "a card for what it was told");
        let c = &w.cards[cards];
        let head = message.lines().next().unwrap();
        assert!(head.starts_with("Your change does not apply"), "{head}");
        assert_eq!(c.title, head);
        assert_eq!((c.owner, c.column), (Some(worker), Column::Working));
        assert_eq!(w.agents[worker].status, Status::Working);
    }

    #[test]
    fn most_changes_fit() {
        let (mut w, mut s, worker, _) = one_at_work();
        let (mut fit, mut not) = (0, 0);
        for _ in 0..64 {
            s.test(&mut w, worker);
            s.tested(&mut w, worker, 0);
            s.apply_change(&w, worker, Vec::new()).unwrap();
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
    fn a_demo_change_says_what_it_cost_its_rounds_add_up_and_an_apply_drops_them() {
        let (mut w, mut s, worker, card) = one_at_work();
        // The round opens at the first busy tick, from the cost then.
        let (before, t0) = (w.agents[worker].cost, w.tick);
        s.step(&mut w);
        assert_eq!(w.agents[worker].task.round, Some((before, t0 + 1)));
        assert_eq!(w.agents[worker].cost, before + COST_PER_TICK);
        let lead = w.projects[0].leader;
        assert_eq!(
            w.agents[lead].task.round, None,
            "a leader's spend is nobody's change"
        );
        // A helper's spend counts with its worker's, from its own round.
        let h = s.helper_for(&mut w, worker, card);
        for _ in 0..3 {
            s.step(&mut w);
        }
        let h_from = w.agents[h].task.round.unwrap().0;
        let expected = (w.agents[worker].cost - before) + (w.agents[h].cost - h_from);
        assert!(expected > 0);
        // Its done: the demo's turn is over as the change is read, so the
        // figure comes with it.
        s.done(&mut w, worker);
        let first = w.change_of(worker).unwrap().spend.unwrap();
        assert_eq!(
            first,
            Spend {
                cost: expected,
                ticks: w.tick - (t0 + 1),
                rounds: 1
            }
        );
        assert!(w.journal.iter().any(|e| matches!(
            e.event,
            Event::ChangeReady { agent, spend: Some(x), .. } if agent == worker && x == first
        )));
        assert_eq!(w.agents[worker].task.round, None);
        assert_eq!(w.agents[h].task.round, None);
        // Sent back and done again: the rounds add up.
        assert_eq!(s.reject_change(&mut w, worker), Ok(()));
        assert_eq!(w.agents[worker].task.carried, first);
        s.told(&mut w, worker, "Keep the old rounding").unwrap();
        // Its helper works on with it: both count again from here.
        let from = (w.agents[worker].cost, w.agents[h].cost);
        s.step(&mut w);
        s.step(&mut w);
        let added = (w.agents[worker].cost - from.0) + (w.agents[h].cost - from.1);
        assert!(added > 0, "busy again");
        s.done(&mut w, worker);
        let second = w.change_of(worker).unwrap().spend.unwrap();
        assert_eq!((second.cost, second.rounds), (first.cost + added, 2));
        assert!(second.ticks > first.ticks);
        // Applied whole: nothing carried into the next change.
        assert_eq!(s.apply_change(&w, worker, Vec::new()), Ok(()));
        s.applying.clear();
        s.applied(&mut w, worker, true, &[]);
        assert_eq!(w.agents[worker].task.carried, Spend::default());
        assert_eq!(w.agents[worker].task.last, Some(second));
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
            matches!(&a.talk, Some(Talk { with: Party::Person, mine: true, text: said, until }) if *said == text && *until == w.tick + 12)
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
        // A worker is there already: the plan comes first (record 114),
        // and the hire once the person said yes to it.
        let planned = |w: &World| {
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::Planned { leader: 0, .. }))
                .count()
        };
        for _ in 0..400 {
            s.leader(&mut w, 0);
            if let Some(i) = w.plan_item(0) {
                assert_eq!(hired(&w), 0, "no hire before the yes");
                assert!(s.decide_plan(&mut w, i, true));
            }
            if hired(&w) > 0 {
                break;
            }
        }
        assert_eq!(hired(&w), 1, "five busy, two waiting: one more");
        assert_eq!(planned(&w), 1, "one plan, said yes to");
        let new = w.agents.len() - 1;
        assert_eq!(w.agents[new].role, Role::Worker);
        assert_eq!(w.agents[new].project, 0);
        // With the gate off, no plan: the hire comes at once.
        let (mut w, mut s) = setup(5, 2);
        w.projects[0].plan.on = false;
        for _ in 0..400 {
            s.leader(&mut w, 0);
            if hired(&w) > 0 {
                break;
            }
        }
        assert_eq!((hired(&w), planned(&w)), (1, 0));
        for (n, waiting) in [(6, 2), (5, 1)] {
            let (mut w, mut s) = setup(n, waiting);
            for _ in 0..400 {
                s.leader(&mut w, 0);
            }
            assert_eq!(hired(&w), 0, "{n} workers, {waiting} waiting");
        }
    }

    #[test]
    fn a_hire_gets_a_tier_s_model_and_some_fall_back() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        let mut models = std::collections::BTreeSet::new();
        let mut fell = 0;
        for _ in 0..240 {
            let n = w.journal.len();
            s.hired(&mut w, 0, worker);
            let Event::Hired {
                by, agent, model, ..
            } = &w.journal[n].event
            else {
                panic!("{:?}", w.journal[n].event);
            };
            assert_eq!((*by, *agent), (0, worker));
            models.insert(model.clone());
            if let Some(Event::Fallback { agent, model: m }) =
                w.journal.get(n + 1).map(|e| &e.event)
            {
                assert_eq!(*agent, worker);
                assert_eq!(Some(m), model.as_ref(), "the model it was given");
                fell += 1;
            }
        }
        let want: std::collections::BTreeSet<Option<String>> =
            [Some("haiku".to_string()), None, Some("opus".to_string())].into();
        assert_eq!(models, want);
        assert!(fell > 0 && fell < 60, "{fell}");
    }

    #[test]
    fn a_worker_hires_a_helper_that_says_done_and_leaves() {
        let (mut w, mut s, worker, card) = one_at_work();
        let name = w.agents[worker].name.clone();
        let title = w.cards[card].title.clone();
        let before = w.messages;
        let h = s.helper_for(&mut w, worker, card);
        let assign = before + 1;
        assert_eq!(w.messages, assign, "its task is the one message");
        let a = &w.agents[h];
        assert_eq!(a.name, format!("{name}.1"));
        assert_eq!(
            (a.role, a.parent, a.project),
            (Role::Helper, Some(worker), 0)
        );
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
                text: format!("{title}: tests"),
                id: assign,
                re: None
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
                text: EN.helper_done.into(),
                id: assign + 1,
                re: Some(assign),
            },
            "its done replies to its task"
        );
        assert_eq!(s.mind(h).assigned, None, "and is through with it");
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
    fn mail_waits_with_an_agent_that_waits_on_the_person_as_it_does_live() {
        let (mut w, mut s, worker, card) = one_at_work();
        let lead = w.projects[0].leader;
        w.agents[worker].status = Status::Idle;
        w.cards[card].column = Column::Done;
        assert!(Sim::free(&w, worker));
        // A question open: no card comes, and mail waits on its row.
        w.asked_you(worker, "Which?");
        assert!(!Sim::free(&w, worker), "as a board offer waits");
        s.send(&mut w, lead, worker, MsgKind::Cancel, "Drop it", None);
        assert_eq!(w.agents[worker].held_mail, 0, "a cancel is never held");
        s.send(&mut w, lead, worker, MsgKind::Assign, "One", None);
        s.send(&mut w, lead, worker, MsgKind::Inform, "Two", None);
        assert_eq!(w.agents[worker].held_mail, 2);
        assert!(
            w.agents[worker]
                .trace
                .back()
                .unwrap()
                .ends_with("held until the person answers: message 3")
        );
        s.step(&mut w);
        assert_eq!(w.agents[worker].held_mail, 2, "still waiting");
        // Answered: it takes it, and the trace says so.
        w.close_questions(worker);
        s.step(&mut w);
        assert_eq!(w.agents[worker].held_mail, 0);
        assert!(
            (w.agents[worker].trace.iter())
                .any(|l| l.ends_with("takes the mail that waited on the person"))
        );
    }

    #[test]
    fn mail_to_an_agent_nobody_keeps_waiting_is_not_held() {
        let (mut w, mut s, worker, _) = one_at_work();
        let lead = w.projects[0].leader;
        s.send(&mut w, lead, worker, MsgKind::Assign, "One", None);
        assert_eq!(w.agents[worker].held_mail, 0);
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

        // Unanswered: the demo's session gives up, nobody decided.
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
        // Between cards, a question still open holds it back; its answer
        // sets it free.
        s.ask_you(&mut w, worker);
        w.cards[card].column = Column::Done;
        assert!(w.has_question(worker) && !Sim::free(&w, worker));
        w.close_questions(worker);
        assert!(Sim::free(&w, worker));
    }

    #[test]
    fn a_step_charges_only_an_agent_that_is_here_and_busy() {
        let (mut w, mut s) = world(1, 1, 1, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        let lead = w.projects[0].leader;
        w.agents[worker].status = Status::Working;
        w.agents[worker].cost = 100;
        let per_tick = COST_PER_TICK;
        // Here but idle: nothing.
        w.agents[lead].status = Status::Idle;
        w.agents[lead].cost = 20;
        assert!(per_tick > 0);
        // Gone, its status still a busy one: nothing.
        let gone = w.agents.len();
        let mut r = Agent::new(
            gone,
            0,
            "researcher-9".into(),
            Role::Researcher,
            Status::Researching {
                for_agent: worker,
                until: 1_000,
            },
            w.tick,
        );
        r.gone = true;
        r.cost = 7;
        w.agents.push(r);
        s.step(&mut w);
        assert_eq!(w.agents[worker].cost, 100 + per_tick, "busy and here");
        assert_eq!(w.agents[lead].cost, 20, "idle");
        assert_eq!(w.agents[gone].cost, 7, "gone");
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
        assert_eq!(w.agents[worker].topic, STOPPED, "why, for its row");
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
        // The second resume of the run is refused: a new session, and why;
        // the third goes on again.
        let resume = |w: &mut World, s: &mut Sim| {
            s.stop(w, worker);
            let n = w.journal.len();
            s.told(w, worker, "go on").unwrap();
            let got: Vec<Event> = w.journal[n..].iter().map(|e| e.event.clone()).collect();
            got
        };
        let got = resume(&mut w, &mut s);
        let refused = Event::Restarted {
            agent: worker,
            why: Fresh::Refused(REFUSED.into()),
        };
        assert!(got.contains(&refused), "{got:?}");
        assert!(!got.contains(&Event::Resumed { agent: worker }), "{got:?}");
        assert!(
            (w.agents[worker].trace.iter()).any(|l| l.ends_with("session/new")),
            "a new session"
        );
        assert_eq!(w.agents[worker].card, Some(card), "on the same card");
        let got = resume(&mut w, &mut s);
        assert!(got.contains(&Event::Resumed { agent: worker }), "{got:?}");
        assert!(!got.contains(&refused), "{got:?}");
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
            failed: false,
            fault: None,
        });
        s.forget(&mut w, lead).unwrap_err();
        s.told(&mut w, lead, "again").unwrap();
        assert!(w.journal.iter().any(|e| e.event
            == Event::Restarted {
                agent: lead,
                why: Fresh::NoSession
            }));
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
    fn the_demo_fills_the_windows_with_what_its_agents_spend() {
        let (mut w, mut s) = world(1, 2, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        let workers = workers_of(&w, 0);
        // Nobody runs yet: nothing said.
        s.limits(&mut w);
        assert!(w.limits.is_empty());
        s.start(&mut w, lead);
        w.agents[lead].cost = 3_000;
        w.agents[workers[1]].cost = 2_100;
        w.tick = 4_000;
        s.limits(&mut w);
        // $0.51: 25% of five hours, 6% of the week.
        let now = w.now();
        assert_eq!(now, 20_000);
        assert_eq!(
            w.limits,
            [
                Limit {
                    minutes: 300,
                    used: 25,
                    resets_at: Some(36_000)
                },
                Limit {
                    minutes: 10_080,
                    used: 6,
                    resets_at: Some(604_800)
                },
            ]
        );
        // Full at most.
        w.agents[workers[0]].cost = 99_999;
        s.limits(&mut w);
        assert_eq!(
            w.tightest(),
            Some(Limit {
                minutes: 300,
                used: 100,
                resets_at: Some(36_000)
            })
        );
        // Said every minute of the demo's clock.
        w.limits.clear();
        w.tick = 11;
        s.step(&mut w);
        assert!(!w.limits.is_empty(), "tick 12");
        w.limits.clear();
        s.step(&mut w);
        assert!(w.limits.is_empty(), "tick 13");
    }

    #[test]
    fn a_session_lists_claude_code_s_own_command_line() {
        assert_eq!(commands_of(), ["/compact", "/cost", "/clear"]);
    }

    #[test]
    fn nothing_held_says_nothing_when_a_step_comes() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let worker = workers_of(&w, 0)[0];
        assert_eq!(w.agents[worker].held_mail, 0);
        s.step(&mut w);
        let said = "takes the mail that waited on the person";
        assert!(!w.agents[worker].trace.iter().any(|l| l.contains(said)));
        w.agents[worker].held_mail = 1;
        s.step(&mut w);
        assert_eq!(w.agents[worker].held_mail, 0);
        assert!(w.agents[worker].trace.iter().any(|l| l.contains(said)));
    }

    /// A helper of `worker` joins the world, asleep to the demo's steps.
    fn with_helper_of(w: &mut World, worker: AgentId) -> AgentId {
        let id = w.agents.len();
        let project = w.agents[worker].project;
        let mut a = Agent::new(
            id,
            project,
            format!("{}.1", w.agents[worker].name),
            Role::Helper,
            Status::Idle,
            0,
        );
        a.parent = Some(worker);
        w.agents.push(a);
        id
    }

    #[test]
    fn the_first_helper_s_session_of_a_run_lists_no_command_line_as_live_says_it() {
        let (mut w, mut s) = world(1, 2, 0, 1, Lang::En);
        let workers = workers_of(&w, 0);
        let lead = w.projects[0].leader;
        let first = with_helper_of(&mut w, workers[0]);
        let second = with_helper_of(&mut w, workers[1]);
        // Others list their own.
        s.start(&mut w, lead);
        assert_eq!(w.agents[lead].commands, commands_of());
        s.start(&mut w, workers[0]);
        assert_eq!(w.agents[workers[0]].commands, commands_of());
        let missing = |w: &World| {
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::CommandsMissing { .. }))
                .map(|e| e.event.agent())
                .collect::<Vec<_>>()
        };
        assert!(missing(&w).is_empty());
        s.start(&mut w, first);
        assert_eq!(w.agents[first].commands, ["/clear"]);
        assert!(
            (w.agents[first].trace.iter())
                .any(|l| l.ends_with(" commands not listed: Claude Code's format may have changed"))
        );
        assert_eq!(missing(&w), [first]);
        // Once a run: the next helper lists its own.
        s.start(&mut w, second);
        assert_eq!(w.agents[second].commands, commands_of());
        assert_eq!(missing(&w), [first]);
    }

    #[test]
    fn a_failed_turn_leaves_the_agent_idle_with_its_reason_told() {
        let (mut w, mut s, worker, card) = one_at_work();
        w.agents[worker].set_status(Status::Working, w.tick);
        w.agents[worker].activity = Some("Read x".into());
        s.failed(&mut w, worker, "no", None);
        let a = &w.agents[worker];
        assert_eq!(a.status, Status::Idle);
        assert_eq!(a.activity, None);
        assert!(
            matches!(&w.cards[card].story.last().unwrap().event, StoryEvent::Stopped(why) if why == "no")
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Stopped {
                agent: worker,
                why: "no".into()
            }
        );
        // One that is gone says nothing.
        w.agents[worker].gone = true;
        let n = w.journal.len();
        s.failed(&mut w, worker, "no", None);
        assert_eq!(w.journal.len(), n);
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
        s.worktree(&mut w, worker, 2);
        assert_eq!(w.journal.len(), n + 2, "2 says nothing");
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
        // One hired from a start point of its own is never moved.
        w.agents[worker].start = Some(DEMO_START.into());
        w.agents[worker].stand.behind = 3;
        let n = w.journal.len();
        s.worktree(&mut w, worker, 0);
        assert_eq!(w.journal.len(), n, "nothing said");
        assert_eq!(w.agents[worker].stand.behind, 3, "nor moved");
    }

    #[test]
    fn a_yes_for_the_turn_lets_the_demo_agents_asks_go_until_its_work_stops() {
        let (mut w, mut s, worker, _) = one_at_work();
        let lead = w.projects[0].leader;
        w.asks_you(worker, "Bash: npm run a");
        w.asks_you(lead, "Bash: make lead");
        w.asks_you(worker, "Edit: src/x.ts");
        let first = w.inbox.len() - 3;
        // Not an open approval: nothing.
        assert!(!s.yes_turn(&mut w, 999));
        w.asked_you(worker, "Which?");
        let question = w.inbox.len() - 1;
        assert!(!s.yes_turn(&mut w, question));
        assert_eq!(w.agents[worker].turn_yes, None);
        assert!(s.yes_turn(&mut w, first));
        let open: Vec<String> = (w.open_inbox().iter())
            .map(|&i| w.inbox[i].detail.clone())
            .collect();
        assert_eq!(open, ["Bash: make lead", "Which?"]);
        assert_eq!(w.agents[worker].turn_yes, Some(1));
        // Its next asks go on, counted, whatever they are.
        let asks = w.inbox.len();
        for _ in 0..5 {
            s.approval(&mut w, worker);
        }
        assert_eq!(w.inbox.len(), asks, "none asked");
        assert_eq!(w.agents[worker].turn_yes, Some(6));
        assert!(
            w.agents[worker]
                .trace
                .iter()
                .any(|l| l.contains("allowed by your yes for this turn: "))
        );
        // Its work stops: the yes ends, said.
        w.agents[worker].status = Status::Idle;
        s.step(&mut w);
        assert_eq!(w.agents[worker].turn_yes, None);
        assert!(w.journal.iter().any(|e| e.event
            == Event::TurnYesEnded {
                agent: worker,
                allowed: 6
            }));
        // A busy one keeps it; a gone one does not.
        w.agents[worker].turn_yes = Some(0);
        w.agents[worker].status = Status::Working;
        s.step(&mut w);
        assert!(w.agents[worker].turn_yes.is_some() || !w.agents[worker].status.busy());
        w.agents[worker].turn_yes = Some(0);
        w.agents[worker].gone = true;
        w.agents[worker].status = Status::Working;
        s.step(&mut w);
        assert_eq!(w.agents[worker].turn_yes, None);
    }

    #[test]
    fn an_ask_the_person_said_yes_to_from_now_on_is_not_asked() {
        let (mut w, mut s, worker, _) = one_at_work();
        let p = w.agents[worker].project;
        for rule in kit(p).asks.iter().filter_map(|a| demo_command(a)) {
            let added = standing::needed(rule, |_| false, &w.projects[p].allowed).unwrap();
            w.stand(p, &added);
        }
        let open = w.open_inbox().len();
        for _ in 0..40 {
            s.approval(&mut w, worker);
        }
        let asked: Vec<String> = (w.open_inbox().iter().skip(open))
            .map(|&i| w.inbox[i].detail.clone())
            .collect();
        assert!(!asked.is_empty(), "edits and fetches still ask");
        assert!(asked.iter().all(|d| demo_command(d).is_none()), "{asked:?}");
        assert!(
            w.agents[worker]
                .trace
                .iter()
                .any(|l| l.contains("allowed by your yes from now on: Bash: "))
        );
        assert_eq!(demo_command("Edit: x"), None);
        assert_eq!(demo_command("Bash: make"), Some("make"));
        // Only an open approval gets a yes from now on: not one decided
        // already, nor an open item of another kind.
        let (mut w, mut s, worker, _) = one_at_work();
        w.asks_you(worker, "Bash: npm run x");
        let asked = w.inbox.len() - 1;
        w.decided(asked, false);
        assert_eq!(s.always(&mut w, asked), Err(standing::Why::NotACommand));
        w.asked_you(worker, "Bash: npm run y");
        let question = w.inbox.len() - 1;
        assert_eq!(s.always(&mut w, question), Err(standing::Why::NotACommand));
        assert!(w.projects[w.agents[worker].project].allowed.is_empty());
        w.asks_you(worker, "Bash: npm run z");
        let open = w.inbox.len() - 1;
        assert_eq!(s.always(&mut w, open).unwrap().len(), 1);
    }

    #[test]
    fn a_yes_from_now_on_holds_for_a_line_a_shell_reads_more_of_as_live() {
        let (mut w, mut s, worker, _) = one_at_work();
        let p = w.agents[worker].project;
        let lines: Vec<&str> = (PROJECTS.iter().flat_map(|(_, _, k)| k.asks.iter()))
            .filter_map(|a| demo_command(a))
            .collect();
        assert!(lines.iter().any(|l| l.contains("$(")), "shown in the demo");
        assert!(lines.iter().any(|l| l.contains('>')), "shown in the demo");
        // A substitution: its rule and what it runs.
        w.asks_you(
            worker,
            "Bash: gh pr view \"$(git branch --show-current)\" 2>&1",
        );
        let item = w.inbox.len() - 1;
        let added: Vec<String> = (s.always(&mut w, item).unwrap().iter())
            .map(Standing::line)
            .collect();
        assert_eq!(added, ["gh pr …", "git branch …"]);
        assert!(!w.inbox[item].open);
        // A redirection to a file: that line alone.
        let line = "dotnet ef migrations script > migrate.sql";
        w.asks_you(worker, &format!("Bash: {line}"));
        let item = w.inbox.len() - 1;
        assert_eq!(s.always(&mut w, item), Ok(vec![Standing::exactly(line)]));
        assert!(!w.inbox[item].open);
        assert!(standing::covered(line, |_| false, &w.projects[p].allowed));
    }

    #[test]
    fn a_project_s_second_worker_starts_from_a_pull_request() {
        let (mut w, mut s) = world(1, 1, 0, 5, Lang::En);
        let lead = w.projects[0].leader;
        let first = workers_of(&w, 0)[0];
        assert_eq!(w.agents[first].start, None);
        let id = w.agents.len();
        w.agents.push(Agent::new(
            id,
            0,
            "claude-9".into(),
            Role::Worker,
            Status::Idle,
            0,
        ));
        s.hired(&mut w, lead, id);
        assert_eq!(w.agents[id].start.as_deref(), Some(DEMO_START));
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Hired { agent, from: Some(f), .. } if *agent == id && f == DEMO_START
        )));
        assert!(
            w.agents[id]
                .trace
                .iter()
                .any(|l| l.ends_with("worktree from #41"))
        );
        // Its helper starts nowhere of its own (it works in its worker's
        // tree), while the project has two workers; the third worker
        // starts at HEAD again.
        let helper = w.agents.len();
        let mut h = Agent::new(
            helper,
            0,
            "claude-9.1".into(),
            Role::Helper,
            Status::Idle,
            0,
        );
        h.parent = Some(id);
        w.agents.push(h);
        s.hired(&mut w, id, helper);
        assert_eq!(w.agents[helper].start, None);
        let third = w.agents.len();
        w.agents.push(Agent::new(
            third,
            0,
            "claude-10".into(),
            Role::Worker,
            Status::Idle,
            0,
        ));
        s.hired(&mut w, lead, third);
        assert_eq!(w.agents[third].start, None);
    }

    #[test]
    fn a_kept_worktree_stands_by_its_ids() {
        assert_eq!(
            kept("claude-1", "5ab", "6cd", true),
            Stand {
                branch: Some("wip-claude-1".into()),
                ahead: 3,
                behind: 3,
                ..Stand::default()
            }
        );
        assert_eq!(
            kept("claude-1", "7ab", "f00", false),
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
        assert_eq!(w.agents[lead].commands, commands_of());
        assert_eq!(w.journal.len(), n, "a leader has no worktree");
        w.agents[lead].commands.clear();
        s.start(&mut w, lead);
        assert!(w.agents[lead].commands.is_empty(), "once");
    }

    #[test]
    fn a_hired_worker_s_new_worktree_asks_for_the_setup_script_and_a_yes_runs_it() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        // The demo's first worker: its worktree is as old as the demo.
        let old = workers_of(&w, 0)[0];
        s.start(&mut w, old);
        assert!(w.inbox.iter().all(|i| i.kind != InboxKind::Script));
        let hire = |w: &mut World, s: &mut Sim| {
            let id = s.hire(w, 0);
            s.hired(w, lead, id);
            s.start(w, id);
            id
        };
        let a = hire(&mut w, &mut s);
        let item = (w.inbox.iter())
            .position(|i| i.kind == InboxKind::Script)
            .expect("asked");
        let it = &w.inbox[item];
        assert_eq!(
            (it.agent, it.card, it.detail.as_str()),
            (a, None, setup::FILE)
        );
        let script = s.setup_asked(&w, item).unwrap();
        assert_eq!(script.text(), DEMO_SETUP);
        assert_eq!(script.sha, crate::sha::hex(DEMO_SETUP.as_bytes()));
        assert_eq!(
            script.real,
            std::path::Path::new("/demo/shop-api/.imece/worktree-setup.sh")
        );
        assert_eq!(s.setup_asked(&w, item + 1), None);
        // The next one goes without it too: counted on the same ask.
        let b = hire(&mut w, &mut s);
        let asks = w
            .inbox
            .iter()
            .filter(|i| i.kind == InboxKind::Script)
            .count();
        assert_eq!(asks, 1);
        assert!(
            !s.decide_setup(&mut w, item + 1, true),
            "nothing asked there"
        );
        assert!(s.decide_setup(&mut w, item, true));
        assert!(!w.inbox[item].open);
        assert_eq!(w.setup_approved, [script.key()]);
        let ran: Vec<AgentId> = (w.journal.iter())
            .filter_map(|e| match &e.event {
                Event::SetupRan {
                    agent,
                    ok: true,
                    lines,
                    ..
                } if *lines == DEMO_SETUP_SAYS => Some(*agent),
                _ => None,
            })
            .collect();
        assert_eq!(ran, [a, b]);
        assert!(w.journal.iter().any(|e| e.event
            == Event::SetupAnswered {
                agent: a,
                yes: true
            }));
        assert!(!s.decide_setup(&mut w, item, true), "answered already");
        // Said yes to: the next new worktree runs it, once.
        let c = hire(&mut w, &mut s);
        let ran_in = |w: &World, id| {
            (w.journal.iter())
                .any(|e| matches!(e.event, Event::SetupRan { agent, .. } if agent == id))
        };
        assert!(ran_in(&w, c));
        let runs = |w: &World| {
            w.journal
                .iter()
                .filter(|e| matches!(e.event, Event::SetupRan { .. }))
                .count()
        };
        let before = runs(&w);
        s.minds[c].started = false;
        s.start(&mut w, c);
        assert_eq!(runs(&w), before, "a worktree is set up once");
        assert_eq!(
            w.inbox
                .iter()
                .filter(|i| i.kind == InboxKind::Script)
                .count(),
            1
        );
    }

    #[test]
    fn a_no_to_the_demo_s_setup_script_holds_for_the_run() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let lead = w.projects[0].leader;
        let hire = |w: &mut World, s: &mut Sim| {
            let id = s.hire(w, 0);
            s.hired(w, lead, id);
            s.start(w, id);
            id
        };
        let a = hire(&mut w, &mut s);
        let item = w
            .inbox
            .iter()
            .position(|i| i.kind == InboxKind::Script)
            .unwrap();
        assert!(s.decide_setup(&mut w, item, false));
        assert!(w.setup_approved.is_empty());
        assert!(w.journal.iter().any(|e| e.event
            == Event::SetupAnswered {
                agent: a,
                yes: false
            }));
        hire(&mut w, &mut s);
        assert_eq!(
            w.inbox
                .iter()
                .filter(|i| i.kind == InboxKind::Script)
                .count(),
            1,
            "not asked again"
        );
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::SetupRan { .. }))
        );
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
    fn a_worker_s_to_do_list_fills_in_with_its_progress_and_goes_with_its_session() {
        use StepState::*;
        let (mut w, mut s, worker, card) = one_at_work();
        let texts = |w: &World| -> Vec<String> {
            w.agents[worker]
                .plan
                .iter()
                .map(|s| s.text.clone())
                .collect()
        };
        let states = |w: &World| -> Vec<StepState> {
            w.agents[worker].plan.iter().map(|s| s.state).collect()
        };
        assert_eq!(
            texts(&w),
            [
                "Read the code around it",
                "Make the change",
                "Write the tests",
                "Run the tests",
                "Write it up for the leader"
            ]
        );
        assert_eq!(states(&w), [Todo; 5]);
        let traced = w.agents[worker].trace.len();
        Sim::plan_at(&mut w, worker, 0);
        assert_eq!(states(&w), [Doing, Todo, Todo, Todo, Todo]);
        assert_eq!(
            w.agents[worker].trace.len(),
            traced,
            "nothing done: no line"
        );
        Sim::plan_at(&mut w, worker, 19);
        assert_eq!(w.agents[worker].plan_count(), (0, 5));
        Sim::plan_at(&mut w, worker, 20);
        assert_eq!(states(&w), [Done, Doing, Todo, Todo, Todo]);
        let last = w.agents[worker].trace.back().unwrap();
        assert!(last.ends_with(r#" plan 1/5 "Make the change""#), "{last}");
        let traced = w.agents[worker].trace.len();
        Sim::plan_at(&mut w, worker, 39);
        assert_eq!(
            w.agents[worker].trace.len(),
            traced,
            "the same step: no new line"
        );
        Sim::plan_at(&mut w, worker, 99);
        assert_eq!(states(&w), [Done, Done, Done, Done, Doing]);
        Sim::plan_at(&mut w, worker, 100);
        assert_eq!(states(&w), [Done; 5]);
        let last = w.agents[worker].trace.back().unwrap();
        assert!(last.ends_with(" plan 5/5"), "{last}");
        // Its done keeps the list, as a live turn's end does.
        s.done(&mut w, worker);
        assert_eq!(states(&w), [Done; 5]);
        assert_eq!(w.cards[card].column, Column::Done);
        // Stopped: gone with the session.
        s.stop(&mut w, worker);
        assert!(w.agents[worker].plan.is_empty());

        // Its work fills it in with the progress bar, a step per fifth.
        let (mut w, mut s, worker, card) = one_at_work();
        for _ in 0..60 {
            s.work(&mut w, worker);
            let a = &w.agents[worker];
            if a.status == Status::Stopped || w.cards[card].column == Column::Done {
                break;
            }
            assert_eq!(
                a.plan_count().0,
                usize::from(a.progress) * 5 / 100,
                "at {}%",
                a.progress
            );
        }
        // Turkish steps for a Turkish demo.
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::Tr);
        let worker = workers_of(&w, 0)[0];
        let card = w
            .cards
            .iter()
            .position(|c| c.column == Column::Waiting)
            .unwrap();
        s.take(&mut w, worker, card, Taken::Claimed);
        assert_eq!(w.agents[worker].plan[0].text, "Çevresindeki kodu oku");
        assert_eq!(w.agents[worker].plan.len(), STEPS);
    }

    #[test]
    fn a_worker_s_context_fills_with_its_work_compacts_near_the_top_and_goes_with_its_session() {
        let (mut w, mut s, worker, _card) = one_at_work();
        assert_eq!(
            w.agents[worker].context, None,
            "nothing said before a request"
        );
        s.fill(&mut w, worker, PER_TICK);
        assert_eq!(
            w.agents[worker].context,
            Some(Fill {
                used: FRESH_CONTEXT + PER_TICK,
                size: Some(WINDOW)
            })
        );
        assert_eq!(
            (WINDOW, FRESH_CONTEXT, PER_TICK, AUTO_COMPACT),
            (200_000, 20_000, 1_600, 180_000),
            "the line is nine tenths of the window"
        );
        s.fill(&mut w, worker, 100);
        assert_eq!(w.agents[worker].context.unwrap().used, 21_700);
        // Up to the top, where the session compacts on its own, said once.
        let traced = w.agents[worker].trace.len();
        s.fill(&mut w, worker, AUTO_COMPACT - 21_700);
        assert_eq!(
            w.agents[worker].context.unwrap().used,
            AUTO_COMPACT,
            "at the line, not over"
        );
        assert_eq!(w.agents[worker].trace.len(), traced);
        s.fill(&mut w, worker, 1);
        assert_eq!(w.agents[worker].context.unwrap().used, FRESH_CONTEXT);
        let last = w.agents[worker].trace.back().unwrap();
        assert!(
            last.ends_with(" context compacted by the session"),
            "{last}"
        );
        // The person's /compact empties it too.
        s.fill(&mut w, worker, 50_000);
        s.command(&mut w, worker, "/compact").unwrap();
        assert_eq!(w.agents[worker].context.unwrap().used, FRESH_CONTEXT);
        // Its work fills it a request a tick.
        let before = w.agents[worker].context.unwrap().used;
        s.work(&mut w, worker);
        let after = w.agents[worker].context.unwrap().used;
        assert!(
            after == before + PER_TICK || w.agents[worker].status != Status::Working,
            "{before} -> {after}"
        );
        // Stopped: gone with the session, and a new one starts fresh.
        s.stop(&mut w, worker);
        assert_eq!(w.agents[worker].context, None);
        assert_eq!(s.mind(worker).filled, 0);
        s.fill(&mut w, worker, 0);
        assert_eq!(w.agents[worker].context.unwrap().used, FRESH_CONTEXT);
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
            said.ends_with(
                r#"text "It **builds**; the app runs at http://localhost:5080/swagger.""#
            ),
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
                failed: false,
                fault: None,
            },
            Due {
                agent: b,
                at: 99,
                text: "b".into(),
                failed: false,
                fault: None,
            },
        ];
        s.close_project(&mut w, 1).unwrap();
        assert_eq!(s.replies.len(), 1);
        assert_eq!(s.replies[0].agent, a);
    }

    #[test]
    fn the_demo_archives_a_project_with_what_was_to_come_of_it_and_never_reopens_the_archive() {
        let (mut w, mut s) = world(2, 2, 0, 1, Lang::En);
        let (a, b) = (w.projects[0].leader, w.projects[1].leader);
        let worker = workers_of(&w, 1)[0];
        let due = |agent| Due {
            agent,
            at: 99,
            text: "x".into(),
            failed: false,
            fault: None,
        };
        s.replies = vec![due(a), due(b)];
        s.applying = vec![(worker, vec![0]), (workers_of(&w, 0)[0], vec![1])];
        let plan = |project| Plan {
            project,
            text: String::new(),
            ready: 0,
        };
        w.plans = vec![plan(1), plan(0)];
        w.research_queue = [worker, a].into();
        s.models.insert(b, "opus@high".into());
        let (code, name) = (w.projects[1].repo, w.projects[1].name.clone());
        let fresh = s.archive_project(&mut w, 1, "done".into()).unwrap();
        assert_eq!(fresh, 2);
        assert_eq!(s.replies.iter().map(|r| r.agent).collect::<Vec<_>>(), [a]);
        assert_eq!(s.applying.len(), 1);
        assert_ne!(s.applying[0].0, worker);
        assert_eq!(w.research_queue, [a]);
        assert_eq!(w.plans.iter().map(|p| p.project).collect::<Vec<_>>(), [0]);
        assert!(w.projects[1].archived.is_some());
        assert!(std::ptr::eq(w.projects[2].repo, code), "the same code map");
        assert_eq!(w.projects[2].name, name);
        assert_eq!(s.model_of(w.projects[2].leader), Some("opus@high"));
        // Closed, the fresh one opens again in place; the archive never.
        assert_eq!(s.close_project(&mut w, 2), Ok(()));
        assert_eq!(s.open_project(&mut w, "/x", Some(&name)), Ok(2));
        assert!(w.projects[1].archived.is_some() && w.projects[1].closed);
        let solo = s.open_solo(&mut w, "/s", None, Some("solo")).unwrap();
        let fresh_solo = s.archive_project(&mut w, solo, "s".into()).unwrap();
        assert_eq!(s.close_project(&mut w, fresh_solo), Ok(()));
        assert_eq!(
            s.open_solo(&mut w, "/s", None, Some("solo")),
            Ok(fresh_solo)
        );
        assert!(w.projects[solo].closed, "the archived solo stays put away");
    }

    #[test]
    fn the_demo_restores_an_archive_and_the_project_of_its_name_and_kind_gives_way() {
        let (mut w, mut s) = world(2, 2, 0, 1, Lang::En);
        let name = w.projects[1].name.clone();
        let fresh = s.archive_project(&mut w, 1, "done".into()).unwrap();
        // The fresh one has a card of its own, and hunks being applied.
        s.post(&mut w, fresh, "next".into());
        let head = w.projects[fresh].leader;
        s.applying = vec![(head, vec![0])];
        let title = w.archive_title(fresh);
        assert_eq!(s.restore_project(&mut w, 1), Ok(Some(fresh)));
        assert!(!w.projects[1].closed && w.projects[1].archived.is_none());
        assert_eq!(
            w.projects[fresh].archived.as_ref().map(|a| a.title.clone()),
            Some(title.clone())
        );
        assert!(s.applying.is_empty(), "what was to come of it goes");
        assert_eq!(
            w.journal.last().map(|e| e.event.clone()),
            Some(Event::Restored {
                leader: w.projects[1].leader,
                title: "done".into(),
                swapped: Some(title),
            })
        );
        // A fresh one that did nothing is closed instead.
        let fresh = s.archive_project(&mut w, 1, "again".into()).unwrap();
        assert!(!w.did_anything(fresh));
        assert_eq!(s.restore_project(&mut w, 1), Ok(Some(fresh)));
        assert!(w.projects[fresh].closed && w.projects[fresh].archived.is_none());
        // A solo of the name is not the team's to swap: refused.
        let fresh = s.archive_project(&mut w, 1, "third".into()).unwrap();
        assert_eq!(s.close_project(&mut w, fresh), Ok(()));
        let solo = s.open_solo(&mut w, "/s", None, Some(&name)).unwrap();
        assert_eq!(
            s.restore_project(&mut w, 1),
            Err(ProjectRefusal::NameTaken(name.clone()))
        );
        assert!(w.projects[1].archived.is_some() && !w.projects[solo].closed);
        // Nothing of its name open: nothing gives way.
        assert_eq!(s.close_project(&mut w, solo), Ok(()));
        assert_eq!(s.restore_project(&mut w, 1), Ok(None));
        assert!(!w.projects[1].closed);
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
    fn the_demo_opens_a_solo_that_answers_the_person_and_shows_a_change_now_and_then() {
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        let p = s
            .open_solo(&mut w, "~/work/shop", Some("@high".into()), None)
            .unwrap();
        assert_eq!(p, 1);
        let head = w.projects[1].leader;
        let a = &w.agents[head];
        assert_eq!(w.projects[1].name, "shop");
        assert_eq!(
            (a.name.as_str(), a.role, a.status),
            ("solo-shop", Role::Solo, Status::Idle)
        );
        assert_eq!(s.model_of(head), Some("@high"));
        assert_eq!(w.projects[1].repo, PROJECTS[1].1, "a code map of its own");
        assert_eq!(
            w.agents.iter().filter(|a| a.project == 1).count(),
            1,
            "alone: no workers, no cards"
        );
        assert!(w.cards.iter().all(|c| c.project != 1));
        // The same directory again: numbered; a name of the person's own.
        assert_eq!(s.open_solo(&mut w, "~/work/shop", None, None), Ok(2));
        assert_eq!(w.projects[2].name, "shop-2");
        assert_eq!(s.model_of(w.projects[2].leader), None);
        assert_eq!(
            s.open_solo(&mut w, "/x", None, Some("shop")),
            Err(ProjectRefusal::NameTaken("shop".into()))
        );
        assert_eq!(
            s.open_solo(&mut w, "/x", None, Some("a b")),
            Err(ProjectRefusal::BadName("a b".into()))
        );
        // Told something, it works and answers; about every other answer
        // shows a change of its worktree, as live reads one at a turn end.
        s.extras.calm = true;
        let mut changes = 0;
        let mut answers = 0;
        for i in 0..40 {
            s.told(&mut w, head, &format!("task {i}")).unwrap();
            assert_eq!(w.agents[head].status, Status::Working);
            for _ in 0..8 {
                s.step(&mut w);
            }
            assert_eq!(
                w.agents[head].status,
                Status::Idle,
                "answered within 8 ticks"
            );
            answers += 1;
            if let Some(c) = w.change_of(head) {
                assert_eq!(c.agent, head);
                assert!(!c.title.is_empty(), "titled by the answer's first line");
                assert!(!c.files.is_empty());
                changes += 1;
                s.apply_change(&w, head, Vec::new()).unwrap();
                s.step(&mut w);
            }
        }
        assert_eq!(answers, 40);
        assert!(
            (5..=35).contains(&changes),
            "about every other answer: {changes}"
        );
        let replied = w
            .journal
            .iter()
            .filter(|e| matches!(e.event, Event::Replied { agent, .. } if agent == head))
            .count();
        assert_eq!(replied, 40);
        // Closed, a solo of that name opens again in place; not one of
        // another name, and never a closed team of that name.
        assert_eq!(s.close_project(&mut w, 1), Ok(()));
        assert_eq!(s.close_project(&mut w, 2), Ok(()));
        assert_eq!(
            s.open_solo(&mut w, "/z", None, Some("third")),
            Ok(3),
            "another name: a project of its own"
        );
        assert_eq!(s.open_solo(&mut w, "/z", None, Some("shop")), Ok(1));
        assert!(w.is_solo(1) && !w.projects[1].closed);
        assert!(w.projects[2].closed, "shop-2 stays closed");
        assert_eq!(s.open_project(&mut w, "/t", Some("team")), Ok(4));
        assert_eq!(s.close_project(&mut w, 4), Ok(()));
        assert_eq!(
            s.open_solo(&mut w, "/t", None, Some("team")),
            Ok(5),
            "a closed team of that name is not a solo's place"
        );
        assert!(w.projects[4].closed && !w.is_solo(4) && w.is_solo(5));
    }

    #[test]
    fn the_demo_fails_now_and_then_as_live_does_and_tries_again_as_live_does() {
        let got: Vec<Option<Fault>> = (1..=21).map(fault_of).collect();
        let p = Some(Fault::Passing);
        let a = Some(Fault::Account);
        assert_eq!(
            got,
            [
                None, None, p, None, None, p, a, None, p, None, None, p, None, a, p, None, None, p,
                None, None, a
            ]
        );
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        let solo = s.open_solo(&mut w, "/x", None, None).unwrap();
        let head = w.projects[solo].leader;
        let answer = |w: &mut World, s: &mut Sim, text: &str| {
            s.told(w, head, text).unwrap();
            for _ in 0..5 {
                s.step(w);
            }
        };
        answer(&mut w, &mut s, "one");
        answer(&mut w, &mut s, "two");
        assert_eq!(w.agents[head].retry, None);
        // The third fails on an overloaded API and waits to be tried.
        answer(&mut w, &mut s, "three");
        let r = w.agents[head].retry.expect("a retry waits");
        assert_eq!(r.attempt, 1);
        assert_eq!(w.agents[head].status, Status::Idle);
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Retrying { agent, why, attempt: 1, seconds: 30 } if *agent == head && why == OVERLOADED
        )));
        while w.tick < r.at {
            s.step(&mut w);
        }
        assert_eq!(w.agents[head].retry, None, "tried at its tick");
        assert_eq!(w.agents[head].status, Status::Working);
        assert!(
            w.agents[head]
                .trace
                .iter()
                .any(|l| l.ends_with("retry 1/3 after a API error"))
        );
        for _ in 0..5 {
            s.step(&mut w);
        }
        assert_eq!(w.agents[head].status, Status::Idle, "and it answered");
        assert_eq!(w.agents[head].tries, 0, "a good turn counts from nothing");
        for text in ["four", "five", "six"] {
            answer(&mut w, &mut s, text);
            w.agents[head].retry = None;
        }
        // The seventh: a refused key, the person's at once.
        answer(&mut w, &mut s, "seven");
        assert_eq!(w.agents[head].retry, None);
        assert_eq!(
            w.open_item(head, InboxKind::Fault)
                .map(|i| i.detail.as_str()),
            Some(BAD_KEY)
        );
        // A session that stops takes a retry and its count with it.
        w.agents[head].retry = Some(Retry {
            attempt: 1,
            at: 999,
        });
        w.agents[head].tries = 1;
        s.stop(&mut w, head);
        assert_eq!((w.agents[head].retry, w.agents[head].tries), (None, 0));
    }

    #[test]
    fn the_demo_opens_a_solo_in_a_folder_without_git_as_live_does() {
        let (mut w, mut s) = world(1, 2, 4, 7, Lang::En);
        let plain = crate::live::testing::plain_folder("demo-nogit", false);
        let shown = plain.display().to_string();
        let p = s.open_solo(&mut w, &shown, None, None).unwrap();
        let head = w.projects[p].leader;
        assert!(w.projects[p].no_git);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::NoGit {
                leader: head,
                path: shown.clone()
            }
        );
        // Its answers come without a change: it wrote in the folder.
        s.extras.calm = true;
        for i in 0..20 {
            s.told(&mut w, head, &format!("task {i}")).unwrap();
            for _ in 0..8 {
                s.step(&mut w);
            }
            assert_eq!(w.agents[head].status, Status::Idle);
            assert!(w.change_of(head).is_none(), "answer {i}");
        }
        // The demo's own paths, and a folder that got git since, are
        // checkouts; closed and opened again, the mark goes.
        let q = s.open_solo(&mut w, "/x", None, Some("made-up")).unwrap();
        assert!(!w.projects[q].no_git);
        assert_eq!(s.close_project(&mut w, p), Ok(()));
        crate::live::testing::commit_all(&plain);
        let name = w.projects[p].name.clone();
        assert_eq!(s.open_solo(&mut w, &shown, None, Some(&name)), Ok(p));
        assert!(!w.projects[p].no_git);
        let _ = std::fs::remove_dir_all(&plain);
    }

    #[test]
    fn the_demo_retunes_a_solo_as_live_does() {
        let (mut w, mut s) = world(1, 1, 4, 7, Lang::En);
        let p = s
            .open_solo(&mut w, "/x", Some("opus".into()), Some("notes"))
            .unwrap();
        let head = w.projects[p].leader;
        let worker = w.agents.iter().find(|a| a.role == Role::Worker).unwrap().id;
        assert_eq!(
            s.rechoose(&mut w, worker, None),
            Err(format!("{} is not a solo session", w.agents[worker].name))
        );
        let e = s.rechoose(&mut w, head, Some("@turbo".into())).unwrap_err();
        assert!(e.starts_with("claude has no effort \"turbo\""), "{e}");
        assert_eq!(s.model_of(head), Some("opus"), "unchanged");
        s.told(&mut w, head, "work").unwrap();
        assert_eq!(
            s.rechoose(&mut w, head, Some("opus@max".into())),
            Err("solo-notes is mid-turn; wait for its answer".into())
        );
        for _ in 0..8 {
            s.step(&mut w);
        }
        assert_eq!(
            s.rechoose(&mut w, head, Some("opus@max".into())),
            Ok("claude opus@max".into())
        );
        assert_eq!(s.model_of(head), Some("opus@max"));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Chose { agent, choice } if *agent == head && choice == "claude opus@max"
        ));
        assert_eq!(s.rechoose(&mut w, head, None), Ok("claude".into()));
        assert_eq!(s.model_of(head), None);
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
    fn a_hired_worker_takes_the_next_free_number() {
        let (mut w, mut s) = world(1, 0, 4, 7, Lang::En);
        // Only workers count: the lead does not, and the first worker is
        // claude-1.
        let first = s.hire(&mut w, 0);
        assert_eq!(w.agents[first].name, "claude-1");
        assert_eq!(w.agents[first].project, 0);
        w.agents[first].gone = true;
        // The number of one that left is free again.
        let same = s.hire(&mut w, 0);
        assert_eq!(w.agents[same].name, "claude-1");
        let again = s.hire(&mut w, 0);
        assert_eq!(w.agents[again].name, "claude-2");
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
        let question = w.messages;
        assert_eq!(s.mind(asker).question, Some(question));
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
            assert!(
                matches!(&w.agents[id].talk, Some(Talk { with: Party::Agent(o), mine, until, .. }) if *until == t + 8 && *mine == (id == asker) && *o == if id == asker { other } else { asker }),
                "{id}"
            );
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
            Event::Sent { from, to, kind: MsgKind::Answer, text, .. } if *from == other && *to == asker && EN.answers.contains(&text.as_str())
        ));
        let Event::Sent { id, re, .. } = w.journal.last().unwrap().event else {
            unreachable!()
        };
        assert_eq!(
            (id, re),
            (question + 1, Some(question)),
            "it answers the ask"
        );
        assert_eq!(s.mind(asker).question, None);
        assert!(
            w.agents[other]
                .trace
                .back()
                .unwrap()
                .ends_with(&format!(" type=answer id={id} re={question}"))
        );
    }

    #[test]
    fn a_workers_done_replies_to_the_assign_that_gave_it_its_card() {
        let (mut w, mut s) = world(1, 1, 0, 3, Lang::En);
        let worker = workers_of(&w, 0)[0];
        let lead = w.projects[0].leader;
        let waiting = |w: &World| {
            w.cards
                .iter()
                .position(|c| c.column == Column::Waiting)
                .unwrap()
        };
        let card = waiting(&w);
        s.take(&mut w, worker, card, Taken::Assigned);
        let assign = w.messages;
        assert_eq!(s.mind(worker).assigned, Some(assign));
        // The newest message; the start may say more after it.
        let sent = w
            .journal
            .iter()
            .rev()
            .find(|e| matches!(e.event, Event::Sent { .. }));
        assert!(matches!(
            &sent.unwrap().event,
            Event::Sent { from, to, kind: MsgKind::Assign, id, re: None, .. }
                if *from == lead && *to == worker && *id == assign
        ));
        s.done(&mut w, worker);
        let done = w.journal.iter().rev().find_map(|e| match &e.event {
            Event::Sent {
                kind: MsgKind::Done,
                id,
                re,
                ..
            } => Some((*id, *re)),
            _ => None,
        });
        assert_eq!(done, Some((assign + 1, Some(assign))));
        assert_eq!(s.mind(worker).assigned, None);
        // A card it claimed came with no message: its done replies to none.
        w.agents[worker].card = None;
        let card = s.new_card(&mut w, 0, "x".into());
        s.take(&mut w, worker, card, Taken::Claimed);
        s.done(&mut w, worker);
        assert!(matches!(
            w.journal
                .iter()
                .rev()
                .find(|e| matches!(e.event, Event::Sent { .. }))
                .unwrap()
                .event,
            Event::Sent {
                kind: MsgKind::Done,
                re: None,
                ..
            }
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
                mine: true,
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

    #[test]
    fn a_change_asks_for_the_check_and_a_yes_runs_it_on_each_the_third_failing() {
        let (mut w, mut s, worker, _) = one_at_work();
        let change = |w: &mut World| {
            w.change_ready(Change {
                agent: worker,
                title: "t".into(),
                files: Vec::new(),
                patch: String::new(),
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            })
        };
        let check_of = |w: &World| w.change_of(worker).unwrap().check.clone();
        change(&mut w);
        s.check(&mut w, worker);
        assert_eq!(check_of(&w), Check::Asked);
        let item = (w.inbox.iter())
            .position(|i| i.kind == InboxKind::Check)
            .expect("asked");
        assert_eq!(w.inbox[item].detail, setup::CHECK_FILE);
        let script = s.setup_asked(&w, item).unwrap();
        assert_eq!(script.kind, setup::Kind::Check);
        assert_eq!(script.bytes, DEMO_CHECK.as_bytes());
        assert_eq!(
            script.real.to_string_lossy(),
            format!("/demo/{}/.imece/check", w.projects[0].name)
        );
        // A second change while the ask waits counts on it.
        change(&mut w);
        s.check(&mut w, worker);
        assert_eq!(check_of(&w), Check::Asked);
        assert_eq!(
            (w.inbox.iter())
                .filter(|i| i.kind == InboxKind::Check)
                .count(),
            1
        );
        assert!(s.decide_setup(&mut w, item, true));
        assert!(!w.inbox[item].open);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckAnswered {
                agent: worker,
                yes: true
            }
        );
        assert_eq!(check_of(&w), Check::Running);
        assert!(
            (w.agents[worker].trace.back().unwrap()).ends_with("check: runs on the change"),
            "{:?}",
            w.agents[worker].trace.back()
        );
        // Two ticks: the first passes.
        s.checks_due(&mut w);
        assert_eq!(check_of(&w), Check::Running, "not due yet");
        w.tick += CHECK_TICKS;
        s.checks_due(&mut w);
        assert_eq!(
            check_of(&w),
            Check::Ran(Checked {
                ok: true,
                end: "exit 0, 41.2 s".into(),
                lines: Vec::new()
            })
        );
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Checked { agent, ok: true, .. } if agent == worker
        ));
        assert_eq!(CHECK_TICKS, 2);
        // Approved now: the next changes run at once; every third fails.
        for n in 2..=4 {
            change(&mut w);
            s.check(&mut w, worker);
            assert_eq!(check_of(&w), Check::Running, "change {n}");
            w.tick += CHECK_TICKS;
            s.checks_due(&mut w);
            let Check::Ran(d) = check_of(&w) else {
                panic!("change {n}: {:?}", check_of(&w));
            };
            assert_eq!(d.ok, n != 3, "change {n}");
            if n == 3 {
                assert_eq!(d.end, "exit 1, 38.7 s");
                assert_eq!(d.lines, DEMO_CHECK_SAYS);
                assert!(matches!(
                    &w.journal.last().unwrap().event,
                    Event::Checked { ok: false, lines, .. } if lines.len() == 3
                ));
            }
        }
        // A change gone before its check ends: nothing said.
        change(&mut w);
        s.check(&mut w, worker);
        w.change_rejected(worker);
        w.tick += CHECK_TICKS;
        let journaled = w.journal.len();
        s.checks_due(&mut w);
        assert_eq!(w.journal.len(), journaled);
        assert!(s.extras.checks_due.is_empty());
        // A no: nothing on the change, not asked again this run.
        let (mut w, mut s, worker, _) = one_at_work();
        change(&mut w);
        s.check(&mut w, worker);
        let item = (w.inbox.iter())
            .position(|i| i.kind == InboxKind::Check)
            .expect("asked");
        assert!(s.decide_setup(&mut w, item, false));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckAnswered {
                agent: worker,
                yes: false
            }
        );
        assert_eq!(check_of(&w), Check::None);
        change(&mut w);
        s.check(&mut w, worker);
        assert_eq!(check_of(&w), Check::None);
        assert!(
            w.inbox
                .iter()
                .all(|i| i.kind != InboxKind::Check || !i.open)
        );
    }
    #[test]
    fn a_taken_card_owns_its_leases_files_and_every_fourth_card_s_worker_strays_once() {
        let (mut w, mut s) = world(2, 40, 4, 3, Lang::En);
        // The trace holds the last 200 lines and a change is kept until
        // the next: both are read as they come.
        let (mut noted, mut outside) = (0, 0);
        for _ in 0..2500 {
            let lines = w.journal.len();
            run(&mut w, &mut s, 1, true);
            for e in &w.journal[lines..] {
                // The change of a worker that strayed holds the file, so
                // the Δ says what is outside.
                if let Event::ChangeReady { agent, .. } = &e.event
                    && let Some(card) = w.agents[*agent].card
                    && (w.cards[card].story.iter())
                        .any(|s| matches!(s.event, StoryEvent::Strayed(_)))
                {
                    let ch = w.change_of(*agent).expect("the change");
                    let c = &w.cards[card];
                    assert!(
                        (ch.files.iter()).any(|f| crate::paths::strays(&c.paths, &f.path)),
                        "#{}: {:?} all in {:?}",
                        card + 1,
                        ch.files,
                        c.paths
                    );
                    outside += 1;
                }
                if let Event::Strayed { agent, card, path } = &e.event {
                    let trace = &w.agents[*agent].trace;
                    let note = format!(
                        "note you changed {path}, which #{} does not own; say so in your done",
                        card + 1
                    );
                    assert!(trace.iter().any(|l| l.ends_with(&note)), "{note}");
                    let edit = format!("tool_use Edit {path}");
                    assert!(trace.iter().any(|l| l.ends_with(&edit)), "{edit}");
                    noted += 1;
                }
            }
        }
        assert!(
            noted >= 2 && outside >= 1,
            "{noted} strayed, {outside} changes"
        );
        let taken: Vec<&Card> = w.cards.iter().filter(|c| c.owner.is_some()).collect();
        assert!(taken.len() > 8, "{} taken", taken.len());
        for c in &taken {
            // Each leased file once, in the order first leased.
            let mut files: Vec<&str> = Vec::new();
            for s in &c.symbols {
                if !files.contains(&s.file) {
                    files.push(s.file);
                }
            }
            let paths: Vec<&str> = c.paths.iter().map(|o| o.path.as_str()).collect();
            assert_eq!(paths, files, "#{}", c.id + 1);
            assert!(c.paths.iter().all(|o| !o.shared));
            let strayed: Vec<&String> = (c.story.iter())
                .filter_map(|s| match &s.event {
                    StoryEvent::Strayed(f) => Some(f),
                    _ => None,
                })
                .collect();
            let can = outside_file(&w, c.id).is_some();
            if c.id % STRAY_EVERY == 1 && can {
                // One still at work may not have reached the middle.
                if c.column != Column::Done {
                    assert!(strayed.len() <= 1, "#{} strays once", c.id + 1);
                    continue;
                }
                assert_eq!(strayed.len(), 1, "#{} strays once", c.id + 1);
                assert!(crate::paths::strays(&c.paths, strayed[0]));
                assert!(
                    (c.story.iter())
                        .any(|s| matches!(&s.event, StoryEvent::Touched(f) if f == strayed[0])),
                    "the write is a change of the card too"
                );
                let owner = c.owner.unwrap();
                let said = Event::Strayed {
                    agent: owner,
                    card: c.id,
                    path: strayed[0].clone(),
                };
                assert_eq!(w.journal.iter().filter(|e| e.event == said).count(), 1);
            } else {
                assert!(strayed.is_empty(), "#{} stays in its paths", c.id + 1);
            }
        }
    }

    #[test]
    fn the_file_a_worker_strays_into_is_the_first_of_the_code_map_outside_its_card() {
        let (mut w, _) = world(1, 2, 0, 1, Lang::En);
        let repo = w.projects[0].repo;
        assert!(repo.len() >= 2);
        w.cards[0].paths = vec![Owned::new(repo[0].0)];
        assert_eq!(outside_file(&w, 0), Some(repo[1].0));
        w.cards[0].paths = repo.iter().map(|(f, _)| Owned::new(f)).collect();
        assert_eq!(outside_file(&w, 0), None, "a card that owns the whole map");
        w.cards[0].paths = Vec::new();
        assert_eq!(
            outside_file(&w, 0),
            None,
            "a card without paths has no outside"
        );
    }
    #[test]
    fn a_worker_strays_from_forty_percent_on() {
        assert!(!strays_now(0));
        assert!(!strays_now(39));
        assert!(strays_now(40));
        assert!(strays_now(41));
        assert!(strays_now(100));
        // The step itself: before the middle nothing, at it the write,
        // the story and the note, and not on a card that is not the
        // fourth.
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let repo = w.projects[0].repo;
        let id = (w.agents.iter())
            .position(|a| a.role == Role::Worker)
            .expect("a worker");
        while w.cards.len() <= STRAY_EVERY + 1 {
            s.new_card(&mut w, 0, "more".into());
        }
        let strayed = |w: &World, card: CardId| {
            (w.cards[card].story.iter())
                .filter(|s| matches!(s.event, StoryEvent::Strayed(_)))
                .count()
        };
        for card in [1, STRAY_EVERY + 1] {
            w.cards[card].paths = vec![Owned::new(repo[0].0)];
            w.agents[id].card = Some(card);
            w.agents[id].progress = 39;
            Sim::stray(&mut w, id, card);
            assert_eq!(strayed(&w, card), 0, "#{} at 39%", card + 1);
            w.agents[id].progress = 40;
            Sim::stray(&mut w, id, card);
            assert_eq!(strayed(&w, card), 1, "#{} at 40%", card + 1);
            let edit = format!("tool_use Edit {}", repo[1].0);
            assert!(
                w.agents[id].trace.iter().any(|l| l.ends_with(&edit)),
                "{edit}"
            );
            let note = format!(
                "note you changed {}, which #{} does not own; say so in your done",
                repo[1].0,
                card + 1
            );
            assert!(
                w.agents[id]
                    .trace
                    .back()
                    .is_some_and(|l| l.ends_with(&note)),
                "{note}"
            );
            assert_eq!(
                w.agents[id].activity.as_deref(),
                Some(&*format!("Edit {}", repo[1].0))
            );
            w.agents[id].trace.clear();
        }
        w.cards[2].paths = vec![Owned::new(repo[0].0)];
        w.agents[id].card = Some(2);
        w.agents[id].progress = 100;
        Sim::stray(&mut w, id, 2);
        assert_eq!(strayed(&w, 2), 0, "#3 is not a fourth card");
        assert!(w.agents[id].trace.is_empty());
    }

    #[test]
    fn the_demo_leader_s_plan_has_a_card_per_waiting_card_each_owning_one_file() {
        let (mut w, mut s) = world(1, 1, 0, 5, Lang::En);
        w.projects[0].plan.on = true;
        for c in &mut w.cards {
            c.column = Column::Done;
        }
        for title in ["one", "two", "three"] {
            s.new_card(&mut w, 0, title.into());
        }
        let repo = w.projects[0].repo;
        s.propose(&mut w, 0);
        let item = w.plan_item(0).expect("the plan waits");
        assert_eq!(w.inbox[item].agent, 0);
        assert_eq!(w.inbox[item].detail, "one");
        let p = w.projects[0].plan.proposal.clone().expect("a plan");
        assert_eq!(p.cost, 3 * 4_000);
        assert_eq!(p.tick, w.tick);
        let titles: Vec<&str> = p.cards.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["one", "two", "three"]);
        for (i, c) in p.cards.iter().enumerate() {
            assert!(Tier::from_wire(&c.tier).is_some(), "{}", c.tier);
            assert_eq!(c.paths, [Owned::new(repo[i].0)], "card {}", i + 1);
            assert_eq!(
                c.body,
                format!("{}. Done when {} passes.", c.title, kit(0).test)
            );
        }
        assert!(
            w.agents[0]
                .trace
                .back()
                .unwrap()
                .ends_with(&format!("plan 3 cards $1.20 (item {item})"))
        );
        assert_eq!(w.plan_hold(0), Some(PlanHold::Reading));
        // No: the plan goes and the leader asks the person what to change.
        assert!(s.decide_plan(&mut w, item, false));
        assert!(!s.decide_plan(&mut w, item, false), "answered already");
        assert_eq!(w.projects[0].plan.proposal, None);
        assert_eq!(w.plan_hold(0), Some(PlanHold::NoPlan));
        let asked = (w.inbox.iter())
            .find(|i| i.open && i.kind == InboxKind::Question && i.agent == 0)
            .expect("the leader asks");
        assert!(
            asked.detail.starts_with("You said no to the plan"),
            "{}",
            asked.detail
        );
        // More waiting cards than files: the rest own nothing.
        for i in 0..repo.len() {
            s.new_card(&mut w, 0, format!("more {i}"));
        }
        s.propose(&mut w, 0);
        let p = w.projects[0].plan.proposal.clone().expect("a plan");
        assert_eq!(p.cards.len(), 3 + repo.len());
        assert!(p.cards[repo.len()..].iter().all(|c| c.paths.is_empty()));
        let item = w.plan_item(0).unwrap();
        assert!(s.decide_plan(&mut w, item, true));
        assert_eq!(w.plan_hold(0), None);
        assert!(w.agents[0].trace.back().unwrap().ends_with("plan yes"));
    }

    #[test]
    fn the_second_project_is_on_the_demos_store_and_answers_from_its_table() {
        let (mut w, mut s) = world(2, 2, 0, 1, Lang::En);
        let repo = w.projects[1].name.clone();
        assert_eq!(w.stores(), [(1, repo.clone())], "only the second");
        assert_eq!(w.store_closed(1), Some("imece is still asking Lonca"));
        assert_eq!(w.store_closed(0), None);
        s.step(&mut w);
        let seen = &w.projects[1].lonca.as_ref().unwrap().seen;
        assert_eq!(
            *seen,
            StoreSeen::Up {
                trunk: demo_trunk(0),
                pending: 0,
                behind: Behind::By(0),
            }
        );
        assert_eq!(w.store_closed(1), None);
        // Its waiting changes are what is pending, and no other project's.
        let (worker, other) = (2, 3);
        assert_eq!((w.agents[worker].project, w.agents[other].project), (0, 1));
        let change = |agent| Change {
            agent,
            title: "t".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::default(),
            spend: None,
        };
        let (a, b) = (change(worker), change(other));
        w.change_ready(a);
        s.store_answers(&mut w);
        let pending = |w: &World| match &w.projects[1].lonca.as_ref().unwrap().seen {
            StoreSeen::Up { pending, .. } => *pending,
            seen => panic!("{seen:?}"),
        };
        assert_eq!(pending(&w), 0);
        w.change_ready(b);
        s.store_answers(&mut w);
        assert_eq!(pending(&w), 1);
        // A repository the table lacks.
        w.on_lonca(1, Some("nobody".into()));
        s.store_answers(&mut w);
        assert_eq!(
            w.projects[1].lonca.as_ref().unwrap().seen,
            StoreSeen::Down(StoreDown::Forbidden)
        );
    }

    #[test]
    fn the_demos_daemon_stops_for_a_while_and_closes_its_project() {
        assert!(!store_stopped(0));
        assert!(!store_stopped(53));
        assert!(store_stopped(54));
        assert!(store_stopped(59));
        assert!(!store_stopped(60));
        assert!(!store_stopped(113));
        assert!(store_stopped(114));
        let (mut w, s) = world(2, 2, 0, 1, Lang::En);
        s.store_answers(&mut w);
        w.tick = 54;
        s.store_answers(&mut w);
        assert_eq!(w.store_closed(1), Some(StoreDown::NoDaemon.why()));
        w.tick = 60;
        s.store_answers(&mut w);
        assert_eq!(w.store_closed(1), None);
        let said: Vec<&Event> = (w.journal.iter()).map(|e| &e.event).collect();
        assert!(said.iter().any(|e| matches!(e, Event::StoreDown { .. })));
        assert!(said.iter().any(|e| matches!(e, Event::StoreBack { .. })));
    }

    /// Runs `job` to its end: not a tick early.
    fn run_job(s: &mut Sim, w: &mut World, job: StoreJob) {
        assert!(s.store_job(w, job));
        w.tick += STORE_JOB_TICKS - 1;
        s.store_job_due(w);
        assert!(s.store_busy(), "not yet");
        w.tick += 1;
        s.store_job_due(w);
        assert!(!s.store_busy());
    }

    /// What the demo's daemon says of how far project 1's checkout is
    /// behind.
    fn demo_behind(s: &Sim, w: &mut World) -> Behind {
        w.tick = 0;
        s.store_answers(w);
        match &w.projects[1].lonca.as_ref().unwrap().seen {
            StoreSeen::Up { behind, .. } => *behind,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_demos_checkout_falls_behind_and_sync_brings_it_up() {
        let (mut w, mut s) = world(2, 2, 0, 1, Lang::En);
        let repo = w.projects[1].lonca.as_ref().unwrap().repo.clone();
        let job = StoreJob::Sync {
            project: 1,
            repo: repo.clone(),
        };
        assert_eq!(demo_behind(&s, &mut w), Behind::By(0));
        s.extras.store[1].1 = 2;
        assert_eq!(demo_behind(&s, &mut w), Behind::By(2));
        run_job(&mut s, &mut w, job.clone());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::StoreSynced {
                how: Synced::Moved,
                ..
            }
        ));
        assert_eq!(demo_behind(&s, &mut w), Behind::By(0));
        run_job(&mut s, &mut w, job.clone());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::StoreSynced {
                how: Synced::Even,
                ..
            }
        ));
        // Every third finds the person's own commits, and nothing moves.
        s.extras.store[1].1 = 3;
        run_job(&mut s, &mut w, job.clone());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::StoreSynced {
                how: Synced::Apart,
                ..
            }
        ));
        assert_eq!(demo_behind(&s, &mut w), Behind::Never);
        run_job(&mut s, &mut w, job);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::StoreSynced {
                how: Synced::Moved,
                ..
            }
        ));
        assert_eq!(demo_behind(&s, &mut w), Behind::By(0));
    }

    #[test]
    fn a_demo_import_adds_a_row_and_refuses_one_that_exists() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        w.on_lonca(0, None);
        let ask = s.import_ask(&w, 0, "fresh").unwrap();
        assert_eq!(ask.branch, "main");
        assert_eq!(ask.repo, "fresh");
        run_job(&mut s, &mut w, StoreJob::Import(ask.clone()));
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "fresh");
        s.store_answers(&mut w);
        assert_eq!(w.store_closed(0), None, "its daemon knows it now");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::StoreImported { warnings, .. } if warnings.len() == 1
        ));
        w.on_lonca(0, None);
        run_job(&mut s, &mut w, StoreJob::Import(ask));
        assert!(w.projects[0].lonca.is_none());
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::StoreFailed { why: Some(why), .. } if why.ends_with("repository fresh exists")
        ));
    }

    #[test]
    fn a_demo_login_is_told_and_nothing_is_due_with_no_job() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let said = w.journal.len();
        s.store_job_due(&mut w);
        assert_eq!(w.journal.len(), said);
        assert!(!s.asking());
        assert!(s.store_job(&w, StoreJob::Login(0)));
        assert!(s.asking());
        assert!(!s.store_job(&w, StoreJob::Login(0)), "one at a time");
    }

    #[test]
    fn a_demo_project_whose_store_is_down_takes_no_decision_and_no_fork() {
        let (mut w, mut s, worker, _) = one_at_work();
        s.test(&mut w, worker);
        s.tested(&mut w, worker, 0);
        s.mind(worker).started = true;
        assert!(w.change_of(worker).is_some());
        let p = w.agents[worker].project;
        w.on_lonca(p, Some("r".into()));
        w.store_seen(p, "r", StoreSeen::Down(StoreDown::NoDaemon));
        let why = Err(StoreDown::NoDaemon.why().to_string());
        assert_eq!(s.apply_change(&w, worker, Vec::new()), why);
        assert!(s.applying_now().is_empty());
        assert_eq!(s.reject_change(&mut w, worker), why);
        assert!(w.change_of(worker).is_some(), "it still waits");
        assert_eq!(s.fork(&mut w, worker), why.map(|()| 0));
        // Back up, the same decision goes.
        let up = StoreSeen::Up {
            trunk: demo_trunk(0),
            pending: 1,
            behind: Behind::By(0),
        };
        w.store_seen(p, "r", up);
        assert_eq!(s.apply_change(&w, worker, Vec::new()), Ok(()));
    }

    #[test]
    fn a_change_that_landed_moves_the_demos_trunk() {
        let (w, mut s) = world(2, 2, 0, 1, Lang::En);
        let repo = w.projects[1].name.clone();
        s.store_integrated(&w, 1);
        assert_eq!(s.extras.store[1], (repo.clone(), 1));
        s.store_integrated(&w, 0);
        assert_eq!(s.extras.store[0].1, 0, "the first project is on no store");
        assert_eq!(s.extras.store[1], (repo, 1));
        assert_ne!(demo_trunk(0), demo_trunk(1));
        assert_eq!(demo_trunk(1).len(), 40);
        assert_eq!(&demo_trunk(0)[..8], "9d3f1c2a");
        assert_eq!(&demo_trunk(1)[..8], "9d9098eb");
        assert_eq!(&demo_trunk(2)[..8], "9de215ac");
        assert!(demo_trunk(1)[8..].bytes().all(|b| b == b'0'));
    }

    #[test]
    fn a_leader_is_short_of_hands_when_work_waits_and_nobody_is_free() {
        assert!(short_of_hands(2, 0, MAX_WORKERS - 1));
        assert!(!short_of_hands(2, 0, MAX_WORKERS), "the team is full");
        assert!(!short_of_hands(1, 0, 1), "too little waits");
        assert!(!short_of_hands(2, 1, 1), "someone is free");
    }

    /// The demo's first project on a store, as `seen` says.
    fn on_store(w: &mut World, seen: StoreSeen) {
        w.on_lonca(0, Some("r".into()));
        w.store_seen(0, "r", seen);
    }

    fn up() -> StoreSeen {
        StoreSeen::Up {
            trunk: demo_trunk(0),
            pending: 0,
            behind: Behind::By(0),
        }
    }

    #[test]
    fn a_demo_leader_whose_store_is_down_hires_nobody() {
        let (mut w, mut s) = world(1, 1, 0, 1, Lang::En);
        let leader = w.projects[0].leader;
        w.projects[0].plan.on = false;
        let hired = |w: &World| workers_of(w, 0).len();
        let before = hired(&w);
        on_store(&mut w, StoreSeen::Down(StoreDown::NoDaemon));
        s.more_hands(&mut w, 0, leader);
        assert_eq!(hired(&w), before);
        w.store_seen(0, "r", up());
        s.more_hands(&mut w, 0, leader);
        assert_eq!(hired(&w), before + 1);
    }

    #[test]
    fn a_demo_workspace_left_behind_can_not_be_read_and_goes_only_on_a_yes() {
        let (mut w, mut s, _, _) = one_at_work();
        assert_eq!(demo_orphans(&w).len(), 2, "off Lonca");
        on_store(&mut w, up());
        let found = demo_orphans(&w);
        assert_eq!(found.len(), 3);
        assert!(found[2].unread && !found[2].clean());
        let name = w.projects[0].name.clone();
        assert!(
            found[2]
                .path
                .ends_with(format!("{name}-5e1f0a2c/claude-33"))
        );
        assert_eq!(found[2].checkout, PathBuf::from(format!("~/src/{name}")));
        w.orphans.found = Some(found);
        s.extras.orphans = w.orphans.found.clone();
        let script = teardown_script(&PathBuf::from(format!("~/src/{name}")));
        w.setup_approved.push(script.key());
        s.tidy(&mut w, &[2], false).unwrap();
        let told = s.extras.tidy_due.clone().flatten().unwrap();
        let why = told.kept[0].1.as_deref();
        assert_eq!(why, Some(WORKSPACE_MAY_HOLD));
    }

    #[test]
    fn a_demo_worker_on_lonca_works_in_a_workspace_and_does_not_fork() {
        let (mut w, mut s, worker, card) = one_at_work();
        assert_eq!(w.agents[worker].workspace, None, "off Lonca");
        on_store(&mut w, up());
        assert_eq!(w.agents[worker].workspace, None, "a worktree stays");
        let id = s.hire_for_test(&mut w, 0);
        assert_eq!(w.agents[id].workspace.as_deref(), Some("r"));
        let traced = |w: &World| {
            w.agents[id]
                .trace
                .iter()
                .any(|l| l.contains("workspace of r"))
        };
        assert!(traced(&w));
        let helper = s.helper_for(&mut w, id, card);
        assert_eq!(w.agents[helper].workspace.as_deref(), Some("r"));
        s.mind(id).started = true;
        w.agents[id].status = Status::Idle;
        assert_eq!(s.fork(&mut w, id), Err(ON_LONCA_FORK.to_string()));
        assert_eq!(w.agents[worker].workspace, None);
    }

    #[test]
    fn a_demo_worker_whose_store_is_down_takes_no_helper() {
        let (mut w, mut s, worker, card) = one_at_work();
        on_store(&mut w, StoreSeen::Down(StoreDown::NoDaemon));
        s.helper_if_it_may(&mut w, worker, card);
        assert_eq!(w.helpers_of(worker).count(), 0);
        w.store_seen(0, "r", up());
        s.helper_if_it_may(&mut w, worker, card);
        assert_eq!(w.helpers_of(worker).count(), 1);
        s.helper_if_it_may(&mut w, worker, card);
        assert_eq!(w.helpers_of(worker).count(), 1, "one helper is enough");
    }
}
