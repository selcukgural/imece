---
title: Komutlar
description: imece'nin aldığı her `:` komutu; argümanları, karşılığı olan tuş, kısa adı, yalnızca demoda mı olduğu, `$` ile sonraki faturayı büyüten diye işaretlenip işaretlenmediği ve menü notunun gösterdiği.
---

# Komutlar

`:` ile başlayan bir satır imece'ye bir komuttur, bir ajana mesaj değil. Komut satırına yazarsınız (`:` onu açar), `tab` eşleşenleri değerleriyle sunar, `enter` çalıştırır. *Eyleyen* her tuşun (yalnızca hareket edenlerin değil) bir `:` adı da vardır; böylece her şey her yerden yazılarak yapılabilir. Tuş işleyicileri ve komutlar aynı kodu çağırır, bu yüzden birbirinden ayrı düşemezler.

## `:` satırı nasıl çalışır

- `:` sonra `tab` komutları listeler. Menü her komutun ardında şu an ne durumda olduğunu ya da neye etki edeceğini ayraç içinde gösterir: `budget (spent $3.10 of $20.00)`, `theme (dark)`, `lead (claude)`, `close (kule)`, `forget (claude-2)`, `next (2 waiting)`, `needs (off)`. Not çizilir, asla satıra eklenmez.
- Bir komutun ardındaki `$` sonraki faturayı büyütenleri işaretler: `:open` (yeni bir lider oturumu), `:solo` (yeni bir oturum), `:forget` (bütün brifingi önbelleksiz yeniden ödeyen yeni bir oturum) ve `:commit` (ücretli bir çağrı). Hiçbir `:` komutu kendi başına ücretli bir tur başlatmaz; `:budget` işaret yerine değerini gösterir.
- Kısa adlar: `:b` budget, `:c` close, `:f` forget, `:o` open, `:q` quit, `:t` theme. Bütünüyle yazılan kısa ad zaten bir komuttur; menü bir şey sunmaz ve `enter` çalıştırır (`:q` sonra `enter` hemen sorar).
- Argümanlar da tamamlanır: `:lang` sonra diller, `:lead` sonra satıcılar, `:notify` sonra üç kelimesi, `:theme` sonra yerleşik adlar, `:close` ve `:pin` sonra açık projeler, `:goto` sonra hiçbir şey (`&` satırının kendi menüsü var), `:history` sonra gönderilen satırlar, `:forget`, `:talk`, `:trace` ve `:peek` sonra açık projelerin ajanları (`@` ile ya da onsuz), `:open` ve `:export` sonra dizinler (`~` ev dizininiz, gizli olanlar yalnızca bir `.` sonrası, `tab` seçilenin içine adım atar).
- imece'nin bilmediği bir ad: `no command xyz; : and tab list them`. Canlıda yazılan yalnızca demoya ait bir komut: `:pause is for the demo: live agents run in real time`.
- `!cmd` kendi kabuğunuzda bir komut çalıştırır, `/cmd` ajanın kendi komut satırıdır; ikisi de `:` komutu değildir ([komut satırı](../screen/command-line.md)).

## Bütün komutlar

| Komut | Tuş | Kısa | Yalnız demo | `$` | Menü notu |
| --- | --- | --- | --- | --- | --- |
| `:awake` | — | | | | açık / kapalı |
| `:board` | `2` | | | | |
| `:budget USD` | — | `:b` | | | harcanan / bütçe |
| `:close [NAME]` | — | `:c` | | | eldeki proje |
| `:code` | `3` | | | | |
| `:commit` | — | | | `$` | |
| `:copy [PATH]` | — | | | | liste, varsa |
| `:defer` | `S` | | | | |
| `:effort LEVEL` | — | | | | eldeki solonun çabası ya da `default` |
| `:export [PATH]` | — | | | | `konuşma` ya da `#12` |
| `:faster` | `+` | | evet | | hız, `×2` |
| `:fold` | `zM` | | | | |
| `:forget [@NAME]` | — | `:f` | | `$` | eldeki ajan |
| `:goto [NAME]` | `&` | | | | |
| `:history [WORDS]` | `^r` | | | | |
| `:help` | `?` | | | | |
| `:hints` | `F` | | | | |
| `:lang [en\|tr]` | `L` | | | | `English` / `Türkçe` |
| `:lead VENDOR` | — | | | | birincil satıcı, ya da `ask` |
| `:mark` | `m` | | | | |
| `:model NAME` | — | | | | eldeki solonun modeli ya da `default` |
| `:needs` | `f` | | | | açık / kapalı |
| `:next` | `W` | | | | N bekliyor |
| `:notify [off\|bell\|text]` | — | | | | off / bell / text |
| `:open PATH [NAME]` | — | `:o` | | `$` | |
| `:output` | `O` | | | | çıktısını alıntıladığı komut |
| `:pause` | `space` | | evet | | açık / kapalı |
| `:peek [@NAME]` | `K` | | | | eldeki ajan |
| `:pending` | `q s` | | | | |
| `:pin [NAME]` | — | | | | eldeki proje, sabitse `▴` |
| `:quit` | `q` | `:q` | | | |
| `:rename NAME` | — | | | | eldeki proje |
| `:salon` | `1` | | | | |
| `:settle` | `s` | | | | seçili satırın ajanı ya da projesi |
| `:slower` | `-` | | evet | | hız, `×2` |
| `:solo [PATH] [CHOICE] [NAME]` | — | | | `$` | |
| `:talk [@NAME]` | `v` | | | | eldeki ajan |
| `:theme [NAME\|FILE]` | `T` | `:t` | | | kullanılan tema |
| `:trace [@NAME]` | `t` | | | | eldeki ajan |
| `:unfold` | `zR` | | | | |
| `:unsettle` | — | | | | kaç şeyin kaldırıldığı |
| `:waiting` | `w` | | | | N bekliyor |

Canlıda `:pause`, `:faster` ve `:slower` menüde yer almaz. Hareket eden tuşların (`j k`, `gg G`, `enter`, `esc`, `tab`) ve seçili satıra etki eden tuşların (`a r c n x`) `:` adı yoktur: yazmak tuştan yavaş olurdu ve satırın adlandırılması gerekirdi.

## Komutlar

### `:awake`

Makineyi uyanık tutmayı açar ya da kapar (`--awake` ile aynı). Açıkken ve herhangi bir ajan tur ortasındayken imece `caffeinate` (macOS) ya da `systemd-inhibit` (Linux) aracını çocuk olarak çalıştırır, hiçbir şey tur ortasında olmayınca bırakır. Flaş `bir ajan turdayken makine uyanık kalır` ya da `makine yeniden uyuyabilir` der; başlayamayan bir araç bir kez söylenir: `makine uyanık tutulamadı: …`. Çalıştırmalar arasında hatırlanmaz.

### `:board`

Pano görünümü, `2` gibi. Bkz. [pano ve kod haritası](../screen/board-and-code-map.md).

### `:budget USD`

Ajanlar çalışırken bütçeyi artırır ya da azaltır (`:b 80`). Değer, noktadan önce en çok dört rakam ve en çok iki ondalıkla dolardır (`80`, `2.50`); başkası: `80$ dolar değil; 80 ya da 2.50 gibi yaz`. Flaş `bütçe artık $80.00` der, günlük `bütçeyi $80.00 yaptın (önce $50.00)`, ve bütçenin azaldığı uyarısı yeniden çalabilir. Süren bir tur eski sınırında biter; sonraki yeni kalanla başlar, eskisi sınırına çarpmışsa yeni bir oturumda. Bütçe bütün projeler için tektir ve çalıştırmalar arasında saklanır. Menü notu: `(spent $3.10 of $20.00)`. Bkz. [bütçe ve sınırlar](../concepts/budget-and-limits.md).

### `:close [NAME]`

`NAME` projesini ya da eldekini kapatır (`:c`). Ajanlarının oturumları ve çalıştırmaları biter, postaları, notları, teklifleri ve değişiklikleri düşer, SENİ BEKLEYEN'deki kalemleri kapanır ve proje kapalı işaretlenir: salon onu atlar, aynı çalışma kopyası yeniden açılırsa yeni bir liderle onun yerini alır. Worktree'ler diskte kalır. Flaş: `kule kapatıldı: agent'ları ayrıldı; worktree'leri diskte kaldı`. Son açık proje reddeder: `kule son açık proje; q q çıkar`; kimsenin olmadığı bir ad: `x adında açık proje yok`. Demoda projenin ajanları aynı biçimde ayrılır. Bkz. [projeler](../concepts/projects.md).

### `:code`

Kod haritası, `3` gibi. Canlıda henüz ortak trunk olmadığını söyler (her ajan kendi worktree'sinde çalışır, gösterilecek lease yoktur); sembol lease'leri yapılmamıştır. Bkz. [pano ve kod haritası](../screen/board-and-code-map.md).

### `:commit`

Claude Code'un ucuz kademesine (`haiku`, ya da sizin `--tier claude:cheap=…`) eldeki projenin çalışma kopyasında stage'dekiler için bir commit mesajı sorar ve onu komut satırına `!git commit -m '…'` olarak koyar; düzenleyip `enter` ile kendi kabuğunuzda çalıştırır ya da `esc` ile bırakırsınız. imece hiçbir şey commit'lemez. Flaş `haiku modeline kule içinde stage'dekiler için commit mesajı soruluyor` der; günlük her öneriyi modeli ve maliyetiyle, her başarısızlığı nedeniyle söyler.

Para eder: her çağrı en çok 0,05 $ (çağrının kendi sert sınırı), bir gün (UTC) bütün projeler için en çok 20 çağrı ve 1,00 $; çağrıdan önce bütçeden ayrılır, sonra gerçek değerle kapatılır. Stage'de bir şey yoksa, gün dolmuşsa, bütçede sınırdan az kalmışsa ya da Claude Code PATH'te değilse `commit mesajı yok: …` ile reddedilir (harcama sınırı alan tek satıcı o; Codex ve Gemini liderleri bunun için Claude Code'a gerek duyar). Aynı stage ağacı yeniden sorulursa para etmez: son öneri geri gelir. Çağrı `git diff --cached --stat`'ı, yamayı (en çok 24 KiB) ve en yeni sekiz commit konusunu okur; araçsız, MCP sunucusuz, kendi kısa sistem istemiyle çalışır, hiçbir şey kaydedilmez. Demo, o projede uygulanan değişikliklerin başlıklarından `git commit --dry-run` olarak yanıtlar. Bkz. [kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md).

### `:copy [PATH]`

Bir işçinin başlangıcının worktree'sine kopyaladığı, git'in yok saydığı yolların listesi (`.env`, `node_modules`, `target`): bir worktree taze bir çalışma kopyasıdır, git'in yok saydığı orada yoktur. `:copy PATH` eldeki projeye bir yol ekler, `:copy PATH` yeniden onu düşürür (sondaki `/` aynı yoldur), `:copy` tek başına listeyi söyler: `kule çalışma ağaçlarına kopyalananlar: .env, target` ya da `kule çalışma ağaçlarına hiçbir şey kopyalanmıyor; :copy YOL git'in yok saydığı bir yolu ekler`.

Yol göreli, düz (glob yok, `..` yok, `.git` içinde değil, en çok 200 bayt), çalışma kopyasında var, bağlantı değil, çözümlenince çalışma kopyasının içinde ve git'çe yok sayılan olmalıdır (izlenen bir dosya worktree'de zaten vardır); yoksa `x kopyalanamaz: …`. Kopya her işçinin başlangıcında olur (yardımcılar işçilerinin worktree'sini paylaşır; lider için asla), yalnızca worktree'de olmayan yollar için, bir başlangıcın bütün yolları 2 GiB içinde, birimin elverdiği yerde APFS klonu ya da reflink olarak. Günlük neyin kopyalandığını ve neyin atlandığını söyler. Projeyle birlikte depoda saklanır. Bkz. [dosyalar](files.md) ve [projeler](../concepts/projects.md).

### `:defer`

SENİ BEKLEYEN'deki eldeki kalemi erteler ve sonrakini açar, `S` gibi. Bkz. [seni bekleyenler](../screen/waiting.md).

### `:effort LEVEL`

Eldeki solo oturumun sonraki turundan itibaren ne kadar derin düşüneceğini ayarlar: satıcısının çabalarından biri (Claude Code: `low`, `medium`, `high`, `xhigh`, `max`; Codex: `low`, `medium`, `high`, `xhigh`) ya da satıcının kendisi için `default`. Eldeki solo, seçili ajan soloysa odur, yoksa eldeki projenin solosu. Çalışan oturumu biter ve sonraki mesaj aynı transkript üzerinde, kaldığı yer anlatılarak yeni bir oturum başlatır; süreçten başka bir şey yitmez. Tur ortasında (`solo-notes değiştirilemedi: solo-notes is mid-turn; wait for its answer`), çabası olmayan bir satıcıda (`gemini has no effort setting; …`), satıcının olmayan bir düzeyde (`claude has no effort "turbo"; one of …`) ve elde solo yokken (`elde solo oturum yok: birini seç ya da :solo ile aç`) reddedilir. Flaş: `solo-notes sonraki turundan itibaren claude opus@max üzerinde; oturumu onunla yeniden başlar`; günlük: `solo-notes artık claude opus@max üzerinde çalışıyor`. Menü notu şimdiki çabayı gösterir, `(xhigh)` ya da `(default)`; `:effort ` ardından düzeyler tamamlanır. Seçimin model kısmı kalır. Bkz. [solo oturumlar](../concepts/solo.md).

### `:export [PATH]`

Açık olanı Markdown dosyası olarak yazar: açık konuşma (`# Konuşma: lead ⇄ claude-1`, her mesaj günü, saati, kimden kime ve türüyle bir başlık), yoksa kartın ekranı (`# #12 başlık`, projesi, sütunu ve dakikaları, KİM, HİKÂYE, onda sizi bekleyenler ve günlük satırları, en eski önce, hiçbiri katlanmamış). Açık bir şey yoksa: `dışa aktarılacak bir şey yok: önce bir konuşma (v) ya da iş (#N) aç`.

`PATH` bir dosya, ya da içinde dosya adlandırılacak bir dizindir; yoksa imece'nin başlatıldığı dizin. Ad neyi ve ne zaman olduğunu söyler: `imece-conversation-20261031-1400.md`, `imece-card-12-20261031-1400.md`. Zaten orada olan bir dosyanın üstüne asla yazılmaz; flaş `… dosyasına yazıldı` ya da `… yazılamadı: …` der. Her saatin günü vardır, her satır ekranla aynı temizlikten geçer. Menü notu neyi yazacağını söyler: `(konuşma)` ya da `(#12)`.

### `:faster`

Yalnız demo, `+` gibi: simülasyon hızlanır. Menü notu hızı gösterir (`×2`).

### `:fold`

Salonda her projeyi ve her işçinin yardımcılarını katlar, `zM` gibi; başka bir görünümden önce salona gider. Açık bir konuşma varken bunun yerine onun her mesajını tek satıra katlar, bu çalıştırmada sonra açılan konuşmalar da katlı başlar. Bkz. [salon](../screen/salon.md) ve [konuşmalar](../screen/conversations.md).

### `:forget [@NAME]`

Ajana satıcı oturumunu unutturur (`:f`): çalışan oturum biter (çalıştırması öldürülür, bitmemiş bir istem kesilmiş olarak tutulur, uyanışta anlatılır ve yeniden çalıştırılmaz), hatırlanan oturum satıcının ondan tuttuğuyla gider (Claude'un transkripti, Codex'in iş parçacığı, Gemini'nin sohbet dosyaları), ajan kendisiyle konuşulana dek uyur ve sonraki mesajı tam brifingle yeni bir oturum başlatır. Ad yoksa eldeki ajan. Flaş: `claude-2 oturumunu unuttu; sonraki mesaj yeni bir oturum başlatır, kaldığı yer anlatılır`; unutacak şeyi olmayan ya da gitmiş bir ajan için reddedilir: `claude-2 oturumunu koruyor: it has no session to forget`. Günlük: `claude-2 oturumunu unutturdun; yeni bir oturumla başlıyor, kaldığı yer anlatılacak`. `--fresh` başlangıçta her ajan için yapar. `$`: yeni oturum bütün brifingi yeniden öder. Bkz. [oturumlar](../concepts/sessions.md).

### `:goto [NAME]`

`NAME` adlı ajana (önünde `@` olabilir), yoksa o adlı açık projeye gider: salon, satırı seçili, projesi açılmış, bir yardımcının işçisi açılmış ve satırı gizleyecekse `f` bırakılmış; bir proje adı başlığını seçer. Büyük küçük harf ve Türkçe harfler fark etmez. O adda bir şey yoksa: `"zzz" adında gidilecek bir yer yok: bir agent ya da proje`, ve hiçbir şey kımıldamaz. Tek başına `&` satırını açar: yerler şu anki hâliyle, sizi bekleyenler önde (takılan, sonra bekleyen, sonra bitmiş ve görülmemiş), kalanlar durumuna göre (çalışan boştakinden önce), sonra açık projeler, her ajan glifi ve durum sözcükleriyle. Satır açıkken liste donar, böylece yazarken numaralar değişmez; `&` hemen ardından `1`-`9` bir rakam anında gider, `&` hemen ardından `#` kart satırına döner, `!` yalnız sizi bekleyenleri tutar, rakamlar bir numaranın başıyla eşleşir, geri kalanı adların içinde arar. `enter` imlecin altındakine, ya da yazılan ada gider. Bkz. [salon](../screen/salon.md).

### `:history [WORDS]`

Gönderdiğiniz satırları arar, `^r` gibi: komut satırı üstünde `WORDS` ile açılır ve tamamlama menüsü gönderilenlerden her sözcüğü geçenleri listeler (büyük küçük harf ve Türkçe harfler fark etmez), en yenisi önce, her satır bir kez ve ne kadar önce gönderildiğiyle; hiçbir şey yazılmamışsa hepsi. `tab`, `↑` ve `↓` aralarında ilerler, `enter` seçili satırı komut satırına bütün olarak koyar (birkaç satırlık bir satır olduğu gibi gelir), yeniden `enter` ile göndermek ya da önce değiştirmek için; hiçbiri eşleşmezse `enter` sözcükleri tutar ve aramayı bitirir. `esc` menüyü, sonra satırı kapatır. Yazarken `^r` yazılanı sözcük olarak alır. Henüz bir şey gönderilmemişse: `geri getirilecek bir şey gönderilmedi henüz`. Gönderilen her satır tutulur (bir mesaj, bir `:` komutu, bir `!kmt`; yalnız bir yere giden `#12`, `&3`, `:goto` ve `:history` hariç), çalıştırmalar arasında da: en yeni 500'ü sonraki başlangıçta geri gelir. Metnin ilk ya da son satırındayken `↑` ve `↓` aynı satırlarda aramasız gezer. Bkz. [klavye](../screen/keyboard.md) ve [dosyalar](files.md).

### `:help`

`?` yardımı: iki dilde her tuş ve komut, glif açıklamaları; içinde `/` her kelimeyle süzer. Bkz. [klavye](../screen/keyboard.md).

### `:hints`

İpucu harfleri, `F` gibi: salon ekranında seçilebilen her satırın (proje başlıkları, ajanlar, bir kartın ekranındaki kişiler, bekleyen kalemler, günlük satırları) başına bir ya da iki harflik bir etiket. Bir etiketi yazmak satırını kendi panelinde seçer; son harfi büyük yazılırsa onu `enter` gibi açar da. `backspace` bir harfi geri alır, `esc` kapatır. O arada ekrandan çıkan bir satır: `o satır ekrandan çıktı`. Salon ekranı dışında: `burada harf verilecek satır yok: F salon ekranında çalışır`. Bkz. [salon](../screen/salon.md).

### `:lang [en|tr]`

Ekranın dili, `L` gibi. Tek başına öteki dile geçer; `:lang en` ya da `:lang tr` birini seçer. Başka bir şey: `de bir dil değil: en ya da tr`. Menü notu kullanılan dili gösterir. Yalnızca bu çalıştırma için; sonraki başlangıç `--lang` ya da `IMECE_LANG`'i okur.

### `:lead VENDOR`

Sonraki başlangıçtan itibaren sormadan kimin lider olacağı: `claude`, `codex` ya da `gemini` o satıcıyı birincil yapar (`bir sonraki açılıştan itibaren claude lider, sorulmadan`); `ask` unutturur (`bir sonraki açılışta kimin lider olacağı sorulur`). Başka bir şey: `x claude, codex, gemini ya da ask değil`. Birincil satıcı depoda saklanır; komut satırındaki `--lead` bir çalıştırma için onu geçer, PATH'te olmayan bir birincil soru ekranında söylenir, asla sessizce değiştirilmez. Menü notu: `(claude)` ya da `(ask)`. Bkz. [satıcılar](../concepts/vendors.md).

### `:mark`

Eldeki satırı sonra dönmek için işaretler, `m` gibi: açık konuşmada, yoksa günlükte; yeniden kaldırır. İşaretli satır `✱` taşır, çalıştırmalar arasında kalır ve asla katlanmaz, temizlenmez ya da yüklemeden dışarıda kalmaz; `'` sonraki işaretli satıra gider. Flaş: `işaretlendi: ' ona geri getirir`. Bkz. [günlük](../screen/journal.md).

### `:model NAME`

Eldeki solo oturumun sonraki turundan itibaren çalıştığı modeli ayarlar: bir takma ad (Claude Code için `opus`, `sonnet`, `haiku`, `fable`; Gemini CLI için `auto`, `pro`, `flash`, `flash-lite`) ya da satıcının aldığı biçimde tam bir ad (`gpt-5.6-terra`), ya da satıcının kendisi için `default`. `:effort` ile aynı kurallar, redler, flaş ve günlük satırı; model adı olmayan bir ad (`a b`, `-x`) reddedilir. `:model ` ardından takma adlar tamamlanır; tam bir ad yazılır. Seçimin çaba kısmı kalır.

### `:needs`

Salon yalnızca sizi bekleyenleri gösterir (sizi bekleyen, takılmış, bitmiş ve bakılmamış), `f` gibi; yeniden, herkesi. Önce salona gider. Menü notu: `(açık)` / `(kapalı)`.

### `:next`

Sizi bekleyen sonraki kalemi her yerden açar, `W` gibi. Menü notu kaç kalemin beklediğini söyler: `(2 bekliyor)`.

### `:notify [off|bell|text]`

Başka bir penceredeyken sizi bekleyen bir şey geldiğinde nasıl haber alacağınız: `bell` terminalin zilini çalar (her terminalin bunun için bir ayarı vardır: ses, sekmede rozet, flaş; tmux'ta `monitor-bell`), `text` imece'nin sözleriyle bir masaüstü bildirimi gönderir (OSC 9: iTerm2, WezTerm, Ghostty; diziyi tanımayan bir terminal yok sayar, orada `bell` seçin), `off` (varsayılan) ikisi de değil. Yalın `:notify` sıradakine geçer: off, bell, text, off. Flaş `başka penceredeyken bir şey beklerse zil çalar`, `başka penceredeyken bir şey beklerse masaüstü bildirimi gelir` ya da `başka penceredeyken haber yok` der; başka bir kelime: `beep off, bell ya da text değil`. Menü notu değeri gösterir; depoda hatırlanır.

Ne çalar: SENİ BEKLEYEN'e yeni düşen bir kalem (değişiklik, soru, cevap, onay isteği, hassas bir şey) ya da takılan bir ajan (bloke, oturumu durmuş), terminal odakta değilken geldiyse ve beş saniye sonra hâlâ oradaysa, her biri bir kez. Bildirim türü ve ajanı söyler, ajanın sözlerini asla: `imece · claude-1 bitirdi: bir değişiklik seni bekliyor`, `imece · codex-2 sana soruyor`, `imece · gemini-1 takıldı`; aynı anda birkaçı tek zil ya da sayıyla tek bildirimdir (`imece · 3 şey seni bekliyor`). Siz ayrılırken ekranda olan hiç çalmaz; pencereye dönünce çalmak üzere olan düşer: artık ekranda. Terminal odağı bildirmeli (tmux'ta `focus-events on` gerekir); bildirmeyen hiç çalmaz. Bkz. [SENİ BEKLEYEN](../screen/waiting.md).

### `:open PATH [NAME]`

`PATH`'teki git çalışma kopyasını kendi başına bir proje olarak açar (`:o`), takımın lider satıcısında (aynı model ve kademelerle) `lead-NAME` adlı yeni bir liderle. `NAME` varsayılan olarak dizinin adının güvenli hali: harf, rakam, `.`, `_`, `-` ile 1-24 karakter (kalanlar `-` olur); yanlış bir ad reddedilir: `"x y" proje adı olamaz: 1-24 harf, rakam, . _ -`. `~` ev dizininizdir; boşluklu bir yol ayrıştırılmaz. Flaş: `kule açıldı (/Users/me/src/kule); lead-kule adına yaz`. Redler: `:open YOL [AD], YOL'daki git çalışma kopyasını proje olarak açar` (yol yok), `kule adında açık bir proje var`, `kule zaten açık, adı k2`, `açılamadı: …` (çalışma kopyası değil). Aynı çalışma kopyasıyla kapalı bir proje yerinde yeniden açılır. Açılan projeler çalıştırmalar arasında hatırlanır; başka bir çalışma kopyasında ikinci bir imece'ye ilkinden `:open` kullanması söylenir. `$`: yeni bir lider oturumu. Demo bir liderle, iki işçiyle ve üç kartla sahte bir proje ekler. Bkz. [projeler](../concepts/projects.md). Commit'i olan bir git checkout'u olmayan bir klasör reddedilir: `… is not a git checkout with a commit: a team works in git worktrees …`; orada `:solo` açılır.

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

### `:quit`

Çıkar, `q` gibi (`:q`): önce sorar (`imece kapatılsın mı? bir daha q çıkar · başka bir tuş kalır`), varsa tur ortasındaki ajanları ve incelenmemiş değişiklikleri sayarak; `s` listeler.

### `:rename NAME`

Eldeki projeyi ve liderini `lead-NAME` olarak adlandırır (0 numaralı projenin `lead`'i de). Oturumlar sürer; projenin her canlı ajanı sonraki isteminde bir not alır ve eski ada bir `send` takım listesiyle yanıtlanır. `:open` ile aynı ad kuralı. Flaş: `kule artık k2; lead-k2 adına yaz`.

### `:salon`

Salon, `1` gibi.

### `:settle`

Salonun seçili satırını kaldırır, oradaki `s` gibi: bir işçinin satırını (yardımcıları ve araştırmacısıyla) ya da bir projenin başlığında veya liderinde bütün projeyi, size yeniden ihtiyacı olana dek salondan çıkarır. Önce salona gider. Bir şeyi sizi bekliyorsa ya da takılmışsa reddedilir: `kaldırılmadı: claude-2 seni bekliyor`. Bitmiş turları görülmüş sayılır. Flaş: `claude-2 kaldırıldı: sana ihtiyacı olunca döner`. Menü notu satırın adını verir. Bkz. [salon](../screen/salon.md).

### `:slower`

Yalnız demo, `-` gibi: simülasyon yavaşlar.

### `:solo [PATH] [CHOICE] [NAME]`

Bir solo oturum açar: lideri ve takımı olmayan, kendinizin sürdüğü tek bir ajan, kendi başına bir proje olarak ([solo oturumlar](../concepts/solo.md)). Üç sözcük herhangi bir sırada gelebilir: içinde `/` olan ya da `~` veya `.` ile başlayan sözcük checkout'un PATH'idir (varsayılan: eldeki projeninki); bir satıcı adıyla başlayan sözcük CHOICE'tur, `SATICI[:MODEL][@ÇABA]` (`claude`, `claude:opus@xhigh`, `codex@high`, `gemini:pro`); kalan sözcük NAME'dir (varsayılan: dizinin adı, alınmışsa `-2`, `-3` ile numaralı). Ajan `solo-NAME`'dir, kendi worktree'sinde yazar ve bir yanıtla biten her turda worktree'sinin diff'ini size `Δ` olarak gösterir. Bir checkout'ta takımının yanında istendiği kadar solo olabilir. Flaş: `notes solo oturum olarak açıldı (~/src/kule); solo-notes adına yaz (claude opus@xhigh)`. Redler: iki sözcük aynı yeri isteyince kullanım satırı, `notes adında açık bir proje var`, `"a:b" proje adı olamaz: …`, `açılamadı: codex is not on PATH here, or imece does not drive it`, `açılamadı: claude has no effort "bogus"; one of low, medium, high, xhigh, max`. `$`: yeni bir oturum. CHOICE yoksa önce bir ekran sorar: satıcı (imece'nin burada çalıştırabildikleri, lider satıcı önce), sonra model (`default`, satıcının kendisi, ya da takma adlarından biri; tam bir ad `:solo SATICI:` ardına yazılır), sonra çaba (`default` ya da satıcının düzeylerinden biri; çabası olmayan Gemini'de atlanır); `j k` ve `1`-`9` seçer, `enter` ileri gider, `esc` bir adım geri (ilkinde vazgeçer), `q` vazgeçer. Çıplak `:solo` ile yazılan bir PATH ya da NAME seçimi bekler. `:solo ` ardından tamamlama satıcıları, sonra `claude@` ardından o satıcının çabalarını ve yola benzeyen bir sözcük için dizinleri önerir. Demo, yanıt veren ve ara sıra bir değişiklik gösteren sahte bir solo açar. Commit'i olan bir git checkout'u olmayan bir klasörde solo doğrudan klasörde çalışır, worktree ve `Δ` olmadan, ve başlığı `⌀ git yok` der ([git'siz bir klasörde](../concepts/solo.md#gitsiz-bir-klasörde)).

### `:talk [@NAME]`

Ajanın sizinle konuşması, `v` gibi; ad yoksa eldeki ajan (henüz yoksa `claude-2 ile henüz konuşma yok`). Bkz. [konuşmalar](../screen/conversations.md).

### `:theme [NAME|FILE]`

Tema, `T` gibi (`:t`). Tek başına: sonraki yerleşik tema, sondan başa dönerek. Bir adla: `terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`'den biri, ya da Ghostty veya Kitty biçiminde bir renk dosyası (içinde `/` olan ya da bir dosya olan her şey). Bir ad yazılırken ya da `tab` ile üstüne gelinirken ekran onu giyer; `enter` tutar, `esc` bırakır. Flaş: `tema: nord`; yanlış biri: `tema solar: not one of terminal|dark|… and no such file`. Sonraki çalıştırma için hatırlanır. Bkz. [temalar ve dil](../screen/themes-and-language.md).

### `:trace [@NAME]`

Ajanın ham akışı, `t` gibi: oturumunun söylediği her satır, veri olarak, en çok 200 satır tutulur. Ad yoksa eldeki ajan.

### `:unfold`

Salonda her şeyi açar, `zR` gibi; açık bir konuşma varken onun her mesajını açar, sonra açılan konuşmalar da yine açık başlar.

### `:unsettle`

`s` ya da `:settle` ile kaldırılan her şeyi geri getirir ve salona gider. Flaş: `kaldırılan 3 şey salona döndü`. Menü notu kaç şeyin kaldırıldığını söyler.

### `:waiting`

Doğrudan SENİ BEKLEYEN'e, `w` gibi. Menü notu kaç kalemin beklediğini söyler.

## Ayrıca bkz.

- [Komut satırı](cli.md): aynı şeylerin başlangıçtaki bayrakları
- [Komut satırı (ekran)](../screen/command-line.md): yazma, tamamlama, readline tuşları, `!cmd` ve `/cmd`
- [Klavye](../screen/keyboard.md): her tuş
- [Dosyalar](files.md): `:export`, `:copy`, `:forget` ve `:open`'ın nereyi okuyup yazdığı
