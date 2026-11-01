---
title: Dosyalar
description: imece'nin diske yazdığı ya da diskten okuduğu her şey - SQLite deposu ve şeması, worktree'ler, Gemini'nin özel dizini, veriyolunun soketleri, dışa aktarmalar, kopya listesi, satıcıların bir oturumdan tuttuğu, kilit, ve --fresh ile :forget'in sildiği.
---

# Dosyalar

imece hatırladığını tek bir dizinde, durum dizininde tutar; kalanı satıcıların tuttuğu yerde bırakır. Bu yollarla bir worktree'nin ne tuttuğunu, bir dışa aktarmanın nereye gittiğini, ikinci bir imece'nin neden yakındığını ya da `--fresh`'in ne sildiğini bilmek istediğinizde karşılaşırsınız. Buradaki hiçbir şey üzerinde çalıştığınız depoya yazılmaz; çalışma kopyanıza yalnızca bir değişiklikte `a`'ya bastığınızda dokunulur.

## Durum dizini

Değişken ayarlı ve mutlaksa `$XDG_STATE_HOME/imece`, yoksa `~/.local/state/imece`, o da yoksa (`HOME` yok) sistemin geçici dizini altında `imece/`. Aşağıda başka türlü işaretlenmemiş her şey orada yaşar.

| Yol | Nedir |
| --- | --- |
| `imece.sqlite` | depo: imece'nin çalıştırmalar arasında hatırladığı her şey (0600, WAL) |
| `imece.sqlite-wal`, `imece.sqlite-shm` | SQLite'ın yazma-öncesi günlüğü ve dizini; dosyayla birlikte taşıyın |
| `imece.sqlite.lock` | bir imece'nin çalışırken tuttuğu kilit (0600, `flock`) |
| `screen` | imece'nin 2026-10-30'dan önce ekran seçimlerini tuttuğu dosya; ilk görüldüğünde depoya taşınır ve silinir |
| `worktrees/<dizin>-<hash>/<ajan>` | işçi başına bir git worktree'si; `<dizin>` çalışma kopyasının dizin adı, `<hash>` yolunun sekiz onaltılık hanesi (`worktrees/kule-3f2a9c01/claude-1`) |
| `worktrees/<dizin>-<hash>-gemini/<ajan>` | bir Gemini ajanının özel dizini (projenin lideri için, proje ne adlandırılsa da `lead`); `GEMINI.md` ve `policy/imece.toml` tutar |

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
| `meta` | anahtar-değer: `schema_version`, `started_at` (dünyanın ilk başlangıcı; saat ve tikler oradan sayar), `budget` (bütçe), `saved_at` (son kayıt, "NEREDE KALMIŞTIN · 2 sa önce" için), `messages` (verilen son mesaj numarası), ve ayarlarınız `side` (yan panelin genişliği), `theme`, `lead` (birincil satıcı) |
| `projects` | her projenin adı, lideri, kapalı olup olmadığı, çalışma kopyası yolu, sabit olup olmadığı ve `:copy`'nin listelediği yollar (`copies`, JSON) |
| `agents` | her ajanın projesi, adı, satıcısı, rolü, ebeveyni (yardımcının işçisi), kartı, maliyeti, kirliliği, gitmiş olup olmadığı, modeli ve bir kademenin seçip seçmediği, siz son konuştuğunuzdan beri mesaj kaynaklı turları ve kart asmaları, kendi komut listesi, ve son satıcı oturumuyla o oturumun harcadığı |
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

Bir kayıt her adımdan ve her tuştan sonra ve çıkışta bir kez daha çalışır ve yalnızca değişeni yazar: günlük, hikâye ve iz satırları eklenir, büyüyen bir tablonun satırı yalnızca hash'i değişince yeniden yazılır. Kaydedilemeyen bir kez flaşlanır: `sonraki çalışma için kaydedilemedi: …`.

### Saklama ve VACUUM

Her canlı başlangıçta, yüklemeden sonra bir kez: en yeni **5000**'in (bir başlangıcın yüklediği, yaşı ne olsun) gerisinde kalan **90 günden** eski günlük satırları silinir, işaretlediğiniz satırlar hariç; en yeni **500**'ün gerisindeki gönderilen satırlar da gider; ve dosyanın boş sayfaları dörtte birini ya da fazlasını buluyorsa `VACUUM` onu yeniden yazar. Hikâyeler kartlarıyla kalır, bekleyen kalemler kalır (ikisi de konumla numaralanır). Başarısız olan bir temizlik yok sayılır ve sonraki başlangıçta denenir; sonucu hiçbir yerde gösterilmez. Uzun süredir kapalı projelerin hikâyeleri ve kapalı bekleyen kalemler için saklama kuralı henüz yoktur.

### Kilit

`imece.sqlite.lock` bütün çalıştırma boyunca `flock` ile tutulur, böylece dosya bir seferde tek bir imece'nindir. İkincisine, çıkış kodu 1 ile, şu söylenir: `another imece is running on /Users/me/.local/state/imece/imece.sqlite; open the other checkout from it with :open`. Yalnızca bırakılmakta olan bir kilit (çıkan bir imece) bir an beklenir. Ayarlarınız (`side`, `theme`, `lead`) o kilit olmadan kendi bağlantısıyla okunur ve yazılır; böylece tema depo alınmadan önce okunur ve canlı bir imece'nin yanındaki bir demo onu değiştirebilir.

## Worktree'ler

Her işçi `worktrees/<dizin>-<hash>/<ad>` alır: çalışma kopyanızın, yapıldığında HEAD'inizde ayrılmış (detached) bir git worktree'si. Yardımcılar işçilerinin worktree'sinde çalışır; lider çalışma kopyanızda çalışır ve worktree'si yoktur. Worktree çalıştırmalar arasında saklanır: daha eski bir commit'te temiz bulunan, ajanı yeniden başlayınca ve turlar arasında HEAD'inize taşınır; commit edilmemiş değişiklikleri ya da kendi commit'leri olan olduğu yerde kalır, ve günlük iki durumda da söyler (`claude-1: çalışma ağacı a1b2c3d yerine HEAD e4f5a6b oldu`, `… a1b2c3d noktasında kaldı, commit edilmemiş değişiklikleri var (HEAD e4f5a6b)`). Orada olan ama bu çalışma kopyasının worktree'si olmayan bir dizin reddedilir, asla yeniden kullanılmaz ya da değiştirilmez; elle silinmiş ama git'in hâlâ listelediği biri de reddedilir, çıkış yolu `git worktree prune`'dur, ve ajan o nedenle durur.

- Uygulanan bir değişiklik worktree'de bir işaret olarak commit'lenir, yazarı `imece <imece@localhost>`; böylece işçinin sonraki `done`'ı yalnızca sonra geleni gösterir ve worktree siz commit'leyince yine HEAD'inizi izler. Satırın `⎇ dal ↑kendi ↓checkout +N −M` kısmı nerede durduğunu söyler.
- Bir değişikliği okumak, bir worktree'nin nerede durduğu ve bir çalıştırmanın öncesi-sonrası ağacı worktree'nin git dizinindeki özel indeks dosyalarından geçer; adları `imece-index`, `imece-tree-<pid>-N`, `imece-stand-<pid>-N`; ajanın kendi indeksine asla dokunulmaz. Üç yollu bir uygulama yamayı önce çalışma kopyanızın indeksinin bir kopyasında, `<git dir>/imece-apply-index`, dener; sonra silinir.
- Her worktree kendi `target/`'ında derlenir: ortak bir tane işçilere birbirlerinin değişikliklerini sırayla yeniden derletirdi.
- `:close` worktree'leri diskte bırakır; hiçbir şey onları silmez. `:copy` listelediğiniz yok sayılan yolları (`.env`, `target`) işçisinin başlangıcında onlardan yoksun bir worktree'ye koyar; birimin elverdiği yerde APFS klonu ya da reflink olarak, asla var olanın üstüne değil, başlangıç başına en çok 2 GiB. Bkz. [değişiklikler](../concepts/changes.md) ve [projeler](../concepts/projects.md).

## Gemini'nin özel dizini

Gemini CLI'ın Claude Code'un `--restricted`'ı gibi bir şeyi yoktur ve ayarları `~/.gemini`'den ve içinde çalıştığı klasörün `.gemini` dizininden alır; bu yüzden imece her Gemini ajanını kendine ait bir dizinde çalıştırır, `worktrees/<dizin>-<hash>-gemini/<ajan>` (0700, worktree'lerin yanında, oturum sürdürülebilsin diye çalıştırmalar arasında saklanır), ve ona çalışma kopyasını ya da worktree'yi dahil edilen dizin olarak verir. Her başlangıçta yeniden yazılır:

| Dosya | Nedir |
| --- | --- |
| `GEMINI.md` | ajanın talimatları (rolü, sonra dosyalarının dizinini adlandıran bir satır); ACP'nin talimat vermenin tek yolu; Gemini'nin `/memory list`'i onu göstermeli |
| `policy/imece.toml` | rolün ve `--confine` düzeyinin araçlarıyla imece'nin kendi araçlarına tam olarak adla izin veren ve `strict` ile `standard` altında başka her aracı reddeden politika (reddedilen bir araç modele gösterilmez bile); `--policy` dizini adlandırır |

Deponun `.gemini` ayarları, hook'ları ve `.env`'i oturuma asla ulaşmaz. Dizin, ajan bütünüyle ayrılınca gider (`:close`, bir yardımcının `done`'ı); `:forget` yalnızca sohbet dosyalarını siler. Bkz. [satıcılar](../concepts/vendors.md).

## Veriyolunun soketleri

Her başlangıçta imece `$TMPDIR/imece-<pid>-<hex>/` yapar (0700; var olan bir dizin reddedilir, yeniden kullanılmaz) ve içinde ajan başına bir Unix soketi bağlar, `a0.sock`, `a1.sock`, …, ajan sırasıyla, işe alım anında. Bir satıcının CLI'ı `imece mcp-relay <soket>` üzerinden bağlanır. Dizin ve soketleri imece bitince silinir, SIGTERM, SIGHUP ve SIGINT'te de. `:commit`'in tek seferlik Claude çağrısı, okunacak bir şey olmayan bu dizinde çalışır.

## Dışa aktarmalar

`:export [PATH]` açık konuşmayı ya da kartın ekranını Markdown olarak yazar: `imece-conversation-YYYYMMDD-HHMM.md` ya da `imece-card-N-YYYYMMDD-HHMM.md`; `PATH` yoksa imece'nin başlatıldığı dizine, `PATH` bir dizinse içine, yoksa `PATH` adlı dosya olarak. Orada olan bir dosyanın üstüne asla yazılmaz. İçindeki her saatin günü vardır; her satır ekranla aynı temizlikten geçmiştir (denetim karakterleri silinmiş, gizli görünen belirteçler gizlenmiş). Bkz. [komutlar](commands.md).

## imece'nin depodan okuduğu

- Çalışma kopyasının tepesindeki `AGENTS.md`, yoksa `CLAUDE.md`: kodlama ajanları için notlar, en çok 16 KiB, her ajana talimatlarıyla geçirilir (Codex `AGENTS.md`'yi kendi okur). Bkz. [mesaj türleri](message-types.md).
- Git: çalışma kopyasının tepesi, HEAD, `:copy` için `git check-ignore`, `:commit` için `git diff --cached` ve en yeni sekiz konu (`--no-ext-diff --no-textconv` ile, böylece sizin bir programınız çalışmaz).
- `--theme FILE` ya da `:theme FILE` ile adlandırılan, Ghostty ya da Kitty biçiminde bir renk dosyası; mutlak yolu tema olarak hatırlanır.

Başka bir şey yok: deponun hiçbir dosyası imece tarafından yazılmaz ve satıcıların CLI'ları depodan hiçbir şey yüklenmesin diye başlatılır (`standard` ve `strict` altında).

## Satıcıların tuttuğu

Bir oturum, satıcısının tuttuğundan sürdürülür; imece yalnızca kimliği hatırlar (`agents.session`). Bunlar satıcıların dosyalarıdır, imece'nin değil.

| Satıcı | Nerede | Notlar |
| --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR` ya da `~/.claude`, `projects/<slug>/<id>.jsonl`; slug, ajanın dizininin harf ya da rakam olmayan her karakteri `-` yapılmış hali | transkript; dosya oradayken, aynı dizinden, oturum sürdürülebilir. Öteki izler (`debug`, `file-history`, `todos`) aranmaz |
| Codex | `$CODEX_HOME` ya da `~/.codex`, `sessions/YYYY/MM/DD/rollout-<zaman>-<id>.jsonl` | iş parçacığının rollout'u; imece iş parçacıklarını kalıcı (non-ephemeral) başlattığı için tutulur; `history.persistence="none"` imece'nin turlarını Codex'in küresel mesaj günlüğünün dışında tutar. Codex sizin kendi `~/.codex` yapılandırmanızı yükler (özel bir ev reddedildi: oturum açılamıyordu) |
| Gemini CLI | `$GEMINI_CLI_HOME` ya da `~/.gemini`, `tmp/<ad>/chats/session-<zaman>-<id8>.jsonl` | `<ad>` Gemini'nin özel dizinle anahtarlanan `projects.json` kaydından gelir; başlangıç başına ikinci, boş bir `session-*.jsonl` Gemini'nin kendi artığıdır; sohbet dosyası başına en çok 50 geçmiş mesajı |

Kendi `~/.claude/CLAUDE.md`'niz, `~/.gemini/GEMINI.md`'niz ve kullanıcı düzeyi hook'larınız, satıcının dizin ne olsun yüklediği yerde yine geçerlidir; `--confine open` bütün kurulumunuzu yükler.

## `--fresh` ve `:forget`'in sildiği

İkisi de bir ajana satıcı oturumunu unutturur; sonraki mesaj yeni bir oturum başlatır, kaldığı yer anlatılır. `--fresh` başlangıçta hatırlanan her ajan için, `:forget @NAME` biri için yapar. Giden, kendi iş parçacığında (iz nasıl gittiğini söyler: `forgot session <id>: its transcript was removed`):

| Satıcı | Silinen |
| --- | --- |
| Claude Code | yukarıdaki transkript dosyası (zaten gitmişse `it had no transcript left`) |
| Codex | iş parçacığı, onun için başlatılan bir app server üzerinden (`thread/delete`, 30 sn sınır); bu onu Codex'in kendi dizininden de çıkarır, rollout dosyasını elle silmek çıkarmazdı |
| Gemini CLI | o oturumun `tmp/<ad>/chats/` altındaki sohbet dosyaları (`N chat files were removed`) ve 0 baytlık artıklar; ajan bütünüyle ayrılırken o ad altındaki her şey ve özel dizin |

Bir yola ya da bir satıcıya yalnızca UUID biçiminde bir kimlik ulaşır. Henüz durması istenmiş bir oturuma önce gitmesi için 4 sn verilir. imece'nin kendisinden hiçbir şey silinmez: ajan, kartları, günlük ve worktree kalır. Bir oturum değiştirildiğinde de silinir: kendisininki olmayan bir satıcıda imece'nin `/clear`'ı, bir model geri dönüşü, sınırına çarpınca yenilenen bir oturum. Bkz. [oturumlar](../concepts/sessions.md).

## Yazılmayanlar

- Günlük dosyası ve iz dosyası yok: ham akış `trace` tablosudur (ajan başına 200 satır), günlük `journal` tablosudur.
- `!cmd` çıktısı terminalinizin ana ekranında kalır; yakalanmaz.
- `:commit`'in çağrısı `--no-session-persistence` ile çalışır: `~/.claude/projects` altında hiçbir şey, sürdürülecek bir şey yok.
- Demo, ekran ayarlarınız (`side`, `theme`, `lead`) dışında depoya hiçbir şey yazmaz.
- Başlıkta gösterilen satıcı kullanım pencereleri saklanmaz; bir oturum ilk turunda yeniden söyler.

## Ayrıca bkz.

- [Oturumlar](../concepts/sessions.md): neyin hatırlandığı ve bir ajanın nasıl uyandığı
- [Komut satırı](cli.md): `XDG_STATE_HOME`, `--fresh`, `--theme`
- [Komutlar](commands.md): `:export`, `:copy`, `:forget`, `:open`, `:close`
- [Güvenlik](../concepts/security.md): çalışma kopyasına neden yalnızca `a` dokunur
- [Sorun giderme](../troubleshooting.md)
