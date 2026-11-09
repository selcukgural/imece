---
title: Bir kartın ekranı
description: Bir kartın her şeyi tek ekranda - nerede durduğu, kim çalıştı, denemeleri ve hikâyesi - # ile kartın numarası ya da başlığından bir sözcükle açılır.
---

# Bir kartın ekranı

Bir kart tek bir iş birimidir: lider asar (ya da boşta bir işçiye mesajınız, işi bir değişikliğe varınca açar), bir işçi alır, bir `done` ile biter. Numarası, `#3`, ana ekranda işçisinin satırında, panoda ve günlükte görünür. `#` bir kartın ekranını açar: [detayının](changes.md#detay) log sekmesi (iş sahibinindir, yoksa ilk pay alanın), detayın başlığı altında kartın durumu, kimlerin payı olduğu ve hikâyesi; `tab` sahibinin diff'ine ve konuşmasına geçer, `esc` ana ekrana döner. Onunla, bütün günlüğü okumadan tek bir işe ne olduğunu öğrenmek istediğinizde karşılaşırsınız. Türkçe ekran karta `#` menüsünde "iş" der.

```text
#1 Kargo ücret hesabı · shop-api · claude-1 · 27 dk · $0.30

diff · konuşma · log                                                                       tab geçer
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│shop-api · İNCELENECEK · 33 dk                                                                    │
│sahibi src/Auth/TokenService.cs, src/Catalog/ProductQuery.cs, src/Orders/Money.cs                 │
│                                                                                                  │
│KİM                                                                                               │
│astı      ○ ▎lead                                         koordine ediyor · 33 dk                 │
│sahibi    ! ▎claude-1          #17  Fiyat listesi önbell… ▸ Çevresi…  $0.30  +6 −3  ↗ localhost:50│
│konuştu   ! ▎claude-2          #16  Arama sonuç sıralama… ▸ Değişik…  $0.26  +2 −1  ↗ localhost:│
│                                                                                                  │
│DENEMELER                                                                                         │
│#1 deneme 1  claude-1  claude-opus-5-5  4 Δ  $0.05  33 dk  sürüyor  2026-11-09                    │
│  modele göre: claude-opus-5-5 $0.04 · claude-haiku-5-5 <$0.01                                    │
│                                                                                                  │
│HİKÂYE                                                                                            │
│14:00 ◎ niyet      lead panoya astı: Kargo ücret hesabı                                           │
│14:00 ⇄ konuşma    lead → claude-1 [görev]: #1 Kargo ücret hesabı                                 │
│14:00 Δ değişiklik değiştirdi: src/Auth/TokenService.cs                                           │
│14:04 ⇄ konuşma    claude-2 → claude-1 [soru]: Bölge kodları enum mu olsun?                       │
│14:05 ⇄ konuşma    claude-1 → claude-2 [cevap] ↩: Hayır, ayrı kuyruk                              │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Bir kartı açmak

`#` komut satırını `#` ile ve kartların menüsüyle açar (en çok 30, açık projelerden). Rakamlar bir numaranın başıyla eşleşir; başka her şey başlığın içinde, büyük küçük harf ayırmadan, en yeni önce; hiçbir şey yazılmazsa her kart listelenir, en yeni önce. Her öge `#N başlık` biçimindedir ve bütün satırın yerini alır; `enter` vurgulu kartı alır ve hemen açar: tek tuş, önce ekle sonra gönder değil. Kart adlandırmayan bir satır `#… diye bir iş yok` diye flaşlanır. `tab` `⇧tab` `↑` `↓` her tamamlamada olduğu gibi menüde ilerler.

`#` eldeki kartı baştan yazar: yakınlaştırılmış ya da seçili agent'ınkini (bir yardımcınınki işçisininkidir), KARAR BEKLEYEN'de seçili ögenin geldiği kartı, seçili günlük satırının adlandırdığı ya da ilk andığı kartı. Bir kartın ekranında kendi kartı haber değildir, o yüzden `#` boş başlar.

`#N` ve `#word` yardımın bunun için kullandığı adlardır (Türkçe yardımda `#N  #kelime`): `#3` numarayla, `#manifest` başlıktan bir sözcükle.

## Ekran ne gösterir

Detayın başlığı (kart ve başlığı, projesi, işin sahibi olan agent, ne zamandır ve neye mal olduğu) ve sekme çubuğu altında:

- İlk satır: projesi, panonun sözcükleriyle sütunu (YAPILACAK, ÜZERİNDE, İNCELENECEK, DOĞRULANIYOR, BİTTİ, ya da başarısız olan için BAŞARISIZ, liderin geri çektiği için GERİ ÇEKİLDİ), ikinci denemesinden itibaren ` · 2. deneme`, ve dakikaları, bitmiş ya da başarısızsa bitişine kadar.
- Varsa neyi düzenlediği (lease'leri; yalnız demoda; sembol lease'leri canlıda yapılmadı).
- `assign` ya da `post`'u verdiyse sahip olduğu yollar (`sahibi src/parser.rs, docs (shared)`; [kartın sahip olduğu yollar](../concepts/messages-and-board.md#kartın-sahip-olduğu-yollar)); dışına bir yazma hikâyesinin bir `değişiklik` satırıdır: `değiştirdi: src/other.rs; kartın yollarının dışında`.
- Lonca'daki bir projede istediği denetimler (`denetim api, e2e`; [kartın istediği denetimler](../concepts/messages-and-board.md#kartın-istediği-denetimler)).
- Beklediği kartlar, her biri nasıl durduğuyla (`sonra  #2 ✓, #5 ⧗`): `✓` bitti, `✕` başarısız ya da geri çekildi, `⧗` henüz değil ([başka kartları bekleyen kartlar](../concepts/messages-and-board.md#başka-kartları-bekleyen-kartlar)).
- **KİM**: kim astı, sahibi, hâlâ sahibinin kartıyken sahibinin yardımcıları, sonra hikâyesinin hakkında konuştuğunu söylediği herkes; her biri bir kez, ilk aldığı rolle. [Uzun satır](salon.md#uzun-satır) olarak çizilirler, böylece durumları canlıdır.
- **DENEMELER**: karta her deneme, eskiden yeniye, [harcamaların](../concepts/budget-and-limits.md#harcamalar) listelediği gibi: deneme, işçi, model, değişiklikler ve geri dönenler, neye mal olduğu, ne kadar sürdüğü ve nasıl bittiği, altında da her modelin bundan ne harcadığı. Bir deneme bir şey harcadığında görünür.
- **HİKÂYE**: kartın hikâyesi, olay başına bir satır, eskiden yeniye, sahibinin gördüğü gibi anlatılır. Her satırda bir işaret ve bir etiket vardır: `◎` niyet, `⚑` varsayım, `⚖` karar, `⇄` konuşma (bir mesaj, türüyle), `•` kanıt (bir çalıştırma ve nasıl bittiği), `Δ` değişiklik, `›` sen (onun hakkında söylediğiniz). Henüz kimsenin almadığı bir kart `Henüz hikâye yok.` der.

Karttan sizi bekleyenler ana ekrandadır, satırları kartı adlandırır; günlüğün onunla ilgili satırları `:export`'a girer (aşağıda).

## Kart ekranındaki tuşlar

Her tuş sahibinin odağında yaptığını yapar: `tab` `⇧tab` öbür sekmelerine geçer (diff'i, konuşması), `enter` ham akışını açar, `a` `e` `A` `r` açık isteğine karar verir, `t` ve `v` izini ya da konuşmasını açar, `@` ona yazar, `W` her yerdeki bekleyen sıradakine gider.

| Tuş | Ne yapar |
| --- | --- |
| `n` / `p` | açık bir projenin sonraki / önceki kartı, numarayla |
| `#` | başka bir kart (menü boş başlar) |
| `esc` | buradan açılan bir iz ya da diff'ten karta geri; kartta ana ekrana geri |
| `^o` | olduğunuz yere geri; kart ekranı da yerlerden biridir |
| `:export [YOL]` | kartın ekranı Markdown dosyası olarak |
| `:home` `:salon` | kartı bırakıp ana ekrana ya da onun ÇALIŞAN'ına |

Ana ekranın katlama tuşları (`h` `l`, `za` `zM` `zR`) burada bir şey yapmaz.

## Dışa aktarma

Bir kartın ekranında (üstünde konuşma açık değilken) `:export` şunu yazar: `# #3 Parse the manifest`, projesi, sütunu ve dakikaları, lease'leri, `## KİM`, `## HİKÂYE` (satır başına bir madde), `## Karar bekleyenler` ve `## Defter`: kartın her satırı, eskiden yeniye, hiçbiri katlanmamış. Her saat gününü taşır, çünkü dosya ekranı terk eder. Varsayılan ad imece'nin başlatıldığı dizinde `imece-card-3-20261031-1401.md`; var olan bir dosyanın üzerine asla yazılmaz.

## Henüz yok

`#` menüsü kartları yalnız numara ve başlıkla bulur; `H` (`:recall`) her günlük ve iz satırının sözcüklerinde arar. Bir kartın kendi `Δ` satırı günlükte (bitirdi, uygulandı) kartı adlandırmaz, bu yüzden kartın dışa aktarımına yalnız karar bekleyen ögeleri yoluyla girer.

## Ayrıca bakınız

- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Pano ve kod haritası](board-and-code-map.md)
- [Günlük](journal.md)
- [Komut satırı](command-line.md)
- [Klavye](keyboard.md)
