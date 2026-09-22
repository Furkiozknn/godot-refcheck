# Changelog

## 0.2.0

From a checker into something that also repairs, and two checks for breakage the
engine never mentions.

- `broken-connection`: a signal connected from or to a node the scene does not
  contain. Godot drops such a connection in silence, so the callback stops
  arriving with nothing in the log. Node paths are resolved through instanced
  sub-scenes, inherited scenes and nested instances; an `instance_placeholder`
  subtree is never judged. Found 16 real dead connections in material-maker and
  none in the other ten corpus repositories.
- `duplicate-class-name`: two scripts declaring the same global `class_name`,
  which the engine reports as `Class "X" hides a global script class`.
- `extends "res://…"` and `@icon("res://…")` are now references like any other.
- `--fix` repairs, in place, every reference the project itself settles: a file
  found under a different spelling, a `uid://` that already resolves elsewhere,
  or a name carried by exactly one file. `--fix-dry-run` shows the same list and
  changes nothing. Nothing is guessed and nothing is deleted.
- `--baseline` and `--write-baseline` let a project with existing findings turn
  the gate on today and fail only on new ones.
- The Action gained `fix`, `fix-dry-run` and `baseline` inputs and a `repaired`
  output.
- `tools/verify_with_godot.py` now also checks a case the engine accepts in
  silence, and runs two repair round trips — a fixture and, with `--real`, any
  real project — breaking them by moving folders, repairing them with `--fix`
  and handing them back to the engine.
- 73 → 107 tests, all offline.

## 0.1.0

First release.

- Eight checks: `missing-resource`, `case-mismatch`, `unknown-uid`,
  `duplicate-uid`, `undeclared-id`, `uid-path-mismatch`, `stale-import` and the
  opt-in `unused-asset`.
- Reads `.tscn`, `.tres`, `.escn`, `project.godot`, `.import`, `plugin.cfg`,
  `.gd`, `.cs`, `.gdshader` and `.gdshaderinc`; Godot 3 and Godot 4 formats.
- Text, JSON and SARIF 2.1.0 output; `--recursive` for a directory of projects.
- Composite GitHub Action with `errors`/`warnings`/`notes`/`findings` outputs.
- `tools/verify_with_godot.py` compares the findings with a headless Godot.
- `tools/corpus.py` scans eleven real repositories for calibration.
- 73 tests, all offline.
