---
title: "Üç çalışma kopyası, tek ekran"
description: Tek imece'de birkaç proje, her birinin kendi lideri; açma, katlama, sabitleme, süzme, yeniden adlandırma, yok sayılan yolları kopyalama, kapatma ve paylaştıkları bütçe.
---

# Üç çalışma kopyası, tek ekran

Tek bir imece aynı anda birkaç çalışma kopyası çalıştırır. Her biri ekipte kendi lideri olan bir projedir; tek bütçeyi, tek veri yolunu ve tek ekranı paylaşırlar. Bununla iş depolar arasına yayıldığında ya da uzun süren bir projeyi içinde bulunduğunuzun yanında açık tuttuğunuzda karşılaşırsınız. Bu senaryo imece'nin içinde başladığı projenin yanına iki proje daha açar ve üç takımı okunur tutan tuşları kullanır.

imece `~/code/cli` içinde başlatıldı; projesi dizinin adını aldı, `cli`, lideri `lead`.

## 1. İki proje daha açın

`:open ~/code/api api` yazın ve `enter`a basın. Anlık mesaj `api açıldı (/Users/you/code/api); lead-api adına yaz` der; günlükte `› api projesini açtın (/Users/you/code/api)`. `~` ev dizininizdir. İkinci projenin yolu imece'nin başlatıldığı yere göredir ve adı dizine bırakılır:

```text
 › komut: :open ../web
```

```text
 15:00 › web projesini açtın (/Users/you/code/web)
```

`:open YOL [AD]`, YOL'daki git çalışma kopyasını açar; AD, dizinin güvenli kılınmış adıdır (1-24 harf, rakam, `.`, `_`, `-`). Yeni projenin lideri `lead-AD`dır; aynı model ve kademelerle. `:open ` yazarken dizinleri tamamlar (gizli olanlar bir `.`ten sonra, `~` çalışır). `:` menüsü `:open`ı `$` ile işaretler: yeni bir lider, ilk yazdığınızda para tutan yeni bir oturumdur. Çalışma kopyası olmayan bir yol, alınmış bir ad ya da zaten açık bir çalışma kopyası nedeniyle reddedilir (`api zaten açık, adı api`). `:o` kısaltmasıdır.

Başlık açık projeleri adlandırır. Ana ekranın ÇALIŞAN kısmı satırlarını projeye göre, her biri bir başlık altında gruplar; birden fazla proje açıkken bir satır projesinin adını taşır. Listede kimsesi olmayan projenin başlığı olmaz: yapacak işi olmayan bir liderin satırı yoktur, bu yüzden `api` ve `web`, liderleri çalışana dek yalnızca üst başlıkta görünür:

```text
 imece  cli · api · web │ $2.10 / $50.00  15:00
 ÇALIŞAN 1
▸▾ cli           ▓▓▓▓░░░░ 1/2  1 çalışıyor  $2.10  ~/code/cli
 ● cli         #2   docs: man sayfası         docs/imece.1 yazıyor · 4 dk
```

`/` eldeki projenin liderine yazar: ana ekranı `]` ile `api`ye daraltın (4. adım) ya da satırlarından birini seçin, sonra `/`. `@lead-web mesaj` her yerden tek bir lidere yazar.

## 2. Katlayın ve açın

Sonra `lead-web` bir işçi alır ve `web`in kendi başlığı olur. `web`in bir satırı eldeyken (başlığı ya da ajanlarından biri) `h`ye basın (ya da `←`). Proje başlığına katlanır; `l` açar. `za` değiştirir, `zM` her projeyi katlar, `zR` hepsini açar (`:fold`, `:unfold`). Başlıkta `enter` ise liderin ayrıntısını açar. Katlı bir proje sessizdir, sağır değil: başlığı sizi bekleyenleri saymayı sürdürür, karar bekleyenler de yukarıda KARAR BEKLEYEN'de kalır.

```text
 ▸ web            ! 1 bekliyor   1 çalışıyor  $0.80
```

Başlıkta `s`, projenin tamamını sizi bekleyene dek ÇALIŞAN'dan gizler (`:unsettle` geri getirir); ÇALIŞAN başlığı gizlenenleri sayar.

## 3. İçinde yaşadığınızı sabitleyin

`:pin api` yazın ve `enter`a basın: `api ekibin en üstüne sabitlendi`. Proje başlığı `▴` taşır ve açılma sırası ne olsa önde kalır; en son da katlanır. `:pin` tek başına eldeki projeyi sabitler; `:pin` yine bırakır (`api artık sabit değil`). Sabitleme kayıt dosyasında çalışmalar arasında saklanır ve kapatılan bir proje yeniden açılınca sabitini korur.

```text
 ▾ ▴ api         2 çalışıyor  $3.10
```

Proje başına renk henüz yapılmadı.

## 4. Her seferinde tek proje

`]`'ye basın. Ana ekran tek projeye daralır: başlık satırı adını aydınlatır, KARAR BEKLEYEN ve ÇALIŞAN yalnız onun satırlarını gösterir. `]` yine sonraki projeye geçer, sonuncudan sonra yine hepsine döner; `[` ters yöne gider. Daraltma yalnızca bu çalışma içindir. `W` yine her yerden karar bekleyen sıradaki öğeyi açar.

## 5. Yeniden adlandırın

`api`nin bir satırı seçiliyken `:rename backend` ve `enter`: `api artık backend; lead-backend adına yaz`. Lider onunla birlikte adlandırılır, oturumlar sürer ve projenin canlı her ajanı sonraki isteminde bir not alır; eski ada `send`, takım listesiyle yanıtlanır. Günlük: `› api projesine backend adını verdin`. Proje 0 da adlandırılabilir (lideri `lead-AD` olur).

## 6. Git'in yok saydığını kopyalayın

Bir işçinin worktree'si taze bir çalışma kopyasıdır, git'in yok saydığı şeyler orada yoktur: `.env`, `node_modules`, `target/`. `web` için satırlarından birini seçin ve `:copy .env` yazın:

```text
 .env, web çalışma ağaçlarında yoksa çalışan başlarken kopyalanacak
```

Liste proje başınadır ve varsayılan olarak boştur. `:copy .env` yine yolu düşürür; `:copy` tek başına listeyi söyler (`web çalışma ağaçlarına kopyalananlar: .env, node_modules`, ya da `web çalışma ağaçlarına hiçbir şey kopyalanmıyor; :copy YOL git'in yok saydığı bir yolu ekler`). `:` menüsü komuttan sonra listeyi gösterir. Bir yol göreli, düz (glob yok), çalışma kopyasında var, bağ değil, çözüldüğünde çalışma kopyasının içinde ve git tarafından yok sayılmış olmalıdır; değilse nedeniyle reddedilir:

```text
 src/main.rs kopyalanamaz: git does not ignore it: a tracked file is in the worktree already
```

`lead-web` bir işçi alınca, yeni worktree'de eksik olan listedeki yollar oturum başlamadan kopyalanır (`cp -R -P -p`, macOS'ta bir APFS klonu, birimin desteklediği yerde reflink), bir başlangıcın hepsi 2 GiB içinde. Günlük neyin gittiğini ve neyin atlandığını söyler; zaten orada olan yol bir şey söylemez:

```text
 15:12 ⚑ .env, node_modules → claude-2 çalışma ağacına kopyalandı (412 MiB)
 15:12 ■ target → claude-2 çalışma ağacına kopyalanmadı: more than what is left of the copy budget (1715470336 bytes)
```

Yardımcılar işçilerinin worktree'sini paylaşır ve aynısını alır; bir lider çalışma kopyasında çalışır ve asla kopya almaz. Çalışma kopyasında sonradan değiştirdiğiniz bir dosya yeniden kopyalanmaz.

## 7. Araştırmacılar

Bir araştırmacı katmanı (takım için dış dünyayı okuyan, her projenin paylaştığı bir tavan altında ajanlar) tasarlandı ama henüz yapılmadı: canlı imece başlıkta araştırmacı sayısı göstermez. `imece --demo --researchers N` önizlemesini verir: demoda tavan bütün projeler için tek havuzdur.

## 8. Birini kapatın

`:close web` yazın ve `enter`a basın:

```text
 web kapatıldı: agent'ları ayrıldı; worktree'leri diskte kaldı
```

Ajanlarının oturumları ve çalıştırmaları biter, postaları, teklifleri ve değişiklikleri düşer, gelen kutusu öğeleri kapanır; worktree'ler `~/.local/state/imece/worktrees/` altında kalır. `:close` tek başına (ya da `:c`) eldeki projeyi kapatır. Son açık proje reddeder: `cli son açık proje; q q çıkar`. Aynı çalışma kopyası sonra yeniden açılınca yerini yeni bir liderle yeniden kullanır. Günlük: `› web projesini kapattın`.

## 9. Hepsi için tek bütçe

Başlıkta bütün projeler için tek bütçe vardır: `$5.90 / $50.00`. Şimdiki hızla yarım saatten az kalınca dikkat rengine döner ve ne kadar yeteceğini ekler (`~25 dk kaldı`); hızın kendisi `:` menüsünde `:budget` notundadır (`$5.90 / $50.00 · $8.20/sa, ~5 sa 20 dk kaldı`). `:budget 80` hepsi için artırır; ana ekranda her projenin başlığı kendi ajanlarının harcadığını gösterir. Bütçe (kese) bitmeye yakınken günlük bir kez, ilk açık projenin liderinin altında söyler:

```text
 15:40 ! $8.20/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır
```

Bütçe bitince hiçbir projede kimse bir tur daha almaz. Kayıt dosyasını aynı anda tek bir imece tutar; başka yerde başlatılan ikincisine, ilkinden `:open` kullanması söylenir.

## Ayrıca bakın

- [Projeler](../concepts/projects.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Ana ekran](../screen/home.md): KARAR BEKLEYEN, ÇALIŞAN, katlama, gizleme
- [Komutlar](../reference/commands.md)
- [Dosyalar](../reference/files.md): worktree'ler ve kayıt dosyası nerede
