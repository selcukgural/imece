---
title: Komut satırı
description: imece'nin her bayrağı, varsayılanı ve aralığıyla; okunan ortam değişkenleri, yanlış bir bayrağın söylediği ve çalıştırmalar arasında hatırlananlar.
---

# Komut satırı

`imece` bir avuç bayrak alır: çalışma kopyası, takımın üst sınırları, modeller, bütçe ve ekran için birkaç anahtar. Onlarla bir kez, başlangıçta karşılaşırsınız; ayarladıklarının çoğu ajanlar çalışırken ekrandan bir `:` komutuyla da değiştirilebilir ([komutlar](commands.md)).

## Özet

```text
usage: imece [--repo DIR] [--agents N] [--helpers N]
             [--solo claude[:MODEL][@EFFORT]] [--model NAME] [--tier T=NAME]...
             [--budget USD] [--fresh] [--awake] [--confine standard|strict|open]
             [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]
       imece --version
```

```sh
imece                                              # içinde olduğunuz çalışma kopyası, lider tek başına
imece --agents 8 --budget 10 --model sonnet
imece --demo --projects 3 --agents 30              # sahte ajanlar, ekranı iş başında görmek için
imece --demo --lang tr                             # ya da IMECE_LANG=tr
```

## Bayraklar nasıl okunur

- `--demo`, `--fresh` ve `--awake` dışında her bayrak bir değer alır: sonraki kelime. Değersiz bir bayrak reddedilir: `--agents needs a value`, ardından kullanım metni gelir.
- Bayraklar herhangi bir sırada gelebilir. `--model` ve `--tier` yinelenebilir; ötekiler son değerlerini alır.
- Bir sayı aralığıyla karşılaştırılır; dışındaysa imece `--helpers: 9 is not in 0-8` der (bayrağın aralığı).
- `--projects`, `--researchers` ve `--seed` `--demo`'ya aittir. Onsuz verilirse imece `--projects is for --demo` der ve kullanımı yazar.
- `-h` ya da `--help` kullanım metnini yazar. Reddedilen her komut satırı gibi 2 çıkış koduyla biter.
- İlk bayrak `--version` (ya da `-V`) ise stdout'a `imece 0.1.0` yazar ve 0 çıkış koduyla biter; kurulum betiği yerine koyduğu dosyayı böyle denetler.
- `--live` kabul edilir ve yok sayılır: canlı ajanlar varsayılan olmadan önce onlar bu bayrakla istenirdi.
- `imece mcp-relay SOCKET` sizin için değildir: Claude Code imece'yi bu biçimde kendi MCP sunucusu olarak başlatır ve süreç yalnızca CLI ile imece'nin o ajana ait soketi arasında bayt taşır ([mesaj türleri](message-types.md)).

## Çıkış kodları

| Kod | Ne zaman | Ne görürsünüz |
| --- | --- | --- |
| 0 | `--version`, ya da olağan bir çıkış (`q q`, `:quit`, ya da dışarıdan SIGTERM, SIGHUP ve SIGINT; imece bunları olağan bir çıkışa çevirir ki oturumlar dursun ve veriyolunun soketleri kaldırılsın; terminalin kendisi gidince de aynısı: sekmesi ya da penceresi kapandığında, tmux sunucusu bittiğinde) | kabuğunuzun başlığı geri gelir |
| 1 | Bayraklar okunduktan sonra başlangıç başarısız oldu: PATH'te kodlama ajanı yok, `--repo` bir git çalışma kopyası değil, depoyu başka bir imece tutuyor, depo okunamıyor, bir oturum başlatılamadı | ekran geri getirildikten sonra stderr'de `imece: <neden>` |
| 2 | Bir bayrak yanlış, ya da `--help` | stderr'de mesaj; eksik değerde, bilinmeyen seçenekte ve canlıda verilen demo bayrağında ardından kullanım metni |

Çıkış kodu 1'in bazı mesajları, aynen: `another imece is running on <yol>/imece.sqlite; open the other checkout from it with :open`.

## Çalıştırmalar arasında hatırlanan ve hatırlanmayan

| Hatırlanır (depoda, [dosyalar](files.md)) | Çalıştırma başına |
| --- | --- |
| tema (`--theme`, `T`, `:theme`; bir bayrak ya da `IMECE_THEME` kayıtlı olanı geçer) | dil (`--lang`, `IMECE_LANG`, yoksa İngilizce; `L` bu çalıştırma için değiştirir) |
| bütçe (`--budget` ayarlar; bayraksız bırakıldığı gibi kalır; ilk seferde 50 $) | `--agents`, `--helpers`, `--confine`, `--run`, `--run-timeout` |
| | `--model` ve `--tier` |
| başka penceredeyken nasıl haber alacağınız (`:notify`), ekranda ne kadar şeyin hareket ettiği (`:motion`) | `--awake` (bilerek: makinenin güç davranışını değiştirir) |
| projeler, ajanlar, kartlar, günlük, posta, pano, sabitlemeler, işaretler, kopya listesi, her ajanın oturumu | başlıktaki kullanım pencereleri (bir oturum ilk turunda yeniden söyler) |

## Bayraklar

### `--repo DIR`

```sh
imece --repo ~/src/kule
```

imece'nin üzerinde çalıştığı çalışma kopyası. Varsayılan: içinde bulunulan dizin. imece `DIR`'ın içinde olduğu git çalışma kopyasının tepesini arar; birinin içinde olmayan bir dizin başlangıçta reddedilir (`<DIR>: …`, çıkış kodu 1). Çalışma kopyası `lead`'in yönettiği bir proje olur; başka çalışma kopyaları ekrandan `:open PATH [NAME]` ile açılır.

### `--agents N`

```sh
imece --agents 8        # canlı: 0-16, varsayılan 4
imece --demo --agents 30 # demo: 1-200, varsayılan 24
```

Canlı: bir liderin işe alabileceği en çok işçi (her projenin liderinde aynı sınır). `0` lideri tek başına çalıştırır; bu ona söylenir ve okuduğundan yanıt verir. 16'nın üstünde imece `--agents: a leader hires at most 16` der. Yardımcılar bu sayıya girmez. Demo aynı bayrağı liderlerin yanındaki işçiler için alır; `--demo --agents 0` reddedilir: `--agents: the demo needs at least 1`.

### `--helpers N`

```sh
imece --helpers 3       # 0-8, varsayılan 2
```

Bir işçinin aynı anda sahip olabileceği en çok yardımcı. `0`, işçinin her şeyi kendinin yapması demektir; işe almayı denediğinde bu ona söylenir.

### `--solo claude[:MODEL][@EFFORT]`

```sh
imece --solo claude                    # Claude Code, kendi modeli ve çabası
imece --solo claude:opus@xhigh         # opus, xhigh çabayla düşünen
```

Checkout'ta bir takım yerine bir solo oturumla başlar: kendinizin sürdüğü tek bir ajan, lidersiz; `MODEL` modelinde (varsayılan: Claude Code'un kendisi ya da `--model`'in adlandırdığı) ve `EFFORT` çabasında (bkz. `--model`). Proje dizinin adını alır, ajan `solo`dur (başka yerde `solo-AD`); kendi worktree'sinde yazar ve bir yanıtla biten her turda diff'i size gösterir. Bu checkout'ta aynı seçimle açık bir solo varsa onunla sürülür, böylece her sabah aynı komut aynı oturumu sürdürür; farklı bir seçim yanına `-2` adlı başka bir solo açar; burada açık bir takım, siz ona yazana dek uykuda kalır. Başka her şey sözlerle reddedilir. Demo da alır. Bkz. [solo oturumlar](../concepts/solo.md).

### `--model NAME`

```sh
imece --model sonnet
imece --model opus@xhigh                 # Claude'un opus'u, xhigh düşünce çabasıyla
imece --model claude:@max                # Claude'un varsayılan modeli en derin çabada
```

Ajanların çalıştığı model. Bu `standard` kademesidir: lider onda çalışır, standart kademede işe alınan her işçi de. Varsayılan: Claude Code'un kendi varsayılanı (orada ayarladığınız). `NAME` `[MODEL][@ÇABA]` biçimindedir, iki parça da isteğe bağlı: Claude Code'un aldığı biçimde bir model adı (`sonnet`, `claude-opus-5-5`, `opus[1m]`), `@` sonrasında bir çaba, ya da ikisi. Çabalar Claude Code'un kendi çabalarıdır: `low`, `medium`, `high`, `xhigh`, `max` (kendi `--effort`'u). Bütünü `-` ile başlamayan 1-64 karakter harf, rakam ve `-._[]@`'dir; başkası: `--model: "x y" is not a model name`. Başka bir çaba, bir şey başlamadan önce reddedilir; Claude Code kendisi yalnız uyarıp varsayılan çabasıyla çalışırdı, bu yüzden imece önce denetler.

### `--tier T=NAME`

```sh
imece --tier strong=opus[1m]
```

Bir kademenin karşılığı; `T` `cheap`, `standard` ya da `strong`. Lider kademeyle işe alır, modelle değil; basamaklar `haiku`, Claude Code'un varsayılanı ve `opus`'tur. Bu bayrak bir basamağı değiştirir; `NAME` `--model`'deki gibi `[MODEL][@ÇABA]`dır, yani `--tier strong=opus@xhigh` güçlü kademeyi Claude'un xhigh çabayla düşünen opus'u yapar. Redler: `--tier: "strong" is not TIER=MODEL, like strong=opus`, `--tier: "huge" is not cheap, standard or strong`, `--tier: "…" is not a model name` ve `--model`'in çaba redleri. Hesabınızda olmayan bir kademe modeli ilk turunda başarısız olur; imece o işçiyi standart modelle yeniden başlatır ve günlük bunu söyler ([takım](../concepts/team.md)).

### `--budget USD`

```sh
imece --budget 10       # dolar, en çok iki ondalık, en az 0.01
```

Tüm ajanların birlikte harcayabileceği en çok: bütün takım için tek bir bütçe, çalıştırmalar arasında saklanır. Bayrak onu ayarlar; bayraksız bütçe son çalıştırmanın bıraktığı gibi kalır, ilk seferde 50 $. Noktadan önce en çok dört, sonra iki rakam; `--budget 0` ve sayı olmayan her şey reddedilir: `--budget: abc is not dollars like 5 or 2.50`. `:budget USD` ajanlar çalışırken değiştirir. Bkz. [bütçe ve sınırlar](../concepts/budget-and-limits.md).

### `--fresh`

```sh
imece --fresh
```

Depo alınırken her ajanın hatırlanan oturumunu unutturur: her ajan yeni bir oturumda uyanır, kaldığı yer anlatılır, ve eski oturumların transkriptleri silinir (bkz. [dosyalar](files.md)). Günlük, gitmemiş her ajan için `X oturumunu unutturdun; yeni bir oturumla başlıyor, kaldığı yer anlatılacak` der. `:forget @NAME` aynısını imece çalışırken tek bir ajan için yapar. Yeni oturum bütün brifingi, önbelleksiz, yeniden öder. Arşivlenmiş bir projenin oturumları saklanır: onu geri getirmek onlarla sürer (bkz. [`:archive`](commands.md#archive-title)).

### `--awake`

```sh
imece --awake
```

Herhangi bir ajan tur ortasındayken makinenin uyumasını engeller: macOS'te `caffeinate -i -w PID`, Linux'ta `systemd-inhibit --what=idle:sleep --mode=block sleep infinity`, başka yerde hiçbir şey; kendi süreç grubunda bir çocuk olarak, hiçbir şey tur ortasında olmayınca ya da imece bitince bırakılır. Varsayılan kapalı ve hatırlanmaz; `:awake` imece çalışırken açar ya da kapar. Başlamayan bir araç flaşta bir kez söylenir ve o çalıştırmada yeniden denenmez. Demo da bayrağı alır.

### `--confine LEVEL`

```sh
imece --confine standard   # varsayılan
imece --confine strict
imece --confine open
```

Her ajanın oturumunun Claude Code'dan ne kadarını aldığı:

| Düzey | İşçiler ve yardımcılar | Lider | Kendi Claude Code kurulumunuz |
| --- | --- | --- | --- |
| `standard` (varsayılan) | Claude Code'un tam kodlama araç takımı: okuma, yazma, kabuk, web arama ve getirme | okuma ve web | yüklenmez |
| `strict` | yalnızca dosya araçları; komutlar imece'nin `run` aracıyla | okuma | yüklenmez |
| `open` | Claude Code sizin kurduğunuz gibi, alt-ajan aracı eksik | okuma, sizin kurulumunuz | yüklenir: ayarlar, hook'lar, MCP sunucuları, skill'ler |

Her düzeyde lider asla yazmaz (sizin çalışma kopyanızda çalışır; orada bir yazma Δ incelemesini atlardı) ve Claude Code'un kendi alt-ajan aracı kapalı kalır. imece hiçbir şeyi sandbox'lamaz: düzey için Claude Code'un kendi kısıtlamalarını açar ve oturumun onlarla çalıştığını, kapalıya düşerek, denetler; yalıtım bir konteynerin işidir. Günlük strict olmayan her başlangıçta bunu söyler. Başka bir şey: `--confine: "loose" is not standard, strict or open`. Bkz. [güvenlik](../concepts/security.md) ve [onaylar ve sorular](../concepts/approvals-and-questions.md).

### `--run PROG[,..]`

```sh
imece --run npm,make
imece --run npm --run pnpm       # yinelenebilir; yinelenenler düşer
```

İşçilerin ve yardımcıların `run` araçlarıyla, `cargo` ve git'in okuma komutlarının yanında çalıştırabileceği programlar. Yalnızca PATH'teki adlar; bir yol reddedilir: `--run: "./bin/x" is a path; name a program on PATH`. Lider bunları hiç almaz. `standard` ve `open` altında aynı liste bir oturumun size sormadan hangi kabuk komutlarını çalıştırabileceğine de karar verir (rutin kuralı, [onaylar](../concepts/approvals-and-questions.md)).

### `--run-timeout S`

```sh
imece --run-timeout 1200   # 10-3600 saniye, varsayılan 600
```

Bir `run`'ın süreç grubu öldürülmeden önce alabileceği süre. Aracın açıklaması ajana sayıyı söyler; geç dönen bir yanıt `killed after 600 s: it did not finish in time` der.

### `--lang en|tr`

```sh
imece --lang tr
IMECE_LANG=tr imece
```

Ekranın dili. Varsayılan İngilizce, ya da `IMECE_LANG`. `L` ya da `:lang` imece çalışırken değiştirir; seçim çalıştırma başınadır. Başka bir şey: `--lang: "de" is not en or tr` (değişken için `IMECE_LANG: "de" is not en or tr`). Ajanlarla her zaman İngilizce konuşulur.

### `--theme NAME|FILE`

```sh
imece --theme dark
imece --theme nord
imece --theme ~/.config/ghostty/themes/mine
IMECE_THEME=catppuccin-mocha imece
```

`terminal` (varsayılan: terminalinizin kendi renkleri; başlangıçta ve pencere yeniden odağa gelince sorulur, böylece açığa ya da koyuya dönen bir terminal izlenir), `dark`, `light`, `mono`, yerleşik şemalardan biri `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`, ya da Ghostty (`palette = 4=#rrggbb`, `background = #rrggbb`) veya Kitty (`color4 #rrggbb`, `background #rrggbb`) biçiminde bir renk dosyası; içinde `/` olan ya da bir dosya olan değer dosya olarak okunur. Son seçim (`T`, `:theme`) hatırlanır; bir bayrak ya da `IMECE_THEME` onu geçer. Yanlış bir ad: `--theme: "solar": not one of terminal|dark|light|mono|dracula|…|monokai and no such file`; sonraki başlangıçta artık olmayan bir dosya sessizce varsayılanı bırakır. Bkz. [temalar ve dil](../screen/themes-and-language.md).

### `--demo`

```sh
imece --demo
```

Gerçek oturumlar yerine sahte ajanlar, ekranı iş başında görmek için: canlıyla aynı modeli besleyen tohumlu bir simülasyon (`sim.rs`). Hiçbir şey başlatılmaz, harcanmaz, ekran seçimleri dışında kaydedilmez. Demo canlıyı yansıtır: canlının her özelliği demoda da görünür (demonun kendine ait birkaçı da vardır: `space`, `+`, `-`, araştırmacılar). Bkz. [demo](../getting-started/demo.md).

### `--projects N`

```sh
imece --demo --projects 3   # 1-4, varsayılan 2
```

Yalnızca demo: aynı anda üzerinde çalışılan projeler, her birinin kendi lideriyle.

### `--researchers N`

```sh
imece --demo --researchers 8   # 1-32, varsayılan 4
```

Yalnızca demo: aynı anda çalışabilecek araştırmacılar. Araştırmacılar canlıda yapılmamıştır; onları yalnızca demo gösterir.

### `--seed N`

```sh
imece --demo --seed 42   # herhangi bir tam sayı, varsayılan 7
```

Yalnızca demo: aynı tohum aynı sahte senaryoyu yeniden oynatır.

## Ortam değişkenleri

imece'nin kendisinin okuduğu:

| Değişken | Ne yapar |
| --- | --- |
| `IMECE_LANG` | `en` ya da `tr`, `--lang` verilmediğinde ekranın dili; yanlış değer bayrak gibi reddedilir |
| `IMECE_THEME` | `--theme` gibi bir tema adı ya da renk dosyası; aynı biçimde denetlenir |
| `IMECE_PROGRESS` | `on` ya da `off`: sekmenin ilerleme çubuğu (OSC 9;4) gönderilsin mi; verilmezse terminalin başlarken verdiği ada göre kitty 0.39+, Ghostty 1.2+ ve iTerm2 3.6.6+'ya, bir de tmux içinde (`TMUX`); cevap vermeyen bir terminal `WT_SESSION`'dan (Windows Terminal), `TERM_PROGRAM` ve `TERM_PROGRAM_VERSION`'dan ya da `LC_TERMINAL` ve `LC_TERMINAL_VERSION`'dan anlaşılır; bkz. [ekip](../screen/salon.md) |
| `TMUX` | tmux içinde: bildirimler passthrough'tan geçer (`allow-passthrough on`), sekme ilerlemesi doğrudan |
| `XDG_STATE_HOME` | durum dizini `imece/`'nin (depo, worktree'ler) gittiği yer; mutlak olmalı; yoksa `~/.local/state/imece`, `HOME` da yoksa sistemin geçici dizini |
| `HOME` | yukarıdaki yedek, `:open`, `:export` ya da `--theme` sonrası yazılan bir yoldaki `~`, ve Claude Code'un varsayılan dizini |
| `COLORTERM` | `truecolor` ya da `24bit` imece'ye RGB'yi olduğu gibi çizdirir; yoksa şemanın renkleri 256'nın en yakınına getirilir |
| `COLORFGBG` | terminale sorulmadan önce koyu mu açık mı olduğuna ilk tahmin |
| `TERM` | `dumb` (ya da tty yokluğu) terminale renklerinin ve kitty klavye protokolünün sorulmasını atlar |
| `SHELL` | `!cmd`'nin içinde çalıştığı; yoksa `/bin/sh` |
| `TMPDIR` | veriyolunun soket dizini `imece-<pid>-<hex>/`'in yapıldığı yer (0700, çıkışta kaldırılır) |
| `WAYLAND_DISPLAY`, `DISPLAY` | Linux'ta: `c` `C` `y` `Y` kopyaları için `wl-copy`, sonra `xclip`/`xsel` denensin mi (macOS `/usr/bin/pbcopy` kullanır); hiçbir araç metni almazsa terminalin OSC 52 kaçışı alır |
| `CLAUDE_CONFIG_DIR` | Claude Code'un oturumlarını tuttuğu yer; imece sürdürecek ya da silecek transkripti orada bulur ([dosyalar](files.md)) |

Çocuklara geçirilen: Claude Code, `cp`, `git` ve `run` aracının programı temizlenmiş bir ortamla başlar. Ayarlıysa `HOME`, `USER`, `LOGNAME`, `PATH`, `SHELL`, `TMPDIR`, `LANG`, `LC_ALL`, `LC_CTYPE` alırlar; ayrıca Claude Code'un oturum açmak ve ağa erişmek için gerekenleri: `ANTHROPIC_API_KEY`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`, vekil değişkenleri (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` ve küçük harfli biçimleri), `NODE_EXTRA_CA_CERTS` ve `SSL_CERT_FILE`. Bir `run` ayrıca `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS` alır ve ona `NO_COLOR=1`, `CARGO_TERM_COLOR=never`, `TERM=dumb` verilir. Ortamınızdan başka hiçbir şey bir ajana ulaşmaz.

## Ayrıca bkz.

- [Komutlar](commands.md): aynı şeylerin ekrandaki adları
- [Dosyalar](files.md): bayrakların ve değişkenlerin gösterdiği durum dizini
- [Kurulum](../getting-started/install.md) ve [ilk çalıştırma](../getting-started/first-run.md)
- [Claude Code](../concepts/claude-code.md), [bütçe ve sınırlar](../concepts/budget-and-limits.md), [güvenlik](../concepts/security.md)
