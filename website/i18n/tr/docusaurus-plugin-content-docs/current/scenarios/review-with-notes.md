---
title: "Bir değişikliği geri göndermek, sonra yarısını almak"
description: İnceleme tuşları derinlemesine; satırlara yazılıp tek mesajla geri gönderilen notlar, tek başına seçilip uygulanan parçalar, bir soruya alıntılanan parça, nedeniyle reddedilen kalan.
---

# Bir değişikliği geri göndermek, sonra yarısını almak

Bir değişikliğin (`Δ`) uygula ya da reddetten fazla yanıtı vardır. Satırlarına not yazıp notları tek mesajla geri gönderebilirsiniz; parçalarının bir kısmını alıp gerisini bekletebilirsiniz; bir parçayı bir soruya alıntılayabilirsiniz. Bunlarla bir değişiklik neredeyse doğru olduğunda karşılaşırsınız. Bu senaryo bir işçinin değişikliğini iki kez inceler.

`claude-2`, `#4 hız sınırlayıcı` kartını tutuyor ve az önce `done` gönderdi.

## 1. Üç dosyalı bir değişiklik

```text
 SENİ BEKLEYEN (1)
 Δ 15:02 claude-2 bitirdi: src/limit.rs içinde token kovası, lib.rs'e bağlandı, testler (3 dosya, +45 −6) · bu ağaçta çalıştırma yok
   enter diff'i gör · a checkout'a uygula · r reddet
```

`bu ağaçta çalıştırma yok`, işçinin tam bu ağaçta imece'nin `run` aracıyla hiçbir şey çalıştırmadığını söyler; derlendiği bilinmiyor. `W`ye (her yerden) ya da öğe üstünde `enter`a basın:

```text
 DEĞİŞİKLİK · claude-2 · 3 dosya +45 −6 · src/limit.rs içinde token kovası, lib.rs'e bağlandı, testler
 tam bu değişiklik üzerinde imece çalıştırması yok: derlenip derlenmediği, testlerinin geçip geçmediği bilinmiyor
 kaynak +27 −6 (2) · testler +18 −0 (1)
 src/limit.rs +22 −6
 src/lib.rs +5 −0
 tests/limit.rs +18 −0
 diff --git a/src/limit.rs b/src/limit.rs
 @@ -1,6 +1,12 @@
 …
```

Yamanın üstünde: çalıştırmalar (burada yok), sonra, dosyalar birden çok türden olduğu için, tür başına satırları veren bir satır, sonra dosya listesi. Türler yalnızca yoldan anlaşılır: `tests/`, `test_*` ve test sonekleri testtir; lock dosyaları, `.min.js`, `generated/`, `dist/`, `vendor/` üretilmiştir; gerisi kaynaktır. Tek türden bir değişiklikte tür satırı yoktur; tek dosyalı bir değişiklikte dosya listesi yoktur. En çok altı dosya listelenir, gerisi sayılır (`… 2 dosya daha`). `[` `]` yamada dosyadan dosyaya atlar.

## 2. İki satıra not

Görünümün en üst satırı eldeki satırdır, seçili gibi çizilir. İstediğiniz satır en üste gelene dek `j` ile kaydırın ve `c`ye basın. Komut satırı satırın yeriyle etiketlenmiş açılır:

```text
 › src/limit.rs:12 için not ▸ 
```

`SystemTime değil Instant: saat geri gidebilir` yazın ve `enter`a basın. Anlık mesaj `not tutuldu (bu değişiklikte 1); n gönderir` der ve not, `✎`den sonra satırının altında görünür. İkinci yere kaydırın ve yine `c`. Silinen bir satıra not, satırı eski dosyadaki numarasıyla eksi işaretli adlandırır (`src/limit.rs:−7`). Notu olan bir satırda `c` onu yeniden yazar; boş bir not onu kaldırır. Bir parçanın satırları dışında (bir başlık, bir `@@` satırı) `c`, notun nereye yazıldığını söyler.

Notlar ekranın belleğinde bu yamaya karşı yaşar: işçiden gelen daha yeni bir `Δ`nın satırları başkadır ve notsuz başlar.

## 3. `n` ile gönderin

`n`e basın. Anlık mesaj `2 not claude-2 adlı ajana gitti; değişiklik ona geri döndü` der ve öğe kapanır. İşçi sizden tek bir mesaj alır; her not kendi `dosya:satır`ının ve satırın altında:

````text
Notes on your change, line by line. It stays in your worktree, not applied: change it as they say and send done again.

src/limit.rs:12
```diff
+    let now = SystemTime::now();
```
SystemTime değil Instant: saat geri gidebilir

tests/limit.rs:9
```diff
+    assert!(bucket.take(1));
```
dönemlik bir uykudan sonra yeniden dolmayı da doğrula
````

Değişiklik `r`nin geri gönderdiği gibi geri döner: günlük `reddedildi: claude-2` der, worktree değişikliği tutar ve işçinin sonraki `done`u yeni bir `Δ` olur. Not yokken `n`, `bu değişiklikte henüz not yok: c en üstteki satıra bir tane yazar` der.

## 4. İşçi yanıtlar

İşçi sonraki turunda notları okur ve çalışır. Veri yolundaki mesajları numaralıdır ve bir yanıt neyi yanıtladığını adlandırır. Lidere dolma dönemini sorar, lider yanıtlar; sonraki `done`u ona kartı veren göreve yanıt verir. Konuşmalarını günlükteki satırlarından birinde `enter` ile açın (bir satırda `v` o ajanın sizinle konuşmasını açar):

```text
 KONUŞMA · lead ⇄ claude-2 · 6/6
 15:02 lead → claude-2 [görev]: #4 hız sınırlayıcı: token kovası, config v2
 15:20 claude-2 → lead [soru]: config v2'de dönem ms mi s mi?
 15:21 lead → claude-2 [cevap] ↩: ms, öteki süreler gibi
   ↩ 12 yanıtı: config v2'de dönem ms mi s mi?
 15:31 claude-2 → lead [bitti] ↩: Instant tabanlı kova; dolma testi eklendi
   ↩ 9 yanıtı: #4 hız sınırlayıcı: token kovası, config v2
```

Türden sonraki `↩` bir yanıtı işaretler; altındaki soluk satır yanıtladığı mesajın ilk satırıdır. `%` bir yanıttan yanıtladığı mesaja, bir mesajdan ilk yanıtına gider; `[` `]` söz sırasından söz sırasına. `done` sonraki `Δ`yı yapar:

```text
 Δ 15:31 claude-2 bitirdi: Instant tabanlı kova; dolma testi eklendi (3 dosya, +51 −9) · ✓ bu ağaçta temiz geçti: cargo test
```

## 5. `x` ile iki parça seçin

`W` ile açın. Bir parçanın `@@` satırı ya da içindeki herhangi bir satır en üstte olacak biçimde kaydırın ve `x`e basın. Parçanın `@@` satırı `✓` taşır ve anlık mesaj sayar: `1 / 4 parça seçili; a yalnız onları uygular, kalanı bekler`. Bir ikincisini seçin: `2 / 4 parça seçili; …`. Seçili bir parçada yine `x` onu bırakır (hiç kalmadığında `seçili parça yok; a değişikliğin tamamını uygular`). Bir dosyanın başlığında `x` dosyanın ilk parçasını alır.

```text
 diff --git a/src/limit.rs b/src/limit.rs
 @@ -1,6 +1,12 @@ ✓
 +use std::time::Instant;
 …
 @@ -40,8 +46,19 @@ impl Bucket { ✓
```

Seçim bu yamanın özetine karşı tutulur; daha yeni bir `Δ` seçimsiz başlar.

## 6. Yalnızca onları uygulayın

`a`ya basın. imece'nin git iş parçacığında seçili parçalar kendi başına bir değişikliğe dönüşür (imece'nin kendi indeks dosyasına `git apply --cached`, işçinin dosyalarına ve indeksine dokunulmaz), bütün bir değişiklik gibi çalışma kopyanıza uygulanır, temizse üç yollu, ve stage'e girer:

```text
 15:33 ✓ claude-2 değişikliğini (1 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7c0e4b2
```

Worktree yalnızca o parçayla işarette commit edilir, dosyalar olduğu gibi kalır. Worktree'nin yeni HEAD'ine karşı tuttuğu şey hemen yeniden okunur ve aynı başlıkla değişiklik olarak bekler:

```text
 Δ 15:33 claude-2 bitirdi: Instant tabanlı kova; dolma testi eklendi (2 dosya, +30 −3) · bu ağaçta çalıştırma yok
```

İşçi sonraki isteminde şunu duyar: "The person applied part of your change (Instant tabanlı kova; dolma testi eklendi) to their checkout: 2 of its hunks. Your worktree is committed at 7c0e4b2 with only that part; your files are as you left them, and the rest is not committed and waits for the person as a change again." Oturmayan bir parça, uygulanmayan bir değişiklik gibi reddedilir ve değişiklik beklemeyi sürdürür. Bir parçanın içindeki satırlar seçilemez; birim parçadır.

## 7. Bir parçayı soruya alıntılayın

Kalanı `W` ile açın. `src/lib.rs` içindeki parçayı en üste kaydırın ve `>`ye basın. Komut satırı işçiye bir mesaj olarak, parça satırınızın altında alıntılı, imleç `@claude-2 `den sonra açılır:

````text
@claude-2 
```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -88,6 +88,11 @@ pub fn build(cfg: &Config) -> App {
+    let limiter = Bucket::from(cfg.limit.clone());
```
````

Soruyu çitin üstüne yazın, `burada neden clone, Config zaten referansla geliyor?`, ve `enter`. İşçiye düz bir mesajdır, söylendiği gibi günlüğe girer. Bir parçanın en çok 40 satırı girer, gerisi `… +N` olarak sayılır; işçinin worktree'sinde bütünü var. En üstte parça yokken (ikili bir dosya) anlık mesaj `burada alıntılanacak bir parça yok` der. Diff görünümünde `>` alıntılar; `.` hâlâ yan paneli daraltır.

## 8. Kalanı nedeniyle reddedin

Diff'e dönün (`W`), `r`ye basın. Anlık mesaj `claude-2 değişikliği reddedildi; nedenini ona yaz` der ve komut satırı nedenler için `@claude-2 ` ile açılır:

```text
@claude-2 lib.rs bağlantısını kaldır: uygulama sınırlayıcıyı main'de kendi kuruyor; testi tut
```

Günlük `reddedildi: claude-2` der. Değişiklik worktree'de kalır, uygulanmaz; işçiye "The person rejected your change (…). It stays in your worktree, not applied; they say why themselves. Do not send it again as it is." söylenir. Mesajınız arkasından gelir. O değişiklik uygulanırken `r` reddedilir (`claude-2 değişikliği reddedilmedi: their change is being applied`), işçinin daha yeni bir `done`u hâlâ okunuyorken de `a`.

## Ayrıca bakın

- [Değişiklikler](../concepts/changes.md): okuma, uygulama, işaret, üç yollu uygulama
- [Değişiklik görünümü](../screen/changes.md): diff görünümünün her tuşu
- [Konuşmalar](../screen/conversations.md): `↩`, `%`, `[ ]`
- [Mesajlar ve pano](../concepts/messages-and-board.md): mesaj numaraları ve `re`
- [Kırık bir test, tek işçi](one-fix.md)
