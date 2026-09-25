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
depends on: the exit code and the declared outputs. The install step is run
too, offline: `curl` and `cargo` are stand-ins on PATH, and the release
archive is packed here the way release.yml packs it, so the unpacking and
the sha256 check are exercised without a download.

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


def _substitute(text: str, inputs: dict[str, str], binary: str) -> str:
    doc = yaml.safe_load((KOK / "action.yml").read_text(encoding="utf-8"))
    for name, spec in doc.get("inputs", {}).items():
        value = inputs.get(name, spec.get("default", ""))
        text = text.replace("${{ inputs.%s }}" % name, str(value))
    text = text.replace("${{ steps.install.outputs.binary }}", binary)
    if "${{" in text:
        raise SystemExit("action.yml'de yerine konmamis ifade kaldi: ${{%s" % text.split("${{")[1][:60])
    return text


def step(index: int, inputs: dict[str, str], binary: str) -> tuple[str, dict[str, str]]:
    """A step's run: block and env:, with inputs substituted as GitHub would."""
    doc = yaml.safe_load((KOK / "action.yml").read_text(encoding="utf-8"))
    s = doc["runs"]["steps"][index]
    run = _substitute(s["run"], inputs, binary)
    env = {k: _substitute(str(v), inputs, binary) for k, v in (s.get("env") or {}).items()}
    return run, env


def step_script(inputs: dict[str, str], binary: str) -> str:
    """The run: block of the second step, with inputs substituted."""
    return step(1, inputs, binary)[0]


def execute(index: int, inputs: dict[str, str], binary: str, extra_env: dict[str, str] | None = None,
            tmp: Path | None = None) -> tuple[int, dict[str, str], str]:
    own = tmp is None
    tmp = tmp or Path(tempfile.mkdtemp())
    try:
        run, step_env = step(index, inputs, binary)
        env = dict(os.environ)
        env.update(step_env)
        env.update(extra_env or {})
        env.setdefault("RUNNER_TEMP", str(tmp))
        env["GITHUB_OUTPUT"] = str(tmp / "out.txt")
        env["GITHUB_STEP_SUMMARY"] = str(tmp / "summary.md")
        Path(env["GITHUB_OUTPUT"]).write_text("", encoding="utf-8")
        Path(env["GITHUB_STEP_SUMMARY"]).write_text("", encoding="utf-8")
        script = tmp / "step.sh"
        script.write_text(run, encoding="utf-8")
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
        return proc.returncode, out, (proc.stdout or "") + "\n" + (proc.stderr or "") + "\n" + summary
    finally:
        if own:
            shutil.rmtree(tmp, ignore_errors=True)


def run_case(binary: str, **inputs) -> tuple[int, dict[str, str], str]:
    return execute(1, inputs, binary)


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


# --- the install step ------------------------------------------------------
#
# It downloads a release archive, checks it against the published .sha256 and
# unpacks the binary. None of that needs the network to test: `curl` and
# `cargo` are replaced on PATH by stand-ins, and the archives are packed here
# exactly the way release.yml packs them.

FAKE_CURL = """#!/usr/bin/env bash
out=""; url=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    --retry) shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
src="$FAKE_RELEASE/${url##*/}"
[ -f "$src" ] || exit 22
cp "$src" "$out"
"""

FAKE_CARGO = """#!/usr/bin/env bash
echo "fake cargo $*"
"""


def pack_release(binary: str, where: Path, version: str, target: str) -> Path:
    """An archive laid out like release.yml's `Package` step, and its .sha256."""
    import hashlib
    import tarfile
    name = "godot-refcheck-v%s-%s" % (version, target)
    archive = where / (name + ".tar.gz")
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(binary, arcname="%s/godot-refcheck" % name)
        tar.add(KOK / "LICENSE", arcname="%s/LICENSE" % name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (where / (archive.name + ".sha256")).write_text("%s  %s\n" % (digest, archive.name), encoding="utf-8")
    return archive


def install_cases(binary: str) -> list[tuple[str, str]]:
    sonuclar: list[tuple[str, str]] = []
    if not sys.platform.startswith("linux"):
        return [("kurulum adimi (yalnizca Linux'ta sinanir)", "")]
    import platform
    arch = {"x86_64": ("X64", "x86_64-unknown-linux-gnu"),
            "aarch64": ("ARM64", "aarch64-unknown-linux-gnu")}.get(platform.machine())
    if arch is None:
        return [("kurulum adimi (bu mimaride sinanmaz)", "")]
    runner_arch, target = arch
    version = "9.9.9"

    def once(tamper: bool, publish: bool) -> tuple[int, dict[str, str], str, Path]:
        tmp = Path(tempfile.mkdtemp())
        rel = tmp / "release"
        tools = tmp / "bin"
        rel.mkdir()
        tools.mkdir()
        for ad, govde in (("curl", FAKE_CURL), ("cargo", FAKE_CARGO)):
            (tools / ad).write_text(govde, encoding="utf-8")
            (tools / ad).chmod(0o755)
        if publish:
            archive = pack_release(binary, rel, version, target)
            if tamper:
                with open(archive, "ab") as fh:
                    fh.write(b"\0")
        runner = tmp / "runner"
        runner.mkdir()
        env = {
            "PATH": "%s:%s" % (tools, os.environ.get("PATH", "")),
            "FAKE_RELEASE": str(rel),
            "RUNNER_OS": "Linux",
            "RUNNER_ARCH": runner_arch,
            "RUNNER_TEMP": str(runner),
            "GITHUB_ACTION_PATH": str(KOK),
        }
        kod, cikti, gurultu = execute(0, {"version": version}, binary, env, tmp)
        return kod, cikti, gurultu, tmp

    kod, cikti, gurultu, tmp = once(tamper=False, publish=True)
    kurulan = cikti.get("binary", "")
    if kod != 0 or "building from source" in gurultu or not Path(kurulan).is_file():
        sonuclar.append(("kurulum: yayimlanmis ikili kullanilir",
                         "kod %d, binary=%r: %s" % (kod, kurulan, gurultu.strip()[-400:])))
    else:
        sonuclar.append(("kurulum: yayimlanmis ikili acilir, sha256 dogrulanir", ""))
    shutil.rmtree(tmp, ignore_errors=True)

    kod, cikti, gurultu, tmp = once(tamper=True, publish=True)
    if kod == 0 or "checksum mismatch" not in gurultu:
        sonuclar.append(("kurulum: bozuk arsiv reddedilir", "kod %d: %s" % (kod, gurultu.strip()[-400:])))
    else:
        sonuclar.append(("kurulum: sha256 tutmayan arsiv reddedilir", ""))
    shutil.rmtree(tmp, ignore_errors=True)

    kod, cikti, gurultu, tmp = once(tamper=False, publish=False)
    if "building from source" not in gurultu or "fake cargo build --release" not in gurultu:
        sonuclar.append(("kurulum: yayim yoksa kaynaktan derlenir", "kod %d: %s" % (kod, gurultu.strip()[-400:])))
    else:
        sonuclar.append(("kurulum: yayim yoksa kaynaktan derlenir", ""))
    shutil.rmtree(tmp, ignore_errors=True)
    return sonuclar


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

    ekstra = 1

    # --fix: the pass that repairs is the pass the counts come from. When both
    # passes carried --fix, the second had nothing left to repair and
    # `repaired` was always 0.
    ekstra += 1
    kopya = Path(tempfile.mkdtemp()) / "moved"
    shutil.copytree(KOK / "tests" / "projects" / "moved", kopya)
    try:
        kod, cikti, gurultu = run_case(binary, path=str(kopya), fix="true", **{"fail-on": "error"})
        if kod != 0 or cikti.get("repaired") != "3" or cikti.get("errors") != "0" \
                or "repaired 3 reference" not in gurultu:
            kalan += 1
            print("HATA fix: 3 onarim bekleniyordu: kod %d, %s" % (kod, cikti))
            print("     " + gurultu.strip().replace("\n", "\n     ")[:600])
        else:
            print("ok   %-32s cikis %d, %s" % ("fix, tasinmis proje", kod, cikti))
    finally:
        shutil.rmtree(kopya.parent, ignore_errors=True)

    # A path with a space and an `only` list written with spaces are one
    # argument each: the inputs travel through the environment.
    ekstra += 1
    kopya = Path(tempfile.mkdtemp()) / "with space"
    shutil.copytree(KOK / "tests" / "projects" / "broken", kopya)
    try:
        kod, cikti, gurultu = run_case(binary, path=str(kopya), only="missing-resource, case-mismatch",
                                       **{"fail-on": "never"})
        if kod != 0 or cikti.get("errors") != "4" or cikti.get("warnings") != "0":
            kalan += 1
            print("HATA bosluklu yol / only listesi: kod %d, %s" % (kod, cikti))
            print("     " + gurultu.strip().replace("\n", "\n     ")[:600])
        else:
            print("ok   %-32s cikis %d, %s" % ("bosluklu yol, only listesi", kod, cikti))
    finally:
        shutil.rmtree(kopya.parent, ignore_errors=True)

    # A path that is not there fails the step instead of reporting a clean
    # project.
    ekstra += 1
    kod, cikti, gurultu = run_case(binary, path="tests/projects/no-such-project")
    if kod == 0:
        kalan += 1
        print("HATA olmayan yol gecti: %s" % cikti)
    else:
        print("ok   %-32s cikis %d" % ("olmayan yol adimi dusurur", kod))

    for ad, sonuc in install_cases(binary):
        ekstra += 1
        if sonuc:
            kalan += 1
            print("HATA %-32s %s" % (ad, sonuc))
        else:
            print("ok   %s" % ad)

    print()
    print("%d vaka, %d sorun" % (len(CASES) + ekstra, kalan))
    return 1 if kalan else 0


if __name__ == "__main__":
    raise SystemExit(main())
