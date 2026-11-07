---
title: Zaman şeridi
description: Zaman şeridi penceresi (`:timeline`) - son bir saatte agent başına bir şerit, her sütunda ne yaptığı, her projenin altındaki mesajlar, ölçek ve tuşlar.
---

# Zaman şeridi

Zaman şeridi takımı zaman içinde gösterir: açık projelerin her agent'ı için bir şerit, zaman sağa doğru şimdiye akar, her sütun agent'ın o anda ne yaptığıdır. Kimin yan yana çalıştığı, kimin boşta oturduğu, bir şeyin nerede ve ne zamandan beri takıldığı bir bakışta okunur. `:timeline` onu ekranın üstünde bir pencerede açar; `esc` kapatır. En çok küçük bir takımda işe yarar; çok agent'la şeritler kayar.

```text
┌ ZAMAN ŞERİDİ · bir sütun 5 sn · [ ] ölçek ──────────────────────────────────────────┐
│shop-api            14:00                    14:04                   şimdi           │
│● lead-shop-api     ··········································  koordine ediyor      │
│! claude-1          ●●●░░■■■■■■■■■■■■■■⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑  seni bekliyor        │
│● claude-2          ●●●░░░░░░░▒▒▒▒▒▒▒▒▒▒░░░░▒▒▒⚑░░░░░░▒▒▒▒▒▒▒██  ▸ Run the tests      │
│●   claude-2.1                       ▒▒▒▒████▒▒✓·············  boşta               │
│! claude-3          ·●●●░░⚑⚑⚑⚑⚑⚑▒▒▒▒▒░░░░???░░░░░░░░▒▒▒▒⚑⚑░░?  ▓░░░░ ? → claude-2   │
│  mesajlar          ▁▁▂▁▂▁▁▁▁▁▁▁▁▁▁▂▁▁▂▁▁▁▁▂▂▁▁▁▁▁▁▁▂▁▁▁▁▁▁▂▁                        │
│ █ yazıyor  ▒ okuyor, çalıştırıyor  ░ düşünüyor  ● başlıyor  ? arkadaşına soruyor  ⚑ seni bekliyor  ■ takıldı  ✓ tur bitti  · boşta
└─────────────────────────────────────────────────────────────────────────────────────┘
```

Her proje adıyla ve ilk sütununun, ortadakinin ve `şimdi`nin saatiyle başlar. Sonra lideri, işçileri ve her işçinin yardımcıları (içeriden); her satır agent'ın durum işareti ve adı, şeridi ve ekip satırının şu an söylediği. Projenin altında `mesajlar`: agent'larının her sütunda birbirine kaç mesaj gönderdiği, etkinlik çubuğunun ölçeğinde (`▁` hiç, `▂` bir, `▃` iki, `█`'e kadar).

## Bir sütun ne söyler

| İşaret | Agent |
| --- | --- |
| `█` | dosya yazıyordu (bir düzenleme, bir yazma, bir patch) |
| `▒` | okuyor, arıyor ya da bir şey çalıştırıyordu, ya da araştırıyordu |
| `░` | düşünüyordu: tur içinde, elinde araç çağrısı yok |
| `●` | başlıyordu: oturumu kalkıyordu |
| `?` | bir takım arkadaşına sorup yanıtını bekliyordu |
| `⚑` | sizi bekliyordu: onayınızı bekleyen bir araç çağrısı, size bir soru ya da tur dışında KARAR BEKLEYEN'de bir şeyi |
| `■` | takılmıştı: bloke ya da oturumu durmuş |
| `✓` | bir turun bittiği tick |
| `·` | boştaydı |
| (boş) | bilinen bir şey yok: imece onu ilk görmeden önce |

Bir sütun, tick'lerinin gördüğünün en yükseğini gösterir, tablonun altından yukarıya o sırayla: okuma sütunundaki kısa bir düzenleme `█` olur, içinde bir durma olan sütun `■`. Bir agent çalışırken şeridi, daha eski bir değişikliği sizi bekliyor olsa bile ne yaptığını söyler; bunu ekibin `!`'i söylemeye devam eder.

## Ölçek ve tuşlar

Bir sütun bir tick'tir, 5 saniye; `]` onu 30 saniye, sonra bir dakika yapar, `[` geri. Şeritler son bir saati tutar; yalnız bellekte yaşarlar, yeni bir çalıştırma onları boş başlatır.

| Tuş | Ne yapar |
| --- | --- |
| `:timeline` | pencereyi aç; `esc` `q` kapatır |
| `[`  `]` | bir sütun 5 sn · 30 sn · 1 dk |
| `j` `k`  `gg` `G` | bir agent seç; seçili satır ters ve görünür tutulur |
| `enter` | seçili agent'ın odağı, pencerenin yerinde |
| `@` | seçili agent'a yaz: pencere kapanır, satır açılır |
| `?` | yardım, pencerenin üstünde; pencere diğer her tuşu alır |

Demo şeritleri aynı biçimde doldurur, böylece `imece --demo` zaman şeridini canlı bir çalıştırma gibi gösterir. Bkz. [karar](https://github.com/selcukgural/imece/blob/main/docs/decisions/100-timeline.md).
