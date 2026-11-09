---
title: Senaryolar
description: imece'de adım adım sekiz yürüyüş; her adımda basılan tuş ya da komut ve ekranın buna verdiği karşılık.
---

# Senaryolar

Kavram ve ekran sayfaları imece'nin her parçasının ne olduğunu anlatır. Senaryolar bu parçaları sıraya koyar: bir geliştirici elinde bir işle oturur ve her adımda basılan tuş ya da yazılan komut ile ekranın verdiği karşılık adıyla söylenir. imece'nin ne yaptığını bildiğinizde ve onunla bir günün nasıl geçtiğini görmek istediğinizde okuyun.

Her senaryo gerçek imece için doğrudur: tuşlar `?` yardımının listelediği tuşlar, günlük satırları ekranın kullandığı sözcükler, sayılar (sınırlar, fiyatlar, varsayılanlar) koddaki sayılardır. Çizili ekranlar kısa ve inandırıcıdır; gerçek bir çalışmanın kaydı değildir.

## Sekiz senaryo

| Senaryo | Tek satırla |
| --- | --- |
| [Kırık bir test, tek işçi](one-fix.md) | `/ düzelt`ten `!git commit`e: lider `claude-1`i işe alır, bir `⚑` sorusu, bir `Δ`, uygulama, önerilen commit mesajı. |
| [Bir değişikliği geri göndermek, sonra yarısını almak](review-with-notes.md) | `c` ve `n` ile notlar, işçinin `↩` ve `%` ile yanıtı, `x` ile seçilen parçalar, birini alıntılayan `>`, kalanı reddeden `r`. |
| [Üç çalışma kopyası, tek ekran](many-projects.md) | `:open`, katlama, `:pin`, ana ekranı tek projeye daraltan `[ ]`, `:rename`, `:copy .env`, `:close` ve her projenin çektiği tek bütçe. |
| [Takımın yanında bir solo](solo-beside-the-team.md) | `:solo claude:opus@xhigh`, lidersiz, kendinizin sürdüğü bir oturum, her yanıttan sonra `Δ`, başka bir modelde ikinci bir solo, ertesi sabah `--solo`. |
| [Geri dönmek](next-morning.md) | Tur ortasında çıkmak, "nerede kalmıştın" penceresi, uyuyan ajanlar, bir oturumun nasıl devam ettiği, `:forget`, uzaktayken özeti, `:notify`, işaretler, `:export`. |
| [Yeni bir epic](new-epic.md) | `:archive v1 fixes`, beklemesi ve `u`, aynı adla temiz bir proje, arşivin sakladıkları, geri getirmek için `:restore v1 fixes`, tek bütçe. |
| [Takımın yanında kendi kabuğunuz](own-shell-and-commit.md) | `!cargo test`, `!vim`, ctrl-c, `:commit` ve tavanları, `:awake`, `:budget 80`, `:theme nord`, `L`. |
| [Bir kart ters gidince](when-a-card-goes-wrong.md) | Başka bir kartı bekleyen kart (`⧗`), başarısız bir kart ve denemeleri, üç denemeden sonra sigorta (`a` bir daha, `r` başarısız say), geri çekilen kart, lidere bir sağlık bildirimi, asılı bir komutu bitiren `:ps`, `:ledger`. |

## Bedavaya denemek

Bir senaryonun ekranda yaptığı her şey `imece --demo` ile denenebilir: sahte ajanlar aynı modeli oynar; ana ekran, KARAR BEKLEYEN, bir değişikliğin ayrıntısı, notlar, seçilen parçalar, `:open`, `:pin`, `:copy`, işaretler ve `:export` aynı biçimde çalışır ve hiçbir şey ödetmez. Demonun gösteremediği, önceki bir çalışma ve bir kayıt dosyası isteyen kısımdır: "nerede kalmıştın" penceresi, yarım kalan turlar, baştan uykuda ajanlar. Başka ne ayrıldığını [demo sayfası](../getting-started/demo.md) söyler.

## Bir okuma sırası

1. [Kırık bir test, tek işçi](one-fix.md): bütün döngü bir kez, tek ajanla.
2. [Bir değişikliği geri göndermek, sonra yarısını almak](review-with-notes.md): inceleme tuşları derinlemesine.
3. [Takımın yanında kendi kabuğunuz](own-shell-and-commit.md): ajanların yanında kişinin kendi araçları.
4. [Üç çalışma kopyası, tek ekran](many-projects.md): birden fazla proje açıkken.
5. [Takımın yanında bir solo](solo-beside-the-team.md): tek bir ajanı kendiniz sürmek istediğinizde.
6. [Geri dönmek](next-morning.md): ikinci gün.
7. [Yeni bir epic](new-epic.md): bir iş bittiğinde.
8. [Bir kart ters gidince](when-a-card-goes-wrong.md): başarısız denemeler, bekleyen kartlar, takılan bir işçi, neye mal olduğu.

## Ayrıca bakın

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Klavye](../screen/keyboard.md)
- [Komutlar](../reference/commands.md)
