"""发版前对比（施工 BM-1，量法设计第五节）的假模型：一个端口说三种话，照剧本回，记下每个请求的时刻。

只用标准库。各家连 `http://127.0.0.1:<端口>/<家名>/...`：路径第一段分家，后缀认形状——
`.../chat/completions` 是 OpenAI chat（SSE 块 + `[DONE]`）；`.../responses` 是 OpenAI Responses；`.../messages` 是
Anthropic（`.../count_tokens` 回个数）；`GET .../models` 回模型表。

剧本（JSON）：
    {"marker": "BMGO1", "ttft_ms": 300, "gap_ms": 20,
     "steps": [{"tool": "<抽象名>", "args": {...}} | {"text": "...", "chunks": 8} | {"pieces": ["BMFIRST ", ..., "BMLAST"]}]}
抽象工具名（shell、read、edit、write、sleep）照 `TOOLS` 换成各家自己的工具和参数。`POST /_script/<家>` 换这一家的剧本。

认第几步：请求带工具表、请求体里提到 marker 的才归剧本；第几步 = 请求体里已经出现了几个本剧本的调用编号（编号里带
marker，所以长会话每轮换一个 marker 就不会认错）。别的请求（起标题、探额度、不带工具表的旁路）一律只回一句「ok」，照样记。
用量报固定的小数（输入 2000）：不让各家自动压缩（审定 Q8）。

每个请求记一行 JSONL（`--log`）：家、形状、是不是剧本、第几步，以及请求头读全、请求体读全、响应头写出、第一块写出、写完、
对端断开的时刻（`time.monotonic_ns`，和驱动是同一个时钟）、请求体字节数、工具名、推理强度那几个字段。不记对话内容；旁路
请求只记系统提示的前 160 个字（各家自己的固定字，用来分清是起标题还是别的）。`BM_DUMP_TOOLS=<目录>` 时每家第一份工具表
（名字、说明、参数）存一份，做出厂核对表用。
"""

import argparse
import json
import os
import re
import select
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
USAGE_IN = 2000
USAGE_OUT = 8


def now():
    return time.monotonic_ns()


def patch(path, old, new):
    """codex 没有单独的改文件、写文件工具（量法设计 4.3）：照它自己的做法，经 exec_command 跑 apply_patch。"""
    if old is None:
        body = "".join(f"+{line}\n" for line in new.splitlines())
        return f"apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: {path}\n{body}*** End Patch\nEOF\n"
    return (f"apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: {path}\n@@\n-{old}\n+{new}\n*** End Patch\nEOF\n")


# 抽象工具 → 各家的（工具名, 参数）。参数名都从各家第一句话的工具表里核过（出厂核对表）。
# shell: command；read: path；edit: path, old, new；write: path, content；sleep: seconds（经 shell 跑 sleep）。
TOOLS = {
    "pi": {
        "shell": lambda a: ("bash", {"command": a["command"]}),
        "read": lambda a: ("read", {"path": a["path"]}),
        "edit": lambda a: ("edit", {"path": a["path"], "edits": [{"oldText": a["old"], "newText": a["new"]}]}),
        "write": lambda a: ("write", {"path": a["path"], "content": a["content"]}),
    },
    "opencode": {
        "shell": lambda a: ("bash", {"command": a["command"], "description": "bm"}),
        "read": lambda a: ("read", {"filePath": a["abs"]}),
        "edit": lambda a: ("edit", {"filePath": a["abs"], "oldString": a["old"], "newString": a["new"]}),
        "write": lambda a: ("write", {"filePath": a["abs"], "content": a["content"]}),
    },
    "codex": {
        "shell": lambda a: ("exec_command", {"cmd": a["command"]}),
        "read": lambda a: ("exec_command", {"cmd": f"sed -n '1,200p' {a['path']}"}),
        "edit": lambda a: ("exec_command", {"cmd": patch(a["path"], a["old"], a["new"])}),
        "write": lambda a: ("exec_command", {"cmd": patch(a["path"], None, a["content"])}),
    },
    "claude": {
        "shell": lambda a: ("Bash", {"command": a["command"], "description": "bm"}),
        "read": lambda a: ("Read", {"file_path": a["abs"]}),
        "edit": lambda a: ("Edit", {"file_path": a["abs"], "old_string": a["old"], "new_string": a["new"]}),
        "write": lambda a: ("Write", {"file_path": a["abs"], "content": a["content"]}),
    },
    "miyu": {
        "shell": lambda a: ("shell", {"command": a["command"]}),
        "read": lambda a: ("read", {"path": a["path"]}),
        "edit": lambda a: ("edit", {"path": a["path"], "old": a["old"], "new": a["new"]}),
        "write": lambda a: ("write", {"path": a["path"], "content": a["content"]}),
    },
    "dsh": {
        "shell": lambda a: ("bash", {"command": a["command"]}),
        "read": lambda a: ("read", {"path": a["path"]}),
        "edit": lambda a: ("edit", {"path": a["path"], "oldText": a["old"], "newText": a["new"]}),
        "write": lambda a: ("write", {"path": a["path"], "content": a["content"]}),
    },
}


def render(rival, step, work):
    """一步剧本 → 这一家的工具名和参数；`work` 是这一家的工作目录（要绝对路径的几家用）。"""
    args = dict(step.get("args", {}))
    kind = step["tool"]
    if kind == "sleep":
        kind, args = "shell", {"command": f"sleep {args.get('seconds', 30)}"}
    if "path" in args:
        args["abs"] = os.path.join(work or "", args["path"]) if work else args["path"]
    table = TOOLS.get(rival) or TOOLS["pi"]
    return table[kind](args)


def tool_names(req):
    out = []
    for t in req.get("tools") or []:
        if isinstance(t, dict):
            out.append(t.get("name") or (t.get("function") or {}).get("name") or t.get("type"))
    return out


def efforts(req):
    """请求体里和推理强度、思考有关的几个字段（出厂核对表要的），原样记。"""
    keep = {}
    for key in ("reasoning_effort", "reasoning", "thinking", "output_config", "max_tokens", "max_completion_tokens",
                "max_output_tokens", "temperature", "parallel_tool_calls", "tool_choice", "store"):
        if key in req:
            keep[key] = req[key]
    return keep


def call_id(marker, n):
    return "call" + re.sub(r"[^A-Za-z0-9]", "", marker) + f"{n:03d}"


class Scripts:
    def __init__(self, default):
        self.default = default
        self.by = {}
        self.work = {}

    def get(self, rival):
        with LOCK:
            return self.by.get(rival, self.default)

    def set(self, rival, script):
        with LOCK:
            self.by[rival] = script
            if script.get("work"):
                self.work[rival] = script["work"]

    @staticmethod
    def step_of(script, body):
        marker = script["marker"].encode()
        if marker not in body:
            return None
        return sum(1 for n in range(len(script["steps"])) if call_id(script["marker"], n).encode() in body)

    def reply(self, rival, script, step):
        steps = script["steps"]
        if step is None:
            return {"text": "ok"}
        if step >= len(steps):
            return {"text": "BMDONE"}
        s = dict(steps[step])
        if "tool" in s:
            name, args = render(rival, s, self.work.get(rival))
            return {"tool": name, "args": args, "id": call_id(script["marker"], step), "text": s.get("say", "")}
        return s


def pieces(r):
    if r.get("pieces"):
        return list(r["pieces"])
    text = r.get("text", "")
    if not text:
        return []
    n = max(1, min(r.get("chunks", 4), len(text)))
    size = -(-len(text) // n)
    return [text[i:i + size] for i in range(0, len(text), size)]


def joined(r):
    return "".join(pieces(r))


def sse(event, data):
    head = f"event: {event}\n" if event else ""
    return (head + "data: " + json.dumps(data, ensure_ascii=False) + "\n\n").encode()


def chat_frames(model, r):
    base = {"id": "chatcmpl-bm", "object": "chat.completion.chunk", "created": int(time.time()), "model": model}
    frames = []
    for i, piece in enumerate(pieces(r)):
        delta = {"content": piece}
        if i == 0:
            delta["role"] = "assistant"
        frames.append(sse(None, {**base, "choices": [{"index": 0, "delta": delta, "finish_reason": None}]}))
    if "tool" in r:
        frames.append(sse(None, {**base, "choices": [{"index": 0, "delta": {"role": "assistant", "tool_calls": [{
            "index": 0, "id": r["id"], "type": "function",
            "function": {"name": r["tool"], "arguments": json.dumps(r["args"], ensure_ascii=False)}}]},
            "finish_reason": None}]}))
    done = "tool_calls" if "tool" in r else "stop"
    frames.append(sse(None, {**base, "choices": [{"index": 0, "delta": {}, "finish_reason": done}],
                             "usage": {"prompt_tokens": USAGE_IN, "completion_tokens": USAGE_OUT,
                                       "total_tokens": USAGE_IN + USAGE_OUT}}))
    frames.append(b"data: [DONE]\n\n")
    return frames


def responses_frames(model, r):
    rid = "resp_bm"
    frames = [sse("response.created", {"type": "response.created", "response": {
        "id": rid, "object": "response", "status": "in_progress", "model": model, "output": []}})]
    output = []
    idx = 0
    text = joined(r)
    if text:
        mid = "msg_bm"
        frames.append(sse("response.output_item.added", {"type": "response.output_item.added", "output_index": idx,
                                                          "item": {"type": "message", "id": mid, "role": "assistant",
                                                                   "status": "in_progress", "content": []}}))
        frames.append(sse("response.content_part.added", {"type": "response.content_part.added", "item_id": mid,
                                                           "output_index": idx, "content_index": 0,
                                                           "part": {"type": "output_text", "text": "",
                                                                    "annotations": []}}))
        for piece in pieces(r):
            frames.append(sse("response.output_text.delta", {"type": "response.output_text.delta", "item_id": mid,
                                                              "output_index": idx, "content_index": 0,
                                                              "delta": piece}))
        item = {"type": "message", "id": mid, "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": text, "annotations": []}]}
        frames.append(sse("response.output_text.done", {"type": "response.output_text.done", "item_id": mid,
                                                         "output_index": idx, "content_index": 0, "text": text}))
        frames.append(sse("response.output_item.done", {"type": "response.output_item.done", "output_index": idx,
                                                         "item": item}))
        output.append(item)
        idx += 1
    if "tool" in r:
        item = {"type": "function_call", "id": "fc_" + r["id"], "call_id": r["id"], "name": r["tool"],
                "arguments": json.dumps(r["args"], ensure_ascii=False), "status": "completed"}
        frames.append(sse("response.output_item.added", {"type": "response.output_item.added", "output_index": idx,
                                                          "item": {**item, "arguments": "", "status": "in_progress"}}))
        frames.append(sse("response.output_item.done", {"type": "response.output_item.done", "output_index": idx,
                                                         "item": item}))
        output.append(item)
    frames.append(sse("response.completed", {"type": "response.completed", "response": {
        "id": rid, "object": "response", "status": "completed", "model": model, "output": output,
        "usage": {"input_tokens": USAGE_IN, "input_tokens_details": {"cached_tokens": 0}, "output_tokens": USAGE_OUT,
                  "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": USAGE_IN + USAGE_OUT}}}))
    return frames


def anthropic_frames(model, r):
    frames = [sse("message_start", {"type": "message_start", "message": {
        "id": "msg_bm", "type": "message", "role": "assistant", "model": model, "content": [],
        "stop_reason": None, "stop_sequence": None,
        "usage": {"input_tokens": USAGE_IN, "output_tokens": 1, "cache_creation_input_tokens": 0,
                  "cache_read_input_tokens": 0}}})]
    idx = 0
    ps = pieces(r)
    if ps:
        frames.append(sse("content_block_start", {"type": "content_block_start", "index": idx,
                                                   "content_block": {"type": "text", "text": ""}}))
        for piece in ps:
            frames.append(sse("content_block_delta", {"type": "content_block_delta", "index": idx,
                                                       "delta": {"type": "text_delta", "text": piece}}))
        frames.append(sse("content_block_stop", {"type": "content_block_stop", "index": idx}))
        idx += 1
    if "tool" in r:
        frames.append(sse("content_block_start", {"type": "content_block_start", "index": idx, "content_block": {
            "type": "tool_use", "id": "toolu_" + r["id"], "name": r["tool"], "input": {}}}))
        frames.append(sse("content_block_delta", {"type": "content_block_delta", "index": idx, "delta": {
            "type": "input_json_delta", "partial_json": json.dumps(r["args"], ensure_ascii=False)}}))
        frames.append(sse("content_block_stop", {"type": "content_block_stop", "index": idx}))
    stop = "tool_use" if "tool" in r else "end_turn"
    frames.append(sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": None},
                                        "usage": {"output_tokens": USAGE_OUT}}))
    frames.append(sse("message_stop", {"type": "message_stop"}))
    return frames


def anthropic_json(model, r):
    content = []
    if joined(r):
        content.append({"type": "text", "text": joined(r)})
    if "tool" in r:
        content.append({"type": "tool_use", "id": "toolu_" + r["id"], "name": r["tool"], "input": r["args"]})
    return {"id": "msg_bm", "type": "message", "role": "assistant", "model": model, "content": content,
            "stop_reason": "tool_use" if "tool" in r else "end_turn", "stop_sequence": None,
            "usage": {"input_tokens": USAGE_IN, "output_tokens": USAGE_OUT}}


def chat_json(model, r):
    msg = {"role": "assistant", "content": joined(r)}
    if "tool" in r:
        msg["tool_calls"] = [{"id": r["id"], "type": "function", "function": {
            "name": r["tool"], "arguments": json.dumps(r["args"], ensure_ascii=False)}}]
    return {"id": "chatcmpl-bm", "object": "chat.completion", "created": int(time.time()), "model": model,
            "choices": [{"index": 0, "message": msg, "finish_reason": "tool_calls" if "tool" in r else "stop"}],
            "usage": {"prompt_tokens": USAGE_IN, "completion_tokens": USAGE_OUT, "total_tokens": USAGE_IN + USAGE_OUT}}


def responses_json(model, r):
    output = []
    if joined(r):
        output.append({"type": "message", "id": "msg_bm", "role": "assistant", "status": "completed",
                       "content": [{"type": "output_text", "text": joined(r), "annotations": []}]})
    if "tool" in r:
        output.append({"type": "function_call", "id": "fc_" + r["id"], "call_id": r["id"], "name": r["tool"],
                       "arguments": json.dumps(r["args"], ensure_ascii=False), "status": "completed"})
    return {"id": "resp_bm", "object": "response", "status": "completed", "model": model, "output": output,
            "usage": {"input_tokens": USAGE_IN, "output_tokens": USAGE_OUT, "total_tokens": USAGE_IN + USAGE_OUT}}


def peer_closed(conn, timeout):
    """等 `timeout` 秒，其间对端关了连接就交回 True（块与块之间看，量打断用）。"""
    ready, _, _ = select.select([conn], [], [], timeout)
    if not ready:
        return False
    try:
        return conn.recv(1, socket.MSG_PEEK) == b""
    except OSError:
        return True


def make_handler(scripts, log):
    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def read_body(self):
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

        def json_reply(self, obj, code=200):
            data = json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            t0 = now()
            rival = self.path.split("/")[1] if self.path.count("/") > 1 else ""
            self.json_reply({"object": "list", "data": [{"id": m, "object": "model", "created": 0, "owned_by": "bm",
                                                          "type": "model", "display_name": m}
                                                         for m in (os.environ.get("BM_MODEL", "bm-model"),)]})
            record(log, {"rival": rival, "method": "GET", "path": self.path.split("?")[0], "t_head": t0})

        def do_HEAD(self):
            self.send_response(200)
            self.send_header("content-length", "0")
            self.end_headers()

        def do_POST(self):
            t_head = now()
            body = self.read_body()
            t_body = now()
            path = self.path.split("?")[0]
            parts = path.split("/")
            rival = parts[1] if len(parts) > 2 else ""
            if parts[1:2] == ["_script"] or parts[1:2] == ["_scene"]:
                if parts[1] == "_script":
                    scripts.set(parts[2], json.loads(body))
                else:
                    record(log, {"scene": json.loads(body), "rival": parts[2], "t_head": t_head})
                self.json_reply({"ok": True})
                return
            try:
                req = json.loads(body or b"{}")
            except ValueError:
                req = {}
            model = req.get("model", "bm-model")
            entry = {"rival": rival, "method": "POST", "path": path, "t_head": t_head, "t_body": t_body,
                     "bytes": len(body), "model": model, "stream": bool(req.get("stream")), "tools": tool_names(req),
                     "effort": efforts(req)}
            if path.endswith("/count_tokens"):
                self.json_reply({"input_tokens": USAGE_IN})
                entry["shape"] = "count_tokens"
                record(log, entry)
                return
            if path.endswith("/chat/completions"):
                shape = "chat"
            elif path.endswith("/responses"):
                shape = "responses"
            elif path.endswith("/messages"):
                shape = "anthropic"
            else:
                self.json_reply({"error": {"message": f"bm fake: unknown path {path}"}}, 404)
                entry["shape"] = "404"
                record(log, entry)
                return
            dump = os.environ.get("BM_DUMP_TOOLS")
            if dump and req.get("tools"):
                target = os.path.join(dump, f"{rival}-tools.json")
                if not os.path.exists(target):
                    with open(target, "w") as f:
                        json.dump(req.get("tools"), f, ensure_ascii=False, indent=1)
            script = scripts.get(rival)
            step = Scripts.step_of(script, body) if req.get("tools") else None
            if step is None:
                sys_text = req.get("instructions") or req.get("system") or next(
                    (m.get("content") for m in req.get("messages") or [] if m.get("role") in ("system", "developer")),
                    "")
                entry["aux_head"] = json.dumps(sys_text, ensure_ascii=False)[:160]
            r = scripts.reply(rival, script, step)
            entry.update(shape=shape, step=step, marker=script["marker"] if step is not None else None)
            ttft = script.get("ttft_ms", 0) / 1000 if step is not None else 0
            gap = script.get("gap_ms", 0) / 1000 if step is not None else 0
            if not req.get("stream"):
                if ttft:
                    time.sleep(ttft)
                obj = {"chat": chat_json, "anthropic": anthropic_json, "responses": responses_json}[shape](model, r)
                entry["t_first"] = now()
                self.json_reply(obj)
                entry["t_done"] = now()
                record(log, entry)
                return
            frames = {"chat": chat_frames, "responses": responses_frames, "anthropic": anthropic_frames}[shape](model, r)
            conn = self.connection
            try:
                self.send_response(200)
                self.send_header("content-type", "text/event-stream")
                self.send_header("cache-control", "no-cache")
                self.send_header("transfer-encoding", "chunked")
                self.end_headers()
                self.wfile.flush()
                entry["t_headers_sent"] = now()
                gone = ttft and peer_closed(conn, ttft)
                for i, f in enumerate(frames):
                    if gone or (i and gap and peer_closed(conn, gap)):
                        entry["t_client_gone"] = now()
                        entry["sent_frames"] = i
                        break
                    self.wfile.write(b"%x\r\n%s\r\n" % (len(f), f))
                    self.wfile.flush()
                    if i == 0:
                        entry["t_first"] = now()
                else:
                    self.wfile.write(b"0\r\n\r\n")
                    self.wfile.flush()
                    entry["t_done"] = now()
            except OSError:
                entry["client_gone"] = True
                entry.setdefault("t_client_gone", now())
            record(log, entry)
            if "t_client_gone" in entry:
                self.close_connection = True

    return H


def record(log, entry):
    with LOCK:
        with open(log, "a") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, default=0)
    p.add_argument("--script")
    p.add_argument("--log", required=True)
    a = p.parse_args()
    default = json.load(open(a.script)) if a.script else {"marker": "BMGO", "steps": [{"text": "BMOK"}]}
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), make_handler(Scripts(default), a.log))
    srv.daemon_threads = True
    print(f"listening {srv.server_address[1]}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
