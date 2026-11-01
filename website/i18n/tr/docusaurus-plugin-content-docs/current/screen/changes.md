---
title: Değişiklik görünümü
description: Bir işçinin değişikliğini (Δ) okumak - diff, dosyaları ve türleri, içinde hareket, satırlara notlar, parça seçmek, bir parçayı işçiye alıntılamak, uygulamak ve reddetmek.
---

# Değişiklik görünümü

Bir işçi `done` gönderdiğinde imece worktree'sini okur ve değişiklik SENİ BEKLEYEN'de `Δ` olarak sizi bekler. Onda `enter` (ya da sıradaki öge oysa `W`) değişiklik görünümünü salonun yerinde açar: bütün diff, üstünde o ağaçta ne çalıştığı. Kararı burada verirsiniz: `a` checkout'unuza uygular, stage'de; `r` geri gönderir; `n` satırlarındaki notlarınızla geri gönderir; `x` ve `a` yalnız seçtiğiniz parçaları uygular.

```text
┌ DEĞİŞİKLİK · claude-1 · 3 dosya +41 −6 · parser boş dosyada hata döndürür ─────────────────┐
│✓ cargo test                                                                                 │
│kaynak +23 −6 (2) · testler +18 −0 (1)                                                       │
│  src/parser/manifest.rs  +19 −6                                                             │
│  src/parser/mod.rs  +4 −0                                                                   │
│  tests/manifest.rs  +18 −0                                                                  │
│diff --git a/src/parser/manifest.rs b/src/parser/manifest.rs                                 │
│--- a/src/parser/manifest.rs                                                                 │
│+++ b/src/parser/manifest.rs                                                                 │
│@@ -12,7 +12,9 @@ pub fn read(path: &Path) -> Result<Manifest>  ✓                            │
│     let text = fs::read_to_string(path)?;                                                   │
│-    let doc: Doc = toml::from_str(&text).unwrap();                                          │
│+    if text.trim().is_empty() {                                                             │
│+        return Err(Error::Empty(path.to_owned()));                                          │
│  ✎ mesajda hangi dosya olduğunu söyle                                                       │
│+    }                                                                                       │
│+    let doc: Doc = toml::from_str(&text)?;                                                  │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
  j k  w kaydır · sar  [ ] dosya  a x uygula · parça seç  r reddet  > ona alıntıla  c n not · notları gönder  esc geri  ? yardım
```

Başlık: işçi, dosyalar ve satırlar, done'un ilk satırı. Çerçeve işçinin satıcı rengindedir.

## Yamanın üstündekiler

1. **Bu ağaçtaki çalıştırmalar.** İşçinin imece'nin `run` aracıyla tam bu değişiklik üzerinde yaptığı her `run`, nasıl bittiğiyle: `✓ cargo test`, `✗ cargo clippy: exit 1`, süre dolunca durduruldu, durduruldu. Hiç yoksa uyarı renginde, tam bu değişiklik üzerinde imece çalıştırması olmadığını, derlenip derlenmediğinin ve testlerinin geçip geçmediğinin bilinmediğini söyler. Aynı kanıt SENİ BEKLEYEN'de ögenin satırını izler.
2. **Türe göre satırlar**, dosyalar birden çok türdense: `kaynak +23 −6 (2) · testler +18 −0 (1) · üretilmiş +7 −5 (1)`. Bir dosya yalnız yoluna göre sınıflanır: bir lock dosyasıysa (`Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `go.sum`, `poetry.lock`, `Gemfile.lock`, `composer.lock`), küçültülmüş ya da üretilmiş kodsa (`.min.js`, `.min.css`, `.pb.go`, `_pb2.py`, `.g.dart`, `.Designer.cs`) ya da `generated/`, `dist/` ya da `vendor/` altındaysa üretilmiş; `tests/`, `test/`, `__tests__/` ya da `spec/` altındaysa, `test_*` adlıysa ya da bir test sonekiyle bitiyorsa (`_test.go`, `_test.rs`, `_test.py`, `.test.ts`, `.spec.ts`, `Tests.cs`, `Test.java`, …) test; yoksa kaynak. Üretilmiş, teste baskındır. Dosyalardan hiçbir şey okunmaz. Yirmi satır kod ve yeniden üretilmiş bir lock dosyası olan bir `+400 −20` değişikliği bunu burada söyler.
3. **Dosya listesi**, iki ya da daha çok dosyalı bir değişiklik için: her dosya `+N −M` ya da `ikili` ile, en çok altı, kalanı sayılmış (`… 2 dosya daha`). Tek dosyalı değişiklik liste göstermez; başlığı söyler.

## Hareket

| Tuş | Ne yapar |
| --- | --- |
| `j` `k` / `↑` `↓` | bir satır aşağı / yukarı. En üst satır eldeki satırdır, seçim gibi çizilir; satırlar altında kayar |
| `gg` / `G` | ilk / son satır |
| `^d` `^u` | yarım sayfa aşağı / yukarı |
| `^f` `^b`, `pgdn` `pgup`, `space` | bir sayfa aşağı / yukarı |
| `[` `]` | önceki / sonraki dosya: `diff --git` başlıkları |
| `w` | uzun satırları alt alta sar (küçültülmüş bir dosya, uzun bir dize); yine basılınca kenarda keser. Kaydırma yine yama satırlarını sayar, böylece `j` kaç satır kaplasa da bütün bir satır gider. Kapatılana dek her değişiklik için açık; çalıştırmalar arasında saklanmaz |
| `esc` | SENİ BEKLEYEN'e geri |

Eklenen ve silinen satırların kendi renkleri vardır; başlıklar öne çıkar. Her satır çizilen her şeyin geçtiği temizlikten geçer.

## Karar

| Tuş | Ne yapar |
| --- | --- |
| `a` | değişikliği checkout'unuza uygula, stage'de, bakıp commit etmeniz için. Parça seçiliyse (`x`) yalnız onları |
| `r` | reddet: öge kapanır, değişiklik worktree'de kalır, nedenini yazmanız için yazma satırı `@worker ` ile açılır |
| `c` | en üstteki satıra bir not |
| `n` | notları işçiye tek mesajda gönder; değişiklik geri döner |
| `x` | en üstteki parçayı seç; `a` sonra yalnız seçili parçaları uygular |
| `>` | en üstteki parçayı alıntılayarak işçiye yaz |

### Uygulamak

`a` uygulamayı yalnız başlatır: flaş sürdüğünü söyler, günlük nasıl bittiğini. Git imece'nin kendi iş parçacığında çalışır, ekran hiç beklemez. Yama `update-index --refresh` ardından `git apply --index` ile girer; bu, kendi stage'lenmemiş düzenlemeleriniz dışında bir nedenle başarısız olursa önce index'in bir kopyasına üç yönlü birleştirilir ve yalnız kopya temizse gerçekten uygulanır; böylece bir çakışma `checkout ile a, b dosyalarında çakışıyor` diye reddedilir ve hiçbir şey yarım uygulanmaz. Değişikliğin dokunduğu bir dosyadaki kendi stage'lenmemiş düzenlemeniz git'in sözleriyle reddedilir (`does not match index`): hiçbir birleştirme işinizin üstünden geçmez. Uygulamadan sonra worktree bir işaret olarak commit'lenir (yazar `imece`), böylece işçinin sonraki `done`'u yalnız ondan sonra geleni gösterir ve temiz bir worktree commit ettiğinizde HEAD'inizi izler. İşçi bunu sonraki prompt'unda öğrenir.

Reddedilir: o değişiklik uygulanırken ya da aynı işçinin daha yeni bir `done`'u hâlâ okunurken `a`.

### Reddetmek

`r` ögeyi kapatır ve worktree'yi olduğu gibi bırakır; yazma satırı nedenleriniz için `@claude-1 ` ile açılır ve işçi nedenlerinizi, imece'den değişikliğinin reddedildiği notuyla alır. Sonraki `done`'u yeni bir Δ'dır.

### Satırlara notlar

Bir parçanın satırında `c` (bağlam, eklenen ya da silinen; başlık ya da `@@` satırı değil) komut satırını bir not için açar, yeriyle etiketli: `src/parser/manifest.rs:14 için not ▸`. `enter` tutar, `esc` vazgeçer, notu olan bir satırda `c` yeniden yazar, boş bir not kaldırır. Not yazılırken ad ya da komut tamamlanmaz ve alt satır gönder değil `enter tut` der. Bir not satırının altında `✎` ardından görünür. Yer, dosyanın yeni adı ve oradaki satır numarasıdır, silinen bir satır için eksi işaretiyle eski dosyadaki numarası, `src/p.rs:−7`.

`n` değişikliğin notlarını satır sırasında işçisine tek mesajda gönderir: değişikliğin worktree'sinde kaldığını, uygulanmadığını, dediklerine göre değiştirip yine `done` göndermesini söyleyen bir satır; sonra her not, yerinin ve satırın kendisinin altında bir `diff` çitinde. Değişiklik `r`'nin geri gönderdiği gibi geri döner, günlük reddedildiğini söyler ve mesaj izler; flaş notları sayar. Not yoksa `n` nasıl yazılacağını söyler. Notlar ekranın belleğinde bu çalıştırma boyunca yaşar, yamanın özetiyle (hash) tutulur: işçiden daha yeni bir Δ'nın satırları başkadır, bu yüzden eski notlar onunla gösterilmez ve gönderilmez.

### Değişikliğin bir parçası

`x` en üstteki parçayı seçer (en üst satırın içinde olduğu parça; bir dosya başlığında sonraki); yine `x` bırakır. Seçili parçanın `@@` satırı `✓` taşır ve flaş sayar: `1 / 3 parça seçili; a yalnız onları uygular, kalanı bekler`. Parça seçiliyken `a` yalnız onları uygular; hiçbiri seçili değilse bütün değişikliği.

Git iş parçacığında parça, bir şey uygulanmadan önce kendi başına bir değişiklik olur: seçili parçalar dosyalarının başlığı altında (parçası olmayan dosya dışarıda kalır, parçası olmayan ikili dosya da), imece'nin kendi index'i üzerinden okunur, böylece işçinin dosyalarına ve index'ine dokunulmaz, ve bütün bir değişiklik gibi uygulanır, temizse üç yönlü. Worktree yalnız o parçayla işaretlenir, kalanı hemen yeniden okunur ve aynı başlıkla yine bir değişiklik olarak SENİ BEKLEYEN'de bekler. İşçi değişikliğinin bir parçasının uygulandığını (`1 of its hunks`) ve kalanın beklediğini öğrenir. Oturmayan bir parça, uygulanmayan bir değişiklik gibi reddedilir. Seçim yamanın özetiyle bu çalıştırma boyunca tutulur; yeni bir Δ hiçbiriyle başlar. Bir parça içinden tek satırlar seçilemez.

### Bir parçayı alıntılamak

`>` değişikliğin işçisine bir mesaj başlatır, `@claude-1 ` ve imleç ardında, satırınızın altında dosyanın `---` ve `+++` satırlarını ve en üstteki parçayı tutan bir `diff` çiti: en çok 40 satır, kalanı `… +N` diye sayılmış (işçide parçanın tamamı worktree'sinde var). `enter`'dan önce hepsini düzenlersiniz; düz bir mesajdır, söylendi diye günlüğe geçer. Alıntılanacak parça yoksa (boş bir değişiklik) flaş söyler. Bu görünümde `>` alıntılar, yan paneli artık daraltmaz; `.` yine daraltır.

## Günlük sonra ne der

```text
14:05 ✓ claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1
14:06 ✕ claude-1 değişikliği uygulanamadı: checkout ile CHANGELOG.md dosyasında çakışıyor
14:07 › reddedildi: claude-1
14:07 › sen → claude-1: boş dosya denetimi çağıranda değil `read` içinde olmalı …
```

Bir `done`, `claude-1 bitirdi: 3 dosya, +41 −6 → seni bekliyor` diye gelir; bir uygulama `… checkout'a uyguladın, stage'de; çalışma ağacı işareti <sha>`; bir başarısızlık `… uygulanamadı: <git'in sözleri>` ve öge kalır; bir ret `reddedildi: claude-1`, ardından mesajınız. Kısmi bir uygulamadan sonra kalan yeni bir `bitirdi` satırı olarak döner.

## Demo

`--demo` aynı değişiklikleri kartlarından ve çalıştırmalarından yapar ve `a`, `r`, `n`, `x` ve `>` tuşlarını aynı şekilde alır (başarısızlığı göstermek için orada sekiz uygulamadan biri başarısız olur).

## Ayrıca bakınız

- [Değişiklikler](../concepts/changes.md)
- [SENİ BEKLEYEN](waiting.md)
- [Notlarla inceleme](../scenarios/review-with-notes.md)
- [Kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md)
- [Klavye](keyboard.md)
