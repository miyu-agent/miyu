"""把原始结果汇成一张表（施工 V-7）：每个场景、每项的中位数和最小、最大，写成 JSON 和 Markdown。"""

import json
import re
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
            ("data_kib", "家目录多了 KiB（直接杀掉，崩溃、断电剩下的）", lambda r: r["data_growth_bytes"] / 1024),
            (
                "data_clean_kib",
                "家目录多了 KiB（好好退出以后，平时）",
                lambda r: r["data_growth_clean_bytes"] / 1024 if "data_growth_clean_bytes" in r else None,
            ),
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
    values = [v for v in values if v is not None]  # 旧的结果里没有的项（例如 V-27 以前没量「好好退出以后」）
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


# 用量表里场的先后和说明（`relay.mark` 记的场名去掉末尾的轮次）。
TOKEN_SCENES = [
    ("homes", "做模板"),
    ("screen", "开终端界面"),
    ("task", "一轮真任务"),
    ("long-gen", "造长会话"),
    ("long-open", "打开长会话"),
]


def tokens(where, rivals):
    """转发记的用量（施工 V-27）：{显示名: {场: {requests, errors, input, cached, output, reasoning, runs}}}；没用转发的空。"""
    log = where.results / "relay.jsonl"
    if not log.exists():
        return {}
    names = {r.key: r.name for r in rivals}
    out = {}
    for line in log.read_text().splitlines():
        entry = json.loads(line)
        if entry.get("method") != "POST" or entry.get("rival") not in names:
            continue
        scene = re.sub(r"-\d+$", "", entry.get("scene") or "")
        cell = out.setdefault(names[entry["rival"]], {}).setdefault(
            scene, {"requests": 0, "errors": 0, "input": 0, "cached": 0, "output": 0, "reasoning": 0, "runs": set()}
        )
        cell["requests"] += 1
        cell["runs"].add(entry.get("scene"))
        if not 200 <= (entry.get("status") or 0) < 300:
            cell["errors"] += 1
        for k in ("input", "cached", "output", "reasoning"):
            cell[k] += (entry.get("usage") or {}).get(k, 0)
    for scenes in out.values():
        for cell in scenes.values():
            cell["runs"] = len(cell["runs"])
    return out


def loads(where, rivals):
    """每一轮开头记的 1 分钟负载：最小、最大（施工 V-27）。"""
    values = [
        run["loadavg"]
        for _key, _title, prefix, _metrics in SCENES
        for runs in load(where, prefix, rivals).values()
        for run in runs
        if "loadavg" in run
    ]
    return [round(min(values), 2), round(max(values), 2)] if values else None


def build(where, rivals, conditions):
    summary = {"conditions": conditions, "scenes": {}, "tokens": tokens(where, rivals), "loads": loads(where, rivals)}
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
        f"- 模型：{c['model']}（三家同一个端点"
        + ("，经本机转发：换上真 key、补会话头，`bench/rivals/relay.py`）" if c.get("relay") else "）"),
        "- 版本：" + "，".join(f"{n} {v or '—'}" for n, v in c["versions"].items()),
        f"- 提交：核心 {c['core']}，终端 {c['tui']}"
        + ("" if c["tui_matches_heads_toml"] else "（不是 heads.toml 钉的那一份）"),
        *(
            [
                f"- 负载（1、5、15 分钟）：量之前 {'、'.join(map(str, c['load_before']))}"
                + (f"；各轮开头的 1 分钟负载 {summary['loads'][0]}–{summary['loads'][1]}" if summary.get("loads") else "")
                + "。负载高时时间类的数会抖，CPU、内存、读写受的影响小"
            ]
            if c.get("load_before")
            else []
        ),
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
            "| 家目录跑过一次的数据 KiB（会话、日志、索引这些；好好退出以后，平时） | "
            + " | ".join(number(disk["home_after_first_run"][n] / 1024) for n in shown)
            + " |"
        )
        crash = disk.get("home_after_first_run_crash") or {}
        if crash:
            lines.append(
                "| 家目录跑过一次的数据 KiB（直接杀掉，崩溃、断电剩下的） | "
                + " | ".join(number(crash[n] / 1024) if n in crash else "—" for n in shown)
                + " |"
            )
        if "home_caches" in disk:
            lines.append(
                "| 会一直复用的缓存 KiB（模型目录、图标这类，不算在上一行） | "
                + " | ".join(number(disk["home_caches"][n] / 1024) for n in shown)
                + " |"
            )
        lines.append(
            "| 长会话那一个会话 KiB | "
            + " | ".join(number(disk["long_session"][n] / 1024) if n in disk["long_session"] else "—" for n in shown)
            + " |"
        )
        lines.append("")
        # 只拆被量的那一家（表里排第一的 Miyu），别家的照总数比（2026-10-31 项目主人要看 Miyu 是什么占的）。
        for n in shown[:1]:
            clean = (disk.get("home_files") or {}).get(n, {})
            crash = (disk.get("home_files_crash") or {}).get(n)
            paths = sorted(set(clean) | set(crash or {}), key=lambda p: -max(clean.get(p, 0), (crash or {}).get(p, 0)))
            if not paths:
                continue
            head = "| 文件 | 好好退出以后 KiB |" + (" 直接杀掉 KiB |" if crash is not None else "")
            lines += [f"{n} 家目录跑过一次的数据，按文件（照占的块；没有的写「—」）：", "", head]
            lines.append("|---|---|" + ("---|" if crash is not None else ""))

            def cell(files, path):
                return number(files[path] / 1024) if path in files else "—"

            for path in paths[:FILES_SHOWN]:
                lines.append(
                    f"| `{path}` | {cell(clean, path)} |" + (f" {cell(crash, path)} |" if crash is not None else "")
                )
            rest = paths[FILES_SHOWN:]
            if rest:
                total = number(sum(clean.get(p, 0) for p in rest) / 1024)
                crash_total = number(sum(crash.get(p, 0) for p in rest) / 1024) if crash is not None else None
                lines.append(f"| 其余 {len(rest)} 个 | {total} |" + (f" {crash_total} |" if crash is not None else ""))
            lines.append("")
    used = summary.get("tokens") or {}
    shown = [n for n in names if n in used]
    if shown:
        lines += [
            "### 模型用量（本机转发记的）",
            "",
            "每格：请求数（出错几次）；输入 token（其中缓存命中）；输出 token。",
            "",
            "| 场 | " + " | ".join(shown) + " |",
            "|---|" + "---|" * len(shown),
        ]
        for scene, title in TOKEN_SCENES:
            cells = []
            for n in shown:
                c = used[n].get(scene)
                cells.append(
                    "—"
                    if not c
                    else f"{c['requests']}（{c['errors']}）；{c['input']}（{c['cached']}）；{c['output']}"
                )
            if any(cell != "—" for cell in cells):
                lines.append(f"| {title} | " + " | ".join(cells) + " |")
        lines.append("")
    return "\n".join(lines)


# 按文件列家目录的数据时最多列几个，剩下的合成一行。
FILES_SHOWN = 25


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
