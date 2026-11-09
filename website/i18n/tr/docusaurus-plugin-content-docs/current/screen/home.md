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
 ana ekran · shop-api · #1 Kargo ücret hesabı    enter  aç   a  evet   r  hayır   ?  yardım
```

[Başlık satırı](salon.md#başlık-satırı) açık projeleri adlandırır; listenin daraltıldığı proje (`[` `]`) aydınlıktır. Lonca'daki bir projenin, nasıl durduğunu söyleyen kendi satırı listenin üstündedir.

## Liste

| Grup | İçindekiler |
| --- | --- |
| KARAR BEKLEYEN | her türü sayar: `1 izin · 2 soru · 3 değişiklik · 1 ertelenen` |
| izinler | her biri tek tuş: bir ajanın çalıştırmak istediği komut, worktree kurulum betiği, deponun kontrolü. Satır neyin çalışacağını söyler: `.imece/check değişikliklerinde çalışsın mı?` ve ardından betiğin komutları (yorum olmayan satırları; `enter` her baytını gösterir) |
| sorular | okuyup yanıtlanacaklar: bir ajanın sorusu bütün olarak, altında sunduğu yanıtlarla (`1`-`9` birini seçer, `/` kendinizinkini yazar: [sorular](../concepts/approvals-and-questions.md#sorular-)), bir araştırmacının raporu, okunacak bir yanıt, bir API hatası, liderin planı |
| değişiklikler | okunacak bir diff: `✓ dotnet build · ✓ dotnet test · 2 dosya +4 −2 · 1 kartın dışında (#51): Money.cs` — başı olmadan [hüküm](changes.md#yamanın-üstündekiler): ağacında kanıtlanan (çalıştıysa deponun kontrolü, sonra imece'nin çalıştırmaları, yoksa `hiçbir şey koşmadı`), kartının sahip olduğu yollara göre dosyalar ve satırlar, bekleyen hangi başka değişiklikle çakıştığı, onayından sonra Lonca'nın ne çalıştıracağı. Hazır olanlar (temiz kanıtlanmış, yollarının içinde, çakışmayan, uyan, bütün) önce gelir; grubun satırı onları sayar, `2 onaya hazır` |
| sonraya | `s` ile ertelediğiniz ögeler, ertelenme sırasıyla |
| ÇALIŞAN | `ÇALIŞAN 12 · takılan 2 · boşta 1 · 2 gizli (:unsettle)`: ekip, proje proje, açıldıkları sırayla, sabitlenenler önce. Görünürde birden çok proje varken (`[` `]` ile birine daraltılmamışsa) her biri ajanlarının başına [ekipteki](salon.md) satırını koyar: `▾ shop-api  ▓▓░░ 3/8  ! 2 bekliyor  1 çalışıyor  $4.10  ~/src/shop-api`; `h` onu o satıra katlar, `l` açar, `za` değiştirir, `zM` `zR` hepsini katlar ve açar. Altında işçiler ve sololar: önce takılanlar (`■`, bloke ya da oturumu bitmiş), sonra tur ortasındakiler ya da bir takım arkadaşına soranlar (`●`, bulundukları adım ya da yaptıkları, sonra bir dakikadan itibaren ne zamandır), sonra boştakiler (`○`), her birinin yardımcıları ve araştırmacısı altına asılı (`└`). Değişikliği bekleyen ve başka işe geçmiş bir işçi burada da görünür: iki ayrı iş. Sizden evet ya da yanıt bekleyen ajan ise o ögenin kendisidir, yardımcıları işçilerden sonra kendi başlarına durur; işi olmayan bir lider listelenmez. Bir ajanda ya da başlıkta `s` onu size ihtiyacı olana dek gizler; başlık gizlenenleri sayar |

Satır işin kendisidir: `!` (ya da ajanın işareti), birden çok proje açıkken ve liste daraltılmamışken projesi, kartı ve başlığı (bir değişikliğin kendi başlığı; kartı olmayan bir ajanın adı), sonra istek ya da durum. Bir istek kendi sütununun altına bütün olarak sarılır; kısa bir satırda değişikliğin parçaları alt alta geçer; ajanın durumu kesilir. Ajanın adı önizlemededir. `W`'nin sırası bu listenin sırasıdır: izinler, sorular, değişiklikler, ertelenenler.

## Önizleme

120 kolondan itibaren eldeki satır tuşa basmadan listenin yanında okunur: bir soru bütün olarak, yanıtlarıyla ve soran ajanın şimdiye dek yaptığıyla, ya da bir isteğin sözleri; bir betiğin satırları, numaralı; bir değişikliğin hüküm satırı, nedeni, maliyeti, geçidi, dosyaları ve git'in kendi satırları olmadan yamasının başı; bir ajanın durumu, yapılacakları ve en yeni iz satırları. 120 kolonun altında, editörün yanında, liste tüm genişliği alır ve satırlar her şeyi söyler.

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
| `@` | eldeki ajana yazar |
| `W` | sıradaki ögeyi açar, listenin sırasıyla |

Diğer tuşlar her yerdeki gibi çalışır; durum satırının tuşları eldeki satırınkilerdir.

Evet aşağı indirir: `a`, `r`, `e` ya da `A` işini yapınca o yere gelen satır elinize geçer, sıradaki tuş onu karara bağlar. Karar asla okumadığınız şeye düşmez: tuşa basmadan önce eldeki satır kendiliğinden gitmişse tuş hiçbir şey yapmaz, bir satır bunu söyler ve yerine gelen satır elinize geçer.

Ekranı başka hiçbir şey paylaşmaz: [günlük](journal.md) (bir başlangıcın notlarının düştüğü yer) bir penceredir, `J` ya da `:journal`; [pano, kod haritası](board-and-code-map.md) ve [zaman şeridi](timeline.md) de adlarıyla açılan pencerelerdir (`:board`, `:code`, `:timeline`); `esc` onları kapatır. Bir [tuş dosyasının](keyboard.md#kendi-tuşlarınız) `[salon]` yeri ana ekranınkidir.
