"""工具一步量哪几种调用、工作区（施工 V-29）：和量尺的 `crates/miyu-perf/src/measure/tools/calls.rs` 一一对上，每种写两家
各自的工具名和参数（照各家给模型的工具表：Miyu 的 `resources/software/basesystem/tools/*.json`，jcode v0.99.1 请求里的
`tools`）。某家没有对应工具的写 None，表里写「—」。
"""

import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

SMALL_LINES, BIG_LINES, BIG_TAIL = 40, 100_000, 200


def lines(count, width):
    out = []
    for n in range(1, count + 1):
        head = f"line {n:06d} "
        out.append(head + "x" * (width - len(head) - 1) + "\n")
    return "".join(out)


@dataclass
class Workspace:
    dir: Path

    @property
    def small(self):
        return self.dir / "small.txt"

    @property
    def big(self):
        return self.dir / "big.txt"

    @property
    def edited(self):
        return self.dir / "edit.txt"

    @property
    def written(self):
        return self.dir / "out.txt"

    @property
    def tree(self):
        return self.dir / "tree"

    @classmethod
    def prepare(cls, dir, tree):
        """建好：小文件、8 MB 的文件、改的文件，`tree` 拷进来（不拷 `target/`），起成一个 git 仓库（同量尺）。"""
        dir.mkdir(parents=True)
        ws = cls(dir)
        ws.small.write_text(lines(SMALL_LINES, 50))
        ws.big.write_text(lines(BIG_LINES, 80))
        shutil.copytree(tree, ws.tree, ignore=shutil.ignore_patterns("target"), symlinks=True)
        subprocess.run(["git", "init", "-q"], cwd=dir, check=True)
        ws.reset()
        return ws

    def reset(self):
        """每种调用开始前：改的文件换回 `value-0`，写的文件删掉。"""
        self.edited.write_text(lines(25, 80) + "value-0\n" + lines(25, 80))
        self.written.unlink(missing_ok=True)


@dataclass
class Kind:
    name: str
    miyu_tool: str | None
    miyu: object
    jcode_tool: str | None
    jcode: object
    full: bool = False


def shell(command):
    return (lambda ws, n: {"command": command(ws), "description": f"perf step {n}"},
            lambda ws, n: {"command": command(ws), "intent": f"perf step {n}"})


CAT = lambda ws: f"cat '{ws.small}'"  # noqa: E731
TRUE = lambda ws: "true"  # noqa: E731
ECHO = lambda ws: "echo perf"  # noqa: E731
WRITTEN = lambda n: f"written {n}\n" + lines(50, 80)  # noqa: E731

KINDS = [
    Kind("read 小文件", "read", lambda ws, n: {"file_path": str(ws.small)},
         "read", lambda ws, n: {"file_path": str(ws.small), "intent": "perf"}),
    Kind("read 8 MB 文件的末尾", "read",
         lambda ws, n: {"file_path": str(ws.big), "offset": BIG_LINES - BIG_TAIL + 1, "limit": BIG_TAIL},
         "read", lambda ws, n: {"file_path": str(ws.big), "start_line": BIG_LINES - BIG_TAIL + 1, "limit": BIG_TAIL,
                                "intent": "perf"}),
    # jcode v0.99.1 给模型的表里没有 `ls`：列目录不比。
    Kind("read 列目录", "read", lambda ws, n: {"file_path": str(ws.tree / "miyu-session" / "src")}, None, None),
    Kind("write 4 KB", "write", lambda ws, n: {"file_path": str(ws.written), "content": WRITTEN(n)},
         "write", lambda ws, n: {"file_path": str(ws.written), "content": WRITTEN(n), "intent": "perf"}),
    Kind("edit 一处", "edit",
         lambda ws, n: {"file_path": str(ws.edited), "edits": [{"old_string": f"value-{n}\n",
                                                                 "new_string": f"value-{n + 1}\n"}]},
         "edit", lambda ws, n: {"file_path": str(ws.edited), "old_string": f"value-{n}\n",
                                "new_string": f"value-{n + 1}\n", "intent": "perf"}),
    Kind("grep 树里搜", "grep", lambda ws, n: {"pattern": "fn main", "path": str(ws.tree)},
         "agentgrep", lambda ws, n: {"mode": "grep", "query": "fn main", "path": str(ws.tree), "paths_only": True,
                                     "intent": "perf"}),
    Kind("grep 树里搜（带行）", "grep",
         lambda ws, n: {"pattern": "fn main", "path": str(ws.tree), "output_mode": "content"},
         "agentgrep", lambda ws, n: {"mode": "grep", "query": "fn main", "path": str(ws.tree), "intent": "perf"}),
    Kind("glob 树里找", "glob", lambda ws, n: {"pattern": "*.rs", "path": str(ws.tree)},
         "agentgrep", lambda ws, n: {"mode": "find", "query": ".rs", "path": str(ws.tree), "intent": "perf"}),
    Kind("shell true", "shell", shell(TRUE)[0], "bash", shell(TRUE)[1]),
    Kind("shell echo", "shell", shell(ECHO)[0], "bash", shell(ECHO)[1]),
    Kind("shell cat 小文件", "shell", shell(CAT)[0], "bash", shell(CAT)[1]),
    Kind("shell true（沙盒关着）", "shell", shell(TRUE)[0], "bash", shell(TRUE)[1], full=True),
    Kind("shell echo（沙盒关着）", "shell", shell(ECHO)[0], "bash", shell(ECHO)[1], full=True),
    Kind("shell cat 小文件（沙盒关着）", "shell", shell(CAT)[0], "bash", shell(CAT)[1], full=True),
]
