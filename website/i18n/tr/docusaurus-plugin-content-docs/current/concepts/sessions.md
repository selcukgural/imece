---
title: Oturumlar ve kalıcılık
description: imece çalıştırmalar arasında SQLite dosyasında neyi hatırlar, bir ajan nasıl uyur ve uyanır, bir oturum nasıl devam ettirilir ya da yeniden başlatılır, :forget ve --fresh, saklama süresi ve "nerede kalmıştın" penceresi.
---

# Oturumlar ve kalıcılık

imece'den çıkıp yeniden başlatın: aynı ekran geri gelir, her ajan uykuda; bir ajana ilk mesaj onu uyandırır, Claude Code hâlâ tutuyorsa sahip olduğu oturumda. Bu sayfa neyin, nerede saklandığını ve neyin devam ettirilebildiğini söyler.

## Depo

imece'nin bildiği her şey tek bir SQLite dosyasındadır: `$XDG_STATE_HOME/imece/imece.sqlite` (varsayılan `~/.local/state/imece/imece.sqlite`), mod `0600`, WAL, yalnız sizin:

- projeler (checkout'u, sabitlemesi ve `:copy` listesiyle), ajanlar (modeli, kademesi, hatırlanan oturumuyla), kartlar ve hikâyeleri, günlük, gelen kutusu, iz (ajan başına en yeni 200 satır), bekleyen posta, pano ve teklifleri, sizi bekleyen değişiklikler (ağaçlarındaki çalıştırmalarla), bütçe ve imece'nin kendi ücretli çağrıları, tercihler (tema, notify, motion, story, review).
- Her adımdan ve her tuştan sonra ve çıkışta bir kez daha kayıt çalışır, yalnız değişeni yazar: yeni günlük, hikâye ve iz satırları eklenir; bir satır yalnız hash'i değişince yeniden yazılır. Başarısız bir kayıt bir kez gösterilir (`sonraki çalışma için kaydedilemedi: …`).
- Dosyayı **bir anda tek imece** tutar (yanında bir `.lock`): ikincisi reddedilir ve ilkinden `:open` kullanması söylenir.
- imece'nin açamadığı ya da yükleyemediği bir dosya (veritabanı değil, daha yeni bir imece'nin şeması, bilmediği bir satır) reddedilir ve olduğu gibi bırakılır; ileti dosyayı adlandırır ve nasıl sıfırdan başlanacağını söyler (`-wal` ve `-shm`'siyle birlikte kenara taşıyın). imece onu kendi başına asla yedeklemez ya da sıfırdan başlamaz; daha yeni bir imece hâlâ okuyabilir.

## Turun ortasında bir ajana yazmak

Bir ajana yazdığınız, tur ortasında bile hemen oturumuna gider. Tur bir aracı beklerken (çalışan bir komut, sizden bir onay) Claude Code sözlerinizi aynı tura alır ve ajan onları orada yanıtlar; değilse ajanın bir sonraki turunu başlatırlar. İki durumda da ajanın satırı, değişikliği ve `done`'u Claude Code'un gerçekte yaptığını izler: imece, Claude Code'un aldığını söylediği istemleri sayar; ikisini alan bir tur ikisini de bitirir.

## Uyku ve uyanış

Sonraki başlangıçta her ajan `◦ önceki çalışmadan beri uyuyor, yazınca uyanır` olarak gelir: siz ona yazana, bir ekip arkadaşının uyandıran mesajı gelene ya da bu çalıştırmada asılan bir kart ona teklif edilene dek süreç yok, maliyet yok. Yeniden başlatmayı bekleyen posta ve kartlar kimseyi uyandırmaz.

imece kapanırken yarıda olan bir tur günlüğe `yarım kaldı` yazılır ve kartta işaretlenir. İstemi **yeniden çalıştırılmaz**; ajan uyanınca ondan haber alır.

## Devam

Uyanan bir ajanın eski oturumunda devam mı edeceği yoksa yeni mi başlayacağı Claude Code'un neyi tuttuğuna bağlıdır. Günlük hangisi olduğunu söyler: `bildiği her şeyle, eski oturumunda devam ediyor` ya da `yeni bir oturumla başlıyor, kaldığı yer anlatıldı`, bir oturumu varsa nedeniyle: `son oturumu bulunamadı` ya da Claude Code'un sözleriyle `son oturumu sürdürülemedi (…)`.

| Ne zaman devam eder | Nasıl |
| --- | --- |
| hatırlanan kimlik bir UUID ve transkript dosyası Claude Code'un ajanın dizini için tuttuğu yerde (`~/.claude/projects/<dizin>/<id>.jsonl`) | aynı bayraklarla `claude -p --resume ID`. Oturum bütün oturumun maliyetini bildirir, bu yüzden imece önceden yazılandan saymaya başlar (maliyet tabanı). Özet kısadır: yalnız kesilen tur ve bekleyen posta. |

Başarısız bir devam (Claude Code oturumun gittiğini söyler ya da başka bir kimlik bildirir) tam uyanış özetiyle ([mesajlar](../concepts/messages-and-board.md)) tek bir yeniden başlatmadır, asla döngü değil.

## API hata verince

Claude Code başarısız bir API çağrısını önce kendisi yeniden dener, on kereye kadar (API aşırı yüklüyse üç); her deneme agent'ın izinde bir satırdır (`api error 529 overloaded: retry 2/10 in 1163 ms`). Tur ancak o pes edince başarısız olur ve imece nedenine bakar:

| Claude Code ne diyor | imece |
| --- | --- |
| aşırı yük, bir sunucu hatası (5xx), kopan bir bağlantı, bir hız sınırı | geçici bir hata: günlük `claude-1 API hatası aldı: … · imece 30 sn sonra yeniden deniyor (1/3)` der, satır geri sayar `↻ 1/3 · 25 sn sonra`, zamanı gelince oturuma kaldığı yerden devam etmesi söylenir. İkinci deneme bir sonraki hatadan 2 dk sonra, üçüncüsü ondan 8 dk sonradır. Üçüncüden sonra sizi bekler. |
| oturum kapalı, reddedilen bir anahtar, biten bir kota, bir fatura, bir planın kullanım sınırı | hesabın işi: hemen sizi bekler, [ana ekranda](../screen/home.md) KARAR BEKLEYEN'de. |
| başka her şey (reddedilen bir istek, dolu bir bağlam) | günlük, önceden olduğu gibi turun durduğunu söyler. |

Oturum başarısız bir turdan sonra da yaşar ve Claude Code başarısız istemi içinde tutar; bu yüzden bir deneme istemi yeniden göndermez, kısa bir "devam et" gönderir. İyi bir tur, sizin mesajınız ya da agent'a gelen posta sayımı sıfırlar; biten bir oturum bekleyen denemeyi de götürür. Demoda bir agent'a söylediğiniz her üçüncü şey aşırı yüklü bir API yüzünden başarısız olur ve yeniden denenir, her yedinci şey reddedilen bir anahtar yüzünden.

## `:forget` ve `--fresh`

imece'nin unuttuğu bir oturum geride hiçbir şey bırakmaz:

- `:forget [@AD]` (`:f`): çalışan oturum biter (çalıştırması öldürülür, bitmemiş istemler yarım kaldı diye tutulur ve uyanışta anlatılır), hatırlanan oturum gider, ajan kendisiyle konuşulana dek uyur ve sonraki mesajı tam özetle yeni bir oturum başlatır. Unutacak bir şeyi olmayan ya da gitmiş bir ajan için reddedilir. Günlük: `claude-1 oturumunu unutturdun`. `:` menüsü `$` ile işaretler: yeni oturum bütün özeti yeniden öder.
- `--fresh` depo okunurken hatırlanan her ajan için bunu yapar, ajan başına günlüğe yazılır.
- Kaldırılan, kendi iş parçacığında: Claude Code'un transkript dosyası. Ajanın karalama dizini de gider (bkz. [dosyalar](../reference/files.md)). Bir yola yalnız bir UUID ulaşır.
- imece'nin kendi `/clear`'ı, bir model geri düşmesi ve yenilenen bir oturum (`:budget`'tan sonra) değiştirilen oturumu aynı yolla unutur.

## Arşivler

Bir epic bitince `:archive [BAŞLIK]` eldeki projeyi kaldırır ve yerine, aynı çalışma kopyasında, aynı adlı yeni bir proje açar ([`:archive`](../reference/commands.md#archive-title)). Arşiv depoda, sahip olduğu her şeyle saklanır:

- ajanları uykuda, her biri hatırlanan oturumu, postası, notları, değişikliği ve yarıda kesilen bir turun istemleriyle (arşiv, süren bir turu çıkışta olduğu gibi yarıda keser);
- kartları, KARAR BEKLEYEN'deki kalemleri ve günlük satırları, gözden uzakta: ekran yalnız yeni projeyi gösterir.

Bir arşivden hiçbir şey uyanmaz, hiçbir şey harcanmaz: işçilerine kart sunulmaz, ajanlarına adla ulaşılmaz (yeni başın adı aynı olabilir) ve birinin arşivden önce yaptığı bir araç çağrısı `your project was put away; nothing was done` diye yanıtlanır. `--fresh` oturumlarına dokunmaz, temizlik de günlük satırlarını yaşları ne olursa olsun saklar. Claude Code eski transkriptleri bir süre sonra kendiliğinden siler (varsayılan 30 gün); o zamana dek gitmiş bir oturum, kaldığı yer anlatılarak yeniden başlar.

`:restore [AD]` birini geri getirir ([`:restore`](../reference/commands.md#restore-name)): kaldırıldığı gibi yeniden açılır, başı seçilir, hiçbir şey uyanmaz. Her ajan, ona bir sonraki yazışınızda oturumunda sürer (Claude Code hâlâ saklıyorsa sürdürülür); önce projenin kaldırıldığı ve arşivin yarıda kestiği tur ona söylenir. Çalışma kopyasında açık olan takım yer açar: bir şey yaptıysa sırası gelince arşivlenir, yapmadıysa kapatılır.

`A` ([`:archives`](../reference/commands.md#archives)) arşivleri bir ağaç olarak listeler: her çalışma kopyası, şu an onda açık projeler, arşivleri en yenisi önce, harcadıkları, biten kartları ve ajanlarıyla; `/` onları süzer, `r` eldekini geri getirir, `enter` onu geri getirmeden salt okunur açar: yalnızca o arşiv, kaldırıldığı gibi, deponun günlüğünden sakladığı her şeyle, bir `ARŞİV · …` şeridinin altında; içinde hiçbir şey uyanmaz, harcamaz, `esc` geri döner.

## Kalanlar: günler, saklama, tercihler

- **Günler:** saat dünyanın ilk başlangıcının yerel saatinde başlar. Günlük ve bir konuşma başka bir günün ilk satırının üstüne soluk bir `─ 2026-11-01` satırı çizer; KARAR BEKLEYEN'de başka bir günden bir öğe tarihini taşır; kopyalanan ve dışa aktarılan metin her zaman damgalıdır.
- **Saklama:** her başlangıçta 90 günden eski ve en yeni 5000'in gerisinde kalan günlük satırları silinir ve dosyanın çeyreği ya da fazlası boşsa `VACUUM` çalışır. Hikâyeler ve gelen kutusu satırları kalır. Başarısız bir temizlik sonraki başlangıçta yeniden denenir.
- **Tercihler** (tema, notify, motion) deponun `meta` tablosunda yaşar, depo alınmadan önce okunabilir; canlı bir imece'nin yanındaki demo onları paylaşır.

## Nerede kalmıştın

Önceki bir çalıştırmayı hatırlayan bir başlangıç, bir şey hareket etmeden önce bir pencere açar, NEREDE KALMIŞTIN · 3 sa 20 dk önce:

```text
 NEREDE KALMIŞTIN · 3 sa 20 dk önce
 $12.40 harcandı, bütçe $50.00; her agent kendisiyle konuşulana dek uyur
 imece: 4 agent, 2 açık kart
 ! 2 şey karar bekliyor:
   Δ claude-1 bitirdi: parser tokens (3 dosya, +34 −8)
   ? claude-2 sana soruyor: eski CLI bayrağı kalsın mı?
 imece kapanırken kesilen turlar: claude-3
 kapanmadan önceki son satırlar:
   14:02 claude-1 bitirdi: değişiklik hazır · 3 dosya +34 −8
   14:05 claude-2 → sen: soru eski CLI bayrağı kalsın mı?
 herhangi bir tuş: kapat
```

Bütçeyi, açık proje başına bir satırı, en eski açık öğeleri (en çok 6, sonra `… ve N daha`), önceki çalıştırmanın kestiği turları ve önceki çalıştırmanın son 6 günlük satırını gösterir. Herhangi bir tuş kapatır ve başka bir şey yapmaz. Anlatacak bir şey yoksa pencere yok; demoda hiç yok. Aynı konu [ertesi sabah](../scenarios/next-morning.md) sayfasında.

## Süreçler

`:ps` her ajanın oturumunun şu an çalıştırdıklarını gösterir: `claude`'u, altındaki imece bus aktarıcısını ve ajanın başlattığı her komutu, `ps`'in söylediği CPU ve bellekle; pencere açıkken üç saniyede bir yeniden okunur. Takılan ya da makineyi yiyen bir komut (hiç bitmeyen bir test, kontrolden çıkmış bir derleme) orada `d` ve `y` ile sonlandırılabilir: yalnızca o süreç, hâlâ o ajanın oturumunun altında olduğu yeniden denetlendikten sonra; oturum sürer ve ajan komutunun hata verdiğini görür. imece burada hiçbir süreci kendiliğinden sonlandırmaz ve başlatmadığı birine asla dokunmaz.

Oturumlarını bitirmeden sona eren bir imece çalışması (bir çökme, kapanan bir terminal, `kill -9`) onları, başlattıklarıyla birlikte, çalışır hâlde bırakabilir. imece açılışta onlara bakar ve `:tidy`'nin worktree'ler için yaptığı gibi kaç tane bulduğunu söyler: `önceki bir çalışmadan kalan 4 süreç var: :ps listeler`. Bunlar, komut satırında imece'nin worktree'lerini anan ve üstünde hiçbir imece olmayan süreçler ile onların altında çalışanlardır; şu an çalışan başka bir imece kendininkileri korur. `:ps` onları kendi başlıkları altında listeler, `d` birini, `a` hepsini sonlandırır, her biri bir `y`'den sonra. Bkz. [`:ps`](../reference/commands.md#ps).

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md): depo ve git iş parçacığı.
- [Dosyalar](../reference/files.md): imece'nin yazdığı her yol.
- [Bütçe ve limitler](../concepts/budget-and-limits.md): bütçe kalıcıdır.
- [Yeni bir epic](../scenarios/new-epic.md): biten bir projeyi arşivlemek.
- [Sorun giderme](../troubleshooting.md): açılmayan bir depo, ikinci bir imece.
