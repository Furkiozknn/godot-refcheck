# godot-refcheck

![godot-refcheck](assets/banner.svg)

Find broken resource references in a Godot project without opening the editor.

[![CI](https://github.com/Furkiozknn/godot-refcheck/actions/workflows/ci.yml/badge.svg)](https://github.com/Furkiozknn/godot-refcheck/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A scene loses a texture, a script preloads a file somebody renamed, two copied
files end up with the same `uid://`, a folder is `Art/` on one machine and
`art/` on another. Godot only tells you when that particular scene is loaded —
often in an exported build, on someone else's computer, long after the commit
that broke it.

`godot-refcheck` reads the project files directly, resolves every reference the
way the engine does, and reports the ones that cannot be satisfied. It is a
single binary with no runtime dependencies, it never launches the engine, and a
whole project is scanned in well under a second.

```
$ godot-refcheck tests/projects/broken --fail-on never
godot-refcheck 0.1.0  tests/projects/broken

main.gd:3: error: missing-resource: res://nope.tscn is not in the project
    preload/load
main.tscn:1: error: duplicate-uid: uid://cbrokensceneaa is claimed by 2 files
    res://main.tscn, res://twin.tscn
main.tscn:4: error: missing-resource: res://art/absent.png is not in the project
    ext_resource (Texture2D)
main.tscn:5: error: case-mismatch: res://Art/Present.PNG does not exist; the file on disk is res://art/present.png
    ext_resource (Texture2D) - loads on Windows and macOS, fails on Linux and in exported builds
main.tscn:17: error: undeclared-id: ExtResource("99") has no matching declaration
    the file declares no ExtResource with id 99
project.godot:7: error: unknown-uid: uid://cnobodyhasthisu matches no file in the project and no path is given
    project setting (application/config/icon)
project.godot:12: error: missing-resource: res://missing_autoload.gd is not in the project
    project setting (autoload/Gone)
twin.tscn:1: error: duplicate-uid: uid://cbrokensceneaa is claimed by 2 files
    res://main.tscn, res://twin.tscn
art/deleted.png.import:1: warning: stale-import: import metadata left over from res://art/deleted.png, which is gone
    delete the .import file, or restore the asset
moved.tres:3: warning: uid-path-mismatch: uid://cpresentaaaaaa resolves to res://art/present.png, but the reference spells the path as res://art/old_name.png
    ext_resource (Texture2D)

8 files, 8 references: 8 errors, 2 warnings, 0 notes.
```

## What it checks

| check | level | what it means |
| --- | --- | --- |
| `missing-resource` | error | a referenced `res://` file is not in the project |
| `case-mismatch` | error | the reference differs from the file on disk only by letter case, so it works on Windows and macOS and fails everywhere else |
| `unknown-uid` | error | a `uid://` reference matches no file and has no usable path to fall back on |
| `duplicate-uid` | error | two files claim the same `uid://`, so the engine may hand out the wrong one |
| `undeclared-id` | error | a scene uses `ExtResource("3")` or `SubResource("x")` that the file never declares |
| `uid-path-mismatch` | warning | the `uid://` and the path in one reference point at different files; the engine follows the uid and ignores the path you read |
| `stale-import` | warning | a `.import` file left behind by an asset that was deleted or renamed |
| `unused-asset` | note | nothing in the project appears to reference this asset (opt-in, advisory) |

References are collected from `.tscn`, `.tres`, `.escn`, `project.godot`,
`.import`, `plugin.cfg`, `.gd`/`.cs` (`preload()` and `load()` with a literal
argument) and `.gdshader` (`#include`). Godot 3 and Godot 4 project formats are
both understood.

## Install

**Download a binary** — Linux, macOS (Intel and Apple Silicon) and Windows
builds are attached to every [release](https://github.com/Furkiozknn/godot-refcheck/releases).
Unpack it and put `godot-refcheck` on your `PATH`.

**With cargo**

```sh
cargo install --git https://github.com/Furkiozknn/godot-refcheck
```

**From source** — needs a Rust toolchain and nothing else:

```sh
git clone https://github.com/Furkiozknn/godot-refcheck
cd godot-refcheck
cargo build --release      # target/release/godot-refcheck
```

## Use

```sh
godot-refcheck                     # the project in the current directory
godot-refcheck path/to/project     # a directory, or the project.godot itself
godot-refcheck . --unused          # also list assets nothing references
godot-refcheck repos --recursive   # every project.godot under a directory
godot-refcheck . --json            # machine-readable
godot-refcheck . --sarif out.sarif # for GitHub code scanning
godot-refcheck . --only duplicate-uid,case-mismatch
godot-refcheck . --skip stale-import --fail-on warning
```

Exit codes: `0` nothing at or above `--fail-on` (default `error`), `1`
something was found, `2` the project could not be read.

## In CI

The repository ships a composite action, so a workflow needs three lines:

```yaml
name: godot-refcheck
on: [push, pull_request]

jobs:
  refcheck:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Furkiozknn/godot-refcheck@v0.1.0
        with:
          path: .
```

Inputs: `path`, `recursive`, `unused`, `only`, `skip`, `fail-on`, `sarif`,
`version`. Outputs: `errors`, `warnings`, `notes`, `findings`.

With `sarif: refcheck.sarif` the findings can be uploaded to GitHub code
scanning and appear inline on the changed lines of a pull request:

```yaml
      - uses: Furkiozknn/godot-refcheck@v0.1.0
        with:
          sarif: refcheck.sarif
          fail-on: never
      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: refcheck.sarif
```

## Why the results can be trusted

A checker that cries wolf gets switched off, so every rule here had to earn its
place against real projects and against the engine itself.

**The engine is the reference.** `tools/verify_with_godot.py` runs a real
headless Godot over the fixture projects and compares what the engine prints
with what `godot-refcheck` reports. Six of the eight checks have a matching
engine message; the other two are listed as static-only, with the reason the
engine stays silent.

```
$ python3 tools/verify_with_godot.py --download
godot 4.4.1-stable as the reference implementation

  [ok] broken   missing-resource   engine=yes refcheck=yes
  [ok] broken   missing-resource   engine=yes refcheck=yes
  [ok] broken   missing-resource   engine=yes refcheck=yes
  [ok] broken   case-mismatch      engine=yes refcheck=yes
  [ok] broken   duplicate-uid      engine=yes refcheck=yes
  [ok] broken   unknown-uid        engine=yes refcheck=yes

  [static-only] stale-import       leftover import metadata is ignored rather than reported
  [static-only] uid-path-mismatch  the engine silently prefers the uid and never mentions the stale path
  [static-only] undeclared-id      the engine only reports it when that particular scene is loaded
  [static-only] unused-asset       advisory; the engine has no notion of an unused asset

  [ok] clean    healthy project: engine errors=0 refcheck findings=0
  [ok] tricky   healthy project: engine errors=0 refcheck findings=0
  [ok] legacy3  healthy project: engine errors=0 refcheck findings=0
```

**Working projects are the other reference.** `tools/corpus.py` scans eleven
real repositories — 237 projects, 19,305 files, 6,761 references. It reports 25
findings in total, every one of them checked by hand and real; nothing else in
those projects produces a finding. [docs/corpus.md](docs/corpus.md) lists each
one. Shapes that look broken and are not — locale-suffixed translation remaps,
translations generated from a `.csv` at import time, the dead `[locale]` block
Godot 3 leaves behind, `load("res://levels/%s.tscn" % name)`, sub-resource
paths, Godot 3's `ExtResource( 1 )` — are all carried in the test suite as named
regression tests, because each of them once produced a false finding here.

`cargo test` runs 73 tests, all offline.

## Limitations

- A path built at run time (`load("res://levels/" + name + ".tscn")`) cannot be
  resolved statically and is deliberately ignored rather than guessed at.
- `unused-asset` is advisory and off by default: an asset reached only through a
  computed path looks unused to any static tool. Scripts are never reported,
  because a `class_name` can be used with no `res://` reference at all.
- Project settings are only followed inside sections the engine itself defines,
  so a custom section cannot produce a false error.
- C# is read for `preload`/`load` calls only; it is not parsed as C#.

## Development

```sh
cargo test                                   # 73 tests, no network
cargo clippy --all-targets -- -D warnings
cargo fmt --check
python3 tools/verify_with_godot.py --download
python3 tools/corpus.py
```

## License

MIT — see [LICENSE](LICENSE).
