---
title: Temalar ve dil
description: Ekran İngilizce ya da Türkçe, ve renkleri - terminalin kendi renkleri, dark, light, mono, on beş base16 şeması ya da Ghostty ya da Kitty biçiminde bir renk dosyası.
---

# Temalar ve dil

Ekran İngilizce ve Türkçe konuşur ve siz başka bir şey seçmedikçe terminalinizin kendi renklerini giyer. İkisi de bir tuş uzakta: `L` dili değiştirir, `T` temalarda ilerler.

## Dil

| Nasıl | Ne |
| --- | --- |
| `L`, `:lang` | diğer dil, hemen, bu çalıştırma için |
| `:lang en`, `:lang tr` | adı verilen |
| `--lang en\|tr` | başlangıçta |
| `IMECE_LANG=tr` | bayrak verilmediğinde varsayılan; `en` ya da `tr` dışındaki her şey başlangıçta reddedilir |

Varsayılan İngilizce. Dil çalıştırmalar arasında saklanmaz: kalıcı bir seçim için `IMECE_LANG` ya da `--lang` kullanın.

Ekrandaki her sözcük tek bir yerde, iki dilde ve aynı yer tutucularla yaşar, böylece hiçbir şey yalnız İngilizce değildir: yardım, lejant, panel başlıkları (WAITING FOR YOU → SENİ BEKLEYEN, JOURNAL → DEFTER, SALON → SALON), günlüğün satırları, flaşlar, terminalin başlığı (`imece · 2 seni bekliyor`), dışa aktarılan Markdown'ın başlıkları. Model olayları veri olarak saklar ve çizim anında sözcüğe döker, bu yüzden `L` yalnız yeni satırları değil bütün günlüğü yeniden yazar. Tuşlar ve `:` komutları iki dilde aynıdır. `?` yardımının `/` süzgeci Türkçe harfleri katlar, böylece `calis`, onlar olmayan bir klavyeden `çalış`'ı bulur.

Agent'ların söylediği hangi dilde yazdıklarıysa odur; imece asla çevirmez. Ham akış (iz) satıcının gönderdiği gibi kalır.

## Temalar

| Nasıl | Ne |
| --- | --- |
| `T`, `:theme` | sonraki yerleşik tema, ilke dönerek; flaş `tema: nord` der |
| `:theme AD` (`:t AD`) | adı verilen; yazarken ekran onu giyer, `enter` tutar, `esc` vazgeçer |
| `:theme DOSYA` | bir renk dosyası |
| `--theme NAME\|FILE` | başlangıçta |
| `IMECE_THEME` | bayrak verilmediğinde varsayılan; bilinmeyen bir ad başlangıçta reddedilir |

Son seçim (`T`, `:theme`) sonraki çalıştırma için depoda hatırlanır, ad ya da dosyanın mutlak yolu olarak; bir önizleme asla hatırlanmaz.

### Yerleşikler

| Ad | Nedir |
| --- | --- |
| `terminal` | varsayılan: 16 ANSI rengi, böylece terminalinizin kendi teması (iTerm2, Ghostty, Kitty, …) imece'yi renklendirir ve oradaki bir değişiklik hemen görünür. imece karanlığı aydınlıktan ayırmak ve seçimi ile tuş ipuçlarını gölgelemek için terminale arka plan ve ön plan rengini sorar |
| `dark`, `light` | ANSI renkleri, sormak yerine karanlık ya da aydınlık bir arka plan varsayarak |
| `mono` | renk yok: yalnız şekil ve kalınlık. İşaretler yine okunsun diye seçilmiştir (`●` `✓` `✕` şekilce ayrılır) |

Sonra tinted-theming/schemes'ten (MIT) kopyalanan on beş base16 şeması, `T`'nin döndürdüğü sırayla: `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`. Bir şema bütün ekranı boyar, arka plan dahil, böylece her terminalde aynı görünür; base16'nın turuncusu imece'nin vurgu rengidir.

`:theme` yerleşik adları tamamlar; `imece --help` ilk birkaçını sayar.

### Renk dosyaları

`:theme DOSYA` ya da `--theme DOSYA`, Ghostty biçiminde (`palette = 4=#rrggbb`, `background = `, `foreground = `) ya da Kitty biçiminde (`color4 #rrggbb`, `background #rrggbb`) bir renk dosyası okur, en çok 64 KiB. Dosyanın gövde adı temanın adıdır, mutlak yoluyla hatırlanır. 16 yuvanın hepsi, arka plan ve ön plan olmalıdır (`selection-background` olabilir); diğer anahtarlar yok sayılır, böylece renkleri tutan bütün bir Ghostty yapılandırması da geçer. Yuvalar rolleri verir (mavi vurgu rengidir), arka plan ve ön plan kalanı. İçinde `/` olan ya da bir dosya olan bir ad dosya olarak okunur; başka her şey yerleşik bir ad olmalıdır. Okunmayanlar: Alacritty'nin TOML'u ve `.itermcolors`; önce dönüştürün (mbadolato'nun iTerm2-Color-Schemes deposunda her biçim var).

`COLORTERM=truecolor` ya da `24bit` olmadan RGB renkler 256 renklik küpe yuvarlanır.

### Terminali izlemek

`terminal` teması terminalin aydınlık ya da karanlığa dönmesini izler. Kendi değişimini bildiren terminaller imece'nin terminal kitaplığı üzerinden duyulamaz, bu yüzden imece odak bildirimlerini açar ve penceresi her odağa döndüğünde terminale renklerini yeniden sorar; tema yalnız değiştiklerinde yeniden kurulur. Pencere odağı tutarken olan bir değişim sonraki odakta görünür; tmux odağı yalnız `focus-events on` ile geçirir; yanıt vermeyen bir terminal bilineni korur. Sorma anında basılan bir tuş yanıtla birlikte okunabilir.

Başlangıçta sonda renkler için 200 ms, sonra bir imleç konumu raporu için (bir saniyeye kadar) bekler, böylece geç bir yanıt tuş akışına asla sızmaz; `COLORFGBG` ipucu olarak alınır.

### Soluk arka plan

Tema ne olsa, salonun üstündeki bir pencere (bir konuşma, yardım, özet, nerede kalmıştın, çıkış listesi) çevresindeki her şeyi soluklaştırır, böylece önce pencere okunur. Dikkat tek renktir: sizi bekleyenler, bitmek üzere bir bütçe, %90'dan itibaren bir satıcı penceresi.

## Ayrıca bakınız

- [Komut satırı](command-line.md)
- [CLI başvurusu](../reference/cli.md)
- [Komutlar başvurusu](../reference/commands.md)
- [Klavye](keyboard.md)
