"""收拾量完留下的后台进程（Miyu 的核心、jcode 的服务）：只杀环境里的家目录指着沙箱家目录的（施工 V-7）。"""

import os
import signal
import subprocess
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


def shut(homes, stop=None, env=None, limit=30.0):
    """好好退出（施工 V-27，2026-10-31 项目主人定家目录两种口径都量）：先跑这一家自己正常退出的命令（`rivals.toml` 的
    `stop`，例如 `jcode server stop`；没写的不跑），再给剩下的进程发 SIGTERM，最多等 `limit` 秒让它们收尾（Miyu 的核心
    这时把 SQLite 的 WAL 合回去、删掉）；还不走的才 SIGKILL。交回 {"seconds": 用了多久, "forced": 最后硬杀了几个}。"""
    started = time.monotonic()
    if stop:
        try:
            subprocess.run(stop, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=limit)
        except subprocess.TimeoutExpired:
            pass  # 自己的退出命令卡住了：照样往下发 SIGTERM，最后硬杀的记在 forced 里。
    for pid in leftover(homes):
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline and leftover(homes):
        time.sleep(0.1)
    left = leftover(homes)
    for pid in left:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    return {"seconds": time.monotonic() - started, "forced": len(left)}
