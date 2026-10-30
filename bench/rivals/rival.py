"""一家被测的程序：照 `rivals.toml` 的一张表，换好占位（施工 V-7）。"""

import subprocess
import tomllib
from pathlib import Path

import places

TABLE = Path(__file__).with_name("rivals.toml")


class Rival:
    """一家。`argv` 系列交回换好占位的命令；`env()` 是起它用的环境；`setup()` 在空的沙箱家目录里配好它。"""

    def __init__(self, key, table, where):
        self.key = key
        self.name = table["name"]
        self.table = table
        self.where = where
        self.home = where.homes / self.key
        # 首屏的字也换占位：Miyu 的是模型名（`{model}`）。
        self.marker = [places.fill(m, where.words({"home": str(self.home)})) for m in table["marker"]]

    def base_url(self):
        """这一家连的地址：用转发的是 `<转发>/<表名>/v1`（转发照路径分出是哪一家），不然照环境变量。"""
        return f"{self.where.relay}/{self.key}/v1" if self.where.relay else self.where.base_url

    def words(self, **extra):
        return self.where.words({"home": str(self.home), "base_url": self.base_url(), **extra})

    def argv(self, kind, **extra):
        """`tui`、`resume`、`task`、`cont`、`daemon` 之一；这一家没有的交回 None。"""
        if kind not in self.table:
            return None
        words = self.words(**extra)
        return [places.fill(part, words) for part in self.table[kind]]

    def env(self):
        env = places.base_env(strip=(self.where.upstream_key_env,) if self.where.upstream_key_env else ())
        if self.where.relay:
            # Miyu 的配置写的是 `{ env = "OPENAI_BASE_URL" }`、`{ env = "OPENAI_API_KEY" }`：照家换成转发、假 key。
            env[places.BASE_URL] = self.base_url()
            env[places.KEY] = places.FAKE_KEY
        words = self.words()
        env.update({k: places.fill(v, words) for k, v in self.table.get("env", {}).items()})
        return env

    def setup(self):
        """在空的家目录里配好：先写文件，再照顺序跑命令（命令失败就停）。"""
        self.home.mkdir(parents=True, exist_ok=True)
        words = self.words()
        for item in self.table.get("files", []):
            path = self.home / item["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(places.fill(item["text"], words).lstrip("\n"))
        for argv in self.table.get("setup", {}).get("run", []):
            argv = [places.fill(part, words) for part in argv]
            done = subprocess.run(
                places.BARE + argv,
                env=self.env(),
                cwd=str(self.home),
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if done.returncode != 0:
                raise RuntimeError(f"{self.name}：配置那一步没成（{argv[1:4]}）：{done.stderr[-400:]}")

    def session_id(self, home=None):
        """长会话的编号：照 `session` 的 glob 找最新的那一个。"""
        rule = self.table["session"]
        found = list((home or self.home).glob(rule["glob"]))
        if not found:
            raise RuntimeError(f"{self.name}：找不到会话（{rule['glob']}）")
        newest = max(found, key=lambda p: p.stat().st_mtime)
        return {"name": newest.name, "stem": newest.stem, "tail36": newest.stem[-36:]}[rule["id"]]

    def session_files(self, home, session):
        words = self.words(session=session)
        out = []
        for pattern in self.table["session_files"]:
            out.extend(home.glob(places.fill(pattern, words)))
        return out

    def install(self):
        """装好占多大要数的那几样：[(说明, 路径)]。"""
        words = self.words()
        return [(label, Path(places.fill(path, words)).resolve()) for label, path in self.table["install"]]

    def version(self):
        """程序自己报的版本（第一个程序的 --version）。"""
        program = self.where.programs[self.table["programs"][0]]
        done = subprocess.run([str(program), "--version"], env=self.env(), capture_output=True, text=True, timeout=30)
        return (done.stdout or done.stderr).strip().splitlines()[0] if done.returncode == 0 else None


def load(where, only=()):
    """照 `rivals.toml` 的先后交回各家；`only` 不空的只要这几家（照表名或显示名）。"""
    tables = tomllib.loads(TABLE.read_text())
    rivals = [Rival(key, table, where) for key, table in tables.items()]
    if only:
        rivals = [r for r in rivals if r.key in only or r.name in only]
    for r in rivals:
        for name in r.table["programs"]:
            if where.programs.get(name) is None:
                places.fail(f"找不到 {name}：用 --{name} 指给它，或者用 --only 只量别的几家")
    return rivals
