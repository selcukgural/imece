//! Lonca's store as this engine sees it (record 116): one thread asks the
//! daemon how each project's repository stands, so the screen never waits
//! on a socket; what it said closes a project to hires and Δ decisions or
//! opens it ([`World::store_closed`]), and what waits there is the workers'
//! Δ (record 120). imece only reads here; what must be signed the `lonca`
//! program does, on a thread of its own, when the person asks (record 117).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use imece_agent::Event as AgentEvent;
use imece_agent::lonca::{
    self, Client, Diff, Down, Failed, Hooks, Place, Seat, Standing, TRUNK_REF,
};
use imece_agent::worktree::{self, Forward};

use super::Live;
use super::{Moved, Slot, apply};
use crate::model::{
    Agent, AgentId, Attested, Behind, Cap, Change, ChangedFile, Check, Decision, Event, ImportAsk,
    Landed, MsgKind, ProjectId, Role, StoreDown, StoreJob, StoreLease, StorePending, StoreProposal,
    StoreSeen, Synced, World, home_tilde,
};

/// How often the store is asked. Each look is a connection and a
/// millisecond call per repository; a person sees a daemon gone or a
/// proposal come within this.
pub(super) const LOOK_EVERY: Duration = Duration::from_secs(5);
/// How many looks in a row a repository's daemon may ask to be asked again
/// (`busy`, `rate_limited`, …) before its project closes as
/// [`StoreDown::Busy`]: one such answer keeps what the last look saw.
const AGAIN_MOST: u8 = 3;

/// The repositories to look at, each with its project and, where git is,
/// its checkout.
type Repos = Vec<(ProjectId, String, Option<PathBuf>)>;
/// One look: the repositories, and the changes whose end is to be read.
type Asked = (Repos, Vec<String>);
/// What a look found of them.
type Found = Vec<(ProjectId, String, StoreSeen)>;

/// How long agents' sessions have left, in milliseconds: repository,
/// agent, time.
type Left = Vec<(String, String, i64)>;

/// A worker's Lonca session with less than this left is renewed before
/// its next prompt: a turn should not run out of session halfway (record 119).
pub(super) const SEAT_MARGIN: Duration = Duration::from_secs(30 * 60);

/// How long after its harness started a session must have been bound
/// before a look takes `bound_ms` `null` as hooks that never ran: the bind
/// comes from its first hook, which may still be on its way.
pub(super) const BIND_GRACE: Duration = Duration::from_secs(10);

/// What a worker hears when it goes on after imece stopped it for hooks
/// Lonca did not hear (lonca#190).
pub(super) const SEAT_UNHOOKED: &str = "Lonca did not hear your hooks in your last session, so imece \
                                        stopped it; a new Lonca session goes on in this workspace: \
                                        your files and proposals are as they were, your leases are \
                                        gone, so lease again before you edit. The new session stays \
                                        tainted: a critical path is refused";

/// Why a worker stopped whose session's hooks Lonca did not hear, as
/// its row says it; the demo says the same.
pub(crate) fn unheard_why(hooks: &Hooks) -> String {
    let go_on = "write to it to go on under a new Lonca session";
    match hooks {
        Hooks::Missed(tool) => {
            format!("Lonca saw a call to its {tool} tool that its hooks did not report: {go_on}")
        }
        _ => format!("Lonca never heard from its hooks: {go_on}"),
    }
}

/// What a worker hears after its session was renewed.
pub(super) const SEAT_RENEWED: &str = "your Lonca session ran out of time and a new one goes on in \
                                       this workspace: your files and proposals are as they were, \
                                       your leases are gone, so lease again before you edit";

/// A worker hired before its project went on Lonca, told at the first
/// prompt in the workspace that took its worktree's place (record 119).
pub(super) const ONTO_LONCA: &str = "your project is on Lonca now: your directory, the same path, \
                                     is a Lonca workspace in place of your git worktree, so \
                                     your work reaches the repository as a proposal (your \
                                     instructions say how); lease what you edit";

/// The longest reason told of a proposal that did not land, in characters.
const REASON_TOLD: usize = 400;
/// The most of a failed check's output a worker is told.
const OUTPUT_TOLD: usize = 1_000;
/// The most lines of a failed check's output kept.
const ATTEST_LINES: usize = 6;
/// The line the verifier starts a log with, which says nothing of the check.
const VERIFY_HEADER: &str = "lonca verify:";

/// A worker's newest proposal, as a look read it: `diff` is read once.
#[derive(Clone, Debug)]
struct Offer {
    /// Whose it is, as Lonca took the name.
    agent: String,
    title: String,
    proposal: StoreProposal,
    diff: Arc<Diff>,
}

/// What one look found.
#[derive(Default)]
struct Looked {
    /// When it was asked: a harness started later is not what it saw.
    asked: Option<Instant>,
    found: Found,
    left: Left,
    /// Each repository looked at, with its agents whose live session's
    /// hooks the daemon did not hear (lonca#190).
    hooks: Vec<(String, Vec<(String, Hooks)>)>,
    /// What waits in each repository, by worker.
    offers: Vec<(ProjectId, Offer)>,
    /// Where every change of each repository stands.
    standings: HashMap<String, Standing>,
    /// The changes a waiting proposal says it replaces.
    replaced: HashSet<String>,
    /// What Lonca's queue checked on the watched changes that settled.
    evidence: HashMap<String, Vec<Attested>>,
    /// What is held in each repository.
    leases: Vec<(ProjectId, String, Vec<lonca::Lease>)>,
    /// Every proposal waiting in each repository, with its trunk.
    pending: Vec<(ProjectId, String, String, Vec<lonca::Pending>)>,
    /// The checks each repository's policy defines, when Lonca says.
    policy_checks: Vec<(ProjectId, String, Option<Vec<String>>)>,
}

/// What the looks keep: the store does not change what it said.
#[derive(Default)]
struct Kept {
    /// The diffs of what waits.
    diffs: HashMap<String, Arc<Diff>>,
    /// Each repository's policy and whether it holds a change for a
    /// person. Only an answer is kept.
    held: HashMap<String, (String, bool)>,
    /// How many looks in a row each repository's daemon asked to be asked
    /// again.
    again: HashMap<String, u8>,
}

impl Kept {
    /// Whether policy `policy` of `repo` holds a change for a person.
    /// What can not be read does not (record 120).
    fn holds(&mut self, client: &mut Client, repo: &str, policy: &str) -> bool {
        if let Some((_, held)) = self.held.get(repo).filter(|(p, _)| p == policy) {
            return *held;
        }
        let Ok(held) = client.holds_unobserved(repo, policy) else {
            return false;
        };
        self.held
            .insert(repo.to_string(), (policy.to_string(), held));
        held
    }

    /// The diff of `change`; none while the daemon can not say.
    fn diff(&mut self, client: &mut Client, repo: &str, change: &str) -> Option<Arc<Diff>> {
        if let Some(diff) = self.diffs.get(change) {
            return Some(diff.clone());
        }
        let diff = Arc::new(client.diff(repo, change).ok()?);
        self.diffs.insert(change.to_string(), diff.clone());
        Some(diff)
    }
}

/// What waits for a person in `status`, per worker: its newest proposal
/// (Lonca withdraws a workspace's waiting one when it proposes again).
/// Anything else that waits is the count on the project's line.
fn offers(status: &lonca::Status, read: &mut impl FnMut(&str) -> Option<Arc<Diff>>) -> Vec<Offer> {
    let mut by_agent: BTreeMap<&str, Vec<&lonca::Pending>> = BTreeMap::new();
    let proposals = (status.pending.iter()).filter(|p| p.op == lonca::PROPOSE);
    for p in proposals {
        if let Some(agent) = p.agent.as_deref() {
            by_agent.entry(agent).or_default().push(p);
        }
    }
    let mut out = Vec::new();
    for (agent, mut waiting) in by_agent {
        waiting.sort_by_key(|p| p.seq);
        let Some(newest) = waiting.pop() else {
            continue;
        };
        let Some(change) = newest.change.as_deref() else {
            continue;
        };
        let Some(diff) = read(change) else {
            continue;
        };
        out.push(Offer {
            agent: agent.to_string(),
            title: newest.intent.clone().unwrap_or_default(),
            proposal: StoreProposal {
                seq: newest.seq,
                change: change.to_string(),
                why: newest.reasons.clone(),
                redacted: diff.redacted.clone(),
                cut: diff.cut,
                verifies: newest.runs.clone().into(),
                on_trunk: newest.base.as_deref() == Some(status.trunk.as_str()),
                intent: newest.intent.clone().unwrap_or_default(),
            },
            diff,
        });
    }
    out
}

/// A proposal's change as the person reads it, titled as its worker's
/// card `card` says (the intent, the worker's words, is its text: they
/// name cards by other numbers); without a card, by the intent.
fn change_of(id: AgentId, offer: &Offer, card: Option<&str>) -> Change {
    let files = offer.diff.files.iter().map(|f| ChangedFile {
        path: f.path.clone(),
        added: f.lines.map_or(0, |(added, _)| added),
        removed: f.lines.map_or(0, |(_, removed)| removed),
        binary: f.lines.is_none(),
    });
    Change {
        agent: id,
        title: card.map_or_else(|| offer.title.clone(), str::to_string),
        files: files.collect(),
        patch: offer.diff.patch.clone(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    }
}

/// What `standing` says became of a proposal: nothing while it waits, or
/// for a state this imece does not know.
fn landed(standing: &Standing) -> Option<Landed> {
    match standing {
        Standing::Pending | Standing::Other => None,
        Standing::Queued => Some(Landed::Queued),
        Standing::Integrated(commit) => Some(Landed::Integrated(commit.clone())),
        Standing::Conflicted(symbols) => Some(Landed::Conflicted(symbols.clone())),
        Standing::Rejected(reasons) => Some(Landed::Rejected(reasons.clone())),
        Standing::Withdrawn => Some(Landed::Withdrawn),
    }
}

/// The reason a proposal gives for the waiting one of its workspace that
/// Lonca withdrew when it came (Lonca 0.2.1): `replaces:<change>`.
const REPLACES: &str = "replaces:";

/// The changes the proposals waiting in `status` say they replace.
fn replaced_in(status: &lonca::Status) -> impl Iterator<Item = String> + '_ {
    (status.pending.iter())
        .flat_map(|p| &p.reasons)
        .filter_map(|why| why.strip_prefix(REPLACES))
        .map(str::to_string)
}

/// `how` a proposal left, said as a replacement when a newer one of its
/// workspace names it: its worker took nothing back.
fn as_replaced(how: Landed, replaced: bool) -> Landed {
    match how {
        Landed::Withdrawn if replaced => Landed::Replaced,
        how => how,
    }
}

/// What a worker hears of its proposal, `title`: nothing for one that
/// needs nothing of it; whether it has to act comes with it.
fn told(how: &Landed, title: &str, checks: &[Attested]) -> Option<(String, bool)> {
    let some = |words: &[String]| {
        let all = words.join("; ");
        all.chars().take(REASON_TOLD).collect::<String>()
    };
    // A proposal the queue kept has lost its title with its Δ.
    let named = match title {
        "" => String::new(),
        title => format!(" ({title})"),
    };
    match how {
        Landed::Queued | Landed::Withdrawn | Landed::Replaced => None,
        Landed::Integrated(commit) => Some((
            format!(
                "Your proposal{named} is on trunk as {commit}. Call the sync tool before you \
                 edit again."
            ),
            false,
        )),
        Landed::Conflicted(symbols) => Some((
            format!(
                "Lonca's merge queue could not merge your proposal{named}: {}. Call the sync \
                 tool, settle the conflict markers it leaves in your files, and propose again.",
                some(symbols)
            ),
            true,
        )),
        // Proposing again does not help until a person acts, or the day
        // turns: the worker says it is blocked instead.
        Landed::Rejected(reasons) if reasons.iter().any(|r| held_back(r)) => Some((
            format!(
                "Your proposal{named} was refused: {}. Lonca takes no proposal of yours for \
                 now, so do not propose again: it stays in your workspace. Tell your leader \
                 you are blocked, with this reason.",
                some(reasons),
            ),
            true,
        )),
        Landed::Rejected(reasons) => Some((
            format!(
                "Your proposal{named} was refused: {}.{} It stays in your workspace. Put it \
                 right, call the sync tool, and propose again.",
                some(reasons),
                failed_output(checks),
            ),
            true,
        )),
    }
}

/// Whether Lonca refused for a reason a new proposal does not get past: a
/// verification quota used up, or the repository frozen.
fn held_back(reason: &str) -> bool {
    reason.starts_with("verification_quota:") || reason.starts_with("frozen:")
}

/// What the checks that failed printed, for the worker who has to put it
/// right; nothing when none did.
fn failed_output(checks: &[Attested]) -> String {
    let said = (checks.iter())
        .filter(|c| !c.ok && !c.lines.is_empty())
        .map(|c| format!(" {} ({}):\n{}\n", c.check, c.end, c.lines.join("\n")));
    let all: String = said.collect();
    all.chars().take(OUTPUT_TOLD).collect()
}

/// What a worker hears of a `done` with work no proposal of its carries.
pub(super) const PROPOSE_FIRST: &str = "propose first: your workspace holds changes no proposal \
                                        of yours carries. Lease what you changed, preview, \
                                        propose, then say done";

/// What it hears when that can not be told.
const NO_WORD: &str = "Lonca did not say what you have proposed, so this done did not go; try \
                       again, and say blocked when it goes on";

/// A worker's done, held until a thread saw what it proposed.
pub(super) struct Gated {
    from: AgentId,
    args: serde_json::Value,
    reply: Sender<imece_agent::mcp::Answer>,
    passed: Result<(), String>,
}

/// Whether a worker at `tree` of `repo` may say done: nothing there to
/// propose, or the change it proposed last still stands. What it edited
/// after that proposal is not told apart: only Lonca's facts could.
fn proposed(place: &Place, repo: &str, tree: &Path) -> Result<(), String> {
    if place.left_in(tree) == Some(0) {
        return Ok(());
    }
    let Some(change) = lonca::workspace_submitted(tree) else {
        return Err(PROPOSE_FIRST.to_string());
    };
    let standing = place.open().and_then(|mut c| c.standing(repo, &change));
    match standing {
        Ok(Standing::Pending | Standing::Queued | Standing::Integrated(_)) => Ok(()),
        Ok(_) => Err(PROPOSE_FIRST.to_string()),
        Err(_) => Err(NO_WORD.to_string()),
    }
}

/// What a job came back with.
enum Done {
    Login(Result<String, Failed>),
    /// An import, and why critical paths stay refused if they do.
    Import(Result<lonca::Imported, Failed>, Option<Failed>),
    Sync(Result<Synced, Failed>),
    Decide(Result<(), Failed>),
}

/// `job` run by the `lonca` program: login in the project's checkout `dir`.
/// A daemon with no repository yet can not sign anyone in, so the first
/// import comes before any session; the same yes signs the person in then
/// (what fails here shows as `⌀ no lonca` with its word, record 117).
fn run(place: &Place, dir: &Path, job: &StoreJob) -> Done {
    match job {
        StoreJob::Login(_) => Done::Login(place.login(dir)),
        StoreJob::Import(ask) => {
            let done = place.import(&ask.repo, &ask.checkout, &ask.branch);
            if done.is_ok() && matches!(place.open(), Err(Down::SignedOut)) {
                let _ = place.login(dir);
            }
            let kept = (done.is_ok())
                .then(|| place.critical_waits(&ask.repo, &ask.checkout))
                .and_then(Result::err);
            Done::Import(done, kept)
        }
        StoreJob::Sync { repo, .. } => Done::Sync(sync(place, repo, dir)),
        StoreJob::Decide {
            repo,
            proposal,
            verdict,
            ..
        } => Done::Decide(decide(place, repo, dir, proposal, verdict)),
    }
}

/// The person's word on `proposal`: an older one of its worker's waits no
/// more, Lonca withdrew it when this one came (`replaces:`).
fn decide(
    place: &Place,
    repo: &str,
    dir: &Path,
    proposal: &StoreProposal,
    verdict: &Decision,
) -> Result<(), Failed> {
    match verdict {
        Decision::Approve(also) => place.approve(repo, dir, proposal.seq, also),
        Decision::Reject(reason) => place.reject(repo, dir, proposal.seq, reason),
    }
}

/// Trunk of `repo` into the checkout `dir`: fetched, then the branch forward
/// when that is a fast-forward. Nothing else: a checkout with commits of its
/// own stays, and work in the way is git's refusal.
fn sync(place: &Place, repo: &str, dir: &Path) -> Result<Synced, Failed> {
    place.fetch(repo, dir)?;
    match worktree::fast_forward(dir, TRUNK_REF) {
        Ok(Forward::Even) => Ok(Synced::Even),
        Ok(Forward::Moved) => Ok(Synced::Moved),
        Ok(Forward::Apart) => Ok(Synced::Apart),
        Err(why) => Err(Failed::Said(why.to_string())),
    }
}

/// What the world's line says of why a command did not go through.
fn said(failed: Failed) -> Option<String> {
    match failed {
        Failed::NoAnswer => None,
        Failed::Said(words) => Some(words),
    }
}

/// Where a writer of a project on `repo` works (record 119), at its tree
/// `tree`: `None`, a worktree as ever; `Some(made)`, the workspace there,
/// `made` just now. A project off Lonca gets a worktree, and a worker
/// that has one keeps it: only the person moves work. A workspace whose
/// session is alive is taken as it stands, which keeps its leases; one
/// over goes on under a new session, and so does one that `renew` says is
/// about to be. Whoever works there next must find its session alive, so a
/// workspace nobody can vouch for stops the start.
pub(super) fn seat(
    place: &Place,
    repo: Option<&str>,
    tree: &Path,
    agent: &str,
    scratch: Option<&Path>,
    renew: bool,
) -> Result<Option<bool>, String> {
    let told = |why: Failed| match said(why) {
        Some(words) => format!("its Lonca session: {words}"),
        None => "its Lonca session: lonca did not answer".to_string(),
    };
    let seat_on = |repo| Seat {
        repo,
        agent,
        workspace: tree,
        scratch,
    };
    if lonca::is_workspace(tree) {
        let Some(repo) = repo else {
            return Err(format!(
                "{} is a Lonca workspace and its project is on no repository: name it with :lonca",
                tree.display()
            ));
        };
        if lonca::workspace_repo(tree).as_deref() != Some(repo) {
            return Err(format!(
                "{} is no workspace of {repo}: leave its project on its own repository",
                tree.display()
            ));
        }
        if renew || !place.seated(tree).map_err(told)? {
            place.sit_again(&seat_on(repo)).map_err(told)?;
        }
        return Ok(Some(false));
    }
    let Some(repo) = repo.filter(|_| !tree.exists()) else {
        return Ok(None);
    };
    place.sit(&seat_on(repo)).map_err(told)?;
    Ok(Some(true))
}

/// At a start of a worker hired before its project went on Lonca: its
/// worktree at `at` goes when it holds nothing (`worktree::held`), so
/// that a workspace takes its place; one that holds work stays, said as
/// [`Moved::Kept`]. A workspace, nothing there, or no worktree of a
/// checkout: nothing to do. git's own refusal to remove stops the start.
pub(super) fn move_off(at: &Path) -> Result<Moved, String> {
    if !at.exists() || lonca::is_workspace(at) {
        return Ok(Moved::Stayed);
    }
    match worktree::held(at) {
        Ok((_, held)) if held.clean() => {
            worktree::remove(at, false)
                .map_err(|e| format!("its worktree {}: {e}", home_tilde(at)))?;
            Ok(Moved::Onto)
        }
        Ok((_, held)) => Ok(Moved::Kept(held)),
        Err(_) => Ok(Moved::Stayed),
    }
}

/// The thread that asks the daemon, and when it is next asked.
pub(super) struct Watch {
    /// Where the daemon and the `lonca` program are, for a job.
    place: Place,
    /// The job under way: one at a time, never on the screen's thread.
    job: Option<(StoreJob, Receiver<Done>)>,
    ask: Sender<Asked>,
    back: Receiver<Looked>,
    /// Changes approved and in the queue, and whose they are: what the
    /// queue makes of them is still to be said.
    queued: HashMap<String, AgentId>,
    /// Rejections the worker hears from the person's own line.
    pub(super) worded: Vec<String>,
    /// Dones held for their gate, and where they come back.
    gated: Sender<Gated>,
    gates: Receiver<Gated>,
    /// When the newest session of each repository's agents ends, as the
    /// last look read it; an agent not here is not known to be short.
    pub(super) ends: HashMap<(String, String), Instant>,
    /// The agents of each repository whose live session's hooks the last
    /// look of it did not hear: a worker's next start goes on under a new
    /// session.
    pub(super) unheard: HashMap<(String, String), Hooks>,
    /// When each agent's harness last started (Claude Code's `init`).
    pub(super) started: HashMap<AgentId, Instant>,
    /// A look is under way: never a second, a slow daemon gets no queue.
    out: bool,
    /// When the next look is due; none: at once.
    next: Option<Instant>,
}

impl Watch {
    /// Where the daemon and the `lonca` program are.
    pub(super) fn place(&self) -> &Place {
        &self.place
    }

    /// What a look read, as moments: its time is from `now`, one already
    /// over ending now.
    fn note_ends(&mut self, left: Left, now: Instant) {
        for (repo, agent, ms) in left {
            let ms = u64::try_from(ms).unwrap_or(0);
            self.ends
                .insert((repo, agent), now + Duration::from_millis(ms));
        }
    }

    /// What a look heard of the hooks of each repository it read: those
    /// it did not read keep what the last look of them said.
    fn note_hooks(&mut self, hooks: Vec<(String, Vec<(String, Hooks)>)>) {
        for (repo, unheard) in hooks {
            self.unheard.retain(|(r, _), _| *r != repo);
            for (agent, h) in unheard {
                self.unheard.insert((repo.clone(), agent), h);
            }
        }
    }

    pub(super) fn start(place: Place) -> Watch {
        let (ask, asked) = mpsc::channel::<Asked>();
        let (answer, back) = mpsc::channel();
        let (gated, gates) = mpsc::channel();
        let kept = place.clone();
        thread::spawn(move || {
            let mut kept = Kept::default();
            for (repos, watched) in asked {
                if answer
                    .send(look(&place, &repos, &watched, &mut kept))
                    .is_err()
                {
                    return;
                }
            }
        });
        Watch {
            place: kept,
            job: None,
            ask,
            back,
            queued: HashMap::new(),
            worded: Vec::new(),
            gated,
            gates,
            ends: HashMap::new(),
            unheard: HashMap::new(),
            started: HashMap::new(),
            out: false,
            next: None,
        }
    }
}

/// How `repos` stand: one connection, a status for each. A daemon that can
/// not be opened leaves them all down for the same reason. A repository
/// whose policy does not hold a change for a person is closed: the Δ gate
/// would not be there (record 120). A daemon that asks to be asked again
/// says nothing of the repository until it has asked [`AGAIN_MOST`] looks
/// in a row, and the next repository is asked on a new connection: the
/// daemon may have closed this one.
fn look(place: &Place, repos: &Repos, watched: &[String], kept: &mut Kept) -> Looked {
    let mut client = place.open();
    let mut looked = Looked {
        asked: Some(Instant::now()),
        ..Looked::default()
    };
    let mut waiting: Vec<String> = Vec::new();
    for (p, repo, checkout) in repos {
        let told = client.as_mut().map_err(|down| *down);
        let seen = match told.and_then(|c| Ok((c.status(repo)?, c))) {
            Ok((status, c)) => {
                let times = status.time_left().into_iter();
                (looked.left).extend(times.map(|(agent, ms)| (repo.clone(), agent, ms)));
                looked.hooks.push((repo.clone(), status.hooks()));
                looked
                    .leases
                    .push((*p, repo.clone(), status.leases.clone()));
                if status.frozen.is_some() {
                    StoreSeen::Down(StoreDown::Frozen)
                } else if kept.holds(c, repo, &status.policy) {
                    let proposals = (status.pending.iter())
                        .filter(|w| w.op == lonca::PROPOSE)
                        .cloned();
                    let trunk = status.trunk.clone();
                    (looked.pending).push((*p, repo.clone(), trunk, proposals.collect()));
                    let names = status.policy_checks.clone();
                    (looked.policy_checks).push((*p, repo.clone(), names));
                    let mut read = |change: &str| kept.diff(c, repo, change);
                    let offered = offers(&status, &mut read);
                    waiting.extend(offered.iter().map(|o| o.proposal.change.clone()));
                    looked.offers.extend(offered.into_iter().map(|o| (*p, o)));
                    for change in settled(&status, watched) {
                        if let Ok(found) = c.evidence(repo, &change) {
                            looked.evidence.insert(change, attested(&found));
                        }
                    }
                    looked.replaced.extend(replaced_in(&status));
                    looked.standings.extend(status.changes);
                    StoreSeen::Up {
                        behind: behind_of(c, repo, checkout.as_deref(), &status.trunk),
                        trunk: status.trunk,
                        pending: status.pending.len(),
                    }
                } else {
                    StoreSeen::Down(StoreDown::Unguarded)
                }
            }
            Err(Down::Again) => {
                client = place.open();
                let asked = kept.again.entry(repo.clone()).or_default();
                *asked = asked.saturating_add(1);
                if *asked < AGAIN_MOST {
                    continue;
                }
                StoreSeen::Down(StoreDown::Busy)
            }
            Err(down) => StoreSeen::Down(down_of(down)),
        };
        if seen != StoreSeen::Down(StoreDown::Busy) {
            kept.again.remove(repo);
        }
        looked.found.push((*p, repo.clone(), seen));
    }
    kept.diffs.retain(|change, _| waiting.contains(change));
    looked
}

/// `lease` as the code map shows it, its holder an agent of project `p`
/// when it is one (record 123): a helper works under its worker's session.
fn lease_of(w: &World, p: ProjectId, lease: lonca::Lease) -> StoreLease {
    let name = lease.agent.unwrap_or_else(|| "?".to_string());
    StoreLease {
        file: lease.file,
        symbol: lease.symbol,
        holder: Live::owner(w, p, &name),
        name,
        shared: lease.shared,
    }
}

/// `pending` as the verdict reads it (record 125): its proposer an agent
/// of project `p` when it is one, its base against `trunk`. Nothing for
/// an operation that is no proposal of a change.
fn pending_of(
    w: &World,
    p: ProjectId,
    trunk: &str,
    pending: lonca::Pending,
) -> Option<StorePending> {
    let name = pending.agent?;
    Some(StorePending {
        seq: pending.seq,
        change: pending.change?,
        holder: Live::owner(w, p, &name),
        name,
        paths: pending.paths,
        on_trunk: pending.base.as_deref() == Some(trunk),
    })
}

/// Those of `watched` that `status` has settled, so that its queue has
/// spoken: a conflict or a withdrawal ran nothing worth saying.
fn settled(status: &lonca::Status, watched: &[String]) -> Vec<String> {
    (status.changes.iter())
        .filter(|(change, _)| watched.contains(change))
        .filter(|(_, s)| matches!(s, Standing::Integrated(_) | Standing::Rejected(_)))
        .map(|(change, _)| change.clone())
        .collect()
}

/// What the screen and the worker get of `evidence`: a check skipped is no
/// check. Output comes only for one that failed, its last lines.
fn attested(evidence: &[lonca::Evidence]) -> Vec<Attested> {
    let lines = |log: &str| {
        let said = (log.lines().rev())
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with(VERIFY_HEADER));
        let mut last: Vec<String> = said.take(ATTEST_LINES).map(str::to_string).collect();
        last.reverse();
        last
    };
    (evidence.iter())
        .filter(|e| e.result != "skipped")
        .map(|e| {
            let ok = e.result == "pass";
            let secs = e.duration_ms.map(|ms| ms as f64 / 1000.0);
            Attested {
                check: e.check.clone(),
                ok,
                end: match (e.exit, secs) {
                    (Some(code), Some(s)) => format!("exit {code}, {s:.1} s"),
                    (Some(code), None) => format!("exit {code}"),
                    (None, _) => String::new(),
                },
                lines: (e.log.as_deref())
                    .filter(|_| !ok)
                    .map_or_else(Vec::new, lines),
            }
        })
        .collect()
}

/// How far the checkout is behind `trunk`, read from where it left trunk,
/// so commits of the person's own on top do not hide it. What can not be
/// read is `Unknown`: this is a word on a line and closes nothing.
fn behind_of(client: &mut Client, repo: &str, checkout: Option<&Path>, trunk: &str) -> Behind {
    let Some(base) = checkout.and_then(|c| worktree::near(c, TRUNK_REF)) else {
        return Behind::Unknown;
    };
    if base == trunk {
        return Behind::By(0);
    }
    match client.distance(repo, &base) {
        Ok(Some(n)) => Behind::By(n),
        Ok(None) => Behind::Never,
        Err(_) => Behind::Unknown,
    }
}

/// The screen's word for why the client can not read the store.
fn down_of(down: Down) -> StoreDown {
    match down {
        Down::NoDaemon => StoreDown::NoDaemon,
        Down::NotTheDaemon => StoreDown::NotTheDaemon,
        Down::Protocol => StoreDown::Protocol,
        Down::Old => StoreDown::Old,
        Down::Hardened => StoreDown::Hardened,
        Down::SignedOut => StoreDown::SignedOut,
        Down::Forbidden => StoreDown::Forbidden,
        Down::NoRepo => StoreDown::NoRepo,
        Down::Refused => StoreDown::Refused,
        Down::Again => StoreDown::Busy,
    }
}

impl Live {
    /// The store is asked when a project is on Lonca and it is time.
    pub(super) fn lonca_due(&mut self, w: &World, now: Instant) {
        let watch = &mut self.lonca;
        if watch.out || watch.next.is_some_and(|at| now < at) {
            return;
        }
        let repos: Repos = (w.stores().into_iter())
            .map(|(p, repo)| {
                let place = self.places.get(p).filter(|place| place.git);
                (p, repo, place.map(|place| place.repo.clone()))
            })
            .collect();
        let shown = w.proposals.values().map(|p| p.change.clone());
        let watched = shown.chain(watch.queued.keys().cloned()).collect();
        if !repos.is_empty() && watch.ask.send((repos, watched)).is_ok() {
            watch.out = true;
            watch.next = Some(now + LOOK_EVERY);
        }
    }

    /// What the person is asked before project `p`'s checkout goes into
    /// Lonca as `repo`; none while it is on no branch.
    pub fn import_ask(&self, p: ProjectId, repo: &str) -> Option<ImportAsk> {
        let checkout = self.places[p].repo.clone();
        let (branch, head) = worktree::on_branch(&checkout)?;
        Some(ImportAsk {
            project: p,
            repo: repo.to_string(),
            checkout,
            branch,
            head,
        })
    }

    /// Whether a `lonca` command of the person's is under way.
    pub fn store_busy(&self) -> bool {
        self.lonca.job.is_some()
    }

    /// `job` starts, unless one is under way: the screen goes on.
    pub fn store_job(&mut self, job: StoreJob) -> bool {
        if self.lonca.job.is_some() {
            return false;
        }
        let place = self.lonca.place.clone();
        let dir = self.places[job.project()].repo.clone();
        let (tell, told) = mpsc::channel();
        let asked = job.clone();
        thread::spawn(move || {
            let _ = tell.send(run(&place, &dir, &asked));
        });
        self.lonca.job = Some((job, told));
        true
    }

    /// What the job under way came to, once it has: the journal says it,
    /// and the store is asked at once. Whether the screen changed.
    fn take_store_job(&mut self, w: &mut World) -> bool {
        let Some((job, told)) = &self.lonca.job else {
            return false;
        };
        let done = match told.try_recv() {
            Ok(done) => Some(done),
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => None,
        };
        let decided = match job {
            StoreJob::Decide { agent, .. } => Some(*agent),
            _ => None,
        };
        match (done, job) {
            (Some(Done::Login(Ok(human))), _) => w.store_signed_in(job.project(), human),
            // Lonca has it: the next look says what became of it, whoever's
            // hand it was.
            (Some(Done::Decide(Ok(()))), _) => {}
            (Some(Done::Import(Ok(made), kept)), StoreJob::Import(ask)) => {
                w.store_imported(ask, made.warnings, made.hardened);
                if let Some(why) = kept {
                    w.store_critical_kept(ask, said(why));
                }
            }
            (Some(Done::Sync(Ok(how))), StoreJob::Sync { project, repo }) => {
                w.store_synced(*project, repo, how);
            }
            (
                Some(
                    Done::Login(Err(e))
                    | Done::Import(Err(e), _)
                    | Done::Sync(Err(e))
                    | Done::Decide(Err(e)),
                ),
                _,
            ) => {
                w.store_failed(job, said(e));
            }
            _ => w.store_failed(job, None),
        }
        if let Some(agent) = decided {
            self.applying[agent] = false;
        }
        self.lonca.job = None;
        self.look_now();
        true
    }

    /// What agent `from` is to be gated by, when `args` is a worker's done
    /// from a workspace: its repository and tree. A helper's done is its
    /// worker's to carry, and a solo says none.
    pub(super) fn gate_of(
        &self,
        w: &World,
        from: AgentId,
        args: &serde_json::Value,
    ) -> Option<(String, PathBuf)> {
        let a = &w.agents[from];
        let done = args["type"].as_str().map(str::trim) == Some(MsgKind::Done.wire());
        // One that failed, or came late, shows no change (record 126): it
        // waits on nothing.
        let shows = args["outcome"].as_str().map(str::trim) != Some("failed")
            && !matches!(w.done_for(from), crate::model::DoneFor::Late(_));
        let repo = a
            .workspace
            .clone()
            .filter(|_| done && shows && a.role == Role::Worker)?;
        Some((repo, self.cwds[from].clone()?))
    }

    /// A thread sees what the done in `args` stands on: `lonca diff` is a
    /// child, and the screen does not wait on it.
    pub(super) fn gate(
        &self,
        from: AgentId,
        args: &serde_json::Value,
        reply: Sender<imece_agent::mcp::Answer>,
        repo: String,
        tree: PathBuf,
    ) {
        let place = self.lonca.place.clone();
        let (back, args) = (self.lonca.gated.clone(), args.clone());
        thread::spawn(move || {
            let passed = proposed(&place, &repo, &tree);
            let _ = back.send(Gated {
                from,
                args,
                reply,
                passed,
            });
        });
    }

    /// The dones whose gate answered: each goes on, or its worker hears
    /// why not. Whether any came.
    fn take_gates(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok(done) = self.lonca.gates.try_recv() {
            any = true;
            match done.passed {
                Ok(()) => self.sent_on(w, done.from, &done.args, done.reply),
                Err(why) => {
                    let _ = done.reply.send(Err(why));
                }
            }
        }
        any
    }

    /// A job's end, else what the last look found (a look that came at the
    /// same time is read at the next step). Whether the screen changed.
    pub(super) fn take_lonca(&mut self, w: &mut World) -> bool {
        let gated = self.take_gates(w);
        self.take_look_or_job(w) || gated
    }

    fn take_look_or_job(&mut self, w: &mut World) -> bool {
        if self.take_store_job(w) {
            return true;
        }
        let Ok(looked) = self.lonca.back.try_recv() else {
            return false;
        };
        self.lonca.out = false;
        self.lonca.note_ends(looked.left, Instant::now());
        self.lonca.note_hooks(looked.hooks);
        let mut changed = looked
            .asked
            .is_some_and(|asked| self.stop_unheard(w, asked));
        for (p, repo, seen) in looked.found {
            changed |= w.store_seen(p, &repo, seen);
        }
        for (p, repo, held) in looked.leases {
            let held = held.into_iter().map(|l| lease_of(w, p, l)).collect();
            changed |= w.store_leases(p, &repo, held);
        }
        for (p, repo, trunk, pending) in looked.pending {
            let pending = (pending.into_iter())
                .filter_map(|waiting| pending_of(w, p, &trunk, waiting))
                .collect();
            changed |= w.store_pending(p, &repo, pending);
        }
        for (p, repo, names) in looked.policy_checks {
            changed |= w.store_checks(p, &repo, names);
        }
        changed |= self.land(w, &looked.standings, &looked.replaced, &looked.evidence);
        for (p, offer) in &looked.offers {
            changed |= self.offered(w, *p, offer);
        }
        changed
    }

    /// The owner of proposals of `agent`, a name as Lonca took it, in
    /// project `p`: a worker or a solo, gone or not, since its proposals
    /// stay. Someone else's stay a count on the project's line.
    fn owner(w: &World, p: ProjectId, agent: &str) -> Option<AgentId> {
        (w.agents.iter())
            .filter(|a| a.project == p && a.parent.is_none() && a.role != Role::Leader)
            .find(|a| lonca::agent_word(&a.name) == agent)
            .map(|a| a.id)
    }

    /// `offer` waits in project `p`: it is its worker's change once that
    /// worker's turn is over, so that the person never approves what it
    /// is still working on. Whether the screen changed.
    fn offered(&mut self, w: &mut World, p: ProjectId, offer: &Offer) -> bool {
        let Some(id) = Self::owner(w, p, &offer.agent).filter(|&id| self.turns[id] == 0) else {
            return false;
        };
        match w.proposals.get_mut(&id) {
            Some(shown) if shown.change == offer.proposal.change => {
                let moved = shown.on_trunk != offer.proposal.on_trunk;
                shown.on_trunk = offer.proposal.on_trunk;
                moved
            }
            _ => {
                let card = w.card_of(id).map(|c| w.cards[c].title.clone());
                w.change_proposed(
                    change_of(id, offer, card.as_deref()),
                    offer.proposal.clone(),
                );
                true
            }
        }
    }

    /// What `standings` says became of the proposals the person was shown
    /// and of those in the queue: the journal says it, whoever decided, and
    /// the worker is told. Whether anything did.
    fn land(
        &mut self,
        w: &mut World,
        standings: &HashMap<String, Standing>,
        replaced: &HashSet<String>,
        evidence: &HashMap<String, Vec<Attested>>,
    ) -> bool {
        let of = |change: &str| {
            let how = standings.get(change).and_then(landed)?;
            Some(as_replaced(how, replaced.contains(change)))
        };
        let checks = |change: &str| evidence.get(change).cloned().unwrap_or_default();
        let mut shown: Vec<(AgentId, String, Landed)> = (w.proposals.iter())
            .filter_map(|(&id, p)| Some((id, p.change.clone(), of(&p.change)?)))
            .collect();
        shown.sort_by_key(|(id, ..)| *id);
        let any = !shown.is_empty();
        for (id, change, how) in shown {
            let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
            if how == Landed::Queued {
                self.lonca.queued.insert(change.clone(), id);
            }
            w.proposal_left(id, how.clone());
            let checks = checks(&change);
            w.verified(id, checks.clone());
            self.tell_landed(w, id, &change, &how, &title, &checks);
        }
        let mut queued: Vec<(String, AgentId, Landed)> = (self.lonca.queued.iter())
            .filter_map(|(c, &id)| Some((c.clone(), id, of(c).filter(|h| *h != Landed::Queued)?)))
            .collect();
        queued.sort_by_key(|(_, id, _)| *id);
        let moved = !queued.is_empty();
        for (change, id, how) in queued {
            self.lonca.queued.remove(&change);
            w.proposal_landed(id, how.clone());
            let checks = checks(&change);
            w.verified(id, checks.clone());
            self.tell_landed(w, id, &change, &how, "", &checks);
        }
        any || moved
    }

    /// Worker `id` hears what became of its proposal; one that has to act
    /// on it is woken. A rejection the person worded went to it as their
    /// message already.
    fn tell_landed(
        &mut self,
        w: &mut World,
        id: AgentId,
        change: &str,
        how: &Landed,
        title: &str,
        checks: &[Attested],
    ) {
        if let Some(i) = (self.lonca.worded.iter()).position(|c| c == change) {
            self.lonca.worded.remove(i);
            return;
        }
        let Some((words, act)) = told(how, title, checks) else {
            return;
        };
        self.notes[id].push(words);
        // Its rounds out, it wakes once the person allows one more: the
        // item holds it (record 126, F9).
        if act && let Some(c) = w.rounds_out_of(id) {
            w.capped(Cap::Rounds, id, c);
        }
        self.lost[id] |= act && !w.agents[id].gone;
    }

    /// The person approves or rejects worker `id`'s proposal: the `lonca`
    /// program signs it on a thread, and a look says what came of it.
    pub(super) fn decide_proposal(
        &mut self,
        w: &World,
        id: AgentId,
        verdict: Decision,
    ) -> Result<(), String> {
        let project = w.agents[id].project;
        if let Some(why) = w.store_closed(project) {
            return Err(why.into());
        }
        if self.applying[id] {
            return Err("their change is being applied".into());
        }
        let (Some(proposal), Some(on)) = (w.proposal_of(id), &w.projects[project].lonca) else {
            return Err("no proposal of theirs waits".into());
        };
        let job = StoreJob::Decide {
            project,
            repo: on.repo.clone(),
            agent: id,
            proposal: proposal.clone(),
            verdict,
        };
        if !self.store_job(job) {
            return Err("another lonca command is under way".into());
        }
        self.applying[id] = true;
        Ok(())
    }

    /// The person rejects worker `id`'s proposal; `reason` also goes to it
    /// as their message, which this note comes before.
    pub fn reject_proposal(&mut self, w: &World, id: AgentId, reason: &str) -> Result<(), String> {
        let shown = w.proposal_of(id).map(|p| p.change.clone());
        self.decide_proposal(w, id, Decision::Reject(reason.to_string()))?;
        self.lonca.worded.extend(shown);
        let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
        self.notes[id].push(format!(
            "The person rejected your proposal ({title}); their words follow. It stays in your \
             workspace. Put it right, call the sync tool, and propose again."
        ));
        Ok(())
    }

    /// Whether agent `id`'s Lonca session is to be renewed: it runs out
    /// within [`SEAT_MARGIN`] of `now`, and nobody else works on it. A
    /// helper shares its worker's, and `--continue` would end it under the
    /// worker; a worker waits for its helpers to be done.
    pub(super) fn seat_short(&self, w: &World, id: AgentId, now: Instant) -> bool {
        let a = &w.agents[id];
        let busy = |h: &Agent| !matches!(self.slots[h.id], Slot::None);
        if a.role == Role::Helper || w.helpers_of(id).any(busy) {
            return false;
        }
        let Some(on) = &w.projects[a.project].lonca else {
            return false;
        };
        let key = (on.repo.clone(), lonca::agent_word(&a.name));
        self.lonca
            .ends
            .get(&key)
            .is_some_and(|end| *end <= now + SEAT_MARGIN)
    }

    /// The hooks Lonca did not hear of agent `id`'s session, as the last
    /// look said; none for a helper, which works under its worker's.
    pub(super) fn unheard(&self, w: &World, id: AgentId) -> Option<&Hooks> {
        let a = &w.agents[id];
        let on = w.projects[a.project].lonca.as_ref()?;
        let key = (on.repo.clone(), lonca::agent_word(&a.name));
        self.lonca
            .unheard
            .get(&key)
            .filter(|_| a.role != Role::Helper)
    }

    /// Stops every worker in a workspace whose session's hooks Lonca did
    /// not hear (lonca#190): Lonca no longer sees what it does. Its
    /// helpers, under the same session, leave. Only a harness that was
    /// there when the look was `asked` is judged, and an unbound one only
    /// [`BIND_GRACE`] after it started. Nothing starts it again but the
    /// person, under a new session: a harness that runs no hooks would
    /// miss them again on every turn. Whether any stopped.
    fn stop_unheard(&mut self, w: &mut World, asked: Instant) -> bool {
        let judged = |live: &Live, w: &World, id: AgentId| {
            let a = &w.agents[id];
            let (Some(hooks), Some(&started)) = (live.unheard(w, id), live.lonca.started.get(&id))
            else {
                return None;
            };
            let due = match hooks {
                Hooks::Heard => return None,
                Hooks::Unbound => started + BIND_GRACE,
                Hooks::Missed(_) => started,
            };
            let running = matches!(live.slots[id], Slot::Running(_));
            (running && a.workspace.is_some() && due <= asked).then(|| hooks.clone())
        };
        let marked: Vec<(AgentId, Hooks)> = (0..w.agents.len())
            .filter_map(|id| Some((id, judged(self, w, id)?)))
            .collect();
        for (id, hooks) in &marked {
            let why = unheard_why(hooks);
            self.end(w, *id);
            self.remembered[*id] = true;
            self.notes[*id].push(SEAT_UNHOOKED.to_string());
            apply(w, *id, AgentEvent::Stopped(why), self.spent_before[*id]);
            // Helpers work for their worker alone, as when it stops for
            // any other reason.
            let helpers: Vec<AgentId> = w.helpers_of(*id).map(|h| h.id).collect();
            for h in helpers {
                self.release(w, h);
            }
        }
        !marked.is_empty()
    }

    /// Before a prompt: a worker in a workspace whose session is running
    /// out, between turns, ends its Claude Code, which goes on with the
    /// same conversation. `lonca mcp` holds the token it started with, so a
    /// new session needs a new harness, and done here, not on a timer, a
    /// team asleep overnight asks nobody for a finger (record 119).
    pub(super) fn renew_seat(&mut self, w: &mut World, id: AgentId, now: Instant) {
        let between_turns = matches!(self.slots[id], Slot::Running(_)) && self.turns[id] == 0;
        if !between_turns || w.agents[id].workspace.is_none() || !self.seat_short(w, id, now) {
            return;
        }
        self.end(w, id);
        self.remembered[id] = true;
        self.notes[id].push(SEAT_RENEWED.to_string());
        let t = w.tick;
        w.agents[id].trace(t, "its Lonca session is running out: a new one takes over");
    }

    /// Whether worker `id`, of a project on Lonca, may be moved from its
    /// worktree onto a workspace at its next start (record 119): a
    /// worker or solo not in a workspace, started at no branch of its
    /// own, whose worktree was not found holding work since its change
    /// was last in, with no change waiting and no helper at work in its
    /// tree, while the store answers.
    pub(super) fn may_move(&self, w: &World, id: AgentId) -> bool {
        let a = &w.agents[id];
        let p = a.project;
        matches!(a.role, Role::Worker | Role::Solo)
            && a.workspace.is_none()
            && a.start.is_none()
            && !self.off_lonca[id]
            && w.projects[p].lonca.is_some()
            && w.store_closed(p).is_none()
            && self.places[p].git
            && w.change_of(id).is_none()
            && w.helpers_of(id)
                .all(|h| matches!(self.slots[h.id], Slot::None))
    }

    /// Before a prompt: a worker hired before its project went on Lonca,
    /// between turns in its worktree, ends its session, which goes on in
    /// the same conversation at the start the prompt makes, where its
    /// worktree goes and a workspace takes its place (`move_off`). Done
    /// at its next task, never mid-turn.
    pub(super) fn onto_lonca(&mut self, w: &mut World, id: AgentId) {
        let between_turns = matches!(self.slots[id], Slot::Running(_)) && self.turns[id] == 0;
        if !between_turns || !self.may_move(w, id) {
            return;
        }
        self.end(w, id);
        self.remembered[id] = true;
        let t = w.tick;
        w.agents[id].trace(
            t,
            "its project is on Lonca: its session goes on where a workspace takes its worktree's place",
        );
    }

    /// What became of worker `id`'s worktree at the start that just came
    /// (`move_off`): the journal says it, the worker hears where it is,
    /// and one that held work is left alone until its change is in.
    pub(super) fn moved(&mut self, w: &mut World, id: AgentId, moved: Moved) {
        match moved {
            Moved::Stayed => {}
            Moved::Onto => {
                let repo = (w.projects[w.agents[id].project].lonca.as_ref())
                    .map(|on| on.repo.clone())
                    .unwrap_or_default();
                w.note(Event::OntoLonca { agent: id, repo });
                self.notes[id].push(ONTO_LONCA.to_string());
            }
            Moved::Kept(held) => {
                self.off_lonca[id] = true;
                w.note(Event::OffLonca {
                    agent: id,
                    changes: held.changes,
                    commits: held.commits,
                });
            }
        }
    }

    /// The next step asks at once: the person just named a repository.
    pub fn look_now(&mut self) {
        self.lonca.next = None;
    }

    /// A look was just made, for a test.
    #[cfg(test)]
    pub fn lonca_after_a_look(&mut self) {
        self.lonca.next = Some(Instant::now() + LOOK_EVERY);
    }

    /// Whether the next step asks, for a test.
    #[cfg(test)]
    pub fn lonca_is_due(&self) -> bool {
        self.lonca.next.is_none()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use imece_agent::lonca::PROTOCOL;
    use imece_agent::lonca_fake::{Daemon, POLICY, TRUNK, status};

    use super::*;
    use crate::live::testing::{Stub, idle_live, idle_live_in, team, with_card, with_helper};
    use crate::model::{Event, Runs, Status, StoreAct};

    const WAIT: Duration = Duration::from_secs(10);

    fn up_daemon() -> Daemon {
        let d = Daemon::start(PROTOCOL, |asked| {
            if asked["repo"] == "demo" {
                Ok(status(&[7, 8]))
            } else {
                Err("not_found")
            }
        });
        d.sign_in();
        d
    }

    fn seen_of(w: &World, p: ProjectId) -> StoreSeen {
        w.projects[p].lonca.as_ref().unwrap().seen.clone()
    }

    /// Steps until the look under way came back, what it brought kept.
    fn settle(live: &mut Live, w: &mut World) {
        settled(live, w);
    }

    /// Waits for the look out to come back; whether it changed the screen.
    fn settled(live: &mut Live, w: &mut World) -> bool {
        let till = Instant::now() + WAIT;
        let mut changed = false;
        while live.lonca.out {
            changed |= live.take_lonca(w);
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
        changed
    }

    #[test]
    fn a_look_reads_every_repository_on_one_connection() {
        let d = up_daemon();
        let repos = vec![
            (0, "demo".to_string(), None),
            (3, "other".to_string(), None),
        ];
        let found = look(&d.place, &repos, &[], &mut Kept::default()).found;
        assert_eq!(found.len(), 2);
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 2,
            behind: Behind::Unknown,
        };
        assert_eq!(found[0], (0, "demo".to_string(), up));
        let none = StoreSeen::Down(StoreDown::NoRepo);
        assert_eq!(found[1], (3, "other".to_string(), none));
    }

    #[test]
    fn a_daemon_that_asks_again_closes_nothing_until_it_has_asked_three_looks_in_a_row() {
        let busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let now = busy.clone();
        let d = Daemon::start(PROTOCOL, move |asked| match asked["repo"].as_str() {
            Some("demo") if now.load(std::sync::atomic::Ordering::Relaxed) => Err("closed"),
            Some("demo" | "other") => Ok(status(&[7, 8])),
            _ => Err("not_found"),
        });
        d.sign_in();
        let repos = vec![
            (0, "demo".to_string(), None),
            (3, "other".to_string(), None),
        ];
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 2,
            behind: Behind::Unknown,
        };
        let mut kept = Kept::default();
        for _ in 0..AGAIN_MOST - 1 {
            let looked = look(&d.place, &repos, &[], &mut kept);
            // The daemon closed the connection: the next repository is
            // asked on a new one.
            assert_eq!(looked.found, [(3, "other".to_string(), up.clone())]);
            assert_eq!(looked.leases.len(), 1, "nothing of demo moves");
        }
        let busy_now = StoreSeen::Down(StoreDown::Busy);
        for _ in 0..2 {
            let found = look(&d.place, &repos, &[], &mut kept).found;
            assert_eq!(found[0], (0, "demo".to_string(), busy_now.clone()));
        }
        // An answer starts the count again.
        busy.store(false, std::sync::atomic::Ordering::Relaxed);
        let found = look(&d.place, &repos, &[], &mut kept).found;
        assert_eq!(found[0], (0, "demo".to_string(), up));
        busy.store(true, std::sync::atomic::Ordering::Relaxed);
        let found = look(&d.place, &repos, &[], &mut kept).found;
        assert_eq!(found.len(), 1, "one ask again closes nothing: {found:?}");
    }

    #[test]
    fn a_daemon_that_can_not_be_opened_leaves_every_repository_down() {
        let d = up_daemon();
        let mut place = d.place.clone();
        place.socket = d.dir.join("nobody");
        let repos = vec![
            (0, "demo".to_string(), None),
            (1, "other".to_string(), None),
        ];
        let Looked { found, left, .. } = look(&place, &repos, &[], &mut Kept::default());
        assert!(left.is_empty());
        let down = StoreSeen::Down(StoreDown::NoDaemon);
        assert_eq!(found[0].2, down);
        assert_eq!(found[1].2, down);
    }

    /// The job comes back: `take_lonca` is asked until it has.
    fn job_ends(live: &mut Live, w: &mut World) {
        let started = Instant::now();
        while !live.take_lonca(w) {
            assert!(started.elapsed() < Duration::from_secs(10), "no end");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn last(w: &World) -> &Event {
        &w.journal.last().unwrap().event
    }

    fn live_with(program_body: &str) -> (Live, std::path::PathBuf) {
        let (place, dir) = imece_agent::lonca_fake::program(program_body);
        let mut live = idle_live_in(&dir, 2);
        live.lonca = Watch::start(place);
        (live, dir)
    }

    /// `args` of git in `dir`, what it printed.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Commits `file` in `dir`; its commit.
    fn commit(dir: &Path, file: &str) -> String {
        std::fs::write(dir.join(file), file).unwrap();
        git(dir, &["add", "."]);
        let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
        git(
            dir,
            &[&who[..], &["commit", "--quiet", "-m", file]].concat(),
        );
        git(dir, &["rev-parse", "HEAD"])
    }

    /// A daemon that says `behind` of every commit.
    fn distant(behind: serde_json::Value) -> Daemon {
        let d = Daemon::start(PROTOCOL, move |asked| {
            if asked["method"] == "trunk.distance" {
                Ok(serde_json::json!({ "behind": behind }))
            } else {
                Ok(status(&[]))
            }
        });
        d.sign_in();
        d
    }

    #[test]
    fn how_far_behind_is_read_from_where_the_checkout_left_trunk() {
        let repo = crate::live::testing::checkout("behind");
        let head = git(&repo, &["rev-parse", "HEAD"]);
        let behind = |d: &Daemon, checkout: Option<&Path>, trunk: &str| {
            behind_of(&mut d.place.open().unwrap(), "demo", checkout, trunk)
        };
        let d = distant(serde_json::json!(3));
        assert_eq!(behind(&d, None, TRUNK), Behind::Unknown, "no checkout");
        assert_eq!(behind(&d, Some(&repo), &head), Behind::By(0), "level");
        assert_eq!(behind(&d, Some(&repo), TRUNK), Behind::By(3));
        let never = distant(serde_json::Value::Null);
        assert_eq!(behind(&never, Some(&repo), TRUNK), Behind::Never);
        let odd = distant(serde_json::json!("many"));
        assert_eq!(behind(&odd, Some(&repo), TRUNK), Behind::Unknown);
        std::fs::remove_dir_all(&repo).unwrap();
    }

    /// A directory for a tree, a place for its scratch, and the fake
    /// `lonca` that runs `body`, which it removes with [`gone`].
    fn seat_at(body: &str) -> (Place, PathBuf, PathBuf, PathBuf) {
        let (place, dir) = imece_agent::lonca_fake::program(body);
        (place, dir.join("tree"), dir.join("scratch"), dir)
    }

    fn make_workspace(tree: &Path, repo: &str) {
        std::fs::create_dir_all(tree.join(".lonca")).unwrap();
        let file = tree.join(".lonca/workspace.json");
        std::fs::write(file, format!(r#"{{"repo":"{repo}"}}"#)).unwrap();
    }

    fn ran(dir: &Path) -> Option<String> {
        std::fs::read_to_string(dir.join("seen")).ok()
    }

    #[test]
    fn a_project_off_lonca_gets_a_worktree_and_asks_nobody() {
        let (place, tree, _, dir) = seat_at("exit 0");
        assert_eq!(seat(&place, None, &tree, "claude-1", None, false), Ok(None));
        assert_eq!(ran(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_with_a_worktree_stays_in_it_when_its_project_goes_on_lonca() {
        let (place, tree, _, dir) = seat_at("exit 0");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join(".git"), "gitdir: /x").unwrap();
        let asked = seat(&place, Some("demo"), &tree, "claude-1", None, false);
        assert_eq!(asked, Ok(None));
        assert_eq!(ran(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_worker_on_lonca_sits_in_a_new_workspace() {
        let (place, tree, scratch, dir) = seat_at("echo '{}'");
        let own = Some(scratch.as_path());
        let asked = seat(&place, Some("demo"), &tree, "claude-1", own, false);
        assert_eq!(asked, Ok(Some(true)));
        let told = ran(&dir).unwrap();
        assert!(told.contains("--repo=demo --agent=claude-1 "), "{told}");
        let at = format!(
            "--workspace {} --scratch {}",
            tree.display(),
            scratch.display()
        );
        assert!(told.contains(&at), "{told}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_whose_session_lives_is_taken_as_it_stands() {
        let (place, tree, _, dir) = seat_at("echo '{}'");
        make_workspace(&tree, "demo");
        let asked = seat(&place, Some("demo"), &tree, "claude-1", None, false);
        assert_eq!(asked, Ok(Some(false)));
        assert!(ran(&dir).unwrap().contains("session heartbeat"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_about_to_run_out_goes_on_under_a_new_one_unasked() {
        let (place, tree, _, dir) = seat_at("echo '{}'");
        make_workspace(&tree, "demo");
        let asked = seat(&place, Some("demo"), &tree, "claude-1", None, true);
        assert_eq!(asked, Ok(Some(false)));
        let told = ran(&dir).unwrap();
        assert!(
            told.contains(&format!("--continue {}", tree.display())),
            "{told}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_renewal_asks_nothing_of_a_tree_that_is_no_workspace() {
        let (place, tree, _, dir) = seat_at("echo '{}'");
        std::fs::create_dir_all(&tree).unwrap();
        let asked = seat(&place, Some("demo"), &tree, "claude-1", None, true);
        assert_eq!(asked, Ok(None));
        assert_eq!(ran(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A team of one on `demo`, its session ending `left` from now.
    fn seated_with(left: Duration) -> (World, Live, Instant) {
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        w.agents[1].workspace = Some("demo".into());
        let mut live = idle_live(2);
        let now = Instant::now();
        let name = w.agents[1].name.clone();
        live.lonca.ends.insert(("demo".into(), name), now + left);
        (w, live, now)
    }

    #[test]
    fn a_session_is_short_within_its_margin_and_not_beyond() {
        // Half an hour, in numbers: the constant is what is under test.
        let (w, live, now) = seated_with(Duration::from_secs(1800));
        assert!(live.seat_short(&w, 1, now), "at the margin");
        let (w, live, now) = seated_with(Duration::from_secs(1801));
        assert!(!live.seat_short(&w, 1, now), "past it");
        let (mut w, live, now) = seated_with(Duration::ZERO);
        assert!(live.seat_short(&w, 1, now), "over");
        w.projects[0].lonca = None;
        assert!(!live.seat_short(&w, 1, now), "off Lonca");
        let (mut w, live, now) = seated_with(Duration::ZERO);
        w.agents[1].name = "another".into();
        assert!(!live.seat_short(&w, 1, now), "not known");
    }

    #[test]
    fn a_session_of_two_is_not_renewed_under_either() {
        let (mut w, mut live, now) = seated_with(Duration::ZERO);
        let helper = with_helper(&mut w, 1);
        live.grow(None, false);
        assert!(!live.seat_short(&w, helper, now), "a helper never");
        assert!(live.seat_short(&w, 1, now), "its helper is not working");
        live.slots[helper] = Slot::Starting(Vec::new());
        assert!(!live.seat_short(&w, 1, now), "its helper works");
    }

    /// The acceptance run: workers hired before `:lonca import` kept
    /// their worktrees and their work skipped Lonca. A worktree that holds
    /// nothing goes, so that the workspace takes its place; one that
    /// holds work stays, said.
    #[test]
    fn a_worktree_moves_off_for_a_workspace_only_when_it_holds_nothing() {
        use crate::live::testing::{checkout, trees};
        let repo = checkout("move-off-repo");
        let root = trees("move-off");
        let clean = worktree::ensure(&repo, &root, "claude-1", None)
            .unwrap()
            .path;
        assert_eq!(move_off(&clean), Ok(Moved::Onto));
        assert!(!clean.exists(), "gone, for the workspace");
        let busy = worktree::ensure(&repo, &root, "claude-2", None)
            .unwrap()
            .path;
        std::fs::write(busy.join("new.txt"), "work").unwrap();
        let kept = move_off(&busy).unwrap();
        assert!(
            matches!(kept, Moved::Kept(h) if h.changes == 1 && h.commits == 0),
            "{kept:?}"
        );
        assert!(busy.join("new.txt").exists(), "nothing lost");
        // Nothing there, or a folder that is no worktree: nothing to do.
        assert_eq!(move_off(&root.join("none")), Ok(Moved::Stayed));
        let plain = root.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(move_off(&plain), Ok(Moved::Stayed));
        assert!(plain.exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_worker_hired_before_the_import_moves_at_its_next_turn_and_is_told() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.on_lonca(0, Some("demo".into()));
        assert!(!live.may_move(&w, 1), "the store has not answered yet");
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 0,
            behind: Behind::Unknown,
        };
        w.store_seen(0, "demo", up);
        assert!(live.may_move(&w, 1));
        live.slots[1] = Slot::Running(Box::new(Stub));
        live.turns[1] = 1;
        live.onto_lonca(&mut w, 1);
        assert!(matches!(live.slots[1], Slot::Running(_)), "mid-turn");
        live.turns[1] = 0;
        // Not when its change waits, it started at a point of its own, or
        // a helper of it works.
        w.change_ready(Change {
            agent: 1,
            title: "t".into(),
            files: Vec::new(),
            patch: "p".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        assert!(!live.may_move(&w, 1), "a change waits");
        w.changes.retain(|c| c.agent != 1);
        assert!(live.may_move(&w, 1));
        w.agents[1].start = Some("v1".into());
        assert!(!live.may_move(&w, 1), "its own start");
        w.agents[1].start = None;
        let helper = with_helper(&mut w, 1);
        live.grow(None, false);
        live.slots[helper] = Slot::Starting(Vec::new());
        assert!(!live.may_move(&w, 1), "its helper works");
        live.slots[helper] = Slot::None;
        assert!(!live.may_move(&w, helper), "a helper never");
        assert!(!live.may_move(&w, 0), "nor the leader");
        live.onto_lonca(&mut w, 1);
        assert!(matches!(live.slots[1], Slot::None), "ended between turns");
        assert!(live.remembered[1], "goes on in its conversation");
        // The start that comes: moved, journaled, told.
        live.moved(&mut w, 1, Moved::Onto);
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::OntoLonca { agent: 1, repo } if repo == "demo"
        )));
        assert!(live.notes[1].iter().any(|n| n == ONTO_LONCA));
        // Holding work: off Lonca, and no prompt ends its session for it
        // again until its change is in.
        live.moved(
            &mut w,
            1,
            Moved::Kept(worktree::Held {
                changes: 2,
                commits: 1,
            }),
        );
        assert!(w.journal.iter().any(|e| matches!(
            e.event,
            Event::OffLonca {
                agent: 1,
                changes: 2,
                commits: 1
            }
        )));
        assert!(!live.may_move(&w, 1));
        // Off Lonca, nothing moves.
        live.off_lonca[1] = false;
        w.projects[0].lonca = None;
        assert!(!live.may_move(&w, 1));
    }

    #[test]
    fn a_harness_is_renewed_between_turns_only_and_told_so() {
        let renewed = |live: &Live| {
            let told = live.notes[1].iter().any(|n| n == SEAT_RENEWED);
            assert_eq!(told, live.remembered[1]);
            told
        };
        let (mut w, mut live, now) = seated_with(Duration::ZERO);
        live.slots[1] = Slot::Starting(Vec::new());
        live.renew_seat(&mut w, 1, now);
        assert!(!renewed(&live), "no harness yet");

        live.slots[1] = Slot::Running(Box::new(Stub));
        live.turns[1] = 1;
        live.renew_seat(&mut w, 1, now);
        assert!(!renewed(&live), "mid-turn");

        live.turns[1] = 0;
        w.agents[1].workspace = None;
        live.renew_seat(&mut w, 1, now);
        assert!(!renewed(&live), "in a worktree");
        assert!(matches!(live.slots[1], Slot::Running(_)));

        w.agents[1].workspace = Some("demo".into());
        let far = now - Duration::from_secs(3600);
        live.renew_seat(&mut w, 1, far);
        assert!(!renewed(&live), "time left");

        live.renew_seat(&mut w, 1, now);
        assert!(renewed(&live));
        assert!(matches!(live.slots[1], Slot::None), "the harness ended");
        let traced = w.agents[1].trace.iter().any(|l| l.contains("running out"));
        assert!(traced);
    }

    #[test]
    fn a_look_brings_the_hooks_lonca_did_not_hear_and_when_it_was_asked() {
        let d = Daemon::start(PROTOCOL, |_| {
            let mut v = status(&[]);
            v["sessions"] = serde_json::json!([
                {"agent": "a", "expires_ms": 3000, "bound_ms": 1, "unhooked": {"tool": "lease"}},
                {"agent": "b", "expires_ms": 3000, "bound_ms": 1},
            ]);
            Ok(v)
        });
        d.sign_in();
        let before = Instant::now();
        let repos = vec![(0, "demo".to_string(), None)];
        let looked = look(&d.place, &repos, &[], &mut Kept::default());
        let missed = vec![("a".to_string(), Hooks::Missed("lease".into()))];
        assert_eq!(looked.hooks, vec![("demo".to_string(), missed)]);
        assert!(looked.asked.is_some_and(|at| at >= before));
    }

    #[test]
    fn what_a_look_heard_replaces_what_the_last_look_of_that_repository_said() {
        let (_, mut live, _) = seated_with(Duration::ZERO);
        let key = |r: &str, a: &str| (r.to_string(), a.to_string());
        let unheard = |agents: &[(&str, Hooks)]| {
            (agents.iter())
                .map(|(a, h)| (a.to_string(), h.clone()))
                .collect::<Vec<_>>()
        };
        live.lonca.note_hooks(vec![
            (
                "r".into(),
                unheard(&[("a", Hooks::Unbound), ("b", Hooks::Unbound)]),
            ),
            ("s".into(), unheard(&[("a", Hooks::Unbound)])),
        ]);
        assert_eq!(live.lonca.unheard.len(), 3);
        // `r` heard everyone now; `s` was not read and keeps its word.
        live.lonca.note_hooks(vec![(
            "r".into(),
            unheard(&[("b", Hooks::Missed("x".into()))]),
        )]);
        let mut kept: Vec<_> = live.lonca.unheard.keys().cloned().collect();
        kept.sort();
        assert_eq!(kept, [key("r", "b"), key("s", "a")]);
        assert_eq!(
            live.lonca.unheard[&key("r", "b")],
            Hooks::Missed("x".into())
        );
    }

    /// [`seated_with`]'s worker running, its harness started at the moment
    /// returned, and Lonca having said `hooks` of its session.
    fn unheard_with(hooks: Hooks) -> (World, Live, Instant) {
        let (w, mut live, now) = seated_with(Duration::from_secs(3600));
        live.slots[1] = Slot::Running(Box::new(Stub));
        live.lonca.started.insert(1, now);
        let key = ("demo".to_string(), w.agents[1].name.clone());
        live.lonca.unheard.insert(key, hooks);
        (w, live, now)
    }

    #[test]
    fn a_worker_whose_hooks_lonca_did_not_hear_stops_with_its_helpers() {
        let (mut w, mut live, started) = unheard_with(Hooks::Missed("lease".into()));
        let helper = with_helper(&mut w, 1);
        live.grow(None, false);
        live.slots[helper] = Slot::Running(Box::new(Stub));
        assert!(
            live.stop_unheard(&mut w, started),
            "a look asked as it started"
        );
        assert!(matches!(live.slots[1], Slot::None), "its harness ended");
        let a = &w.agents[1];
        assert_eq!(a.status, Status::Stopped);
        assert!(
            a.topic.starts_with("Lonca saw a call to its lease tool"),
            "{}",
            a.topic
        );
        assert!(live.remembered[1], "it goes on where it left off");
        assert_eq!(live.notes[1], [SEAT_UNHOOKED]);
        assert!(matches!(live.slots[helper], Slot::None));
        assert!(w.agents[helper].gone, "its helper leaves");
        // Stopped, it is not judged again.
        assert!(!live.stop_unheard(&mut w, started));
        let (mut w, mut live, started) = unheard_with(Hooks::Unbound);
        assert!(live.stop_unheard(&mut w, started + BIND_GRACE));
        let topic = &w.agents[1].topic;
        assert!(
            topic.starts_with("Lonca never heard from its hooks"),
            "{topic}"
        );
    }

    #[test]
    fn a_look_that_finds_a_workers_hooks_unheard_stops_it() {
        let d = Daemon::start(PROTOCOL, |_| {
            let mut v = status(&[]);
            v["sessions"] = serde_json::json!([{"agent": "claude-1", "expires_ms": 1_000_000,
                "bound_ms": 1, "unhooked": {"tool": "propose", "seen_ms": 2}}]);
            Ok(v)
        });
        d.sign_in();
        let (mut w, mut live, _) = unheard_with(Hooks::Heard);
        live.lonca = Watch::start(d.place.clone());
        live.lonca.started.insert(1, Instant::now());
        assert_eq!(w.agents[1].name, "claude-1");
        live.lonca_due(&w, Instant::now());
        assert!(settled(&mut live, &mut w), "the screen changed");
        assert_eq!(w.agents[1].status, Status::Stopped);
        let key = ("demo".to_string(), "claude-1".to_string());
        assert_eq!(live.lonca.unheard[&key], Hooks::Missed("propose".into()));
    }

    #[test]
    fn a_harness_start_is_kept_for_the_hooks_to_be_judged_by() {
        let (mut w, mut live, _) = seated_with(Duration::ZERO);
        live.slots[1] = Slot::Running(Box::new(Stub));
        let before = Instant::now();
        let started = AgentEvent::Started {
            session: "7d1c3f2e-5b6a-4c8d-9e0f-1a2b3c4d5e6f".into(),
            model: "m".into(),
        };
        live.tx.send((1, 0, started)).unwrap();
        live.step(&mut w);
        assert!(live.lonca.started.get(&1).is_some_and(|t| *t >= before));
    }

    #[test]
    fn only_a_harness_there_when_the_look_was_asked_is_judged() {
        let stops = |hooks: Hooks, asked: fn(Instant) -> Instant| {
            let (mut w, mut live, started) = unheard_with(hooks);
            let stopped = live.stop_unheard(&mut w, asked(started));
            assert_eq!(stopped, w.agents[1].status == Status::Stopped);
            stopped
        };
        let missed = || Hooks::Missed("status".into());
        assert!(
            !stops(missed(), |t| t - Duration::from_millis(1)),
            "started after"
        );
        assert!(stops(missed(), |t| t));
        // An unbound session may still be binding: ten seconds, in numbers.
        let early = |t| t + Duration::from_millis(9_999);
        assert!(!stops(Hooks::Unbound, early), "within the grace");
        assert!(stops(Hooks::Unbound, |t| t + Duration::from_secs(10)));
        assert!(
            !stops(Hooks::Heard, |t| t + Duration::from_secs(60)),
            "heard"
        );
    }

    #[test]
    fn a_worker_is_judged_only_with_a_running_harness_in_a_workspace() {
        let (mut w, mut live, started) = unheard_with(Hooks::Unbound);
        let late = started + Duration::from_secs(60);
        live.slots[1] = Slot::Starting(Vec::new());
        assert!(!live.stop_unheard(&mut w, late), "not started");
        live.slots[1] = Slot::Running(Box::new(Stub));
        w.agents[1].workspace = None;
        assert!(!live.stop_unheard(&mut w, late), "in a worktree");
        w.agents[1].workspace = Some("demo".into());
        live.lonca.started.clear();
        assert!(!live.stop_unheard(&mut w, late), "no start seen");
        live.lonca.started.insert(1, started);
        w.agents[1].name = "another".into();
        assert!(!live.stop_unheard(&mut w, late), "not this agent's session");
    }

    #[test]
    fn a_helper_answers_to_its_workers_session_never_its_own() {
        let (mut w, mut live, started) = unheard_with(Hooks::Unbound);
        let helper = with_helper(&mut w, 1);
        live.grow(None, false);
        w.agents[helper].workspace = Some("demo".into());
        w.agents[helper].name = w.agents[1].name.clone();
        assert!(live.unheard(&w, 1).is_some());
        assert_eq!(live.unheard(&w, helper), None, "a helper never");
        w.projects[0].lonca = None;
        assert_eq!(live.unheard(&w, 1), None, "off Lonca");
        let _ = started;
    }

    #[test]
    fn what_a_look_read_of_a_session_is_a_moment() {
        let (_, mut live, now) = seated_with(Duration::ZERO);
        let left = vec![
            ("r".to_string(), "a".to_string(), 1500),
            ("r".to_string(), "b".to_string(), -7),
        ];
        live.lonca.note_ends(left, now);
        let end = |who: &str| live.lonca.ends[&("r".to_string(), who.to_string())];
        assert_eq!(end("a"), now + Duration::from_millis(1500));
        assert_eq!(end("b"), now);
    }

    #[test]
    fn a_look_brings_how_long_sessions_have_left() {
        let d = Daemon::start(PROTOCOL, |_| {
            let mut v = status(&[]);
            v["now_ms"] = serde_json::json!(1000);
            v["sessions"] = serde_json::json!([{"agent": "a", "expires_ms": 3000}]);
            Ok(v)
        });
        d.sign_in();
        let left = look(
            &d.place,
            &vec![(0, "demo".to_string(), None)],
            &[],
            &mut Kept::default(),
        )
        .left;
        assert_eq!(left, vec![("demo".to_string(), "a".to_string(), 2000)]);
    }

    #[test]
    fn a_workspace_whose_session_is_over_goes_on_under_a_new_one() {
        let (place, tree, scratch, dir) = seat_at(
            r#"case "$*" in
               *heartbeat*) echo 'lonca: session_inactive: over' >&2; exit 1;;
               esac
               echo '{}'"#,
        );
        make_workspace(&tree, "demo");
        let own = Some(scratch.as_path());
        let asked = seat(&place, Some("demo"), &tree, "claude-1", own, false);
        assert_eq!(asked, Ok(Some(false)));
        let told = ran(&dir).unwrap();
        let at = format!(
            "--continue {} --scratch {}",
            tree.display(),
            scratch.display()
        );
        assert!(told.contains(&at), "{told}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_of_nothing_or_of_another_repository_stops_the_start() {
        let (place, tree, _, dir) = seat_at("exit 0");
        make_workspace(&tree, "demo");
        let e = seat(&place, None, &tree, "claude-1", None, false).unwrap_err();
        assert!(e.contains("on no repository: name it with :lonca"), "{e}");
        let e = seat(&place, Some("other"), &tree, "claude-1", None, false).unwrap_err();
        assert!(e.contains("is no workspace of other"), "{e}");
        assert_eq!(ran(&dir), None, "nobody was asked");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_daemon_that_can_not_say_stops_the_start_in_its_words() {
        let (place, tree, _, dir) = seat_at("echo 'lonca: cannot reach it' >&2; exit 1");
        let e = seat(&place, Some("demo"), &tree, "claude-1", None, false).unwrap_err();
        assert_eq!(e, "its Lonca session: cannot reach it");
        make_workspace(&tree, "demo");
        let e = seat(&place, Some("demo"), &tree, "claude-1", None, false).unwrap_err();
        assert_eq!(e, "its Lonca session: cannot reach it");
        let mut none = place.clone();
        none.program = dir.join("no-such-program").display().to_string();
        let e = seat(&none, Some("demo"), &tree, "claude-1", None, false).unwrap_err();
        assert_eq!(e, "its Lonca session: lonca did not answer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_taken_again_that_is_refused_stops_the_start() {
        let (place, tree, _, dir) = seat_at(
            r#"case "$*" in
               *heartbeat*) echo 'lonca: session_inactive: over' >&2; exit 1;;
               esac
               echo 'lonca: denied: scope_widening:paths' >&2; exit 1"#,
        );
        make_workspace(&tree, "demo");
        let e = seat(&place, Some("demo"), &tree, "claude-1", None, false).unwrap_err();
        assert_eq!(e, "its Lonca session: denied: scope_widening:paths");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_project_with_git_is_asked_how_far_behind_it_is() {
        let repo = crate::live::testing::checkout("behind-git");
        let d = distant(serde_json::json!(2));
        let mut live = idle_live_in(&repo, 1);
        live.lonca = Watch::start(d.place.clone());
        let mut w = team(0);
        w.on_lonca(0, Some("demo".into()));
        let seen_behind = |w: &World| match seen_of(w, 0) {
            StoreSeen::Up { behind, .. } => behind,
            other => panic!("{other:?}"),
        };
        live.places[0].git = true;
        live.lonca_due(&w, Instant::now());
        settle(&mut live, &mut w);
        assert_eq!(seen_behind(&w), Behind::By(2));
        live.places[0].git = false;
        live.lonca.next = None;
        live.lonca_due(&w, Instant::now());
        settle(&mut live, &mut w);
        assert_eq!(seen_behind(&w), Behind::Unknown);
        std::fs::remove_dir_all(&repo).unwrap();
    }

    /// `:sync`'s run against `checkout`, trunk being what `to` names there.
    fn synced(checkout: &Path, to: &str) -> Result<Synced, Failed> {
        let body = format!("git update-ref refs/lonca/trunk {to}");
        let (place, dir) = imece_agent::lonca_fake::program(&body);
        let done = sync(&place, "demo", checkout);
        std::fs::remove_dir_all(dir).unwrap();
        done
    }

    #[test]
    fn sync_fetches_then_moves_the_branch_forward_and_says_how_it_stood() {
        let repo = crate::live::testing::checkout("sync");
        let was = git(&repo, &["symbolic-ref", "--short", "HEAD"]);
        let start = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(synced(&repo, "HEAD"), Ok(Synced::Even));
        git(&repo, &["checkout", "--quiet", "-b", "later"]);
        let tip = commit(&repo, "later.txt");
        git(&repo, &["checkout", "--quiet", &was]);
        assert_eq!(synced(&repo, &tip), Ok(Synced::Moved));
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), tip);
        git(&repo, &["reset", "--quiet", "--hard", &start]);
        let own = commit(&repo, "own.txt");
        assert_eq!(synced(&repo, &tip), Ok(Synced::Apart));
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), own, "nothing moved");
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn a_refused_fetch_moves_nothing_and_work_in_the_way_is_said() {
        let repo = crate::live::testing::checkout("sync-refused");
        let head = git(&repo, &["rev-parse", "HEAD"]);
        let (place, dir) = imece_agent::lonca_fake::program("echo 'lonca: redacted' >&2\nexit 1");
        assert_eq!(
            sync(&place, "demo", &repo),
            Err(Failed::Said("redacted".into()))
        );
        std::fs::remove_dir_all(dir).unwrap();
        // No such commit to go to: git's refusal comes back as words.
        let (place, dir) = imece_agent::lonca_fake::program("exit 0");
        let Err(Failed::Said(why)) = sync(&place, "demo", &repo) else {
            panic!("no trunk ref, so no answer");
        };
        assert!(why.contains("rev-parse"), "{why}");
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn a_sync_comes_back_as_a_line_and_a_failed_one_as_its_failure() {
        let repo = crate::live::testing::checkout("sync-job");
        let (place, dir) = imece_agent::lonca_fake::program("git update-ref refs/lonca/trunk HEAD");
        let mut live = idle_live_in(&repo, 1);
        live.lonca = Watch::start(place);
        let mut w = team(0);
        let job = StoreJob::Sync {
            project: 0,
            repo: "demo".into(),
        };
        assert!(live.store_job(job.clone()));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreSynced { leader: 0, repo, how: Synced::Even } if repo == "demo"
        ));
        let mut gone = live.lonca.place.clone();
        gone.program = dir.join("nothing").display().to_string();
        live.lonca = Watch::start(gone);
        assert!(live.store_job(job));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreFailed {
                act: StoreAct::Sync,
                why: None,
                ..
            }
        ));
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn a_login_runs_in_the_checkout_one_at_a_time_and_is_told() {
        let (mut live, dir) = live_with("printf '{\"human\":\"ayse\"}\\n'");
        let mut w = team(0);
        live.lonca_after_a_look();
        assert!(!live.store_busy() && !live.asking());
        assert!(!live.take_lonca(&mut w), "nothing under way");
        assert!(live.store_job(StoreJob::Login(0)));
        assert!(live.store_busy() && live.asking());
        assert!(!live.store_job(StoreJob::Login(0)), "one at a time");
        job_ends(&mut live, &mut w);
        assert!(!live.store_busy() && !live.asking());
        assert!(live.lonca_is_due(), "asked at once");
        assert!(matches!(
            last(&w),
            Event::StoreSignedIn { leader: 0, human } if human == "ayse"
        ));
        let told = imece_agent::lonca_fake::seen(&dir);
        assert!(
            told.starts_with(&format!("{}|human login", dir.display())),
            "{told}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn ask_for(checkout: std::path::PathBuf) -> ImportAsk {
        ImportAsk {
            project: 0,
            repo: "demo".into(),
            checkout,
            branch: "main".into(),
            head: "1".repeat(40),
        }
    }

    #[test]
    fn an_import_puts_the_project_on_its_repository_with_what_lonca_warned() {
        let (mut live, dir) = live_with("echo 'warning: big' >&2");
        let mut w = team(0);
        assert!(live.store_job(StoreJob::Import(ask_for(dir.clone()))));
        job_ends(&mut live, &mut w);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "demo");
        assert_eq!(seen_of(&w, 0), StoreSeen::Asking);
        assert!(w.journal.iter().any(|line| matches!(
            &line.event,
            Event::StoreImported { repo, warnings, .. }
                if repo == "demo" && warnings == &["big".to_string()]
        )));
        assert!(!live.store_busy());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_import_that_can_not_open_critical_paths_says_so() {
        // No daemon behind the fake program: nothing reads the policy.
        let (mut live, dir) = live_with("exit 0");
        let mut w = team(0);
        assert!(live.store_job(StoreJob::Import(ask_for(dir.clone()))));
        job_ends(&mut live, &mut w);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "demo");
        assert!(matches!(
            last(&w),
            Event::StoreCriticalKept { repo, why: None, .. } if repo == "demo"
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_first_import_signs_the_person_in_and_a_later_one_does_not() {
        let (fake, dir) = imece_agent::lonca_fake::program("exit 0");
        let d = up_daemon();
        let place = Place {
            program: fake.program,
            ..d.place.clone()
        };
        let job = StoreJob::Import(ask_for(dir.clone()));
        let last_run = || imece_agent::lonca_fake::seen(&dir);
        assert!(matches!(run(&place, &dir, &job), Done::Import(Ok(_), _)));
        assert!(last_run().contains("|init --import"), "{}", last_run());
        std::fs::remove_file(place.humans.join("ayse.session")).unwrap();
        assert!(matches!(run(&place, &dir, &job), Done::Import(Ok(_), _)));
        let login = last_run();
        assert!(login.contains("|human login --json|"), "{login}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_import_signs_nobody_in() {
        let (fake, dir) = imece_agent::lonca_fake::program("exit 1");
        let d = up_daemon();
        let place = Place {
            program: fake.program,
            ..d.place.clone()
        };
        let job = StoreJob::Import(ask_for(dir.clone()));
        assert!(matches!(
            run(&place, &dir, &job),
            Done::Import(Err(_), None)
        ));
        let told = imece_agent::lonca_fake::seen(&dir);
        assert!(told.contains("|init --import"), "{told}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_import_is_told_and_leaves_the_project_off_lonca() {
        let (mut live, dir) = live_with("echo 'lonca: conflict: it exists' >&2\nexit 1");
        let mut w = team(0);
        live.lonca_after_a_look();
        assert!(live.store_job(StoreJob::Import(ask_for(dir.clone()))));
        job_ends(&mut live, &mut w);
        assert!(w.projects[0].lonca.is_none());
        assert!(live.lonca_is_due());
        assert!(matches!(
            last(&w),
            Event::StoreFailed { act: StoreAct::Import, why: Some(why), .. }
                if why == "conflict: it exists"
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_login_that_fails_says_what_it_said_and_no_program_says_nothing() {
        let (mut live, dir) = live_with("echo 'lonca: no key' >&2\nexit 1");
        let mut w = team(0);
        assert!(live.store_job(StoreJob::Login(0)));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreFailed { act: StoreAct::Login, why: Some(why), .. } if why == "no key"
        ));
        let mut place = live.lonca.place.clone();
        place.program = dir.join("nothing").display().to_string();
        live.lonca = Watch::start(place);
        assert!(live.store_job(StoreJob::Login(0)));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreFailed {
                act: StoreAct::Login,
                why: None,
                ..
            }
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_job_whose_thread_died_is_a_failed_one() {
        let (mut live, dir) = live_with("exit 0");
        let mut w = team(0);
        let (tell, told) = mpsc::channel();
        live.lonca.job = Some((StoreJob::Login(0), told));
        assert!(!live.take_lonca(&mut w), "still under way");
        drop(tell);
        assert!(live.take_lonca(&mut w));
        assert!(matches!(last(&w), Event::StoreFailed { why: None, .. }));
        assert!(!live.store_busy());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_import_is_asked_of_a_branch_and_never_of_a_detached_head() {
        let repo = crate::live::testing::checkout("import-ask");
        let live = idle_live_in(&repo, 1);
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["checkout", "--quiet", "-b", "trunk-to-be"]);
        let ask = live.import_ask(0, "demo").expect("on a branch");
        assert_eq!(ask.project, 0);
        assert_eq!(ask.repo, "demo");
        assert_eq!(ask.checkout, repo);
        assert_eq!(ask.branch, "trunk-to-be");
        assert_eq!(ask.head, git(&["rev-parse", "HEAD"]));
        git(&["checkout", "--quiet", "--detach"]);
        assert_eq!(live.import_ask(0, "demo"), None);
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn every_reason_keeps_its_word() {
        for (down, word) in [
            (Down::NoDaemon, StoreDown::NoDaemon),
            (Down::NotTheDaemon, StoreDown::NotTheDaemon),
            (Down::Protocol, StoreDown::Protocol),
            (Down::Old, StoreDown::Old),
            (Down::Hardened, StoreDown::Hardened),
            (Down::SignedOut, StoreDown::SignedOut),
            (Down::Forbidden, StoreDown::Forbidden),
            (Down::NoRepo, StoreDown::NoRepo),
            (Down::Refused, StoreDown::Refused),
            (Down::Again, StoreDown::Busy),
        ] {
            assert_eq!(down_of(down), word);
        }
    }

    #[test]
    fn the_store_is_asked_at_its_pace_and_never_twice_at_once() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        let t0 = Instant::now();
        // Nothing on Lonca: nothing is asked, and it stays due.
        live.lonca_due(&w, t0);
        assert!(!live.lonca.out);
        assert_eq!(live.lonca.next, None);

        w.on_lonca(0, Some("demo".into()));
        live.lonca_due(&w, t0);
        assert!(live.lonca.out);
        assert_eq!(live.lonca.next, Some(t0 + LOOK_EVERY));
        // Under way: no second look, even when it is time.
        live.lonca_due(&w, t0 + LOOK_EVERY);
        assert_eq!(live.lonca.next, Some(t0 + LOOK_EVERY));
        settle(&mut live, &mut w);
        assert_eq!(
            seen_of(&w, 0),
            StoreSeen::Up {
                trunk: TRUNK.into(),
                pending: 2,
                behind: Behind::Unknown,
            }
        );
        // Before its time, none; at its time, one.
        live.lonca_due(&w, t0 + LOOK_EVERY - Duration::from_millis(1));
        assert!(!live.lonca.out);
        live.lonca_due(&w, t0 + LOOK_EVERY);
        assert!(live.lonca.out);
        settle(&mut live, &mut w);
        let looks = |d: &Daemon| {
            let heard = d.heard.lock().unwrap();
            heard.iter().filter(|h| h["method"] == "status").count()
        };
        assert_eq!(looks(&d), 2);
        // The person named a repository: at once.
        live.look_now();
        live.lonca_due(&w, t0 + LOOK_EVERY + Duration::from_millis(1));
        assert!(live.lonca.out);
        settle(&mut live, &mut w);
    }

    #[test]
    fn what_a_look_found_says_whether_the_screen_changed() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        assert!(!live.take_lonca(&mut w), "nothing out");
        w.on_lonca(0, Some("demo".into()));
        let ask = |live: &mut Live, w: &World| {
            live.look_now();
            live.lonca_due(w, Instant::now());
        };
        let wait = |live: &mut Live, w: &mut World| {
            let till = Instant::now() + WAIT;
            loop {
                let changed = live.take_lonca(w);
                if !live.lonca.out {
                    return changed;
                }
                assert!(Instant::now() < till, "no answer");
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        ask(&mut live, &w);
        assert!(wait(&mut live, &mut w), "the first answer");
        ask(&mut live, &w);
        assert!(!wait(&mut live, &mut w), "the same again");
    }

    #[test]
    fn a_look_that_changed_any_project_changed_the_screen() {
        let d = up_daemon();
        let mut w = team(0);
        let second = w.open_project(None, "b".into(), "/b".into(), crate::model::Role::Leader);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        w.on_lonca(0, Some("demo".into()));
        w.on_lonca(second, Some("demo".into()));
        live.lonca_due(&w, Instant::now());
        let till = Instant::now() + WAIT;
        let mut changed = false;
        while live.lonca.out {
            changed |= live.take_lonca(&mut w);
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(changed, "both changed, not one undoing the other");
        assert_eq!(seen_of(&w, second), seen_of(&w, 0));
    }

    #[test]
    fn a_step_asks_and_the_step_that_brings_the_answer_says_something_came() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        w.on_lonca(0, Some("demo".into()));
        live.step(&mut w);
        assert!(live.lonca.out, "the first step asks");
        let till = Instant::now() + WAIT;
        loop {
            let came = live.step(&mut w);
            if w.store_closed(0).is_none() {
                assert!(came, "the answer was not said to have come");
                break;
            }
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_project_on_a_store_that_does_not_answer_hires_nobody() {
        let mut w = team(1);
        let live = idle_live(2);
        assert!(live.hire_room(&w, 0).is_ok());
        w.on_lonca(0, Some("demo".into()));
        let said = live.hire_room(&w, 0).unwrap_err();
        assert!(said.contains("imece is still asking Lonca"), "{said}");
        w.store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        let said = live.hire_room(&w, 0).unwrap_err();
        assert_eq!(
            said,
            "nobody is hired now: no Lonca daemon answers; only the person can bring Lonca back"
        );
        // A worker's helpers too, and `whoami` says what `hire` would.
        assert!(live.hire_room(&w, 1).is_err());
        assert!(
            live.whoami(&w, 0)
                .contains("may hire: no: nobody is hired now")
        );
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 0,
            behind: Behind::By(0),
        };
        w.store_seen(0, "demo", up);
        assert!(live.hire_room(&w, 0).is_ok());
    }

    #[test]
    fn a_project_on_a_store_that_does_not_answer_decides_no_change_and_forks_nobody() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.on_lonca(0, Some("demo".into()));
        w.store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        let why = Err(StoreDown::NoDaemon.why().to_string());
        assert_eq!(live.apply_change(&w, 1, Vec::new()), why);
        assert_eq!(live.reject_change(&mut w, 1), why);
        assert_eq!(
            live.fork(&mut w, 1).unwrap_err(),
            "nobody is hired now: no Lonca daemon answers; only the person can bring Lonca back"
        );
    }

    /// A `status` with `rows` as its pending list: (seq, agent, change).
    fn waiting(rows: &[(u64, Option<&str>, Option<&str>)]) -> lonca::Status {
        lonca::Status {
            trunk: TRUNK.into(),
            policy: POLICY.into(),
            policy_checks: None,
            frozen: None,
            leases: Vec::new(),
            next_seq: 9,
            pending: rows
                .iter()
                .map(|&(seq, agent, change)| lonca::Pending {
                    seq,
                    created_ms: 5,
                    op: lonca::PROPOSE.into(),
                    reasons: vec!["unobserved_session".into()],
                    change: change.map(str::to_string),
                    base: Some(TRUNK.into()),
                    intent: Some(format!("intent {seq}")),
                    agent: agent.map(str::to_string),
                    task: None,
                    paths: Vec::new(),
                    runs: None,
                })
                .collect(),
            changes: Vec::new(),
            sessions: Vec::new(),
            now_ms: 0,
        }
    }

    fn diff_of(patch: &str) -> Arc<Diff> {
        Arc::new(Diff {
            commit: "c1".into(),
            files: vec![
                lonca::DiffFile {
                    path: "a.rs".into(),
                    lines: Some((3, 1)),
                },
                lonca::DiffFile {
                    path: "b.png".into(),
                    lines: None,
                },
            ],
            redacted: vec!["s.txt".into()],
            patch: patch.to_string(),
            cut: true,
        })
    }

    fn offer(agent: &str, seq: u64, change: &str) -> Offer {
        Offer {
            agent: agent.to_string(),
            title: format!("intent {seq}"),
            proposal: StoreProposal {
                seq,
                change: change.to_string(),
                why: vec!["unobserved_session".into()],
                redacted: Vec::new(),
                cut: false,
                verifies: Runs::default(),
                on_trunk: true,
                intent: format!("intent {seq}"),
            },
            diff: diff_of("diff --git a/a.rs b/a.rs\n"),
        }
    }

    #[test]
    fn a_workers_newest_proposal_is_its_offer_and_holds_the_older() {
        let mut status = waiting(&[
            (5, Some("w1"), Some("c5")),
            (9, Some("w1"), Some("c9")),
            (7, Some("w1"), Some("c7")),
            (6, Some("w2"), Some("c6")),
        ]);
        status.pending[1].base = Some("0ther".into());
        status.pending[1].runs = Some(vec!["build".into(), "api".into()]);
        status.pending[0].runs = Some(vec!["older".into()]);
        let mut asked = Vec::new();
        let got = offers(&status, &mut |change: &str| {
            asked.push(change.to_string());
            Some(diff_of("p"))
        });
        assert_eq!(asked, ["c9", "c6"], "only what is shown is read");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].agent, "w1");
        assert_eq!(got[0].title, "intent 9");
        let want = StoreProposal {
            seq: 9,
            change: "c9".into(),
            why: vec!["unobserved_session".into()],
            redacted: vec!["s.txt".into()],
            cut: true,
            verifies: Runs::These(vec!["build".into(), "api".into()]),
            on_trunk: false,
            intent: "intent 9".into(),
        };
        assert_eq!(got[0].proposal, want);
        assert_eq!(got[1].proposal.verifies, Runs::Unsaid, "Lonca said nothing");
        assert_eq!(got[1].agent, "w2");
        assert!(got[1].proposal.on_trunk, "based on trunk as it stands");
    }

    #[test]
    fn what_is_no_workers_proposal_is_no_offer() {
        let mut status = waiting(&[
            (5, None, Some("human")),
            (6, Some("w1"), None),
            (7, Some("w2"), Some("c7")),
            (8, Some("w3"), Some("unread")),
        ]);
        status.pending[2].op = "policy.update".into();
        let got = offers(&status, &mut |change: &str| {
            (change != "unread").then(|| diff_of("p"))
        });
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn a_proposal_reads_as_a_change_with_its_binary_file_said() {
        let change = change_of(3, &offer("w1", 5, "c5"), None);
        assert_eq!(change.agent, 3);
        assert_eq!(change.title, "intent 5");
        // A worker's card names it; the intent is its text then.
        let titled = change_of(3, &offer("w1", 5, "c5"), Some("Settle the ledger"));
        assert_eq!(titled.title, "Settle the ledger");
        assert_eq!(change.patch, "diff --git a/a.rs b/a.rs\n");
        let (text, binary) = (&change.files[0], &change.files[1]);
        assert_eq!(
            (text.path.as_str(), text.added, text.removed),
            ("a.rs", 3, 1)
        );
        assert!(!text.binary);
        assert_eq!((binary.added, binary.removed), (0, 0));
        assert!(binary.binary);
        assert!(change.runs.is_empty() && change.spend.is_none());
        assert_eq!(change.check, Check::None);
    }

    #[test]
    fn what_became_of_a_proposal_is_what_the_store_says() {
        let c = || vec!["x".to_string()];
        assert_eq!(landed(&Standing::Pending), None);
        assert_eq!(landed(&Standing::Other), None);
        assert_eq!(landed(&Standing::Queued), Some(Landed::Queued));
        assert_eq!(landed(&Standing::Withdrawn), Some(Landed::Withdrawn));
        assert_eq!(
            landed(&Standing::Integrated("c".into())),
            Some(Landed::Integrated("c".into()))
        );
        assert_eq!(
            landed(&Standing::Conflicted(c())),
            Some(Landed::Conflicted(c()))
        );
        assert_eq!(
            landed(&Standing::Rejected(c())),
            Some(Landed::Rejected(c()))
        );
    }

    fn a_check(check: &str, ok: bool, lines: &[&str]) -> Attested {
        Attested {
            check: check.into(),
            ok,
            end: "exit 3, 1.0 s".into(),
            lines: lines.iter().map(|l| l.to_string()).collect(),
        }
    }

    fn evidence(result: &str, exit: Option<i64>, ms: Option<u64>, log: &str) -> lonca::Evidence {
        lonca::Evidence {
            check: "build".into(),
            result: result.into(),
            exit,
            duration_ms: ms,
            log: Some(log.into()),
        }
    }

    #[test]
    fn a_check_says_how_it_ended_and_a_failed_one_what_it_printed_last() {
        let log = "lonca verify: sandbox\n1\n2\n\n3\n4\n5\n6\n7\n";
        let got = attested(&[evidence("fail", Some(3), Some(1536), log)]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].check, "build");
        assert!(!got[0].ok);
        assert_eq!(got[0].end, "exit 3, 1.5 s");
        assert_eq!(got[0].lines, ["2", "3", "4", "5", "6", "7"]);
        // The verifier's header says nothing, in a short log too.
        let got = attested(&[evidence("error", None, None, "lonca verify: x\nboom\n")]);
        assert_eq!((got[0].ok, got[0].end.as_str()), (false, ""));
        assert_eq!(got[0].lines, ["boom"]);
        // A pass keeps no output, an exit with no time says the exit, a
        // skipped check is no check.
        let got = attested(&[
            evidence("pass", Some(0), None, "fine"),
            evidence("skipped", None, None, "x"),
        ]);
        assert_eq!(got.len(), 1);
        assert!(got[0].ok);
        assert_eq!(got[0].end, "exit 0");
        assert!(got[0].lines.is_empty());
        let none = lonca::Evidence {
            log: None,
            ..evidence("fail", None, None, "")
        };
        assert!(attested(&[none])[0].lines.is_empty());
    }

    #[test]
    fn only_a_watched_change_the_queue_decided_has_its_checks_read() {
        let mut v = status(&[]);
        v["changes"] = serde_json::json!([
            {"change": "on", "status": {"status": "integrated", "commit": "c"}},
            {"change": "no", "status": {"status": "rejected", "reasons": ["x"]}},
            {"change": "clash", "status": {"status": "conflicted", "symbols": []}},
            {"change": "wait", "status": {"status": "queued"}},
            {"change": "far", "status": {"status": "integrated", "commit": "c"}},
        ]);
        let d = Daemon::start(PROTOCOL, move |asked| {
            Ok(match asked["method"].as_str() {
                Some("status") => v.clone(),
                _ => serde_json::json!({"evidence": [
                    {"check": "tests", "result": "pass"}, {"check": "x", "result": "skipped"},
                ]}),
            })
        });
        d.sign_in();
        let watched = ["on", "no", "clash", "wait"].map(String::from);
        let repos = vec![(0, "demo".to_string(), None)];
        let found = look(&d.place, &repos, &watched, &mut Kept::default()).evidence;
        let mut keys: Vec<&String> = found.keys().collect();
        keys.sort();
        assert_eq!(keys, ["no", "on"]);
        assert_eq!(found["on"].len(), 1, "what ran");
        assert_eq!(found["on"][0].check, "tests");
    }

    #[test]
    fn what_lonca_says_runs_after_a_yes_reaches_the_offer() {
        let mut v = status(&[7]);
        v["policy"] = serde_json::json!("P");
        let d = Daemon::start(PROTOCOL, move |asked| {
            Ok(match asked["method"].as_str() {
                Some("status") => v.clone(),
                // `[verification]\ntest = "t"\n`: Lonca's word is what counts.
                Some("tree") => serde_json::json!([{
                    "path": "lonca.toml",
                    "content_b64": "W3ZlcmlmaWNhdGlvbl0KdGVzdCA9ICJ0Igo="
                }]),
                _ => serde_json::json!({"commit": "c", "diff": "diff --git a b\n", "files": [],
                    "truncated": false, "redacted": []}),
            })
        });
        d.sign_in();
        let repos = vec![(0, "demo".to_string(), None)];
        let offers = look(&d.place, &repos, &[], &mut Kept::default()).offers;
        assert_eq!(offers.len(), 1, "a proposal waits");
        let want = ["build", "tests", "api"].map(String::from).to_vec();
        assert_eq!(offers[0].1.proposal.verifies, Runs::These(want));
    }

    #[test]
    fn a_look_says_what_is_held_and_whose_it_is() {
        let (mut w, mut live, word) = proposer();
        let held = serde_json::json!([{"session": "s1", "items": [
            {"symbol": "src/a.rs#T.f", "kind": "edit"},
        ]}, {"session": "s2", "items": [{"symbol": "b.rs#g", "kind": "edit"}]}]);
        let sessions = serde_json::json!([
            {"id": "s1", "agent": word, "expires_ms": 9},
            {"id": "s2", "agent": "someone", "expires_ms": 9},
        ]);
        let d = Daemon::start(PROTOCOL, move |_| {
            let mut v = status(&[]);
            v["leases"] = held.clone();
            v["sessions"] = sessions.clone();
            Ok(v)
        });
        d.sign_in();
        live.lonca = Watch::start(d.place.clone());
        live.lonca_due(&w, Instant::now());
        settle(&mut live, &mut w);
        assert_eq!(
            w.projects[0].lonca.as_ref().unwrap().leases,
            [
                StoreLease {
                    file: "b.rs".into(),
                    symbol: "g".into(),
                    holder: None,
                    name: "someone".into(),
                    shared: false,
                },
                StoreLease {
                    file: "src/a.rs".into(),
                    symbol: "T.f".into(),
                    holder: Some(1),
                    name: word.clone(),
                    shared: false,
                },
            ]
        );
    }

    #[test]
    fn a_session_nobody_lists_holds_as_a_question_mark() {
        let w = team(1);
        let lease = lonca::Lease {
            file: "a".into(),
            symbol: "f".into(),
            agent: None,
            shared: true,
        };
        let shown = lease_of(&w, 0, lease);
        assert_eq!((shown.holder, shown.name.as_str()), (None, "?"));
        assert!(shown.shared, "kept");
    }

    #[test]
    fn a_failed_check_tells_the_worker_what_it_printed() {
        let no = Landed::Rejected(vec!["verification_failed:build".into()]);
        let checks = [
            a_check("syntax", true, &["not this"]),
            a_check("build", false, &["error[E0]", "here"]),
            a_check("tests", false, &[]),
        ];
        let (words, act) = told(&no, "T", &checks).unwrap();
        assert!(
            words.contains(
                "verification_failed:build. build (exit 3, 1.0 s):\nerror[E0]\nhere\n It"
            ),
            "{words}"
        );
        assert!(
            !words.contains("not this") && !words.contains("tests ("),
            "{words}"
        );
        assert!(act);
        // A failing check with nothing to show adds nothing.
        let (words, _) = told(&no, "T", &[a_check("tests", false, &[])]).unwrap();
        assert!(words.contains("build. It stays"), "{words}");
        // Two say both, one under the other, and it is cut.
        let two = [a_check("a", false, &["1"]), a_check("b", false, &["2"])];
        assert_eq!(
            failed_output(&two),
            " a (exit 3, 1.0 s):\n1\n b (exit 3, 1.0 s):\n2\n"
        );
        let long = "x".repeat(OUTPUT_TOLD + 1);
        let cut = failed_output(&[a_check("a", false, &[long.as_str()])]);
        assert_eq!(cut.chars().count(), 1000);
    }

    #[test]
    fn a_proposal_with_no_title_is_told_with_no_brackets() {
        for how in [
            Landed::Integrated("c1".into()),
            Landed::Conflicted(vec!["a#b".into()]),
            Landed::Rejected(vec!["no".into()]),
        ] {
            let (words, _) = told(&how, "", &[]).unwrap();
            assert!(!words.contains("()"), "{words}");
            let (words, _) = told(&how, "T", &[]).unwrap();
            assert!(words.contains("your proposal (T)") || words.contains("Your proposal (T)"));
        }
    }

    #[test]
    fn a_conflict_points_at_the_markers_sync_leaves() {
        let (words, _) = told(&Landed::Conflicted(vec!["a#b".into()]), "T", &[]).unwrap();
        assert!(words.contains("conflict markers"), "{words}");
        assert!(!words.contains(".trunk"), "{words}");
    }

    #[test]
    fn what_the_queue_checked_is_journaled_and_reaches_the_worker() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let no = Standing::Rejected(vec!["verification_failed:build".into()]);
        let checks = vec![a_check("build", false, &["broke"])];
        let evidence = HashMap::from([("c5".to_string(), checks.clone())]);
        assert!(live.land(&mut w, &standing("c5", no), &HashSet::new(), &evidence));
        assert!(
            matches!(last(&w), Event::Verified { agent: 1, ok: false, checks: c } if *c == checks)
        );
        assert!(live.notes[1][0].contains("broke"), "{:?}", live.notes[1]);
    }

    #[test]
    fn a_queued_proposal_says_its_checks_when_the_queue_ends() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let none = HashMap::new();
        assert!(live.land(
            &mut w,
            &standing("c5", Standing::Queued),
            &HashSet::new(),
            &none
        ));
        assert!(!matches!(last(&w), Event::Verified { .. }));
        let on = Standing::Integrated("c1".into());
        let checks = vec![a_check("tests", true, &[])];
        let evidence = HashMap::from([("c5".to_string(), checks)]);
        assert!(live.land(&mut w, &standing("c5", on), &HashSet::new(), &evidence));
        assert!(matches!(
            last(&w),
            Event::Verified {
                agent: 1,
                ok: true,
                ..
            }
        ));
    }

    #[test]
    fn a_look_watches_what_is_shown_and_what_is_queued() {
        let mut v = status(&[]);
        v["changes"] = serde_json::json!([
            {"change": "c5", "status": {"status": "integrated", "commit": "c"}},
            {"change": "c6", "status": {"status": "integrated", "commit": "c"}},
            {"change": "c7", "status": {"status": "integrated", "commit": "c"}},
        ]);
        let d = Daemon::start(PROTOCOL, move |asked| {
            Ok(match asked["method"].as_str() {
                Some("status") => v.clone(),
                _ => serde_json::json!({"evidence": [{"check": "tests", "result": "pass"}]}),
            })
        });
        d.sign_in();
        let (mut w, mut live, word) = proposer();
        live.lonca = Watch::start(d.place.clone());
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        live.lonca.queued.insert("c6".into(), 1);
        live.lonca_due(&w, Instant::now());
        settle(&mut live, &mut w);
        let got: Vec<&Event> = (w.journal.iter()).map(|l| &l.event).collect();
        let verified = got.iter().filter(|e| matches!(e, Event::Verified { .. }));
        assert_eq!(verified.count(), 2, "not c7");
    }

    #[test]
    fn a_worker_hears_what_it_must_act_on_and_is_woken_for_it() {
        assert_eq!(told(&Landed::Queued, "t", &[]), None);
        assert_eq!(told(&Landed::Withdrawn, "t", &[]), None);
        assert_eq!(told(&Landed::Replaced, "t", &[]), None);
        let (on_trunk, act) = told(&Landed::Integrated("c1".into()), "T", &[]).unwrap();
        assert!(
            on_trunk.contains("(T)") && on_trunk.contains("c1"),
            "{on_trunk}"
        );
        assert!(!act, "nothing to do until its next task");
        let (clash, act) = told(
            &Landed::Conflicted(vec!["a#b".into(), "c#d".into()]),
            "T",
            &[],
        )
        .unwrap();
        assert!(clash.contains("(T): a#b; c#d."), "{clash}");
        assert!(clash.contains("sync") && clash.contains("propose again"));
        assert!(act);
        let (no, act) = told(&Landed::Rejected(vec!["rejected: no".into()]), "T", &[]).unwrap();
        assert!(no.contains("(T) was refused: rejected: no."), "{no}");
        assert!(no.contains("sync") && no.contains("propose again"));
        assert!(act);
    }

    #[test]
    fn a_worker_refused_for_a_quota_or_a_freeze_is_told_to_say_it_is_blocked() {
        for why in [
            "verification_quota:day",
            "verification_quota:session",
            "frozen:audit",
        ] {
            let refused = Landed::Rejected(vec!["x".into(), why.into()]);
            let (said, act) = told(&refused, "T", &[]).unwrap();
            assert!(said.contains(&format!("refused: x; {why}.")), "{said}");
            assert!(said.contains("do not propose again"), "{said}");
            assert!(said.contains("blocked") && !said.contains("sync"), "{said}");
            assert!(act);
        }
        for why in ["verification_failed:build", "quota:day", "unfrozen:x"] {
            let (said, _) = told(&Landed::Rejected(vec![why.into()]), "T", &[]).unwrap();
            assert!(
                said.contains("propose again.") && said.contains("sync"),
                "{said}"
            );
        }
    }

    #[test]
    fn what_a_worker_is_told_of_a_reason_has_a_length() {
        let long = "x".repeat(REASON_TOLD);
        let (said, _) = told(&Landed::Rejected(vec![long.clone() + "y"]), "T", &[]).unwrap();
        assert!(
            said.contains(&format!("{long}. ")),
            "cut after {REASON_TOLD}"
        );
    }

    #[test]
    fn a_decision_that_goes_through_says_nothing_of_itself() {
        let (mut live, dir) = live_with("exit 0");
        let mut w = team(1);
        let before = w.journal.len();
        assert!(live.store_job(StoreJob::Decide {
            project: 0,
            repo: "demo".into(),
            agent: 1,
            proposal: offer("w1", 9, "c9").proposal,
            verdict: Decision::Approve(Vec::new()),
        }));
        job_ends(&mut live, &mut w);
        assert_eq!(w.journal.len(), before, "a look says what came of it");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_decision_is_one_word_on_its_proposal() {
        let (place, dir) =
            imece_agent::lonca_fake::program("echo \"$*\" >> \"$(dirname \"$0\")/all\"\nexit 0");
        let calls = |dir: &Path| std::fs::read_to_string(dir.join("all")).unwrap();
        let proposal = offer("w1", 9, "c9").proposal;
        let yes = Decision::Approve(vec!["api".into(), "e2e".into()]);
        decide(&place, "demo", &dir, &proposal, &yes).unwrap();
        assert_eq!(calls(&dir), "approve 9 --repo=demo --also=api,e2e --json\n");
        std::fs::remove_file(dir.join("all")).unwrap();
        let no = Decision::Reject("not this".into());
        decide(&place, "demo", &dir, &proposal, &no).unwrap();
        assert_eq!(
            calls(&dir),
            "reject 9 --repo=demo --reason=not this --json\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_decision_stops_at_what_lonca_refuses() {
        let (place, dir) = imece_agent::lonca_fake::program(
            "echo \"$*\" >> \"$(dirname \"$0\")/all\"\necho 'lonca: no' >&2\nexit 1",
        );
        let proposal = offer("w1", 9, "c9").proposal;
        let got = decide(
            &place,
            "demo",
            &dir,
            &proposal,
            &Decision::Approve(Vec::new()),
        );
        assert_eq!(got, Err(Failed::Said("no".into())));
        let calls = std::fs::read_to_string(dir.join("all")).unwrap();
        assert_eq!(calls.lines().count(), 1, "{calls}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_policy_is_asked_once_and_only_an_answer_is_kept() {
        let d = Daemon::start(PROTOCOL, |asked| match asked["params"]["commit"].as_str() {
            Some("off") => Ok(serde_json::json!([{"path": "lonca.toml",
                "content_b64": "W29ic2VydmF0aW9uXQp1bm9ic2VydmVkX2lzX3RhaW50ZWQgPSBmYWxzZQo="}])),
            Some("bad") => Err("internal"),
            _ => Ok(serde_json::json!([])),
        });
        d.sign_in();
        let mut c = d.place.open().unwrap();
        let mut kept = Kept::default();
        let asked = || d.heard.lock().unwrap().len();
        assert!(kept.holds(&mut c, "demo", "on"));
        assert!(kept.holds(&mut c, "demo", "on"));
        assert_eq!(asked(), 1, "the same commit is not read again");
        assert!(!kept.holds(&mut c, "demo", "off"), "a new commit is");
        assert_eq!(asked(), 2);
        assert!(!kept.holds(&mut c, "demo", "off"));
        assert_eq!(asked(), 2, "a no is kept as well");
        assert!(!kept.holds(&mut c, "other", "off"));
        assert_eq!(asked(), 3, "each repository has its own");
        assert!(
            !kept.holds(&mut c, "demo", "bad"),
            "no answer holds nothing"
        );
        assert!(!kept.holds(&mut c, "demo", "bad"));
        assert_eq!(asked(), 5, "and is asked again");
    }

    /// A daemon with `proposals` waiting, one worker's each, answering a
    /// diff of each.
    fn waiting_daemon(policy: serde_json::Value) -> Daemon {
        let d = Daemon::start(PROTOCOL, move |asked| match asked["method"].as_str() {
            Some("status") => {
                let mut v = status(&[7]);
                v["agent"] = serde_json::json!("x");
                v["changes"] = serde_json::json!([
                    {"change": "kkkk", "status": {"status": "pending", "seq": 7}}]);
                v["policy"] = policy.clone();
                v["pending"][0]["reasons"] =
                    serde_json::json!(["unobserved_session", "replaces:jjjj"]);
                // An operation waiting that is no proposal.
                let mut rule = v["pending"][0].clone();
                rule["op"] = serde_json::json!("policy.update");
                rule["seq"] = serde_json::json!(8);
                v["pending"].as_array_mut().unwrap().push(rule);
                Ok(v)
            }
            Some("change.diff") => Ok(serde_json::json!({"commit": "c", "files": [],
                "redacted": [], "diff": "the patch", "truncated": false})),
            Some("tree") => Ok(serde_json::json!([{"path": "lonca.toml",
                "content_b64": "W29ic2VydmF0aW9uXQp1bm9ic2VydmVkX2lzX3RhaW50ZWQgPSBmYWxzZQo="}])),
            _ => Err("not_found"),
        });
        d.sign_in();
        d
    }

    #[test]
    fn a_look_brings_what_waits_and_reads_each_diff_once() {
        let d = waiting_daemon(serde_json::json!(POLICY));
        let repos = vec![(4, "demo".to_string(), None)];
        let mut kept = Kept::default();
        let looked = look(&d.place, &repos, &[], &mut kept);
        assert_eq!(looked.offers.len(), 1);
        assert_eq!(looked.offers[0].0, 4);
        assert_eq!(looked.offers[0].1.agent, "a");
        assert_eq!(looked.offers[0].1.diff.patch, "the patch");
        assert!(looked.offers[0].1.proposal.on_trunk);
        assert_eq!(looked.standings.get("kkkk"), Some(&Standing::Pending));
        assert_eq!(looked.replaced, HashSet::from(["jjjj".to_string()]));
        // Every proposal waiting, with trunk; the other operation is not.
        assert_eq!(looked.pending.len(), 1);
        let (p, repo, trunk, pending) = &looked.pending[0];
        assert_eq!((*p, repo.as_str(), trunk.as_str()), (4, "demo", TRUNK));
        assert_eq!(pending.iter().map(|w| w.seq).collect::<Vec<_>>(), [7]);
        assert_eq!(pending[0].paths, ["src/a.rs"]);
        // An older daemon says nothing of the policy's checks.
        assert_eq!(looked.policy_checks, [(4, "demo".to_string(), None)]);
        look(&d.place, &repos, &[], &mut kept);
        let diffs = |d: &Daemon| {
            let heard = d.heard.lock().unwrap();
            heard
                .iter()
                .filter(|h| h["method"] == "change.diff")
                .count()
        };
        assert_eq!(diffs(&d), 1);
        // Gone from what waits, it is not kept.
        kept.diffs.insert("gone".into(), diff_of("old"));
        look(&d.place, &repos, &[], &mut kept);
        assert!(kept.diffs.contains_key("kkkk") && !kept.diffs.contains_key("gone"));
    }

    #[test]
    fn a_frozen_repository_is_closed_and_offers_nothing() {
        let d = Daemon::start(PROTOCOL, |asked| match asked["method"].as_str() {
            Some("status") => {
                let mut v = status(&[7]);
                v["frozen"] = serde_json::json!("integrity alarm");
                Ok(v)
            }
            _ => Err("not_found"),
        });
        d.sign_in();
        let repos = vec![(0, "demo".to_string(), None)];
        let looked = look(&d.place, &repos, &[], &mut Kept::default());
        assert_eq!(looked.found[0].2, StoreSeen::Down(StoreDown::Frozen));
        assert!(looked.offers.is_empty() && looked.pending.is_empty());
        assert!(looked.policy_checks.is_empty());
        let asked: Vec<String> = (d.heard.lock().unwrap().iter())
            .map(|h| h["method"].as_str().unwrap_or("").to_string())
            .collect();
        assert_eq!(asked, ["status"], "nothing more is read of it");
    }

    #[test]
    fn a_repository_that_lets_changes_in_unseen_is_closed_and_offers_nothing() {
        let d = waiting_daemon(serde_json::json!("0123"));
        let repos = vec![(0, "demo".to_string(), None)];
        let looked = look(&d.place, &repos, &[], &mut Kept::default());
        let down = StoreSeen::Down(StoreDown::Unguarded);
        assert_eq!(looked.found[0].2, down);
        assert!(looked.offers.is_empty() && looked.standings.is_empty());
        assert!(looked.replaced.is_empty());
        assert!(looked.pending.is_empty(), "nothing waits where no gate is");
        assert!(looked.policy_checks.is_empty());
    }

    #[test]
    fn a_proposal_waiting_reads_with_its_holder_and_its_base_against_trunk() {
        let (w, _, word) = proposer();
        let status = waiting(&[
            (5, Some(word.as_str()), Some("c5")),
            (6, Some("someone"), Some("c6")),
            (7, Some("x"), None),
            (8, None, Some("c8")),
        ]);
        let mut pending = status.pending.clone();
        pending[0].paths = vec!["src/a.rs".into(), "b.rs".into()];
        pending[1].base = Some("0000".into());
        assert_eq!(
            pending_of(&w, 0, TRUNK, pending[0].clone()),
            Some(StorePending {
                seq: 5,
                change: "c5".into(),
                name: word.clone(),
                holder: Some(1),
                paths: vec!["src/a.rs".into(), "b.rs".into()],
                on_trunk: true,
            })
        );
        let other = pending_of(&w, 0, TRUNK, pending[1].clone()).unwrap();
        assert_eq!(
            (other.holder, other.name.as_str(), other.on_trunk),
            (None, "someone", false)
        );
        assert_eq!(
            pending_of(&w, 3, TRUNK, pending[0].clone()).unwrap().holder,
            None,
            "another project's"
        );
        assert_eq!(
            pending_of(&w, 0, TRUNK, pending[2].clone()),
            None,
            "no change"
        );
        assert_eq!(
            pending_of(&w, 0, TRUNK, pending[3].clone()),
            None,
            "no agent"
        );
    }

    #[test]
    fn a_look_s_proposals_waiting_reach_the_project_with_their_holders() {
        let (mut w, mut live, word) = proposer();
        let rows = serde_json::json!([
            {"seq": 5, "created_ms": 5, "actor": "agent:a/s", "op": "change.propose",
                "reasons": ["unobserved_session"], "approvals": [], "required": 1, "change": "c5",
                "base": TRUNK, "intent": "fix", "agent": word, "task": "T-1", "paths": ["src/a.rs"]},
            {"seq": 6, "created_ms": 5, "actor": "agent:b/s", "op": "change.propose",
                "reasons": ["unobserved_session"], "approvals": [], "required": 1, "change": "c6",
                "base": "0000", "intent": "fix", "agent": "someone", "task": "T-2", "paths": ["b.rs"]},
        ]);
        let rows = Arc::new(std::sync::Mutex::new(rows));
        let said = rows.clone();
        let d = Daemon::start(PROTOCOL, move |asked| match asked["method"].as_str() {
            Some("status") => {
                let mut v = status(&[]);
                v["pending"] = said.lock().unwrap().clone();
                v["policy_checks"] = serde_json::json!(["e2e", "api"]);
                Ok(v)
            }
            Some("tree") => Ok(serde_json::json!([{"path": "lonca.toml",
                "content_b64": "W29ic2VydmF0aW9uXQp1bm9ic2VydmVkX2lzX3RhaW50ZWQgPSBmYWxzZQo="}])),
            _ => Err("not_found"),
        });
        d.sign_in();
        live.lonca = Watch::start(d.place.clone());
        live.lonca_due(&w, Instant::now());
        assert!(settled(&mut live, &mut w));
        let on = w.projects[0].lonca.as_ref().unwrap();
        let policy = Some(vec!["api".to_string(), "e2e".to_string()]);
        assert_eq!(on.policy_checks, policy, "the policy's checks too");
        assert!(
            matches!(on.seen, StoreSeen::Up { pending: 2, .. }),
            "{:?}",
            on.seen
        );
        assert_eq!(
            on.pending,
            [
                StorePending {
                    seq: 5,
                    change: "c5".into(),
                    name: word.clone(),
                    holder: Some(1),
                    paths: vec!["src/a.rs".into()],
                    on_trunk: true,
                },
                StorePending {
                    seq: 6,
                    change: "c6".into(),
                    name: "someone".into(),
                    holder: None,
                    paths: vec!["b.rs".into()],
                    on_trunk: false,
                },
            ]
        );
        assert!(
            w.changes.is_empty(),
            "no diff could be read: no change shown"
        );
        // Trunk moved on to the stranger's base: the only thing that changed.
        rows.lock().unwrap()[1]["base"] = serde_json::json!(TRUNK);
        live.look_now();
        live.lonca_due(&w, Instant::now());
        assert!(settled(&mut live, &mut w), "what waits moved");
        let on = w.projects[0].lonca.as_ref().unwrap();
        assert!(on.pending[1].on_trunk);
        live.look_now();
        live.lonca_due(&w, Instant::now());
        assert!(!settled(&mut live, &mut w), "nothing moved");
    }

    /// A team of one on Lonca, its worker between turns, its word for
    /// Lonca returned.
    fn proposer() -> (World, Live, String) {
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        w.agents[1].workspace = Some("demo".into());
        let word = lonca::agent_word(&w.agents[1].name);
        (w, idle_live(2), word)
    }

    fn standing(change: &str, s: Standing) -> HashMap<String, Standing> {
        HashMap::from([(change.to_string(), s)])
    }

    #[test]
    fn a_proposal_is_owned_by_its_worker_or_solo_gone_or_not() {
        let (mut w, _, word) = proposer();
        assert_eq!(Live::owner(&w, 0, &word), Some(1));
        assert_eq!(Live::owner(&w, 1, &word), None, "another project's");
        assert_eq!(Live::owner(&w, 0, "nobody"), None);
        let leader = lonca::agent_word(&w.agents[0].name);
        assert_eq!(Live::owner(&w, 0, &leader), None);
        w.agents[1].gone = true;
        assert_eq!(Live::owner(&w, 0, &word), Some(1));
        w.agents[1].parent = Some(0);
        assert_eq!(
            Live::owner(&w, 0, &word),
            None,
            "a helper's is its worker's"
        );
    }

    #[test]
    fn a_proposal_waits_for_its_workers_turn_to_end() {
        let (mut w, mut live, word) = proposer();
        live.turns[1] = 1;
        assert!(!live.offered(&mut w, 0, &offer(&word, 5, "c5")));
        assert!(w.change_of(1).is_none());
        live.turns[1] = 0;
        assert!(live.offered(&mut w, 0, &offer(&word, 5, "c5")));
        assert_eq!(w.change_of(1).unwrap().title, "intent 5");
        assert_eq!(w.proposal_of(1).unwrap().seq, 5);
        assert_eq!(w.open_inbox().len(), 1);
    }

    /// The acceptance run: a change's title was the agent's raw intent
    /// ("Card 16: …", its own numbering); its card's title is the title,
    /// the intent its text.
    #[test]
    fn a_proposal_of_a_worker_on_a_card_is_titled_by_the_card() {
        let (mut w, mut live, word) = proposer();
        let c = with_card(&mut w, 1);
        w.cards[c].title = "Settle the ledger".into();
        assert!(live.offered(&mut w, 0, &offer(&word, 5, "c5")));
        assert_eq!(w.change_of(1).unwrap().title, "Settle the ledger");
        assert_eq!(w.proposal_of(1).unwrap().intent, "intent 5");
        assert_eq!(w.change_why(1), Some("intent 5"));
        // Words the same as the title say nothing more.
        w.proposals.get_mut(&1).unwrap().intent = "Settle the ledger".into();
        assert_eq!(w.change_why(1), None);
    }

    #[test]
    fn someone_elses_proposal_is_no_ones_change() {
        let (mut w, mut live, _) = proposer();
        assert!(!live.offered(&mut w, 0, &offer("nobody", 5, "c5")));
        assert!(w.changes.is_empty());
    }

    #[test]
    fn the_same_proposal_stays_and_a_newer_one_takes_its_place() {
        let (mut w, mut live, word) = proposer();
        assert!(live.offered(&mut w, 0, &offer(&word, 5, "c5")));
        assert!(
            !live.offered(&mut w, 0, &offer(&word, 5, "c5")),
            "nothing new"
        );
        let same = offer(&word, 5, "c5");
        let mut rebased = same.clone();
        rebased.proposal.on_trunk = false;
        assert!(live.offered(&mut w, 0, &rebased), "trunk moved on");
        assert!(!w.proposal_of(1).unwrap().on_trunk);
        assert!(!live.offered(&mut w, 0, &rebased), "nothing new");
        assert_eq!(w.changes.len(), 1);
        assert!(live.offered(&mut w, 0, &offer(&word, 8, "c8")));
        assert_eq!(w.proposal_of(1).unwrap().seq, 8);
        assert_eq!(w.changes.len(), 1);
        assert_eq!(w.open_inbox().len(), 1);
    }

    #[test]
    fn a_proposal_on_trunk_is_said_and_the_worker_told_without_waking_it() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let on_trunk = Standing::Integrated("c1".into());
        assert!(live.land(
            &mut w,
            &standing("c5", on_trunk),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(w.change_of(1).is_none() && w.proposal_of(1).is_none());
        assert!(w.open_inbox().is_empty());
        assert!(matches!(
            last(&w),
            Event::Landed { agent: 1, how: Landed::Integrated(c) } if c == "c1"
        ));
        assert!(
            live.notes[1][0].contains("(intent 5)"),
            "{:?}",
            live.notes[1]
        );
        assert!(!live.lost[1]);
    }

    #[test]
    fn what_a_newer_proposal_replaces_is_read_from_its_reasons() {
        let mut status = waiting(&[(9, Some("w1"), Some("c9")), (6, Some("w2"), Some("c6"))]);
        status.pending[0].reasons = vec![
            "unobserved_session".into(),
            "replaces:c5".into(),
            "approvals=1".into(),
        ];
        status.pending[1].reasons = vec!["replaces:c2".into(), "replaces".into()];
        let got: Vec<String> = replaced_in(&status).collect();
        assert_eq!(got, ["c5", "c2"]);
        assert_eq!(replaced_in(&waiting(&[(9, None, None)])).count(), 0);
    }

    #[test]
    fn only_a_withdrawal_a_newer_proposal_names_is_a_replacement() {
        assert_eq!(as_replaced(Landed::Withdrawn, true), Landed::Replaced);
        assert_eq!(as_replaced(Landed::Withdrawn, false), Landed::Withdrawn);
        let on = Landed::Integrated("c1".into());
        assert_eq!(as_replaced(on.clone(), true), on);
        assert_eq!(as_replaced(Landed::Queued, true), Landed::Queued);
    }

    #[test]
    fn a_proposal_replaced_by_a_newer_one_tells_and_wakes_nobody() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let gone = standing("c5", Standing::Withdrawn);
        let named = HashSet::from(["c5".to_string()]);
        assert!(live.land(&mut w, &gone, &named, &HashMap::new()));
        assert!(w.change_of(1).is_none() && w.proposal_of(1).is_none());
        assert!(matches!(
            last(&w),
            Event::Landed {
                agent: 1,
                how: Landed::Replaced
            }
        ));
        assert!(live.notes[1].is_empty() && !live.lost[1]);
        // Named by nobody, it is its worker's taking back.
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let other = HashSet::from(["c4".to_string()]);
        assert!(live.land(&mut w, &gone, &other, &HashMap::new()));
        assert!(matches!(
            last(&w),
            Event::Landed {
                agent: 1,
                how: Landed::Withdrawn
            }
        ));
    }

    #[test]
    fn a_proposal_still_waiting_stays() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        assert!(!live.land(
            &mut w,
            &standing("c5", Standing::Pending),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(!live.land(
            &mut w,
            &standing("c5", Standing::Other),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(
            !live.land(&mut w, &HashMap::new(), &HashSet::new(), &HashMap::new()),
            "not listed"
        );
        assert!(w.proposal_of(1).is_some());
    }

    #[test]
    fn one_the_queue_sent_back_wakes_its_worker_with_the_way_on() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let clash = Standing::Conflicted(vec!["a#b".into()]);
        assert!(live.land(
            &mut w,
            &standing("c5", clash),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(live.notes[1][0].contains("a#b"), "{:?}", live.notes[1]);
        assert!(live.lost[1]);
    }

    /// Worker 1's proposal for its card, sent back `back` times before,
    /// is sent back by Lonca's queue once more.
    fn sent_back_by_lonca(back: u32) -> (World, Live, usize) {
        let (mut w, mut live, word) = proposer();
        let c = with_card(&mut w, 1);
        w.cards[c].breaker.sent_back = back;
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let no = Standing::Rejected(vec!["verification_failed:tests".into()]);
        assert!(live.land(
            &mut w,
            &standing("c5", no),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert_eq!(w.cards[c].breaker.sent_back, back + 1);
        (w, live, c)
    }

    #[test]
    fn a_change_back_its_last_time_waits_for_the_person_and_their_answer_wakes_or_fails() {
        use crate::model::{Column, InboxKind};
        let open = |w: &World| -> Vec<(InboxKind, usize, Option<usize>)> {
            (w.open_inbox().into_iter())
                .map(|i| (w.inbox[i].kind, w.inbox[i].agent, w.inbox[i].card))
                .collect()
        };
        // Its second time back: woken as ever.
        let (w, live, _) = sent_back_by_lonca(1);
        assert!(open(&w).is_empty());
        assert!(live.lost[1] && !w.waits_on_person(1));
        // Its third: the person decides; its words wait, the item holds
        // the wake (record 126, F9).
        let (mut w, mut live, c) = sent_back_by_lonca(2);
        assert_eq!(open(&w), [(InboxKind::Rounds, 1, Some(c))]);
        assert!(live.lost[1] && !live.notes[1].is_empty());
        assert!(w.waits_on_person(1));
        let item = w.open_inbox()[0];
        // A run since forgot the wake: the yes wakes it all the same.
        live.lost[1] = false;
        assert!(live.decide_cap(&mut w, item, true));
        assert!(!live.decide_cap(&mut w, item, true), "answered");
        assert!(live.lost[1] && !w.waits_on_person(1), "woken");
        assert_eq!(w.cards[c].breaker.more_rounds, 1);
        assert!(!live.lost[0], "its leader sleeps");
        // No: the card fails, its worker sleeps, its leader hears.
        let (mut w, mut live, c) = sent_back_by_lonca(2);
        let item = w.open_inbox()[0];
        assert!(live.decide_cap(&mut w, item, false));
        assert_eq!(w.cards[c].column, Column::Failed);
        assert!(!live.lost[1]);
        assert_eq!(
            live.notes[1].last().unwrap(),
            "The person failed #1 for good: leave it, and send nothing more for it."
        );
        assert!(live.lost[0]);
        assert_eq!(
            live.notes[0],
            [
                "The person failed #1 for good after its change came back 3 times: nobody hands \
              it again."
            ]
        );
    }

    #[test]
    fn a_change_the_person_sent_back_never_meets_the_breaker() {
        let (mut w, mut live, word) = proposer();
        let c = with_card(&mut w, 1);
        w.cards[c].breaker.sent_back = 5;
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        live.lonca.worded.push("c5".into());
        let no = Standing::Rejected(vec!["human".into()]);
        live.land(
            &mut w,
            &standing("c5", no),
            &HashSet::new(),
            &HashMap::new(),
        );
        assert!(w.open_inbox().is_empty(), "they decide already");
        // Nor one that is on trunk, while another card of its worker's,
        // at work, has its rounds out: nothing woke it.
        let (mut w, mut live, word) = proposer();
        let c = with_card(&mut w, 1);
        w.cards[c].breaker.sent_back = 5;
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let other = with_card(&mut w, 1);
        w.cards[other].breaker.sent_back = 5;
        assert_eq!(w.rounds_out_of(1), Some(other));
        let on_trunk = Standing::Integrated("c1".into());
        live.land(
            &mut w,
            &standing("c5", on_trunk),
            &HashSet::new(),
            &HashMap::new(),
        );
        assert!(w.open_inbox().is_empty());
    }

    #[test]
    fn a_worker_woken_already_stays_woken() {
        let (mut w, mut live, word) = proposer();
        live.lost[1] = true;
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        let clash = Standing::Conflicted(vec!["a#b".into()]);
        live.land(
            &mut w,
            &standing("c5", clash),
            &HashSet::new(),
            &HashMap::new(),
        );
        assert!(live.lost[1], "the other reason still stands");
    }

    #[test]
    fn a_gone_worker_is_not_woken_for_it() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        w.agents[1].gone = true;
        let no = Standing::Rejected(vec!["no".into()]);
        live.land(
            &mut w,
            &standing("c5", no),
            &HashSet::new(),
            &HashMap::new(),
        );
        assert!(!live.lost[1]);
    }

    #[test]
    fn what_the_queue_holds_is_said_when_it_comes_out() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        assert!(live.land(
            &mut w,
            &standing("c5", Standing::Queued),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(w.proposal_of(1).is_none(), "the person is done with it");
        assert!(matches!(
            last(&w),
            Event::Landed {
                how: Landed::Queued,
                ..
            }
        ));
        assert!(live.notes[1].is_empty());
        let lines = w.journal.len();
        assert!(
            !live.land(
                &mut w,
                &standing("c5", Standing::Queued),
                &HashSet::new(),
                &HashMap::new()
            ),
            "it stays"
        );
        assert_eq!(w.journal.len(), lines);
        let out = Standing::Conflicted(vec!["a#b".into()]);
        assert!(live.land(
            &mut w,
            &standing("c5", out),
            &HashSet::new(),
            &HashMap::new()
        ));
        assert!(matches!(
            last(&w),
            Event::Landed {
                how: Landed::Conflicted(_),
                ..
            }
        ));
        assert!(live.lost[1]);
        assert!(live.lonca.queued.is_empty());
        assert!(!live.land(
            &mut w,
            &standing("c5", Standing::Rejected(vec![])),
            &HashSet::new(),
            &HashMap::new()
        ));
    }

    #[test]
    fn a_rejection_the_person_worded_is_told_once_by_them() {
        let (mut w, mut live, word) = proposer();
        live.offered(&mut w, 0, &offer(&word, 5, "c5"));
        live.lonca.worded.push("c5".into());
        let no = Standing::Rejected(vec!["rejected: no".into()]);
        live.land(
            &mut w,
            &standing("c5", no),
            &HashSet::new(),
            &HashMap::new(),
        );
        assert!(live.notes[1].is_empty() && !live.lost[1]);
        assert!(live.lonca.worded.is_empty());
    }

    #[test]
    fn a_decision_goes_through_lonca_one_at_a_time() {
        let (mut live, dir) = live_with("exit 0");
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 1,
            behind: Behind::Unknown,
        };
        w.store_seen(0, "demo", up);
        let no_proposal = "no proposal of theirs waits".to_string();
        assert_eq!(
            live.decide_proposal(&w, 1, Decision::Approve(Vec::new())),
            Err(no_proposal)
        );
        w.change_proposed(
            change_of(1, &offer("w", 5, "c5"), None),
            offer("w", 5, "c5").proposal,
        );
        assert_eq!(live.apply_change(&w, 1, Vec::new()), Ok(()));
        assert!(live.applying[1]);
        assert_eq!(
            live.apply_change(&w, 1, Vec::new()),
            Err("their change is being applied".to_string())
        );
        live.applying[1] = false;
        assert_eq!(
            live.decide_proposal(&w, 1, Decision::Approve(Vec::new())),
            Err("another lonca command is under way".to_string())
        );
        live.applying[1] = true;
        job_ends(&mut live, &mut w);
        assert!(!live.applying[1], "free again");
        assert!(live.lonca_is_due(), "the look that says what came of it");
        assert!(w.proposal_of(1).is_some(), "the store says when it left");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_yes_asks_for_the_checks_of_the_change_s_card() {
        let (mut live, dir) = live_with("echo \"$*\" >> \"$(dirname \"$0\")/all\"\nexit 0");
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 1,
            behind: Behind::Unknown,
        };
        w.store_seen(0, "demo", up);
        let c = with_card(&mut w, 1);
        w.cards[c].checks = vec!["api".into(), "e2e".into()];
        w.change_proposed(
            change_of(1, &offer("w", 5, "c5"), None),
            offer("w", 5, "c5").proposal,
        );
        assert_eq!(live.apply_change(&w, 1, Vec::new()), Ok(()));
        job_ends(&mut live, &mut w);
        let calls = std::fs::read_to_string(dir.join("all")).unwrap();
        assert!(
            calls.contains("approve 5 --repo=demo --also=api,e2e --json"),
            "{calls}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_decision_lonca_refused_is_said_and_the_change_still_waits() {
        let (mut live, dir) = live_with("echo 'lonca: no finger' >&2\nexit 1");
        let (mut w, _, _) = proposer();
        w.store_seen(
            0,
            "demo",
            StoreSeen::Up {
                trunk: TRUNK.into(),
                pending: 1,
                behind: Behind::Unknown,
            },
        );
        w.change_proposed(
            change_of(1, &offer("w", 5, "c5"), None),
            offer("w", 5, "c5").proposal,
        );
        live.apply_change(&w, 1, Vec::new()).unwrap();
        job_ends(&mut live, &mut w);
        assert!(!live.applying[1]);
        assert!(matches!(
            last(&w),
            Event::StoreFailed { act: StoreAct::Decide, why: Some(why), .. } if why == "no finger"
        ));
        assert!(w.proposal_of(1).is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_rejection_tells_the_worker_before_its_words_and_is_not_told_again() {
        let (mut live, dir) = live_with("exit 0");
        let (mut w, _, _) = proposer();
        w.store_seen(
            0,
            "demo",
            StoreSeen::Up {
                trunk: TRUNK.into(),
                pending: 1,
                behind: Behind::Unknown,
            },
        );
        w.change_proposed(
            change_of(1, &offer("w", 5, "c5"), None),
            offer("w", 5, "c5").proposal,
        );
        live.reject_proposal(&w, 1, "not this").unwrap();
        assert_eq!(live.lonca.worded, ["c5"]);
        assert!(
            live.notes[1][0].contains("(intent 5)"),
            "{:?}",
            live.notes[1]
        );
        job_ends(&mut live, &mut w);
        let told = imece_agent::lonca_fake::seen(&dir);
        assert!(
            told.contains("|reject 5 --repo=demo --reason=not this --json|"),
            "{told}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_rejection_that_did_not_start_tells_nobody() {
        let (mut w, mut live, _) = proposer();
        w.store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        w.change_proposed(
            change_of(1, &offer("w", 5, "c5"), None),
            offer("w", 5, "c5").proposal,
        );
        let why = Err(StoreDown::NoDaemon.why().to_string());
        assert_eq!(live.reject_proposal(&w, 1, "no"), why);
        assert_eq!(live.apply_change(&w, 1, Vec::new()), why);
        assert!(live.notes[1].is_empty() && live.lonca.worded.is_empty());
        assert!(!live.applying[1]);
    }

    #[test]
    fn a_look_whose_only_news_is_the_policy_s_checks_changed_the_screen() {
        let names = Arc::new(std::sync::Mutex::new(serde_json::json!(["api"])));
        let said = names.clone();
        let d = Daemon::start(PROTOCOL, move |asked| match asked["method"].as_str() {
            Some("status") => {
                let mut v = status(&[]);
                v["policy_checks"] = said.lock().unwrap().clone();
                Ok(v)
            }
            Some("tree") => Ok(serde_json::json!([{"path": "lonca.toml",
                "content_b64": "W29ic2VydmF0aW9uXQp1bm9ic2VydmVkX2lzX3RhaW50ZWQgPSBmYWxzZQo="}])),
            _ => Err("not_found"),
        });
        d.sign_in();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        w.on_lonca(0, Some("demo".into()));
        let look = |live: &mut Live, w: &mut World| {
            live.look_now();
            live.lonca_due(w, Instant::now());
            settled(live, w)
        };
        // The store coming up and its checks both are news: neither undoes
        // the other.
        assert!(look(&mut live, &mut w), "the first answer");
        assert_eq!(w.policy_checks(0), Some(&["api".to_string()][..]));
        assert!(!look(&mut live, &mut w), "the same again");
        *names.lock().unwrap() = serde_json::json!(["api", "e2e"]);
        assert!(look(&mut live, &mut w), "only the checks moved");
        assert_eq!(w.policy_checks(0).map(<[String]>::len), Some(2));
        *names.lock().unwrap() = serde_json::Value::Null;
        assert!(look(&mut live, &mut w), "Lonca no longer says");
        assert_eq!(w.policy_checks(0), None);
    }

    #[test]
    fn a_look_that_found_a_proposal_changed_the_screen() {
        let d = waiting_daemon(serde_json::json!(POLICY));
        let mut w = team(1);
        w.agents[1].name = "a".into();
        w.on_lonca(0, Some("demo".into()));
        let mut live = idle_live(2);
        live.lonca = Watch::start(d.place.clone());
        live.lonca_due(&w, Instant::now());
        settle(&mut live, &mut w);
        assert_eq!(w.proposal_of(1).map(|p| p.seq), Some(7));
        assert_eq!(w.change_of(1).unwrap().patch, "the patch");
    }

    /// A workspace of `demo` and a daemon saying its change stands as
    /// `standing`, with a `lonca diff` that says `facts`.
    fn gate_place(facts: &str, submitted: &str, standing: &str) -> (Place, Daemon, PathBuf) {
        let status: serde_json::Value = serde_json::from_str(standing).unwrap();
        let d = Daemon::start(PROTOCOL, move |asked| {
            if asked["params"]["change"] == "kkkk" {
                Ok(serde_json::json!({ "status": status.clone() }))
            } else {
                Err("not_found")
            }
        });
        d.sign_in();
        let (stub, tree) = imece_agent::lonca_fake::program(&format!("echo '{facts}'"));
        std::fs::create_dir_all(tree.join(".lonca")).unwrap();
        let file = serde_json::json!({"repo": "demo", "submitted": submitted_of(submitted)});
        std::fs::write(tree.join(".lonca/workspace.json"), file.to_string()).unwrap();
        let mut place = d.place.clone();
        place.program = stub.program.clone();
        (place, d, tree)
    }

    fn submitted_of(change: &str) -> serde_json::Value {
        match change {
            "" => serde_json::Value::Null,
            change => serde_json::json!([change, "c1"]),
        }
    }

    const CHANGED: &str = r#"{"added_files":[],"modified_files":["a.rs"],"deleted_files":[]}"#;

    #[test]
    fn a_workspace_with_nothing_to_propose_may_say_done() {
        let (place, _d, tree) = gate_place("no local changes", "", "{}");
        assert_eq!(proposed(&place, "demo", &tree), Ok(()));
        std::fs::remove_dir_all(tree).unwrap();
    }

    #[test]
    fn changes_with_no_proposal_are_refused() {
        let (place, _d, tree) = gate_place(CHANGED, "", "{}");
        let first = Err(PROPOSE_FIRST.to_string());
        assert_eq!(proposed(&place, "demo", &tree), first);
        std::fs::remove_dir_all(tree).unwrap();
    }

    #[test]
    fn a_proposal_that_stands_lets_the_done_go() {
        for standing in [
            r#"{"status":"pending"}"#,
            r#"{"status":"queued"}"#,
            r#"{"status":"integrated","commit":"c9"}"#,
        ] {
            let (place, _d, tree) = gate_place(CHANGED, "kkkk", standing);
            assert_eq!(proposed(&place, "demo", &tree), Ok(()), "{standing}");
            std::fs::remove_dir_all(tree).unwrap();
        }
    }

    #[test]
    fn a_proposal_that_fell_is_no_proposal() {
        for standing in [
            r#"{"status":"rejected","reasons":["no"]}"#,
            r#"{"status":"conflicted","symbols":["a"]}"#,
            r#"{"status":"withdrawn"}"#,
            r#"{"status":"a newer word"}"#,
        ] {
            let (place, _d, tree) = gate_place(CHANGED, "kkkk", standing);
            let first = Err(PROPOSE_FIRST.to_string());
            assert_eq!(proposed(&place, "demo", &tree), first, "{standing}");
            std::fs::remove_dir_all(tree).unwrap();
        }
    }

    #[test]
    fn what_lonca_can_not_say_stops_the_done_and_says_so() {
        let (mut place, d, tree) = gate_place(CHANGED, "kkkk", "{}");
        assert_eq!(
            proposed(&place, "demo", &tree),
            Err(NO_WORD.to_string()),
            "no status"
        );
        place.socket = d.dir.join("nobody");
        assert_eq!(
            proposed(&place, "demo", &tree),
            Err(NO_WORD.to_string()),
            "no daemon"
        );
        std::fs::remove_dir_all(tree).unwrap();
    }

    #[test]
    fn only_a_workers_done_from_a_workspace_is_gated() {
        let (mut w, mut live, _) = proposer();
        let done = serde_json::json!({"type": " done ", "to": "lead", "body": "x"});
        let tree = PathBuf::from("/tree");
        assert_eq!(live.gate_of(&w, 1, &done), None, "no tree yet");
        live.cwds[1] = Some(tree.clone());
        let gated = Some(("demo".to_string(), tree));
        assert_eq!(live.gate_of(&w, 1, &done), gated);
        // One that failed, or came late, shows no change: nothing to wait
        // for (record 126).
        let mut failed = done.clone();
        failed["outcome"] = serde_json::json!(" failed ");
        assert_eq!(live.gate_of(&w, 1, &failed), None);
        failed["outcome"] = serde_json::json!("done");
        assert_eq!(live.gate_of(&w, 1, &failed), gated);
        let c = crate::live::testing::with_card(&mut w, 1);
        w.agents[1].attempt = 1;
        assert_eq!(live.gate_of(&w, 1, &done), gated, "its card");
        w.cards[c].owner = Some(0);
        assert_eq!(live.gate_of(&w, 1, &done), None, "handed away");
        w.cards[c].owner = Some(1);
        let inform = serde_json::json!({"type": "inform", "to": "lead", "body": "x"});
        assert_eq!(live.gate_of(&w, 1, &inform), None);
        w.agents[1].role = Role::Helper;
        assert_eq!(live.gate_of(&w, 1, &done), None);
        w.agents[1].role = Role::Worker;
        w.agents[1].workspace = None;
        assert_eq!(live.gate_of(&w, 1, &done), None, "a worktree");
    }

    /// Steps until `answer` has a word.
    fn answered(
        live: &mut Live,
        w: &mut World,
        answer: &Receiver<imece_agent::mcp::Answer>,
    ) -> imece_agent::mcp::Answer {
        let till = Instant::now() + WAIT;
        loop {
            live.take_lonca(w);
            if let Ok(said) = answer.try_recv() {
                return said;
            }
            assert!(Instant::now() < till, "no answer");
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// A worker's done with work it never proposed, and one it did.
    #[test]
    fn a_done_goes_on_only_past_its_gate() {
        let (place, _d, tree) = gate_place(CHANGED, "", "{}");
        let (mut w, mut live, _) = proposer();
        w.agents[0].name = "lead".into();
        live.lonca = Watch::start(place);
        live.cwds[1] = Some(tree.clone());
        let done = serde_json::json!({"type": "done", "to": "lead", "body": "all done"});
        let (tx, rx) = mpsc::channel();
        live.send(&mut w, 1, &done, tx);
        assert_eq!(live.mail[0].len(), 0, "held");
        let said = answered(&mut live, &mut w, &rx);
        assert_eq!(said, Err(PROPOSE_FIRST.to_string()));
        assert_eq!(live.mail[0].len(), 0, "refused: it never went");

        let file = serde_json::json!({"repo": "demo", "submitted": null});
        std::fs::write(tree.join(".lonca/workspace.json"), file.to_string()).unwrap();
        let stub = tree.join("lonca");
        std::fs::write(&stub, "#!/bin/sh\necho 'no local changes'\n").unwrap();
        let (tx, rx) = mpsc::channel();
        live.send(&mut w, 1, &done, tx);
        assert!(answered(&mut live, &mut w, &rx).is_ok());
        assert_eq!(live.mail[0].len(), 1, "passed: it went");
        std::fs::remove_dir_all(tree).unwrap();
    }

    #[test]
    fn a_gate_answering_changed_the_screen() {
        let (mut w, mut live, _) = proposer();
        assert!(!live.take_gates(&mut w), "nothing came");
        let (reply, answer) = mpsc::channel();
        let args = serde_json::json!({});
        live.lonca
            .gated
            .send(Gated {
                from: 1,
                args,
                reply,
                passed: Err("no".into()),
            })
            .unwrap();
        live.lonca_after_a_look();
        assert!(live.take_lonca(&mut w));
        assert_eq!(answer.try_recv().unwrap(), Err("no".to_string()));
    }
}
