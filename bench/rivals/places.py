"""三家对比用到的东西在哪：程序、端点、输出目录（施工 V-7）。

端点和 key 只从环境变量来（OPENAI_BASE_URL、OPENAI_API_KEY、MIYU_BENCH_MODEL）：key 留在环境变量里交给各家，
不写进任何文件、不打印；结果里只记模型名，不记地址。

用本机转发的（`--upstream`，施工 V-27，`relay.py`）：真 key 在 `--upstream-key-env` 写的环境变量里，只交给转发进程；
各家连转发，环境里是假 key（`FAKE_KEY`），不用设 OPENAI_BASE_URL、OPENAI_API_KEY。
"""

import os
import re
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

# 环境变量：端点、key、模型名。
BASE_URL = "OPENAI_BASE_URL"
KEY = "OPENAI_API_KEY"
MODEL = "MIYU_BENCH_MODEL"

# 终端仓库里装成软件包的那个目录：和包的编号同名（核心设计 48、施工 PN-4 起；以前叫 `tui/`），`heads.toml` 开头写着。
TUI_PACKAGE = "miyu-tui"

# 用转发时交给各家的假 key：转发换成真的。
FAKE_KEY = "rivals-relay-placeholder"

# 起各家之前从环境里去掉的：别让真家目录、真会话的设置漏进来。
STRIPPED = ("MIYU_", "HERDR_", "CODEX_", "JCODE_")

# 起被测的程序都套这一层：没有 XDG_RUNTIME_DIR，沙箱的核心把套接字放进自己的数据根（`ipc.md`），跑完和家目录一起清掉。
# 放进 scope 的那一种由 `meter.prefix` 套：systemd-run 自己还要用它。
BARE = ["env", "-u", "XDG_RUNTIME_DIR"]


def fail(message):
    """说一句为什么不能量，退出码 2。"""
    print(f"rivals: {message}", file=sys.stderr)
    raise SystemExit(2)


@dataclass
class Places:
    """一趟量的条件。`programs` 照名字找程序（链接解开了）；`out` 下面放家目录、模板、工作目录、原始结果。"""

    core: Path
    tui_repo: Path
    programs: dict
    out: Path
    cpus: str | None
    model: str
    base_url: str
    # 用转发时：转发的根地址（`http://127.0.0.1:<端口>`，起来以后填）、真 key 在哪个环境变量里（施工 V-27）。
    relay: str | None = None
    upstream_key_env: str | None = None

    @property
    def homes(self):
        return self.out / "homes"

    @property
    def templates(self):
        return self.out / "templates"

    @property
    def work(self):
        return self.out / "work"

    @property
    def results(self):
        return self.out / "results"

    @property
    def logs(self):
        return self.out / "logs"

    def words(self, extra=None):
        """占位换成什么（`rivals.toml` 开头那一段）。"""
        words = {
            "model": self.model,
            "base_url": self.base_url,
            "work": str(self.work),
            "core": str(self.core),
            "release": str(self.core / "target/release"),
            "tui_package": str(self.tui_repo / TUI_PACKAGE),
        }
        for name, path in self.programs.items():
            # 没找到的（没装、也没指）不给占位：只量别的几家时用不到它（`rival.load` 查过要量的几家都找到了）。
            if path is None:
                continue
            words[name] = str(path)
            words[f"{name}_dir"] = str(path.parent)
        words.update(extra or {})
        return words


def fill(text, words):
    """把 `{名字}` 换成 `words` 里的；不认识的、带空白的（`{ env = … }`）原样留着。"""
    return re.sub(r"\{([a-z_0-9]+)\}", lambda m: words.get(m.group(1), m.group(0)), text)


def program(given, name):
    """照参数、再照 PATH 找程序，解开链接。参数指的不存在就不量；没指、PATH 里也没有的交回 None，要量这一家时再说。"""
    if given and not Path(given).is_file():
        fail(f"--{name} 指的 {given} 不存在")
    found = given or shutil.which(name)
    return Path(found).resolve() if found else None


def load(args):
    """照参数和环境变量定条件；缺了什么就说清楚、退出。"""
    if sys.platform != "linux":
        fail("只在 Linux 上量：CPU、内存照 cgroup v2 和 /proc 读（蓝图 perf.md「三家对比」）")
    upstream = getattr(args, "upstream", None)
    key_env = getattr(args, "upstream_key_env", None)
    if upstream and not key_env:
        fail("用 --upstream 时要用 --upstream-key-env 说真 key 在哪个环境变量里")
    wanted = (MODEL, key_env) if upstream else (BASE_URL, KEY, MODEL)
    missing = [name for name in wanted if not os.environ.get(name)]
    if missing:
        fail("没设环境变量 " + "、".join(missing))
    core = Path(args.core).resolve()
    tui_repo = (
        Path(args.tui).resolve() if args.tui else fail("用 --tui 指给终端界面的仓库（已经 cargo build --release）")
    )
    miyu = core / "target/release/miyu"
    tui = tui_repo / "target/release/miyu-tui"
    for path, how in ((miyu, f"在 {core} 里 cargo build --release"), (tui, f"在 {tui_repo} 里 cargo build --release")):
        if not path.is_file():
            fail(f"没有 {path}：先{how}")
    if not (tui_repo / TUI_PACKAGE / "package.toml").is_file():
        fail(f"{tui_repo} 里没有 {TUI_PACKAGE}/package.toml，不是终端界面的仓库（施工 PN-4 以前的叫 tui/，太旧了）")
    programs = {"miyu": miyu, "tui": tui}
    for name in ("jcode", "codex"):
        programs[name] = program(getattr(args, name), name)
    return Places(
        core=core,
        tui_repo=tui_repo,
        programs=programs,
        out=Path(args.out).resolve(),
        cpus=args.cpus,
        model=os.environ[MODEL],
        # 用转发的，各家的地址照家算（`Rival.words`），这里只是占着。
        base_url=os.environ.get(BASE_URL, "") if not upstream else "",
        upstream_key_env=key_env if upstream else None,
    )


def base_env(strip=()):
    """各家共用的环境：去掉 Miyu、herdr、codex、jcode 自己的变量和 `strip` 里的（真 key 的变量），终端照同一种报。"""
    env = {k: v for k, v in os.environ.items() if not k.startswith(STRIPPED) and k not in strip}
    env.update(TERM="xterm-256color", COLORTERM="truecolor", LANG="zh_CN.UTF-8")
    return env
