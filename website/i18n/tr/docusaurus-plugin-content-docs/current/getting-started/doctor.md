---
title: imece doctor
description: Bir açılışın ihtiyaç duyduğu her şeyin tek raporu (Claude Code girişi, git, çalışma kopyasının testleri, Lonca, terminal, dosyalarınız) ve yanlış olan için ne yapılacağı.
---

# imece doctor

`imece doctor` bir açılışın neye ihtiyaç duyduğuna bakar; neyin yanlış olduğunu ve nasıl düzeltileceğini, hiçbir ajan bir kuruş harcamadan önce söyler. imece'yi kurduktan sonra ya da bir takım başlamadığında çalıştırın.

```sh
cd sizin/checkout
imece doctor --lang tr
```

```text
Doktor özeti (imece 0.1.0, çalışma kopyası ~/code/shelf)

[✓] imece: 0.1.0, durum ~/.local/state/imece içinde
[✗] Claude Code
    ✓ claude --version: 2.1.321 (Claude Code)
    ✗ giriş yapılmamış
      `claude auth login` komutunu çalıştırın
    ✓ imece'nin verdiği her seçeneği alıyor
[!] git
    ✓ git version 2.56.0
    ! ~/code/shelf hiçbir dalda değil (kopuk HEAD)
      ajanlar her zamanki gibi çalışır; :lonca import bir dal ister (`git switch AD`)
[✓] Bu çalışma kopyasının testleri: python3 -m pytest … · python3 -m unittest … · pytest …
[✓] Lonca: lonca 0.2.2, bu çalışma kopyası Lonca'da değil
[✓] Terminal: iterm2 3.6.6, 120×40
[✓] Dosyalarınız: ^x ^e nvim açar

✗ Doktor 2 başlıkta sorun buldu.
```

## Nasıl okunur

Her satır bir başlıktır ve en kötü bulgusuyla işaretlenir:

| İşaret | Anlamı |
| --- | --- |
| `[✓]` | düzeltilecek bir şey yok; satır ne bulunduğunu söyler |
| `[!]` | çalışır, ama bir şey reddedilecek ya da olabileceği kadar iyi değil |
| `[✗]` | açılış ya da adı geçen kısım çalışmaz |

Sorunu olan bir başlık bütün bulgularını listeler; doğru olmayan her birinin altında ne yapılacağını söyleyen bir satır vardır. Başlığın içinde `•` bir nottur: bilmeniz gereken ama düzeltme istemeyen bir şey (tuş dosyası yok, dar bir terminal). Bir not, başlığı asla sorun yapmaz.

Son satır sorunlu başlıkları sayar. Çıkış kodu 0'dır; bir başlık `[✗]` ise 1 olur, böylece bir betik onda durabilir.

## Neye bakar

| Başlık | Bulgular |
| --- | --- |
| **imece** | sürümü; durum dizini (var, ya da oluşturulabilir ve yazabildiğiniz bir yer); oradaki boş alan; depo (şeması, daha yeni bir imece'nin yazdığı dosya, okunamayan dosya); depo üzerinde çalışan başka bir imece; kaç worktree tutulduğu |
| **Claude Code** | PATH'te mi ve sürümü; giriş yapılmış mı (plan ya da API anahtarı); `--help`'i imece'nin verdiği her seçeneği hâlâ anıyor mu |
| **git** | sürümü (seyrek bir çalışma kopyasının worktree'si 2.35 ister); klasör: bir checkout mu, commit'i var mı, bir dalda mı; varsa `.worktreeinclude` dosyası |
| **Bu çalışma kopyasının testleri** | imece'nin dosyalardan bulduğu test komutları (cargo, Python, Node, Go); ajanlar bunları sormadan çalıştırır ve bir değişikliğin hükmünde sayılırlar; hiç yoksa bir ipucu |
| **Lonca** | yalnızca `lonca` PATH'teyse ya da proje Lonca'daysa: sürümü, daemon ve oturumunuz, projenin deposu ve politikası |
| **Terminal** | adı ve sürümü, boyutu, 24 bit renk, kitty klavye protokolü (shift-enter), sekmede ilerleme, bildirimlerin nasıl gittiği, tmux, SSH |
| **Dosyalarınız** | `instructions.md`, tuş dosyası, push dosyası, `^x ^e`'nin açtığı editör, `:awake`'in makinenin uykusunu tuttuğu araç |

Commit'i olan bir git checkout'u yoksa testlere bakılmaz. Çıktı bir boruya ya da dosyaya gidiyorsa terminale soru sorulmaz.

## Seçenekler

| Seçenek | Ne yapar |
| --- | --- |
| `--repo DIR` | bakılacak çalışma kopyası (varsayılan: bulunduğunuz dizin) |
| `--verbose`, `-v` | her başlığın her bulgusunu listeler, yanlış bir şey olmayan yerde de |
| `--json` | rapor JSON olarak, betikler için |
| `--lang en\|tr` | raporun dili (varsayılan `en`, ya da `IMECE_LANG`) |

İşaretler terminalde renklidir; `NO_COLOR` bunu kapatır.

### `--json`

```json
{
  "imece": "0.1.0",
  "issues": 1,
  "failed": true,
  "categories": [
    {
      "id": "claude",
      "title": "Claude Code",
      "status": "fail",
      "items": [
        { "id": "claude.login", "status": "fail", "text": "not signed in", "fix": "run `claude auth login`" }
      ]
    }
  ]
}
```

Bir bulgunun `id`'si her dilde aynıdır; `status`'u `ok`, `note`, `warn` ya da `fail` olur, bir başlığınki en kötüsüdür (yalnızca notları varsa `ok`). `text` ve `fix` raporun dilindedir.

## Ekranda: `:doctor`

imece çalışırken `:doctor` aynı raporu, eldeki proje için, ekranın üstünde bir pencerede açar. imece ayrı bir iş parçacığında sorar, ajanlar çalışmaya devam eder; `v` her bulguyu listeler, `r` yeniden bakar, `esc` kapatır ([komutlar](../reference/commands.md#doctor)). Claude Code PATH'te değilse ya da giriş yapılmamışsa hoş geldin penceresi onu gösterir.

## Ne yapmaz

- Hiçbir modeli çağırmaz, hiçbir şey harcamaz.
- Hiçbir şey yazmaz: depo alınmadan ve değiştirilmeden okunur, bu yüzden çalışan bir imece'nin yanında çalıştırmak güvenlidir. Hiçbir şey oluşturulmaz, durum dizini bile.
- Claude Code'a ve Lonca'ya kendi komutlarıyla sorar (`claude --version`, `claude auth status`, `claude --help`, `lonca --version`, daemon'un soketi); onların dosyalarını asla okumaz.
- Düzeltmez. Her bulgu neyin çalıştırılacağını söyler.

imece'nin bir açılışta kendisinin verdiği her ret [sorun giderme](../troubleshooting.md) sayfasındadır.
