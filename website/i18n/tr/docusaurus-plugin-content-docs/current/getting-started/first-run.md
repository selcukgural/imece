---
title: İlk çalıştırma
description: Bir checkout'ta `imece` yazmaktan ilk uygulanan değişikliğe, her adımda tuşlarıyla.
---

# İlk çalıştırma

Bu, tek bir oturuşun yürüyüşüdür: imece'yi bir checkout'ta başlatırsınız, liderden bir düzeltme istersiniz, bir işçi bunu kendi worktree'sinde yapar, değişikliğini okur ve uygularsınız, çıkarsınız. Burada adı geçen tuşlardan fazlası gerekmez; her biri `?` yardımında vardır.

## Başlangıç

```sh
cd sizin/checkout
imece
```

Dizin commit'i olan bir git checkout'u olmalıdır (`--repo DİZİN` başkasını adlandırır); git'siz bir klasörde yalnız bir solo başlar (`imece --solo claude`, bkz. [solo oturumlar](../concepts/solo.md#gitsiz-bir-klasörde)).

### Hoş geldin penceresi

İlk başlangıçta (depo önceki bir çalıştırmayı hatırlamıyorsa), bir şey hareket etmeden önce bir pencere açılır: imece nedir, bu makinedeki kodlama agent'ları ve başlamak için tuşlar.

```text
┌ IMECE 0.1.0 SÜRÜMÜNE HOŞ GELDİN ─────────────────────────────────────────────────┐
│                                                                                  │
│       ██        imece v0.1.0                                                     │
│   ██  ▄▄  ██    tek terminal, birçok agent, bir lider                            │
│     ▄▄▀▀▄▄      checkout'un · lider claude                                       │
│     ▀▀  ▀▀                                                                       │
│                                                                                  │
│ Birçok kodlama agent'ı için tek terminal. Lider worker tutar; her biri kendi      │
│ worktree'sinde çalışır, değiştirdiği şey senin onayını bekler.                    │
│                                                                                  │
│ Bu makinedeki kodlama agent'ları:                                                │
│   ✓ claude  2.1.316 (Claude Code) · giriş yapılmış                               │
│                                                                                  │
│ Başlamak için tuşlar:                                                            │
│   /  i            lidere yaz: ne yapılmasını istediğini söyle                    │
│   @ad mesaj       tek bir agent'a yaz                                            │
│   W               seni bekleyen sıradaki şey: bir değişiklik, bir soru           │
│   …                                                                              │
│ herhangi bir tuş: kapat · :welcome bunu yine gösterir                            │
└──────────────────────────────────────────────────────────────────────────────────┘
```

En üstte imece'nin işareti durur: bir işin etrafında beş el; yanında sürümü, ne olduğu ve eldeki proje ile başında kimin olduğu. Ekrana geldiği yerde noktaları birer vuruşta gelir, iş en son (iki saniyeden kısa); herhangi bir tuş onu hemen bütün gösterir, [`:motion off`](../reference/commands.md#motion-offcalmfull), başka bir pencere ve SSH de öyle. Ona dar gelen bir ekranda yazılar tek başına kalır, fazla kısa bir ekranda pencere onlarsız açılır. Salon da, henüz hiçbir projede işçi yokken boş satırlarında aynı işareti gösterir. Terminalin başlığı da her şey sakinken işareti taşır: `⁙ imece`.

Satır Claude Code'un sürümünü ve giriş yapılmış olup olmadığını, ya da nasıl giriş yapılacağını söyler: `claude auth login`. imece bunu ona sorar (`claude auth status`), kendi iş parçacığında, ağa çıkmadan ve para harcamadan; cevabı gelene dek `bakılıyor…`, program yoksa `PATH'te yok` yazar.

Sonraki her başlangıçta imece yine sorar ve giriş yapılmadığını söylerse aynı pencereyi açar (sen yazarken bunun yerine bir satır `claude giriş yapmamış: :welcome nasıl yapılacağını söyler` der). `:welcome` onu her an açar ve yeniden sorar. Herhangi bir tuş onu kapatır ve başka bir şey yapmaz.

### Yeni olanlar

Yeni bir sürümün ilk başlangıcı, ondan önce başlayan sürümden bu yana yeni olanları bir kez gösterir (en çok üç sürüm), NEREDE KALMIŞTIN'dan önce:

```text
┌ IMECE 0.1.0 SÜRÜMÜNDE YENİ OLANLAR ──────────────────────────────────────────────────────┐
│ 0.1.0                                                                                    │
│   · İlk sürüm, macOS ve Linux için: tek bir curl satırı kurar ve günceller.               │
│   · …                                                                                    │
│ herhangi bir tuş: kapat · :news bunu yine gösterir                                       │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

En son başlayan sürüm depoda tutulur (`seen`); en ilk başlangıç bunun yerine hoş geldin penceresini gösterir. Herhangi bir tuş kapatır; `:news` son sürümlerin yeniliklerini her an gösterir.

### Nerede kalmıştın

İlk seferde gösterilecek bir şey yoktur. Önceki bir çalıştırmayı hatırlayan sonraki bir başlangıçta, bir şey hareket etmeden önce bir pencere açılır:

```text
┌ NEREDE KALMIŞTIN · 2 sa 10 dk önce ─────────────────────┐
│ $3.10 harcandı, bütçe $50.00; her agent kendisiyle konuşulana dek uyur │
│ imece: 3 agent, 1 açık kart                               │
│ 1 şey seni bekliyor:                                      │
│ Δ 11:42 claude-1 bitirdi: parser: sondaki virgülü kabul et │
│ kapanmadan önceki son satırlar:                           │
│ 11:42 claude-1 bitirdi: 2 dosya, +31 −4 → seni bekliyor   │
│ 11:43 `cargo test` çalıştırdın → çıkış 0                  │
│                            herhangi bir tuş: kapat        │
└───────────────────────────────────────────────────────────┘
```

Keseyi, açık proje başına bir satırı, hâlâ sizi bekleyen en eski öğeleri (en çok altı), imece kapanırken yarım kalan turları ve günlüğün son satırlarını listeler. Herhangi bir tuş kapatır ve başka bir şey yapmaz.

## Ana ekran {#the-salon}

Açılan [ana ekrandır](../screen/home.md): üstte sıranın sizde olduğu işler, altında çalışanlar, eldeki satır hemen yanında okunur. Henüz bir şey yoksa bunu söyler.

```text
 imece   hepsi boşta │ $0.00 / $50.00 harcandı  10:02
 hepsi   imece                                 / lead için yazar
 Sizi bekleyen yok, çalışan da yok. / lidere yazar.

 enter  aç   @  bu ajana yaz   j k  gez   [ ]  proje   /  lidere yaz   ?  yardım
```

Salon bir tuş uzakta, `1`: solda lonca, sağda SENİ BEKLEYEN ve DEFTER.

Takım yalnız liderdir (`○ lead`, siz yazana dek boşta). Salondaki günlüğün ilk satırı, `--confine strict` olmayan her başlangıçta bir kez, yalıtımın imece'nin işi olmadığını söyler.

## Lidere yazmak

`/` (ya da `i`) tuşuna basın. Alt satır yazma satırına dönüşür: ` › lead için: `. İstediğinizi yazın ve `enter` basın:

```text
 › lead için: CSV ayrıştırıcı son sütundaki sondaki virgülü reddediyor; kabul
              etsin, testiyle
```

Ekrandan uzun bir satır sonraki sırada sürer. Siz yazmadan önce alt satır `enter gönder · ⇧enter yeni satır · esc vazgeç` der.

Düz metin seçili projenin liderine gider. `@claude-1 metin` tek bir ajana giderdi. İstemden sonra `/` ile başlayan bir satır bir mesaj değil, ajanın kendi komutudur (`/cost`, `/compact`, `/clear`). `⇧enter` (ya da `\` `enter`) uzun bir istekte yeni satır başlatır; alt satır terminalinizde hangisinin çalıştığını söyler. `tab`, `@` sonrasında adları, `:` sonrasında komutları tamamlar.

## Lider ne yapar

Lider checkout'u okur (okuma araçları, web ve salt okunur komutları vardır; yazamaz: sizin checkout'unuzda çalışır, orada bir yazma incelemeyi atlardı). İsteği kartlara böler, panoya asar ve bir kart el gerektirince `hire` aracıyla bir işçi **tutar**. Model değil **katman** seçer: küçük mekanik iş için `cheap`, sıradan düzeltmeler için `standard` (varsayılan), kesen tasarım için `strong`; bunlar haiku, Claude Code'un varsayılanı ve opus demektir. En çok `--agents` işçi (varsayılan 4).

```text
   ● lead      koordine ediyor                                      $0.08
   ● claude-1  #1   sondaki virgülü kabul et   düşünüyor…
```

Günlük `lead panoya astı: #1 sondaki virgülü kabul et`, `lead işe aldı: claude-1 (varsayılan model)`, `⇄ lead → claude-1 [görev]: #1 …` der. Liderin kapanış metni size SENİ BEKLEYEN'de bir yanıt (`»`) olarak gelir: `enter` tamamını okur, `r` kapatır, ya da doğrudan yanıt yazarsınız.

Hesabınızda olmayan bir katman modeli ilk turunda düşer; imece o işçiyi standart modelde yeniden başlatır ve günlük `claude-1: opus modeli burada yok, varsayılanla devam` der.

## Bir işçinin satırı

```text
   ● claude-1  #1   sondaki virgülü kabul et   src/csv.rs düzenliyor   $0.41  +18 −3
```

Soldan sağa: glif (`●` çalışıyor, `○` boşta, `!` bir şeyi sizi bekliyor, `✓` bitti ve henüz bakılmadı, `■` takıldı: bloke ya da oturumu durdu), adı, kartı, şu an ne yaptığı, ne harcadığı ve worktree'sinin nerede durduğu: `+18 −3` sonraki `done` ile size vereceği satırlar, `⎇ ad` açtığı bir dal, `↑N` kendi commit'leri, `↓N` checkout'unuzun daha yeni commit'leri. Açık bir izin sorusu olan işçi `⚑ onayını bekliyor` gösterir ve soru SENİ BEKLEYEN'de bir öğedir: `a` izin verir, `r` reddeder (rutin komutlar, derleme, testler, git'in okuma komutları kendiliğinden çalışır).

Satırda `enter` işçinin odağını açar: kartının hikâyesi (niyet, varsayımlar, kararlar, konuşma, kanıt, değişiklikler). Bir `enter` daha (ya da `t`) izi açar, oturumunun ham akışı. `esc` geri döner.

## Değişiklik (Δ)

İşçi `done` gönderince imece onun worktree'sini tek yama olarak okur ve SENİ BEKLEYEN'e koyar. Günlük `claude-1 bitirdi: 2 dosya, +31 −4 → seni bekliyor` der, başlık `! 1 seni bekliyor`, terminalin başlığı `! imece · 1 seni bekliyor` olur.

```text
 SENİ BEKLEYEN (1) ──────────────────────────────────────────────────────
 Δ 10:19 claude-1 bitirdi: parser: sondaki virgülü kabul et (2 dosya, +31 −4) · ✓ bu ağaçta temiz geçti: cargo test
   enter diff'i gör · a checkout'a uygula · r reddet
```

Satırın sonu kanıttır: imece'nin `run` aracının tam bu ağaçta gördüğü çalıştırmalar. `✓ bu ağaçta temiz geçti: cargo test` (yeşil), `✗ bu ağaçta başarısız: …` (kırmızı) ya da `bu ağaçta çalıştırma yok` (uyarı). Gösterilir, kapı değildir: karar sizin.

- `w` doğrudan SENİ BEKLEYEN'e gider; `tab` paneller arasında geçer.
- `W` her yerden (bir kartın ekranı, bir diff, pano) sıradaki bekleyen öğeyi açar; yine basılınca sonrakini. `S` eldeki öğeyi sonraya erteler ve sonrakini açar.
- Değişiklikte `enter` diff'in tamamını açar:

```text
 DEĞİŞİKLİK · claude-1 · 2 dosya +31 −4 · sondaki virgülü kabul et ─────────
 ✓ cargo test
 kaynak +24 −4 (1) · testler +7 −0 (1)
 src/csv.rs +24 −4
 tests/csv.rs +7 −0
 --- a/src/csv.rs
 +++ b/src/csv.rs
 @@ -41,7 +41,9 @@ fn fields(line: &str) -> Vec<&str> {
 -    line.split(',').collect()
 +    let line = line.strip_suffix(',').unwrap_or(line);
 +    line.split(',').collect()
 j k kaydır · [ ] dosya · a uygula · r reddet · c not · n notları gönder · x parça seç · > alıntıla · w sar
```

`j` `k` kaydırır, `^d` `^u` yarım sayfa, `gg` `G` uçlar, `[` `]` dosyadan dosyaya atlar, `w` uzun satırları sarar. Birden çok dosya yamanın üstünde listelenir (en çok altı); türleri birden çoksa önce türe göre satırlar.

### `a`: uygula

`a` değişikliği checkout'unuza `git apply --index` ile uygular: **stage'e** iner, bakıp commit atmanız için. Flash `claude-1 değişikliği checkout'una uygulanıyor; stage'e girince günlük söyler` der; git kendi iş parçacığında çalışır, ekran hiç beklemez. Sonra günlük `claude-1 değişikliğini (2 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 3f9a1c2` der: incelenen ağaç worktree'de bir işaret olarak commit'lenir (yazar `imece`), böylece işçinin sonraki `done`'ı yalnız sonrasını gösterir. Uymayan bir değişiklik (aynı satırları siz düzenlediniz ya da checkout'unuz ilerledi) önce index'in bir kopyasında üç yollu denenir ve çakışırsa git'in sözleriyle reddedilir; hiçbir şey yarım uygulanmaz. Hunk'larda `x`, sonra `a` yalnız seçilenleri uygular; kalanı yine değişiklik olarak bekler.

Sonra her zamanki gibi commit atın: `!git commit` kendi kabuğunuzda, ekran kenara çekilerek çalışır (herhangi bir tuş geri getirir); `:commit` stage'dekiler için ucuz bir modelden mesaj ister ve `!git commit -m '…'` satırını komut satırına koyar.

### `r`: reddet

`r` öğeyi kapatır, değişikliği worktree'de bırakır ve nedenini yazmanız için yazma satırını `@claude-1 ` olarak açar. Nedenleriniz işçinin sonraki turudur. Uygulama ile ret arasında üçüncü bir yanıt vardır: `c` diff'in en üstteki satırına not yazar, `n` bütün notları tek mesajda, her birini `dosya:satır` altında gönderir ve değişiklik işçiye aynı yoldan geri döner. `>` üstteki hunk'ı bir mesaja alıntılar.

## Çıkış

`q` çıkmadan önce sorar. Bir şey yoldaysa soru ne kadar olduğunu söyler:

```text
 imece kapatılsın mı? tur ortasındaki agent: 1, incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır
```

Bir `q` daha çıkar; `s` ÇIKMADAN ÖNCE penceresini açar: tur ortasındaki ajanlar (çıkmak turlarını yarıda keser) ve incelenmemiş değişiklikler; orada `q` hemen çıkar, `W` sıradaki bekleyeni açar, başka bir tuş geri döner. `:q` ve `:quit` aynı şekilde sorar.

Ekran kapanınca imece terminalinde tek bir satır bırakır, önce seni bekleyeni:

```text
⁙ imece · 1 seni bekliyor · 3 agent uykuda · bu oturumda $2.40 · devam: imece
```

Söyleyecek bir şeyi olmayan parça yazılmaz; demonun satırı `demo hiçbir şey saklamaz` ile biter.

## Sonraki `imece`

Aynı checkout'ta sonraki başlangıç aynı ekranı, her ajan uykuda gösterir (`◦ önceki çalışmadan beri uyuyor, yazınca uyanır`): siz birine yazana, bir takım arkadaşının mesajı ya da yeni bir kart onu uyandırana kadar süreç yok, maliyet yok. Uyandırılan bir ajan, Claude Code transkriptini hâlâ tutuyorsa eski oturumunda devam eder (`claude-1 bildiği her şeyle, eski oturumunda devam ediyor`); yoksa yeni bir oturum başlatır ve kaldığı yer anlatılır: kartının hikâyesi, yarım kalan tur, worktree'sinin değişiklikleri, pano (`claude-1 yeni bir oturumla başlıyor, kaldığı yer anlatıldı`). Eski bir commit'te temiz bulunan bir worktree HEAD'inize taşınır; değişiklikleri ya da kendi commit'leri olan yerinde kalır ve günlük hangisi olduğunu söyler.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Değişiklikler](../concepts/changes.md) ve [değişiklik ekranı](../screen/changes.md)
- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [SENİ BEKLEYEN](../screen/waiting.md)
- [Oturumlar](../concepts/sessions.md)
- [Klavye](../screen/keyboard.md)
- [Senaryo: tek düzeltme](../scenarios/one-fix.md)
