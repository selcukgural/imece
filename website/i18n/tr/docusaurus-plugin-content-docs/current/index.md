---
title: imece nedir?
description: Bir Claude Code ajanları takımı için tek terminal; tek kod tabanı, son kapı sizsiniz.
---

# imece nedir?

imece bir Claude Code ajanları takımını tek kod tabanında, tek ekranda çalıştırır. Bir lider ajan isteğinizi alır, iş gerektirdikçe işçi tutar ve bir işçinin yaptığı her değişiklik, checkout'unuza dokunmadan önce sizi bekler. imece ile, tek bir kodlama ajanının yetmediği, birkaçının aynı depoda birbirinin ve sizin ayağınıza basmadan çalışması gerektiği anda karşılaşırsınız.

## Sorun

Tek terminalde tek kodlama ajanı basittir. Birkaçı değildir:

- **Çok ajan.** Bir Claude Code oturumu, kendi konuşması ve neler olduğuna dair kendi bilgisiyle tek bir penceredir. Birkaçını çalıştırmak birkaç pencere demek ve aralarında hiçbir şey yok.
- **Tek kod tabanı.** Aynı dosyaya aynı anda yazan iki ajan birbirinin işini bozar. Sonradan bir merge geç kalır.
- **Tek ekran.** Her biri ne yaptığını yazan otuz ajan gürültüdür. Gereken, yalnız sizin verebileceğiniz birkaç karar; gerisi siz isteyene dek katlanmış durmalı.
- **Tek fatura.** Birkaç oturum tek hesaptan harcar. Takımın tek bütçesi var ve bütçe bitince herkesi durdurması gerekir.

## imece nedir

imece, Rust ile yazılmış bir terminal programıdır: Claude Code oturumlarını alt süreç olarak sürer ve söylediklerini tek bir olay akışına çevirir. Ekran hiçbir zaman bir tel biçimi görmez. Bunun üstüne, bir takımın ihtiyaç duyduğu, tek bir ajanda olmayan şeyleri ekler:

- **Bir lider** (`lead`): checkout'unuzu okur, sizinle konuşur, bir isteği kartlara böler ve `hire` aracıyla işçi tutar; en çok `--agents` kadar (varsayılan 4). Takım boyunu siz hiç seçmezsiniz.
- **İşçiler** (`claude-1`, `claude-2`, `claude-3`, …): her biri imece'nin durum dizini altında, HEAD'inizde ayrılmış (detached) kendi git worktree'sinde çalışır. Checkout'unuza hiçbir ajan dokunmaz. Bir işçi kendi worktree'sinde **yardımcılar** (`claude-1.1`) tutabilir; en çok `--helpers` kadar (varsayılan 2).
- **Bir bus**: ajanlar arasında tipli mesajlar (`assign`, `ask`, `answer`, `inform`, `handoff`, `done`, `blocked`, `progress`, `notice`, `cancel`); her mesaj numaralı, böylece bir yanıt hangisini yanıtladığını söyler. Ve boştaki bir işçinin alabileceği kartların durduğu bir **pano**.
- **Değişiklikler (`Δ`)**: bir işçinin `done` demesiyle imece onun worktree'sini tek yama olarak okur ve KARAR BEKLEYEN'e koyar; başında, onaya hazır olup olmadığını söyleyen bir hüküm satırı durur. Diff'i okursunuz; checkout'unuza uygularsınız (stage'e girer, commit'i siz atarsınız), bir nedenle reddedersiniz, satır satır not gönderirsiniz ya da yalnız seçtiğiniz hunk'ları uygularsınız. [Lonca](concepts/projects.md#loncada)'daki bir projede değişiklik işçinin önerisidir ve evetiniz onu onaylar.
- **Onaylar (`⚑`) ve sorular (`?`)**: bir oturumun bir komutu çalıştırmadan önce soracağı şey size gelir; bir ajan yalnız sizin karar verebileceğiniz bir şeyi size sorabilir.
- **Tek kese**: `--budget` (ilk seferde 50 dolar) bütün takımın tek bütçesidir, çalıştırmalar arasında saklanır; bitince kimse bir tur daha almaz. `:budget USD` ajanlar çalışırken değiştirir. Başlık onu gösterir; planınızın kullanım penceresini de, o pencere %90 dolunca yanında.
- **Kalıcılık**: projeler, ajanlar, kartlar, günlük, posta ve pano tek bir SQLite dosyasında durur. Sonraki `imece` aynı ekranı, her ajan uykuda, siz birine yazana kadar hiçbir maliyet olmadan gösterir.
- **Çok proje**: `:open YOL` başka bir checkout'u kendi lideriyle ayrı bir proje olarak açar; ana ekran ajanları projeye göre gruplar.

## imece ne değildir

- **Bir sandbox değildir.** imece Claude Code'un kendi kısıtlamalarını açar (`--restricted` kipi) ve bir oturumun istenenle çalıştığını, kapalı düşerek (fail closed) denetler: başka araçlar bildiren bir oturum durur. Ama kabuğu olan bir ajan, hesabınızın yapabildiğini yapabilir. Bunun ötesindeki yalıtım bir konteynerin işidir. Bkz. [güvenlik](concepts/security.md).
- **Kapı ne bir model ne de bir araç listesidir.** Son kapı kişidir: bir işçinin değişikliği checkout'unuza yalnız siz uyguladığınızda (`Δ` incelemesi) girer; Claude Code'un soracağı bir komut yalnız siz izin verince çalışır.
- **Bir merge aracı değildir.** Düz bir git checkout'unda aynı dosyada iki işçi hâlâ bir sorundur; lider, ortak bir dosyayı (changelog, sürüm) aynı anda tek işçiye vererek bundan kaçınır. [Lonca](concepts/projects.md#loncada)'daki bir projede daha fazlası var: işçiler düzenlediklerini lease'ler, birleştirmeyi Lonca'nın merge kuyruğu yapar ve kod haritası (`:code`) kimin neyi tuttuğunu gösterir.
- **Bitmiş değildir.** Araştırmacılar (güvenilir kaynaklardan soru yanıtlayan, tavanlı bir havuz) tasarlandı ama yapılmadı; demo onları ön izleme olarak gösterir. Proje başına bir renk de henüz yok.

## Döngü

1. Lidere yazarsınız (`/`, sonra isteğiniz, `enter`). Düz metin lidere gider; `@ad metin` tek bir ajana.
2. Lider checkout'u okur (yazamaz), kartlar asar ve işçileri model adıyla değil **katmanla** (`cheap`, `standard`, `strong`) tutar.
3. Her işçi kendi worktree'sinde, varsayılan `--confine standard` altında Claude Code'un tam kodlama araç setiyle (okuma, yazma, kabuk, web) çalışır. Claude Code'un izin soracağı şey size `⚑` olarak gelir.
4. Bir işçinin `done` demesi KARAR BEKLEYEN'de bir değişiklik `Δ` olur. Hüküm satırı onu neyin kanıtladığını ve nereye uzandığını söyler: `✓ onaya hazır · ✓ cargo test · 2 dosya, ikisi de #1 içinde`, ya da `! önce bak` ve nedeni (başarısız bir çalıştırma, hiçbir şey koşmamış, kartının dışında bir dosya, başka bir değişiklikle çakışma).
5. Açarsınız (`enter` ya da `W`), diff'i okursunuz ve yanıt verirsiniz: `a` checkout'unuza stage'e uygular ve worktree'yi işaretler, sonraki `done` yalnız sonrasını gösterir; `r` reddeder ve nedenini yazmanız için yazma satırını açar; `c` ve `n` yerine satırlara not gönderir; `x` kısmi uygulama için hunk seçer. Checkout'unuza uymayan bir değişiklik git'in sözleriyle reddedilir ve hiçbir şey yarım uygulanmaz.
6. Günlük ne olduğunu söyler; terminalin başlığı bekleyenleri sayar (`! imece · 2 karar bekliyor`).

## İki ekran

imece'nin iki ekranı, bir de üstlerinde açtığınız pencereler vardır:

1. **Ana ekran**, imece'nin açıldığı yer: tek liste. Üstte KARAR BEKLEYEN: sizi bekleyenler, karar vermenin maliyetine göre gruplu (önce tek tuşun karara bağladığı izinler, sonra sorular, sonra bir diff okumak isteyen değişiklikler). Altında ÇALIŞAN: kim çalışıyor, kim takıldı, kim boşta, proje proje. Her satır bir ajanın adıyla değil kartıyla başlar: `#12 Refunds`. Eldeki satır onu karara bağlayan tuşlarla biter (`a evet  r hayır`) ve bir karar sizi sonraki satıra götürür.
2. **Tek bir iş**: bir satırda `enter` onu açar; bir baş satırı (kart, proje, ajan, süre, maliyet) ve üç sekme: `diff · konuşma · log`; `tab` geçer. Bir değişiklik diff'iyle, en üstte hüküm satırıyla açılır; bir soru sorusunun kendisiyle ve rakam olarak yanıtlarıyla; bir ajan log'uyla: kartının hikâyesi, yapılacaklar listesi ve oturumunun ham akışı.
3. **Pencereler**, ikisinin de üstünde: pano (`:board`), kod haritası (`:code`), zaman çizelgesi (`:timeline`), günlük (`J`), harcamalar (`:ledger`), süreçler (`:ps`) ve Lonca (`:lonca`). `esc` kapatır.

`esc` geri çıkar; `^o` vim'in jump list'i gibi olduğunuz yere döner. Alttaki durum satırı nerede olduğunuzu ve eldeki şey için en çok üç tuşu söyler; `?` bulunduğunuz ekranın tuşlarını, bir `?` daha bütün tuşları listeler.

## Ekran ilkeleri

1. **Varsayılan sakinlik.** Bir anlamı yoksa hiçbir şey kaymaz; bir değişiklik bir an öne çıkar, sonra sakinleşir.
2. **Liderle konuşursunuz**, otuz ajanla değil.
3. **Satır iştir:** her liste bir kartın numarası ve başlığıyla başlar; ajan, ayrıntının içindeki bir bilgidir.
4. **Bir dikkat bütçesi:** KARAR BEKLEYEN'e yalnız sizi gerektiren kararlar, karar vermenin maliyeti sırasıyla ulaşır; başlıkta dört şey vardır: projeler, karar bekleyenler (`! 2 karar bekliyor`), kese, saat.
5. **Geri dönmek ucuzdur:** bir aradan sonra ilk tuş siz yokken olanları gösterir; başka bir pencerede birkaç dakika sonra terminale dönünce, tuşsuz, hemen gösterir.

Her şey tuşlarla, vim tarzı yapılır (`hjkl`, `gg` `G`, `^d` `^u`, `esc` geri, `/` yazmak); fare hiç gerekmez ve hiç yakalanmaz, terminalinizin kendi seçimi çalışmayı sürdürür. `?` tuşları İngilizce ve Türkçe listeler (`L` değiştirir).

## Durum

imece erken bir sürümdür. Gerçek Claude Code oturumları çalıştırır; `--demo` ile de oturumsuz ve maliyetsiz aynı ekranı gösteren tohumlu bir simülasyon. macOS ve Linux'ta tek satırla kurulur (`curl -fsSL https://selcukgural.github.io/imece/install.sh | sh`), ya da `cargo` ile kaynaktan. Kendini yazmak için kullanılıyor.

Bu sitedeki her davranış, GitHub'daki [`docs/decisions/`](https://github.com/selcukgural/imece/tree/main/docs/decisions) içindeki bir karar kaydından gelir: karar başına bir dosya; nedenleri, sondaların bulduğu olgular ve kalan boşluklar. Bu site *neden* diyorsa neden oradan gelir; bir olgunun hangi sürümde sondalandığını isterseniz kaydı okuyun.

## Nereye gidilir

- [Kurulum](getting-started/install.md): araç zinciri, Claude Code ve durumun durduğu yer.
- [İlk çalıştırma](getting-started/first-run.md): `imece`'den ilk uygulanan değişikliğe bir yürüyüş.
- [Demo](getting-started/demo.md): `imece --demo`, bütün ekran iş başında, bedava.
- [Kopya kâğıdı](cheat-sheet.md): ilk haftanın tuşları ve komutları, yazdırılabilir tek sayfada.
- [Senaryolar](scenarios/index.md): tek düzeltme, notlu inceleme, çok proje, ertesi sabah.
- [Klavye](screen/keyboard.md): her tuş ve her `:` komutu.
- [Videolar](videos.mdx): ekran hareket halinde.
- [imece nasıl çalışır](concepts/how-imece-works.md): parçalar daha ayrıntılı.
