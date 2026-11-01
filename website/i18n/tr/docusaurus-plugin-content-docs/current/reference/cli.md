---
title: Komut satırı
description: imece'nin her bayrağı, varsayılanı ve aralığıyla; satıcı öneki kuralı, okunan ortam değişkenleri, yanlış bir bayrağın söylediği ve çalıştırmalar arasında hatırlananlar.
---

# Komut satırı

`imece` bir avuç bayrak alır: çalışma kopyası, takımın üst sınırları, kimin lider olacağı, modeller, bütçe ve ekran için birkaç anahtar. Onlarla bir kez, başlangıçta karşılaşırsınız; ayarladıklarının çoğu ajanlar çalışırken ekrandan bir `:` komutuyla da değiştirilebilir ([komutlar](commands.md)).

## Özet

```text
usage: imece [--repo DIR] [--agents N] [--helpers N] [--lead VENDOR]
             [--solo V[:MODEL][@EFFORT]] [--model [V:]NAME]... [--tier [V:]T=NAME]...
             [--budget USD] [--fresh] [--awake] [--confine standard|strict|open]
             [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]
       imece --version
```

```sh
imece                                              # içinde olduğunuz çalışma kopyası, lider tek başına
imece --agents 8 --budget 10 --model sonnet
imece --lead codex                                 # Codex lider; yine de claude işçileri alabilir
imece --lead gemini --tier gemini:strong=gemini-2.5-pro
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
- `imece mcp-relay SOCKET` sizin için değildir: bir satıcının CLI'ı imece'yi bu biçimde kendi MCP sunucusu olarak başlatır ve süreç yalnızca CLI ile imece'nin o ajana ait soketi arasında bayt taşır ([mesaj türleri](message-types.md)).

### `[V:]` satıcı öneki

`--model` ve `--tier` değerlerinin önüne iki noktayla ayrılmış isteğe bağlı bir satıcı alır: `--model codex:gpt-5.6-terra`, `--tier gemini:strong=gemini-2.5-pro`. Bir model adında iki nokta olmaz, bu yüzden iki nokta her zaman bir satıcı demektir. Öneksiz ikisi de liderin satıcısından söz eder. Satıcı imece'nin sürdüğü biri olmalıdır (`claude`, `codex`, `gemini`); başkası reddedilir: `--tier: "cursor" is not a vendor imece drives (claude, codex, gemini)`.

## Çıkış kodları

| Kod | Ne zaman | Ne görürsünüz |
| --- | --- | --- |
| 0 | `--version`, ya da olağan bir çıkış (`q q`, `:quit`, ya da dışarıdan SIGTERM, SIGHUP ve SIGINT; imece bunları olağan bir çıkışa çevirir ki oturumlar dursun ve veriyolunun soketleri kaldırılsın) | kabuğunuzun başlığı geri gelir |
| 1 | Bayraklar okunduktan sonra başlangıç başarısız oldu: PATH'te kodlama ajanı yok, `--repo` bir git çalışma kopyası değil, depoyu başka bir imece tutuyor, depo okunamıyor, bir satıcının oturumu başlatılamadı | ekran geri getirildikten sonra stderr'de `imece: <neden>` |
| 2 | Bir bayrak yanlış, ya da `--help` | stderr'de mesaj; eksik değerde, bilinmeyen seçenekte ve canlıda verilen demo bayrağında ardından kullanım metni |

Çıkış kodu 1'in bazı mesajları, aynen: `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code (https://claude.com/claude-code) and log in`; `codex is not on PATH (found claude); install it, or pick the leader with --lead`; `another imece is running on <yol>/imece.sqlite; open the other checkout from it with :open`.

## Çalıştırmalar arasında hatırlanan ve hatırlanmayan

| Hatırlanır (depoda, [dosyalar](files.md)) | Çalıştırma başına |
| --- | --- |
| tema (`--theme`, `T`, `:theme`; bir bayrak ya da `IMECE_THEME` kayıtlı olanı geçer) | dil (`--lang`, `IMECE_LANG`, yoksa İngilizce; `L` bu çalıştırma için değiştirir) |
| bütçe (`--budget` ayarlar; bayraksız bırakıldığı gibi kalır; ilk seferde 50 $) | `--agents`, `--helpers`, `--confine`, `--run`, `--run-timeout` |
| birincil satıcı (soru ekranında `p`, `:lead`) | `--model` ve `--tier` |
| yan panelin genişliği (`,` `.` `<` `>`), başka penceredeyken nasıl haber alacağınız (`:notify`) | `--awake` (bilerek: makinenin güç davranışını değiştirir) |
| projeler, ajanlar, kartlar, günlük, posta, pano, sabitlemeler, işaretler, kopya listesi, her ajanın satıcı oturumu | başlıktaki satıcı kullanım pencereleri (bir oturum ilk turunda yeniden söyler) |

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

### `--lead VENDOR`

```sh
imece --lead codex      # claude, codex, gemini ya da ask
```

Liderin kodlama ajanı. Onsuz: birincil satıcınız (soru ekranında `p` ile, ya da `:lead VENDOR` ile ayarlanır) PATH'teyse ve imece onu sürüyorsa o; yoksa PATH'teki tek böyle satıcı; o da yoksa salondan önce KİM LİDER? sorusu. `--lead ask` soruyu zorlar. Burada adı geçen ama PATH'te olmayan bir satıcı, sessizce değiştirilmez: takım kurulurken dürüst hatayı verir. Başka bir şey: `--lead: "cursor" is not claude, codex, gemini or ask`. Bkz. [satıcılar](../concepts/vendors.md).

### `--solo V[:MODEL][@EFFORT]`

```sh
imece --solo claude                    # Claude Code, kendi modeli ve çabası
imece --solo claude:opus@xhigh         # opus, xhigh çabayla düşünen
imece --solo codex:gpt-5.6-terra@high
```

Checkout'ta bir takım yerine bir solo oturumla başlar: kendinizin sürdüğü tek bir ajan, lidersiz; `V` satıcısında (`claude`, `codex`, `gemini`), `MODEL` modelinde (varsayılan: satıcının kendisi ya da `--model`'in adlandırdığı) ve `EFFORT` çabasında (satıcının kendi düzeyleri, bkz. `--model`; Gemini'de yoktur). Proje dizinin adını alır, ajan `solo`dur (başka yerde `solo-AD`); kendi worktree'sinde yazar ve bir yanıtla biten her turda diff'i size gösterir. Bu checkout'ta aynı satıcı ve seçimle açık bir solo varsa onunla sürülür, böylece her sabah aynı komut aynı oturumu sürdürür; farklı bir seçim yanına `-2` adlı başka bir solo açar; burada açık bir takım, siz ona yazana dek uykuda kalır. `--lead` yoksa `:open`'ın eklediği takımlara `V` liderlik eder, KİM LİDER? sorusu gelmez. Redler: `--solo: "x" is not VENDOR[:MODEL][@EFFORT], like claude:opus@xhigh`, `--solo: imece does not drive qwen`, `--solo: claude has no effort "bogus"; …` ve başlangıçta `--solo: codex is not on PATH here, or imece does not drive it` (çıkış kodu 1). Demo da alır. Bkz. [solo oturumlar](../concepts/solo.md).

### `--model [V:]NAME`

```sh
imece --model sonnet
imece --model opus@xhigh                 # Claude'un opus'u, xhigh düşünce çabasıyla
imece --model claude:@max                # Claude'un varsayılan modeli en derin çabada
imece --model codex:gpt-5.6-terra@high
```

`V` satıcısının ajanlarının çalıştığı model; `V` yoksa liderin satıcısı. Bu `standard` kademesidir: lider onda çalışır, standart kademede işe alınan her işçi de. Varsayılan: satıcının kendi varsayılanı (Claude Code, Codex ya da Gemini CLI'da ayarladığınız). `NAME` `[MODEL][@ÇABA]` biçimindedir, iki parça da isteğe bağlı: satıcının aldığı biçimde bir model adı (`sonnet`, `claude-opus-5-5`, `opus[1m]`, `gpt-5.6-terra`), `@` sonrasında bir çaba, ya da ikisi. Çabalar satıcının kendi çabalarıdır: Claude Code'un `low`, `medium`, `high`, `xhigh`, `max` (kendi `--effort`'u); Codex'in `low`, `medium`, `high`, `xhigh` (her turun akıl yürütme çabası); Gemini CLI'da yoktur. Bütünü `-` ile başlamayan 1-64 karakter harf, rakam ve `-._[]@`'dir; başkası: `--model: "x y" is not a model name`. Satıcının olmayan bir çaba, satıcı adlandırılmışsa komut satırında reddedilir (`--model: claude has no effort "bogus"; one of low, medium, high, xhigh, max`, `--model: gemini has no effort setting; name only a model, not @high`); öneksiz bir değer için takım kurulup liderin satıcısı bilinince (`--model/--tier "opus@bogus": …`, çıkış kodu 1); Claude Code kendisi yalnız uyarıp varsayılan çabasıyla çalışırdı, bu yüzden imece önce denetler. Satıcı başına bir kez yinelenebilir.

### `--tier [V:]T=NAME`

```sh
imece --tier strong=opus[1m]
imece --tier codex:strong=gpt-5.6-terra@xhigh
imece --tier gemini:cheap=gemini-2.5-flash-lite
```

Bir kademenin karşılığı; `T` `cheap`, `standard` ya da `strong`. Lider kademeyle işe alır, modelle değil; her satıcının kendi basamakları vardır (Claude: `haiku`, varsayılan, `opus`; Codex: varsayılan modelinde bir çaba, `@low`, varsayılan, `@high`; Gemini: `flash`, varsayılan `auto`, `pro`). Bu bayrak bir basamağı değiştirir; `NAME` `--model`'deki gibi `[MODEL][@ÇABA]`dır, yani `--tier strong=opus@xhigh` güçlü kademeyi Claude'un xhigh çabayla düşünen opus'u yapar. Redler: `--tier: "strong" is not TIER=MODEL, like strong=opus`, `--tier: "huge" is not cheap, standard or strong`, `--tier: "…" is not a model name` ve `--model`'in çaba redleri. Hesabınızda olmayan bir kademe modeli ilk turunda başarısız olur; imece o işçiyi standart modelle yeniden başlatır ve günlük bunu söyler ([takım](../concepts/team.md)).

### `--budget USD`

```sh
imece --budget 10       # dolar, en çok iki ondalık, en az 0.01
```

Tüm ajanların birlikte harcayabileceği en çok: bütün takım için tek bir bütçe, çalıştırmalar arasında saklanır. Bayrak onu ayarlar; bayraksız bütçe son çalıştırmanın bıraktığı gibi kalır, ilk seferde 50 $. Noktadan önce en çok dört, sonra iki rakam; `--budget 0` ve sayı olmayan her şey reddedilir: `--budget: abc is not dollars like 5 or 2.50`. `:budget USD` ajanlar çalışırken değiştirir. Bkz. [bütçe ve sınırlar](../concepts/budget-and-limits.md).

### `--fresh`

```sh
imece --fresh
```

Depo alınırken her ajanın hatırlanan satıcı oturumunu unutturur: her ajan yeni bir oturumda uyanır, kaldığı yer anlatılır, ve satıcıların eski oturumlardan tuttuğu silinir (Claude'un transkripti, Codex'in iş parçacığı, Gemini'nin sohbet dosyaları; bkz. [dosyalar](files.md)). Günlük, gitmemiş her ajan için `X oturumunu unutturdun; yeni bir oturumla başlıyor, kaldığı yer anlatılacak` der. `:forget @NAME` aynısını imece çalışırken tek bir ajan için yapar. Yeni oturum bütün brifingi, önbelleksiz, yeniden öder.

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

Her ajanın oturumunun satıcısından ne kadarını aldığı:

| Düzey | İşçiler ve yardımcılar | Lider | Kendi satıcı kurulumunuz |
| --- | --- | --- | --- |
| `standard` (varsayılan) | satıcının tam kodlama araç takımı: okuma, yazma, kabuk, web arama ve getirme | okuma ve web | yüklenmez |
| `strict` | yalnızca dosya araçları; komutlar imece'nin `run` aracıyla | okuma | yüklenmez |
| `open` | satıcı sizin kurduğunuz gibi, alt-ajan aracı eksik | okuma, sizin kurulumunuz | yüklenir: ayarlar, hook'lar, MCP sunucuları, skill'ler, uzantılar |

Her düzeyde lider asla yazmaz (sizin çalışma kopyanızda çalışır; orada bir yazma Δ incelemesini atlardı) ve satıcının kendi alt-ajan aracı kapalı kalır. imece hiçbir şeyi sandbox'lamaz: düzey için her satıcının kendi kısıtlamalarını açar ve oturumun onlarla çalıştığını, kapalıya düşerek, denetler; yalıtım bir konteynerin işidir. Günlük strict olmayan her başlangıçta bunu söyler. Başka bir şey: `--confine: "loose" is not standard, strict or open`. Bkz. [güvenlik](../concepts/security.md) ve [onaylar ve sorular](../concepts/approvals-and-questions.md).

### `--run PROG[,..]`

```sh
imece --run npm,make
imece --run npm --run pnpm       # yinelenebilir; yinelenenler düşer
```

İşçilerin ve yardımcıların `run` araçlarıyla, `cargo` ve git'in okuma komutlarının yanında çalıştırabileceği programlar. Yalnızca PATH'teki adlar; bir yol reddedilir: `--run: "./bin/x" is a path; name a program on PATH`. Lider bunları hiç almaz. `standard` ve `open` altında aynı liste bir Claude, Codex ya da Gemini oturumunun size sormadan hangi kabuk komutlarını çalıştırabileceğine de karar verir (rutin kuralı, [onaylar](../concepts/approvals-and-questions.md)).

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
| `XDG_STATE_HOME` | durum dizini `imece/`'nin (depo, worktree'ler) gittiği yer; mutlak olmalı; yoksa `~/.local/state/imece`, `HOME` da yoksa sistemin geçici dizini |
| `HOME` | yukarıdaki yedek, `:open`, `:export` ya da `--theme` sonrası yazılan bir yoldaki `~`, ve satıcıların varsayılan dizinleri |
| `COLORTERM` | `truecolor` ya da `24bit` imece'ye RGB'yi olduğu gibi çizdirir; yoksa şemanın renkleri 256'nın en yakınına getirilir |
| `COLORFGBG` | terminale sorulmadan önce koyu mu açık mı olduğuna ilk tahmin |
| `TERM` | `dumb` (ya da tty yokluğu) terminale renklerinin ve kitty klavye protokolünün sorulmasını atlar |
| `SHELL` | `!cmd`'nin içinde çalıştığı; yoksa `/bin/sh` |
| `TMPDIR` | veriyolunun soket dizini `imece-<pid>-<hex>/`'in yapıldığı yer (0700, çıkışta kaldırılır) |
| `WAYLAND_DISPLAY`, `DISPLAY` | Linux'ta: `c` `C` `y` `Y` kopyaları için `wl-copy`, sonra `xclip`/`xsel` denensin mi (macOS `/usr/bin/pbcopy` kullanır); hiçbir araç metni almazsa terminalin OSC 52 kaçışı alır |
| `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME` | satıcıların oturumlarını tuttuğu yer; imece sürdürecek ya da silecek transkripti orada bulur ([dosyalar](files.md)) |

Çocuklara geçirilen: her satıcı CLI'ı, `cp`, `git` ve `run` aracının programı temizlenmiş bir ortamla başlar. Ayarlıysa `HOME`, `USER`, `LOGNAME`, `PATH`, `SHELL`, `TMPDIR`, `LANG`, `LC_ALL`, `LC_CTYPE` alırlar; ayrıca satıcının oturum açmak ve ağa erişmek için gerekenleri: Claude `ANTHROPIC_API_KEY`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`; Codex `CODEX_HOME`, `OPENAI_API_KEY`, `CODEX_API_KEY`, `SSL_CERT_DIR`; Gemini `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_LOCATION`, `GEMINI_CLI_HOME`; üçü de vekil değişkenleri (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` ve küçük harfli biçimleri), `NODE_EXTRA_CA_CERTS` (Claude, Gemini) ve `SSL_CERT_FILE`. Bir `run` ayrıca `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS` alır ve ona `NO_COLOR=1`, `CARGO_TERM_COLOR=never`, `TERM=dumb` verilir. Gemini oturumlarına `GEMINI_CLI_TRUST_WORKSPACE=true`, `GEMINI_TELEMETRY_ENABLED=false`, `NO_BROWSER=true` verilir. Ortamınızdan başka hiçbir şey bir ajana ulaşmaz.

## Ayrıca bkz.

- [Komutlar](commands.md): aynı şeylerin ekrandaki adları
- [Dosyalar](files.md): bayrakların ve değişkenlerin gösterdiği durum dizini
- [Kurulum](../getting-started/install.md) ve [ilk çalıştırma](../getting-started/first-run.md)
- [Satıcılar](../concepts/vendors.md), [bütçe ve sınırlar](../concepts/budget-and-limits.md), [güvenlik](../concepts/security.md)
