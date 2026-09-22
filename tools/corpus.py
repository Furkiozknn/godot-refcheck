#!/usr/bin/env python3
"""Run godot-refcheck over a list of real Godot repositories.

The point of the corpus is calibration: these projects are known to work, so
anything reported here has to be a defect that can be confirmed by hand. A
result that cannot be confirmed is a bug in godot-refcheck, not in the project.

    python3 tools/corpus.py                 # clone (shallow) and scan
    python3 tools/corpus.py --cache /tmp/x  # keep the clones somewhere else
"""

import argparse
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def repos(path):
    out = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            out.append((parts[0], parts[1] if len(parts) > 1 else None))
    return out


def clone(slug, branch, cache):
    name = slug.split("/")[1] + ("-" + branch if branch else "")
    dest = os.path.join(cache, name)
    if os.path.isdir(dest):
        return dest
    cmd = ["git", "clone", "--depth", "1", "-q"]
    if branch:
        cmd += ["-b", branch]
    cmd += [f"https://github.com/{slug}.git", dest]
    subprocess.run(cmd, check=True)
    return dest


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=os.path.join(ROOT, "target", "corpus"))
    ap.add_argument("--list", default=os.path.join(ROOT, "tools", "corpus.txt"))
    ap.add_argument("--binary", default=os.path.join(ROOT, "target", "release", "godot-refcheck"))
    args = ap.parse_args()

    if not os.path.exists(args.binary):
        sys.exit(f"{args.binary} not found; run: cargo build --release")
    os.makedirs(args.cache, exist_ok=True)

    projects = files = refs = 0
    findings = []
    print(f"{'repository':<34} {'projects':>8} {'files':>7} {'refs':>6} {'findings':>9}")
    for slug, branch in repos(args.list):
        path = clone(slug, branch, args.cache)
        p = subprocess.run(
            [args.binary, path, "--recursive", "--fail-on", "never", "--json"],
            capture_output=True,
            text=True,
        )
        if p.returncode != 0:
            print(f"{slug:<34} {p.stderr.strip()}")
            continue
        data = json.loads(p.stdout)
        n = subprocess.run(
            [args.binary, path, "--recursive", "--fail-on", "never", "--quiet"],
            capture_output=True,
            text=True,
        ).stdout
        count = int(n.split()[0]) if "projects" in n else 1
        label = slug + (f"@{branch}" if branch else "")
        print(
            f"{label:<34} {count:>8} {data['files_scanned']:>7} "
            f"{data['references']:>6} {len(data['findings']):>9}"
        )
        projects += count
        files += data["files_scanned"]
        refs += data["references"]
        for f in data["findings"]:
            f["repo"] = label
            findings.append(f)

    print(f"\n{'TOTAL':<34} {projects:>8} {files:>7} {refs:>6} {len(findings):>9}\n")
    for f in findings:
        print(f"  {f['level']:<7} {f['check']:<17} {f['repo']}  {f['located']}:{f['line']}")
        print(f"          {f['message']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
