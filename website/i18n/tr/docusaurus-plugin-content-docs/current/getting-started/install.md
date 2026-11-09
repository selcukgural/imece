---
title: Kurulum
description: imece'yi tek satırla kurun (ya da kaynaktan derleyin), Claude Code'u PATH'e koyun ve durumunu nerede tuttuğunu bilin.
---

# Kurulum

imece macOS ve Linux'ta, x86_64 ve arm64 üzerinde çalışır (Windows'ta: WSL içinde). Git (bir solo, checkout olmayan bir klasörde git'siz de çalışır) ve PATH'te oturum açılmış Claude Code gerekir. Bu sayfa, [ilk çalıştırma](first-run.md) öncesinde yerinde olması gerekenlerdir.

## Kurulum betiğiyle

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | sh
```

Betik sisteminiz için önceden derlenmiş imece'yi [sürümlerden](https://github.com/selcukgural/imece/releases) indirir (Linux derlemeleri statiktir, her dağıtımda çalışır) ve sürümün `SHA256SUMS` dosyasıyla karşılaştırır: uyuşmazsa hiçbir şey kurulmaz. `imece`'yi `~/.local/bin`'e koyar; orası henüz PATH'te değilse kabuğunuzun başlangıç dosyasına bir kez, bir `# imece` satırının altına ekler: `~/.zshrc`, `~/.bashrc` (macOS'ta `~/.bash_profile`), fish'in `conf.d/imece.fish` dosyası, yoksa `~/.profile`. Yeni bir terminal açın ya da yazdığı satırı çalıştırın; `imece --version` cevap verir. Terminal program başına bir ikon gösteriyorsa imece'nin sekmesine de işaretini verir (bkz. [terminaller](#terminaller)). Linux'ta işareti ikon temanıza koyar (`~/.local/share/icons/hicolor/scalable/apps/imece.svg`), imece de çalışırken onu Konsole'a sekmesi için adıyla söyler. macOS'ta iTerm2 varsa bunu iTerm2'nin Claude Code'unkini gösterdiği gibi yapar: iTerm2 bir sekmedeki programın ikonunu kendi program tablosundan çizer, imece onun getirdiği tabloda olmadığı için sekmesinde daire içinde bir harf görünürdü. Betik işareti (`graphic_imece.png`) ve bir `imece` girdisini iTerm2'nin kendi tablolarına (`~/Library/Application Support/iTerm2` içindeki `graphic_icons.json`, `graphic_colors.json`) koyar; iTerm2'nin bir sonraki açılışından itibaren görünür. Sekme ikonu *Settings → Profiles → General → Icon* *No icon* değilken çizilir. Bu tablolardaki sizin girdileriniz olduğu gibi kalır; imece'yi zaten anan bir tabloya dokunulmaz, JSON nesnesi olmayan (ya da bir bağlantı olan) bir tablo bırakılır ve söylenir. Sonunda PATH'te hangi kodlama ajanlarını bulduğunu söyler. Yeniden çalıştırmak imece'yi günceller; çalışan bir imece bitene kadar kendi dosyasını tutar.

| Değişken | Etkisi |
| --- | --- |
| `IMECE_VERSION=0.1.0` | en yenisi yerine o sürüm |
| `IMECE_INSTALL_DIR=DIZIN` | imece'nin gideceği yer (varsayılan `~/.local/bin`) |
| `IMECE_NO_MODIFY_PATH=1` | hiçbir başlangıç dosyasına dokunulmaz |
| `IMECE_NO_ICON=1` | imece'nin sekmesine ikon konmaz (iTerm2'nin tablolarına ve ikon temasına dokunulmaz) |
| `IMECE_DOWNLOAD_URL=URL` | sürüm dosyalarının yeri, bir yansı için: bir `https://` adresi, bir `file://` dizini ya da bir dizinin mutlak yolu; başka bir şey reddedilir |

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | IMECE_VERSION=0.1.0 sh
```

Betiği çalışmadan önce okumak için: `curl -fsSL https://selcukgural.github.io/imece/install.sh -o install.sh`, okuyun, sonra `sh install.sh`. Her arşiv ayrıca bir derleme kökeni onayı (build provenance attestation) taşır; `gh attestation verify imece-aarch64-apple-darwin.tar.gz --repo selcukgural/imece` onu bu deponun sürüm iş akışının kendi etiketinden derlediğini denetler.

Kaldırmak için: `rm ~/.local/bin/imece` ve başlangıç dosyasındaki `# imece` altındaki satırlar; sekme ikonu için macOS'ta `graphic_imece.png` ve iTerm2'nin iki tablosundaki `imece` girdileri, Linux'ta `~/.local/share/icons/hicolor/scalable/apps/imece.svg`. Durum dizini (aşağıda) siz silene kadar kalır.

## Homebrew ile

```sh
brew tap selcukgural/imece
brew trust --formula selcukgural/imece/imece
brew install imece
```

Ortadaki satır Homebrew 7 ve sonrası içindir: Homebrew, kendisine ait olmayan bir tap'in formülünü ancak siz ona güvendiğinizi söyledikten sonra yükler; bu satır olmadan `brew install`, `Refusing to load formula selcukgural/imece/imece from untrusted tap` diyerek durur. Daha eski bir Homebrew'da `brew trust` yoktur ve gerekmez: satırı atlayın.

[Tap](https://github.com/selcukgural/homebrew-imece), kurulum betiğinin kurduğu aynı önceden derlenmiş imece'yi kurar: macOS (arm64, x86_64) ve Linux (x86_64, arm64). Formülü son sürüm için bir iş akışı yazar: her arşiv sürümün `SHA256SUMS` dosyasına ve derleme kaynağı kanıtına (attestation) karşı denetlenir; formül commit edilmeden önce macOS ve Linux'ta kurulup sınanır. `brew upgrade imece` günceller, `brew uninstall imece` kaldırır. Homebrew sekme ikonunu ayarlamaz; onu kurulum betiği yapar.

## crates.io'dan

```sh
cargo install --locked imece
```

imece'yi [crate](https://crates.io/crates/imece)'inden kendi Rust'ınızla (1.99 ya da daha yenisi) derler ve `~/.cargo/bin`'e koyar. `--locked`, sürümün sınandığı bağımlılık sürümleriyle derler. Aynı komut yeniden çalıştırılınca günceller; `cargo uninstall imece` kaldırır.

Hangi yolla gelmiş olursa olsun `imece --version` hangisinin çalıştığını, [`imece doctor`](doctor.md) bir açılışın başka neye ihtiyaç duyduğunu söyler. Tek bir yolla kurun: PATH'teki iki kopya er geç iki ayrı sürüm olur.

## Kaynaktan

Depo derleyicisini `rust-toolchain.toml` dosyasında sabitler (kanal `1.99.1`, `clippy` ve `rustfmt` ile). [rustup](https://rustup.rs) kuruluysa `cargo` ilk derlemede o araç zincirini kendisi alır; yeni bir derleyici yalnız o dosyadaki bir değişiklikle gelir.

```sh
git clone https://github.com/selcukgural/imece.git
cd imece
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --locked --path crates/imece-tui   # crate'in ve ikili dosyanın adı imece
```

Ya da kurmadan, checkout'tan çalıştırın:

```sh
cargo run -p imece -- --help
cargo run -p imece -- --demo
```

Bu yolla `~/.cargo/bin`'e konan bir imece ile kurulum betiğinin `~/.local/bin`'e koyduğu iki ayrı kopyadır; diğeri PATH'te önce geliyorsa betik bunu söyler.

Crate'in adı da ikili dosyası gibi `imece` (`crates/imece-tui` içinde). SQLite derlemeye gömülür, depo için sistem kütüphanesi gerekmez. Git PATH'te olmalı: imece değişiklikleri git'in kendisiyle okur ve uygular; üç yollu uygulama git 2.56 üzerinde sondalandı.

## Claude Code

imece başlarken PATH'te `claude`'u arar ve onu sürer. Yoksa durur ve Claude Code'u kurup (https://claude.com/claude-code) oturum açmanızı söyler.

Ajan başına uzun ömürlü bir `claude -p` stream-json oturumu çalıştırır; `--confine open` değilse `--restricted` altında. Oturum açılmış olmalıdır; maliyetler dolar olarak bildirilir ve keseye yazılır. Claude Code 2.1.314 üzerinde sondalandı.

Varsayılan `--confine standard` ve `strict` altında her oturum, Claude Code kurulumunuz yüklenmeyecek şekilde başlatılır (hook yok, imece'ninki dışında MCP sunucusu yok, ayar yok, sizin komutlarınız yok), yalnızca aşağıdaki notlar yüklenir; `--confine open` Claude Code'u sizin kurduğunuz gibi çalıştırır. Her durumda lider hiç yazmaz ve Claude Code'un kendi alt ajan aracı kapalı kalır. Bir oturumun neyle çalıştığını bildirdiği, kapalı düşerek denetlenir; bkz. [Claude Code](../concepts/claude-code.md) ve [güvenlik](../concepts/security.md).

Her ajan deponun `CLAUDE.md`'sini ve `~/.claude/CLAUDE.md`'nizi, Claude Code'un kendi yükleyicisiyle, tümüyle okur; yalnızca `AGENTS.md`'si olan bir depoda onun yerine o okunur. Yalnızca imece'nin ajanlarına yönelik talimatlar `~/.config/imece/instructions.md` dosyasına (ya da `$XDG_CONFIG_HOME/imece/` altına) gider. Bkz. [dosyalar](../reference/files.md).

## Terminaller

imece alternatif ekranda çizer ve tuşları crossterm ile okur; her terminal çalışır. Birkaç şey terminale bağlıdır:

- **Yazarken yeni satır için `⇧enter`**, imece'nin başta istediği kitty klavye protokolünü gerektirir. Konuşan terminaller: kitty, Ghostty, WezTerm (`enable_kitty_keyboard` ile), foot, Alacritty, iTerm2. Başka yerde `⇧enter` düz `enter`dır ve gönderir; yeni satır için `\` sonra `enter` yazın (her yerde çalışır) ya da Option'ın Meta olduğu yerde `⌥enter`. `?` yardımı ve alt satır terminalinizde hangisinin çalıştığını söyler. Sorguyu yanıtlamayan bir terminal çoğullayıcısı (tmux, screen), çevresindeki terminal daha fazlasını yapabilse de `\` `enter` yoluna düşer.
- **macOS'ta alt tuşları** (`⌥b` `⌥f` `⌥d` `⌥backspace` `⌥enter`) terminalin Option'ı Meta olarak göndermesini gerektirir: iTerm2 → Profiles → Keys → Left Option key: *Esc+*; Terminal.app → *Use Option as Meta key*. Yoksa `⌥b` `∫` yazar.
- **`terminal` teması** (varsayılan) terminalinizin kendi 16 rengini kullanır ve ekran başlamadan önce koyu mu açık mı olduğunu bilmek için arka ve ön planını sorar (OSC 11 / OSC 10); iTerm2, Ghostty, kitty, WezTerm, Alacritty, foot ve tmux yanıtlar. Yanıtlamayan bir terminal koyu sayılır ya da `COLORFGBG` ne derse. `COLORTERM=truecolor` (ya da `24bit`) yoksa bir şemanın RGB renkleri 256 renk küpüne yuvarlanır. Tema, pencere yeniden odaklanınca terminalinizin açığa ya da koyuya dönmesini izler; tmux altında bunun için `focus-events on` gerekir. Bkz. [temalar ve dil](../screen/themes-and-language.md).
- **Sekmenin ikonu.** Bir program terminale sekmesi için bir resim gönderemez; terminal program başına bir ikon gösteriyorsa imece'nin işareti oradadır: **iTerm2** onu kurulum betiğinin doldurduğu kendi tablosundan alır (iTerm2'nin bir sonraki açılışından itibaren, *Settings → Profiles → General → Icon* *No icon* değilken); **Konsole**'a (ve Yakuake'ye) imece çalışırken ikonun adı söylenir (`OSC 32`), sonra varsayılan profilin ikonu geri verilir (kendi ikonu olan bir profilinki sorulamaz, o da varsayılanı geri alır). **Windows Terminal** bir sekmenin ikonunu yalnızca açıldığı profilden alır. Terminal.app, Ghostty, kitty, WezTerm, GNOME Terminal, Ptyxis, Tilix, xfce4-terminal, Alacritty ve foot program başına ikon göstermez. Her yerde, her şey sakinken başlık imece'nin işareti `⁙` ile başlar. Konsole ikonuna `IMECE_TAB_ICON=on` ya da `off` ile kendiniz karar verirsiniz; tmux içinde hiç gönderilmez.
- **Başlık** (`imece · 2 karar bekliyor`) xterm'in başlık yığınını kullanır; xterm, iTerm2, kitty, WezTerm ve Ghostty imece bitince kabuğunuzun başlığını geri verir; yığını olmayan bir terminal kendi varsayılanını gösterir.

Fare hiç gerekmez ve hiç yakalanmaz; terminalinizin kendi metin seçimi çalışmayı sürdürür.

## Durum nerede durur

imece'nin hatırladığı her şey yalnız sizindir (dosyalar `0600` yapılır) ve tek bir dizin altında durur: `$XDG_STATE_HOME/imece`, ya da `XDG_STATE_HOME` yoksa `~/.local/state/imece` (mutlak bir yol gerekir).

| Yol | Nedir |
| --- | --- |
| `<state>/imece.sqlite` | depo: projeler, ajanlar, kartlar ve hikâyeleri, günlük, gelen kutusu, posta, pano, kese, ekran tercihleriniz (tema, notify, motion). WAL kipinde; açıkken `imece.sqlite-wal` ve `imece.sqlite-shm` yanında durur |
| `<state>/imece.sqlite.lock` | bir imece çalışırken `flock` ile tutulur; aynı dosyada ikinci bir imece reddedilir ve ilkinden `:open` kullanması söylenir |
| `<state>/worktrees/<dizin>-<özet>/<ajan>` | işçi başına bir git worktree (`claude-1`, `claude-2`, …), HEAD'inizde ayrılmış, proje başına bir dizin altında (checkout'un dizin adı ve yolunun 8 haneli özeti). Çalıştırmalar arasında saklanır. Her birinin kendi `target/` dizini var |

Bu dizinin dışında Claude Code her zaman tuttuğunu tutar: transkriptlerini `~/.claude/projects/` altında. `--fresh` ve `:forget` imece'nin unuttuğu oturumlar için bunları kaldırır; bkz. [oturumlar](../concepts/sessions.md). imece'nin MCP soketleri, ajan başına bir tane, çalıştırma süresince sistemin geçici dizini altında özel bir dizinde durur.

Checkout'unuza, stage'e uyguladığınız değişiklikler ve üç yollu uygulama denetlenirken `.git` altında özel bir index dosyası dışında hiçbir şey girmez.

## `--awake`

`--awake` (ya da imece çalışırken `:awake`) bir ajan turdayken makinenin uyumasını engeller. Sistemin kendi aracını imece'nin alt süreci olarak çalıştırır: macOS'ta `caffeinate -i -w PID` (işletim sisteminin parçası), Linux'ta `systemd-inhibit --what=idle:sleep --mode=block sleep infinity` (systemd gerekir). Başka yerde çalıştıracak bir şey yok. Varsayılan olarak kapalıdır ve makinenin güç davranışını değiştirdiği için çalıştırmalar arasında hatırlanmaz. Başlamayan bir program flash'ta bir kez söylenir (`makine uyanık tutulamadı: …`) ve o çalıştırmada bir daha denenmez.

## Ortam değişkenleri

| Değişken | Etkisi |
| --- | --- |
| `IMECE_LANG=en\|tr` | ekranın dili (`--lang` üstündür; `L` çalışırken değiştirir) |
| `IMECE_THEME=AD\|DOSYA` | tema (`--theme` üstündür; son seçim depoda hatırlanır) |
| `XDG_STATE_HOME` | durum dizininin yeri |
| `COLORTERM` | RGB renkler için `truecolor` ya da `24bit` |

## Ayrıca bakınız

- [İlk çalıştırma](first-run.md)
- [Demo](demo.md)
- [Komut satırı başvurusu](../reference/cli.md)
- [Dosyalar](../reference/files.md)
- [Sorun giderme](../troubleshooting.md)
