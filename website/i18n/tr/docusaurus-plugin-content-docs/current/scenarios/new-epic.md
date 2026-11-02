---
title: "Yeni bir epic"
description: "Bir işi bitirip sonrakine temiz bir ekranda başlamak: :archive, beklemesi ve u, arşivin sakladıkları, yeni projenin neyle başladığı ve ortak kalanlar."
---

# Yeni bir epic

`kule` üzerinde bir haftalık iş bitti: issue'lar düzeldi, değişiklikler uygulandı, liderin yapacak bir şeyi kalmadı. Salon hâlâ işi yapan iki işçiyi gösteriyor, günlük onların konuşmalarıyla, pano kartlarıyla dolu. Sıradaki iş ilgisiz ve ona eskisini atmadan temiz bir ekranda başlamak istiyorsunuz.

## 1. Projeyi arşivleyin

`kule`'den bir satır seçiliyken yazın:

```text
:archive v1 fixes
```

Henüz bir şey olmaz. On saniye boyunca alt satır geri sayar:

```text
 ◷ kule “v1 fixes” adıyla arşivleniyor, 7 sn sonra · u tutar
```

O sürede `u` (ya da `:undo`) her şeyi olduğu gibi bırakır: `kule olduğu gibi kalıyor`. `kule`'nin bir ajanı hâlâ tur ortasındaysa satır neyi yarıda keseceğinizi söyler:

```text
 ◷ kule “v1 fixes” adıyla arşivleniyor, 7 sn sonra; 1 tur yarıda kesilecek · u tutar
```

Arşiv ajanlarını çıkışta olduğu gibi durdurur: süren tur biter, istemi saklanır. Başlıksız arşiv adı ve günleriyle anılır: `kule 2026-10-26 → 2026-11-02`.

## 2. Temiz bir kule

Sayım bitince:

```text
 kule “v1 fixes” adıyla arşivlendi: agent'ları her şeyiyle uyuyor; yerine yenisi açıldı
```

Salon yine `kule`'yi gösterir, `lead`'i tek başına ve boşta: aynı çalışma kopyasında, aynı adla, aynı iğneyle ve aynı `:copy` listesiyle yeni bir proje; lideri aynı satıcı ve modelde. Günlükte tek bir yeni satır vardır, `› kule projesini “v1 fixes” adıyla arşivledin; yerine yeni bir kule açıldı`; eski satırlar, panodaki ve kuyruktaki eski kartlar, SENİ BEKLEYEN'deki eski kalemler ekrandan gitmiştir. `@lead` yeni lidere ulaşır.

Ona ilk günkü gibi yazın:

```text
/ sıradaki epic: depoyu kendi crate'ine taşı
```

Lider yeni bir oturum başlatır. İşe aldığı işçiler eskilerin ardından numaralanır (`claude-3`, yeniden `claude-1` değil); böylece worktree'leri arşivinkilerle asla karşılaşmaz.

## 3. Arşivin sakladıkları

Arşivden hiçbir şey atılmadı:

- lideri ve işçileri, uykuda, her biri satıcı oturumuyla (sürdürmek için), postası, notları ve değişikliğiyle;
- kartları öyküleriyle, günlük satırları (temizlik onları yaşları ne olursa olsun saklar), sizi bekleyen kalemleri.

Hiçbiri uyanmaz, hiçbir şey harcamaz: işçilerine kart sunulmaz, onlara adla ulaşılmaz. `imece --fresh` oturumlarına dokunmaz. Oturumları satıcılar saklar ve Claude Code eski transkriptleri bir süre sonra siler (varsayılan 30 gün); o zamana dek gitmiş bir oturum, kaldığı yer anlatılarak yeniden başlar.

Şimdilik arşiv saklanır, gösterilmez: ona bakmak ve onu geri getirmek sonraki adımlardır.

## 4. Ortak kalanlar

Bütçe tektir: arşivlemek hiçbir şey harcamaz, hiçbir şeyi sıfırlamaz. Başlık hâlâ bütün projelerin harcadığını söyler, `:commit`'in günlük tavanları günü saymaya devam eder. Diğer açık projelere dokunulmaz; bir arşiv tek bir projenindir.

## Ayrıca bakınız

- [Oturumlar ve kalıcılık: arşivler](../concepts/sessions.md#arşivler)
- [`:archive`](../reference/commands.md#archive-title)
- [Projeler](../concepts/projects.md)
- [Üç çalışma kopyası, tek ekran](many-projects.md)
