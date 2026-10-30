"""工具一步和 jcode 比（施工 V-29，蓝图 `docs/blueprint/perf.md`「工具一步」）：同一个假模型（`toolfake.py`），两家各收到
同样意思的工具调用（工具名、参数照各家自己给模型的工具表），量假模型写完带调用的回答（`[DONE]`）到读全带着结果的
下一个请求头，和这一段那一家的 cgroup 花的 CPU（含子进程）。

- Miyu：核心（`miyu core`）放进自己的 scope 起着，每种调用跑一次 `miyu ask`（关记忆；沙盒关着的几种 `--permission full`）。
  CPU 只算核心那个 scope（核心和它拉起的沙盒、命令），`miyu ask` 这个头不算。
- jcode：每种调用跑一次 `jcode run`，整个进程放进自己的 scope。jcode 的命令不进沙盒（它的代码里没有 bwrap、Landlock）。
- 每种调 `--samples` 次另加头一次（不计），两家挨着量。家目录、工作区都在 `--out` 下，jcode 的 HOME、XDG_* 都指进去，不碰本机的
  `~/.jcode`。不调真模型。只在 Linux 上、有 systemd 用户会话时能量。

    python3 bench/rivals/toolstep.py --miyu target/release/miyu --jcode <jcode> --samples 20 --cpus 4-7 \\
        --into docs/perf --tag v29

只用标准库。
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import toolfake  # noqa: E402
import toolkinds  # noqa: E402

REPO = Path(__file__).resolve().parents[2]


def scope_dir(unit):
    uid = os.getuid()
    return f"/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}.scope"


def prefix(unit, cpus):
    argv = ["systemd-run", "--user", "--scope", "-q", "--collect", f"--unit={unit}", "--"]
    if cpus:
        argv += ["taskset", "-c", cpus]
    return argv + ["env", "-u", "XDG_RUNTIME_DIR"]


def clean_env(extra):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("MIYU_", "JCODE_", "OPENAI_")) and k not in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY")}
    env["NO_PROXY"] = "127.0.0.1"
    env.update(extra)
    return env


def wait_unlocked(home, limit=60):
    """等核心放开数据根：停了再起要等上一个走干净（套接字、锁）。"""
    end = time.monotonic() + limit
    while time.monotonic() < end and list(home.glob("run/*.sock")):
        time.sleep(0.05)


def steps_of(records, samples):
    """照剧本的记录算每一步：第 n 次调用的回答写完到带着它的结果的请求读全；头一次不算。"""
    by_step = {}
    for record in records:
        by_step.setdefault(record["step"], record)
    steps = []
    for n in range(1, samples + 1):
        done, nxt = by_step.get(n), by_step.get(n + 1)
        if not done or not nxt:
            continue
        cpu = None
        if done["cpu_done"] is not None and nxt["cpu_at"] is not None:
            cpu = nxt["cpu_at"] - done["cpu_done"]
        steps.append({"ms": nxt["at"] - done["done"], "cpu_ms": cpu})
    return steps


class Miyu:
    name = "miyu"

    def __init__(self, program, out, base_url, cpus):
        self.program, self.cpus = program, cpus
        self.home = out / "miyu-home"
        self.home.mkdir(parents=True)
        self.env = clean_env({"MIYU_HOME": str(self.home), "MIYU_RESOURCES": str(REPO / "resources"),
                              "MIYU_CATALOG_UPDATE": "false", "XDG_CACHE_HOME": str(out / "miyu-cache"),
                              "MIYU_PERF_KEY": "perf"})
        # 空的家目录先让核心起一次、建好数据根（有别的东西的目录它不认），停了再写配置。
        boot = self.start(None)
        boot.terminate()
        boot.wait(timeout=30)
        (self.home / "config.toml").write_text(
            f'[providers.perf]\ndriver = "openai-chat"\nbase_url = "{base_url}"\nkey = {{ env = "MIYU_PERF_KEY" }}\n\n'
            '[providers.perf.models.perf-model]\nwindow = 200000\n\n[models]\nchat = "perf/perf-model"\n')
        unit = f"toolstep-miyu-{uuid.uuid4().hex[:8]}"
        self.core = self.start(unit)
        self.group = scope_dir(unit)

    def start(self, unit):
        """起核心，等它写来 `ready`；`unit` 给了的放进那个 scope。"""
        argv = prefix(unit, self.cpus) if unit else ["env", "-u", "XDG_RUNTIME_DIR"]
        core = subprocess.Popen(argv + [str(self.program), "core", "--idle-seconds", "3600"], env=self.env,
                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if core.stdout.readline() != b"ready\n":
            core.kill()
            raise SystemExit("toolstep: Miyu 的核心没起来")
        return core

    def tool(self, kind):
        return kind.miyu_tool

    def args(self, kind, ws, n):
        return kind.miyu(ws, n)

    def run(self, kind, ws, prompt, arm):
        arm(self.group)
        argv = [str(self.program), "ask", "--no-memory"]
        if kind.full:
            argv += ["--permission", "full"]
        command = (["taskset", "-c", self.cpus] if self.cpus else []) + argv + [prompt]
        return subprocess.run(command, cwd=ws.dir, env=self.env, stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=600)

    def close(self):
        self.core.terminate()
        self.core.wait(timeout=30)
        wait_unlocked(self.home)


class Jcode:
    name = "jcode"

    def __init__(self, program, out, base_url, cpus):
        self.program, self.cpus = program, cpus
        home = out / "jcode-home"
        home.mkdir(parents=True)
        self.env = clean_env({
            "HOME": str(home), "XDG_CONFIG_HOME": f"{home}/.config", "XDG_DATA_HOME": f"{home}/.local/share",
            "XDG_CACHE_HOME": f"{home}/.cache", "XDG_STATE_HOME": f"{home}/.local/state",
            "JCODE_NO_TELEMETRY": "1", "DO_NOT_TRACK": "1", "JCODE_NO_AUTO_UPDATE": "1", "JCODE_CHECK_UPDATES": "0",
            "JCODE_PROVIDER": "openai-compatible", "JCODE_OPENAI_COMPAT_API_BASE": base_url,
            "JCODE_OPENAI_COMPAT_DEFAULT_MODEL": "perf-model", "JCODE_OPENAI_COMPAT_API_KEY_NAME": "OPENAI_API_KEY",
            "OPENAI_API_KEY": "fake"})

    def tool(self, kind):
        return kind.jcode_tool

    def args(self, kind, ws, n):
        return kind.jcode(ws, n)

    def run(self, kind, ws, prompt, arm):
        # 整个 `jcode run` 进自己的 scope：scope 的目录照单元名定，先交给假模型，起来以后它读得到。
        unit = f"toolstep-jcode-{uuid.uuid4().hex[:8]}"
        arm(scope_dir(unit))
        argv = prefix(unit, self.cpus) + [str(self.program), "--no-update", "run", "-p", "openai-compatible", prompt]
        return subprocess.run(argv, cwd=ws.dir, env=self.env, stdin=subprocess.DEVNULL, capture_output=True,
                              timeout=600)

    def close(self):
        pass


def measure(rival, fake, kind, index, ws, samples):
    """一种调用：剧本、跑一次、收记录。交回每一步，和那一次的退出码、输出的最后几行（出了错时看）。"""
    marker = f"perf-tool-{index:02d}"
    calls = [{"id": f"perf_{index:02d}_{n:03d}", "name": rival.tool(kind),
              "arguments": json.dumps(rival.args(kind, ws, n), ensure_ascii=False)} for n in range(samples + 1)]
    result = rival.run(kind, ws, f"{marker}: go.", lambda group: fake.script(marker, calls, group))
    records = fake.taken()
    tail = (result.stdout + result.stderr).decode(errors="replace").strip().splitlines()[-6:]
    return steps_of(records, samples), result.returncode, tail


def spread(values):
    values = sorted(values)
    if not values:
        return None
    pick = lambda p: values[max(1, -(-len(values) * p // 100)) - 1]  # noqa: E731 最近秩，同量尺
    return {"p50": pick(50), "p95": pick(95)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--miyu", type=Path, default=REPO / "target/release/miyu")
    parser.add_argument("--jcode", type=Path, default=shutil.which("jcode"))
    parser.add_argument("--tree", type=Path, default=REPO / "crates")
    parser.add_argument("--samples", type=int, default=20)
    parser.add_argument("--cpus")
    parser.add_argument("--out", type=Path, default=REPO / "target/toolstep")
    parser.add_argument("--only", action="append", choices=["miyu", "jcode"])
    parser.add_argument("--into", type=Path, help="汇总写到哪个目录（`<日期>-toolstep-<标签>.json/.md`）")
    parser.add_argument("--tag", default="local")
    args = parser.parse_args()
    args.out, args.tree = args.out.resolve(), args.tree.resolve()
    if sys.platform != "linux" or not shutil.which("systemd-run"):
        print("toolstep: 只在有 systemd 用户会话的 Linux 上量", file=sys.stderr)
        raise SystemExit(2)
    if args.out.exists():
        shutil.rmtree(args.out)
    args.out.mkdir(parents=True)
    ws = toolkinds.Workspace.prepare(args.out / "ws", args.tree)
    fake = toolfake.Fake()
    only = args.only or ["miyu", "jcode"]
    load_before = Path("/proc/loadavg").read_text().split()[:3]
    rivals = {}
    for name in only:
        program = args.miyu if name == "miyu" else args.jcode
        if not program or not Path(program).exists():
            print(f"toolstep: 找不到 {name}，跳过", file=sys.stderr)
            continue
        rivals[name] = (Miyu if name == "miyu" else Jcode)(Path(program).resolve(), args.out, fake.base_url, args.cpus)
    results = {name: [] for name in rivals}
    try:
        # 每种调用两家挨着量：机器上别的活忽多忽少，挨着量两家碰上的差不多。
        for index, kind in enumerate(toolkinds.KINDS):
            for name, rival in rivals.items():
                if rival.tool(kind) is None:
                    results[name].append({"name": kind.name, "tool": None, "steps": []})
                    continue
                ws.reset()
                steps, code, tail = measure(rival, fake, kind, index, ws, args.samples)
                print(f"  {name} {kind.name}: {len(steps)} 步，退出码 {code}", file=sys.stderr)
                if len(steps) < args.samples:
                    print("    " + "\n    ".join(tail), file=sys.stderr)
                results[name].append({"name": kind.name, "tool": rival.tool(kind), "exit": code, "steps": steps})
    finally:
        for rival in rivals.values():
            rival.close()
    load_after = Path("/proc/loadavg").read_text().split()[:3]
    summary = {"date": time.strftime("%Y-%m-%d", time.gmtime()), "samples": args.samples, "cpus": args.cpus,
               "load_before": " ".join(load_before), "load_after": " ".join(load_after),
               "versions": versions(args), "results": results}
    text = render(summary)
    print(text)
    if args.into:
        args.into.mkdir(parents=True, exist_ok=True)
        stem = args.into / f"{summary['date']}-toolstep-{args.tag}"
        stem.with_suffix(".json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
        stem.with_suffix(".md").write_text(text)


def versions(args):
    found = {}
    for name, argv in (("miyu", [str(args.miyu), "--version"]), ("jcode", [str(args.jcode), "--version"])):
        try:
            found[name] = subprocess.run(argv, capture_output=True, timeout=30).stdout.decode().strip().splitlines()[0]
        except (OSError, IndexError, subprocess.SubprocessError, TypeError):
            found[name] = None
    return found


def render(summary):
    names = list(summary["results"])
    lines = [f"# 工具一步：Miyu 和 jcode（{summary['date']}）", "",
             f"每种 {summary['samples']} 步（另加头一次不计）；绑核 {summary['cpus'] or '不绑'}；"
             f"负载 {summary['load_before']} → {summary['load_after']}；版本 {summary['versions']}。", "",
             "毫秒，p50 / p95；CPU 是这一段那一家的 cgroup 花的（含子进程），p50。", "",
             "| 调用 | " + " | ".join(f"{n} 工具 | {n} 一步 | {n} CPU" for n in names) + " |",
             "|---|" + "---|---|---|" * len(names)]
    for index, kind in enumerate(toolkinds.KINDS):
        cells = []
        for name in names:
            row = summary["results"][name][index]
            ms = spread([s["ms"] for s in row["steps"]])
            cpu = spread([s["cpu_ms"] for s in row["steps"] if s["cpu_ms"] is not None])
            cells += [f"`{row['tool']}`" if row["tool"] else "—",
                      f"{ms['p50']:.2f} / {ms['p95']:.2f}" if ms else "—",
                      f"{cpu['p50']:.2f}" if cpu else "—"]
        lines.append(f"| {kind.name} | " + " | ".join(cells) + " |")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
