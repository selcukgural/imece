---
title: "Bir kart ters gidince"
description: "İş planlandığı gibi gitmeyince imece ne yapar: başka bir kartı bekleyen kart, başarısız olup yeniden denenen kart, üç denemeden sonraki sigorta, geri çekilen kart, lidere bir sağlık bildirimi, :ps ile bitirilen asılı bir komut ve hepsinin :ledger'daki maliyeti."
---

# Bir kart ters gidince

Kartların çoğu `assign`'dan onaylanmış bir değişikliğe gider. Bu senaryo ötekiler hakkında: başka bir kart bitmeden başlayamayan kart, işçisinin bitiremediği kart, artık kimsenin istemediği kart, kendi etrafında dönen bir işçi, asılı kalan bir test koşusu. imece bunların hiçbirini gizlemez ve kendiliğinden tekrarlanacak olanı siz evet diyene dek durdurur. Sonunda `:ledger` her denemenin neye mal olduğunu ve neyin boşa harcandığını gösterir.

Buradaki her şey demoda izlenebilir; demo hepsini kendi saatiyle bir saat içinde oynar:

```sh
imece --demo --projects 1 --agents 3
```

Demonun projesi `shop-api`, lideri `lead`, üç işçisi var. Rakamları `≈` ile başlar, çünkü demo bir planın girişiyle oturum açar (bir API anahtarında rakamlarda `≈` olmaz).

## 1. Başka bir kartı bekleyen kart

Lider bir kartın önce başka bir kartın işine ihtiyaç duyduğunu söyleyebilir: bir `assign` ya da `post` üzerindeki `after`, beklediği kartları numarasıyla adlandırır (en çok sekiz; yalnız projenin, kesin başarısız sayılmamış kartları). `after` taşıyan bir görev liderin aklında beklemez: hemen gönderilir, kartı panoda açılır ve kimin içinse o işçi onu ancak beklediği iş bitince alır. Günlük bunu söyler:

```text
 14:07 → claude-3 alacak: #6 Ürün aramasında sayfalama, #5 bitince
```

`:board` onu YAPILACAK altında, neyi beklediği ve kimin için tutulduğuyla işaretli gösterir:

```text
 ┌ YAPILACAK (10) ────┐
 │#6  Ürün aramasında…│
 │    ⧗ #5 bitince → c│
 │#21 Sipariş doğrula…│
 │    ✕ #20 başarısız…│
```

`⧗ #5 bitince` soluktur: yalnızca bekliyor. `✕ #20 başarısız` dikkat renginde: beklediği kart başarısız oldu, o kart yeniden verilip bitmeden bu başlamaz. Liderine bu hemen söylenir ve lider uyandırılır; lider ilgilenirken Home kartı ÇALIŞAN altında listeler, `■ #21 Import from CSV  #20 kartını bekliyor, o başarısız oldu · lead ilgileniyor`, liderin kendi hâline bıraktığı size gelir ([gelmeyecek bir şeyi bekleyen kart](../concepts/messages-and-board.md#gelmeyecek-bir-seyi-bekleyen-kart)). Panoda kimsenin almadığı bir kart da stall'un dakikaları dolunca aynı yolla liderin olur. Panoda `r` yalnız alınmaya hazır kartları gösterir ve başlık kaçını gizlediğini söyler (`· yalnız hazırlar: N bekleyen gizli · r hepsi`); yine `r` hepsini gösterir. `esc` panoyu kapatır.

Komut satırında `#6` kartı açar. Sütununun altında neyi beklediğini sayar: `✓` bitti, `✕` başarısız, `⧗` henüz değil:

```text
 shop-api · YAPILACAK · 75 dk
 sonra  #5 ⧗
```

`#5` bitince sıradaki teslim `#6`'yı başlatır (`claude-3 başladı: #6 Ürün aramasında sayfalama, beklediği bitti`) ve işçinin postasının başlığı `[assign 14 · #6, try 1]` olur. Tutulan bir görev uyuyan bir işçiyi uyandırmaz ve bir işçi hazır olmayan bir kartı panodan `claim` edemez.

## 2. Başarısız olan kart

Kartını bitiremeyen işçi bunu söyler: `outcome: "failed"` taşıyan bir `done`. Değişiklik gösterilmez, kart bitenler sütununa `✕` ile geçer ve günlükte işçinin ilk satırı olur:

```text
 14:34 ■ claude-1: #9 Para birimi desteği başarısız: Bitiremedim: çağıranlar kartın yollarının dışında bir değişiklik istiyor.
```

Liderin postasının başlığı `[done 12 · #9 failed]` olur ve lidere kartı neyi farklı yapacağıyla yeniden vermesi söylenir. Verir, başka bir işçiye:

```text
 14:35 → claude-3 aldı: #9 Para birimi desteği, claude-1 elinden, 2. deneme
```

Pano kartın üstünde ` · 2. deneme` der, kartın ekranı denemeleri sayar. `claude-1` hâlâ `#9` üzerinde çalışıyorduysa ve kart el değiştirdikten sonra onun için bir `done` gönderirse, o `done` geç kalmıştır: hiçbir şeyi kapatmaz, değişiklik göstermez ve worktree'si yazdıklarını tutar:

```text
 15:58 ⚑ claude-2: #50 için geç bir done; kart başka bir denemede, hiçbir şeyi kapatmıyor
```

Demoda her yedinci ilk deneme başarısız olur ve başarısız bir kart, lideri onu kendiliğinden her verdiğinde yine başarısız olur; böylece sonraki adım çabuk gelir.

## 3. Sigorta: üç deneme, sonra karar sizin

Bir kartın üç denemesi, değişikliğinin üç turu vardır. Onlardan sonra hiçbir şey kendiliğinden sürmez: liderin kart için sonraki `assign`'ı reddedilir ve kart KARAR BEKLEYEN'de, sorular altında sizi bekler:

```text
 KARAR BEKLEYEN 1 izin · 1 soru · 3 değişiklik
 ...
 sorular
▸! #9   Para birimi desteği       3 denemede başarısız: bir deneme  a  bir daha   r  başarısız say
                                  daha, ya da kart başarısız sayılır
```

```text
 15:00 ■ #9 Para birimi desteği 3 denemede başarısız: bir deneme daha onayınızı bekliyor
```

Durum satırı `a  bir daha   r  başarısız say` der. `#9` kartın ekranını açar ve nedenini gösterir: DENEMELER altında her deneme, işçisi, modeli ve maliyetiyle; hikâyede `3 denemede başarısız: karar kişinin`.

- `a` bir deneme daha verir. Flaş: `#9 için bir deneme daha: lider duyar`; günlük: `› #9 için bir deneme daha verdiniz`. Lider uyanır, kartı yeniden verir, pano `4. deneme` der.
- `r` kartı kesin başarısız sayar. Flaş: `#9 başarısız sayıldı`; günlük: `› #9 kartını başarısız saydınız: kimse onu yeniden vermez`. Hiçbir `assign` onu artık almaz ve lider onu hangi kartların beklediğini duyar; onları geri çeker ya da size söyler.

[Lonca](../concepts/projects.md#loncada) üzerindeki bir projede turlar da sayılır: Lonca'nın kuyruğu bir değişikliği üçüncü kez geri gönderince işçisi uyandırılmaz ve satır `değişikliği 3 kez geri döndü: bir tur daha, ya da kart başarısız sayılır` der. Sizin kendi geri göndermeniz, notlarınız ve geri gönderdiğiniz bir çakışma her zaman geçer: zaten karar veriyorsunuz.

## 4. Kimsenin istemediği kart

Lider bir kartın artık gerekmediğini görünce onu `withdraw` aracıyla, tek satırlık bir nedenle geri çeker:

```text
 15:15 → lead geri çekti: #34 Bildirim tercihleri: artık gerekmiyor: başka bir kart karşılıyor
```

Bekleyen, çalışılan ya da başarısız bir kart geri çekilebilir; değişikliği sizi bekleyen, Lonca'nın kuyruğunda olan ya da bitmiş bir kart çekilemez (değişikliğini siz geri gönderirsiniz). Onu tutan sonraki isteminde bunu duyar, hâlâ gönderdiği bir `done` geç kalmıştır ve pano kartı bitenlerin yanına, `–` ile ve soluk koyar. Ekranı GERİ ÇEKİLDİ der. Geri çekilmiş bir kartı bekleyen kart hiç başlamaz: pano onun üstünde `✕ #34 geri çekildi` der ve lidere onu da geri çekmesi söylenir.

## 5. Lidere bir sağlık bildirimi

imece her ajanı dört işarete karşı izler ve bunları size değil lidere söyler, çünkü onlarla lider bir şey yapabilir:

| İşaret | Ne zaman | Günlük satırı |
| --- | --- | --- |
| stalled | bir işçinin kartı çalışılıyor, işçi 10 dk boşta | `claude-2 10 dk'dır #9 Para birimi desteği üzerinde boşta, done demedi` |
| silent | liderden başka bir ajan turunun ortasında, 10 dk iz satırı yok | `claude-2 10 dk'dır turunda hiçbir şey demedi; son: Bash dotnet test` |
| free | kartı olmayan bir işçi 10 dk boşta | `claude-3 10 dk'dır kartsız boşta` |
| looping | tek bir çağrı bir turda 3 kez başarısız | `claude-2 bu turda 3 kez başarısız oldu: run dotnet test` |

Bir takım arkadaşının yanıtını 10 dakikadır bekleyen işçi bir uyarı değildir: bir `blocked` gibi liderinin çözeceği bir şeydir, satırı da `claude-3 yanıt vermedi "…" · lead ilgileniyor` der ([yanıt bekleyen işçi](../concepts/messages-and-board.md#yanıt-bekleyen-işçi)).

Demo döngüyü gösterir: başarısız her dördüncü test koşusu art arda üç kez başarısız olur.

```text
 15:05 ■ claude-2 bu turda 3 kez başarısız oldu: run dotnet test · lead uyarıldı
```

`· lead uyarıldı`, liderin bunun için uyandırıldığını söyler. Boştaki bir işçi `· lead sonraki turunda duyacak` ile biten bir `⚑` satırıdır ve kimseyi uyandırmaz (boştaki bir işçi bir şeye mal olmaz). Liderin notu ne yapacağını imece'nin sözleriyle söyler: `claude-2's call run dotnet test failed 3 times this turn: it may be going round. Cancel it, or tell it another way.` Her durgunluk bir kez söylenir; yeniden kıpırdamak yenisini başlatır. Sizi bekleyen, başlamakta olan, yeniden denemesi gelecek ya da yardımcıları çalışan bir ajan için, ya da lideri olmayan bir solo için hiçbir şey söylenmez.

Yalnız `:health` ayarları söyler: `lidere sağlık bildirimleri: stall 10m · idle 10m · loop 3 (stall ve idle dakika bekler; loop bir turda aynı çağrının hatalarını sayar)`. `:health stall 20m`, `:health idle 1h`, `:health loop 5` ya da `off` birini değiştirir; hatırlanırlar.

## 6. Asılı kalan bir test koşusu: `:ps`

Canlı bir işçi Claude Code'un kabuğuyla `dotnet test` çalıştırır ve test asılı kalır. On dakika sonra lider `claude-2 10 dk'dır turunda hiçbir şey demedi; son: Bash dotnet test · lead uyarıldı` duyar. Neyin çalıştığını görmek için `:ps` yazın:

```text
 ┌ SÜREÇLER · 9 ─────────────────────────────────────────────────────────┐
 │her ajanın oturumu ve altında çalışanlar, ps'in söylediği gibi: bir ç…│
 │                                                                       │
 │claude-1 · 75.5% · 716 MiB                                             │
 │  claude      41010    1.5%  287 MiB                                   │
 │    imece     41011    0.0%  9 MiB                                     │
 │    dotnet    61010   74.0%  420 MiB                                   │
 │claude-2 · 75.8% · 723 MiB                                             │
 │  claude      41020    1.8%  294 MiB                                   │
 │    imece     41021    0.0%  9 MiB                                     │
 │▶   dotnet    62010   74.0%  420 MiB                                   │
 │                                                                       │
 │ d  bunu sonlandır   j k  gez   esc  geri                              │
 └───────────────────────────────────────────────────────────────────────┘
```

Her ajanın toplamıyla bir satırı var, sonra oturumunun süreci (`claude`), altında imece'nin aktarıcısı (`imece`) ve oturumun başlattıkları. Pencere açıkken `ps`'i her üç saniyede yeniden okur. `j` `k` asılı `dotnet`'e gider; `d` sorar:

```text
 dotnet (62010) sonlandırılsın mı (claude-2 altında)? y sonlandırır, başka bir tuş bırakır
```

`y` ona `SIGTERM` gönderir, iki saniye sonra hâlâ oradaysa `SIGKILL`. Flaş ve günlük: `dotnet (62010) sonlandırıldı, claude-2 altında`. Yalnız o süreç biter; oturum turuna devam eder, komutun başarısızlığını okumak ona kalır. imece her sinyalden hemen önce sürecin hâlâ o oturumun altında olduğunu yeniden denetler. Oturumun kendi süreci reddedilir (`bu claude-2 ajanının oturumunun kendisi: onu :forget ya da durdurmak bitirir, :ps değil`), imece'nin aktarıcısı da.

Oturumlarını bitirmeden kapanan bir imece çalışması (bir çökme, kapanan bir terminal) onları çalışır bırakabilir. Bir başlangıç kaç tane bulduğunu söyler (`önceki bir çalışmadan kalan 4 süreç var: :ps listeler`) ve `:ps` onları `ÖNCEKİ BİR ÇALIŞMADAN KALAN · 4` altında, her ağacı çalıştığı worktree'nin altında listeler. `d` birini, `a` hepsini sonlandırır, her biri `y`'den sonra. Sizin `y`'niz olmadan hiçbir şey sonlandırılmaz.

Demonun süreçleri uydurmadır: her oturum altında `imece` olan bir `claude` ve çalışırken projenin test programı; bir de çökmüş bir çalışmanın artıkları.

## 7. Hepsi neye mal oldu: `:ledger`

Bir karta yapılan her deneme, Claude Code'un her oturumun harcadığı için söylediğinden yazılan, defterde bir satırdır. `:ledger` onu açar:

```text
 HARCAMALAR · 30 deneme · API karşılığı
 shop-api  uygulanan ≈$0.00 · boşa ≈$0.52 · süren ≈$1.26+? · lider ≈$0.00
   modele göre: claude-opus-5-5 ≈$0.95 · claude-sonnet-5-5 ≈$0.61 · claude-haiku-5-5 ≈$0.22
 ▶ #30 deneme 3  claude-2  claude-sonnet-5-5  0 Δ  ≈<$0.01+?  15 sn  sürüyor  2026-11-09 · 2.1.0
     modele göre: claude-sonnet-5-5 ≈<$0.01 · claude-haiku-5-5 ≈<$0.01
   #35 deneme 1  claude-3  claude-opus-5-5  0 Δ  ≈<$0.01  25 sn  sürüyor  2026-11-09 · 2.1.0
   #30 deneme 2  claude-3  claude-opus-5-5  0 Δ  ≈$0.07  6 dk  başarısız  2026-11-09 · 2.1.0
   #33 deneme 1  claude-2  claude-sonnet-5-5  1 Δ  ≈$0.07  8 dk  sürüyor  2026-11-09 · 2.1.0
```

Projenin satırı harcananı böler: `uygulanan`, değişikliğini uyguladığınız (ya da değişikliksiz bitmiş) denemelere; `boşa`, başarısız olan, geri çekilen ya da devredilen denemelere; `süren`, hâlâ açık denemelere; `lider`, hiçbir kartın olmayan liderin kendi harcaması. Her satır: kart ve denemesi, işçi, oturumunun çalıştığı model, önünüze koyduğu değişiklikler (`Δ` ve ` · N geri döndü`), maliyeti, süresi, nasıl bittiği (sürüyor, uygulandı, bitti, değişiklik yok, başarısız, geri çekildi, devredildi, geç done), gün ve Claude Code'un sürümü. `modele göre` bir satırı ona harcayan modellere böler: işçininki, yardımcılarınınki ve Claude Code'un kendi arka plan modeli.

`≈`, bir planın girişinde API karşılığı rakamları işaretler. `+?`, Claude Code'un hiç fiyatlamadığı bir parçası olan rakamı işaretler (maliyetsiz bir tur, hiçe fiyatlanmış bir model, turun ortasında biten bir oturum): gösterilen en az harcanan kadardır, bilinmeyen için asla `$0` değildir.

`j` `k` gezer, `enter` satırın kartını açar, `esc` kapatır. Bir kartın ekranı kendi denemelerini DENEMELER altında listeler.

## Ayrıca bkz.

- [Mesajlar ve pano](../concepts/messages-and-board.md): `assign`, `post`, `done`
- [Mesaj türleri](../reference/message-types.md): zarfın `after`, `outcome` ve `card` alanları
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md#harcamalar): defter, `≈` ve `+?`
- [Komutlar](../reference/commands.md): `:board`, `:health`, `:ps`, `:ledger`
