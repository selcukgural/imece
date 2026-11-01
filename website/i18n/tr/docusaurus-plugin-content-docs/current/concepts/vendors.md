---
title: Satıcılar
description: Claude Code, Codex ve Gemini CLI için bir ajan oturumu nedir, --confine kadranı her satıcıdan ne kadarını verir, imece fail-closed neyi denetler, satıcıların kendi komut satırları ve kullanım limitleri.
---

# Satıcılar

imece PATH'te `claude`, `codex` ve `gemini`'yi arar ve üçünü de sürer. Bir ajan, satıcısının CLI'sinin uzun ömürlü tek bir oturumudur; imece o satıcının kendi kısıtlamalarını ajanın rolü ve `--confine` düzeyi için açar ve oturumun tam olarak onunla çalıştığını fail closed denetler. Bu sayfa satıcı başına bir oturumun ne olduğunu ve neyin denetlendiğini söyler.

## Confine kadranı

`--confine standard|strict|open` her ajanın satıcısından ne kadarını aldığını söyler. Her düzeyde lider asla yazmaz ve satıcının kendi alt ajan aracı kapalı kalır.

| Düzey | İşçiler ve yardımcılar | Lider | Kendi satıcı kurulumunuz |
| --- | --- | --- | --- |
| `standard` (varsayılan) | satıcının tam kodlama araç takımı: okuma, yazma, kabuk, web arama ve getirme | okuma araçları ve web | yüklenmez |
| `strict` | yalnız dosya araçları; komutlar imece'nin `run` aracından; hiçbir şey sormaz, soracak olan reddedilir | okuma araçları | yüklenmez |
| `open` | satıcı sizin kurduğunuz gibi (ayarlar, hook'lar, MCP sunucuları, skill'ler, uzantılar), alt ajan aracı hariç | okuma listesi | yüklenir |

imece hiçbir şeyi sandbox'lamaz: her satıcının kendi kısıtlamalarını açar ve doğrular; yalıtım bir konteynerin işidir. Günlük strict olmayan her başlangıçta bunu bir kez söyler: `confine standard: işçiler satıcılarının kabuğunu ve web'ini alır; yalıtım bir konteynerin işi, imece'nin değil`.

## Claude Code

Bir oturum, ajan başına bir, uzun ömürlü bir `claude -p` akışıdır (stream-json).

- **Standard ve strict** `--restricted` modunda çalışır: kullanıcı ya da proje ayarı yok (dolayısıyla hook yok), imece'ninki dışında MCP sunucusu yok, dosya erişimi ajanın dizinine sınırlı. Araçlar tam adlarıyla anılır: okuyan için `Glob`, `Grep`, `Read`; yazan için artı `Edit`, `Write`; standard'dan itibaren herkese `WebFetch`, `WebSearch` ve yazanlara `Bash`. Web'e doğrudan izin verilir (yalnız okur); Claude Code'un soracağı bir kabuk komutu imece'nin `approve` aracına gelir ([onaylar](../concepts/approvals-and-questions.md)). Strict `--permission-prompts none` ile çalışır: soracak olan reddedilir.
- **Open** `--restricted`'ı bırakır: yazan, Claude Code'un varsayılan araç takımını `--disallowedTools Agent,Task` ile alır; ayarlarınız, hook'larınız ve MCP sunucularınız yüklenir; okuyan listesini korur.
- **Her turda denetlenir:** `init` olayı tam istenen araçları bildirmeli (open altında: en az onları, imece'nin sunucusu bir kez ve bağlı, alt ajan aracı yok), imece'nin MCP sunucusu tam araçlarıyla. Başka her şey oturumu durdurur.
- Her oturum `--max-budget-usd` alır: başladığında bütçeden kalan; kaçan bir oturuma sert bir tavan ([bütçe](../concepts/budget-and-limits.md)). Maliyetler dolar olarak gelir.
- Çabalı bir seçim (`opus@xhigh`, `@max`) `--effort` olur: `low`, `medium`, `high`, `xhigh` ya da `max`. Claude Code bilmediği bir çabayı yalnız uyarıp varsayılanıyla çalışır ve `init` olayı hangisi olduğunu değil, yalnız tur başına bir çabanın açık olduğunu söyler; bu yüzden imece bilinmeyen bir çabayı oturum başlamadan reddeder ([komut satırı](../reference/cli.md)).
- Talimatlar Claude Code'un kendi sistem istemine eklenir, sonunda deponun `AGENTS.md` ya da `CLAUDE.md`'si (16 KiB tavan), çünkü `--restricted` onları düşürür.

## Codex

Bir oturum, uzun ömürlü bir `codex app-server`'dır (stdio üzerinde JSON-RPC).

- Codex'in her zaman kabuğu vardır, bu yüzden sınırlayan **sandbox**'tır: lider için `read-only`, işçiler ve yardımcılar için `workspace-write` (ajanın dizini). Strict altında sandbox'ın ağı yoktur ve web arama kapalıdır; standard altında bir işçinin sandbox'ı ağa ulaşır ve web arama açıktır (bir Codex liderinin web araması vardır ama salt okunur kabuğunda ağ yoktur).
- Standard ve open altında yazanlar `approval_policy = "on-request"` ile çalışır ve sandbox'ın verdiğinden fazlasını gerektirenden önce sorar; lider ve strict altındaki her şey `never`'da kalır, orada sandbox'ın reddettiği yalnızca başarısız olur.
- Standard ve strict eklentileri, uygulamaları, hook'ları, alt ajanları, bellekleri ve diğer özellikleri kapatır, oturum dosyası yazmaz (`history.persistence = "none"`; thread'in kendi rollout'u devam için tutulur) ve kendi `~/.codex` yapılandırmanızın getirdiği MCP sunucularını ve skill'leri adlarıyla kapatır: Codex yapılandırmanızı imece ne yaparsa yapsın yükler, bu yüzden ilk başlangıç neyin açık olduğunu okur ve bir şey açıksa imece Codex'i her birini adıyla kapatarak ikinci kez başlatır. Open yapılandırmanızı korur, yalnız alt ajan özelliği (`multi_agent`) kapalı.
- **Thread başlamadan önce denetlenir:** `account/read` (yalnız ChatGPT oturumu), `config/read` her geçersiz kılmayı yansıtır, etkin eklenti yok, hook yok, tam imece'nin MCP sunucusu tam araçlarıyla, thread'in yanıtı onay politikasını, sandbox'ı ve dizini yansıtır. MCP sunucuları her turdan önce yeniden denetlenir. Gelmemesi gereken bir onay isteği, bir elicitation, dinamik bir araç çağrısı, bir web arama ya da alt ajan öğesi oturumu durdurur.
- **Dolar değil, token.** ChatGPT planında bir tur bütçeye hiçbir şey yazmaz; iz bunu söyler ve token'ları gösterir. API anahtarıyla oturum reddedilir; imece maliyetini henüz sayamaz. Codex için oturum başına dolar tavanı yoktur; bütçe yine yeni turları durdurur.
- Kademeler Codex'in varsayılan modelinde akıl yürütme çabalarıdır: `@low`, varsayılan, `@high`; bir `--tier codex:T=MODEL@ÇABA` değeri ikisini de belirler. Codex'in çabaları `low`, `medium`, `high` ve `xhigh`; çaba her turla gider, Codex'in olmayan biri oturum başlamadan reddedilir.
- Codex `AGENTS.md`'yi kendi okur, bu yüzden ona yalnız `CLAUDE.md` verilir.

## Gemini CLI

Bir oturum, uzun ömürlü bir `gemini --acp`'dir (Agent Client Protocol v1, stdio üzerinde JSON-RPC). Gemini CLI'da çaba ayarı yoktur: bir seçim yalnız model adlandırır, `gemini:pro@high` reddedilir.

- Gemini'de `--restricted` gibi bir şey yoktur ve ayarlarını `~/.gemini`'den ve çalıştığı klasörün `.gemini` dizininden alır. Bu yüzden imece onu kendi **özel dizininde** çalıştırır (worktree'lerin yanında, `<worktrees kökü>-gemini/<ajan>`, güvenilir) ve ajanın dosyalarını dahil edilen dizin olarak verir: bir deponun `.gemini` ayarları, hook'ları ve `.env`'i oturuma asla ulaşmaz.
- imece'nin oraya yazdığı bir **politika dosyası** tam rolün ve düzeyin araçlarına izin verir (lider için okuma araçları; yazanlar için okuma ve düzenleme araçları; standard'dan itibaren herkese sormadan web ve yazanlara her komuttan önce soran bir kabuk), artı imece'nin kendi araçları; strict ve standard altında diğer her aracı reddeder, o araç modele gösterilmez bile. Uzantılar kapalıdır ve imece'ninki dışında MCP sunucusuna izin yoktur. Open altında uzantılarınız, MCP sunucularınız ve politikalarınız çalışır; imece yalnız Gemini'nin ajan araçlarını ve lider için yazma araçlarını adıyla reddeder.
- Ajanın talimatları o özel dizinde bir `GEMINI.md`'ye gider; ACP'nin onları vermenin tek yolu budur. Kendi `~/.gemini/GEMINI.md`'niz ve kullanıcı düzeyi hook'larınız yine geçerlidir.
- **İlk turdan önce denetlenir:** `initialize`, `session/new` (mod `default`, istenen model), sonra sıfır token'lık komutlar `/about` (model, kimlik doğrulama türü), `/extensions list` (etkin yok) ve `/memory list` (imece'nin GEMINI.md'si kullanımda). Rolün alamayacağı türden bir araç çağrısı, sunulmamış bir MCP aracı ya da bir mod değişikliği oturumu durdurur.
- **Dolar değil, token.** API anahtarıyla bir tur imece'nin taşıdığı liste fiyatlarından yazılır (200K token'a kadar istemler, düşünce token'ları çıktı olarak, birden çok modeldeki bir tur en pahalısından; `auto` gibi bir takma ad anlamına gelebileceği en pahalı modelden). imece'nin fiyatlandıramadığı bir model oturumu durdurur: `--model` ya da `--tier` ile bildiği birini verin. Google oturumuyla bir tur bütçeye hiçbir şey yazmaz; iz bunu söyler. Vertex AI ve gateway oturumları reddedilir.
- Kademeler: `flash`, varsayılan (`auto`), `pro`. Gemini sohbet dosyalarını her zamanki gibi `~/.gemini/tmp` altında tutar.

## Satıcının kendi komut satırı

`/` ile başlayan bir satır mesaj değil, ajanın kendi CLI'sinin bir komutudur: `/clear`, `/compact`, `/cost`, `/context`, satıcı ne biliyorsa; `@claude-2 /compact` tek bir ajana gönderir. Bir tur olarak çalışır ve yanıtı bir cevap gibi döner. `/`'den sonra tamamlama menüsü o ajanın kendi komutlarını sunar.

| Satıcı | Komutlar |
| --- | --- |
| Claude Code | slash komutları olduğu gibi; `/clear` yeni bir oturum kimliği başlatır ve maliyet sayacı saymaya devam eder |
| Codex | app server'ın komut satırı yoktur: yalnız `/compact` çalışır (`thread/compact/start`; ret, başarısız bir turdur); `/clear` oturumu bitirir, sonraki mesaj yenisini başlatır |
| Gemini CLI | komutları olduğu gibi |

## Kullanım limitleri

Claude Code `rate_limit_event`, Codex `account/rateLimits/updated` gönderir; Gemini hiçbirini bildirmez. imece hesap için satıcı ve uzunluk başına (beş saat, bir hafta) bir pencere tutar ve başlık bütçeden sonra her satıcının dolmaya en yakın penceresini gösterir: `claude 5 sa: %48 kaldı, 1 sa 59 dk sonra yenilenir`, %90 kullanımdan itibaren dikkat renginde. Bir aboneliğin limiti ekibi bütçeden önce durdurabilir; imece bir turu kendisi durdurmaz, satıcının reddini gösterir. Pencereler çalıştırmalar arasında saklanmaz; bir oturum ilk turunda yeniden söyler.

## Hangi satıcı lider, hangileri işe alınır

`--lead` liderin satıcısını adlandırır; lider PATH'teki her satıcıdan işçi alabilir (`hire` bir satıcı alır). Karma ekipler işin özüdür: bkz. [karma satıcılar](../scenarios/mixed-vendors.md) ve [ekip](../concepts/team.md). Satıcı başına devam [oturumlar](../concepts/sessions.md) sayfasında.

## Ayrıca bakınız

- [Onaylar ve sorular](../concepts/approvals-and-questions.md): her satıcı neyi ne zaman sorar.
- [Bütçe ve limitler](../concepts/budget-and-limits.md): token'lar nasıl yazılır.
- [Güvenlik](../concepts/security.md): denetimler neyi korur, neyi korumaz.
- [CLI başvurusu](../reference/cli.md): `--lead`, `--model`, `--tier`, `--confine`.
