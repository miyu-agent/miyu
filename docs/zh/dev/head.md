# 写一个头

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

头（head）是连接核心的界面程序，人通过它和 AI 对话，例如终端界面、网页界面，这两个是出厂自带的头。这一页带你从零开始写一个自己的头（例如编辑器插件、手机端或者其他界面），并把它做成软件包安装。协议里每个方法的全部参数都在 [协议参考](../reference/README.md) 里，这一页只把要走的步骤串起来。

## 一个头要做的几步

一个头从启动到和 AI 对话，要依次做下面几步：

1. 找到数据根，连接核心的本机套接字；如果核心没有在运行，就先启动它。
2. 用 `hello` 握手，出示本机令牌。
3. 打开一个会话（`session.create`），或者接入一个已有的会话。
4. 订阅这个会话的视图流（`subscribe`，`stream` 写 `view`）。
5. 发送一句话（`session.send`）。
6. 根据核心推送过来的条目画出时间线；遇到需要确认的操作或者 AI 的提问，先问人，再用 `session.answer` 回答。

## 最小的例子

下面这个头只用了 Python 标准库，三个平台用同一份文件。它会打开一个新会话，发送一句话，一边接收回答一边把文字输出到终端，遇到确认和提问时在终端里问你。这个例子在 Linux、macOS 上运行通过；Windows 上走的是 `WINDOWS` 那几行，是按照 `miyu-client` 的做法写的，还没有在 Windows 上实际测试过。把它保存成 `tiny-head`；在 Linux、macOS 上还要加上执行权限：

```python
#!/usr/bin/env python3
"""最小的头：开一个会话，发一句话，把回答的字印出来。"""
import io, json, os, socket, subprocess, sys, time, uuid
from pathlib import Path

ROOT = Path(os.path.expanduser(os.environ.get("MIYU_HOME") or "~/.miyu"))
WINDOWS = os.name == "nt"
PREFIX = uuid.uuid4().hex[:8]          # 每个连接取一个随机前缀，这样 id 不会和以前用过的重复
seq, entries, shown, answered, done = 0, {}, {}, set(), False

def connect():
    """连上了交回（写的函数，读的文件）；连不上交回 None。"""
    try:
        where = (ROOT / "run" / "socket").read_bytes().rstrip(b"\n")
        if WINDOWS:                    # 命名管道：当作文件打开，读写用同一个句柄
            pipe = open(where.decode(), "r+b", buffering=0)
            return pipe.write, io.BufferedReader(pipe)
        sock = socket.socket(socket.AF_UNIX)
        sock.connect(where)
        return sock.sendall, sock.makefile("rb")
    except OSError:
        return None

def start_core():
    ROOT.mkdir(parents=True, exist_ok=True)
    flags = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP if WINDOWS else 0
    core = subprocess.Popen([os.environ.get("MIYU_PROGRAM", "miyu"), "core"], cwd=ROOT,
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, start_new_session=not WINDOWS,
                            creationflags=flags)
    line = core.stdout.readline().decode().strip()      # ready、running，或者 error …
    if not line or line.startswith("error"):
        sys.exit(f"核心起不来：{line}")

def read():
    line = lines.readline()
    if not line:
        sys.exit("核心断开了")
    return json.loads(line)

def call(method, params):
    global seq
    seq += 1
    rid = f"{PREFIX}-{seq}"
    msg = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}
    send(json.dumps(msg, ensure_ascii=False).encode() + b"\n")
    while True:
        got = read()
        if got.get("id") == rid:
            if "error" in got:
                sys.exit(f"{method}：{got['error'].get('data', {}).get('reason')}")
            return got["result"]
        push(got.get("method"), got.get("params", {}))

def push(method, p):
    global done
    if method in ("view.add", "view.update"):
        entry = entries[p["entry"]["id"]] = p["entry"]
    elif method == "view.append" and p["id"] in entries:
        entry = entries[p["id"]]
        entry["text"] = entry.get("text", "") + p["text"]
    elif method == "view.status":
        for wait in p["status"].get("waiting", []):
            if wait["call"] not in answered:
                answered.add(wait["call"])
                respond(p["session"], wait)
        return
    elif method == "resync":
        sys.exit("读得太慢掉了队：要重新订阅")
    else:
        return
    if entry["kind"] == "reply":
        print(entry["text"][shown.get(entry["id"], 0):], end="", flush=True)
        shown[entry["id"]] = len(entry["text"])
    if entry["kind"] == "end":
        done = True

def respond(session, wait):
    entry = entries.get(wait["entry"], {})
    if wait["what"] == "approve":
        title = entry.get("title", {})
        yes = input(f"\n允许 {title.get('name')} {title.get('object', '')}？[y/N] ").strip() == "y"
        answer = {"decision": "once" if yes else "deny"}
    else:
        answer = {"answers": [{"text": input(f"\n{q['question']} ")} for q in entry.get("questions", [])]}
    if "nested" in wait:
        answer["nested"] = wait["nested"]
    call("session.answer", {"session": session, "call": wait["call"], **answer})

got = connect()
if got is None:
    start_core()
    for _ in range(50):
        if (got := connect()):
            break
        time.sleep(0.2)
    else:
        sys.exit("连不上核心")
send, lines = got
token = (ROOT / "run" / "token").read_text().strip()   # 先连接，再读取令牌

call("hello", {"protocol": [1, 1], "head": {"kind": "tiny", "version": "0.1.0"},
               "locale": os.environ.get("LANG", ""), "caps": {"input": True}, "token": token})
session = call("session.create", {})["session"]
call("subscribe", {"session": session, "stream": "view"})
call("session.send", {"session": session, "text": " ".join(sys.argv[1:]) or "你好"})
while not done:
    got = read()
    push(got.get("method"), got.get("params", {}))
print()
```

在 Linux、macOS 上运行：

```bash
./tiny-head 用一句话介绍你自己
```

在 Windows 上运行：

```powershell
python tiny-head 用一句话介绍你自己
```

下面几节一步一步讲它发出了什么、收到了什么。`→` 表示头发出的消息，`←` 表示核心发来的消息；`…` 表示省略掉的字段。

### 连接核心

连接核心时要注意下面几点：

- 数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，就是它指向的目录）里的 `run/socket` 文件只有一行路径，写的是核心在哪个套接字上等待连接。Linux、macOS 上是 Unix 域套接字，Windows 上是命名管道 `\\.\pipe\miyu-<指纹>`，只对你这个用户开放。命名管道按字节流读写，像文件一样打开即可；之后的规则和 Unix 套接字上一样：一行一条消息，第一条是出示令牌的 `hello`。
- 连接时要核对对面是不是你自己的核心，以免其他用户抢先占了这个位置冒充核心：Linux、macOS 上，在连接之前检查套接字所在的目录属于你、其他用户进不来；Windows 上，在连接之后检查管道另一头的进程属于你这个用户。例子里省略了这一步；`miyu-client`（见下面的 [用 Rust 写](#用-rust-写miyu-client)）在连接时会按平台做这项检查。
- `run/token` 是本机令牌，64 位十六进制数。核心每次启动都会换一个新的令牌，所以要先连接，再读取令牌。
- 如果连接不上，就启动核心：运行 `miyu core`，工作目录设成数据根，标准输入、标准错误接到空设备。要让它和你的终端脱离，这样你按 `Ctrl+C` 时不会影响到它：Linux、macOS 上让它自成一个进程组；Windows 上带 `DETACHED_PROCESS`、`CREATE_NEW_PROCESS_GROUP` 启动它，这样它不占用你的控制台，也不会打开新窗口。核心启动以后会往标准输出写一行：`ready`（启动好了）、`running`（已经有一个核心在运行，这一个会退出，你去连接已有的那一个）或者 `error <原因>`（启动失败）。如果读到的是 `running`，过一会儿再连接。
- 通过 `miyu <名字>` 打开的头，环境变量里有 `MIYU_PROGRAM`，它是主程序 `miyu` 的真实位置：要用它运行 `<它> core`，不要去 `PATH` 里找。如果是直接运行 `miyu` 打开的默认界面，核心在打开你之前就已经启动了。
- 一个数据根只有一个核心。头根据 `MIYU_HOME` 确定数据根，只连接这个数据根的核心。

### 握手

连接以后发出的第一条消息必须是 `hello`；如果 10 秒内没有完成握手，核心会断开连接。

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tiny","version":"0.1.0"},"locale":"zh_CN.UTF-8","caps":{"input":true},"token":"9c41…"}}
← {"id":"3f2a91c0-1","jsonrpc":"2.0","result":{"core":{"version":"…"},"host":{"home":"/home/alice","platform":"linux","workspace":"…","dirs":{…}},"language":"zh","protocol":1,"sandbox":{"usable":true},"view":1}}
```

| 握手时报的字段 | 说明 |
|---|---|
| `protocol` | 你能使用的协议主版本，写成 `[最低, 最高]`。核心只有版本 `1`，如果它不在这个范围里，核心会回 `protocol_mismatch` 并断开连接。 |
| `head` | `kind`、`version`：你是什么程序、哪一个版本。它们只记录在核心的运行日志里。 |
| `locale` | 系统语言（`zh-CN`、`en_US.UTF-8` 这两种写法都能识别）。`ui.language` 是 `auto` 时，核心根据它决定这个连接使用哪种语言。 |
| `caps.input` | 能不能让人当场回答。不写时是 `false`：此时你打开的会话没有人能确认，需要确认的工具调用不会执行，AI 的提问会直接得到「没人能回答」的结果。 |
| `caps.prefix_ask` | 声明在文件改动、下一轮请求的前缀要变化时，你会先问人（见下面的「前缀要变」）。 |
| `token` | 本机令牌。 |

回应里你会用到这几个字段：`language`（之后给人看的文字使用 `zh`、`en`、`ja` 中的哪一种）、`host.workspace`（会话没有指定工作目录时使用的默认工作区）和 `sandbox.usable`。握手的每一个字段见 [连接](../reference/connection.md#hello)。

### 打开会话、订阅视图流

握手成功以后，打开一个会话，再订阅它的视图流：

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-2","method":"session.create","params":{}}
← {"id":"3f2a91c0-2","jsonrpc":"2.0","result":{"cwd":"…","events":[1],"session":"01a14aca-d2c4-768c-acc6-a67e35106d22"}}
→ {"jsonrpc":"2.0","id":"3f2a91c0-3","method":"subscribe","params":{"session":"01a14aca-…","stream":"view"}}
← {"id":"3f2a91c0-3","jsonrpc":"2.0","result":{"entries":[],"more":false,"status":{"state":"idle",…},"permission":…,"model":…,…}}
```

`session.create` 没有写 `cwd` 时，使用这个会话自己的默认工作区；没有写 `persona`、`preset`、`model` 时，使用配置里的默认值。如果要接入一个已有的会话，就跳过 `session.create`，直接订阅它；会话列表用 `session.list` 获取。

订阅的回应里带有最新一页条目和完整的会话状态 `status`。之后这个会话的所有变化都会推送过来。

### 发送一句话

用 `session.send` 发送一句话：

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-4","method":"session.send","params":{"session":"01a14aca-…","text":"用一句话介绍你自己"}}
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":null,"entry":{"id":"m2","kind":"user","text":"用一句话介绍你自己",…}}}
← {"jsonrpc":"2.0","method":"view.status","params":{"session":"01a14aca-…","status":{"state":"running",…}}}
← {"id":"3f2a91c0-4","jsonrpc":"2.0","result":{"cwd":"…","events":[2]}}
```

这句话保存到磁盘以后，核心就会回应，不会等 AI 回答完。一个请求引起的推送一定比它的回应先到，所以在等待回应的同时也要处理推送。如果 AI 正在忙，这句话会排队；带 `urgent: true` 的会插进下一步。

### 画出回答

AI 回答时，核心会推送下面这些消息：

```json
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":"m2","entry":{"id":"b3.0","kind":"reply","text":"我是","open":true,"persona":null,…}}}
← {"jsonrpc":"2.0","method":"view.append","params":{"session":"01a14aca-…","id":"b3.0","text":"一个在你电脑上干活的助手。"}}
← {"jsonrpc":"2.0","method":"view.update","params":{"session":"01a14aca-…","entry":{"id":"b3.0","kind":"reply","text":"我是一个在你电脑上干活的助手。",…}}}
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":"b3.0","entry":{"id":"e5","kind":"end",…}}}
```

画出回答时要注意以下几点：

- 按 `id` 保存条目。`view.add` 表示新的一条，排在 `after` 指定的那一条后面（`null` 表示排在最前面）；`view.update` 表示整条替换，如果带了 `after`，说明这一条挪了位置；`view.append` 表示把文字接在那一条的 `text`（工具条目是 `args`）后面；`view.hidden` 用于撤销、恢复，把几条藏起来或者重新显示；`view.remove` 表示删掉一条。
- 同一条可能先收到 `add` 再收到 `update`，`update` 里的文字可能已经包含了没有单独 `append` 过的部分。所以要按整条的文字来画，不要只拼接增量。例子里记录了每一条已经输出到第几个字，只输出多出来的部分。
- `kind` 有这几种：`user`（人说的话）、`reply`（回答正文，是 Markdown 原文，由头负责渲染）、`media`（AI 发出的图片）、`group`（时间线里的一段）、`thought`（一步思考）、`tool`（一步工具调用）、`end`（一轮的结束）、`notice`（旁白，例如压缩、撤销、换模型，具体是哪一种要看 `what`）。
- `open: true` 表示这一条还在写。工具运行时的 `progress`（最新一行进度）和 `retitle_failed` 旁白只在视图流里推送，翻页或者重新订阅时拿不到。
- 一轮结束时，核心会加一条 `kind` 为 `end` 的条目，同时会话状态 `status.state` 回到 `idle`。

每种条目的字段、`notice` 的每种 `what`、会话状态的每个字段，详细的见 [视图流](../reference/view.md)。

### 掉队和重新连接

如果你读得太慢，核心给一个订阅积压的消息达到上限（会话是 1024 条），就会推送 `resync {session, stream: "view"}`，这个订阅随之停止。会话停止时也会推送 `resync`。此时重新订阅一次：回应里是最新一页和完整的状态，用它把你手里的内容整个替换掉。

连接断开以后，重新连接、重新握手、重新订阅。往上翻看历史用 `view.page`。

视图流是给界面画图用的；如果要原始事件（例如自己保存日志），就订阅 `stream: "events"`，可以带上 `after` 补上漏掉的事件，详细的见 [事件](../reference/events.md)。

## 确认和提问

AI 要做受保护的操作时（例如开放权限下的危险命令、只读模式下的写操作），会停下来等待确认；AI 用 `ask_user` 提问时也会等待。这两种情况都记录在会话状态里：`status.state` 是 `waiting`，`status.waiting` 里每件事一项 `{what, entry, call}`，其中 `what` 是 `approve`（确认）或 `ask`（提问），`entry` 指向那一步工具的条目。

处理确认：

- 那一步的条目里有 `approval`，包含 `access`、`rule`、`detail`。带有 `expires_at` 的确认，到了那个时刻还没有人回答就会自动拒绝，头可以画一个倒计时。等待的时长由配置 `permission.confirm.timeout` 决定，出厂值是 300 秒，设为 0 表示一直等待。
- 回答用 `session.answer {"session", "call", "decision"}`，`decision` 可以是 `once`（允许这一次）、`session`（这个会话里都允许，条目里有 `rule` 时才能选）或 `deny`（拒绝，可以带上 `reason` 给 AI 看）。

处理提问：

- 那一步的条目里有 `questions`：每道题有 `question`、可选的 `header`、`options`（包含 `label`、`description`、`preview`）和 `multiple`。提问会一直等待，不会超时。
- 回答用 `session.answer {"session", "call", "answers"}`，按题目的先后顺序一题一项，写成 `{"picked": [选的选项的 label…], "text": <自己写的>}`。

如果 `waiting` 里的一项带有 `nested`（表示 AI 在命令里通过 `miyu call` 调用的工具需要确认），回答时要把它原样写上。几个头同时打开同一个会话时，以最先回答的为准，后回答的会收到 `not_asking`。每个字段和出错情况见 [会话](../reference/sessions.md#sessionanswer)。

没有人看着的头（例如定时运行的脚本、只推送通知的程序）握手时不写 `caps.input`，或者写成 `false`。这样它打开的会话里，需要确认的操作不会执行，提问会直接得到「没人能回答」的结果，AI 会根据这个结果继续工作，不会卡住等人。

## 会话的其他操作

界面上其他常用的会话操作和对应的方法如下：

| 界面上的动作 | 方法 |
|---|---|
| 新建会话 | `session.create` |
| 列出会话、跟踪列表的变化 | `session.list`；订阅 `sessions` 流以后，列表变化时会推送 `sessions.changed` |
| 告诉核心你正在打开哪个会话 | `session.viewing`：你正在打开的会话的后台通知由你发出，其他头不再弹出 |
| 往上翻看历史 | `view.page` |
| 发送一句话 | `session.send` |
| 带附件发送 | 先用 `blob.put` 上传（用 `path` 交本机文件，或者用 `data` 交 base64 内容），再把回应里的 `blob`、`name`、`media_type` 放进 `session.send` 的 `attachments` |
| 打断 | `session.interrupt`，必须写 `queued`，它决定排队中的话怎么处理：`send`（打断后马上发送）、`return`（退回来）、`keep`（继续留着） |
| 退回一条排队中的话 | `session.withdraw` |
| 撤销、恢复 | `session.revert`、`session.unrevert` |
| 重做最后一轮 | `session.redo` |
| 压缩、清空上下文 | `session.compact`、`session.clear` |
| 更换人格、预设、模型、看图模型、思考强度 | `session.configure` |
| 切换到只读、切回开放权限 | `session.set_permission_level`，写 `{"read_only": true}` 或 `false` |
| 更换工作目录 | `session.set_workspace` |
| 修改标题、置顶 | `session.set_meta`；如果要让模型重新起标题，用 `session.retitle` |
| 分叉 | `session.fork` |
| 导出 | `session.export`：`format` 写 `bundle`（原始包）或 `markdown` |
| 删除 | `session.delete` |

多数方法只回 `{}` 或几条事件的序号，变化本身会通过推送到达。每个方法详细的见 [会话](../reference/sessions.md)。

**前缀要变**：如果头在握手时写了 `caps.prefix_ask: true`，开始新一轮的请求（`session.send`、`session.redo`、会开始一轮的 `command.run`）可能会回 `prefix_changing`，意思是人格、预设、提示词这些文件改了，下一轮的缓存会失效。`data.files` 列出了改过的文件。这时要问人选择 `直接发送`、`压缩上下文并发送` 还是 `取消`，再根据选择带上 `"prefix": "keep"` 或 `"compact"` 重新发送一次。没有声明的头仍然直接发送。

## 斜杠命令

头要能列出和执行斜杠命令：

- 列出：调用 `command.catalog {"session"}`，回应里有 `commands`（`name`、`aliases`、`summary`、`argument`、`usage`、`opens`）和 `skills`（在输入框里用 `$技能名` 点名的技能）。
- 执行：把人输入的整行交给 `command.run {"session", "text"}`，回应里的 `said` 是回执，原样显示给人看。如果回应带有 `opens`，表示要打开一个扩展的整页（见 [往界面上放东西](ui-surfaces.md)）。
- 头自己的命令（例如 `/help`、换主题）由头自己识别、自己处理，并拼进同一个菜单。

核心能识别的命令和出错情况见 [软件包与命令](../reference/packages.md#commandrun)。

## 头自己的设置

头的设置分为几种，存放的位置各不相同：

- 皮肤、配色这类跟着人走的设置存进核心：用 `prefs.get {"package"}` 读取、`prefs.set {"package", "values"}` 写入，`package` 是你这个头的软件包编号，整份最多 64 KiB。其他头改了以后会推送 `prefs.changed`（需要订阅 `prefs` 流，带上 `package`）。只有安装成界面包（见下文）的头才能使用，其他的会回 `unknown_package` 或 `not_a_ui_package`。详细的见 [头自己的设置](../reference/prefs.md)。
- 只和这台设备有关的设置（例如窗口大小、输入历史）存在头自己那里。
- 核心的配置用 `config.get` 读取；订阅 `config` 流以后，配置变化时会推送 `config.changed`。头需要遵照的几项是：`ui.language`（界面语言）和 `ui.startup`（打开头时是开新会话 `new`，还是接着最近的会话 `recent`）。
- 你自己的配置项写进清单的 `[settings.<名字>]`，键是 `<前缀>.<名字>`，设置页会根据它画出设置项（见 [清单](manifest.md)）。

## 用 Rust 写：`miyu-client`

核心仓库里的 crate `miyu-client` 是给头使用的门面（facade）。依赖它时要钉住一个具体的提交：

```toml
[dependencies]
miyu-client = { git = "https://github.com/miyu-agent/miyu", rev = "<核心的提交号>" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt", "io-util"] }
```

核心的 crate 需要 Rust 1.97 以上的版本。`miyu-client` 使用 AGPL-3.0-or-later 许可证；只通过协议和核心通信的程序不受这个许可证约束。

它负责连接、查找数据根和启动核心；握手、订阅、收发消息由你自己写，做法和上面一样：

```rust
use std::process::Command;

use miyu_client::connect::connect_or_start;
use miyu_client::places::{DataRoot, Env};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = DataRoot::locate(&Env::current())?;
    root.prepare()?;
    let miyu = std::env::var_os("MIYU_PROGRAM").unwrap_or_else(|| "miyu".into());
    let (connection, token) = connect_or_start(&root, move || {
        let mut core = Command::new(miyu);
        core.arg("core");
        core
    })
    .await?;
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read).lines();
    let hello = json!({"jsonrpc": "2.0", "id": "h1", "method": "hello", "params": {
        "protocol": [1, 1],
        "head": {"kind": "tiny", "version": "0.1.0"},
        "locale": miyu_client::places::locale().unwrap_or_default(),
        "caps": {"input": true},
        "token": token,
    }});
    write.write_all(format!("{hello}\n").as_bytes()).await?;
    while let Some(line) = lines.next_line().await? {
        let got: Value = serde_json::from_str(&line)?;
        if got["id"] == "h1" {
            println!("{}", got["result"]);
            break;
        }
    }
    Ok(())
}
```

`connect_or_start` 连接成功后交回连接和本机令牌；如果核心没有在运行，它会用你给的命令启动核心，并等待核心输出的那一行；几个头同时启动核心时，只会启动一个。它不会替你重新连接：连接断开以后，你要再调用一次，然后重新握手、重新订阅。

| 模块 | 提供的内容 |
|---|---|
| `connect` | `connect`（只连接）、`connect_or_start`（核心没有在运行时启动它）、`Connection`（异步读写的字节流）和错误类型 |
| `places` | `DataRoot`（`locate`、`prepare`）、`Env`、`locale`、缓存目录，以及配置文件的读写 |
| `open` | `Core`：出示本机令牌握手，一问一答，不接收推送。只问几句就退出的程序（例如命令行工具）用它 |
| `texts` | 核心给人看的文字，按语言读取 |
| `protocol`、`ids`、`time`、`events` | 会话编号、事件、时刻这些协议里的类型 |
| `settings` | 配置的种类和读取方法 |
| `widgets` | 界面块内容的校验和语言选择 |
| `manifest` | 读取自己的软件包清单 |
| `log`、`heap`、`open_files` | 运行日志、进程的堆、打开文件数的上限 |
| `seeds` | 从一张图片里挑选主题色的候选 |

有几项功能默认不开启，需要时打开对应的 feature：`view`（视图流条目、推送、会话状态的类型）、`wallpaper`（查找桌面壁纸、跟随壁纸变化）、`http`、`tool`。

## 做成软件包

做成界面包以后，可以用 `miyu tiny` 打开你的头，人还可以把它设成直接运行 `miyu` 时打开的界面。

包的目录结构如下：

```text
tiny-head/
├── package.toml
└── bin/
    └── tiny-head      # 上面那个 Python 文件，Linux、macOS 上带执行权限
```

`package.toml` 写成这样：

```toml
[package]
version = "0.1.0"
protocol = [1, 1]
name = { zh = "小头", en = "Tiny head" }
summary = { zh = "一个最小的头", en = "A minimal head" }

[command]
name = "tiny"
program = "tiny-head"
about = { zh = "打开小头", en = "Open the tiny head" }

[ui]
```

1. 安装：`miyu pkg install ./tiny-head`。
2. 打开：`miyu tiny 你好`。人输入的参数会原样交给你的程序，环境变量里有 `MIYU_PROGRAM`。在 Linux、macOS 上，`miyu` 进程会换成你的程序；在 Windows 上，`miyu` 会启动你的程序并等待它退出，退出码和你的程序一样，按 `Ctrl+C` 会送到你的程序。
3. 设成默认：`miyu config set ui.head tiny-head`。之后在终端里直接运行 `miyu` 就会打开它（只在标准输入、标准输出都是终端时才这样）。安装了两个以上界面时，设置页的 `通用` 里也有 `默认界面` 这一项。

- 清单里有 `[ui]` 这张表，就是界面包。`opens` 写你能处理的页，例如 `opens = ["config"]`：人运行 `miyu config` 时，核心会打开你的头，并带上 `--page config`；如果没有写，`miyu config` 不会打开你的头。
- `[command]` 的每个字段、`[settings]` 的写法见 [清单](manifest.md)。`[ui]` 这张表只给界面包使用，不在那一页列出的冻结格式里，以后可能会改：升级核心以后要先核对一遍。
- 在 Windows 上，核心会先找 `bin/tiny-head.exe`；如果没有，就根据 `bin/tiny-head` 第一行的 `#!` 找解释器，`#!/usr/bin/env python3` 找的是 `PATH` 里的 `python`。所以这个包在三个平台上都能用，文件名不加 `.py`；在 Windows 上要先安装好 Python，并且能在 `PATH` 里找到 `python`。详细的见 [写一个 Miyu 软件包：三个平台](packages.md#三个平台)。
- 如果包里带有编译出来的程序，要写 `targets`，一种系统打一个包；打包和发布到软件源的方法见 [写一个软件包](packages.md)。

## 网页类的头

浏览器不能连接本机套接字，所以出厂的网页界面是这样接入的：网页服务 `miyu-web` 在本机监听一个端口，浏览器通过 WebSocket 连接它，它把每一行消息原样转给核心，握手用的凭据由页面自己出示（`miyu-web` 不读取本机令牌）。

要做网页类的头，有两种做法：

- **自己启动一个本机服务做转发**，做法和上面一样。如果转发的是从其他机器来的连接，要在握手之前先替它写一行 `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":"<来源地址>"}}`：这样这个连接就按远程连接处理，读取本机文件、传本机路径的方法一律回 `local_machine_only`，凭据要用访问密码。详细的见 [连接](../reference/connection.md#怎么连)。
- **往出厂的网页界面里加东西**：见 [往界面上放东西](ui-surfaces.md)。

## 注意事项

- 协议 1 是冻结的：已有的方法、字段不会删除、改名或者改变意思，只会增加。老版本的核心收到新方法会回 `unknown_method`，收到新字段会忽略；使用新的方法、字段之前，先看握手回应里核心的版本，或者在收到 `unknown_method` 时退回旧的做法。
- 判断出了什么错要看 `error.data.reason`，不要看 `error.message`：`error.message` 是按照连接的语言写给人看的一句话。全部原因码见 [出错](../reference/errors.md)。
- 每条消息占一行，最长 1 MiB。大的内容通过 `blob.put` 传，超过上传上限的文件只传路径（见 [会话](../reference/sessions.md#sessionsend)）。
- 每个连接上请求的 `id` 都要和以前用过的不一样：核心根据 `id` 识别断线后的重发，如果和旧的 `id` 重复，拿到的是上一次的回应，核心什么都不会做。做法是每次连接取一个随机前缀，后面接上递增的数字。
- 你的头不是唯一的头。终端、网页、托盘可能同时打开同一个会话：别人发的话、别人回答的确认、别人改的设置，都要根据推送来更新，不要假设只有你在修改。
