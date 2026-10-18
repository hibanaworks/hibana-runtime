#!/usr/bin/env python3
"""Sequential, memory-bounded validation of the exact current source tree."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
REPORTS = ROOT / "validation"
MIRI_TOOLCHAIN = "nightly-2026-10-12"
LEAN_TOOLCHAIN = "leanprover/lean4:v4.34.1"


def run(name, *command, expected=None):
    with (REPORTS / f"{name}.log").open("w") as log:
        process = subprocess.run(
            [sys.executable, "tools/resource_guard.py", f"validation/{name}.json", "--", *command],
            cwd=ROOT,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
    output = (REPORTS / f"{name}.log").read_text()
    print(output, end="", flush=True)
    if process.returncode:
        sys.exit(process.returncode)
    if expected is not None:
        answers = [line for line in output.splitlines() if line in ("sat", "unsat", "unknown")]
        if answers != expected:
            sys.exit(f"{name}: unexpected proof answers: {answers}")


def main():
    os.chdir(ROOT)
    os.environ["CARGO_INCREMENTAL"] = "0"
    REPORTS.mkdir(exist_ok=True)
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"], text=True
    ).splitlines()
    fingerprints = {name: hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in files}
    (REPORTS / "source-before.json").write_text(json.dumps(fingerprints, indent=2) + "\n")

    run("versions", sys.executable, "tools/versions.py")
    run("format", "cargo", "+stable", "fmt", "--all", "--", "--check")
    run("clippy", "cargo", "+stable", "clippy", "--all-targets", "--workspace", "--locked", "-j", "1", "--", "-D", "warnings")
    run("host-tests", "cargo", "+stable", "test", "--all-targets", "--workspace", "--locked", "-j", "1")
    run("doc-tests", "cargo", "+stable", "test", "--doc", "--locked", "-j", "1")
    run("ui", sys.executable, "tools/check_ui.py")
    run("localsides", "cargo", "+stable", "run", "--example", "localsides", "--locked", "-j", "1")
    run("images", "cargo", "+stable", "run", "--example", "images", "--locked", "-j", "1")
    run("registration-pressure", "cargo", "+stable", "run", "--example", "registration_pressure", "--locked", "-j", "1")
    run("thumbv6m", "cargo", "+stable", "check", "--lib", "--target", "thumbv6m-none-eabi", "--no-default-features", "--locked", "-j", "1")
    run("pico-link", "cargo", "+stable", "rustc", "--example", "pico", "--release", "--target", "thumbv6m-none-eabi", "--locked", "-j", "1", "--", "-C", "panic=abort", "-C", "link-arg=-Tpico/link.x")
    run("lean", "lean", f"+{LEAN_TOOLCHAIN}", "proofs/Scheduler.lean")
    run("z3", "z3", "proofs/scheduler.smt2", expected=["sat", "unsat"] * 8)
    run("miri", "cargo", f"+{MIRI_TOOLCHAIN}", "miri", "test", "--test", "runtime", "--test", "localside", "--test", "no_alloc", "--test", "placement", "--locked", "-j", "1")
    run("miri-localsides", "cargo", f"+{MIRI_TOOLCHAIN}", "miri", "run", "--example", "localsides", "--locked", "-j", "1")
    core = os.environ.get("HIBANA_DELEGATION_CORE")
    if core:
        run("delegation-host", sys.executable, "tools/check_delegation.py", core)
        run("delegation-miri", sys.executable, "tools/check_delegation.py", core, "--miri")

    after = {name: hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in files}
    if fingerprints != after:
        sys.exit("Sources changed during validation; run again on the final tree.")
    (REPORTS / "source-after.json").write_text(json.dumps(after, indent=2) + "\n")
    print("All checks passed; source fingerprints match.")


if __name__ == "__main__":
    main()
