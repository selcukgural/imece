---
title: Kurulum
description: imece'yi tek satırla kurun (ya da kaynaktan derleyin), satıcı CLI'larını PATH'e koyun ve durumunu nerede tuttuğunu bilin.
---

# Kurulum

imece macOS ve Linux'ta, x86_64 ve arm64 üzerinde çalışır (Windows'ta: WSL içinde). Git (bir solo, checkout olmayan bir klasörde git'siz de çalışır) ve PATH'te en az bir satıcının oturum açılmış komut satırı ajanı gerekir. Bu sayfa, [ilk çalıştırma](first-run.md) öncesinde yerinde olması gerekenlerdir.

## Kurulum betiğiyle

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | sh
```

Betik sisteminiz için önceden derlenmiş imece'yi [sürümlerden](https://github.com/selcukgural/imece/releases) indirir (Linux derlemeleri statiktir, her dağıtımda çalışır) ve sürümün `SHA256SUMS` dosyasıyla karşılaştırır: uyuşmazsa hiçbir şey kurulmaz. `imece`'yi `~/.local/bin`'e koyar; orası henüz PATH'te değilse kabuğunuzun başlangıç dosyasına bir kez, bir `# imece` satırının altına ekler: `~/.zshrc`, `~/.bashrc` (macOS'ta `~/.bash_profile`), fish'in `conf.d/imece.fish` dosyası, yoksa `~/.profile`. Yeni bir terminal açın ya da yazdığı satırı çalıştırın; `imece --version` cevap verir. Sonunda PATH'te hangi kodlama ajanlarını bulduğunu söyler. Yeniden çalıştırmak imece'yi günceller; çalışan bir imece bitene kadar kendi dosyasını tutar.

| Değişken | Etkisi |
| --- | --- |
| `IMECE_VERSION=0.1.0` | en yenisi yerine o sürüm |
| `IMECE_INSTALL_DIR=DIZIN` | imece'nin gideceği yer (varsayılan `~/.local/bin`) |
| `IMECE_NO_MODIFY_PATH=1` | hiçbir başlangıç dosyasına dokunulmaz |
| `IMECE_DOWNLOAD_URL=URL` | sürüm dosyalarının yeri, bir yansı için |

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | IMECE_VERSION=0.1.0 sh
```

Betiği çalışmadan önce okumak için: `curl -fsSL https://selcukgural.github.io/imece/install.sh -o install.sh`, okuyun, sonra `sh install.sh`. Her arşiv ayrıca bir derleme kökeni onayı (build provenance attestation) taşır; `gh attestation verify imece-aarch64-apple-darwin.tar.gz --repo selcukgural/imece` onu bu deponun sürüm iş akışının kendi etiketinden derlediğini denetler.

Kaldırmak için: `rm ~/.local/bin/imece` ve başlangıç dosyasındaki `# imece` altındaki satırlar. Durum dizini (aşağıda) siz silene kadar kalır.

## Kaynaktan

Depo derleyicisini `rust-toolchain.toml` dosyasında sabitler (kanal `1.99.1`, `clippy` ve `rustfmt` ile). [rustup](https://rustup.rs) kuruluysa `cargo` ilk derlemede o araç zincirini kendisi alır; yeni bir derleyici yalnız o dosyadaki bir değişiklikle gelir.

```sh
git clone https://github.com/selcukgural/imece.git
cd imece
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --path crates/imece-tui      # ikili dosyanın adı imece
```

Ya da kurmadan, checkout'tan çalıştırın:

```sh
cargo run -p imece-tui -- --help
cargo run -p imece-tui -- --demo
```

Bu yolla `~/.cargo/bin`'e konan bir imece ile kurulum betiğinin `~/.local/bin`'e koyduğu iki ayrı kopyadır; diğeri PATH'te önce geliyorsa betik bunu söyler.

Crate `imece-tui`, ikili dosyası `imece`. SQLite derlemeye gömülür, depo için sistem kütüphanesi gerekmez. Git PATH'te olmalı: imece değişiklikleri git'in kendisiyle okur ve uygular; üç yollu uygulama git 2.56 üzerinde sondalandı.

## Satıcı CLI'ları

imece başlarken PATH'te üç programı arar (`PATH'te kodlama ajanları aranıyor…`) ve bulduklarını sürer: `claude`, `codex` ve `gemini`. Biri yeter. Hiçbiri yoksa şu sözle durur: `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code (https://claude.com/claude-code) and log in`.

| Program | imece ne çalıştırır | Nesi olmalı |
| --- | --- | --- |
| `claude` (Claude Code) | ajan başına uzun ömürlü bir `claude -p` stream-json oturumu; `--confine open` değilse `--restricted` altında | bir oturum açma; maliyetler dolar olarak bildirilir ve keseye yazılır. Claude Code 2.1.314 üzerinde sondalandı |
| `codex` (Codex CLI) | ajan başına uzun ömürlü bir `codex app-server` (stdio üzerinde JSON-RPC) | bir **ChatGPT oturumu** (`codex login`). API anahtarıyla oturum reddedilir: Codex dolar değil token bildirir ve imece bunları henüz fiyatlandıramaz. ChatGPT planında bir Codex turu keseye hiçbir şey yazmaz; iz bunu söyler ve token'ları gösterir. Codex 0.165.0 üzerinde sondalandı |
| `gemini` (Gemini CLI) | ajan başına uzun ömürlü bir `gemini --acp` (Agent Client Protocol), imece'nin sahip olduğu özel bir dizinde | bir API anahtarı ya da Google oturumu. API anahtarıyla turlar imece'nin taşıdığı liste fiyatlarından yazılır (fiyatlandıramadığı bir model oturumu durdurur; `--model` ya da `--tier` ile bildiği bir model adı verin). Google oturumuyla keseye hiçbir şey yazmaz. Vertex AI ve gateway oturumları reddedilir. Gemini CLI 0.65.0 üzerinde sondalandı |

Varsayılan `--confine standard` ve `strict` altında her satıcının oturumu, kendi satıcı kurulumunuzdan hiçbir şey yüklenmeyecek şekilde başlatılır (hook yok, imece'ninki dışında MCP sunucusu yok, depo ayarı yok); `--confine open` satıcıyı sizin kurduğunuz gibi çalıştırır. Her durumda lider hiç yazmaz ve satıcının kendi alt ajan aracı kapalı kalır. Bir oturumun neyle çalıştığını bildirdiği, kapalı düşerek denetlenir; bkz. [satıcılar](../concepts/vendors.md) ve [güvenlik](../concepts/security.md).

Her ajan deponuzun kodlama ajanları için notlarını da alır: `AGENTS.md`, yoksa `CLAUDE.md`, en çok 16 KiB; daha uzun bir dosya kesilerek girer ve günlük bunu söyler.

## Kim lider

PATH'te tek satıcı varsa o liderdir. Birden çoksa, `--lead claude|codex|gemini` söylemediyse ya da soruyu bir kez `p` ile yanıtlayıp o satıcıyı birincil yapmadıysanız, imece başta sorar (`KİM LİDER?` ekranı). `--lead ask` yine de sorar. Ayrıntı [ilk çalıştırma](first-run.md) sayfasında.

## Terminaller

imece alternatif ekranda çizer ve tuşları crossterm ile okur; her terminal çalışır. Birkaç şey terminale bağlıdır:

- **Yazarken yeni satır için `⇧enter`**, imece'nin başta istediği kitty klavye protokolünü gerektirir. Konuşan terminaller: kitty, Ghostty, WezTerm (`enable_kitty_keyboard` ile), foot, Alacritty, iTerm2. Başka yerde `⇧enter` düz `enter`dır ve gönderir; yeni satır için `\` sonra `enter` yazın (her yerde çalışır) ya da Option'ın Meta olduğu yerde `⌥enter`. `?` yardımı ve alt satır terminalinizde hangisinin çalıştığını söyler. Sorguyu yanıtlamayan bir terminal çoğullayıcısı (tmux, screen), çevresindeki terminal daha fazlasını yapabilse de `\` `enter` yoluna düşer.
- **macOS'ta alt tuşları** (`⌥b` `⌥f` `⌥d` `⌥backspace` `⌥enter`) terminalin Option'ı Meta olarak göndermesini gerektirir: iTerm2 → Profiles → Keys → Left Option key: *Esc+*; Terminal.app → *Use Option as Meta key*. Yoksa `⌥b` `∫` yazar.
- **`terminal` teması** (varsayılan) terminalinizin kendi 16 rengini kullanır ve ekran başlamadan önce koyu mu açık mı olduğunu bilmek için arka ve ön planını sorar (OSC 11 / OSC 10); iTerm2, Ghostty, kitty, WezTerm, Alacritty, foot ve tmux yanıtlar. Yanıtlamayan bir terminal koyu sayılır ya da `COLORFGBG` ne derse. `COLORTERM=truecolor` (ya da `24bit`) yoksa bir şemanın RGB renkleri 256 renk küpüne yuvarlanır. Tema, pencere yeniden odaklanınca terminalinizin açığa ya da koyuya dönmesini izler; tmux altında bunun için `focus-events on` gerekir. Bkz. [temalar ve dil](../screen/themes-and-language.md).
- **Başlık** (`imece · 2 seni bekliyor`) xterm'in başlık yığınını kullanır; xterm, iTerm2, kitty, WezTerm ve Ghostty imece bitince kabuğunuzun başlığını geri verir; yığını olmayan bir terminal kendi varsayılanını gösterir.

Fare hiç gerekmez ve hiç yakalanmaz; terminalinizin kendi metin seçimi çalışmayı sürdürür.

## Durum nerede durur

imece'nin hatırladığı her şey yalnız sizindir (dosyalar `0600` yapılır) ve tek bir dizin altında durur: `$XDG_STATE_HOME/imece`, ya da `XDG_STATE_HOME` yoksa `~/.local/state/imece` (mutlak bir yol gerekir).

| Yol | Nedir |
| --- | --- |
| `<state>/imece.sqlite` | depo: projeler, ajanlar, kartlar ve hikâyeleri, günlük, gelen kutusu, posta, pano, kese, ekran tercihleriniz (yan panel genişliği, tema, birincil satıcı). WAL kipinde; açıkken `imece.sqlite-wal` ve `imece.sqlite-shm` yanında durur |
| `<state>/imece.sqlite.lock` | bir imece çalışırken `flock` ile tutulur; aynı dosyada ikinci bir imece reddedilir ve ilkinden `:open` kullanması söylenir |
| `<state>/worktrees/<dizin>-<özet>/<ajan>` | işçi başına bir git worktree (`claude-1`, `codex-2`, …), HEAD'inizde ayrılmış, proje başına bir dizin altında (checkout'un dizin adı ve yolunun 8 haneli özeti). Çalıştırmalar arasında saklanır. Her birinin kendi `target/` dizini var |
| `<state>/worktrees/<dizin>-<özet>-gemini/<ajan>` | bir Gemini oturumunun çalıştığı özel dizin (policy dosyası ve `GEMINI.md`); liderinki proje ne adlandırılırsa adlandırılsın `lead` |

Bu dizinin dışında satıcılar her zaman tuttuklarını tutar: Claude Code transkriptlerini `~/.claude/projects/` altında, Codex thread kayıtlarını `~/.codex` altında, Gemini sohbet dosyalarını `~/.gemini/tmp` altında. `--fresh` ve `:forget` imece'nin unuttuğu oturumlar için bunları kaldırır; bkz. [oturumlar](../concepts/sessions.md). imece'nin MCP soketleri, ajan başına bir tane, çalıştırma süresince sistemin geçici dizini altında özel bir dizinde durur.

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
