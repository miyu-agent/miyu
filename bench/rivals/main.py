# /// script
# requires-python = ">=3.11"
# dependencies = ["pyte"]
# ///
"""三家对比（施工 V-7，蓝图 `docs/blueprint/perf.md`「三家对比」）：同一台机器、同一个模型端点、各自干净的沙箱家目录，
量 Miyu、jcode、codex 的 CPU、内存、读写、硬盘。发版前跑一次，结果写进 `docs/perf/`。

    uv run bench/rivals/main.py --tui <终端仓库> all --into docs/perf

端点、key、模型名照环境变量 OPENAI_BASE_URL、OPENAI_API_KEY、MIYU_BENCH_MODEL。
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import conditions  # noqa: E402
import disk  # noqa: E402
import homes  # noqa: E402
import leaks  # noqa: E402
import long  # noqa: E402
import places  # noqa: E402
import rival  # noqa: E402
import screen  # noqa: E402
import summary  # noqa: E402
import task  # noqa: E402


def parse(argv):
    p = argparse.ArgumentParser(prog="rivals", description="三家对比：Miyu、jcode、codex")
    p.add_argument("--core", default=str(places.REPO), help="核心仓库（已经 cargo build --release），默认是这个仓库")
    p.add_argument("--tui", help="终端界面的仓库（已经 cargo build --release）")
    p.add_argument("--jcode", help="jcode 的程序，默认照 PATH 找")
    p.add_argument("--codex", help="codex 的程序，默认照 PATH 找")
    p.add_argument("--out", default=str(places.REPO / "target/rivals"), help="家目录、模板、原始结果放哪")
    p.add_argument("--cpus", help="绑到这几个核上量，例如 0-5；不写不绑")
    p.add_argument("--only", action="append", default=[], help="只量这一家（rivals.toml 的表名或显示名），可以写几次")
    sub = p.add_subparsers(dest="command", required=True)
    sub.add_parser("homes", help="做家目录模板：配好模型、跑一轮、开一次终端界面")
    timing = argparse.ArgumentParser(add_help=False)
    timing.add_argument("--runs", type=int, default=5, help="每个场景量几次")
    timing.add_argument("--settle", type=float, default=13, help="首屏后先等几秒再量空闲")
    timing.add_argument("--idle", type=float, default=60, help="空闲量几秒（三个会话同时开的量一半）")
    sub.add_parser("screen", parents=[timing], help="冷启动、空闲、热启动、三个会话同时开")
    t = sub.add_parser("task", help="一轮真任务")
    t.add_argument("--runs", type=int, default=5)
    g = sub.add_parser("long", help="长会话：gen 造、open 量打开")
    g.add_argument("mode", choices=["gen", "open"])
    g.add_argument("--runs", type=int, default=5)
    sub.add_parser("disk", help="硬盘")
    m = sub.add_parser("summary", help="汇成表")
    m.add_argument("--into", help="再写一份进这个目录（例如 docs/perf）")
    m.add_argument("--tag", help="文件名后缀，例如 v0.8.0")
    a = sub.add_parser(
        "all", parents=[timing], help="从头量一遍：homes、screen、task、long gen、long open、disk、summary"
    )
    a.add_argument("--into", help="同 summary")
    a.add_argument("--tag", help="同 summary")
    return p.parse_args(argv)


def remember(where, rivals):
    """量之前记下条件，汇表时照它写（量和汇可以分开跑）。"""
    where.results.mkdir(parents=True, exist_ok=True)
    found = conditions.collect(where, rivals)
    (where.results / "conditions.json").write_text(json.dumps(found, ensure_ascii=False, indent=2))
    return found


def recalled(where, rivals):
    path = where.results / "conditions.json"
    return json.loads(path.read_text()) if path.exists() else conditions.collect(where, rivals)


def main(argv):
    args = parse(argv)
    where = places.load(args)
    rivals = rival.load(where, args.only)
    into = Path(args.into).resolve() if getattr(args, "into", None) else None
    if args.command in ("homes", "all"):
        remember(where, rivals)
        for r in rivals:
            homes.build(r, rivals)
    if args.command == "screen":
        remember(where, rivals)
        screen.run(where, rivals, args.runs, args.settle, args.idle)
    if args.command == "task":
        task.run(where, rivals, args.runs)
    if args.command == "long":
        long.run(where, rivals, args.mode, args.runs)
    if args.command == "all":
        screen.run(where, rivals, args.runs, args.settle, args.idle)
        task.run(where, rivals, args.runs)
        long.run(where, rivals, "gen", 1)
        long.run(where, rivals, "open", args.runs)
    if args.command in ("disk", "all"):
        disk.run(where, rivals)
    if args.command in ("summary", "all"):
        summary.run(where, rivals, recalled(where, rivals), into, args.tag)
    leaks.report(where.out)


if __name__ == "__main__":
    main(sys.argv[1:])
