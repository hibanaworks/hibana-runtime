#!/usr/bin/env python3
import subprocess

for command in (
    ["rustc", "+stable", "--version"],
    ["rustc", "+nightly-2026-10-12", "--version"],
    ["lean", "+leanprover/lean4:v4.34.1", "--version"],
    ["z3", "--version"],
):
    subprocess.run(command, check=True)
