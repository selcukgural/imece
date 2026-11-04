---
title: Ekip
description: Bir imece ekibinde kim var - lider, işçiler, yardımcılar ve henüz yapılmamış araştırmacılar - lider kademelerle nasıl işe alır, ajanlar nasıl adlandırılır.
---

# Ekip

Bir imece ekibi, sizinle konuşan ve iş gerektirdikçe işçi alan bir lider, görevlerinin parçaları için yardımcı alabilen işçiler ve (bir gün) araştırmacılardan oluşur. Ekip boyutunu siz seçmezsiniz; üst sınırları koyarsınız. Ekiple salonda, projeye göre gruplanmış olarak karşılaşırsınız.

## Roller

| Rol | Nerede çalışır | Bus'ta ne yapabilir |
| --- | --- | --- |
| Lider (`lead`) | Checkout'unuz, yalnız okuma | `send`, `post`, `board`, `run`, `whoami`, `hire` (işçiler) |
| İşçi (`claude-1`, `claude-2`, …) | HEAD'inizde detached kendi worktree'si | `send`, `post`, `board`, `run`, `whoami`, `claim`, `hire` (yardımcılar) |
| Yardımcı (`claude-1.1`, …) | İşçisinin worktree'si, işçisinin adını verdiği dosyalar | `send`, `post`, `board`, `run`, `whoami` |
| Solo (`solo`, `solo-notes`) | Kendi worktree'si, lideri olmayan bir projede | `send`, `run`, `hire` (yardımcılar) |
| Araştırmacı | henüz yapılmadı | — |

- **Lider** checkout'u okur, isteğinizi kartlara böler ve her birini sahip olduğu yollarla ([kartın sahip olduğu yollar](messages-and-board.md#kartın-sahip-olduğu-yollar)) `send` (tür `assign`) ile dağıtır ya da herhangi bir işçinin alması için panoya asar. İkinci işçiyi almadan önce planı önce size gelir (SENİ BEKLEYEN'de `▤`; [plan](messages-and-board.md#plan)). Hiçbir `--confine` düzeyinde dosya değiştirmez: checkout'unuzda çalışır, orada bir yazma incelemeyi atlar. Claude Code'un okuma araçlarını ve `standard`'dan itibaren web'i alır.
- **Bir işçi** kendi worktree'sinde dosya değiştirir, işini `run` aracıyla denetler ve görevi kim verdiyse ona `done` gönderir. `done`'ı, incelemeniz için bir değişikliğe dönüşür ([değişiklikler](../concepts/changes.md)). İşçi başına bir anda bir kart.
- **Bir yardımcı**, işçi için liderin işçisi neyse odur. İşçi, kendi işinin yanında yürüyebilecek bir parça için, değiştirmediği dosyalarda bir yardımcı alır. Yardımcıya kart teklif edilmez, kimseyi işe almaz (yardımcının yardımcısı yok) ve `--agents` sınırına sayılmaz. İşçisine gönderdiği `done` ayrılışını işaretler; o tur bitince ayrılır, böylece turun maliyeti yine raporlanır. Günlük `işini bitirdi, ayrıldı` der. Duran bir işçi yardımcılarını da götürür. `done`'undan önce duran bir yardımcıyı (oturumu biter ya da son turu kalıcı olarak düşer: reddedilen bir anahtar, biten bütçe) imece işçisine onun bir sonraki turunda söyler, "ona verdiğin parça yapılmadı: kendin yap ya da başka bir yardımcı al", ve boştaki bir işçi bunun için uyandırılır; böylece yanıt vermeyecek bir yardımcıyı beklemez. imece'nin yeniden denediği bir API hatası henüz durmak sayılmaz.
- **Bir solo**, lideri olmayan bir işçidir: onu, seçtiğiniz model ve çabada (`:solo`, `--solo`) kendi başına bir projede siz sürersiniz. Panosu ve kartı yoktur; bir yanıtla biten her turda worktree'sinin diff'ini size gösterir. Bir işçi gibi yardımcı alabilir ([solo oturumlar](solo.md)).
- **Araştırmacılar** planlanmış bir katmandır (soruları yanıtlayan sınırlı bir havuz, dereceli kirlilik ve güvenilir kaynaklarla). Yapılmadı; yalnız demo gösterir (`--researchers N`).

Claude Code'un kendi alt ajan aracı (`Agent`/`Task`) her düzeyde kapalıdır: oturumun içinde açılan bir alt ajan worktree'yi, bütçeyi, bus'ı ve ekranı atlardı. Yardımcılar imece'nin bu ihtiyaca yanıtıdır.

## İşe alma

```sh
imece                              # lider tek başına; gerektikçe işe alır
imece --agents 8 --helpers 3       # sınırlar: ekip başına işçi, işçi başına yardımcı
```

- Yalın `imece` lideri tek başına başlatır. Lider, iş gerektirdikçe `hire` aracıyla işçi alır, en çok `--agents` kadar (0-16, varsayılan 4). Bir işçi aynı yolla yardımcı alır, bir anda en çok `--helpers` kadar (0-8, varsayılan 2).
- `hire` bir `tier` alır.
- **Bir işçinin başladığı yer.** Bir işçinin worktree'si HEAD'inizde başlar ve onu izler. Liderden başka bir noktada iş isteyin ("PR 41'i incele", "bunu release-0.2'de düzelt"), `from` ile işe alır: checkout'unuzun bir dalı, etiketi ya da commit'i, ya da `origin`'in bir pull request'i için `#41` (işçi başlarken çekilir). O worktree orada başlar ve orada kalır: HEAD'inizi asla izlemez, değişikliği o noktaya göredir ve onu checkout'unuza her değişiklik gibi uygularsınız (temiz oturmazsa üç yollu). Günlük `lead işe aldı: claude-2 (haiku), başlangıç noktası #41` der, satır `⎇ #41` gösterir ve nokta çalıştırmalar arasında saklanır. Yardımcıları aynı worktree'de çalışır.
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
 SALON
 ◆ lead            koordine ediyor
 ● claude-1        #12 parser: tokens      ⎇ ↑1 +34 −8   ▾ 1 yardımcı
   └ ● claude-1.1  claude-1 yardımcısı
 ○ claude-2         boşta
 ✓ claude-3        bitti, henüz bakılmadı
```

Sizi bekleyen işçiler önce gelir (takılan, sonra sizi bekleyen, sonra bitip bakılmamış: [durum merdiveni](../screen/salon.md#durum-merdiveni)); yardımcılar işçilerinin altında durur ve `h` `l` ile katlanır. Başlık filoyu sayar: `● 2 çalışıyor ✓ 1 bitti ✕ 1 takıldı`.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Claude Code](../concepts/claude-code.md): confine düzeyleri, bir oturum nedir.
- [Projeler](../concepts/projects.md): proje başına bir lider.
- [Salon](../screen/salon.md) ve [CLI başvurusu](../reference/cli.md).
