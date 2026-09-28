# Corpus results

Rules are only worth having if they stay quiet on projects that work. These
eleven repositories are real, maintained Godot projects; `tools/corpus.py`
clones them and runs `godot-refcheck` over every `project.godot` inside.

Findings measured on 2026-09-22 with the code released as godot-refcheck 0.3.0, and
re-measured on 2026-09-25 after `.gdignore` support: the findings are identical,
the file counts are lower because a directory Godot does not scan is no longer
read. Shallow clones of the default branch unless a branch is named.

| repository | projects | files | references | findings |
| --- | ---: | ---: | ---: | ---: |
| godotengine/godot-demo-projects | 139 | 3,738 | 2,023 | 9 |
| godotengine/godot-demo-projects@3.x | 89 | 2,061 | 1,452 | 12 |
| godotengine/godot-benchmarks | 1 | 843 | 73 | 0 |
| Orama-Interactive/Pixelorama | 1 | 1,258 | 684 | 0 |
| RodZill4/material-maker | 1 | 1,883 | 997 | 24 |
| mbrlabs/Lorien | 1 | 212 | 133 | 0 |
| GDQuest/godot-open-rpg | 1 | 1,371 | 643 | 5 |
| Maaack/godot-game-template | 1 | 543 | 356 | 0 |
| Maaack/Godot-Menus-Template | 1 | 405 | 215 | 0 |
| MakovWait/godots | 1 | 4,061 | 174 | 0 |
| bitbrain/beehave | 1 | 823 | 154 | 0 |
| **total** | **237** | **17,198** | **6,904** | **50** |

50 findings: 35 errors and 15 warnings. Each one was checked by hand against
the repository it came from, and each one is a real defect. No other reference
in those 6,904 produced a finding, none of the 2,741 signal connections in these
projects was wrongly called broken, none of the 880 `class_name` declarations
collided, and `--fix-dry-run` proposes no change anywhere in the corpus.

## The sixteen dead connections

All sixteen are in `material-maker`, and Godot reports none of them: a
connection whose node cannot be resolved is dropped without a message, so the
signal simply never arrives.

| file | connection | why it cannot resolve |
| --- | --- | --- |
| `widgets/curve_edit/curve_editor.tscn` | three connections from and to `ControlPoint` | the scene inherits from `curve_view.tscn`, whose only node is `CurveView`; control points are created at run time by `curve_editor.gd` and wired there in code |
| `widgets/curve_edit/curve_dialog.tscn` | three connections to `…/CurveEditor/@Control@283287` | `@Control@…` is a name Godot generates for an unnamed node at run time; it is regenerated on every run and never matches |
| `widgets/polygon_edit/polygon_dialog.tscn` | one connection to `…/PolygonEditor/@Control@242305` | the same |
| `panels/preview_2d/preview_2d_panel.tscn` | one connection to `PolygonEditor/@Control@42512` | the same |

Each of these is reported once for `from` and once for `to`, which is how the
count reaches sixteen.

## The ten path errors

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

## The nine node paths

`missing-node-path` reads `$Head/Body` and `get_node("Head/Body")` out of a
script and resolves them against the scene the script is attached to. These
nine are all the same defect: the scene was reorganised and the script was not.

| project | finding | confirmed by |
| --- | --- | --- |
| `2d/finite_state_machine` (4.x and 3.x) | `$States/Stagger` | `player/Player.tscn` has `StateMachine/Stagger`; there is no `States` node |
| `2d/finite_state_machine` (4.x and 3.x) | `$Health` | `Player.tscn` has no `Health` node at all, and no scene in that project is rooted at one |
| `compute/heightmap` | `$CenterContainer/…/HBoxContainer2/Label2` | `main.tscn` has `HBoxContainer` under that parent, not `HBoxContainer2`; the line only runs when `RenderingDevice` is unavailable |
| material-maker `panels/paint/paint.gd` (5 lines) | `$…/Painter/Options`, `…/Options/OptionsPanel`, `…/Options/Buttons` | `paint.tscn` has `Painter/OptionsPanel` directly; there is no `Options` container between them |

Each takes a `null` from the engine and dies on the next line, on whichever
branch reaches it — which is why they survive in working projects: none of
these lines is on a path the demo normally takes.

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

## Repairs, on projects that work and on projects that do not

`--fix-dry-run` over the whole corpus proposes **no change at all**. The findings
there are unknown uids, a missing addon, dead connections and leftover import
files, and none of those has a single provable answer.

To see repairs at work, a project has to be broken first. Two experiments, both
reproducible with `tools/verify_with_godot.py --real <project>`:

| experiment | before | `--fix` | after |
| --- | --- | --- | --- |
| `2d/dodge_the_creeps` (Godot 4), `art/` and `fonts/` moved under `assets/` | 13 references now disagree with the files on disk; the engine still loads the project because every reference also carries a `uid://` | repaired 13 | no findings, and the engine still reports nothing |
| `2d/platformer` from the 3.x branch (Godot 3, no uids), `src/` moved under `assets/` | 31 references broken outright | repaired 31 | no findings |

The Godot 4 case is the interesting one: nothing was visibly broken, because the
engine quietly followed the uid. Every path written in those scene files was
wrong, and would have broken the day a `.uid` or `.import` sidecar went missing.

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
| `from="Panel/Inner"` | connections in almost every project | the node comes from an instanced scene and is written down in that file, not this one |
| `from="FromBase"` in an inherited scene | `godot-game-template` and others | the node comes from the scene this one inherits from |
| a connection under an `instance_placeholder` | deferred loading | the subtree only exists at run time |
| `method="set_h_offset"` | `godot-demo-projects` | a signal wired to a built-in method, which is why the method is not checked at all |
| `$Combatants/Player` | `2d/role_playing_game` | the child is `add_child`ed from a `PackedScene` whose root node is named `Player`; the call site takes the scene as a parameter, so no static tool can tie it to one file |
| `get_node("PreviewViewport")` | material-maker | `tools/share/preview_viewport.tscn` is rooted at `PreviewViewport` and instantiated into place two lines above |
| `if has_node("TextBubbleLayer")` | `godot-open-rpg`, vendored dialogic | the script tests for the node before using it: absence is the documented case, not a defect |

## Reproducing

```sh
cargo build --release
python3 tools/corpus.py
```

The clones land in `target/corpus/` and are reused on the next run. Upstream
repositories move, so counts drift over time; the numbers above are a snapshot
of the date given at the top.
