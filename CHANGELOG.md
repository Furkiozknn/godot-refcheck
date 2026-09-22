# Changelog

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
