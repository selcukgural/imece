---
title: Günlük
description: Günlük paneli - imece'nin neyi yazdığı, satırların nasıl katlandığı ve gününü taşıdığı, işaretler, uzaktayken özeti, nerede kalmıştın penceresi ve bir satırdan konuşma açmak.
---

# Günlük

Günlük (ekrandaki panelin adı DEFTER) ekibin alt yan panelidir (varsayılan olarak [hikâye](#hikâye) olarak anlatılır): ne olduğu, en yeni önce, olay başına bir satır. Bir agent'ın ya da sizin yaptığınız anlamlı her şey oradadır; olağan görev dağıtımları panoya ve liderin izine bırakılır. Depoda saklanır, böylece sonraki `imece` aynı günlükle açılır; uzaktayken özeti ve "nerede kalmıştın" penceresi ondan kurulur.

```text
┌ DEFTER ─────────────────────────────────────────────────┐
│14:07 › sen → claude-1: boş dosya denetimi `read` içinde …│
│14:05 ✓ claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1
│13:58 ! claude-1 bitirdi: değişiklik hazır · 3 dosya +41 −6
│13:57 ⇄ claude-1 → lead [soru] ↩: Hangi parser? İki …  +2
│  enter konuşmayı aç · c mesajı kopyala · m işaretle      │
│13:40✱› `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü │
│─ 2026-10-30                                              │
│18:12 → lead işe aldı: claude-1 (varsayılan model)        │
└──────────────────────────────────────────────────────────┘
```

## Hikâye

Alt yan panel varsayılan olarak günlüğü satır satır göstermez, hikâyeyi anlatır: aynı satırları karta göre gruplar. `J` (ya da `:story`) ikisi arasında geçer; seçim hatırlanır ve defterin başlığı o zaman geri dönüş tuşunu söyler: `DEFTER · J hikâye`. Bir kartın ekranında panel her zaman o kartın günlüğüdür.

```text
┌ HİKÂYE · J defter ─────────────────────────────────────────┐
│! seni bekleyen (3): claude-2, claude-3, claude-1 (claude-2 üzerinden)
│#1 Payment retry policy · claude-2 ≡ 4/5                    │
│  14:00 ⇄ lead-shop-api → claude-2 [assign]: #1 Payment re… │
│  14:01 ⇄ claude-4 → claude-2 [ask]: Zone codes an enum?    │
│  14:06 ⇄ claude-1 → claude-2 [ask]: Retry count from config?  yanıt yok · 2 dk
│  şimdi ⚑ çalıştırmak istiyor "WebFetch: https://…" · 4 dk     │
│#7 Payment retry policy · claude-3  ✓ bitti · değişiklik hazır│
│  14:05 ⇄ claude-3 → lead-shop-api [done]: Backoff capped…  │
│başka                                                       │
│  14:00 ⚑ node_modules claude-5'nin worktree'sine kopyalandı  ×12
└────────────────────────────────────────────────────────────┘
```

- **Kimi bekletiyorsunuz**, biri varsa en üstte: bir şeyi sizi bekleyen her agent (KARAR BEKLEYEN'de bir öge ya da size bir soru), ardından zaten bekleyen birine soru soran her agent, beklediğiyle birlikte (`claude-1 (claude-2 üzerinden)`). İlkini yanıtlamak diğerlerini de serbest bırakır.
- **Kart başına bir blok**, günlüğün son zamanda sözünü ettiği her kart için; en yeni satırı olan kart önce. Başlığı: numara ve başlık, sahibi, sonra ne kadar ilerlediği (`≡ 4/5` yapılacaklar listesi, `✓ bitti`, o kartın bir değişikliği bekliyorsa `değişiklik hazır`). Altında kartın en yeni üç satırı, en eskisi önce: onunla ilgili söylenenler, sahibinin değişikliği, bir durma, bir hata, bir uygulama. Bir satır, agent'ının yazıldığı anda üzerinde olduğu karta aittir; başka karta geçen bir işçi eski satırlarını yanında götürmez; söylenen bir şey soranın kartına aittir. Yanıt almamış bir soru bunu söyler: `henüz yanıt yok`, sonra `yanıt yok · 2 dk`. En sonda, kart bitmemişken, `şimdi` ve sahibinin şu an yaptığı: adımı ve ne kadar süredir.
- **Başka**: hiçbir karta ait olmayan satırlar (liderin işe alması, bir worktree'nin kurulması, bir API çağrısının yeniden denemesi); her tür bir kez, en yeni satırıyla ve kaç tane olduğuyla (`×12`). Bir görev ataması kartını adlandırır, bu yüzden o kartın bloğunda anlatılır.

Hikâye her karede en çok en yeni 2.000 günlük satırını okur ve paneli dolduracak kadar kart gösterir; daha eskiler için `J` ve günlük. `:clear`'ın kaldırdığı satırlar anlatılmaz (işaretli olan anlatılır) ve bir arşivin satırları arşivde bekler.

| Tuş | Ne yapar |
| --- | --- |
| `j` `k`  `gg` `G` | bir kartın başlığını ya da başka'daki bir satırı seç |
| `enter` | bir kart: ekranı ([kartlar](cards.md)) · başka'daki bir satır: günlük, o satırda, şimdilik (`DEFTER · esc hikâye`; `esc` ya da `J` hikâyeye döner) |
| `#` | seçili kartın numarasını yaz |
| `J` | günlük |

## Neler günlüğe geçer

Her satırda saat, türü için bir işaret, birden çok proje açıksa projenin adı ve sözcüklerle olay vardır.

| İşaret | Satırlar |
| --- | --- |
| `⇄` | iki taraf arasında bir mesaj, `kimden → kime [tür]: ilk satır`; başka bir mesaja yanıtsa türden sonra `↩`. Kendi mesajlarınız, `sen → claude-1: …`, ve bir agent'ın size kapanış metni, `claude-1 yanıt verdi: …` |
| `→` | liderin işleri: asılan bir kart, işe alınan bir işçi (`lead işe aldı: claude-1 (varsayılan model)`), işini bitirip ayrılan bir işçi. Görev dağıtımı ve panodan almalar panelde gösterilmez (pano ve kartın ekranında vardır) |
| `!` | sizin için bir şey: bir işçi bitirdi (`claude-1 bitirdi: değişiklik hazır · 3 dosya +41 −6`), `:commit`'in önerisi geldi, bütçe azalıyor |
| `✓` | bir değişiklik uyguladınız: `claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1` |
| `⚑` | bir agent'ın varsaydığı ve oturumun notları: `bildiği her şeyle, eski oturumunda devam ediyor`, `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`, burada olmayan bir model (`varsayılanla devam`), confine düzeyi, HEAD'inize taşınan ya da yerinde kalan bir worktree, bir worktree'ye kopyalanan bir yol, komut listelemeyen bir oturum |
| `■` | bloke, durdu (`claude-1: oturum bitti: <neden>`), uygulanamayan bir değişiklik, imece kapanınca yarım kalan bir tur, atlanan bir kopya, gelmeyen commit mesajı |
| `›` | siz: söylediğiniz, çalıştırdığınız, onayladığınız, reddettiğiniz, izin verdiğiniz ya da reddettiğiniz, ayarladığınız (`bütçeyi $80.00 yaptın (önce $50.00)`), açtığınız, kapattığınız, adlandırdığınız, unutturduğunuz |

Bunların arasında:

- **Yanıt verdi.** Bir turun kapanış metni yalnız size ulaştığında `yanıt verdi` olarak günlüğe geçer: siz sorduğunuzda, ya da takım sakinleşince liderin raporu. Postanın ya da bir pano teklifinin başlattığı bir turun kapanış metni agent'ın gönderdiğini yineler, ki günlükte zaten mesaj olarak vardır; kartın hikâyesinde ve izde kalır. Bu kural olmadan konuşmalar kendini yineler gibi okunuyordu.
- **Değişiklikler.** `bitirdi: değişiklik hazır · …`, `… checkout'a uyguladın, stage'de; çalışma ağacı işareti <sha>`, `… uygulanamadı: <git'in sözleri>`, `geri gönderildi: claude-1`; kısmi bir uygulamadan sonra kalan yeni bir `bitirdi` olarak döner.
- **Oturumlar.** `bildiği her şeyle, eski oturumunda devam ediyor`, `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`, `claude-1 oturumunu unutturdun`, `durdu: …`, `turu imece kapanınca yarım kaldı; uyanınca öğrenecek`, kendi komutlarını listelemeyen bir oturum, strict olmayan her başlangıçta söylenen confine düzeyi.
- **Worktree'ler.** `çalışma ağacı <sha> yerine HEAD <sha> oldu`, ya da commit edilmemiş değişiklikleri ya da kendi commit'leri olduğu için yerinde kaldı.
- **Kopyalar.** `.env → claude-1 çalışma ağacına kopyalandı (1.2 KiB)`, ve neyin neden kopyalanmadığı (`:copy`); checkout'un `.worktreeinclude` dosyasının saydıkları kendi satırlarında (`…, .worktreeinclude'un saydığı`), ve onun uygulanmayan bir satırı nedeniyle.
- **Kabuğunuz.** `` `cargo test` çalıştırdın → çıkış 0 · 12 sn sürdü ``, ya da `2 sinyali durdurdu`, süre onu söyleyen en büyük birimlerle (`40 ms`, `2 sn`, `1 dk 5 sn`).
- **Bütçe.** `bütçeyi … yaptın`, ve bir kez, bu hızla yarım saatten az kaldığında, `$4.00/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır`.
- **İstekler.** `claude-4 için izin verdin: npm install left-pad`, `… için reddettin: …`.
- **Commit mesajları.** `:commit`'in önerisi ve yapan model, ya da neden gelmediği.

### Katlama ve günler

Bir konuşmanın eski satırları en yenisinin altına katlanır: satır sonundaki `+2`, altında iki satır daha olduğunu söyler. İşaretli bir satır asla katlanmaz: `'`'in ineceği bir satır tutar.

Üstündeki satırdan başka bir günün ilk satırının üstünde soluk bir `─ 2026-10-30` satırı durur; en yeni satırı dün olan bir günlük tarihiyle başlar, hepsi bugün olan hiç tarih göstermez. Kopyalanan ve dışa aktarılan metin hep günü taşır, çünkü ekranı terk eder. Saat imece'nin ilk başladığı günü izler (bir dünyanın yaşamı içindeki yaz saati değişimi onu bir saat kaydırır).

90 günden eski ve en yeni 5000'in dışında kalan satırlar canlı bir başlangıçta temizlenir; işaretli bir satır asla temizlenmez.

## Günlükteki tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `tab` `⇧tab` | günlük paneline (ekip → bekleyenler → günlük) |
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

Yarım saat tuşa basılmazsa bastığınız ilk tuş iş yapmak yerine SEN YOKKEN · son N dk penceresini açar: birleşen kartlar, sizi bekleyen kararlar, blokeler, yapılan varsayımlar, konuşmalar, her biri sayılmış, sonra o zamandan beri önemli satırlar. Günlükten kurulur, model sorulmaz. Herhangi bir tuş kapatır.

Başka bir pencerede yarım saat ya da daha uzun kaldıktan sonra terminale döndüğünüzde aynı özet tuş beklemeden hemen açılır: terminal odağı ne zaman kaybettiğini ve geri aldığını bildirir. Daha kısa uzaklıklar bir şey göstermez. Yazdığınızın, yardımın, çıkış sorusunun ya da başka bir pencerenin üstünde açılmaz ve günlükte yeni bir şey yoksa gösterilecek bir şey yoktur. Odak bildirmeyen bir terminal ya da çoklayıcı (`focus-events on` olmayan tmux) yalnız tuşla geleni alır.

`:away` ikisinin de süresini ayarlar: `:away 10m`, `:away 2h`, bir dakikadan bir güne; `:away off` hiç kendiliğinden açmaz. Depoda hatırlanır. `:digest` ne zaman isterseniz açar: pencereden son ayrıldığınızdan, ya da çalıştırma başladığından beri olanlar.

## Ekranı temizlemek

`:clear` günlüğün satırlarını ve KARAR BEKLEYEN'in listelediklerini ekibin ekranından kaldırır; `:clear journal` ve `:clear inbox` birini. Önce on saniye bekler, `u` ekranı olduğu gibi bırakır. Hiçbir şey silinmez: depo her satırı tutar, `:export`, bir kartın ekranı ve arşivler hâlâ gösterir, temizlemenin kendisi de saklanır, böylece sonraki açılış da temiz başlar. İşaretli bir satır kalır. Sizi hâlâ bekleyen açık kalır ve başlıkta sayılır, `KARAR BEKLEYEN (0 · +2 gizli)`, `W` sıradakini açar ve listeye geri koyar. `:unclear` (ya da `:unclear journal`, `:unclear inbox`) hepsini geri getirir.

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
