//! The screen's words in English (default) and Turkish. Templates use `{}`
//! for their arguments; `fill` cleans every argument, because arguments carry
//! text that agents wrote.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Tr,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Lang::En),
            "tr" | "turkish" | "türkçe" => Some(Lang::Tr),
            _ => None,
        }
    }

    pub fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Tr,
            Lang::Tr => Lang::En,
        }
    }

    pub fn text(self) -> &'static Text {
        match self {
            Lang::En => &EN,
            Lang::Tr => &TR,
        }
    }
}

/// Removes control characters (an agent's text must not drive the terminal)
/// and hides tokens that look like secrets.
pub fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let word: String = word
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let upper = word.to_ascii_uppercase();
        let secret = ["API_KEY=", "TOKEN=", "SECRET=", "PASSWORD="]
            .iter()
            .any(|k| upper.contains(k))
            || word.starts_with("sk-")
            || word.starts_with("ghp_")
            || word.starts_with("github_pat_");
        out.push_str(if secret { "[***]" } else { &word });
    }
    out
}

/// `template` with each `{}` replaced by the next argument, cleaned.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    let mut args = args.iter();
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        out.push_str(&clean(args.next().copied().unwrap_or("")));
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

pub struct Text {
    // Header
    pub cards: &'static str,
    pub agents_working: &'static str,
    pub per_hour: &'static str,
    pub budget: &'static str,
    pub researchers: &'static str,
    pub waiting_for_you: &'static str,
    pub paused: &'static str,
    // Footer
    pub write_lead: &'static str,
    pub to_agent: &'static str,
    pub panel: &'static str,
    pub fold: &'static str,
    pub focus: &'static str,
    pub pause: &'static str,
    pub help: &'static str,
    pub trace: &'static str,
    pub back: &'static str,
    pub write_this: &'static str,
    pub views: [&'static str; 3],
    pub typing_to: &'static str,
    pub send_cancel: &'static str,
    // Notices
    pub sent: &'static str,
    pub no_agent: &'static str,
    pub empty_message: &'static str,
    pub budget_spent: &'static str,
    pub cannot_start: &'static str,
    pub approved: &'static str,
    pub rejected: &'static str,
    pub language: &'static str,
    // Salon
    pub salon: &'static str,
    pub inbox_title: &'static str,
    pub journal: &'static str,
    pub queue: &'static str,
    pub queue_empty: &'static str,
    pub latest: &'static str,
    pub nothing_needed: &'static str,
    pub item_keys: &'static str,
    pub reply_keys: &'static str,
    pub journal_keys: &'static str,
    pub thread_title: &'static str,
    pub event_title: &'static str,
    pub thread_keys: &'static str,
    pub copied: &'static str,
    pub copy_failed: &'static str,
    pub folded: &'static str,
    pub group_working: &'static str,
    pub group_waiting: &'static str,
    pub you: &'static str,
    // Statuses
    pub coordinating: &'static str,
    pub answered: &'static str,
    pub idle: &'static str,
    pub planning: &'static str,
    pub blocked: &'static str,
    pub research_queue: &'static str,
    pub waiting_human: &'static str,
    pub verifying: &'static str,
    pub thinking: &'static str,
    pub stopped: &'static str,
    pub researching: &'static str,
    // Board
    pub columns: [&'static str; 4],
    // Code map
    pub code_title: &'static str,
    pub leases: &'static str,
    pub shared_files: &'static str,
    pub code_legend: &'static str,
    pub same_file: &'static str,
    pub no_code_map: &'static str,
    // Focus
    pub focus_title: &'static str,
    pub minutes: &'static str,
    pub no_card: &'static str,
    pub research: &'static str,
    pub status: &'static str,
    pub origin: &'static str,
    pub lease: &'static str,
    pub clean: &'static str,
    pub soft: &'static str,
    pub full: &'static str,
    pub no_story: &'static str,
    pub story_labels: [&'static str; 7],
    pub trace_title: &'static str,
    // Journal
    pub ev_plan: &'static str,
    pub ev_assign: &'static str,
    pub ev_assumption: &'static str,
    pub ev_blocked: &'static str,
    pub ev_blocked_joined: &'static str,
    pub ev_research_opened: &'static str,
    pub ev_research_delivered: &'static str,
    pub ev_research_needs_you: &'static str,
    pub ev_from_cache: &'static str,
    pub ev_sensitive: &'static str,
    pub ev_merged: &'static str,
    pub ev_failed: &'static str,
    pub ev_approved: &'static str,
    pub ev_approved_research: &'static str,
    pub ev_rejected: &'static str,
    pub ev_stopped: &'static str,
    pub ev_posted: &'static str,
    pub ev_claimed: &'static str,
    /// Message types, in `MsgKind::ALL` order.
    pub kinds: [&'static str; 10],
    // Card stories
    pub st_took: &'static str,
    pub st_writing_tests: &'static str,
    pub st_blocked_on: &'static str,
    pub st_research_arrived: &'static str,
    pub st_research_needs_you: &'static str,
    pub st_from_cache: &'static str,
    pub st_change: &'static str,
    pub st_sensitive: &'static str,
    pub st_verified: &'static str,
    pub st_queue_failed: &'static str,
    pub st_you_approved: &'static str,
    pub st_you_approved_research: &'static str,
    pub st_you_rejected: &'static str,
    pub st_you_rejected_research: &'static str,
    pub st_touched: &'static str,
    pub st_stopped: &'static str,
    pub st_posted: &'static str,
    // Inbox
    pub in_research: &'static str,
    pub in_reply: &'static str,
    // Overlays
    pub help_title: &'static str,
    pub help_keys: [(&'static str, &'static str); 16],
    pub help_legend: [&'static str; 3],
    pub close: &'static str,
    pub away_title: &'static str,
    pub away_counts: [&'static str; 5],
}

pub const EN: Text = Text {
    cards: "{}/{} cards",
    agents_working: "{} agents working",
    per_hour: "{}/h",
    budget: " (budget {}%)",
    researchers: "researchers {}/{}",
    waiting_for_you: " ! {} waiting for you ",
    paused: " ‖ PAUSED ",
    write_lead: "write to the lead",
    to_agent: "to an agent",
    panel: "panel",
    fold: "fold",
    focus: "focus",
    pause: "pause",
    help: "help",
    trace: "trace",
    back: "back",
    write_this: "write to this agent",
    views: ["salon", "board", "code"],
    typing_to: " › to {}: ",
    send_cancel: "   enter send · esc cancel",
    sent: "sent to {}",
    no_agent: "no agent named \"{}\"",
    empty_message: "the message is empty",
    budget_spent: "{} has spent its budget",
    cannot_start: "{} could not start: {}",
    approved: "approved",
    rejected: "rejected",
    language: "English",
    salon: " SALON ",
    inbox_title: " WAITING FOR YOU ({}) ",
    journal: " JOURNAL ",
    queue: "QUEUE ",
    queue_empty: " empty",
    latest: "  │ latest:",
    nothing_needed: "Nothing needs you right now.",
    item_keys: "  a approve · r reject · enter focus",
    reply_keys: "  enter read all · r dismiss · or just write back",
    journal_keys: "  enter open the conversation · c copy",
    thread_title: " CONVERSATION · {} ⇄ {} ({}) ",
    event_title: " EVENT ",
    thread_keys: " ↑↓ line · pgup pgdn scroll · c copy line · C copy all · esc close",
    copied: "copied ({})",
    copy_failed: "could not copy: {}",
    folded: "… {} agents doing fine",
    group_working: "{} working",
    group_waiting: " ! {} waiting ",
    you: "you",
    coordinating: "coordinating",
    answered: "answered you",
    idle: "idle",
    planning: "planning",
    blocked: "BLOCKED",
    research_queue: " in the researcher queue",
    waiting_human: "waiting for you",
    verifying: "◌ verifying",
    thinking: "thinking…",
    stopped: "■ stopped, write to it to start again",
    researching: "researching: {}",
    columns: ["WAITING", "IN PROGRESS", "VERIFYING", "DONE"],
    code_title: " CODE MAP · {} · who works where (leases) ",
    leases: "{} leases",
    shared_files: " · {} files with several agents at once",
    code_legend: "   ██ working  ◌◌ verifying  ✕✕ blocked  !! waiting for you",
    same_file: "⇆ {} agents in one file",
    no_code_map: "no shared trunk yet: each agent works in its own worktree, so there are no leases to show",
    focus_title: " FOCUS · {} ",
    minutes: " · {} min",
    no_card: "no card right now",
    research: "research: {}",
    status: "status ",
    origin: "origin ",
    lease: "lease  ",
    clean: "clean",
    soft: "soft-tainted: its sensitive changes come to you",
    full: "tainted: reads the outside world, all its output comes to you",
    no_story: "No story yet. Enter for the raw stream.",
    story_labels: [
        "intent",
        "assumption",
        "decision",
        "talk",
        "evidence",
        "change",
        "you",
    ],
    trace_title: " TRACE · {} · raw stream ",
    ev_plan: "{}: \"{}\" split into 3 cards",
    ev_assign: "{} → {}: {} {}",
    ev_assumption: "{} assumes: {}",
    ev_blocked: "{} blocked: {}",
    ev_blocked_joined: "{} blocked: {} (already being researched, waits for {})",
    ev_research_opened: "{} opened: {}",
    ev_research_delivered: "{} report delivered: {}",
    ev_research_needs_you: "{} report waits for you: {}",
    ev_from_cache: "from cache: \"{}\" → {}",
    ev_sensitive: "{} {} → needs your approval",
    ev_merged: "{} {} merged ({})",
    ev_failed: "{}: a test failed in the merge queue, fixing",
    ev_approved: "approved: {} sensitive change",
    ev_approved_research: "approved: research report for {}",
    ev_rejected: "rejected: {}",
    ev_stopped: "{} stopped: {}",
    ev_posted: "{} posted {} {}",
    ev_claimed: "{} took {} {} from the board",
    kinds: [
        "assign", "ask", "answer", "inform", "handoff", "done", "blocked", "progress", "notice",
        "cancel",
    ],
    st_took: "{}: {}",
    st_writing_tests: "writing tests",
    st_blocked_on: "blocked: \"{}\" needs research",
    st_research_arrived: "{} report arrived: {} (soft-tainted)",
    st_research_needs_you: "{} report proposes a new package; waits for you",
    st_from_cache: "research came from the cache: {}",
    st_change: "{} files, +{} −{}",
    st_sensitive: "sensitive: {}",
    st_verified: "build ✓ · {} affected tests ✓ · trunk #{}",
    st_queue_failed: "merge queue: test ✕, fixing",
    st_you_approved: "you approved the sensitive change",
    st_you_approved_research: "you approved the report (reviewed, no longer taints)",
    st_you_rejected: "you rejected it; trying another way",
    st_you_rejected_research: "you rejected the report; going on without it",
    st_touched: "changed {}",
    st_stopped: "stopped: {}",
    st_posted: "{} posted it: {}",
    in_research: "{} report \"{}\" proposes a new package → {}",
    in_reply: "{} answered you: {}",
    help_title: " HELP ",
    help_keys: [
        ("/", "write to the lead of the selected project"),
        ("@name text", "write to one agent"),
        ("1 2 3", "salon · board · code map"),
        ("↑ ↓  j k", "select"),
        ("← →  h l", "fold / unfold a project"),
        ("enter", "look deeper: focus, then the raw stream"),
        ("esc", "go back"),
        ("tab", "salon → waiting for you → journal"),
        ("enter", "in the journal: the whole conversation"),
        ("c  C", "copy the line · the conversation"),
        ("<  >", "wider / narrower side panel"),
        ("a  r", "approve · reject (in waiting for you)"),
        ("space", "pause / resume"),
        ("+  -", "faster / slower simulation"),
        ("L", "Türkçe / English"),
        ("q", "quit"),
    ],
    help_legend: [
        " ◆ lead  ● working  ◐ asking  ✕ blocked  ! waiting for you",
        " ◌ verifying  ◇ researcher  ○ idle  ⇄ conversation  ▾ ▸ project",
        " ~ soft-tainted (read a report)  ≈ tainted (reads the outside world)",
    ],
    close: " any key: close",
    away_title: " WHILE YOU WERE AWAY · last {} min ",
    away_counts: [
        "cards merged",
        "decisions wait for you",
        "blocks",
        "assumptions made",
        "conversations",
    ],
};

pub const TR: Text = Text {
    cards: "{}/{} kart",
    agents_working: "{} agent çalışıyor",
    per_hour: "{}/sa",
    budget: " (bütçe %{})",
    researchers: "araştırmacı {}/{}",
    waiting_for_you: " ! {} seni bekliyor ",
    paused: " ‖ DURDU ",
    write_lead: "lidere yaz",
    to_agent: "agent'a",
    panel: "panel",
    fold: "katla",
    focus: "odak",
    pause: "durdur",
    help: "yardım",
    trace: "iz",
    back: "geri",
    write_this: "bu agent'a yaz",
    views: ["salon", "pano", "kod"],
    typing_to: " › {} için: ",
    send_cancel: "   enter gönder · esc vazgeç",
    sent: "iletildi → {}",
    no_agent: "\"{}\" adında bir agent yok",
    empty_message: "mesaj boş",
    budget_spent: "{} bütçesini bitirdi",
    cannot_start: "{} başlatılamadı: {}",
    approved: "onaylandı",
    rejected: "reddedildi",
    language: "Türkçe",
    salon: " SALON ",
    inbox_title: " SENİ BEKLEYEN ({}) ",
    journal: " DEFTER ",
    queue: "KUYRUK ",
    queue_empty: " boş",
    latest: "  │ son:",
    nothing_needed: "Şu an senden bir şey gerekmiyor.",
    item_keys: "  a onayla · r reddet · enter odak",
    reply_keys: "  enter tamamını oku · r kapat · ya da doğrudan yanıt yaz",
    journal_keys: "  enter konuşmayı aç · c kopyala",
    thread_title: " KONUŞMA · {} ⇄ {} ({}) ",
    event_title: " OLAY ",
    thread_keys: " ↑↓ satır · pgup pgdn kaydır · c satırı kopyala · C tümünü kopyala · esc kapat",
    copied: "kopyalandı ({})",
    copy_failed: "kopyalanamadı: {}",
    folded: "… {} agent yolunda",
    group_working: "{} çalışıyor",
    group_waiting: " ! {} bekliyor ",
    you: "sen",
    coordinating: "koordine ediyor",
    answered: "sana yanıt verdi",
    idle: "boşta",
    planning: "planlıyor",
    blocked: "BLOKE",
    research_queue: " araştırmacı sırasında",
    waiting_human: "onayını bekliyor",
    verifying: "◌ doğrulanıyor",
    thinking: "düşünüyor…",
    stopped: "■ durdu, yazınca yeniden başlar",
    researching: "araştırıyor: {}",
    columns: ["BEKLİYOR", "ÜZERİNDE", "DOĞRULANIYOR", "BİTTİ"],
    code_title: " KOD HARİTASI · {} · kim nerede çalışıyor (lease'ler) ",
    leases: "{} lease",
    shared_files: " · {} dosyada birden çok agent aynı anda çalışıyor",
    code_legend: "   ██ çalışıyor  ◌◌ doğrulanıyor  ✕✕ bloke  !! onay bekliyor",
    same_file: "⇆ aynı dosyada {} agent",
    no_code_map: "henüz ortak trunk yok: her agent kendi worktree'sinde çalışıyor, gösterilecek lease yok",
    focus_title: " ODAK · {} ",
    minutes: " · {} dk",
    no_card: "şu an bir kartı yok",
    research: "araştırma: {}",
    status: "durum  ",
    origin: "köken  ",
    lease: "lease  ",
    clean: "temiz",
    soft: "hafif kirli: hassas değişiklikleri sana gelir",
    full: "kirli: dış dünyayı okuyor, her çıktısı sana gelir",
    no_story: "Hikâye yok. Ham akış için enter.",
    story_labels: [
        "niyet",
        "varsayım",
        "karar",
        "konuşma",
        "kanıt",
        "değişiklik",
        "sen",
    ],
    trace_title: " İZ · {} · ham akış ",
    ev_plan: "{}: \"{}\" 3 karta bölündü",
    ev_assign: "{} → {}: {} {}",
    ev_assumption: "{} varsayım: {}",
    ev_blocked: "{} bloke: {}",
    ev_blocked_joined: "{} bloke: {} (zaten araştırılıyor, {} bekleniyor)",
    ev_research_opened: "{} açıldı: {}",
    ev_research_delivered: "{} raporu teslim edildi: {}",
    ev_research_needs_you: "{} raporu onay bekliyor: {}",
    ev_from_cache: "önbellekten: \"{}\" → {}",
    ev_sensitive: "{} {} → onayın gerekiyor",
    ev_merged: "{} {} birleşti ({})",
    ev_failed: "{}: merge queue'da test kırıldı, düzeltiyor",
    ev_approved: "onaylandı: {} hassas değişiklik",
    ev_approved_research: "onaylandı: {} için araştırma raporu",
    ev_rejected: "reddedildi: {}",
    ev_stopped: "{} durdu: {}",
    ev_posted: "{} panoya astı: {} {}",
    ev_claimed: "{} panodan aldı: {} {}",
    kinds: [
        "görev",
        "soru",
        "cevap",
        "bilgi",
        "devir",
        "bitti",
        "takıldı",
        "ilerleme",
        "duyuru",
        "iptal",
    ],
    st_took: "{}: {}",
    st_writing_tests: "testleri yazıyor",
    st_blocked_on: "bloke: \"{}\" araştırılmalı",
    st_research_arrived: "{} raporu geldi: {} (hafif kirli)",
    st_research_needs_you: "{} raporu yeni paket öneriyor; onayını bekliyor",
    st_from_cache: "araştırma önbellekten geldi: {}",
    st_change: "{} dosya, +{} −{}",
    st_sensitive: "hassas: {}",
    st_verified: "build ✓ · {} etkilenen test ✓ · trunk #{}",
    st_queue_failed: "merge queue: test ✕, düzeltiyor",
    st_you_approved: "sen: hassas değişikliği onayladın",
    st_you_approved_research: "sen: raporu onayladın (incelendi, kirletmez)",
    st_you_rejected: "sen: reddettin; başka bir yol deniyor",
    st_you_rejected_research: "sen: raporu reddettin; rapor olmadan devam ediyor",
    st_touched: "değiştirdi: {}",
    st_stopped: "durdu: {}",
    st_posted: "{} panoya astı: {}",
    in_research: "{} raporu \"{}\" yeni paket öneriyor → {}",
    in_reply: "{} yanıt verdi: {}",
    help_title: " YARDIM ",
    help_keys: [
        ("/", "seçili projenin liderine yaz"),
        ("@ad mesaj", "tek bir agent'a yaz"),
        ("1 2 3", "salon · pano · kod haritası"),
        ("↑ ↓  j k", "seç"),
        ("← →  h l", "projeyi katla / aç"),
        ("enter", "derine in: odak, sonra ham akış"),
        ("esc", "yukarı çık"),
        ("tab", "salon → seni bekleyenler → defter"),
        ("enter", "defterde: konuşmanın tamamı"),
        ("c  C", "satırı · konuşmayı kopyala"),
        ("<  >", "yan paneli genişlet / daralt"),
        ("a  r", "onayla · reddet (seni bekleyenlerde)"),
        ("space", "durdur / devam"),
        ("+  -", "simülasyonu hızlandır / yavaşlat"),
        ("L", "English / Türkçe"),
        ("q", "çık"),
    ],
    help_legend: [
        " ◆ lider  ● çalışıyor  ◐ soruyor  ✕ bloke  ! onayını bekliyor",
        " ◌ doğrulanıyor  ◇ araştırmacı  ○ boşta  ⇄ konuşma  ▾ ▸ proje",
        " ~ hafif kirli (rapor okudu)  ≈ kirli (dış dünyayı okuyor)",
    ],
    close: " herhangi bir tuş: kapat",
    away_title: " SEN YOKKEN · son {} dk ",
    away_counts: [
        "kart birleşti",
        "karar seni bekliyor",
        "kez bloke olundu",
        "varsayım yapıldı",
        "konuşma",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_text_can_not_drive_the_terminal() {
        let s = fill("{} said", &["\u{1b}[2J\u{1b}]0;owned\u{7}hi\r\nthere"]);
        assert!(!s.chars().any(char::is_control), "{s:?}");
        assert!(s.contains("hi") && s.contains("there"));
    }

    #[test]
    fn secret_looking_tokens_are_hidden() {
        let s = fill("{}", &["use sk-abc123 and API_KEY=xyz or ghp_456 please"]);
        assert!(
            !s.contains("sk-abc123") && !s.contains("xyz") && !s.contains("ghp_456"),
            "{s}"
        );
        assert!(s.contains("please"));
    }

    #[test]
    fn templates_take_their_arguments_in_order() {
        assert_eq!(fill("{} → {}: {}", &["a", "b", "c"]), "a → b: c");
        assert_eq!(fill("{} and {}", &["only"]), "only and ");
    }

    #[test]
    fn both_languages_have_the_same_placeholders() {
        let count = |s: &str| s.matches("{}").count();
        let pairs = [
            (EN.cards, TR.cards),
            (EN.ev_blocked_joined, TR.ev_blocked_joined),
            (EN.ev_merged, TR.ev_merged),
            (EN.st_verified, TR.st_verified),
            (EN.in_research, TR.in_research),
            (EN.in_reply, TR.in_reply),
            (EN.thread_title, TR.thread_title),
            (EN.copied, TR.copied),
            (EN.copy_failed, TR.copy_failed),
            (EN.code_title, TR.code_title),
            (EN.typing_to, TR.typing_to),
            (EN.cannot_start, TR.cannot_start),
            (EN.ev_stopped, TR.ev_stopped),
            (EN.st_touched, TR.st_touched),
            (EN.ev_posted, TR.ev_posted),
            (EN.ev_claimed, TR.ev_claimed),
            (EN.st_posted, TR.st_posted),
        ];
        for (en, tr) in pairs {
            assert_eq!(count(en), count(tr), "{en} / {tr}");
        }
    }
}
