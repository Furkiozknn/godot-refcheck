# Tasarım: godot-refcheck arayüz yenilemesi

## Hedef

Yeni gelen biri README'nin ilk ekranında ne olduğunu, nasıl kurulacağını ve çıktının neye benzediğini görsün; yanlış bir komut yazdığında ekranı dolduran yardım metni yerine tek satırlık "ne yanlış, ne yapmalı" görsün. Çıkış kodları (0/1/2), JSON/SARIF sözleşmesi ve mevcut 147 test aynen kalır.

## Önce / sonra

| | Önce | Sonra |
|---|---|---|
| README ilk ekranı | banner, 15 sn reel (üreticisiz), rozetler, gif (üreticisiz), uzun paragraflar; kurulum aşağıda | tanım, rozetler, iki satırlık kurulum + kullanım, gerçek çıktılı demo, ne zaman kullanılır / kullanılmaz |
| Kullanım hatası | hata satırı + tüm USAGE (31 satır) | hata satırı + `Run 'godot-refcheck --help' ...` (2 satır); stdout boş |
| `--fail-on bogus` | `unknown level: bogus` | `unknown level: bogus (expected error, warning, info or never)` |
| `--only nope` | `unknown check: nope` | `... (godot-refcheck --list-checks shows the names)` |
| iki yol | `only one PATH is accepted` | iki yolu adıyla söyler, `--recursive` önerir |
| `--help` | seçenekler + çıkış kodları | + EXAMPLES bölümü; çıkış kodu 2 "kullanım hatası ya da okunamayan proje" |
| `--list-checks` | 18 karakter sütun, uzun kimlikte kayar | sütun genişliği en uzun kimlikten hesaplanır |
| demo | elle yapılmış, yeniden üretilemez | `tools/demo_uret.py`: deponun kendi `tests/projects/moved` fixture'ı, gerçek komut, gerçek çıktı |

## CLI akışı (demo)

```
godot-refcheck --version
godot-refcheck --list-checks | head -n 5
godot-refcheck moved            # 3 hata, çıkış 1
godot-refcheck moved --fix      # 3 onarım
godot-refcheck moved            # temiz, çıkış 0
godot-refcheck moved --fail-on warnin   # yeni hata mesajı, çıkış 2
```

Demo CI'ın derlediği 0.3.0 ikilisiyle kaydedildi (bu makinede cargo yok); `.github/workflows/ci.yml` her işletim sisteminde ikiliyi 14 gün tutulan bir artifact olarak yükler, `tools/demo_uret.py --bin <ikili>` oradan yeniden kaydeder.

## Görsel kimlik

FRK-OS: zemin `#0e0d0b`, panel `#14120e`, krem `#f1ece2`, sarı `#ffc21a`, JetBrains Mono (SIL OFL 1.1, `assets/yazi/`, metin dahil). Renk ve sahne sistemi `sosyal/uret/tema.mjs` "klasik" temasından (mcp-vet yenilemesindeki terminal sayfasıyla aynı sayfa şablonu). Vurgu renkleri: hata `#ff4d6d`, uyarı `#ff7a1a`, ipucu sarı. Kontrast (WCAG göreli parlaklık, hesaplandı): krem/panel 15,9:1, sönük gri `#b6ae9d`/panel 8,5:1, sarı/panel 11,6:1, hata pembesi `#ff4d6d`/panel 5,8:1, uyarı turuncusu `#ff7a1a`/panel 7,2:1; hepsi ≥4,5:1.

## Dokunulmayanlar

Sürüm numarası, etiket, release, action sürümü/etiketi, GitHub description/homepage, Pages. `assets/banner.svg` (profil deposunun üreticisinden).
