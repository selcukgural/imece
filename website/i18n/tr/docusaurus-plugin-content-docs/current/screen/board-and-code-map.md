---
title: Pano ve kod haritası
description: Ekranın üstünde iki pencere - her kartı kendi sütununda gösteren pano ve sembol lease'leri gelince kimin neyi düzenlediğini gösteren kod haritası.
---

# Pano ve kod haritası

Ekip takımı gösterir; pano işi, kod haritası dosyaları gösterir. `:board` ve `:code` onları ekranda ne varsa onun üstünde bir pencerede açar; `esc` (ya da `q`) pencereyi kapatır. Açıkken her tuşu alır, bu yüzden `W`, `/` ve gerisi kapanmasını bekler; `?` yardımı üstünde açar. İkisinin de kendi tuşu yoktur.

## Pano

```text
┌ YAPILACAK (2) ──────┐┌ ÜZERİNDE (2) ───────┐┌ İNCELENECEK (1) ────┐┌ DOĞRULANIYOR (0) ───┐┌ BİTTİ (2) ──────────┐
│#6  Legacy parser    ││#3  Parse the manif… ││#4  Retry budget     ││                     ││#2  Money rounding   │
│#7  Changelog entry  ││    ● claude-1 ▰▰▰▱▱ ││    claude-2         ││                     ││    claude-2         │
│                     ││#5  Retry on timeout ││                     ││                     ││#1  CI cache         │
│                     ││    ● claude-3 ▰▱▱▱▱ ││                     ││                     ││    claude-1         │
└─────────────────────┘└─────────────────────┘└─────────────────────┘└─────────────────────┘└─────────────────────┘
```

Beş sütun, her biri sayısıyla başlıklı:

| Sütun | İçindeki kartlar |
| --- | --- |
| YAPILACAK | asılmış, henüz alınmamış: liderin `post`'u ya da liderin henüz boşta olmayan bir işçi için yazdığı kart |
| ÜZERİNDE | alınmış: lider görev vermiş ya da boşta bir işçi panodan almış. Başlığın altında sahibi, sahibinin durum işaretiyle ve bir ilerleme çubuğuyla; çalışan bir sahibin işareti ekipteki gibi nefes alır (`:motion`) |
| İNCELENECEK | bir `done` ile bitmiş ve değişikliği (`Δ`) KARAR BEKLEYEN'de sizi bekliyor, en yeni önce: ajan işini bitirdi, iş henüz içeri girmedi. Geri gönderilince kart ÜZERİNDE'ye döner |
| DOĞRULANIYOR | Lonca'da, değişikliği onayınızdan sonra merge queue'da |
| BİTTİ | değişikliği tamamen içeride (ya da `done`'ı alınacak bir şey bırakmadı), en yeni önce, bitiren sahibiyle; başarısız olan bir kart da burada durur, `✕` ile. Sahibinin durumunu yalnız üzerinde çalışılan kart gösterir: son iki sütunda sahibi başka bir iş yapıyor olabilir |

Her kart `#N` ve başlığıdır, ikinci denemesinden itibaren ardından ` · 2. deneme` ([kartın denemeleri](../concepts/messages-and-board.md#kartın-denemeleri)). Birden çok proje açıkken projenin adı soluk olarak izler. Kartlar modelde ve depoda yaşar, bu yüzden pano yeniden başlatmadan sonra da aynıdır.

Kartın ne olduğu, liderin nasıl astığı ve görev verdiği, bir işçinin nasıl aldığı [mesajlar ve pano](../concepts/messages-and-board.md) sayfasında. Panonun kendi tuşu yoktur: listeler; bir kartı açmak için her yerden `#N`, sahibine yazmak için `@name text`.

## Kod haritası

`:code`, KOD HARİTASI · proje · kim neyi düzenliyor penceresini açar. Projenin her dosyası için bir satır ve o satırda dosyada tutulan her sembol lease'i için bir hücre göstermek üzere tasarlandı:

```text
┌ KOD HARİTASI · shop-api · kim neyi düzenliyor ──────────────────────────────┐
│src/billing/                                                                  │
│  Money.cs             ● claude-2    Round         ● claude-1   Parse   ⇆ 2  │
│  Invoice.cs           ● claude-2    Total                                   │
│src/parser/                                                                   │
│  Manifest.cs          ● claude-1    Read                                    │
│  Legacy.cs                                                                  │
│3 düzenleniyor · 1 dosyada birden çok agent aynı anda çalışıyor   ● çalışıyor  ⇆ aynı dosyada birden çok agent
└──────────────────────────────────────────────────────────────────────────────┘
```

- Dosyalar dizinlerinin altında gruplanır, her grubun üstünde soluk bir `dizin/` satırı.
- Bir lease hücresi, tutanın durum işareti (ekibin işareti), adı ve sembol (son parçası, `Money.Round` için `Round`). Her hücre projedeki en geniş ada ve sembole göre doldurulur, bir üst sınırla, böylece bir lease gelip gittiğinde sütunlar zıplamaz.
- Kimsenin tutmadığı dosya soluktur, ardında hiçbir şey yok. Birden çok agent'ın tuttuğu dosya uyarı renginde `⇆ N` ile biter.
- Alt satır neyin düzenlendiğini (lease'leri) ve paylaşılan dosyaları sayar ve yalnız o an haritada olan işaretleri sözcüğe döker: sabit bir lejant, haritanın kullanmaya başladığı işaretleri kaçırıyordu. `⇆` yalnız bir dosya paylaşıldığında açıklanır.

### Harita bugün ne gösterir

[Lonca](../concepts/projects.md#loncada)'daki bir projede harita, daemon'ının tutulduğunu söylediği şeydir, birkaç saniyede bir okunur: yalnız birinin tuttuğu dosyalar, her lease aynı hücrelerde, bekleyen bir değişikliğin lease'leri de aralarında. Bu projenin agent'ı olmayan biri soluk `· ad` olarak görünür. Ortak tutulan bir lease, deponun izin verdiği yerde başkalarının da tutabildiği, `(ortak)` ile biter. Hiçbir şey tutulmuyorsa `AD içinde şu an kimsede lease yok` der, yanıt vermeyen bir daemon da haritayı temizler. Worktree'lerdeki bir projede agent'lar kendi dosyalarında kalır, bu yüzden harita gösterecek bir şey olmadığını söyler; `--demo` haritayı kartlarından çizer.

Yukarıdaki satırlı ve hücreli harita `--demo`'nun gösterdiğidir: projeleri bir dosya listesi taşır ve kartları sembol kiralar, böylece görünümün iş başında görülebildiği yer demodur. Bkz. [demo](../getting-started/demo.md).

## Ayrıca bakınız

- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Bir kartın ekranı](cards.md)
- [Ekip](salon.md)
- [Değişiklikler](../concepts/changes.md)
- [Klavye](keyboard.md)
