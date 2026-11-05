---
title: Projeler
description: Tek imece'de birkaç checkout - :open, :close, :rename, proje başına bir lider, salonda katlanan gruplar, :pin ve bir işçinin worktree'sine kopyalanan yok sayılan yolların proje başına :copy listesi.
---

# Projeler

Bir proje, kendi lideri ve işçileri olan bir git checkout'udur. imece, içinde çalıştığı checkout'la ilk proje olarak başlar (lider `lead`); daha fazlasını ekrandan açarsınız ve her biri salonda bir gruptur. Tek bütçe ve tek bus hepsine hizmet eder.

## Açmak, kapatmak, yeniden adlandırmak

| Komut | Ne yapar |
| --- | --- |
| `:open YOL [AD]` (`:o`) | YOL'daki git checkout'unu kendi başına bir proje olarak açar; `lead-AD` adlı yeni bir liderle (aynı model ve kademeler). AD varsayılan olarak dizinin güvenli hale getirilmiş adıdır: 1-24 harf, rakam, `.`, `_`, `-`; kalanı `-` olur. `:` menüsü `$` ile işaretler: yeni bir lider oturumu. |
| `:solo [YOL] [SEÇİM] [AD]` | kendi başına bir proje olarak bir solo oturum açar: lidersiz, kendinizin sürdüğü, adını verdiğiniz model ve çabada tek bir ajan, `solo-AD` ([solo oturumlar](solo.md)). Bir checkout'ta takımının yanında istendiği kadar olabilir. `$`: yeni bir oturum. |
| `:close [AD]` (`:c`) | projenin ajanlarının oturumlarını ve çalıştırmalarını bitirir, postalarını, notlarını, tekliflerini ve değişikliklerini düşürür, gelen kutusu öğelerini kapatır, projeyi kapalı işaretler. Worktree'ler diskte kalır. Ad yoksa eldeki proje. Son açık proje kapanmayı reddeder (`q q` çıkar). |
| `:archive [BAŞLIK]` | eldeki projeyi, ajanları uykuda ve her şeyleriyle (oturumlar, posta, değişiklikler, kartlar, günlük satırları) gözden uzağa kaldırır ve yerine aynı ad, baş ve `:copy` listesiyle yeni bir proje açar. On saniyelik bekleme, `u` tutar. Bkz. [arşivler](sessions.md#arşivler). |
| `:restore [AD]` | bir arşivi kaldırıldığı gibi geri getirir, ajanları kendilerine yazılana dek uykuda; çalışma kopyasında açık olan takım yer açar (arşivlenir, ya da hiçbir şey yapmadıysa kapatılır). Bkz. [arşivler](sessions.md#arşivler). |
| `A`  `:archives` | arşivler bir ağaç olarak, çalışma kopyasına göre, her birinin harcadığı, biten kartları ve ajanlarıyla; `/` süzer, `enter` birini salt okunur açar, `r` birini geri getirir. |
| `:rename AD` | eldeki projeyi ve liderini `lead-AD` olarak yeniden adlandırır (ilk projenin de). Oturumlar sürer; projenin her canlı ajanı sonraki isteminde bir not alır ve eski ada `send` ekip listesiyle yanıtlanır. |

- Kapalı bir proje kimliğini korur; aynı checkout yeniden açılınca yeni bir liderle onu yeniden kullanır (kapalı bir soloyu o checkout'taki sonraki solo kullanır, bir takım asla). Sabitlemesi o zaman için saklanır. Arşivlenmiş bir proje bu yolla asla yeniden kullanılmaz.
- Bir checkout'ta en çok bir açık takım (`that checkout is open already, as kule`) ve istendiği kadar solo bulunur. Bir projede varsa dizinin adı numara alır: `kule`, `kule-2`.
- Adlar küreseldir: `@lead-api` o projenin liderine her yerden ulaşır; `@`'den sonraki tamamlama önce eldeki projeyi listeler.
- Boşluklu bir YOL ayrıştırılmaz (tırnak yok). `:open ` dosya sisteminden dizinleri tamamlar (gizli olanlar `.`'dan sonra, ev için `~`); `:close ` ve `:pin ` açık projelerin adlarını tamamlar.
- Günlük: `api projesini açtın (/src/api)`, `api projesini kapattın`, `api projesine gateway adını verdin`, `api projesini “v1” adıyla arşivledin; yerine yeni bir api açıldı`, `api projesinin “v1” arşivini geri getirdin; orada açık olan “api 2026-11-02” adıyla arşivlendi`.
- Projeler depoda hatırlanır; checkout'u gitmiş hatırlanan bir proje kapalı yüklenir.

Bir solonun modeli ve çabası solo başına sizin seçiminizdir.

## Salonda gruplar

```text
 SALON
 ▴ api        ! 1 bekliyor
   ◆ lead-api        koordine ediyor
   ● claude-1        #4 auth: refresh      +12 −3
 ▸ shop       2 çalışıyor
 ▾ imece
   ◆ lead            boşta
   ✓ claude-2         bitti, henüz bakılmadı
```

- Her proje kendi lideri olan bir gruptur. `h` `l` (ya da `←` `→`, başlıkta `enter`) bir grubu katlar ve açar; `za` değiştirir, `zM` her işçiyi katlar, `zR` hepsini açar (`:fold`, `:unfold`).
- Katlanmış bir proje tek satırdır ve o satır yine sizi bekleyeni (`! 1 bekliyor`) ya da ne yaptığını (`2 çalışıyor`) gösterir.
- Salon kısa kalınca sakin işçiler alttan yukarı `… N agent yolunda` içine katlanır; sizi bekleyen işçiler en son, sabitlenmiş bir proje hepsinden sonra katlanır.
- Birkaç proje açıkken günlük ve gelen kutusu satırları ajanın adının önünde projenin adını taşır.
- Komut satırındaki düz metin **seçili** projenin liderine gider. Seçim, `/`'nin kime yazdığına karar verir.

## `:pin [AD]`

`:pin` adı verilen projeyi, yoksa eldekini salonun en üstüne sabitler; `:pin` yine sabitlemeyi kaldırır. Sabitlenmiş projeler önce, sonra kalanı gelir; her grup açılış sırasıyla. Sabitlenmiş bir projenin başlığı `▴` taşır. Sabitleme sizindir ve çalıştırmayı aşar (depoda saklanır). `:` menüsü `:pin`'in hangi projeye etki edeceğini gösterir, zaten sabitse `▴` ile.

Proje başına bir renk yapılmadı: temalar renkleri role göre adlandırır ve konuşmacı renklerini farklı sıralar; adıyla seçilen bir renk önce her temada adlandırılmış bir palet gerektirirdi.

## `:copy YOL`: bir worktree'nin gerektirdiği yok sayılan yollar

Bir işçinin worktree'si taze bir checkout'tur; git'in yok saydığı orada yoktur: `.env`, `node_modules`, `target/`. Bazı projeler bunları her worktree'de ister ve hangilerinin olduğunu yalnız siz bilirsiniz.

- `:copy YOL` eldeki projenin bir yolunu listesine ekler; `:copy YOL` yine düşürür; `:copy` tek başına listeyi söyler, `:` menüsü de. Sondaki `/` aynı yoldur. Liste varsayılan olarak boştur ve projeyle depoda saklanır.
- **Ne listelenebilir** (eklenirken ve her kopyada yeniden denetlenir): en çok 200 baytlık göreli bir yol, düz (glob, ters bölü ya da kontrol karakteri yok, `.` ya da `..` yok, `.git` içinde değil); checkout'ta var; bir bağlantı değil ve dizinleri çözülünce checkout'un içinde (dışarıya bağlanan bir dizin reddedilir); ve git'in yok saydığı (`git check-ignore`: izlenen bir dosya zaten worktree'dedir).
- **Ne zaman:** her işçinin başlangıcında, worktree'si yapılır ya da yerleştirilir yerleştirilmez; yardımcılar işçilerinin worktree'sini paylaşır; checkout'ta çalışan lider için asla.
- **Nasıl:** worktree'nin zaten sahip olduğu bir yol (dosya, dizin ya da bağlantı) olduğu gibi bırakılır; imece işçinin kopyasının üzerine asla yazmaz, bu yüzden sonraki bir başlangıç yalnız o zamandan beri listeye ekleneni kopyalar. Her yol önce ölçülür (bağlantılar kendileri olarak, asla izlenmez; en çok 100 000 girdi) ve bir başlangıcın bütün yolları birlikte 2 GiB içinde kalır; kalanı aşacak bir yol atlanır. Kopya `cp -R -P -p`'dir: macOS'ta `-c` ile (APFS klonu, neredeyse bedava) ve birim klonlayamıyorsa onsuz yeniden, başka yerde `--reflink=auto` ile. Başarısız bir kopya yarım bir şey bırakmaz; `cp` kabuksuz, imece'nin temizlenmiş ortamıyla çalışır.
- Günlük kopyalananı tek satırda söyler, `.env, target → claude-1 çalışma ağacına kopyalandı (1.2 GiB)`, ve atlanan her yolu nedeniyle. Zaten orada olan bir yol hiçbir şey söylemez.

### `.worktreeinclude`: checkout bunları kendisi sayar

Bir checkout neyin kopyalanacağını kendisi de sayabilir: tepesindeki bir `.worktreeinclude` dosyasında, gitignore sözdizimiyle (agent-deck ve Claude Code'un masaüstü uygulaması da aynı dosyayı okur):

```gitignore
# imece'nin yaptığı her worktree'ye kopyalanır
.env*
node_modules/
```

imece onu her işçinin başlangıcında `:copy` listesinin yanında okur. Git, desenlerinin izlemediği yollar arasında neye uyduğunu sayar (desenlerin bütünüyle uyduğu bir dizin tek yoldur) ve bunlardan yalnız git'in yok saydığı kopyalanır: listenin yollarından sonra, aynı kurallarla, aynı 2 GiB içinde. Listede zaten olan bir yol listenin sayılır. Sizin evetiniz gerekmez: dosya yalnız kendi checkout'unuzun yollarını imece'nin worktree'lerine kopyalanmak üzere sayar.

- **Reddedilen ve her başlangıçta günlükte söylenen:** bir bağlantı, düz dosya olmayan, 16 KiB'tan büyük ya da metin olmayan dosya; kontrol karakteri olan, 200 bayttan uzun, `..` parçası olan ya da `.git`'i anan bir satır (dosyanın geri kalanı uygulanır); desenleri git'in yok saydığı 100'den fazla yola uyan ya da git'in neye uyduklarını 10 sn'de söylemediği bütün dosya (o zaman ondan hiçbir şey kopyalanmaz).
- Günlük dosyanın kopyalarını kendi satırında söyler, `node_modules → claude-1 çalışma ağacına kopyalandı (310 MiB), .worktreeinclude'un saydığı`; atlananı `node_modules (.worktreeinclude'dan) → claude-1 çalışma ağacına kopyalanmadı: …` diye; reddedilen bir satırı `.worktreeinclude'un 3. satırı (../shared), claude-1 çalışma ağacı için uygulanmadı: reaches outside the checkout` diye.
- Checkout'ta böyle bir dosya varsa tek başına `:copy` sonuna `; bir de .worktreeinclude'un saydıkları` ekler; dosyanın neye uyduğunu listelemez.
- Demoda ikinci projenin checkout'u böyle bir dosyası varmış gibi davranır: `node_modules`'ü sayar ve dışarıya uzanan bir satırı her başlangıçta reddedilir.

Bilinen sınırlar: checkout'ta sonradan değiştirdiğiniz bir dosya yeniden kopyalanmaz (worktree'de vardır); kopyalama işçi başınadır, paylaşılmaz; 2 GiB bütçesi, girdi tavanı ve `.worktreeinclude`'un 100 yolu sabittir; `.worktreeinclude` checkout'tan diskte olduğu gibi okunur, yani commit edilmemiş bir düzenleme de sayılır.

## Worktree kurulum betiği

Bir worktree'de iş yapılması gerektiğinde kopya yetmez: bağımlılıkları kurmak, migration çalıştırmak. Checkout'unuzda `.imece/worktree-setup.sh` yoluna bir kabuk betiği koyun; imece onu her yeni worktree'de, kopyalardan sonra ve ajanın oturumu başlamadan önce çalıştırır, **tam baytlarına evet dediğinizde**:

```sh
#!/bin/sh
set -e
npm ci --no-audit
npm run db:migrate
```

- **Ne zaman:** bir işçinin ya da bir solonun az önce yapılan worktree'sinde (bir yardımcı işçisininkinde, bir lider checkout'unuzda çalışır; zaten orada olan bir worktree yeniden kurulmaz). Git yoksa worktree yoktur, dolayısıyla betik de yoktur.
- **Evetiniz:** evet demediğiniz bir betiği bulan ilk yeni worktree SENİ BEKLEYEN'e bir soru koyar (`▶ claude-1: worktree kurulum betiği .imece/worktree-setup.sh yeni worktree'sinde çalışsın mı? enter gösterir`); işçi beklemez, betiksiz başlar. `enter` betiği imece'nin okuduğu ve hash'lediği haliyle, her satırı numaralı, neyin nasıl çalışacağıyla açar (`KURULUM BETİĞİ · claude-1 · sha256 1a2b…`); `j` `k`, `ctrl-d` `ctrl-u`, `g` `G` gezinir, `a` evet, `r` hayır der, `esc` kapatır. `a` ve `r` seçili soruda da çalışır. Bir evet betiği onsuz kalan worktree'lerde hemen, sonra her yenisinde oturumdan önce çalıştırır; bir hayır bu çalıştırma boyunca geçerlidir (sonraki bir başlangıç yine sorar). Soru beklerken başlayan işçiler aynı soruya sayılır.
- **Evetin neye bağlı olduğu:** baytların SHA-256'sı, dosyanın gerçek yolu (bağlar çözülmüş) ve nasıl çalıştığı (`/bin/sh`); deposunda değil, imece'nin deposunda saklanır. Değişen bir bayt yeni bir betiktir ve yeniden sorulur. Çalışan, okuduğunuz baytların imece'nin durum dizinine yazılmış özel bir kopyasıdır (`0700`, kopya `0600`), dosyanın kendisi asla; böylece evetinizle çalışma arasında hiçbir şey onu değiştiremez.
- **Nasıl çalışır:** o kopya üzerinde `/bin/sh`, yeni worktree'de, imece'nin temizlenmiş ortamı ve `IMECE_REPO` (checkout'unuz) ile `IMECE_WORKTREE` (worktree) ile, kendi süreç grubunda, en çok 60 saniye (sonra grup öldürülür). Betik en çok 64 KiB.
- **Ne görürsünüz:** günlük nasıl bittiğini ve son satırını söyler (`claude-1: worktree kurulumu çalıştı, exit 0, 4.2 s · migrations: 7 applied`, ya da `başarısız`); okunamayan ya da başlatılamayan bir betik nedenini söyler (`worktree kurulumu çalışmadı: …`). İşçi her durumda başlar.
- **Deponun kontrolü** (`.imece/check`) aynı koşullarla onaylanır ve çalışır; bir işçinin worktree'sinde oradan okunan her değişiklikte; bkz. [değişiklikler](changes.md#deponun-kontrolü).
- **Sökümü:** checkout'unuzdaki `.imece/worktree-teardown.sh`, `:tidy` bir worktree'yi silmeden önce onun içinde çalışır (aşağıda), aynı koşullarla: tam baytlarına evetiniz, özel bir kopya, `/bin/sh`, `IMECE_REPO` ve `IMECE_WORKTREE`, kendi süreç grubu, 60 saniye. Kurulumun worktree dışında yaptığını geri alır (bir veritabanı, bir container, bir port). Henüz yok: bir işçinin kendi worktree'sini silmek; bu yüzden söküm yalnızca hiçbir ajanın sahip olmadığı worktree'ler için çalışır.

## Seyrek (sparse) checkout

Büyük bir depoda ağacın yalnızca bir kısmını checkout'ta tutabilirsiniz (`git sparse-checkout`). imece'nin böyle bir checkout'tan yaptığı worktree de onun kadar seyrektir: imece onu hiçbir şey checkout etmeden ekler, checkout'unuzun dizgilerini verir (`git sparse-checkout set`, sizinki gibi koni kipinde ya da değil) ve ancak ondan sonra checkout eder; böylece dışarıda bıraktığınız hiçbir şey içine yazılmaz. Dizgiler worktree'nin kendisinindir: orada değiştirmek sizinkini değiştirmez, sizinkini sonradan değiştirmek de önceden yapılmış bir worktree'yi değiştirmez. Aynı seyreklikte yapılamayan bir worktree (2.35'ten eski git) yeniden silinir ve ajan başlamaz; günlük nedenini söyler, böylece bir monorepo arkanızdan bütünüyle yazılmaz.

## Sahibi olmayan worktree'ler

Her işçinin worktree'si çalıştırmalar arasında diskte kalır, böylece işi asla atılmaz. imece'nin artık tanımadığı bir ajanınki de kalır: bir çökmeden, silinen bir depodan, hiç başlamamış bir çataldan sonra. `:tidy` onları bir pencerede, her birini checkout'u ve tuttuklarıyla listeler:

```text
 SAHİBİ OLMAYAN WORKTREE'LER · 2
▶ claude-31  ~/src/shop  temiz
  claude-32  ~/src/shop  değişen dosya: 3 · kendi commit'i: 1
 a temizleri sil   d bunu sil   j k gez   esc geri
```

- **Ne sayılır:** imece'nin durum dizininde (`worktrees/<checkout>-<hash>/<ad>`) git'in bir checkout'a eklenmiş worktree olarak bildiği ve hiçbir projenin hiçbir ajanının (açık, kapalı ya da arşivlenmiş) sahip olmadığı bir dizin; bir yardımcınınki işçisinindir. Bağlar, dosyalar ve git'in bilmedikleri atlanır.
- **Ne tutar:** değişen ya da izlenmeyen dosyalar (`git status`; git'in yok saydıkları, `target/` gibi, sayılmaz ve onunla gider) ve hiçbir dalın, etiketin ya da uzak deponun sahip olmadığı kendi commit'leri (imece'nin uyguladığınız değişiklikler için yaptığı commit'ler zaten checkout'unuzdadır ve sayılmaz). Temiz, ikisi de yok demektir.
- **Silmek:** `a` temiz olanları, `d` eldekini siler. İş tutan biri ancak onun için bir `y`'den sonra, `git worktree remove --force` ile gider; temiz olan `git worktree remove` ile, ki arada bir şey değiştiyse yine reddeder. Silme ekranın dışında çalışır ve pencere ardından yeniden bakar.
- **Önce söküm betiği:** worktree'nin checkout'unda `.imece/worktree-teardown.sh` varsa silmeden hemen önce worktree'de çalışır. Yanıtlamadığınız bir betik önce pencerenin üstünde, kurulum betiğininki gibi açılır (`SÖKÜM BETİĞİ · shop · sha256 1a2b…`, satırları numaralı, `j` `k` `ctrl-d` `ctrl-u` `g` `G` kaydırır): `a` evet der (imece'nin deposunda kurulum betiklerinin evetleriyle birlikte tutulur, değişen bir bayt yeniden sorar) ve silme sürer, `r` bu çalıştırma için hayır der ve onsuz siler, `esc` hiçbir şey silmez. Flaş nasıl gittiğini söyler (`silindi: claude-31 · claude-31 söküldü: exit 0, 0.4 s · dropped database`); başarısız olan ya da okunamayan bir söküm söylenir, silme yine kendi karar verir: worktree'de bir dosya bırakan bir söküm onu tutar.
- **Asla bir ajanınki değil:** kimin neye sahip olduğu bir silme istendiğinde yeniden okunur; o arada işe alınan ve adı eski worktree'ninki olan bir işçi (onu devralırdı) onu tutar (`silinmedi: claude-1 (şimdi bir ajan içinde çalışıyor)`). Worktree'si silinirken başlayan bir işçi gitmesini bekler ve yenisini alır.
- **Başlangıçta:** imece ekranın dışında bir kez bakar ve bulduysa `sahibi olmayan 2 worktree var: :tidy listeler` der; kendiliğinden bir şey silmez.
- Demoda biri temiz, biri iş tutan iki uydurma worktree vardır; `:tidy` onları canlıdaki gibi siler. Lonca'da bir proje varsa okunamayan bir workspace de vardır.

## Lonca'da

[Lonca](https://github.com/selcukgural/lonca), imece'nin depo işlerinin gideceği yerdir: bir deponun tek yazarı olan bir daemon, sembol kiralamaları, bir birleştirme kuyruğu ve imzalı bir günlük. İlk adımlar burada: imece bir depoyu daemon üzerinden okur, `:lonca AD` eldeki proje için bir depo adlandırır, `:lonca import` da çalışma kopyasından bir depo oluşturur. Bundan sonra işe alınan işçiler Lonca workspace'lerinde çalışır (aşağıda); worktree'deki bir işçinin Δ'sı yine çalışma kopyana iner.

imece, işbirlikçi kipte çalışan bir `lonca daemon`'ın istemcisidir: soket `LONCA_SOCKET`, yoksa `$LONCA_HOME/lonca.sock`, yoksa `~/.lonca/lonca.sock` olur ve senin olmalıdır. Yalnız protokol 1 konuşan bir daemon'la konuşur ve senin adına, en çok bir gün süren bir oturumla okur. `:lonca login` bir oturum açar (imece o zaman `lonca human login` çalıştırır, kendiliğinden asla: Secure Enclave anahtarı parmağını ister). Birden çok kimliğin varsa `LONCA_HUMAN` hangisi olduğunu söyler.

`:lonca import`, çalışma kopyasından bir depo oluşturur: üzerinde olduğun dal ve geçmişi, projenin adıyla ya da verdiğin adla Lonca'nın deposuna kopyalanır. Önce bir pencere neyin gideceğini söyler ve yalnız `a` çalıştırır, çünkü imece bunu geri alamaz; çalışma kopyan olduğu gibi kalır, commit edilmemiş olan gitmez. Yeni bir daemon'da ilk adım budur: hiç deposu olmayan bir daemon kimsenin oturumunu açamaz, bu yüzden aktarma oturumunu da açar. Bir öneri için fazla büyük dosya günlükte uyarılır.

Her 5 saniyede bir projenin satırı nerede durduğunu söyler:

| Satır | Anlamı |
| --- | --- |
| `lonca kule 693d629 · 2 bekliyor` | daemon yanıt veriyor: trunk'ın commit'i, insan bekleyenler |
| `lonca kule 693d629 ↓3` | çalışma kopyan trunk'ın üç entegrasyon gerisinde: `:sync` |
| `lonca kule 693d629 ↓?` | çalışma kopyanın commit'i hiç trunk olmadı ve henüz hiçbir şey çekilmedi |
| `lonca kule …` | az önce adlandırıldı, henüz yanıt yok |
| `⌀ lonca yok` | okunamıyor; günlük nedenini bir kez söyler |

Nedenler: hiçbir daemon yanıt vermiyor, soketteki senin daemon'ın değil, başka bir protokol, oturum açmamışsın, ya da bu adda bir depoya kayıtlı değilsin (daemon olmayan bir depo için de aynısını söyler, adı denetle ya da `:lonca import` ile oluştur).

Yanıt verene dek proje kapalıdır: kimse işe alınmaz, `:fork` reddedilir, hiçbir Δ uygulanmaz, reddedilmez, geri gönderilmez. Ajanlar çalışmayı sürdürür, Δ'leri bekler. Hiçbir depoda olmayan bir proje sokete hiç dokunmaz. Bkz. [`:lonca`](../reference/commands.md#lonca-nameoffloginimport).

Lonca'daki bir projede işe alınan işçi, worktree'de değil deponun bir **workspace**'inde çalışır: aynı yolda aynı dosyalar, `.git` olmadan; Claude Code'unda imece'nin veri yolunun yanında `lonca hook` ve `lonca mcp` bulunur. Odak ekranı `· lonca AD için workspace` der. `lonca` programı `PATH`'inde olmalıdır. Zaten worktree'si olan işçi orada kalır. İşçi düzenlediğini kiralar, değişikliğini önerir ve öneri bir insanı bekler: projenin satırı onu sayar (`2 pending`). Şimdilik kararı kendi kabuğunda `lonca approve` ile verirsin; SENİ BEKLEYEN'deki Δ sonraki bir adımda gelecek. Bir oturum 8 saat sürer; oturumundan yarım saatten az kalmış bir işçiye yazıldığında aynı workspace'te yeni bir oturumla, konuşması, dosyaları ve önerisiyle sürer, yeniden kiralar. Yardımcısı çalışan işçi yardımcının bitmesini bekler, uyuyan takıma da hiçbir şey olmaz. Ayrılan işçi oturumunu kapatır; önerileri kalır, workspace'i de `:tidy` için kalır. Lonca'daki işçi trunk'tan başlar, bu yüzden `hire`'ın `from`'u reddedilir, `:fork` da onu reddeder. Secure Enclave anahtarıyla her yeni oturum parmağını ister: `session start` imzalıdır.

Lonca'nın işbirlikçi kipinde hiçbir oturum gözlenmiş sayılmaz, bu yüzden her öneri seni bekler, kritik bir yoldaki değişiklik (`Cargo.toml`, kilit dosyaları, `package.json`, `.github/**`, `build.rs`) ise reddedilir: işçi takıldığını söyler, o değişikliği sen yaparsın. `lonca.toml`'da `unobserved_is_tainted = false` yapma: öneriler o zaman kimse bakmadan birleşir.

Çalışma kopyan deponun bir istemcisidir, bu yüzden trunk onsuz ilerler. [`:sync`](../reference/commands.md#sync) onu getirir: `lonca fetch` trunk'ı `refs/lonca/trunk`'a koyar, dalın da fast-forward olabiliyorsa ilerler. imece dalını asla merge, rebase ya da reset etmez: kendi commit'lerin varsa hiçbir şey kımıldamaz ve günlük bunu söyler, yoldaki iş ise git'in kendi sözleriyle durdurur.

## Demonun kopyaları

`--demo --projects N` (1-4, varsayılan 2) aynı anda birkaç sahte proje gösterir; oradaki `:open YOL [AD]` bir lider, iki işçi ve üç kartla sahte bir proje ekler, `:close` ve `:rename` aynı şekilde çalışır, `:pin` deposuz sabitler ve `:copy` bir liste tutar; demo işçisinin başlangıcı onu uydurma boyutlarla "kopyalar", ara sıra birini checkout'tan gitmiş diye atlar. Demonun liderinin işe aldığı bir işçi yeni bir worktree alır; demonun kendi kurulum betiği (`npm ci`, `npm run db:migrate`) sorulur, `enter` ile gösterilir ve bir evette canlıdaki gibi uydurma çıktıyla "çalışır".

## Ayrıca bakınız

- [Salon](../screen/salon.md): katlama, dikkat sırası, `f`.
- [Birçok proje](../scenarios/many-projects.md): işlenmiş bir örnek.
- [Oturumlar](../concepts/sessions.md): deponun proje başına tuttuğu.
- [Komutlar](../reference/commands.md).
