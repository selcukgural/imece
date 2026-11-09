---
title: Claude Code
description: Bir ajan oturumu nedir, --confine kadranı Claude Code'dan ne kadarını verir, imece fail-closed neyi denetler, Claude Code'un kendi komut satırı ve kullanım limitleri.
---

# Claude Code

imece PATH'te `claude`'u arar ve onu sürer. Bir ajan, uzun ömürlü tek bir Claude Code oturumudur; imece Claude Code'un kendi kısıtlamalarını ajanın rolü ve `--confine` düzeyi için açar ve oturumun tam olarak onunla çalıştığını fail closed denetler. Bu sayfa bir oturumun ne olduğunu ve neyin denetlendiğini söyler.

## Confine kadranı

`--confine standard|strict|open` her ajanın Claude Code'dan ne kadarını aldığını söyler. Her düzeyde lider asla yazmaz ve Claude Code'un kendi alt ajan aracı kapalı kalır.

| Düzey | İşçiler ve yardımcılar | Lider | Kendi Claude Code kurulumunuz |
| --- | --- | --- | --- |
| `standard` (varsayılan) | Claude Code'un tam kodlama araç takımı: okuma, yazma, kabuk, web arama ve getirme | okuma araçları ve web | yüklenmez |
| `strict` | yalnız dosya araçları; komutlar imece'nin `run` aracından; hiçbir şey sormaz, soracak olan reddedilir | okuma araçları | yüklenmez |
| `open` | Claude Code sizin kurduğunuz gibi (ayarlar, hook'lar, MCP sunucuları, skill'ler), alt ajan aracı hariç | okuma listesi | yüklenir |

imece hiçbir şeyi sandbox'lamaz: Claude Code'un kendi kısıtlamalarını açar ve doğrular; yalıtım bir konteynerin işidir. Günlük strict olmayan her başlangıçta bunu bir kez söyler.

## Bir oturum

Bir oturum, ajan başına bir, uzun ömürlü bir `claude -p` akışıdır (stream-json).

- **Standard ve strict** `--restricted` modunda çalışır: kullanıcı ya da proje ayarı yok (dolayısıyla hook yok), imece'ninki dışında MCP sunucusu yok, sizin komutlarınız yok, dosya erişimi ajanın dizinine sınırlı. Araçlar tam adlarıyla anılır: okuyan için `Glob`, `Grep`, `Read`; yazan için artı `Edit`, `Write`; standard'dan itibaren herkese `WebFetch`, `WebSearch` ve yazanlara `Bash`; görev araçları (`TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`: agent'ın kendi yapılacaklar listesi, dosya değiştirmez) her seviyede herkese. Görev araçları olmayan bir Claude Code çalışmaya devam eder; iz bunu bir kez söyler. Web'e doğrudan izin verilir (yalnız okur); Claude Code'un soracağı bir kabuk komutu imece'nin `approve` aracına gelir ([onaylar](../concepts/approvals-and-questions.md)). Strict `--permission-prompts none` ile çalışır: soracak olan reddedilir.
- **Open** `--restricted`'ı bırakır: yazan, Claude Code'un varsayılan araç takımını `--disallowedTools Agent,Task` ile alır; ayarlarınız, hook'larınız ve MCP sunucularınız yüklenir; okuyan listesini korur.
- **Her turda denetlenir:** `init` olayı tam istenen araçları bildirmeli (open altında: en az onları, imece'nin sunucusu bir kez ve bağlı, alt ajan aracı yok), imece'nin MCP sunucusu tam araçlarıyla. Başka her şey oturumu durdurur.
- Her oturum `--max-budget-usd` alır: başladığında bütçeden kalan; kaçan bir oturuma sert bir tavan ([bütçe](../concepts/budget-and-limits.md)). Maliyetler dolar olarak gelir.
- Çabalı bir seçim (`opus@xhigh`, `@max`) `--effort` olur: `low`, `medium`, `high`, `xhigh` ya da `max`. Claude Code bilmediği bir çabayı yalnız uyarıp varsayılanıyla çalışır ve `init` olayı hangisi olduğunu değil, yalnız tur başına bir çabanın açık olduğunu söyler; bu yüzden imece bilinmeyen bir çabayı oturum başlamadan reddeder ([komut satırı](../reference/cli.md)).
- imece'nin talimatları Claude Code'un kendi sistem istemine eklenir. Notları Claude Code kendisi okur: `--restricted` tek başına hiçbir `CLAUDE.md` yüklemez, bu yüzden imece `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` ayarlar ve ajanın kendi dizinini `--add-dir` ile ekler; yükleyici o dizinin `CLAUDE.md`'sini içe aktarmaları ve `.claude/rules` ile birlikte tümüyle okur. `~/.claude/CLAUDE.md`'niz imece'ye ait bir dizindeki kopya olarak, aynı yolla gelir, çünkü `~/.claude`'u eklemek onu dosya araçlarına açardı; içindeki bir `@import` orada izlenmez. Open altında bunların hepsini Claude Code kendisi okur. Ayrıntılar [dosyalar](../reference/files.md) sayfasında.
- Bir oturum sonraki bir çalıştırmada kaldığı yerden sürer ve bir işçi çatallanabilir: [oturumlar](../concepts/sessions.md).

## Claude Code'un kendi komut satırı

`/` ile başlayan bir satır mesaj değil, Claude Code'un bir komutudur: `/clear`, `/compact`, `/cost`, `/context`, ne biliyorsa; `@claude-2 /compact` tek bir ajana gönderir. Bir tur olarak çalışır ve yanıtı bir cevap gibi döner. `/`'den sonra tamamlama menüsü o ajanın kendi komutlarını sunar. `/clear` yeni bir oturum kimliği başlatır ve maliyet sayacı saymaya devam eder.

## Kullanım limitleri

Claude Code `rate_limit_event` gönderir. imece hesap için uzunluk başına (beş saat, bir hafta) bir pencere tutar ve `:budget` notu keseden sonra dolmaya en yakın pencereyi gösterir: `claude 5 sa: %48 kaldı`; %90 kullanımdan itibaren başlıkta da durur, `claude 5 sa: %10 kaldı, 1 sa 59 dk sonra yenilenir`, dikkat renginde. Bir aboneliğin limiti ekibi bütçeden önce durdurabilir; imece bir turu kendisi durdurmaz, Claude Code'un reddini gösterir. Pencereler çalıştırmalar arasında saklanmaz; bir oturum ilk turunda yeniden söyler.

Her agent'ın bağlam penceresi de bildirilir: her mesaj isteğin token'larını (o ana kadarki bağlam), sonuç modelin penceresini taşır. Agent'ın kartının ekranındaki satırı %70'ten itibaren `◔ %72` gösterir, log sekmesi de rakamı; iz doluluğu onda birlik adımlarla not eder. Oturumla birlikte gider.

## Diğer kodlama ajanları

imece Codex ve Gemini CLI'yi de sürüyordu; işi sade tutmak için bu çıkarıldı. O kod `multi-vendor` dalında duruyor (etiket `multi-vendor-last`).

Onları süren bir sürümün kaydettiği proje yine açılır. Bu sürümün ilk başlangıcında Codex ya da Gemini üzerindeki lider ya da solo, kaldığı yer kendisine söylenerek yeni bir oturumda Claude Code olarak sürer; onlar üzerindeki diğer ajanlar gitmiş sayılır. Diskte kalan bir `<worktrees kökü>-gemini` dizini artık kullanılmaz, elle silinebilir.

## Ayrıca bakınız

- [Ekip](../concepts/team.md): kademeler ve kimin kimi işe aldığı.
- [Onaylar ve sorular](../concepts/approvals-and-questions.md): bir oturum neyi ne zaman sorar.
- [Bütçe ve limitler](../concepts/budget-and-limits.md): bir tur nasıl yazılır.
- [Güvenlik](../concepts/security.md): denetimler neyi korur, neyi korumaz.
- [CLI başvurusu](../reference/cli.md): `--model`, `--tier`, `--confine`.
