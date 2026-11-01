---
title: Senaryolar
description: imece'de adım adım altı yürüyüş; her adımda basılan tuş ya da komut ve ekranın buna verdiği karşılık.
---

# Senaryolar

Kavram ve ekran sayfaları imece'nin her parçasının ne olduğunu anlatır. Senaryolar bu parçaları sıraya koyar: bir geliştirici elinde bir işle oturur ve her adımda basılan tuş ya da yazılan komut ile ekranın verdiği karşılık adıyla söylenir. imece'nin ne yaptığını bildiğinizde ve onunla bir günün nasıl geçtiğini görmek istediğinizde okuyun.

Her senaryo gerçek imece için doğrudur: tuşlar `?` yardımının listelediği tuşlar, günlük satırları ekranın kullandığı sözcükler, sayılar (sınırlar, fiyatlar, varsayılanlar) koddaki sayılardır. Çizili ekranlar kısa ve inandırıcıdır; gerçek bir çalışmanın kaydı değildir.

## Altı senaryo

| Senaryo | Tek satırla |
| --- | --- |
| [Kırık bir test, tek işçi](one-fix.md) | `/ düzelt`ten `!git commit`e: lider `claude-1`i işe alır, bir `⚑` sorusu, bir `Δ`, uygulama, önerilen commit mesajı. |
| [Codex lider, Claude ve Gemini işçi](mixed-vendors.md) | `--lead codex`, satıcılar arası işe alım, bir kademenin her satıcıda anlamı, dolara karşı token, başlıktaki kullanım sınırları, `--confine strict`. |
| [Bir değişikliği geri göndermek, sonra yarısını almak](review-with-notes.md) | `c` ve `n` ile notlar, işçinin `↩` ve `%` ile yanıtı, `x` ile seçilen parçalar, birini alıntılayan `>`, kalanı reddeden `r`. |
| [Üç çalışma kopyası, tek ekran](many-projects.md) | `:open`, katlama, `:pin`, `f`, `:rename`, `:copy .env`, `:close` ve her projenin çektiği tek bütçe. |
| [Geri dönmek](next-morning.md) | Tur ortasında çıkmak, "nerede kalmıştın" penceresi, uyuyan ajanlar, her satıcının nasıl devam ettiği, `:forget`, uzaktayken özeti, işaretler, `:export`. |
| [Takımın yanında kendi kabuğunuz](own-shell-and-commit.md) | `!cargo test`, `!vim`, ctrl-c, `:commit` ve tavanları, `:awake`, `:budget 80`, `:theme nord`, `L`. |

## Bedavaya denemek

Bir senaryonun ekranda yaptığı her şey `imece --demo` ile denenebilir: sahte ajanlar aynı modeli oynar; salon, SENİ BEKLEYEN, diff görünümü, notlar, seçilen parçalar, `:open`, `:pin`, `:copy`, işaretler ve `:export` aynı biçimde çalışır ve hiçbir şey ödetmez. Demonun gösteremediği, önceki bir çalışma ve bir kayıt dosyası isteyen kısımdır: "nerede kalmıştın" penceresi, yarım kalan turlar, baştan uykuda ajanlar. Başka ne ayrıldığını [demo sayfası](../getting-started/demo.md) söyler.

## Bir okuma sırası

1. [Kırık bir test, tek işçi](one-fix.md): bütün döngü bir kez, tek ajanla.
2. [Bir değişikliği geri göndermek, sonra yarısını almak](review-with-notes.md): inceleme tuşları derinlemesine.
3. [Takımın yanında kendi kabuğunuz](own-shell-and-commit.md): ajanların yanında kişinin kendi araçları.
4. [Codex lider, Claude ve Gemini işçi](mixed-vendors.md): PATH'te birden fazla satıcı varken.
5. [Üç çalışma kopyası, tek ekran](many-projects.md): birden fazla proje açıkken.
6. [Geri dönmek](next-morning.md): ikinci gün.

## Ayrıca bakın

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Klavye](../screen/keyboard.md)
- [Komutlar](../reference/commands.md)
