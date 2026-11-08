---
title: Değişiklik görünümü
description: Bir işçinin değişikliğini (Δ) okumak - diff, dosyaları ve türleri, içinde hareket, satırlara notlar, parça seçmek, bir parçayı işçiye alıntılamak, uygulamak ve reddetmek.
---

# Değişiklik görünümü

Bir işçi `done` gönderdiğinde imece worktree'sini okur ve değişiklik KARAR BEKLEYEN'de `Δ` olarak sizi bekler. Onda `enter` (ya da sıradaki öge oysa `W`) değişiklik görünümünü ekibin yerinde açar: bütün diff, üstünde hükmü. Kararı burada verirsiniz: `a` checkout'unuza uygular, stage'de; `r` geri gönderir; `n` satırlarındaki notlarınızla geri gönderir; `x` ve `a` yalnız seçtiğiniz parçaları uygular; `b` checkout'unuzla çakışanı geri gönderir.

```text
┌ DEĞİŞİKLİK · claude-1 · 3 dosya +41 −6 · parser boş dosyada hata döndürür ─────────────────┐
│✓ onaya hazır · ✓ cargo test · 3 dosya, hepsi #6 içinde · başka değişiklikle çakışmıyor     │
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
  j k  w kaydır · sar  [ ] dosya  a x onayla · parça seç  r geri gönder  > ona alıntıla  c n not · notları gönder  v satır kopyala  esc geri  ? yardım
```

## Detay

Bir değişiklik, bir konuşma ve bir odak, tek ekranın üç sekmesidir: bir işin detayı (kayıt 125). Sekmenin kendi çerçevesinin üstünde bir baş satırı kartı ve başlığını, projeyi, ajanı, ne zamandır böyle olduğunu ve kaça mal olduğunu söyler; diff'te dosyalar ve satırlar, birden çok değişiklik bekliyorsa sağda `değişiklik 2 / 5`. Ajanın açık bir sorusu varsa soru, sunduğu yanıtlar ve tuşlarıyla (`1`-`9` birini seçer, `/` kendinizinkini yazar) başın altında durur, hangi sekme açık olursa olsun. Sonra sekme çubuğu, `diff · konuşma · log`: açık sekme kalın, gösterecek bir şeyi olmayan (değişiklik yok, henüz bir şey söylenmemiş) soluk; `tab` ve `⇧tab` bir şeyi olan sonraki ve önceki sekmeye gider. Diff bu sayfadır; konuşma ajanla [konuşmadır](conversations.md) (varsa sizinle, yoksa bir ekip arkadaşıyla en yenisi); log odağıdır, ya da `enter` açtıysa ham akışı. `esc` sekmeyi eskisi gibi bırakır: konuşma altındakine kapanır, diff ve odak Ana ekrana ya da ekibe döner.

```text
#6 E-posta şablonları · shop-api · claude-9 · 6 dk · $0.06 · 3 dosya +41 −6                           değişiklik 1 / 3

diff · konuşma · log                                                                                    tab geçer
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│✓ onaya hazır · ✓ cargo test · 3 dosya, hepsi #6 içinde · başka değişiklikle çakışmıyor     │
```

## Yamanın üstündekiler

**Hüküm satırı** önce gelir: Δ geçidi tek satırda, `✓ onaya hazır` ya da `! önce bak`, ardından dayandığı olgular, her biri kendi renginde, pencere darsa sarılmış:

```text
✓ onaya hazır · ✓ kontrol · ✓ dotnet test · 2 dosya, ikisi de #6 içinde · başka değişiklikle çakışmıyor · evetinden sonra Lonca şunları çalıştırır: build, test
! önce bak · ✗ dotnet test: exit 1 · 3 dosya · 1 kartın dışında (#18): Cargo.toml · #51 ile Money.cs üzerinde çakışıyor
```

- **Neyin kanıtladığı:** çalıştıysa deponun [kontrolü](../concepts/changes.md#deponun-kontrolü) (`✓ kontrol`, ya da satırın altında çıktısının son satırlarıyla `✗ kontrol · exit 1, 3.0 s`), sonra işçinin imece'nin `run` aracıyla tam bu ağaçta yaptığı her `run`, nasıl bittiğiyle (`✓ cargo test`, `✗ cargo clippy: exit 1`, süre dolunca durduruldu, durduruldu); ikisi de yoksa uyarı renginde `hiçbir şey koşmadı`. Claude Code'un kendi kabuğu sayılmaz; imece onun hangi ağaçta koştuğunu görmez.
- **Kartın yollarına göre dosyalar:** işçinin kartının yolları varsa ([kartın sahip olduğu yollar](../concepts/messages-and-board.md#kartın-sahip-olduğu-yollar)) ve her dosya içlerindeyse `2 dosya, ikisi de #6 içinde`; bazıları değilse `3 dosya · 1 kartın dışında (#18): Cargo.toml`, en çok üç ad, kalanı sayılmış; kartın yolu yoksa yalnız `3 dosya`. Kart, değişikliğin yapıldığı karttır, bitmiş olsun olmasın.
- **Çakışma:** projede bekleyen ve bu değişikliğin dokunduğu bir dosyaya dokunan her başka değişiklik için `#51 ile Money.cs üzerinde çakışıyor` (gerisi sizin yapacağınız bir birleştirmedir); Lonca'da ayrıca depoda bekleyen ve burada hiçbir değişikliğin göstermediği her öneri için, başkasının ya da turu bitmemiş bir işçinin, numarası ve kimin yaptığıyla `12 numaralı öneri (ayse) ile Money.cs üzerinde çakışıyor` (imece Lonca'nın onun için listelediği yolları okur); başkaları bekliyor ve hiçbiri dosya paylaşmıyorsa `başka değişiklikle çakışmıyor`; bekleyen tek değişiklik buysa hiçbir şey.
- **Checkout:** reddedilen bir uygulamadan sonra `çalışma kopyana uymuyor` (alttaki kırmızı satır nerede olduğunu söyler, `b` geri gönderir).
- **Lonca:** Lonca bir dosyayı redakte ettiyse ya da yamayı kestiyse `yama tam değil`; önerinin temelinden bu yana gövde ilerlediyse `temeli gövdenin gerisinde, evetinden sonra birleştirilir`, hazır olmasını bozmayan bir olgu, çünkü birleştirme kuyruğu onu birleştirir ([Lonca'da](../concepts/changes.md#loncada)); bir öneri için `evetinden sonra Lonca şunları çalıştırır: build, test` ya da `evetinden sonra hiçbir şey çalışmaz` ([Lonca'da ne denetlenir](../concepts/changes.md#what-is-checked-on-lonca)).

Her kanıt geçtiyse, her dosya kartın yollarının içindeyse, hiçbir şey çakışmıyorsa, uyuyorsa ve bütün gösteriliyorsa hazırdır; bunlardan biri eksikse `! önce bak`, satır da hangisi olduğunu söyler. Aynı olgular, baş olmadan, [Ana ekran](home.md)da değişikliğin satırını izler ve hazır olanlar orada önce gelir. Bir nottur, asla kapı değil: `a`, satırın önce bak dediği bir değişikliği de onaylar.

Sonra, işçi `done`'ında ilk satırdan fazlasını yazdıysa söyledikleri: neden, ne kararlaştırdı, nereye bakılmalı; en çok dört satır, fazlası `…` ile kesilir. Birden çok değişiklik bekliyorsa pencerenin sağ üstü bunun hangisi olduğunu söyler: `değişiklik 2 / 5`. Onun altında:

1. **Kontrolün durumu**, henüz bitmediyse: çalışırken `● kontrol bu değişiklikte çalışıyor…`; sorusunu yanıtlayana dek `▶ deponun kontrolü çalışmak istiyor: KARAR BEKLEYEN'de`; ya da neden çalışamadığı. Başarısız bir kontrolün son çıktı satırları da burada durur.
2. **Maliyeti**, işçinin turu bitince: `maliyet · $0.84 · 12 dk`, soluk; işçinin (yardımcıları dahil) son değişikliğinden sonraki ilk isteminden bu `done`'a harcadığı ve çalıştığı süre; geri gönderilip yeniden gönderilen bir değişiklik turlarını ekler (`$1.20 · 25 dk · 3 tur`). Aynı rakam KARAR BEKLEYEN'de ögenin satırını bitirir ve günlük söyler (`claude-1 değişikliği $0.84 · 12 dk tuttu`). Tur ortasında okunan bir `done` onu bir an sonra, turun maliyeti bilinince gösterir.
3. **Türe göre satırlar**, dosyalar birden çok türdense: `kaynak +23 −6 (2) · testler +18 −0 (1) · üretilmiş +7 −5 (1)`. Bir dosya yalnız yoluna göre sınıflanır: bir lock dosyasıysa (`Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `go.sum`, `poetry.lock`, `Gemfile.lock`, `composer.lock`), küçültülmüş ya da üretilmiş kodsa (`.min.js`, `.min.css`, `.pb.go`, `_pb2.py`, `.g.dart`, `.Designer.cs`) ya da `generated/`, `dist/` ya da `vendor/` altındaysa üretilmiş; `tests/`, `test/`, `__tests__/` ya da `spec/` altındaysa, `test_*` adlıysa ya da bir test sonekiyle bitiyorsa (`_test.go`, `_test.rs`, `_test.py`, `.test.ts`, `.spec.ts`, `Tests.cs`, `Test.java`, …) test; yoksa kaynak. Üretilmiş, teste baskındır. Dosyalardan hiçbir şey okunmaz. Yirmi satır kod ve yeniden üretilmiş bir lock dosyası olan bir `+400 −20` değişikliği bunu burada söyler.
4. **Dosya listesi**, iki ya da daha çok dosyalı bir değişiklik için: her dosya `+N −M` ya da `ikili` ile, en çok altı, kalanı sayılmış (`… 2 dosya daha`). Tek dosyalı değişiklik liste göstermez; başlığı söyler. 120 kolondan itibaren iki ya da daha çok dosyalı bir değişiklik hepsini yamanın yanında listeler, eldeki dosya `▸` ile işaretlidir ve `[` `]` aralarında gezer.

Yama, git'in `diff --git` yazdığı yerde dosyanın yolunu gösterir ve ilk hunk'ın üstündeki `index`, `---`, `+++` satırlarını atlar. Dosyanın aynı biçimde başlayan bir satırı yerinde kalır. Kopyalama, notlar ve alıntılar yine git'in yazdığı yama üzerinde çalışır.

## Hareket

| Tuş | Ne yapar |
| --- | --- |
| `j` `k` / `↑` `↓` | bir satır aşağı / yukarı. En üst satır eldeki satırdır, seçim gibi çizilir; satırlar altında kayar |
| `gg` / `G` | ilk / son satır |
| `^d` `^u` | yarım sayfa aşağı / yukarı |
| `^f` `^b`, `pgdn` `pgup`, `space` | bir sayfa aşağı / yukarı |
| `[` `]` | önceki / sonraki dosya: `diff --git` başlıkları |
| `w` | uzun satırları alt alta sar (küçültülmüş bir dosya, uzun bir dize); yine basılınca kenarda keser. Kaydırma yine yama satırlarını sayar, böylece `j` kaç satır kaplasa da bütün bir satır gider. Kapatılana dek her değişiklik için açık; çalıştırmalar arasında saklanmaz |
| `R` | ikinci okuma (`:review`), dosya listesinin üstünde: ilk altı satırı ya da tamamı ([değişiklikler](../concepts/changes.md#ikinci-bir-okuma)) |
| `v` | kopyalanacak satırları seç (aşağıda) |
| `esc` | KARAR BEKLEYEN'e geri |

Eklenen ve silinen satırların kendi renkleri vardır; başlıklar öne çıkar. Her satır çizilen her şeyin geçtiği temizlikten geçer.

## Karar

| Tuş | Ne yapar |
| --- | --- |
| `a` | onayla: değişiklik checkout'unuza uygulanır, stage'de, bakıp commit etmeniz için. Parça seçiliyse (`x`) yalnız onları. [Ana ekrandan](home.md) açıldıysa sıradaki değişiklik açılır; sonuncudan sonra ana ekrana döner |
| `r` | geri gönder: öge kapanır, değişiklik worktree'de kalır, nedenini yazmanız için yazma satırı `@worker ` ile açılır |
| `c` | en üstteki satıra bir not |
| `n` | notları işçiye tek mesajda gönder; değişiklik geri döner |
| `x` | en üstteki parçayı seç; `a` sonra yalnız seçili parçaları uygular |
| `>` | en üstteki parçayı alıntılayarak işçiye yaz |
| `b` | reddedilen bir uygulamadan sonra: çakışmayı geri gönder (değişikliği çakışan yerleriyle); `r`'deki gibi geri döner |

### Uygulamak

`a` uygulamayı yalnız başlatır: flaş sürdüğünü söyler, günlük nasıl bittiğini. Git imece'nin kendi iş parçacığında çalışır, ekran hiç beklemez. Yama `update-index --refresh` ardından `git apply --index` ile girer; bu, kendi stage'lenmemiş düzenlemeleriniz dışında bir nedenle başarısız olursa önce index'in bir kopyasına üç yönlü birleştirilir ve yalnız kopya temizse gerçekten uygulanır; böylece bir çakışma `checkout ile a, b dosyalarında çakışıyor` diye reddedilir ve hiçbir şey yarım uygulanmaz. Değişikliğin dokunduğu bir dosyadaki kendi stage'lenmemiş düzenlemeniz git'in sözleriyle reddedilir (`does not match index`): hiçbir birleştirme işinizin üstünden geçmez. Uygulamadan sonra worktree bir işaret olarak commit'lenir (yazar `imece`), böylece işçinin sonraki `done`'u yalnız ondan sonra geleni gösterir ve temiz bir worktree commit ettiğinizde HEAD'inizi izler. İşçi bunu sonraki prompt'unda öğrenir.

### Bir çakışmayı geri göndermek

Git bir değişikliği reddettiğinde imece parçalarından hangilerinin checkout'unuza uymadığını da bulur: index'inizin bir kopyası, uygulamadaki düz ve üç yönlü denemelerin aynısı, sizin hiçbir şeyinize dokunulmaz. Günlük satırı o zaman `· değişiklikte b onu geri gönderir` ile biter, değişiklik kırmızı bir satırın altında açılır ve altbilgide `b  geri gönder` görünür:

```text
 ✗ checkout'a (4f3a2c1) uymuyor: CHANGELOG.md · b onu claude-1 adlı ajana geri gönderir
```

`b` işçiye tek mesaj gönderir: HEAD'in kısa kimliği, ne yapacağı (worktree'sini değişikliği üstünde, commit'lenmemiş olarak o commit'e getirmek, çözmek, yeniden `done` göndermek) ve her çakışma dosyasının altında, `@@` satırı ve git'in sözleriyle. Checkout'unuzda stage'de olup commit edilmemiş bir dosya, onu gösteren komutla birlikte anılır. Öge `r`'deki gibi kapanır, günlük reddedildi der, worktree değişikliği tutar ve işçinin sonraki `done`'u yeni bir `Δ` olur. imece kendisi hiçbir şeyi çözmez ve worktree'yi taşımaz.

Hiçbir şey çakışmıyorsa (kendi stage'lenmemiş düzenlemeleriniz bir uygulamayı durdurur ama hiçbir parçayla çakışmaz: commit'leyin ya da stash'leyin ve `a` tuşuna basın), işçi gitmiş ya da durmuşsa veya bütçe bitmişse `b` nedenini söyler ve `Δ`'yı olduğu gibi bırakır. Giden, son `a`'nın bulduğudur: checkout'unuz o zamandan beri ilerlediyse `a` tuşuna yeniden basın. En çok 200 parça dener, sonra bunu söyler.

Reddedilir: o değişiklik uygulanırken ya da aynı işçinin daha yeni bir `done`'u hâlâ okunurken `a`.

### Reddetmek

`r` ögeyi kapatır ve worktree'yi olduğu gibi bırakır; yazma satırı nedenleriniz için `@claude-1 ` ile açılır ve işçi nedenlerinizi, imece'den değişikliğinin reddedildiği notuyla alır. Sonraki `done`'u yeni bir Δ'dır.

### Lonca'daki bir öneri

Lonca workspace'indeki bir işçinin Δ'sı önerisidir ([daha fazlası](../concepts/changes.md#loncada)). Yamanın üstünde sizi neden beklediğini sözcüklerle söyler (`evetiniz gerekiyor: bu oturumun araçlarını kimse izlemedi · öneri 5` ya da `kritik bir yolu değiştiriyor: Cargo.toml`; kancaları çalışmayan bir oturum `bu oturumun kancaları çalışmadı` der ve imece işçisini durdurmuştur ([daha fazlası](../concepts/projects.md#loncada)); imece'nin sözcüğü olmayan neden Lonca'nın yazdığı gibi durur), varsa `· 1 eski öneriyi kapsıyor` da; Lonca'nın sansürlediği dosyalar ya da kestiği bir yama için göze çarpan satırlar gelir; onayından sonra ne çalışacağını hüküm satırı söyler. `a` onaylar. `r` yazma satırını `@claude-1 ` ile açar ve `gerekçeni yaz` der; gönderdiğiniz satır öneriyi reddeder ve işçiye gider, `esc` ise onu bekler bırakır. `n` ile gönderilen notlar da aynı şekilde bir rettir. `x` `bir öneri bütün olarak onaylanır` der, `b`'nin de gönderecek bir şeyi yoktur: çakışma kuyruğundur ve işçiye kendisi söyler.

### Satırlara notlar

Bir parçanın satırında `c` (bağlam, eklenen ya da silinen; başlık ya da `@@` satırı değil) komut satırını bir not için açar, yeriyle etiketli: `src/parser/manifest.rs:14 için not ▸`. `enter` tutar, `esc` vazgeçer, notu olan bir satırda `c` yeniden yazar, boş bir not kaldırır. Not yazılırken ad ya da komut tamamlanmaz ve alt satır gönder değil `enter tut` der. Bir not satırının altında `✎` ardından görünür. Yer, dosyanın yeni adı ve oradaki satır numarasıdır, silinen bir satır için eksi işaretiyle eski dosyadaki numarası, `src/p.rs:−7`.

`n` değişikliğin notlarını satır sırasında işçisine tek mesajda gönderir: değişikliğin worktree'sinde kaldığını, uygulanmadığını, dediklerine göre değiştirip yine `done` göndermesini söyleyen bir satır; sonra her not, yerinin ve satırın kendisinin altında bir `diff` çitinde. Değişiklik `r`'nin geri gönderdiği gibi geri döner, günlük reddedildiğini söyler ve mesaj izler; flaş notları sayar. Not yoksa `n` nasıl yazılacağını söyler. Notlar ekranın belleğinde bu çalıştırma boyunca yaşar, yamanın özetiyle (hash) tutulur: işçiden daha yeni bir Δ'nın satırları başkadır, bu yüzden eski notlar onunla gösterilmez ve gönderilmez.

### Değişikliğin bir parçası

`x` en üstteki parçayı seçer (en üst satırın içinde olduğu parça; bir dosya başlığında sonraki); yine `x` bırakır. Seçili parçanın `@@` satırı `✓` taşır ve flaş sayar: `1 / 3 parça seçili; a yalnız onları uygular, kalanı bekler`. Parça seçiliyken `a` yalnız onları uygular; hiçbiri seçili değilse bütün değişikliği.

Git iş parçacığında parça, bir şey uygulanmadan önce kendi başına bir değişiklik olur: seçili parçalar dosyalarının başlığı altında (parçası olmayan dosya dışarıda kalır, parçası olmayan ikili dosya da), imece'nin kendi index'i üzerinden okunur, böylece işçinin dosyalarına ve index'ine dokunulmaz, ve bütün bir değişiklik gibi uygulanır, temizse üç yönlü. Worktree yalnız o parçayla işaretlenir, kalanı hemen yeniden okunur ve aynı başlıkla yine bir değişiklik olarak KARAR BEKLEYEN'de bekler. İşçi değişikliğinin bir parçasının uygulandığını (`1 of its hunks`) ve kalanın beklediğini öğrenir. Oturmayan bir parça, uygulanmayan bir değişiklik gibi reddedilir. Seçim yamanın özetiyle bu çalıştırma boyunca tutulur; yeni bir Δ hiçbiriyle başlar. Bir parça içinden tek satırlar seçilemez.

### Bir parçayı alıntılamak

`>` değişikliğin işçisine bir mesaj başlatır, `@claude-1 ` ve imleç ardında, satırınızın altında dosyanın `---` ve `+++` satırlarını ve en üstteki parçayı tutan bir `diff` çiti: en çok 40 satır, kalanı `… +N` diye sayılmış (işçide parçanın tamamı worktree'sinde var). `enter`'dan önce hepsini düzenlersiniz; düz bir mesajdır, söylendi diye günlüğe geçer. Alıntılanacak parça yoksa (boş bir değişiklik) flaş söyler.

### Satır kopyalama

`v` eldeki satırdan başlayarak kopyalanacak satırları seçer: başlık `KOPYALA · 3 seçili · y yazıldığı gibi · Y yeni hali · v esc çık` olur, alt satır yalnız bu tuşları listeler. `j` `k`, sayfa tuşları ve `gg` `G` imleci gezdirir; başladığınız yerle imleç arasındaki satırlar işaretlenir, imlecinki kalın; satırlar sığdıkça görünüm başladığınız yeri en üstte tutar, sığmazsa imleci olabildiğince az kayarak izler.

| Tuş | Ne yapar |
| --- | --- |
| `y` | satırları yamadaki gibi kopyala, her biri `+`, `-` ya da boşluğuyla |
| `Y` | yeni hali kopyala: yalnız parçaların eklenen ve kalan satırları, işaretsiz, kod olarak yapıştırmaya hazır. Silinen satırlar, dosya başlıkları ve `@@` satırları dışarıda kalır; geriye bir şey kalmazsa flaş söyler ve satırlar seçili kalır |
| `v` `esc` | kopyalamadan çık |

Seçerken başka tuşlar bir şey yapmaz, yani `a` değişikliği yanlışlıkla uygulamaz; `?` ve `q` yine çalışır. Kopya, konuşmadaki `c` gibi sistemin panosuna ya da terminalinkine (OSC 52) gider ve her satır önce temizlenir. İşçiden daha yeni bir Δ ya da değişiklikten çıkmak seçimi bitirir.

## Günlük sonra ne der

```text
14:05 ✓ claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 7f3a2c1
14:06 ■ claude-1 değişikliği uygulanamadı: checkout ile CHANGELOG.md dosyasında çakışıyor
14:07 › reddedildi: claude-1
14:07 › sen → claude-1: boş dosya denetimi çağıranda değil `read` içinde olmalı …
```

Bir `done`, `claude-1 bitirdi: değişiklik hazır · 3 dosya +41 −6` diye gelir; bir uygulama `… checkout'a uyguladın, stage'de; çalışma ağacı işareti <sha>`; bir başarısızlık `… uygulanamadı: <git'in sözleri>` ve öge kalır; bir ret `geri gönderildi: claude-1`, ardından mesajınız. Kısmi bir uygulamadan sonra kalan yeni bir `bitirdi` satırı olarak döner.

## Demo

`--demo` aynı değişiklikleri kartlarından ve çalıştırmalarından yapar ve `a`, `r`, `n`, `x`, `b`, `>` ve `v` tuşlarını aynı şekilde alır (başarısızlığı göstermek için orada sekiz uygulamadan biri başarısız olur). Lonca projesinde işe alınan işçi öneri yapar; onay demonun trunk'ına iner ya da sekizde bir kuyruğun yapamadığı bir birleştirme olarak geri döner. Her demo checkout'unun bir `.imece/check`'i vardır; ilk değişiklikte sorulur ve her üçüncüde başarısız olur; her değişiklik uydurma harcamasının neye geldiğini söyler. Demodaki Lonca projesinde bir iniş denetimlerini günlükte söyler, her beşinci iniş testlerinde kalır ve işçi işi yeniden alır.

## Ayrıca bakınız

- [Değişiklikler](../concepts/changes.md)
- [KARAR BEKLEYEN](waiting.md)
- [Notlarla inceleme](../scenarios/review-with-notes.md)
- [Kendi kabuğu ve commit](../scenarios/own-shell-and-commit.md)
- [Klavye](keyboard.md)
