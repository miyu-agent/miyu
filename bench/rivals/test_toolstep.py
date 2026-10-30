"""工具一步和 jcode 比（施工 V-29）不跑别家的那几条：剧本怎么认、怎么回，每一步怎么算，两家的调用都对得上。"""

import json
import socket
import tempfile
import time
import unittest
from pathlib import Path

import toolfake
import toolkinds
import toolstep

CALLS = [{"id": f"perf_05_{n:03d}", "name": "read", "arguments": json.dumps({"file_path": f"/w/{n}"})}
         for n in range(3)]


class Script(unittest.TestCase):
    def test_only_marked_requests_with_tools_count(self):
        self.assertIsNone(toolfake.answered(b'{"messages":"perf-tool-05"}', "perf-tool-05", CALLS))
        self.assertIsNone(toolfake.answered(b'{"tools":[],"messages":"perf-tool-06"}', "perf-tool-05", CALLS))
        self.assertEqual(toolfake.answered(b'{"tools":[],"m":"perf-tool-05"}', "perf-tool-05", CALLS), 0)
        body = b'{"tools":[],"m":"perf-tool-05 perf_05_000 perf_05_001"}'
        self.assertEqual(toolfake.answered(body, "perf-tool-05", CALLS), 2)

    def test_a_tool_call_is_streamed_then_done(self):
        stream = toolfake.tool_stream(CALLS[1], 100)
        chunks = [json.loads(c[len("data: "):]) for c in stream.split("\n\n") if c.startswith("data: {")]
        self.assertTrue(stream.endswith("data: [DONE]\n\n"))
        call = chunks[1]["choices"][0]["delta"]["tool_calls"][0]
        self.assertEqual((call["id"], call["function"]["name"]), ("perf_05_001", "read"))
        self.assertEqual(chunks[2]["choices"][0]["finish_reason"], "tool_calls")
        self.assertIn('"finish_reason": "stop"', toolfake.text_stream(10))

    def test_the_fake_records_a_scripted_request(self):
        fake = toolfake.Fake()
        fake.script("perf-tool-05", CALLS, None)
        body = b'{"tools":[],"m":"perf-tool-05 perf_05_000"}'
        with socket.create_connection(("127.0.0.1", fake.port)) as conn:
            conn.sendall(b"POST /v1/chat/completions HTTP/1.1\r\nContent-Length: %d\r\n\r\n" % len(body) + body)
            reply = b""
            while piece := conn.recv(65536):
                reply += piece
        self.assertIn(b"perf_05_001", reply)
        for _ in range(100):
            if fake.taken():
                break
            time.sleep(0.01)
        [record] = fake.taken()
        self.assertEqual(record["step"], 1)
        self.assertGreaterEqual(record["done"], record["at"])
        fake.server.shutdown()


class Steps(unittest.TestCase):
    def test_each_step_runs_from_done_to_the_next_request(self):
        records = [{"step": n, "at": 10.0 * n, "done": 10.0 * n + 1, "cpu_at": float(n), "cpu_done": n + 0.5}
                   for n in range(4)]
        steps = toolstep.steps_of(records, 2)
        # 头一次（第 0 步）不算：第 1、2 次调用。
        self.assertEqual([s["ms"] for s in steps], [9.0, 9.0])
        self.assertEqual([s["cpu_ms"] for s in steps], [0.5, 0.5])
        self.assertEqual(toolstep.spread([3.0, 1.0, 2.0]), {"p50": 2.0, "p95": 3.0})


class Kinds(unittest.TestCase):
    def test_both_rivals_get_arguments_for_every_kind(self):
        with tempfile.TemporaryDirectory() as dir:
            ws = toolkinds.Workspace(Path(dir))
            for kind in toolkinds.KINDS:
                self.assertIsInstance(kind.miyu(ws, 1), dict, kind.name)
                if kind.jcode_tool:
                    self.assertIsInstance(kind.jcode(ws, 1), dict, kind.name)
                    self.assertIn("intent", kind.jcode(ws, 1), kind.name)
            edit = next(k for k in toolkinds.KINDS if k.miyu_tool == "edit")
            self.assertEqual(edit.miyu(ws, 3)["edits"][0]["old_string"], "value-3\n")
            self.assertEqual(edit.jcode(ws, 3)["new_string"], "value-4\n")

    def test_the_kinds_follow_the_measuring_tool(self):
        # 和量尺的 `calls.rs` 一一对上：名字、先后都一样。
        calls = (Path(__file__).resolve().parents[2] / "crates/miyu-perf/src/measure/tools/calls.rs").read_text()
        names = [kind.name for kind in toolkinds.KINDS]
        positions = [calls.find(f'"{name}"') for name in names]
        self.assertNotIn(-1, positions, names)
        self.assertEqual(positions, sorted(positions))


if __name__ == "__main__":
    unittest.main()
