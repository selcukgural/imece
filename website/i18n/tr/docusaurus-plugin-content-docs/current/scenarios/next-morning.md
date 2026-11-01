---
title: "Geri dönmek"
description: Ajanlar tur ortasındayken çıkmak ve ertesi gün yeniden başlamak; nerede kalmıştın penceresi, uyuyan ajanlar, her satıcının nasıl devam ettiği, uyanış özeti, bir oturumu unutturmak, uzaktayken özeti, başka penceredeyken haber, işaretler ve dışa aktarma.
---

# Geri dönmek

imece çalışmalar arasında hatırlar: projeler, ajanlar, kartlar, günlük, sizi bekleyenler ve bütçe tek bir SQLite dosyasında yaşar ve her ajan kendisiyle konuşulana dek uyur. Bununla bir çalışma kopyasında imece'yi ikinci kez başlattığınızda karşılaşırsınız. Bu senaryo akşam iki ajan tur ortasındayken çıkar ve ertesi sabah geri döner.

## 1. Tur ortasında çıkmak

`q`ya basın. Yarıda bir şey varken soru onu sayar:

```text
 imece kapatılsın mı? tur ortasındaki agent: 2, incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır
```

Görmek için `s`ye basın:

```text
 ÇIKMADAN ÖNCE
 Tur ortasında: çıkmak bu turları yarıda keser
    claude-1   src/parser.rs düzenliyor
    codex-1    cargo test çalıştırıyor
 İncelenmemiş değişiklikler
    Δ 18:40 gemini-1 bitirdi: config yükleyici (2 dosya, +30 −4)
 q çıkar · W bekleyen sıradakini açar · başka bir tuş: geri
```

`q` hemen çıkar; `W` onun yerine bekleyen sonraki öğeyi açar (önce değişikliği inceleyin, sonra çıkın); başka bir tuş geri döner. Yarıda bir şey yokken `q s` hiçbir şey açmaz ve soru sadedir. `:quit` (`:q`) aynı biçimde sorar, `:pending` listeyi açar.

`q`ya basın. Oturumlar biter; sürmekte olan turların istemleri yarım kalmış olarak saklanır, yeniden çalıştırılmaz. Gerisi kaydedilir; durum her adımdan sonra zaten yazılır, bir çökme az şey kaybettirir.

## 2. Sonraki `imece`

```sh
cd ~/code/parser
imece
```

Bir şey kımıldamadan önce bir pencere nerede kaldığınızı söyler:

```text
 NEREDE KALMIŞTIN · 14 sa 20 dk önce
 $12.40 harcandı, bütçe $50.00; her agent kendisiyle konuşulana dek uyur
 parser: 4 agent, 2 açık kart
 1 şey seni bekliyor:
   Δ 18:40 gemini-1 bitirdi: config yükleyici (2 dosya, +30 −4)
 imece kapanırken kesilen turlar: claude-1, codex-1
 kapanmadan önceki son satırlar:
   ─ 2026-10-31
   18:40 ! gemini-1 bitirdi: 2 dosya, +30 −4 → seni bekliyor
   18:41 › `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü
 herhangi bir tuş: kapat
```

Bütçe olduğu yerdedir: `--budget` verilmemişse korunur, `--budget 80` ayarlardı. Herhangi bir tuş pencereyi kapatır ve başka bir şey yapmaz. Yalnızca söyleyecek bir şey varken açılır; demoda hiç açılmaz.

Günlükte her kesilen tur için bir satır vardır ve salon herkesi uykuda gösterir:

```text
 09:00 ✕ claude-1 turu imece kapanınca yarım kaldı; uyanınca öğrenecek
 09:00 ✕ codex-1 turu imece kapanınca yarım kaldı; uyanınca öğrenecek
```

```text
   ◦ claude-1   #3   parser: iç içe listeler   ◦ önceki çalışmadan beri uyuyor, yazınca uyanır
   ◦ codex-1    #5   config: varsayılanlar     ◦ önceki çalışmadan beri uyuyor, yazınca uyanır
   ◦ gemini-1   #4   config yükleyici          ◦ önceki çalışmadan beri uyuyor, yazınca uyanır
```

Uykuda demek süreç yok, para yok demek. Bir ajanı üç şey uyandırır: ona yazarsınız, bir takım arkadaşından uyandıran türde bir mesaj gelir ya da bu çalışmada asılan bir kart ona önerilir. Yeniden başlatmayı bekleyen postalar ve kartlar kimseyi uyandırmaz. Bugünden eski bir günlük satırının üstünde bir `─ YYYY-MM-DD` satırı vardır; SENİ BEKLEYEN içinde başka günden bir öğe de öyle.

## 3. Yazmak uyandırır: her satıcı nasıl devam eder

`@claude-1 devam et` yazın ve `enter`. imece ajanın sahip olduğu oturumu arar ve yapabildiğinde devam ettirir; günlük hangi yoldan gittiğini söyler.

**Claude Code.** Hatırlanan oturum kimliği bir UUID olmalı ve kaydı Claude Code'un worktree için tuttuğu yerde durmalı (`~/.claude/projects/<alfanümerik olmayan her karakteri - olmuş worktree yolu>/<id>.jsonl`). O zaman oturum `--resume ID` ile başlar ve:

```text
 09:02 ⚑ claude-1 bildiği her şeyle, eski oturumunda devam ediyor
```

Devam eden bir Claude oturumu bütün oturumun maliyetini bildirir, bu yüzden imece saymaya daha önce yazdığı yerden başlar (maliyet tabanı) ve yalnızca üstünü yazar. Özet kısadır, çünkü oturum hikâyesini bilir: "imece started again since your last turn; this session goes on where it was, so you remember everything up to then. Since then:" ardından kesilen turun istemi ("A turn of yours was cut short when imece closed, before it ended; it is not run again. Its prompt was: …") ve kaç mesajın beklediği, ya da "Nothing; go on."

**Codex.** Thread'ler geçici değildir; Codex kaydını `~/.codex/sessions/` altında tutar; başlangıç thread kimliğiyle `thread/resume` gönderir ve yanıt o kimliği taşımalıdır. Kaydı gitmiş bir thread'i Codex herhangi bir turdan önce reddeder. Devam eden bir Codex thread'i başlatıldığı talimatları korur; o zamandan beri değişen (bir yeniden adlandırma, yeni takım arkadaşları) ona yalnızca imece'nin notlarıyla ulaşır.

**Gemini.** Oturum kimliğiyle `session/load`, Gemini'nin `~/.gemini/tmp/` altında tuttuğu sohbeti yeniden oynatır; geçmiş ilk turdan önce akar (en çok 50 mesaj, uzun bir oturum kuyruğuyla devam eder).

**Devam etmek olmadığında** (kayıt yok, rollout gitmiş, başka biçimde bir kimlik ya da `--fresh`), ajan yeni bir oturum başlatır ve nerede kaldığı ona anlatılır:

```text
 09:02 ⚑ codex-1 yeni bir oturumla başlıyor, kaldığı yer anlatıldı
```

Uyanış özeti ilk istemin önüne imece'den bir not olarak gider: "imece started again since your last turn, so this is a new session that remembers nothing of the old one. Where you left off:", sonra kartı ("Your task: #5 config: defaults (not finished). Its story, latest last:" ile son 12 hikâye satırı), kesilen turun istemi, kaç mesajın beklediği, worktree'sinin commit edilmemiş değişiklikleri ("Your worktree holds changes the person has not applied: 2 files, +12 −3: src/a.rs, src/b.rs") ve panonun açık kartları. Her satır temizlenir ve kesilir (satır başına 300 karakter, toplam 6000). Yeniden anlatılan bir işçi bitmemiş kartıyla sürer.

Her iki durumda worktree ilk istemden önce denetlenir: temiz olan HEAD'inize taşınır (`çalışma ağacı … yerine HEAD … oldu`), değişikliği olan kalır ve günlük söyler.

## 4. Bir oturumu bile bile unutturmak

Ters gitmiş (ya da uzamış) bir oturum düşürülebilir. `:forget @claude-1` (`:f`, tamamlama ajanları önerir):

```text
 claude-1 oturumunu unuttu; sonraki mesaj yeni bir oturum başlatır, kaldığı yer anlatılır
```

```text
 09:30 › claude-1 oturumunu unutturdun; yeni bir oturumla başlıyor, kaldığı yer anlatılacak
```

Çalışan oturum biter (çalıştırması öldürülür, bitmemiş istemleri yarım kalmış olarak saklanır) ve satıcının ondan tuttuğu şey imece'nin kendi iş parçacığında silinir: Claude'un kayıt dosyası, Codex'in thread'i (`thread/delete`), Gemini'nin sohbet dosyaları. Ajan kendisiyle konuşulana dek uyur; sonraki mesajı tam özetle taze bir oturum başlatır (`Restarted`). `:` menüsü `:forget`i `$` ile işaretler: yeni oturum bütün özeti önbelleksiz yeniden öder. Unutacak bir şeyi olmayan ajan reddedilir (`claude-1 oturumunu koruyor: …`). `imece --fresh` aynısını başta hatırlanan her ajan için yapar.

## 5. Uzaktayken özeti

Bir süre başka pencereye geçin. Terminal odağı üç dakika ya da daha uzun süre sonra geri aldığında, olanların özeti tuşsuz hemen açılır:

```text
 SEN YOKKEN · son 12 dk
  ✓ 0 kart birleşti
  ! 1 karar seni bekliyor
  ✕ 0 kez bloke olundu
  ⚑ 1 varsayım yapıldı
  ⇄ 4 konuşma
  09:33 ⚑ codex-1: çalışma ağacı 3f2a9c1 yerine HEAD 9b1d2e0 oldu
  09:41 ! codex-1 bitirdi: 1 dosya, +18 −2 → seni bekliyor
 herhangi bir tuş: kapat
```

Sayılar türe göre yeni günlük satırları ve şu an sizi bekleyenlerdir; altlarında o zamandan beri önemli satırlar (uygulamalar, hazır değişiklikler, blokajlar, hatalar, varsayımlar; son sekizi). Odağın gittiği andan bu yana günlükten kurulur, modele sorulmaz. Yazdığınız bir şeyin, yardımın, çıkış sorusunun ya da başka bir pencerenin üstüne açılmaz; daha kısa bir yokluk hiçbir şey göstermez ve tuşsuz geçen yirmi saniyeden sonraki ilk tuş hâlâ tuşla gelen özeti getirir. Terminal odağı bildirmeli (tmux'ta `focus-events on` gerekir).

Özet döndüğünüzde anlatır; `:notify` sizi çağırır. `:notify bell` terminalin zilini çalar, `:notify text` masaüstü bildirimi gönderir: siz başka penceredeyken sizi bekleyen bir şey geldiyse ve beş saniye sonra hâlâ oradaysa; bir değişiklik, bir soru, bir onay isteği, takılan bir ajan, her biri bir kez. Siz ayrılırken ekranda olan sessiz kalır, bekleme süresi içinde gelip giden de.

```text
 imece · codex-1 bitirdi: bir değişiklik seni bekliyor
```

Aynı anda birkaçı tek zil ya da sayıyla tek bildirimdir. Yalın `:notify` sırayla döner: off, bell, text, off; seçim hatırlanır. Özetle aynı odak bildirimlerini ister, metin için de OSC 9 bilen bir terminal (iTerm2, WezTerm, Ghostty; başka yerde `bell`).

## 6. İşaretlerle yer tutun

Günlükte (ya da açık bir konuşmada) yeniden isteyeceğiniz bir satırı seçin, diyelim planı taşıyan mesajı, ve `m`ye basın: `işaretlendi: ' ona geri getirir`. Satır saatinden sonra `✱` taşır. `'` sonraki işaretli satırı seçer, başa dönerek: açık konuşmada, yoksa günlükte; başka her yerden önce salonun günlüğüne gider. `m` yine işareti kaldırır (`işaret kaldırıldı`).

```text
 09:41✱ ⇄ lead → codex-1 [görev]: #5 config: varsayılanlar, src/config.rs
```

İşaretler sizindir: kayıt dosyasında çalışmalar arasında saklanır, asla daha yeni bir satırın altına katlanmaz ve ne kadar eski olsa da temizlikte korunur (günlük bunun dışında en yeni 5000 satırının ötesinde 90 günü tutar).

## 7. Bir konuşmayı yanınıza alın

`codex-1`in satırında `v` ile konuşmayı açın (sizinle konuşması) ya da bir günlük satırında `enter` (iki ajanın konuşması), sonra `:export`:

```text
 /Users/you/code/parser/imece-conversation-20261101-0945.md dosyasına yazıldı
```

Dosya `# Konuşma: lead ⇄ codex-1`dir; her mesaj, günü ve saati, kimden kime ve türüyle bir başlık (`### 2026-11-01 09:41 codex-1 → lead [bitti]`), altında metni. Onun yerine bir kartın ekranı açıkken (`#5`) dosya karttır: KİM, HİKÂYE, üstünde bekleyenler ve bütün günlüğü, katlanmamış. `:export ~/notes/` o dizinde bir dosya adlandırır; `:export ~/notes/config.md` dosyayı adlandırır. Hiçbir şeyin üstüne yazılmaz (`… yazılamadı`). Her satır ekranın temizlediği gibi temizlenir; işaretler dosyada görünmez.

## Ayrıca bakın

- [Oturumlar](../concepts/sessions.md): kayıt dosyası, satıcı başına devam, hijyen
- [Günlük](../screen/journal.md): günler, işaretler, saklama
- [Dosyalar](../reference/files.md): kayıt dosyası ve satıcıların dosyaları nerede
- [Kırık bir test, tek işçi](one-fix.md)
