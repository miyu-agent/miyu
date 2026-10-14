"""一轮真任务（施工 V-7）：不开终端界面，各家的一次性命令修好一个小 Python 项目的 bug，量整轮的 CPU、内存、读写。"""

import json
import shutil
import subprocess
import time
from pathlib import Path

import homes
import meter
import reap

FIXTURE = Path(__file__).with_name("fixture") / "calc"
PROMPT = (
    "这个目录里有个小 Python 项目：calc.py 和 test_calc.py。先运行 python3 test_calc.py 看看哪里失败，"
    "再读 calc.py 修好 bug，然后再运行一次测试确认全部通过。最后用一句话说明你改了什么。"
)
# 一轮最多等多久。
LIMIT_S = 900


def size(path):
    total = files = 0
    for f in Path(path).rglob("*"):
        if f.is_file() and not f.is_symlink():
            total += f.stat().st_size
            files += 1
    return total, files


def one(r, rivals, i):
    homes.restore(r, rivals)
    where = r.where
    work = where.work / "task"
    if work.exists():
        shutil.rmtree(work)
    shutil.copytree(FIXTURE, work)
    homes.settle_disk()
    before = size(r.home)
    unit = meter.new_unit(f"{r.key}-task-{i}")
    m = meter.Meter(unit)
    where.logs.mkdir(parents=True, exist_ok=True)
    log = where.logs / f"task-{r.key}-{i}.out"
    # 命令跑完以后 scope 里留一个 sleep 占着：cgroup 不消失，读得到这一轮的 CPU；读完由 meter 收掉。
    code_file = where.logs / f"task-{r.key}-{i}.code"
    code_file.unlink(missing_ok=True)
    hold = ["sh", "-c", '"$@"; echo $? > "$0"; exec sleep 3600', str(code_file)]
    started = time.monotonic()
    with open(log, "w") as out:
        proc = subprocess.Popen(
            meter.prefix(unit, where.cpus) + hold + r.argv("task", prompt=PROMPT),
            env=r.env(),
            cwd=str(work),
            stdin=subprocess.DEVNULL,
            stdout=out,
            stderr=subprocess.STDOUT,
        )
        m.start()
        while not code_file.exists() and time.monotonic() - started < LIMIT_S:
            time.sleep(0.05)
    wall = time.monotonic() - started
    code = int(code_file.read_text().strip() or -1) if code_file.exists() else -1
    settled = m.quiet(settle=1.0, limit=30.0)
    final = m.stop()
    proc.wait(timeout=10)
    after = size(r.home)
    passed = subprocess.run(["python3", "test_calc.py"], cwd=work, capture_output=True, text=True).returncode == 0
    out = {
        "tool": r.name,
        "run": i,
        "exit": code,
        "wall_s": wall,
        "settle_s": settled,
        "tests_pass": passed,
        "data_growth_bytes": after[0] - before[0],
        "data_growth_files": after[1] - before[1],
        "tail": log.read_text(errors="replace")[-600:],
        "final": final,
    }
    (where.results / f"task-{r.key}-{i}.json").write_text(json.dumps(out, ensure_ascii=False, indent=2))
    print(
        f"[task] {r.name} #{i}：退出 {code}，测试{'过' if passed else '没过'}，用时 {wall:.1f} s，"
        f"CPU {final['cpu_usec'] / 1000:.0f} ms，峰值 PSS {final['peak_pss_sum'] / 2**20:.1f} MiB，"
        f"写盘 {final['io']['write_bytes'] / 1024:.0f} KiB，数据 +{out['data_growth_bytes'] / 1024:.0f} KiB",
        flush=True,
    )
    return out


def run(where, rivals, runs):
    where.results.mkdir(parents=True, exist_ok=True)
    for i in range(runs):
        for r in rivals:
            one(r, rivals, i)
    reap.reap(homes.homes(rivals))
