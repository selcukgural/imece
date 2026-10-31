---
title: "Kırık bir test, tek işçi"
description: Baştan commit'e tek bir iş; lider bir işçi alır, işçi bir komut için izin ister, değişikliğini gönderir, siz uygular ve commit edersiniz.
---

# Kırık bir test, tek işçi

Bütün döngü bir kez, tek ajanla: lidere neyin bozuk olduğunu söylersiniz, lider bir işçi alır, işçinin değişikliği (`Δ`) SENİ BEKLEYEN paneline gelir, okur, uygular ve commit edersiniz. Tek bir işin dokunduğu her imece parçasıyla, olduğu sırayla karşılaşırsınız.

Çalışma kopyası, bir testi kırık bir Rust crate'i. PATH'te Claude Code var.

## 1. Başlatma

```sh
cd ~/code/parser
imece
```

PATH'te tek kodlama ajanı varsa soru sorulmaz; birkaçı varsa KİM LİDER? ekranı sorar (`j k` seç, `enter` bu seferlik, `p` hep). Salon, takım yalnızca liderden ibaretken açılır: `lead`, çalışma kopyanızı okur ve ona asla yazmaz. Günlüğün (ekranda DEFTER) ilk satırı ajanlara ne verildiğini söyler, çünkü varsayılan seviye `standard`:

```text
 DEFTER
 14:00 ⚑ confine standard: işçiler satıcılarının kabuğunu ve web'ini alır; yalıtım bir konteynerin işi, imece'nin değil
```

Henüz hiçbir şey para etmez: ona yazmadığınız sürece `lead`in bir süreci yoktur.

## 2. Lidere söyleyin

`/` tuşuna basın. Alt satır ` › lead için: ` etiketli bir komut satırına döner. İsteği yazın ve `enter`a basın:

```text
 › lead için: parser testi boş girdide kırılıyor, düzelt
```

Günlük satırınızı gösterir (`›` sizin yaptığınızı işaretler) ve lider ilk turuna başlar: satırı `◆ lead  claude  düşünüyor…` olur, sonra ne yaptığını söyler. Testi okuma araçlarıyla okur, imece'nin `run` aracıyla `cargo test` çalıştırabilir. Dosya değiştirmez: lider her confine seviyesinde yazmaz, çünkü çalışma kopyanızda çalışır ve orada bir yazma incelemeyi atlardı.

## 3. Lider işe alır

Lider `hire` aracını bir kademeyle çağırır, asla model adıyla değil: küçük mekanik işler için `cheap`, sıradan düzeltmeler için `standard` (varsayılan), kodu boydan boya kesen tasarım için `strong`. Bu düzeltme için `standard` seçer. İşçi satıcısının adını alır, `claude-1`, ve günlük kimin kimi neyle aldığını söyler:

```text
 14:01 ◆ lead işe aldı: claude-1 (varsayılan model)
 14:01 ⇄ lead → claude-1 [görev]: parser: boş girdi boş ağaç yerine Err veriyor; src/parser.rs, test parse_empty
```

"varsayılan model", `standard`ın Claude için anlamı: Claude Code'unuzun çalıştığı model. Bir `strong` alım `(opus)`, bir `cheap` alım `(haiku)` derdi. Liderin araçtan aldığı yanıt `claude-1 joined (claude, the default model). Give it work with send (type assign). 3 more may be hired.` idi: `--agents` (varsayılan 4) tavandır ve takım boyutunu asla siz seçmezsiniz.

`assign` mesajı bir kart açar, `#1`, başlığı mesajın ilk satırı. İşçinin satırı onu gösterir:

```text
 ▾ parser                      1 çalışıyor  $0.09
   ◆ lead       claude          koordine ediyor                    $0.09
   ● claude-1   #1   parser: boş girdi      src/parser.rs okuyor
```

İlk turundan önce imece `claude-1` için `~/.local/state/imece/worktrees/` altında, HEAD'inizde ayrık bir git worktree'si yapar. Çalışma kopyanıza bir ajan asla dokunmaz.

## 4. Çalışmasını izlemek

İşçi ilerlerken satırında iki şey değişir.

Durumu, o an yaptığı şeydir (okuyor, düzenliyor, çalıştırıyor), satıcısının bildirdiği sözcüklerle. İlk araç kullanımından sonra satır worktree'nin durduğu yeri de gösterir; işçi tur ortasındayken her 15 saniyede okunur:

```text
   ● claude-1   #1   parser: boş girdi      src/parser.rs düzenliyor   $0.14  +12 −3
```

`+12 −3`, bir sonraki `done`unun size vereceği satırlardır, yazıldıkça sayılır. `⎇ ad` yalnızca ajan bir dal yaptıysa görünür, `↑N` kendi commit'leri, `↓N` onda olmayan sizin commit'leriniz için (`?` içindeki lejant hepsini adlandırır).

Bir `progress` mesajı, işçinin liderini uyandırmadan gönderdiği mesajdır: başka bir şey liderin turunu başlatana dek bekler. Günlükte bir konuşma satırı olarak görünür:

```text
 14:04 ⇄ claude-1 → lead [ilerleme]: buldum: parse() len'e bakmadan tokens[0] okuyor; düzeltiyor, test ekliyorum
```

## 5. Bir `⚑` sorusu

`standard` altında Claude Code'un kendi kabuğu açıktır ve Claude Code'un kurallarının soracağı bir komut reddedilmek yerine size gelir. **Rutin** bir komut hemen çalışır: tek satır, kabuk sözdizimi yok (boru, yönlendirme, `&&`, `$` ya da glob yok) ve `run` aracının politikasının bu role izin verdiği bir program (bağımlılık değiştiren komutları olmadan cargo, git'in okuma komutları, `--run` ile adlandırdığınız programlar). `cargo test` rutindir; iz `allowed on its own: Bash: cargo test` der. Gerisi bekler:

```text
 SENİ BEKLEYEN (1)
 ⚑ 14:07 claude-1 onayını bekliyor: Bash: cargo test 2>&1 | tail -20
   a izin ver · r reddet · enter odak
```

Boru komutu rutin olmaktan çıkarır. Komut bütünüyle gösterilir, asla markdown olarak okunmaz: izin verdiğiniz şey çalışan şeydir. Bu arada işçinin satırı `⚑ onayını bekliyor` der ve terminalin başlığı `imece · 1 seni bekliyor` olur.

`tab` (ya da `w`) ile panele geçin, ya da her yerden `W`, ve `a`. Anlık mesaj `izin verildi: claude-1 devam ediyor` der; günlük kararınızı tutar:

```text
 14:07 › claude-1 için izin verdin: Bash: cargo test 2>&1 | tail -20
```

`r`, `reddedildi: claude-1 bunu öğrenecek` derdi ve modele "The person refused this. Do not try it another way; if you need it, ask them" gider. Bir çağrı yanıtınızı bir güne kadar bekler; oturumun bitmesi onu geri çeker.

## 6. `done` değişikliğe dönüşür

İşçi işini `run` aracıyla doğrular (talimatları öyle der: yalnızca imece'nin kendi çalıştırmaları kanıt sayılır, çünkü hangi ağaçta çalıştıklarını imece bilir) ve lidere `done` gönderir, göreve yanıt olarak (`↩`). imece worktree'yi HEAD'inize karşı okur ve değişiklik sizi bekler:

```text
 14:12 ⇄ claude-1 → lead [bitti] ↩: parse() token yokken Ok(Tree::empty()) döndürüyor; parse_empty testi eklendi
 14:12 ! claude-1 bitirdi: 1 dosya, +14 −3 → seni bekliyor
```

```text
 SENİ BEKLEYEN (1)
 Δ 14:12 claude-1 bitirdi: parse() token yokken Ok(Tree::empty()) döndürüyor (1 dosya, +14 −3) · ✓ bu ağaçta temiz geçti: cargo test
   enter diff'i gör · a checkout'a uygula · r reddet
```

Öğenin metni işçinin `done`unun ilk satırıdır; bundan sonra değişikliğin başlığı odur. `✓ bu ağaçta temiz geçti`, imece'nin tam bu ağaçta yaptığı çalıştırmaları adlandırır. İşçi yalnızca kendi kabuğunda test etseydi satır `bu ağaçta çalıştırma yok` derdi: bir satıcının kabuğu, imece'nin güvenebileceği bir çıkış kodu taşımaz. Çalıştırma yok diye hiçbir şey reddedilmez; karar sizin.

## 7. Diff'i okumak

`W`ye basın. Nerede olsanız sizi bekleyen sonraki öğeyi açar: burada diff görünümünü.

```text
 DEĞİŞİKLİK · claude-1 · 1 dosya +14 −3 · parse() token yokken Ok(Tree::empty()) döndürüyor
 ✓ cargo test
 diff --git a/src/parser.rs b/src/parser.rs
 @@ -40,7 +40,9 @@ impl Parser {
 -    let first = tokens[0];
 +    if tokens.is_empty() {
 +        return Ok(Tree::empty());
 +    }
 j k  w  kaydır · sar   [ ]  dosya   a x  uygula · parça seç   r  reddet   >  ona alıntıla   c n  not · notları gönder   esc  geri
```

İlk satırlar bu ağaçtaki çalıştırmalar, sonra yama. `j k` kaydırır, `gg G` uçlara, `^d ^u ^f ^b` sayfa sayfa. `[` `]` dosyadan dosyaya gider; tek dosyada yamanın üstünde dosya listesi yoktur (iki ya da daha çok dosyalı bir değişiklik onları listeler). `w` uzun satırları altlarına sarar, `w` yine kenarda keser. `esc` geri çıkar.

## 8. Uygulama

`a`ya basın. Anlık mesaj `claude-1 değişikliği checkout'una uygulanıyor; stage'e girince günlük söyler` der. Git kendi iş parçacığında çalışır, ekran hiç beklemez. Sonra:

```text
 14:13 ✓ claude-1 değişikliğini (1 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 3f2a9c1
```

Uygulamak, çalışma kopyanızda `git apply --index` demektir: dosyalar değişir ve stage'e girer, hiçbir şey commit edilmez. Aynı satırları siz de düzenlemiş olsaydınız uygulama git'in sözleriyle reddedilir ve hiçbir şey yarım kalmaz; çalışma kopyanız yalnızca ilerlemiş olsaydı yama önce indeksin bir kopyasında üç yollu birleştirilir ve yalnızca temiz çıkarsa uygulanır.

İşaret: işçinin worktree'si `3f2a9c1`de `imece <imece@localhost>` olarak commit edilir, dosyalar olduğu gibi bırakılır. Böylece işçinin sonraki `done`u yalnızca sonrasında geleni gösterir ve satırdaki `+14 −3` sıfırlanır. Sonraki isteminde işçiye şu söylenir: "The person applied your change (parse() returns Ok(Tree::empty()) on no tokens) to their checkout. Your worktree is committed at 3f2a9c1 as a mark of that; your files are as you left them. Go on from here with the next task."

## 9. Kendi kabuğunuzda bakın

`!` tuşuna basın. Komut satırı ` › kabuk: ` olarak, `!` yazılmış hâlde açılır. Yazın ve `enter`a basın:

```text
 › kabuk: !git diff --cached
```

Ekran kenara çekilir; `git diff --cached` terminali tek başına kullanır (bir sayfalayıcı çalışır). Çıktısının altında:

```text
[imece] çıkış 0 · 40 ms sürdü · herhangi bir tuş: ekrana dön
```

Herhangi bir tuş ekranı geri getirir. Günlük komutu, nasıl bittiğini ve ne kadar sürdüğünü tutar; çıktı hiçbir ajana ulaşmaz:

```text
 14:14 › `git diff --cached` çalıştırdın → çıkış 0 · 40 ms sürdü
```

## 10. Bir commit mesajı

`:commit` yazın ve `enter`a basın. Anlık mesaj `haiku modeline parser içinde stage'dekiler için commit mesajı soruluyor` der. imece çağrıdan önce bütçeden $0.05 ayırır (bir önerinin en çok tutabileceği) ve Claude Code'un ucuz kademesine, araçsız ve kendi kısa sistem istemiyle, `git diff --cached` için bir mesaj sorar. Yanıt gelince komut satırı onunla, tek tırnak içinde, düzenlemeniz için açılır:

```text
 › kabuk: !git commit -m 'Return an empty tree for empty input

The parser read tokens[0] before checking the length. parse_empty covers it.'
```

```text
 14:15 ! haiku stage'dekiler için bir commit mesajı önerdi, $0.01 tuttu
```

imece hiçbir şeyi commit etmez: `enter` satırı `!` gibi kendi kabuğunuzda çalıştırır, `esc` bırakır. Bir gün bütün projeler için 20 öneri ve $1.00 alır; aynı stage'lenmiş ağaç yeniden sorulduğunda hiçbir şey tutmaz. `:` menüsü `:commit`i `$` ile işaretler, çünkü faturayı büyütür.

`enter`a basın. Commit girer; ekran herhangi bir tuşla geri gelir.

## 11. Worktree izler

HEAD'iniz ilerledi. İşçinin sonraki isteminden önce, kendi commit'lerinin hepsi imece'nin işaretleri olan temiz bir worktree HEAD'inize taşınır ve günlük söyler:

```text
 14:20 ⚑ claude-1: çalışma ağacı 3f2a9c1 yerine HEAD 9b1d2e0 oldu
```

Commit edilmemiş değişiklikleri ya da kendi commit'leri olan bir worktree olduğu yerde kalır ve günlük onu söyler (`çalışma ağacı … noktasında kaldı, commit edilmemiş değişiklikleri var`).

## 12. Çıkış

`q`ya basın. Tur ortasında kimse yokken soru sadedir:

```text
 imece kapatılsın mı? bir daha q çıkar · başka bir tuş kalır
```

Bir daha `q` çıkar. Takım, kartlar, günlük ve bütçe tek bir SQLite dosyasına kaydedilir; bu çalışma kopyasında sonraki `imece` aynı ekranı, her ajan uykuda ve kendisiyle konuşulana dek para harcamadan gösterir. [Geri dönmek](next-morning.md) buradan sürer.

## Ayrıca bakın

- [Değişiklikler](../concepts/changes.md): bir `Δ` nedir, uygulama nasıl çalışır
- [Onaylar ve sorular](../concepts/approvals-and-questions.md): rutin kuralı
- [Takım](../concepts/team.md): lider, işçiler, yardımcılar, kademeler
- [Değişiklik görünümü](../screen/changes.md)
- [Takımın yanında kendi kabuğunuz](own-shell-and-commit.md)
