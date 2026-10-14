"""三家的家目录模板（施工 V-7）：配好模型，跑一次很短的任务（留一段历史），开一次终端界面看到首屏再关。

每次量之前照模板还原，三家的起点一样：都是干净的沙箱家目录，跑过一次（`23-性能预算.md` 第四节：jcode 测别家用的是
真实的家目录，两边条件不对等，这里不照搬）。
"""

import os
import shutil
import subprocess
import time

import meter
import places
import reap
import term

WARM_UP = "Reply with the single word: ok"


def homes(rivals):
    return [r.home for r in rivals]


def restore(r, rivals, kind=""):
    """收拾残留进程，照模板 `<表名><kind>` 还原家目录。"""
    reap.reap(homes(rivals))
    if r.home.exists():
        shutil.rmtree(r.home)
    shutil.copytree(r.where.templates / f"{r.key}{kind}", r.home, symlinks=True)
    settle_disk()


def settle_disk():
    """把拷出来的先落盘：不然被测的程序一 fsync，文件系统顺手把这些脏页一起刷了，块设备的写算到它头上
    （btrfs 上实测一轮多出 17 MiB）。"""
    os.sync()


def keep(r, rivals, kind=""):
    """收拾残留进程，把现在的家目录存成模板 `<表名><kind>`。"""
    reap.reap(homes(rivals))
    dst = r.where.templates / f"{r.key}{kind}"
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(r.home, dst, symlinks=True)
    return dst


def build(r, rivals):
    """做一家的模板：配、跑一次、开一次终端界面（顺带验证首屏那几个字还对得上）。"""
    reap.reap(homes(rivals))
    if r.home.exists():
        shutil.rmtree(r.home)
    for name in ("tui", "task", "long"):
        (r.where.work / name).mkdir(parents=True, exist_ok=True)
    r.setup()
    done = subprocess.run(
        places.BARE + r.argv("task", prompt=WARM_UP),
        env=r.env(),
        cwd=str(r.where.work / "tui"),
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if done.returncode != 0:
        raise RuntimeError(f"{r.name}：试跑一轮没成（退出码 {done.returncode}）：{(done.stdout + done.stderr)[-400:]}")
    # 开一次终端界面：看到首屏后等它整个安静下来再关。起来以后后台还有活的（codex 第一次开会在后台 clone
    # 一个插件仓库），没做完就存，每次还原都从半截开始。
    unit = meter.new_unit(f"{r.key}-home")
    m = meter.Meter(unit)
    screen = term.Term(meter.prefix(unit, r.where.cpus) + r.argv("tui"), r.env(), str(r.where.work / "tui"))
    m.start()
    seen = screen.wait_for(r.marker, 30)
    if seen is None:
        m.stop()
        screen.close()
        raise RuntimeError(f"{r.name}：30 秒里没看到首屏的字 {r.marker}，rivals.toml 的 marker 要改：\n{screen.text()}")
    waited = quiet(screen, m)
    m.stop()
    screen.close()
    dst = keep(r, rivals)
    print(f"[homes] {r.name}：首屏后等了 {waited:.0f} 秒才安静，模板 {du(dst) / 1024:.0f} KiB", flush=True)


def quiet(screen, m, every=2.0, limit=300.0, eps_usec=20000, eps_file=2**20):
    """一边接着读终端，一边等它安静：`every` 秒里 CPU 用了不到 `eps_usec`、页缓存涨了不到 `eps_file`、进程没多没少。

    只看 CPU 不够：在后台下东西的主要在等网络，CPU 很低，写下来的却在涨。最多等 `limit` 秒，交回等了多久。"""
    start = time.monotonic()
    last = (m.cpu_usec()[0], m.file_bytes(), set(m.pids()))
    while time.monotonic() - start < limit:
        screen.pump(every)
        now = (m.cpu_usec()[0], m.file_bytes(), set(m.pids()))
        if now[0] - last[0] < eps_usec and abs(now[1] - last[1]) < eps_file and now[2] == last[2]:
            break
        last = now
    return time.monotonic() - start


def du(path):
    return sum(f.stat().st_blocks * 512 for f in path.rglob("*") if f.is_file() and not f.is_symlink())
