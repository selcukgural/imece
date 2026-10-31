---
title: Projeler
description: Tek imece'de birkaç checkout - :open, :close, :rename, proje başına bir lider, salonda katlanan gruplar, :pin ve bir işçinin worktree'sine kopyalanan yok sayılan yolların proje başına :copy listesi.
---

# Projeler

Bir proje, kendi lideri ve işçileri olan bir git checkout'udur. imece, içinde çalıştığı checkout'la ilk proje olarak başlar (lider `lead`); daha fazlasını ekrandan açarsınız ve her biri salonda bir gruptur. Tek bütçe ve tek bus hepsine hizmet eder.

## Açmak, kapatmak, yeniden adlandırmak

| Komut | Ne yapar |
| --- | --- |
| `:open YOL [AD]` (`:o`) | YOL'daki git checkout'unu kendi başına bir proje olarak açar; ekibin lider satıcısında (aynı model ve kademeler) `lead-AD` adlı yeni bir liderle. AD varsayılan olarak dizinin güvenli hale getirilmiş adıdır: 1-24 harf, rakam, `.`, `_`, `-`; kalanı `-` olur. `:` menüsü `$` ile işaretler: yeni bir lider oturumu. |
| `:close [AD]` (`:c`) | projenin ajanlarının oturumlarını ve çalıştırmalarını bitirir, postalarını, notlarını, tekliflerini ve değişikliklerini düşürür, gelen kutusu öğelerini kapatır, projeyi kapalı işaretler. Worktree'ler diskte kalır. Ad yoksa eldeki proje. Son açık proje kapanmayı reddeder (`q q` çıkar). |
| `:rename AD` | eldeki projeyi ve liderini `lead-AD` olarak yeniden adlandırır (ilk projenin de). Oturumlar sürer; projenin her canlı ajanı sonraki isteminde bir not alır ve eski ada `send` ekip listesiyle yanıtlanır. |

- Kapalı bir proje kimliğini korur; aynı checkout yeniden açılınca yeni bir liderle onu yeniden kullanır. Sabitlemesi o zaman için saklanır.
- Adlar küreseldir: `@lead-api` o projenin liderine her yerden ulaşır; `@`'den sonraki tamamlama önce eldeki projeyi listeler.
- Boşluklu bir YOL ayrıştırılmaz (tırnak yok). `:open ` dosya sisteminden dizinleri tamamlar (gizli olanlar `.`'dan sonra, ev için `~`); `:close ` ve `:pin ` açık projelerin adlarını tamamlar.
- Günlük: `api projesini açtın (/src/api)`, `api projesini kapattın`, `api projesine gateway adını verdin`.
- Projeler depoda hatırlanır; checkout'u gitmiş hatırlanan bir proje kapalı yüklenir.

Lider satıcısı proje başına seçilemez: her lider ekibin lider satıcısında çalışır.

## Salonda gruplar

```text
 SALON
 ▴ api        ! 1 bekliyor
   ◆ lead-api        koordine ediyor
   ● claude-1        #4 auth: refresh      +12 −3
 ▸ shop       2 çalışıyor
 ▾ imece
   ◆ lead            boşta
   ✓ codex-1         bitti, henüz bakılmadı
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

Bilinen sınırlar: checkout'ta sonradan değiştirdiğiniz bir dosya yeniden kopyalanmaz (worktree'de vardır); kopyalama işçi başınadır, paylaşılmaz; 2 GiB bütçesi ve girdi tavanı sabittir.

## Demonun kopyaları

`--demo --projects N` (1-4, varsayılan 2) aynı anda birkaç sahte proje gösterir; oradaki `:open YOL [AD]` bir lider, rastgele satıcılardan iki işçi ve üç kartla sahte bir proje ekler, `:close` ve `:rename` aynı şekilde çalışır, `:pin` deposuz sabitler ve `:copy` bir liste tutar; demo işçisinin başlangıcı onu uydurma boyutlarla "kopyalar", ara sıra birini checkout'tan gitmiş diye atlar.

## Ayrıca bakınız

- [Salon](../screen/salon.md): katlama, dikkat sırası, `f`.
- [Birçok proje](../scenarios/many-projects.md): işlenmiş bir örnek.
- [Oturumlar](../concepts/sessions.md): deponun proje başına tuttuğu.
- [Komutlar](../reference/commands.md).
