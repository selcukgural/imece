---
title: Solo oturumlar
description: Lidersiz, kendinizin sürdüğü bir oturum - :solo ve --solo, seçtiğiniz satıcı, model ve çaba, kendi worktree'si, her yanıttan sonra Δ, yardımcılar, git'siz bir klasör ve soloların takımlarla bir checkout'u nasıl paylaştığı.
---

# Solo oturumlar

Takım, imece'nin alışılmış biçimidir: bir lider checkout'unuzu okur, işçi alır, onların değişiklikleri incelemeniz için size gelir. **Solo** öteki biçimdir: lidersiz ve takımsız, adını verdiğiniz satıcı, model ve çabada, kendinizin sürdüğü tek bir ajan. imece'yi değerli kılanı (worktree, `Δ` kapısı, bütçe, salon, depo) korur; tek bir oturumun gerek duymadığını (pano, kartlar, işçi alma) dışarıda bırakır. Takımların yanında istediğiniz kadar açın: takım, solo, takım, solo, solo; her biri salonda bir grup.

## Açmak

| Nasıl | Ne yapar |
| --- | --- |
| `:solo [PATH] [CHOICE] [NAME]` | ekrandan, imece çalışırken. Sözcükler herhangi bir sırada gelebilir: içinde `/` olan ya da `~` veya `.` ile başlayan sözcük checkout'un yoludur (varsayılan: eldeki projeninki); bir satıcı adıyla başlayan sözcük seçimdir; kalan sözcük projenin adıdır (varsayılan: dizinin adı, alınmışsa `-2`, `-3` ile numaralı). Seçimsiz, bir ekran `j k enter` ile satıcıyı, modeli ve çabayı sorar. |
| `:effort DÜZEY`, `:model AD` | eldeki solonun sonraki turundan itibaren üzerinde çalıştığını değiştirir; `default` satıcının kendisidir. Oturumu aynı transkript üzerinde yeniden başlar. |
| `imece --solo V[:MODEL][@EFFORT]` | komut satırından: imece checkout'ta takım yerine bir soloyla başlar. Aynı satıcı ve seçimle açık bir solo sürdürülür, böylece her sabah aynı komut aynı oturumu sürdürür; farklı bir seçim yanına başka bir solo açar. `--lead` yoksa `:open`'ın eklediği takımlara solonun satıcısı liderlik eder, KİM LİDER? sorusu gelmez. |

**Seçim** `SATICI[:MODEL][@ÇABA]` biçimindedir, satıcıdan sonraki her parça isteğe bağlı:

| Seçim | Anlamı |
| --- | --- |
| `claude` | Claude Code, kendi modeli ve çabasıyla (ya da `--model`'in ona verdiğiyle) |
| `claude:opus` | `opus` takma adı |
| `claude@xhigh` | varsayılan modeli, `xhigh` çabayla düşünen |
| `claude:opus@max` | ikisi de |
| `codex:gpt-5.6-terra@high` | Codex, o modelde `high` akıl yürütme çabasıyla |
| `gemini:pro` | Gemini CLI `pro`da; Gemini'de çaba ayarı yoktur, `gemini:pro@high` reddedilir |

Çabalar satıcının kendi çabalarıdır: Claude Code'un `low`, `medium`, `high`, `xhigh`, `max`; Codex'in `low`, `medium`, `high`, `xhigh`. Satıcının olmayan bir çaba, bir şey başlamadan önce sözcüklerle reddedilir: `claude has no effort "bogus"; one of low, medium, high, xhigh, max` ([komut satırı](../reference/cli.md)). PATH'te olmayan bir satıcı da reddedilir.

Flaş neyin açıldığını söyler: `notes solo oturum olarak açıldı (~/src/kule); solo-notes adına yaz (claude opus@xhigh)`. Yeni solo seçilidir; sonra yazdığınız ona gider.

## Solo nedir

- **Kendi başına bir proje.** Salon onu tek ajanlı bir grup olarak gösterir: `solo-AD` (imece'nin içinde başladığı projeyse `solo`). Satırı, liderin satırının satıcıyı adlandırdığı yerde satıcıyı ve seçimi adlandırır: `○ solo-notes  claude opus@xhigh  idle`. `:close`, `:rename` (ajan `solo-YENİAD` olur), `:pin` ve `:copy` her projede olduğu gibi çalışır.
- **Kendi worktree'si.** Bir işçi gibi solo da checkout'un imece'nin durum dizini altındaki bir git worktree'sinde yazar, asla checkout'unuzda değil. Bir checkout'taki iki solo ve kendi düzenlemeleriniz hiç çakışmaz. Worktree projenin kimliğiyle adlandırılır (`solo-3`), bu yüzden projeyi yeniden adlandırmak onu korur. Satırı, bir işçininki gibi worktree'nin nerede durduğunu gösterir (`⎇ dal ↑kendi ↓checkout +N −M`).
- **İşçinin araçları.** Confine kadranı altında solo işçinin aldığını alır: `standard`'da okuma, yazma, kabuk, web arama ve getirme; `strict`'te dosya araçları ve imece'nin `run`'ı; `open`'da kendi kurulumunuz ([satıcılar](vendors.md)). Bus'ta `send` (size ve yardımcılarına), `run` ve yardımcılar için `hire` vardır. `post`, `board` ya da `claim` yok: pano bir takımındır.
- **Her yanıttan sonra Δ.** Solo için kimse `done` göndermez. Size bir yanıtla biten her turda imece worktree'sini okur; ağaç HEAD'inizden farklıysa değişiklik, yanıtın ilk satırıyla adlandırılmış bir `Δ` olarak SİZİ BEKLEYENLER'de durur; `a` checkout'unuza uygular, `r` reddeder, `c` ve `n` not gönderir, `x` parça seçer; tıpkı bir işçide olduğu gibi ([değişiklikler](changes.md)). Daha yeni bir yanıtın değişikliği eskisinin yerini alır. Söyleyecek bir şeyi olmayan bir tur (bir `/komut`) hiçbir şey okumaz.
- **Yardımcılar.** Solo, bir işçi gibi `--helpers`'a kadar yardımcı alabilir (`solo-notes.1`): kendi işinin yanında yürüyebilecek bir parça için worktree'sinde bir kodlama ajanı. Onlara, kişiyle tek başına çalışan bir ajan için çalıştıkları söylenir.
- **Tek bütçe.** Solonun turları her takımla aynı bütçeden ödenir ve satıcısının kullanım penceresi her oturumunki gibi başlıkta görünür ([bütçe ve sınırlar](budget-and-limits.md)).
- **Hatırlanır.** Solo, oturumu ve seçimi her ajan gibi depodadır: uykuda geri gelir, nerede kaldığı anlatılır ([oturumlar](sessions.md)).

## Git'siz bir klasörde

Solo, git checkout'u olmayan bir klasörde ya da `git init` yapılmış ama henüz hiçbir şey commit'lenmemiş bir klasörde de çalışır: ilk dosyasından önceki yeni bir proje ya da git'siz tuttuğunuz bir klasör. Orada **doğrudan klasörde** çalışır: worktree yoktur, yani yazdığı anında dosyalarınızdadır, ve yanıtlarından sonra `Δ` gelmez (karşılaştırılacak bir taban, uygulanacak bir şey yoktur). Yardımcıları da orada çalışır. Projenin başlığı bunu uyarı renginde söyler, `▾ notes  ⌀ git yok`, ve günlük bunu açılışta ve her başlangıçta söyler: `solo-notes doğrudan ~/src/notes içinde çalışıyor: burada git yok, düzenlemeleri Δ olarak incelenmez (ilk commit'ten sonra bir sonraki açılışta kendi worktree'si olur)`. Ajana da aynısı söylenir, ve `git init` ya da commit'i yalnız siz isteyince yapması. Böyle bir klasörde git'ten hiçbir şey çalışmaz: satırında worktree durumu yok, dosya tamamlama yok, `:commit` `this folder is not a git checkout with a commit` der, `:copy`'nin kopyalayacağı bir yer yoktur. Git'in kurulu olması bile gerekmez.

Klasörün ilk commit'i olunca (ajan siz isteyince yapabilir: `git init`, sonra bir commit), bir sonraki başlangıç ya da soloyu kapatıp yeniden açmak ona her solo gibi bir worktree ve `Δ` verir.

Bir **takım** böyle bir klasörde başlamaz: orada `imece` şunu der: `~/src/notes is not a git checkout with a commit: a team works in git worktrees. A solo works here without git (--solo VENDOR, :solo); or run git init && git commit --allow-empty -m init first`, `:open` da aynısını der. Git'ini kaybetmiş bir klasördeki kayıtlı takım bir sonraki başlangıçta kapanır. Takımları git'siz çalıştırmak [issue #162](https://github.com/selcukgural/imece/issues/162).

## Bir checkout'ta solo ve takım

Bir checkout'ta en çok bir açık **takım** (ikinci bir `:open` `that checkout is open already, as kule` der) ve istendiği kadar **solo** bulunur. Bir projede zaten varsa dizinin adı numara alır: `kule`, `kule-2`, `kule-3`. Kapalı bir soloyu o checkout'taki sonraki solo kullanır, bir takım asla; tersi de öyle.

Çıplak bir `imece`, checkout'ta açık takım yoksa, sololar olsa bile bir takım açar: sololar takımın yerini tutmaz. `imece --solo …` bir solo açar ya da sürdürür ve açık bir takımı olduğu gibi, siz ona yazana dek uykuda bırakır.

## Henüz olmayanlar

- Yardımcılarından biri tur ortasındayken solonun yanıtı (bir liderinki gibi) hikâyesinde kalır ve sonraki yanıtla birlikte, `Δ`sıyla size gelir.

## Bkz.

- [Takım](team.md): roller ve solonun işçiyle paylaştığı
- [Projeler](projects.md): salonda gruplar, `:close`, `:rename`, `:pin`
- [Değişiklikler](changes.md): `Δ`, uygulama, reddetme, notlar
- [Komut satırı](../reference/cli.md): `--solo`, `--model`, `--tier`
- [Komutlar](../reference/commands.md): `:solo`
- [Takımın yanında bir solo](../scenarios/solo-beside-the-team.md): içinden bir yürüyüş
