---
title: "Klavye"
description: "imece'nin bildiği her tuş, çalıştığı yere göre gruplanmış, : adıyla - ? yardımının gösterdiği listenin aynısı, ve o yardımın nasıl süzüldüğü."
---

# Klavye

imece önce klavyedir: her şey tuşlarla yapılır, vim biçiminde, okların yanında; fare hiç gerekmez ve hiç yakalanmaz, böylece terminalinizin kendi seçimi çalışmayı sürdürür. Bu sayfa tam listedir, `?`'nin ekranda iki dilde gösterdiğinin aynısı. İş yapan her tuşun bir de `:` adı vardır, böylece her yerden yazılabilir; hareket tuşlarının ve seçili satıra iş yapan tuşların adı yoktur.

Gelenekler vim'inkidir: `h` `j` `k` `l` hareket ve katlama, `gg` ve `G` uçlara, `za` `zM` `zR` katlama, `^d` `^u` `^f` `^b` sayfa, `/` ve `i` yazmaya başlar, `esc` geri, `q` çıkar (sorduktan sonra), `m` ve `'` işaretler ve işarete gider, `%` eşleşen mesaja atlar, `^o` bırakılan yerlerden geriye gider, `[` `]` bölümden bölüme. `^a` ctrl-a demektir, `⌥b` alt-b (Mac'te Option, Meta olarak gönderilmiş), `⇧tab` shift-tab. Tuşlar iki dilde aynıdır; bu tablo tuş sütununu İngilizce yardımın yazdığı gibi verir (`@name text`, `/cmd`, `PATH`, `NAME`), Türkçe yardım aynı tuşları `@ad mesaj`, `/kmt`, `YOL`, `AD`, `#kelime`, `!kmt` diye yazar.

## Yardım penceresi

`?` (`:help`) YARDIM'ı açar: her tuş ne yaptığıyla, geniş ekranda iki sütunda, sonra işaretlerin lejantı, sonra terminalinizin yeni satır tuşu. Herhangi bir tuş kapatır, `/` dışında: o bir süzgeç başlatır, pencerenin son satırında yazılır. Basılan her tuş süzgece girer (`?`, `q` ve `/` de), böylece yazarken yardım açık kalır; backspace bir harf geri alır, `^u` boşaltır. Bir tuş satırı ya da lejant satırı, süzgecin her sözcüğü iki sütundan birinde geçiyorsa kalır: `zM` katlama tuşlarını bulur, `ilk son` `gg` `G`'yi bulur. Büyük küçük harf ve Türkçe harfler düz Latin'e katlanır (`calis` `çalış`'ı bulur, `BÜTÇE` `bütçe`'yi). Hiçbir şey eşleşmezse söyler. `enter` süzgeci tutar ve yazmayı bitirir (sonra `/` değiştirir, başka bir tuş kapatır); `esc` süzgeci bırakır ve bütün yardım geri gelir; boş süzgeçte backspace de bırakır. Pencere süzülürken boyutunu korur, zıplamaz.

## Yazmak

| Tuş | Ne yapar |
| --- | --- |
| `/`  `i` | seçili projenin liderine yaz |
| `@name text` | tek bir agent'a yaz: `@name` agent, `text` mesaj |
| `/cmd`  `@name /cmd` | agent'ın kendi komut satırı (`/clear`, `/compact`, `/cost` …) |
| `:` | imece'ye bir komut; `tab` komutların değerleriyle bir menü açar |
| `$` | `:` menüsünde: bu komut sonraki faturayı büyütür |
| `:budget USD` | agent'lar çalışırken bütçeyi artır ya da azalt; `USD` `80` ya da `2.50` gibi dolar |
| `:lead VENDOR` | sonraki açılıştan itibaren kim lider; `VENDOR` `claude`, `codex`, `gemini` ya da `ask` |
| `:open PATH` | `PATH`'teki çalışma kopyasını proje olarak aç; ardına bir `NAME` adlandırır |
| `:solo CHOICE` | lidersiz, kendin sürdüğün bir solo oturum: `CHOICE` `satıcı[:model][@çaba]`; bir `PATH` ve bir `NAME` de gelebilir; seçimsiz bir ekran sorar |
| `:effort LEVEL` | eldeki solo sonraki turundan itibaren `LEVEL`'da düşünür (`default`: satıcının kendisi) |
| `:model NAME` | eldeki solo sonraki turundan itibaren `NAME` üzerinde çalışır (`default`: satıcının kendisi) |
| `:close NAME` | `NAME` projesini kapat: agent'lar ayrılır, worktree'ler kalır |
| `:rename NAME` | eldeki projeyi adlandır; lideri `lead-NAME` olur |
| `:forget @NAME` | `@NAME` agent'ı oturumunu unutur; yenisine kaldığı yer anlatılır |
| `:export PATH` | açık konuşma ya da kart, Markdown olarak, `PATH`'e (bir dosya ya da dizin) |
| `:commit` | ucuz bir model stage'dekiler için commit mesajı önerir (`$`) |
| `:copy PATH` | git'in yok saydığı bir yolu (`.env`) worktree'lere kopyala; yine: bırak |
| `:pin NAME` | projeyi salonun en üstüne sabitle; yine: bırak |
| `!cmd` | `cmd`'yi kendi kabuğunuzda çalıştır, ekran kenara çekilir |
| `tab` `⇧tab` `↑` `↓` | yazarken: ad ve komut tamamla (`^n` `^p` de ilerler) |
| `^a` `^e` `⌥b` `⌥f` | yazarken: başa / sona, bir sözcük geri / ileri |
| `^w` `^u` `^k` `^y` | yazarken: bir sözcük / imleçten öncesini / sonrasını kes; kesileni koy |
| `⇧enter` `⌥enter` | yazarken: yeni satır; `↑` `↓` satırlar arası (terminal kitty klavye protokolünü konuştuğunda) |
| `\enter` `⌥enter` | yazarken: yeni satır (burada `⇧enter` = `enter`); `\` sonra `enter` her terminalde çalışır |
| `enter` | gönder; `esc` vazgeçer |

## Hareket

| Tuş | Ne yapar |
| --- | --- |
| `#N`  `#word` | numara ya da başlıkla bir kart: hepsi tek ekranda |
| `n`  `p` | kart ekranında: sonraki / önceki kart |
| `1` `2` `3` | `:salon` · `:board` pano · `:code` harita |
| `↑` `↓`  `j` `k` | seç |
| `gg`  `G` | ilk / son satır |
| `^d` `^u` `^f` `^b` | yarım sayfa / tam sayfa |
| `^o` | olduğunuz yere geri: görünüm, panel, odak, kart (vim'in atlama listesi, elli yer; açık konuşma önce kapanır) |
| `enter` | derine in: odak, sonra ham akış |
| `t`  `:trace` | eldeki agent'ın ham akışı |
| `v`  `:talk` | onun sizinle konuşması |
| `esc` | geri |
| `tab` `⇧tab` `[` `]` | sonraki / önceki panel |
| `,`  `.`  `<`  `>` | yan paneli genişlet / daralt |

## Salon

| Tuş | Ne yapar |
| --- | --- |
| `←` `→`  `h` `l` | projeyi ya da işçinin yardımcılarını katla / aç |
| `za`  `zM`  `zR` | katla: değiştir · hepsi (`:fold`) · hiçbiri (`:unfold`) |
| `f`  `:needs` | salon: yalnız sizi bekleyenler · herkes |

## SENİ BEKLEYEN

| Tuş | Ne yapar |
| --- | --- |
| `w`  `:waiting` | doğrudan sizi bekleyenlere |
| `W`  `S` | `:next` bekleyen, her yerden · `:defer` bunu ertele ve sıradakini aç |
| `a`  `r` | onayla / uygula / izin ver · reddet (sizi bekleyenlerde) |
| `enter` | ögeyi aç: bir değişikliğin diff'i, bir sorunun konuşması, bir yanıt (bununla okunur) |

## Bir konuşma

| Tuş | Ne yapar |
| --- | --- |
| `enter`  `%` | konuşma (bir günlük satırından) · içinde `%`: yanıt ↔ soru |
| `[`  `]` | söz sırasından söz sırasına (bir tarafın diğeri yanıt vermeden önce söylediği satırlar) |
| `c` `C`  `y` `Y` | mesajı · konuşmayı kopyala |
| `m`  `:mark` | eldeki satırı sonra dönmek için işaretle; yine: kaldır |
| `'` | sonraki işaretli satır: konuşma, yoksa günlük |
| `space`, `pgup` `pgdn` | bir sayfa |
| `esc` | kapat |

## Bir değişiklik

| Tuş | Ne yapar |
| --- | --- |
| `[`  `]` | değişiklikte: önceki / sonraki dosya · konuşmada: söz sırası |
| `>` | değişiklikte: üstteki parça, işçisine bir mesajda alıntı |
| `c`  `n` | değişiklikte: üstteki satıra not · notları gönder, geri döner |
| `x` | değişiklikte: üstteki parçayı seç; `a` yalnız seçilenleri uygular |
| `w` | değişiklikte: uzun satırları sar · yine kes |
| `a`  `r` | uygula (seçili parçalar, yoksa hepsi) · reddet |
| `j` `k`, `gg` `G`, `^d` `^u` `^f` `^b`, `space` | hareket |

## Kart ekranı

| Tuş | Ne yapar |
| --- | --- |
| `#N`  `#word` | bir kart aç |
| `n`  `p` | sonraki / önceki kart |
| `esc` | salona geri |
| `tab`, `enter`, `a` `r`, `c`, `t`, `v`, `m`, `W` | ana ekrandaki gibi |

## Yalnız demo

| Tuş | Ne yapar |
| --- | --- |
| `space` | `:pause` simülasyonu durdur / devam |
| `+`  `-` | `:faster` / `:slower` simülasyon (`=` de `+` sayılır) |

## Her yerde

| Tuş | Ne yapar |
| --- | --- |
| `L`  `:lang` | English / Türkçe; `:lang en\|tr` |
| `T`  `:theme` | sonraki tema · `:theme NAME` ya da `FILE` |
| `:awake` | tur ortasında makineyi uyanık tut; yine: kapat |
| `:notify` | başka penceredeyken bir şey beklemeye düşerse haber: `:notify off\|bell\|text`; yalın yazılınca sıradaki |
| `q` `q`  `:quit` | çık (`q` ya da `:quit` önce sorar; `^c` hemen çıkar) |
| `q` `s`  `:pending` | çıkmadan önce: tur ortası, incelenmemiş |
| `?`  `:help` | bu yardım; içinde `/` tuşları süzer |
| `W` | bekleyen sıradaki öge, her yerden |
| `^o` | olduğunuz yere geri |

## Lejant

| İşaret | Anlamı |
| --- | --- |
| `◆` lider  `●` çalışıyor  `◐` soruyor  `✕` bloke  `!` onayını bekliyor | bir agent'ın durumu |
| `◇` araştırmacı  `○` boşta  `✓` bitti, bakılmadı  `■` durdu  `⇄` konuşma  `▾` `▸` proje / yardımcılar  `▴` sabit | |
| `~` hafif kirli (rapor okudu)  `≈` kirli (dış dünyayı okuyor) | yalnız demo: araştırmacılar canlıda yapılmadı |
| `⎇` dal  `↑` kendi commit'leri  `↓` checkout'un yeni commit'leri  `+N −M` henüz verilmemiş satırlar | bir işçinin worktree'si |
| `↗` worker'ın yazdığı yerel adres (localhost), terminalin açtığı bir bağlantı | |
| `✱` işaretlediğiniz satır (`m`), çalıştırmalar arasında kalır; `'` sonrakine gider | |

## Her `:` komutu ve tuşu

| Komut | Tuş |
| --- | --- |
| `:salon` `:board` `:code` | `1` `2` `3` |
| `:lang` | `L` |
| `:theme` | `T` |
| `:help` | `?` |
| `:quit` (`:q`) | `q` `q` |
| `:pending` | `q` `s` |
| `:next` / `:defer` | `W` / `S` |
| `:waiting` | `w` |
| `:needs` | `f` |
| `:fold` / `:unfold` | `zM` / `zR` |
| `:trace` / `:talk` | `t` / `v` |
| `:mark` | `m` |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` (demo) |
| `:budget` (`:b`), `:lead`, `:open` (`:o`), `:close` (`:c`), `:rename`, `:solo`, `:effort`, `:model`, `:forget` (`:f`), `:export`, `:commit`, `:copy`, `:pin`, `:awake`, `:notify` | tuşu yok: `:` ardından yazılır |

Her komutun ne aldığının tam listesi [komutlar başvurusunda](../reference/commands.md).

## Ayrıca bakınız

- [Komut satırı](command-line.md)
- [Salon](salon.md)
- [SENİ BEKLEYEN](waiting.md)
- [Değişiklik görünümü](changes.md)
- [Komutlar başvurusu](../reference/commands.md)
