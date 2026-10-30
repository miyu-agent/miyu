"""量的条件（施工 V-7）：机器、系统、三家的版本、核心和终端的提交、模型名。端点地址、key 不记。"""

import datetime
import os
import platform
import subprocess
import tomllib
from pathlib import Path

import places


def _git(repo, *args):
    done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else None


def _rev(repo):
    """提交号，工作树有改动的后面加「+改动」。"""
    rev = _git(repo, "rev-parse", "--short=8", "HEAD")
    if rev and _git(repo, "status", "--porcelain", "--untracked-files=no"):
        rev += "+改动"
    return rev


def _cpu():
    for line in Path("/proc/cpuinfo").read_text().splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    return platform.processor()


def _memory_gb():
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            return round(int(line.split()[1]) / 2**20, 1)
    return None


def collect(where, rivals):
    """这一趟的条件；终端界面的提交顺带和 heads.toml 钉的比一比。"""
    heads = tomllib.loads((places.REPO / "heads.toml").read_text())
    pinned = heads["miyu-tui"]["rev"]
    tui_rev = _git(where.tui_repo, "rev-parse", "HEAD")
    return {
        "date": datetime.date.today().isoformat(),
        "machine": {
            "cpu": _cpu(),
            "logical_cores": os.cpu_count(),
            "memory_gb": _memory_gb(),
            "kernel": platform.release(),
            "arch": platform.machine(),
            "pinned_cpus": where.cpus,
        },
        "model": where.model,
        # 经本机转发的（施工 V-27）：只记用没用，不记上游地址。
        "relay": bool(where.relay),
        # 量之前的 1、5、15 分钟负载：别的会话在编译时，时间类的数会抖（施工 V-27）。
        "load_before": [round(x, 2) for x in os.getloadavg()],
        "core": _rev(where.core),
        "tui": _rev(where.tui_repo),
        "tui_matches_heads_toml": tui_rev == pinned
        or (pinned.startswith("v") and _git(where.tui_repo, "rev-parse", f"{pinned}^{{commit}}") == tui_rev),
        "versions": {r.name: r.version() for r in rivals},
    }
