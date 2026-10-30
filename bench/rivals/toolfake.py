"""工具一步的假模型（施工 V-29，蓝图 `docs/blueprint/perf.md`「工具一步」的「和 jcode 比」）：本机回环上的 OpenAI 兼容端点。

和量尺的假模型（`crates/miyu-perf/src/fake.rs`、`fake/script.rs`）同一个回法：提到记号、带工具表的请求，照请求体里
已经有前几次调用的结果，回下一次调用；回完了回一句话收尾。不带工具表的（起标题这些辅助请求）、不提记号的回一句话，
不记。记两个时刻：读全请求头的那一刻、回答连同 `[DONE]` 写完的那一刻；给了 cgroup 的，两个时刻各读一次它的
`cpu.stat`。

只用标准库。
"""

import json
import socket
import socketserver
import threading
import time

# 调用前面的一句正文：同量尺的 `fake/script.rs` 的 PREAMBLE。
PREAMBLE = "Checking."


def usage_ms(group):
    """cgroup 目录 `group` 到这时花的 CPU（毫秒，含退出了的子进程）；没给、读不到的是 None。"""
    if not group:
        return None
    try:
        with open(f"{group}/cpu.stat") as stat:
            for line in stat:
                name, _, value = line.partition(" ")
                if name == "usage_usec":
                    return int(value) / 1000.0
    except OSError:
        return None
    return None


def chunk(payload):
    return f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"


def tool_stream(call, prompt):
    """一次带工具调用的回答：正文一块、调用一块（参数一次给全）、收尾一块 `tool_calls` 带用量、`[DONE]`。"""
    said = {"choices": [{"index": 0, "delta": {"role": "assistant", "content": PREAMBLE}, "finish_reason": None}]}
    called = {"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0, "id": call["id"], "type": "function",
              "function": {"name": call["name"], "arguments": call["arguments"]}}]}, "finish_reason": None}]}
    completion = (len(PREAMBLE) + len(call["arguments"]) + 3) // 4
    done = {"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": prompt + completion}}
    return chunk(said) + chunk(called) + chunk(done) + "data: [DONE]\n\n"


def text_stream(prompt):
    """一句话：正文一块、收尾一块 `stop` 带用量、`[DONE]`。"""
    said = {"choices": [{"index": 0, "delta": {"role": "assistant", "content": PREAMBLE}, "finish_reason": None}]}
    done = {"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": prompt, "completion_tokens": 3, "total_tokens": prompt + 3}}
    return chunk(said) + chunk(done) + "data: [DONE]\n\n"


def answered(body, marker, calls):
    """请求体 `body`（字节）归不归剧本、已经有前几次调用的结果：不提记号、不带工具表的是 None。"""
    if marker.encode() not in body or b'"tools"' not in body:
        return None
    count = 0
    for call in calls:
        if call["id"].encode() not in body:
            break
        count += 1
    return count


class Fake:
    """跑着的假模型。`script(marker, calls, group)` 换剧本；`records` 是归剧本的请求，每条
    `{"step", "at", "cpu_at", "done", "cpu_done"}`（时刻是 `time.perf_counter_ns()` 折成的毫秒）。"""

    def __init__(self):
        self.lock = threading.Lock()
        self.marker = None
        self.calls = []
        self.group = None
        self.records = []
        self.models = []
        fake = self

        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                fake.serve(self.request)

        class Server(socketserver.ThreadingTCPServer):
            daemon_threads = True
            allow_reuse_address = True

        self.server = Server(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    @property
    def base_url(self):
        return f"http://127.0.0.1:{self.port}/v1"

    def script(self, marker, calls, group):
        with self.lock:
            self.marker, self.calls, self.group, self.records = marker, list(calls), group, []

    def taken(self):
        with self.lock:
            return list(self.records)

    def serve(self, conn):
        conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        data = b""
        while b"\r\n\r\n" not in data:
            piece = conn.recv(65536)
            if not piece:
                return
            data += piece
        at = time.perf_counter_ns() / 1e6
        head, _, body = data.partition(b"\r\n\r\n")
        lines = head.decode("latin-1").split("\r\n")
        method = lines[0].split(" ")[0]
        length = 0
        for line in lines[1:]:
            name, _, value = line.partition(":")
            if name.strip().lower() == "content-length":
                length = int(value.strip())
        while len(body) < length:
            piece = conn.recv(65536)
            if not piece:
                return
            body += piece
        with self.lock:
            marker, calls, group = self.marker, self.calls, self.group
        step = answered(body, marker, calls) if marker else None
        cpu_at = usage_ms(group) if step is not None else None
        if method != "POST":
            listed = json.dumps({"object": "list", "data": [{"id": "perf-model", "object": "model"}]}).encode()
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n"
                         + f"Content-Length: {len(listed)}\r\n\r\n".encode() + listed)
            conn.close()
            return
        prompt = (len(body) + 3) // 4
        stream = tool_stream(calls[step], prompt) if step is not None and step < len(calls) else text_stream(prompt)
        conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                     + stream.encode())
        done = time.perf_counter_ns() / 1e6
        cpu_done = usage_ms(group) if step is not None else None
        try:
            conn.shutdown(socket.SHUT_WR)
        except OSError:
            pass
        conn.close()
        if step is not None:
            with self.lock:
                self.records.append({"step": step, "at": at, "cpu_at": cpu_at, "done": done, "cpu_done": cpu_done})
