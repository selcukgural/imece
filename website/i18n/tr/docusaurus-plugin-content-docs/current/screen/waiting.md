---
title: SENİ BEKLEYEN
description: Yalnız sizin verebileceğiniz her kararı tutan panel - incelenecek değişiklikler, izin verilecek araç çağrıları, yanıtlanacak sorular - ve içinde ilerleyen tuşlar.
---

# SENİ BEKLEYEN

SENİ BEKLEYEN (İngilizce ekranda WAITING FOR YOU) salonun üst yan panelidir. Oraya yalnız sizi gerektiren kararlar düşer; geri kalan her şey günlükte kalır. Onunla ilk kez bir işçi `done` gönderdiğinde karşılaşırsınız: değişikliği (`Δ`) görünür, başlık `! 1 seni bekliyor` sayar ve terminalin başlığı `imece · 1 seni bekliyor` der. Başka bir penceredeyken `:notify bell` terminalin zilini çalar, `:notify text` masaüstü bildirimi gönderir (`imece · claude-1 bitirdi: bir değişiklik seni bekliyor`), yeni bir şey beş saniyedir bekliyorsa; makineden uzaktayken `:notify push` bunu ntfy üzerinden telefonunuza gönderir; siz istemedikçe kapalıdır, bkz. [`:notify`](../reference/commands.md#notify-offbelltextpush).

```text
┌ SENİ BEKLEYEN (3) ─────────────────────────────────────────────────┐
│Δ 13:58 claude-1 bitirdi: parser (3 dosya, +41 −6) · ✓ bu ağaçta temiz geçti: cargo test
│  enter diff'i gör · a checkout'a uygula · r reddet                   │
│⚑ 14:01 claude-2 onayını bekliyor: Bash: npm install left-pad        │
│? 14:03 sonra · claude-3 sana soruyor: Eski CLI bayrağı kalsın mı?   │
└─────────────────────────────────────────────────────────────────────┘
```

## Oraya ne düşer

| İşaret | Öge | Nereden gelir | `a` | `r` |
| --- | --- | --- | --- | --- |
| `Δ` | bir değişiklik | bir işçinin `done`'u: worktree'sinin HEAD'inize göre bütün değişikliği, imece okur. Satır done'un ilk satırını, dosya ve satır sayılarını ve imece'nin `run` aracının tam bu ağaçta ne çalıştırdığını söyler (`✓ bu ağaçta temiz geçti`, `✗ başarısız`, ya da `bu ağaçta çalıştırma yok`). | checkout'unuza uygula, stage'de | reddet; nedenini yazmanız için yazma satırı `@worker ` ile açılır |
| `⚑` | bir izin isteği | Claude Code'un izin soracağı bir araç çağrısı. Olağan komutlar (derleme, testler, git'in okuma komutları) kendiliğinden çalışır ve buraya hiç gelmez. Seçili istek satırının altında bütünüyle gösterilir, satırları ` ↵ ` ile birleştirilmiş, asla Markdown olarak okunmaz: izin verdiğiniz şey çalışan şeydir. | izin ver | reddet; agent'a bunun sizin kararınız olduğu söylenir |
| `?` | bir soru | bir agent'ın `send(to: "you", type: ask)` çağrısı. Aynı agent'ın yeni sorusu eskisinin yerini alır. | ikisi de yazma satırını `@agent ` ile açar; yanıtınız gidince öge kapanır | |
| `»` | bir yanıt | bir agent size yanıt verdi ve okumanızı bekliyor | `enter` okur (konuşma açılır) ve kapatır; `r` kapatır | |
| `✕` | bir API hatası | bir tur yalnız sizin giderebileceğiniz bir hatayla bitti: hesap (oturum kapalı, reddedilen bir anahtar, bir kota ya da fatura) ya da imece'nin üç denemesi tükendikten sonra geçici bir hata (aşırı yük, bir sunucu hatası, kopan bir bağlantı) (bkz. [API hata verince](../concepts/sessions.md#api-hata-verince)). Satırda Claude Code'un kendi sözleri yazar. | ikisi de yazma satırını `@agent ` ile açar; mesajınız gidince öge kapanır ve o mesaj agent'ın bir sonraki denemesidir | |
| `▶` | bir kurulum betiği | yeni bir worktree checkout'unuzda evet demediğiniz baytlarla `.imece/worktree-setup.sh` buldu; işçi onsuz başladı. Bkz. [worktree kurulum betiği](../concepts/projects.md#worktree-kurulum-betiği). | evet: kaçırıldığı yerde hemen, sonra her yeni worktree'de çalışır; `enter` önce baytlarını gösterir | hayır, bu çalıştırma için |
| `▤` | bir plan | liderin ikinci işçiyi almadan önceki `plan` aracı: görev vermeyi ya da panoya asmayı düşündüğü kartlar, her biri bir kademe ve sahip olduğu yollarla, ve beklediği maliyet. `enter` tamamını, maliyetin yanında bütçeyle gösterir; bu arada liderin turu bitmiştir ve postası bekler ([plan](../concepts/messages-and-board.md#plan); liderine güvenen bir proje için `:plan off`). | evet: lider, sonraki planına dek, planladığı gibi işe alır ve görev verir | hayır: plan düşer ve lider size sorar ya da yeniden planlar; `n` onun yerine lidere yazar ve yazdığınız planı yanıtsız kapatır |
| `▶` | deponun kontrolü | bir işçinin değişikliği okundu ve checkout'unuzda evet demediğiniz baytlarla `.imece/check` var; değişiklik bu arada kontrolsüz bekler. Bkz. [deponun kontrolü](../concepts/changes.md#deponun-kontrolü). | evet: önce onu bekleyen değişikliklerde, sonra okunan her değişiklikte çalışır; `enter` önce baytlarını gösterir | hayır, bu çalıştırma için |

Satır saati taşır, ertelediyseniz `sonra ·`, birden çok proje açıksa projenin adını. Başka bir günün ögesi tarihini de taşır.

Bir değişiklik (`Δ`) işçisine notlarla (diff'te `n`) ya da reddedilen bir uygulamadan sonra çakışan yerleriyle (`b`) de geri gönderilebilir ya da kısmen uygulanabilir (`x` sonra `a`); bkz. [değişiklik görünümü](changes.md). Bir öge geldiğinde agent'ının üzerinde çalıştığı kartı tutar, böylece o [kartın ekranında](cards.md) da görünür.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `tab` `⇧tab` | panelleri döndür: salon, bekleyenler, günlük |
| `w` (`:waiting`) | salonun herhangi bir panelinden doğrudan bekleyenler paneline |
| `↑` `↓` / `j` `k`, `gg` `G` | bir öge seç |
| `enter` | ögeyi aç: bir değişikliğin diff'i, bir sorunun ya da yanıtın konuşması, bir kurulum betiğinin baytları, geri kalanı için agent'ın odağı |
| `a` / `r` | seçili ögeyi onayla / uygula / izin ver · reddet, yukarıdaki tablonun dediği gibi |
| `W` (`:next`) | her yerden: bekleyen sıradaki ögeyi aç |
| `S` (`:defer`) | bu ögeyi ertele ve sıradakini aç |
| `esc` | diff'ten panele geri |

Seçili ögenin altında soluk bir satır tuşlarını söyler (`enter diff'i gör · a checkout'a uygula · r reddet`; bir istek için `a izin ver · r reddet · enter odak`; bir yanıt için `enter tamamını oku · r kapat · ya da doğrudan yanıt yaz`; bir kurulum betiği için `enter betiği gösterir · a evet · r hayır`).

## `W` ve `S`: birbiri ardına

`W` her ekranda çalışır: pano, kod haritası, bir kartın ekranı, bir diff, bir konuşma, bir ham akış. Salonun üstünde açık olanı kapatır, paneldeki sıradaki açık ögeyi seçer ve `enter`'ın yapacağı gibi açar, böylece `a` ve `r` hemen yanıtlar. Yine basılınca ondan sonrakine geçer, sonuncudan sonra ilke döner. Bekleyen yoksa `Şu an senden bir şey gerekmiyor` der ve hiçbir şey kımıldamaz.

`S` eldeki ögeyi erteler ve sıradakini açar. Ertelenen ögeler diğerlerinin hepsinden sonra gelir, ertelendikleri sırayla, ve satırları en önde `sonra ·` der. Zaten ertelenmiş birini ertelemek onu en sona taşır. Liste kapanan ögeleri unutur ve yalnız bu çalıştırma boyunca sürer: yeni bir başlangıç her şeyi yine yaş sırasında gösterir.

"Eldeki", `W`'nin son açtığı ögedir; panelin seçimini kendiniz oynattığınızda (`j` `k`, `gg` `G`) eldeki o olur ve `W` oradan devam eder. "Sıradaki", ögenin ertelenmeden önceki yerine göredir, böylece `S` en başa değil ondan sonra gelene geçer.

## Satırdaki `!`

Ögesi bekleyen bir agent bunu kendi salon satırında gösterir: bir soru beklerken `? → sen`, bir istek beklerken `⚑ onayını bekliyor`, bir yanıt beklerken `sana yanıt verdi`, bir API hatası beklerken `✕ API hatası`; durumu "onayını bekliyor" olan bir agent işaret olarak `!` taşır. Proje başlığı bunları dikkat renginde sayar, `! 2 bekliyor`, proje katlıyken bile. Salonda `f` yalnız sizi bekleyen agent'ları gösterir, bunlar en önde.

## Ögeler açıkken çıkmak

`q` çıkmadan önce sorar ve şimdi çıkmanın neyi geride bırakacağını sayar: `imece kapatılsın mı? tur ortasındaki agent: 2, incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır`. Tur ortasındaki agent'lar token harcayanlardır; incelenmemiş değişiklikler açık `Δ` ögeleridir. İstekler ve sorular burada sayılmaz: zaten başlıkta ve terminal başlığındadır ve çıkmak hiçbirini kesmez. `q` `s` (`:pending`) ÇIKMADAN ÖNCE penceresini açar: tur ortasındaki agent'lar, her biri satırının söylediği işle, ve değişiklikler panelin gösterdiği gibi; orada `q` hemen çıkar, `W` bekleyen sıradakini açar, başka bir tuş geri döner. Hiçbir şey yolda değilse soru düz sorudur. Geri sayımı süren bir `:close`, `:forget`, `:archive` ya da `:clear` da söylenir (`:close kule şimdi yapılır`): çıkmak, süresinin dolması gibi onu yapar.

## Ayrıca bakınız

- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [Değişiklikler](../concepts/changes.md)
- [Değişiklik görünümü](changes.md)
- [Salon](salon.md)
- [Klavye](keyboard.md)
