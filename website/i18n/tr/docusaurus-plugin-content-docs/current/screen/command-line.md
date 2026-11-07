---
title: "Komut satırı"
description: "Yazdığınız alt satır - lidere ya da tek bir agent'a, Claude Code komutları, imece'nin : komutları ve menüsü, tamamlama, readline tuşları, çok satırlı yazma, kendi kabuğunuz ve editörünüz."
---

# Komut satırı

Alt satır yazdığınız yerdir. `/` (ya da vim'deki gibi `i`) onu seçili projenin liderine açar, `@` tek bir agent'a, `:` imece'ye, `!` kendi kabuğunuza, `#` bir karta. Siz yazana dek bir ipucu `enter`'ın ne yaptığını, satır nasıl ekleneceğini ve `esc`'in vazgeçtiğini söyler; ilk karakterle kaybolur, satır yeniden boşalınca geri gelir.

```text
 › lead için: █   enter gönder · ⇧enter yeni satır · esc vazgeç
```

```text
 budget ($3.10 / $20.00 harcandı)   board   close (shop-api)   code   commit   copy (shop-api)
 › komut: :b█                                       tab sonraki · enter al · esc gizle
```

## Bir satır nereye gider

| Satır | Nereye |
| --- | --- |
| düz metin | seçili projenin liderine; isteği kartlara böler |
| `@name text` | tek bir agent'a (Türkçe yardımda `@ad mesaj`). Boşta bir işçiye mesaj onun için bir kart açar; yanıtı kartı kapatır. Bir odakta, izde ya da değişiklikte `@` agent'ın adını doldurur |
| `/cmd`, `@name /cmd` | agent'ın kendi komut satırı, mesaj değil (Türkçe yardımda `/kmt`): `/clear`, `/compact`, `/cost`, `/context`, Claude Code ne biliyorsa. Bir tur olarak çalışır ve yanıtı bir cevap gibi döner |
| `:komut` | imece'nin kendisine; aşağıda |
| `!cmd` | kendi kabuğunuza (Türkçe yardımda `!kmt`) |
| `#N`, `#word` | bir kartın ekranı; bkz. [kartlar](cards.md) |

Baştaki etiket nereye gittiğini söyler: `lead için:`, `claude-1 için:`, `komut:`, `kabuk:`, `iş:`, ya da bir değişikliğin notu yazılırken `dosya:satır için not ▸`. Agent adları tamamlanır: `@cl` `tab` → `@claude-1 `.

Bir mesaj birkaç satıra yayılabilir; kart açan bir mesajın ilk satırı kartın başlığıdır. Biten bir bütçe sözleri ve komutları reddeder: `:budget` artırana dek hiçbir şey tur başlatmaz.

## `:` komutları

`:` satırı `:` ile ve bütün komutların menüsüyle açar. Her komutun varsa bir tuşu vardır ve nerede olursanız olun tuşun yaptığını yapar; hareket tuşlarının adı yoktur, çünkü yazmak tuştan yavaş olurdu.

| Komut | Tuş | Ne yapar |
| --- | --- | --- |
| `:budget USD` (`:b`) | | agent'lar çalışırken bütçeyi artır ya da azalt; süren bir tur eski sınırında biter |
| `:open PATH [NAME]` (`:o`) | | PATH'teki çalışma kopyasını kendi lideriyle bir proje olarak aç (Türkçe yardımda `:open YOL`) |
| `:close NAME` (`:c`) | | bir projeyi kapat: agent'ları ayrılır, worktree'leri diskte kalır (`:close AD`) |
| `:rename NAME` | | eldeki projeyi adlandır; lideri `lead-NAME` olur (`:rename AD`) |
| `:forget @NAME` (`:f`) | | agent oturumunu unutur; yenisine kaldığı yer anlatılır (`:forget @AD`) |
| `:export [PATH]` | | açık konuşma, yoksa kartın ekranı, Markdown olarak (`:export YOL`) |
| `:commit` | | ucuz bir model stage'dekiler için commit mesajı önerir, satıra `!git commit -m '…'` olarak konur |
| `:copy PATH` | | git'in yok saydığı bir yolu (`.env`, `target`) işçilerin worktree'lerine başlarken kopyala; yine basılınca bırakır (`:copy YOL`) |
| `:pin [NAME]` | | projeyi ekibin en üstüne sabitle; yine basılınca bırakır (`:pin AD`) |
| `:awake` | | bir agent tur ortasındayken makineyi uyanık tut; yine basılınca kapatır |
| `:notify [off\|bell\|text\|push]` | | başka penceredeyken sizi bekleyen bir şey gelirse zil ya da masaüstü bildirimi; `push`: uzaktayken ntfy üzerinden telefonunuza; yalın yazılınca sıradaki |
| `:motion [off\|calm\|full]` | | ekranda ne hareket eder: çalışan satır nefes alır, bekleyen yanıp söner; full dönücüleri de ekler; yalın yazılınca sıradaki |
| `:salon` `:home` | | iki görünüm |
| `:board` `:code` `:timeline` | | ekranın üstünde pencereler; `esc` kapatır |
| `:lang [en\|tr]` | `L` | diğer dil, ya da adı verilen |
| `:theme [NAME\|FILE]` (`:t`) | `T` | sonraki yerleşik tema, ya da adı verilen (`:theme AD ya da DOSYA`) |
| `:help` | `?` | yardım |
| `:quit` (`:q`) | `q` | çık; `q` gibi önce sorar |
| `:pending` | `q` `s` | çıkmadan önce: tur ortasındaki agent'lar ve incelenmemiş değişiklikler |
| `:next` / `:defer` | `W` / `S` | bekleyen sıradaki ögeyi aç / bunu ertele |
| `:waiting` | `w` | bekleyenler paneli |
| `:needs` | `f` | ekip yalnız sizi bekleyenleri gösterir |
| `:settle` / `:unsettle` | `s` / — | seçili satırı size yeniden ihtiyacı olana dek gizle / gizlenen her şeyi geri getir |
| `:fold` / `:unfold` | `zM` / `zR` | her projeyi ve her işçinin yardımcılarını katla / aç |
| `:trace [@NAME]` / `:talk [@NAME]` | `t` / `v` | adı verilen, yoksa eldeki agent'ın ham akışı / konuşması |
| `:peek [@NAME]` | `K` | adı verilen, yoksa eldeki agent'ın odağı görünümün üstünde bir pencerede |
| `:hints` | `F` | ekip ekranında seçilebilen her satırda ipucu harfleri |
| `:mark` | `m` | eldeki satırı işaretle |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` | yalnız demo; canlıda bunu söyler ve menüde yer almaz |

Bir ekip panelinde çalışan komutlar (`:waiting`, `:needs`, `:fold`, `:unfold`, `:settle`, `:unsettle`) önce o panele gider. Her komutun ne aldığı ve ne yanıtladığı [komutlar başvurusunda](../reference/commands.md).

### Menü ve değerler

Menü her komutun değerini ardında, ayraç içinde gösterir; çizilir, asla eklenmez: `budget ($3.10 / $20.00 harcandı)`, `theme (dark)`, `lang (Türkçe)`, `close` ve `rename` eldeki projeyle, `forget` `talk` `trace` eldeki agent'la, `next` ve `waiting` kaç öge beklediğiyle, `needs` ve `pause` açık ya da kapalı, `faster` `slower` hızla, `pin` projeyle (sabitse `▴` ile), `export` ne yazacağıyla (`konuşma`, `#12`). `$` sonraki faturayı büyüteni işaretler: `:open` (yeni bir lider oturumu), `:forget` (sonraki mesaj, bütün brifingi yeniden ödeyen yeni bir oturum başlatır) ve `:commit` (mesajı bir modele sorar). Başka hiçbir `:` komutu kendi başına ücretli bir tur başlatmaz.

### Kısa adlar

`b` `c` `f` `o` `q` `t`, `budget` `close` `forget` `open` `quit` `theme` yerine geçer. Bütünüyle yazılan bir kısa ad zaten bir komuttur, menü hiçbir şey önermez ve `enter` çalıştırır: `:q` `enter` hemen sorar, `:b 80` bütçeyi artırır.

### Tema önizlemesi

`:theme AD` yerleşik bir tema adıyla yazılırken, yazılmış ya da `tab` ile üstüne gelinmiş, ekran onu giyer; `enter` tutar, `esc` vazgeçer ve satır artık tema adlandırmadığında kullanılan tema geri gelir. Bkz. [temalar ve dil](themes-and-language.md).

## Tamamlama

Yazmak bir kabuktaki gibi tamamlanır: `@` ardından agent adları (eldeki proje önce, gidenler dışarıda, önek eşleşmesi, büyük küçük harf ayırmadan), `:` ardından imece'nin komutları, `/` ya da `@name /` ardından o agent'ın oturumunun bildirdiği kendi komut satırı (asla sabit bir liste değil, böylece Claude Code'un yeni bir komutu sürüm gerektirmez; oturum söyleyene dek boş, ve bir oturum hiç komut listelemediğinde günlük yalnız `/clear`'ın tamamlanacağını söyler), `#` ardından kartlar, `:open` ve `:export` ardından dizinler; `:pin` ve `:close` açık projelerin adlarını tamamlar. Eşleşmeler yazarken alt satırın üstünde tek satırda görünür; sözcüğe eşit tek eşleşme menü değildir.

| Tuş | Ne yapar |
| --- | --- |
| `tab` | ilk eşleşmeyi koy, sonra ilerle |
| `⇧tab`, `↑` `↓`, `^n` `^p` | eşleşmelerde ilerle, metin izler (zsh menü biçimi) |
| `enter` | vurgulu eşleşme henüz girilmemişse al, yoksa gönder |
| `esc` | menüyü gizle; yine basılınca yazmaktan vazgeç |

Menü yalnız imleç sondayken önerilir, çünkü bir tamamlama satırın bittiği sözcüğün yerini alır; sondan ayrılmak gizler, orada `tab` bir şey yapmaz, geri gelmek yine önerir. `!` ardından hiçbir şey tamamlanmaz.

## Readline tuşları

Satır bir kabuğun satırı gibi düzenlenir, readline'ın emacs takımı:

| Tuş | Ne yapar |
| --- | --- |
| `^a` / `^e` (home / end) | imlecin olduğu satırın başına / sonuna |
| `←` `→`, `^b` `^f` | bir karakter geri / ileri |
| `⌥b` / `⌥f` (ya da alt ya da ctrl ile `←` `→`) | bir sözcük geri / ileri; sözcük harf ve rakamlardır |
| `^w` | önceki boşluğa kadar kes (bash'teki gibi boşluğa kadar her şey) |
| `⌥backspace` / `⌥d` | bir sözcük geri / ileri kes |
| `^u` / `^k` | bu satırda imleçten öncesini / sonrasını kes |
| `^y` | son kesileni imleçte geri koy |
| `^d`, delete | imlecin altındaki karakter; `^d` boş satırdan çıkmaz (`esc` çıkar) |
| `^x` `^e` | satır editörünüzde ([aşağıda](#editörünüz)) |

Tek bir kesik vardır, halka değil: bir şey alan her kesme öncekinin yerini alır. İmleç terminalin kendi imlecidir, bir sonraki karakterin geleceği yerde durur ve şekli ne yazdığınızı söyler: mesaj ya da `!` komutu için yanıp sönen çubuk, `:` komutu için temanın vurgu renginde sabit çubuk, bir şey bulan sözcükler için yanıp sönen alt çizgi (`#`, `&`, `^r`, yardımın ve arşivlerin `/` süzgeci). Yanıp sönmeyi terminal yapar, imece'ye çizim maliyeti yoktur; imleç şeklini değiştirmeyen bir terminal kendi şeklini korur. imece terminali geri verdiğinde (sonunda ya da bir `!` komutuna) imleç terminalinizin ayarlarındaki gibidir. Her kare terminale tek bir senkron güncelleme olarak gider, bu yüzden bunu destekleyen bir terminal hiçbir zaman yarım kare göstermez. macOS'ta alt tuşları için terminalin Option'ı Meta olarak göndermesi gerekir (iTerm2: Profiles → Keys → Left Option key: Esc+; Terminal.app: Use Option as Meta key); yoksa Option-b `∫` yazar. Burada olmayanlar: kill-ring döngüsü, geri alma, yer değiştirme, vim'in normal kipi.

## Birkaç satır

`⇧enter` yeni satır başlatır, Claude Code'daki gibi, terminal onu `enter`'dan ayırabildiğinde: kitty klavye protokolünü konuşuyorsa (kitty, Ghostty, `enable_kitty_keyboard` ile WezTerm, foot, Alacritty, iTerm2), imece bunu başlangıçta sorar. Her yerde `\` sonra `enter`, `\`'i göndermek yerine yeni satıra çevirir; Option Meta olduğu yerde `⌥enter` da. `?` yardımı ve alt satırın ipucu terminalinizdekini adlandırır: `⇧enter yeni satır` ya da `\ enter yeni satır`. Sorguya yanıt vermeyen bir çoklayıcı (tmux, screen) yedeği alır.

Alt satır metnin her sırası için bir sıra alır, en çok sekiz ve ekranın üçte biri: ekrana sığmayacak kadar uzun bir satır sonraki sırada sürer (hücre hücre kesilir; sığmayan geniş bir karakter bütün olarak aşağı iner), yani yazdığınız hep görünür. İlk sıra metnin kime gittiğini taşır, diğerleri altında içeriden gelir. Bütün sıralar sığmadığında imlecin sırası görünür kalır ve son sıra nerede olduğunu söyler, `3/12`. `^a` `^e`, home/end ve `^u` `^k` imlecin olduğu satırda kalır; `↑` `↓` satırlar arasında gider, sütunu koruyarak (menü gösterilirken onun yerine menüde ilerler); satır başında backspace üsttekiyle birleştirir. `@name` bir boşlukta ya da yeni satırda biter, yani `@name` `⇧enter` metin o agent'a gider.

Yapıştırılan satıra bütünüyle girer (bracketed paste): satır sonları yeni satır olur, sekmeler korunur, diğer kontrol karakterleri atılır ve hiçbir şey onunla gönderilmez. Bundan önce bir yapıştırmanın ilk satır sonu yarım bir isteği gönderiyordu. Yazmıyorken yapıştırılan atılır ve alt satır önce yazmaya başlamanızı söyler. Günlük ve ekip çok satırlı bir mesajı tek satırda gösterir; konuşma satırlarıyla.

## Kendi kabuğunuz

Satır `!` ile başladığı sürece etiketi ` › kabuk: ` temanın kabuk rengi üzerinde durur (terminalin temasında pembe, mono'da ters renk); böylece kabuğunuz için bir komut bir mesajla ya da bir `:` komutuyla karışmaz; `!` gidince etiket yine düzdür. `!cmd`, `cmd`'yi kendi kabuğunuzda (`$SHELL`, yoksa `/bin/sh`) checkout içinde çalıştırır, vim'deki `:!` gibi: ekran kenara çekilir ve komut kendine ait bir terminalde (`script`'in yaptığı gibi bir pty) çalışır; imece tuşlarınızı ona iletir, çıktısını geldikçe ekrana kopyalar. O terminal sizinki gibi ayarlı başlar ve pencerenizin boyutunu izler; böylece `git log`, `less` ve `vim` çalışır ve ctrl-c imece'yi değil komutu durdurur. ctrl-z hiçbir şeyi durdurmaz, çünkü durmuş bir komut ekranı tutardı. Bir satır nasıl bittiğini ve ne kadar sürdüğünü söyler (`[imece] çıkış 0 · 3 sn sürdü · herhangi bir tuş: ekrana dön`) ve herhangi bir tuş ekranı geri getirir. Günlük komutu, çıkış kodunu (ya da durduran sinyali) ve süresini tutar: `` `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü ``. Çıktı terminalin ana ekranında kalır, imece çıkınca yeniden görürsünüz. imece sonunu (son 64 KiB) `O` için bellekte tutar, diske hiç yazmaz; bir agent'a yalnızca siz gönderirseniz ulaşır. Terminalin başlığı sonra yeniden yazılır.

`O` (ya da `:output`) eldeki projenin son `!kmt`'sini bir mesajda alıntılar. Komut satırı eldeki agent'a (yoksa projenin başına) açılır; yazdığınız satırın altında komutun nasıl bittiği ve gösterdiği son 40 satır durur. Renkler ve bir ilerleme çubuğunun üstüne yazdıkları gider, sır gibi görünen sözcükler gizlenir; satırlar, içlerindeki her ters tırnak dizisinden uzun bir `text` çiti içindedir:

````text
@lead 
`cargo test` çalıştırdım → çıkış 101 · 12 sn sürdü
```text
… üstteki satırlar: 214
test parse::tests::empty ... FAILED
test result: FAILED. 41 passed; 1 failed
```
````

`enter`a basana kadar hiçbir şey gönderilmez; `esc` bırakır. Her proje yalnızca en yeni çıktısını tutar; başlayamayan bir komut sonuncuyu olduğu gibi bırakır. Projede henüz komut çalışmadıysa: `bu projede henüz !komut çalışmadı`.

`:commit`, düzenleyip bu yolla çalıştırmanız için satıra `!git commit -m '…'` koyar. Bkz. [kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md).

## Editörünüz

`^x` `^e` (bash ve zsh'teki gibi) ya da `:edit` yazmakta olduğunuz satırı editörünüzde açar: `$VISUAL`, yoksa `$EDITOR`, yoksa `vi`; `code --wait` ya da `emacsclient -t` gibi bir satır da olur, kabuğunuzun okuyacağı gibi. Ekran bir `!` komutundaki gibi kenara çekilir ve editör projenin checkout'unda, sizin terminalinizde çalışır. Kaydedip çıkınca metin komut satırına döner, imleç sonunda, sondaki satır sonları alınmış; hiçbir şey gönderilmez: `enter` gönderir, ya da önce değiştirirsiniz. `^x`'ten sonra `^e`'den başka bir tuş, tek başına ne yapıyorsa onu yapar. Taslak, imece'nin durum dizininde yalnız sizin okuyabildiğiniz (0600), yeni yaratılmış kendi dosyasıdır (`.md`, editör Markdown olarak renklendirsin diye) ve editör kapanınca, ne olursa olsun silinir. Hatayla çıkan bir editör (vim'de `:cq` vazgeçmenin yoludur), durdurulan, 1 MiB'tan fazlasını ya da UTF-8 metin olmayanı bırakan bir editör taslağı olduğu gibi bırakır; terminalde bıraktıklarının (kendi şikâyeti de içinde) altındaki bir satır nedenini söyler ve herhangi bir tuş ekranı geri getirir. Editör açıkken ctrl-c editörün, imece'nin değil. `:edit TEXT` editörü `TEXT` ile açar. Bkz. [komutlar](../reference/commands.md).

## Ayrıca bakınız

- [Komutlar başvurusu](../reference/commands.md)
- [Konuşmalar](conversations.md)
- [Temalar ve dil](themes-and-language.md)
- [Klavye](keyboard.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
