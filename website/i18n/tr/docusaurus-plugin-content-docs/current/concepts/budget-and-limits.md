---
title: Bütçe ve limitler
description: "Bütün ekibin harcadığı tek bütçe, nasıl belirlenir ve değiştirilir, başlık yakma hızı hakkında ne der, satıcıların kullanım pencereleri, : menüsünde neye para gider, :commit'in kendi tavanları."
---

# Bütçe ve limitler

Bütçe bütün ekip için tek bir kesedir: bütün ajanlar birlikte onu bitirince kimse yeni bir tur almaz. Onunla her ekranın başlığında, `:budget`'ta ve `:` menüsünün `$` işaretlerinde karşılaşırsınız.

## Bütçe

- `--budget USD` belirler; ilk seferinde 50 $. Bütçe **çalıştırmalar arasında saklanır**: `--budget`'sız bir başlangıç kalandan devam eder. Sonraki bir başlangıçta `--budget` yeniden belirler.
- Her projenin her ajanının turları ondan çıkar; imece'nin kendi ücretli çağrıları da (`:commit`). Ajan başına pay yoktur.
- Bitince hiçbir ajan uyandıran mesaj almaz ve kart teklif edilmez; birini uyandıracak bir `send`, "the budget is spent" ile reddedilir.
- **Oturum başına sert tavan:** her Claude Code oturumu, başladığında bütçeden kalanı kendi `--max-budget-usd`'si olarak alır; tek başına kaçan bir oturum da onu geçemez. Bu bir pay değil, son duraktır. Codex ve Gemini'de böyle bir tavan yoktur; bütçe yine sonraki turlarını durdurur.

### `:budget USD`

`:budget 80` (`:b 80`) ajanlar çalışırken bütçeyi artırır ya da azaltır. Süren bir tur eski tavanında biter; sonraki yeni kalandan başlar. Turu eski tavanında biten ama bütçede yer olan bir Claude oturumu bitirilir ve aynı istemlerle, yeni kalanla yeniden başlatılır; bir tur bitene dek oturum başına bir kez. Günlük `bütçeyi $80.00 yaptın (önce $50.00)` der. `80` ya da `2.50` gibi dolar olmayan reddedilir.

## Başlık

```text
 imece  3/7 kart │ ● 2 çalışıyor ✓ 1 bitti │ $3.10 / $20.00 harcandı ($2.40/sa, ~2 sa 10 dk kaldı) │ claude 5 sa: %48 kaldı, 1 sa 59 dk sonra yenilenir
```

- `$/sa`, **son on dakikanın** saat başına maliyetidir; başlangıçtan beri ortalama değil (boş bir saat onu aşağı, ilk tur yukarı çekerdi). Maliyetler tur sonlarında gelir, bu yüzden hız basamak basamak oynar. Yalnız bu çalıştırma sayılır.
- `~… kaldı`, bütçenin o hızla ne kadar dayanacağıdır. Yarım saatin altında bütün bütçe **dikkat rengine** döner ve günlük bir kez, hız ve dakikalarla, bütçenin azaldığını söyler: `$12.00/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır`. `:budget` yeniden söyletir. Bu bir günlük satırıdır, SENİ BEKLEYEN'de bir öğe değil: içinde yanıt gereken bir şey yoktur ve bütçe turları kendi başına durdurur.
- Her ajanın kendi toplamı salon satırındadır; ajan başına hız yoktur.

## Satıcı kullanım pencereleri

Claude ya da ChatGPT aboneliğindeki bir ekip, planın beş saatlik ya da haftalık limitiyle bütçe bitmeden çok önce durabilir. Claude Code ve Codex oturumları hesaplarının pencerelerini bildirir; Gemini bildirmez. imece satıcı ve pencere uzunluğu başına birini tutar ve her satıcının en sıkışık olanını bütçeden sonra gösterir: `codex 7 g: %12 kaldı`, %90 kullanımdan itibaren dikkat renginde, yenilenme ilerideyken ona kalan süreyle. Bir pencere dolmadan önce hiçbir şey turu durdurmaz: satıcı reddeder, imece reddi gösterir. Pencereler çalıştırmalar arasında saklanmaz.

## Ne sayılır, nasıl

| Satıcı | Bir tur bütçeye ne yazar |
| --- | --- |
| Claude Code | bildirdiği dolarlar (`total_cost_usd`); devam eden bir oturum bütün oturumunkini bildirir ve imece yalnız önceden yazılanın üstünde kalanı sayar |
| Codex | ChatGPT planında hiçbir şey (token'lar izde gösterilir); API anahtarıyla oturum reddedilir, imece henüz fiyatlandıramaz |
| Gemini CLI | API anahtarıyla imece'nin taşıdığı liste fiyatları (200K token'a kadar istemler, düşünce token'ları çıktı olarak, önbellekli token'lar önbellek fiyatından, birden çok model en pahalısından); Google oturumuyla hiçbir şey; fiyatlandıramadığı bir model oturumu durdurur |

Harcanan, her başlangıçta depodan geri okunur; ajanlar ve imece'nin kendi çağrıları birlikte.

## `:` menüsünde `$`

`:` menüsü her komutun değerini ardında gösterir, `budget (spent $3.10 of $20.00)`, ve sonraki faturayı büyüten komutları `$` ile işaretler:

- `:open` yeni bir lider oturumu başlatır.
- `:forget` sonraki mesajın bütün özeti yeniden, önbelleksiz ödeyen yeni bir oturum başlatmasına yol açar.
- `:commit` ücretli bir model çağrısıdır.

Hiçbir `:` komutu kendi başına ücretli bir tur başlatmaz; `:budget` işaret yerine değerini gösterir.

## `:commit` ve tavanları

`:commit`, eldeki projede stage'dekiler için Claude Code'un ucuz kademesinden (`haiku`, ya da sizin `--tier claude:cheap=…`'iniz) bir commit mesajı ister ve komut satırına `!git commit -m '…'` olarak koyar. İstenince çalışır, her uygulamada değil; çünkü çoğu zaman birkaç değişikliği uygulayıp bir kez commit edersiniz. Tavanları yerleşik maliyet kurallarını izler: günlük bir tavan ve çağrı başına bir üst sınır, çağrıdan önce ayrılmış.

| Tavan | Değer |
| --- | --- |
| Bir öneri | en çok 0,05 $ (çağrının kendi `--max-budget-usd`'si) |
| Bir gün (UTC), bütün projeler birlikte | 20 öneri ve 1,00 $ |
| Aynı anda çalışan | bir |

Çağrıdan önce depo onu tek bir işlemde tavanından sayar ve bütçe tavanı tutar; bütçede tavandan az varsa ya da gün dolmuşsa reddedilir. Çağrı bitince bildirdiği, tavanın yerine geçer; başlayamayan hiçbir şey ödemez; bildirmeden duran tavanını korur. Aynı stage'li ağaç yeniden sorulunca hiçbir şey ödenmez: proje başına son öneri saklanır. Günlük her öneriyi modeli ve maliyetiyle, her başarısızlığı nedeniyle söyler. Yalnız Claude Code: çağrısı sert bir harcama tavanı alan tek satıcıdır; PATH'te yoksa `:commit` reddedilir.

## Demo

`--demo` uydurma bir bütçeyi aynı yolla harcar: başlık, hız, azalan bütçe satırı, `:budget`, kullanım pencereleri (harcamadan doldurulur) ve `:commit`'in birkaç kuruşu canlıdaki gibi görünür.

## Ayrıca bakınız

- [Satıcılar](../concepts/vendors.md): satıcı başına token mı dolar mı.
- [Oturumlar](../concepts/sessions.md): bütçe depoda.
- [Kendi kabuğunuz ve commit](../scenarios/own-shell-and-commit.md): `:commit` kullanımda.
- [Komutlar](../reference/commands.md): `:budget`, `:commit`.
