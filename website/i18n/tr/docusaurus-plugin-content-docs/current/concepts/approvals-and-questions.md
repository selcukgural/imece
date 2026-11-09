---
title: Onaylar ve sorular
description: Bir ajan yapmadan önce size neyi sormalı, rutin bir komut nasıl kendi başına çalışır, bir ⚑ istek ve bir ? soru KARAR BEKLEYEN'de nasıl bekler, run aracı ve izin listesi.
---

# Onaylar ve sorular

İki tür şey bir ajanı siz yanıtlayana dek durdurur: bir **onay** (`⚑`), Claude Code bir araç çağrısından önce soracakken imece'nin soruyu size vermesi; ve bir **soru** (`?`), ajanın `send(to: "you", type: ask)` ile size kendisinin bir şey sorması. İkisi de KARAR BEKLEYEN'de bekler; ilkini `a` ve `r` yanıtlar, ikincisini ajana yazmak.

## Onaylar (`⚑`)

`--confine standard` ve `open` altında bir oturumun yapmadan önce soracağı şey, reddedilmek yerine size gelir. `strict` altında hiçbir şey sormaz: soracak olan reddedilir ve komutlar imece'nin `run` aracından geçer.

Claude Code `--permission-prompts host --permission-prompt-tool mcp__imece__approve` ile çalışır: onay gereken bir araç çağrısından önce imece'nin `approve` aracını kendisi çağırır (kendi salt okunur kuralları neyin hiç sormayacağına karar verir). Araç modelden gizlidir.

### Rutin komutlar hemen çalışır

Bir kabuk komutu **rutin**dir: tek satırsa, argümanlarıyla tek bir programsa (kabuğun başka türlü okuyacağı hiçbir şey yok: boru, yönlendirme, `&&`, `$`, glob yok) ve `run` aracının politikası role onu çalıştırmaya izin veriyorsa (aşağıda). Rutin bir komuta kendi başına izin verilir, izde "allowed on its own" satırıyla; derleme, testler ve git'in okuma komutları sizi asla beklemez.

### Kalan her şey bekler

Başka her şey KARAR BEKLEYEN'de, ana ekranda **izinler** altında bekler: istek başına bir satır, ajanının üzerinde çalıştığı kartla başlar, isteğin tamamını tek yerde söyler, `Bash: curl … | head çalışsın mı?`, kendi sütununun altına sarılır; satırları ` ↵ ` ile birleştirilir ve asla markdown olarak okunmaz: izin verdiğiniz, çalışandır. Eldeki satır tuşlarıyla biter, durum satırı da onları tam söyler:

```text
 izinler                                                                                           her biri tek tuş
▸! #6   Toplu e-posta kuyruğu     WebFetch:                                                    a  evet   r  hayır
                                  https://learn.microsoft.com/dotnet/api/system.text.json çalışsın mı?
 …
 ana ekran · shop-api · #6 Toplu…    a  bir kez izin ver   r  reddet   e  bu tur evet   ?  yardım
```

İsteği beklerken ajan o satırın kendisidir: ÇALIŞAN altında bir daha listelenmez. Bir evet ya da hayır satırı karara bağlar ve sonrakini ele getirir; arka arkaya izinler, arka arkaya tuşlardır.

- `a` izin verir; çağrı istendiği gibi çalışır ve günlük `claude-1 için izin verdin: Bash: …` der.
- `r` reddeder; ajana "The person refused this. Do not try it another way; if you need it, ask them (send to you, type ask) and go on with what does not depend on it." denir ve günlük `… için reddettin` der. Ret bir karardır, hata değil; tur devam eder.
- Bir çağrı en çok bir gün bekler. Bağlantı başına bir anda bir çağrı; bekleyen bir istek o oturumun diğer imece araçlarını tutar (istek ortasında zaten kullanamazdı).
- Oturum bitince bekleyen her istek "the session ended" ile yanıtlanır ve öğeleri kapanır. Deponun önceki çalıştırmadan tuttuğu bir istek başlangıçta kapatılır: soran süreç gitmiştir.

Bir istek ne gösterir: kabuk komutu için komutu; başka bir araç için dokunduğunu (yol, adres, sorgu ya da desen) ya da girdisini 200 karakterde kesilmiş JSON olarak. Bir evet bir kez geçerlidir; artık hep evet için aşağıya bakın.

### Artık hep evet

Bir istekte `A`, o projenin bütün ajanları için isteğin komutlarına artık hep evet der: `npm run build` `npm run …` verir (program ve ilk argümanı), `make` `make` verir. İstek sürer, projenin artık kapsanan diğer açık istekleri de; yalnız böyle komutlar (ve rutin olanlar) çalıştıran sonraki bir istek size sormadan sürer, izi `allowed by your yes from now on: Bash: …` der. Birkaç komutlu bir satır (`&&`, `||`, `;`, `|`) her biri rutinse ya da hep eveti varsa sürer. Bir hep evetin iki kelimesinden sonra satırda `$NAME`, glob ya da `$(…)` olabilir; `$(…)` içinde çalışan da kendi hep evetini ister: `gh pr view "$(git branch --show-current)"` üzerinde `A`, `gh pr …` ve `git branch …` verir. `2>&1` ve `>/dev/null` bir şey değiştirmez. Bunlar rutin bir komut ya da git için asla sayılmaz, çünkü kabuk yasaklı kelimeleri gizleyebilir (`cargo $X`, `git grep $X`). Böyle bir hep evetin tutamayacağı satır (`rm -rf target` gibi önce seçenek, dosyaya yönlendirme, ters tırnak) yalnız o satıra hep evet alır, `` `rm -rf target` `` diye gösterilir: yalnız o baytlar sürer, `rm -rf /` yine sorar. Bir dosya düzenlemenin ya da bir getirmenin hep eveti olamaz: `A` nedenini söyler, `a` bir kez izin verir. Liste çalıştırmalar arasında projeyle saklanır; `:allow` gösterir, `:allow npm run` adıyla ekler (başından kalkmadan önce), yeniden `:allow npm run` çıkarır; `:allow rm -rf target` o satırın hep evetini çıkarır. Günlük `shop-api: artık hep evet diyorsun: npm run …` der.

### Bu tur evet

Bir istekte `e` (sizi bekleyenlerde ya da ajanının log sekmesinde) ya da `:yes [@NAME]`, o ajan turunu bitirene dek ne isterse evet der: istek sürer, diğer açık istekleri de, o ajanın turu (ya da oturumu) bitene dek sonraki her isteği de size sormadan sürer, ne olursa olsun: `$(…)` içeren ya da bir seçenekle başlayan bir komut, bir düzenleme, bir getirme. Başlattığınız bir işte çalışan tek bir ajan için, istekler çok olduğunda ve siz başındayken; artık hep evet (`A`) bir projenin hep çalıştırdığı komutlar içindir. İzi `allowed by your yes for this turn: …` der, sürdükçe satırı ve odağı `✓ tur` der, günlük de ne zaman verdiğinizi ve tur bitince kaç isteğe sormadan izin verdiğini söyler. Başka bir ajanın istekleri sizi beklemeye devam eder. Yeni bir tur yine sorar.

## Sorular (`?`)

Bir ajan size `send(to: "you", type: "ask", body)` ile sorar ve seçilecek yanıtları sunabilir (`options`: en çok dört, her biri kısa bir satır, kendi seçeceği önce). **Sorular** altında bir `?` öğesi olur, günlükte "claude-2 sana soruyor: eski CLI bayrağı kalsın mı?", ve soru açık kaldıkça ajan o satırın kendisidir. Aynı ajandan daha yeni bir soru eskisinin yerini alır.

[Ana ekranda](../screen/home.md) satır soruyu, altında da sunulan her yanıtı onu seçen tuşla söyler:

```text
▸! #12 İadeler     İade karta mı dönsün, mağaza kredisine mi?   1-3 seç   a yanıtla
                    1  karta
                    2  mağaza kredisine
                    3  hangisiyle ödendiyse ona
```

- O satırda `1`-`9`, adlandırdığı yanıtı ajana sizin mesajınız olarak gönderir: soru kapanır ve ajanın sonraki turu onunla başlar. Rakamlar Ana ekranda ve sorunun detayında bir yanıt seçer; başka yerde bir şey yapmaz (bir zamanlar açtıkları görünümler artık `:board`, `:code` ve `:timeline` pencereleridir).
- Satırda `/`, ya da `a` ya da `r`, kendi sözcüklerinizle yanıt için yazma satırını `@claude-2 ` ile açar; o ajana yazdığınız her şey soruyu kapatır ve sonraki turunu başlatır.
- `enter` konuşmayı soruda açar. Listenin yanındaki önizleme (120 kolondan itibaren) soruyu bütün olarak, yanıtlarını ve ajanın şimdiye dek yaptığını okur: durumu, yapılacaklar listesi ve en yeni iz satırları.
- Gitmiş ya da durmuş bir ajanın sorusu siz ona yazana dek açık kalır.

Kim ne zaman sorar: lider görev ortasında size sorabilir; bir işçi önce liderine sorar ve size yalnız sizin karar verebileceğinizi gönderir; bir yardımcı yalnız hiçbir ekip arkadaşının yanıtlayamayacağını. Sorular posta sınırlarına ve bütçeye sayılmaz ([mesajlar](../concepts/messages-and-board.md)).

## Bekleyene ulaşmak

- `W` her yerden sizi bekleyen sonraki öğeyi açar (bir değişikliğin diff'i, bir sorunun konuşması, bir onayın ajanı log sekmesinde), böylece `a`/`r` hemen yanıtlar; yine basılınca sonrakine geçer, sondan ilkine döner. Bekleyen yoksa: "Şu an senden bir şey gerekmiyor".
- `S` eldeki öğeyi erteler ve sonrakini açar: ertelenenler diğerlerinin hepsinden sonra, ertelenme sırasıyla gelir, satırları `sonra ·` ile işaretli. Erteleme yalnız bu çalıştırma içindir.
- Ana ekran sizi bekleyeni üste, KARAR BEKLEYEN'e koyar ve ÇALIŞAN'da her projenin işçilerini sizden ne beklediklerine göre sıralar: önce takılan (bloke ya da durmuş), sonra çalışan, sonra sakinler; eşitler sırasını korur. ÇALIŞAN başlığı (`takılan 2 · boşta 1`), başlık satırı (`! 2 karar bekliyor`) ve terminalin başlığı aynı şeyleri sayar.

## `run` aracı

Her ajanın bus'ta bir `run` aracı vardır: argümanlarıyla tek bir program, kabuk yok, ajanın kendi dizininde (lider için checkout, işçiler ve yardımcılar için worktree).

| | İzinli |
| --- | --- |
| `cargo` | bağımlılıkları değiştiren ya da projenin dışına uzanan komutlar dışında hepsi: `add`, `remove`, `rm`, `install`, `uninstall`, `publish`, `login`, `logout`, `owner`, `yank` yok. Lider için dosya değiştirenler de yok (`--check`'siz `fmt`, `fix`, `clean`, `init`, `new`). |
| `git` | yalnız okuma komutları: `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status`. Komuttan önce seçenek yok (`-c`, `-C`, `--git-dir`), pager başlatan ya da dosya yazan yok (`-O…`, `--open-files-in-pager…`, `--output…`); kısa seçenekler birleşik yazılsa da (`-iO…`), uzun ad kısaltılsa da (`--open=…`). |
| `--run PROG[,..]` | adını verdiğiniz programlar (`--run npm,make`), yalnız işçiler ve yardımcılar için; yinelenebilir. |
| başka her şey | reddedilir; izinli olanların listesi ve nasıl ekleneceğiyle. |

- Program tek başına, PATH'te adıyla anılır, asla yol olarak değil. Komut satırı en çok 1000 karakter; `| & ; < > ( ) $ \` # * ? ~ {` reddedilir.
- Bir çalıştırma `--run-timeout` saniye sonra (10-3600, varsayılan 600) ve imece ile birlikte öldürülür. Yanıt ekranın döngüsünü değil programı bekler.
- Çıktı derlenip toplanmış döner: renkler soyulur, geçen testlerin satırları atılır, tekrarlar katlanır, 48 KiB'ı aşınca ortası kesilir. Her worktree kendi `target/`'ında derler: ortak olanı işçileri sırayla birbirlerinin değişikliklerini yeniden derletirdi.
- `run`'ın kendi ağ engeli yoktur; lockfile uygun değilse cargo registry'ye uzanabilir.

Hangi kabuk komutlarının rutin olduğuna aynı politika karar verir; `standard` altında bir işçinin `cargo test`'i `run`'dan da Claude Code'un kabuğundan da geçse çalışır; yalnız bir `run` sonucunu çalıştığı ağaca bağlar ve değişiklikte gösterir ([değişiklikler](../concepts/changes.md)).

## Ayrıca bakınız

- [Claude Code](../concepts/claude-code.md): confine düzeyleri.
- [Ana ekran](../screen/home.md): KARAR BEKLEYEN, grupları, `W`, `S`.
- [Mesajlar ve pano](../concepts/messages-and-board.md).
- [Güvenlik](../concepts/security.md): son kapı sizsiniz, bir araç listesi değil.
