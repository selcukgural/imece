---
title: Dosyalar
description: imece'nin diske yazdığı ya da diskten okuduğu her şey - SQLite deposu ve şeması, worktree'ler, veriyolunun soketleri, dışa aktarmalar, kopya listesi, Claude Code'un bir oturumdan tuttuğu, kilit, ve --fresh ile :forget'in sildiği.
---

# Dosyalar

imece hatırladığını tek bir dizinde, durum dizininde tutar; kalanı Claude Code'un tuttuğu yerde bırakır. Bu yollarla bir worktree'nin ne tuttuğunu, bir dışa aktarmanın nereye gittiğini, ikinci bir imece'nin neden yakındığını ya da `--fresh`'in ne sildiğini bilmek istediğinizde karşılaşırsınız. Buradaki hiçbir şey üzerinde çalıştığınız depoya yazılmaz; çalışma kopyanıza yalnızca bir değişiklikte `a`'ya bastığınızda dokunulur.

## Durum dizini

Değişken ayarlı ve mutlaksa `$XDG_STATE_HOME/imece`, yoksa `~/.local/state/imece`, o da yoksa (`HOME` yok) sistemin geçici dizini altında `imece/`. Aşağıda başka türlü işaretlenmemiş her şey orada yaşar.

| Yol | Nedir |
| --- | --- |
| `imece.sqlite` | depo: imece'nin çalıştırmalar arasında hatırladığı her şey (0600, WAL) |
| `imece.sqlite-wal`, `imece.sqlite-shm` | SQLite'ın yazma-öncesi günlüğü ve dizini; dosyayla birlikte taşıyın |
| `imece.sqlite.lock` | bir imece'nin çalışırken tuttuğu kilit (0600, `flock`) |
| `screen` | imece'nin 2026-10-30'dan önce ekran seçimlerini tuttuğu dosya; ilk görüldüğünde depoya taşınır ve silinir |
| `worktrees/<dizin>-<hash>/<ajan>` | işçi başına bir git worktree'si; `<dizin>` çalışma kopyasının dizin adı, `<hash>` yolunun sekiz onaltılık hanesi (`worktrees/kule-3f2a9c01/claude-1`) |
| `worktrees/<dizin>-<hash>-notes/<ajan>` | `CLAUDE.md` (0600, dizin 0700): ajanın Claude Code'unun kendi dizinindeki dosyadan başka okuduğu, her başlangıçta yeniden yazılır. Bkz. [aşağısı](#what-an-agent-reads) |
| `worktrees/<dizin>-<hash>-scratch/<ajan>` | yazan bir ajanın karalama dizini (0700; bir solonunki `solo-<proje numarası>`): değişikliğinin parçası olmayan notlar, günlükler ve indirilenler için. Dosya araçları oraya yazabilir (`--add-dir`), talimatları onu adlandırır, odağı gösterir. Çalıştırmalar arasında kalır; `:forget`, `--fresh` ve ajan ayrılınca (`:close`, bir yardımcının `done`'ı) silinir, aynı işe devam eden yeni bir oturumda (model geri dönüşü, yenilenen sınır, `/clear`) silinmez |
| `scripts/<sha256>.sh` | evet dediğiniz bir worktree kurulum ya da söküm betiğinin, ya da deponun kontrolünün özel kopyası, okuduğunuz baytlar (dizin 0700, dosya 0600): çalışan budur. Bkz. [worktree kurulum betiği](../concepts/projects.md#worktree-kurulum-betiği) ve [deponun kontrolü](../concepts/changes.md#deponun-kontrolü) |

## Depo

Her proje için, yalnızca size ait (SQLite açmadan önce 0600 yapılır) tek bir SQLite dosyası. Şema sürümü **13**; sürüm `meta`'dadır ve daha yeni bir imece'nin dosyası reddedilir. imece'nin açamadığı ya da yükleyemediği bir dosya (veritabanı olmayan, daha yeni bir şema, tanımadığı bir satır) reddedilir ve olduğu gibi bırakılır; mesaj dosyayı adlandırır ve nasıl sıfırdan başlanacağını söyler:

```text
/Users/me/.local/state/imece/imece.sqlite: <neden>
imece left the file as it was. To start afresh, move it somewhere else with its
/Users/me/.local/state/imece/imece.sqlite-wal and /Users/me/.local/state/imece/imece.sqlite-shm
files, if they are there; the next start makes a new one.
```

imece okuyamadığı bir dosyayı asla taşımaz, onarmaz ya da üstüne yazmaz: daha yeni bir imece onu hâlâ okuyabilir.

### Tablolar

| Tablo | Tuttuğu |
| --- | --- |
| `meta` | anahtar-değer: `schema_version`, `started_at` (dünyanın ilk başlangıcı; saat ve tikler oradan sayar), `budget` (bütçe), `saved_at` (son kayıt, "NEREDE KALMIŞTIN · 2 sa önce" için), `messages` (verilen son mesaj numarası), `setup_approved` (evet dediğiniz worktree kurulum ve söküm betikleri ile deponun kontrolü: her birinin nasıl çalıştığı, baytlarının SHA-256'sı ve gerçek yolu), ve ayarlarınız `theme`, `notify`, `motion`, `seen` (en son başlayan sürüm, yeni olanlar için) |
| `projects` | her projenin adı, lideri, kapalı olup olmadığı, çalışma kopyası yolu, sabit olup olmadığı ve `:copy`'nin listelediği yollar (`copies`, JSON) |
| `agents` | her ajanın projesi, adı, rolü, ebeveyni (yardımcının işçisi), kartı, maliyeti, kirliliği, gitmiş olup olmadığı, modeli ve bir kademenin seçip seçmediği, siz son konuştuğunuzdan beri mesaj kaynaklı turları ve kart asmaları, kendi komut listesi, son oturumuyla o oturumun harcadığı, başlangıç noktası, ve henüz başlamamış bir çatal için devam ettiği işçi (`fork_of`) |
| `cards` | her kartın projesi, başlığı, sahibi, sütunu ve ne zaman başlayıp bittiği |
| `story` | bir kartın hikâye satırları, türleriyle JSON olaylar olarak |
| `journal` | her günlük satırı (tik, proje, ajan, tür, olay JSON olarak) ve işaretlediniz mi (`marked`, kısmi bir dizinle; işaretli satırlar yaşı ne olsun bulunur) |
| `inbox` | sizi bekleyen her kalem, açık ya da kapalı, ajanı ve geldiği kartla |
| `trace` | her ajanın ham akışı, en yeni 200 satır |
| `mail` | bir ajan için okunmamış bekleyen mesajlar, numaraları ve yanıtladıklarıyla |
| `notes` | imece'nin bir ajana sonraki isteminin önündeki notları |
| `inflight` | imece kapanırken bir ajanın elindeki istemler; sonraki başlangıçta yarıda kesilmiş diye günlüğe yazılır, yeniden çalıştırılmaz |
| `posts` | her pano kartının gövdesi ve kimin astığı |
| `offers` | her kartın hangi işçilere sunulduğu |
| `changes` | her işçinin sizi bekleyen değişikliği: worktree yolu, ağacı, dosyaları, yaması ve o ağaçtaki çalıştırmalar |
| `paid` | imece'nin kendi ücretli çağrıları, güne (UTC) göre: kaç tane ve ne kadar (`:commit`) |
| `sent` | gönderdiğiniz satırlar, tick'iyle, `↑` `↓` ve `^r` (`:history`) için; en yeni 500'ü yüklenir |
| `recall`, `recall_rows` | `H` ve `:recall`'un aradığı tam metin dizini: her günlük ve iz satırının katlanmış sözcükleri (metnin kopyası olmadan), her satırın nerede olduğu ve ne zaman yazıldığı; silinen bir satır kendi satırlarını da götürür, bundan önceki bir dosya ilk açılışında bir kez dizinlenir |

Bir kayıt her adımdan ve her tuştan sonra ve çıkışta bir kez daha çalışır ve yalnızca değişeni yazar: günlük, hikâye ve iz satırları eklenir, büyüyen bir tablonun satırı yalnızca hash'i değişince yeniden yazılır. Kaydedilemeyen bir kez flaşlanır: `sonraki çalışma için kaydedilemedi: …`.

### Saklama ve VACUUM

Her canlı başlangıçta, yüklemeden sonra bir kez: en yeni **5000**'in (bir başlangıcın yüklediği, yaşı ne olsun) gerisinde kalan **90 günden** eski günlük satırları silinir, işaretlediğiniz satırlar hariç; en yeni **500**'ün gerisindeki gönderilen satırlar da gider; ve dosyanın boş sayfaları dörtte birini ya da fazlasını buluyorsa `VACUUM` onu yeniden yazar. Hikâyeler kartlarıyla kalır, bekleyen kalemler kalır (ikisi de konumla numaralanır). Başarısız olan bir temizlik yok sayılır ve sonraki başlangıçta denenir; sonucu hiçbir yerde gösterilmez. Uzun süredir kapalı projelerin hikâyeleri ve kapalı bekleyen kalemler için saklama kuralı henüz yoktur.

### Kilit

`imece.sqlite.lock` bütün çalıştırma boyunca `flock` ile tutulur, böylece dosya bir seferde tek bir imece'nindir. İkincisine, çıkış kodu 1 ile, şu söylenir: `another imece is running on /Users/me/.local/state/imece/imece.sqlite; open the other checkout from it with :open`. Yalnızca bırakılmakta olan bir kilit (çıkan bir imece) bir an beklenir. Ayarlarınız (`side`, `theme`, `lead`) o kilit olmadan kendi bağlantısıyla okunur ve yazılır; böylece tema depo alınmadan önce okunur ve canlı bir imece'nin yanındaki bir demo onu değiştirebilir.

## Worktree'ler

Her işçi `worktrees/<dizin>-<hash>/<ad>` alır: çalışma kopyanızın, yapıldığında HEAD'inizde ayrılmış (detached) bir git worktree'si. Yardımcılar işçilerinin worktree'sinde çalışır; lider çalışma kopyanızda çalışır ve worktree'si yoktur. Worktree çalıştırmalar arasında saklanır: daha eski bir commit'te temiz bulunan, ajanı yeniden başlayınca ve turlar arasında HEAD'inize taşınır; commit edilmemiş değişiklikleri ya da kendi commit'leri olan olduğu yerde kalır, ve günlük iki durumda da söyler (`claude-1: çalışma ağacı a1b2c3d yerine HEAD e4f5a6b oldu`, `… a1b2c3d noktasında kaldı, commit edilmemiş değişiklikleri var (HEAD e4f5a6b)`). Orada olan ama bu çalışma kopyasının worktree'si olmayan bir dizin reddedilir, asla yeniden kullanılmaz ya da değiştirilmez; elle silinmiş ama git'in hâlâ listelediği biri de reddedilir, çıkış yolu `git worktree prune`'dur, ve ajan o nedenle durur.

- Uygulanan bir değişiklik worktree'de bir işaret olarak commit'lenir, yazarı `imece <imece@localhost>`; böylece işçinin sonraki `done`'ı yalnızca sonra geleni gösterir ve worktree siz commit'leyince yine HEAD'inizi izler. Satırın `⎇ dal ↑kendi ↓checkout +N −M` kısmı nerede durduğunu söyler.
- Bir değişikliği okumak, bir worktree'nin nerede durduğu ve bir çalıştırmanın öncesi-sonrası ağacı worktree'nin git dizinindeki özel indeks dosyalarından geçer; adları `imece-index`, `imece-tree-<pid>-N`, `imece-stand-<pid>-N`; ajanın kendi indeksine asla dokunulmaz. Üç yollu bir uygulama yamayı önce çalışma kopyanızın indeksinin bir kopyasında, `<git dir>/imece-apply-index`, dener; sonra silinir; reddedilen bir uygulamadan sonra neyin çakıştığını bulmak da aynısını `<git dir>/imece-clash-<pid>-N` içinde yapar.
- Her worktree kendi `target/`'ında derlenir: ortak bir tane işçilere birbirlerinin değişikliklerini sırayla yeniden derletirdi.
- `:close` worktree'leri diskte bırakır; hiçbir şey onları silmez. `:copy` listelediğiniz yok sayılan yolları (`.env`, `target`) işçisinin başlangıcında onlardan yoksun bir worktree'ye koyar; birimin elverdiği yerde APFS klonu ya da reflink olarak, asla var olanın üstüne değil, başlangıç başına en çok 2 GiB; checkout'un `.worktreeinclude` dosyasının git'in yok saydığı yollar arasında saydıkları da öyle (`O_NOFOLLOW` ile, en çok 16 KiB okunur). Bkz. [değişiklikler](../concepts/changes.md) ve [projeler](../concepts/projects.md).

## Veriyolunun soketleri

Her başlangıçta imece `$TMPDIR/imece-<pid>-<hex>/` yapar (0700; var olan bir dizin reddedilir, yeniden kullanılmaz) ve içinde ajan başına bir Unix soketi bağlar, `a0.sock`, `a1.sock`, …, ajan sırasıyla, işe alım anında. Claude Code `imece mcp-relay <soket>` üzerinden bağlanır. Dizin ve soketleri imece bitince silinir, SIGTERM, SIGHUP ve SIGINT'te de. `:commit`'in tek seferlik Claude çağrısı, okunacak bir şey olmayan bu dizinde çalışır.

## Dışa aktarmalar

`:export [PATH]` açık konuşmayı ya da kartın ekranını Markdown olarak yazar: `imece-conversation-YYYYMMDD-HHMM.md` ya da `imece-card-N-YYYYMMDD-HHMM.md`; `PATH` yoksa imece'nin başlatıldığı dizine, `PATH` bir dizinse içine, yoksa `PATH` adlı dosya olarak. Orada olan bir dosyanın üstüne asla yazılmaz. İçindeki her saatin günü vardır; her satır ekranla aynı temizlikten geçmiştir (denetim karakterleri silinmiş, gizli görünen belirteçler gizlenmiş). Bkz. [komutlar](commands.md).

## imece'nin yapılandırmanızdan okuduğu

`$XDG_CONFIG_HOME/imece/` içinde, ya da `~/.config/imece/` (göreli bir `XDG_CONFIG_HOME` yok sayılır):

- `keys`: kendi tuşlarınız, açılışta ve değiştikçe yeniden okunur. Bkz. [klavye](../screen/keyboard.md).
- `push`: `:notify push`'un bekleyeni nereye gönderdiği (ntfy sunucusu, topic ve bir token'ın dosyası); push seçildiğinde ve her başlangıçta okunur. Bkz. [`:notify`](commands.md#notify-offbelltextpush).
- `instructions.md`: her proje için kendi talimatlarınız, tümüyle, yalnızca imece'nin ajanları için; `~/.claude/CLAUDE.md` ise her Claude Code oturumu içindir. Her ajanın başlangıcında okunur, bir değişiklik bir sonrakinde; yalnız düz bir dosya: bir boru ya da aygıt atlanır. Bkz. [aşağısı](#what-an-agent-reads).

## imece'nin depodan okuduğu

- Ajanın kendi dizininde `CLAUDE.md` yoksa çalışma kopyasının tepesindeki `CLAUDE.md`, o da yoksa `AGENTS.md`. Bkz. [aşağısı](#what-an-agent-reads).
- `.imece/check`: deponun kontrolü; baytlarına evet dediğinizde bir işçinin worktree'sinde oradan okunan her değişiklikte çalışır; `.imece/worktree-setup.sh` ve `.imece/worktree-teardown.sh` aynı yolla, worktree yapılırken ve kaldırılmadan önce. Bkz. [değişiklikler](../concepts/changes.md#deponun-kontrolü) ve [projeler](../concepts/projects.md#worktree-kurulum-betiği).
- Git: çalışma kopyasının tepesi, HEAD, `:copy` için `git check-ignore`, `.worktreeinclude`'un neye uyduğu için `git ls-files --others --ignored --directory --exclude=…` (bir işçinin başlangıcında), `:commit` için `git diff --cached` ve en yeni sekiz konu (`--no-ext-diff --no-textconv` ile, böylece sizin bir programınız çalışmaz), ve bir mesajda dosya adı tamamlamak için `git ls-files` (izlenenler ve izlenmeyen ama yok sayılmayanlar; ilk 20.000): projenin açılışında, uyguladığınız her değişiklikten sonra ve ajanlarından biri tur ortasındayken her dakika.
- `--theme FILE` ya da `:theme FILE` ile adlandırılan, Ghostty ya da Kitty biçiminde bir renk dosyası; mutlak yolu tema olarak hatırlanır.

Başka bir şey yok: deponun hiçbir dosyası imece tarafından yazılmaz.

## Bir ajanın okuduğu {#what-an-agent-reads}

`standard` ve `strict` altında `--restricted` hiçbir `CLAUDE.md` yüklemezdi. imece, Claude Code'un kendi yükleyicisini iki dizin için yeniden açar (`CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`, `--add-dir`):

- **Ajanın kendi dizini**, worktree'si ya da lider için çalışma kopyası: `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules` ve dizin içindeki `@import`'ları. Bir işçi bunları worktree'sinde olduğu gibi okur; yani `CLAUDE.md`'deki bir değişiklik commit edilip worktree'si taşınınca ona ulaşır. Oradaki hook'lar, ayarlar, skill'ler ve komutlar dışarıda kalır.
- **Yukarıdaki notlar dizini**: imece'nin her başlangıçta yazdığı bir dosya, tümüyle ve bu sırayla: `~/.claude/CLAUDE.md`'niz (önce `$CLAUDE_CONFIG_DIR`), `instructions.md` ve yükleyici ajanın kendi dizininde hiçbir şey bulamayacaksa çalışma kopyasının `CLAUDE.md`'si ya da `AGENTS.md`'si (git dosyayı yok sayıyordur ya da işçi onun olmadığı bir yerden başlamıştır). Oradaki bir dosya kopyadır, bu yüzden `~/.claude/CLAUDE.md`'nizdeki bir `@import` izlenmez. Çalışma kopyasının dışına çıkan bir bağlantı okunmaz.

Bu dizinlerin dışına çıkan bir `@import` yüklenmez. `CLAUDE.local.md` bu seviyelerde okunmaz. `--confine open` altında bütün kurulumunuzu ve ajanın dizinini Claude Code kendisi okur; notlar dizininde yalnızca onun bulamayacağı kalır: `instructions.md` ve deponun yedek dosyası. `:commit` ve `:review` not okumaz: tek bir diff hakkında, token başına yanıt verirler.

## Claude Code'un tuttuğu

Bir oturum, Claude Code'un tuttuğundan sürdürülür; imece yalnızca kimliği hatırlar (`agents.session`). Bunlar Claude Code'un dosyalarıdır, imece'nin değil.

| Nerede | Notlar |
| --- | --- |
| `$CLAUDE_CONFIG_DIR` ya da `~/.claude`, `projects/<slug>/<id>.jsonl`; slug, ajanın dizininin harf ya da rakam olmayan her karakteri `-` yapılmış hali | transkript; dosya oradayken, aynı dizinden, oturum sürdürülebilir. Öteki izler (`debug`, `file-history`, `todos`) aranmaz |

## `--fresh` ve `:forget`'in sildiği

İkisi de bir ajana oturumunu unutturur; sonraki mesaj yeni bir oturum başlatır, kaldığı yer anlatılır. `--fresh` başlangıçta hatırlanan her ajan için, `:forget @NAME` biri için yapar. Giden, kendi iş parçacığında (iz nasıl gittiğini söyler: `forgot session <id>: its transcript was removed`):

Silinen: yukarıdaki transkript dosyası (zaten gitmişse `it had no transcript left`).

Bir yola ya da Claude Code'a yalnızca UUID biçiminde bir kimlik ulaşır. Henüz durması istenmiş bir oturuma önce gitmesi için 4 sn verilir. Ajanın notlar dizini ve karalama dizini de gider; hemen kenara alınır, böylece hemen ardından başlayan oturum yenisini alır (`its scratch directory was removed`). imece'nin kendisinden başka hiçbir şey silinmez: ajan, kartları, günlük ve worktree kalır. Bir oturum değiştirildiğinde de silinir: bir model geri dönüşü, sınırına çarpınca yenilenen bir oturum. Bkz. [oturumlar](../concepts/sessions.md).

## Yazılmayanlar

- Günlük dosyası ve iz dosyası yok: ham akış `trace` tablosudur (ajan başına 200 satır), günlük `journal` tablosudur.
- `!cmd` çıktısı terminalinizin ana ekranında kalır; sonu (`O` için, proje başına son 64 KiB) yalnızca bellekte tutulur ve imece bitince gider.
- `:commit`'in çağrısı `--no-session-persistence` ile çalışır: `~/.claude/projects` altında hiçbir şey, sürdürülecek bir şey yok.
- Demo, ekran ayarlarınız (`side`, `theme`, `lead`) dışında depoya hiçbir şey yazmaz.
- Başlıkta gösterilen kullanım pencereleri saklanmaz; bir oturum ilk turunda yeniden söyler.

## Ayrıca bkz.

- [Oturumlar](../concepts/sessions.md): neyin hatırlandığı ve bir ajanın nasıl uyandığı
- [Komut satırı](cli.md): `XDG_STATE_HOME`, `--fresh`, `--theme`
- [Komutlar](commands.md): `:export`, `:copy`, `:forget`, `:open`, `:close`
- [Güvenlik](../concepts/security.md): çalışma kopyasına neden yalnızca `a` dokunur
- [Sorun giderme](../troubleshooting.md)
