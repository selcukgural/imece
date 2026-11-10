---
title: Ana ekran
description: imece'nin açıldığı ekran - karar listesi - karar bekleyenler, karar vermenin maliyetine göre gruplu, sonra çalışanlar; her satır isteğin tamamını ve karar tuşlarını söyler.
---

# Ana ekran

Ana ekran, imece'nin açıldığı ve karar verdiğiniz ekrandır; `:home` geri getirir. Açık projelerin hepsi için tek liste: önce karar bekleyenler, karar vermenin size maliyetine göre gruplu, sonra çalışanlar. Her satır isteğin tamamını söyler, eldeki satırı karara bağlayan tuşlar satırın sonunda durur ve bir evet imleci aşağı indirir. 120 kolondan itibaren eldeki satır listenin yanında okunur.

```text
 imece  shop-api │  ! 4 karar bekliyor  │ $0.89 / $200.00  14:33
 KARAR BEKLEYEN 1 izin · 3 değişiklik
 izinler                                                                           her biri tek tuş
▸! #1   Kargo ücret hesabı        .imece/check değişikliklerinde çalışsın mı?  a  evet   r  hayır
                                  set -e; cd "$IMECE_WORKTREE"; npm run build; npm test -- --silent
 değişiklikler                                                                        3 onaya hazır
 ! #11  Para yuvarlama kuralları  ✓ dotnet build · ✓ dotnet test · 1 dosya +2 −1, #11 içinde ·
                                  başka değişiklikle çakışmıyor
 ! #12  Arama sonuç sıralaması    ✓ dotnet build · ✓ dotnet test · 1 dosya +2 −1, #12 içinde ·
                                  başka değişiklikle çakışmıyor
 ! #13  E-posta şablonları        ✓ dotnet build · ✓ dotnet test · 3 dosya +6 −3, hepsi #13 içinde
                                  başka değişiklikle çakışmıyor

 ÇALIŞAN 3
 ● #17  Fiyat listesi önbelleği   Çevresindeki kodu oku · 27 dk
 ● #16  Arama sonuç sıralaması    Değişikliği yap · 3 dk
 ● #7   Ödeme dokümanı            Lidere yaz · 5 dk

 ○ lead  Üç kart çıktı; ayrıştırıcı claude-2'de. · 2 dk                      /  lidere yaz
 ana ekran · shop-api · #1 Kargo ücret hesabı    enter  aç   a  evet   r  hayır   ?  yardım
```

[Başlık satırı](salon.md#başlık-satırı) açık projeleri adlandırır; listenin daraltıldığı proje (`[` `]`) aydınlıktır. Lonca'daki bir projenin, nasıl durduğunu söyleyen kendi satırı listenin üstündedir.

## Liste

| Grup | İçindekiler |
| --- | --- |
| KARAR BEKLEYEN | her türü sayar: `1 durdu · 1 izin · 2 soru · 3 değişiklik · 1 yanıt · 1 ertelenen` |
| durdu | bütün bir ekibi durduran şey, her şeyden önce: [bütün ekip durunca](#butun-ekip-durunca) |
| izinler | her biri tek tuş: bir ajanın çalıştırmak istediği komut, worktree kurulum betiği, deponun kontrolü. Satır neyin çalışacağını söyler: `.imece/check değişikliklerinde çalışsın mı?` ve ardından betiğin komutları (yorum olmayan satırları; `enter` her baytını gösterir) |
| sorular | okuyup yanıtlanacaklar: bir ajanın sorusu bütün olarak, altında sunduğu yanıtlarla (`1`-`9` birini seçer, `/` kendinizinkini yazar: [sorular](../concepts/approvals-and-questions.md#sorular-)), bir araştırmacının raporu, bir API hatası, liderin planı; liderin size sorduğu bir takılma (satır takılan işindir: `claude-2 takıldı · lead soruyor: …`) ya da kendi hâline bıraktığı bir takılma (`takıldı: … · lead çözmedi`): [takılan işçi](../concepts/messages-and-board.md#takılan-işçi) |
| değişiklikler | okunacak bir diff: `✓ dotnet build · ✓ dotnet test · 2 dosya +4 −2 · 1 kartın dışında (#51): Money.cs` — başı olmadan [hüküm](changes.md#yamanın-üstündekiler): ağacında kanıtlanan (çalıştıysa deponun kontrolü, sonra imece'nin çalıştırmaları, yoksa `hiçbir şey koşmadı`), kartının sahip olduğu yollara göre dosyalar ve satırlar, bekleyen hangi başka değişiklikle çakıştığı, onayından sonra Lonca'nın ne çalıştıracağı. Hazır olanlar (temiz kanıtlanmış, yollarının içinde, çakışmayan, uyan, bütün) önce gelir; grubun satırı onları sayar, `2 onaya hazır` |
| yanıtlar | bir ajanın kapanış sözleri (`answered: …`): karar verilecek bir şey yok, bu yüzden her karardan sonra gelir; `enter` tamamını okur, `r` kapatır. Başlık çubuğu bunları ayrı sayar, `1 okunacak`, hiçbir zaman `! N karar bekliyor` içinde değil |
| sonraya | `s` ile ertelediğiniz ögeler, ertelenme sırasıyla |
| ÇALIŞAN | `ÇALIŞAN 12 · takılan 2 · bitiren 1 · boşta 1 · 2 gizli (:unsettle)`: ekip, proje proje, açıldıkları sırayla, sabitlenenler önce. Görünürde birden çok proje varken (`[` `]` ile birine daraltılmamışsa) her biri ajanlarının başına [ekipteki](salon.md) satırını koyar: `▾ shop-api  ▓▓░░ 3/8  ! 2 bekliyor  1 çalışıyor  $4.10  ~/src/shop-api`; `h` onu o satıra katlar, `l` açar, `za` değiştirir, `zM` `zR` hepsini katlar ve açar. Altında işçiler ve sololar: önce takılanlar (`■`, bloke ya da oturumu bitmiş), sonra tur ortasındakiler ya da bir takım arkadaşına soranlar (`●`, bulundukları adım ya da yaptıkları, sonra bir dakikadan itibaren ne zamandır), sonra boştakiler (`○`), her birinin yardımcıları ve araştırmacısı altına asılı (`└`). Boştaki bir işçi ya da solo, elindeki son işin ne olduğunu da söyler; böylece işini bitiren, hiç iş almamış olandan ayrı okunur: `boşta · bitirdi, uygulandı` (değişikliği alındı), `boşta · bitirdi` (bitti, uygulanacak değişiklik yok), `boşta · #12 başarısız`, `boşta · #12 geri çekildi`, `boşta · henüz iş yok`; başlık bitirenleri öteki boştakilerden ayrı sayar. Buna dar gelen satır önce süreyi, sonra `boşta` sözünü bırakır. Değişikliği bekleyen ve başka işe geçmiş bir işçi burada da görünür: iki ayrı iş. Sizden evet ya da yanıt bekleyen ajan ise o ögenin kendisidir, yardımcıları işçilerden sonra kendi başlarına durur; işi olmayan bir lider listelenmez. Bir ajanda ya da başlıkta `s` onu size ihtiyacı olana dek gizler; başlık gizlenenleri sayar. Liderine takıldığını söyleyen işçi burada, çalışanlar arasında kalır: dikkat renginde `■`, `takıldı "…" · lead ilgileniyor`; lider ilgilenirken takılanlara sayılmaz; oturumu biten ya da turu başarısız olan işçi de öyle (`oturum bitti "…" · lead ilgileniyor`, [duran işçi](../concepts/messages-and-board.md#duran-işçi)) |

Satır işin kendisidir: `!` (ya da ajanın işareti), birden çok proje açıkken ve liste daraltılmamışken projesi, kartı ve başlığı (bir değişikliğin kendi başlığı; kartı olmayan bir ajanın adı), sonra istek ya da durum. Bir istek kendi sütununun altına bütün olarak sarılır; kısa bir satırda değişikliğin parçaları alt alta geçer; ajanın durumu kesilir. Sütunundan uzun bir başlık, tek satırlık bir satırın durumundan (ve eldeyse tuşlarından) sonra boş kalan yeri alır. Ajanın adı önizlemededir. `W`'nin sırası bu listenin sırasıdır: izinler, sorular, değişiklikler, yanıtlar, ertelenenler.

Liste ancak eldeki satır ekrandan çıkacaksa kayar: altında bir satırla göründüğü sürece KARAR BEKLEYEN ve grup satırları, üstüne yeni satırlar gelse bile görünür kalır. Listede hiçbir şey yokken liste bunu bir kez, tüm genişlikte söyler ve durum satırı `/ lidere yaz` önerir; hiçbir projenin işçisi yokken (değişikliği bekleyen boştaki bir işçi bile yokken) boş satırlarda imece'nin işareti durur.

## İş bitince {#is-bitince}

Liderin dağıttığı her kart sonuçlanınca ana ekran bunu ilk satırında söyler; satır satır okuyup sizin çıkarmanız gerekmez. İki adımdır.

```
 ◐ EKİP BİTİRDİ  shop-api · 5 kart · 3 tamamlandı · 2 karar sizi bekliyor · $4.12 · 30 dk
```

Ekip bitirdi: bekleyen ya da üzerinde çalışılan kart yok, projeden kimse tur ortasında değil, kalan iş sizin ve satırda sayılı (okunacak bir değişiklik, bir soru, denemesi biten bir kart). KARAR BEKLEYEN hemen altındadır.

```
 ✓ HEPSİ BİTTİ  shop-api · 5 kart · 4 tamamlandı · ✕ 1 başarısız · $4.12 · 38 dk
 Ekibe de size de kalan iş yok. / lidere sıradakini söyler.
```

Hepsi bitti: sizi bekleyen de yok. Başlıkta projenin adının yanına `✓` gelir; açık projelerin hepsi bu durumdaysa terminalin başlığı `✓ imece · hepsi bitti` olur (bir kart başarısızsa `· 1 başarısız`). Başka bir penceredeyken [`:notify`](../reference/commands.md#notify-offbelltextpush) bunu tur başına bir kez söyler: `imece · shop-api: işin hepsi bitti, 4 / 5 tamamlandı, 1 başarısız`.

Başarısız olan ya da geri çekilen bir kart satırı engellemez ve her zaman satırda sayılır, başarısız olan kırmızıyla. Rakamlar turundur: projenin en son açık kartı kalmadığı andan sonra açılan kartlar, onların denemelerinin tuttuğu para (liderin kendi harcaması içinde değildir) ve ilk karttan sonuncunun bitişine kadar geçen süre. Yeni bir kart durumu bitirir ve, hepsi bittikten sonra, sıradaki turu başlatır. Birden çok proje görünürken satır onları birlikte sayar (`2 proje · 9 kart …`) ve ancak kartı olan her ekip bitirdiğinde gelir; `[` `]` tek projeye daraltır. Ekrana sığmayan satır kesilmez, altına devam eder. Bir solonun böyle bir satırı yoktur: ona kart dağıtan yoktur.

Değişikliği evetinizi bekleyen bir karta bağlı bir kart hâlâ bekliyorsa bir şey söylenmez: o kart başlamamıştır, yani ekip bitirmemiştir.

## Bütün ekip durunca {#butun-ekip-durunca}

Bazı duruşları lider çözemez: bütçe bitmiştir, ekip siz son yazdığınızdan beri birbirinin iletileriyle 12 turunu almıştır, Lonca yanıt vermiyordur, liderin kendi oturumu sona ermiştir. Her biri doğrudan size gelir, projesinin tek bir duruşu olarak: ana ekranın ilk satırında ve KARAR BEKLEYEN'de en üstte bir satır olarak, arkasında kimin beklediğiyle ve imece'nin kendi seçenekleri rakamlarla.

```
 ■ DURDU  shop-api · bütçe bitti · arkasında 3 iş bekliyor · 2 dk

 KARAR BEKLEYEN 1 durdu · 1 değişiklik
 durdu
▸■      shop-api                  bütçe bitti · $200.00 / $200.00 · arkasında 3 iş bekliyor
                                  1  $50.00 artır   2  $20.00 artır   3  burada kalsın
```

| Duruş | Ne zaman | Seçenekler |
| --- | --- | --- |
| `bütçe bitti` | bütçe bitmiştir ve ona bekleyen bir şey vardır: teslim edilmemiş ileti, alınmamış bir uyandırma ya da yeniden deneme, otobüsün reddettiği bir ileti | `1` $50 artır, `2` $20 artır, `3` burada kalsın (satır bekleyen her şeyin ardına geçer, üstteki satır kalır); başka bir tutar için `:budget USD` |
| `sen son yazdığından beri ekip iletilerle 12 tur aldı` | bir ajan takım arkadaşlarının iletileriyle 12 turunu almıştır ve ona bekleyen bir şey vardır: bir uyandırma, otobüsün reddettiği bir ileti | `1` sürsün (12 tur daha); ekipten herhangi birine yazmak da aynısını yapar |
| `Lonca yanıt vermiyor` | proje Lonca'dadır ve daemon'ı yanıt vermiyordur, oturumu kapalıdır ya da depo dondurulmuştur (satır nedenini söyler) | `1` şimdi yeniden bak, `2` [Lonca penceresini](../concepts/projects.md#loncada) aç |
| `lead oturumu sona erdi` | liderin oturumu sona ermiştir; işçileri ona ulaşamaz | `1` kaldığı yerden yeniden başlasın |

Önizleme arkasında kimin, neyle beklediğini (`2 ileti ona teslim edilmedi`, `API hatasından sonraki yeniden denemesi`, `lead ile konuşamıyor`) ve her seçeneğin ne yaptığını söyler. `enter` durduran şeyi açar (Lonca penceresini, yoksa liderin işini), `s` onu `3` gibi bekletir. Başlık projeyi `■` ile işaretler ve duruşu `! N karar`'a sayar; sekme takılmış gösterir. Başka bir penceredeyseniz [`:notify`](../reference/commands.md#notify-offbelltextpush) bunu bir kez, yüksek sesle söyler: `imece · shop-api durdu: bütçe bitti`. Günlük, ekibin ne zaman durduğunu ve ne zaman yeniden sürdüğünü yazar (`shop-api: ekip 4 dk sonra sürüyor: bütçede yine yer var`).

imece bunların hiçbirini kendiliğinden kaldırmaz: bütçeyi artırmaz, ekibi sürdürmez, lideri sorulmadan başlatmaz. Duran bir lider duruşun satırıdır; çalışanlar arasında takılmış bir satır değil.

## Liderin satırı

Liderin çalışanlar arasında satırı yoktur; bu yüzden eldeki projenin liderinin listenin altında, hep aynı yerde kendi satırı durur: işareti ve adı, tur ortasındayken o an ne yaptığı, değilse size son sözünün ilk satırı ve ne kadar önce söylendiği (ilk sözünden önce `henüz bir şey söylenmedi`), sağda da ona yazan tuş. `/` [onunla konuşmanızı](conversations.md#bir-agentla-kendi-konuşmanız) altında yazı satırıyla açar. Kimin lideri olduğunu eldeki satır belirler; bir solonun böyle bir satırı yoktur (kendi satırı vardır), kaldırılmış bir projenin de. Sözleri defterin en yeni 2.000 satırında aranır.

## Önizleme

120 kolondan itibaren eldeki satır tuşa basmadan listenin yanında okunur: bir soru bütün olarak, yanıtlarıyla ve soran ajanın şimdiye dek yaptığıyla, ya da bir isteğin sözleri; bir betiğin satırları, numaralı; bir değişikliğin hüküm satırı, nedeni (işçinin sözleri olduğu gibi Markdown olarak, listeleri liste olarak), maliyeti, geçidi, dosyaları ve git'in kendi satırları olmadan yamasının başı; bir ajanın durumu, yapılacakları ve en yeni iz satırları, en yenisi en altta, iz penceresinin izlediği gibi. 120 kolonun altında, editörün yanında, liste tüm genişliği alır ve satırlar her şeyi söyler.

## Küçük bir terminal

imece ssh üzerinden bir telefonda da çalışır. 80 sütunun altında her satır alt alta durur: kart ve başlığı bir satırda, istek, hüküm ya da durum altında, eksiksiz; çalışan bir ajan sığdığı sürece tek satırda kalır. Satırın sonunda o zaman tuş olmaz: tuşlar durum satırındadır, durum satırı da her ekranda nerede olduğunuzu söylemeden önce tuşların hepsine yer açar. Yardımın sözleri ekrana göre sarılır, bir işin baş satırı başlığından sonra ikiye bölünür, bir konuşma bütün genişliği alır.

40 sütunun ya da 12 satırın altında imece yalnızca terminalin boyutunu ve kendi en azını gösterir; `ctrl-c` dışında hiçbir tuş işlemez: gösteremeyen bir ekranda hiçbir şeye karar verilmez. Pencereyi büyütün (ya da yazıyı küçültün), her şey olduğu gibi geri gelir.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | gezer; önizleme izler |
| `gd` `gc` `gw` | karar bekleyen ilk satır · izinlerin ve soruların altındaki ilk değişiklik (hiçbiri yoksa `seni bekleyen bir değişiklik yok`) · çalışan ilk satır |
| `[` `]` | tüm projeler, sonra her biri, dönerek |
| `enter` | açar: değişikliğin diff'i, sorunun konuşması, betiğin baytları, ajanın odağı, projenin lideri. `esc` buraya döner |
| `a` `r` | evet ve hayır, [KARAR BEKLEYEN](waiting.md)'deki gibi; değişiklikte onayla ve geri gönder; soruda `a` yanıt yazar. Eldeki satır bunları sonunda söyler |
| `e` `A` | bir istekte: bu tur evet · bundan sonra evet |
| `s`  `:settle` | sonra: öge KARAR BEKLEYEN'in sonundaki `sonraya` grubuna gider, listede kalırsınız; bir ajanın satırında ya da projenin başlığında: size ihtiyacı olana dek gizlenir ([ekip](salon.md#biteni-kaldırmak); `:unsettle` hepsini geri getirir) |
| `h` `l`  `za` `zM` `zR`  `:fold` `:unfold` | eldeki satırın projesini başlığına katla · aç · değiştir · her projeyi · hiçbirini; yalnız projeler satırların başındayken |
| `/` `i` | eldeki projenin liderine yazar, onunla konuşmanız satırın üstünde |
| `@` | tek bir ajana yazar: önce adı (`tab` tamamlar); ad tamamlanınca onunla konuşmanız satırın üstünde görünür |
| `W` | sıradaki ögeyi açar, listenin sırasıyla |

Diğer tuşlar her yerdeki gibi çalışır; durum satırının tuşları eldeki satırınkilerdir.

Evet aşağı indirir: `a`, `r`, `e` ya da `A` işini yapınca o yere gelen satır elinize geçer, sıradaki tuş onu karara bağlar. Karar asla okumadığınız şeye düşmez: tuşa basmadan önce eldeki satır kendiliğinden gitmişse tuş hiçbir şey yapmaz, bir satır bunu söyler ve yerine gelen satır elinize geçer.

Ekranı başka hiçbir şey paylaşmaz: [günlük](journal.md) (bir başlangıcın notlarının düştüğü yer) bir penceredir, `J` ya da `:journal`; [pano, kod haritası](board-and-code-map.md) ve [zaman şeridi](timeline.md) de adlarıyla açılan pencerelerdir (`:board`, `:code`, `:timeline`); `esc` onları kapatır. Bir [tuş dosyasının](keyboard.md#kendi-tuşlarınız) `[salon]` yeri ana ekranınkidir.
