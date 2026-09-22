#!/usr/bin/env python3
"""Run action.yml's own shell the way a GitHub runner would, and check it.

The action shipped a bug that no test could have caught, because nothing
tested the action: on a project with **no findings** the counts were scraped
out of prose that a clean run never prints, the grep matched nothing, and
under `bash -e` the whole step exited 1. The action failed precisely on the
projects it had nothing to say about - and a green tool with a red step is
the worst of both.

So this runs the real thing. It reads `action.yml`, substitutes the inputs
the way GitHub does, and executes the step with the same shell the runner
uses (`bash --noprofile --norc -e -o pipefail`), against the fixture
projects already in this repository. Then it checks what a caller actually
depends on: the exit code and the declared outputs.

    python3 tools/action_smoke.py [path-to-binary]

Exit code 0 if every case behaves. No dependencies beyond PyYAML, which the
CI job installs; without it the script says so and exits 2 rather than
guessing at the YAML.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

KOK = Path(__file__).resolve().parent.parent

try:
    import yaml
except ImportError:  # pragma: no cover - the CI job installs it
    print("PyYAML gerekli: pip install pyyaml", file=sys.stderr)
    raise SystemExit(2)


def step_script(inputs: dict[str, str], binary: str) -> str:
    """The run: block of the second step, with inputs substituted."""
    doc = yaml.safe_load((KOK / "action.yml").read_text(encoding="utf-8"))
    steps = doc["runs"]["steps"]
    run = steps[1]["run"]
    declared = doc.get("inputs", {})
    for name, spec in declared.items():
        value = inputs.get(name, spec.get("default", ""))
        run = run.replace("${{ inputs.%s }}" % name, str(value))
    run = run.replace("${{ steps.install.outputs.binary }}", binary)
    left = [f for f in run.split("${{")[1:]]
    if left:
        raise SystemExit("action.yml'de yerine konmamis ifade kaldi: ${{%s" % left[0][:60])
    return run


def run_case(binary: str, **inputs) -> tuple[int, dict[str, str], str]:
    tmp = Path(tempfile.mkdtemp())
    try:
        env = dict(os.environ)
        env["RUNNER_TEMP"] = str(tmp)
        env["GITHUB_OUTPUT"] = str(tmp / "out.txt")
        env["GITHUB_STEP_SUMMARY"] = str(tmp / "summary.md")
        Path(env["GITHUB_OUTPUT"]).touch()
        Path(env["GITHUB_STEP_SUMMARY"]).touch()
        script = tmp / "step.sh"
        script.write_text(step_script(inputs, binary), encoding="utf-8")
        proc = subprocess.run(
            ["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", str(script)],
            cwd=str(KOK), env=env, capture_output=True, text=True, timeout=300,
        )
        out: dict[str, str] = {}
        for line in Path(env["GITHUB_OUTPUT"]).read_text(encoding="utf-8").splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                out[k] = v
        summary = Path(env["GITHUB_STEP_SUMMARY"]).read_text(encoding="utf-8")
        return proc.returncode, out, (proc.stderr or "") + "\n" + summary
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


CASES = [
    # A clean project with the default fail-on. This is the case the action
    # used to fail, and the reason this file exists.
    ("temiz proje, fail-on error", dict(path="tests/projects/clean", **{"fail-on": "error"}),
     0, {"errors": "0", "warnings": "0", "notes": "0", "findings": "0"}),
    ("temiz proje, fail-on warning", dict(path="tests/projects/clean", **{"fail-on": "warning"}),
     0, {"errors": "0", "findings": "0"}),
    ("temiz proje, fail-on info", dict(path="tests/projects/clean", **{"fail-on": "info"}),
     0, {"findings": "0"}),
    ("bozuk proje, fail-on error", dict(path="tests/projects/broken", **{"fail-on": "error"}),
     1, {}),
    ("bozuk proje, fail-on never", dict(path="tests/projects/broken", **{"fail-on": "never"}),
     0, {}),
]


def main() -> int:
    binary = sys.argv[1] if len(sys.argv) > 1 else str(KOK / "target" / "release" / "godot-refcheck")
    if not Path(binary).is_file():
        print("ikili yok: %s (once `cargo build --release`)" % binary, file=sys.stderr)
        return 2

    kalan = 0
    for ad, inputs, beklenen_kod, beklenen_cikti in CASES:
        kod, cikti, gurultu = run_case(binary, **inputs)
        sorun = []
        if kod != beklenen_kod:
            sorun.append("cikis kodu %d, beklenen %d" % (kod, beklenen_kod))
        for k, v in beklenen_cikti.items():
            if cikti.get(k) != v:
                sorun.append("%s=%r, beklenen %r" % (k, cikti.get(k), v))
        # Her kosuda sayaclarin tamami bildirilmeli; eksik bir cikti,
        # cagiranin `steps.x.outputs.errors` okumasini sessizce bozar.
        for k in ("errors", "warnings", "notes", "findings", "repaired"):
            if k not in cikti:
                sorun.append("cikti eksik: %s" % k)
        if sorun:
            kalan += 1
            print("HATA %-32s %s" % (ad, "; ".join(sorun)))
            print("     " + gurultu.strip().replace("\n", "\n     ")[:600])
        else:
            print("ok   %-32s cikis %d, %s" % (ad, kod, cikti))

    # Bozuk projede sayilar gercekten dolu mu -- "hepsi sifir" ile gecmesin.
    kod, cikti, _ = run_case(binary, path="tests/projects/broken", **{"fail-on": "never"})
    if int(cikti.get("errors", "0")) <= 0:
        kalan += 1
        print("HATA bozuk projede hata sayisi 0 goruluyor; sayac okunmuyor")
    else:
        print("ok   bozuk projede sayaclar dolu       %s" % cikti)

    print()
    print("%d vaka, %d sorun" % (len(CASES) + 1, kalan))
    return 1 if kalan else 0


if __name__ == "__main__":
    raise SystemExit(main())
