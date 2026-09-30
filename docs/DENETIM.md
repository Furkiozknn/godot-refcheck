# Denetim: godot-refcheck (30 Eylül 2026)

Yenilemeden önce, bu makinede (Windows 11, Git Bash) ölçüldü. Bu makinede Rust araç zinciri yok; bu yüzden "önce" ölçümü **yayımlanmış v0.2.0 Windows ikilisiyle** yapıldı (release'ten indirildi, `.sha256` doğrulandı) ve "sonra" ölçümü **CI'ın Windows işinin ürettiği 0.3.0 ikilisiyle** (PR #14, koşu 36643194075, artifact `godot-refcheck-Windows`). Ölçülmeyen bir şey yazılmadı. Ham çıktılar depo dışında: `kanit/godot-refcheck/{once,sonra}/`.

## Sürüm farkı (önemli)

`main` Cargo.toml'da 0.3.0; en son yayımlanan release v0.2.0 ve `v0.3.0` etiketi **yok**. Bu iki sonuç doğurur:

- "Önce" ikilisi (v0.2.0) `main`'in bugün yaptığının gerisinde. Örnek: `tests/projects/clean` v0.2.0'da `res://addons is not in the project` hatası veriyor (CHANGELOG'da "Unreleased / Fixed": klasör varsa artık eksik sayılmıyor), 0.3.0 CI ikilisinde çıkış 0. Arayüz farkı küçük: v0.2.0 ile `main` arasında `src/main.rs`'te tek değişiklik var, olmayan yol için ayrı bir `does not exist` mesajı (v0.2.0 bunu "proje içinde değil" diye söylüyor). Aşağıdaki tablo bu yüzden v0.2.0 ölçümünü "önce" alır ve `main`'de zaten düzelmiş olanı ayrıca belirtir.
- README'nin CI bölümü `Furkiozknn/godot-refcheck@v0.3.0` diyor (iki yerde); etiket atılana kadar bu satır kopyalanınca çalışmaz. Etiket/sürüm kararı Furki'nin; bu PR'da dokunulmadı.

## Kurulum ve ilk sonuç

| Yol | Süre | Not |
|---|---|---|
| `curl` ile v0.2.0 Windows zip'i (219 KB) + `Expand-Archive` + `godot-refcheck --version` | 2,8 s | ikili 446 KB, çalışma zamanı bağımlılığı yok |
| `cargo install --git ...` | **ölçülmedi** (cargo yok) | CI'da `cargo build --release` macOS'ta 12,9 s (önbelleksiz, bağımlılıksız) |

"Tek komut, bir dakikada ilk sonuç" ikili indirme yolunda tutuyor. `cargo install` süresi ölçülemedi; README'de sayı verilmedi.

## Komutlar ve hata mesajları (22 komut, çıkış kodları hep doğruydu)

Çıkış sözleşmesi (0 temiz, 1 bulgu, 2 kullanım/okunamayan proje) her durumda tutuyor. Sorun sözlerdeydi:

| Girdi | Önce | Sorun |
|---|---|---|
| `--fail-on bogus`, `--only nope`, `--only` (değersiz), iki yol, `--frobnicate`, `--fix --fix-dry-run`, `--write-baseline` (değersiz) | hata satırı + **tüm USAGE metni (31 satır)** | Asıl mesaj en üstte, ekranın dışına kayıyor; her yanlış yazımda aynı duvar |
| `--fail-on bogus` | `unknown level: bogus` | geçerli değerleri söylemiyor |
| `--only nope` | `unknown check: nope` | adları nerede bulacağını söylemiyor |
| `godot-refcheck a b` | `only one PATH is accepted` | hangi iki yol, ve klasör listesi için `--recursive` var demiyor |
| proje olmayan klasör | `... is not inside a Godot project` | ne verilmesi gerektiğini söylemiyor |
| `--recursive` boş klasör | `no project.godot found under bos` | `--recursive`'in ne aradığını söylemiyor |
| `--baseline yok.txt` | `cannot read yok.txt: ...` | baseline dosyası olduğu belli değil, nasıl üretileceği yok |
| `--list-checks` | 18 karakterlik sütun; `duplicate-class-name` (20) taşıp sütunu kaydırıyor | hizasız çıktı |
| olmayan yol | v0.2.0: `not inside a Godot project` (`main`'de düzelmiş: `does not exist`) | v0.2.0'a özgü, `main`'de yok |
| `--help` | seçenekler + çıkış kodları | örnek yok; çıkış kodu 2 "proje okunamadı" diyor, kullanım hatasını da kapsıyor |

Olduğu gibi bırakılanlar: işletim sistemi hata metni Windows'ta yerelleşmiş geliyor (`Sistem belirtilen dosyayı bulamıyor`); bu Rust'ın `io::Error` çıktısı, tasarım gereği aynen aktarılıyor. `--sarif` yazılamazsa rapor önce basılıyor, sonra hata veriliyor (çıkış 2); rapor kaybolmadığı için değiştirilmedi.

## README bulguları

- İlk ekranda kurulum komutu yoktu (banner, 15 sn'lik reel, rozetler, gif, uzun açıklama; kurulum "Install" bölümündeydi). Yeni ilk ekran: tanım, tek komutlu kurulum, gerçek çıktılı demo, ne zaman kullanılır/kullanılmaz.
- `docs/reel/reel.gif`, `reel.mp4` ve `assets/demo.gif`: depoda **üreticisi yok** (tools/ altında biri de yok). Yeniden üretilemediği için README'den ve depodan çıkarıldı (git geçmişinde duruyor). Yerine `tools/demo_uret.py` ile gerçek çıktıdan üretilen `docs/demo/demo.gif` ve düz metin kaydı `docs/demo/komutlar.txt` geldi.
- `assets/banner.svg` profil deposundaki üreticiden geliyor, bu depoda elle değiştirilmedi.
- "147 tests": koşudan geliyordu (CI günlüğü `=== 147 tests passed ===`); yeni testlerle 152, üç işletim sisteminde de `=== 152 tests passed ===`. `project-meta.json` ve README güncellendi.
- "6,991 dosya / 141 proje ... bir buçuk saniyede" ve korpus sayıları (237 proje, 17.198 dosya, 50 bulgu): bu denetimde **yeniden ölçülmedi**. Haftalık `Corpus` iş akışı `--expect docs/corpus.md` ile kırmızıya döner; son iki koşu (22 ve 28 Eylül) yeşil.
- 147 sayısı `#[test]` sayımıyla da tutuyordu (147 işaret).

## CI

`main`'deki son CI koşuları yeşil (28 Eylül). Günlük "Ekosistem denetimi" (#19, profil deposu): bu depoya ait açık bulgu **yok**; kapatılacak bir şey bulunmadı. Oyun depolarının kullandığı action sürümü/etiketi bu çalışmada değişmedi.

## Çözülmeyenler

- `v0.3.0` etiketi yok; README'deki action satırı etiket atılana kadar çalışmaz (Furki'nin sürüm kararı).
- `--baseline` okunamadığında mesajda işletim sistemi metni ile bizim ipucumuz art arda parantezle geliyor; kozmetik, dokunulmadı.
- `cargo install` süresi ölçülemedi.
