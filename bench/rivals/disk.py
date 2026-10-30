"""硬盘（施工 V-7）：装好占多大、家目录跑过一次以后多大、长会话那一个会话自己的文件多大。照占的块算，硬链接只算一次。

家目录分两样报（2026-10-31 项目主人定，施工 V-27）：会一直复用的缓存（`rivals.toml` 的 `caches`：模型目录、图标这类）
单列，不算进「跑过一次多了多少」；剩下的是跑一次产生的数据（会话、日志、索引这些），另按文件列出来，看是什么占的。
数据报两种口径（2026-10-31 项目主人定）：模板是后台进程好好退出以后存的（平时正常用剩下的）；做模板时退出之前记下的那一份
（`home-crash-<表名>.json`，`homes.open_once`）是直接杀掉、崩溃、断电剩下的。"""

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


def blocks(f):
    return f.stat().st_blocks * 512


def cached(rel, caches):
    """家目录里的相对路径 `rel` 落在不落在 `caches` 里（照路径的前几段比）。"""
    parts = Path(rel).parts
    return any(parts[: len(Path(c).parts)] == Path(c).parts for c in caches)


def split(home, caches):
    """家目录里的文件分成缓存（照 `caches` 的 glob）和数据：交回 (缓存的文件, 数据的文件)。"""
    cached = set()
    for pattern in caches:
        for p in home.glob(pattern):
            cached.update([p] if p.is_file() else [f for f in p.rglob("*") if f.is_file() and not f.is_symlink()])
    files = [f for f in home.rglob("*") if f.is_file() and not f.is_symlink()]
    return sorted(f for f in files if f in cached), sorted(f for f in files if f not in cached)


def run(where, rivals):
    out = {"install": {}, "home_after_first_run": {}, "home_caches": {}, "home_files": {}, "long_session": {}}
    for r in rivals:
        # 没有的那一样记 null：表里写「—」，不当成 0。
        out["install"][r.name] = {label: du([path]) if path.exists() else None for label, path in r.install()}
        home = where.templates / r.key
        kept, data = split(home, r.table.get("caches", []))
        out["home_after_first_run"][r.name] = du(data)
        out["home_caches"][r.name] = du(kept)
        out["home_files"][r.name] = {str(f.relative_to(home)): blocks(f) for f in data}
        crash = where.results / f"home-crash-{r.key}.json"
        if crash.exists():
            files = {k: v for k, v in json.loads(crash.read_text()).items() if not cached(k, r.table.get("caches", []))}
            out.setdefault("home_after_first_run_crash", {})[r.name] = sum(files.values())
            out.setdefault("home_files_crash", {})[r.name] = files
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
            f"[disk] {r.name}：装好 {total / 2**20:.1f} MiB，家目录的数据 {home / 1024:.0f} KiB，"
            f"复用的缓存 {out['home_caches'][r.name] / 1024:.0f} KiB",
            flush=True,
        )
