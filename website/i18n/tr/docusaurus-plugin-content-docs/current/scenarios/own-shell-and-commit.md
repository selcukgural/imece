---
title: "Takımın yanında kendi kabuğunuz"
description: Ajanların yanında kişinin kendi araçları; kabuğunuzda komutlar, önerilen commit mesajı ve tavanları, makineyi uyanık tutmak, çalışırken bütçeyi değiştirmek, dışa aktarma, temalar ve dil.
---

# Takımın yanında kendi kabuğunuz

imece terminalinizin yerini almaz: `!kmt` ekran kenara çekilmiş hâlde kendi kabuğunuzda kendi komutunuzu çalıştırır, `:commit` stage'lediğiniz için ucuz bir modelden commit mesajı ister ve birkaç komut ajanlar çalışırken çalışmayı değiştirir. Bunlarla bir değişiklik uygulanır uygulanmaz, kendiniz derlemek, bakmak ve commit etmek istediğinizde karşılaşırsınız. Bu senaryo bununla geçen bir akşamdır.

## 1. `!cargo test`

`!` tuşuna basın. Komut satırı ` › kabuk: ` olarak, `!` yazılmış hâlde açılır. `cargo test` yazın ve `enter`a basın. Ekran kenara çekilir (alternatif ekran bırakılır) ve komut çalışma kopyasında, `$SHELL`inizde, tuşlarınızı alan ve çıktısını geldikçe gösteren kendine ait bir terminalde çalışır. Çıktısının altında:

```text
test result: ok. 42 passed; 0 failed

[imece] çıkış 0 · 12 sn sürdü · herhangi bir tuş: ekrana dön
```

Herhangi bir tuş ekranı geri getirir (tuş yutulur, başka bir şey olmaz). Günlük komutu, çıkış kodunu ve ne kadar sürdüğünü tutar:

```text
 20:02 › `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü
```

Süre onu söyleyen en büyük birimlerle verilir (`40 ms`, `2 sn`, `1 dk 5 sn`, `1 sa 2 dk`). Çıktı terminalin ana ekranında kalır, imece çıkınca yeniden görürsünüz; sonu `O` için bellekte tutulur ve bir ajana yalnızca siz gönderirseniz ulaşır. Bir test mi kaldı? `O`ya basın: komut satırı lidere `` `cargo test` çalıştırdım → çıkış 101 · 12 sn sürdü `` ve yazdığınız satırın altında bir çit içinde son 40 satırla açılır; üstüne `önce bunu düzelt` yazıp `enter`a basın. Başlayamayan bir komut yalnızca anlık mesajla söylenir (`… çalıştırılamadı`); boş olanı `komut boş` der.

## 2. `!git log`, `!vim`

Komutun terminali tuşlarınızı ve pencere boyutunu ilettiği için etkileşimli programlar çalışır: `!git log` sayfalayıcısını açar, `!vim src/lib.rs` düzenler, `!less target/out.txt` kaydırır. Onlardan her zamanki gibi çıkın; `[imece] çıkış …` satırı altlarında bekler.

## 3. ctrl-c komutu durdurur, imece'yi değil

`!cargo bench` çalıştırın ve ctrl-c'ye basın. Komut çalışırken imece SIGINT'i yok sayar ve çocuk varsayılan işleyiciyi alır; böylece komut ölür, imece ölmez:

```text
[imece] 2 sinyali durdurdu · 3 sn sürdü · herhangi bir tuş: ekrana dön
```

```text
 20:05 › `cargo bench` çalıştırdın → 2 sinyali durdurdu · 3 sn sürdü
```

## 4. `:commit`

Bir işçinin değişikliği uygulandı: çalışma kopyanızda stage'de. `:commit` yazın ve `enter`a basın. Anlık mesaj `haiku modeline parser içinde stage'dekiler için commit mesajı soruluyor` der.

Olan şu: imece `git diff --cached --stat`ı, stage'deki yamayı (en çok 24 KiB, bir satırda kesilir ve kesildiği söylenir) ve deponun üslubu için en yeni sekiz commit konusunu okur; `--no-ext-diff --no-textconv` ile, sizin hiçbir programınız çalışmaz. Ucuz kademede (`haiku`, ya da sizin `--tier claude:cheap=…`) tek bir Claude Code süreci başlatır: araçsız, MCP sunucusuz, Claude Code'unki yerine imece'nin kendi kısa sistem istemiyle ve `--no-session-persistence` ile; `init`i her oturumunki gibi denetlenir; okunacak hiçbir şeyin olmadığı bir dizinde çalışır ve 120 sn sonra vazgeçilir. Sistem istemi diff'in veri olduğunu, talimat olmadığını söyler. Yanıt temizlenir, çitler ve boş uçlar düşürülür, en çok 30 satır ve 2000 karakter, kabuk için tek tırnaklanır (`'` → `'\''`, böylece içindeki `$(…)` metin kalır).

Öneri, komut satırı boşalınca, okuyup düzenlemeniz ve `enter` ile çalıştırmanız ya da `esc` ile bırakmanız için bir `!` komutu olarak gelir:

```text
 › kabuk: !git commit -m 'Read defaults from config v2

Values missing from the file fall back to Config::default(); the loader no longer panics on a partial file.'
```

Eldeki projeden başka bir proje için `!git -C /path/to/checkout commit -m '…'` olur. imece kendisi hiçbir şeyi commit etmez.

```text
 20:10 ! haiku stage'dekiler için bir commit mesajı önerdi, $0.01 tuttu
```

### Ne tutar

| Tavan | Değer |
| --- | --- |
| Bir öneri, en çok | $0.05; çağrıdan önce bütçeden ayrılır, sonra tuttuğuyla hesaplaşılır |
| Bir gün (UTC), bütün projeler birlikte | 20 öneri ve $1.00 |
| Aynı stage'lenmiş ağaç yeniden | hiçbir şey: proje başına son öneri saklanır ve satıra yeniden konur |

Ayırma kayıt dosyasında tek bir işlemdir ve bütçe onu tutar; bütçede $0.05'ten az varken ya da gün dolmuşken reddedilir (`commit mesajı yok: today's 20 suggestions were made`). Başlayamayan bir çağrı hiçbir şey tutmaz; bildirmeden zamanı dolan tavanını tutar. Başarısızlıklar nedeni ve tuttuğuyla günlüğe girer: `commit mesajı gelmedi ($0.00): nothing is staged`. Yalnızca Claude Code'a sorulabilir, çünkü çağrısı sert bir harcama tavanı alan tek satıcı odur; bir Codex ya da Gemini liderinin bunun için PATH'te Claude Code'a ihtiyacı vardır, yoksa `:commit` reddedilir. `:` menüsü `:commit`i `$` ile işaretler.

## 5. Uzun bir gece çalışması: `--awake`, `:awake`

Tur ortasında uyuyan bir dizüstü her oturumu keser. `imece --awake` ya da çalışırken `:awake`:

```text
 bir ajan turdayken makine uyanık kalır
```

Açıkken ve herhangi bir ajan tur ortasındayken imece sistemin kendi aracını çocuk olarak çalıştırır, macOS'ta `caffeinate -i -w PID` ya da Linux'ta `systemd-inhibit --what=idle:sleep --mode=block sleep infinity`, ve hiç kimse tur ortasında değilken, siz kapattığınızda ya da imece bitince bırakır. `:awake` yine kapatır (`makine yeniden uyuyabilir`); `:` menüsü `açık` ya da `kapalı` gösterir. Varsayılan olarak kapalıdır ve çalışmalar arasında hatırlanmaz, çünkü makinenin güç davranışını değiştirir. Başlamayan bir araç bir kez söylenir (`makine uyanık tutulamadı: …`) ve o çalışmada yeniden denenmez.

## 6. Çalışırken `:budget 80`

Başlık `$41.20 / $50.00 harcandı ($6.10/sa, ~1 sa 26 dk kaldı)` gösteriyor; yarım saatten az kalınca dikkat rengine döner ve günlük bir kez söylerdi. `:budget 80` (ya da `:b 80`) yazın ve `enter`:

```text
 bütçe artık $80.00
```

```text
 20:30 › bütçeyi $80.00 yaptın (önce $50.00)
```

Sürmekte olan bir tur eski tavanında biter; sonraki yeni kalanla başlar. Bütçede yer varken turu tavanında biten bir Claude oturumu aynı istemlerle ve yeni kalanla yeniden başlatılır. Bütçe her proje için tektir ve çalışmalar arasında saklanır; harcanandan düşük bir `:budget` her sonraki turu durdurur.

## 7. `:export`

Bir konuşma (satırda `v`) ya da bir kartın ekranı (`#N`) açıkken `:export` onu imece'nin başladığı yerin yanına Markdown olarak yazar (`/Users/you/code/parser/imece-conversation-20261101-2035.md dosyasına yazıldı`) ya da adlandırdığınız dizine ya da dosyaya (`:export ~/notes/`). Var olan bir dosyanın üstüne asla yazmaz. Açık bir şey yokken: `dışa aktarılacak bir şey yok: önce bir konuşma (v) ya da iş (#N) aç`.

## 8. Önizleme olarak `:theme nord`

`:theme ` yazın ve `tab`. Menü yerleşik temaları listeler (`terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `catppuccin-mocha`, `tokyo-night-dark`, `solarized-dark`, …) ve adı satırdayken, yazılmış ya da adımlanmış, ekran her birini giyer. `enter` tutar (`tema: nord`), `esc` vazgeçer ve kullanılan tema geri gelir. `T` sırayla dolaşır; `:theme DOSYA` Ghostty ya da Kitty biçiminde bir renk dosyası alır; `--theme` ve `IMECE_THEME` başta aynısını yapar. Seçim hatırlanır. Varsayılan `terminal`, terminalinizin kendi 16 rengidir ve açığa ya da koyuya dönmesini izler: penceresi odağa geri geldiğinde imece renkleri yeniden sorar.

## 9. `L`

`L`ye basın. Ekrandaki her sözcük Türkçeye döner (`:lang tr`; yine `L` ya da `:lang en` geri; başta `--lang` ve `IMECE_LANG`). Günlük veri olarak saklanır ve çizim anında sözcüklere dökülür, bu yüzden eski satırlar da döner; ajanların yazdığı yazdıkları gibi kalır.

## Ayrıca bakın

- [Komut satırı](../screen/command-line.md): `!`, `:`, tamamlama, readline tuşları
- [Temalar ve dil](../screen/themes-and-language.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Komutlar](../reference/commands.md)
- [Kırık bir test, tek işçi](one-fix.md)
