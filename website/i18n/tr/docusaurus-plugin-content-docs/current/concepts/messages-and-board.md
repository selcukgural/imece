---
title: Mesajlar ve pano
description: Ajanlar imece'nin bus'ında nasıl konuşur - türlü zarf, on mesaj türü, posta sınırları, numaralı mesajlar ve yanıtlar, size sorular - ve pano kartları nasıl dağıtır.
---

# Mesajlar ve pano

Ajanlar birbirleriyle ve sizinle imece'nin bus'ı üzerinden tek bir araçla, `send` ile konuşur; henüz kimsenin olmayan işi pano üzerinden paylaşır (`post`, `board`, `claim`). Mesajlarla bir kartın hikâyesinde, bir konuşmada ve günlükte karşılaşırsınız; panoyla ikinci görünümde (`2`).

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

- Size bir `ask`, SENİ BEKLEYEN panelinde bir soru olur (`?`, "claude-1 sana soruyor: …"). Soru açıkken ve ajan boştayken salon satırı `? → you` okunur. Aynı ajandan daha yeni bir soru eskisinin yerini alır. Ajana yazarak yanıt verirsiniz (`@claude-1 …`, ya da öğede `a`/`r`, ikisi de yazma satırını açar) ve bu onun sonraki turunu başlatır.
- Size `inform`, `progress`, `notice`, `done`, `blocked` ve `answer` yalnız bilgilendirir: konuşmada ve hikâyede bir satır, öğe yok, kimse uyanmaz. Bir işçinin size `done`'ı değişikliğini her `done` gibi gösterir.
- Size `assign`, `handoff` ve `cancel` reddedilir.
- Size mesajlar numara ve `re` almaz; sizin mesajlarınızın da numarası yoktur. Size giden hiçbir şey posta sınırlarına ya da bütçeye sayılmaz.

İstemler ne zaman sorulacağını söyler: lider görev ortasında size sorabilir; bir işçi önce liderine sorar ve size yalnız sizin karar verebileceğinizi gönderir; bir yardımcı yalnız hiçbir ekip arkadaşının yanıtlayamayacağını. Daha fazlası [onaylar ve sorular](../concepts/approvals-and-questions.md) sayfasında.

## Sizi bekleyen ajanın postası da bekler

Yalnız sizin çözebileceğiniz bir şeyi açık olan bir ajan, bir takım arkadaşı yazdı diye devam etmez: SENİ BEKLEYEN'deki bir sorusu, onayınızı bekleyen bir araç çağrısı, bir API hatası, bir kurulum betiği, hassas bir değişiklik ya da bir araştırma raporu. Postası kuyrukta kalır, tur başlatmaz ve ona pano kartı önerilmez. Gönderen `held as message 7; …` yanıtını alır; ajanın satırı ve `:why` kaç mesajın beklediğini söyler: `✉ 2 bekliyor`.

Ajana yazdığınızda, sözünüz beklemeyi bitiriyorsa posta aynı turda, sözlerinizden sonra gider. Başka bir yolla çözerseniz posta kendi turunu başlatır. Bir `cancel` asla tutulmaz: hemen tur başlatır ve bekleyen posta sırasıyla onunla gider. Posta sınırları tutulan postayı da diğerleri gibi sayar.

SENİ BEKLEYEN'de iki şey hiçbir şeyi tutmaz: okunmayı bekleyen bir yanıt ve bir işçinin değişikliği. Değişikliği bekleyen işçi işini bitirmiştir, devam etmek için yanıta ihtiyacı yoktur; siz incelerken sonraki kartı alabilir.

## Bus'ta siz

Komut satırındaki düz metin seçili projenin liderine gider; `@ad mesaj` tek bir ajana. Boşta bir işçiye mesaj bir kart açar; yanıtı onu kapatır. Mesajlarınızın numarası yoktur. Bir ajanın size yanıtı SENİ BEKLEYEN'de bekler ("claude-1 yanıt verdi: …"); açarsınız (`enter`), kapatırsınız (`r`) ya da doğrudan yanıt yazarsınız.

## Pano

Pano henüz kimsenin olmayan iş içindir. Belli bir ekip arkadaşının yapacağı iş için lider `assign` gönderir.

| Araç | Kim | Ne |
| --- | --- | --- |
| `post(title, body)` | herkes | projenin panosuna bir kart asar |
| `board()` | herkes | açık kartları ve diğerlerini kimin aldığını listeler |
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

## Kartlar ve hikâyeleri

Bir kart, hikâyesi olan bir iş birimidir: `niyet`, `varsayım`, `karar`, `konuşma`, `kanıt`, `değişiklik`, `sen` (sizin söylediğiniz ya da karar verdiğiniz). Pano görünümü kartları sütunlarda gösterir (BEKLİYOR, ÜZERİNDE, DOĞRULANIYOR, BİTTİ); `#N` ya da `#kelime` bir kartın kendi ekranını açar: kim üzerinde çalıştı, hikâyesi, kendi gelen kutusu ve günlük satırları ([kartlar](../screen/cards.md)).

## Uyanış özeti

Son çalıştırmadan beri uyuyan bir ajan, sizin mesajınızla, bir ekip arkadaşının uyandıran mesajıyla ya da bu çalıştırmada asılan bir kartla uyanır. Oturumu belleği yerinde devam etmiyorsa ([oturumlar](../concepts/sessions.md)), imece yeni oturumu ilk istemden önce özetler: kart ve son 12 hikâye satırı, yarım kalan istem (yeniden çalıştırılmaz), kaç mesajın beklediği, worktree'nin commit edilmemiş değişiklikleri ve panonun açık kartları. Her satır `clean`'den geçer; bir satır en çok 300 karakter, özet 6000.

## Ayrıca bakınız

- [Mesaj türleri](../reference/message-types.md): başvuru tablosu.
- [Konuşmalar](../screen/conversations.md): bir alışverişi okumak, `[ ]` söz sıraları, `%`.
- [Pano ve kod haritası](../screen/board-and-code-map.md).
- [Kartlar](../screen/cards.md).
- [Değişiklikler](../concepts/changes.md): bir işçinin `done`'ı neye dönüşür.
