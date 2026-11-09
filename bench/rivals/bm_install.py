"""发版前对比（施工 BM-1）：照 `rivals.lock.toml` 装各家、核对哈希（量法设计 1.1）。

    python3 bm_install.py --out <目录> [--only pi ...]

- node：下 nodejs.org 的官方包，和 SHASUMS256.txt 对上才解开，放 <out>/node/。
- 各家：`npm install --prefix <out>/prefix/<家> --ignore-scripts --omit=dev`，HOME、npm 的缓存、配置全指进 <out>/install-home，
  不碰真家目录；装完照 package-lock.json 核对每个钉住的包的 integrity，再算实际跑的程序的 sha256。
- 结果写 <out>/installed.json：版本、integrity、程序的 sha256、安装体积。
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tomllib
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def du(path):
    total = 0
    for root, _, files in os.walk(path):
        for name in files:
            p = os.path.join(root, name)
            if not os.path.islink(p):
                total += os.lstat(p).st_size
    return total


def isolated_env(out):
    """跑 npm（第三方程序）用的环境：家目录、缓存、配置全在 <out> 里。"""
    home = out / "install-home"
    for sub in (".config", ".cache", "tmp"):
        (home / sub).mkdir(parents=True, exist_ok=True)
    for rc in ("npmrc", "npmrc-global"):
        (home / rc).touch()
    node_bin = out / "node/bin"
    return {
        "PATH": f"{node_bin}:/usr/bin:/bin",
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_CACHE_HOME": str(home / ".cache"),
        "XDG_DATA_HOME": str(home / ".local/share"),
        "XDG_STATE_HOME": str(home / ".local/state"),
        "TMPDIR": str(home / "tmp"),
        "npm_config_cache": str(out / "npm-cache"),
        "npm_config_userconfig": str(home / "npmrc"),
        "npm_config_globalconfig": str(home / "npmrc-global"),
        "npm_config_update_notifier": "false",
        "npm_config_fund": "false",
        "npm_config_audit": "false",
        "LANG": "C.UTF-8",
    }


def install_node(lock, out):
    node = lock["node"]
    dest = out / "node"
    dl = out / "downloads"
    dl.mkdir(parents=True, exist_ok=True)
    name = node["url"].rsplit("/", 1)[1]
    tarball = dl / name
    if not tarball.exists():
        urllib.request.urlretrieve(node["url"], tarball)
    sums = urllib.request.urlopen(node["url"].rsplit("/", 1)[0] + "/SHASUMS256.txt").read().decode()
    want = next(line.split()[0] for line in sums.splitlines() if line.endswith(" " + name))
    got = sha256(tarball)
    if got != want or (node.get("sha256") and node["sha256"] != got):
        sys.exit(f"node 的 sha256 对不上：{got}，SHASUMS256 写的 {want}，锁文件写的 {node.get('sha256')}")
    if (dest / "bin/node").exists():
        return {"version": node["version"], "sha256": got, "bytes": du(dest)}
    tmp = out / "node.tmp"
    shutil.rmtree(tmp, ignore_errors=True)
    with tarfile.open(tarball) as tf:
        tf.extractall(tmp, filter="tar")
    (tmp / name.removesuffix(".tar.xz")).rename(dest)
    shutil.rmtree(tmp)
    return {"version": node["version"], "sha256": got, "bytes": du(dest)}


def install_rival(key, spec, out, env):
    """一家装进 <out>/prefix/<家>/；有几个直接装的包（dsh 那样，各自是一个全局命令）就各装一个子目录，
    和 `npm i -g` 一样互不干扰（装进同一个目录 npm 会解不开它们的依赖）。"""
    prefix = out / "prefix" / key
    log = out / "logs" / f"install-{key}.log"
    log.parent.mkdir(parents=True, exist_ok=True)
    direct = [x for x in spec["packages"] if x.get("direct")]
    groups = [(prefix / x["name"].rsplit("/", 1)[-1], x) for x in direct] or [(prefix, spec["packages"][0])]
    locks = {}
    with open(log, "a") as f:
        for where, pkg in groups:
            where.mkdir(parents=True, exist_ok=True)
            if not (where / "package-lock.json").exists():
                subprocess.run(
                    [str(out / "node/bin/npm"), "install", "--prefix", str(where), "--ignore-scripts", "--omit=dev",
                     "--no-audit", "--no-fund", f"{pkg['name']}@{pkg['version']}"],
                    env=env, cwd=where, stdout=f, stderr=subprocess.STDOUT, check=True)
            for name, entry in json.loads((where / "package-lock.json").read_text())["packages"].items():
                locks.setdefault(name, entry)
    checked = []
    for pkg in spec["packages"]:
        entry = locks.get(f"node_modules/{pkg['name']}")
        if not entry:
            sys.exit(f"{key}: package-lock 里没有 {pkg['name']}")
        if entry.get("version") != pkg["version"] or entry.get("integrity") != pkg["sha512"]:
            sys.exit(f"{key}: {pkg['name']} 对不上：装的 {entry.get('version')} {entry.get('integrity')}，"
                     f"锁的 {pkg['version']} {pkg['sha512']}")
        checked.append({"name": pkg["name"], "version": pkg["version"], "sha512": pkg["sha512"]})
    program = prefix / spec["program"]
    got = sha256(program)
    if spec.get("sha256") and spec["sha256"] != got:
        sys.exit(f"{key}: 程序 sha256 对不上：{got}，锁的 {spec['sha256']}")
    return {"name": spec["name"], "packages": checked, "program": str(program), "program_sha256": got,
            "program_bytes": program.stat().st_size, "prefix_bytes": du(prefix)}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out", required=True)
    p.add_argument("--only", action="append")
    a = p.parse_args()
    out = Path(a.out).resolve()
    lock = tomllib.loads((HERE / "rivals.lock.toml").read_text())
    result_path = out / "installed.json"
    result = json.loads(result_path.read_text()) if result_path.exists() else {}
    result["frozen"] = lock["frozen"]
    result["node"] = install_node(lock, out)
    env = isolated_env(out)
    for key, spec in lock.items():
        if not isinstance(spec, dict) or "packages" not in spec:
            continue
        if a.only and key not in a.only:
            continue
        result[key] = install_rival(key, spec, out, env)
        print(key, result[key]["program_sha256"], f"{result[key]['prefix_bytes'] / 1e6:.0f} MB", flush=True)
    result_path.write_text(json.dumps(result, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
