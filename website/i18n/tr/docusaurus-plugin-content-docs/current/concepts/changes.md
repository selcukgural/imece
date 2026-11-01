---
title: Değişiklikler (Δ kapısı)
description: Bir işçinin işi checkout'unuza nasıl ulaşır - done'ı SENİ BEKLEYEN'de bir değişiklik olur; uygularsınız, reddedersiniz, not yazarsınız ya da bir kısmını uygularsınız; worktree bu sırada ne yapar.
---

# Değişiklikler (Δ kapısı)

Bir değişiklik (`Δ`), bir işçi `done` dediğinde worktree'sinin tuttuğu şeydir. SENİ BEKLEYEN panelinde sizi bekler ve siz uygulamadan hiçbir parçası checkout'unuza ulaşmaz. Bu, ajanlarla çalışma kopyanız arasındaki kapıdır; açan sizsiniz.

## `done`'dan Δ'ya

1. Bir işçi (yalnız İşçi rolü; bir yardımcının `done`'ı işçisinin işidir, lider hiçbir şeyi değiştirmez) `done` gönderir.
2. imece'nin git iş parçacığında worktree özel bir index dosyası üzerinden okunur: `read-tree HEAD`, `add -A`, `write-tree`, HEAD'ine karşı `diff --cached --binary`. İzlenmeyen dosyalar sayılır; ajanın kendi index'ine dokunulmaz. 4 MiB'ı aşan bir yama reddedilir ve işçiye söylenir.
3. Sonuç SENİ BEKLEYEN'de bir `Δ` öğesidir, ayrıntısı `done`'ın ilk satırı; günlük `claude-1 bitirdi: 3 dosya, +34 −8 → seni bekliyor` der. Aynı işçiden daha yeni bir `done` eskisinin yerine geçer.
4. İşçinin `send` yanıtı okumayı bekler ve değişikliği gördüğünüzü söyler.

```text
 SENİ BEKLEYEN (2)
 Δ 14:02 claude-1 bitirdi: parser tokens (3 dosya, +34 −8) · ✓ bu ağaçta temiz geçti: cargo test -p parser
 ? 14:05 codex-1 sana soruyor: eski CLI bayrağı kalsın mı?
  enter diff'i gör · a checkout'a uygula · r reddet
```

### Satırdaki kanıt

İşçinin ya da bir yardımcının imece'nin `run` aracıyla yaptığı bir çalıştırma, üzerinde koştuğu ağaca bağlanır (worktree öncesinde ve sonrasında okunur; ağacı değiştiren bir çalıştırma hiçbirine sayılmaz). Bir `done` okununca, tam o ağaçtaki çalıştırmalar onunla gelir: `Δ` satırı `✓ bu ağaçta temiz geçti: …`, `✗ bu ağaçta başarısız: …` ya da `bu ağaçta çalıştırma yok` ile biter ve diff her çalıştırma için bir satırla başlar. Gösterilir, asla kapı değildir: çalıştırması olmayan bir değişiklik yine uygulanabilir; karar sizin. Satıcının kendi kabuğu sayılmaz; imece onun hangi ağaçta koştuğunu görmez.

## Okumak

`enter` diff'in tamamını açar: `j k` kaydırır, `^d ^u ^f ^b` sayfalar, `gg G`, `[ ]` dosyadan dosyaya. Her satır çizilmeden önce temizlenir. Yamanın üstünde:

- ağacındaki çalıştırmalar (ya da derlenip testleri geçtiğinin bilinmediği);
- dosyalar birden çok türdense türe göre satırlar: `kaynak +20 −1 (2) · testler +18 −0 (1) · üretilmiş +7 −5 (1)`. Türler yalnız yola göredir: lock dosyaları, minified ve üretilmiş kod ile `generated/`, `dist/`, `vendor/` dizinleri üretilmiş; `tests/`, `test/`, `__tests__/`, `spec/`, `test_*` ve alışılmış test sonekleri test; kalanı kaynak. Üretilmiş, testi yener;
- iki ya da daha çok dosyalı bir değişiklik için her birinin `+N −M`'si (ya da `ikili`) ile dosya listesi, en çok altı, kalanı sayılır.

`w` uzun satırları altlarına sarar (`w` yine kenarda keser). En üst satır eldeki satırdır: `c`, `>` ve `x` ona etki eder.

## Uygulamak

`a` uygulamayı git iş parçacığında başlatır; anlık ileti sürdüğünü, günlük nasıl gittiğini söyler.

- Checkout'unuzda `git apply --index`, atomik: değişiklik **stage'e** iner, bakıp commit etmeniz için. Önce `update-index --refresh` çalışır; dokunulmuş ama değişmemiş bir dosya uyumsuzluk sayılmaz.
- **Temizse üç yollu:** düz uygulama sizin stage'lenmemiş düzenlemelerinizden başka bir nedenle başarısız olunca yama, index'inizin bir kopyasına (`<git dir>/imece-apply-index`) üç yollu birleştirilir; bu hiçbir dosyaya dokunmaz. Yalnız o temizse gerçek `git apply --index --3way` çalışır. Bir çakışma `conflicts with the checkout in a, b` diye reddedilir; aynı dosyadaki stage'lenmemiş düzenlemeniz git'in kendi sözleriyle reddedilir (işinizin üzerinden birleştirme geçmez). Hiçbir şey yarım uygulanmaz.
- **İşaret:** uygulamadan sonra worktree'nin incelenen ağacı orada yazar `imece <imece@localhost>` olarak commit edilir ve worktree ona sıfırlanır (dosyalara dokunulmaz, index = HEAD). İşçinin sonraki `done`'ı yalnız sonrasında geleni gösterir ve kendi commit'lerinin hepsi imece'nin olan bir worktree HEAD'inizi izlemeyi sürdürür.
- Günlük: `claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 4f3a2c1`. İşçi sonraki isteminde imece'den bir not alır ("From imece, not from the person or a teammate").

`a`, o değişiklik uygulanırken ya da işçinin daha yeni bir `done`'ı hâlâ okunurken reddedilir; `r`, uygulanırken reddedilir.

## Reddetmek

`r` öğeyi kapatır; değişiklik worktree'de kalır ve yazma satırı nedenini söylemeniz için `@claude-1 ` ile açılır. İşçi sonraki isteminde değişikliğin uygulanmadığını öğrenir.

## Notlar

Uygula ve reddetin yanındaki üçüncü yanıt:

- Bir parçanın satırında (bağlam, eklenen ya da silinen; başlık ya da `@@` satırı değil) `c`, komut satırını satırın yeriyle etiketlenmiş açar: `src/p.rs:12 için not ▸`. `enter` tutar, `esc` bırakır, `c` yine yeniden yazar, boş bir not kaldırır. Not, satırının altında `✎` ile gösterilir.
- Yer, dosyanın yeni adı ve satırın oradaki numarasıdır; silinen bir satır için eski dosyadaki numarası eksi işaretiyle: `src/p.rs:−7`.
- `n` değişikliğin bütün notlarını işçisine tek mesajda gönderir, satır sırasıyla, her biri `dosya:satır`ının ve satırın kendisinin altında bir `diff` çitinde; bir satır da değişikliğin worktree'sinde kaldığını, uygulanmadığını, dediğiniz gibi değiştirip yeniden `done` göndermesini söyler. Değişiklik ret gibi geri döner: öğe kapanır, günlük reddedildi der, işçinin sonraki `done`'ı yeni bir Δ olur.

Notlar ekranın belleğinde, yamanın hash'ine karşı yaşar: daha yeni bir Δ notsuz başlar ve notlar çalıştırmalar arasında saklanmaz. İşlenmiş bir örnek [notlarla inceleme](../scenarios/review-with-notes.md) sayfasında.

## Bir parçayı alıntılamak

Bir değişiklikte `>`, işçisine bir mesaj başlatır: imleç ardında `@claude-1 `, altında en üst satırın içinde bulunduğu parça, dosyasının `---`/`+++` satırlarıyla bir `diff` çitinde: en çok 40 satır, kalanı `… +N` olarak sayılır. `enter`'dan önce hepsini düzenleyebilirsiniz; işçiye düz bir mesajdır.

## Değişikliğin bir kısmını uygulamak

- `x` en üstteki parçayı seçer (bir dosya başlığındayken sonrakini); `@@` satırı `✓` taşır; `x` yine bırakır. Anlık ileti sayar: "1 / 3 parça seçili; a yalnız onları uygular, kalanı bekler".
- Parçalar seçiliyken `a` yalnız onları uygular: seçili parçalar git iş parçacığında kendi başına bir değişiklik olur (hiç parçası olmayan bir dosya dışarıda kalır, ikili dosya da), imece'nin kendi index'i üzerinden okunur, bütün bir değişiklik gibi uygulanır (temizse üç yollu) ve worktree yalnız o kısımla işaretlenir.
- Kalan, worktree'nin yeni HEAD'ine karşı tuttuğudur; hemen yeniden okunur ve SENİ BEKLEYEN'de aynı başlıkla yine bir değişiklik olarak bekler. İşçi hangi kısmın uygulandığını ve kalanın beklediğini öğrenir.

Seçim yamanın hash'ine karşı yalnız bu çalıştırma için tutulur. Bir parçanın içinden tek satırlar seçilemez.

## Bu sırada worktree

- Bir işçinin satırı worktree'sinin nerede durduğunu söyler: `⎇ fix-parser ↑2 ↓1 +12 −3`. `⎇ ad` ajanın açtığı bir daldır (detached iken yok), `↑N` HEAD'inizde olmayan kendi commit'leri (imece'nin işaretleri sayılmaz), `↓N` onda olmayan HEAD commit'leriniz, `+N −M` sonraki `done`'ının size vereceği satırlar, izlenmeyen dosyalar dahil. Her parça yalnız söyleyecek bir şey varken gösterilir; oturum başlarken, bir tur bitince, bir uygulamadan ve bir taşımadan sonra ve tur ortasında her 15 saniyede okunur. Çalıştırmalar arasında saklanmaz.
- **HEAD'i izlemek:** kendi commit'lerinin hepsi imece'nin olan temiz bir worktree, turlar arasındaki bir işçinin her isteminden önce HEAD'inize taşınır (yardımcılar da), günlüğe `çalışma ağacı … yerine HEAD … oldu` yazılır. Commit edilmemiş değişiklikleri ya da kendi commit'leri olan yerinde kalır ve günlük söyler. Böylece uyguladığınızı commit edince işçiler yeni HEAD'inizden devam eder.
- Elle silinmiş ama git'in hâlâ listelediği bir worktree, kaybolanla ve `git worktree prune` ile reddedilir; ajan o nedenle durur.

## Commit etmek

imece checkout'unuzda hiçbir şeyi commit etmez. `:commit`, stage'dekiler için Claude'un ucuz kademesinden bir mesaj ister ve komut satırına `!git commit -m '…'` koyar; kendi kabuğunuzda düzenleyip çalıştırırsınız. Kendi tavanları vardır ([bütçe](../concepts/budget-and-limits.md)) ve [kendi kabuğunuz ve commit](../scenarios/own-shell-and-commit.md) sayfasında adım adım anlatılır.

## Ayrıca bakınız

- [Değişiklikler ekranı](../screen/changes.md): diff görünümündeki her tuş.
- [SENİ BEKLEYEN](../screen/waiting.md): `W`, `S`, öğelerin sırası.
- [Tek düzeltme](../scenarios/one-fix.md) ve [notlarla inceleme](../scenarios/review-with-notes.md).
- [Güvenlik](../concepts/security.md): kapı neden sizin.
