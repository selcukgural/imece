---
title: "Takımın yanında bir solo"
description: "Takımın yanında kendinizin sürdüğü bir oturum açmak: model ve çabayla :solo, her yanıttan sonra Δ, başka bir modelde ikinci bir solo, yardımcılar ve ertesi sabah --solo."
---

# Takımın yanında bir solo

Bazen sizinle iş arasında bir lider istemezsiniz: tek bir ajan, seçtiğiniz model ve düşünce derinliği, ne yapılacağını siz söylersiniz. **Solo** imece'nin içinde tam budur: worktree'yi, `Δ` incelemesini, bütçeyi ve ekibi korur; panoyu ve işe almayı dışarıda bırakır. Bu senaryo çalışan bir takımın yanında bir solo, sonra başka bir modelde ikincisini açar ve ertesi gün ona geri döner.

imece her zamanki gibi `~/code/kule` içinde başlatıldı: takım `lead` ve işe aldığı iki işçi.

## 1. Bir solo aç

Model ve çabayı tek sözcükte yazarak komutu verin:

```text
 › :solo claude:opus@xhigh notes
```

Sözcükler herhangi bir sırada gelebilir; imece onları biçimlerinden ayırır. `claude:opus@xhigh` `claude` ile başlar, bu yüzden seçimdir: Claude Code `opus` takma adında, `xhigh` çabayla düşünerek. `notes` ne bir yol (`/`, `~` ya da `.` yok) ne de bir seçim, bu yüzden projenin adıdır. Yol yoksa eldeki projenin checkout'u: `kule`. Flaş yanıtlar:

```text
 notes solo oturum olarak açıldı (/Users/me/code/kule); solo-notes adına yaz (claude opus@xhigh)
```

Ana ekranın ÇALIŞAN kısmı takımın yanında tek ajanlı yeni bir proje gösterir ve o seçilidir:

```text
 ÇALIŞAN 4 · boşta 2
 ▾ kule          ░░░░░░░░ 0/2  2 çalışıyor  $0.42  ~/code/kule
 ● kule        #3   limiter: config v2        src/limit.rs düzenliyor · 2 dk
 ○ kule        #4   limiter tests             boşta
 ● kule             lead                      koordine ediyor
 ▾ notes         0 çalışıyor  $0.00  ~/code/kule
▸○ notes            solo-notes                boşta
```

Solonun kartı yoktur, bu yüzden satırı adını taşır; seçimi flaşta ve `:` menüsünde `:model` ile `:effort` notlarındadır. Simge durumudur, her satırda olduğu gibi: solo çalışır, eşgüdüm yapmaz. Henüz hiçbir şey para etmiyor: her ajan gibi, siz yazana dek süreci yoktur.

## 2. Ne yapacağını söyle

`/`'ye basın. Komut satırı seçili projenin başına yazar; burada bu solonun kendisidir:

```text
 › solo-notes'a: docs/limits.md'yi yaz: hız sınırlayıcı ne yapar, src/limit.rs'den
```

İlk turu başlar. Önce imece `solo-notes`a `~/.local/state/imece/worktrees/` altında, projenin kimliğinden `solo-3` adlı, HEAD'inizde detached bir `kule` worktree'si açar. Solo, `--confine standard` altında bir işçinin aldığını alır: Claude Code'un okuma ve düzenleme araçları, kabuğu (soracak olan size `⚑` olarak gelir), web ve imece'nin `run` aracı. Satırı ne yaptığını gösterir:

```text
 ● notes            solo-notes                src/limit.rs okuyor · 1 dk
```

## 3. Yanıt ve onunla gelen Δ

Tur bir yanıtla biter. Her ajanın yanıtı gibi günlüğe ve KARAR BEKLEYEN'e gider ve bu bir solo olduğu için imece aynı anda worktree'sini okur:

```text
 KARAR BEKLEYEN 1 soru · 1 değişiklik
 sorular
▸! notes            solo-notes                yanıtladı: docs/limits.md'yi src/limit.rs'den
                                              yazdım: iki pencere, anahtar başına…
 değişiklikler
 ! notes            docs/limits.md'yi src/l…  hiçbir şey koşmadı · 1 dosya +48 −0
```

Solo için kimse `done` göndermez. Bir yanıtla biten her tur, yanıtın ilk satırıyla adlandırılmış olarak worktree'sinin tuttuğunu size gösterir. `Δ` üzerinde `enter` diff'i açar; `a` checkout'unuza uygular, `r` reddeder ve gerekçeleriniz için `@solo-notes ` açar, `c` ve `n` satırlara not gönderir, `x` parça seçer ([değişiklikler](../concepts/changes.md)). Dosyaya dokunmadan bir soruyu yanıtlayan bir tur aynı ağacı yeniden okur: `Δ` kalır, adı yenilenir.

Diff'i okur ve uygulamak yerine bir not gönderirsiniz:

```text
 › @solo-notes patlama penceresi 60 değil 10 s; düzelt ve girişi kısalt
```

Sonraki yanıt, düzeltmeyle sonraki `Δ`yı getirir. `a` uygular. Günlük `solo-notes'un değişikliğini uyguladın` der ve worktree işaretlenir, böylece sonraki değişikliği yalnız sonra geleni tutar.

## 4. Başka bir modelde ikinci bir solo

Başka bir soru, başka bir model. Bu kez bir yol ve ad yok:

```text
 › :solo claude:sonnet@high ~/code/kule
```

`~/code/kule` bir yoldur (içinde `/` var), seçim `sonnet`i `high` çabasında adlandırır ve ad varsayılan olarak dizininkidir: `kule` takımda olduğundan proje `kule-2`, ajanı `solo-kule-2` olur:

```text
 imece  kule · notes · kule-2 │ $4.10 / $50.00  16:20
 ÇALIŞAN 4 · boşta 4
 ▾ kule          ░░░░░░░░ 0/2  0 çalışıyor  $3.52  ~/code/kule
 ○ kule        #3   limiter: config v2        boşta
 ○ kule        #4   limiter tests             boşta
 ▾ notes         0 çalışıyor  $0.58  ~/code/kule
 ○ notes            solo-notes                boşta
 ▾ kule-2        0 çalışıyor  $0.00  ~/code/kule
▸○ kule-2           solo-kule-2               boşta
```

Bir checkout'ta üç grup: bir takım ve iki solo, her biri kendi worktree'sinde, checkout'unuza hiçbiri dokunmuyor. Hepsini tek bütçe öder; başlıktaki bütçe ve `:budget` notundaki hız onları birlikte sayar. Aynı checkout'ta ikinci bir *takım* reddedilir (`that checkout is open already, as kule`): checkout başına bir takım, istendiği kadar solo.

Çıplak bir `:solo` tahmin etmez, sorar: bir ekran modelleri, sonra çabaları listeler; `j k` ve `enter` seçer, `esc` bir adım geri gider. Çalışan bir solo da yeniden ayarlanır: `:effort max`, `solo-notes`u sonraki turundan itibaren `max`ta düşündürür (flaş: `solo-notes sonraki turundan itibaren claude opus@max üzerinde; oturumu onunla yeniden başlar`), `:model sonnet` kalan küçük düzeltmeler için onu daha ucuz bir modele taşır; oturum aynı transkript üzerinde yeniden başlar, konuşmadan hiçbir şey yitmez.

imece'nin yerine getiremeyeceği bir seçim, bir şey başlamadan reddedilir. `:solo claude@turbo` `açılamadı: claude has no effort "turbo"; one of low, medium, high, xhigh, max`.

## 5. Bir yardımcı

Solo da işini bir işçi gibi bölebilir. Testleri de eklemesi söylenince `solo-notes` `hire`'ı çağırır ve günlük söyler:

```text
 15:02 → solo-notes solo-notes.1'i işe aldı (varsayılan model)
 15:02 ⇄ solo-notes → solo-notes.1 [assign]: tests/limit.rs'de patlama penceresi testleri
```

Yardımcı solonun worktree'sinde, solonun adını verdiği dosyalarda çalışır ve `done` turu bitince ayrılır; satırı bir işçinin yardımcısı gibi solonun altında asılıdır. Aynı anda en çok `--helpers` (varsayılan 2). Solonun sahip olmadığı şey panodur: `post`, `board` ya da `claim` yok; takımın kartlarından hiçbiri ona, onun işinden hiçbiri takıma ulaşmaz.

## 6. Ertesi sabah

`q q` ile çıkın. Sololar her ajan gibi, oturumları ve seçimleriyle depodadır. Yarın:

```sh
cd ~/code/kule
imece --solo claude:opus@xhigh
```

`--solo` bu checkout'ta aynı seçimle açık bir solo arar, `notes`u bulur ve onu eldeki proje yapar: aynı oturum, nerede kaldığı anlatılarak sürer. Takım ve `kule-2` yanında uykuda geri gelir, siz onlara yazana dek. `--solo claude:sonnet` yazsaydınız üçüncü bir solo açılırdı, `kule-3`. Çıplak `imece` yazsaydınız eldeki proje takım olurdu: sololar onun yerini tutmaz.

## Bu senaryonun kullandıkları

| Tuş ya da komut | Ne yaptı |
| --- | --- |
| `:solo claude:opus@xhigh notes` | eldeki checkout'ta, Claude'un opus'unda xhigh çabayla, `notes` adlı bir solo |
| `:solo claude:sonnet@high ~/code/kule` | dizinin adıyla ve bir numarayla ikinci bir solo |
| `/`, `@solo-notes` | soloya yazmak, lidere yazmak gibi |
| `Δ`, `enter`, `a`, `r`, `c`, `n`, `x` | her yanıttan sonraki değişiklik, bir işçininki gibi incelenir |
| `imece --solo claude:opus@xhigh` | ertesi sabah: aynı solo, sürdürülür |

## Bkz.

- [Solo oturumlar](../concepts/solo.md)
- [Değişiklikler](../concepts/changes.md)
- [Komut satırı](../reference/cli.md): `--solo`
- [Üç çalışma kopyası, tek ekran](many-projects.md): yan yana takımlar
