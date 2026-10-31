---
title: Mesaj türleri ve araçlar
description: imece'nin veriyolundaki on zarf türü ve imece'nin her birinde yaptığı, mesaj numaraları ve yanıtlar, sınırlar, bir ajanın imece'nin MCP sunucusundan aldığı her araç rol başına alanlarıyla, ve bir ajana verilen talimatlar.
---

# Mesaj türleri ve araçlar

Veriyolu (bus) ajanların birbirine ve size ulaştığı yoldur: tek bir araç, `send(to, type, body)`, imece'nin kendisinin MCP üzerinden sunduğu, ajan başına bir Unix soketi. Bir mesaj, türü olan bir zarf ve serbest metin bir gövdedir. Yönlendirmeyi, uyandırmayı ve maliyeti yalnızca zarf sürer; gövdeyi imece asla yorumlamaz ve okuyan ajana onu sizin değil bir takım arkadaşının yazdığı söylenir. Veriyoluyla günlükte (`lead → claude-1: görev #3 Fix the parser`), bir konuşmada ve bir kartın hikâyesinde karşılaşırsınız.

## Zarf

`send` şunları alır: `to` (bir takım arkadaşının adı, ya da kişi için `you`), `type` (aşağıdaki ondan biri), `body` (en çok 4000 karakter) ve isteğe bağlı `re` (bu mesajın yanıtladığı mesajın numarası). Gönderen, çağrının geldiği sokettir; ajanın söylediği bir şey asla değil. Bir mesaj yalnızca aynı projenin bir takım arkadaşına gider; `no teammate is called "codex-9"; the team is lead, claude-1, claude-2` takımı sayar.

| Tür | Alıcıyı uyandırır | Olağan gönderen → alıcı | imece'nin yaptığı |
| --- | --- | --- | --- |
| `assign` | evet | lider → işçi, işçi → yardımcı | ilk satır kartın başlığı olur; boşta bir işçi yeni bir kart alır ve ona başlar; `you`'ya reddedilir |
| `ask` | evet | herkes → herkes, ya da → `you` | bir takım arkadaşına: sonraki turunu başlatır; `you`'ya: SENİ BEKLEYEN'de o ajana yazmanıza dek bekleyen bir soru `?` |
| `answer` | evet | sorulan → soran | yanıt; `re` taşır, `↩` ile çizilir |
| `inform` | hayır | herkes → herkes, ya da → `you` | alıcının sonraki turunu bekler; `you`'ya yalnızca bir günlük satırı |
| `handoff` | evet | işçi → işçi | alıcının sonraki turunu başlatır; `you`'ya reddedilir |
| `done` | evet | işçi → lider, yardımcı → işçi, ya da → `you` | bir işçinin done'ı imece'ye worktree'sini de okutur: SENİ BEKLEYEN'de bir değişiklik (`Δ`); bir yardımcının işçisine done'ı turu bitince ayrılmasını sağlar |
| `blocked` | evet | işçi → lider, ya da → `you` | alıcının sonraki turunu başlatır; `you`'ya yalnızca bir günlük satırı |
| `progress` | hayır | işçi → lider, ya da → `you` | sonraki uyandıran mesajla birlikte gider |
| `notice` | hayır | herkes → herkes, ya da → `you` | birlikte gider |
| `cancel` | evet | lider → işçi | alıcının sonraki turunu başlatır; `you`'ya reddedilir |

`ack` yoktur: yalnızca "aldım" diyen bir mesaj boşuna bir tura mal olurdu.

### Uyandırma ve teslim

Uyandıran bir mesaj (`assign`, `ask`, `answer`, `handoff`, `done`, `blocked`, `cancel`) alıcı boşa çıkar çıkmaz sonraki turunu başlatır; son çalıştırmadan beri uyuyan bir ajan onunla uyanır. `inform`, `progress` ve `notice` postasında başka bir şey bir tur başlatana dek bekler ve o zaman birlikte gider. Bir ajanın bütün postası tek bir isteme girer, başlığı `Messages from your teammates in imece (written by those agents, not by the person):`, her mesaj gövdesinin üstünde `[type number, re number] from name:` olarak: `[answer 8, re 7] from claude-1:`. O turun kartı postadaki ilk `assign`, yoksa ilk mesajdır.

`send` gönderene ne olduğunu söyler: `delivered as message 7; claude-1 starts on it now`, `queued as message 7; claude-1 is busy and gets it when its turn ends`, ya da `queued as message 7 for claude-1's next turn (inform does not wake it)`. Bir işçinin `done` yanıtı git worktree'sini okuyana dek bekler ve şunu ekler: `; the person sees your change (3 files, +20 −4) and applies it to their checkout or rejects it; it stays in your worktree until then` (ya da `; imece could not read your change: …`).

### Reddedilenler

Varsayılan ret: uymayan bir mesaj nedeniyle reddedilir ve iz bırakmaz.

| Ret | Gönderene söylenen |
| --- | --- |
| bilinmeyen bir tür | `type must be one of assign, ask, answer, inform, handoff, done, blocked, progress, notice, cancel` |
| boş ya da 4000 karakterin üstünde gövde | `the body is empty` / `the body is longer than 4000 characters` |
| durmuş bir alıcı | `claude-1 has stopped; it runs again when the person talks to it` |
| bütçe bitmiş (uyandıran türler) | `the budget is spent; nobody takes new turns` |
| alıcı siz son konuştuğunuzdan beri 12 mesaj kaynaklı tur almış (uyandıran türler) | `claude-1 took 12 turns on messages since the person last spoke; stop and tell the person what you need` |
| alıcının 20 okunmamış mesajı var | `claude-1 has 20 unread messages; wait for it` |
| `you`'ya `assign`, `handoff` ya da `cancel` | `assign is not for the person; ask them, or tell them (inform, done, blocked)` |
| bozuk bir `re` | aşağıda |

12 tur ve 10 kart sayaçları siz herhangi bir ajana her yazdığınızda yeniden başlar: ajanlar sizsiz birbirlerini sonsuza dek meşgul edemez.

### Size mesajlar

`to: you` kişiye ulaşır. Bir `ask` SENİ BEKLEYEN'de bir soru olur (`claude-1 sana soruyor: Which parser?`); açık bir sorusu varken boşta duran ajanın satırı `? → you` okunur ve soru o ajana yazdığınızda kapanır, bu da sonraki turunu başlatır. Gönderen şunu duyar: `delivered; the person sees it under WAITING FOR YOU and answers when they can, which starts your next turn. Until then, end your turn or go on with what does not depend on the answer.` Öteki her tür yalnızca bilgilendirir: günlükte ve konuşmada bir satır (`noted; the person sees it in the journal. Nothing wakes them: a question goes as type ask.`); bir işçinin size `done`'ı değişikliğini her done gibi gösterir. Size mesajlar numara ve `re` almaz, hiçbir sınıra girmez ve para etmez. Bkz. [onaylar ve sorular](../concepts/approvals-and-questions.md).

### Kartlar ve veriyolu

Boşta bir işçiye bir mesaj, ilk satırıyla adlandırılmış bir kart açar (varsa bir `assign`'ın ilk satırı); kart, başlattığı tur ajanın yanıtıyla bitince kapanır. Bir işçinin liderine `done`'ı liderin kartını kapatmaz: liderin kendi turu kapatır. Bir yardımcının işçisine `done`'ı ayrılacağını işaretler; o tur bitince ayrılır, böylece turun maliyeti yine bildirilir. Bkz. [mesajlar ve pano](../concepts/messages-and-board.md) ve [kartlar](../screen/cards.md).

## Mesaj numaraları ve yanıtlar

Bir ajanın başka bir ajana gönderdiği her mesaj veriyolunda bir numara alır: 1'den başlayarak bütün dünya boyunca sırayla, çalıştırmalar arasında saklanır. `send` onu yanıtında söyler ve posta onunla başlıklanır. `re` bu mesajın yanıtladığını adlandırır; araç bunu bir `ask`'a `answer`'da ve bir `assign` için `done`'da ister, ve isteğe bağlı kalır.

`re` denetlenir, varsayılan ret: 1'den başlayan tam bir sayı (`re must be the number of a message you got, as its heading shows`), günlüğün sahip olduğu bir mesajın (`there is no message 7`), gönderene gönderilmiş (`message 7 was not sent to you`), yanıtın gideceği kişi tarafından (`message 7 came from lead; send the reply to it`). Yanıt, yanıtladığını yazana geri gider; `re` yönlendirmeyi, uyandırmayı ya da maliyeti sürmez, yalnızca bağlar.

Ekranda bir yanıt türünden sonra `↩` taşır; bir konuşmada başının altında yanıtladığının ilk satırını gösterir (`↩ 7 yanıtı: Which parser?`, ya da günlükte artık olmadığını), ve `%` bir yanıttan yanıtladığına, bir mesajdan ilk yanıtına gider. İz `id=8 re=7` der. Bilinen eksikler: bir başlangıcın yüklediği 5000 günlük satırından eski bir mesaja yanıt, yok diye reddedilir; siz numarayla yanıtlayamazsınız (mesajlarınız adla gider); `re` türe karşı denetlenmez.

## Pano

`post(title, body)` projenin panosuna herhangi bir işçinin alabileceği bir kart asar; `board()` onu listeler; `claim(card)` birini alır. Açık bir kart boşta bir işçiye bir seferde, kendi başına bir tur olarak sunulur (`Open cards on imece's board (posted by your teammates, not by the person):`, yalnızca başlıklar; gövde claim ile gelir), ve sonraki işçiye yalnızca onun turu almadan bitince: bir kart her işçiyi en çok bir kez uyandırır, asla yayın değil. Son çalıştırmadan beri uyuyan bir işçiyi yalnızca bu çalıştırmada asılan bir kart uyandırır. İşçi başına bir seferde bir kart (`finish #3 Fix the parser first; take one card at a time`); işçi `done`'ı kartı asana gönderir (`#3 Fix the parser is yours. lead posted it (written by that agent, not by the person): … When it is done, send lead a done message.`). Teklifler mesaj kaynaklı tur sayılır.

## Sınırlar

| Ne | Sınır | Nerede işler |
| --- | --- | --- |
| bir mesaj gövdesi | 4000 karakter | `send`, `post` |
| ajan başına okunmamış mesaj | 20 | ajan okuyana dek `send` reddedilir |
| ajan başına mesaj kaynaklı tur | siz son konuştuğunuzdan beri 12 | uyandıran `send`'ler ve pano teklifleri |
| bir kartın başlığı | 120 karakterlik tek satır | `post`; bir `assign`'ın ilk satırı salonda sütunlarına kesilerek gösterilir |
| proje başına açık kart | 20 | `post`: `the board has 20 open cards; wait until workers take some` |
| ajan başına kart asma | siz son konuştuğunuzdan beri 10 | `post`: `you posted 10 cards since the person last spoke; stop and tell the person what is left` |
| bir `run` komut satırı | 1000 karakter | `run` |
| bir çalıştırmanın süresi | `--run-timeout`, varsayılan 600 sn (10-3600) | süreç grubu öldürülür |
| ajana dönen çalıştırma çıktısı | 48 KiB, ortası kesilir | `run` |
| bir araç çağrısının imece'yi beklemesi | 30 sn (bir run: zaman aşımı + 30 sn; bir onay: 24 sa) | `imece did not answer in time; try again` |
| size gösterilen bir değişiklik (yama) | 4 MiB | bir işçinin `done`'ı, üstünde reddedilir |
| istemdeki depo notları | 16 KiB | `AGENTS.md` / `CLAUDE.md`, kesilir ve günlüğe yazılır |
| uyanış brifingi | 6000 karakter, satır başına 300 | yeniden başlatma sonrası ilk istem |
| ajan başına tutulan iz satırı | 200 | ham akış |

## Araçlar

imece'nin MCP sunucusunun adı `imece`'dir; araçları bir Claude oturumuna `mcp__imece__send`, bir Gemini oturumuna `mcp_imece_send` olarak ulaşır. Her ajanın kendi soketi vardır (`TMPDIR` altında özel bir dizinde `a<N>.sock`), işe alım anında, ajan var olmadan önce bağlanır; satıcısının CLI'ı yalnızca bayt kopyalayan `imece mcp-relay <soket>`'i başlatır. Her oturum tam olarak imece'nin sunucusunu, bağlı, ve tam olarak bu araçları bildirmelidir, yoksa durur (kapalıya düşer). Her aracın girdisi adlandırmadığı özellikleri reddeder (`additionalProperties: false`).

| Araç | Lider | İşçi | Yardımcı |
| --- | --- | --- | --- |
| `send` | evet | evet | evet |
| `post` | evet | evet | evet |
| `board` | evet | evet | evet |
| `run` | evet (okuma komutları) | evet | evet |
| `claim` | — | evet | — |
| `hire` | işçiler | yardımcılar | — |
| `approve` | yalnızca Claude oturumları; Claude Code'un kendisi çağırır, modelden gizli | | |

### `send`

> Sends a message to a teammate in imece, or to the person (to: you). To a teammate it arrives as the start of their next turn: assign, ask, answer, handoff, done, blocked and cancel start that turn as soon as they are free; inform, progress and notice wait until something else does. To the person, an ask waits on their screen until they answer (which starts your next turn); the rest only informs them.

| Alan | Tür | Zorunlu | Anlamı |
| --- | --- | --- | --- |
| `to` | dize | evet | takım arkadaşının adı, ya da kişi için `you` |
| `type` | dize, ondan biri | evet | zarf |
| `body` | dize, en çok 4000 | evet | `assign` için: ilk satır kartın başlığı olur (ekran ilk 20 karakterini işçinin yanında gösterir, bu yüzden her görevin paylaştığı bir fiille değil, onu öteki kartlardan ayıranla başlamalı); sonra görev, dokunduğu dosyalar ve ne zaman bittiği |
| `re` | tam sayı ≥ 1 | hayır | yanıtladığı mesajın numarası |

### `post`

> Puts a card on imece's board for any worker to take. Use it for work nobody has yet; for work one teammate should do, send assign.

| Alan | Tür | Zorunlu | Anlamı |
| --- | --- | --- | --- |
| `title` | dize, tek satır, en çok 120 | evet | ekran ilk 20 karakterini gösterir |
| `body` | dize, en çok 4000 | evet | görev: ne yapılacak, hangi dosyalar, ne zaman bitti |

`posted as #4; an idle worker will be offered it` diye yanıtlar.

### `board`

> Lists the cards on imece's board: open ones, and who took the others.

Alan yok. `Open:` ve `Taken:` listeleriyle yanıtlar (`#4 Add tests (posted by lead)`, `#2 Rename (taken by claude-1)`, `#1 Docs (done by claude-2)`), ya da `the board is empty`.

### `claim`

> Takes an open card from imece's board; the answer is its task. One card at a time.

| Alan | Tür | Zorunlu | Anlamı |
| --- | --- | --- | --- |
| `card` | tam sayı ≥ 1 | evet | kartın numarası, `#` olmadan |

Yalnızca işçiler (`only workers take cards; assign or post instead`). Redler: `there is no such card on the board; call board to see it`, `#4 was taken by claude-2; pick another`.

### `hire`

Liderin kopyası: *Adds a worker to your team in imece: a coding agent in its own git worktree of the repository. The answer names it; then give it work with send (type assign). Hire for work that can run at once, not in advance: one worker takes several tasks in turn.* Bir işçinin kopyası kendi worktree'sinde, kendi işinin yanında yürüyebilecek bir parça için, değiştirmediği dosyalarda bir yardımcı alır; yardımcı `done` gönderir ve ayrılır.

| Alan | Tür | Zorunlu | Anlamı |
| --- | --- | --- | --- |
| `tier` | `cheap`, `standard`, `strong` | hayır (varsayılan `standard`) | cheap: küçük, iyi tanımlı mekanik iş (yeniden adlandırmalar, bilinen davranışın testleri, belgeler); standard: olağan düzeltmeler ve özellikler; strong: kodu enine kesen tasarım, zor hata ayıklama, inceleme |
| `vendor` | PATH'teki satıcılardan biri | hayır (varsayılan: işe alanın kendisi) | hangi kodlama ajanı |

`claude-1 joined (claude, haiku). 3 more may be hired` (ya da `the default model`) diye yanıtlar. Redler: `the team is full: 4 workers, the most the person allowed (--agents 4); give the work to one of them`; `the person runs you without workers (--agents 0); answer from what you read`; `you have 2 helpers, the most the person allowed (--helpers 2); wait for one to send done`; `the person runs you without helpers (--helpers 0); do it yourself`; `there is no vendor "cursor" here; you can hire claude, codex`; `tier must be cheap, standard or strong`. İşçiler `<satıcı>-N`, yardımcılar `<işçi>.N` adını alır; numara alınmış adları atlar. Bkz. [takım](../concepts/team.md).

### `run`

> Runs one program in your directory and gives back its exit code and output (stdout and stderr together; lines of passing tests are left out, very long output is cut in the middle). No shell: one program and its arguments, no pipes, redirections or variables; quote an argument with spaces. Programs: …. A run is killed after 600 s. Use it to check your work (build, lint, test) before you say it is done.

| Alan | Tür | Zorunlu | Anlamı |
| --- | --- | --- | --- |
| `command` | dize, en çok 1000 | evet | program ve argümanları tek satır olarak: `cargo test --workspace` |

Satır bir kabuğun kelimeleri böldüğü gibi bölünür (tırnaklar, ters bölüler) ve bir kabuğun başka türlü okuyacağı şey reddedilir: ``| & ; < > ( ) $ ` # * ? ~ {``, ikinci bir satır. Çalışabilecekler: bağımlılıkları değiştiren ya da dışarıya uzanan komutlar olmadan `cargo` (`add`, `install`, `login`, `logout`, `owner`, `publish`, `remove`, `rm`, `uninstall`, `yank`); lider için dosyaları değiştirenler de olmadan (`clean`, `fix`, `--check` olmadan `fmt`, `init`, `new`); yalnızca `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status` ile `git`, komuttan önce seçenek olmadan ve sayfalayıcı başlatan ya da dosya yazan hiçbir seçenek olmadan (`-O…`, `--open-files-in-pager…`, `--output…`); ve işçilerle yardımcılar için `--run` ile adlandırdığınız programlar. Ajanın kendi dizininde, temizlenmiş bir ortamla, kendi süreç grubunda çalışır; süre dolunca ve imece ile birlikte öldürülür. Yanıt `exit code 0`, `exit code 101 (failed)`, `killed after 600 s: it did not finish in time` ya da `killed by a signal` ile başlar, sonra düzeltilmiş çıktı (renkler soyulmuş, geçen test satırları atılmış, yinelenenler katlanmış, 48 KiB'den sonra ortası kesilmiş), ya da `, no output`. Her worktree kendi `target/`'ında derlenir.

Bir işçinin ya da yardımcının çalıştırması, worktree'sinin öncesinde ve sonrasında sahip olduğu ağaçla birlikte tutulur; ikisi aynıysa ve sonraki bir `done` size tam o ağacı verirse değişiklik `✓ bu ağaçta temiz geçti: cargo test` der (ya da `✗ başarısız`, ya da `bu ağaçta çalıştırma yok`). Kanıt olarak yalnızca imece'nin kendi `run`'ı sayılır; bir satıcının kabuğu sayılmaz, çünkü imece hangi ağaçta çalıştığını görmez. Bkz. [değişiklikler](../concepts/changes.md).

### `approve`

> Asks the person whether a tool call may run.

| Alan | Tür | Zorunlu |
| --- | --- | --- |
| `tool_name` | dize | evet |
| `input` | nesne | evet |
| `tool_use_id` | dize | hayır |

Yalnızca bir Claude ajanının soketinde ve yalnızca `standard` ile `open` altında: Claude Code kendi kurallarının soracağı bir araç çağrısından önce bunu kendisi çağırır (`--permission-prompt-tool mcp__imece__approve`) ve modelden gizler. Rutin bir komut (tek satır, bir program ve argümanları, rolün `run` politikasınca ve `--run` listenizce izinli) hemen izin alır, izde `allowed on its own` satırıyla; başka her şey SENİ BEKLEYEN'de bütünüyle gösterilen bir `⚑` kalemidir ve çağrı `a` ya da `r`'niz için bir güne dek bekler. Bir ret `The person refused this. Do not try it another way; if you need it, ask them …` diye yanıtlanır. Codex'in ve Gemini'nin soruları kendi hatlarından gelir ve araçsız aynı biçimde yönlendirilir. Bkz. [onaylar ve sorular](../concepts/approvals-and-questions.md).

## Bir ajana anlatılanlar

Her oturum, bu sırayla şunları alır:

1. **Rolü**, sistem istemi olarak (Claude: `--append-system-prompt`, Claude Code'un kendisinin üstüne eklenir; Codex: iş parçacığının geliştirici talimatları; sürdürülen bir iş parçacığı ilk başlangıcındakileri korur; Gemini: özel dizinindeki, her başlangıçta yeniden yazılan, dosyalarının dizinini adlandıran bir satırla biten bir `GEMINI.md`). Liderinki, dizinindeki depoda bir kodlama ajanları takımına liderlik ettiğini, sizin onunla konuştuğunuzu, nasıl işe aldığını (kademeyle, eldeki iş için, önceden değil; bir işçi sırayla birkaç görev alır), her işçiye payını `send` türü `assign` ile ve elinden geldiğince ayrı dosyalar vermesini, çoğu görevin dokunduğu bir dosyanın bir seferde bir işçiye ait olduğunu, herhangi bir işçinin alabileceği işin panoya gidebileceğini, bir işçinin `done`'ının size yalnızca sizin uygulayacağınız değişikliğini gösterdiğini, kendisinin asla dosya değiştirmediğini, size kısa yanıt vermesini ve görev ortasındaki bir soruyu `ask` olarak göndermesini söyler. Bir işçininki kendi worktree'sinde çalıştığını, commit etmemesini, görevi verene özetle `done` göndermesini, `done`'ının size worktree diff'ini gösterdiğini (bu yüzden önce bitirip son düzenlemesinden sonra `run` aracıyla denetlemesini; kendi kabuğundaki bir denetim orada görünmez), tahmin yerine `blocked` ya da `ask` göndermesini (önce lidere; size yalnızca yalnız sizin karar verebileceğinizi), imece'nin ona kart sunabileceğini, görevi dışındaki işin panoya gittiğini ve yardımcıları nasıl aldığını söyler. Bir yardımcınınki kimin worktree'sinde ve dosyalarında çalıştığını, commit etmemesini, işçisine `done` gönderip ayrılmasını ve herkesten önce işçisine sormasını söyler.
2. **Ne yapabildiğini**, satıcı, rol ve `--confine` düzeyi başına bir cümle: örneğin `standard` altındaki bir Claude işçisine burada dosya okuyup değiştirebileceği, kabuğunda komut çalıştırıp web'e erişebileceği; derlemeyi, testleri ya da git'in okuma komutlarını çalıştıran, argümanlarıyla tek bir programın hemen çalıştığı ve çoğu ötekinin sizin onayınızı beklediği; bir reddin sizin kararınız olduğu, hata olmadığı söylenir. `strict` altındaki bir lidere dosya okuyabileceği ama değiştiremeyeceği ve `run` aracının çalışma kopyasında denetimler çalıştırdığı söylenir.
3. **Deponun kodlama ajanları için notları**: `AGENTS.md`, yoksa `CLAUDE.md`, en çok 16 KiB, başlığı `The repository's notes for coding agents, from its CLAUDE.md:`. Satıcılar depodan hiçbir şey yüklenmesin diye başlatılır (hook yok, ayar yok), bu da bunları düşürürdü; o yüzden imece onları kendi geçirir; Codex `AGENTS.md`'yi kendi okur ve bu yolla yalnızca bir `CLAUDE.md` alır. Sınırı aşan bir dosya `[cut: the file goes on]` ile kesilir ve günlük bunu söyler.
4. **imece'den notlar**, varsa bir istemin önünde, başlığı `From imece (not from the person or a teammate):`: değişikliğini uyguladığınız (`Your worktree is committed at <mark> as a mark of that; your files are as you left them. Go on from here with the next task.`), bir parçasını uyguladığınız, ya da reddettiğiniz (`It stays in your worktree, not applied; they say why themselves. Do not send it again as it is.`), projenin yeniden adlandırıldığı, ve yeniden başlatma sonrası uyanışta brifing: kartı ve hikâyesinin son 12 satırı, yarıda kesilen bir tur (yeniden çalıştırılmaz), kaç mesajın beklediği, worktree'sinin commit edilmemiş değişiklikleri, panonun açık kartları.
5. **Postası** ve **pano teklifleri**, yukarıdaki gibi, her biri sizin değil bir ajanın yazdığını söyleyerek. Sizin mesajlarınız kendileri olarak gelir.

`/` ya da `$` ile başlayan bir istem satıcı komutu olarak çalışırdı, bu yüzden `Text:` ardından satır olarak gönderilir. Bkz. [imece nasıl çalışır](../concepts/how-imece-works.md) ve [oturumlar](../concepts/sessions.md).

## Ayrıca bkz.

- [Mesajlar ve pano](../concepts/messages-and-board.md): zarfın ve panonun arkasındaki düşünceler
- [Takım](../concepts/team.md): roller, işe alım, kademeler
- [Değişiklikler](../concepts/changes.md): bir işçinin `done`'ının neye dönüştüğü
- [Onaylar ve sorular](../concepts/approvals-and-questions.md): `⚑` sorular ve `?` sorular
- [Dosyalar](files.md): soketler, worktree'ler ve depo
