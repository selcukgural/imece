---
title: Sorun giderme
description: Bir şey ters gidince ne görürsünüz, imece bunu neden yapar ve ne yapmalısınız.
---

# Sorun giderme

imece kapalı düşer (fail closed): bir dosya, bir oturum ya da bir değişiklik beklediği gibi değilse onarmak ya da tahmin etmek yerine reddeder ve söyler. Bu sayfa o retleri ve her biri için ne yapılacağını listeler. Mesajlar ekranın ya da terminalin yazdığı gibi alıntılanmıştır; terminalde `imece: …` imece'nin başlamadığı demektir (çıkış kodu 1), flash alt satırın üstündeki tek satırlık mesajdır, günlük ise ne olduğunu saklar. Satıcı katından gelen mesajlar (oturum açma, fiyat, araç listesi, worktree) kaynakta İngilizcedir ve ekrana öyle gelir; burada da öyle alıntılanır.

## Başlangıç

### `another imece is running on …; open the other checkout from it with :open`

**Ne görürsünüz.** `imece` o satırla hemen durur.

**Neden.** Depoyu aynı anda tek imece tutar (`imece.sqlite.lock`, `flock` ile). Tek SQLite dosyasında ve tek kesede iki yazar birbirinin görüşünü bozardı.

**Ne yapmalı.** Çalışan imece'ye gidin ve öteki checkout'u kendi lideriyle ayrı bir proje olarak eklemek için `:open YOL` yazın. Çalışan imece yoksa (sonuncusu öldürüldü), kilit süreci bitince işletim sistemi tarafından bırakılır; kalan bir kilit, hâlâ bir sürecin orada olduğu demektir (`pgrep imece`).

### `…/imece.sqlite: … imece left the file as it was. To start afresh, move it somewhere else …`

**Ne görürsünüz.** `imece: <yol>: <neden>`, sonra `imece left the file as it was. To start afresh, move it somewhere else with its <yol>-wal and <yol>-shm files, if they are there; the next start makes a new one.` Neden `the file was written by a newer imece (schema 11, this one knows 9)`, `file is not a database` ya da tanımadığı bir satır olabilir.

**Neden.** imece okuyamadığı bir dosyayı asla taşımaz, onarmaz ya da üstüne yazmaz: içindekiler hâlâ istenebilir ve daha yeni bir imece hâlâ okuyabilir.

**Ne yapmalı.** Ya onu yazan imece'yi derleyin (şema numarası daha yeni olduğunu söyler) ya da yeniden başlamak için dosyayı `-wal` ve `-shm` kardeşleriyle kenara taşıyın. Hiçbir şey kendiliğinden silinmez.

### `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code … and log in`

**Neden.** imece satıcıların kendi CLI'larını sürer ve hiçbirini bulamadı. Birini kurun ve programının imece'yi başlattığınız kabuğun PATH'inde olduğundan emin olun.

### `codex is not on PATH (found claude); install it, or pick the leader with --lead`

**Neden.** `--lead` (ya da önceki bir `p` ile birinciliniz) orada olmayan bir satıcıyı adlandırıyor. Eksik bir birincil KİM LİDER? ekranında da söylenir (`codex birincil ajanın, ama PATH'te yok`), sessizce değiştirilmez. `:lead ask` birinciliyi unutturur.

### `--projects is for --demo` / `--agents: a leader hires at most 16`

**Neden.** `--projects`, `--researchers` ve `--seed` simülasyona aittir; canlıda `--agents` 0-16 (varsayılan 4), `--helpers` 0-8 (varsayılan 2). Mesajın ardından kullanım metni gelir.

### `… is not a git checkout with a commit: a team works in git worktrees …`

**Neden.** imece git checkout'u olmayan ya da henüz hiçbir şey commit'lenmemiş bir klasörde başladı (ya da `:open` çalıştı): bir takımın işçilerinin her biri bir git worktree'sinde çalışır, bu da bir commit'ten yapılır.

**Ne yapmalı.** Kendiniz sürdüğünüz tek bir ajan için `imece --solo claude` (ya da `:solo YOL`) doğrudan klasörde çalışır ([solo oturumlar](concepts/solo.md#gitsiz-bir-klasörde)). Bir takım için önce orada `git init && git commit --allow-empty -m init`.

## Oturumlar

### `codex-1 başlatılamadı: is logged in with apikey, whose cost imece can not count yet; log in with ChatGPT (codex login)`

**Neden.** Codex dolar değil token bildirir. ChatGPT planında bir tur keseye hiçbir şey yazmaz ve imece bunu söyler; API anahtarıyla token'ları fiyatlandırması gerekirdi, bunu henüz yapamaz, bu yüzden ücretli bir çağrıya sıfır yazmak yerine reddeder. `is not logged in; run codex login` hesapsız aynı denetimdir.

**Ne yapmalı.** Bir ChatGPT hesabıyla `codex login`.

### `gemini-1 durdu: ran on gemini-x, whose price imece does not know; name a model it knows with --model or --tier`

**Neden.** API anahtarıyla Gemini turları imece'nin taşıdığı liste fiyat tablosundan yazılır; tabloda olmayan bir model yazılamaz, oturum görünmeden harcamak yerine durur. Google oturumuyla tur keseye hiçbir şey yazmaz ve bu olmaz.

**Ne yapmalı.** Tablonun bildiği bir modelle `--model gemini:AD` ya da `--tier gemini:strong=AD` (`pro`, `flash` ve `auto` takma adları bilinir); ya da Google ile oturum açın.

### `claude-1 durdu: tools differ: extra ["WebFetch"], missing []`

**Ne görürsünüz.** Satır `■ durdu, yazınca yeniden başlar` gösterir; günlükte `claude-1 durdu: tools differ …`, ya da Codex ve Gemini için uymayanı adlandıran bir satır (imece'ninki olmayan bir MCP sunucusu, bir hook, bir eklenti, bir sandbox, bir kip, bir model, bir uzantı).

**Neden.** imece her satıcıdan tam olarak rolün ve confine düzeyinin araçlarını ister, sonra oturumun neyle çalıştığını bildirdiğini denetler (Claude her turda, Codex thread'den önce ve MCP sunucularını her turdan önce, Gemini ilk turdan önce). Başka her şey oturumu durdurur: araç ekleyen bir satıcı güncellemesi, sunucu getiren bir ayar, etrafından dolaşılmak yerine bakılmalıdır.

**Ne yapmalı.** Raporun tamamı için izi (`t`) okuyun. Yüklenmesini istediğiniz kendi satıcı kurulumunuzsa, `--confine open` satıcıyı sizin kurduğunuz gibi çalıştırır (lider yine yazmaz ve alt ajan aracı kapalı kalır). Ajana yazmak onu aynı denetimlerle yeniden başlatır. Bu yolla duran bir oturum hâlâ hatırlanan oturumdur; `:forget @claude-1` sonraki başlangıcı yeni bir oturum yapar.

### Bir Codex onayı, elicitation ya da dinamik araçla `codex-1 durdu: refused …`

**Neden.** imece'nin yanıtı olmayan bir Codex isteği (soramaması gereken bir oturumdan onay, örneğin bir lider ya da `--confine strict` altındaki her şey; eski protokolün onayları; bir elicitation; dinamik bir araç çağrısı) körce yanıtlanmak yerine oturumu durdurur. `standard` ve `open` altında bir işçinin onay istekleri SENİ BEKLEYEN'e (`⚑`) gelir.

### `claude-1: opus modeli burada yok, varsayılanla devam`

**Neden.** Lider katmanla tuttu ve katmanın modeli hesabınızda yok (Claude Code bunu sıfır maliyetli düşen bir ilk tur olarak bildirir). imece işçiyi aynı istemlerle standart modelde bir kez yeniden başlatır; başka bir nedenle düşen bir model yeniden denenmez.

**Ne yapmalı.** Hiçbir şey, ya da hesabınızda olan bir modelle `--tier strong=AD`.

### Takılmış görünen bir oturum ya da ters giden bir uyanış

**Ne yapmalı.** `:forget @AD` (`:f`): çalışan oturum biter (çalıştırması öldürülür, bitmemiş istemler yarım kalmış olarak saklanır ve uyanışta anlatılır, yeniden çalıştırılmaz), satıcının ondan tuttuğu kaldırılır (Claude'un transkripti, Codex'in thread'i, Gemini'nin sohbet dosyaları) ve ajan kendisiyle konuşulana dek uyur; sonraki mesajı tam brifingle yeni bir oturum başlatır (`Restarted`). `:` menüsü bunu `$` ile işaretler: brifing yeniden ödenir. `--fresh` başlangıçta bunu hatırlanan her ajan için yapar. Unutacak bir şeyi olmayan ya da gitmiş bir ajan için reddedilir (`claude-1 oturumunu koruyor: …`).

### `/clear` hemen ardından `lead durdu: …`

**Neden.** Kendi komut satırı olmayan bir satıcıda (Codex) `/clear` oturumu bitirir; sonraki mesaj yenisini başlatır. Codex'in app server'ı yalnız `/compact` bilir ve arada tur olmadan ikinci bir `/compact` düşen bir tur olarak reddedilir, oturum sürer.

## Değişiklikler

### `claude-1 değişikliği uygulanamadı: error: patch failed: src/csv.rs:41 … conflicts with the checkout in src/csv.rs`

**Ne görürsünüz.** Flash ve günlükte aynı satır; `Δ` öğesi SENİ BEKLEYEN'de kalır.

**Neden.** `a` checkout'unuzda `git apply --index` çalıştırır. Checkout'unuz yamanın altında ilerlediyse imece önce yamayı index'in bir kopyasına üç yollu birleştirir ve çakışırsa reddeder (`conflicts with the checkout in a, b`); aynı dosyada stage'lenmemiş düzenlemeleriniz varsa git'in kendi `does not match index` sözü olduğu gibi döner ve hiçbir birleştirme işinizin üstünden geçmez. Hiçbir şey asla yarım uygulanmaz: git'in çakışmadaki üç yollusu dosyalarda işaretler bırakırdı, bu yüzden denetimsiz hiç çalışmaz.

**Ne yapmalı.** Dosyadaki kendi düzenlemelerinizi commit'leyin ya da stash'leyin ve `a` tuşuna yeniden basın; ya da bir notla `r` (`checkout src/csv.rs içinde ilerledi; worktree'ni rebase et`): temiz bir worktree işçinin sonraki isteminden önce HEAD'inizi izler, bu yüzden parçayı yeniden yapması söylenen bir işçi genellikle uyan bir yamayla döner. `x` uyan hunk'ları uygulamanıza ve kalanı geri göndermenize izin verir.

### `a`, `reading` ya da `applying` ile reddedildi

**Neden.** Git tek iş parçacığında, aynı anda tek iş çalışır: o işçinin daha yeni `done`'ı hâlâ okunurken `a`, ya da uygulaması sürerken `a`/`r`, yarışmak yerine reddedilir. Günlük satırını bekleyin.

### Her değişiklikte `bu ağaçta çalıştırma yok`

**Neden.** Yalnız imece'nin kendi `run` aracı kanıt sayılır, tam ağaca bağlı; satıcının kendi kabuğu görünmez (imece hangi ağaçta çalıştığını görmez). İşçilere son düzenlemeden sonra `run` ile denetlemeleri söylenir. Ağacı değiştiren bir çalıştırma (bir biçimlendirici) hiçbirine sayılmaz.

### Değişiklik getirmeyen bir `done`: yama 4 MiB'ın üstünde

**Neden.** 4 MiB'ın üstündeki bir yama okunurken reddedilir ve işçiye `done`'ının yanıtında söylenir. Üretilmiş dosyaları dışarıda bırakmasını ya da işi bölmesini isteyin.

## Worktree'ler

### `claude-1 başlatılamadı: …/worktrees/…/claude-1 is gone, but git still has it as a worktree of …: whatever it held that was not committed is lost; \`git worktree prune\` there lets imece make it anew`

**Neden.** Worktree dizini elle silindi ama git'in listesinde hâlâ var. Git'in kendi öğüdü `add -f`, bu git'in ondan bildiğini atardı; imece bunu size bırakır.

**Ne yapmalı.** Checkout'ta `git worktree prune`, sonra ajana yeniden yazın.

### `… is there but is not a worktree of …`

**Neden.** İşçinin worktree'sinin olması gereken yerde başka bir şey duruyor. imece yapmadığı bir dizini asla yeniden kullanmaz ya da değiştirmez. Kenara taşıyın.

### `claude-1: çalışma ağacı 3f9a1c2 noktasında kaldı, commit edilmemiş değişiklikleri var (HEAD 8b20d44)`

**Neden.** Worktree'ler iş hiç atılmasın diye çalıştırmalar arasında saklanır. Eski bir commit'te temiz bulunan bir worktree ajanı başlarken ve turlar arasında HEAD'inize taşınır; commit edilmemiş değişiklikleri ya da kendi commit'leri olan yerinde kalır (`… kendi commit'leri var …`) ve satır eksik commit'ler için `↓N` gösterir. İşçinin `done`'ı kendi HEAD'ine göre okunur ve uygulama üç yolludur, genellikle yine uyar.

**Ne yapmalı.** Genellikle hiçbir şey. Artık istemediğiniz bir artık worktree ise (kapatılmış bir projenin: `AD kapatıldı: agent'ları ayrıldı; worktree'leri diskte kaldı`), checkout'tan `git worktree remove <yol>` ile kendiniz kaldırın ya da dizini sildikten sonra `git worktree prune`; imece kendiliğinden hiçbir worktree kaldırmaz. `~/.local/state/imece/worktrees/` (ya da `$XDG_STATE_HOME/imece/worktrees/`) altında, proje başına bir dizinde dururlar. Her birinin kendi `target/` dizini var, yer tutarlar.

## Tuşlar ve terminal

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

### Başlangıçta `3` kendiliğinden kod haritasını, `/` yazma satırını açtı

**Neden.** Bir terminalin renk sondasına geç yanıtı bir zamanlar tuş olarak okunuyordu (Terminal.app sondanın ilk süresinden sonra yanıtladı). Sonda şimdi bir imleç konumu raporunu bekler, bu düzeltildi; güncel bir derlemede görülmesi bir hatadır.

## Kese ve makine

### Ajanlar tur almıyor; günlük `$4.20/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır` diyor

**Neden.** Kese bütün takımın tek bütçesidir, çalıştırmalar arasında saklanır; bitince kimse tur almaz. Başlıktaki hız son on dakikanın maliyetidir. Yarım saatten az kaldığında bütçe dikkat rengine döner ve günlük bir kez söyler.

**Ne yapmalı.** `:budget 80` ajanlar çalışırken artırır; süren bir tur eski tavanında biter, sonraki yeni kalanla başlar. Kendi tavanına çarpan bir oturum aynı istemlerle bir kez yeniden başlatılır.

### `makine uyanık tutulamadı: …`

**Neden.** `--awake`/`:awake`, `caffeinate` (macOS) ya da `systemd-inhibit` (Linux, systemd gerekir) komutunu alt süreç olarak çalıştırır; başlamayan ya da kendi biten bir program flash'ta bir kez söylenir ve o çalıştırmada bir daha denenmez. Başka yerde çalıştırılacak bir şey yoktur. imece'ye `kill -9`, `systemd-inhibit`'in kilidini `sleep`i öldürülene kadar bırakabilir; `caffeinate -w` imece'nin pid'ini kendisi izler.

### `sonraki çalışma için kaydedilemedi: …`

**Neden.** Depoya bir kayıt başarısız oldu (disk dolu, dosya gitmiş). Bir kez flash'lanır; ekran sürer ve sonraki kayıt yeniden dener.

## Ayrıca bakınız

- [Kurulum](getting-started/install.md)
- [Oturumlar](concepts/sessions.md)
- [Satıcılar](concepts/vendors.md)
- [Değişiklikler](concepts/changes.md)
- [Bütçe ve sınırlar](concepts/budget-and-limits.md)
- [Temalar ve dil](screen/themes-and-language.md)
- [Güvenlik](concepts/security.md)
