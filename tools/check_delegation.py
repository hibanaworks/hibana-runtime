#!/usr/bin/env python3
"""Check real owned Endpoints in an isolated copy; never modify the live core.

Invoke under resource_guard.py. The core's public owned-projection guard remains
enabled: this is internal integration evidence, not public delegation completion.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("core", type=Path)
    parser.add_argument("--miri", action="store_true")
    args = parser.parse_args()
    core = args.core.resolve()
    report = ROOT / "validation/delegation"
    report.mkdir(parents=True, exist_ok=True)
    if not args.miri:
        paths = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=core, text=True
        ).splitlines()
        sources = {name: (core / name).read_bytes() for name in paths if (core / name).is_file()}
        fingerprints = {name: hashlib.sha256(data).hexdigest() for name, data in sources.items()}
        for name in sources:
            if hashlib.sha256((core / name).read_bytes()).hexdigest() != fingerprints[name]:
                raise SystemExit("Core changed during snapshot; retry.")
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=core, text=True).strip()
        digest = hashlib.sha256(json.dumps(fingerprints, sort_keys=True).encode()).hexdigest()
        checkout = report / f"source-{digest[:12]}"
        if checkout.exists():
            shutil.rmtree(checkout)
        checkout.mkdir()
        for name, data in sources.items():
            dest = checkout / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
        manifest = checkout / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace(
            "[dev-dependencies]", "[dev-dependencies]\nhibana-runtime = { path = " + json.dumps(str(ROOT)) + " }"
        ))
        tests = checkout / "src/g/role_projection/tests.rs"
        tests.write_text(tests.read_text() + "\nmod runtime_localsides;\n")
        shutil.copyfile(ROOT / "fixtures/owned_views.rs", checkout / "src/g/role_projection/tests/runtime_localsides.rs")
        snapshot = {"source_path": str(core), "revision": revision, "fingerprints": fingerprints,
                    "checkout": str(checkout), "target": str(ROOT / f"target/delegation-{digest[:12]}"),
                    "public_owned_projection": "guard remains enabled; internal image test only"}
        (report / "snapshot.json").write_text(json.dumps(snapshot, indent=2) + "\n")
    snapshot = json.loads((report / "snapshot.json").read_text())
    env = dict(os.environ, CARGO_TARGET_DIR=snapshot["target"], CARGO_INCREMENTAL="0",
               CARGO_PROFILE_TEST_DEBUG="0", RUSTFLAGS="--cfg hibana_repo_tests")
    command = ["cargo", "+nightly-2026-10-12", "miri", "test"] if args.miri else ["cargo", "+stable", "test"]
    command += ["--lib", "-j", "1", "g::role_projection::tests::runtime_localsides", "--", "--test-threads=1"]
    print(json.dumps({key: snapshot[key] for key in ("source_path", "revision", "checkout", "public_owned_projection")}), flush=True)
    subprocess.run(command, cwd=snapshot["checkout"], env=env, check=True)


if __name__ == "__main__":
    main()
