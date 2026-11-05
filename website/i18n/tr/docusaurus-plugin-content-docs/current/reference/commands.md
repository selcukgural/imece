---
title: Komutlar
description: imece'nin aldığı her `:` komutu; argümanları, karşılığı olan tuş, kısa adı, yalnızca demoda mı olduğu, `$` ile sonraki faturayı büyüten diye işaretlenip işaretlenmediği ve menü notunun gösterdiği.
---

# Komutlar

`:` ile başlayan bir satır imece'ye bir komuttur, bir ajana mesaj değil. Komut satırına yazarsınız (`:` onu açar), `tab` eşleşenleri değerleriyle sunar, `enter` çalıştırır. *Eyleyen* her tuşun (yalnızca hareket edenlerin değil) bir `:` adı da vardır; böylece her şey her yerden yazılarak yapılabilir. Tuş işleyicileri ve komutlar aynı kodu çağırır, bu yüzden birbirinden ayrı düşemezler.

## `:` satırı nasıl çalışır

- `:` sonra `tab` komutları listeler. Menü her komutun ardında şu an ne durumda olduğunu ya da neye etki edeceğini ayraç içinde gösterir: `budget (spent $3.10 of $20.00)`, `theme (dark)`, `close (kule)`, `forget (claude-2)`, `next (2 waiting)`, `needs (off)`. Not çizilir, asla satıra eklenmez.
- Bir komutun ardındaki `$` sonraki faturayı büyütenleri işaretler: `:open` (yeni bir lider oturumu), `:solo` (yeni bir oturum), `:forget` (bütün brifingi önbelleksiz yeniden ödeyen yeni bir oturum), `:commit` ve `:review` (ücretli çağrılar). Hiçbir `:` komutu kendi başına ücretli bir tur başlatmaz; `:budget` işaret yerine değerini gösterir.
- Kısa adlar: `:b` budget, `:c` close, `:f` forget, `:o` open, `:q` quit, `:t` theme. Bütünüyle yazılan kısa ad zaten bir komuttur; menü bir şey sunmaz ve `enter` çalıştırır (`:q` sonra `enter` hemen sorar).
- Argümanlar da tamamlanır: `:lang` sonra diller, `:notify` sonra dört kelimesi, `:motion` sonra üç kelimesi, `:review` ve `:plan` sonra `on off`, `:away` sonra `15m 30m 1h off`, `:clear` ve `:unclear` sonra `inbox journal`, `:theme` sonra yerleşik adlar, `:close` ve `:pin` sonra açık projeler, `:restore` sonra arşivlerin başlıkları (en yenisi önce, her biri projesinin adı ve arşivlendiği günle), `:goto` sonra hiçbir şey (`&` satırının kendi menüsü var), `:history` sonra gönderilen satırlar, `:forget`, `:talk`, `:trace` ve `:peek` sonra açık projelerin ajanları (`@` ile ya da onsuz), `:open` ve `:export` sonra dizinler (`~` ev dizininiz, gizli olanlar yalnızca bir `.` sonrası, `tab` seçilenin içine adım atar).
- imece'nin bilmediği bir ad: `no command xyz; : and tab list them`. Canlıda yazılan yalnızca demoya ait bir komut: `:pause is for the demo: live agents run in real time`.
- `!cmd` kendi kabuğunuzda bir komut çalıştırır, `/cmd` ajanın kendi komut satırıdır; ikisi de `:` komutu değildir ([komut satırı](../screen/command-line.md)).

## Bütün komutlar

| Komut | Tuş | Kısa | Yalnız demo | `$` | Menü notu |
| --- | --- | --- | --- | --- | --- |
| `:archive [TITLE]` | — | | | | |
| `:archives` | `A` | | | | |
| `:allow [PROGRAM [WORD]]` | `A` (bir istekte) | | | | projenin hep eveti, varsa |
| `:awake` | — | | | | açık / kapalı |
| `:away [TIME\|off]` | — | | | | `30m` / `1h` / `off` |
| `:board` | `2` | | | | |
| `:budget USD` | — | `:b` | | | harcanan / bütçe |
| `:clear [inbox\|journal]` | — | | | | |
| `:close [NAME]` | — | `:c` | | | eldeki proje |
| `:code` | `3` | | | | |
| `:commit` | — | | | `$` | |
| `:copy [PATH]` | — | | | | liste, varsa |
| `:defer` | `S` | | | | |
| `:digest` | — | | | | |
| `:edit [TEXT]` | `^x^e` (yazarken) | | | | açılacak editör |
| `:effort LEVEL` | — | | | | eldeki solonun çabası ya da `default` |
| `:export [PATH]` | — | | | | `konuşma` ya da `#12` |
| `:faster` | `+` | | evet | | hız, `×2` |
| `:fold` | `zM` | | | | |
| `:forget [@NAME]` | — | `:f` | | `$` | eldeki ajan |
| `:fork [@NAME]` | — | | | `$` | eldeki ajan |
| `:goto [NAME]` | `&` | | | | |
| `:history [WORDS]` | `^r` | | | | |
| `:help` | `?` | | | | |
| `:hints` | `F` | | | | |
| `:keys` | — | | | | dosyanızın verdiği tuş sayısı |
| `:lang [en\|tr]` | `L` | | | | `English` / `Türkçe` |
| `:lonca [NAME\|off\|login\|import [NAME]]` | — | | | | projenin deposu, varsa |
| `:mark` | `m` | | | | |
| `:model NAME` | — | | | | eldeki solonun modeli ya da `default` |
| `:motion [off\|calm\|full]` | — | | | | off / calm / full |
| `:needs` | `f` | | | | açık / kapalı |
| `:news` | — | | | | bu sürüm |
| `:next` | `W` | | | | N bekliyor |
| `:notify [off\|bell\|text\|push]` | — | | | | off / bell / text / push |
| `:now` | `U` | | | | |
| `:open PATH [NAME]` | — | `:o` | | `$` | |
| `:output` | `O` | | | | çıktısını alıntıladığı komut |
| `:pause` | `space` | | evet | | açık / kapalı |
| `:peek [@NAME]` | `K` | | | | eldeki ajan |
| `:pending` | `q s` | | | | |
| `:pin [NAME]` | — | | | | eldeki proje, sabitse `▴` |
| `:plan [on\|off]` | — | | | | on / off |
| `:quit` | `q` | `:q` | | | |
| `:recall [WORDS]` | `H` | | | | |
| `:rename NAME` | — | | | | eldeki proje |
| `:restore [NAME]` | — | | | | |
| `:review [on\|off]` | `R` (değişiklikte: tamamı) | | | `$` | on / off |
| `:salon` | `1` | | | | |
| `:settle` | `s` | | | | seçili satırın ajanı ya da projesi |
| `:slower` | `-` | | evet | | hız, `×2` |
| `:solo [PATH] [CHOICE] [NAME]` | — | | | `$` | |
| `:story` | `J` | | | | açık / kapalı |
| `:sync` | — | | | | trunk'ın ne kadar gerisinde (`↓3`) |
| `:talk [@NAME]` | `v` | | | | eldeki ajan |
| `:theme [NAME\|FILE]` | `T` | `:t` | | | kullanılan tema |
| `:tidy` | — | | | | |
| `:timeline` | `4` | | | | |
| `:trace [@NAME]` | `t` | | | | eldeki ajan |
| `:unclear [inbox\|journal]` | — | | | | |
| `:undo` | `u` | | | | |
| `:unfold` | `zR` | | | | |
| `:unsettle` | — | | | | kaç şeyin kaldırıldığı |
| `:waiting` | `w` | | | | N bekliyor |
| `:welcome` | — | | | | |
| `:why [@NAME]` | — | | | | eldeki ajan |
| `:yes [@NAME]` | `e` | | | | eldeki ajan |

Canlıda `:pause`, `:faster` ve `:slower` menüde yer almaz. Hareket eden tuşların (`j k`, `gg G`, `enter`, `esc`, `tab`) ve seçili satıra etki eden tuşların (`a r c n x`) `:` adı yoktur: yazmak tuştan yavaş olurdu ve satırın adlandırılması gerekirdi.

## Komutlar

### `:archive [TITLE]`

Eldeki projeyi `TITLE` adıyla kaldırır ve yerine yenisini açar, istendikten on saniye sonra: alt satır geri sayar (`◷ kule “first epic” adıyla arşivleniyor, 7 sn sonra · u tutar`; ajanlarından biri tur ortasındaysa `…; 2 tur yarıda kesilecek …`), `u` ([`:undo`](#undo)) olduğu gibi bırakır. Başlıksız, adı ve günleriyle anılır (`kule 2026-10-30 → 2026-11-02`); başlık tek satırdır, en çok 60 karakter.

Arşivlenen projenin ajanları çıkışta olduğu gibi durur (süren bir tur yarıda kesilir, istemi saklanır) ve her şeyiyle uyur: oturumları, postaları, notları, değişiklikleri, kartları ve SENİ BEKLEYEN'deki kalemleri. Hepsi ekrandan çıkar: salon, SENİ BEKLEYEN, günlük, pano ve kuyruk yalnız yeni projeyi gösterir; yenisinin adı, iğnesi ve `:copy` listesi aynıdır, başındaki ajanın adı ve modeli de. Hiçbir şey harcanmaz, bütçe aynı bütçedir. Flaş: `kule “first epic” adıyla arşivlendi: agent'ları her şeyiyle uyuyor; yerine yenisi açıldı`; günlük: `kule projesini “first epic” adıyla arşivledin; yerine yeni bir kule açıldı`. `--fresh` arşivin oturumlarına dokunmaz. Demo da aynı biçimde arşivler. Bkz. [oturumlar](../concepts/sessions.md#arşivler) ve [yeni bir epic](../scenarios/new-epic.md) senaryosu.

### `:archives`

Arşivleri bir ağaç olarak açar, `A` gibi: arşivi olan her çalışma kopyası (eldeki önce, sonra en yeni arşivine göre), altında şu an onda açık projeler (`● kule · şu an açık · 1 agent`), sonra arşivleri, en yenisi önce; her biri ajanlarının harcadığı, biten kartları, ajanları ve arşivlendiği günle (`▸ first epic · $4.20 · 3 kart bitti · 4 agent · 2026-11-02 arşivlendi`). İçinde hiçbir şey uyanmaz, hiçbir şey harcanmaz.

İçinde `j` `k` gezer, `gg` `G` ilk / son satıra gider, `l` (ya da `→`) bir arşivin ajanlarını gösterir (her biri harcadığıyla, başı `◆` ile işaretli) ve katlanmış bir çalışma kopyasını açar, `h` (ya da `←`) onları yeniden katlar (zaten katlı bir arşivde, çalışma kopyasını), `za` eldeki satırı katlar ya da açar; bir çalışma kopyasında `enter` da öyle. `/` süzer: yalnızca başlığında, proje adında, çalışma kopyasında ya da bir ajanının adında yazılan her kelime olan arşivler kalır, büyük/küçük harf ve Türkçe harfler katlanır; `enter` süzgeci tutar, `esc` bırakır. Bir arşivde ya da ajanlarından birinde `r` onu geri getirir, [`:restore`](#restore-name) gibi, ve pencereyi kapatır; `esc`, `q` ya da `A` kapatır (`esc` önce süzgeci bırakır). Hiç arşiv yoksa: `henüz arşiv yok; :archive elindeki projeyi kaldırır`. Geri sayan bir `:archive` önce olur, böylece listede yer alır. Demo da arşivlerini aynı biçimde, her birini projesinin adı altında listeler.

Bir arşivde (ya da ajanlarından birinde, o zaman o seçili) `enter` onu geri getirmeden, ekranın üstünde salt okunur açar: salon, pano, SENİ BEKLEYENLER ve günlük yalnızca o arşivi kaldırıldığı gibi gösterir; günlüğü, deponun ondan sakladığı her şeydir. En üst satır hangisi olduğunu ve bu arada seni bekleyeni söyler: `ARŞİV · kule “v1 fixes” · 2026-11-02 arşivlendi · şu an seni 1 şey bekliyor · esc geri`. İçinde hiçbir şey uyanmaz, harcamaz; arkadaki ekran sürer. Konuşmalar (`v` kopyalar), kart ekranları, odak, `:export` ve yalnızca bakan ya da giden `:` komutları orada çalışır; bir mesaj, `!cmd`, `m`, `A`, SENİ BEKLEYENLER'de bir cevap ve diğer her komut `arşiv salt okunur · :restore geri getirir · esc geri döner` der. İçinde `:restore` o arşivi geri getirir. `esc` ya da `q` (görünümde açık bir şey yokken) geri döner, `:quit` de; `ctrl-c` imece'den çıkar. Depo okunamazsa: `arşivin günlüğü okunamadı: …`.

### `:awake`

Makineyi uyanık tutmayı açar ya da kapar (`--awake` ile aynı). Açıkken ve herhangi bir ajan tur ortasındayken imece `caffeinate` (macOS) ya da `systemd-inhibit` (Linux) aracını çocuk olarak çalıştırır, hiçbir şey tur ortasında olmayınca bırakır. Flaş `bir ajan turdayken makine uyanık kalır` ya da `makine yeniden uyuyabilir` der; başlayamayan bir araç bir kez söylenir: `makine uyanık tutulamadı: …`. Çalıştırmalar arasında hatırlanmaz.

### `:away [TIME|off]`

SEN YOKKEN penceresinin kendiliğinden açılması için ne kadar uzakta kalmanız gerektiği: başka bir pencerede bu kadar kaldıktan sonra terminale dönünce ya da bu kadar tuşsuz geçen süreden sonraki ilk tuşta. Dakika (`45m` ya da `45`) veya saat (`2h`), bir dakikadan bir güne kadar; `off` hiç açmaz (`:digest` yine açar). Varsayılan yarım saat. Yalın `:away` sıradakine geçer: 15m, 30m, 1h, off; size ait bir süre bir sonraki daha uzun olana geçer. Flaş: `30m ya da daha uzun yokken olanlar dönünce hemen gösterilir`, `olanlar kendiliğinden gösterilmez; :digest gösterir`; başka bir kelime: `3s off, dakika (30m) ya da saat (1h) değil; en çok bir gün`. Menü notu değeri gösterir; depoda hatırlanır. Bkz. [günlük](../screen/journal.md#sen-yokken).

### `:board`

Pano görünümü, `2` gibi. Bkz. [pano ve kod haritası](../screen/board-and-code-map.md).

### `:budget USD`

Ajanlar çalışırken bütçeyi artırır ya da azaltır (`:b 80`). Değer, noktadan önce en çok dört rakam ve en çok iki ondalıkla dolardır (`80`, `2.50`); başkası: `80$ dolar değil; 80 ya da 2.50 gibi yaz`. Flaş `bütçe artık $80.00` der, günlük `bütçeyi $80.00 yaptın (önce $50.00)`, ve bütçenin azaldığı uyarısı yeniden çalabilir. Süren bir tur eski sınırında biter; sonraki yeni kalanla başlar, eskisi sınırına çarpmışsa yeni bir oturumda. Bütçe bütün projeler için tektir ve çalıştırmalar arasında saklanır. Menü notu: `(spent $3.10 of $20.00)`. Bkz. [bütçe ve sınırlar](../concepts/budget-and-limits.md).

### `:clear [inbox|journal]`

SENİ BEKLEYEN'in listelediklerini ve günlüğün satırlarını salonun ekranından kaldırır: yalın ikisini, `inbox` ya da `journal` birini. Hiçbir şey silinmez: depo hepsini tutar, `:export`, bir kartın ekranı ve arşivler hâlâ gösterir. `:close` gibi önce on saniye bekler (`◷ SENİ BEKLEYEN ve günlük 7 sn sonra ekrandan kalkıyor · u tutar`; `u` ekranı olduğu gibi bırakır), sonra neyin gittiğini söyler: `2 bekleyen ve 40 günlük satırı ekrandan kalktı; :unclear geri getirir`. Sizi hâlâ bekleyen bir şey kaybolmaz: açık kalır, ajanı hâlâ bekler, başlık onu sayar (`SENİ BEKLEYEN (0 · +2 gizli)`), terminalin başlığı ve sekmesi de sayar, `W` sıradakini açar ve onu listeye geri koyar. İşaretli bir satır (`m`) günlükte kalır. Sonra gelenler her zamanki gibi görünür. Temizleme depoda saklanır, böylece sonraki açılış da temiz başlar. Temizlenecek bir şey yoksa `ekranda temizlenecek bir şey yok` der; başka bir kelime: `all inbox ya da journal değil`. Bkz. [günlük](../screen/journal.md#ekranı-temizlemek).

### `:close [NAME]`

`NAME` projesini ya da eldekini kapatır (`:c`), istendikten on saniye sonra: alt satır geri sayar, `u` ([`:undo`](#undo)) açık tutar. Ajanlarının oturumları ve çalıştırmaları biter, postaları, notları, teklifleri ve değişiklikleri düşer, SENİ BEKLEYEN'deki kalemleri kapanır ve proje kapalı işaretlenir: salon onu atlar, aynı çalışma kopyası yeniden açılırsa yeni bir liderle onun yerini alır. Worktree'ler diskte kalır. Flaş: `kule kapatıldı: agent'ları ayrıldı; worktree'leri diskte kaldı`. Son açık proje reddeder: `kule son açık proje; q q çıkar`; kimsenin olmadığı bir ad: `x adında açık proje yok`. Demoda projenin ajanları aynı biçimde ayrılır. Bkz. [projeler](../concepts/projects.md).

### `:code`

Kod haritası, `3` gibi. Canlıda henüz ortak trunk olmadığını söyler (her ajan kendi worktree'sinde çalışır, gösterilecek lease yoktur); sembol lease'leri yapılmamıştır. Bkz. [pano ve kod haritası](../screen/board-and-code-map.md).

### `:commit`

Claude Code'un ucuz kademesine (`haiku`, ya da sizin `--tier claude:cheap=…`) eldeki projenin çalışma kopyasında stage'dekiler için bir commit mesajı sorar ve onu komut satırına `!git commit -m '…'` olarak koyar; düzenleyip `enter` ile kendi kabuğunuzda çalıştırır ya da `esc` ile bırakırsınız. imece hiçbir şey commit'lemez. Flaş `haiku modeline kule içinde stage'dekiler için commit mesajı soruluyor` der; günlük her öneriyi modeli ve maliyetiyle, her başarısızlığı nedeniyle söyler.

Para eder: her çağrı en çok 0,05 $ (çağrının kendi sert sınırı), bir gün (UTC) bütün projeler için en çok 20 çağrı ve 1,00 $; çağrıdan önce bütçeden ayrılır, sonra gerçek değerle kapatılır. Stage'de bir şey yoksa, gün dolmuşsa ya da bütçede sınırdan az kalmışsa `commit mesajı yok: …` ile reddedilir. Aynı stage ağacı yeniden sorulursa para etmez: son öneri geri gelir. Çağrı `git diff --cached --stat`'ı, yamayı (en çok 24 KiB) ve en yeni sekiz commit konusunu okur; araçsız, MCP sunucusuz, kendi kısa sistem istemiyle çalışır, hiçbir şey kaydedilmez. Demo, o projede uygulanan değişikliklerin başlıklarından `git commit --dry-run` olarak yanıtlar. Bkz. [kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md).

### `:allow [PROGRAM [WORD]]`

Eldeki projenin hep eveti: ajanlarının size sormadan çalıştırdığı komutlar (bkz. [onaylar](../concepts/approvals-and-questions.md#artık-hep-evet)). Tek başına `:allow` listeyi söyler (`kule: artık hep evet: make, npm run …` ya da `kule: hep evet yok; bir istekte A, ya da :allow PROGRAM [SÖZCÜK]`); `:allow npm run` bir tane ekler (projenin onu kapsayan açık istekleri sürer), aynısı yeniden onu çıkarır (`kule: artık hep evet değil: npm run …`). Bir program ve en çok bir sözcük, önce seçenek değil: `:allow rm -rf` reddedilir. SENİ BEKLEYEN'deki bir istekte `A` o isteğin çalıştırdığını ekler, böyle bir hep evet tutmuyorsa yalnız o satırı; `:allow` o satırla yazılınca onu çıkarır. Çalıştırmalar arasında projeyle saklanır.

### `:copy [PATH]`

Bir işçinin başlangıcının worktree'sine kopyaladığı, git'in yok saydığı yolların listesi (`.env`, `node_modules`, `target`): bir worktree taze bir çalışma kopyasıdır, git'in yok saydığı orada yoktur. `:copy PATH` eldeki projeye bir yol ekler, `:copy PATH` yeniden onu düşürür (sondaki `/` aynı yoldur), `:copy` tek başına listeyi söyler: `kule çalışma ağaçlarına kopyalananlar: .env, target` ya da `kule çalışma ağaçlarına hiçbir şey kopyalanmıyor; :copy YOL git'in yok saydığı bir yolu ekler`.

Yol göreli, düz (glob yok, `..` yok, `.git` içinde değil, en çok 200 bayt), çalışma kopyasında var, bağlantı değil, çözümlenince çalışma kopyasının içinde ve git'çe yok sayılan olmalıdır (izlenen bir dosya worktree'de zaten vardır); yoksa `x kopyalanamaz: …`. Kopya her işçinin başlangıcında olur (yardımcılar işçilerinin worktree'sini paylaşır; lider için asla), yalnızca worktree'de olmayan yollar için, bir başlangıcın bütün yolları 2 GiB içinde, birimin elverdiği yerde APFS klonu ya da reflink olarak. Günlük neyin kopyalandığını ve neyin atlandığını söyler. Projeyle birlikte depoda saklanır. Listenin yanında, checkout'un kendi `.worktreeinclude` dosyasının (gitignore sözdizimi) git'in yok saydığı yollar arasında uyduğu da kopyalanır, günlükte ayrı söylenir; tek başına `:copy` o zaman `; bir de .worktreeinclude'un saydıkları` ile biter (bkz. [projeler](../concepts/projects.md)). Bkz. [dosyalar](files.md) ve [projeler](../concepts/projects.md).

### `:defer`

SENİ BEKLEYEN'deki eldeki kalemi erteler ve sonrakini açar, `S` gibi. Bkz. [seni bekleyenler](../screen/waiting.md).

### `:digest`

SEN YOKKEN penceresini şimdi açar: bu çalıştırmada terminalin penceresinden son ayrıldığınızdan, ya da çalıştırma başladığından beri olanlar. Yeni bir şey yoksa: `son ayrılışından beri yeni bir şey yok`. Kendiliğinden açıldığındaki gibi herhangi bir tuş kapatır.

### `:edit [TEXT]`

`TEXT`'i (yalın hâlde boş) editörünüzde açar, `^x ^e`'nin yazmakta olduğunuz satırla yaptığı gibi: `$VISUAL`, yoksa `$EDITOR`, yoksa `vi`. Ekran bir `!` komutundaki gibi kenara çekilir, editör projenin checkout'unda çalışır ve kaydettiğiniz komut satırına döner, imleç sonunda, gönderilmeden. Hatayla çıkan (vim'in `:cq`'su), durdurulan, 1 MiB'tan fazlasını ya da metin olmayan bir şey bırakan bir editör taslağı olduğu gibi geri verir ve terminalde bıraktıklarının altındaki bir satır, bir tuş ekranı geri getirene dek nedenini söyler: `` [imece] `vim` 1 ile bitti: taslak duruyor · herhangi bir tuş: ekrana dön ``; bulunamayan biri `` `nvim` diye bir editör yok: $VISUAL ya da $EDITOR verin; taslak duruyor `` der. Taslak, imece'nin durum dizininde yalnız sizin okuyabildiğiniz kendi dosyasıdır ve editör kapanınca silinir. Menü notu: açılacak editör, `(vim)`. Bir arşivin görünümünde olmaz. Bkz. [komut satırı](../screen/command-line.md#editörünüz).

### `:effort LEVEL`

Eldeki solo oturumun sonraki turundan itibaren ne kadar derin düşüneceğini ayarlar: Claude Code'un çabalarından biri (`low`, `medium`, `high`, `xhigh`, `max`) ya da onun kendisi için `default`. Eldeki solo, seçili ajan soloysa odur, yoksa eldeki projenin solosu. Çalışan oturumu biter ve sonraki mesaj aynı transkript üzerinde, kaldığı yer anlatılarak yeni bir oturum başlatır; süreçten başka bir şey yitmez. Tur ortasında (`solo-notes değiştirilemedi: solo-notes is mid-turn; wait for its answer`), başka bir düzeyde (`claude has no effort "turbo"; one of …`) ve elde solo yokken (`elde solo oturum yok: birini seç ya da :solo ile aç`) reddedilir. Flaş: `solo-notes sonraki turundan itibaren claude opus@max üzerinde; oturumu onunla yeniden başlar`; günlük: `solo-notes artık claude opus@max üzerinde çalışıyor`. Menü notu şimdiki çabayı gösterir, `(xhigh)` ya da `(default)`; `:effort ` ardından düzeyler tamamlanır. Seçimin model kısmı kalır. Bkz. [solo oturumlar](../concepts/solo.md).

### `:export [PATH]`

Açık olanı Markdown dosyası olarak yazar: açık konuşma (`# Konuşma: lead ⇄ claude-1`, her mesaj günü, saati, kimden kime ve türüyle bir başlık), yoksa kartın ekranı (`# #12 başlık`, projesi, sütunu ve dakikaları, KİM, HİKÂYE, onda sizi bekleyenler ve günlük satırları, en eski önce, hiçbiri katlanmamış). Açık bir şey yoksa: `dışa aktarılacak bir şey yok: önce bir konuşma (v) ya da iş (#N) aç`.

`PATH` bir dosya, ya da içinde dosya adlandırılacak bir dizindir; yoksa imece'nin başlatıldığı dizin. Ad neyi ve ne zaman olduğunu söyler: `imece-conversation-20261031-1400.md`, `imece-card-12-20261031-1400.md`. Zaten orada olan bir dosyanın üstüne asla yazılmaz; flaş `… dosyasına yazıldı` ya da `… yazılamadı: …` der. Her saatin günü vardır, her satır ekranla aynı temizlikten geçer. Menü notu neyi yazacağını söyler: `(konuşma)` ya da `(#12)`.

### `:faster`

Yalnız demo, `+` gibi: simülasyon hızlanır. Menü notu hızı gösterir (`×2`).

### `:fold`

Salonda her projeyi ve her işçinin yardımcılarını katlar, `zM` gibi; başka bir görünümden önce salona gider. Açık bir konuşma varken bunun yerine onun her mesajını tek satıra katlar, bu çalıştırmada sonra açılan konuşmalar da katlı başlar. Bkz. [salon](../screen/salon.md) ve [konuşmalar](../screen/conversations.md).

### `:forget [@NAME]`

Ajana oturumunu unutturur (`:f`), istendikten on saniye sonra: alt satır geri sayar, `u` ([`:undo`](#undo)) tutar. Sonra: çalışan oturum biter (çalıştırması öldürülür, bitmemiş bir istem kesilmiş olarak tutulur, uyanışta anlatılır ve yeniden çalıştırılmaz), hatırlanan oturum transkriptiyle gider, ajan kendisiyle konuşulana dek uyur ve sonraki mesajı tam brifingle yeni bir oturum başlatır. Ad yoksa eldeki ajan. Flaş: `claude-2 oturumunu unuttu; sonraki mesaj yeni bir oturum başlatır, kaldığı yer anlatılır`; unutacak şeyi olmayan ya da gitmiş bir ajan için reddedilir: `claude-2 oturumunu koruyor: it has no session to forget`. Günlük: `claude-2 oturumunu unutturdun; yeni bir oturumla başlıyor, kaldığı yer anlatılacak`. `--fresh` başlangıçta her ajan için yapar. `$`: yeni oturum bütün brifingi yeniden öder. Bkz. [oturumlar](../concepts/sessions.md).

### `:fork [@NAME]`

`NAME` işçisinden (yoksa eldekinden) başka bir yolu denemek için devam eden yeni bir işçi yapar, o işçi kendi yolunda sürerken: yenisi onun konuşmasına (oturumunun bir çatalı: Claude Code'un `--fork-session`'ı) ve işlenmemiş değişikliğine, izlenmeyen dosyalar dahil, o işçinin HEAD'inde yapılan kendi çalışma ağacında sahip olur. İşe alınan bir işçi gibi adlandırılır (`claude-3`), aynı modelde çalışır, `--agents` içinde sayılır ve turları ortak kasadan çıkar (Claude Code ilk oturumun maliyetini de sayar; imece yalnız çatalın turlarını sayar). Satırı seçilir ve komut satırı `@claude-3 ` ile açılır, `claude-3, claude-1 ajanının çatalı: ne denemesi gerektiğini yazın` flaşıyla; yazdığınız ilk turudur ve çatal ancak o zaman olur: oturum ve değişiklik o anki halleriyle alınır. Günlük `claude-3, claude-1 ajanından çatallandı: …` der; değişikliği kendi Δ'sıdır. Nedeniyle reddedilir: işçi olmayan her şey, henüz oturumu olmayan ya da tur ortasındaki bir işçi, dolu bir ekip, yerinde olmayan bir çalışma ağacı ya da Lonca workspace'indeki bir işçi. Çatallanamayan bir oturum yeni bir oturum olarak sürer, nerede olduğu söylenerek; günlük nedenini söyler. Demo da çatallar, `--agents` sınırı olmadan.

### `:goto [NAME]`

`NAME` adlı ajana (önünde `@` olabilir), yoksa o adlı açık projeye gider: salon, satırı seçili, projesi açılmış, bir yardımcının işçisi açılmış ve satırı gizleyecekse `f` bırakılmış; bir proje adı başlığını seçer. Büyük küçük harf ve Türkçe harfler fark etmez. O adda bir şey yoksa: `"zzz" adında gidilecek bir yer yok: bir agent ya da proje`, ve hiçbir şey kımıldamaz. Tek başına `&` satırını açar: yerler şu anki hâliyle, sizi bekleyenler önde (takılan, sonra bekleyen, sonra bitmiş ve görülmemiş), kalanlar durumuna göre (çalışan boştakinden önce), sonra açık projeler, her ajan glifi ve durum sözcükleriyle. Satır açıkken liste donar, böylece yazarken numaralar değişmez; `&` hemen ardından `1`-`9` bir rakam anında gider, `&` hemen ardından `#` kart satırına döner, `!` yalnız sizi bekleyenleri tutar, rakamlar bir numaranın başıyla eşleşir, geri kalanı adların içinde arar. `enter` imlecin altındakine, ya da yazılan ada gider. Bkz. [salon](../screen/salon.md).

### `:history [WORDS]`

Gönderdiğiniz satırları arar, `^r` gibi: komut satırı üstünde `WORDS` ile açılır ve tamamlama menüsü gönderilenlerden her sözcüğü geçenleri listeler (büyük küçük harf ve Türkçe harfler fark etmez), en yenisi önce, her satır bir kez ve ne kadar önce gönderildiğiyle; hiçbir şey yazılmamışsa hepsi. `tab`, `↑` ve `↓` aralarında ilerler, `enter` seçili satırı komut satırına bütün olarak koyar (birkaç satırlık bir satır olduğu gibi gelir), yeniden `enter` ile göndermek ya da önce değiştirmek için; hiçbiri eşleşmezse `enter` sözcükleri tutar ve aramayı bitirir. `esc` menüyü, sonra satırı kapatır. Yazarken `^r` yazılanı sözcük olarak alır. Henüz bir şey gönderilmemişse: `geri getirilecek bir şey gönderilmedi henüz`. Gönderilen her satır tutulur (bir mesaj, bir `:` komutu, bir `!kmt`; yalnız bir yere giden `#12`, `&3`, `:goto` ve `:history` hariç), çalıştırmalar arasında da: en yeni 500'ü sonraki başlangıçta geri gelir. Metnin ilk ya da son satırındayken `↑` ve `↓` aynı satırlarda aramasız gezer. Bkz. [klavye](../screen/keyboard.md) ve [dosyalar](files.md).

### `:help`

`?` yardımı: iki dilde her tuş ve komut, glif açıklamaları; içinde `/` her kelimeyle süzer. Bkz. [klavye](../screen/keyboard.md).

### `:hints`

İpucu harfleri, `F` gibi: salon ekranında seçilebilen her satırın (proje başlıkları, ajanlar, bir kartın ekranındaki kişiler, bekleyen kalemler, günlük satırları) başına bir ya da iki harflik bir etiket. Bir etiketi yazmak satırını kendi panelinde seçer; son harfi büyük yazılırsa onu `enter` gibi açar da. `backspace` bir harfi geri alır, `esc` kapatır. O arada ekrandan çıkan bir satır: `o satır ekrandan çıktı`. Salon ekranı dışında: `burada harf verilecek satır yok: F salon ekranında çalışır`. Bkz. [salon](../screen/salon.md).

### `:keys`

Tuş dosyanızı (`~/.config/imece/keys`) yeniden okur ve ne çıktığını söyler: `…/imece/keys: tuşlar alındı (5)`, imece'nin bir tuşunun anlamı başka bir tuşa geçmeden gittiyse `; artık hiçbir tuş j işini yapmıyor` ile; `tuş dosyası yok; … içine yazılan TUŞ = TUŞ satırları sana kendi tuşlarını verir` (imece'nin kendi tuşları geri gelir); ya da dosyanın neden reddedildiği, elinizdeki tuşlar kalarak: `…/imece/keys: satır 3: TUŞ = TUŞ diye yaz; elindeki tuşlar kalıyor`. Menü notu dosyanın kaç tuş verdiğidir. imece dosya değişince onu kendiliğinden de aynı sözlerle okur; `:keys` yalnızca bunları yeniden duymak için gerekir. Bkz. [kendi tuşlarınız](../screen/keyboard.md#kendi-tuşlarınız).

### `:lang [en|tr]`

Ekranın dili, `L` gibi. Tek başına öteki dile geçer; `:lang en` ya da `:lang tr` birini seçer. Başka bir şey: `de bir dil değil: en ya da tr`. Menü notu kullanılan dili gösterir. Yalnızca bu çalıştırma için; sonraki başlangıç `--lang` ya da `IMECE_LANG`'i okur.

### `:lonca [NAME|off|login|import]`

Eldeki projenin [Lonca](https://github.com/selcukgural/lonca) deposu. `:lonca AD` onu adlandırır (ad 1 ile 64 arası harf, rakam, `.`, `_`, `-`) ve daemon'a hemen sorar: `kule artık kule Lonca deposunda; imece daemon'ına soruyor`. `:lonca off` depoyu bırakır. Tek başına projenin nerede durduğunu söyler: `kule kule Lonca deposunda: trunk 693d629, insan bekleyen 2`, ya da neden okunamadığını, ya da `kule hiçbir Lonca deposunda değil; :lonca AD bir depo adlandırır, :lonca import oluşturur`. Depoda proje başına saklanır; arşivlenen proje onu yerine açılan yenisine bırakır.

`:lonca login` oturum açar: imece, sen işine devam ederken bir kez `lonca human login` çalıştırır (Secure Enclave anahtarı parmağını bekler), günlük de `Lonca'da NAME olarak oturum açıldı` ya da neden açılamadığını söyler. Bir oturum en çok bir gün sürer.

`:lonca import [AD]`, çalışma kopyasından AD ya da projenin adıyla bir depo oluşturur. Bir pencere çalışma kopyasını, dalı ve commit'ini, olacağı depoyu gösterir; `a` aktarır, `esc` ya da `q` kapatır ve hiçbir şey gitmez, `?` yardımı açar. imece bunu geri alamaz. Yanda çalışır, günlük nasıl gittiğini ve Lonca'nın ilk uyarısını söyler. Hemen reddedilenler: zaten bir depoda olan proje, git'siz klasör, ayrık HEAD, süren bir `lonca` komutu. Yeni bir daemon'da ilk aktarma oturumunu da açar, çünkü hiç deposu olmayan bir daemon kimsenin oturumunu açamaz. Ondan önceki `:lonca login` daemon'ın reddini gösterir.

Lonca'daki bir proje daemon'ından yanıt almadıkça kapalıdır: yardımcılar dahil kimse işe alınmaz, `:fork` reddedilir, hiçbir Δ uygulanmaz, reddedilmez, geri gönderilmez. Satırı `⌀ lonca yok` der, günlük de nedenini bir kez söyler. Bkz. [projeler](../concepts/projects.md#loncada). Bundan sonra işe alınan işçiler Lonca workspace'lerinde çalışır, önerdikleri `lonca approve`'u bekler; zaten worktree'de olan işçi orada kalır, Δ'sı çalışma kopyana iner. Demonun ikinci projesi Lonca'dadır ve daemon'ı ara sıra durur; `:lonca import` ile `:lonca login` onun tablosunda çalışır, orada olan bir ad bir daemon'ın reddettiği gibi reddedilir.

### `:mark`

Eldeki satırı sonra dönmek için işaretler, `m` gibi: açık konuşmada, yoksa günlükte; yeniden kaldırır. İşaretli satır `✱` taşır, çalıştırmalar arasında kalır ve asla katlanmaz, temizlenmez ya da yüklemeden dışarıda kalmaz; `'` sonraki işaretli satıra gider. Flaş: `işaretlendi: ' ona geri getirir`. Bkz. [günlük](../screen/journal.md).

### `:model NAME`

Eldeki solo oturumun sonraki turundan itibaren çalıştığı modeli ayarlar: bir takma ad (`opus`, `sonnet`, `haiku`, `fable`) ya da Claude Code'un aldığı biçimde tam bir ad, ya da onun kendisi için `default`. `:effort` ile aynı kurallar, redler, flaş ve günlük satırı; model adı olmayan bir ad (`a b`, `-x`) reddedilir. `:model ` ardından takma adlar tamamlanır; tam bir ad yazılır. Seçimin çaba kısmı kalır.

### `:motion [off|calm|full]`

Ekranda ne kadar şeyin hareket ettiği. `calm` (varsayılan): çalışan bir satırın işareti nefes alır, rengi 0,6 saniyede bir kısılır, şekli hiç değişmez (panoda da, kartının altındaki çalışan sahibinki); `başlıyor` noktalarını sayar; imece bir yanıt beklerken (`:commit`) alt satırda bir dönücü görünür, ilk açılış penceresinde hâlâ denetlenen Claude Code'un yanında da; sizi beklemeye düşen bir satır ya da kalem üç kez yanıp söner; turu biten ya da oturumu kalkan bir satır yeşil bir zeminden kendi renklerine yatışır (terminalin kendi renklerini kullanan bir temada: ters renk, sonra değişen satırların görünüşü); bir satır etkinlik çubuğunu taşır, `▁▃▅▂▁`, agent'ın son beş tikin her birinde yaptıkları (yalnızca agent bir şey yaptıkça ve tikler geçtikçe değişir); imece'nin işareti, ekrana geldiği yerde (hoş geldin ve yenilikler pencereleri, henüz işçisi olmayan bir salon), noktalarını birer vuruşta getirir, herhangi bir tuş onu bütün gösterir. `full` biri çalışırken başlıktaki filonun yanına bir dönücü ekler ve daha hızlı döner; SSH üzerinde sakin çalışır (her kare ağdan geçer). `off`: hiçbir şey hareket etmez, hiçbir satır etkinlik çubuğu taşımaz ve işaret bütün çizilir, ekran eskisi gibidir. Başka penceredeyken hiçbir şey hareket etmez ve orada olanlar dönünce yanıp sönmez; her şey tek bir vuruşa (0,3 sn) düşer, böylece aynı anda birkaç şeyin hareket etmesi tek bir şeyinkinden fazla kareye mal olmaz ve bir kare yalnızca gösterdiği şey hareket edince çizilir (bkz. [karar](https://github.com/selcukgural/imece/blob/main/docs/decisions/84-motion.md)). Yalın `:motion` sıradakine geçer: off, calm, full, off. Flaş yenisinin ne yaptığını söyler (`sakin hareket: çalışan satır nefes alır, seni bekleyen yanıp söner`, `tam hareket: dönücüler de, daha hızlı`, `SSH üzerinde tam hareket sakin çalışır`, `ekranda hiçbir şey hareket etmez`); başka bir kelime: `wild off, calm ya da full değil`. Menü notu değeri gösterir; depoda hatırlanır.

### `:needs`

Salon yalnızca sizi bekleyenleri gösterir (sizi bekleyen, takılmış, bitmiş ve bakılmamış), `f` gibi; yeniden, herkesi. Önce salona gider. Menü notu: `(açık)` / `(kapalı)`.

### `:news`

Son sürümlerde yeni olanlar (en çok üç), en yenisi önce, herhangi bir tuşun kapattığı bir pencerede. Yeni bir sürümün ilk başlangıcı onu kendiliğinden bir kez gösterir, önceki başlayan sürümden bu yana yeni olanlarla. Menü notu bu derlemenin sürümüdür. Bkz. [ilk çalıştırma](../getting-started/first-run.md#yeni-olanlar).

### `:next`

Sizi bekleyen sonraki kalemi her yerden açar, `W` gibi. Menü notu kaç kalemin beklediğini söyler: `(2 bekliyor)`.

### `:notify [off|bell|text|push]`

Başka bir penceredeyken sizi bekleyen bir şey geldiğinde nasıl haber alacağınız: `bell` terminalin zilini çalar (her terminalin bunun için bir ayarı vardır: ses, sekmede rozet, flaş; tmux'ta `monitor-bell`), `text` imece'nin sözleriyle bir masaüstü bildirimi gönderir, terminalinizin en iyi gösterdiği biçimde (başlarken adı sorulur): kitty 0.36+ kendi biçimini alır (OSC 99), başlığı `imece`, takılan bir şey ya da API hatası için daha yüksek sesle ve hata sesiyle, yoksa soru sesiyle, ve **tıklayınca neyle ilgiliyse o açılır** (bekleyen öge, takılan agent, birkaçı içinse `W` gibi sıradaki bekleyen); Ghostty başlığı `imece` olan OSC 777 alır; tmux içinde tmux'un passthrough'undan geçer, bunun için tmux ayarınızda `set -g allow-passthrough on` gerekir (yoksa tmux düşürür; `bell` her zaman geçer); başka her terminal OSC 9 alır (iTerm2, WezTerm; diziyi tanımayan bir terminal yok sayar, orada `bell` seçin), `off` (varsayılan) ikisi de değil. Yalın `:notify` sıradakine geçer: off, bell, text, off. Flaş `başka penceredeyken bir şey beklerse zil çalar`, `başka penceredeyken bir şey beklerse masaüstü bildirimi gelir` ya da `başka penceredeyken haber yok` der; başka bir kelime: `beep off, bell, text ya da push değil`. Menü notu değeri gösterir; depoda hatırlanır.

Ne çalar: SENİ BEKLEYEN'e yeni düşen bir kalem (değişiklik, soru, cevap, onay isteği, hassas bir şey) ya da takılan bir ajan (bloke, oturumu durmuş), terminal odakta değilken geldiyse ve beş saniye sonra hâlâ oradaysa, her biri bir kez. Bildirim türü ve ajanı söyler, ajanın sözlerini asla: `imece · claude-1 bitirdi: bir değişiklik seni bekliyor`, `imece · claude-2 sana soruyor`, `imece · claude-3 takıldı`; aynı anda birkaçı tek zil ya da sayıyla tek bildirimdir (`imece · 3 şey seni bekliyor`). Siz ayrılırken ekranda olan hiç çalmaz; pencereye dönünce çalmak üzere olan düşer: artık ekranda. Terminal odağı bildirmeli (tmux'ta `focus-events on` gerekir); bildirmeyen hiç çalmaz. Bkz. [SENİ BEKLEYEN](../screen/waiting.md).

`push` bunun yerine bekleyen şeyi [ntfy](https://ntfy.sh) üzerinden telefonunuza gönderir, makineden uzaktayken: terminal odağı kaybettiğinde ya da `:away` süresince (varsayılan 30 dakika; `:away off` yalnız odağı bırakır) hiçbir tuşa basılmadığında söylenir, çünkü masada bırakılan bir terminal odağını korur. Yalnız adıyla seçilir (yalın döngüde asla) ve yalnız ayarlarınız geçerliyse: imece'nin ayar dizininde bir `push` dosyası (`$XDG_CONFIG_HOME/imece/push`, yoksa `~/.config/imece/push`):

```text
# bildirimler nereye gider
server = https://ntfy.sh
topic = imece-alerts-7f3k9q2m
# token = ~/.config/imece/ntfy-token
```

`server` yazılmazsa herkese açık `https://ntfy.sh` olur; kendi sunucunuz `https` olmalı (`http` yalnız bu makinede), kullanıcı, sorgu ya da parça içermemeli. `topic` harf, rakam, `-` ve `_` olur, en çok 64; herkese açık sunucuda bir topic'i bilen onu okur, bu yüzden 16'dan kısa ya da rakamsız bir topic reddedilir. `token`, kendi sunucunuzun istediği token'ı tutan bir dosyayı adlandırır: başka kimsenin okuyup yazamadığı (`chmod 600`) size ait düz bir dosya, tek sözcük; `Authorization` başlığı olarak gider, URL'de, komut satırında ya da bir günlükte asla. Bir bildirim `curl`'dür (kurulu olmalı): nereye gideceğini, sözleri ve token'ı stdin'den okur, sizin `.curlrc`'nizden hiçbir şey almaz, yönlendirme izlemez, en çok on saniye sürer. Bir notun söylediğini ve projeyi söyler, bir ajanın sözlerini ya da bir komutu asla: `imece · claude-2 sana soruyor · shop`, `imece · 3 şey seni bekliyor`; takılan bir şey yüksek öncelikle gider. Her şey bir kez gönderilir; günlük her bildirimi söyler (`telefonuna bildirim gitti: …`); başarısız olan bir kez söylenir (`telefonuna bildirim gitmedi: …`) ve yeniden denenmez, bekleyen bir sonraki şey kendi başına gönderilir. Flaş `uzaktayken bir şey beklerse https://ntfy.sh üzerinden telefonuna bildirim gider` ya da ayarlarda neyin yanlış olduğunu (`bildirim ayarı: ntfy.sh'te topic en az 16 karakter ve bir rakamlı olmalı, …`) söyler; ayarlar her başlangıçta yeniden okunur ve artık geçerli değilse hiçbir şey gönderilmez (`:notify off`). Demo ayar okumaz, bir şey göndermez: her bildirim bir günlük satırıdır, `telefonuna bildirim şunu derdi (demo bir şey göndermez): …`.

### `:now`

Geri sayan `:close`, `:forget`, `:archive` ya da `:clear`'ı on saniyesini beklemeden hemen yapar, `U` gibi: süresi dolduğunda olanın aynısı. Geri sayarken alt satır bunu söyler (`◷ kule 7 sn sonra kapanıyor · u açık tutar · U hemen`); geri sayan bir şey yoksa `geri sayan bir şey yok` der. Geri dönüş yolu hâlâ `u`dur ([`:undo`](#undo)). Demo da canlı oturumlar da.

### `:open PATH [NAME]`

`PATH`'teki git çalışma kopyasını kendi başına bir proje olarak açar (`:o`), `lead-NAME` adlı yeni bir liderle (aynı model ve kademelerle). `NAME` varsayılan olarak dizinin adının güvenli hali: harf, rakam, `.`, `_`, `-` ile 1-24 karakter (kalanlar `-` olur); yanlış bir ad reddedilir: `"x y" proje adı olamaz: 1-24 harf, rakam, . _ -`. `~` ev dizininizdir; boşluklu bir yol ayrıştırılmaz. Flaş: `kule açıldı (/Users/me/src/kule); lead-kule adına yaz`. Redler: `:open YOL [AD], YOL'daki git çalışma kopyasını proje olarak açar` (yol yok), `kule adında açık bir proje var`, `kule zaten açık, adı k2`, `açılamadı: …` (çalışma kopyası değil). Aynı çalışma kopyasıyla kapalı bir proje yerinde yeniden açılır. Açılan projeler çalıştırmalar arasında hatırlanır; başka bir çalışma kopyasında ikinci bir imece'ye ilkinden `:open` kullanması söylenir. `$`: yeni bir lider oturumu. Demo bir liderle, iki işçiyle ve üç kartla sahte bir proje ekler. Bkz. [projeler](../concepts/projects.md). Commit'i olan bir git checkout'u olmayan bir klasör reddedilir: `… is not a git checkout with a commit: a team works in git worktrees …`; orada `:solo` açılır.

### `:output`

`O` gibi, eldeki projenin son `!kmt`'sini bir mesajda alıntılar: komut satırı eldeki agent'a (yoksa projenin başına) açılır; yazdığınız satırın altında komutun nasıl bittiği ve gösterdiği son 40 satır, temizlenmiş, bir `text` çiti içinde durur. `enter`a basana kadar hiçbir şey gönderilmez. Çıktı yalnızca bellekte tutulur, proje başına en yenisi. Henüz yoksa: `bu projede henüz !komut çalışmadı`. Menü notu: komut, `(cargo test)`. Bkz. [komut satırı](../screen/command-line.md).

### `:pause`

Yalnız demo, `space` gibi: simülasyonu durdurur ya da sürdürür. Menü notu: `(açık)` / `(kapalı)`.

### `:peek [@NAME]`

Ajanın odağı görünümün üstünde bir pencerede, `K` gibi; ad yoksa eldeki ajan (salonda seçili olan, bekleyen kalemin ya da günlük satırının ardındaki, ya da yakınlaşılmış olan). Alttaki görünüm kıpırdamaz. Pencerede `j` `k` (ya da oklar) salonun sırasında sonraki / önceki ajana geçer, `enter` odağın tamamını açar, `@` ona bir mesaj başlatır, `?` yardımı açar, `esc` ya da `K` kapatır; başka tuşlar bir şey yapmaz. Bir ajana böyle bakmak bitmiş turunu görmüş sayılır. Bkz. [salon](../screen/salon.md).

### `:pending`

Çıkmadan önce `q s`'nin gösterdiği liste: tur ortasındaki ajanlar ve incelenmemiş değişiklikler; orada `q` çıkar, `W` sonraki bekleyeni açar.

### `:pin [NAME]`

`NAME` projesini ya da eldekini salonun en üstüne sabitler; yeniden kaldırır. Sabit projeler önce gelir, açıldıkları sırayla, ve salon kısaysa en son katlanır; sabit bir başlık `▴` taşır. Flaş: `kule salonun en üstüne sabitlendi` / `kule artık sabit değil`. Depoda saklanır; kapalı bir proje yeniden açıldığı zaman için sabitini korur. Menü notu etki edeceği projeyi gösterir, zaten sabitse `▴ kule`. Proje başına bir renk yapılmamıştır.

### `:plan [on|off]`

Eldeki projenin plan kapısı ([plan](../concepts/messages-and-board.md#plan)): `on` (varsayılan) lidere ikinci işçiyi almadan önce planını SENİ BEKLEYEN'e `▤` ögesi olarak koydurur; `off` uygun gördüğü gibi işe almasına izin verir, yine de gönderdiği bir plan `the person turned the plan gate off for this project` yanıtını alır ve saklanmaz. Yalın, kapının nerede durduğunu söyler: `kule için plan kapısı açık: ikinci işçiden önce onayınızı alan bir plan` / `kule için plan kapısı kapalı: lider uygun gördüğü gibi işe alır`. Depoda proje başına saklanır. Menü notu on / off gösterir.

### `:quit`

Çıkar, `q` gibi (`:q`): önce sorar (`imece kapatılsın mı? bir daha q çıkar · başka bir tuş kalır`), varsa tur ortasındaki ajanları ve incelenmemiş değişiklikleri sayarak; `s` listeler.

### `:recall [WORDS]`

Herkesin yaptığını arar, `H` gibi: imece'nin tuttuğu her günlük satırı ve her iz satırı, her projenin ve arşivin, önceki çalıştırmalar da. Komut satırı üstünde `WORDS` ile açılır (başında `› hatırla:`) ve tamamlama menüsü yazılan her sözcüğün satırın bir sözcüğünü başlattığı satırları listeler (büyük küçük harf ve Türkçe harfler fark etmez: `deploy fix` `fixed the deploy script` satırını bulur, `ploy` bulmaz), en yenisi önce, en çok 50, her biri günlüğün söylediği gibi tek satırda, ajan, proje ve ne zaman notuyla (bugünse saat, değilse tarih). Bir günlük satırı taşıdığı metinlerle (bir mesajın ilk satırı, bir neden, bir komut, bir yol) ve ajanının adıyla bulunur; bir iz satırı kendisiyle ve ajanının adıyla. `tab`, `↑` ve `↓` ilerler; `enter` seçili satıra gider: bir günlük satırının konuşmasına, satır günlükte seçili; bir arşivin satırı arşivin salt okunur görünümünde; bir iz satırının ajanına, ham akışı açık. Ekranın tutmadığı bir satır (bir açılışın yüklediği günlük satırlarından eski, ya da kapalı bir projenin izi) yerine söylenir: `o satır ekranda değil: …`. Hiçbir şey bulunmazsa: `WORDS geçen bir günlük ya da iz satırı yok`. `esc` önce menüyü, sonra satırı kapatır. Bir arşivin görünümünde yoktur, o yalnız kendi arşivine bakar. Demo bellekte tuttuğunda arar. Deponun tam metin dizininde tutulur ([dosyalar](files.md)); satır gidince dizin de onu unutur (90 gün, ajan başına 200 iz satırı).

### `:rename NAME`

Eldeki projeyi ve liderini `lead-NAME` olarak adlandırır (0 numaralı projenin `lead`'i de). Oturumlar sürer; projenin her canlı ajanı sonraki isteminde bir not alır ve eski ada bir `send` takım listesiyle yanıtlanır. `:open` ile aynı ad kuralı. Flaş: `kule artık k2; lead-k2 adına yaz`.

### `:restore [NAME]`

Bir arşivi geri getirir: `NAME` başlıklı en yenisini, yoksa `NAME` adlı projenin en yeni arşivini; ad verilmezse eldeki projenin adını taşıyan en yeni arşivi (yani bir `:archive`'ın hemen ardından `:restore`, bekleme bittikten sonra onu geri alır). `:restore ` sonrasındaki menü arşivlerin başlıklarını sunar, en yenisi önce, her biri projesinin adı ve arşivlendiği günle (`first epic   kule · 2026-11-02`).

Arşiv kaldırıldığı gibi yeniden açılır: ajanları oturumları, postaları, notları ve değişiklikleriyle uykuda; kartları, SENİ BEKLEYEN'deki kalemleri ve günlük satırları yeniden görünür; başı seçilir. Siz yazana dek hiçbir şey uyanmaz, hiçbir şey harcanmaz: o zaman her ajan sahip olduğu oturumda sürer (Claude Code onu hâlâ saklıyorsa; yoksa kaldığı yer anlatılarak yeni bir oturumda), önce projenin kaldırılıp geri getirildiği, arşivin yarıda kestiği tur da (yeniden çalıştırılmaz) ona söylenir.

Bir çalışma kopyasında tek bir açık takım olur; bu yüzden arşivin çalışma kopyasında açık olan takım yer açar (solo bir arşiv için: onun adındaki açık solo). Bir şey yaptıysa (bir kart, işe alınmış bir ajan, harcanan ya da söylenen bir şey, bir oturum) sırası gelince adı ve günleriyle arşivlenir, yapmadıysa kapatılır; böylece boş arşivler birikmez. Flaş: `kule “first epic” geri geldi: agent'ları sen yazana kadar uyuyor`; günlük: `kule projesinin “first epic” arşivini geri getirdin; orada açık olan “kule 2026-11-02” adıyla arşivlendi` (ya da önünde bir `kule projesini kapattın` satırı).

Hiçbir şey değişmeden reddedilir: o başlıkta ya da adda arşiv yoksa (`x başlıklı ya da adlı arşiv yok`, `kule projesinin arşivi yok`), klasörü gitmişse (`/src/kule is not there any more`), bir takımın klasörü artık ilk commit'i olan bir git çalışma kopyası değilse ya da adını başka bir açık proje taşıyorsa (`a project named kule is open already`: önce onu yeniden adlandırın ya da kapatın). Geri getirilip sonra yeniden arşivlenen bir arşiv yeni bir başlık alır. Demo da aynı biçimde geri getirir (klasörleri kendisinindir, asla gitmez). Bkz. [oturumlar](../concepts/sessions.md#arşivler) ve [yeni bir epic](../scenarios/new-epic.md) senaryosu.

### `:review [on|off]`

Bir değişikliği taze bir Claude Code oturumuna okutur; bulduğunu değişikliğin dosyalarının üstünde bir not olarak söyler. Yalın, eldeki değişiklik için (açık olan, yoksa SENİ BEKLEYENLER'de seçili olan) şimdi ister; `on` beklemeye gelen her değişiklik için ister, `off` bunu durdurur (depoda hatırlanır, varsayılan kapalı); elde değişiklik yokken yalın, hangisinin açık olduğunu söyler. Gelince değişikliğin ekranı `✎ inceleme · sonnet · $0.04` ve ilk altı satırını gösterir (`R` tamamını gösterir); günlük her incelemeyi ilk satırıyla, her başarısızlığı nedeni ve maliyetiyle söyler.

Not bir öneridir: hiçbir şey onu beklemez ve bir değişikliği asla tutmaz; uygulamak ya da reddetmek yine sizdedir. Kim okur: işçininkinden hiçbir şey bilmeyen kendi oturumu; sizin `--tier standard=…`, yoksa `sonnet` ile, araçsız ve inceleme başına 0,10 $ sert sınırla. Okuyan yalnızca işçinin ne yaptığını söylediğini, dosya listesini ve yamayı (en çok 64 KiB) görür. Bir gün (UTC) `:commit`'inkilerden ayrı olarak 20 inceleme ve 2,00 $ tutar; her çağrıdan önce bütçeden ayrılır, sonra kapatılır; aynı anda biri çalışır, kalanlar sırasını bekler. Elde değişiklik yoksa, incelemesi zaten varsa, biri sürüyorsa, bütçede sınırdan az kalmışsa ya da gün dolmuşsa `inceleme yok: …` ile reddedilir. Aynı işçinin daha yeni bir değişikliği kendi incelemesini alır. Bkz. [değişiklikler](../concepts/changes.md#ikinci-bir-okuma). Demo da aynı biçimde inceler, birkaç tik sonra değişikliğin dosyalarından yanıtlar.

### `:salon`

Salon, `1` gibi.

### `:settle`

Salonun seçili satırını kaldırır, oradaki `s` gibi: bir işçinin satırını (yardımcıları ve araştırmacısıyla) ya da bir projenin başlığında veya liderinde bütün projeyi, size yeniden ihtiyacı olana dek salondan çıkarır. Önce salona gider. Bir şeyi sizi bekliyorsa ya da takılmışsa reddedilir: `kaldırılmadı: claude-2 seni bekliyor`. Bitmiş turları görülmüş sayılır. Flaş: `claude-2 kaldırıldı: sana ihtiyacı olunca döner`. Menü notu satırın adını verir. Bkz. [salon](../screen/salon.md).

### `:slower`

Yalnız demo, `-` gibi: simülasyon yavaşlar.

### `:solo [PATH] [CHOICE] [NAME]`

Bir solo oturum açar: lideri ve takımı olmayan, kendinizin sürdüğü tek bir ajan, kendi başına bir proje olarak ([solo oturumlar](../concepts/solo.md)). Üç sözcük herhangi bir sırada gelebilir: içinde `/` olan ya da `~` veya `.` ile başlayan sözcük checkout'un PATH'idir (varsayılan: eldeki projeninki); `claude` ile başlayan sözcük CHOICE'tur, `claude[:MODEL][@ÇABA]` (`claude`, `claude:opus@xhigh`, `claude@high`); kalan sözcük NAME'dir (varsayılan: dizinin adı, alınmışsa `-2`, `-3` ile numaralı). Ajan `solo-NAME`'dir, kendi worktree'sinde yazar ve bir yanıtla biten her turda worktree'sinin diff'ini size `Δ` olarak gösterir. Bir checkout'ta takımının yanında istendiği kadar solo olabilir. Flaş: `notes solo oturum olarak açıldı (~/src/kule); solo-notes adına yaz (claude opus@xhigh)`. Redler: iki sözcük aynı yeri isteyince kullanım satırı, `notes adında açık bir proje var`, `"a:b" proje adı olamaz: …`, `açılamadı: claude has no effort "bogus"; one of low, medium, high, xhigh, max`. `$`: yeni bir oturum. CHOICE yoksa önce bir ekran sorar: model (`default`, Claude Code'un kendisi, ya da takma adlarından biri; tam bir ad `:solo claude:` ardına yazılır), sonra çaba (`default` ya da düzeylerinden biri); `j k` ve `1`-`9` seçer, `enter` ileri gider, `esc` bir adım geri (ilkinde vazgeçer), `q` vazgeçer. Çıplak `:solo` ile yazılan bir PATH ya da NAME seçimi bekler. `:solo ` ardından tamamlama `claude`'u, sonra `claude@` ardından çabaları ve yola benzeyen bir sözcük için dizinleri önerir. Demo, yanıt veren ve ara sıra bir değişiklik gösteren sahte bir solo açar. Commit'i olan bir git checkout'u olmayan bir klasörde solo doğrudan klasörde çalışır, worktree ve `Δ` olmadan, ve başlığı `⌀ git yok` der ([git'siz bir klasörde](../concepts/solo.md#gitsiz-bir-klasörde)).

### `:story`

Alt yan panel, `J` gibi, hikâyeyi anlatır: ne olduğunu, karta göre, en yeni önce, en üstte kimi bekletiyor olduğunuzla (bkz. [hikâye](../screen/journal.md#hikâye)); yeniden: defter. Önce salona gider. Menü notu: `(açık)` / `(kapalı)`; depoda hatırlanır, varsayılan olarak açıktır.

### `:sync`

Projenin [Lonca](../concepts/projects.md#loncada) deposunun trunk'ını yanda çalışma kopyana getirir: `lonca fetch` onu `refs/lonca/trunk`'a koyar, dalın da fast-forward olabiliyorsa ilerler. Defter nerede durduğunu söyler: `kule, kule trunk'ına ilerledi`, `kule, kule trunk'ıyla aynı yerde` ya da `kule projesinin kendi commit'leri var: kule trunk'ı refs/lonca/trunk'ta, hiçbir şey kımıldamadı`. imece dalını asla merge, rebase ya da reset etmez, yoldaki iş ise git'in kendi sözleriyle durdurur. Lonca dışında, git yokken, depo yanıt vermezken ya da süren bir `lonca` komutu varken hemen reddedilir. Menü notu ne kadar geride olduğunu söyler: `(↓3)`; commit'in hiç trunk olmadıysa ve henüz hiçbir şey çekilmediyse `(↓?)`; aynı yerdeysen hiçbir şey. Demo aynı sayımı tablosunda tutar ve her üçüncü `:sync` senin kendi commit'lerini bulur.

### `:talk [@NAME]`

Ajanın sizinle konuşması, `v` gibi; ad yoksa eldeki ajan (henüz yoksa `claude-2 ile henüz konuşma yok`). Bkz. [konuşmalar](../screen/conversations.md).

### `:theme [NAME|FILE]`

Tema, `T` gibi (`:t`). Tek başına: sonraki yerleşik tema, sondan başa dönerek. Bir adla: `terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`'den biri, ya da Ghostty veya Kitty biçiminde bir renk dosyası (içinde `/` olan ya da bir dosya olan her şey). Bir ad yazılırken ya da `tab` ile üstüne gelinirken ekran onu giyer; `enter` tutar, `esc` bırakır. Flaş: `tema: nord`; yanlış biri: `tema solar: not one of terminal|dark|… and no such file`. Sonraki çalıştırma için hatırlanır. Bkz. [temalar ve dil](../screen/themes-and-language.md).

### `:tidy`

imece'nin durum dizininde hiçbir ajanın sahip olmadığı worktree'ler (bir çökme, silinen bir depo, hiç başlamamış bir çatal), bir pencerede: `SAHİBİ OLMAYAN WORKTREE'LER · 2`, her biri için adı, checkout'u ve tuttukları (`temiz`, ya da `değişen dosya: 3 · kendi commit'i: 1`) ile bir satır. `j` `k` `g` `G` gezinir, `a` temiz olanları siler, `d` eldekini (iş tutan biri önce sorar: `y` siler, başka her tuş bırakır), `esc` kapatır. Worktree'nin checkout'unda bir söküm betiği (`.imece/worktree-teardown.sh`) varsa önce worktree'de çalışır; henüz yanıtlanmamışsa pencerenin üstünde açılır (`a` evet, `r` bu çalıştırma için hayır, `esc` hiçbir şey silmez). Flaş nasıl gittiğini söyler: `silindi: claude-31`, `silinmedi: claude-32 (…)`. Bir işçinin geride kalan Lonca workspace'i de listelenir: `lonca diff` onu yalnız oturumu yaşarken okuyabilir, bu yüzden okunamayan biri `iş tutuyor olabilecek workspace: oturumu bitmiş` der ve yalnız `y` ile gider; önerdikleri Lonca'da kalır. Bir ajanın worktree'si asla listelenmez, asla silinmez. imece her başlangıçta bir kez bakar ve bulursa `sahibi olmayan 2 worktree var: :tidy listeler` der. Bkz. [projeler](../concepts/projects.md#sahibi-olmayan-worktreeler).

### `:timeline`

Zaman şeridi, `4` gibi: açık projelerin her agent'ı için son bir saatte bir şerit, her sütun ne yaptığı, her projenin altında mesajlar; `[` `]` bir sütunun uzunluğunu değiştirir. Bkz. [zaman şeridi](../screen/timeline.md).

### `:trace [@NAME]`

Ajanın ham akışı, `t` gibi: oturumunun söylediği her satır, veri olarak, en çok 200 satır tutulur. Ad yoksa eldeki ajan.

### `:unclear [inbox|journal]`

`:clear`'ın ekrandan kaldırdığını geri getirir: yalın ikisini, `inbox` ya da `journal` birini. Ne kadarının geri geldiğini söyler (`ekrana geri geldi: 2 bekleyen, 40 günlük satırı`) ya da `ekrandan kaldırılmış bir şey yok`.

### `:undo`

Bir `:close`, `:forget`, `:archive` ya da `:clear`'ın olmasını önler, `u` gibi; `U` ([`:now`](#now)) ise hemen yapar. İkisi de iş görmeden önce on saniye bekler, bu arada alt satır geri sayar: `◷ kule 7 sn sonra kapanıyor · u açık tutar`, `◷ claude-2 7 sn sonra oturumunu unutuyor · u tutar`. O sürede `u` her şeyi olduğu gibi bırakır (`kule açık kalıyor`, `claude-2 oturumunu tutuyor`); geri sayan bir şey yoksa `geri alınacak bir şey yok` der. Bir seferde biri bekler: başka bir `:close` ya da `:forget`, bekleyeni önce uygular. Biri geri sayarken çıkmak onu uygular. Bir ret (son açık proje, unutulacak oturum yok, kimsenin taşımadığı bir ad) hemen söylenir, hiçbir şey beklemez. Demo da canlı oturumlar da bekler.

### `:unfold`

Salonda her şeyi açar, `zR` gibi; açık bir konuşma varken onun her mesajını açar, sonra açılan konuşmalar da yine açık başlar.

### `:unsettle`

`s` ya da `:settle` ile kaldırılan her şeyi geri getirir ve salona gider. Flaş: `kaldırılan 3 şey salona döndü`. Menü notu kaç şeyin kaldırıldığını söyler.

### `:waiting`

Doğrudan SENİ BEKLEYEN'e, `w` gibi. Menü notu kaç kalemin beklediğini söyler.

### `:welcome`

İlk çalıştırma penceresi: imece nedir, Claude Code sürümü ve giriş yapılmış olup olmadığıyla (ya da giriş yapan komutla), ve başlamak için tuşlar. Ağa çıkmadan yeniden sorulur; cevabı gelene dek satırda `bakılıyor…` yazar. İlk başlangıçta ve Claude Code'un giriş yapmadığı bir başlangıçta kendiliğinden açılır. Herhangi bir tuş kapatır. Bkz. [ilk çalıştırma](../getting-started/first-run.md#hoş-geldin-penceresi).

### `:why [@NAME]`

Ajanı bu duruma neyin getirdiği, alt satırda: merdivendeki basamağı (boşta, bitti ve bakılmadı, başlıyor, çalışıyor, onayını bekliyor, takıldı), ne zamandan beri ve onu oraya getiren günlük satırı, günlüğün kendi sözcükleriyle: `claude-1: takıldı · 14:02 itibarıyla · claude-1 durdu: oturum bitti`. Postası sizi bekleyen bir ajanda sonunda kaç tane olduğu yazar: `· sen yanıtlayana dek bekleyen mesaj: 2`. Ad yoksa `:peek`'teki gibi eldeki ajan. "İtibarıyla", en eski kaleminin seni beklemeye başladığı an, yoksa durumunun son değiştiği andır; satır o andan ya da öncesinden en yeni günlük satırıdır. Günlükte artık yoksa (toplanmış, ya da ajan önceki bir çalışmadan gelmiş) bunu söyler. Yalnız baktığı için bir arşivin görünümünde de çalışır.


### `:yes [@NAME]`

Ajan turunu bitirene dek ne isterse evet der, sizi bekleyenlerdeki ya da odağındaki isteğinde `e` gibi; ad yoksa eldeki ajan. Beklediği istek ve diğer açık istekleri şimdi sürer, o ajanın turu (ya da oturumu) bitene dek sonraki her isteği de: komutlar, düzenlemeler, getirmeler, Claude Code'un sorduğu ne varsa. İzi `allowed by your yes for this turn: Bash: …` der, evet sürdükçe satırı ve odağı `✓ tur` der, günlük de ne zaman verildiğini ve kaç isteğe sormadan izin verip bittiğini söyler (`claude-1 turunu bitirdi, onun için evetin de bitti: 7 isteğe sormadan izin verdi`). Başka bir ajanın istekleri sizi beklemeye devam eder. Ajanın bekleyen bir isteği yoksa bunu söyler (`claude-1 şimdi bir evet istemiyor: …`). Demo da canlı oturumlar da. Bkz. [onaylar](../concepts/approvals-and-questions.md#bu-tur-evet).
## Ayrıca bkz.

- [Komut satırı](cli.md): aynı şeylerin başlangıçtaki bayrakları
- [Komut satırı (ekran)](../screen/command-line.md): yazma, tamamlama, readline tuşları, `!cmd` ve `/cmd`
- [Klavye](../screen/keyboard.md): her tuş
- [Dosyalar](files.md): `:export`, `:copy`, `:forget` ve `:open`'ın nereyi okuyup yazdığı
