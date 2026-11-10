---
title: Değişiklikler (Δ kapısı)
description: Bir işçinin işi checkout'unuza nasıl ulaşır - done'ı KARAR BEKLEYEN'de bir değişiklik olur; uygularsınız, reddedersiniz, not yazarsınız ya da bir kısmını uygularsınız; worktree bu sırada ne yapar.
---

# Değişiklikler (Δ kapısı)

Bir değişiklik (`Δ`), bir işçi `done` dediğinde worktree'sinin tuttuğu şeydir. KARAR BEKLEYEN'de sizi bekler ve siz uygulamadan hiçbir parçası checkout'unuza ulaşmaz. Bu, ajanlarla çalışma kopyanız arasındaki kapıdır; açan sizsiniz.

## `done`'dan Δ'ya

1. Bir işçi (yalnız İşçi rolü; bir yardımcının `done`'ı işçisinin işidir, lider hiçbir şeyi değiştirmez) `done` gönderir.
2. imece'nin git iş parçacığında worktree özel bir index dosyası üzerinden okunur: `read-tree HEAD`, `add -A`, `write-tree`, HEAD'ine karşı `diff --cached --binary`. İzlenmeyen dosyalar sayılır; ajanın kendi index'ine dokunulmaz. 4 MiB'ı aşan bir yama reddedilir ve işçiye söylenir.
3. Sonuç KARAR BEKLEYEN'de bir `Δ` öğesidir, ayrıntısı `done`'ın ilk satırı; günlük `claude-1 bitirdi: değişiklik hazır · 3 dosya +34 −8` der. Aynı işçiden daha yeni bir `done` eskisinin yerine geçer.
4. İşçinin `send` yanıtı okumayı bekler ve değişikliği gördüğünüzü söyler.

Ana ekranda bir değişiklik **değişiklikler** altında, kartıyla başlayan bir satırdır ve hükmünü başı olmadan söyler: onu neyin kanıtladığını, kartının sahip olduğu yollara göre dosyalarını ve bekleyen hangi başka değişiklikle çakıştığını. Onaya hazır olanlar önce gelir, grubun satırı onları sayar:

```text
 değişiklikler                                                                                        2 onaya hazır
 ! #1   Kargo ücret hesabı        ✓ dotnet build · ✓ dotnet test · 3 dosya +6 −3, hepsi #1 içinde ·
                                  başka değişiklikle çakışmıyor
 ! #3   Fiyat listesi önbelleği   ✓ dotnet build · ✓ dotnet test · 2 dosya +4 −2, ikisi de #3 içinde ·
                                  başka değişiklikle çakışmıyor
 ! #2   Ürün aramasında sayfala…  ✓ dotnet build · ✓ dotnet test · 2 dosya +6 −3 ·
                                  1 kartın dışında (#2): src/Orders/OrderService.cs · başka değişiklikle çakışmıyor
```

Eldeki satır `a onayla  r geri gönder` ile biter; `enter` diff'i açar.

### Satırdaki kanıt

İşçinin ya da bir yardımcının imece'nin `run` aracıyla yaptığı bir çalıştırma, üzerinde koştuğu ağaca bağlanır (worktree öncesinde ve sonrasında okunur; ağacı değiştiren bir çalıştırma hiçbirine sayılmaz). Bir `done` okununca, tam o ağaçtaki çalıştırmalar onunla gelir: hüküm satırı, ana ekranda değişikliğin satırında ve diff'inin üstünde, her birini nasıl bittiğiyle adlandırır (`✓ cargo test`, `✗ cargo test: exit 101`) ya da `hiçbir şey koşmadı` der; o zaman değişiklik önce bakılacak bir değişikliktir. Gösterilir, asla kapı değildir: çalıştırması olmayan bir değişiklik yine uygulanabilir; karar sizin. Claude Code'un kendi kabuğu sayılmaz; imece onun hangi ağaçta koştuğunu görmez.

### Deponun kontrolü {#deponun-kontrolü}

Checkout'unuzda `.imece/check` yoluna bir betik koyun ([worktree kurulum betiği](projects.md#worktree-kurulum-betiği) gibi onaylanan bir kabuk betiği); imece onu, oradan her değişiklik okunduğunda işçinin worktree'sinde çalıştırır, **tam baytlarına evet dediğinizde**: okuduğunuz baytların özel bir kopyası üzerinde `/bin/sh`, temizlenmiş ortam ve `IMECE_REPO` ile `IMECE_WORKTREE`, kendi süreç grubunda, `run` aracının süresince (`--run-timeout`, varsayılan on dakika). Vardığı sonuç değişikliğin üstüne gider: hüküm satırında, ana ekranda ve diff'in üstünde, önce `✓ kontrol` ya da `✗ kontrol · exit 1, 38.7 s`, diff'te altında bir başarısızlığın çıktısının son satırları, ve bir günlük satırı (`claude-1: kontrol değişikliğinde başarısız, exit 1, 38.7 s · Tests: 1 failed, 211 passed`). Bir nottur, asla kapı değil: `a` kırmızı bir değişikliği de her değişiklik gibi uygular; karar sizin.

- **Evetiniz:** evet demediğiniz bir kontrolü bulan ilk değişiklik KARAR BEKLEYEN'e bir soru koyar (`▶ claude-1: deponun kontrolü .imece/check değişikliklerinde çalışsın mı? enter gösterir`); değişiklik bu arada kontrolsüz bekler ve yamasının üstünde bunu söyler. `enter` baytları gösterir, `a` evet der (bekleyen her değişiklikte hemen, sonra okunan her birinde çalışır), `r` bu çalıştırma için hayır. Evet, baytların SHA-256'sına, dosyanın gerçek yoluna ve `/bin/sh`'ye bağlıdır, imece'nin deposunda saklanır; değişen bir bayt yeniden sorar. Sorular sonraki başlangıçta kapanır; sonraki değişiklik yeniden sorar.
- **Ağaç başına bir kez:** aynı ağaçla yeniden okunan bir değişiklik (geri gönderilip olduğu gibi yeniden gönderilen) eski sonucunu alır, çalıştırma olmaz; değişen ağaç yeniden çalıştırır. imece kapanırken çalışan ya da sorulan bir kontrol sonraki başlangıçta yeniden çalışır ya da sorulur.
- **Yapmaması gereken:** izlenen dosyaları değiştirmek. Değişiklik kontrol çalışmadan önce okunur; kontrolün yazdığı ona dahil değildir ve işçinin sonraki `done`'unda ortaya çıkar; git'in yok saydığı yollardaki derleme çıktısı sorun değil.
- **İşçiye söylenmez.** Sonuç sizindir: `n` notlarınızı geri gönderir; başarısız kontrolün son satırları ilk not olur. Kırmızı bir kontrolün değişikliği bir tur alıkoyup önce işçiye söylemesi gerekir mi, sonraki bir kaydın sorusu.
- Depoda kontrol yoksa hiçbir şey çalışmaz ve görünmez; okunamayan (düz dosya olmayan, 64 KiB'den büyük) biri değişikliğin üstünde ve günlükte bunu söyler.

### Maliyeti {#maliyeti}

Her değişiklik işçinin ona ne harcadığını söyler; böylece liderin işe alımını (bir yeniden adlandırmaya güçlü katmandan işçi) ya da tek satırlık bir düzeltmeye 4 $ yakan bir işçiyi değerlendirebilirsiniz: diff'in üstünde ve ana ekranda değişikliğin ön izlemesinde `maliyet · $0.84 · 12 dk`, günlükte (`claude-1 değişikliği $0.84 · 12 dk tuttu`) ve dolayısıyla kartın ekranında ve `:why`'da. Rakam işçinin, yardımcıları dahil, son değişikliğinden sonraki ilk isteminden bu `done`'a harcadığı ve çalıştığı süredir (değişikliğin sizi beklediği süre değil); `done` tur ortasında gelir, bu yüzden rakam bir an sonra, turun maliyeti bilinince görünür. Geri gönderilip (`r`, `n`, `b`) yeniden gönderilen bir değişiklik toplanır: `$1.20 · 25 dk · 3 tur`; eski bir değişikliğin yerine geçen daha yeni bir `done` onun maliyetini de taşır, kısmen uyguladığınız bir değişikliğin kalanı da. Bir değişiklik bütünüyle uygulanınca sonraki sıfırdan başlar. Lider bunu işçinin `done`'unda duyar; onu işçinin turu bitince, `[done 12 · $0.84]` başlığıyla alır, böylece katmanlarının neye mal olduğunu öğrenir. Bir solonun her tur sonundaki değişikliği aynısını söyler. Bir planla girişte her rakam turların API fiyatıyla neye geleceğidir ve `≈` ile işaretlidir; kimsenin fiyatlamadığı harcama `+?` okunur ([bir rakam ne der](budget-and-limits.md#bir-rakam-ne-der)); bir karta yapılan her denemenin neye mal olduğu ve bir projenin harcamasının ne kadarının uygulandığı ya da boşa gittiği [`:ledger`](budget-and-limits.md#harcamalar)'dadır.

## Okumak

`enter` diff'in tamamını, değişikliğin ayrıntısının diff sekmesi olarak açar: kartı, projeyi, işçiyi, süreyi ve maliyeti, dosyaları ve satırları, bir de bekleyen değişiklikler arasındaki yerini (`değişiklik 1 / 3`) söyleyen bir baş satırı; `tab` konuşmasına ve log'una geçer. `j k` kaydırır, `^d ^u ^f ^b` sayfalar, `gg G`, `[ ]` dosyadan dosyaya. Her satır çizilmeden önce temizlenir. Yamanın üstünde:

- hüküm satırı: `✓ onaya hazır` ya da `! önce bak`, sonra neyin kanıtladığı (çalıştıysa deponun [kontrolü](#deponun-kontrolü), ağacındaki çalıştırmalar ya da hiçbir şeyin koşmadığı), kartının sahip olduğu yollara göre dosyaları, bekleyen hangi başka değişiklikle çakıştığı, reddedilen bir uygulama, Lonca'nın bütün göstermediği bir yama ve onayından sonra Lonca'nın ne çalıştıracağı ([değişiklik görünümü](../screen/changes.md#yamanın-üstündekiler));
- kontrolün durumu, henüz bitmediyse: çalıştığı ya da onayınızı beklediği;
- dosyalar birden çok türdense türe göre satırlar: `kaynak +20 −1 (2) · testler +18 −0 (1) · üretilmiş +7 −5 (1)`. Türler yalnız yola göredir: lock dosyaları, minified ve üretilmiş kod ile `generated/`, `dist/`, `vendor/` dizinleri üretilmiş; `tests/`, `test/`, `__tests__/`, `spec/`, `test_*` ve alışılmış test sonekleri test; kalanı kaynak. Üretilmiş, testi yener;
- iki ya da daha çok dosyalı bir değişiklik için her birinin `+N −M`'si (ya da `ikili`) ile dosya listesi, en çok altı, kalanı sayılır.

`w` uzun satırları altlarına sarar (`w` yine kenarda keser). En üst satır eldeki satırdır: `c`, `>` ve `x` ona etki eder.

`h` (ya da `←`) eldeki satırın dosyasını `diff --git` başlığına katlar; başlık kaç satır gizlediğini söyler (`▸ 42 satır`). `l` (ya da `→`) açar, `za` çevirir, `zM` (`:fold`) her dosyayı katlar, `zR` (`:unfold`) hepsini açar. `j` `k`, sayfalar ve `G` gösterilen satırlar üzerinde gider, yani katlı bir dosya üstünden geçilecek tek satırdır; `[` `]` yine başlıktan başlığa gider. Gizli bir satırın dosyasını katlamak eldeki satırı başlığına koyar. `v` önce her dosyayı açar, çünkü satırlar bütün yamadan seçilir. Katlar o yama için geçerlidir: işçiden gelen yeni bir yama açık başlar.

## İkinci bir okuma {#ikinci-bir-okuma}

`:review` eldeki değişikliği taze bir Claude Code oturumuna okutur ve bulduğunu söyletir; `:review on` gelen her değişiklik için ister (varsayılan kapalı, hatırlanır). Okuyan işçinin oturumundan hiçbir şey bilmez; işçinin ne yaptığını söylediğini, dosya listesini ve yamayı (en çok 64 KiB) alır, aracı yoktur. Yanıtı dosya listesinin üstünde durur:

```
✎ inceleme · sonnet · $0.04
1. src/parse.rs:41: boş girdi Ok(()) döndürüyor, bunun testi yok.
2. Başlık lexer'ın da düzeldiğini söylüyor; yama ona dokunmuyor.
```

altı satırı, fazlası varsa `… +N satır · R hepsini gösterir`; `R` tamamını gösterir. Gelene kadar bir satır okunduğunu söyler. Bir öneriden fazlası değildir: hiçbir değişiklik onu beklemez, `a` onunla da onsuz da uygular ve işçiye söylenmez (`>` ile alıntılayın ya da satırlarını `c` ile not edin). Bir inceleme bütçeden en çok 0,10 $ tutar; günde 20 inceleme ve 2,00 $; incelemesi olan bir değişiklik için iki kez ödenmez. İnceleme günlükte saklanır ve yeniden başlatmadan sonra değişikliğiyle geri gelir. Bkz. [`:review`](../reference/commands.md#review-onoff) ve [bütçe ve sınırlar](budget-and-limits.md).

## Uygulamak

`a` uygulamayı git iş parçacığında başlatır; anlık ileti sürdüğünü, günlük nasıl gittiğini söyler.

- Checkout'unuzda `git apply --index`, atomik: değişiklik **stage'e** iner, bakıp commit etmeniz için. Önce `update-index --refresh` çalışır; dokunulmuş ama değişmemiş bir dosya uyumsuzluk sayılmaz.
- **Temizse üç yollu:** düz uygulama sizin stage'lenmemiş düzenlemelerinizden başka bir nedenle başarısız olunca yama, index'inizin bir kopyasına (`<git dir>/imece-apply-index`) üç yollu birleştirilir; bu hiçbir dosyaya dokunmaz. Yalnız o temizse gerçek `git apply --index --3way` çalışır. Bir çakışma `conflicts with the checkout in a, b` diye reddedilir; aynı dosyadaki stage'lenmemiş düzenlemeniz git'in kendi sözleriyle reddedilir (işinizin üzerinden birleştirme geçmez). Hiçbir şey yarım uygulanmaz.
- **İşaret:** uygulamadan sonra worktree'nin incelenen ağacı orada yazar `imece <imece@localhost>` olarak commit edilir ve worktree ona sıfırlanır (dosyalara dokunulmaz, index = HEAD). İşçinin sonraki `done`'ı yalnız sonrasında geleni gösterir ve kendi commit'lerinin hepsi imece'nin olan bir worktree HEAD'inizi izlemeyi sürdürür.
- Günlük: `claude-1 değişikliğini (3 dosya) checkout'a uyguladın, stage'de; çalışma ağacı işareti 4f3a2c1`. İşçi sonraki isteminde imece'den bir not alır ("From imece, not from the person or a teammate").

`a`, o değişiklik uygulanırken ya da işçinin daha yeni bir `done`'ı hâlâ okunurken reddedilir; `r`, uygulanırken reddedilir.

## Bir çakışmayı geri göndermek

Git bir değişikliği reddettiğinde imece hangi dosya ve parçaların uymadığını da bulur (index'inizin bir kopyasında, uygulamadaki düz ve üç yollu denemelerin aynısıyla). Değişiklik görünümünde `b` bunları işçiye tek mesajda gönderir: HEAD'in kısa kimliği, her biri için git'in sözleri ve ne yapacağı: worktree'sini değişikliği üstünde, commit'lenmemiş olarak o commit'e getirmek, çözmek, yeniden `done` göndermek. Değişiklik bir ret gibi geri döner: öge kapanır, worktree onu tutar, sonraki `done` yeni bir Δ olur. imece hiçbir şeyi çözmez. Kendi stage'lenmemiş düzenlemeleriniz hiçbir parçayla çakışmaz, bu yüzden onlar için hiçbir şey geri gitmez; işçi gitmiş ya da durmuşsa veya bütçe bitmişse Δ olduğu gibi kalır. Bkz. [değişiklik görünümü](../screen/changes.md).

## Reddetmek

`r` öğeyi kapatır; değişiklik worktree'de kalır ve yazma satırı nedenini söylemeniz için `@claude-1 ` ile açılır. İşçi sonraki isteminde değişikliğin uygulanmadığını öğrenir.

## Notlar

Uygula ve reddetin yanındaki üçüncü yanıt:

- Bir parçanın satırında (bağlam, eklenen ya da silinen; başlık ya da `@@` satırı değil) `c`, komut satırını satırın yeriyle etiketlenmiş açar: `src/p.rs:12 için not ▸`. `enter` tutar, `esc` bırakır, `c` yine yeniden yazar, boş bir not kaldırır. Not, satırının altında `✎` ile gösterilir.
- Yer, dosyanın yeni adı ve satırın oradaki numarasıdır; silinen bir satır için eski dosyadaki numarası eksi işaretiyle: `src/p.rs:−7`.
- `n` değişikliğin bütün notlarını işçisine tek mesajda gönderir, satır sırasıyla, her biri `dosya:satır`ının ve satırın kendisinin altında bir `diff` çitinde; bir satır da değişikliğin worktree'sinde kaldığını, uygulanmadığını, dediğiniz gibi değiştirip yeniden `done` göndermesini söyler. Değişiklik ret gibi geri döner: öğe kapanır, günlük reddedildi der, işçinin sonraki `done`'ı yeni bir Δ olur.

Notlar ekranın belleğinde, yamanın hash'ine karşı yaşar: daha yeni bir Δ notsuz başlar ve notlar çalıştırmalar arasında saklanmaz. İşlenmiş bir örnek [notlarla inceleme](../scenarios/review-with-notes.md) sayfasında.

## Bir parçayı alıntılamak

Bir değişiklikte `>`, işçisine bir mesaj başlatır: imleç ardında `@claude-1 `, altında en üst satırın içinde bulunduğu parça, dosyasının `---`/`+++` satırlarıyla bir `diff` çitinde: en çok 40 satır, kalanı `… +N` olarak sayılır. `enter`'dan önce hepsini düzenleyebilirsiniz; işçiye düz bir mesajdır.

## Değişikliğin bir kısmını uygulamak

- `x` en üstteki parçayı seçer (bir dosya başlığındayken sonrakini); `@@` satırı `✓` taşır; `x` yine bırakır. Anlık ileti sayar: "1 / 3 parça seçili; a yalnız onları uygular, kalanı bekler".
- Parçalar seçiliyken `a` yalnız onları uygular: seçili parçalar git iş parçacığında kendi başına bir değişiklik olur (hiç parçası olmayan bir dosya dışarıda kalır, ikili dosya da), imece'nin kendi index'i üzerinden okunur, bütün bir değişiklik gibi uygulanır (temizse üç yollu) ve worktree yalnız o kısımla işaretlenir.
- Kalan, worktree'nin yeni HEAD'ine karşı tuttuğudur; hemen yeniden okunur ve KARAR BEKLEYEN'de aynı başlıkla yine bir değişiklik olarak bekler. İşçi hangi kısmın uygulandığını ve kalanın beklediğini öğrenir.

Seçim yamanın hash'ine karşı yalnız bu çalıştırma için tutulur. Bir parçanın içinden tek satırlar seçilemez.

## Bir değişikliğin ardındaki kart

Kart değişikliğini izler ([kartın denemeleri](messages-and-board.md#kartın-denemeleri)). Değişiklik sizi beklerken kart panoda (`:board`) İNCELENECEK'te durur; geri gönderilince bir tur daha ÜZERİNDE'dir; bütünüyle uygulanınca BİTTİ. Lonca'da evetinizden sonra merge kuyruğunda DOĞRULANIYOR'dur, gövdeye girince BİTTİ. Ondan başka gördükleriniz:

- **Bir deneme.** Yeniden alınan bir kart bir deneme daha demektir: panoda ve ekranında ` · 2. deneme`, `:ledger`'daki satırında `deneme 2`.
- **Başarısız bir kart ya da geç bir `done`** değişiklik göstermez: işçi kartı yapamadığını söylemiştir (panoda `✕`), ya da bu arada kartı başka bir denemeye geçmiş ya da geri çekilmiştir. Hangisi olduğunu günlük söyler.
- **Devre kesici.** Üçüncü denemesinde başarısız olan ya da değişikliği Lonca'nın kuyruğundan üçüncü kez geri dönen bir kart, ana ekranda **sorular** altında sizi bekler: `3 denemede başarısız: bir deneme daha, ya da kart başarısız sayılır`, `değişikliği 3 kez geri döndü: bir tur daha, ya da kart başarısız sayılır`. `a` bir tane daha verir, `r` kartı kesin başarısız sayar. Sizin geri göndermeniz (bir ret, notlar, bir çakışma) her zaman geçer: zaten karar veriyorsunuz.
- **Geri çekilmiş.** Liderin geri çektiği bir kart bitmiştir (panoda `–`); onun için gelen bir `done` değişiklik göstermez.

## Bu sırada worktree

- Bir işçinin kartının ekranındaki satırı (`#N`, ayrıntısının log sekmesi) worktree'sinin nerede durduğunu söyler: `⎇ fix-parser ↑2 ↓1 +12 −3`. `⎇ ad` ajanın açtığı bir daldır (detached iken yok), `↑N` HEAD'inizde olmayan kendi commit'leri (imece'nin işaretleri sayılmaz), `↓N` onda olmayan HEAD commit'leriniz, `+N −M` sonraki `done`'ının size vereceği satırlar, izlenmeyen dosyalar dahil. Her parça yalnız söyleyecek bir şey varken gösterilir; oturum başlarken, bir tur bitince, bir uygulamadan ve bir taşımadan sonra ve tur ortasında her 15 saniyede okunur. Çalıştırmalar arasında saklanmaz.
- **HEAD'i izlemek:** kendi commit'lerinin hepsi imece'nin olan temiz bir worktree, turlar arasındaki bir işçinin her isteminden önce HEAD'inize taşınır (yardımcılar da), günlüğe `çalışma ağacı … yerine HEAD … oldu` yazılır. Commit edilmemiş değişiklikleri ya da kendi commit'leri olan yerinde kalır ve günlük söyler. Böylece uyguladığınızı commit edince işçiler yeni HEAD'inizden devam eder. Kendi başlangıç noktasından işe alınan bir işçi (`hire`'ın `from`'u: bir dal, bir etiket, bir commit, bir pull request için `#N`) izlemez: worktree'si başladığı yerde kalır, değişikliği o noktaya göredir ve uygulaması aynı üç yollu uygulamadır ([takım](team.md)).
- Elle silinmiş ama git'in hâlâ listelediği bir worktree, kaybolanla ve `git worktree prune` ile reddedilir; ajan o nedenle durur.

## Commit etmek

imece checkout'unuzda hiçbir şeyi commit etmez. `:commit`, stage'dekiler için Claude'un ucuz kademesinden bir mesaj ister ve komut satırına `!git commit -m '…'` koyar; kendi kabuğunuzda düzenleyip çalıştırırsınız. Kendi tavanları vardır ([bütçe](../concepts/budget-and-limits.md)) ve [kendi kabuğunuz ve commit](../scenarios/own-shell-and-commit.md) sayfasında adım adım anlatılır.

## Lonca'da {#loncada}

Bir [Lonca workspace](projects.md#loncada)'indeki işçinin okunacak worktree'si yoktur. Değişikliği daemon'ın tuttuğu bir öneridir; hiçbir önerinin taşımadığı değişikliklerle gelen bir `done` `propose first` ile reddedilir, kartını da kapatmaz. Δ'sı o öneridir: imece birkaç saniyede bir bakar ve işçinin turu bitince en yeni önerisi, depodan okunmuş olarak sizi bekler. Bir workspace elindekinin tümünü önerir, bu yüzden en yenisi eskilerin yaptığını da kapsar; workspace yeniden önerince Lonca bekleyeni geri çeker: günlük `claude-1: yeniden önerdi; gördüğün önerinin yerini yenisi alıyor` der, işçinin turu bitince Δ yenisi olur.

`a` onaylar, `r` geri gönderir (önce nedenini sorar); ikisini de `lonca` programı imzalar, bu yüzden Secure Enclave anahtarı parmağınızı ister. Lonca'ya ulaşmayan bir karar günlükte söylenir ve Δ beklemeyi sürdürür. Lonca gerekçesiz ret almaz, bu yüzden `r` önce satırı açar ve gönderdiğiniz hem gerekçe hem de işçiye mesajınızdır; `esc` ile satırdan çıkmak hiçbir şeyi reddetmez. İmleç işçinin adından sonra durur ve adı silseniz de satır gerekçedir; başkasına giden bir satır, bir `:` komutu ya da bir `!komut` hiçbir şeyi reddetmez. Öneri bütün olarak onaylanır: `x` reddedilir ve çalışma kopyanıza hiçbir şey inmez, onu [`:sync`](../reference/commands.md#sync) trunk'a getirir. Önerinin başlığı işçisinin kartıdır; işçinin onu ne diye önerdiği (niyeti, trunk'ın commit mesajı olur) kararın altında değişikliğin metnidir. İşçilere bu niyeti tek satır, bir commit başlığı olarak, kart numarası koymadan yazmaları söylenir.

Lonca'nın sonra yaptığı, kim karar vermiş olursa olsun söylenir, kendi kabuğunuzdaki `lonca approve` dahil: `trunk'ta, 174cfc1 olarak`, kuyruğun yapamadığı bir birleştirme ya da nedenleriyle bir ret. Birleştirme kuyruğunun denetimleri de günlüğe girer: `claude-1: Lonca'nın denetimleri · ✓ build exit 0, 12.4 s · ✗ tests exit 101, 31.0 s · son satırı`. Geri gönderilen işçi uyandırılır, eşitleyip yeniden önermesi söylenir ve başarısız denetimin yazdıkları ona iletilir; trunk'a giren işçi bunu sonraki isteminde duyar. Geri gönderilen öneriyi lideri de sonraki isteminde, nedeniyle duyar, uyandırılmaz: işçi düzeltir; turları biten işçi sizi bekler. Politikası değişiklikleri görülmeden alacak bir depo kapatılır: bkz. [projeler](projects.md#loncada). Maliyeti imece'de kalır ve biri inene dek sonraki öneriye taşınır.

### Lonca'da ne denetlenir {#what-is-checked-on-lonca}

`.imece/check` workspace'te çalışmaz. Lonca'daki bir depo ne çalışacağını `lonca.toml`'daki `[verification]` ile söyler, Lonca'nın birleştirme kuyruğu da bunu onayından sonra birleşmiş ağaçta, korumalı alanda çalıştırır; imece hiçbir şey çalıştırmaz. Δ'nın hüküm satırı, Lonca'nın o değişiklik için söylediği gibi, hangisi olduğunu söyler: `evetinden sonra Lonca şunları çalıştırır: build, tests`, `evetinden sonra hiçbir şey çalışmaz` ya da `Lonca evetinden sonra neyin çalışacağını söylemedi` (eski bir Lonca ya da okuyamadığı bir politika: o zaman hiçbir şey bir kontrol sözü vermez). `extended`, başka koda ulaşan bir değişiklik için `on_signature_change`'dir; politikanın adlandırılmış bir kontrolü (`verification.checks`) burada da, neyin çalıştığını söyleyen günlük satırında da adıyla görünür. Ayarlar imzalıdır, bu yüzden değiştirmek senin yapacağın bir `lonca policy set` işidir:

```toml
[verification]
build = "cargo build --offline"
test = "cargo test --offline"
required = true
env_passthrough = ["HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR"]
```

Komutlar `PATH`, geçici bir `HOME` ve `LANG` dışında boş bir ortam görür, bu yüzden ev dizinindeki bir araç zincirinin dizinleri `env_passthrough`'ta olmalıdır. Başarısız bir denetimde değişiklik inmez: Lonca reddeder, onayın günlükte kalır, işçi işi yeniden alır.

Bir kart politikanın adlandırılmış denetimlerinden fazlasını isteyebilir ([kartın istediği denetimler](messages-and-board.md#kartın-istediği-denetimler)): değişikliğinin karar satırı onları ekler (`evetinden sonra Lonca şunları çalıştırır: build, tests, api · kart ayrıca şunları da istiyor: api`) ve evetin onları Lonca'dan ister (`--also`).

## Ayrıca bakınız

- [Değişiklikler ekranı](../screen/changes.md): diff görünümündeki her tuş.
- [Ana ekran](../screen/home.md): `W`, `S`, karar bekleyenlerin sırası.
- [Tek düzeltme](../scenarios/one-fix.md) ve [notlarla inceleme](../scenarios/review-with-notes.md).
- [Güvenlik](../concepts/security.md): kapı neden sizin.
