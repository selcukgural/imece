---
title: Ana ekran
description: Görünüm 5 - projeler arası sıra sizde olanlar, takılanlar ve çalışanlar tek listede, eldeki satır hemen yanında okunur.
---

# Ana ekran

Ana ekran, imece'nin açıldığı ve karar verdiğiniz ekrandır; `5` ya da `:home` geri getirir. Ekip takımı gösterir; ana ekran sıranın sizde olduğu şeyleri. Açık projelerin hepsi için tek liste ve yanında eldeki satır, tuşa basmadan okunur.

```text
 hepsi   shop-api !2 ●1   mobile-app !3 ■1 ●2                              / lead-shop-api için yazar
 lonca mobile-app 9d3f1c2 · 3 bekliyor
 KARAR BEKLEYEN 5                                       │#19 Paging in product search  shop-api · claude-1 · $0.90
▸ Δ shop-api    #19  Paging in product… +41 −6 ✓ 38 dk│
  ? shop-api    #31  Refund rules             soru 12 dk│neden    Paging in product search
  ⚑ mobile-app  #52  Notification sett…      izin? 4 dk│kanıt    ✓ bu ağaçta temiz geçti: dotnet test
  Δ mobile-app  #21  Currency support   +6 −3 ✗ 41 dk│maliyet  $0.90 · 11 dk · 2 tur
                                                     │
 TAKILAN 1                                           │dosyalar src/Catalog/ProductQuery.cs    +19 −6
  ■ mobile-app  #14  Price list cache    durdu 52 dk│         tests/PagingTests.cs           +18 −0
                                                     │
 ÇALIŞAN 3                                           │@@ -11,3 +11,4 @@
  ● shop-api    #33  Order validation  Write the tests 2 dk│-    var rows = db.Skip(q.Offset);
```

İlk satır açık projelerin her birini tuttuklarıyla adlandırır: `!` sizi bekliyor, `■` takılı, `●` çalışıyor. Daraltılmış proje aydınlıktır, satırın sonu `/` ile kime yazılacağını söyler. İkinci satır, Lonca'daki projelerin nasıl durduğunu söyler.

## Liste

| Grup | İçindekiler |
| --- | --- |
| KARAR BEKLEYEN | KARAR BEKLEYEN'deki her şey, `W`'nin sırasıyla |
| TAKILAN | bloke olmuş ya da durmuş ajanlar |
| ÇALIŞAN | tur ortasındaki ya da bir takım arkadaşına soran ajanlar. Değişikliği bekleyen ve başka işe geçmiş bir işçi burada da görünür: iki ayrı iş. Sizden evet ya da yanıt bekleyen ajan ise o ögenin kendisidir |
| SESSİZ | geri kalanlar, işi olmayan liderler hariç |

Satırda önce iş gelir: işareti, projesi (birden çok proje açıksa), kartı ve başlığı, sonra ne olduğu ve bir dakikadan sonra ne zamandır durduğu. Değişiklik satırları ve çalıştırmaların sonucuyla biter; soru `soru` der; ajan bulunduğu adımı söyler. Ajanın adı önizlemededir.

## Önizleme

Eldeki satır, tamamıyla: bir sorunun ya da isteğin sözleri; bir değişikliğin başlığı, kanıtı, maliyeti, geçidi, dosyaları ve git'in kendi satırları olmadan yamasının başı; bir ajanın durumu, yapılacakları ve en yeni iz satırları.

100 kolondan itibaren listenin yanında durur. Daha dar ekranda, editörün yanında, liste tüm genişliği alır ve eldeki satır kendi altında açılır.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | gezer; önizleme izler |
| `[` `]` | tüm projeler, sonra her biri, dönerek |
| `enter` | açar: değişikliğin diff'i, sorunun konuşması, ajanın odağı. `esc` buraya döner |
| `a` `r` | evet ve hayır, [KARAR BEKLEYEN](waiting.md)'deki gibi. Soruda `a` yanıt yazar |
| `e` `A` | bir istekte: bu tur evet · bundan sonra evet |
| `s` | sonra: öge diğerlerinin arkasına gider, listede kalırsınız |
| `/` | eldeki projenin liderine yazar; ilk satır kim olduğunu söyler |
| `@` | eldeki ajana yazar |
| `W` | sıradaki ögeyi açar |

Diğer tuşlar her yerdeki gibi çalışır; durum satırının tuşları eldeki satırınkilerdir.

Karar asla okumadığınız şeye düşmez: `a` ya da `r`'ye bastığınızda eldeki satır gitmişse tuş hiçbir şey yapmaz, bir satır bunu söyler ve yerine gelen satır elinize geçer.

Ekip (bir başlangıcın notlarının düştüğü günlükle), pano, kod haritası ve zaman şeridi yerinde, bir tuş uzakta. Tuş dosyasının `[ekip]` bölümü ana ekranda da geçerlidir.
