"""三家对比里不用跑别家就能查的几条（施工 V-7）：`rivals.toml` 写得对、占位都认识、汇表。

python3 -m unittest discover -s bench/rivals -p 'test_*.py'
"""

import argparse
import http.server
import io
import json
import os
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import types
import urllib.request
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

# CI 的量尺那一项照 `python3 -m unittest` 跑、没装 pyte：`homes` 经 `term` 引它，这里只测不开伪终端的几条，先占个空的。
sys.modules.setdefault("pyte", types.ModuleType("pyte"))

import disk  # noqa: E402
import homes  # noqa: E402
import places  # noqa: E402
import reap  # noqa: E402
import relay  # noqa: E402
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


class Missing(unittest.TestCase):
    def test_a_rival_that_is_not_installed_does_not_stop_the_others(self):
        # 没装 jcode 的机器上 `--only Miyu`：jcode 找不到是 None，换占位时不能拿它算目录（V-7 修）。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            where.programs["jcode"] = None
            miyu = rival.load(where, ["Miyu"])[0]
            self.assertEqual(miyu.argv("task", prompt="hi")[-1], "hi")
            self.assertEqual(miyu.env()["MIYU_HOME"], str(where.homes / "miyu"))


class Layout(unittest.TestCase):
    """跟着核心和终端现在的样子（施工 V-27）：终端包在 `miyu-tui/`、会话在数据根的 `sessions/<编号>/`。"""

    def test_the_tui_package_is_the_folder_named_after_its_id(self):
        # 施工 PN-4 起终端仓库里的包叫 `miyu-tui/`（以前 `tui/`）：`miyu pkg install` 装的是它。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            self.assertEqual(where.words()["tui_package"], str(where.tui_repo / "miyu-tui"))

    def test_load_takes_a_tui_repo_with_miyu_tui_and_refuses_the_old_layout(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "core/target/release").mkdir(parents=True)
            (root / "core/target/release/miyu").write_text("")
            (root / "tui/target/release").mkdir(parents=True)
            (root / "tui/target/release/miyu-tui").write_text("")
            (root / "tui/tui").mkdir()
            (root / "tui/tui/package.toml").write_text("")
            args = argparse.Namespace(core=str(root / "core"), tui=str(root / "tui"), jcode=None, codex=None,
                                      out=str(root / "out"), cpus=None)
            env = {places.BASE_URL: "https://example.invalid/v1", places.KEY: "k", places.MODEL: "m"}
            with mock.patch.dict(os.environ, env), mock.patch("sys.stderr", io.StringIO()) as err:
                with self.assertRaises(SystemExit):
                    places.load(args)
                self.assertIn("miyu-tui/package.toml", err.getvalue())
                (root / "tui/miyu-tui").mkdir()
                (root / "tui/miyu-tui/package.toml").write_text("")
                self.assertEqual(places.load(args).words()["tui_package"], str(root / "tui/miyu-tui"))

    def test_miyu_finds_the_newest_session_under_the_data_root(self):
        # 单人的：会话在数据根（MIYU_HOME）的 `sessions/<编号>/`，不再在 `home/admin/sessions/` 下（`store.md`）。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            miyu = rival.load(where, ["miyu"])[0]
            for name, at in (("old", 1_000_000), ("new", 2_000_000)):
                path = miyu.home / "sessions" / name
                path.mkdir(parents=True)
                (path / "log.jsonl").write_text("{}")
                os.utime(path, (at, at))
            self.assertEqual(miyu.session_id(), "new")
            self.assertEqual(miyu.session_files(miyu.home, "new"), [miyu.home / "sessions/new"])

    def test_markers_are_filled_and_miyu_waits_for_the_model_name(self):
        # Miyu 的输入框照缓存先画、那时还没连上核心：首屏等状态行报出模型名（施工 V-27）。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            miyu, jcode = rival.load(where, ["miyu", "jcode"])
            self.assertEqual(miyu.marker, [where.model])
            self.assertFalse([m for r in (miyu, jcode) for m in r.marker if "{" in m])

    def test_jcode_finds_its_session_by_the_file_stem(self):
        # v0.99.1 照旧写 `.jcode/sessions/<编号>.json`，旁边一份 `.bak`：编号是 json 的文件名去掉后缀，两份都算会话的文件。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            jcode = rival.load(where, ["jcode"])[0]
            sessions = jcode.home / ".jcode/sessions"
            sessions.mkdir(parents=True)
            (sessions / "session_hog_1_ab.json").write_text("{}")
            (sessions / "session_hog_1_ab.bak").write_text("{}")
            self.assertEqual(jcode.session_id(), "session_hog_1_ab")
            self.assertEqual(len(jcode.session_files(jcode.home, "session_hog_1_ab")), 2)


class Template(unittest.TestCase):
    def test_a_stale_socket_does_not_stop_saving_the_template(self):
        # 后台进程收掉时没删的套接字（Miyu 的 `run/core.sock`）拷不了，模板里也不该有（施工 V-27）。
        with tempfile.TemporaryDirectory() as tmp, socket.socket(socket.AF_UNIX) as sock:
            where = fake_places(Path(tmp))
            miyu = rival.load(where, ["miyu"])[0]
            (miyu.home / "run").mkdir(parents=True)
            (miyu.home / "run/token").write_text("t")
            sock.bind(str(miyu.home / "run/core.sock"))
            with mock.patch.object(homes.reap, "reap"):
                dst = homes.keep(miyu, [miyu])
            self.assertEqual(sorted(p.name for p in (dst / "run").iterdir()), ["token"])


class Relay(unittest.TestCase):
    """本机转发（施工 V-27）：真 key 只在转发里，会话头同一段对话稳定，用量记下来。"""

    def test_a_conversation_keeps_its_session_header_and_scenes_rivals_split_it(self):
        def body(*users):
            msgs = [{"role": "system", "content": "s"}] + [{"role": "user", "content": u} for u in users]
            return json.dumps({"messages": msgs}).encode()

        first = relay.session_id("jcode", "task-0", body("fix it"))
        self.assertTrue(first.startswith("ses_"))
        # 同一段对话往后接着说：第一句没变，头不变。
        self.assertEqual(relay.session_id("jcode", "task-0", body("fix it", "go on")), first)
        self.assertNotEqual(relay.session_id("jcode", "task-1", body("fix it")), first)
        self.assertNotEqual(relay.session_id("miyu", "task-0", body("fix it")), first)
        self.assertNotEqual(relay.session_id("jcode", "task-0", body("other")), first)
        self.assertTrue(relay.session_id("jcode", "", b"not json").startswith("ses_"))

    def test_usage_is_picked_from_the_stream(self):
        sse = relay.SseUsage()
        chunk = {"choices": [], "usage": {"prompt_tokens": 9, "completion_tokens": 2,
                                          "prompt_tokens_details": {"cached_tokens": 4}}}
        text = b'data: {"choices":[{"delta":{"content":"ok"}}]}\n\ndata: ' + json.dumps(chunk).encode()
        sse.feed(text[:30])
        sse.feed(text[30:] + b"\n\ndata: [DONE]\n\n")
        self.assertEqual(sse.usage, {"input": 9, "cached": 4, "output": 2, "reasoning": 0})

    def test_the_relay_swaps_the_key_adds_the_session_header_and_logs_usage(self):
        seen = []

        class Upstream(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers["content-length"]))
                seen.append((self.path, dict(self.headers)))
                data = json.dumps({"usage": {"prompt_tokens": 5, "completion_tokens": 1}}).encode()
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        upstream = http.server.HTTPServer(("127.0.0.1", 0), Upstream)
        threading.Thread(target=upstream.serve_forever, daemon=True).start()
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / "relay.jsonl"
            state = relay.Relay(f"http://127.0.0.1:{upstream.server_address[1]}/zen/go/v1", "real-key", str(log))
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), relay.handler(state))
            threading.Thread(target=server.serve_forever, daemon=True).start()
            base = f"http://127.0.0.1:{server.server_address[1]}"
            where = types.SimpleNamespace(relay=base)
            relay.mark(where, "jcode", "task-0")
            req = urllib.request.Request(
                f"{base}/jcode/v1/chat/completions",
                data=json.dumps({"model": "m", "messages": [{"role": "user", "content": "hi"}]}).encode(),
                headers={"Authorization": f"Bearer {places.FAKE_KEY}", "content-type": "application/json"},
            )
            with urllib.request.urlopen(req) as response:
                self.assertEqual(json.loads(response.read())["usage"]["prompt_tokens"], 5)
            path, headers = seen[0]
            self.assertEqual(path, "/zen/go/v1/chat/completions")
            self.assertEqual(headers["Authorization"], "Bearer real-key")
            self.assertTrue(headers[relay.SESSION_HEADER].startswith("ses_"))
            entry = json.loads(log.read_text())
            self.assertEqual((entry["rival"], entry["scene"], entry["status"]), ("jcode", "task-0", 200))
            self.assertEqual(entry["usage"]["input"], 5)
            self.assertNotIn("real-key", log.read_text())
            server.shutdown()
        upstream.shutdown()

    def test_with_the_relay_each_rival_gets_its_own_path_and_only_a_fake_key(self):
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            where.relay, where.upstream_key_env = "http://127.0.0.1:9", "REAL_UPSTREAM_KEY"
            with mock.patch.dict(os.environ, {"REAL_UPSTREAM_KEY": "secret"}):
                for r in rival.load(where, ["miyu", "jcode"]):
                    env = r.env()
                    self.assertEqual(env[places.BASE_URL], f"http://127.0.0.1:9/{r.key}/v1")
                    self.assertEqual(env[places.KEY], places.FAKE_KEY)
                    self.assertNotIn("REAL_UPSTREAM_KEY", env)
                    self.assertNotIn("secret", json.dumps(env))
                jcode = rival.load(where, ["jcode"])[0]
                self.assertEqual(jcode.env()["JCODE_OPENAI_COMPAT_API_BASE"], "http://127.0.0.1:9/jcode/v1")


class Disk(unittest.TestCase):
    def test_run_reads_both_ways_of_the_home(self):
        # 模板（好好退出以后）和做模板时记下的「直接杀掉」那一份都进 disk.json，缓存照样不算（施工 V-27 跑出来的错：
        # 局部变量盖住了 `cached`）。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            miyu = rival.load(where, ["miyu"])[0]
            home = where.templates / "miyu"
            (home / "sessions/s").mkdir(parents=True)
            (home / "sessions/s/log.jsonl").write_text("{}")
            where.results.mkdir(parents=True)
            crash = {"sessions/s/log.jsonl": 4096, "state/usage.db-wal": 8192, ".cache/miyu/x": 99999}
            (where.results / "home-crash-miyu.json").write_text(json.dumps(crash))
            disk.run(where, [miyu])
            out = json.loads((where.results / "disk.json").read_text())
            self.assertEqual(out["home_after_first_run_crash"]["Miyu"], 4096 + 8192)
            self.assertNotIn(".cache/miyu/x", out["home_files_crash"]["Miyu"])
            self.assertIn("sessions/s/log.jsonl", out["home_files"]["Miyu"])

    def test_caches_are_counted_apart_from_the_data_and_the_data_is_listed_by_file(self):
        # 2026-10-31 项目主人定：复用的缓存单列，不算进「跑过一次多了多少」（施工 V-27）。
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            (home / ".cache/miyu/models").mkdir(parents=True)
            (home / ".cache/miyu/models/models-dev.json").write_bytes(b"x" * 50000)
            (home / "sessions/s1").mkdir(parents=True)
            (home / "sessions/s1/log.jsonl").write_text("{}")
            cached, data = disk.split(home, [".cache"])
            self.assertEqual([f.name for f in cached], ["models-dev.json"])
            self.assertEqual([f.relative_to(home).as_posix() for f in data], ["sessions/s1/log.jsonl"])
            # 做模板时记下的「直接杀掉」那一份照相对路径分。
            self.assertTrue(disk.cached(".jcode/cache/x.json", [".jcode/cache"]))
            self.assertTrue(disk.cached(".config/jcode/remote_model_catalog_cache.json", [".config/jcode/remote_model_catalog_cache.json"]))
            self.assertFalse(disk.cached(".jcode/cachex/y", [".jcode/cache"]))
            self.assertFalse(disk.cached("state/usage.db-wal", [".cache"]))


class Shut(unittest.TestCase):
    """好好退出（施工 V-27）：先跑自己的退出命令，再 SIGTERM 等它收尾，不走的才硬杀。"""

    def spawn(self, home, script):
        """起一个照 `MIYU_HOME` 认得出的进程，等它装好信号处理（写出 `ready`）再交回。光等进程列表里看得到不够：解释器一起来
        就看得到，那时 `script` 还没跑到装处理的那一行，慢的机器上 SIGTERM 先到、照默认的死掉（V-27 修：CI 上两条红过）。"""
        env = {**os.environ, "MIYU_HOME": str(home)}
        ready = home / "ready"
        script = script.replace("#READY", f"pathlib.Path({str(ready)!r}).write_text('')")
        proc = subprocess.Popen([sys.executable, "-c", script], env=env)
        deadline = time.monotonic() + 30
        while not (ready.exists() and reap.leftover([home])):
            self.assertIsNone(proc.poll(), "子进程没装好就退了")
            self.assertLess(time.monotonic(), deadline, "30 秒里没装好")
            time.sleep(0.02)
        return proc

    def test_a_process_that_cleans_up_on_sigterm_is_left_to_finish(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            done = home / "checkpointed"
            script = (
                "import signal,sys,time,pathlib\n"
                f"def bye(*a):\n time.sleep(0.3); pathlib.Path({str(done)!r}).write_text('ok'); sys.exit(0)\n"
                "signal.signal(signal.SIGTERM, bye)\n#READY\nwhile True: time.sleep(0.05)\n"
            )
            proc = self.spawn(home, script)
            stop_ran = home / "stop-ran"
            result = reap.shut([home], ["touch", str(stop_ran)], dict(os.environ), limit=5)
            proc.wait(timeout=5)
            self.assertEqual(result["forced"], 0)
            self.assertTrue(done.exists())
            self.assertTrue(stop_ran.exists())

    def test_one_that_ignores_sigterm_is_killed_after_the_limit(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            script = "import pathlib,signal,time\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\n#READY\ntime.sleep(60)\n"
            proc = self.spawn(home, script)
            result = reap.shut([home], limit=0.5)
            proc.wait(timeout=5)
            self.assertEqual(result["forced"], 1)


class Sandbox(unittest.TestCase):
    def test_caches_stay_inside_each_home(self):
        # Miyu 的缓存跟 XDG_CACHE_HOME 走、不跟 MIYU_HOME 走（store.md）：不指进沙箱，就读写本机真实的缓存（V-7 修）。
        with tempfile.TemporaryDirectory() as tmp:
            where = fake_places(Path(tmp))
            for r in rival.load(where):
                env = r.env()
                cache = env.get("XDG_CACHE_HOME") or env.get("CODEX_HOME")
                self.assertTrue(cache and cache.startswith(str(r.home)), f"{r.name} 的缓存不在沙箱里：{cache}")


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
        data["disk"] = {
            "install": {"Miyu": {"a": 2**20}, "jcode": {"b": 2**21}},
            "home_after_first_run": {"Miyu": 4096, "jcode": 8192},
            "home_caches": {"Miyu": 2**20, "jcode": 0},
            "home_files": {"Miyu": {"sessions/s/log.jsonl": 4096}, "jcode": {}},
            "home_after_first_run_crash": {"Miyu": 4096 + 94208},
            "home_files_crash": {"Miyu": {"sessions/s/log.jsonl": 4096, "state/usage.db-wal": 94208}},
            "long_session": {},
        }
        data["tokens"] = {"Miyu": {"task": {"requests": 3, "errors": 1, "input": 900, "cached": 600, "output": 40}}}
        text = summary.markdown(data, ["Miyu", "jcode"])
        self.assertIn("| 到首屏 ms | 100（100–100） | — |", text)
        self.assertIn("| 会一直复用的缓存 KiB（模型目录、图标这类，不算在上一行） | 1024 | 0.0 |", text)
        self.assertIn("| `sessions/s/log.jsonl` | 4.0 | 4.0 |", text)
        self.assertIn("| `state/usage.db-wal` | — | 92.0 |", text)
        self.assertIn("| 家目录跑过一次的数据 KiB（直接杀掉，崩溃、断电剩下的） | 96.0 | — |", text)
        self.assertIn("| 一轮真任务 | 3（1）；900（600）；40 |\n", text)
        self.assertIn("不是 heads.toml 钉的那一份", text)
        self.assertIn("jcode —", text)


if __name__ == "__main__":
    unittest.main()
