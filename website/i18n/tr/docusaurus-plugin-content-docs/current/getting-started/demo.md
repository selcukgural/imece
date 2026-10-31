---
title: "Demo"
description: "`imece --demo` bütün ekranı sahte ajanlarla, maliyetsiz, canlının aynasında çalıştırır."
---

# Demo

`imece --demo` ekranı gerçek ajanlar yerine tohumlu bir simülasyonla çalıştırır: liderleri ve birkaç satıcıdan işçileriyle projeler; kart asar, alır ve dağıtırlar, bus'ta konuşurlar, size soru sorarlar, onayınızı beklerler, yardımcı tutarlar ve sizi bekleyen değişikliklerle bitirirler. Hiçbir satıcı CLI'ı başlatılmaz ve hiçbir şey para harcamaz. Demoyla, bir kuruş harcamadan önce ekranı iş başında görmek ya da tuşları öğrenmek istediğinizde karşılaşırsınız.

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

`--projects`, `--researchers` ve `--seed` yalnız demoya aittir: canlı imece bunları `--projects is for --demo` sözüyle reddeder. Demonun saati bugünün tarihinde 14:00'te başlar. Demo deponun ekran ayarlarını (tema, yan panel genişliği) canlıyla paylaşır ama başka hiçbir şey yazmaz: proje yok, günlük yok, kese yok.

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

- **Liderin işi**: kart asar (başlangıçta da), isteğinizi astığı kartlarla yanıtlar (SENİ BEKLEYEN'de bir yanıt `»`), bir kartı boştaki bir işçiye verir ya da panodan almasına bırakır, iki kart bekler ve kimse boşta değilse işçi tutar (en çok altı), katmanlı bir tutma zaman zaman varsayılan modele düşer.
- **İşçiler**: bus'ta tipli mesajlar (`assign`, `ask`, `answer`, `progress`, `done`), varsaydıklarını size `inform` olarak söylemeleri, dokunduklarının kartın hikâyesinde görünmesi, testler için tutulan ve `done` gönderip ayrılan bir yardımcı.
- **Onaylar (`⚑`)**: bir işçinin oturumu projenin hassas adımlarından biri için onayınızı ister; öğe siz karar verene (`a` izin, `r` ret) ya da Claude Code'un kendi sorusu gibi 100 saniye sonra vazgeçilene kadar tutulur.
- **Sorular (`?`)**: bir işçi size bir şey sorar; bir süre bekler, sonra yanıta bağlı olmayanla devam eder. `enter` konuşmayı soruda açar; işçiye yazmak yanıtlar.
- **Değişiklikler (`Δ`)**: bir kart imece'nin `run` aracıyla derleme ve testlerle biter (altı test çalıştırmasından biri düşer ve işçi geri döner), lidere bir `done` ve SENİ BEKLEYEN'de kartın dosyalarından yapılmış bir yama ve o ağaçtaki çalıştırmalarla bir değişiklik. `enter` diff'i açar; `a` sonraki adımda uygular (sekiz uygulamadan biri canlıdaki gibi git'in sözleriyle düşer), `r` reddeder ve nedenleriniz işçinin sonraki kartı olur; `c` `n` notlar, `x` kısmi uygulama, `>` alıntı ve `w` sarma canlıdaki gibi çalışır.
- **Oturumlar**: bir oturum bir kez başlar (satıcısının `/` komutlarıyla; bir işçininki worktree'sinin taşındığını ya da yerinde kaldığını, ya da deponun notlarının kesildiğini söyleyebilir), zaman zaman durur (`■ durdu, yazınca yeniden başlar`), `:forget` ya da `/clear` ile unutulur ve yazılınca yeniden başlar; `/cost` ve `/compact` bir tur olarak yanıtlanır; bir Codex ajanının arada tur olmadan ikinci `/compact` komutu düşen bir tur olarak reddedilir.
- **Kese ve sınırlar**: ajan başına ve saat başına harcama, kesenin azalması, biten bir kesenin sözleri ve komutları reddetmesi; başlıkta her satıcının kullanım penceresi, ajanlarının harcadığından doldurulur.
- **Projeler**: `:open YOL [AD]` bir lider, rastgele satıcılardan iki işçi ve üç kartla sahte bir proje ekler; `:close` ajanlarını gönderir; `:rename` canlıdaki gibi. `:pin` de.
- **Bütün ekran**: salon, katlama, `f`, pano (`2`), lease'leriyle kod haritası (`3`), kartlar (`#N`), söz sıraları ve yanıtlarıyla (`↩`) konuşmalar, işaretler, günlük, `W` `S`, çıkış sorusu, yardım, temalar, iki dil, readline tuşları, çok satırlı yazma ve yapıştırma. `Confine` başlangıçta canlının söylediği gibi söylenir.

## Demo canlıyı yansıtır

Canlının yaptığına her düzeltme, özellik ya da değişiklik, aynı değişiklikte, testiyle demoda da gösterilir; bir şeyi taklit edemeyen bir değişiklik bunu söyler. Bu projenin yerleşik bir kuralıdır; demoda gördüğünüz, aynı kodun çizdiği, canlının yaptığıdır. Demonun bir zamanlar canlıda olmayan şeyleri (bir merge kuyruğu, trunk numaraları, doğrulanıyor durumu) çıkarıldı.

## Taklit edilemeyenler

- **"Nerede kalmıştın" penceresi**, bir kapanışla yarım kalan turlar ve başlangıçtan uykuda ajanlar: bunlar bir depo ve önceki bir çalıştırma gerektirir; demo hiçbirini tutmaz.
- **Araştırmacılar, sembol lease'leri ve kod haritasının lease'leri, Qwen ve Local satıcıları**: demo bunları planlananın ön izlemesi olarak gösterir; canlıda henüz yok. Canlının kod haritası `henüz ortak trunk yok: her agent kendi worktree'sinde çalışıyor, gösterilecek lease yok` der.
- **Artık hiçbir şeyin doldurmadığı iki ekran parçası**: salonun KUYRUK satırı ve panonun DOĞRULANIYOR sütunu, merge kuyruğu gidince kaldı.

## Ayrıca bakınız

- [İlk çalıştırma](first-run.md)
- [Klavye](../screen/keyboard.md)
- [Salon](../screen/salon.md)
- [Komut satırı başvurusu](../reference/cli.md)
- [Videolar](../videos.mdx)
