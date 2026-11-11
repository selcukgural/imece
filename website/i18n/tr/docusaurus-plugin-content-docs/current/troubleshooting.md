---
title: Sorun giderme
description: Bir şey ters gidince ne görürsünüz, imece bunu neden yapar ve ne yapmalısınız.
---

# Sorun giderme

imece kapalı düşer (fail closed): bir dosya, bir oturum ya da bir değişiklik beklediği gibi değilse onarmak ya da tahmin etmek yerine reddeder ve söyler. Bu sayfa o retleri ve her biri için ne yapılacağını listeler. Mesajlar ekranın ya da terminalin yazdığı gibi alıntılanmıştır; terminalde `imece: …` imece'nin başlamadığı demektir (çıkış kodu 1), flash alt satırın üstündeki tek satırlık mesajdır, günlük ise ne olduğunu saklar. Oturum katından gelen mesajlar (oturum açma, fiyat, araç listesi, worktree) kaynakta İngilizcedir ve ekrana öyle gelir; burada da öyle alıntılanır.

## Başlangıç

Listeye girmeden önce: `imece doctor` bir açılışın ihtiyaç duyduğu her şeyi tek seferde denetler (Claude Code girişi, git, çalışma kopyası, Lonca, terminal, dosyalarınız) ve yanlış olan için ne yapılacağını söyler; hiçbir şey başlatmaz ([imece doctor](getting-started/doctor.md)).

### `another imece is running on …; open the other checkout from it with :open`

**Ne görürsünüz.** `imece` o satırla hemen durur.

**Neden.** Depoyu aynı anda tek imece tutar (`imece.sqlite.lock`, `flock` ile). Tek SQLite dosyasında ve tek kesede iki yazar birbirinin görüşünü bozardı.

**Ne yapmalı.** Çalışan imece'ye gidin ve öteki checkout'u kendi lideriyle ayrı bir proje olarak eklemek için `:open YOL` yazın. Çalışan imece yoksa (sonuncusu öldürüldü), kilit süreci bitince işletim sistemi tarafından bırakılır; kalan bir kilit, hâlâ bir sürecin orada olduğu demektir (`pgrep imece`).

### `…/imece.sqlite: … imece left the file as it was. To start afresh, move it somewhere else …`

**Ne görürsünüz.** `imece: <yol>: <neden>`, sonra `imece left the file as it was. To start afresh, move it somewhere else with its <yol>-wal and <yol>-shm files, if they are there; the next start makes a new one.` Neden `the file was written by a newer imece (schema 11, this one knows 9)`, `file is not a database` ya da tanımadığı bir satır olabilir.

**Neden.** imece okuyamadığı bir dosyayı asla taşımaz, onarmaz ya da üstüne yazmaz: içindekiler hâlâ istenebilir ve daha yeni bir imece hâlâ okuyabilir.

**Ne yapmalı.** Ya onu yazan imece'yi derleyin (şema numarası daha yeni olduğunu söyler) ya da yeniden başlamak için dosyayı `-wal` ve `-shm` kardeşleriyle kenara taşıyın. Hiçbir şey kendiliğinden silinmez.

### Claude Code PATH'te yok

**Neden.** imece Claude Code'u sürer ve onu bulamadı. Kurun (https://claude.com/claude-code), oturum açın ve `claude`'un imece'yi başlattığınız kabuğun PATH'inde olduğundan emin olun.

### `claude giriş yapmamış: :welcome nasıl yapılacağını söyler`

**Neden.** Başlangıçta imece Claude Code'a giriş yapılmış olup olmadığını sordu (`claude auth status`) ve hayır dedi; sen yazıyordun, bu yüzden hoş geldin penceresi satırın üstüne açılmadı.

**Ne yapmalı.** `:welcome` giriş yapan komutu gösterir (`claude auth login`); onu bir kabukta çalıştırın (`!claude auth login` imece'nin içinden çalıştırır), sonra lidere yeniden yazın.

### `--projects is for --demo` / `--agents: a leader hires at most 16`

**Neden.** `--projects`, `--researchers` ve `--seed` simülasyona aittir; canlıda `--agents` 0-16 (varsayılan 4), `--helpers` 0-8 (varsayılan 2). Mesajın ardından kullanım metni gelir.

### `… is not a git checkout with a commit: a team works in git worktrees …`

**Neden.** imece git checkout'u olmayan ya da henüz hiçbir şey commit'lenmemiş bir klasörde başladı (ya da `:open` çalıştı): bir takımın işçilerinin her biri bir git worktree'sinde çalışır, bu da bir commit'ten yapılır.

**Ne yapmalı.** Kendiniz sürdüğünüz tek bir ajan için `imece --solo claude` (ya da `:solo YOL`) doğrudan klasörde çalışır ([solo oturumlar](concepts/solo.md#gitsiz-bir-klasörde)). Bir takım için önce orada `git init && git commit --allow-empty -m init`.

## Oturumlar

### `claude-1 durdu: tools differ: extra ["WebFetch"], missing []`

**Ne görürsünüz.** Satır `■ oturum bitti · yazınca yeniden başlar` gösterir; günlükte `claude-1 durdu: tools differ …`. Oturumun istenenle çalışmadığı böyle bir duruş yalnızca sizindir: lideri onu yeniden başlatmaya çağrılmaz ([duran işçi](concepts/messages-and-board.md#duran-işçi)).

**Neden.** imece Claude Code'dan tam olarak rolün ve confine düzeyinin araçlarını ister, sonra oturumun neyle çalıştığını bildirdiğini denetler her turda. Başka her şey oturumu durdurur: araç ekleyen bir Claude Code güncellemesi, sunucu getiren bir ayar, etrafından dolaşılmak yerine bakılmalıdır.

**Ne yapmalı.** Raporun tamamı için izi (`t`) okuyun. Yüklenmesini istediğiniz kendi Claude Code kurulumunuzsa, `--confine open` onu sizin kurduğunuz gibi çalıştırır (lider yine yazmaz ve alt ajan aracı kapalı kalır). Ajana yazmak onu aynı denetimlerle yeniden başlatır. Bu yolla duran bir oturum hâlâ hatırlanan oturumdur; `:forget @claude-1` sonraki başlangıcı yeni bir oturum yapar.

### `claude-1: opus modeli burada yok, varsayılanla devam`

**Neden.** Lider katmanla tuttu ve katmanın modeli hesabınızda yok (Claude Code bunu sıfır maliyetli düşen bir ilk tur olarak bildirir). imece işçiyi aynı istemlerle standart modelde bir kez yeniden başlatır; başka bir nedenle düşen bir model yeniden denenmez.

**Ne yapmalı.** Hiçbir şey, ya da hesabınızda olan bir modelle `--tier strong=AD`.

### Takılmış görünen bir oturum ya da ters giden bir uyanış

**Ne görürsünüz.** Uzun süredir hiçbir şey demeyen ya da kartında boşta oturan, çalışan bir işçi; günlük bunu belki söylemiştir bile: `claude-1 10 dk'dır turunda hiçbir şey demedi`, `claude-1 10 dk'dır #3 parser üzerinde boşta, done demedi`, `claude-1 bu turda 3 kez başarısız oldu: Bash cargo test`.

**Neden.** imece birkaç saniyede bir ekibe bakar ve lidere bir takılmayı, bir sessizliği ya da bir döngüyü, her birini bir kez, kimin duyduğuyla bitirerek söyler (`· lead uyarıldı`). Hiçbir şeyi durdurmaz: uzun süren bir komut ile asılı kalmış bir oturum dışarıdan aynı görünür. Dakikalar ve sayı [`:health`](reference/commands.md#health-stallidleloop-value)'ındır; bkz. [sağlık bildirimleri](concepts/team.md#sağlık-bildirimleri).

**Ne yapmalı.** Önce neyin çalıştığına bakın: `:ps` her oturumun süreçlerini CPU'su ve belleğiyle listeler. Birinde (asılı kalan bir test çalıştırması, hiç bitmeyen bir sunucu) `d` ve `y` yalnız o süreci sonlandırır: `SIGTERM`, iki saniye sonra `SIGKILL`; oturum sürer, günlük de `cargo (60321) sonlandırıldı, claude-1 altında` der. Oturumun kendi süreci ve imece'nin onun için çalıştırdığı aktarıcı orada `d`'de, hiç `y` sorulmadan reddedilir (`bu claude-1 ajanının oturumunun kendisi: onu :forget ya da durdurmak bitirir, :ps değil`, `bu imece'nin claude-1 için bus'ı: kalır`). Takılan oturumun kendisiyse `:forget @AD` (`:f`): çalışan oturum biter (çalıştırması öldürülür, bitmemiş istemler yarım kalmış olarak saklanır ve uyanışta anlatılır, yeniden çalıştırılmaz), transkripti kaldırılır ve ajan kendisiyle konuşulana dek uyur; sonraki mesajı tam brifingle yeni bir oturum başlatır (`Restarted`). `:` menüsü bunu `$` ile işaretler: brifing yeniden ödenir. `--fresh` başlangıçta bunu hatırlanan her ajan için yapar. Unutacak bir şeyi olmayan ya da gitmiş bir ajan için reddedilir (`claude-1 oturumunu koruyor: …`).

## Değişiklikler

### `claude-1 değişikliği uygulanamadı: error: patch failed: src/csv.rs:41 … conflicts with the checkout in src/csv.rs`

**Ne görürsünüz.** Flash ve günlükte aynı satır; `Δ` öğesi KARAR BEKLEYEN'de kalır.

**Neden.** `a` checkout'unuzda `git apply --index` çalıştırır. Checkout'unuz yamanın altında ilerlediyse imece önce yamayı index'in bir kopyasına üç yollu birleştirir ve çakışırsa reddeder (`conflicts with the checkout in a, b`); aynı dosyada stage'lenmemiş düzenlemeleriniz varsa git'in kendi `does not match index` sözü olduğu gibi döner ve hiçbir birleştirme işinizin üstünden geçmez. Hiçbir şey asla yarım uygulanmaz: git'in çakışmadaki üç yollusu dosyalarda işaretler bırakırdı, bu yüzden denetimsiz hiç çalışmaz.

**Ne yapmalı.** Dosyadaki kendi düzenlemelerinizi commit'leyin ya da stash'leyin ve `a` tuşuna yeniden basın. Commit'lerinizle ya da stage'dekilerle çakışıyorsa değişiklikte `b` onu uymayan parçalar ve git'in sözleriyle geri gönderir; ya da bir notla `r` (`checkout src/csv.rs içinde ilerledi; worktree'ni rebase et`): temiz bir worktree işçinin sonraki isteminden önce HEAD'inizi izler, bu yüzden parçayı yeniden yapması söylenen bir işçi genellikle uyan bir yamayla döner. `x` uyan hunk'ları uygulamanıza ve kalanı geri göndermenize izin verir.

### `a`, `reading` ya da `applying` ile reddedildi

**Neden.** Git tek iş parçacığında, aynı anda tek iş çalışır: o işçinin daha yeni `done`'ı hâlâ okunurken `a`, ya da uygulaması sürerken `a`/`r`, yarışmak yerine reddedilir. Günlük satırını bekleyin.

### Her değişiklikte `hiçbir şey koşmadı`

**Ne görürsünüz.** Her değişikliğin hükmü `hiçbir şey koşmadı` ve `! önce bak` der.

**Neden.** Yalnız imece'nin kendi `run` aracı kanıt sayılır, tam ağaca bağlı; Claude Code'un kendi kabuğu sayılmaz (imece hangi ağaçta çalıştığını görmez). İşçinin orada koştuğu bir derleme ya da test değişikliğin üstünde söylenir (`npm test kendi kabuğunda geçti, bu ağaçta kanıtlanmadı`); yani `hiçbir şey koşmadı` hiçbir şeyin koşmadığı demektir. İşçilere son düzenlemeden sonra `run` ile denetlemeleri söylenir. Ağacı değiştiren bir çalıştırma (bir biçimlendirici) hiçbirine sayılmaz.

### Değişiklik getirmeyen bir `done`: yama 4 MiB'ın üstünde

**Neden.** 4 MiB'ın üstündeki bir yama okunurken reddedilir ve işçiye `done`'ının yanıtında söylenir. Üretilmiş dosyaları dışarıda bırakmasını ya da işi bölmesini isteyin.

## Worktree'ler

### `claude-1 başlatılamadı: …/worktrees/…/claude-1 is gone, but git still has it as a worktree of …: whatever it held that was not committed is lost; \`git worktree prune\` there lets imece make it anew`

**Neden.** Worktree dizini elle silindi ama git'in listesinde hâlâ var. Git'in kendi öğüdü `add -f`, bu git'in ondan bildiğini atardı; imece bunu size bırakır.

**Ne yapmalı.** Checkout'ta `git worktree prune`, sonra ajana yeniden yazın.

### `… is there but is not a worktree of …`

**Neden.** İşçinin worktree'sinin olması gereken yerde başka bir şey duruyor. imece yapmadığı bir dizini asla yeniden kullanmaz ya da değiştirmez. Kenara taşıyın.

### `claude-1: çalışma ağacı 3f9a1c2 noktasında kaldı, commit edilmemiş değişiklikleri var (HEAD 8b20d44)`

**Neden.** Worktree'ler iş hiç atılmasın diye çalıştırmalar arasında saklanır. Eski bir commit'te temiz bulunan bir worktree ajanı başlarken ve turlar arasında HEAD'inize taşınır; commit edilmemiş değişiklikleri ya da kendi commit'leri olan yerinde kalır (`… kendi commit'leri var …`) ve kartının ekranındaki satırı eksik commit'ler için `↓N` gösterir. İşçinin `done`'ı kendi HEAD'ine göre okunur ve uygulama üç yolludur, genellikle yine uyar.

**Ne yapmalı.** Genellikle hiçbir şey. Artık istemediğiniz bir artık worktree ise (kapatılmış bir projenin: `AD kapatıldı: agent'ları ayrıldı; worktree'leri diskte kaldı`), checkout'tan `git worktree remove <yol>` ile kendiniz kaldırın ya da dizini sildikten sonra `git worktree prune`; imece kendiliğinden hiçbir worktree kaldırmaz. `~/.local/state/imece/worktrees/` (ya da `$XDG_STATE_HOME/imece/worktrees/`) altında, proje başına bir dizinde dururlar. Her birinin kendi `target/` dizini var, yer tutarlar.

## Tuşlar ve terminal

### `bu terminal 38×10; imece en az 40×12 ister`

**Ne görürsünüz.** Ekranın ortasında yalnız bu satır; `ctrl-c` dışında hiçbir tuş bir şey yapmaz.

**Neden.** 40 kolonun ya da 12 satırın altında imece başka bir şey çizmez: göremediğinize karar vermemelisiniz. Bir tuş, ona bastığınızda ekranda olana göre çalışır; ekranda yalnız bu satır varken hiçbiri çalışmaz.

**Ne yapmalı.** Pencereyi büyütün ya da yazıyı küçültün; imece hemen yeniden çizer ve tuşlar geri gelir. 80 kolonun altında ana ekranın satırları alt alta durur, durum satırı da yeri adlandırmadan önce tuşlarını tutar.

### Önceki bir çalışmadan kalan süreçler: `önceki bir çalışmadan kalan 2 süreç var: :ps listeler`

**Neden.** Oturumlarını bitirmeden kapanan bir imece (bir çökme, kapatılan bir terminal, `kill -9`) onları başlattıklarıyla birlikte çalışır bırakır. imece her başlangıçta komut satırı kendi worktree dizinini adlandıran ve üstünde çalışan bir imece olmayan süreçleri arar ve kaç tane bulduğunu söyler.

**Ne yapmalı.** `:ps` onları ÖNCEKİ BİR ÇALIŞMADAN KALAN altında, her ağacı çalıştığı yerin altında listeler. `d` birini, `a` kalanların hepsini bir `y`'den sonra sonlandırır: `SIGTERM`, süre dolunca `SIGKILL`. Hiçbir şey kendiliğinden sonlandırılmaz; şu an çalışan başka bir imece kendi oturumlarını korur.

### `⇧enter` yeni satır başlatmak yerine mesajı gönderiyor

**Neden.** Çoğu terminal `⇧enter`ı düz `enter` olarak gönderir; yalnız kitty klavye protokolünü konuşan bir terminal ikisini ayırır (kitty, Ghostty, `enable_kitty_keyboard` ile WezTerm, foot, Alacritty, iTerm2). imece başta sorar ve alt satır bulduğunu söyler: `⇧enter yeni satır` ya da `\ enter yeni satır`.

**Ne yapmalı.** `\` sonra `enter` yazın: `\` yeni satıra dönüşür. Option'ın Meta olduğu yerde `⌥enter` çalışır. tmux ya da screen altında sorgu geçmeyebilir, orada da bu yol geçerlidir.

### `⌥b` `∫` yazıyor (macOS)

**Neden.** Terminal Option'ı Meta değil karakter olarak gönderiyor. iTerm2: Profiles → Keys → Left Option key: *Esc+*. Terminal.app: *Use Option as Meta key*. `^←` `^→` da Meta olmadan kelime kelime gider.

### Yapıştırma bir şey yapmadı ve alt satır `yapıştırılan, yazdığın satıra girer: önce / ya da @` diyor

**Neden.** Köşeli yapıştırma açıktır: bir yapıştırma tek olaydır, satıra bütün girer ve satır sonlarıyla asla gönderilmez. Yazmıyorken karakterleri tuş olarak okunurdu, bu yüzden atılır ve alt satır bunu söyler.

### Ekran koyu zeminde koyu yazı, ya da seçim zor görünüyor

**Neden.** `terminal` teması başta terminalinizin renklerini sorar (OSC 10/11, geç bir yanıtın tuşlara sızmaması için bir imleç konumu raporuyla). Yanıtlamayan bir terminal koyu sayılır ya da `COLORFGBG` ne derse; bir kez açık yanıtlayan, pencere yeniden odaklanınca yeniden okunur (tmux `focus-events on` ister). `COLORTERM=truecolor` yoksa RGB renkler 256'ya yuvarlanır.

**Ne yapmalı.** `T` yerleşik temaları dolaşır; `:theme dark`, `:theme light` ya da bir şema (`:theme nord`) sormak yerine varsayar; `:theme DOSYA` Ghostty ya da kitty renk dosyası alır (`--theme`, `IMECE_THEME` de). Seçim hatırlanır. Alacritty'nin TOML'u ve `.itermcolors` okunmaz; önce dönüştürün.

### Boyut değişiminden sonra iTerm2'nin geçmişinde imece kopyaları

**Neden.** iTerm2'nin *Save lines to scrollback in alternate screen mode* seçeneği silinen bir alternatif ekranı geçmişe kopyalar. imece satır satır siler ve ekranı tutarken otomatik sarmayı kapatır, bu yüzden güncel derlemelerde bu olmamalıdır; olursa bakılacak yer o seçenektir.

### Boyut değişiminden sonra ekran başka bir boyuta çizilmiş görünüyor

**Neden.** Terminalin boyut değişimi imece'nin girdisine bir şey yazmaz ve önceki bir derleme bunu ancak başka bir şey değişince fark ediyordu: o zamana kadar terminal eski kareyi kendi kaydırdığı haliyle gösteriyordu (durum satırı pencerenin ortasında, altında boş satırlar, sağ kenardan taşan satırlar). imece artık boyut değişiminde uyanıyor, yeni boyuta hemen çiziyor ve boyut değişmeyi bıraktıktan çeyrek saniye sonra bütün ekranı bir kez daha çiziyor; yani bu düzeldi, güncel bir derlemede görülürse hatadır.

### Başlangıçta kod haritası ve yazma satırı kendiliğinden açıldı

**Neden.** Bir terminalin renk sondasına geç yanıtı bir zamanlar tuş olarak okunuyordu (Terminal.app sondanın ilk süresinden sonra yanıtladı). Sonda şimdi bir imleç konumu raporunu bekler, bu düzeltildi; güncel bir derlemede görülmesi bir hatadır.

## Kese ve makine

### Ajanlar tur almıyor; günlük `$4.20/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır` diyor

**Neden.** Kese bütün takımın tek bütçesidir, çalıştırmalar arasında saklanır; bitince kimse tur almaz. `:budget` notundaki hız son on dakikanın maliyetidir. Yarım saatten az kaldığında başlıktaki kese dikkat rengine döner, ne kadar kaldığını söyler ve günlük bir kez söyler.

**Ne yapmalı.** `:budget 80` ajanlar çalışırken artırır; süren bir tur eski tavanında biter, sonraki yeni kalanla başlar. Kendi tavanına çarpan bir oturum aynı istemlerle bir kez yeniden başlatılır.

### `makine uyanık tutulamadı: …`

**Neden.** `--awake`/`:awake`, `caffeinate` (macOS) ya da `systemd-inhibit` (Linux, systemd gerekir) komutunu alt süreç olarak çalıştırır; başlamayan ya da kendi biten bir program flash'ta bir kez söylenir ve o çalıştırmada bir daha denenmez. Başka yerde çalıştırılacak bir şey yoktur. imece'ye `kill -9`, `systemd-inhibit`'in kilidini `sleep`i öldürülene kadar bırakabilir; `caffeinate -w` imece'nin pid'ini kendisi izler.

### `sonraki çalışma için kaydedilemedi: …`

**Neden.** Depoya bir kayıt başarısız oldu (disk dolu, dosya gitmiş). Bir kez flash'lanır; ekran sürer ve sonraki kayıt yeniden dener.

### imece makineden kendisi ne kadar kullanıyor?

**Neden.** Az: ekran yalnız üzerindeki bir şey değiştiğinde çizilir (bir tuş, bir ajanın sözü, git'in yanıtı, bus'ta bir çağrı, saatin beş saniyelik adımı, gelen ya da giden bir uyarı), en geç bir tik (canlıda 100 ms) sonra, ve yalnız değişen hücreler gönderilir; hiçbir şey olmuyorsa hiçbir şey çizilmez. Terminal odakta değilken (başka bir penceredesiniz; tmux'ta `focus-events on` gerekir) değişen en fazla saniyede bir çizilir, araya bir tuş, yapıştırma ya da boyut değişimi girmedikçe; başlık, bildirimler ve ajanların turları eskisi gibi sürer.

**Ne yapmalı.** Kendi makinenizdeki rakamlar için `cargo build --release`, sonra `python3 -I tools/measure.py --fast` çalıştırın: biri odakta, biri başka pencerede iki demo çalışması, her birinin CPU süresi, en yüksek belleği ve terminale gönderilen baytlarıyla. M3 Max bir MacBook Pro'da demo'nun en yüksek hızında 15 sn, odakta 0.35 sn CPU, başka pencerede 0.07 sn tuttu (0.64'e karşı 0.17 MiB çizildi), iki durumda da 6 MiB bellek. Demo'nun dünyası her tikte ilerler; turlar arasındaki canlı bir ekip çok daha az çizer.

## Ayrıca bakınız

- [Kurulum](getting-started/install.md)
- [Oturumlar](concepts/sessions.md)
- [Claude Code](concepts/claude-code.md)
- [Değişiklikler](concepts/changes.md)
- [Bütçe ve sınırlar](concepts/budget-and-limits.md)
- [Temalar ve dil](screen/themes-and-language.md)
- [Güvenlik](concepts/security.md)
