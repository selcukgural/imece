---
title: Bir kartın ekranı
description: Bir kartın her şeyi tek ekranda - kim çalıştı, hikâyesi, ondan sizi bekleyenler ve günlüğü - # ile kartın numarası ya da başlığından bir sözcükle açılır.
---

# Bir kartın ekranı

Bir kart tek bir iş birimidir: lider asar (ya da boşta bir işçiye mesajınız açar), bir işçi alır, bir `done` ile biter. Numarası, `#3`, salonda işçisinin satırında, panoda ve günlükte görünür. `#` bir kartın ekranını açar: ana ekranın üç paneli, ama yalnız tek bir kart için. Onunla, bütün günlüğü okumadan tek bir işe ne olduğunu öğrenmek istediğinizde karşılaşırsınız. Türkçe ekran karta `#` menüsünde "iş" der.

```text
┌ #3 Parse the manifest ──────────────────────────────┐┌ SENİ BEKLEYEN · #3 (1) ───────────────┐
│shop-api · ÜZERİNDE · 23 dk                           ││Δ 13:58 claude-1 bitirdi: parser (3 dosya, +41 −6)
│                                                      │└───────────────────────────────────────┘
│KİM                                                   │┌ DEFTER · #3 (4) ──────────────────────┐
│astı      ◆ ▎lead      claude            koordine ediyor│14:01 ⇄ lead → claude-1 [cevap] ↩: #3 için manifest::read …
│sahibi    ● ▎claude-1  #3   Parse the manifest  testleri yazıyor
│yardımcı  ● ▎claude-1.1                  düşünüyor…   ││13:57 ⇄ claude-1 → lead [soru]: #3 için hangi parser? …
│                                                      ││13:55 → lead → claude-1: #3 Parse the manifest
│HİKÂYE                                                ││13:50 ◆ lead panoya astı: #3 Parse the manifest
│13:55 ◎ niyet       Manifest paniklemeden okunsun     ││                                       │
│13:57 ⇄ konuşma     → lead [soru] Hangi parser? …     ││                                       │
│14:01 ⚖ karar       manifest::read, eskisi sonra      ││                                       │
│14:10 ◌ kanıt       ✓ cargo test                      ││                                       │
└──────────────────────────────────────────────────────┘└───────────────────────────────────────┘
  tab panel  enter odak  n p sonraki / önceki iş  # başka iş  esc geri  ? yardım
```

## Bir kartı açmak

`#` komut satırını `#` ile ve kartların menüsüyle açar (en çok 30, açık projelerden). Rakamlar bir numaranın başıyla eşleşir; başka her şey başlığın içinde, büyük küçük harf ayırmadan, en yeni önce; hiçbir şey yazılmazsa her kart listelenir, en yeni önce. Her öge `#N başlık` biçimindedir ve bütün satırın yerini alır; `enter` vurgulu kartı alır ve hemen açar: tek tuş, önce ekle sonra gönder değil. Kart adlandırmayan bir satır `#… diye bir iş yok` diye flaşlanır. `tab` `⇧tab` `↑` `↓` her tamamlamada olduğu gibi menüde ilerler.

`#` eldeki kartı baştan yazar: yakınlaştırılmış ya da seçili agent'ınkini (bir yardımcınınki işçisininkidir), SENİ BEKLEYEN'de seçili ögenin geldiği kartı, seçili günlük satırının adlandırdığı ya da ilk andığı kartı. Bir kartın ekranında kendi kartı haber değildir, o yüzden `#` boş başlar.

`#N` ve `#word` yardımın bunun için kullandığı adlardır (Türkçe yardımda `#N  #kelime`): `#3` numarayla, `#manifest` başlıktan bir sözcükle.

## Ekran ne gösterir

Solda, salonun yerinde:

- Başlık satırı: `#3` ve kartın başlığı; sonra projesi, panonun sözcükleriyle sütunu (BEKLİYOR, ÜZERİNDE, DOĞRULANIYOR, BİTTİ) ve dakikaları, bitmişse bitişine kadar.
- Varsa lease'leri (yalnız demoda; sembol lease'leri canlıda yapılmadı).
- **KİM**: kim astı, sahibi, hâlâ sahibinin kartıyken sahibinin yardımcıları, sonra hikâyesinin hakkında konuştuğunu söylediği herkes; her biri bir kez, ilk aldığı rolle. Salonun agent satırları gibi çizilirler, böylece durumları canlıdır; `j` `k` aralarında seçer ve `enter` seçilenin odağını salondaki gibi açar.
- **HİKÂYE**: kartın hikâyesi, olay başına bir satır, eskiden yeniye, sahibinin gördüğü gibi anlatılır. Her satırda bir işaret ve bir etiket vardır: `◎` niyet, `⚑` varsayım, `⚖` karar, `⇄` konuşma (bir mesaj, türüyle), `◌` kanıt (bir çalıştırma ve nasıl bittiği), `Δ` değişiklik, `›` sen (onun hakkında söylediğiniz). Henüz kimsenin almadığı bir kart `Henüz hikâye yok.` der.

Sağda, yalnız bu kartın yan panelleri:

- **SENİ BEKLEYEN · #3**: bu kartta gelen açık ögeler. Bir öge geldiğinde agent'ının üzerinde çalıştığı kartı tutar, böylece bir işçinin `Δ`'sı işçi sonraki kartı aldıktan sonra da bitirdiği kartta kalır.
- **DEFTER · #3**: kartı adlandıran satırlar (asıldı, alındı, görev verildi) ve metninde `#3` geçen her söylenmiş satır (ardında rakam yoksa; `#30`, `#3` değildir), günlük gibi katlanmış, en yeni önce; numara geçtiği her yerde vurgu renginde işaretlenir.

## Kart ekranındaki tuşlar

Her tuş ana ekranda yaptığını yapar: `tab` panelleri döndürür, bir günlük satırında `enter` konuşmayı açar, `a` `r` seçili ögeyi yanıtlar, `c` kopyalar, `t` ve `v` seçili agent'ın izini ya da konuşmasını açar, `m` işaretler, `W` her yerdeki bekleyen sıradakine gider.

| Tuş | Ne yapar |
| --- | --- |
| `n` / `p` | açık bir projenin sonraki / önceki kartı, numarayla |
| `#` | başka bir kart (menü boş başlar) |
| `esc` | buradan açılan bir odak, iz ya da diff'ten karta geri; kartta salona geri |
| `^o` | olduğunuz yere geri; kart ekranı da yerlerden biridir |
| `:export [YOL]` | kartın ekranı Markdown dosyası olarak |
| `1` `2` `3` | kartı bırakıp salona, panoya ya da kod haritasına |

Salonun katlama tuşları (`h` `l`, `za` `zM` `zR`) ve `f` burada bir şey yapmaz.

## Dışa aktarma

Bir kartın ekranında (üstünde konuşma açık değilken) `:export` şunu yazar: `# #3 Parse the manifest`, projesi, sütunu ve dakikaları, lease'leri, `## KİM`, `## HİKÂYE` (satır başına bir madde), `## Seni bekleyenler` ve `## Defter`: kartın her satırı, eskiden yeniye, hiçbiri katlanmamış. Her saat gününü taşır, çünkü dosya ekranı terk eder. Varsayılan ad imece'nin başlatıldığı dizinde `imece-card-3-20261031-1401.md`; var olan bir dosyanın üzerine asla yazılmaz.

## Henüz yok

Günlükte serbest metin arama yok; menü kartları yalnız numara ve başlıkla bulur. Bir kartın kendi `Δ` satırı günlükte (bitirdi, uygulandı) kartı adlandırmaz, bu yüzden kartın ekranına yalnız SENİ BEKLEYEN yoluyla düşer.

## Ayrıca bakınız

- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Pano ve kod haritası](board-and-code-map.md)
- [Günlük](journal.md)
- [Komut satırı](command-line.md)
- [Klavye](keyboard.md)
