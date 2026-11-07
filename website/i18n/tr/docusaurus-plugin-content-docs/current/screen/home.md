---
title: Ana ekran
description: Görünüm 5 - karar listesi - karar bekleyenler, karar vermenin maliyetine göre gruplu, sonra çalışanlar; her satır isteğin tamamını ve karar tuşlarını söyler.
---

# Ana ekran

Ana ekran, imece'nin açıldığı ve karar verdiğiniz ekrandır; `5` ya da `:home` geri getirir. Açık projelerin hepsi için tek liste: önce karar bekleyenler, karar vermenin size maliyetine göre gruplu, sonra çalışanlar. Her satır isteğin tamamını söyler, eldeki satırı karara bağlayan tuşlar satırın sonunda durur ve bir evet imleci aşağı indirir. 120 kolondan itibaren eldeki satır listenin yanında okunur.

```text
 imece  shop-api │  ! 5 karar bekliyor  │ $0.49 / $200.00  14:20
 KARAR BEKLEYEN 1 izin · 1 soru · 3 değişiklik
 izinler                                                                            her biri tek tuş
▸! #2   Money rounding rules      .imece/check değişikliklerinde çalışsın mı?        a  evet   r  hayır
                                  set -e; cd "$IMECE_WORKTREE"; npm run build; npm test -- --silent
 sorular
 ! #6   Shipping zones            Half-up rounding for Money, or banker's rounding?
 değişiklikler                                                                       2 onaya hazır
 ! #4   Order validation rules    1 dosya +2 −1 · ✓ dotnet build, dotnet test
 ! #5   Order tests               1 dosya +2 −1 · ✓ dotnet build, dotnet test
 ! #6   Shipping zones            4 dosya +8 −4 · ✓ dotnet build, dotnet test · 1 tanesi #6 dışında

 ÇALIŞAN 2
 ● #8   Notification settings     Write it up for the leader · 5 dk
 ● #7   Bulk e-mail queue         run dotnet test · 12 dk
 ana ekran · shop-api · #2 Money rounding rules    enter  aç   a  evet   r  hayır   ?  yardım
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
| ÇALIŞAN | `ÇALIŞAN 12 · takılan 2 · boşta 1`: önce takılan ajanlar (`■`, bloke ya da oturumu bitmiş), sonra tur ortasındakiler ya da bir takım arkadaşına soranlar (`●`, bulundukları adım ya da yaptıkları, sonra bir dakikadan itibaren ne zamandır), sonra boştakiler (`○`). Değişikliği bekleyen ve başka işe geçmiş bir işçi burada da görünür: iki ayrı iş. Sizden evet ya da yanıt bekleyen ajan ise o ögenin kendisidir; işi olmayan bir lider listelenmez |

Satır işin kendisidir: `!` (ya da ajanın işareti), birden çok proje açıkken ve liste daraltılmamışken projesi, kartı ve başlığı, sonra istek ya da durum. Bir istek kendi sütununun altına bütün olarak sarılır; kısa bir satırda değişikliğin parçaları alt alta geçer; ajanın durumu kesilir. Ajanın adı önizlemededir. `W`'nin sırası bu listenin sırasıdır: izinler, sorular, değişiklikler, ertelenenler.

## Önizleme

120 kolondan itibaren eldeki satır tuşa basmadan listenin yanında okunur: bir soru bütün olarak, yanıtlarıyla ve soran ajanın şimdiye dek yaptığıyla, ya da bir isteğin sözleri; bir betiğin satırları, numaralı; bir değişikliğin hüküm satırı, nedeni, maliyeti, geçidi, dosyaları ve git'in kendi satırları olmadan yamasının başı; bir ajanın durumu, yapılacakları ve en yeni iz satırları. 120 kolonun altında, editörün yanında, liste tüm genişliği alır ve satırlar her şeyi söyler.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | gezer; önizleme izler |
| `gd` `gw` | karar bekleyen ilk satır · çalışan ilk satır |
| `[` `]` | tüm projeler, sonra her biri, dönerek |
| `enter` | açar: değişikliğin diff'i, sorunun konuşması, betiğin baytları, ajanın odağı. `esc` buraya döner |
| `a` `r` | evet ve hayır, [KARAR BEKLEYEN](waiting.md)'deki gibi; değişiklikte onayla ve geri gönder; soruda `a` yanıt yazar. Eldeki satır bunları sonunda söyler |
| `e` `A` | bir istekte: bu tur evet · bundan sonra evet |
| `s` | sonra: öge KARAR BEKLEYEN'in sonundaki `sonraya` grubuna gider, listede kalırsınız |
| `@` | eldeki ajana yazar |
| `W` | sıradaki ögeyi açar, listenin sırasıyla |

Diğer tuşlar her yerdeki gibi çalışır; durum satırının tuşları eldeki satırınkilerdir.

Evet aşağı indirir: `a`, `r`, `e` ya da `A` işini yapınca o yere gelen satır elinize geçer, sıradaki tuş onu karara bağlar. Karar asla okumadığınız şeye düşmez: tuşa basmadan önce eldeki satır kendiliğinden gitmişse tuş hiçbir şey yapmaz, bir satır bunu söyler ve yerine gelen satır elinize geçer.

Ekip (bir başlangıcın notlarının düştüğü günlükle), pano, kod haritası ve zaman şeridi yerinde, bir tuş uzakta. Tuş dosyasının `[ekip]` bölümü ana ekranda da geçerlidir.
