#!/usr/bin/env python3
"""Check godot-refcheck's findings against the Godot engine itself.

For every fixture project the script runs the real engine in headless mode and
compares what the engine reports with what godot-refcheck reports. A check is
only allowed to exist here if the engine agrees, or if it is listed as
static-only with a reason.

    python3 tools/verify_with_godot.py --godot /path/to/godot
    python3 tools/verify_with_godot.py --download     # fetch a headless build

Exit code 0 means every case matched.
"""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROJECTS = os.path.join(ROOT, "tests", "projects")
GODOT_VERSION = "4.4.1-stable"

# (project, check id, substring the engine must print)
ENGINE_CASES = [
    ("broken", "missing-resource", "Preload file \"res://nope.tscn\" does not exist"),
    ("broken", "missing-resource", "can't load from path: res://missing_autoload.gd"),
    ("broken", "missing-resource", "res://art/absent.png"),
    ("broken", "case-mismatch", "res://Art/Present.PNG"),
    ("broken", "duplicate-uid", "UID duplicate detected"),
    ("broken", "unknown-uid", "Unrecognized UID"),
]

# Checks the engine cannot report on its own, with the reason they still matter.
STATIC_ONLY = {
    "undeclared-id": "the engine only reports it when that particular scene is loaded",
    "uid-path-mismatch": "the engine silently prefers the uid and never mentions the stale path",
    "stale-import": "leftover import metadata is ignored rather than reported",
    "unused-asset": "advisory; the engine has no notion of an unused asset",
}

CLEAN_PROJECTS = ["clean", "tricky", "legacy3"]


def download_godot(dest):
    system = platform.system()
    if system != "Linux" or platform.machine() not in ("x86_64", "AMD64"):
        sys.exit("--download only provides the linux x86_64 build; pass --godot instead")
    url = (
        "https://github.com/godotengine/godot/releases/download/"
        f"{GODOT_VERSION}/Godot_v{GODOT_VERSION}_linux.x86_64.zip"
    )
    print(f"downloading {url}")
    archive = os.path.join(dest, "godot.zip")
    urllib.request.urlretrieve(url, archive)
    with zipfile.ZipFile(archive) as z:
        z.extractall(dest)
    binary = os.path.join(dest, f"Godot_v{GODOT_VERSION}_linux.x86_64")
    os.chmod(binary, 0o755)
    return binary


def run_engine(godot, project):
    """Import the project twice and return everything the engine printed."""
    work = tempfile.mkdtemp(prefix="refcheck-oracle-")
    copy = os.path.join(work, os.path.basename(project))
    shutil.copytree(project, copy)
    output = []
    for _ in range(2):
        p = subprocess.run(
            [godot, "--headless", "--path", copy, "--import"],
            capture_output=True,
            text=True,
            timeout=600,
        )
        output.append(p.stdout + p.stderr)
    p = subprocess.run(
        [godot, "--headless", "--path", copy, "--quit"],
        capture_output=True,
        text=True,
        timeout=600,
    )
    output.append(p.stdout + p.stderr)
    shutil.rmtree(work, ignore_errors=True)
    return "\n".join(output)


def run_refcheck(binary, project):
    p = subprocess.run(
        [binary, project, "--json", "--fail-on", "never", "--unused"],
        capture_output=True,
        text=True,
        timeout=300,
    )
    if p.returncode != 0:
        sys.exit(f"godot-refcheck failed on {project}:\n{p.stderr}")
    return json.loads(p.stdout)["findings"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--godot", help="path to a Godot 4 binary")
    ap.add_argument("--download", action="store_true")
    ap.add_argument(
        "--binary",
        default=os.path.join(ROOT, "target", "release", "godot-refcheck"),
        help="path to the built godot-refcheck",
    )
    args = ap.parse_args()

    if not os.path.exists(args.binary):
        sys.exit(f"{args.binary} not found; run: cargo build --release")

    tmp = None
    godot = args.godot
    if args.download:
        tmp = tempfile.mkdtemp(prefix="godot-")
        godot = download_godot(tmp)
    if not godot:
        sys.exit("pass --godot <binary> or --download")

    failures = []
    engine_output = {}
    findings = {}
    for name in sorted(set([c[0] for c in ENGINE_CASES] + CLEAN_PROJECTS)):
        project = os.path.join(PROJECTS, name)
        engine_output[name] = run_engine(godot, project)
        findings[name] = run_refcheck(args.binary, project)

    print(f"\ngodot {GODOT_VERSION} as the reference implementation\n")
    for project, check, fragment in ENGINE_CASES:
        engine_says = fragment in engine_output[project]
        we_say = any(f["check"] == check for f in findings[project])
        ok = engine_says and we_say
        print(
            f"  [{'ok' if ok else 'FAIL'}] {project:<8} {check:<18} "
            f"engine={'yes' if engine_says else 'no':<3} refcheck={'yes' if we_say else 'no'}"
        )
        if not ok:
            failures.append(f"{project}/{check}: engine={engine_says} refcheck={we_say}")

    print()
    for check, why in sorted(STATIC_ONLY.items()):
        print(f"  [static-only] {check:<18} {why}")

    print()
    for name in CLEAN_PROJECTS:
        errors = [
            line
            for line in engine_output[name].splitlines()
            if line.startswith("ERROR:") or line.startswith("SCRIPT ERROR:")
        ]
        # A second import pass regenerates translations, so the first-pass
        # message about them is expected and is filtered out here.
        errors = [e for e in errors if ".translation" not in e]
        ours = [f for f in findings[name] if f["level"] in ("error", "warning")]
        ok = not errors and not ours
        print(
            f"  [{'ok' if ok else 'FAIL'}] {name:<8} healthy project: "
            f"engine errors={len(errors)} refcheck findings={len(ours)}"
        )
        if not ok:
            failures.append(f"{name}: engine={errors[:2]} refcheck={[f['check'] for f in ours]}")

    if tmp:
        shutil.rmtree(tmp, ignore_errors=True)

    print()
    if failures:
        for f in failures:
            print("FAIL " + f)
        return 1
    print(f"{len(ENGINE_CASES)} engine-confirmed cases, {len(CLEAN_PROJECTS)} healthy projects: all matched.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
