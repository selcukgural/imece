---
title: "Yeni bir epic"
description: "Bir işi bitirip sonrakine temiz bir ekranda başlamak: :archive, beklemesi ve u, arşivin sakladıkları ve ona salt okunur bakmak, yeni projenin neyle başladığı, :restore ile geri getirmek ve ortak kalanlar."
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

Salon yine `kule`'yi gösterir, `lead`'i tek başına ve boşta: aynı çalışma kopyasında, aynı adla, aynı iğneyle ve aynı `:copy` listesiyle yeni bir proje; lideri aynı modelde. Günlükte tek bir yeni satır vardır, `› kule projesini “v1 fixes” adıyla arşivledin; yerine yeni bir kule açıldı`; eski satırlar, panodaki ve kuyruktaki eski kartlar, SENİ BEKLEYEN'deki eski kalemler ekrandan gitmiştir. `@lead` yeni lidere ulaşır.

Ona ilk günkü gibi yazın:

```text
/ sıradaki epic: depoyu kendi crate'ine taşı
```

Lider yeni bir oturum başlatır. İşe aldığı işçiler eskilerin ardından numaralanır (`claude-3`, yeniden `claude-1` değil); böylece worktree'leri arşivinkilerle asla karşılaşmaz. İkincisini almadan önce planı SENİ BEKLEYEN'e `▤` olarak gelir: `enter` kartları ve beklenen maliyeti okur, `a` devam ettirir ([seni bekleyen](../screen/waiting.md)).

## 3. Arşivin sakladıkları

Arşivden hiçbir şey atılmadı:

- lideri ve işçileri, uykuda, her biri oturumuyla (sürdürmek için), postası, notları ve değişikliğiyle;
- kartları öyküleriyle, günlük satırları (temizlik onları yaşları ne olursa olsun saklar), sizi bekleyen kalemleri.

Hiçbiri uyanmaz, hiçbir şey harcamaz: işçilerine kart sunulmaz, onlara adla ulaşılmaz. `imece --fresh` oturumlarına dokunmaz. Oturumları Claude Code saklar ve eski transkriptleri bir süre sonra siler (varsayılan 30 gün); o zamana dek gitmiş bir oturum, kaldığı yer anlatılarak yeniden başlar.

Geri getirmeden yeniden bakmak için `A`'ya basın ve `▸ v1 fixes` üstünde `enter`: arşiv salt okunur açılır, ekranda kaldırıldığı gibi tek başına, bütün günlüğüyle, şunun altında

```text
 ARŞİV · kule “v1 fixes” · 2026-11-02 arşivlendi · şu an seni 0 şey bekliyor · esc geri
```

Konuşmaları, kartları ve değişiklikleri her yerdeki gibi açılır, `:export` birini dosyaya yazar; ajanlarına yazmak `arşiv salt okunur · :restore geri getirir · esc geri döner` der. `esc` bu arada sürmüş olan yeni `kule`'ye döner.

## 4. Geri getirmek

Bir ay sonra eski işte bir hata çıkar ve o kodu `kule`'nin eski takımı bilir. Yazın:

```text
:restore v1 fixes
```

(`:restore ` ve `tab` arşivleri listeler, en yenisi önce: `v1 fixes   kule · 2026-11-02`. Ya da `A`'ya basın: arşivler bir ağaç olarak, önce `kule`'nin çalışma kopyası, `▸ v1 fixes · $6.10 · 4 kart bitti · 3 agent · 2026-11-02 arşivlendi` satırının üstünde `● kule · şu an açık`; `l` ajanlarını gösterir, `/fix` süzer, `r` eldekini geri getirir.) Arşiv kaldırıldığı gibi yeniden açılır; `lead`'i seçilir, kartları, günlük satırları ve bekleyen kalemleri ekrana döner:

```text
 kule “v1 fixes” geri geldi: agent'ları sen yazana kadar uyuyor
```

Açık olan `kule` yer açar: bir şey yaptı (bir oturumu ve bir kartı vardı), bu yüzden sırası gelince adı ve günleriyle arşivlenir; günlük de bunu söyler: `› kule projesinin “v1 fixes” arşivini geri getirdin; orada açık olan “kule 2026-11-02 → 2026-12-02” adıyla arşivlendi`. Hiçbir şey yapmamış yeni bir `kule` ise kapatılırdı.

Henüz hiçbir şey uyanmadı. Eski lidere yazın; sahip olduğu oturumda sürer, önce projenin kaldırılıp geri getirildiği ve arşivin yarıda kestiği tur (yeniden çalıştırılmaz) ona söylenir. Claude Code'un bu arada sildiği bir oturum, kaldığı yer anlatılarak yeniden başlar. Daha yeni işe dönmek için onu da aynı biçimde `:restore` edin.

## 5. Ortak kalanlar

Bütçe tektir: arşivlemek hiçbir şey harcamaz, hiçbir şeyi sıfırlamaz. Başlık hâlâ bütün projelerin harcadığını söyler, `:commit`'in günlük tavanları günü saymaya devam eder. Diğer açık projelere dokunulmaz; bir arşiv tek bir projenindir.

## Ayrıca bakınız

- [Oturumlar ve kalıcılık: arşivler](../concepts/sessions.md#arşivler)
- [`:archive`](../reference/commands.md#archive-title)
- [`:restore`](../reference/commands.md#restore-name)
- [Projeler](../concepts/projects.md)
- [Üç çalışma kopyası, tek ekran](many-projects.md)
