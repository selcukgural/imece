---
title: "Üç çalışma kopyası, tek ekran"
description: Tek imece'de birkaç proje, her birinin kendi lideri; açma, katlama, sabitleme, süzme, yeniden adlandırma, yok sayılan yolları kopyalama, kapatma ve paylaştıkları bütçe.
---

# Üç çalışma kopyası, tek ekran

Tek bir imece aynı anda birkaç çalışma kopyası çalıştırır. Her biri salonda kendi lideri olan bir projedir; tek bütçeyi, tek veri yolunu ve tek ekranı paylaşırlar. Bununla iş depolar arasına yayıldığında ya da uzun süren bir projeyi içinde bulunduğunuzun yanında açık tuttuğunuzda karşılaşırsınız. Bu senaryo imece'nin içinde başladığı projenin yanına iki proje daha açar ve üç takımı okunur tutan tuşları kullanır.

imece `~/code/cli` içinde başlatıldı; projesi dizinin adını aldı, `cli`, lideri `lead`.

## 1. İki proje daha açın

`:open ~/code/api api` yazın ve `enter`a basın. Anlık mesaj `api açıldı (/Users/you/code/api); lead-api adına yaz` der; günlükte `› api projesini açtın (/Users/you/code/api)`. `~` ev dizininizdir. İkinci projenin yolu imece'nin başlatıldığı yere göredir ve adı dizine bırakılır:

```text
 › komut: :open ../web
```

```text
 15:00 › web projesini açtın (/Users/you/code/web)
```

`:open YOL [AD]`, YOL'daki git çalışma kopyasını açar; AD, dizinin güvenli kılınmış adıdır (1-24 harf, rakam, `.`, `_`, `-`). Yeni projenin lideri `lead-AD`dır; takımın lider satıcısında, aynı model ve kademelerle (lider satıcı proje başına seçilemez). `:open ` yazarken dizinleri tamamlar (gizli olanlar bir `.`ten sonra, `~` çalışır). `:` menüsü `:open`ı `$` ile işaretler: yeni bir lider, ilk yazdığınızda para tutan yeni bir oturumdur. Çalışma kopyası olmayan bir yol, alınmış bir ad ya da zaten açık bir çalışma kopyası nedeniyle reddedilir (`api zaten açık, adı api`). `:o` kısaltmasıdır.

Salon artık ajanları projeye göre gruplar, her birinin bir başlık satırı vardır; birden fazla proje varken gelen kutusu ve günlük satırları projenin adını taşır:

```text
 ▾ cli                        1 çalışıyor  $2.10
   ◆ lead       claude          koordine ediyor                   $0.40
   ● claude-1   #2   docs: man sayfası    docs/imece.1 yazıyor      $1.70
 ▾ api                        0 çalışıyor  $0.00
   ◆ lead-api   claude          boşta
 ▾ web                        0 çalışıyor  $0.00
   ◆ lead-web   claude          boşta
```

`/` seçili projenin liderine yazar; önce `api` içinde bir satır seçin (`j k`), sonra `/`. `@lead-web mesaj` her yerden tek bir lidere yazar.

## 2. Katlayın ve açın

`web` başlığını seçin ve `h`ye basın (ya da `←`, ya da başlıkta `enter`). Proje tek satıra katlanır; `l` açar. `za` değiştirir, `zM` her projeyi ve işçiyi katlar, `zR` hepsini açar (`:fold`, `:unfold`). Katlı bir proje sessizdir, sağır değil: satırı sizi bekleyenleri saymayı sürdürür.

```text
 ▸ web                        1 çalışıyor  $0.80  ! 1 bekliyor
```

Salon herkese dar gelince sakin işçiler alttan yukarı `… N agent yolunda` içine katlanır; sizi bekleyen işçiler (onayınızı bekleyen, takılan, bitmiş ama bakılmamış) projelerinde öne sıralanır, bu yüzden en son katlanırlar.

## 3. İçinde yaşadığınızı sabitleyin

`:pin api` yazın ve `enter`a basın: `api salonun en üstüne sabitlendi`. Başlığı `▴` taşır ve açılma sırası ne olsa önde kalır; en son da katlanır. `:pin` tek başına eldeki projeyi sabitler; `:pin` yine bırakır (`api artık sabit değil`). Sabitleme kayıt dosyasında çalışmalar arasında saklanır ve kapatılan bir proje yeniden açılınca sabitini korur.

```text
 ▾ ▴ api                      2 çalışıyor  $3.10
```

Proje başına renk henüz yapılmadı.

## 4. Yalnızca sizi bekleyenler

`f`ye basın. Salon yalnızca sizden bir şey bekleyen ajanları gösterir, her biri projesinin başlığı altında; bir şeyi olmayan proje dışarıda kalır. Başlık ` SALON · yalnızca seni bekleyenler (f) ` olur, anlık mesaj `salon yalnızca seni bekleyenleri gösteriyor · f: herkes`. Hiç yoksa:

```text
 Şu an senden bir şey gerekmiyor · f herkesi gösterir
```

`f` yine herkesi gösterir. Süzgeç yalnızca bu çalışma içindir. `W`, süzgeç açık ya da kapalı, her yerden sizi bekleyen sonraki öğeyi açar.

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
 15:12 ✕ target → claude-2 çalışma ağacına kopyalanmadı: more than what is left of the copy budget (1715470336 bytes)
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

Başlıkta bütün projeler için tek bütçe satırı vardır: `$5.90 / $50.00 harcandı ($8.20/sa, ~5 sa 20 dk kaldı)`. `:budget 80` hepsi için artırır; her projenin başlık satırı kendi ajanlarının harcadığını gösterir. Bütçe (kese) bitmeye yakınken günlük bir kez, ilk açık projenin liderinin altında söyler:

```text
 15:40 ! $8.20/sa hızla bütçe yaklaşık 25 dk daha yeter; :budget USD ile artır
```

Bütçe bitince hiçbir projede kimse bir tur daha almaz. Kayıt dosyasını aynı anda tek bir imece tutar; başka yerde başlatılan ikincisine, ilkinden `:open` kullanması söylenir.

## Ayrıca bakın

- [Projeler](../concepts/projects.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Salon](../screen/salon.md): katlama, sıra, filo
- [Komutlar](../reference/commands.md)
- [Dosyalar](../reference/files.md): worktree'ler ve kayıt dosyası nerede
