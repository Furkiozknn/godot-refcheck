# Corpus results

Rules are only worth having if they stay quiet on projects that work. These
eleven repositories are real, maintained Godot projects; `tools/corpus.py`
clones them and runs `godot-refcheck` over every `project.godot` inside.

Measured on 2026-09-22 with godot-refcheck 0.1.0, shallow clones of the default
branch unless a branch is named.

| repository | projects | files | references | findings |
| --- | ---: | ---: | ---: | ---: |
| godotengine/godot-demo-projects | 139 | 4,040 | 2,007 | 6 |
| godotengine/godot-demo-projects@3.x | 89 | 2,257 | 1,442 | 10 |
| godotengine/godot-benchmarks | 1 | 896 | 73 | 0 |
| Orama-Interactive/Pixelorama | 1 | 1,380 | 682 | 0 |
| RodZill4/material-maker | 1 | 2,912 | 922 | 4 |
| mbrlabs/Lorien | 1 | 212 | 131 | 0 |
| GDQuest/godot-open-rpg | 1 | 1,373 | 627 | 5 |
| Maaack/godot-game-template | 1 | 740 | 346 | 0 |
| Maaack/Godot-Menus-Template | 1 | 593 | 211 | 0 |
| MakovWait/godots | 1 | 4,070 | 169 | 0 |
| bitbrain/beehave | 1 | 832 | 151 | 0 |
| **total** | **237** | **19,305** | **6,761** | **25** |

25 findings: 10 errors and 15 warnings. Each one was checked by hand against
the repository it came from, and each one is a real defect. No other reference
in those 6,761 produced a finding.

## The ten errors

| project | finding | confirmed by |
| --- | --- | --- |
| `3d/soft_body_physics` | `config/icon` is `uid://c5uks4gy25jah`, which no file in the project owns | the engine prints `ERROR: Unrecognized UID: "uid://c5uks4gy25jah"` |
| `3d/tonemap_color_correction` | same, `uid://drhyssghe5h2u` | same message from the engine |
| `audio/audio_effects` | same, `uid://ck73wxgd3mrvp` | same message from the engine |
| `audio/rhythm_game` | same, `uid://bdd4ws8b7jdqh`; the icon on disk is `icon.webp` with a different uid | same message from the engine |
| `xr/mobile_vr_interface_demo` | `run/main_scene.mobile` is `res://main_mobile.tscn`; the file does not exist | the directory listing: the project has `main.tscn` and nothing else |
| `mobile/android_iap` | `editor_plugins/enabled` lists `res://addons/GodotGooglePlayBilling/plugin.cfg`; the addon is not in the repository | the engine prints `Addon '…/plugin.cfg' failed to load. No directory found.` and then fails to parse the scripts that use it |
| `networking/webrtc_signaling` (3.x) | `[network] modules/webrtc_gdnative_script` points at `res://demo/webrtc/webrtc.gdns`; `demo/webrtc/` is not in the repository | the directory listing |
| material-maker `brush_pattern.gdshader` | `#include "…/brush_common_decl.shader"`; the file is `brush_common_decl.gdshader` — the include kept the Godot 3 extension | the directory listing: only the `.gdshader` exists |
| material-maker `brush_stamp.gdshader` | same include, same shader directory | same |
| material-maker `brush_uv_pattern.gdshader` | same include, same shader directory | same |

## The fifteen warnings

All fifteen are `stale-import`: a `.import` file whose asset is gone.

- `godot-demo-projects@3.x`, `audio/spectrum`: `maldita.wav.import` remains while
  the audio file in the repository is `maldita.ogg`, with its own `.import`.
- `godot-demo-projects@3.x`, `misc/2.5d`: eight `.import` files under
  `assets/platform/textures` and `assets/shadow/textures` — `fortyfive`,
  `frontside`, `topdown`, `obliqueY`, `obliqueZ` — sit next to the renamed
  `forty_five.png`, `front_side.png` and friends, each of which has its own
  current `.import`.
- `godot-open-rpg`, vendored `addons/dialogic`: five `.import` files for images
  the addon no longer ships.
- `material-maker`: `material_maker/theme/new_theme_icons.png.import` without
  `new_theme_icons.png`.

None of these break a build; the engine ignores them. They are reported as
warnings because they are dead files that confuse the next person who renames
something in that folder.

## Shapes that produce nothing, and once did

Every item below produced a false finding during development. Each is now a
named regression test, and each is carried in `tests/projects/tricky` or the
unit tests.

| shape | where it came from | why it is not a defect |
| --- | --- | --- |
| `res://audio/hello_es.wav:es` | `gui/translation` | the `:es` is a locale tag in `translation_remaps`, not part of the file name |
| `res://translations/text.en.translation` | `gui/translation` | generated from `text.csv` at import time and git-ignored; the engine recreates it |
| `[locale] translations=…` | `gui/translation` | a dead Godot 3 block; Godot 4 reads `[internationalization]` and never touches it |
| `[replication] config={"uid://…": …}` | `networking/multiplayer_bomber` | editor bookkeeping in a section the engine does not resolve as a path |
| `load("res://Translations/%s.po" % lang)` | Pixelorama | the path is built at run time; the format string is not a file |
| `load("res://material_maker/theme" + name)` | material-maker | same, by concatenation |
| `instance=ExtResource( 1 )` | Godot 3 scenes throughout the 3.x branch | the spaces are the Godot 3 spelling, not an empty id |
| `res://scene.tscn::3` | sub-resources everywhere | the `::3` selects a resource inside the file |

## Reproducing

```sh
cargo build --release
python3 tools/corpus.py
```

The clones land in `target/corpus/` and are reused on the next run. Upstream
repositories move, so counts drift over time; the numbers above are a snapshot
of the date given at the top.
