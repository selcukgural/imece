---
title: Ekip
description: Bir imece ekibinde kim var - lider, işçiler ve yardımcılar - lider kademelerle nasıl işe alır, ajanlar nasıl adlandırılır.
---

# Ekip

Bir imece ekibi, sizinle konuşan ve iş gerektirdikçe işçi alan bir lider ile görevlerinin parçaları için yardımcı alabilen işçilerden oluşur. Ekip boyutunu siz seçmezsiniz; üst sınırları koyarsınız. Ekiple ana ekranda, ÇALIŞAN altında, projeye göre gruplanmış olarak karşılaşırsınız.

## Roller

| Rol | Nerede çalışır | Bus'ta ne yapabilir |
| --- | --- | --- |
| Lider (`lead`) | Checkout'unuz, yalnız okuma | `send`, `post`, `board`, `run`, `whoami`, `hire` (işçiler) |
| İşçi (`claude-1`, `claude-2`, …) | HEAD'inizde detached kendi worktree'si | `send`, `post`, `board`, `run`, `whoami`, `claim`, `hire` (yardımcılar) |
| Yardımcı (`claude-1.1`, …) | İşçisinin worktree'si, işçisinin adını verdiği dosyalar | `send`, `post`, `board`, `run`, `whoami` |
| Solo (`solo`, `solo-notes`) | Kendi worktree'si, lideri olmayan bir projede | `send`, `run`, `hire` (yardımcılar) |

- **Lider** checkout'u okur, isteğinizi kartlara böler ve her birini sahip olduğu yollarla ([kartın sahip olduğu yollar](messages-and-board.md#kartın-sahip-olduğu-yollar)) `send` (tür `assign`) ile dağıtır ya da herhangi bir işçinin alması için panoya asar. İkinci işçiyi almadan önce planı önce size gelir (KARAR BEKLEYEN'de `▤`; [plan](messages-and-board.md#plan)). Boşta bir işçi (kartı bitmiş, bekleyen değişikliği yok) kimse alınmadan sonraki kartı alır: `whoami` boştaki işçileri listeler, boştaki birinin katmanında `hire` da `claude-1 is free … give it the next card` diye yanıt verir. Hiçbir `--confine` düzeyinde dosya değiştirmez: checkout'unuzda çalışır, orada bir yazma incelemeyi atlar. Claude Code'un okuma araçlarını ve `standard`'dan itibaren web'i alır. Bir işçinin `blocked` mesajını önce lider çözer; size yalnızca yol riskliyse ya da yol bulamadıysa sorar ([takılan işçi](messages-and-board.md#takılan-işçi)).
- **Bir işçi** kendi worktree'sinde dosya değiştirir, işini `run` aracıyla denetler ve görevi kim verdiyse ona `done` gönderir. `done`'ı, incelemeniz için bir değişikliğe dönüşür ([değişiklikler](../concepts/changes.md)). İşçi başına bir anda bir kart.
- **Bir yardımcı**, işçi için liderin işçisi neyse odur. İşçi, kendi işinin yanında yürüyebilecek bir parça için, değiştirmediği dosyalarda bir yardımcı alır. Yardımcıya kart teklif edilmez, kimseyi işe almaz (yardımcının yardımcısı yok) ve `--agents` sınırına sayılmaz. İşçisine gönderdiği `done` ayrılışını işaretler; o tur bitince ayrılır, böylece turun maliyeti yine raporlanır. Günlük `işini bitirdi, ayrıldı` der. Duran bir işçi yardımcılarını da götürür. `done`'undan önce duran bir yardımcıyı (oturumu biter ya da son turu kalıcı olarak düşer: reddedilen bir anahtar, biten bütçe) imece işçisine onun bir sonraki turunda söyler, "ona verdiğin parça yapılmadı: kendin yap ya da başka bir yardımcı al", ve boştaki bir işçi bunun için uyandırılır; böylece yanıt vermeyecek bir yardımcıyı beklemez. imece'nin yeniden denediği bir API hatası henüz durmak sayılmaz.
- **Bir solo**, lideri olmayan bir işçidir: onu, seçtiğiniz model ve çabada (`:solo`, `--solo`) kendi başına bir projede siz sürersiniz. Panosu ve kartı yoktur; bir yanıtla biten her turda worktree'sinin diff'ini size gösterir. Bir işçi gibi yardımcı alabilir ([solo oturumlar](solo.md)).
- **Araştırmacılar** kapatıldı, yapılmayacak: işçilerin ve yardımcıların web'i zaten kendilerinde, bu yüzden ayrı bir okuyucu havuzu planlanmıyor. Yalnız demo hâlâ bir tavan gösterir (`--researchers N`).

Claude Code'un kendi alt ajan aracı (`Agent`/`Task`) her düzeyde kapalıdır: oturumun içinde açılan bir alt ajan worktree'yi, bütçeyi, bus'ı ve ekranı atlardı. Yardımcılar imece'nin bu ihtiyaca yanıtıdır.

## İşe alma

```sh
imece                              # lider tek başına; gerektikçe işe alır
imece --agents 8 --helpers 3       # sınırlar: ekip başına işçi, işçi başına yardımcı
```

- Yalın `imece` lideri tek başına başlatır. Lider, iş gerektirdikçe `hire` aracıyla işçi alır, en çok `--agents` kadar (0-16, varsayılan 4). Bir işçi aynı yolla yardımcı alır, bir anda en çok `--helpers` kadar (0-8, varsayılan 2).
- `hire` bir `tier` alır.
- **Bir işçinin başladığı yer.** Bir işçinin worktree'si HEAD'inizde başlar ve onu izler. Liderden başka bir noktada iş isteyin ("PR 41'i incele", "bunu release-0.2'de düzelt"), `from` ile işe alır: checkout'unuzun bir dalı, etiketi ya da commit'i, ya da `origin`'in bir pull request'i için `#41` (işçi başlarken çekilir). O worktree orada başlar ve orada kalır: HEAD'inizi asla izlemez, değişikliği o noktaya göredir ve onu checkout'unuza her değişiklik gibi uygularsınız (temiz oturmazsa üç yollu). Lonca'daki bir projede işçi trunk'tan başlar ve `from` reddedilir. Günlük `lead işe aldı: claude-2 (haiku), başlangıç noktası #41` der, satır `⎇ #41` gösterir ve nokta çalıştırmalar arasında saklanır. Yardımcıları aynı worktree'de çalışır.
- Yeni ajan var olmadan önce bus'ta ona bir soket bağlanır. İşe alma, hemen yürüyebilecek iş içindir, önceden değil: bir işçi birçok görevi sırayla alır.
- Ajan başına bütçe payı yoktur; bütün ekip tek bütçeden harcar ([bütçe](../concepts/budget-and-limits.md)).

## Model değil, kademe

Lider bir işçi için kademe seçer, asla model adı değil:

| Kademe | Ne için | Model |
| --- | --- | --- |
| `cheap` | küçük, iyi tanımlı mekanik iş (yeniden adlandırma, bilinen davranışın testleri, dokümantasyon) | `haiku` |
| `standard` (varsayılan) | sıradan düzeltmeler ve özellikler | Claude Code'un kendi varsayılanı |
| `strong` | kodu enine kesen tasarım, zor hata ayıklama, inceleme | `opus` |

- Liderin kendisi Claude Code'un varsayılanında ya da `--model`'de çalışır.
- `--tier T=AD` bir kademenin anlamını değiştirir: `--tier strong=opus[1m]`, `--tier strong=opus@xhigh` (bir değer `[MODEL][@ÇABA]`dır). `--model AD` ajanların çalıştığı modeli belirler.
- **Geri düşme:** hesabınızda olmayan bir kademe modeli ilk turunda başarısız olur (Claude Code bunu başlayan ve sonra sıfır maliyetle hata veren bir oturum olarak bildirir). imece o işçiyi aynı istemlerle, bir kez, standart modelle değiştirir ve günlük `X modeli burada yok, varsayılanla devam` der. Tek deneme, asla döngü değil.

## Adlar

- imece'nin başladığı projenin lideri `lead`'dir; `:open YOL AD` ile açılan bir proje `lead-AD` alır ve `:rename AD` lideri projesiyle birlikte yeniden adlandırır.
- İşçiler `claude-N`'dir: `claude-1`, `claude-2`. Sayı işçileri sayar, ayrılanlar dahil, ve hâlâ alınmış bir adı atlar.
- Yardımcılar `<işçi>.N`'dir: `claude-1.1`, `claude-1`'in ilk yardımcısıdır.
- Tek bir ajana `@ad mesaj` ile yazarsınız; `you`, bir ajanın size hitabıdır (`send(to: "you")`).

Ajanlar bus'ta birbirlerine bu adlarla seslenir; aynı projenin ekibinde olmayan bir ada `send`, ekip listesiyle reddedilir.

## Ekranda

```text
 ÇALIŞAN 4
 ● #13  E-posta şablonları        Değişikliği yap · 22 dk
   └ ● claude-1.1                başlıyor…
 ● #12  Arama sonuç sıralaması    Lidere yaz · 4 dk
 ● #7   Ödeme dokümanı            Çevresindeki kodu oku
```

Bir işçinin satırı kartıyla başlar (`#13 E-posta şablonları`), sonra yapılacaklar listesinde bulunduğu adım ya da yaptığı şey ve ne zamandır (bir dakikadan itibaren); kartı olmayan bir işçi adını gösterir. Nasıl durduğunu beş glif söyler: `●` çalışıyor, `○` boşta, `■` takıldı (bloke, ya da oturumu bitti), `!` sizi bekliyor, `✓` bitti. Önce takılan işçiler gelir, sonra çalışanlar, sonra boştakiler; yardımcılar işçilerinin altına asılır (`└`). Evetinizi ya da yanıtınızı bekleyen bir işçi burada listelenmez: KARAR BEKLEYEN'deki o ögenin kendisidir. Bir lider, çalışırken ya da takıldığında projesinin işçilerinden sonra listelenir, boştayken listelenmez; bir projenin başlığında `enter` onu açar. Bir satırda `enter` o ajanın işini açar; baş satırı kartını, projesini, ajanı, süresini ve maliyetini söyler. Başlık ajan saymaz: sizi gerektireni `! N karar bekliyor` söyler, kalanını ana ekranın başlıkları sayar (`ÇALIŞAN 4`, `takılan 1 · boşta 2`).

## Sağlık bildirimleri

imece birkaç saniyede bir ekibe bakar ve liderin bilmek isteyeceğini liderin bir sonraki isteminde söyler:

- **takılan bir kart**: işçisi on dakikadır `done` demeden boşta ya da turunda hiçbir şey demiyor (bu kadar zamandır bir takım arkadaşının yanıtını bekleyen işçi ise liderinin çözeceği bir duruştur: [yanıt bekleyen işçi](messages-and-board.md#yanıt-bekleyen-işçi));
- **boştaki bir işçi**: kartı olmayan bir işçi on dakikadır boşta;
- **bir döngü**: bir çağrı (bir hedefte bir araç, `Bash cargo test`) bir turda üç kez hata verdi.

Takılan kart ve döngü boştaki lideri bunun için uyandırır (mesajlarla alabileceği turlardan sayılır); boştaki işçi hiçbir şeye mal olmadığından liderin sonraki turunu bekler. Her biri ajan yeniden kıpırdayana kadar bir kez söylenir. imece hiçbir şeyi durdurmaz: ne yapılacağı (bir soru, kartın yeniden verilmesi ya da geri çekilmesi, bir `cancel`) liderin ya da sizindir. Sizi bekleyen bir şey (bir soru, bir izin, bir değişiklik) asla takılma sayılmaz. Günlük her birini ve kimin duyduğunu söyler: `claude-1 10 dk'dır #3 parser üzerinde boşta, done demedi · lead uyarıldı`; liderin kendi döngüsü yalnızca size söylenir. Dakikalar ve sayı sizindir: [`:health`](../reference/commands.md#health-stallidleloop-value).

Bir yardımcı hiç okumadığı mesajlarla ayrılırsa, ekipte kalan her gönderene hangi mesajının okunmadığı söylenir; gönderilen hiçbir şey sessizce kaybolmaz.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Claude Code](../concepts/claude-code.md): confine düzeyleri, bir oturum nedir.
- [Projeler](../concepts/projects.md): proje başına bir lider.
- [Ana ekran](../screen/home.md) ve [CLI başvurusu](../reference/cli.md).
