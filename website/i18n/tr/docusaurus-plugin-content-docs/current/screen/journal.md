---
title: Günlük
description: Günlük paneli - imece'nin neyi yazdığı, satırların nasıl katlandığı ve gününü taşıdığı, işaretler, uzaktayken özeti, nerede kalmıştın penceresi ve bir satırdan konuşma açmak.
---

# Günlük

Günlük (ekrandaki panelin adı DEFTER) salonun alt yan panelidir: ne olduğu, en yeni önce, olay başına bir satır. Bir agent'ın ya da sizin yaptığınız anlamlı her şey oradadır; olağan görev dağıtımları panoya ve liderin izine bırakılır. Depoda saklanır, böylece sonraki `imece` aynı günlükle açılır; uzaktayken özeti ve "nerede kalmıştın" penceresi ondan kurulur.

```text
┌ DEFTER ─────────────────────────────────────────────────┐
│14:07 › sen → claude-1: boş dosya denetimi `read` içinde …│
│14:05 ✓ claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1
│13:58 ! claude-1 bitirdi: 3 dosya, +41 −6 → seni bekliyor
│13:57 ⇄ claude-1 → lead [soru] ↩: Hangi parser? İki …  +2
│  enter konuşmayı aç · c mesajı kopyala · m işaretle      │
│13:40✱› `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü │
│─ 2026-10-30                                              │
│18:12 ◆ lead işe aldı: claude-1 (varsayılan model)        │
└──────────────────────────────────────────────────────────┘
```

## Neler günlüğe geçer

Her satırda saat, türü için bir işaret, birden çok proje açıksa projenin adı ve sözcüklerle olay vardır.

| İşaret | Satırlar |
| --- | --- |
| `⇄` | iki taraf arasında bir mesaj, `kimden → kime [tür]: ilk satır`; başka bir mesaja yanıtsa türden sonra `↩`. Kendi mesajlarınız, `sen → claude-1: …`, ve bir agent'ın size kapanış metni, `claude-1 yanıt verdi: …` |
| `◆` | liderin işleri: asılan bir kart, işe alınan bir işçi (`lead işe aldı: claude-1 (varsayılan model)`), işini bitirip ayrılan bir işçi. Görev dağıtımı ve panodan almalar panelde gösterilmez (pano ve kartın ekranında vardır) |
| `!` | sizin için bir şey: bir işçi bitirdi (`claude-1 bitirdi: 3 dosya, +41 −6 → seni bekliyor`), `:commit`'in önerisi geldi, bütçe azalıyor |
| `✓` | bir değişiklik uyguladınız: `claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1` |
| `⚑` | bir agent'ın varsaydığı ve oturumun notları: `bildiği her şeyle, eski oturumunda devam ediyor`, `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`, burada olmayan bir model (`varsayılanla devam`), confine düzeyi, HEAD'inize taşınan ya da yerinde kalan bir worktree, bir worktree'ye kopyalanan bir yol, 16 KiB'de kesilen depo notları, komut listelemeyen bir oturum |
| `✕` | bloke, durdu (`claude-1 durdu: <neden>`), uygulanamayan bir değişiklik, imece kapanınca yarım kalan bir tur, atlanan bir kopya, gelmeyen commit mesajı |
| `›` | siz: söylediğiniz, çalıştırdığınız, onayladığınız, reddettiğiniz, izin verdiğiniz ya da reddettiğiniz, ayarladığınız (`bütçeyi $80.00 yaptın (önce $50.00)`), açtığınız, kapattığınız, adlandırdığınız, unutturduğunuz |

Bunların arasında:

- **Yanıt verdi.** Bir turun kapanış metni yalnız size ulaştığında `yanıt verdi` olarak günlüğe geçer: siz sorduğunuzda, ya da takım sakinleşince liderin raporu. Postanın ya da bir pano teklifinin başlattığı bir turun kapanış metni agent'ın gönderdiğini yineler, ki günlükte zaten mesaj olarak vardır; kartın hikâyesinde ve izde kalır. Bu kural olmadan konuşmalar kendini yineler gibi okunuyordu.
- **Değişiklikler.** `bitirdi … → seni bekliyor`, `… checkout'a uyguladın, stage'de; çalışma ağacı işareti <sha>`, `… uygulanamadı: <git'in sözleri>`, `reddedildi: claude-1`; kısmi bir uygulamadan sonra kalan yeni bir `bitirdi` olarak döner.
- **Oturumlar.** `bildiği her şeyle, eski oturumunda devam ediyor`, `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`, `claude-1 oturumunu unutturdun`, `durdu: …`, `turu imece kapanınca yarım kaldı; uyanınca öğrenecek`, kendi komutlarını listelemeyen bir oturum, strict olmayan her başlangıçta söylenen confine düzeyi, 16 KiB'de kesilen depo notları.
- **Worktree'ler.** `çalışma ağacı <sha> yerine HEAD <sha> oldu`, ya da commit edilmemiş değişiklikleri ya da kendi commit'leri olduğu için yerinde kaldı.
- **Kopyalar.** `.env → claude-1 çalışma ağacına kopyalandı (1.2 KiB)`, ve neyin neden kopyalanmadığı (`:copy`).
- **Kabuğunuz.** `` `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü ``, ya da `2 sinyali durdurdu`, süre onu söyleyen en büyük birimlerle (`40 ms`, `2 sn`, `1 dk 5 sn`).
- **Bütçe.** `bütçeyi … yaptın`, ve bir kez, bu hızla yarım saatten az kaldığında, `$4.00/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır`.
- **İstekler.** `codex-2 için izin verdin: npm install left-pad`, `… için reddettin: …`.
- **Commit mesajları.** `:commit`'in önerisi ve yapan model, ya da neden gelmediği.

### Katlama ve günler

Bir konuşmanın eski satırları en yenisinin altına katlanır: satır sonundaki `+2`, altında iki satır daha olduğunu söyler. İşaretli bir satır asla katlanmaz: `'`'in ineceği bir satır tutar.

Üstündeki satırdan başka bir günün ilk satırının üstünde soluk bir `─ 2026-10-30` satırı durur; en yeni satırı dün olan bir günlük tarihiyle başlar, hepsi bugün olan hiç tarih göstermez. Kopyalanan ve dışa aktarılan metin hep günü taşır, çünkü ekranı terk eder. Saat imece'nin ilk başladığı günü izler (bir dünyanın yaşamı içindeki yaz saati değişimi onu bir saat kaydırır).

90 günden eski ve en yeni 5000'in dışında kalan satırlar canlı bir başlangıçta temizlenir; işaretli bir satır asla temizlenmez.

## Günlükteki tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `tab` `⇧tab` | günlük paneline (salon → bekleyenler → günlük) |
| `↑` `↓` / `j` `k`, `gg` `G`, `^d` `^u` `^f` `^b` | bir satır seç |
| `enter` | satırın ait olduğu konuşmayı o satır seçili olarak aç; konuşma olmayan bir olay OLAY olarak açılır |
| `c` / `y` | seçili mesajı kopyala (yalnız metni; bir olay satırı günü ve saatiyle kopyalanır) |
| `m` (`:mark`) | satırı işaretle, yine basılınca kaldır; saatinin ardında `✱` görünür |
| `'` | eldekinden sonraki işaretli satır, en yeni önce, ilke dönerek; başka her yerden `'` önce günlüğe gelir |
| `#` | seçili satırın adlandırdığı ya da ilk andığı kart, kendi ekranında |

Seçili satırın altında soluk bir satır tuşlarını söyler: `enter konuşmayı aç · c mesajı kopyala · m işaretle`.

## İşaretler

`m` eldeki satırı sonra dönmek için işaretler: açık konuşmanın seçili satırını, yoksa seçili günlük satırını (bir kartın ekranında kartınkini); yine `m` işareti kaldırır ve flaş hangisi olduğunu söyler. Eldeki satır yoksa (başka bir panel, bir odak, pano) flaş `m`'nin nerede çalıştığını söyler. `'` sonraki işaretli satıra gider, ilke dönerek; hiç yoksa söyler. İşaret sizindir, çalıştırmayı aşar: depoda satırıyla tutulur, bir yükleme işaretli satırları ne kadar eski olsa da hep getirir ve temizlik hiçbirini bırakmaz. Demonun deposu yok, bu yüzden işaretleri çalıştırmayla biter. İşaretler `:export`'ta görünmez ve bütün işaretlerin bir listesi yok: `'` ile günlük onun yerine geçer.

## Sen yokken

Yirmi saniye tuşa basılmazsa bastığınız ilk tuş iş yapmak yerine SEN YOKKEN · son N dk penceresini açar: birleşen kartlar, sizi bekleyen kararlar, blokeler, yapılan varsayımlar, konuşmalar, her biri sayılmış, sonra o zamandan beri önemli satırlar. Günlükten kurulur, model sorulmaz. Herhangi bir tuş kapatır.

Başka bir pencerede üç dakika ya da daha uzun kaldıktan sonra terminale döndüğünüzde aynı özet tuş beklemeden hemen açılır: terminal odağı ne zaman kaybettiğini ve geri aldığını bildirir. Daha kısa uzaklıklar bir şey göstermez. Yazdığınızın, yardımın, çıkış sorusunun ya da başka bir pencerenin üstünde açılmaz ve günlükte yeni bir şey yoksa gösterilecek bir şey yoktur. Odak bildirmeyen bir terminal ya da çoklayıcı (`focus-events on` olmayan tmux) yalnız tuşla geleni alır.

## Nerede kalmıştın

Önceki bir çalıştırmayı hatırlayan canlı bir başlangıç, bir şey kımıldamadan önce NEREDE KALMIŞTIN · 2 sa 10 dk önce penceresini açar: bütçe (`$3.10 harcandı, bütçe $20.00; her agent kendisiyle konuşulana dek uyur`), açık proje başına bir satır (agent'lar, açık kartlar), panelin listelediği gibi sizi bekleyen en eski ögeler (en çok altı, sonra `… ve N daha`), önceki çalıştırmanın yarıda kestiği turlar ve önceki çalıştırmanın son altı günlük satırı, gün değiştiği yerde bir gün satırıyla. Herhangi bir tuş kapatır ve başka bir şey yapmaz. Anlatılacak bir şey yoksa pencere açılmaz; demoda hiç olmaz.

## Dışa aktarma

`:export` açık konuşmayı, yoksa açık kartın ekranını günlük satırlarıyla bir Markdown dosyası olarak yazar. Bkz. [konuşmalar](conversations.md) ve [kartlar](cards.md).

## Ayrıca bakınız

- [Konuşmalar](conversations.md)
- [Bir kartın ekranı](cards.md)
- [Oturumlar](../concepts/sessions.md)
- [Ertesi sabah](../scenarios/next-morning.md)
- [Klavye](keyboard.md)
