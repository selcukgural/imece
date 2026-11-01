---
title: Güvenlik
description: "imece neyi korur - bus'ta kim hangi aracı çağırabilir, ekranda ajan metni, fail-closed oturum denetimleri, bütçe ve Δ kapısı - ve neyi korumaz: bir sandbox değildir. Bir sorun nasıl bildirilir."
---

# Güvenlik

imece, Claude Code, Codex ve Gemini CLI'yi kendi makinenizde kendi hesaplarınızla sürer. Kendi sınırlarını korur ve sahip olmadığı sınır konusunda dürüsttür: bir sandbox değildir. Bu sayfa onu nerede çalıştıracağınıza ve ne bekleyeceğinize karar vermeniz içindir.

## imece neyi korur

Yerleşik kurallar: varsayılan ret, hiçbir şey sessizce zayıflamaz, her sınırın onsuz başarısız olan bir testi var.

### Bus'ta kim hangi aracı çağırabilir

- Bus imece'nin kendi MCP sunucusudur; taze `0700` bir dizinde ajan başına bir Unix soketi. **Gönderen sokettir**, ajanın verdiği bir ad asla: hiçbir ajan bir başkası ya da siz gibi konuşamaz.
- Her soket yalnız rolünün araçlarını sunar: `claim` yalnız işçilere, `hire` lidere (işçiler) ve işçilere (yardımcılar), `approve` yalnız bir Claude oturumuna, o da kendisi çağırır. Bir yardımcı kimseyi işe almaz. Bir oturumun `init`'i tam imece'nin sunucusunu ve tam araçlarını bildirmelidir.
- Bir yanıttaki `re` varsayılan retle denetlenir: var olan, gönderene gönderilmiş, yanıtın gittiği kişinin yazdığı bir mesaj. Size iş verecek türlerde size mesaj reddedilir.
- Her yazmada sınır: gövde boyutu, okunmamış posta, siz yeniden konuşana dek ajan başına mesajla başlayan tur ve asma, proje başına açık kart ([mesajlar](../concepts/messages-and-board.md)).

### Ekranda ajan metni

Bir ajanın yazdığı her şey terminale yalnız `i18n::fill` ya da `i18n::clean` üzerinden ulaşır: kontrol karakterleri değiştirilir (bir kaçış dizisi ekranı yeniden çizemez, pencereyi yeniden adlandıramaz ya da bir satırı taklit edemez) ve gizli anahtara benzeyen sözcükler `[***]` olarak saklanır (`sk-…`, `ghp_…`, `github_pat_…`, `API_KEY=`, `TOKEN=`, `SECRET=`, `PASSWORD=`). Kural diff'ler, alıntılanan parçalar, notlar, dışa aktarılan Markdown, uyanış özeti ve `:commit` önerisi için de geçerlidir. Bir onay isteği bütünüyle gösterilir ve asla markdown olarak okunmaz: izin verdiğiniz, çalışandır.

### Fail-closed oturum denetimleri

imece her satıcının kendi kısıtlamalarını rol ve `--confine` düzeyi için açar ve oturumun neyle çalıştığını bildirdiğini doğrular: Claude'un `init`'i her turda; Codex'in yapılandırma yansıması, eklentileri, hook'ları, MCP sunucuları (her turdan önce yeniden) ve thread'i; Gemini'nin modu, modeli, `/about`, `/extensions list` ve `/memory list`'i. İstenenden başka her şey, bir araç, bir sunucu, bir hook, bir sandbox, oturumu durdurur; hiçbir şey sessizce düşürülmez. Alt ajan araçları her düzeyde kapalıdır ([satıcılar](../concepts/vendors.md)).

### Bütçe

Bütün ekip için tek bütçe, depoda saklı, uyandıran bir mesaj ya da kart teklifi iletilmeden önce denetlenir; Claude oturumları kendi sert tavanını taşır; imece'nin kendi ücretli çağrısı (`:commit`) tavanını çalışmadan önce ayırır ve günlük bir tavanı vardır ([bütçe](../concepts/budget-and-limits.md)).

### Δ kapısı

Hiçbir ajan checkout'unuza yazmaz. Lider orada yalnız okur; işçiler worktree'lerde çalışır; bir değişiklik checkout'unuza yalnız siz uygulayınca, stage'e ulaşır ve oturmayan bir değişiklik bütünüyle reddedilir. Rutin olmayan komutların onayları ve ajanlardan sorular sizi bekler ([değişiklikler](../concepts/changes.md), [onaylar](../concepts/approvals-and-questions.md)).

### Daha küçük kurallar

- Alt süreçler temizlenmiş bir ortam (satıcının oturum açmak için gerekenleri artı), kendi süreç grubunu ve sınırlı borular alır.
- `run` aracı argümanlarıyla tek bir program alır, asla kabuk değil; ondan geçen git komuttan önce seçenek, pager, çıktı dosyası almaz.
- Bir oturum devam ettirilir ya da unutulurken bir yola ya da satıcıya yalnız bir UUID ulaşır.
- `:export` var olan bir dosyanın üzerine asla yazmaz; `:copy` checkout'tan çıkan ya da bağlantı olan yolları reddeder.
- Depo `0600`'dür; imece'nin okuyamadığı bir dosya reddedilir, üzerine yazılmaz.
- Deponun `AGENTS.md`/`CLAUDE.md`'si ajanlara 16 KiB'da kesilerek gider ve günlük kesildiğinde söyler.

## imece neyi yapmaz

:::warning

imece **bir sandbox değildir** ve kendisine asla öyle demez. `--confine standard` (varsayılan) altında bir işçinin satıcısının kabuğu vardır ve kabuğu olan bir ajan hesabınızın yapabildiğini yapabilir: ev dizininizi okur, ağa ulaşır, sizin çalıştırabileceğiniz her şeyi çalıştırır. Onu sınırlayan, satıcının kendi kısıtlamasıdır (Claude'un `--restricted`'ı ve izin istemleri, Codex'in sandbox'ı, Gemini'nin politika dosyası); imece bunları açar ve doğrular. Bunun ötesinde yalıtım bir konteynerin işidir. Kod ya da görev güvenebileceğiniz bir şey değilse imece'yi bir konteynerin içinde çalıştırın.

:::

- **Liderin yazmaması bir roldür, güvenlik değil.** Checkout'unuzda çalışır çünkü oraya bir yazma Δ kapısını atlar ve çalışma kopyanızla çakışır; fail closed doğrulanan bir istem ve araç listesidir, bir işletim sistemi sınırı değil.
- **Son kapı sizsiniz**, bir model ya da araç listesi değil: Δ incelemesi, `⚑` istekleri ve SENİ BEKLEYEN'deki `?` soruları kararların verildiği yerdir.
- `open` altında kendi ayarlarınız, hook'larınız, MCP sunucularınız ve uzantılarınız yüklenir; imece orada tasarım gereği daha az denetler.
- `run`'ın kendi ağ engeli yoktur. Salt okunur sandbox bir Codex liderinin bütün diski okumasına izin verir. `~/.gemini/GEMINI.md`'niz ve kullanıcı düzeyi hook'larınız Gemini için yine geçerlidir. Claude Code'un kendi salt okunur kuralları neyin imece'ye istek olarak hiç ulaşmayacağına karar verir.
- Bir ajanın bağlamındaki sırlar satıcının ve modelindir; imece ekranında ve yazdıklarında sır gibi görüneni saklar, daha fazlasını değil.

## Bir sorunu bildirmek

Lütfen bir güvenlik sorunu için herkese açık issue, pull request ya da tartışma açmayın. GitHub üzerinden özel olarak bildirin: deponun Security sekmesi, "Report a vulnerability". Etkilenen crate'i ve commit'i, bir saldırganın neyi hangi koşullarda yapabildiğini (ele geçirilmiş bir ajan, kötü niyetli bir depo, yerel bir kullanıcı) ve yeniden üreten adımları ya da bir testi ekleyin. Bir hafta içinde yanıt almalısınız; düzeltme, anonim kalmayı yeğlemezseniz sizi anan bir duyuruyla yayımlanır.

Özellikle ilgi çeken: terminale temizlenmeden ulaşan ajan metni, bus'ta bir ajanın bir başkasını ya da sizi taklit etmesi, imece fark etmeden imece'nin istediğinden fazlasıyla çalışan bir oturum, incelemenizi aşarak checkout'a ulaşan bir işçi değişikliği ve ekranda, günlüklerde ya da bir modelin bağlamında biten sırlar.

## Ayrıca bakınız

- [imece nasıl çalışır](../concepts/how-imece-works.md)
- [Satıcılar](../concepts/vendors.md): confine kadranı ve satıcı başına denetimler.
- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [Değişiklikler](../concepts/changes.md)
