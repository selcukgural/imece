---
title: Ekip
description: Ana ekrandaki ekip - başlık satırı, proje proje ÇALIŞAN, her agent'ın satırı ve işareti, katlama, kaldırma, pencereler.
---

# Ekip

Ekip, [ana ekranın](home.md) alt yarısıdır, ÇALIŞAN (kayıt 125): `:salon` oraya gider, `gw` gibi. Bu sayfanın anlattıkları (her proje, işçileri ve yardımcıları, katlama, `s`) orada proje proje durur; yanında bekleyenler ana ekranın KARAR BEKLEYEN'i, [günlük](journal.md) (hikâye olarak anlatılan) ise bir penceredir, `J` ya da `:journal`. Pano, kod haritası ve zaman şeridi de pencere olarak açılır (`:board`, `:code`, `:timeline`); geri kalan her şey ana ekranın üstünde açılır ve `esc` onu geri getirir.

```text
 imece  shop-api · mobile-app │ $0.13 / $200.00  14:05
  lonca mobile-app 9d3f1c2 · 1 bekliyor
 ÇALIŞAN 3
▸▾ shop-api      ░░░░░░░░ 0/1  1 çalışıyor  $0.04  ~/src/shop-api
 ● shop-api    #1   Kargo bölgeleri           Testleri çalıştır · 4 dk
 ▾ mobile-app    ░░░░░░░░ 0/3  2 çalışıyor  $0.08  lonca mobile-app 9d3f1c2 · 1 bekliyor  ~/src/mobile-app
 ● mobile-app  #2   Kargo ücret hesabı        Testleri çalıştır · 2 dk
 ● mobile-app  #3   Ürün aramasında sayfala…  Testleri çalıştır · 4 dk
 ana ekran · shop-api    enter  aç   h l  katla · aç   s  gizle   ?  yardım
```

## Başlık satırı

Tek satır, dört şey, parçalar `│` ile ayrılmış: projeler, sizi bekleyenler, kese, saat; bir kullanım penceresi yalnız dolmak üzereyken aralarına girer. Ekran satırdan darsa önce en az istenen parça düşer; böylece ekranın kenarı sizi bekleyeni asla kesmez: saat, sonra projeler. Sizi bekleyenler, kese, duraklatma ve dolmak üzere olan bir pencere her zaman kalır.

| Parça | Ne söyler |
| --- | --- |
| `shop-api · mobile-app` | açık projeler. [Ana ekranda](home.md) listenin daraltıldığı proje aydınlıktır. |
| `! 2 karar bekliyor` | KARAR BEKLEYEN'deki, karar vermenizi isteyen açık ögeler, dikkat renginde; okunacak yanıtlar ayrı ve soluk sayılır, `1 okunacak`, dar ekranda ilk onlar gider. Terminalin başlığı karar bekleyenlerin sayısını taşır, `! imece · 2 karar bekliyor`, böylece arkadaki bir sekme de gösterir; başında açık projelerin durum merdiveninin tepesi vardır: `■` takılan var, `!` sizi bekleyen var, `●` çalışan ya da başlayan var, her şey sakinse imece'nin işareti `⁙` (bitmiş ama görülmemiş bir tur sakindir). Windows Terminal, kitty 0.39+, Ghostty 1.2+ ve iTerm2 3.6.6+ bunu sekmenin ilerleme çubuğu olarak da gösterir (OSC 9;4): takılınca dolu kırmızı, beklerken dolu sarı, çalışırken hareketli, sakinken yok. imece başlarken terminale adını sorar (XTVERSION) ve `TERM_PROGRAM`'a değil onun cevabına bakar, çünkü başka bir terminalden açılan terminal onu miras alır; bazı terminaller bu diziyi bildirim sandığı için başkalarına gönderilmez. tmux içinde gönderilir: tmux 3.8 onu Ghostty ve iTerm2'ye iletir, daha eski bir tmux düşürür. `IMECE_PROGRESS=on` ya da `off` ile kendiniz karar verirsiniz. imece bitince ya da terminali bir `!` komutuna bırakınca kabuğunuzun başlığı geri gelir, çubuk kalkar. |
| `$3.10 / $20.00` | kese (bütçe): bütün agent'ların birlikte harcadığı. O anki hızla yarım saatten az kaldığında parça dikkat rengine döner ve ne kadar kaldığını söyler, `$19.60 / $20.00, ~25 dk kaldı`; günlük bunu bir kez söyler. Hız ve kalan süre başka her zaman `:budget` menüsünün notundadır. |
| `claude 5 sa: %10 kaldı, 1 sa 59 dk sonra yenilenir` | planınızın dolmaya en yakın kullanım penceresi, yalnız %90 kullanımdan itibaren, dikkat renginde: bir aboneliğin sınırı takımı keseden önce durdurabilir. Onun altında `:budget` notundadır. Çalıştırmalar arasında saklanmaz: oturum ilk turunda yeniden söyler. |
| `14:02` | saat; demonun `‖ DURAKLATILDI`sı ya da `×4`ü ona yapışık. |

Son on dakikada olanlar, kart sayısı ve araştırmacı üst sınırı kayıt 125 ile başlıktan çıktı: kartlar panoda, olanlar günlükte. Filo (`● 2 çalışıyor  ✓ 1 bitti  ■ 1 takıldı`) 2. adımla çıktı: ana ekranın ÇALIŞAN başlığı kimin çalıştığını, kimin takıldığını, kimin boşta olduğunu sayar, her satır da agent'ının ne yaptığını söyler.

Kesenin ve pencerelerin nasıl sayıldığı [bütçe ve sınırlar](../concepts/budget-and-limits.md) sayfasında.

## Proje proje

ÇALIŞAN açık projeleri açıldıkları sırayla listeler, sabitlenenler önce. Görünürde birden çok proje varken (`[` `]` ile birine daraltılmamışsa) her biri ajanlarının başına kendi satırını koyar; birine daraltılmışken ya da tek proje açıkken satırlar tek başına durur. Bir projenin altında işçileri ve sololarıdır: önce takılanlar, sonra çalışanlar, sonra sakinler; eşitler sırasını korur, böylece agent'lar boştan işe gidip gelirken liste zıplamaz; her birinin yardımcıları ve onun için çalışan araştırmacı altına asılıdır (`└`). Bir lider yalnız çalışırken ya da takılmışken listelenir; sizden evet ya da yanıt bekleyen bir agent ise KARAR BEKLEYEN'deki o ögenin kendisidir. ÇALIŞAN başlığı onları sayar: `ÇALIŞAN 12 · takılan 2 · bitiren 1 · boşta 1` (bitiren: boşta, elindeki son işi bitmiş; boştaki bir satırın sözleri [ana ekranda](home.md)). Satırlar ekrana sığmadığında liste eldeki satırla kayar; hiçbir şey kendiliğinden katlanmaz.

Henüz hiçbir projede işçi yokken (yalnızca lideri olan bir ekip, bir solo) imece'nin işareti ve yazıları boş satırların ortasında durur: `imece v…`, `tek terminal, birçok agent, bir lider`, eldeki proje ve başında kim olduğu. Göründüğünde noktaları birer vuruşta gelir (bkz. [ilk çalıştırma](../getting-started/first-run.md#hoş-geldin-penceresi)); ilk işçinin satırı yerini alır.

Bir projenin satırı: `▾ shop-api  ▓▓▓░░░░░ 3/8  ! 2 bekliyor  ■2  2 çalışıyor  $2.40  ~/src/shop-api`. `▾` açık, `▸` katlı. Adın ardından: kartlarının ne kadar ilerlediği (sekiz hücre, biten payı aşağı yuvarlanarak dolu, ardından biten / hepsi; lideri kart asmadan önce yok), dikkat renginde bu projenin sizi bekleyen öge sayısı, varsa `■N` takılan (bloke ya da durmuş) agent, boşta olmayan işçi ve yardımcı sayısı, projenin agent'larının harcadığı, Lonca deposunun durumu ve en sonda, açıkken, çalıştığı klasör. Sizi isteyen önce gelir; ekrana sığmayan satır gerisini kaybeder. Katlı bir proje tek satırdır, o satır da bekleyenleri gösterir (sessiz ama sağır değil) ve en yüksek üyesiyle biter, `▸ shop-api  ! 1 bekliyor  2 çalışıyor  $2.40  ! claude-5`: aşağıdaki durum merdiveninde en üstteki agent, eşitler arasında sıradaki ilk. Sabitlenmiş bir proje (`:pin`) adının önünde `▴` taşır ve listenin en üstünde kalır; sabitleme çalıştırmalar arasında saklanır.

## Bir agent'ın satırı

Ana ekranda bir agent'ın satırı kısadır ve önce işi söyler:

```text
 ● #16  Arama sonuç sıralaması    Değişikliği yap · 3 dk
 ■ #8   Kargo ücret hesabı        oturum bitti: süreç 1 koduyla çıktı · 9 dk
```

- Durumunun işareti: `●` iş başında (tur ortasında, başlarken, bir takım arkadaşına sorarken, araştırırken), `○` boşta ya da uyuyor, `■` takıldı (bloke ya da oturumu bitti: hangisi olduğunu sözler söyler), `!` sizi bekliyor. Her durum kendi biçimidir, mono tema da ayırt eder; liderin satırı da herkesinki gibi durumunun işaretini taşır. Hareket açıkken ([`:motion`](../reference/commands.md#motion-offcalmfull), varsayılan calm) çalışan bir satırın işareti renginde nefes alır (şeklinde asla), sizi beklemeye düşen bir satır üç kez yanıp söner, turu biten ya da oturumu kalkan bir satır yeşil bir zeminden yatışır; başka penceredeyken hiçbir şey hareket etmez.
- Projesi, soluk, birden çok proje açıkken ve liste birine daraltılmamışken.
- Kartın numarası ve başlığı; kartı olmayan bir agent bunun yerine adını gösterir. Bir yardımcı ya da araştırmacı işçisinin altına asılır, `└`, numarasız.
- Ne yaptığı: yapılacaklar listesinde devam eden adım (`Değişikliği yap`), yoksa aşağıdaki sözlerle durumu, satırda kalan yere göre kesilmiş.
- O durumda ne zamandır olduğu, bir dakikadan başlayarak: `· 3 dk`, `· 1 sa 5 dk`. Sizi bekleyen bir şey için öge geldiğinden beri.

Eldeki satırın önünde `▸` vardır ve seçim rengindedir. `enter` agent'ın odağını açar; onunla ilgili geri kalan her şey oradadır: klasörü, worktree durumu, yapılacakları, bağlam penceresi, etkinliği ve hikâyesi.

### Durum sözleri

Oturumu kalkarken (süreç, ilk worktree'nin checkout'u) `başlıyor…`, `düşünüyor…`, yaptığını söylediği şey, bir takım arkadaşının yanıtını beklerken `? → claude-2`, bir sorusu sizi beklerken `? → sen`, bir araç çağrısı KARAR BEKLEYEN'de beklerken `⚑ çalıştırmak istiyor`, yanıtı okunmayı beklerken `sana yanıt verdi`, geçici bir API hatasıyla biten bir turu imece yeniden denemeyi beklerken `↻ 2/3 · 25 sn sonra` (deneme, deneme sayısı, kalan süre), bir API hatası sizi beklerken `■ API hatası`, işçileri çalışan bir lider için `koordine ediyor`, `BLOKE`, `■ oturum bitti · yazınca yeniden başlar`, `◦ önceki çalışmadan beri uyuyor, yazınca uyanır`. Durumun kendi metni varsa satır onu taşır: `? → sen "hangi index?"`, `sana yanıt verdi "enum kullan"`, `⚑ çalıştırmak istiyor "cargo test"`, `■ API hatası "Invalid API key"`, `BLOKE "JWT saat kayması" ← researcher-1`, `■ oturum bitti: süreç 1 koduyla çıktı`. Agent sizi beklerken onunla bekleyen mesajlar önce yazar, `✉ 2 kuyrukta ? → sen "hangi index?"`; başta `✓ tur`, bu tur istediği her şeye evet dediğinizi söyler.

### Uzun satır

Bir [kartın ekranı](cards.md), kartta payı olanları uzun satırla listeler; bir agent'ın her rakamı tek satırda:

```text
sahibi    ● ▎claude-1  #3   Parse the manifest  ▸ Write the tests  Edit tests/parse.rs · 2 dk  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  1 yardımcı  ↗ localhost:5173/
```

- Durum işareti: agent'ın [durum merdivenindeki](#durum-merdiveni) yeri. Yine beş işaret, iki anlam daha ile: bir şeyi sizi beklerken, o sırada ne yapıyorsa yapsın `!`, ve bir turu bitirmiş ama henüz açmadığınız bir işçi için `✓`.
- Agent'ın konuşmacı renginde `▎`: günlükte ve bir konuşmada satırlarının taşıdığı renk, böylece satır ve konuşması tek agent olarak okunur.
- Adı; sonra kirliyse `~` ya da `≈` (araştırma kirliliği, yalnız demoda).
- Bir işçinin kartı: `#3` ve kartın başlığı.
- Durumu, yukarıdaki sözlerle. Yapılacaklar listesinde devam eden bir adımı olan çalışan bir agent önce o adımı, ardından onun için ne yaptığını soluk renkte söyler: `▸ Write the tests  Edit tests/parse.rs`, `▸ Run the tests  düşünüyor…`; böylece satır yalnız son araç çağrısını değil, agent'ın neyin peşinde olduğunu söyler. Adım en çok 24 sütun alır ve satırın sabit parçalarının bıraktığından fazlasını asla almaz; araç çağrısı adımdan arta kalanı alır (onayınızı bekleyen bir araç çağrısı ya da hâlâ başlayan bir oturum bunun yerine onu söyler). Yoksa durum, agent'ın en son alışverişiyle biter; ok kimin söylediğini gösterir: `düşünüyor… ← lead "parser'ı düzelt"`, `boşta → sen "parser düzeldi, testler yeşil"`, `? → claude-2 "hangi parser?"` (beklediği kişiye kendi sorusu, tek başına alıntılanır). Alışveriş bir sonrakine dek kalır; böylece satır agent'ın en son ne yaptığını söyler, durum ve metni kırk sütunu geçmez.
- `▁▃▅▂▁`: agent'ın son zamanda yaptıkları, son beş tikin her biri için bir sütun (her biri 5 saniye, toplam 25), en eskisi önce: o tikte izine kaç satır düştüğü (bir metin, bir araç çağrısı, bir mesaj, bir not), her satırda aynı ölçekle (`▁` hiç, `▂` bir, `▃` iki, `▄` dörde kadar, `▅` altı, `▆` dokuz, `▇` on dört, `█` daha fazla); böylece yoğun bir agent'la sakin bir agent bir bakışta ayrılır. Yalnızca agent bir şey yaptıkça ve tikler geçtikçe değişir, kendi zamanlayıcısıyla asla; beş tik boyunca hiçbir şey yazılmazsa kaybolur. Dar bir satırda alıntıdan da önce, ilk yer veren odur ve [`:motion off`](../reference/commands.md#motion-offcalmfull) onu göstermez. Odakta (`enter`) daha uzunu vardır, iki dakika, aynı ölçekte 24 sütun, saydıkları satır sayısıyla: `etkinlik: ▁▁▂▄▁…█▃ · 2 dakikada iz satırı: 21`. Çalıştırmalar arasında saklanmaz.
- O durumda ne zamandır olduğu, bir dakikadan başlayarak, ana ekrandaki gibi.
- Varsa harcadığı.
- `≡ 3/7`: kendi yapılacaklar listesinin neresinde olduğu, biten adımlar ve hepsi; planlama aracı bir liste tutuyorsa (Claude Code'un görevleri). Devam eden adım satırın durumudur (`▸`); odak hepsini listeler. Satır darsa işaret alıntıdan ve süreden sonra yer verir. Oturumla birlikte gider.
- `◔ %72`: bağlam penceresinin ne kadar dolu olduğu, %70'ten itibaren, %90'dan itibaren dikkat renginde, Claude Code'un son söylediğince (mesaj kullanımı ve sonucun adlandırdığı pencere). Odak rakamı her hâlde gösterir, `bağlam: %22 · 45k / 200k`, ya da pencerenin boyutu söylenmiyorsa yalnız token sayısını. Dar bir satırda yapılacaklar işaretinden daha uzun kalır. Oturumla birlikte gider.
- Bir işçinin worktree durumu, `⎇ fix-parser ↑2 ↓1 +12 −3`: agent'ın açtığı dal (worktree imece'nin bıraktığı gibi detached iken yok; bir başlangıç noktasından işe alınan işçi onun yerine o noktayı gösterir, `⎇ #41`), checkout'ta olmayan kendi commit'leri (imece'nin uygulanan değişiklik işaretleri sayılmaz), kendisinde olmayan checkout commit'leri ve kendi HEAD'ine göre eklenen ve silinen satırlar, izlenmeyen dosyalar dahil: bir sonraki `done`'un size vereceği şey. Her parça yalnız söyleyecek bir şey varken görünür. imece'nin git iş parçacığında okunur: oturum başlarken, bir tur bitince, bir uygulamadan sonra ve işçi ya da bir yardımcısı tur ortasındayken her 15 saniyede; çalıştırmalar arasında saklanmaz.
- `1 yardımcı`: işçinin kaç yardımcısı olduğu; ana ekran onları altında (`└`) listeler.
- `↗ localhost:5173/`: işçi ya da yardımcının kendi metninde yazdığı en yeni yerel adres, terminalin açtığı bir bağlantı olarak (OSC 8). Yalnız `localhost`, `127.0.0.1` ya da `[::1]` üzerinde `http`/`https` sayılır, sorgu ve parça atılır, 60 karakteri aşan bir adres hiç gösterilmez. Oturumla birlikte gider.

Süre, alıntı ve etkinlik çubuğu yalnız satırda maliyet, worktree durumu, yardımcılar ve bağlantıdan arta kalan yeri alır; dar bir ekran önce çubuğu, sonra alıntıyı, en son süreyi düşürür.

## Durum merdiveni

Her agent tek bir basamakta durur, en yüksekten: takıldı (bloke ya da oturumu durdu: siz yazmadan bir yere varamaz) > bir şeyi sizi bekliyor > çalışıyor (tur ortasında, bir takım arkadaşına soruyor, araştırıyor) > başlıyor > bitti, bakılmadı > boşta (ya da uyuyor, ya da gitti). Basamak uzun satırın işaretini, terminalin başlığının ve sekmesinin açık projeler için taşıdığı işareti ve katlı bir projenin hepsi adına hangi üyeyi gösterdiğini belirler: en yüksek basamaktaki, eşitler arasında sıradaki ilk. Ana ekran satırlarını basamağa göre değil, kendi gruplarına göre sıralar (takılan, çalışan, sakin).

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
| `▾` `▸` | bir proje, açık ya da katlı |
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
| `↑` `↓` / `j` `k` | üstteki ya da alttaki satır; önizleme onu izler |
| `gg` / `G`, `gd` / `gw` | ilk / son satır · karar bekleyen ilk satır / çalışan ilk satır |
| `^d` `^u` / `^f` `^b` | yarım sayfa / tam sayfa |
| `[` / `]` | bütün projeler, sonra sırayla her biri, dönerek: başlık satırı gösterileni aydınlatır |
| `←` `→` / `h` `l` | eldeki satırın projesini başlığına katla / yeniden aç |
| `za` | eldeki katlamayı değiştir |
| `zM` / `zR` (`:fold` / `:unfold`) | her projeyi katla / aç |
| `s` (`:settle`) | eldeki satırın agent'ını, bir başlıkta projeyi, size yeniden ihtiyacı olana dek gizle; `:unsettle` hepsini geri getirir |
| `&` (`:goto`) | bir agent'a ya da projeye adıyla ya da numarasıyla git, sizi bekleyenler önde; projesi açılır, gizliyse satırı geri gelir |
| `enter` | derine in: bir agent'ın odağı (kartı, durumu, kökeni ve hikâyesi), yine `enter` ham akışı (iz); bir başlıkta liderin odağı |
| `K` (`:peek`) | göz at: eldeki agent'ın odağı ekranın üstünde bir pencerede, ekran olduğu gibi kalır; `j` `k` sonraki / önceki agent, `enter` odağın tamamı, `@` ona yaz, `esc` ya da `K` kapatır |
| `t` (`:trace`) / `v` (`:talk`) | eldeki agent'ın ham akışı / konuşması |
| `esc` | bir derinlik yukarı |

Katlama imleci başlığa taşır, böylece kaybolmaz.

## Biteni kaldırmak

İşini bitirmiş ve baktığınız bir işçi yine de bir satır tutar. Satırında `s` onu kaldırır: satır, yardımcıları ve onun için çalışan araştırmacıyla birlikte, size yeniden ihtiyacı olana dek gider. Bir projenin başlığında `s` bütün projeyi kaldırır. Seçim hâlâ görünen sonraki satıra (yoksa öncekine) geçer.

Sizi bekleyen ya da takılmış bir şey hiç gizlenmez: `gizlenmedi: claude-2 seni bekliyor`. Henüz görülmemiş bitmiş bir tur sorun değildir; gizlemek onu görülmüş sayar. Gizlenmiş bir agent, satırındaki bir şeyin size yeniden ihtiyacı olduğu anda kendiliğinden döner: KARAR BEKLEYEN'de bir soru ya da değişiklik, bloke ya da durmuş olmak, ya da yeni bir turu bitirmek. Gizlenmiş bir proje, agent'larından biri dönünce döner. Hiçbir şey sessizce gitmez: ÇALIŞAN başlığı kaldırılanları sayar, `ÇALIŞAN 12 · 2 gizli (:unsettle)`, ve `&` hepsini yine listeler; `&` ya da `:goto` ile birine gitmek satırını (ve projesini) geri getirir. `:unsettle` hepsini bir kerede geri getirir. Kaldırma yalnız bu çalıştırma boyunca sürer.

## Göz atmak

`K` eldeki agent'ın (ana ekranda eldeki satır, eldeki ögenin agent'ı, ya da yakınlaştığınız) odağını ekranın üstünde, ondan biraz küçük bir pencerede açar; altındaki kıpırdamaz. Pencerede `j` `k` (ya da oklar) ÇALIŞAN'ın sırasında sonraki / önceki agent'a geçer, `enter` odağın tamamını açar, `@` ona bir mesaj başlatır, `esc` ya da `K` kapatır; başka tuşlar bir şey yapmaz. Bitmiş bir agent'a böyle bakmak onu görmüş sayılır.

## Pencereler

Bir zamanlar ekibin yanında duranlar ana ekranın üst yarısı ve birkaç penceredir. KARAR BEKLEYEN üst yarıdır ([KARAR BEKLEYEN](waiting.md)). [Günlük](journal.md) bir penceredir, `J` ya da `:journal`: satırları en yeni önce, ya da kart kart hikâye (içinde `J` ikisi arasında geçer); pano, kod haritası ve zaman şeridi de pencere olarak açılır (`:board`, `:code`, `:timeline`). `esc` ya da `q` bir pencereyi kapatır, bir başkasını açan tuş ilkini kapatır, `W` de karar bekleyen sıradaki ögeyi açmak için hepsini kapatır. `:clear` karara bağlanan ögeleri ve günlüğün satırlarını hiçbir şey silmeden ekrandan kaldırır; bkz. [günlük](journal.md#ekranı-temizlemek).

## Okunaklılık

Ana ekranın üstünde açılan bir pencere (bir konuşma, bir göz atma, yardım, uzaktayken özeti, nerede kalmıştın) çevresindeki her şeyi soluklaştırır, böylece liste arka plan olarak okunur. Günlük penceresi ve bir sorunun satırı, bir mesajı tek satır düz söz olarak gösterir: temizlenmiş, satırları birleştirilmiş, başlık ve liste işaretleri atılmış, bir bağlantı etiketiyle. İz her satırı, ilk sözcüğünden okunan türüne göre bir glifle işaretler: `›` agent'ın kendi metni, Markdown olarak çizilir (listeler, başlıklar, kod renginde kod, çitler hiç gösterilmez); `▸` bir araç çağrısı, adı bilgi renginde, komutu ya da yolu kod renginde, bir çalıştırmanın sonu çıkış koduna göre; `⇄` bus (bir mesaj, bir uyandırma, bir işe alım, pano), sözcüğü vurgu renginde; `•` Claude Code'un kendi notu (bir yeniden deneme, bir ret); `✕` ters giden; `·` imece'nin kendi satırları. Odak, hikâyesinde agent'ların yazdıklarını aynı biçimde çizer: tek satırlık bir mesaj ya da bir karar işaretleriyle kendi satırında, daha uzun bir mesaj (ya da bir liste, bir başlık, kod) başlığının altında Markdown olarak, ilk sekiz satırı ve kaç tane daha olduğu; imece'nin kendi hikâye satırları düz kalır. İkisinde de satırında bütün görünen bir web adresi (yalnız `http` ya da `https`) terminalinizin tıklayınca açtığı bir bağlantıdır (OSC 8): metin adresin kendisidir, çizildiği gibi temizlenmiştir; satır sonunda kesilen bir adres hiçbir yere bağlanmaz. Açık bir konuşma konuşmayı izler: gelen satırlar eklenir, seçim son satırdaysa onunla gider.

Bir agent'ın yazdığı her şey ekrana yalnız kontrol karakterlerini atan ve gizli anahtara benzeyen parçaları saklayan bir temizlikten geçerek ulaşır; hiçbir şey ham çizilmez.

## Ayrıca bakınız

- [KARAR BEKLEYEN](waiting.md)
- [Günlük](journal.md)
- [Konuşmalar](conversations.md)
- [Pano ve kod haritası](board-and-code-map.md)
- [Klavye](keyboard.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Projeler](../concepts/projects.md)
