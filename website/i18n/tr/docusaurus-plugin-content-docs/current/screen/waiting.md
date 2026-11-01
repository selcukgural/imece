---
title: SENİ BEKLEYEN
description: Yalnız sizin verebileceğiniz her kararı tutan panel - incelenecek değişiklikler, izin verilecek araç çağrıları, yanıtlanacak sorular - ve içinde ilerleyen tuşlar.
---

# SENİ BEKLEYEN

SENİ BEKLEYEN (İngilizce ekranda WAITING FOR YOU) salonun üst yan panelidir. Oraya yalnız sizi gerektiren kararlar düşer; geri kalan her şey günlükte kalır. Onunla ilk kez bir işçi `done` gönderdiğinde karşılaşırsınız: değişikliği (`Δ`) görünür, başlık `! 1 seni bekliyor` sayar ve terminalin başlığı `imece · 1 seni bekliyor` der.

```text
┌ SENİ BEKLEYEN (3) ─────────────────────────────────────────────────┐
│Δ 13:58 claude-1 bitirdi: parser (3 dosya, +41 −6) · ✓ bu ağaçta temiz geçti: cargo test
│  enter diff'i gör · a checkout'a uygula · r reddet                   │
│⚑ 14:01 codex-2 onayını bekliyor: Bash: npm install left-pad         │
│? 14:03 sonra · gemini-3 sana soruyor: Eski CLI bayrağı kalsın mı?   │
└─────────────────────────────────────────────────────────────────────┘
```

## Oraya ne düşer

| İşaret | Öge | Nereden gelir | `a` | `r` |
| --- | --- | --- | --- | --- |
| `Δ` | bir değişiklik | bir işçinin `done`'u: worktree'sinin HEAD'inize göre bütün değişikliği, imece okur. Satır done'un ilk satırını, dosya ve satır sayılarını ve imece'nin `run` aracının tam bu ağaçta ne çalıştırdığını söyler (`✓ bu ağaçta temiz geçti`, `✗ başarısız`, ya da `bu ağaçta çalıştırma yok`). | checkout'unuza uygula, stage'de | reddet; nedenini yazmanız için yazma satırı `@worker ` ile açılır |
| `⚑` | bir izin isteği | agent'ın kendi satıcısının izin soracağı bir araç çağrısı: Claude Code'un soracağı bir kabuk komutu, bir Codex `on-request` isteği, bir Gemini kabuğu. Olağan komutlar (derleme, testler, git'in okuma komutları) kendiliğinden çalışır ve buraya hiç gelmez. Seçili istek satırının altında bütünüyle gösterilir, satırları ` ↵ ` ile birleştirilmiş, asla Markdown olarak okunmaz: izin verdiğiniz şey çalışan şeydir. | izin ver | reddet; agent'a bunun sizin kararınız olduğu söylenir |
| `?` | bir soru | bir agent'ın `send(to: "you", type: ask)` çağrısı. Aynı agent'ın yeni sorusu eskisinin yerini alır. | ikisi de yazma satırını `@agent ` ile açar; yanıtınız gidince öge kapanır | |
| `»` | bir yanıt | bir agent size yanıt verdi ve okumanızı bekliyor | `enter` okur (konuşma açılır) ve kapatır; `r` kapatır | |

Satır saati taşır, ertelediyseniz `sonra ·`, birden çok proje açıksa projenin adını. Başka bir günün ögesi tarihini de taşır.

Bir değişiklik (`Δ`) işçisine notlarla da geri gönderilebilir (diff'te `n`) ya da kısmen uygulanabilir (`x` sonra `a`); bkz. [değişiklik görünümü](changes.md). Bir öge geldiğinde agent'ının üzerinde çalıştığı kartı tutar, böylece o [kartın ekranında](cards.md) da görünür.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `tab` `⇧tab` | panelleri döndür: salon, bekleyenler, günlük |
| `w` (`:waiting`) | salonun herhangi bir panelinden doğrudan bekleyenler paneline |
| `↑` `↓` / `j` `k`, `gg` `G` | bir öge seç |
| `enter` | ögeyi aç: bir değişikliğin diff'i, bir sorunun ya da yanıtın konuşması, geri kalanı için agent'ın odağı |
| `a` / `r` | seçili ögeyi onayla / uygula / izin ver · reddet, yukarıdaki tablonun dediği gibi |
| `W` (`:next`) | her yerden: bekleyen sıradaki ögeyi aç |
| `S` (`:defer`) | bu ögeyi ertele ve sıradakini aç |
| `esc` | diff'ten panele geri |

Seçili ögenin altında soluk bir satır tuşlarını söyler (`enter diff'i gör · a checkout'a uygula · r reddet`; bir istek için `a izin ver · r reddet · enter odak`; bir yanıt için `enter tamamını oku · r kapat · ya da doğrudan yanıt yaz`).

## `W` ve `S`: birbiri ardına

`W` her ekranda çalışır: pano, kod haritası, bir kartın ekranı, bir diff, bir konuşma, bir ham akış. Salonun üstünde açık olanı kapatır, paneldeki sıradaki açık ögeyi seçer ve `enter`'ın yapacağı gibi açar, böylece `a` ve `r` hemen yanıtlar. Yine basılınca ondan sonrakine geçer, sonuncudan sonra ilke döner. Bekleyen yoksa `Şu an senden bir şey gerekmiyor` der ve hiçbir şey kımıldamaz.

`S` eldeki ögeyi erteler ve sıradakini açar. Ertelenen ögeler diğerlerinin hepsinden sonra gelir, ertelendikleri sırayla, ve satırları en önde `sonra ·` der. Zaten ertelenmiş birini ertelemek onu en sona taşır. Liste kapanan ögeleri unutur ve yalnız bu çalıştırma boyunca sürer: yeni bir başlangıç her şeyi yine yaş sırasında gösterir.

"Eldeki", `W`'nin son açtığı ögedir; panelin seçimini kendiniz oynattığınızda (`j` `k`, `gg` `G`) eldeki o olur ve `W` oradan devam eder. "Sıradaki", ögenin ertelenmeden önceki yerine göredir, böylece `S` en başa değil ondan sonra gelene geçer.

## Satırdaki `!`

Ögesi bekleyen bir agent bunu kendi salon satırında gösterir: bir soru beklerken `? → sen`, bir istek beklerken `⚑ onayını bekliyor`, bir yanıt beklerken `sana yanıt verdi`; durumu "onayını bekliyor" olan bir agent işaret olarak `!` taşır. Proje başlığı bunları dikkat renginde sayar, `! 2 bekliyor`, proje katlıyken bile. Salonda `f` yalnız sizi bekleyen agent'ları gösterir, bunlar en önde.

## Ögeler açıkken çıkmak

`q` çıkmadan önce sorar ve şimdi çıkmanın neyi geride bırakacağını sayar: `imece kapatılsın mı? tur ortasındaki agent: 2, incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır`. Tur ortasındaki agent'lar token harcayanlardır; incelenmemiş değişiklikler açık `Δ` ögeleridir. İstekler ve sorular burada sayılmaz: zaten başlıkta ve terminal başlığındadır ve çıkmak hiçbirini kesmez. `q` `s` (`:pending`) ÇIKMADAN ÖNCE penceresini açar: tur ortasındaki agent'lar, her biri satırının söylediği işle, ve değişiklikler panelin gösterdiği gibi; orada `q` hemen çıkar, `W` bekleyen sıradakini açar, başka bir tuş geri döner. Hiçbir şey yolda değilse soru düz sorudur.

## Ayrıca bakınız

- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [Değişiklikler](../concepts/changes.md)
- [Değişiklik görünümü](changes.md)
- [Salon](salon.md)
- [Klavye](keyboard.md)
