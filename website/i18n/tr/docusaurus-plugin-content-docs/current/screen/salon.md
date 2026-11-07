---
title: Ekip
description: imece'nin varsayılan ekranı - başlık satırı, projeye göre gruplanmış lonca ağacı, her agent'ın satırı ve işareti, katlama ve yandaki paneller.
---

# Ekip

Ekip, [ana ekranın](home.md) alt yarısıdır, ÇALIŞAN (kayıt 125): `:salon` oraya gider, `gw` gibi. Bu sayfanın lonca ağacı için anlattıkları (her proje, lideri, işçileri ve yardımcıları, katlama, `s`) orada proje proje durur; yanında bekleyenler ana ekranın KARAR BEKLEYEN'i, [günlük](journal.md) (hikâye olarak anlatılan) ise bir penceredir, `J` ya da `:journal`. Pano, kod haritası ve zaman şeridi de pencere olarak açılır (`:board`, `:code`, `:timeline`); geri kalan her şey ana ekranın üstünde açılır ve `esc` onu geri getirir.

```text
 imece  shop-api │  ! 2 karar bekliyor  │ $3.10 / $20.00  14:02
┌ EKİP ───────────────────────────────────────────┐┌ KARAR BEKLEYEN (2) ────────────────────┐
│▾ ▴ shop-api  ▓▓▓░░░░░ 3/8  ! 2 bekliyor  2 çalışıyor││Δ 13:58 claude-1 bitirdi: parser (3 dosya, +41 −6) · ✓ bu ağaçta temiz geçti: cargo test
│  ● ▎lead      koordine ediyor                   ││⚑ 14:01 claude-5 çalıştırmak istiyor: Bash: npm install left-pad
│  ├ ● ▎claude-1 #3   Parse the manifest  testleri yazıyor · 2 dk  $1.10  ⎇ fix-parser ↑2 +12
│  │ └ ● ▎claude-1.1                      düşünüyor… ← claude-1 "testleri ekle"   ⎇         │
│  ├ ✓ ▎claude-5 #5   Money rounding      bitti, henüz bakılmadı → sen "yukarı yuvarlandı" · 5 dk│
│  └ … 2 agent yolunda                             │└────────────────────────────────────────┘
│▸ web  1 çalışıyor  $0.70                         │┌ DEFTER ────────────────────────────────┐
│                                                  ││13:58 ! claude-1 bitirdi: değişiklik hazır · 3 dosya +41 −6
│                                                  ││13:55 ⇄ lead → claude-1 [görev] #3 Parse the manifest
│                                                  ││13:40 › sen: boş dosyadaki parser çökmesini düzelt
│KUYRUK  boş  │ son: #2 ✓ #1 ✓                     ││                                        │
└──────────────────────────────────────────────────┘└────────────────────────────────────────┘
  W sıradaki karar  / lidere yaz  @ agent'a  # iş  tab panel  enter odak  ? yardım   1 ekip  2 pano  3 kod
```

## Başlık satırı

Tek satır, dört şey, parçalar `│` ile ayrılmış: projeler, sizi bekleyenler, kese, saat; bir kullanım penceresi yalnız dolmak üzereyken aralarına girer. Ekran satırdan darsa önce en az istenen parça düşer; böylece ekranın kenarı sizi bekleyeni asla kesmez: saat, sonra projeler. Sizi bekleyenler, kese, duraklatma ve dolmak üzere olan bir pencere her zaman kalır.

| Parça | Ne söyler |
| --- | --- |
| `shop-api · mobile-app` | açık projeler. [Ana ekranda](home.md) listenin daraltıldığı proje aydınlıktır. |
| `! 2 karar bekliyor` | KARAR BEKLEYEN'deki açık ögeler, dikkat renginde. Terminalin başlığı aynı sayıyı taşır, `! imece · 2 karar bekliyor`, böylece arkadaki bir sekme de gösterir; başında açık projelerin durum merdiveninin tepesi vardır: `■` takılan var, `!` sizi bekleyen var, `●` çalışan ya da başlayan var, her şey sakinse imece'nin işareti `⁙` (bitmiş ama görülmemiş bir tur sakindir). Windows Terminal, kitty 0.39+, Ghostty 1.2+ ve iTerm2 3.6.6+ bunu sekmenin ilerleme çubuğu olarak da gösterir (OSC 9;4): takılınca dolu kırmızı, beklerken dolu sarı, çalışırken hareketli, sakinken yok. imece başlarken terminale adını sorar (XTVERSION) ve `TERM_PROGRAM`'a değil onun cevabına bakar, çünkü başka bir terminalden açılan terminal onu miras alır; bazı terminaller bu diziyi bildirim sandığı için başkalarına gönderilmez. tmux içinde gönderilir: tmux 3.8 onu Ghostty ve iTerm2'ye iletir, daha eski bir tmux düşürür. `IMECE_PROGRESS=on` ya da `off` ile kendiniz karar verirsiniz. imece bitince ya da terminali bir `!` komutuna bırakınca kabuğunuzun başlığı geri gelir, çubuk kalkar. |
| `$3.10 / $20.00` | kese (bütçe): bütün agent'ların birlikte harcadığı. O anki hızla yarım saatten az kaldığında parça dikkat rengine döner ve ne kadar kaldığını söyler, `$19.60 / $20.00, ~25 dk kaldı`; günlük bunu bir kez söyler. Hız ve kalan süre başka her zaman `:budget` menüsünün notundadır. |
| `claude 5 sa: %10 kaldı, 1 sa 59 dk sonra yenilenir` | planınızın dolmaya en yakın kullanım penceresi, yalnız %90 kullanımdan itibaren, dikkat renginde: bir aboneliğin sınırı takımı keseden önce durdurabilir. Onun altında `:budget` notundadır. Çalıştırmalar arasında saklanmaz: oturum ilk turunda yeniden söyler. |
| `14:02` | saat; demonun `‖ DURAKLATILDI`sı ya da `×4`ü ona yapışık. |

Son on dakikada olanlar, kart sayısı ve araştırmacı üst sınırı kayıt 125 ile başlıktan çıktı: kartlar panoda, olanlar günlükte.

Kesenin ve pencerelerin nasıl sayıldığı [bütçe ve sınırlar](../concepts/budget-and-limits.md) sayfasında.

## Lonca ağacı

Agent'lar projeye göre gruplanır. Her proje bir başlık satırıdır; altında lideri, sonra işçileri, her işçinin altında yardımcıları. Projeler açıldıkları sırayla gelir, sabitlenenler önce.

Henüz hiçbir projede işçi yokken (yalnızca lideri olan bir ekip, bir solo) imece'nin işareti ve yazıları boş satırların ortasında durur: `imece v…`, `tek terminal, birçok agent, bir lider`, eldeki proje ve başında kim olduğu. Göründüğünde noktaları birer vuruşta gelir (bkz. [ilk çalıştırma](../getting-started/first-run.md#hoş-geldin-penceresi)); ilk işçinin satırı yerini alır, `f` onu göstermez.

Bir proje başlığı: `▾ shop-api  ▓▓▓░░░░░ 3/8  ! 2 bekliyor  ■2  2 çalışıyor  $2.40`. `▾` açık, `▸` katlı. Adın ardından: kartlarının ne kadar ilerlediği (sekiz hücre, biten payı aşağı yuvarlanarak dolu, ardından biten / hepsi; lideri kart asmadan önce yok), dikkat renginde bu projenin sizi bekleyen öge sayısı, varsa `■N` takılan (bloke ya da durmuş) agent, boşta olmayan işçi ve yardımcı sayısı, projenin agent'larının harcadığı, sonra Lonca deposunun durumu. Sizi isteyen önce gelir; bölmeye sığmayan satır gerisini kaybeder. Katlı bir proje tek satırdır, o satır da bekleyenleri gösterir (sessiz ama sağır değil) ve en yüksek üyesiyle biter, `▸ shop-api  ! 1 bekliyor  2 çalışıyor  $2.40  ! claude-5`: aşağıdaki durum merdiveninde en üstteki agent, eşitler arasında sıradaki ilk. Sabitlenmiş bir proje (`:pin`) adının önünde `▴` taşır ve ekibin en üstünde kalır; sabitleme çalıştırmalar arasında saklanır.

Bir agent'ın satırı:

```text
  ├ ● ▎claude-1 #3   Parse the manifest  ▸ Write the tests  Edit tests/parse.rs · 2 dk  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  ▾ 1 yardımcı  ↗ localhost:5173/
```

- Ağaç çizgileri `├ └ │` kimin kimin altında olduğunu gösterir; bir yardımcı işçisinin altındadır ve `claude-1.1` diye adlanır.
- Durum işareti: agent'ın durum merdivenindeki basamağı (aşağıdaki lejant). Beş işaret, yalnız bunlar: `■` takıldı (bloke ya da oturumu durdu: hangisi olduğunu durum sözleri söyler); bir şeyi sizi beklerken, o sırada ne yapıyorsa yapsın `!`; bir turu bitirmiş ama henüz açmadığınız bir işçi için `✓`, durum olarak `bitti, henüz bakılmadı`; `●` iş başında (tur ortasında, başlarken, bir takım arkadaşına sorarken, araştırırken); `○` boşta (ya da uyuyor). Her durum kendi biçimidir, mono tema da ayırt eder. Liderin satırı da herkesinki gibi durumunun işaretini taşır: rolü adındadır, biçiminde değil. Hareket açıkken ([`:motion`](../reference/commands.md#motion-offcalmfull), varsayılan calm) çalışan bir satırın işareti renginde nefes alır (şeklinde asla), sizi beklemeye düşen bir satır üç kez yanıp söner, turu biten ya da oturumu kalkan bir satır yeşil bir zeminden yatışır ve `başlıyor` noktalarını sayar; başka penceredeyken hiçbir şey hareket etmez.
- Agent'ın konuşmacı renginde `▎`: günlükte ve bir konuşmada satırlarının taşıdığı renk, böylece satır ve konuşması tek agent olarak okunur.
- Adı; sonra kirliyse `~` ya da `≈` (araştırma kirliliği, yalnız demoda).
- Bir işçinin kartı: `#3` ve kartın başlığı.
- Durumu: oturumu kalkarken (süreç, ilk worktree'nin checkout'u) `başlıyor…`, `düşünüyor…`, yaptığını söylediği şey, bir takım arkadaşının yanıtını beklerken `? → claude-2`, bir sorusu sizi beklerken `? → sen`, bir araç çağrısı KARAR BEKLEYEN'de beklerken `⚑ çalıştırmak istiyor`, yanıtı okunmayı beklerken `sana yanıt verdi`, geçici bir API hatasıyla biten bir turu imece yeniden denemeyi beklerken `↻ 2/3 · 25 sn sonra` (deneme, deneme sayısı, kalan süre), bir API hatası sizi beklerken `■ API hatası`, işçileri çalışan bir lider için `koordine ediyor`, `BLOKE`, `■ oturum bitti · yazınca yeniden başlar`, `◦ önceki çalışmadan beri uyuyor, yazınca uyanır`. Yapılacaklar listesinde devam eden bir adımı olan çalışan bir agent önce o adımı, ardından onun için ne yaptığını soluk renkte söyler: `▸ Write the tests  Edit tests/parse.rs`, `▸ Run the tests  düşünüyor…`; böylece satır yalnız son araç çağrısını değil, agent'ın neyin peşinde olduğunu söyler. Adım en çok 24 sütun alır ve satırın sabit parçalarının bıraktığından fazlasını asla almaz; araç çağrısı adımdan arta kalanı alır (onayınızı bekleyen bir araç çağrısı ya da hâlâ başlayan bir oturum bunun yerine onu söyler). Durumun kendi metni varsa satır onu taşır: `? → sen "hangi index?"`, `sana yanıt verdi "enum kullan"`, `⚑ çalıştırmak istiyor "cargo test"`, `■ API hatası "Invalid API key"`, `BLOKE "JWT saat kayması" ← researcher-1`, `■ oturum bitti: süreç 1 koduyla çıktı`. Yoksa durum, agent'ın en son alışverişiyle biter; ok kimin söylediğini gösterir: `düşünüyor… ← lead "parser'ı düzelt"`, `boşta → sen "parser düzeldi, testler yeşil"`, `? → claude-2 "hangi parser?"` (beklediği kişiye kendi sorusu, tek başına alıntılanır). Alışveriş bir sonrakine dek kalır; böylece satır agent'ın en son ne yaptığını söyler, durum ve metni kırk sütunu geçmez. Agent sizi beklerken onunla bekleyen mesajlar satırın başında yazar: `✉ 2 kuyrukta ? → sen "hangi index?"`.
- `▁▃▅▂▁`: agent'ın son zamanda yaptıkları, son beş tikin her biri için bir sütun (her biri 5 saniye, toplam 25), en eskisi önce: o tikte izine kaç satır düştüğü (bir metin, bir araç çağrısı, bir mesaj, bir not), her satırda aynı ölçekle (`▁` hiç, `▂` bir, `▃` iki, `▄` dörde kadar, `▅` altı, `▆` dokuz, `▇` on dört, `█` daha fazla); böylece yoğun bir satırla sakin bir satır bir bakışta ayrılır. Yalnızca agent bir şey yaptıkça ve tikler geçtikçe değişir, kendi zamanlayıcısıyla asla; beş tik boyunca hiçbir şey yazılmazsa kaybolur. Dar bir satırda alıntıdan da önce, ilk yer veren odur ve [`:motion off`](../reference/commands.md#motion-offcalmfull) onu göstermez. Odakta (`enter`) daha uzunu vardır, iki dakika, aynı ölçekte 24 sütun, saydıkları satır sayısıyla: `etkinlik: ▁▁▂▄▁…█▃ · 2 dakikada iz satırı: 21`. Çalıştırmalar arasında saklanmaz.
- O durumda ne zamandır olduğu, bir dakikadan başlayarak: `· 3 dk`, `· 1 sa 5 dk`. Sizi bekleyen bir şey için öğe geldiğinden beri.
- Varsa harcadığı.
- `≡ 3/7`: kendi yapılacaklar listesinin neresinde olduğu, biten adımlar ve hepsi; planlama aracı bir liste tutuyorsa (Claude Code'un görevleri). Devam eden adım satırın durumudur (`▸`); odak hepsini listeler. Satır darsa işaret alıntıdan ve süreden sonra yer verir. Oturumla birlikte gider.
- `◔ %72`: bağlam penceresinin ne kadar dolu olduğu, %70'ten itibaren, %90'dan itibaren dikkat renginde, Claude Code'un son söylediğince (mesaj kullanımı ve sonucun adlandırdığı pencere). Odak rakamı her hâlde gösterir, `bağlam: %22 · 45k / 200k`, ya da pencerenin boyutu söylenmiyorsa yalnız token sayısını. Dar bir satırda yapılacaklar işaretinden daha uzun kalır. Oturumla birlikte gider.
- Bir işçinin worktree durumu, `⎇ fix-parser ↑2 ↓1 +12 −3`: agent'ın açtığı dal (worktree imece'nin bıraktığı gibi detached iken yok; bir başlangıç noktasından işe alınan işçi onun yerine o noktayı gösterir, `⎇ #41`), checkout'ta olmayan kendi commit'leri (imece'nin uygulanan değişiklik işaretleri sayılmaz), kendisinde olmayan checkout commit'leri ve kendi HEAD'ine göre eklenen ve silinen satırlar, izlenmeyen dosyalar dahil: bir sonraki `done`'un size vereceği şey. Her parça yalnız söyleyecek bir şey varken görünür. imece'nin git iş parçacığında okunur: oturum başlarken, bir tur bitince, bir uygulamadan sonra ve işçi ya da bir yardımcısı tur ortasındayken her 15 saniyede; çalıştırmalar arasında saklanmaz.
- `▾ 1 yardımcı` ya da `▸ 1 yardımcı`: yardımcıları altında listelenir ya da katlanmıştır.
- `↗ localhost:5173/`: işçi ya da yardımcının kendi metninde yazdığı en yeni yerel adres, terminalin açtığı bir bağlantı olarak (OSC 8). Yalnız `localhost`, `127.0.0.1` ya da `[::1]` üzerinde `http`/`https` sayılır, sorgu ve parça atılır, 60 karakteri aşan bir adres hiç gösterilmez. Oturumla birlikte gider.
- Yardımcıları olan bir işçi `▾ 2 yardımcı` ile, katlıysa `▸ 2 yardımcı  ● claude-1.1` ile biter: sayı ve en yüksekleri.

Her agent tek satırdır, ne söylerse söylesin: satırların yüksekliği değişmez, satırın parçaları da sütun sütun durur: her durum, derinlik ne olursa olsun aynı sütunda başlar, ondan sonraki her parça bir satırdaki en geniş hali kadar yer tutar, onu olmayan satırda boş kalır. Dar ekranda önce boşluklar gider. Süre, alıntı ve etkinlik çubuğu yalnız satırda maliyet, worktree durumu, yardımcılar ve bağlantıdan arta kalan yeri alır; dar bir bölme önce çubuğu, sonra alıntıyı, en son süreyi düşürür (`.` yan paneli daraltır). Yeni değişen bir satır bir an öne çıkar, sonra sakinleşir. Seçili satır ters renklidir.

Açık bir projenin bazı işçileri sizi bekliyorsa ve proje birden fazla işçi gösteriyorsa, işçiler neye ihtiyaç duyduklarına göre bir başlığın altında durur, merdivenin sırasıyla; geri kalanlar onları sayan tek bir başlığın altındadır:

```text
▾ shop-api      7 çalışıyor  $0.45   ! 2 bekliyor
  ● ▎lead-shop-api     koordine ediyor
  ■ takıldı · 1
  └ ■ ▎claude-6          #17  Order tests           ■ oturum bitti: oturum bitti
  ! seni bekliyor · 2
  ├ ! ▎claude-4          #1   Payment retry policy  ⚑ çalıştırmak istiyor "WebFetch: …"
  └ ! ▎claude-3          #7   Payment retry policy  ▸ Write it up  düşünüyor…
  ● 4 çalışıyor  ○ 1 boşta
  ├ ● ▎claude-5          #2   Price list cache      ▸ Run the tests  Bash: dotnet test
  └ …
```

Başlıklar `■ takıldı`, `! seni bekliyor`, `✓ bitti, bakılmadı` ve geri kalanlar için `● N çalışıyor  ○ N boşta`'dır (sıfır olan sayı yazılmaz); her biri kendi dalını `└` ile kapatır. Sizi bekleyenin nerede bitip sakin kısmın nerede başladığını gösterirler. Bir projenin bütün işçileri sakinse ya da proje tek bir işçi gösteriyorsa başlık yoktur. İlk yer veren onlardır: başlıklarla birlikte satırlar panele sığmazsa hiçbiri gösterilmez ve satırlar aşağıdaki gibi katlanır.

Satırlar panele sığmadığında sakin işçiler (çalışan ya da boşta, son zamanda bir şey dememiş, seçili olmayan) ekibin altından yukarıya doğru tek satıra katlanır: `… 2 agent katlı, en yükseği  ● claude-5`, en yükseklerinin adıyla; sabitlenmiş bir proje en son katlanır ve sizi bekleyen hiç katlanmaz. Panelin alt satırı `KUYRUK`, doğrulanan kartları ve son dört biteni (`#2 ✓`) listeler; canlıda doğrulama kısmını hiçbir şey doldurmaz, `boş` yazar.

## Durum merdiveni

Her agent tek bir basamakta durur, en yüksekten: takıldı (bloke ya da oturumu durdu: siz yazmadan bir yere varamaz) > bir şeyi sizi bekliyor > çalışıyor (tur ortasında, bir takım arkadaşına soruyor, araştırıyor) > başlıyor > bitti, bakılmadı > boşta (ya da uyuyor, ya da gitti). Basamak satırın işaretini, projesinde hangi işçinin önce geldiğini (takılan, sonra sizi bekleyen, sonra bitip bakılmamış; sakin basamaklar sıralarını korur ki agent'lar boştan işe gidip gelirken ekran zıplamasın), `f`'nin ne gösterdiğini ve katlı bir grubun (bir proje, bir işçinin yardımcıları, katlanan sakin işçiler) hepsi adına hangi üyeyi gösterdiğini belirler: en yüksek basamaktaki, eşitler arasında sıradaki ilk.

## Lejant

`?` yardımının listelediği her işaret, ekranın çizdiği gibi:

| İşaret | Anlamı |
| --- | --- |
| `!` | bir şeyi sizi bekliyor, o sırada ne yapıyorsa yapsın; başlıktaki sayı ve KARAR BEKLEYEN'in kendi işareti de |
| `●` | iş başında: tur ortasında, oturumu kalkıyor (durumu `başlıyor…`), bir takım arkadaşına soruyor (`? → claude-2`), araştırıyor |
| `○` | boşta ya da önceki çalıştırmadan beri uyuyor (ona yazmak uyandırır) |
| `✓` | bitti, bakılmadı: bir turu bitirdi, o zamandan beri açmadınız |
| `■` | takıldı: bloke (durumu `BLOKE`) ya da oturumu bitti (`■ durdu`; ona yazmak yeni bir oturum başlatır); bir proje başlığında takılan agent sayısı da |
| `⇄` | bir konuşma satırı |
| `▾` `▸` | bir proje ya da bir işçinin yardımcıları, açık ya da katlı |
| `▴` | sabitlenmiş proje |
| `~` | hafif kirli: bir araştırma raporu okudu (yalnız demoda) |
| `≈` | kirli: dış dünyayı okuyor (yalnız demoda) |
| `⎇` `↑` `↓` `+N −M` | bir işçinin worktree'si: dal, kendi commit'leri, checkout'un yeni commit'leri, henüz verilmemiş satırlar |
| `↗` | işçinin yazdığı yerel adres, terminalin açtığı bir bağlantı |
| `≡ 3/7` | kendi yapılacaklar listesi: biten adımlar ve hepsi |
| `▸ Run the tests` | çalışan bir satırda: yapılacaklar listesinde devam eden adım |
| `◔ %72` | bağlam penceresi bu kadar dolu; %70'ten gösterilir, %90'dan yüksek sesle |
| `✱` | `m` ile işaretlediğiniz satır; `'` sonrakine gider |
| `▁▃▅` | son zamanda yaptıkları: son beş tikin her birinde izine düşen satırlar, sütun başı 5 sn |

Ana ekranın ÇALIŞAN satırları aynı şekilleri kullanır, böylece mono temada da okunur.

## Hareket ve katlama

| Tuş | Ne yapar |
| --- | --- |
| `↑` `↓` / `j` `k` | üstteki ya da alttaki satırı seç |
| `gg` / `G` | ilk / son satır |
| `^d` `^u` / `^f` `^b` | yarım sayfa / tam sayfa |
| `←` `→` / `h` `l` | seçili projeyi ya da seçili işçinin yardımcılarını (seçili yardımcının işçisini de) katla / aç. Bir proje başlığında `enter` da katlar ve açar. |
| `za` | eldeki katlamayı değiştir |
| `zM` / `zR` (`:fold` / `:unfold`) | her projeyi ve her işçinin yardımcılarını katla / aç |
| `f` (`:needs`) | ekip yalnız sizi bekleyenleri gösterir, sonra yine herkesi |
| `s` (`:settle`) | seçili işçinin satırını ya da seçili projeyi (başlığı ya da lideri) size yeniden ihtiyacı olana dek gizle; `:unsettle` hepsini geri getirir |
| `&` (`:goto`) | bir agent'a ya da projeye adıyla ya da numarasıyla git, sizi bekleyenler önde; projesi ve işçisinin yardımcıları açılır, satırı gizleyecekse `f` bırakılır |
| `enter` | derine in: bir agent'ın odağı (kartı, durumu, kökeni ve hikâyesi), yine `enter` ham akışı (iz) |
| `K` (`:peek`) | göz at: eldeki agent'ın odağı görünümün üstünde bir pencerede, görünüm olduğu gibi kalır; `j` `k` sonraki / önceki agent, `enter` odağın tamamı, `@` ona yaz, `esc` ya da `K` kapatır |
| `F` (`:hints`) | seçilebilen her satırda ipucu harfleri; bir etiket satırını seçer, son harfi büyük yazılırsa açar |
| `t` (`:trace`) / `v` (`:talk`) | eldeki agent'ın ham akışı / konuşması |
| `esc` | bir derinlik yukarı |

Katlama seçimi işçiye ya da başlığa taşır, böylece seçim kaybolmaz.

## Yalnız sizi bekleyenler

Bir agent'ın sizden neye ihtiyacı olduğu tek bir sıradır, en önemlisi önce: bir şeyi KARAR BEKLEYEN'de bekliyor, takılmış (bloke ya da oturumu durmuş), bakmadığınız bir turu bitirmiş, hiçbiri. Bir projede işçiler bu sıraya göre dizilir, eşitler sırasını korur, böylece sakin olanlar yer değiştirmez. Satırlar sığmadığında önce sakinler katlanır.

`f` yalnız bir şeye ihtiyacı olan agent'ları tutar, her biri projesinin başlığı altında: bir işçi, bir şeye ihtiyacı olan yardımcısı için gelir, lider yalnız kendisinin ihtiyacı varsa. Hiçbir şeyi olmayan proje dışarıda kalır, bir şeyi olan katlı proje yalnız başlığıyla görünür ve hiçbir şey yoksa ekip `Şu an senden bir şey gerekmiyor · f herkesi gösterir` der. Panel başlığı süzgecin açık olduğunu söyler: `SALON · yalnızca seni bekleyenler (f)`. Açıkken, satırı giden bir seçim projesinin hâlâ görünen ilk satırına taşınır (yazarken değil: seçim `/` ile kime yazdığınızı belirler). Süzgeç yalnız bu çalıştırma boyunca sürer.

## Biteni kaldırmak

İşini bitirmiş ve baktığınız bir işçi yine de bir satır tutar. Satırında `s` onu kaldırır: satır, yardımcıları ve onun için çalışan araştırmacıyla birlikte, size yeniden ihtiyacı olana dek gider. Bir projenin başlığında ya da liderinin satırında `s` bütün projeyi kaldırır. Seçim hâlâ görünen sonraki satıra (yoksa öncekine) geçer.

Sizi bekleyen ya da takılmış bir şey hiç gizlenmez: `gizlenmedi: claude-2 seni bekliyor`. Henüz görülmemiş bitmiş bir tur sorun değildir; gizlemek onu görülmüş sayar. Gizlenmiş bir agent, satırındaki bir şeyin size yeniden ihtiyacı olduğu anda kendiliğinden döner: KARAR BEKLEYEN'de bir soru ya da değişiklik, bloke ya da durmuş olmak, ya da yeni bir turu bitirmek. Gizlenmiş bir proje, agent'larından biri dönünce döner. Hiçbir şey sessizce gitmez: panel başlığı kaldırılanları sayar, `SALON · 2 kaldırıldı (:unsettle)`, başlık satırındaki filo yine her agent'ı sayar ve `&` hepsini yine listeler; `&` ya da `:goto` ile birine gitmek satırını (ve projesini) geri getirir. `:unsettle` hepsini bir kerede geri getirir. Kaldırma yalnız bu çalıştırma boyunca sürer.

## Göz atmak

`K` eldeki agent'ın (ekipte seçili olan, bekleyen kalemin ya da günlük satırının ardındaki, ya da yakınlaştığınız) odağını ekranın üstünde, ondan biraz küçük bir pencerede açar; alttaki görünüm (ekip, pano, kod haritası ya da bir kart) kıpırdamaz. Pencerede `j` `k` (ya da oklar) ekibin sırasında sonraki / önceki agent'a geçer, `enter` odağın tamamını açar, `@` ona bir mesaj başlatır, `esc` ya da `K` kapatır; başka tuşlar bir şey yapmaz. Bitmiş bir agent'a böyle bakmak onu görmüş sayılır.

## İpucu harfleri

`F`, ekip ekranında seçilebilen her satırın başına bir ya da iki harflik bir etiket koyar: ekipteki proje başlıkları ve agent'lar (bir kartın ekranında onun kişileri), KARAR BEKLEYEN'in kalemleri ve görünen günlük satırları. İlk çizilen satırlar tek harf alır, önce orta sıra (`a s d f …`); satır harften çoksa son harfler iki harflik etiketleri başlatır (`ma`, `ms`, …), gerektiği kadar az, ve iki harflik bir etiketi başlatan harf kendi başına hiç etiket olmaz. Bir etiketi yazmak satırını seçer ve paneline geçer; son harfi büyük yazılırsa (`D`, `mS`) satırı `enter` gibi açar da: bir agent'ın odağı, bir değişiklik, bir sorunun konuşması, bir günlük satırının konuşması. O ana dek yazılan harfler etiketleri onlarla başlayanlara daraltır; hiçbir etikete uymayan harf bir şey yapmaz, `backspace` birini geri alır, `?` yardımı üstlerinde gösterir, `esc` kapatır. Etiketler `F` açıldığında verilir ve satırlarıyla kalır: yer değiştiren satır etiketini götürür, ekrandan çıkan onu kaybeder ve etiketini yazmak `o satır ekrandan çıktı` der. Ekranda böyle satır yoksa (pano, kod haritası, bir odak, ekibin üstünde bir konuşma) `F` bunu söyler. Hem demo hem canlı oturumlarda vardır.

## Yan paneller

Sağda üstte KARAR BEKLEYEN, altında DEFTER. `tab` ve `⇧tab` (ya da `]` ve `[`) panelleri döndürür: ekip → bekleyenler → günlük; `w` (`:waiting`) doğrudan bekleyenler paneline gider. Bekleyenler paneli ögelerinin gerektiği kadar satır alır, en çok yan tarafın yarısı; ögeler beklerken ve panel odakta değilken çerçevesi uyarı rengine döner. `:clear` iki panelin içeriğini hiçbir şey silmeden ekrandan kaldırır; bkz. [günlük](journal.md#ekranı-temizlemek).

`,` ve `.` (ya da `<` ve `>`) yan paneli dört sütun genişletir ya da daraltır, ekranın %20'si ile %70'i arasında; flaş `yan panel %38 (, . ya da < >)` der ve genişlik sonraki çalıştırma için saklanır. Bir değişikliğin görünümünde `>` bunun yerine bir parçayı alıntılar; `.` yine daraltır.

## Okunaklılık

Ekibin üstünde açılan bir pencere (bir konuşma, bir göz atma, yardım, uzaktayken özeti, nerede kalmıştın) çevresindeki her şeyi soluklaştırır, böylece ağaç ve günlüğün kesilen satır sonları arka plan olarak okunur. Günlük paneli ve bekleyenlerdeki bir soru, bir mesajı tek satır düz söz olarak gösterir: temizlenmiş, satırları birleştirilmiş, başlık ve liste işaretleri atılmış, bir bağlantı etiketiyle. İz her satırı, ilk sözcüğünden okunan türüne göre bir glifle işaretler: `›` agent'ın kendi metni, Markdown olarak çizilir (listeler, başlıklar, kod renginde kod, çitler hiç gösterilmez); `▸` bir araç çağrısı, adı bilgi renginde, komutu ya da yolu kod renginde, bir çalıştırmanın sonu çıkış koduna göre; `⇄` bus (bir mesaj, bir uyandırma, bir işe alım, pano), sözcüğü vurgu renginde; `•` Claude Code'un kendi notu (bir yeniden deneme, bir ret); `✕` ters giden; `·` imece'nin kendi satırları. Odak, hikâyesinde agent'ların yazdıklarını aynı biçimde çizer: tek satırlık bir mesaj ya da bir karar işaretleriyle kendi satırında, daha uzun bir mesaj (ya da bir liste, bir başlık, kod) başlığının altında Markdown olarak, ilk sekiz satırı ve kaç tane daha olduğu; imece'nin kendi hikâye satırları düz kalır. İkisinde de satırında bütün görünen bir web adresi (yalnız `http` ya da `https`) terminalinizin tıklayınca açtığı bir bağlantıdır (OSC 8): metin adresin kendisidir, çizildiği gibi temizlenmiştir; satır sonunda kesilen bir adres hiçbir yere bağlanmaz. Açık bir konuşma konuşmayı izler: gelen satırlar eklenir, seçim son satırdaysa onunla gider.

Bir agent'ın yazdığı her şey ekrana yalnız kontrol karakterlerini atan ve gizli anahtara benzeyen parçaları saklayan bir temizlikten geçerek ulaşır; hiçbir şey ham çizilmez.

## Ayrıca bakınız

- [KARAR BEKLEYEN](waiting.md)
- [Günlük](journal.md)
- [Konuşmalar](conversations.md)
- [Pano ve kod haritası](board-and-code-map.md)
- [Klavye](keyboard.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Projeler](../concepts/projects.md)
