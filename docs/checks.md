# Checks

Each check is listed with the failure it stands for, what the engine does at
run time, and what makes it stay quiet.

## `missing-resource` (error)

A reference names a `res://` path that is not in the project and is not produced
by an importer.

Sources: `ext_resource` in `.tscn`/`.tres`, `preload()`/`load()` with a literal
argument, `#include` in a shader, `script=` in a `plugin.cfg`, and any `res://`
value of a project setting in a section the engine defines.

The engine's answer: `Resource file not found: res://…`, `Preload file "res://…"
does not exist`, or `Failed to create an autoload, can't load from path: …`.

Quiet when: the path is produced at import time (the `.translation` built from a
`.csv` is declared in the `.csv.import` and is usually git-ignored); the path is
assembled at run time; the setting sits in a section the engine does not define.

## `case-mismatch` (error)

The exact path is missing but a file differing only in letter case exists. The
project runs on Windows and macOS and fails on Linux, on exported builds with a
case-sensitive pack, and on any CI runner.

Reported separately from `missing-resource` because the fix is different: the
file is there, the spelling is not.

## `unknown-uid` (error)

A `uid://` reference matches no file, and the reference carries no path to fall
back on — a project setting such as `config/icon="uid://…"`, or a `preload` of a
uid.

The engine's answer: `ERROR: Unrecognized UID: "uid://…"`.

Quiet when: the uid is unknown but the same reference also carries a path that
exists. The engine falls back to the path and loads the resource, so nothing is
broken.

## `duplicate-uid` (error)

Two files declare the same `uid://`. This is what copying a `.tscn`, a `.tres`
or a `.gd.uid` outside the editor produces. Which file a uid reference resolves
to then depends on scan order.

The engine's answer: `WARNING: UID duplicate detected between res://a and res://b`.

Uid declarations are read from scene and resource headers, from `.import` files,
and from `.uid` sidecars.

## `undeclared-id` (error)

A scene body uses `ExtResource("3")` or `SubResource("x")` with no matching
`[ext_resource … id="3"]` or `[sub_resource … id="x"]` in the same file. Hand
edits and bad merges produce this.

The engine reports it only when that scene is loaded, which is why a static pass
is worth having.

## `uid-path-mismatch` (warning)

One reference carries both a `uid://` and a path, and they resolve to different
files. Godot follows the uid, so the project works and the path you read in the
diff is a lie. Usually the aftermath of moving a file with the editor closed.

Not raised when the uid is also reported as a duplicate; the duplicate is the
real problem there.

## `stale-import` (warning)

A `.import` file whose `source_file` is gone. Harmless to the engine, confusing
to people, and it keeps showing up in searches.

## `unused-asset` (note, opt-in)

Nothing in the project references the asset. Off unless `--unused` is given.

Deliberately conservative: scripts are never reported, because a `class_name`
can be used without any `res://` reference; anything under `addons/` is skipped;
an asset whose imported product is referenced counts as used; and a file whose
name appears in any string literal in any script is treated as reachable.

It is still advisory. An asset reached only through a computed path is invisible
to every static tool, this one included.
