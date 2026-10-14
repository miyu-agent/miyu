"""打开一个长会话（施工 V-7）：先照同样的 50 轮话把三家各自的长会话造出来（`gen`），再量打开它（`open`）。"""

import json
import subprocess
import time

import homes
import meter
import places
import reap
import term

TURNS = 50
END = "LONGEND"


def filler(k):
    lines = [
        f"// 第 {k} 段资料 第 {j} 行：fn step_{k}_{j}(x: i64) -> i64 {{ x * {k} + {j} }} 用来压测长会话的填充文字。"
        for j in range(24)
    ]
    return "\n".join(lines)


def message(k):
    if k == TURNS:
        return f"资料都给完了。这一次只回复 {END} 这几个字母，别的什么都不要说。"
    return f"第 {k} 段资料，先记着，不用分析也不要调用任何工具，只回复「收到 {k}」：\n{filler(k)}"


def gen(r, rivals):
    """造长会话：第一轮开新会话，后面照 `cont` 接着说；造完存成模板 `<表名>-long`，会话编号记在模板里。"""
    homes.restore(r, rivals)
    session = None
    started = time.monotonic()
    for k in range(1, TURNS + 1):
        argv = r.argv("task" if k == 1 else "cont", prompt=message(k), session=session or "")
        done = subprocess.run(
            places.BARE + argv,
            env=r.env(),
            cwd=str(r.where.work / "long"),
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=300,
        )
        if done.returncode != 0:
            raise RuntimeError(f"{r.name} 第 {k} 轮没成：{(done.stdout + done.stderr)[-400:]}")
        if k == 1:
            session = r.session_id()
        if k % 10 == 0:
            print(f"[gen] {r.name} {k}/{TURNS}，{time.monotonic() - started:.0f} s", flush=True)
    dst = homes.keep(r, rivals, "-long")
    (dst / ".rivals-session").write_text(session)
    print(f"[gen] {r.name} 造好了", flush=True)


def session_of(r):
    return (r.where.templates / f"{r.key}-long" / ".rivals-session").read_text().strip()


def open_once(r, rivals, i):
    """照模板还原长会话，开终端界面接着它，等最后一句出现在屏幕上。"""
    homes.restore(r, rivals, "-long")
    unit = meter.new_unit(f"{r.key}-long-{i}")
    m = meter.Meter(unit)
    screen = term.Term(
        meter.prefix(unit, r.where.cpus) + r.argv("resume", session=session_of(r)), r.env(), str(r.where.work / "long")
    )
    m.start()
    hit = screen.wait_for([END], 60)
    if hit is None:
        raise RuntimeError(f"{r.name}：长会话打不开，或者看不到最后一句：\n{screen.text()}")
    out = {
        "tool": r.name,
        "run": i,
        "open_s": hit - screen.started,
        "open_cpu_usec": m.cpu_usec()[0],
        "open_pss": m.pss_now(),
    }
    screen.pump(3)
    out["final"] = m.stop()
    screen.close()
    (r.where.results / f"long-{r.key}-{i}.json").write_text(json.dumps(out, ensure_ascii=False, indent=2))
    print(
        f"[long] {r.name} #{i}：打开 {out['open_s'] * 1000:.0f} ms，CPU {out['open_cpu_usec'] / 1000:.0f} ms，"
        f"PSS {out['open_pss'] / 2**20:.1f} MiB",
        flush=True,
    )


def run(where, rivals, mode, runs):
    where.results.mkdir(parents=True, exist_ok=True)
    for i in range(runs if mode == "open" else 1):
        for r in rivals:
            if mode == "gen":
                gen(r, rivals)
            else:
                open_once(r, rivals, i)
    reap.reap(homes.homes(rivals))
