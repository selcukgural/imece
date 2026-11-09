---
title: Kopya kâğıdı
description: "imece'yle ilk haftanın tuşları ve komutları, yazdırılabilir tek sayfada - başlamak, yazmak, karar vermek, bir değişikliği okumak, gezinmek, işe yarar : komutları, kendi kabuğunuz, çıkmak ve geri gelmek."
hide_table_of_contents: true
---

# Kopya kâğıdı

imece kurulup [tur](videos.mdx) izlendikten sonra gereken tuşlar ve komutlar, yazdırabileceğiniz tek bir sayfada. Ekranda `?` bulunduğunuz ekranın tuşlarını, yine `?` her tuşu listeler; listenin tamamı [Klavye](screen/keyboard.md) ve [Komutlar](reference/commands.md) sayfalarındadır.

## Başlamak

| Komut | Ne yapar |
| --- | --- |
| `imece` | bir git checkout'unda: lider tek başına, siz yazana dek boşta |
| `imece --model sonnet` | liderin ve standart kademedeki işçilerin çalıştığı model; `opus@xhigh` bir çaba ekler |
| `imece --budget 10` | bütün ekibin bütçesi, dolar; çalıştırmalar arasında kalır (ilk seferde 50 $) |
| `imece --agents 8` | liderin işe alabileceği en çok işçi (0-16, varsayılan 4) |
| `imece --run npm,make` | işçilerin `run` aracıyla `cargo`'nun yanında çalıştırabileceği programlar; varsayılan kısıtta size sormadan da |
| `imece --demo` | bütün ekran iş başında, sahte ajanlarla, bedava |

## Yazmak

| Tuş | Ne yapar |
| --- | --- |
| `/`  `i` | eldeki projenin liderine yaz; `enter` gönderir, `esc` vazgeçer |
| `@name text` | tek bir ajana yaz; `tab` adı tamamlar |
| `⇧enter`  `\` `enter` | yeni bir satır (hangisinin terminalinizde çalıştığını alt satır söyler) |
| `↑` `↓`  `^r` | daha önce gönderilen satırlar · onlarda ara |
| `^x` `^e` | satır editörünüzde (`$VISUAL`, `$EDITOR`, yoksa `vi`) |
| `/cmd`  `@name /cmd` | ajanın kendi komutu (`/compact`, `/cost`, `/clear`) |

## Karar vermek

Sizi bekleyenler ana ekranın üstünde, KARAR BEKLEYEN altındadır: önce izinler, sonra sorular, sonra değişiklikler. Eldeki satır onu karara bağlayan tuşlarla biter, bir karar sizi sonraki satıra götürür.

| Tuş | Ne yapar |
| --- | --- |
| `W` | karar verilecek sıradaki şey, açılmış, her yerden |
| `S` | bunu sonraya bırak; sıradaki açılır |
| `enter` | satırı aç: bir değişiklik diff'inde, bir soru bütünüyle, liderin planı bir pencerede |
| `a` | evet: bir komuta bir kez izin, bir değişikliği onayla (checkout'unuza uygulanır, stage'de; Lonca'da öneri onaylanır), bir plana evet |
| `r` | hayır: bir komutu reddet, bir değişikliği geri gönder (nedenini yazarsınız), bir plana hayır |
| `e`  `:yes` | bir izinde: o ajan turunu bitirene dek ne isterse evet |
| `A`  `:allow` | bir izinde: o komuta proje için bundan sonra hep evet |
| `1`-`9` | bir sorunun yanıtları, satırın numaraladığı gibi |
| `/` | bir soruda: kendi sözlerinizle yanıtla |
| `n` | plan penceresinde: yanıtlamak yerine lidere yaz |

## Bir değişikliği okumak

| Tuş | Ne yapar |
| --- | --- |
| `tab`  `⇧tab` | bir işin sekmeleri: `diff · konuşma · log` |
| `j` `k`  `^d` `^u`  `gg` `G` | kaydır: bir satır, yarım sayfa, uçlar |
| `[`  `]` | önceki / sonraki dosya |
| `h`  `l` | eldeki dosyayı başlığına katla · aç |
| `c` | üstteki satıra bir not |
| `n` | notları gönder; değişiklik işçisine geri döner |
| `x` | üstteki parçayı seç; `a` sonra yalnız seçilenleri uygular |
| `>` | üstteki parça, işçisine bir mesajda alıntı |
| `b` | git reddettiyse: çakışanı geri gönder |
| `w` | uzun satırları sar · yine kes |

## Gezinmek

| Tuş | Ne yapar |
| --- | --- |
| `esc` | geri |
| `^o` | olduğunuz yere geri, vim'in atlama listesi gibi |
| `&`  `:goto` | adıyla bir ajana ya da projeye; `1`-`9` n'inci, `!` yalnız sizi bekleyenler |
| `#N` | numarasıyla bir kart |
| `gd`  `gw` | ana ekranda: karar bekleyen ilk satır · çalışan ilk satır |
| `[`  `]` | ana ekranda: bir seferde bir proje |
| `t`  `v` | eldeki ajanın ham akışı · onun sizinle konuşması |
| `?` | bu ekranın tuşları; yine `?` her tuş; içinde `/` arar |

## İşe yarar `:` komutları

| Komut | Ne yapar |
| --- | --- |
| `:commit` | ucuz bir model stage'dekiler için bir mesaj önerir, düzenleyip çalıştırmanız için satıra `!git commit -m '…'` olarak (en çok 0,05 $) |
| `:lonca import` | checkout'tan bir [Lonca](concepts/projects.md#loncada) deposu yap (penceresinde `a` içe aktarır) |
| `:sync` | Lonca'nın trunk'ını checkout'unuza getir, yalnız ileri sararak |
| `:code` | kod haritası: Lonca'da kimin neyi lease'lediği |
| `:journal`  `J` | günlük bir pencerede; oradaki `J` hikâyeyi karta göre anlatır |
| `:budget 80` | ajanlar çalışırken bütçeyi artır ya da azalt |
| `:yes` | `e`: bir ajanın isteklerine turu bitene dek evet |
| `:allow npm run` | projenin ajanlarının sormadan çalıştıracağı bir program ve bir kelime; yalnız yazılınca liste |
| `:plan on\|off` | lider ikinci bir işçiyi işe almadan önce planını size sunar (varsayılan açık) |

## Kendi kabuğunuz

| Tuş | Ne yapar |
| --- | --- |
| `!cmd` | `cmd`'yi kendi kabuğunuzda çalıştır, ekran kenara çekilir |
| `!git commit` | onayladığınızı commit edin: stage'de durur |
| `O` | son `!cmd`'yi (nasıl çalıştığı, son 40 satırı) eldeki ajana bir mesajda alıntıla |

## Çıkmak ve geri gelmek

| Tuş | Ne yapar |
| --- | --- |
| `q` `q` | çık; ilk `q` sorar, tur ortasında olanı ya da incelenmemişi söyler |
| `q` `s` | çıkmanın bırakacaklarını listele: tur ortasındaki ajanlar, incelenmemiş değişiklikler |
| `^c` | hemen çık |
| `imece` | aynı checkout'ta yeniden: aynı ekran, siz yazana dek her ajan uykuda ve bedava, sonra eski oturumunda devam ([oturumlar](concepts/sessions.md)) |
