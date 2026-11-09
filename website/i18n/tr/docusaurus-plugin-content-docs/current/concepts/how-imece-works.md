---
title: imece nasıl çalışır
description: Bir kullanıcının karşılaştığı parçalar - ekran ve modeli, Claude Code adaptörü, ajan başına bir oturum, bus, git iş parçacığı, SQLite deposu, bir işçinin worktree'si ile checkout'unuz arasındaki kapı ve bir projenin altındaki Lonca.
---

# imece nasıl çalışır

imece, birçok Claude Code ajanını tek bir kod tabanında çalıştıran ve tek ekranda gösteren bir terminal programıdır. Bu sayfa, rehberin kalanını okumak için gereken mimaridir: ne nerede çalışır, dosyalarınıza ne dokunur, ne asla dokunmaz.

## İki crate

| Crate | Ne tutar |
| --- | --- |
| `imece` (`crates/imece-tui`) | Ekran ve modeli. Model (`model.rs`) olanları veri olarak saklar; ekran çizim anında İngilizce ya da Türkçe sözcüklere çevirir. Aynı modeli canlı oturumlar (`live/`, varsayılan) ya da `--demo`'nun sahte ajanları besler. |
| `imece-agent` | Claude Code adaptörü. `claude`'u alt süreç olarak sürer ve çıktısını olaylara çevirir (tur başladı, metin, araç çağrısı, maliyet, limit, soru). Ekran hiçbir zaman bir tel biçimi görmez. |

Bir ajanın yazdığı her şey terminale ancak temizlenerek ulaşır: kontrol karakterleri kaldırılır, gizli anahtara benzeyen sözcükler saklanır. Bkz. [güvenlik](../concepts/security.md).

## Ajan başına bir oturum

Her ajan, uzun ömürlü tek bir alt süreçtir; tur başına süreç açılmaz:

- Alt süreç `claude -p`'dir (stream-json). Ne olduğu ve imece'nin neyi denetlediği [Claude Code](../concepts/claude-code.md) sayfasında.
- Alt süreç temizlenmiş bir ortam alır (Claude Code'un oturum açmak için gerekenleri artı), kendi süreç grubunu (imece kapanınca öldürülür) ve sınırlı boruları (8 MiB'ı aşan bir satır oturumu bitirir).
- Oturumlar ekranın döngüsünün dışında başlar; ekran bir oturumun başlamasını asla beklemez.

İstenenden başka bir şeyle çalıştığını bildiren bir oturum (başka bir araç, başka bir MCP sunucusu, bir hook, başka bir sandbox) durdurulur: fail closed.

## Bus: imece'nin kendi MCP sunucusu

Ajanlar birbirleriyle ve sizinle, imece'nin kendisinin MCP üzerinden sunduğu araçlarla konuşur:

- Ajan başına bir Unix soketi, yeni açılmış `0700` geçici bir dizinde. Claude Code imece'nin kendi ikilisi olan `imece mcp-relay <soket>`'i başlatır; o yalnızca bayt kopyalar. Araçlar imece'nin içinde çalışır.
- Gönderen, çağrının geldiği sokettir; ajanın söylediği bir ad asla değil. Hiçbir ajan bir başkası ya da siz gibi konuşamaz.
- Araçlar `send`, `post`, `board`, `run`, `whoami` (agent'ın imece'de kim olduğu: adı, rolü, projesi, kime bağlı olduğu, kalan bütçe, işe alıp alamayacağı) ve role göre `claim` (işçiler), `hire` (lider işçi, işçi yardımcı işe alır) ve `approve` (yalnız bir Claude oturumu; Claude Code onay gereken bir araç çağrısından önce bunu kendisi çağırır).

Araçların ne yaptığı [mesajlar ve pano](../concepts/messages-and-board.md) ile [onaylar ve sorular](../concepts/approvals-and-questions.md) sayfalarında.

## Ajanlar nerede çalışır

- **Lider** sizin checkout'unuzda çalışır ve onu okur. Hiç yazmaz: oraya bir yazma incelemeyi atlar ve çalışma kopyanızla çakışır. Bu bir roldür, bir güvenlik sınırı değil.
- **İşçiler** her biri `$XDG_STATE_HOME/imece/worktrees/` altında (varsayılan `~/.local/state/imece/worktrees/`) HEAD'inizde detached bir git worktree alır. Checkout'unuza hiçbir ajan dokunmaz.
- **Yardımcılar** işçilerinin worktree'sinde, işçinin adını verdiği dosyalarda çalışır.

Bir worktree çalıştırmalar arasında saklanır. Eski bir commit'te temiz bulunan, ajanı bir sonraki başlayışında ve turları arasında HEAD'inize taşınır; commit edilmemiş değişiklikleri ya da kendi commit'leri olan yerinde kalır. Günlük hangisinin olduğunu söyler.

## Δ kapısı

Bir işçi `done` gönderince imece onun worktree'sini okur (izlenmeyen dosyalar dahil, özel bir index üzerinden; ajanın kendi index'ine dokunulmaz) ve diff'i KARAR BEKLEYEN'de bir değişiklik (`Δ`) olarak gösterir. Siz uygularsınız (`a`: checkout'unuza `git apply --index`, stage'e; düz uygulama oturmazsa üç yollu), reddedersiniz (`r`) ya da not gönderirsiniz (`n`). Siz olmadan hiçbir şey checkout'unuza ulaşmaz. Yolun tamamı [değişiklikler](../concepts/changes.md) sayfasında.

## Ekran

Ekran, üstündeki bir şey değiştiğinde modeli çizer; bir oturumu, git'i ya da depoyu asla beklemez. İki ekranı vardır:

- **Ana ekran**, imece'nin açıldığı yer, tek bir listedir. Önce KARAR BEKLEYEN gelir, karar vermenin maliyetine göre gruplu: izinler (bir ajanın çalıştırmak istediği bir komut, evet denecek bir betik), sorular (bir ajanın sorusu, liderin planı, denemeleri biten bir kart), değişiklikler (okunacak bir diff, hazır olanlar önce). Altında ÇALIŞAN: her projenin işçileri ve soloları, önce takılanlar, sonra çalışanlar, sonra boştakiler, her birinin yardımcıları altında. Her satır kartıyla başlar. Bkz. [Ana ekran](../screen/home.md).
- **Tek bir iş** bir satırda `enter` ile açılır: kartı, projeyi, ajanı, süreyi ve maliyeti söyleyen bir baş satırı ve üç sekme, `diff · konuşma · log`. Bir değişiklik hüküm satırıyla başlar, tek satırda Δ kapısı; bir soru, sorunun kendisi ve sunduğu yanıtlarla; bir ajan log'uyla, kartının hikâyesinden oturumunun ham akışına kadar.

Pano, kod haritası, zaman çizelgesi, günlük, harcamalar, süreçler ve Lonca penceresi ikisinin de üstünde açılan pencerelerdir (`:board`, `:code`, `:timeline`, `J`, `:ledger`, `:ps`, `:lonca`); `esc` kapatır. Başlıkta dört şey vardır: açık projeler, `! N karar bekliyor`, kese ve saat.

## Lonca'da

[Lonca](https://github.com/selcukgural/lonca), bir deponun tek yazarı olan bir daemon'dır; lease'leri, bir merge kuyruğu ve imzalı bir log'u vardır. Bir proje bir depoya dayanabilir (`:lonca AD` bir depoyu adlandırır, `:lonca import` checkout'tan bir tane yapar); imece o zaman daemon'ın soketinin bir istemcisidir, asla ikinci bir yazar değil:

- O andan sonra tutulan bir işçi worktree yerine bir Lonca **workspace**'inde çalışır; Claude Code'unda imece'nin bus'ının yanında Lonca'nın kancası ve araçları vardır. Düzenlediğini lease'ler ve değişikliğini önerir.
- Onun Δ'sı, turu bitince, bekleyen en yeni önerisidir. `a` onu onaylar, `r` bir gerekçeyle reddeder; ikisini de `lonca` programı imzalar. Checkout'unuza hiçbir şey inmez.
- Evetinizden sonra Lonca'nın merge kuyruğu deponun kontrollerini birleşmiş ağaçta çalıştırır ve değişikliği gövdeye (trunk) koyar ya da işçiye geri gönderir. Checkout'unuz gövdenin bir istemcisidir: `:sync` onu yetiştirir.
- imece her 5 saniyede daemon'ın durumunu okur: projenin satırı (`lonca mobile-app 9d3f1c2 · 3 bekliyor`), kod haritasındaki lease'ler, `:lonca` penceresi. Daemon yanıt vermedikçe proje kapalıdır: kimse tutulmaz ve hiçbir Δ'ya karar verilmez.

Ayrıntılar [projeler](projects.md#loncada) ve [değişiklikler](changes.md#loncada) sayfalarında. Hiçbir depoda olmayan bir proje yukarıdaki gibi worktree'lerle ve checkout'unuzla çalışır.

## Git iş parçacığı

Bir değişikliği okumak, uygulamak, bir worktree'yi HEAD'inize taşımak ve bir worktree'nin nerede durduğunu okumak, imece'nin kendi tek git iş parçacığının işleridir; teker teker, verilen sırayla. İş başına değil tek iş parçacığı: iki uygulama checkout'unuzda asla karşılaşmaz ve bir uygulamadan sonra gönderilen `done`, o uygulama worktree'yi işaretledikten sonra okunur. Ekran git'i asla beklemez; bir işin nasıl gittiğini günlük söyler.

## SQLite deposu

imece'nin bildiği her şey tek bir SQLite dosyasında saklanır: `$XDG_STATE_HOME/imece/imece.sqlite` (`0600`, yalnız sizin): projeler, ajanlar, kartlar ve hikâyeleri, günlük, gelen kutusu, bekleyen posta, pano, bütçe ve imece'nin kendi ücretli çağrıları. Her adımdan ve her tuştan sonra bir kayıt çalışır ve yalnız değişeni yazar. Dosyayı bir anda tek bir imece tutar; ikincisine ilkinden `:open` kullanması söylenir. Ayrıntılar [oturumlar](../concepts/sessions.md) sayfasında.

## Bir turda ne olur

```text
siz  ─/ Parser'ı düzelt─▶  lead (checkout'u okur)
                              │ hire(tier: standard)  → claude-1 kendi worktree'sinde
                              │ send(to: claude-1, type: assign, body: …)
                            claude-1 düzenler, `cargo test`'i run ile çalıştırır
                              │ send(to: lead, type: done, re: 1)
                            imece worktree'yi okur ─▶ KARAR BEKLEYEN'de Δ
siz  ─a─▶ checkout'unuzda stage'de; worktree işaretli (yazar imece)
```

Yalnız bir mesajın zarfı (türü, kime gittiği, hangi mesaja yanıt olduğu) bir şeyi sürer; gövdesini imece asla yorumlamaz.

## Ayrıca bakınız

- [Ekip](../concepts/team.md): lider, işçiler, yardımcılar, işe alma.
- [Claude Code](../concepts/claude-code.md): bir oturum nedir, ne denetlenir.
- [Değişiklikler](../concepts/changes.md): Δ kapısı uçtan uca.
- [Oturumlar](../concepts/sessions.md): çalıştırmalar arasında ne saklanır.
- [Dosyalar](../reference/files.md): imece'nin yazdığı her yol.
