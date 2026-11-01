---
title: "Komut satırı"
description: "Yazdığınız alt satır - lidere ya da tek bir agent'a, satıcı komutları, imece'nin : komutları ve menüsü, tamamlama, readline tuşları, çok satırlı yazma ve kendi kabuğunuz."
---

# Komut satırı

Alt satır yazdığınız yerdir. `/` (ya da vim'deki gibi `i`) onu seçili projenin liderine açar, `@` tek bir agent'a, `:` imece'ye, `!` kendi kabuğunuza, `#` bir karta. Satırı izleyen ipucu `enter`'ın ne yaptığını ve satır nasıl ekleneceğini söyler; `esc` vazgeçer.

```text
 › lead için: boş dosyadaki parser çökmesini düzelt█     enter gönder · ⇧enter yeni satır · esc vazgeç
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
| `/cmd`, `@name /cmd` | agent'ın kendi komut satırı, mesaj değil (Türkçe yardımda `/kmt`): `/clear`, `/compact`, `/cost`, `/context`, satıcının CLI'si ne biliyorsa. Bir tur olarak çalışır ve yanıtı bir cevap gibi döner. Codex'in app server'ında komut satırı yok, orada yalnız `/compact` çalışır; kendi `/clear`'ı olmayan bir satıcıda `/clear` oturumu bitirir, sonraki mesaj yenisini başlatır |
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
| `:lead VENDOR` | | sonraki açılıştan itibaren kim lider: `claude`, `codex`, `gemini`, ya da o zaman sorulması için `ask` |
| `:open PATH [NAME]` (`:o`) | | PATH'teki çalışma kopyasını kendi lideriyle bir proje olarak aç (Türkçe yardımda `:open YOL`) |
| `:close NAME` (`:c`) | | bir projeyi kapat: agent'ları ayrılır, worktree'leri diskte kalır (`:close AD`) |
| `:rename NAME` | | eldeki projeyi adlandır; lideri `lead-NAME` olur (`:rename AD`) |
| `:forget @NAME` (`:f`) | | agent oturumunu unutur; yenisine kaldığı yer anlatılır (`:forget @AD`) |
| `:export [PATH]` | | açık konuşma, yoksa kartın ekranı, Markdown olarak (`:export YOL`) |
| `:commit` | | ucuz bir model stage'dekiler için commit mesajı önerir, satıra `!git commit -m '…'` olarak konur |
| `:copy PATH` | | git'in yok saydığı bir yolu (`.env`, `target`) işçilerin worktree'lerine başlarken kopyala; yine basılınca bırakır (`:copy YOL`) |
| `:pin [NAME]` | | projeyi salonun en üstüne sabitle; yine basılınca bırakır (`:pin AD`) |
| `:awake` | | bir agent tur ortasındayken makineyi uyanık tut; yine basılınca kapatır |
| `:notify [off\|bell\|text]` | | başka penceredeyken sizi bekleyen bir şey gelirse zil ya da masaüstü bildirimi; yalın yazılınca sıradaki |
| `:salon` `:board` `:code` | `1` `2` `3` | üç görünüm |
| `:lang [en\|tr]` | `L` | diğer dil, ya da adı verilen |
| `:theme [NAME\|FILE]` (`:t`) | `T` | sonraki yerleşik tema, ya da adı verilen (`:theme AD ya da DOSYA`) |
| `:help` | `?` | yardım |
| `:quit` (`:q`) | `q` | çık; `q` gibi önce sorar |
| `:pending` | `q` `s` | çıkmadan önce: tur ortasındaki agent'lar ve incelenmemiş değişiklikler |
| `:next` / `:defer` | `W` / `S` | bekleyen sıradaki ögeyi aç / bunu ertele |
| `:waiting` | `w` | bekleyenler paneli |
| `:needs` | `f` | salon yalnız sizi bekleyenleri gösterir |
| `:fold` / `:unfold` | `zM` / `zR` | her projeyi ve her işçinin yardımcılarını katla / aç |
| `:trace [@NAME]` / `:talk [@NAME]` | `t` / `v` | adı verilen, yoksa eldeki agent'ın ham akışı / konuşması |
| `:mark` | `m` | eldeki satırı işaretle |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` | yalnız demo; canlıda bunu söyler ve menüde yer almaz |

Bir salon panelinde çalışan komutlar (`:waiting`, `:needs`, `:fold`, `:unfold`) önce o panele gider. Her komutun ne aldığı ve ne yanıtladığı [komutlar başvurusunda](../reference/commands.md).

### Menü ve değerler

Menü her komutun değerini ardında, ayraç içinde gösterir; çizilir, asla eklenmez: `budget ($3.10 / $20.00 harcandı)`, `lead (claude)` ya da `(ask)`, `theme (dark)`, `lang (Türkçe)`, `close` ve `rename` eldeki projeyle, `forget` `talk` `trace` eldeki agent'la, `next` ve `waiting` kaç öge beklediğiyle, `needs` ve `pause` açık ya da kapalı, `faster` `slower` hızla, `pin` projeyle (sabitse `▴` ile), `export` ne yazacağıyla (`konuşma`, `#12`). `$` sonraki faturayı büyüteni işaretler: `:open` (yeni bir lider oturumu), `:forget` (sonraki mesaj, bütün brifingi yeniden ödeyen yeni bir oturum başlatır) ve `:commit` (mesajı bir modele sorar). Başka hiçbir `:` komutu kendi başına ücretli bir tur başlatmaz.

### Kısa adlar

`b` `c` `f` `o` `q` `t`, `budget` `close` `forget` `open` `quit` `theme` yerine geçer. Bütünüyle yazılan bir kısa ad zaten bir komuttur, menü hiçbir şey önermez ve `enter` çalıştırır: `:q` `enter` hemen sorar, `:b 80` bütçeyi artırır.

### Tema önizlemesi

`:theme AD` yerleşik bir tema adıyla yazılırken, yazılmış ya da `tab` ile üstüne gelinmiş, ekran onu giyer; `enter` tutar, `esc` vazgeçer ve satır artık tema adlandırmadığında kullanılan tema geri gelir. Bkz. [temalar ve dil](themes-and-language.md).

## Tamamlama

Yazmak bir kabuktaki gibi tamamlanır: `@` ardından agent adları (eldeki proje önce, gidenler dışarıda, önek eşleşmesi, büyük küçük harf ayırmadan), `:` ardından imece'nin komutları ve `:lead` ardından satıcılar, `/` ya da `@name /` ardından o agent'ın oturumunun bildirdiği kendi komut satırı (asla sabit bir liste değil, böylece satıcının yeni komutu sürüm gerektirmez; oturum söyleyene dek boş, ve bir oturum hiç komut listelemediğinde günlük yalnız `/clear`'ın tamamlanacağını söyler), `#` ardından kartlar, `:open` ve `:export` ardından dizinler; `:pin` ve `:close` açık projelerin adlarını tamamlar. Eşleşmeler yazarken alt satırın üstünde tek satırda görünür; sözcüğe eşit tek eşleşme menü değildir.

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

Tek bir kesik vardır, halka değil: bir şey alan her kesme öncekinin yerini alır. İmlecin altındaki karakter ters renkte çizilir, `█` bloğu sonu gösterir. macOS'ta alt tuşları için terminalin Option'ı Meta olarak göndermesi gerekir (iTerm2: Profiles → Keys → Left Option key: Esc+; Terminal.app: Use Option as Meta key); yoksa Option-b `∫` yazar. Burada olmayanlar: kill-ring döngüsü, geri alma, yer değiştirme, vim'in normal kipi.

## Birkaç satır

`⇧enter` yeni satır başlatır, Claude Code'daki gibi, terminal onu `enter`'dan ayırabildiğinde: kitty klavye protokolünü konuşuyorsa (kitty, Ghostty, `enable_kitty_keyboard` ile WezTerm, foot, Alacritty, iTerm2), imece bunu başlangıçta sorar. Her yerde `\` sonra `enter`, `\`'i göndermek yerine yeni satıra çevirir; Option Meta olduğu yerde `⌥enter` da. `?` yardımı ve alt satırın ipucu terminalinizdekini adlandırır: `⇧enter yeni satır` ya da `\ enter yeni satır`. Sorguya yanıt vermeyen bir çoklayıcı (tmux, screen) yedeği alır.

Alt satır metnin her satırı için bir sıra alır, en çok sekiz ve ekranın üçte biri; ilk sıra metnin kime gittiğini taşır, diğerleri altında içeriden gelir. Bütün satırlar sığmadığında imlecin satırı görünür kalır ve ipucu nerede olduğuyla başlar, `3/12`. `^a` `^e`, home/end ve `^u` `^k` imlecin olduğu satırda kalır; `↑` `↓` satırlar arasında gider, sütunu koruyarak (menü gösterilirken onun yerine menüde ilerler); satır başında backspace üsttekiyle birleştirir. `@name` bir boşlukta ya da yeni satırda biter, yani `@name` `⇧enter` metin o agent'a gider.

Yapıştırılan satıra bütünüyle girer (bracketed paste): satır sonları yeni satır olur, sekmeler korunur, diğer kontrol karakterleri atılır ve hiçbir şey onunla gönderilmez. Bundan önce bir yapıştırmanın ilk satır sonu yarım bir isteği gönderiyordu. Yazmıyorken yapıştırılan atılır ve alt satır önce yazmaya başlamanızı söyler. Günlük ve salon çok satırlı bir mesajı tek satırda gösterir; konuşma satırlarıyla.

## Kendi kabuğunuz

`!cmd`, `cmd`'yi kendi kabuğunuzda (`$SHELL`, yoksa `/bin/sh`) checkout içinde çalıştırır, vim'deki `:!` gibi: ekran kenara çekilir, komutun terminali kendine olur (böylece `git log`, `less` ve `vim` çalışır ve ctrl-c imece'yi değil komutu durdurur), bir satır nasıl bittiğini ve ne kadar sürdüğünü söyler (`[imece] çıkış 0 · 3 sn sürdü · herhangi bir tuş: ekrana dön`) ve herhangi bir tuş ekranı geri getirir. Günlük komutu, çıkış kodunu (ya da durduran sinyali) ve süresini tutar: `` `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü ``. Çıktı terminalin ana ekranında kalır, imece çıkınca yeniden görürsünüz; hiçbir şeyi bir agent'a ulaşmaz. Terminalin başlığı sonra yeniden yazılır.

`:commit`, düzenleyip bu yolla çalıştırmanız için satıra `!git commit -m '…'` koyar. Bkz. [kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md).

## Ayrıca bakınız

- [Komutlar başvurusu](../reference/commands.md)
- [Konuşmalar](conversations.md)
- [Temalar ve dil](themes-and-language.md)
- [Klavye](keyboard.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
