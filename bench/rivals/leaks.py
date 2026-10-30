"""跑完查一遍：key 有没有落进输出目录（施工 V-7）。测具自己不写 key，别家可能写（codex 把环境存进 shell_snapshots）。

只报文件，不印 key；报了的跑完连同整个输出目录删掉。"""

import os
from pathlib import Path

import places


def find(out, names=(places.KEY,)):
    """输出目录下内容里有 key 的文件：`names` 是 key 所在的环境变量（用转发的另有真 key 的那一个，施工 V-27）。"""
    keys = [os.environ.get(n, "").encode() for n in names if n]
    keys = [k for k in keys if k and k != places.FAKE_KEY.encode()]
    if not keys or not Path(out).exists():
        return []
    found = []
    for f in Path(out).rglob("*"):
        if f.is_file() and not f.is_symlink():
            try:
                data = f.read_bytes()
                if any(k in data for k in keys):
                    found.append(f)
            except OSError:
                pass
    return found


def report(out, names=(places.KEY,)):
    found = find(out, names)
    if found:
        print(f"[leaks] {len(found)} 个文件里有 key（别家写的）：跑完把 {out} 整个删掉", flush=True)
        for f in found[:20]:
            print(f"  {f.relative_to(out)}", flush=True)
    return found
