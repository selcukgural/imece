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
│   W               karar bekleyen sıradaki şey: bir değişiklik, bir soru          │
│   …                                                                              │
│ herhangi bir tuş: kapat · :welcome bunu yine gösterir                            │
└──────────────────────────────────────────────────────────────────────────────────┘
```

En üstte imece'nin işareti durur: bir işin etrafında beş el; yanında sürümü, ne olduğu ve eldeki proje ile başında kimin olduğu. Ekrana geldiği yerde noktaları birer vuruşta gelir, iş en son (iki saniyeden kısa); herhangi bir tuş onu hemen bütün gösterir, [`:motion off`](../reference/commands.md#motion-offcalmfull), başka bir pencere ve SSH de öyle. Ona dar gelen bir ekranda yazılar tek başına kalır, fazla kısa bir ekranda pencere onlarsız açılır. Ana ekran da, henüz hiçbir projede işçi yokken boş satırlarında aynı işareti gösterir. Terminalin başlığı da her şey sakinken işareti taşır: `⁙ imece`.

Satır Claude Code'un sürümünü ve giriş yapılmış olup olmadığını, ya da nasıl giriş yapılacağını söyler: `claude auth login`. claude.ai planıyla giriş yapılmışsa `giriş yapılmış, plan · ≈ API fiyatı` der: imece'nin gösterdiği dolarlar turların API fiyatıyla tutacağıdır, `≈` ile işaretli ([bir rakam ne der](../concepts/budget-and-limits.md#bir-rakam-ne-der)). imece bunu ona sorar (`claude auth status`), kendi iş parçacığında, ağa çıkmadan ve para harcamadan; cevabı gelene dek `bakılıyor…`, program yoksa `PATH'te yok` yazar.

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
│ ! 1 şey karar bekliyor:                                   │
│ Δ 11:42 claude-1 bitirdi: parser: sondaki virgülü kabul et │
│ kapanmadan önceki son satırlar:                           │
│ 11:42 claude-1 bitirdi: değişiklik hazır · 2 dosya +31 −4   │
│ 11:43 `cargo test` çalıştırdın → çıkış 0                  │
│                            herhangi bir tuş: kapat        │
└───────────────────────────────────────────────────────────┘
```

Keseyi, açık proje başına bir satırı, hâlâ karar bekleyen en eski öğeleri (en çok altı; yalnız okunacak yanıtlar ayrı sayılır: `» Home'da 1 yanıt okunacak`), imece kapanırken yarım kalan turları ve günlüğün son satırlarını listeler. Herhangi bir tuş kapatır ve başka bir şey yapmaz.

## Ana ekran {#home}

Açılan [ana ekrandır](../screen/home.md): üstte karar vermeniz gerekenler (KARAR BEKLEYEN), altında çalışanlar (ÇALIŞAN), 120 kolondan itibaren de eldeki satır listenin yanında okunur. Henüz bir şey yoksa bunu söyler ve boş satırlarda imece'nin işareti durur:

```text
 imece  your-checkout │ $0.00 / $50.00  10:02
Sizi bekleyen yok, çalışan da yok. / lidere yazar.

 ana ekran · your-checkout    enter  aç   s  gizle   @  bu agent'a yaz   ?  yardım
```

Başlıkta dört şey vardır: açık projeler, karar bekleyenler (henüz yok), kese (bütçeden harcanan) ve saat. Alttaki satır durum satırıdır: nerede olduğunuz, sonra eldeki şey için en çok üç tuş; `?` bulunduğunuz ekranın tuşlarını, bir `?` daha bütün tuşları listeler.

Takım yalnız liderdir, siz yazana dek boşta; boştaki lider listelenmez. Günlük (`J` onu ekranın üstünde açar, `esc` kapatır), `--confine strict` olmayan her başlangıçta bir kez, ilk satırında yalıtımın imece'nin işi olmadığını söyler.

## Lidere yazmak

`/` (ya da `i`) tuşuna basın. Alt satır yazma satırına dönüşür: ` › lead için: `. İstediğinizi yazın ve `enter` basın:

```text
 › lead için: CSV ayrıştırıcı son sütundaki sondaki virgülü reddediyor; kabul
              etsin, testiyle
```

Ekrandan uzun bir satır sonraki sırada sürer. Siz yazmadan önce alt satır `enter gönder · ⇧enter yeni satır · esc vazgeç` der.

Düz metin eldeki projenin liderine gider. `@claude-1 metin` tek bir ajana giderdi. İstemden sonra `/` ile başlayan bir satır bir mesaj değil, ajanın kendi komutudur (`/cost`, `/compact`, `/clear`). `⇧enter` (ya da `\` `enter`) uzun bir istekte yeni satır başlatır; alt satır terminalinizde hangisinin çalıştığını söyler. `tab`, `@` sonrasında adları, `:` sonrasında komutları tamamlar.

## Lider ne yapar

Lider checkout'u okur (okuma araçları, web ve salt okunur komutları vardır; yazamaz: sizin checkout'unuzda çalışır, orada bir yazma incelemeyi atlardı). İsteği kartlara böler, panoya asar ve bir kart el gerektirince `hire` aracıyla bir işçi **tutar**. Model değil **katman** seçer: küçük mekanik iş için `cheap`, sıradan düzeltmeler için `standard` (varsayılan), kesen tasarım için `strong`; bunlar haiku, Claude Code'un varsayılanı ve opus demektir. En çok `--agents` işçi (varsayılan 4).

```text
 ÇALIŞAN 1
 ● #1   sondaki virgülü kabul et  Çevresindeki kodu oku
```

Günlük `lead panoya astı: #1 sondaki virgülü kabul et`, `lead işe aldı: claude-1 (varsayılan model)`, `⇄ lead → claude-1 [görev]: #1 …` der. Liderin kapanış metni size KARAR BEKLEYEN'de, yanıtlar altında, karar verilecek her şeyden sonra bir yanıt olarak gelir (`answered: …`): `enter` tamamını okur, `r` kapatır, ya da doğrudan yanıt yazarsınız. Başlık çubuğu onu `! N karar bekliyor`'dan ayrı, `1 okunacak` olarak sayar.

Hesabınızda olmayan bir katman modeli ilk turunda düşer; imece o işçiyi standart modelde yeniden başlatır ve günlük `claude-1: opus modeli burada yok, varsayılanla devam` der.

## Bir işçinin satırı

```text
 ● #1   sondaki virgülü kabul et  Değişikliği yap · 4 dk
```

Satır ajan değil, iştir. Soldan sağa: glif (`●` çalışıyor, `○` boşta, `!` sizi bekliyor, `✓` bitti, `■` takıldı: bloke ya da oturumu bitti), kartı ve başlığı, yapılacaklar listesinde bulunduğu adım (ya da şu an ne yaptığı) ve ne zamandır. Evetinizi isteyen bir işçi beklerken ÇALIŞAN'dan çıkar: isteği KARAR BEKLEYEN'de, izinler altında kendi satırıdır (`Bash: … çalışsın mı?`); orada `a` izin verir, `r` reddeder (rutin komutlar, derleme, testler, git'in okuma komutları kendiliğinden çalışır).

Satırda `enter` işçinin işini log sekmesinde açar: kartının hikâyesi (niyet, varsayımlar, kararlar, konuşma, kanıt, değişiklikler) ve yapılacaklar listesi; `t` izi açar, oturumunun ham akışı. Baş satırı kartını, projesini, ajanı, süresini ve maliyetini söyler; `tab` sonraki sekmeye geçer, `esc` geri döner.

## Değişiklik (Δ)

İşçi `done` gönderince imece onun worktree'sini tek yama olarak okur ve KARAR BEKLEYEN'e koyar. Günlük `claude-1 bitirdi: değişiklik hazır · 2 dosya +31 −4` der, başlık `! 1 karar bekliyor`, terminalin başlığı `! imece · 1 karar bekliyor` olur. Ana ekranda değişiklik, değişiklikler altında bir satırdır:

```text
 KARAR BEKLEYEN 1 değişiklik
 değişiklikler                                                                               1 onaya hazır
▸! #1   sondaki virgülü kabul et  ✓ cargo test · 2 dosya +31 −4, ikisi de #1 içinde   a  onayla   r  geri gönder
```

Satır değişikliğin hükmüdür: imece'nin `run` aracının tam bu ağaçta gördüğü çalıştırmalar (`✓ cargo test`, `✗ cargo test: exit 101` ya da `hiçbir şey koşmadı`), kartının sahip olduğu yollara göre dosyaları ve çakıştığı bekleyen başka bir değişiklik. Temiz kanıtlanmış, kartının yollarının içinde kalan ve hiçbir şeyle çakışmayan bir değişiklik onaya hazırdır; hazırlar önce gelir. Gösterilir, kapı değildir: karar sizin.

- `W` her yerden (bir kartın ekranı, bir diff, bir pencere) karar bekleyen sıradakini açar; yine basılınca sonrakini. `S` eldeki öğeyi sonraya erteler ve sonrakini açar.
- Değişiklikte `enter` onu diff sekmesinde açar:

```text
#1 sondaki virgülü kabul et · your-checkout · claude-1 · 6 dk · $0.41 · 2 dosya +31 −4    değişiklik 1 / 1

diff · konuşma · log                                                                             tab geçer
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│✓ onaya hazır · ✓ cargo test · 2 dosya, ikisi de #1 içinde                                              │
│ maliyet · $0.41 · 6 dk                                                                                 │
│ kaynak +24 −4 (1) · testler +7 −0 (1)                                                                  │
│ src/csv.rs +24 −4                                                                                      │
│ tests/csv.rs +7 −0                                                                                     │
│src/csv.rs                                                                                              │
│@@ -41,7 +41,9 @@ fn fields(line: &str) -> Vec<&str> {                                                  │
│-    line.split(',').collect()                                                                          │
│+    let line = line.strip_suffix(',').unwrap_or(line);                                                 │
│+    line.split(',').collect()                                                                          │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 değişiklik · your-checkout · #1 sondaki v…    a  onayla   r  geri gönder   c  not   esc  geri   ?  yardım
```

Baş satırı kartı, projeyi, işçiyi, süresini ve maliyetini, dosyaları ve satırları, bir de değişikliğin bekleyenler arasındaki yerini söyler. Diff hüküm satırıyla başlar, `✓ onaya hazır` ya da `! önce bak` ve nedeni; sonra değişikliğin maliyeti, birden çok tür varsa türe göre satırları ve dosyaları (en çok altı). `j` `k` kaydırır, `^d` `^u` yarım sayfa, `gg` `G` uçlar, `[` `]` dosyadan dosyaya atlar, `w` uzun satırları sarar, `tab` konuşmaya ve log'a geçer.

### `a`: onayla

`a` değişikliği checkout'unuza `git apply --index` ile uygular: **stage'e** iner, bakıp commit atmanız için. Flash `onaylandı: claude-1 değişikliği checkout'una uygulanıyor; stage'e girince günlük söyler` der; git kendi iş parçacığında çalışır, ekran hiç beklemez. Değişiklikleri birer birer uygular: biri hâlâ girerken onaylanan başkası `… şu an uygulanandan sonra checkout'una girer …` der, bu arada onaylanacak bir şey yokken `a` bir değişikliğin hâlâ uygulandığını söyler. Sonra günlük `claude-1 değişikliğini (2 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 3f9a1c2` der: incelenen ağaç worktree'de bir işaret olarak commit'lenir (yazar `imece`), böylece işçinin sonraki `done`'ı yalnız sonrasını gösterir. Uymayan bir değişiklik (aynı satırları siz düzenlediniz ya da checkout'unuz ilerledi) önce index'in bir kopyasında üç yollu denenir ve çakışırsa git'in sözleriyle reddedilir; hiçbir şey yarım uygulanmaz. Hunk'larda `x`, sonra `a` yalnız seçilenleri uygular; kalanı yine değişiklik olarak bekler.

Sonra her zamanki gibi commit atın: `!git commit` kendi kabuğunuzda, ekran kenara çekilerek çalışır (herhangi bir tuş geri getirir); `:commit` stage'dekiler için ucuz bir modelden mesaj ister ve `!git commit -m '…'` satırını komut satırına koyar.

### `r`: geri gönder

`r` öğeyi kapatır, değişikliği worktree'de bırakır ve nedenini yazmanız için yazma satırını `@claude-1 ` olarak açar. Nedenleriniz işçinin sonraki turudur. Uygulama ile ret arasında üçüncü bir yanıt vardır: `c` diff'in en üstteki satırına not yazar, `n` bütün notları tek mesajda, her birini `dosya:satır` altında gönderir ve değişiklik işçiye aynı yoldan geri döner. `>` üstteki hunk'ı bir mesaja alıntılar.

## Çıkış

`q` çıkmadan önce sorar. Bir şey yoldaysa soru ne kadar olduğunu söyler:

```text
 imece kapatılsın mı? tur ortasındaki agent: 1, incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır
```

Bir `q` daha çıkar; `s` ÇIKMADAN ÖNCE penceresini açar: tur ortasındaki ajanlar (çıkmak turlarını yarıda keser) ve incelenmemiş değişiklikler; orada `q` hemen çıkar, `W` sıradaki bekleyeni açar, başka bir tuş geri döner. `:q` ve `:quit` aynı şekilde sorar.

Ekran kapanınca imece terminalinde tek bir satır bırakır, önce seni bekleyeni:

```text
⁙ imece · 1 karar bekliyor · 3 agent uykuda · bu oturumda $2.40 · devam: imece
```

Söyleyecek bir şeyi olmayan parça yazılmaz; demonun satırı `demo hiçbir şey saklamaz` ile biter.

## Sonraki `imece`

Aynı checkout'ta sonraki başlangıç aynı ekranı, her ajan uykuda gösterir (`◦ önceki çalışmadan beri uyuyor, yazınca uyanır`): siz birine yazana, bir takım arkadaşının mesajı ya da yeni bir kart onu uyandırana kadar süreç yok, maliyet yok. Uyandırılan bir ajan, Claude Code transkriptini hâlâ tutuyorsa eski oturumunda devam eder (`claude-1 bildiği her şeyle, eski oturumunda devam ediyor`); yoksa yeni bir oturum başlatır ve kaldığı yer anlatılır: kartının hikâyesi, yarım kalan tur, worktree'sinin değişiklikleri, pano (`claude-1 yeni bir oturumla başlıyor, kaldığı yer anlatıldı`). Eski bir commit'te temiz bulunan bir worktree HEAD'inize taşınır; değişiklikleri ya da kendi commit'leri olan yerinde kalır ve günlük hangisi olduğunu söyler.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Değişiklikler](../concepts/changes.md) ve [değişiklik ekranı](../screen/changes.md)
- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [Ana ekran](../screen/home.md)
- [Oturumlar](../concepts/sessions.md)
- [Klavye](../screen/keyboard.md)
- [Senaryo: tek düzeltme](../scenarios/one-fix.md)
