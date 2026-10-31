---
title: Onaylar ve sorular
description: Bir ajan yapmadan önce size neyi sormalı, rutin bir komut nasıl kendi başına çalışır, bir ⚑ istek ve bir ? soru SENİ BEKLEYEN'de nasıl bekler, run aracı ve izin listesi.
---

# Onaylar ve sorular

İki tür şey bir ajanı siz yanıtlayana dek durdurur: bir **onay** (`⚑`), satıcısı bir araç çağrısından önce soracakken imece'nin soruyu size vermesi; ve bir **soru** (`?`), ajanın `send(to: "you", type: ask)` ile size kendisinin bir şey sorması. İkisi de SENİ BEKLEYEN'de bekler; ilkini `a` ve `r` yanıtlar, ikincisini ajana yazmak.

## Onaylar (`⚑`)

`--confine standard` ve `open` altında bir oturumun yapmadan önce soracağı şey, reddedilmek yerine size gelir. `strict` altında hiçbir şey sormaz: soracak olan reddedilir ve komutlar imece'nin `run` aracından geçer.

| Satıcı | İstek imece'ye nasıl ulaşır |
| --- | --- |
| Claude Code | `--permission-prompts host --permission-prompt-tool mcp__imece__approve`: Claude Code, onay gereken bir araç çağrısından önce imece'nin `approve` aracını kendisi çağırır (kendi salt okunur kuralları neyin hiç sormayacağına karar verir). Araç modelden gizlidir. |
| Codex | yazanlar (işçiler, yardımcılar) `approval_policy = "on-request"` ile çalışır: sandbox'ın verdiğinden fazlasını gerektiren (dizin dışına yazma, sandbox dışında komut) bir istek olarak gelir. Lider ve strict altındaki her şey `never`'da kalır: sandbox'ın reddettiği yalnızca başarısız olur. |
| Gemini CLI | politika dosyası yazanların `run_shell_command`'ını her komuttan önce sorar yapar (`ask_user`); diğer araçları sormadan izinli kalır. Open altında kendi politikalarınız karar verir. |

### Rutin komutlar hemen çalışır

Bir kabuk komutu **rutin**dir: tek satırsa, argümanlarıyla tek bir programsa (kabuğun başka türlü okuyacağı hiçbir şey yok: boru, yönlendirme, `&&`, `$`, glob yok) ve `run` aracının politikası role onu çalıştırmaya izin veriyorsa (aşağıda). Rutin bir komuta kendi başına izin verilir, izde "allowed on its own" satırıyla; derleme, testler ve git'in okuma komutları sizi asla beklemez. Codex için komutu sardığı kabuk (`/bin/zsh -lc '…'`) önce soyulur ve içteki komut olduğu gibi yargılanır; içindeki `&&` rutin değildir. Gemini için yalnız ajanın dizininde çalışan bir komut rutin olabilir.

### Kalan her şey bekler

Başka her şey SENİ BEKLEYEN'de bir `⚑` öğesidir: "claude-1 onayını bekliyor: Bash: curl … | head". Her istek kendi başına bir öğedir ve seçili olan satırının altında **bütünüyle** gösterilir, satırları ` ↵ ` ile birleştirilmiş, asla markdown olarak okunmaz: izin verdiğiniz, çalışandır. Ajanın salon satırı `⚑ onayını bekliyor` okunur.

```text
 SENİ BEKLEYEN (1)
 ⚑ 14:10 claude-1 onayını bekliyor: Bash: curl -s https://example.org/schema.json | head -40
    curl -s https://example.org/schema.json | head -40
  a izin ver · r reddet · enter odak
```

- `a` izin verir; çağrı istendiği gibi çalışır ve günlük `claude-1 için izin verdin: Bash: …` der.
- `r` reddeder; ajana "The person refused this. Do not try it another way; if you need it, ask them" denir ve günlük `… için reddettin` der. Ret bir karardır, hata değil; tur devam eder.
- Bir çağrı en çok bir gün bekler. Bağlantı başına bir anda bir çağrı; bekleyen bir istek o oturumun diğer imece araçlarını tutar (istek ortasında zaten kullanamazdı).
- Oturum bitince bekleyen her istek "the session ended" ile yanıtlanır ve öğeleri kapanır. Deponun önceki çalıştırmadan tuttuğu bir istek başlangıçta kapatılır: soran süreç gitmiştir. Codex ve Gemini tur bitince isteği geri çeker; öğe kararsız kapanır.

Bir istek ne gösterir: kabuk komutu için komutu; başka bir araç için dokunduğunu (yol, adres, sorgu ya da desen) ya da girdisini 200 karakterde kesilmiş JSON olarak. Codex dosya değişiklikleri öğenin adını verdiği dosyaları ve verilen kök için `writes under <root>`'u gösterir. "Oturum boyunca izin ver" yoktur: rutin olmayan her istek bekler.

## Sorular (`?`)

Bir ajan size `send(to: "you", type: "ask", body)` ile sorar. Bir `?` öğesi olur, "codex-1 sana soruyor: eski CLI bayrağı kalsın mı?", ve soru açıkken ajan boştaysa salon satırı `? → you` okunur. Aynı ajandan daha yeni bir soru eskisinin yerini alır.

- `enter` konuşmayı soruda açar.
- `a` ya da `r` yazma satırını `@codex-1 ` ile açar ("sözsüz yanıt" yoktur); o ajana yazdığınız her şey soruyu kapatır ve sonraki turunu başlatır.
- Gitmiş ya da durmuş bir ajanın sorusu siz ona yazana dek açık kalır.

Kim ne zaman sorar: lider görev ortasında size sorabilir; bir işçi önce liderine sorar ve size yalnız sizin karar verebileceğinizi gönderir; bir yardımcı yalnız hiçbir ekip arkadaşının yanıtlayamayacağını. Sorular posta sınırlarına ve bütçeye sayılmaz ([mesajlar](../concepts/messages-and-board.md)).

## Bekleyene ulaşmak

- `W` her yerden sizi bekleyen sonraki öğeyi açar (bir değişikliğin diff'i, bir sorunun konuşması, bir onayın odağı), böylece `a`/`r` hemen yanıtlar; yine basılınca sonrakine geçer, sondan ilkine döner. Bekleyen yoksa: "Şu an senden bir şey gerekmiyor".
- `S` eldeki öğeyi erteler ve sonrakini açar: ertelenenler diğerlerinin hepsinden sonra, ertelenme sırasıyla gelir, satırları `sonra ·` ile işaretli. Erteleme yalnız bu çalıştırma içindir.
- Salon her projenin işçilerini sizden ne beklediklerine göre sıralar: sizi bekleyen (açık bir öğe ya da bekleyen durum) önce, sonra takılan (bloke ya da durmuş), sonra bitip bakılmamış, sonra kalanı; eşitler sırasını korur. `f` yalnız bir şey bekleyenleri gösterir. Başlığın filosu (`● çalışıyor ✓ bitti ✕ takıldı`) ve terminalin başlığı (`imece · 2 seni bekliyor`) aynı şeyleri sayar.

## `run` aracı

Her ajanın bus'ta bir `run` aracı vardır: argümanlarıyla tek bir program, kabuk yok, ajanın kendi dizininde (lider için checkout, işçiler ve yardımcılar için worktree).

| | İzinli |
| --- | --- |
| `cargo` | bağımlılıkları değiştiren ya da projenin dışına uzanan komutlar dışında hepsi: `add`, `remove`, `rm`, `install`, `uninstall`, `publish`, `login`, `logout`, `owner`, `yank` yok. Lider için dosya değiştirenler de yok (`--check`'siz `fmt`, `fix`, `clean`, `init`, `new`). |
| `git` | yalnız okuma komutları: `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status`. Komuttan önce seçenek yok (`-c`, `-C`, `--git-dir`), pager başlatan ya da dosya yazan yok (`-O…`, `--open-files-in-pager…`, `--output…`). |
| `--run PROG[,..]` | adını verdiğiniz programlar (`--run npm,make`), yalnız işçiler ve yardımcılar için; yinelenebilir. |
| başka her şey | reddedilir; izinli olanların listesi ve nasıl ekleneceğiyle. |

- Program tek başına, PATH'te adıyla anılır, asla yol olarak değil. Komut satırı en çok 1000 karakter; `| & ; < > ( ) $ \` # * ? ~ {` reddedilir.
- Bir çalıştırma `--run-timeout` saniye sonra (10-3600, varsayılan 600) ve imece ile birlikte öldürülür. Yanıt ekranın döngüsünü değil programı bekler.
- Çıktı derlenip toplanmış döner: renkler soyulur, geçen testlerin satırları atılır, tekrarlar katlanır, 48 KiB'ı aşınca ortası kesilir. Her worktree kendi `target/`'ında derler: ortak olanı işçileri sırayla birbirlerinin değişikliklerini yeniden derletirdi.
- `run`'ın kendi ağ engeli yoktur; lockfile uygun değilse cargo registry'ye uzanabilir.

Hangi kabuk komutlarının rutin olduğuna aynı politika karar verir; `standard` altında bir işçinin `cargo test`'i `run`'dan da satıcının kabuğundan da geçse çalışır; yalnız bir `run` sonucunu çalıştığı ağaca bağlar ve değişiklikte gösterir ([değişiklikler](../concepts/changes.md)).

## Ayrıca bakınız

- [Satıcılar](../concepts/vendors.md): confine düzeyleri ve her satıcının ne sorduğu.
- [SENİ BEKLEYEN](../screen/waiting.md): panel, `W`, `S`.
- [Mesajlar ve pano](../concepts/messages-and-board.md).
- [Güvenlik](../concepts/security.md): son kapı sizsiniz, bir araç listesi değil.
