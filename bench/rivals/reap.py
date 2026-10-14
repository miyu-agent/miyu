"""收拾量完留下的后台进程（Miyu 的核心、jcode 的服务）：只杀环境里的家目录指着沙箱家目录的（施工 V-7）。"""

import os
import signal
import time
from pathlib import Path

# 各家认家目录的变量。
HOME_VARS = (b"MIYU_HOME=", b"HOME=", b"CODEX_HOME=")


def leftover(homes):
    """环境里的家目录在 `homes` 里的进程号。"""
    homes = {str(h) for h in homes}
    found = []
    for p in Path("/proc").iterdir():
        if not p.name.isdigit():
            continue
        try:
            env = (p / "environ").read_bytes().split(b"\0")
        except OSError:
            continue
        for item in env:
            if item.startswith(HOME_VARS) and item.split(b"=", 1)[1].decode(errors="replace") in homes:
                found.append(int(p.name))
                break
    return found


def reap(homes):
    """先 SIGTERM，5 秒还在的 SIGKILL；交回杀了哪些。"""
    pids = leftover(homes)
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline and leftover(homes):
        time.sleep(0.1)
    for pid in leftover(homes):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    return pids
