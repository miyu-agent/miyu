"""跑完查一遍：key 有没有落进输出目录（施工 V-7）。测具自己不写 key，别家可能写（codex 把环境存进 shell_snapshots）。

只报文件，不印 key；报了的跑完连同整个输出目录删掉。"""

import os
from pathlib import Path

import places


def find(out):
    """输出目录下内容里有 key 的文件。"""
    key = os.environ.get(places.KEY, "").encode()
    if not key or not Path(out).exists():
        return []
    found = []
    for f in Path(out).rglob("*"):
        if f.is_file() and not f.is_symlink():
            try:
                if key in f.read_bytes():
                    found.append(f)
            except OSError:
                pass
    return found


def report(out):
    found = find(out)
    if found:
        print(f"[leaks] {len(found)} 个文件里有 key（别家写的）：跑完把 {out} 整个删掉", flush=True)
        for f in found[:20]:
            print(f"  {f.relative_to(out)}", flush=True)
    return found
