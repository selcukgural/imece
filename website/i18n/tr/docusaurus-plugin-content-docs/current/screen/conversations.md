---
title: Konuşmalar
description: Konuşma penceresi - iki taraf arasında söylenen her şey bütünüyle, söz sıraları, yanıtlar, işaretler, kopyalama ve dışa aktarma.
---

# Konuşmalar

Bir konuşma, iki taraf arasında söylenen her şeydir: siz ve bir agent, ya da iki agent; eskiden yeniye, her mesaj bütünüyle. Günlükte bir satırda `enter` ile ya da eldeki agent'ta `v` (`:talk`) ile açarsınız; salonun üstünde bir pencere olarak açılır, çevresini soluklaştırır, `esc` kapatır.

```text
┌ KONUŞMA · lead ⇄ claude-1 · 4/5 ──────────────────────────────────────────┐
│ 13:55 lead → claude-1 [görev]                                             │
│   #3 Parse the manifest                                                   │
│   Manifest parser boş dosyada panikliyor; hata döndürsün.                 │
│                                                                           │
│ 13:57 claude-1 → lead [soru] ↩                                            │
│   ↩ 7 yanıtı: #3 Parse the manifest                                       │
│   Hangi parser? İki tane var: `manifest::read` ve eskisi.                 │
│                                                                           │
│▌✱14:01 lead → claude-1 [cevap] ↩                                          │
│▌  ↩ 8 yanıtı: Hangi parser? İki tane var                                  │
│▌  `manifest::read`; eskisi bir sonraki karta.                             │
│                                                                           │
│ ↑↓ j k mesaj  [ ] söz sırası  % yanıt ↔ soru  pgup pgdn kaydır  c mesajı kopyala  C konuşmayı kopyala  esc kapat
└───────────────────────────────────────────────────────────────────────────┘
```

Başlık iki tarafı ve seçimin yerini söyler, `4/5`. Konuşma olmayan bir satırda (günlüğün bir olayı) açılan pencere OLAY başlığını taşır ve o satırı tutar.

## Açmak

- Günlükte bir satırda `enter`, o satırın ait olduğu konuşmayı o satır seçili olarak açar.
- `v` ya da `:talk` (`:talk @AD` adlı bir agent için) sizinle eldeki agent arasındaki konuşmayı açar: seçili ya da yakınlaştırılmış agent. Henüz konuşma yoksa söyler.
- SENİ BEKLEYEN'de bir soruda (`?`) ya da yanıtta (`»`) `enter` konuşmayı o mesajda açar; bir yanıt bununla okunmuş olur ve kapanır.
- Bekleyen sıradaki öge bir soru ya da yanıtsa `W` de aynı şekilde açar.

Boşta bir işçiye mesaj onun için bir kart açar (ilk satır kartın başlığıdır), yanıtı kartı kapatır. Yani pencereden `@claude-2 metin` yazmak bir mesaj olduğu kadar bir karttır.

## Okumak

| Tuş | Ne yapar |
| --- | --- |
| `↑` `↓` / `j` `k` | üstteki ya da alttaki mesaj; pencere onu görünür tutmak için kayar |
| `[` `]` | söz sırasından söz sırasına: bir söz sırası, bir tarafın diğeri yanıt vermeden önce söylediği satırlardır. `]` sonraki söz sırasının ilk satırına, `[` eldeki söz sırasının başına, zaten baştaysa öncekine gider |
| `%` | bir yanıttan yanıtladığı mesaja, bir mesajdan ilk yanıtına |
| `gg` `G` | ilk / son mesaj |
| `pgup` `pgdn`, `^d` `^u` `^f` `^b`, `space` | bir sayfa ya da yarım sayfa kaydır |
| `m` (`:mark`) / `'` | seçili mesajı sonra dönmek için işaretle, yine basılınca kaldır · sonraki işaretli mesaj, ilke dönerek |
| `c` `y` / `C` `Y` | mesajı / bütün konuşmayı kopyala |
| `:export [YOL]` | konuşmayı Markdown dosyası olarak yaz |
| `esc` | kapat |

### Yanıtlar

Bir agent'ın diğerine gönderdiği her mesajın bus'ta bir numarası vardır; dünya genelinde sırayla verilir ve çalıştırmalar arasında saklanır. Agent'lar bunu aldıkları postada görür (`[answer 8, re 7] from claude-1:`), iz `id=8 re=7` der. Ekranda bir yanıt türünden sonra `↩` taşır: günlük satırında, bir kartın hikâyesinde ve bir konuşmada; konuşmada yanıtladığı mesajın ilk satırı başlığının altında soluk görünür (`↩ 8 yanıtı: Hangi parser?`, ya da günlükte artık olmadığı). `%` aralarında gider. Size gelen mesajlar numara almaz: sorularınız, yanıtı soruya zaten bağlayan SENİ BEKLEYEN'de bekler.

### İşaretler

`m` seçili mesajı işaretler; ilk satırının kenarında `✱` görünür ve flaş `işaretlendi: ' ona geri getirir` der. `'` konuşmanın kendi satırları arasında eldekinden sonraki işaretli mesajı seçer. İşaret sizindir, çalıştırmayı aşar (depoda saklanır) ve günlük işaretli bir satırı asla katlamaz ya da temizlemez. Konuşma dışındaki işaretler için [günlük](journal.md).

### Kopyalama

`c` ya da `y` seçili mesajı kopyalar: yalnız metnini. `C` ya da `Y` bütün konuşmayı kopyalar: her mesaj günü ve saatiyle, kimden kime ve türü, `2026-10-31 14:01 lead → claude-1 [cevap]`, sonra metin. Pano sistemin kendi aracıdır (`pbcopy`, `wl-copy`, `xclip`, `xsel`) ya da OSC 52 üzerinden terminalin panosu, ki terminal izin verdiğinde ssh üzerinden de çalışır; flaş hangisi olduğunu söyler, `kopyalandı (pbcopy)`. Metin çizilen her şey gibi temizlenir: kontrol karakteri yok, gizli anahtara benzeyen parçalar saklı.

### Dışa aktarma

Bir konuşma açıkken `:export` onu Markdown olarak yazar: `# Konuşma: lead ⇄ claude-1`, sonra her mesaj günü ve saati, kimden kime ve türüyle bir başlık, altında bütün metni. YOL bir dosya ya da içinde dosya adlandırılacak bir dizindir; verilmezse imece'nin başlatıldığı dizin, ad `imece-conversation-20261031-1401.md`. Var olan bir dosyanın üzerine asla yazılmaz. Bir kartın dışa aktarımı için [günlük](journal.md).

## Konuşma izler

Açık bir pencere konuşmaya ayak uydurur: çiftinin gelen günlük satırları eklenir. Pencereden `@` ile yazdığınız kendi satırınız seçilir; son satır seçiliyken gelen bir satır da. Daha yukarıdaki bir seçim okuduğunuz yerde kalır. Bundan önce bir yanıtı görmek için kapatıp yeniden açmak gerekiyordu.

## İz

Salonda bir agent'ta `enter` odağını açar (ODAK · ad: kartı, durumu, kökeni, lease'leri, satıcısı söylüyorsa bağlam penceresinin doluluğu, tutuyorsa kendi yapılacaklar listesi ve hikâyesi). Yapılacaklar listesi YAPILACAKLAR 3/7 ve satır başına bir adımdır: `✓` bitti, `▸` elinde, `○` başlanmadı, agent'ın sırasıyla; sekiz satırı aşan bir liste tepesindeki biten adımları `✓ 4 bitti` diye katlar ve `… 2 daha` ile biter, böylece elindeki görünür kalır; yine `enter`, ya da `t` (`:trace`), ham akışını açar: İZ · ad. İz, agent'ın oturumunun imece'nin gördüğü hâlidir, satır satır, satırın ilk sözcüğüne göre renkli: agent'ın kendi metni satıcı renginde, `tool_use` aracın adıyla bilgi renginde, bir `run:` çıkış koduna göre, `stopped` ve `run refused` kötü renkte, bus sözcükleri vurgu renginde. Birkaç satıra yayılan bir araç çağrısı satırlarını korur, içeriden, en çok altı, sonra `… +N`. `esc` odağa, sonra salona döner. Bir odakta ya da izde `@` o agent'a bir mesaj başlatır.

## Ayrıca bakınız

- [Günlük](journal.md)
- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Mesaj türleri](../reference/message-types.md)
- [Komut satırı](command-line.md)
- [Klavye](keyboard.md)
