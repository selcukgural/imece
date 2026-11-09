---
title: Mesajlar ve pano
description: Ajanlar imece'nin bus'ında nasıl konuşur - türlü zarf, on mesaj türü, posta sınırları, numaralı mesajlar ve yanıtlar, size sorular - ve pano kartları nasıl dağıtır.
---

# Mesajlar ve pano

Ajanlar birbirleriyle ve sizinle imece'nin bus'ı üzerinden tek bir araçla, `send` ile konuşur; henüz kimsenin olmayan işi pano üzerinden paylaşır (`post`, `board`, `claim`). Mesajlarla bir kartın hikâyesinde, bir konuşmada ve günlükte karşılaşırsınız; panoyla penceresinde (`:board`).

## Zarf

Her mesaj, türlü bir zarf ile serbest metin bir gövdedir: `send(to, type, body)`, isteğe bağlı `re`. Yönlendirmeyi, uyandırmayı ve maliyeti yalnız zarf sürer. imece gövdeyi asla yorumlamaz: yazıldığı gibi iletilir (çizilirken temizlenir) ve içindeki hiçbir şey imece'ye bir şey yaptıramaz.

| Tür | Anlamı | Alıcıyı uyandırır mı? |
| --- | --- | --- |
| `assign` (görev) | bir görev; gövdenin ilk satırı kartın başlığı olur | evet |
| `ask` (soru) | yanıt gereken bir soru | evet |
| `answer` (cevap) | bir `ask`'ın yanıtı | evet |
| `inform` (bilgi) | bilinmesi iyi olan bir şey, eylem yok | hayır |
| `handoff` (devir) | başka bir ajana devredilen görev | evet |
| `done` (bitti) | görev bitti; bir işçinin `done`'ı bir değişiklik olur | evet |
| `blocked` (takıldı) | devam edilemiyor ve nedeni | evet |
| `progress` (ilerleme) | işin neresinde olunduğu | hayır |
| `notice` (duyuru) | ekibe kısa bir not | hayır |
| `cancel` (iptal) | görev geri çekildi | evet |

`ack` yoktur: uyandıran bir mesaj alıcının sonraki turunu başlatır; alındı bilgisi budur.

Posta ajan başına bekler. Uyandıran bir tür, alıcı boşa çıkar çıkmaz sonraki turunu başlatır; `inform`, `progress` ve `notice`, sonraki turu neyin başlattığına eşlik eder. Alıcı postasını o turun başında, her mesaj numarası ve göndereniyle başlıklanmış olarak okur:

```text
[assign 1] from lead:
#12 parser: tokens
src/parser.rs içindeki tokeniser'ı düzelt; `cargo test -p parser` geçince bitmiştir.
```

## Sınırlar

Sınırlar bir ajanın bir başkasını boğmasını ve bütçeyi tek başına tüketmesini engeller:

| Sınır | Değer |
| --- | --- |
| Gövde | 4000 karakter |
| Ajan başına okunmamış posta | 20 (dolu kutuya `send` reddedilir: "wait for it") |
| Ajan başına mesajla başlayan tur | ekiple son konuşmanızdan beri 12; sonra uyandıran posta siz yazana kadar reddedilir |
| Duran bir ajan | uyandıran posta almaz |
| Bütçe bitince | kimse uyandıran mesaj almaz |

Reddedilen bir `send` gönderene nedenini söyler, model ne yapacağına karar verebilir; hiçbir şey sessizce kuyruğa alınmaz.

## Numaralar ve yanıtlar

Bir ajanın bir başkasına gönderdiği her mesaj bir numara alır; bütün dünya boyunca 1'den başlayarak sırayla, çalıştırmalar arasında saklanır. `send` numarayla yanıt verir ("delivered as message 7"). Bir yanıt, yanıtladığı mesajı `re` ile adlandırabilir; alıcı bunu başlıkta görür:

```text
[answer 8, re 7] from claude-1:
Yenisi.
```

`re` denetlenir, varsayılan ret: günlükte var olan, gönderene gönderilmiş ve yanıtın gittiği kişinin yazdığı bir mesajın numarası olmalıdır. Başka her şey nedeniyle reddedilir ve numara almaz. `re` bağlar; asla yönlendirmez, uyandırmaz, maliyet çıkarmaz.

Ekranda bir yanıt türünden sonra `↩` taşır: günlük satırında, kartın hikâyesinde ve konuşmada; konuşmada yanıtladığı mesajın ilk satırı altında soluk gösterilir (`↩ 7 yanıtı: Hangi parser?`). `%` bir yanıttan yanıtladığına, bir mesajdan ilk yanıtına atlar ([konuşmalar](../screen/conversations.md)).

## Size mesajlar

Bus size `you` olarak ulaşır: `send(to: "you", type, body)`.

- Size bir `ask`, KARAR BEKLEYEN'de bir soru olur (`?`, "claude-1 sana soruyor: …"). Soru açıkken ajan o satırın kendisidir ve ÇALIŞAN altında listelenmez. Aynı ajandan daha yeni bir soru eskisinin yerini alır. Ajana yazarak yanıt verirsiniz (`@claude-1 …`, ya da öğede `a`/`r`, ikisi de yazma satırını açar) ve bu onun sonraki turunu başlatır.
- Size `inform`, `progress`, `notice`, `done`, `blocked` ve `answer` yalnız bilgilendirir: konuşmada ve hikâyede bir satır, öğe yok, kimse uyanmaz. Bir işçinin size `done`'ı değişikliğini her `done` gibi gösterir.
- Size `assign`, `handoff` ve `cancel` reddedilir.
- Size mesajlar numara ve `re` almaz; sizin mesajlarınızın da numarası yoktur. Size giden hiçbir şey posta sınırlarına ya da bütçeye sayılmaz.

İstemler ne zaman sorulacağını söyler: lider görev ortasında size sorabilir; bir işçi önce liderine sorar ve size yalnız sizin karar verebileceğinizi gönderir; bir yardımcı yalnız hiçbir ekip arkadaşının yanıtlayamayacağını. Daha fazlası [onaylar ve sorular](../concepts/approvals-and-questions.md) sayfasında.

## Sizi bekleyen ajanın postası da bekler

Yalnız sizin çözebileceğiniz bir şeyi açık olan bir ajan, bir takım arkadaşı yazdı diye devam etmez: KARAR BEKLEYEN'deki bir sorusu, onayınızı bekleyen bir araç çağrısı, bir API hatası, bir kurulum betiği, hassas bir değişiklik ya da bir araştırma raporu. Postası kuyrukta kalır, tur başlatmaz ve ona pano kartı önerilmez. Gönderen `held as message 7; …` yanıtını alır; ajanın satırı ve `:why` kaç mesajın beklediğini söyler: `✉ 2 kuyrukta`.

Ajana yazdığınızda, sözünüz beklemeyi bitiriyorsa posta aynı turda, sözlerinizden sonra gider. Başka bir yolla çözerseniz posta kendi turunu başlatır. Bir `cancel` asla tutulmaz: hemen tur başlatır ve bekleyen posta sırasıyla onunla gider. Posta sınırları tutulan postayı da diğerleri gibi sayar.

KARAR BEKLEYEN'de iki şey hiçbir şeyi tutmaz: okunmayı bekleyen bir yanıt ve bir işçinin değişikliği. Değişikliği bekleyen işçi işini bitirmiştir, devam etmek için yanıta ihtiyacı yoktur; siz incelerken sonraki kartı alabilir.

## Bus'ta siz

Komut satırındaki düz metin eldeki projenin liderine gider; `@ad mesaj` tek bir ajana. Boşta bir işçiye mesaj bir kart açar; değişikliği onu kapatır ([kartın denemeleri](#kartın-denemeleri)). Mesajlarınızın numarası yoktur. Bir ajanın size yanıtı KARAR BEKLEYEN'de bekler ("claude-1 yanıt verdi: …"); açarsınız (`enter`), kapatırsınız (`r`) ya da doğrudan yanıt yazarsınız.

## Pano

Pano henüz kimsenin olmayan iş içindir. Belli bir ekip arkadaşının yapacağı iş için lider `assign` gönderir.

| Araç | Kim | Ne |
| --- | --- | --- |
| `post(title, body)` | herkes | projenin panosuna bir kart asar; `after` beklediği kartları adlandırır |
| `board()` | herkes | projenin asılmış ya da görev olarak verilmiş her kartını listeler, açıkları ayrı, her biri neyi beklediğiyle; `ready: true` yalnız bir işçinin şimdi alabileceği açık kartları listeler |
| `claim(card)` | yalnız işçiler | açık bir kartı alır; yanıt, kartın görevidir |

- Açık bir kart boşta bir işçiye bir anda birine, kendi turu olarak, yalnız başlığıyla **teklif edilir** (gövde claim ile gelir). O işçinin turu kartı almadan bitince bir sonraki işçiye gider; böylece bir kart her işçiyi en çok bir kez uyandırır, asla yayın değil. Bir teklif mesajla başlayan tur sayılır.
- İşçi başına bir anda bir kart. İşçi `done`'ı kartı asana gönderir.
- Yeniden başlatmadan önce asılmış bir kart uyuyan işçiyi uyandırmaz; bu çalıştırmada asılan uyandırır.

Sınırlar: başlık en çok 120 karakterlik tek satır (ekran işçinin yanında ilk 20 karakterini gösterir, bu yüzden onu diğer kartlardan ayıranla başlamalı: bir issue numarası, bir dosya, bir sembol); proje başına 20 açık kart; siz yeniden konuşana dek ajan başına 10 asma.

## Kartın sahip olduğu yollar

Bir `assign` ya da `post`, kartının sahip olduğu yolları adlandırır: `paths`, depoya göre en çok 20 git pathspec'i (`src/parser.rs`, bir dizin `docs`, bir desen `tests/*_test.rs`; `*` her şeyin, `/` dahil, yerine geçer). Kart onları saklar, çalıştırmalar arasında da; işçinin görevi `This card owns src/parser.rs, docs (shared): stay in them; what else needs changing goes in your done, or ask for the card to be widened` ile biter ve kartın ekranı onları listeler (`sahibi`). `paths`'siz bir kartın sınırı yoktur.

- **Bir projenin iki açık kartı aynı yola sahip olamaz.** Yollarına açık bir kartın (bekleyen ya da üzerinde çalışılan) zaten sahip olduğu bir `assign` ya da `post`, hangi kartın neye sahip olduğu söylenerek reddedilir: `#3 Fix the parser owns "src/parser.rs", which "src" names too; give this card other paths, or end one with the word shared (src shared)`. İki karttan birinin ötekini ters yönde adlandırdığı yollar da çakışır (`src` ile `src/a.rs`, `src/*` ile `src/a.rs`). Alıcı işçinin şu an tuttuğu kart sayılmaz: sonraki kartı aynı dosyalarda sürebilir. Bitmiş bir kart artık hiçbir şeye sahip değildir.
- **Her kartın dokunduğu bir yol `shared`'dır.** `CHANGELOG.md shared` yolu sahiplenmeden adlandırır: başka bir kart da adlandırabilir, işçi de yazabilir. Lidere, çoğu görevin dokunduğu bir dosyayı tek bir karta vermesi ve ötekilere o dosyanın ne alması gerektiğini `done`'larında söylemeleri gerektiğini anlatması söylenir.
- **Yollarının dışına yazan işçi sapar ve bu ona söylenir.** Oturumunun `Edit`, `Write` ve `MultiEdit` çağrıları dosyayı gösterir; kartın yollarının dışındaki bir dosyaya ilk yazışta kartın hikâyesi `değiştirdi: src/other.rs; kartın yollarının dışında`, defter `claude-1 şunu değiştirdi: src/other.rs; #3 kartının dışında` der ve işçinin sonraki istemi `you changed src/other.rs, which #3 does not own (it owns src/parser.rs); say so in your done, or ask lead to widen the card` ile başlar. Kart başına dosya başına bir kez; asla durdurulmaz, asla reddedilmez: kişi değişikliği, kartın dışındaki dosyalar yamanın üstünde sayılmış olarak okur ([değişiklikler](../screen/changes.md#yamanın-üstündekiler)).

Bu, sembol lease'lerinin ucuz vekilidir (bkz. [kod haritası](../screen/board-and-code-map.md)): yol, liderin planlarken adlandırabildiği ve bir değişikliğin dosyalar okunmadan karşılaştırılabildiği şeydir. Lease'ler ancak yollar yetmezse gelir.

## Kartın istediği denetimler

Lonca'daki bir projede bir `assign`, bir `post` ya da bir planın kartı denetimler de adlandırabilir: `checks`, deponun Lonca politikasının tanımladığı en çok 32 denetimin adı (`lonca.toml`'daki `verification.checks`, `api` ya da `e2e` gibi). Lonca'nın kendi denetimleri (`build`, `tests`) zaten çalışır ve burada ad değildir. Kart onları sıralı olarak saklar, çalıştırmalar arasında da; işçinin görevi `This card asks for Lonca's checks api, e2e: once the person says yes, Lonca's merge queue runs them on your change too, and a failure comes back to you` ile biter ve kartın ekranı onları listeler (`denetim`).

Siz karar vermeden onları hiçbir şey çalıştırmaz. Kartın değişikliği bunları karar satırında söyler (`evetinden sonra Lonca şunları çalıştırır: build, tests, api · kart ayrıca şunları da istiyor: api`) ve `a`'nız onları Lonca'dan ister (`lonca approve 5 --also=api`): onayı yalnızca bir kişi imzalar, bu yüzden hiçbir ajan Lonca'dan kendi başına denetim isteyemez. Lonca onları değişikliğin sonraki sürümleri için de saklar.

Lonca'nın `status`'u yürürlükteki politikanın tanımladığı denetimleri söylediğinde (`policy_checks`, 0.2.0'dan yeni bir Lonca), politikada olmayan bir ad yazıldığı yerde reddedilir: liderin `assign`, `post` ya da plan kartı `"apii" is not a check of Lonca's policy; it has api, e2e. Did you mean api?` alır ve aynı turda düzeltir, hiçbir şey gönderilmez. Yeni bir politika denetimlerinden birini düşürmeden önce yazılmış bir kart bunu değişikliğinin karar satırında söyler, `Lonca'nın politikasında artık smoke yok: :checks kartın istediklerini değiştirir`, ve değişiklik hazır sayılmaz. Daha eski bir daemon ya da Lonca'nın okuyamadığı bir politikada ad evet anında değerlendirilir. Orada politikada olmayan bir ad evet'i durdurur: `öneri hakkındaki kararın Lonca'ya ulaşmadı: Lonca'nın politikasında api denetimi yok · :checks ADLAR kartın istediklerini değiştirir` ve değişiklik beklemeye devam eder; kartın denetimlerini değiştirin (aşağıda), denetimi politikaya ekleyin (`lonca policy set`) ya da değişikliği reddedin. Lonca dışında onları hiçbir şey çalıştırmaz, bu yüzden `checks` reddedilir (`checks run in Lonca's merge queue, and this project is not on Lonca: leave checks out, and say in the body what to run`); orada çalışan [deponun denetimi](changes.md#deponun-kontrolü)'dir. Demonun ikinci projesi Lonca'dadır ve her üçüncü kartı `api` ister.

Bir kartın denetimleri kart yapıldıktan sonra da değişebilir, iki elden; ikisi de günlükte ve kartın hikâyesinde görünür (`lead, #3 kartının denetimlerini değiştirdi: api, e2e`, `#3 kartının denetimlerini değiştirdin: hiçbiri`):

- **Lider**, kartı yeniden verirken (`card` ile bir `assign`): orada verilen `checks` kartınkilerin yerine geçer, `[]` hiçbiri demektir; yazılmazsa kart kendi denetimlerini korur. Yeni işçinin görevi onları adlandırır.
- **Siz**, evet anında: elinizdeki değişikliğin üstünde `:checks api e2e` (aralarında virgül ya da boşluk) evetinizin Lonca'dan isteyeceklerini belirler, `:checks none` hepsini kaldırır, yalın `:checks` onları söyler. Böylece liderin bir yazım hatası iyi bir değişikliği reddetmenize yol açmaz: adı düzeltin ya da çıkarın ve evet deyin. Lonca politikanın denetimlerini söylediğinde `:checks ` sonrasındaki menü onları önerir. Lonca dışında, bir kart için yapılmamış bir değişiklikte, Lonca'nın almayacağı bir adda ve politikada olmayan bir adda reddedilir (`Lonca'nın politikasında apii yok; olanlar: api, e2e · api mi demek istedin?`).

Karar satırı hemen buna uyar. imece kendi başına hiçbir denetim eklemez ya da düşürmez, politikada artık olmayanı bile: bunu siz ya da lider yaparsınız. Demonun lideri, Lonca'daki projenin hiçbir şey istemeyen bir kartını yeniden verirken ona `api` istetir. Demonun politikası `api` ve `e2e`'yi tanımlar; yedinci kartı ve ondan sonra her dokuzuncusu politikanın düşürdüğü `smoke`'u da ister ve değişikliğine evet, `:checks api` `smoke`'u kaldırana kadar reddedilir.

## Kartın denemeleri

Bir kart işçisinin turlarını değil, değişikliğini izler. İşçinin `done`'ına kadar üzerinde çalışılır; değişikliği (`Δ`) KARAR BEKLEYEN'deyken incelemenizi bekler; değişikliği geri gönderdiğinizde (bir red, notlar, bir çakışma) yeniden çalışmaya döner ve işçiye yazdıklarınız o kartla devam eder, bir tur daha; değişiklik tamamen uygulandığında ya da `done` alınacak bir şey bırakmadığında biter. Lonca'da, onayınızdan sonra merge queue'da doğrulanıyor olur, trunk'a girince biter, kuyruk ya da bir inceleyen hayır derse yeniden çalışmaya döner.

- **Bir deneme.** Kart kaç kez alındığını sayar: onu açan `assign` ilki, panodan bir `claim` bir deneme; lider bir kartı `assign` ve `card: 3` ile yeniden verebilir (başarısız olmuş, işçisi durmuş ya da panoda bekleyen bir kartı), bir sonraki denemesi olarak. Kart başlığını ve yollarını korur; yeni işçinin postası `[assign 14 · #3, try 2]` başlığıyla gelir, kartı elinde tutana `Card #3 (…) went to claude-2. Stop work on it; a done from you for it closes nothing now.` denir. İncelemede, kuyrukta ya da bitmiş bir kart yeniden verilemez: `#3's change waits for the person; it comes back if they send it back`.
- **Geç bir `done`.** Kartı yeniden verilmeden önce başlamış bir turdan gelen `done` hiçbir şeyi kapatmaz ve değişiklik göstermez: günlük `claude-1: #3 için geç bir done; kart başka bir denemede, hiçbir şeyi kapatmıyor` der, liderin postası `[done 12 · late: #3 is claude-2's now, it closes nothing]` başlığıyla gelir. imece bir `done`'ın hangi turdan geldiğini bilir; ajan denemesini hiç yazmaz.
- **Başarısız bir kart.** Bir görevi istendiği gibi yapamayan işçi `done`'ını `outcome: "failed"` ve nedeniyle gönderir. Kart başarısız olur (panoda `✕`, ekranında BAŞARISIZ), değişiklik gösterilmez, liderin postası `[done 12 · #3 failed]` başlığıyla gelir; lidere kartı neyin farklı yapılacağıyla yeniden vermesi söylenir.

- **Geri çekilen bir kart.** Lider artık kimsenin yapmayacağı bir kartı `withdraw(card, why)` ile geri çeker: panodaki bir kartı, kendi verdiği bir kartı (üzerinde çalışılan ya da başka kartları bekleyen) ya da başarısız olmuş bir kartı. Kart biter: bitenlerin yanında `–` ile durur, ekranında GERİ ÇEKİLDİ yazar, günlük `lead geri çekti: #3 Legacy parser: covered by #5` der. Onu üzerinde çalışırken ya da beklerken elinde tutan şunu duyar: `Card #3 (…) was withdrawn by lead: stop work on it; a done from you for it closes nothing now.`, onun için bekleyen görevini de kaybeder; sonraki `done`'ı hiçbir şeyi kapatmaz ve değişiklik göstermez (`[done 12 · late: #3 was withdrawn, it closes nothing]`). Yanıt, onu bekleyen kartları, başkaları aracılığıyla bekleyenleri de, adlandırır: `#3 is withdrawn; #6, #7 wait on it and never begin: withdraw them too, or tell the person`. Değişikliği sizi bekleyen, merge queue'daki ya da bitmiş bir kart geri çekilemez: bir değişikliği siz kendiniz reddedersiniz.
- **Bir devre kesici.** Bir kartın üç denemesi, değişikliğinin üç turu vardır. Üçüncü denemesinde başarısız olmuş bir kartı yeniden veren liderin `assign`'ı reddedilir (`#3 failed 3 tries: the person decides whether it gets another`), kart KARAR BEKLEYEN'de `⊘` ile sizi bekler: `3 denemede başarısız: bir deneme daha, ya da kart başarısız sayılır`. Lonca'da kuyruğun (ya da oradaki bir gözden geçirenin) üçüncü kez geri gönderdiği değişiklik işçisini uyandırmaz: aynı biçimde sizi bekler, `değişikliği 3 kez geri döndü: bir tur daha, ya da kart başarısız sayılır`, bu arada işçinin postasını tutar. `a` bir deneme ya da bir tur daha verir (lider ya da işçi bunu duyar); `r` kartı kalıcı olarak başarısız sayar: kimse onu yeniden vermez, işçisi bırakır, lideri haber alır. Sizin kendi reddiniz, notlarınız ve geri gönderdiğiniz çakışma her zaman geçer: zaten siz karar veriyorsunuz. `board` aracı böyle bir kart için `its tries are out` ya da `the person failed it for good` der.

Demo dördünü de gösterir: her yedinci ilk deneme başarısız olur ve lideri başarısız kartı başka bir işçiye verir; böyle bir kart kendiliğinden verilen her denemede yine başarısız olur, dördüncüsü sizi bekler, sizin verdiğiniz deneme geçer. Demonun kuyruğunda testleri başarısız olan bir değişiklik, siz bir tur daha verene kadar her turda yine başarısız olur. Arada bir lider uzun süren bir kartı işçisinden alır; o işçinin turu geç bir `done` ile biter.

## Başka kartları bekleyen kartlar

Önce başka bir kartın işine ihtiyacı olan kart bunu söyler: bir `assign` ya da `post` üzerindeki `after: [2, 5]` beklediği kartları adlandırır (en çok 8, aynı projenin daha eski kartları). Her biri bitene dek:

- **Asılmış kart önerilmez ve alınamaz.** `claim` `#6 waits on #2, #5; pick a card that is ready (board with ready: true)` diye yanıtlar, `board` aracı onu `#6 Tests (posted by lead; waits on #2, #5)` diye listeler.
- **Bir `assign` kartında bekler.** Kart hemen açılır, işçinindir, henüz denenmemiştir; lider `held as message 14 on #6: it waits on #2, #5, and claude-1 gets it once they are done` yanıtını alır, günlük `claude-1 alacak: #6 Tests, #2, #5 bitince` der. İşçinin öteki postası onun yanından geçer. Sonuncusu da bitince görev her görev gibi gider (`[assign 14 · #6, try 1]`) ve günlük `claude-1 başladı: #6 Tests, beklediği bitti` der. Böylece lider bütün bir sırayı tek seferde verebilir. Beklerken yeniden verilen bir kart (`card: 6`) yeni işçi için tutulur, eskisinin bekleyen görevi geri çekilir.
- **Başarısız bir kart onu bekleyenleri**, onları bekleyenleri de, yeniden verilip bitene dek **tutar**: pano penceresi kartı `✕ #2 başarısız` diye işaretler, `board` aracı `waits on #2, #5, and #2 failed` der (#1 zincirin daha gerisindeyse `and #1 failed`), lider `#6, #7 wait on #2, which failed; they begin once it is done: hand it again, or tell the person.` notunu alır. Sizin kalıcı olarak başarısız saydığınız ya da liderin geri çektiği bir kart onları sonsuza dek tutar: pano `✕ #2 geri çekildi` der, lidere onları geri çekmesi ya da size söylemesi söylenir, ona yeni bir kart bekletilemez.

Pano penceresi bekleyen bir kartı `⧗ #2 bitince` (ya da `✕ #2 başarısız`) diye, bir işçi için tutulanı `→ claude-1` diye işaretler; orada `r` yalnız bir işçinin şimdi alabileceği kartları gösterir (`yalnız hazırlar: 3 bekleyen gizli`), yine `r` hepsini. Bir kartın ekranı beklediklerini listeler: her biri `✓` bitti, `✕` başarısız ya da `⧗` henüz değil. Bir planda kart, aynı planın beklediği kartlarını yerleriyle adlandırır (`after: [1]`) ve planın penceresi `yukarıdaki 1 bitince` der.

Demo bunu gösterir: kişinin lidere verdiği bir iş, testlerini ve belgelerini işin kendisinden sonraya asar; liderin astığı her üç karttan biri henüz bitmemiş en yeni kartı bekler; arada bir lider bekleyen bir kartı erkenden verir, kart başlayabilene dek işçisi için tutulur. Lideri her 300 tikte kimsenin almadığı en yeni kartı geri çeker, hiç bitmeyecek bir kartı bekleyen her kartı da geri çeker.

## Plan

İlk işçi ucuzdur ve işi siz istediniz; lider ikincisini almadan önce ne yapmayı düşündüğünü görürsünüz. `plan(cards, cost)` çağırır: görev vermeyi ya da panoya asmayı düşündüğü kartlar, her biri başlık, kademe, sahip olduğu yollar ve gövdeyle, ve hepsinin dolar olarak beklediği maliyeti. Plan KARAR BEKLEYEN'e tek bir `▤` ögesi olarak gelir (`lead: 3 kart planladı, $1.20; enter okur`), defter `lead 3 kart planladı, $1.20; onayınızı bekliyor` der ve lidere turunu bitirmesi söylenir; plan beklerken liderin postası da onunla bekler ([posta bekler](#sizi-bekleyen-ajanın-postası-da-bekler)) ve `hire`, `the person is reading your plan; end your turn, their yes starts your next` yanıtını verir.

- `enter` planı tamamıyla açar: her kartın numarası, başlığı ve kademesi, sahip olduğu yollar, istediği denetimler, planın beklediği kartları, gövdesi; beklenen maliyet, bütçede kalanın yanında. Her sözcük liderindir, temizlenmiş, asla Markdown olarak okunmaz.
- `a` evet der: lider sonraki istemiyle (turu bitmişse hemen) `The person said yes to your plan: hire and assign as planned` duyar ve sonraki planına dek işe alabilir; yeni bir plan evet'i geri alır.
- `r` hayır der: plan düşer ve lider `The person said no to your plan: ask them what they want, or make another plan` duyar.
- `n` onun yerine lidere yazar: gönderdiğiniz planı yanıtsız kapatır ve yeni bir plan onayınızı alana dek `hire` bunu söyler (`the person answered your plan in words, not with a yes: make a new plan from what they said`). Lidere söylediğiniz her söz aynı şeyi yapar.

Yeni bir plan bekleyenin yerini alır. Plan, kartların denetleneceği gibi denetlenir: tek satırlık başlık, gövde, kademe, yollar ve planın iki kartının aynı yola sahip olmaması (`cards 1 and 3 both own a path: "src" and "src/a.rs"`); zarf yürütür, gövde asla yorumlanmaz. Bir ile yirmi kart. İlk işçi plan gerektirmez, işçisi kalmamış bir liderin işe alması da; solo asla planlamaz. `:plan off` liderine güvenen bir proje için kapıyı kapatır (depoda saklanır, varsayılan açık); o zaman gönderilen bir plan saklanmaz ve lidere uygun gördüğü gibi işe alması söylenir. Demonun lideri de ikinci işçisinden önce planlar; oradaki evet de canlıdaki gibi `a` ile gelir.

## Kartlar ve hikâyeleri

Bir kart, hikâyesi olan bir iş birimidir: `niyet`, `varsayım`, `karar`, `konuşma`, `kanıt`, `değişiklik`, `sen` (sizin söylediğiniz ya da karar verdiğiniz). Pano penceresi kartları sütunlarda gösterir (YAPILACAK, ÜZERİNDE, İNCELENECEK, DOĞRULANIYOR, BİTTİ; başarısız bir kart bitenlerle durur, `✕` ile); `#N` ya da `#kelime` bir kartın kendi ekranını açar: kim üzerinde çalıştı, hikâyesi, kendi gelen kutusu ve günlük satırları ([kartlar](../screen/cards.md)).

## Uyanış özeti

Son çalıştırmadan beri uyuyan bir ajan, sizin mesajınızla, bir ekip arkadaşının uyandıran mesajıyla ya da bu çalıştırmada asılan bir kartla uyanır. Oturumu belleği yerinde devam etmiyorsa ([oturumlar](../concepts/sessions.md)), imece yeni oturumu ilk istemden önce özetler: kart ve son 12 hikâye satırı, yarım kalan istem (yeniden çalıştırılmaz), kaç mesajın beklediği, worktree'nin commit edilmemiş değişiklikleri, onun için tutulan bir görev ve neyi beklediği (`#6 Tests is yours once what it waits on is done (#2); its assign comes then.`), liderin başka kartları bekleyen kartları ve onları neyin tuttuğu, ve panonun açık kartları. Her satır `clean`'den geçer; bir satır en çok 300 karakter, özet 6000.

## Ayrıca bakınız

- [Mesaj türleri](../reference/message-types.md): başvuru tablosu.
- [Konuşmalar](../screen/conversations.md): bir alışverişi okumak, `[ ]` söz sıraları, `%`.
- [Pano ve kod haritası](../screen/board-and-code-map.md).
- [Kartlar](../screen/cards.md).
- [Değişiklikler](../concepts/changes.md): bir işçinin `done`'ı neye dönüşür.
