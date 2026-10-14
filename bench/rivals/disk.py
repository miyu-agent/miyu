"""硬盘（施工 V-7）：装好占多大、家目录跑过一次以后多大、长会话那一个会话自己的文件多大。照占的块算，硬链接只算一次。"""

import json
from pathlib import Path


def du(paths):
    seen = set()
    total = 0
    for p in map(Path, paths):
        items = [p] if p.is_file() else [f for f in p.rglob("*") if f.is_file() and not f.is_symlink()]
        for f in items:
            st = f.stat()
            if (st.st_dev, st.st_ino) in seen:
                continue
            seen.add((st.st_dev, st.st_ino))
            total += st.st_blocks * 512
    return total


def run(where, rivals):
    out = {"install": {}, "home_after_first_run": {}, "long_session": {}}
    for r in rivals:
        # 没有的那一样记 null：表里写「—」，不当成 0。
        out["install"][r.name] = {label: du([path]) if path.exists() else None for label, path in r.install()}
        out["home_after_first_run"][r.name] = du([where.templates / r.key])
        long_home = where.templates / f"{r.key}-long"
        if long_home.exists():
            session = (long_home / ".rivals-session").read_text().strip()
            out["long_session"][r.name] = du(r.session_files(long_home, session))
    where.results.mkdir(parents=True, exist_ok=True)
    (where.results / "disk.json").write_text(json.dumps(out, ensure_ascii=False, indent=2))
    for r in rivals:
        total = sum(v or 0 for v in out["install"][r.name].values())
        home = out["home_after_first_run"][r.name]
        print(
            f"[disk] {r.name}：装好 {total / 2**20:.1f} MiB，家目录 {home / 1024:.0f} KiB",
            flush=True,
        )
