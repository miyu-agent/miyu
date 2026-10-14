"""量一个 systemd 的 scope（施工 V-7）：被测的程序放进它自己的 cgroup 跑，旁边采样。

- CPU 照 cgroup 的 cpu.stat：退出了的子进程也算在内。
- 内存：cgroup 的 memory.peak，加上进程树 PSS 之和（每 0.5 秒）。
- 读写：每个进程 /proc/<pid>/io 的最大值相加（每 0.05 秒；很短命的子进程会漏）。
- 唤醒：每个线程的上下文切换（主动加被动）之和，只在要的时候数。
"""

import os
import threading
import time
import uuid
from pathlib import Path

import places


def root():
    """用户服务下放 scope 的 cgroup 目录。"""
    uid = os.getuid()
    path = Path(f"/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service/app.slice")
    if not path.is_dir():
        places.fail(f"没有 {path}：要 cgroup v2 和 systemd 的用户会话")
    return path


def prefix(unit, cpus):
    """把命令放进新 scope 的前缀；`cpus` 给了的再绑到这几个核上。

    XDG_RUNTIME_DIR 留给 systemd-run 用，进了 scope 再去掉（`places.BARE`）。"""
    argv = ["systemd-run", "--user", "--scope", "-q", f"--unit={unit}", "-p", "MemoryAccounting=yes", "--"]
    if cpus:
        argv += ["taskset", "-c", cpus]
    return argv + ["env", "-u", "XDG_RUNTIME_DIR"]


def new_unit(tag):
    return f"rivals-{tag}-{uuid.uuid4().hex[:8]}"


def _read(path):
    try:
        return Path(path).read_text()
    except OSError:
        return ""


def _kv(text):
    out = {}
    for line in text.splitlines():
        parts = line.replace(":", " ").split()
        if len(parts) >= 2:
            try:
                out[parts[0]] = int(parts[1])
            except ValueError:
                pass
    return out


class Meter:
    """量一个 scope。`start()` 以后开始采样；`cpu_usec()`、`pss_now()`、`switches()` 随时读；`stop()` 读最后的数、杀掉
    scope 里剩下的进程。"""

    def __init__(self, unit):
        self.unit = unit
        self.dir = root() / f"{unit}.scope"
        self.per_pid = {}
        self.peak_rss = 0
        self.peak_pss = 0
        self.last_pss = 0
        self.samples = 0
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)

    def wait_exists(self, timeout=5):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if (self.dir / "cgroup.procs").exists():
                return True
            time.sleep(0.005)
        raise RuntimeError(f"scope {self.unit} 没起来")

    def start(self):
        self.wait_exists()
        self._thread.start()

    def pids(self):
        return [int(p) for p in _read(self.dir / "cgroup.procs").split()]

    def cpu_usec(self):
        stat = _kv(_read(self.dir / "cpu.stat"))
        return stat.get("usage_usec", 0), stat.get("user_usec", 0), stat.get("system_usec", 0)

    def file_bytes(self):
        """scope 现在占的页缓存（memory.stat 的 file）：在写盘、在下东西的时候涨。"""
        return _kv(_read(self.dir / "memory.stat")).get("file", 0)

    def pss_now(self):
        return sum(_kv(_read(f"/proc/{pid}/smaps_rollup")).get("Pss", 0) * 1024 for pid in self.pids())

    def switches(self):
        """{(进程号, 线程号): (进程名, 切换次数)}：两次相减就是这段时间里每个线程醒了几次。"""
        out = {}
        for pid in self.pids():
            comm = _read(f"/proc/{pid}/comm").strip()
            for task in Path(f"/proc/{pid}/task").glob("*"):
                stat = _kv(_read(task / "status"))
                n = stat.get("voluntary_ctxt_switches", 0) + stat.get("nonvoluntary_ctxt_switches", 0)
                out[(pid, task.name)] = (comm, n)
        return out

    def _sample(self, with_pss):
        rss = pss = 0
        for pid in self.pids():
            base = f"/proc/{pid}"
            comm = _read(f"{base}/comm").strip()
            statm = _read(f"{base}/statm").split()
            if len(statm) > 1:
                rss += int(statm[1]) * 4096
            io = _kv(_read(f"{base}/io"))
            stat = _read(f"{base}/stat")
            ticks = 0
            if stat:
                fields = stat.rsplit(")", 1)[-1].split()
                ticks = int(fields[11]) + int(fields[12])
            entry = self.per_pid.setdefault(pid, {"comm": comm, "io": {}, "ticks": 0})
            if comm:
                entry["comm"] = comm
            for k in ("rchar", "wchar", "read_bytes", "write_bytes"):
                if k in io:
                    entry["io"][k] = max(entry["io"].get(k, 0), io[k])
            entry["ticks"] = max(entry["ticks"], ticks)
            if with_pss:
                pss += _kv(_read(f"{base}/smaps_rollup")).get("Pss", 0) * 1024
        self.peak_rss = max(self.peak_rss, rss)
        if with_pss:
            self.peak_pss = max(self.peak_pss, pss)
            self.last_pss = pss
        self.samples += 1

    def _run(self):
        n = 0
        while not self._stop.is_set():
            self._sample(with_pss=(n % 10 == 0))
            n += 1
            self._stop.wait(0.05)

    def quiet(self, settle=1.0, limit=30.0, eps_usec=20000):
        """等 scope 里的 CPU 停下来（`settle` 秒内用了不到 `eps_usec`），最多等 `limit` 秒；交回等了多久。"""
        start = time.monotonic()
        last = self.cpu_usec()[0]
        while time.monotonic() - start < limit:
            time.sleep(settle)
            now = self.cpu_usec()[0]
            if now - last < eps_usec:
                break
            last = now
        return time.monotonic() - start

    def stop(self):
        self._sample(with_pss=True)
        self._stop.set()
        self._thread.join(timeout=2)
        usage, user, system = self.cpu_usec()
        peak = int(_read(self.dir / "memory.peak") or 0)
        mstat = _kv(_read(self.dir / "memory.stat"))
        io = {"rchar": 0, "wchar": 0, "read_bytes": 0, "write_bytes": 0}
        by_comm = {}
        for entry in self.per_pid.values():
            for k in io:
                io[k] += entry["io"].get(k, 0)
            by_comm[entry["comm"]] = by_comm.get(entry["comm"], 0) + entry["ticks"] * 10000
        result = {
            "cpu_usec": usage,
            "cpu_user_usec": user,
            "cpu_system_usec": system,
            "cpu_by_comm_usec": by_comm,
            "cgroup_memory_peak": peak,
            "anon_now": mstat.get("anon", 0),
            "file_now": mstat.get("file", 0),
            "peak_rss_sum": self.peak_rss,
            "peak_pss_sum": self.peak_pss,
            "last_pss_sum": self.last_pss,
            "io": io,
            "samples": self.samples,
            "procs": sorted({e["comm"] for e in self.per_pid.values()}),
        }
        if (self.dir / "cgroup.kill").exists():
            (self.dir / "cgroup.kill").write_text("1")
        end = time.monotonic() + 5
        while self.dir.exists() and time.monotonic() < end:
            time.sleep(0.05)
        return result
