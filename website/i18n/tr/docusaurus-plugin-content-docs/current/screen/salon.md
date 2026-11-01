---
title: Salon
description: imece'nin varsayılan ekranı - başlık satırı, projeye göre gruplanmış lonca ağacı, her agent'ın satırı ve işareti, katlama ve yandaki paneller.
---

# Salon

Salon, imece'nin açıldığı ve `1` ya da `:salon` ile geri döndüğünüz ekrandır. Bütün takımı bir bakışta gösterir: üstte bütçe ve filoyu sayan başlık satırı, solda lonca ağacı (her proje, lideri, işçileri ve yardımcıları), sağda iki yan panel: SENİ BEKLEYEN ve DEFTER (günlük). Pano ve kod haritası bir tuş uzakta (`2`, `3`); geri kalan her şey salonun üstünde açılır ve `esc` salonu geri getirir.

```text
 imece  3/9 kart │ ● 2 çalışıyor  ✓ 1 bitti │ $3.10 / $20.00 harcandı ($1.80/sa, ~9 sa kaldı) │ claude 5 sa: %48 kaldı, 1 sa 59 dk sonra yenilenir │  ! 2 seni bekliyor   14:02
┌ SALON ──────────────────────────────────────────┐┌ SENİ BEKLEYEN (2) ─────────────────────┐
│▾ ▴ shop-api  2 çalışıyor  $2.40   ! 2 bekliyor  ││Δ 13:58 claude-1 bitirdi: parser (3 dosya, +41 −6) · ✓ bu ağaçta temiz geçti: cargo test
│  ◆ ▎lead      claude             koordine ediyor││⚑ 14:01 codex-2 onayını bekliyor: Bash: npm install left-pad
│  ├ ● ▎claude-1 #3   Parse the manifest  testleri yazıyor · 2 dk  $1.10  ⎇ fix-parser ↑2 +12
│  │ └ ● ▎claude-1.1                      düşünüyor… ← claude-1 "testleri ekle"   ⎇         │
│  ├ ✓ ▎codex-2  #5   Money rounding      bitti, henüz bakılmadı → sen "yukarı yuvarlandı" · 5 dk│
│  └ … 2 agent yolunda                             │└────────────────────────────────────────┘
│▸ web  1 çalışıyor  $0.70                         │┌ DEFTER ────────────────────────────────┐
│                                                  ││13:58 ! claude-1 bitirdi: 3 dosya, +41 −6 → seni bekliyor
│                                                  ││13:55 ⇄ lead → claude-1 [görev] #3 Parse the manifest
│                                                  ││13:40 › sen: boş dosyadaki parser çökmesini düzelt
│KUYRUK  boş  │ son: #2 ✓ #1 ✓                     ││                                        │
└──────────────────────────────────────────────────┘└────────────────────────────────────────┘
  / lidere yaz  @ agent'a  # iş  tab panel  ← → katla  enter odak  ? yardım   1 salon  2 pano  3 kod
```

## Başlık satırı

Tek satır, soldan sağa, parçalar `│` ile ayrılmış:

| Parça | Ne söyler |
| --- | --- |
| `3/9 kart` | biten kartlar / bütün kartlar; kart yokken gösterilmez (canlı bir başlangıçta lider kart asana dek yoktur). |
| `● 2 çalışıyor  ✓ 1 bitti  ✕ 1 takıldı` | filo: tur ortasındaki (token harcayan) agent'lar, bir turu bitirmiş ama o zamandan beri açmadığınız agent'lar, bloke olan ya da oturumu duran agent'lar. Sıfır olan sayı yazılmaz; hiçbiri yoksa `hepsi boşta` der. Sizi bekleyenler filoda değildir: kendi sayısı vardır. |
| `$3.10 / $20.00 harcandı ($1.80/sa, ~9 sa kaldı)` | kese (bütçe): bütün agent'ların birlikte harcadığı, son on dakikanın saat başına maliyeti ve bu hızla bütçenin ne kadar dayanacağı. Yarım saatten az kaldığında parçanın tamamı dikkat rengine döner ve günlük bunu bir kez söyler. |
| `claude 5 sa: %48 kaldı, 1 sa 59 dk sonra yenilenir` | her satıcı planının dolmaya en yakın kullanım penceresi, Claude ve Codex oturumlarının bildirdiği gibi (Gemini bildirmez); %90'dan itibaren dikkat renginde. Bir aboneliğin sınırı takımı bütçeden önce durdurabilir. Çalıştırmalar arasında saklanmaz: oturum ilk turunda yeniden söyler. |
| `araştırmacı 1/4` | çalışan araştırmacılar / üst sınır; yalnız bir üst sınır varken (`--demo --researchers N`; araştırmacılar canlıda yapılmadı). |
| `! 2 seni bekliyor` | SENİ BEKLEYEN'deki açık ögeler, dikkat renginde. Terminalin başlığı aynı sayıyı taşır, `imece · 2 seni bekliyor`, böylece arkadaki bir sekme de gösterir; imece bitince kabuğunuzun başlığı geri gelir. |
| `14:02` | saat. |

Bütçenin ve pencerelerin nasıl sayıldığı [bütçe ve sınırlar](../concepts/budget-and-limits.md) sayfasında.

## Lonca ağacı

Agent'lar projeye göre gruplanır. Her proje bir başlık satırıdır; altında lideri, sonra işçileri, her işçinin altında yardımcıları. Projeler açıldıkları sırayla gelir, sabitlenenler önce.

Bir proje başlığı: `▾ shop-api  2 çalışıyor  $2.40  ✕1  ■1   ! 2 bekliyor`. `▾` açık, `▸` katlı. Adın ardından: boşta olmayan işçi ve yardımcı sayısı, projenin agent'larının harcadığı, varsa `✕N` bloke ve `■N` durmuş agent ve dikkat renginde bu projenin sizi bekleyen öge sayısı. Katlı bir proje tek satırdır, o satır da bekleyenleri gösterir (sessiz ama sağır değil) ve en yüksek üyesiyle biter, `▸ shop-api  2 çalışıyor  $2.40   ! 1 bekliyor  ! codex-2`: aşağıdaki durum merdiveninde en üstteki agent, eşitler arasında sıradaki ilk. Sabitlenmiş bir proje (`:pin`) adının önünde `▴` taşır ve salonun en üstünde kalır; sabitleme çalıştırmalar arasında saklanır.

Bir agent'ın satırı:

```text
  ├ ● ▎claude-1 #3   Parse the manifest  testleri yazıyor · 2 dk  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  ▾ 1 yardımcı  ↗ localhost:5173/
```

- Ağaç çizgileri `├ └ │` kimin kimin altında olduğunu gösterir; bir yardımcı işçisinin altındadır ve `claude-1.1` diye adlanır.
- Durum işareti: agent'ın durum merdivenindeki basamağı (aşağıdaki lejant). Takılan durumunun dediği gibi görünür (`✕` bloke, `■` durdu); bir şeyi sizi beklerken, o sırada ne yapıyorsa yapsın `!`; bir turu bitirmiş ama henüz açmadığınız bir işçi için `✓`, durum olarak `bitti, henüz bakılmadı`; oturumu başlarken `◌`; yoksa durumunun kendi işareti (`●` `◐` `◇` iş başında, `○` `◦` boşta). Her durum kendi biçimidir, mono tema da ayırt eder. Lider hep `◆` kalır.
- Agent'ın konuşmacı renginde `▎`: günlükte ve bir konuşmada satırlarının taşıdığı renk, böylece satır ve konuşması tek agent olarak okunur.
- Satıcısının renginde adı; sonra kirliyse `~` ya da `≈` (araştırma kirliliği, yalnız demoda).
- Bir işçinin kartı: `#3` ve kartın başlığı. Lider yerine satıcısını gösterir.
- Durumu: oturumu kalkarken (süreç, ilk worktree'nin checkout'u) `başlıyor…`, `düşünüyor…`, yaptığını söylediği şey, bir takım arkadaşının yanıtını beklerken `? → claude-2`, bir sorusu sizi beklerken `? → sen`, bir araç çağrısı SENİ BEKLEYEN'de beklerken `⚑ onayını bekliyor`, yanıtı okunmayı beklerken `sana yanıt verdi`, işçileri çalışan bir lider için `koordine ediyor`, `BLOKE`, `■ durdu, yazınca yeniden başlar`, `◦ önceki çalışmadan beri uyuyor, yazınca uyanır`. Durumun kendi metni varsa satır onu taşır: `? → sen "hangi index?"`, `sana yanıt verdi "enum kullan"`, `⚑ onayını bekliyor "cargo test"`, `BLOKE "JWT saat kayması" ← researcher-1`, `■ durdu: süreç 1 koduyla çıktı`. Yoksa durum, agent'ın en son alışverişiyle biter; ok kimin söylediğini gösterir: `düşünüyor… ← lead "parser'ı düzelt"`, `boşta → sen "parser düzeldi, testler yeşil"`, `? → claude-2 "hangi parser?"` (beklediği kişiye kendi sorusu, tek başına alıntılanır). Alışveriş bir sonrakine dek kalır; böylece satır agent'ın en son ne yaptığını söyler, durum ve metni kırk sütunu geçmez.
- O durumda ne zamandır olduğu, bir dakikadan başlayarak: `· 3 dk`, `· 1 sa 5 dk`. Sizi bekleyen bir şey için öğe geldiğinden beri.
- Varsa harcadığı.
- `≡ 3/7`: kendi yapılacaklar listesinin neresinde olduğu, biten adımlar ve hepsi; planlama aracı bir liste tutuyorsa (Claude Code'un görevleri, Codex'in `update_plan`'i; Gemini'nin `write_todos`'u imece'ye adımları olmadan ulaşır). Odak adımları listeler. Satır darsa işaret alıntıdan ve süreden sonra yer verir. Oturumla birlikte gider.
- `◔ %72`: bağlam penceresinin ne kadar dolu olduğu, %70'ten itibaren, %90'dan itibaren dikkat renginde, satıcının son söylediğince (Claude Code'un mesaj kullanımı ve sonucun adlandırdığı pencere, Codex'in token kullanımı güncellemesi, Gemini'nin kullanım sayacı). Odak rakamı her hâlde gösterir, `bağlam: %22 · 45k / 200k`, ya da satıcı pencerenin boyutunu söylemiyorsa yalnız token sayısını. Dar bir satırda yapılacaklar işaretinden daha uzun kalır. Oturumla birlikte gider.
- Bir işçinin worktree durumu, `⎇ fix-parser ↑2 ↓1 +12 −3`: agent'ın açtığı dal (worktree imece'nin bıraktığı gibi detached iken yok), checkout'ta olmayan kendi commit'leri (imece'nin uygulanan değişiklik işaretleri sayılmaz), kendisinde olmayan checkout commit'leri ve kendi HEAD'ine göre eklenen ve silinen satırlar, izlenmeyen dosyalar dahil: bir sonraki `done`'un size vereceği şey. Her parça yalnız söyleyecek bir şey varken görünür. imece'nin git iş parçacığında okunur: oturum başlarken, bir tur bitince, bir uygulamadan sonra ve işçi ya da bir yardımcısı tur ortasındayken her 15 saniyede; çalıştırmalar arasında saklanmaz.
- `▾ 1 yardımcı` ya da `▸ 1 yardımcı`: yardımcıları altında listelenir ya da katlanmıştır.
- `↗ localhost:5173/`: işçi ya da yardımcının kendi metninde yazdığı en yeni yerel adres, terminalin açtığı bir bağlantı olarak (OSC 8). Yalnız `localhost`, `127.0.0.1` ya da `[::1]` üzerinde `http`/`https` sayılır, sorgu ve parça atılır, 60 karakteri aşan bir adres hiç gösterilmez. Oturumla birlikte gider.
- Yardımcıları olan bir işçi `▾ 2 yardımcı` ile, katlıysa `▸ 2 yardımcı  ● claude-1.1` ile biter: sayı ve en yüksekleri.

Her agent tek satırdır, ne söylerse söylesin: satırların yüksekliği değişmez. Süre ve alıntı yalnız satırda maliyet, worktree durumu, yardımcılar ve bağlantıdan arta kalan yeri alır; dar bir bölme önce alıntıyı, sonra süreyi düşürür (`.` yan paneli daraltır). Yeni değişen bir satır bir an öne çıkar, sonra sakinleşir. Seçili satır ters renklidir.

Satırlar panele sığmadığında sakin işçiler (çalışan ya da boşta, son zamanda bir şey dememiş, seçili olmayan) salonun altından yukarıya doğru tek satıra katlanır: `… 2 agent katlı, en yükseği  ● codex-2`, en yükseklerinin adıyla; sabitlenmiş bir proje en son katlanır ve sizi bekleyen hiç katlanmaz. Panelin alt satırı `KUYRUK`, doğrulanan kartları ve son dört biteni (`#2 ✓`) listeler; canlıda doğrulama kısmını hiçbir şey doldurmaz, `boş` yazar.

## Durum merdiveni

Her agent tek bir basamakta durur, en yüksekten: takıldı (bloke ya da oturumu durdu: siz yazmadan bir yere varamaz) > bir şeyi sizi bekliyor > çalışıyor (tur ortasında, bir takım arkadaşına soruyor, araştırıyor) > başlıyor > bitti, bakılmadı > boşta (ya da uyuyor, ya da gitti). Basamak satırın işaretini, projesinde hangi işçinin önce geldiğini (takılan, sonra sizi bekleyen, sonra bitip bakılmamış; sakin basamaklar sıralarını korur ki agent'lar boştan işe gidip gelirken ekran zıplamasın), `f`'nin ne gösterdiğini ve katlı bir grubun (bir proje, bir işçinin yardımcıları, katlanan sakin işçiler) hepsi adına hangi üyeyi gösterdiğini belirler: en yüksek basamaktaki, eşitler arasında sıradaki ilk.

## Lejant

`?` yardımının listelediği her işaret, ekranın çizdiği gibi:

| İşaret | Anlamı |
| --- | --- |
| `◆` | lider (ne yapsa bu işareti taşır) |
| `◌` | başlıyor: oturumu kalkıyor, henüz yanıt vermedi |
| `●` | çalışıyor: tur ortasında |
| `◐` | bir takım arkadaşına soruyor, yanıtını bekliyor |
| `✕` | bloke (araştırma bekliyor) ya da bir proje başlığında bloke agent sayısı |
| `!` | bir şeyi sizi bekliyor, o sırada ne yapıyorsa yapsın; başlıktaki sayı ve SENİ BEKLEYEN'in kendi işareti de |
| `◇` | araştırmacı (yalnız demoda; araştırmacı katmanı canlıda yapılmadı) |
| `○` | boşta |
| `✓` | bitti, bakılmadı: bir turu bitirdi, o zamandan beri açmadınız |
| `■` | durdu: oturum bitti; ona yazmak yeni bir oturum başlatır; bir proje başlığında durmuş agent sayısı da |
| `◦` | önceki çalıştırmadan beri uyuyor; ona yazmak uyandırır |
| `⇄` | bir konuşma satırı |
| `▾` `▸` | bir proje ya da bir işçinin yardımcıları, açık ya da katlı |
| `▴` | sabitlenmiş proje |
| `~` | hafif kirli: bir araştırma raporu okudu (yalnız demoda) |
| `≈` | kirli: dış dünyayı okuyor (yalnız demoda) |
| `⎇` `↑` `↓` `+N −M` | bir işçinin worktree'si: dal, kendi commit'leri, checkout'un yeni commit'leri, henüz verilmemiş satırlar |
| `↗` | işçinin yazdığı yerel adres, terminalin açtığı bir bağlantı |
| `≡ 3/7` | kendi yapılacaklar listesi: biten adımlar ve hepsi |
| `◔ %72` | bağlam penceresi bu kadar dolu; %70'ten gösterilir, %90'dan yüksek sesle |
| `✱` | `m` ile işaretlediğiniz satır; `'` sonrakine gider |

Başlıktaki filo aynı şekilleri kullanır, böylece mono temada da okunur.

## Hareket ve katlama

| Tuş | Ne yapar |
| --- | --- |
| `↑` `↓` / `j` `k` | üstteki ya da alttaki satırı seç |
| `gg` / `G` | ilk / son satır |
| `^d` `^u` / `^f` `^b` | yarım sayfa / tam sayfa |
| `←` `→` / `h` `l` | seçili projeyi ya da seçili işçinin yardımcılarını (seçili yardımcının işçisini de) katla / aç. Bir proje başlığında `enter` da katlar ve açar. |
| `za` | eldeki katlamayı değiştir |
| `zM` / `zR` (`:fold` / `:unfold`) | her projeyi ve her işçinin yardımcılarını katla / aç |
| `f` (`:needs`) | salon yalnız sizi bekleyenleri gösterir, sonra yine herkesi |
| `s` (`:settle`) | seçili işçinin satırını ya da seçili projeyi (başlığı ya da lideri) size yeniden ihtiyacı olana dek kaldır; `:unsettle` hepsini geri getirir |
| `&` (`:goto`) | bir agent'a ya da projeye adıyla ya da numarasıyla git, sizi bekleyenler önde; projesi ve işçisinin yardımcıları açılır, satırı gizleyecekse `f` bırakılır |
| `enter` | derine in: bir agent'ın odağı (kartı, durumu, kökeni ve hikâyesi), yine `enter` ham akışı (iz) |
| `K` (`:peek`) | göz at: eldeki agent'ın odağı görünümün üstünde bir pencerede, görünüm olduğu gibi kalır; `j` `k` sonraki / önceki agent, `enter` odağın tamamı, `@` ona yaz, `esc` ya da `K` kapatır |
| `t` (`:trace`) / `v` (`:talk`) | eldeki agent'ın ham akışı / konuşması |
| `esc` | bir derinlik yukarı |

Katlama seçimi işçiye ya da başlığa taşır, böylece seçim kaybolmaz.

## Yalnız sizi bekleyenler

Bir agent'ın sizden neye ihtiyacı olduğu tek bir sıradır, en önemlisi önce: bir şeyi SENİ BEKLEYEN'de bekliyor, takılmış (bloke ya da oturumu durmuş), bakmadığınız bir turu bitirmiş, hiçbiri. Bir projede işçiler bu sıraya göre dizilir, eşitler sırasını korur, böylece sakin olanlar yer değiştirmez. Satırlar sığmadığında önce sakinler katlanır.

`f` yalnız bir şeye ihtiyacı olan agent'ları tutar, her biri projesinin başlığı altında: bir işçi, bir şeye ihtiyacı olan yardımcısı için gelir, lider yalnız kendisinin ihtiyacı varsa. Hiçbir şeyi olmayan proje dışarıda kalır, bir şeyi olan katlı proje yalnız başlığıyla görünür ve hiçbir şey yoksa salon `Şu an senden bir şey gerekmiyor · f herkesi gösterir` der. Panel başlığı süzgecin açık olduğunu söyler: `SALON · yalnızca seni bekleyenler (f)`. Açıkken, satırı giden bir seçim projesinin hâlâ görünen ilk satırına taşınır (yazarken değil: seçim `/` ile kime yazdığınızı belirler). Süzgeç yalnız bu çalıştırma boyunca sürer.

## Biteni kaldırmak

İşini bitirmiş ve baktığınız bir işçi yine de bir satır tutar. Satırında `s` onu kaldırır: satır, yardımcıları ve onun için çalışan araştırmacıyla birlikte, size yeniden ihtiyacı olana dek gider. Bir projenin başlığında ya da liderinin satırında `s` bütün projeyi kaldırır. Seçim hâlâ görünen sonraki satıra (yoksa öncekine) geçer.

Sizi bekleyen ya da takılmış bir şey hiç kaldırılmaz: `kaldırılmadı: claude-2 seni bekliyor`. Henüz görülmemiş bitmiş bir tur sorun değildir; kaldırmak onu görülmüş sayar. Kaldırılmış bir agent, satırındaki bir şeyin size yeniden ihtiyacı olduğu anda kendiliğinden döner: SENİ BEKLEYEN'de bir soru ya da değişiklik, bloke ya da durmuş olmak, ya da yeni bir turu bitirmek. Kaldırılmış bir proje, agent'larından biri dönünce döner. Hiçbir şey sessizce gitmez: panel başlığı kaldırılanları sayar, `SALON · 2 kaldırıldı (:unsettle)`, başlık satırındaki filo yine her agent'ı sayar ve `&` hepsini yine listeler; `&` ya da `:goto` ile birine gitmek satırını (ve projesini) geri getirir. `:unsettle` hepsini bir kerede geri getirir. Kaldırma yalnız bu çalıştırma boyunca sürer.

## Göz atmak

`K` eldeki agent'ın (salonda seçili olan, bekleyen kalemin ya da günlük satırının ardındaki, ya da yakınlaştığınız) odağını ekranın üstünde, ondan biraz küçük bir pencerede açar; alttaki görünüm (salon, pano, kod haritası ya da bir kart) kıpırdamaz. Pencerede `j` `k` (ya da oklar) salonun sırasında sonraki / önceki agent'a geçer, `enter` odağın tamamını açar, `@` ona bir mesaj başlatır, `esc` ya da `K` kapatır; başka tuşlar bir şey yapmaz. Bitmiş bir agent'a böyle bakmak onu görmüş sayılır.

## Yan paneller

Sağda üstte SENİ BEKLEYEN, altında DEFTER. `tab` ve `⇧tab` (ya da `]` ve `[`) panelleri döndürür: salon → bekleyenler → günlük; `w` (`:waiting`) doğrudan bekleyenler paneline gider. Bekleyenler paneli ögelerinin gerektiği kadar satır alır, en çok yan tarafın yarısı; ögeler beklerken ve panel odakta değilken çerçevesi uyarı rengine döner.

`,` ve `.` (ya da `<` ve `>`) yan paneli dört sütun genişletir ya da daraltır, ekranın %20'si ile %70'i arasında; flaş `yan panel %38 (, . ya da < >)` der ve genişlik sonraki çalıştırma için saklanır. Bir değişikliğin görünümünde `>` bunun yerine bir parçayı alıntılar; `.` yine daraltır.

## Okunaklılık

Salonun üstünde açılan bir pencere (bir konuşma, bir göz atma, yardım, uzaktayken özeti, nerede kalmıştın) çevresindeki her şeyi soluklaştırır, böylece ağaç ve günlüğün kesilen satır sonları arka plan olarak okunur. Günlük paneli ve bekleyenlerdeki bir soru, bir mesajı tek satır düz söz olarak gösterir: temizlenmiş, satırları birleştirilmiş, başlık ve liste işaretleri atılmış, bir bağlantı etiketiyle. İz, satırın ilk sözcüğüne göre renklenir: agent'ın kendi metni satıcı renginde, araç çağrıları bilgi renginde, bir çalıştırma çıkış koduna göre, bus sözcükleri vurgu renginde. Açık bir konuşma konuşmayı izler: gelen satırlar eklenir, seçim son satırdaysa onunla gider.

Bir agent'ın yazdığı her şey ekrana yalnız kontrol karakterlerini atan ve gizli anahtara benzeyen parçaları saklayan bir temizlikten geçerek ulaşır; hiçbir şey ham çizilmez.

## Ayrıca bakınız

- [SENİ BEKLEYEN](waiting.md)
- [Günlük](journal.md)
- [Konuşmalar](conversations.md)
- [Pano ve kod haritası](board-and-code-map.md)
- [Klavye](keyboard.md)
- [Bütçe ve sınırlar](../concepts/budget-and-limits.md)
- [Projeler](../concepts/projects.md)
