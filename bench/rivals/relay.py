"""本机转发（施工 V-27，蓝图 `docs/blueprint/perf.md`「三家对比」）：各家都连它，它换上真的 key、补上会话头，再转给上游。

- 各家连 `http://127.0.0.1:<端口>/<表名>/v1/...`：路径里的表名分出是哪一家，转发时换成上游的路径。
- key 只在这个进程的环境里（变量名照 `--key-env`）：各家家目录、环境里都是假 key，跑完也搜不到真的。
- 会话头 `x-opencode-session`：opencode 要每段对话带一个稳定的（不带回 400 MissingSessionID）。客户端自己带了的原样转；
  没带的照「这一家、这一场、这段对话的第一句用户的话」算一个：同一段对话的每一次请求都一样，换一场（测具在每一轮开头
  `mark` 一下）、换一段对话就换。两家一样补，谁也不占便宜。
- 每个请求记一行 JSONL（`--log`）：哪一家、哪一场、模型、状态码、流不流、用了多久、用量（输入、其中缓存命中、输出、
  推理）。不记请求体、回应体、key。
- 控制：`POST /_scene/<表名>`，请求体是场的名字（例如 `task-3`）。

    python3 relay.py --upstream https://opencode.ai/zen/go/v1 --key-env OPENCODE_GO_API_KEY --log relay.jsonl

起来以后第一行往标准输出写 `listening <端口>`（`--port 0` 由系统挑）。只用标准库。
"""

import argparse
import hashlib
import http.client
import json
import os
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

# 会话头的名字（opencode 的约定）。
SESSION_HEADER = "x-opencode-session"
# 不往上游转的头：逐跳的、要换掉的（key、会话头另补）、要自己算的。压缩关掉，流里的用量才读得出来。
DROPPED = {
    "host",
    "authorization",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "accept-encoding",
    "proxy-connection",
    "te",
    "upgrade",
}
# 不往回转的头：逐跳的、长度（流着回的不定长，照关连接收尾）。
DROPPED_BACK = {"transfer-encoding", "connection", "keep-alive", "content-length", "content-encoding"}


def session_id(rival, scene, body):
    """没带会话头的请求补一个：同一家、同一场、第一句用户的话一样的，就是同一段对话。读不懂的请求体照空的算。"""
    first = ""
    try:
        for message in json.loads(body or b"{}").get("messages", []):
            if message.get("role") == "user":
                content = message.get("content")
                first = content if isinstance(content, str) else json.dumps(content, sort_keys=True)
                break
    except (ValueError, AttributeError):
        pass
    digest = hashlib.sha256(f"{rival}\0{scene}\0{first}".encode()).hexdigest()[:24]
    return f"ses_{digest}"


def usage_of(obj):
    """一个回应（或流里的一块）里的用量，统一成 input、cached、output、reasoning；没有的交回 None。"""
    usage = obj.get("usage") if isinstance(obj, dict) else None
    if not usage:
        return None
    details = usage.get("prompt_tokens_details") or {}
    out_details = usage.get("completion_tokens_details") or {}
    return {
        "input": usage.get("prompt_tokens") or 0,
        # 缓存命中各家写法不一：OpenAI 的 `prompt_tokens_details`、DeepSeek 的 `prompt_cache_hit_tokens`、opencode 自己的
        # `opencode.tokens.cache_read`。
        "cached": details.get("cached_tokens")
        or usage.get("prompt_cache_hit_tokens")
        or ((usage.get("opencode") or {}).get("tokens") or {}).get("cache_read")
        or 0,
        "output": usage.get("completion_tokens") or 0,
        "reasoning": out_details.get("reasoning_tokens") or 0,
    }


class SseUsage:
    """一边转一边从 SSE 里捡用量：照行切，`data: {…}` 里带 `usage` 的取最后一个。"""

    def __init__(self):
        self.pending = b""
        self.usage = None

    def feed(self, chunk):
        self.pending += chunk
        *lines, self.pending = self.pending.split(b"\n")
        for line in lines:
            line = line.strip()
            if not line.startswith(b"data:"):
                continue
            data = line[5:].strip()
            if not data or data == b"[DONE]":
                continue
            try:
                found = usage_of(json.loads(data))
            except ValueError:
                continue
            if found:
                self.usage = found


class Relay:
    """转发的状态：上游、key、每一家现在是哪一场、日志。"""

    def __init__(self, upstream, key, log):
        self.upstream = urlsplit(upstream.rstrip("/"))
        self.key = key
        self.log = log
        self.scenes = {}
        self.lock = threading.Lock()

    def record(self, entry):
        with self.lock:
            with open(self.log, "a") as f:
                f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    def connect(self):
        host, port = self.upstream.hostname, self.upstream.port
        if self.upstream.scheme == "https":
            return http.client.HTTPSConnection(host, port or 443, timeout=900)
        return http.client.HTTPConnection(host, port or 80, timeout=900)


def handler(relay):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt, *args):
            pass

        def body(self):
            if self.headers.get("transfer-encoding", "").lower() == "chunked":
                out = b""
                while True:
                    size = int(self.rfile.readline().strip().split(b";")[0], 16)
                    if size == 0:
                        self.rfile.readline()
                        return out
                    out += self.rfile.read(size)
                    self.rfile.readline()
            return self.rfile.read(int(self.headers.get("content-length") or 0))

        def reply(self, code, text):
            data = text.encode()
            self.send_response(code)
            self.send_header("content-type", "text/plain; charset=utf-8")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            self.forward("GET")

        def do_POST(self):
            parts = self.path.split("/")
            if len(parts) == 3 and parts[1] == "_scene":
                relay.scenes[parts[2]] = self.body().decode().strip()
                self.reply(200, "ok")
                return
            self.forward("POST")

        def forward(self, method):
            # /<表名>/v1/<其余> → 上游路径 + /<其余>
            parts = self.path.split("/", 3)
            if len(parts) < 4 or parts[2] != "v1":
                self.reply(404, "want /<rival>/v1/...")
                return
            rival, rest = parts[1], "/" + parts[3]
            scene = relay.scenes.get(rival, "")
            body = self.body() if method == "POST" else b""
            headers = {k: v for k, v in self.headers.items() if k.lower() not in DROPPED}
            headers["Authorization"] = f"Bearer {relay.key}"
            headers["Accept-Encoding"] = "identity"
            own_session = any(k.lower() == SESSION_HEADER for k in headers)
            if method == "POST" and not own_session:
                headers[SESSION_HEADER] = session_id(rival, scene, body)
            try:
                request = json.loads(body) if body else {}
            except ValueError:
                request = {}
            entry = {
                "at": time.time(),
                "rival": rival,
                "scene": scene,
                "method": method,
                "path": rest.split("?")[0],
                "model": request.get("model"),
                "stream": bool(request.get("stream")),
                # 会话头是客户端自己带的（不是转发补的）。
                "own_session": own_session,
            }
            started = time.monotonic()
            conn = relay.connect()
            try:
                conn.request(method, relay.upstream.path + rest, body=body or None, headers=headers)
                response = conn.getresponse()
            except OSError as error:
                entry.update(status=0, error=type(error).__name__, seconds=time.monotonic() - started)
                relay.record(entry)
                self.reply(502, f"relay: upstream unreachable ({type(error).__name__})")
                return
            self.send_response(response.status)
            for k, v in response.getheaders():
                if k.lower() not in DROPPED_BACK:
                    self.send_header(k, v)
            # 不定长：照关连接收尾，流着回的也一样（客户端照 HTTP/1.1 认）。
            self.send_header("connection", "close")
            self.end_headers()
            self.close_connection = True
            sse = SseUsage()
            whole = b""
            streaming = "text/event-stream" in (response.getheader("content-type") or "")
            try:
                while chunk := response.read1(65536):
                    self.wfile.write(chunk)
                    self.wfile.flush()
                    if streaming:
                        sse.feed(chunk)
                    elif len(whole) < 4 * 2**20:
                        whole += chunk
            except OSError:
                entry["client_gone"] = True
            finally:
                conn.close()
            usage = sse.usage
            if not streaming and whole:
                try:
                    usage = usage_of(json.loads(whole))
                except ValueError:
                    usage = None
            entry.update(status=response.status, seconds=time.monotonic() - started, usage=usage)
            if response.status >= 400 and not streaming:
                # 出错的回应只记一小段说明（不会有 key：key 只在请求头里）。
                entry["error"] = whole[:300].decode(errors="replace")
            relay.record(entry)

    return Handler


def start(where, upstream):
    """起转发（真 key 只在它的环境里），等它报端口，填进 `where.relay`；测具退出时收掉。交回进程。"""
    import atexit
    import subprocess

    where.results.mkdir(parents=True, exist_ok=True)
    proc = subprocess.Popen(
        [
            sys.executable,
            "-I",
            __file__,
            "--upstream",
            upstream,
            "--key-env",
            where.upstream_key_env,
            "--log",
            str(where.results / "relay.jsonl"),
        ],
        stdout=subprocess.PIPE,
        stdin=subprocess.DEVNULL,
        text=True,
    )
    atexit.register(proc.terminate)
    line = proc.stdout.readline().split()
    if len(line) != 2 or line[0] != "listening":
        proc.terminate()
        raise RuntimeError("转发没起来")
    where.relay = f"http://127.0.0.1:{line[1]}"
    return proc


def mark(where, rival, scene):
    """测具在每一轮开头告诉转发现在是哪一场（会话头、用量照它分）；没用转发的什么也不做。"""
    if not where.relay:
        return
    request = urllib.request.Request(f"{where.relay}/_scene/{rival}", data=scene.encode(), method="POST")
    with urllib.request.urlopen(request, timeout=5) as response:
        response.read()


def main(argv):
    p = argparse.ArgumentParser(prog="relay", description="三家对比的本机转发")
    p.add_argument("--upstream", required=True, help="上游的地址，到 /v1 为止")
    p.add_argument("--key-env", required=True, help="真 key 在哪个环境变量里")
    p.add_argument("--log", required=True, help="每个请求记一行 JSONL")
    p.add_argument("--port", type=int, default=0)
    args = p.parse_args(argv)
    key = os.environ.get(args.key_env)
    if not key:
        print(f"relay: 没设环境变量 {args.key_env}", file=sys.stderr)
        raise SystemExit(2)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), handler(Relay(args.upstream, key, args.log)))
    server.daemon_threads = True
    print(f"listening {server.server_address[1]}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main(sys.argv[1:])
