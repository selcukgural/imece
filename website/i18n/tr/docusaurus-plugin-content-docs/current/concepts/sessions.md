---
title: Oturumlar ve kalıcılık
description: imece çalıştırmalar arasında SQLite dosyasında neyi hatırlar, bir ajan nasıl uyur ve uyanır, her satıcının oturumu nasıl devam ettirilir ya da yeniden başlatılır, :forget ve --fresh, saklama süresi ve "nerede kalmıştın" penceresi.
---

# Oturumlar ve kalıcılık

imece'den çıkıp yeniden başlatın: aynı ekran geri gelir, her ajan uykuda; bir ajana ilk mesaj onu uyandırır, satıcısı hâlâ tutuyorsa sahip olduğu oturumda. Bu sayfa neyin, nerede saklandığını ve her satıcının neyi devam ettirebildiğini söyler.

## Depo

imece'nin bildiği her şey tek bir SQLite dosyasındadır: `$XDG_STATE_HOME/imece/imece.sqlite` (varsayılan `~/.local/state/imece/imece.sqlite`), mod `0600`, WAL, yalnız sizin:

- projeler (checkout'u, sabitlemesi ve `:copy` listesiyle), ajanlar (modeli, kademesi, hatırlanan oturumuyla), kartlar ve hikâyeleri, günlük, gelen kutusu, iz (ajan başına en yeni 200 satır), bekleyen posta, pano ve teklifleri, sizi bekleyen değişiklikler (ağaçlarındaki çalıştırmalarla), bütçe ve imece'nin kendi ücretli çağrıları, tercihler (tema, yan panel genişliği, `:lead` için birincil satıcı).
- Her adımdan ve her tuştan sonra ve çıkışta bir kez daha kayıt çalışır, yalnız değişeni yazar: yeni günlük, hikâye ve iz satırları eklenir; bir satır yalnız hash'i değişince yeniden yazılır. Başarısız bir kayıt bir kez gösterilir (`sonraki çalışma için kaydedilemedi: …`).
- Dosyayı **bir anda tek imece** tutar (yanında bir `.lock`): ikincisi reddedilir ve ilkinden `:open` kullanması söylenir.
- imece'nin açamadığı ya da yükleyemediği bir dosya (veritabanı değil, daha yeni bir imece'nin şeması, bilmediği bir satır) reddedilir ve olduğu gibi bırakılır; ileti dosyayı adlandırır ve nasıl sıfırdan başlanacağını söyler (`-wal` ve `-shm`'siyle birlikte kenara taşıyın). imece onu kendi başına asla yedeklemez ya da sıfırdan başlamaz; daha yeni bir imece hâlâ okuyabilir.

## Uyku ve uyanış

Sonraki başlangıçta her ajan `◦ önceki çalışmadan beri uyuyor, yazınca uyanır` olarak gelir: siz ona yazana, bir ekip arkadaşının uyandıran mesajı gelene ya da bu çalıştırmada asılan bir kart ona teklif edilene dek süreç yok, maliyet yok. Yeniden başlatmayı bekleyen posta ve kartlar kimseyi uyandırmaz.

imece kapanırken yarıda olan bir tur günlüğe `yarım kaldı` yazılır ve kartta işaretlenir. İstemi **yeniden çalıştırılmaz**; ajan uyanınca ondan haber alır.

## Satıcı başına devam

Uyanan bir ajanın eski oturumunda devam mı edeceği yoksa yeni mi başlayacağı satıcısının neyi tuttuğuna bağlıdır. Günlük hangisi olduğunu söyler: `bildiği her şeyle, eski oturumunda devam ediyor` ya da `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`.

| Satıcı | Ne zaman devam eder | Nasıl |
| --- | --- | --- |
| Claude Code | hatırlanan kimlik bir UUID ve transkript dosyası Claude Code'un ajanın dizini için tuttuğu yerde (`~/.claude/projects/<dizin>/<id>.jsonl`) | aynı bayraklarla `claude -p --resume ID`. Oturum bütün oturumun maliyetini bildirir, bu yüzden imece önceden yazılandan saymaya başlar (maliyet tabanı). Özet kısadır: yalnız kesilen tur ve bekleyen posta. |
| Codex | hatırlanan thread kimliği bir UUID | `thread/resume`; yanıt istenen kimliği taşımalıdır. Rollout'u gitmiş bir thread Codex tarafından herhangi bir turdan önce reddedilir ve imece tam özetle yeniden başlatır. Devam eden bir thread başladığı talimatları korur; o zamandan beri değişen bir rol istemi ona yalnız imece'nin notlarıyla ulaşır. |
| Gemini CLI | hatırlanan oturum kimliği bir UUID | `session/load`; geçmiş yanıtın etrafında yeniden akıtılır ve okunmadan geçer. Gemini'nin sohbeti olmayan bir kimlik herhangi bir turdan önce reddedilir ve imece yeniden başlatır. Gemini sohbet başına en çok 50 mesaj tutar, uzun bir oturum kuyruğuyla devam eder; `GEMINI.md` yeniden okunur. |

Başarısız bir devam (satıcı oturumun gittiğini söyler ya da başka bir kimlik bildirir) tam uyanış özetiyle ([mesajlar](../concepts/messages-and-board.md)) tek bir yeniden başlatmadır, asla döngü değil. Codex thread'leri bunun için saklanır (`ephemeral: false`); Codex'in küresel mesaj günlüğü kapalı kalır.

## `:forget` ve `--fresh`

imece'nin unuttuğu bir oturum satıcısında hiçbir şey bırakmaz:

- `:forget [@AD]` (`:f`): çalışan oturum biter (çalıştırması öldürülür, bitmemiş istemler yarım kaldı diye tutulur ve uyanışta anlatılır), hatırlanan oturum gider, ajan kendisiyle konuşulana dek uyur ve sonraki mesajı tam özetle yeni bir oturum başlatır. Unutacak bir şeyi olmayan ya da gitmiş bir ajan için reddedilir. Günlük: `claude-1 oturumunu unutturdun`. `:` menüsü `$` ile işaretler: yeni oturum bütün özeti yeniden öder.
- `--fresh` depo okunurken hatırlanan her ajan için bunu yapar, ajan başına günlüğe yazılır.
- Kaldırılan, kendi iş parçacığında: Claude'un transkript dosyası; Codex'in thread'i onun için başlatılan bir app server üzerinden (`thread/delete`, Codex'in dizinini de temizler); Gemini'nin o oturumun sohbet dosyaları `~/.gemini/tmp/<ad>/chats/` altında ve ajanın kendisi gidince (`:close`, bir yardımcının ayrılışı) o ad altındaki her şey artı özel dizini. Bir yola ya da satıcıya yalnız bir UUID ulaşır.
- imece'nin kendi `/clear`'ı, bir model geri düşmesi ve yenilenen bir oturum (`:budget`'tan sonra) değiştirilen oturumu aynı yolla unutur.

## Kalanlar: günler, saklama, tercihler

- **Günler:** saat dünyanın ilk başlangıcının yerel saatinde başlar. Günlük ve bir konuşma başka bir günün ilk satırının üstüne soluk bir `─ 2026-11-01` satırı çizer; SENİ BEKLEYEN'de başka bir günden bir öğe tarihini taşır; kopyalanan ve dışa aktarılan metin her zaman damgalıdır.
- **Saklama:** her başlangıçta 90 günden eski ve en yeni 5000'in gerisinde kalan günlük satırları silinir ve dosyanın çeyreği ya da fazlası boşsa `VACUUM` çalışır. Hikâyeler ve gelen kutusu satırları kalır. Başarısız bir temizlik sonraki başlangıçta yeniden denenir.
- **Tercihler** (tema, yan panel, `:lead`) deponun `meta` tablosunda yaşar, depo alınmadan önce okunabilir; canlı bir imece'nin yanındaki demo onları paylaşır.

## Nerede kalmıştın

Önceki bir çalıştırmayı hatırlayan bir başlangıç, bir şey hareket etmeden önce bir pencere açar, NEREDE KALMIŞTIN · 3 sa 20 dk önce:

```text
 NEREDE KALMIŞTIN · 3 sa 20 dk önce
 $12.40 harcandı, bütçe $50.00; her agent kendisiyle konuşulana dek uyur
 imece: 4 agent, 2 açık kart
 2 şey seni bekliyor:
   Δ claude-1 bitirdi: parser tokens (3 dosya, +34 −8)
   ? codex-1 sana soruyor: eski CLI bayrağı kalsın mı?
 imece kapanırken kesilen turlar: gemini-1
 kapanmadan önceki son satırlar:
   14:02 claude-1 bitirdi: 3 dosya, +34 −8 → seni bekliyor
   14:05 codex-1 → sen: soru eski CLI bayrağı kalsın mı?
 herhangi bir tuş: kapat
```

Bütçeyi, açık proje başına bir satırı, en eski açık öğeleri (en çok 6, sonra `… ve N daha`), önceki çalıştırmanın kestiği turları ve önceki çalıştırmanın son 6 günlük satırını gösterir. Herhangi bir tuş kapatır ve başka bir şey yapmaz. Anlatacak bir şey yoksa pencere yok; demoda hiç yok. Aynı konu [ertesi sabah](../scenarios/next-morning.md) sayfasında.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md): depo ve git iş parçacığı.
- [Dosyalar](../reference/files.md): imece'nin yazdığı her yol.
- [Bütçe ve limitler](../concepts/budget-and-limits.md): bütçe kalıcıdır.
- [Sorun giderme](../troubleshooting.md): açılmayan bir depo, ikinci bir imece.
