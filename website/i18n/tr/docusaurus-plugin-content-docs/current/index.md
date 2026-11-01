---
title: imece nedir?
description: Birçok satıcıdan birçok kodlama ajanı için tek terminal; tek kod tabanı, son kapı sizsiniz.
---

# imece nedir?

imece bir kodlama ajanı takımını (Claude Code, Codex, Gemini CLI) tek kod tabanında, tek ekranda çalıştırır. Bir lider ajan isteğinizi alır, iş gerektirdikçe işçi tutar ve bir işçinin yaptığı her değişiklik, checkout'unuza dokunmadan önce sizi bekler. imece ile, tek bir kodlama ajanının yetmediği, birkaçının, birkaç satıcıdan, aynı depoda birbirinin ve sizin ayağınıza basmadan çalışması gerektiği anda karşılaşırsınız.

## Sorun

Tek terminalde tek kodlama ajanı basittir. Birkaçı değildir:

- **Çok ajan, çok satıcı.** Claude Code, Codex ve Gemini CLI'ın her birinin kendi komut satırı, kendi izin soruları, kendi oturum anlayışı ve bir turun neye mal olduğunu söylemenin kendi yolu var. Üçünü çalıştırmak üç pencere ve üç kural seti demek.
- **Tek kod tabanı.** Aynı dosyaya aynı anda yazan iki ajan birbirinin işini bozar. Sonradan bir merge geç kalır.
- **Tek ekran.** Her biri ne yaptığını yazan otuz ajan gürültüdür. Gereken, yalnız sizin verebileceğiniz birkaç karar; gerisi siz isteyene dek katlanmış durmalı.
- **Tek fatura.** Her satıcı farklı sayar (dolar, token, abonelik penceresi). Takımın tek bütçesi var ve bütçe bitince herkesi durdurması gerekir.

## imece nedir

imece, Rust ile yazılmış bir terminal programıdır: satıcıların kendi komut satırı ajanlarını alt süreç olarak sürer ve söylediklerini satıcıdan bağımsız tek bir olay akışına çevirir. Ekran hiçbir zaman bir tel biçimi görmez. Bunun üstüne, bir takımın ihtiyaç duyduğu, tek bir ajanda olmayan şeyleri ekler:

- **Bir lider** (`lead`): checkout'unuzu okur, sizinle konuşur, bir isteği kartlara böler ve `hire` aracıyla işçi tutar; en çok `--agents` kadar (varsayılan 4). Takım boyunu siz hiç seçmezsiniz.
- **İşçiler** (`claude-1`, `codex-2`, `gemini-3`, …): her biri imece'nin durum dizini altında, HEAD'inizde ayrılmış (detached) kendi git worktree'sinde çalışır. Checkout'unuza hiçbir ajan dokunmaz. Bir işçi kendi worktree'sinde **yardımcılar** (`claude-1.1`) tutabilir; en çok `--helpers` kadar (varsayılan 2).
- **Bir bus**: ajanlar arasında tipli mesajlar (`assign`, `ask`, `answer`, `inform`, `handoff`, `done`, `blocked`, `progress`, `notice`, `cancel`); her mesaj numaralı, böylece bir yanıt hangisini yanıtladığını söyler. Ve boştaki bir işçinin alabileceği kartların durduğu bir **pano**.
- **Değişiklikler (`Δ`)**: bir işçinin `done` demesiyle imece onun worktree'sini tek yama olarak okur ve SENİ BEKLEYEN'e koyar. Diff'i okursunuz; checkout'unuza uygularsınız (stage'e girer, commit'i siz atarsınız), bir nedenle reddedersiniz, satır satır not gönderirsiniz ya da yalnız seçtiğiniz hunk'ları uygularsınız.
- **Onaylar (`⚑`) ve sorular (`?`)**: bir satıcı oturumunun bir komutu çalıştırmadan önce soracağı şey size gelir; bir ajan yalnız sizin karar verebileceğiniz bir şeyi size sorabilir.
- **Tek kese**: `--budget` (ilk seferde 50 dolar) bütün takımın tek bütçesidir, çalıştırmalar arasında saklanır; bitince kimse bir tur daha almaz. `:budget USD` ajanlar çalışırken değiştirir. Yanında başlık, her satıcı planının dolmaya en yakın kullanım penceresini gösterir.
- **Kalıcılık**: projeler, ajanlar, kartlar, günlük, posta ve pano tek bir SQLite dosyasında durur. Sonraki `imece` aynı ekranı, her ajan uykuda, siz birine yazana kadar hiçbir maliyet olmadan gösterir.
- **Çok proje**: `:open YOL` başka bir checkout'u kendi lideriyle ayrı bir proje olarak açar; salon ajanları projeye göre gruplar.

## imece ne değildir

- **Bir sandbox değildir.** imece her satıcının kendi kısıtlamalarını açar (Claude Code'un `--restricted` kipi, Codex'in sandbox'ı, Gemini'nin policy dosyası) ve bir oturumun istenenle çalıştığını, kapalı düşerek (fail closed) denetler: başka araçlar bildiren bir oturum durur. Ama kabuğu olan bir ajan, hesabınızın yapabildiğini yapabilir. Bunun ötesindeki yalıtım bir konteynerin işidir. Bkz. [güvenlik](concepts/security.md).
- **Kapı ne bir model ne de bir araç listesidir.** Son kapı kişidir: bir işçinin değişikliği checkout'unuza yalnız siz uyguladığınızda (`Δ` incelemesi) girer; satıcının soracağı bir komut yalnız siz izin verince çalışır.
- **Bir merge aracı değildir.** Aynı dosyada iki işçi hâlâ bir sorundur; lider, ortak bir dosyayı (changelog, sürüm) aynı anda tek işçiye vererek bundan kaçınır. Gerçek çözüm olan sembol lease'leri henüz yapılmadı. Kod haritası (`3`) onların bir ön izlemesidir.
- **Bitmiş değildir.** Araştırmacılar (güvenilir kaynaklardan soru yanıtlayan, tavanlı bir havuz) tasarlandı ama yapılmadı; demo onları ön izleme olarak gösterir. Proje başına bir renk de henüz yok.

## Döngü

1. Lidere yazarsınız (`/`, sonra isteğiniz, `enter`). Düz metin lidere gider; `@ad metin` tek bir ajana.
2. Lider checkout'u okur (yazamaz), kartlar asar ve işçileri model adıyla değil **katmanla** (`cheap`, `standard`, `strong`) tutar; her satıcı bir katmanı kendi modellerine eşler.
3. Her işçi kendi worktree'sinde, varsayılan `--confine standard` altında satıcısının tam kodlama araç setiyle (okuma, yazma, kabuk, web) çalışır. Satıcısının izin soracağı şey size `⚑` olarak gelir.
4. Bir işçinin `done` demesi SENİ BEKLEYEN'de bir değişiklik `Δ` olur; yanında imece'nin tam o ağaçta gördüğü çalıştırmalar (`✓ bu ağaçta temiz geçti: cargo test` ya da `bu ağaçta çalıştırma yok`).
5. Açarsınız (`enter` ya da `W`), diff'i okursunuz ve yanıt verirsiniz: `a` checkout'unuza stage'e uygular ve worktree'yi işaretler, sonraki `done` yalnız sonrasını gösterir; `r` reddeder ve nedenini yazmanız için yazma satırını açar; `c` ve `n` yerine satırlara not gönderir; `x` kısmi uygulama için hunk seçer. Checkout'unuza uymayan bir değişiklik git'in sözleriyle reddedilir ve hiçbir şey yarım uygulanmaz.
6. Günlük ne olduğunu söyler; terminalin başlığı bekleyenleri sayar (`imece · 2 seni bekliyor`).

## Üç derinlik

Ekranın her parçasının üç derinliği vardır; ânın gerektirdiği kadar derine bakarsınız:

1. **Lonca**: salon; projeler, liderleri, işçiler ve yardımcılar, her biri bir satır, sizi bekleyenler en üstte.
2. **Bir kartın hikâyesi**: bir ajanda `enter` odağını açar; kartının niyeti, kararları, varsayımları, konuşmaları ve kanıtı. `#N` ya da `#kelime` bir kartın kendi ekranını açar.
3. **Ham akış**: bir `enter` daha (ya da `t`) izi gösterir; satıcı oturumunun gerçekten gönderdiği.

`esc` bir derinlik geri çıkar; `^o` vim'in jump list'i gibi olduğunuz yere döner.

## Ekran ilkeleri

1. **Varsayılan sakinlik.** Bir anlamı yoksa hiçbir şey kaymaz; bir değişiklik bir an öne çıkar, sonra sakinleşir.
2. **Liderle konuşursunuz**, otuz ajanla değil.
3. **Her yerde üç derinlik:** lonca, bir kartın hikâyesi (niyet, kararlar, varsayımlar, konuşmalar, kanıt), ham akış.
4. **Bir dikkat bütçesi:** gelen kutusuna yalnız sizi gerektiren kararlar ulaşır; sakin ajanlar tek satıra katlanır.
5. **Geri dönmek ucuzdur:** bir aradan sonra ilk tuş siz yokken olanları gösterir; başka bir pencerede birkaç dakika sonra terminale dönünce, tuşsuz, hemen gösterir.

Her şey tuşlarla, vim tarzı yapılır (`hjkl`, `gg` `G`, `^d` `^u`, `esc` geri, `/` yazmak); fare hiç gerekmez ve hiç yakalanmaz, terminalinizin kendi seçimi çalışmayı sürdürür. `?` her tuşu İngilizce ve Türkçe listeler (`L` değiştirir).

## Durum

imece erken bir sürümdür. Gerçek Claude Code, Codex ve Gemini CLI oturumları çalıştırır; `--demo` ile de satıcısız ve maliyetsiz aynı ekranı gösteren tohumlu bir simülasyon. Kurulum kaynaktandır (bir Rust araç zinciri ve `cargo`); paket henüz yok. Kendini yazmak için kullanılıyor.

Bu sitedeki her davranış, GitHub'daki [`docs/decisions/`](https://github.com/selcukgural/imece/tree/main/docs/decisions) içindeki bir karar kaydından gelir: karar başına bir dosya; nedenleri, sondaların bulduğu olgular ve kalan boşluklar. Bu site *neden* diyorsa neden oradan gelir; bir olgunun hangi sürümde sondalandığını isterseniz kaydı okuyun.

## Nereye gidilir

- [Kurulum](getting-started/install.md): araç zinciri, satıcı CLI'ları ve durumun durduğu yer.
- [İlk çalıştırma](getting-started/first-run.md): `imece`'den ilk uygulanan değişikliğe bir yürüyüş.
- [Demo](getting-started/demo.md): `imece --demo`, bütün ekran iş başında, bedava.
- [Senaryolar](scenarios/index.md): tek düzeltme, karışık satıcılar, notlu inceleme, çok proje, ertesi sabah.
- [Klavye](screen/keyboard.md): her tuş ve her `:` komutu.
- [Videolar](videos.mdx): ekran hareket halinde.
- [imece nasıl çalışır](concepts/how-imece-works.md): parçalar daha ayrıntılı.
