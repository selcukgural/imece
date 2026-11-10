---
title: "Demo"
description: "`imece --demo` bütün ekranı sahte ajanlarla, maliyetsiz, canlının aynasında çalıştırır."
---

# Demo

`imece --demo` ekranı gerçek ajanlar yerine tohumlu bir simülasyonla çalıştırır: liderleri ve işçileriyle projeler; kart asar, alır ve dağıtırlar, bus'ta konuşurlar, size soru sorarlar, onayınızı beklerler, yardımcı tutarlar ve sizi bekleyen değişikliklerle bitirirler. Claude Code başlatılmaz ve hiçbir şey para harcamaz. Demoyla, bir kuruş harcamadan önce ekranı iş başında görmek ya da tuşları öğrenmek istediğinizde karşılaşırsınız.

```sh
imece --demo
imece --demo --projects 3 --agents 30 --seed 12
imece --demo --lang tr --theme nord
```

## Bayraklar

| Bayrak | Aralık (varsayılan) | Ne yapar |
| --- | --- | --- |
| `--demo` | | canlı ajanlar yerine sahteleri |
| `--projects N` | 1-4 (2) | aynı anda çalışılan projeler, her birinin kendi lideri |
| `--agents N` | 1-200 (24) | liderlerin yanındaki işçiler, projelere dağılmış |
| `--researchers N` | 1-32 (4) | aynı anda çalışabilecek araştırmacılar, bütün projelerin paylaştığı bir havuz (ön izleme: araştırmacılar canlıda yapılmadı) |
| `--seed N` | (7) | aynı senaryoyu yeniden oynatır; başka tohum, başka hikâye |
| `--lang en\|tr` | (en ya da `IMECE_LANG`) | ekranın dili; `L` çalışırken değiştirir |
| `--theme AD\|DOSYA` | (terminal) | bir tema, canlının aldığı gibi |
| `--awake` | kapalı | sahte bir ajan turdayken makinenin uykusunu tutar, canlıdaki gibi |

`--projects`, `--researchers` ve `--seed` yalnız demoya aittir: canlı imece bunları `--projects is for --demo` sözüyle reddeder. Demonun saati bugünün tarihinde 14:00'te başlar. Demo deponun ekran ayarlarını (tema, notify, motion) canlıyla paylaşır ama başka hiçbir şey yazmaz: proje yok, günlük yok, kese yok.

## Hız

Canlı ajanlar gerçek zamanda çalışır; simülasyonun tutabileceğiniz bir saati vardır.

| Tuş | `:` adı | Ne yapar |
| --- | --- | --- |
| `space` | `:pause` | simülasyonu durdur; yine: devam. Başlık `‖ DURDU` gösterir |
| `+` | `:faster` | hızlandır |
| `-` | `:slower` | yavaşlat |

`:` menüsü komutun ardında hızı (`faster (×2)`) ve durmuş olup olmadığını gösterir. Canlıda yazılınca bu üçü `:pause demo için: canlı agent'lar gerçek zamanda çalışır` der; canlı `:` menüsünde yer almazlar.

## Ne gösterir

Simülasyon canlının olaylarını canlının kullandığı aynı model çağrılarıyla üretir; ekran gerçek ekrandır:

- **Liderin işi**: kart asar (başlangıçta da), isteğinizi astığı kartlarla yanıtlar (KARAR BEKLEYEN'de bir yanıt), bir kartı boştaki bir işçiye verir ya da panodan almasına bırakır, iki kart bekler ve kimse boşta değilse işçi tutar (en çok altı), katmanlı bir tutma zaman zaman varsayılan modele düşer.
- **Turlar**: lider kendiliğinden on iki kart dağıtır, sonra hepsi sonuçlanana ve sizi bekleyen kalmayana kadar yenisini asmaz; böylece ana ekran önce `◐ EKİP BİTİRDİ`, son evetinizden sonra da `✓ HEPSİ BİTTİ` der ([iş bitince](../screen/home.md#is-bitince)); iki dakika sonra sıradaki tur başlar. Liderden istediğiniz, ne zaman isterseniz asılır. Kendi hâline bırakılan bir demo, değişiklikleri sizi beklerken durur; oturumu duran bir işçi de siz ona yazana kadar turunu tutar.
- **İşçiler**: bus'ta tipli mesajlar (`assign`, `ask`, `answer`, `progress`, `done`), varsaydıklarını size `inform` olarak söylemeleri, dokunduklarının kartın hikâyesinde görünmesi, testler için tutulan ve `done` gönderip ayrılan bir yardımcı. İki kart arasındaki bir işçiye `@claude-2 …` yazmanız kart açmaz: işçi yanıtlar, her ikinci yanıt bir değişiklikle gelir ve o değişiklik başlığı sizin satırınız olan kartını açar; diğerleri gösterecek bir şey bırakmaz ve satır orada biter.
- **Onaylar (`⚑`)**: bir işçinin oturumu projenin hassas adımlarından biri için onayınızı ister; öğe siz karar verene (`a` izin, `r` ret) ya da Claude Code'un kendi sorusu gibi 100 saniye sonra vazgeçilene kadar tutulur.
- **Sorular (`?`)**: bir işçi size bir şey sorar; bir süre bekler, sonra yanıta bağlı olmayanla devam eder. `enter` konuşmayı soruda açar; işçiye yazmak yanıtlar.
- **Değişiklikler (`Δ`)**: bir kart imece'nin `run` aracıyla derleme ve testlerle biter (altı test çalıştırmasından biri düşer ve işçi geri döner), lidere bir `done` ve KARAR BEKLEYEN'de kartın dosyalarından yapılmış bir yama ve o ağaçtaki çalıştırmalarla bir değişiklik. `enter` diff'i açar; `a` sonraki adımda uygular (sekiz uygulamadan biri canlıdaki gibi git'in sözleriyle düşer), `r` reddeder ve nedenleriniz işçinin sonraki kartı olur; `c` `n` notlar, `x` kısmi uygulama, `>` alıntı ve `w` sarma canlıdaki gibi çalışır.
- **Oturumlar**: bir oturum bir kez başlar (Claude Code'un `/` komutlarıyla; bir işçininki worktree'sinin taşındığını ya da yerinde kaldığını söyleyebilir), zaman zaman durur (`■ oturum bitti · yazınca yeniden başlar`), `:forget` ya da `/clear` ile unutulur ve yazılınca yeniden başlar; `/cost` ve `/compact` bir tur olarak yanıtlanır.
- **Kese ve sınırlar**: ajan başına ve saat başına harcama, kesenin azalması, biten bir kesenin sözleri ve komutları reddetmesi; `:budget` notunda ve dolmak üzereyken başlıkta kullanım penceresi, ajanların harcadığından doldurulur.
- **Projeler**: `:open YOL [AD]` bir lider, iki işçi ve üç kartla sahte bir proje ekler; `:close` ajanlarını gönderir; `:rename` canlıdaki gibi. `:pin` de.
- **Kartlar ve denemeleri**: bir kart değişikliğini panonun sütunlarında izler; her yedinci ilk deneme başarısız olur ve lideri kartı başka bir işçiye verir, başarısız olmayı sürdüren bir kart dördüncü denemesinde devre kesiciye takılır ve sizi bekler (`a` bir daha, `r` başarısız say), uzun süren bir kart işçisinden alınır ve o işçinin turu geç bir `done` ile biter, bazı kartlar başkalarını bekler (`⧗ #2 bitince`), lider de ara sıra kimsenin almadığı bir kartı geri çeker. Liderin kendiliğinden panoya koyduğu her dördüncü kartı hiçbir işçi almaz: stall'un dakikalarından sonra liderin olur; lider onu bir işçiye verir, size sorar ya da size gelsin diye bırakır. Başarısız bir kartı bekleyen kart hemen liderin olur; lider başarısız kartı yeniden verir ya da kartı geri çeker.
- **Deponun kontrolü**: demonun her checkout'unda bir tane vardır; ilk değişiklik ona evetinizi ister, sonra her üçüncü değişiklikte başarısız olur.
- **Kartın kapattığı issue**: her üçüncü kart uydurma bir issue adlandırır (`#41 kapanır`); değişikliği bunu hükmün altında, issue'nun başlığıyla söyler. Lonca'daki projede değişiklik girince kapanır (her yedincisi kapanmaz: `gh yanıt vermedi`), `:closes off` sonrasında ise önce size sorar; diğerlerinde `:commit` iletisi `Closes #41` ile biter. Makineden hiçbir şey çıkmaz.
- **Sağlık**: başarısız her dördüncü test çalıştırması döngüye girer ve lidere döngü söylenir. Takılan işçi liderine söyler: lider çoğuna yolu söyler, her üçüncüyü riskli diye size sorar (ne denediği ve seçilecek yanıtlarla), her beşinciyi kendi hâline bırakır, o da size gelir. Arada bir oturumu biten işçi (böyle her üçüncü duruş, bunun yerine başarısız bir turdur) de aynı yolla liderinindir: lider onu kaldığı yerden yeniden başlatır, kartını başka bir işçiye verip vermemeyi size sorar ya da kendi hâline bırakır; başlatıldıktan sonra yeniden duran işçi ikinci kez başlatılmaz. İşçiler arasındaki her dördüncü `ask` yanıtsız kalır: stall'ın dakikaları geçince liderindir, lider de işçiye yanıtsız nasıl süreceğini söyler. Bir worktree'nin kurulum betiğinin her üçüncü çalışması başarısız olur (`npm ERR! network timeout`), işçi ve lideri bunu duyar.
- **Bütün ekibin durması**: saate göre, her 900 tikte (75 demo dakikası) her ekip, siz son yazdığınızdan beri iletilerle turlarını almış olur ve siz yazana ya da duruşunda `1`'e basana kadar iş dağıtmaz; her 1.300 tikte liderinin oturumu sona erer, `1` onu kaldığı yerden yeniden başlatır. Liderinin turu her 1.100 tikte başarısız olur (`1` sürsün der) ve her 2.300 tikte takılır (siz `1` ile turu bitirene dek stall dakikaları boyunca hiçbir şey demez); her 1.700 tikte de planın kullanım sınırına 60 tik için ulaşılır: ekip bekler, sınır kendiliğinden açılır. Harcanandan aşağı bir `:budget`, arkasında lider ve yeniden denemelerle bütçenin duruşunu gösterir. Her biri ana ekranda en üstte `■ DURDU` olarak gelir ([bütün ekip durunca](../screen/home.md#butun-ekip-durunca)).
- **Süreçler**: `:ps` her oturumun altında uydurma süreçler listeler (çalışırken test programı), `y` dediğinizde birini sonlandırır ve önceki bir çalıştırmadan kalan iki süreç ağacını listeler.
- **Lonca'daki bir proje**: iki ya da daha çok projeyle ikincisi sahte bir Lonca deposuna dayanır. İşçileri öneri yapar; onlarınkinin yanında bir yabancının önerisi (`ayse`) bekler, üç öneriden biri gövdenin gerisindeki bir temele dayanır, her beşinci iniş kuyrukta testlerinden kalır ve her üçüncü kart `api` kontrolünü ister, evetiniz de onu ister. Satırı deponun nasıl durduğunu söyler, `:lonca` penceresini açar, `:sync` checkout'u yetiştirir (her üçüncüsü sizin commit'lerinizi bulur) ve daemon 60 tikte 6 tik yanıt vermez.
- **Harcamalar**: `:ledger` kartlar uygulandıkça, başarısız oldukça, başkasına geçtikçe ve geri çekildikçe dolar; demonun girişi bir plan olduğundan her rakam `≈` okunur.
- **Bütün ekran**: gruplarıyla KARAR BEKLEYEN ve projeleriyle ÇALIŞAN'ıyla ana ekran, katlama ve gizleme, ayrıntının sekmeleri, pencereler (`:board`, lease'leriyle `:code`, `:timeline`, `J`), kartlar (`#N`), söz sıraları ve yanıtlarıyla (`↩`) konuşmalar, işaretler, `W` `S`, çıkış sorusu, yardım, temalar, iki dil, readline tuşları, çok satırlı yazma ve yapıştırma. `Confine` başlangıçta canlının söylediği gibi söylenir.

## Demo canlıyı yansıtır

Canlının yaptığına her düzeltme, özellik ya da değişiklik, aynı değişiklikte, testiyle demoda da gösterilir; bir şeyi taklit edemeyen bir değişiklik bunu söyler. Bu projenin yerleşik bir kuralıdır; demoda gördüğünüz, aynı kodun çizdiği, canlının yaptığıdır. Demonun bir zamanlar canlıda olmayan şeyleri (bir merge kuyruğu, trunk numaraları, doğrulanıyor durumu) çıkarıldı; Lonca'daki projesi artık canlının Lonca'da yaptığını gösterir.

## Taklit edilemeyenler

- **"Nerede kalmıştın" penceresi**, bir kapanışla yarım kalan turlar ve başlangıçtan uykuda ajanlar: bunlar bir depo ve önceki bir çalıştırma gerektirir; demo hiçbirini tutmaz.
- **Araştırmacılar**: demo onları bir ön izleme olarak gösterir; canlıda yok, kapatıldı, yapılmayacak (kayıt 01).
- **Worktree'lerdeki bir projenin kod haritası**: demo her projede kartlarından lease çizer, bir ön izleme olarak; canlıda lease yalnız Lonca'da vardır, orada harita daemon'ınkileri gösterir, worktree'lerdeki bir proje ise `henüz ortak trunk yok: her agent kendi worktree'sinde çalışıyor, kimin neyi düzenlediği gösterilemez` der. Demonun Lonca projesi `:lonca`'da tutulan sembol listelemez.
- **Panonun DOĞRULANIYOR sütunu**: canlı kartlar, Lonca'nın merge kuyruğu değişikliklerini tutarken orada durur; demo bir öneriyi hemen indirir, bu yüzden sütunu boş kalır.

## Ayrıca bakınız

- [İlk çalıştırma](first-run.md)
- [Klavye](../screen/keyboard.md)
- [Ana ekran](../screen/home.md)
- [Komut satırı başvurusu](../reference/cli.md)
- [Videolar](../videos.mdx)
