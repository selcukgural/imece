---
title: "Codex lider, Claude ve Gemini işçi"
description: Üç satıcıdan tek takım; kim lider, lider satıcılar arası nasıl işe alır, bir kademe her satıcıda ne demek, dolara karşı token ve başlıktaki kullanım sınırları.
---

# Codex lider, Claude ve Gemini işçi

imece Claude Code, Codex ve Gemini CLI'ı sürer ve bir takım bunları karıştırabilir: lider bir satıcıdan gelir ve PATH'teki herhangi bir satıcıdan işçi alır. Bununla birden fazla satıcı kuruluyken ya da bir abonelik birini ucuz kılıyorken karşılaşırsınız. Bu senaryo bir Claude işçisi ve bir Gemini işçisi alan bir Codex lideri başlatır ve tek satıcılı bir takımdan ayrılan yerleri izler.

Üç CLI de PATH'te ve giriş yapmış: Codex ChatGPT ile, Gemini bir API anahtarıyla.

## 1. Lideri adlandırarak başlatma

```sh
imece --lead codex --tier claude:strong=opus --agents 6
```

`--lead codex` liderin satıcısını adlandırır, bu yüzden KİM LİDER? sorusu atlanır. Bayrak olmadan imece birincil satıcınızı alır (o ekranda `p` ile ya da `:lead VENDOR` ile ayarlanır), yoksa PATH'teki tek satıcıyı, yoksa sorar: birkaç satıcı arasında sessiz bir varsayılan yoktur.

`--tier claude:strong=opus`, Claude işçileri için `strong` kademesinin ne demek olduğunu söyler (opus zaten Claude'un `strong`u; bayrak burada biçimi göstermek için, `opus[1m]` ya da `sonnet` onu değiştirirdi). Satıcı öneki önemli: önek olmadan `--tier` ve `--model` liderin satıcısından söz eder, ki burada Codex, ve `strong=opus` bir Codex modeli olarak okunurdu. `--agents 6` liderin en çok altı işçi almasına izin verir.

## 2. Bir Codex lideri nedir

Codex'in araç izin listesi yoktur ve her zaman kabuğu vardır, bu yüzden imece onu Codex'in kendi sandbox'ıyla sınırlar: lider için `read-only`. Çalışma kopyasını okur, web'de arar (`standard` altında açık) ve yazamaz; daha fazlasını isteyen bir komut düpedüz başarısız olur, çünkü bir lider asla sormaz (`approval_policy = "never"`). Salt okunur sandbox'ta kabuğunun ağı yoktur; web'i yalnızca web aramasıyla okur.

İz (satırında iki kez `enter`, ya da `t`) imece'nin dolara çeviremediği iki satır gösterir:

```text
 İZ · lead · ham akış
 codex runs on your ChatGPT plan: its turns cost the budget nothing
 tokens so far: 1200 in (1000 cached) · 34 out
```

Codex token bildirir, dolar değil. Bir ChatGPT planında turları bütçeye $0.00 yazılır ve token toplamları her turdan sonra gelir. API anahtarıyla giriş yapmış bir Codex başta reddedilir, çünkü imece henüz fiyatlandıramaz: `is logged in with apikey, whose cost imece can not count yet; log in with ChatGPT (codex login)`.

Thread başlamadan önce imece Codex'in neyle çalıştığını bildirdiğini denetler (config yankısı, eklenti yok, hook yok, tam olarak imece'nin MCP sunucusu) ve herhangi bir uyumsuzlukta oturumu durdurur. Bir Codex liderinin kendi komut satırı yoktur: `/` komutlarından yalnızca `/compact` çalışır.

## 3. Satıcılar arası işe alım

`/` tuşuna basın ve işi isteyin:

```text
 › lead için: hız sınırlayıcıyı yeni config biçimine taşı ve test ekle
```

`hire` aracı bir `tier` ve bir `vendor` alır: `cheap`, `standard` ya da `strong`, ve `claude`, `codex` ya da `gemini` (PATH'te olan ve imece'nin adaptörü bulunan satıcılar; varsayılan, alanın kendi satıcısı). Lider tasarım için `strong` bir Claude işçisi, testler için `cheap` bir Gemini işçisi alır:

```text
 14:02 ◆ lead işe aldı: claude-1 (opus)
 14:02 ◆ lead işe aldı: gemini-1 (flash)
 14:02 ⇄ lead → claude-1 [görev]: limiter: src/limit.rs içinde config v2
 14:02 ⇄ lead → gemini-1 [görev]: tests/limit.rs içinde limiter testleri
```

İşçiler satıcı başına numaralanır: `claude-1`, `gemini-1`, sonra alınacak bir Codex işçisi `codex-1` olurdu. Her satır satıcısının rengini taşır.

### Bir kademe her satıcıda ne demek

| Kademe | Claude | Codex | Gemini |
| --- | --- | --- | --- |
| `cheap` | `haiku` | `@low` (varsayılan modelinde düşük çaba) | `flash` |
| `standard` | satıcının kendi varsayılanı | varsayılan çabası | `auto` |
| `strong` | `opus` | `@high` | `pro` |

Codex'in kademeleri model değil akıl yürütme çabasıdır: bir kademe değeri `[MODEL][@EFFORT]` biçimindedir, `--tier codex:strong=gpt-5.6-terra@xhigh` ikisini de adlandırır. Hesabınızda olmayan bir kademe modeli ilk turunu bedelsiz kaybeder; imece o işçiyi standart modelde yeniden başlatır ve söyler:

```text
 14:03 ⚑ claude-1: opus modeli burada yok, varsayılanla devam
```

## 4. Gemini'nin fiyat tablosu

Gemini de token bildirir. API anahtarıyla imece her turu taşıdığı liste fiyatlarından yazar (200K token'a kadar istemler, düşünce token'ları çıktı olarak, önbellekli token'lar önbellek fiyatından); `gemini.rs` içindeki tablo `gemini-2.5` modellerini (`pro`, `flash`, `flash-lite`) ve `gemini-3` hattını (`3-flash-preview`, `3.1-pro-preview`, `3.1-flash-lite`, `3.5-flash-lite`, `3.6-flash`, `3.8-flash`) bilir. Yalnızca bir takma adla (`auto`, `pro`, `flash`) adlandırılan bir tur, takma adın karşılık gelebileceği en pahalı modelden yazılır. imece'nin fiyatlandıramadığı bir model oturumu durdurur; bildiği birini `--model gemini:…` ya da `--tier gemini:…` ile adlandırın. Google girişiyle turlar bütçeye hiçbir şey yazmaz. İz hangisi olduğunu söyler:

```text
 gemini runs on your API key: its turns are booked at list price
```

Gemini'de `--restricted` yoktur; imece onu kendine ait özel bir dizinde, worktree'yi dahil dizin olarak vererek çalıştırır ve oraya yazdığı bir politika dosyası tam olarak rolün ve seviyenin araçlarına izin verip gerisini reddeder (reddedilen bir araç modele gösterilmez bile). Deponun `.gemini` ayarları, hook'ları ve `.env`i oturuma asla ulaşmaz.

## 5. Bir Gemini işçisinin kabuğu sorar

`standard` altında bir Gemini yazıcısının `run_shell_command`ı politikada `ask_user`dır: her komut imece'ye gelir. Ajanın dizininde çalışan rutin komutlar hemen çalışır (işçiye her komuta `dir_path` vermesi söylenir); gerisi bekler:

```text
 SENİ BEKLEYEN (1)
 ⚑ 14:09 gemini-1 onayını bekliyor: shell: npm run lint:fix (in /Users/you/code/api)
   a izin ver · r reddet · enter odak
```

`a` bir kez izin verir, `r` reddeder: imece Gemini'nin sorusunu onun kendi iptal seçeneğiyle yanıtlar, çağrı başarısız olur ve tur sürer. Bir Codex işçisi `workspace-write` sandbox'ının verdiğinden fazlasını gerektiren şeyler için aynı yolla sorar (`on-request`), Codex'in kendi kabuk sargısı çıkarılmış hâlde `shell: …` olarak gösterilir. Turun bitmesi hâlâ bekleyen bir soruyu geri çeker (öğesi kapanır, iz `no longer asks` der). Günlük her kararı tutar:

```text
 14:09 › gemini-1 için izin verdin: shell: npm run lint:fix (in /Users/you/code/api)
```

## 6. Başlıktaki kullanım sınırları

Aboneliklerle çalışan bir takımı, bütçe bitmeden çok önce planın beş saatlik ya da haftalık penceresi durdurabilir. Claude ve Codex oturumları pencerelerini bildirir; başlık her satıcının dolmaya en yakın penceresini bütçeden sonra gösterir:

```text
 imece  ● 2 çalışıyor │ $4.20 / $50.00 harcandı ($6.10/sa, ~7 sa 30 dk kaldı) │ claude 5 sa: %48 kaldı, 1 sa 59 dk sonra yenilenir │ codex 7 g: %81 kaldı  14:20
```

%90 kullanımdan sonra bir satıcının satırı dikkat rengine döner. Gemini pencere bildirmez. Pencereler ajanın değil hesabındır ve çalışmalar arasında saklanmaz: bir oturum ilk turunda onları yeniden söyler. Bir pencere dolunca turdan önce hiçbir şey durdurmaz; satıcı turu kendisi reddeder ve imece o reddi gösterir.

Sınırların yanında bütçe: `$6.10/sa` son on dakikanın tuttuğu, `~7 sa 30 dk kaldı` bu hızla bütçenin ne kadar yeteceği. Codex'in turları buraya hiçbir şey eklemez; Gemini'nin ve Claude'un turları ekler.

## 7. Seçenek olarak `--confine strict`

`--confine strict` ile yeniden başlatın: her ajan yalnızca satıcısının dosya araçlarını alır; komutlar imece'nin `run` aracından geçer (derleme, testler, git'in okuma komutları, `--run` programlarınız) ve hiçbir şey size bir şey sormaz:

| Satıcı | `standard` (varsayılan) | `strict` |
| --- | --- | --- |
| Claude | `--restricted`, dizinindeki dosya araçları, `Bash`, `WebFetch`, `WebSearch`; Claude Code'un soracağı bir kabuk komutu size gelir | `--restricted`, yalnızca dosya araçları; soracak olan reddedilir |
| Codex | ağlı `workspace-write` sandbox, web araması açık; sandbox'ın reddettiği şeyi işçi sorar | ağsız sandbox, web araması kapalı, onay `never`: sandbox'ın reddettiği başarısız olur |
| Gemini | politika dosya araçlarına, web'e ve soran bir kabuğa izin verir; gerisi reddedilir | politika yalnızca dosya araçlarına izin verir; kabuk yok, web yok; soracak olan reddedilir |

Her seviyede lider yazmaz ve satıcının alt ajan aracı kapalı kalır (CLI içinde doğan bir alt ajan worktree'yi, bütçeyi, veri yolunu ve ekranı atlardı). `--confine open` öteki yöne gider: satıcı sizin kurduğunuz gibi çalışır; ayarlarınız, hook'larınız, MCP sunucularınız ve uzantılarınız yüklenir. Günlük strict olmayan her başlangıçta seviyeyi söyler (`confine standard: …` ya da `confine open: …`). imece hiçbir şeyi sandbox'lamaz: her satıcının kendi kısıtlarını açar ve oturumun onlarla çalıştığını, kapalıya düşerek, denetler; yalıtım bir konteynerin işidir.

## Ayrıca bakın

- [Satıcılar](../concepts/vendors.md): her adaptör ayrıntısıyla
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Onaylar ve sorular](../concepts/approvals-and-questions.md)
- [Güvenlik](../concepts/security.md): imece neyi korur, neyi korumaz
- [Komut satırı seçenekleri](../reference/cli.md)
