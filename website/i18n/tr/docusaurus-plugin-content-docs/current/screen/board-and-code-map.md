---
title: Pano ve kod haritası
description: Salonun diğer iki görünümü - her kartı sütununda gösteren pano ve sembol lease'leri geldiğinde kimin nerede çalıştığını gösterecek kod haritası.
---

# Pano ve kod haritası

Salon takımı gösterir; pano işi, kod haritası dosyaları gösterir. `2` (`:board`) ve `3` (`:code`) onları salonun yerinde açar; `1` (`:salon`) geri getirir. İkisi de başlık satırını ve alt satırı korur; her yerde çalışan her tuş (`W`, `?`, `:` komutları, yazmak) onlarda da çalışır.

## Pano

```text
┌ BEKLİYOR (2) ─────────┐┌ ÜZERİNDE (2) ─────────┐┌ DOĞRULANIYOR (0) ────┐┌ BİTTİ (3) ───────────┐
│#6  Legacy parser      ││#3  Parse the manifest ││                      ││#2  Money rounding    │
│#7  Changelog entry    ││    ● claude-1 ▰▰▰▱▱   ││                      ││    ○ codex-2         │
│                       ││#5  Retry on timeout   ││                      ││#1  CI cache          │
│                       ││    ● gemini-3 ▰▱▱▱▱   ││                      ││    ○ claude-1        │
└───────────────────────┘└───────────────────────┘└──────────────────────┘└──────────────────────┘
```

Dört sütun, her biri sayısıyla başlıklı:

| Sütun | İçindeki kartlar |
| --- | --- |
| BEKLİYOR | asılmış, henüz alınmamış: liderin `post`'u ya da liderin henüz boşta olmayan bir işçi için yazdığı kart |
| ÜZERİNDE | alınmış: lider görev vermiş ya da boşta bir işçi panodan almış. Başlığın altında sahibi, sahibinin durum işaretiyle ve bir ilerleme çubuğuyla |
| DOĞRULANIYOR | canlıda hiçbir şey doldurmaz. Demonun merge queue'su kullanıyordu; canlıda biten bir kart doğrudan BİTTİ'ye geçer ve değişikliği onun yerine SENİ BEKLEYEN'de bekler |
| BİTTİ | bir `done` ile bitmiş, en yeni önce, bitiren sahibiyle |

Her kart `#N` ve başlığıdır. Birden çok proje açıkken projenin adı soluk olarak izler. Kartlar modelde ve depoda yaşar, bu yüzden pano yeniden başlatmadan sonra da aynıdır.

Kartın ne olduğu, liderin nasıl astığı ve görev verdiği, bir işçinin nasıl aldığı [mesajlar ve pano](../concepts/messages-and-board.md) sayfasında. Panonun kendi tuşu yoktur: listeler; bir kartı açmak için her yerden `#N`, sahibine yazmak için `@name text`.

## Kod haritası

`3`, KOD HARİTASI · proje · kim nerede çalışıyor (lease'ler) görünümünü açar. Projenin her dosyası için bir satır ve o satırda dosyada tutulan her sembol lease'i için bir hücre göstermek üzere tasarlandı:

```text
┌ KOD HARİTASI · shop-api · kim nerede çalışıyor (lease'ler) ─────────────────┐
│src/billing/                                                                  │
│  Money.cs             ● codex-2     Round         ◐ claude-1   Parse   ⇆ 2  │
│  Invoice.cs           ● codex-2     Total                                   │
│src/parser/                                                                   │
│  Manifest.cs          ● claude-1    Read                                    │
│  Legacy.cs                                                                  │
│3 lease · 1 dosyada birden çok agent aynı anda çalışıyor   ● çalışıyor  ◐ soruyor  ⇆ aynı dosyada birden çok agent
└──────────────────────────────────────────────────────────────────────────────┘
```

- Dosyalar dizinlerinin altında gruplanır, her grubun üstünde soluk bir `dizin/` satırı.
- Bir lease hücresi, tutanın durum işareti (salonun işareti), satıcı renginde adı ve sembol (son parçası, `Money.Round` için `Round`). Her hücre projedeki en geniş ada ve sembole göre doldurulur, bir üst sınırla, böylece bir lease gelip gittiğinde sütunlar zıplamaz.
- Kimsenin tutmadığı dosya soluktur, ardında hiçbir şey yok. Birden çok agent'ın tuttuğu dosya uyarı renginde `⇆ N` ile biter.
- Alt satır lease'leri ve paylaşılan dosyaları sayar ve yalnız o an haritada olan işaretleri sözcüğe döker: sabit bir lejant, haritanın kullanmaya başladığı işaretleri kaçırıyordu. `⇆` yalnız bir dosya paylaşıldığında açıklanır.

### Harita bugün ne gösterir

Sembol lease'leri henüz yapılmadı. Bir lease, birkaç agent'ın ortak bir trunk üzerinde tek dosyada, her biri kendi sembollerinde çalışmasına izin verecek; imece, agent'lar dosya paylaştığında lease kodunu Lonca'dan (`lonca-symbols`, `lonca-daemon`) kopyalayacak ve o günden sonra kendisi sahiplenecek. Canlıda her agent kendi git worktree'sinde çalışır ve değişikliğini Δ incelemesiyle teslim eder, bu yüzden ne ortak trunk ne de gösterilecek lease var: harita bunu söyler, `henüz ortak trunk yok: her agent kendi worktree'sinde çalışıyor, gösterilecek lease yok`.

Yukarıdaki satırlı ve hücreli harita `--demo`'nun gösterdiğidir: projeleri bir dosya listesi taşır ve kartları sembol kiralar, böylece görünümün iş başında görülebildiği yer demodur. Bkz. [demo](../getting-started/demo.md).

## Ayrıca bakınız

- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Bir kartın ekranı](cards.md)
- [Salon](salon.md)
- [Değişiklikler](../concepts/changes.md)
- [Klavye](keyboard.md)
