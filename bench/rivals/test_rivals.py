"""三家对比里不用跑别家就能查的几条（施工 V-7）：`rivals.toml` 写得对、占位都认识、汇表。

python3 -m unittest discover -s bench/rivals -p 'test_*.py'
"""

import re
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import places  # noqa: E402
import rival  # noqa: E402
import summary  # noqa: E402

# 每一家都要有的键；daemon、env、files、setup 可以没有。
REQUIRED = ("name", "programs", "marker", "tui", "resume", "task", "cont", "session", "session_files", "install")
# 测具自己在跑的时候才填的占位。
RUNTIME = {"home", "prompt", "session"}


def fake_places(root):
    programs = {name: root / "bin" / name for name in ("miyu", "tui", "jcode", "codex")}
    return places.Places(
        core=root / "core",
        tui_repo=root / "tui",
        programs=programs,
        out=root / "out",
        cpus=None,
        model="m",
        base_url="https://example.invalid/v1",
    )


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for v in value.values():
            yield from strings(v)
    elif isinstance(value, list):
        for v in value:
            yield from strings(v)


class Table(unittest.TestCase):
    def setUp(self):
        self.tables = tomllib.loads(rival.TABLE.read_text())

    def test_every_rival_has_what_the_scenes_read(self):
        self.assertEqual(list(self.tables), ["miyu", "jcode", "codex"])
        for key, table in self.tables.items():
            for field in REQUIRED:
                self.assertIn(field, table, f"[{key}] 少了 {field}")
            self.assertIn(table["session"]["id"], ("name", "stem", "tail36"), key)
            self.assertTrue(table["marker"], key)

    def test_every_placeholder_is_known(self):
        with tempfile.TemporaryDirectory() as tmp:
            known = set(fake_places(Path(tmp)).words()) | RUNTIME
        for key, table in self.tables.items():
            for text in strings(table):
                for name in re.findall(r"\{([a-z_0-9]+)\}", text):
                    self.assertIn(name, known, f"[{key}] 不认识的占位 {{{name}}}：{text}")

    def test_programs_are_ones_the_harness_finds(self):
        with tempfile.TemporaryDirectory() as tmp:
            found = fake_places(Path(tmp)).programs
        for key, table in self.tables.items():
            for name in table["programs"]:
                self.assertIn(name, found, key)

    def test_no_key_goes_into_a_file(self):
        # key 只经环境变量交给各家：表里没有 key 的占位，写到的值里只认 OPENAI_API_KEY 这一个变量名。
        values = [text for table in self.tables.values() for text in strings(table)]
        self.assertFalse([v for v in values if "{key}" in v])
        named = {word for v in values for word in re.findall(r"[A-Z_]*API_KEY[A-Z_]*", v)}
        self.assertEqual(named, {"OPENAI_API_KEY"})


class Fill(unittest.TestCase):
    def test_known_words_are_replaced_and_the_rest_left_alone(self):
        words = {"model": "m", "home": "/h"}
        self.assertEqual(places.fill("{home}/x {model}", words), "/h/x m")
        self.assertEqual(places.fill('{ env = "OPENAI_API_KEY" }', words), '{ env = "OPENAI_API_KEY" }')
        self.assertEqual(places.fill("{unknown}", words), "{unknown}")

    def test_a_rival_fills_its_argv_and_env(self):
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            jcode = rival.load(where, ["jcode"])[0]
            self.assertEqual(jcode.argv("cont", prompt="hi", session="s1")[-1], "hi")
            self.assertIn("s1", jcode.argv("cont", prompt="hi", session="s1"))
            self.assertEqual(jcode.env()["HOME"], str(where.homes / "jcode"))
            self.assertIsNone(rival.load(where, ["codex"])[0].argv("daemon"))


class Summary(unittest.TestCase):
    def test_a_table_comes_out_with_dashes_for_what_was_not_measured(self):
        conditions = {
            "date": "2026-10-15",
            "machine": {
                "cpu": "c",
                "logical_cores": 4,
                "memory_gb": 8,
                "kernel": "k",
                "arch": "x",
                "pinned_cpus": None,
            },
            "model": "m",
            "core": "abc",
            "tui": "def",
            "tui_matches_heads_toml": False,
            "versions": {"Miyu": "miyu 0.8.0", "jcode": None},
        }
        cold = {"first_screen_s": 0.1, "startup_cpu_usec": 50000, "startup_pss": 2**20}
        data = {"conditions": conditions, "scenes": {"cold": {"Miyu": {}, "jcode": {}}}}
        for metric, _label, fn in summary.SCENES[0][3]:
            data["scenes"]["cold"]["Miyu"][metric] = summary.stat([fn(cold)])
            data["scenes"]["cold"]["jcode"][metric] = None
        text = summary.markdown(data, ["Miyu", "jcode"])
        self.assertIn("| 到首屏 ms | 100（100–100） | — |", text)
        self.assertIn("不是 heads.toml 钉的那一份", text)
        self.assertIn("jcode —", text)


if __name__ == "__main__":
    unittest.main()
