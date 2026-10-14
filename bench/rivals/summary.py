"""把原始结果汇成一张表（施工 V-7）：每个场景、每项的中位数和最小、最大，写成 JSON 和 Markdown。"""

import json
import statistics

MIB = 2**20

# 场景、原始结果的前缀、每一项：(键, 说明, 照一份结果怎么算)。
SCENES = [
    (
        "cold",
        "冷启动到首屏（没有后台进程）",
        "cold",
        [
            ("first_screen_ms", "到首屏 ms", lambda r: r["first_screen_s"] * 1000),
            ("cpu_ms", "CPU ms", lambda r: r["startup_cpu_usec"] / 1000),
            ("pss_mib", "PSS MiB", lambda r: r["startup_pss"] / MIB),
        ],
    ),
    (
        "idle",
        "空闲（首屏后先等收尾，再量）",
        "cold",
        [
            ("cpu_pct", "CPU %（一个核）", lambda r: r["idle_cpu_usec"] / (r["idle_s"] * 1e6) * 100),
            ("wakeups_per_s", "每秒醒几次", lambda r: r["idle_wakeups"] / r["idle_s"]),
            ("pty_bytes_per_s", "终端每秒收到字节", lambda r: r["idle_pty_bytes"] / r["idle_s"]),
            ("pss_mib", "常驻 PSS MiB", lambda r: r["idle_end_pss"] / MIB),
            ("wchar_kib", "这一趟写了 KiB（交给系统的）", lambda r: r["final"]["io"]["wchar"] / 1024),
            ("write_kib", "这一趟写盘 KiB（落到盘上的）", lambda r: r["final"]["io"]["write_bytes"] / 1024),
        ],
    ),
    (
        "warm",
        "热启动到首屏（后台进程已经在跑）",
        "warm",
        [
            ("first_screen_ms", "到首屏 ms", lambda r: r["first_screen_s"] * 1000),
            ("cpu_ms", "CPU ms", lambda r: r["startup_cpu_usec"] / 1000),
            ("pss_mib", "PSS MiB（连后台进程）", lambda r: r["startup_pss"] / MIB),
        ],
    ),
    (
        "three",
        "三个会话同时开",
        "three",
        [
            ("pss_mib", "PSS MiB（三个加起来）", lambda r: r["pss"] / MIB),
            ("idle_cpu_pct", "空闲 CPU %", lambda r: r["idle_cpu_usec"] / (r["idle_s"] * 1e6) * 100),
        ],
    ),
    (
        "task",
        "一轮真任务（修一个小项目的 bug）",
        "task",
        [
            ("wall_s", "用时 s", lambda r: r["wall_s"]),
            ("cpu_ms", "CPU ms", lambda r: r["final"]["cpu_usec"] / 1000),
            ("peak_pss_mib", "峰值 PSS MiB", lambda r: r["final"]["peak_pss_sum"] / MIB),
            ("wchar_kib", "写了 KiB（交给系统的）", lambda r: r["final"]["io"]["wchar"] / 1024),
            ("write_kib", "写盘 KiB（落到盘上的）", lambda r: r["final"]["io"]["write_bytes"] / 1024),
            ("read_kib", "读盘 KiB", lambda r: r["final"]["io"]["read_bytes"] / 1024),
            ("data_kib", "家目录多了 KiB", lambda r: r["data_growth_bytes"] / 1024),
        ],
    ),
    (
        "long",
        "打开 50 轮的长会话",
        "long",
        [
            ("open_ms", "到看见最后一句 ms", lambda r: r["open_s"] * 1000),
            ("cpu_ms", "CPU ms", lambda r: r["open_cpu_usec"] / 1000),
            ("pss_mib", "PSS MiB", lambda r: r["open_pss"] / MIB),
        ],
    ),
]


def stat(values):
    if not values:
        return None
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "n": len(values)}


def load(where, prefix, rivals):
    out = {}
    for r in rivals:
        runs = [json.loads(f.read_text()) for f in sorted(where.results.glob(f"{prefix}-{r.key}-*.json"))]
        if runs:
            out[r.name] = runs
    return out


def trace(runs):
    """每秒 CPU（毫秒）的中位数：看首屏以后收尾要多久才平。"""
    traces = [r["cpu_trace_usec"] for r in runs if r.get("cpu_trace_usec")]
    if not traces:
        return []
    return [
        round(statistics.median(t[k] for t in traces if len(t) > k) / 1000, 1) for k in range(max(map(len, traces)))
    ]


def build(where, rivals, conditions):
    summary = {"conditions": conditions, "scenes": {}}
    for key, _title, prefix, metrics in SCENES:
        runs = load(where, prefix, rivals)
        summary["scenes"][key] = {
            name: {m: stat([fn(r) for r in rs]) for m, _label, fn in metrics} for name, rs in runs.items()
        }
        if key == "idle":
            summary["cpu_trace_ms"] = {name: trace(rs) for name, rs in runs.items()}
        if key == "task":
            summary["task_passed"] = {
                name: f"{sum(r['tests_pass'] for r in rs)}/{len(rs)}" for name, rs in runs.items()
            }
    disk = where.results / "disk.json"
    if disk.exists():
        summary["disk"] = json.loads(disk.read_text())
    return summary


def number(value):
    if value is None:
        return "—"
    return f"{value:.0f}" if abs(value) >= 100 else f"{value:.1f}"


def markdown(summary, names):
    c = summary["conditions"]
    m = c["machine"]
    lines = [
        f"## 三家对比 {c['date']}",
        "",
        f"- 机器：{m['cpu']}，{m['logical_cores']} 个逻辑核，内存 {m['memory_gb']} GB；内核 {m['kernel']}，{m['arch']}"
        + (f"；绑在 {m['pinned_cpus']} 号核上" if m["pinned_cpus"] else ""),
        f"- 模型：{c['model']}（三家同一个端点）",
        "- 版本：" + "，".join(f"{n} {v or '—'}" for n, v in c["versions"].items()),
        f"- 提交：核心 {c['core']}，终端 {c['tui']}"
        + ("" if c["tui_matches_heads_toml"] else "（不是 heads.toml 钉的那一份）"),
        "- 量法：`docs/blueprint/perf.md`「三家对比」。数是中位数，括号里是最小到最大；"
        "每一轮的原始数据在同名的 `.json` 的 `runs` 里。",
        "",
    ]
    for key, title, _prefix, metrics in SCENES:
        scene = summary["scenes"].get(key) or {}
        shown = [n for n in names if n in scene]
        if not shown:
            continue
        runs = max(scene[n][metrics[0][0]]["n"] for n in shown if scene[n][metrics[0][0]])
        lines += [
            f"### {title}（{runs} 次）",
            "",
            "| 项目 | " + " | ".join(shown) + " |",
            "|---|" + "---|" * len(shown),
        ]
        for metric, label, _fn in metrics:
            cells = []
            for n in shown:
                s = scene[n][metric]
                cells.append("—" if s is None else f"{number(s['median'])}（{number(s['min'])}–{number(s['max'])}）")
            lines.append(f"| {label} | " + " | ".join(cells) + " |")
        if key == "task":
            lines.append("| 测试过了几次 | " + " | ".join(summary["task_passed"].get(n, "—") for n in shown) + " |")
        lines.append("")
    disk = summary.get("disk")
    if disk:
        shown = [n for n in names if n in disk["install"]]
        lines += ["### 硬盘", "", "| 项目 | " + " | ".join(shown) + " |", "|---|" + "---|" * len(shown)]
        install = [sum(v or 0 for v in disk["install"][n].values()) / MIB for n in shown]
        lines.append("| 装好 MiB | " + " | ".join(number(v) for v in install) + " |")
        lines.append(
            "| 家目录跑过一次 KiB | " + " | ".join(number(disk["home_after_first_run"][n] / 1024) for n in shown) + " |"
        )
        lines.append(
            "| 长会话那一个会话 KiB | "
            + " | ".join(number(disk["long_session"][n] / 1024) if n in disk["long_session"] else "—" for n in shown)
            + " |"
        )
        lines.append("")
    return "\n".join(lines)


# 原始结果里不入库的格：命令输出的最后几行，带着本机的路径。
PRIVATE = ("tail",)


def raw(where, rivals):
    """每个场景每一轮的原始结果（入库的那一份用）：{场景: {家: [每一轮]}}。"""
    prefixes = sorted({prefix for _key, _title, prefix, _metrics in SCENES})
    return {
        prefix: {
            name: [{k: v for k, v in run.items() if k not in PRIVATE} for run in runs]
            for name, runs in load(where, prefix, rivals).items()
        }
        for prefix in prefixes
    }


def run(where, rivals, conditions, into=None, tag=None):
    summary = build(where, rivals, conditions)
    text = markdown(summary, [r.name for r in rivals])
    (where.results / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2))
    (where.results / "summary.md").write_text(text)
    if into:
        stem = f"{conditions['date']}-rivals" + (f"-{tag}" if tag else "")
        into.mkdir(parents=True, exist_ok=True)
        record = {**summary, "runs": raw(where, rivals)}
        (into / f"{stem}.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n")
        (into / f"{stem}.md").write_text(text)
        print(f"[summary] 写进了 {into / stem}.md、.json", flush=True)
    print(text)
