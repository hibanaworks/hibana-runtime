#!/usr/bin/env python3
"""Run one validation job; stop its whole process tree at the 3 GB RSS limit."""

import fcntl
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

LIMIT_BYTES = 3_000_000_000


def snapshot():
    output = subprocess.check_output(
        ["ps", "-axo", "pid=,ppid=,pgid=,rss="], text=True
    )
    return [tuple(map(int, row.split())) for row in output.splitlines()]


def main():
    if len(sys.argv) < 4 or sys.argv[2] != "--":
        sys.exit("usage: resource_guard.py REPORT.json -- COMMAND [ARG...]")
    report = Path(sys.argv[1])
    report.parent.mkdir(parents=True, exist_ok=True)
    lock_path = Path(tempfile.gettempdir()) / "hibana-heavy-validation.lock"
    with lock_path.open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        start = time.monotonic()
        child = subprocess.Popen(sys.argv[3:], start_new_session=True)
        observed = {child.pid}
        peak = 0
        exceeded = False
        try:
            while True:
                processes = snapshot()
                # Include descendants even if they start their own process group.
                changed = True
                while changed:
                    changed = False
                    for pid, parent, group, _ in processes:
                        if (parent in observed or group == child.pid) and pid not in observed:
                            observed.add(pid)
                            changed = True
                live = [(pid, rss) for pid, _, _, rss in processes if pid in observed]
                total = sum(rss * 1024 for _, rss in live)
                peak = max(peak, total)
                if total >= LIMIT_BYTES:
                    exceeded = True
                    for pid, _ in live:
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                code = child.poll()
                if code is not None and not live:
                    break
                if code is not None and len(live) == 1 and live[0][0] == child.pid:
                    break
                time.sleep(0.1)
        except BaseException:
            for pid in observed:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            child.wait()
            raise
        result = {
            "command": sys.argv[3:],
            "limit_bytes": LIMIT_BYTES,
            "peak_rss_bytes": peak,
            "elapsed_seconds": round(time.monotonic() - start, 3),
            "resource_limit_exceeded": exceeded,
            "exit_code": code,
            "verified": code == 0 and not exceeded,
        }
        report.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result), flush=True)
        return 125 if exceeded else (code if code >= 0 else 128 - code)


if __name__ == "__main__":
    sys.exit(main())
