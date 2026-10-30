"""开终端界面的几个场景（施工 V-7）：冷启动到首屏、空闲、热启动到首屏、三个会话同时开。"""

import json
import os
import subprocess
import time

import homes
import meter
import reap
import relay
import term


def save(where, name, data):
    where.results.mkdir(parents=True, exist_ok=True)
    (where.results / name).write_text(json.dumps(data, ensure_ascii=False, indent=2))


def open_tui(r, tag, argv=None):
    """在新 scope 里开这一家的终端界面：交回伪终端和计量。"""
    unit = meter.new_unit(tag)
    m = meter.Meter(unit)
    screen = term.Term(meter.prefix(unit, r.where.cpus) + (argv or r.argv("tui")), r.env(), str(r.where.work / "tui"))
    m.start()
    return screen, m


def first_screen(r, screen, timeout=30):
    hit = screen.wait_for(r.marker, timeout)
    if hit is None:
        raise RuntimeError(f"{r.name}：{timeout} 秒里等不到首屏：\n{screen.text()}")
    return hit - screen.started


def wakeups(before, after):
    """两次 `Meter.switches()` 之间醒了几次：总数，和照进程名分的。"""
    by_comm = {}
    for key, (comm, n) in after.items():
        if key in before:
            by_comm[comm] = by_comm.get(comm, 0) + n - before[key][1]
    return sum(by_comm.values()), by_comm


def cold(r, rivals, i, settle, idle):
    """冷启动（没有后台进程）到首屏；首屏后先等 `settle` 秒（起来以后的收尾），再空闲 `idle` 秒。

    记每秒的 CPU，看收尾要多久才平；空闲那段记 CPU、唤醒、终端收到多少字节、结束时的常驻内存。"""
    homes.restore(r, rivals)
    relay.mark(r.where, r.key, f"screen-{i}")
    load = os.getloadavg()[0]
    screen, m = open_tui(r, f"{r.key}-cold-{i}")
    shown = first_screen(r, screen)
    out = {
        "tool": r.name,
        "run": i,
        "first_screen_s": shown,
        "first_byte_s": screen.first_byte - screen.started,
        "startup_cpu_usec": m.cpu_usec()[0],
        "startup_pss": m.pss_now(),
        "settle_s": settle,
        "idle_s": idle,
        "loadavg": load,
    }
    trace = []
    for _ in range(int(settle)):
        before = m.cpu_usec()[0]
        screen.pump(1)
        trace.append(m.cpu_usec()[0] - before)
    cpu0, switches0, bytes0 = m.cpu_usec()[0], m.switches(), len(screen.raw)
    end = time.monotonic() + idle
    while time.monotonic() < end:
        before = m.cpu_usec()[0]
        screen.pump(min(1, end - time.monotonic()))
        trace.append(m.cpu_usec()[0] - before)
    woke, woke_by = wakeups(switches0, m.switches())
    out.update(
        idle_cpu_usec=m.cpu_usec()[0] - cpu0,
        idle_wakeups=woke,
        idle_wakeups_by_comm=woke_by,
        idle_pty_bytes=len(screen.raw) - bytes0,
        idle_end_pss=m.pss_now(),
        cpu_trace_usec=trace,
    )
    out["final"] = m.stop()
    screen.close()
    save(r.where, f"cold-{r.key}-{i}.json", out)
    print(
        f"[cold] {r.name} #{i}：首屏 {shown * 1000:.0f} ms，CPU {out['startup_cpu_usec'] / 1000:.0f} ms，"
        f"PSS {out['startup_pss'] / 2**20:.1f} MiB｜空闲 CPU {out['idle_cpu_usec'] / 1000:.0f} ms，"
        f"每秒醒 {woke / idle:.1f} 次，常驻 {out['idle_end_pss'] / 2**20:.1f} MiB",
        flush=True,
    )
    return out


def warm(r, rivals, i):
    """后台进程已经在跑时开一个终端界面到首屏：两个 scope 的增量相加。没有后台进程的一家不量。"""
    daemon = r.argv("daemon")
    if daemon is None:
        return None
    homes.restore(r, rivals)
    load = os.getloadavg()[0]
    unit = meter.new_unit(f"{r.key}-daemon-{i}")
    dm = meter.Meter(unit)
    proc = subprocess.Popen(
        meter.prefix(unit, r.where.cpus) + daemon,
        env=r.env(),
        cwd=str(r.where.work / "tui"),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    dm.start()
    time.sleep(3)
    d0 = dm.cpu_usec()[0]
    screen, m = open_tui(r, f"{r.key}-warm-{i}")
    shown = first_screen(r, screen)
    tui_cpu, daemon_cpu = m.cpu_usec()[0], dm.cpu_usec()[0] - d0
    out = {
        "tool": r.name,
        "run": i,
        "first_screen_s": shown,
        "startup_cpu_usec": tui_cpu + daemon_cpu,
        "tui_cpu_usec": tui_cpu,
        "daemon_cpu_usec": daemon_cpu,
        "startup_pss": m.pss_now() + dm.pss_now(),
        "loadavg": load,
    }
    out["final"], out["daemon"] = m.stop(), dm.stop()
    screen.close()
    proc.wait(timeout=5)
    save(r.where, f"warm-{r.key}-{i}.json", out)
    print(
        f"[warm] {r.name} #{i}：首屏 {shown * 1000:.0f} ms，CPU {out['startup_cpu_usec'] / 1000:.0f} ms，"
        f"PSS {out['startup_pss'] / 2**20:.1f} MiB",
        flush=True,
    )
    return out


def pump_all(opened, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        for screen, _ in opened:
            screen.pump(0.05)


def three(r, rivals, i, settle, idle):
    """同时开三个终端界面（各自一个会话），都到首屏后先等 `settle` 秒，再空闲 `idle` 秒：三个 scope 加起来。"""
    homes.restore(r, rivals)
    load = os.getloadavg()[0]
    opened = []
    for k in range(3):
        screen, m = open_tui(r, f"{r.key}-three-{i}-{k}")
        opened.append((screen, m))
        first_screen(r, screen)
    shown_cpu = sum(m.cpu_usec()[0] for _, m in opened)
    pump_all(opened, settle)
    cpu0 = sum(m.cpu_usec()[0] for _, m in opened)
    pump_all(opened, idle)
    out = {
        "tool": r.name,
        "run": i,
        "open_cpu_usec": shown_cpu,
        "idle_s": idle,
        "idle_cpu_usec": sum(m.cpu_usec()[0] for _, m in opened) - cpu0,
        "pss": sum(m.pss_now() for _, m in opened),
        "loadavg": load,
    }
    out["finals"] = [m.stop() for _, m in opened]
    out["peak_cgroup_sum"] = sum(f["cgroup_memory_peak"] for f in out["finals"])
    for screen, _ in opened:
        screen.close()
    save(r.where, f"three-{r.key}-{i}.json", out)
    print(
        f"[three] {r.name} #{i}：三个 PSS {out['pss'] / 2**20:.1f} MiB，空闲 CPU {out['idle_cpu_usec'] / 1000:.0f} ms",
        flush=True,
    )
    return out


def run(where, rivals, runs, settle, idle):
    for i in range(runs):
        for r in rivals:
            cold(r, rivals, i, settle, idle)
            warm(r, rivals, i)
            three(r, rivals, i, settle, idle / 2)
    reap.reap(homes.homes(rivals))
