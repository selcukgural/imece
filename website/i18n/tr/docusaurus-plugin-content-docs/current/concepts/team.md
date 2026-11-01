---
title: Ekip
description: Bir imece ekibinde kim var - lider, işçiler, yardımcılar ve henüz yapılmamış araştırmacılar - lider kademelerle nasıl işe alır, kim lider olur, ajanlar nasıl adlandırılır.
---

# Ekip

Bir imece ekibi, sizinle konuşan ve iş gerektirdikçe işçi alan bir lider, görevlerinin parçaları için yardımcı alabilen işçiler ve (bir gün) araştırmacılardan oluşur. Ekip boyutunu siz seçmezsiniz; üst sınırları koyarsınız. Ekiple salonda, projeye göre gruplanmış olarak karşılaşırsınız.

## Roller

| Rol | Nerede çalışır | Bus'ta ne yapabilir |
| --- | --- | --- |
| Lider (`lead`) | Checkout'unuz, yalnız okuma | `send`, `post`, `board`, `run`, `hire` (işçiler) |
| İşçi (`claude-1`, `codex-2`, …) | HEAD'inizde detached kendi worktree'si | `send`, `post`, `board`, `run`, `claim`, `hire` (yardımcılar) |
| Yardımcı (`claude-1.1`, …) | İşçisinin worktree'si, işçisinin adını verdiği dosyalar | `send`, `post`, `board`, `run` |
| Solo (`solo`, `solo-notes`) | Kendi worktree'si, lideri olmayan bir projede | `send`, `run`, `hire` (yardımcılar) |
| Araştırmacı | henüz yapılmadı | — |

- **Lider** checkout'u okur, isteğinizi kartlara böler ve `send` (tür `assign`) ile dağıtır ya da herhangi bir işçinin alması için panoya asar. Hiçbir `--confine` düzeyinde dosya değiştirmez: checkout'unuzda çalışır, orada bir yazma incelemeyi atlar. Satıcısının okuma araçlarını ve `standard`'dan itibaren web'i alır.
- **Bir işçi** kendi worktree'sinde dosya değiştirir, işini `run` aracıyla denetler ve görevi kim verdiyse ona `done` gönderir. `done`'ı, incelemeniz için bir değişikliğe dönüşür ([değişiklikler](../concepts/changes.md)). İşçi başına bir anda bir kart.
- **Bir yardımcı**, işçi için liderin işçisi neyse odur. İşçi, kendi işinin yanında yürüyebilecek bir parça için, değiştirmediği dosyalarda bir yardımcı alır. Yardımcıya kart teklif edilmez, kimseyi işe almaz (yardımcının yardımcısı yok) ve `--agents` sınırına sayılmaz. İşçisine gönderdiği `done` ayrılışını işaretler; o tur bitince ayrılır, böylece turun maliyeti yine raporlanır. Günlük `işini bitirdi, ayrıldı` der. Duran bir işçi yardımcılarını da götürür.
- **Bir solo**, lideri olmayan bir işçidir: onu, seçtiğiniz satıcı, model ve çabada (`:solo`, `--solo`) kendi başına bir projede siz sürersiniz. Panosu ve kartı yoktur; bir yanıtla biten her turda worktree'sinin diff'ini size gösterir. Bir işçi gibi yardımcı alabilir ([solo oturumlar](solo.md)).
- **Araştırmacılar** planlanmış bir katmandır (soruları yanıtlayan sınırlı bir havuz, dereceli kirlilik ve güvenilir kaynaklarla). Yapılmadı; yalnız demo gösterir (`--researchers N`).

Satıcının kendi alt ajan aracı (Claude Code'un `Agent`/`Task`'ı, Codex'in `multi_agent`'ı, Gemini'nin `invoke_agent`'ı ve benzerleri) her düzeyde kapalıdır: CLI'nin içinde açılan bir alt ajan worktree'yi, bütçeyi, bus'ı ve ekranı atlardı. Yardımcılar imece'nin bu ihtiyaca yanıtıdır.

## İşe alma

```sh
imece                              # lider tek başına; gerektikçe işe alır
imece --agents 8 --helpers 3       # sınırlar: ekip başına işçi, işçi başına yardımcı
```

- Yalın `imece` lideri tek başına başlatır. Lider, iş gerektirdikçe `hire` aracıyla işçi alır, en çok `--agents` kadar (0-16, varsayılan 4). Bir işçi aynı yolla yardımcı alır, bir anda en çok `--helpers` kadar (0-8, varsayılan 2).
- `hire` bir `tier` ve bir `vendor` alır. Satıcı, işe alanın kendisininkidir; lider PATH'te bulunan her satıcıdan (`claude`, `codex`, `gemini`) işçi alabilir. Yalnız imece'nin adaptörü olan satıcılar işe alınabilir.
- Yeni ajan var olmadan önce bus'ta ona bir soket bağlanır. İşe alma, hemen yürüyebilecek iş içindir, önceden değil: bir işçi birçok görevi sırayla alır.
- Ajan başına bütçe payı yoktur; bütün ekip tek bütçeden harcar ([bütçe](../concepts/budget-and-limits.md)).

## Model değil, kademe

Lider bir işçi için kademe seçer, asla model adı değil. Her satıcı kademeleri kendi modellerine eşler:

| Kademe | Ne için | Claude | Codex | Gemini |
| --- | --- | --- | --- | --- |
| `cheap` | küçük, iyi tanımlı mekanik iş (yeniden adlandırma, bilinen davranışın testleri, dokümantasyon) | `haiku` | `@low` (varsayılan modeli düşük çabayla) | `flash` |
| `standard` (varsayılan) | sıradan düzeltmeler ve özellikler | satıcının kendi varsayılanı | varsayılan | varsayılan (`auto`) |
| `strong` | kodu enine kesen tasarım, zor hata ayıklama, inceleme | `opus` | `@high` | `pro` |

- Liderin kendisi satıcısının varsayılanında ya da `--model`'de çalışır.
- `--tier [V:]T=AD` bir kademenin anlamını değiştirir: `--tier strong=opus[1m]`, `--tier strong=opus@xhigh`, `--tier codex:strong=gpt-5.6-terra@xhigh` (bir değer `[MODEL][@ÇABA]`dır; çabalar satıcının kendi çabalarıdır, Gemini'de yoktur). `--model [V:]AD`, V satıcısının ajanlarının çalıştığı modeli belirler. Satıcı öneki olmadan ikisi de liderin satıcısından söz eder.
- **Geri düşme:** hesabınızda olmayan bir kademe modeli ilk turunda başarısız olur (Claude Code bunu başlayan ve sonra sıfır maliyetle hata veren bir oturum olarak bildirir). imece o işçiyi aynı istemlerle, bir kez, standart modelle değiştirir ve günlük `X modeli burada yok, varsayılanla devam` der. Tek deneme, asla döngü değil.

## Kim lider

Sessiz bir varsayılan yoktur.

1. `--lead SATICI` kazanır (`claude`, `codex`, `gemini`).
2. Yoksa belirlediğiniz birincil satıcı, PATH'te ve sürülebilirse.
3. Yoksa PATH'teki tek sürülebilir satıcı.
4. Yoksa salondan önce bir soru, KİM LİDER?: `j k` seçer, `enter` bu seferlik, `p` birincil yapar, `esc` çıkar.

`--lead ask` soruyu zorlar. `:lead SATICI` bir sonraki açılıştan itibaren birincili belirler; `:lead ask` unutturur. PATH'te olmayan bir birincil soru ekranında söylenir, sessizce değiştirilmez. Liderin satıcısı, yalın `--model` ve `--tier`'in de söz ettiği satıcıdır.

## Adlar

- imece'nin başladığı projenin lideri `lead`'dir; `:open YOL AD` ile açılan bir proje `lead-AD` alır ve `:rename AD` lideri projesiyle birlikte yeniden adlandırır.
- İşçiler `<satıcı>-N`'dir: `claude-1`, `claude-2`, `codex-1`, `gemini-1`. Sayı o satıcının işçilerini sayar, ayrılanlar dahil, ve hâlâ alınmış bir adı atlar.
- Yardımcılar `<işçi>.N`'dir: `claude-1.1`, `claude-1`'in ilk yardımcısıdır.
- Tek bir ajana `@ad mesaj` ile yazarsınız; `you`, bir ajanın size hitabıdır (`send(to: "you")`).

Ajanlar bus'ta birbirlerine bu adlarla seslenir; aynı projenin ekibinde olmayan bir ada `send`, ekip listesiyle reddedilir.

## Ekranda

```text
 SALON
 ◆ lead            koordine ediyor
 ● claude-1        #12 parser: tokens      ⎇ ↑1 +34 −8   ▾ 1 yardımcı
   └ ● claude-1.1  claude-1 yardımcısı
 ○ codex-1         boşta
 ✓ gemini-1        bitti, henüz bakılmadı
```

Sizi bekleyen işçiler önce gelir (onayınızı bekleyen, sonra takılan, sonra bitip bakılmamış); yardımcılar işçilerinin altında durur ve `h` `l` ile katlanır. Başlık filoyu sayar: `● 2 çalışıyor ✓ 1 bitti ✕ 1 takıldı`.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Mesajlar ve pano](../concepts/messages-and-board.md)
- [Satıcılar](../concepts/vendors.md): her kademe ne olarak çalışır, confine düzeyleri.
- [Projeler](../concepts/projects.md): proje başına bir lider.
- [Salon](../screen/salon.md) ve [CLI başvurusu](../reference/cli.md).
