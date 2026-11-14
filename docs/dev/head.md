# 写一个头

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

头是连上核心、让人和 AI 说话的程序。终端、网页是出厂的两个。这一页带你从零写一个自己的头（编辑器插件、手机端、别的界面），做成软件包装上。协议每个方法的全部参数在 [协议参考](../reference/README.md)，这一页只串起要走的路。

## 一个头要做的几步

1. 找到数据根，连上核心的本机套接字；核心没在跑的拉起它。
2. 握手 `hello`，出示本机令牌。
3. 开一个会话（`session.create`），或者接一个已有的。
4. 订阅这个会话的视图流（`subscribe`，`stream` 写 `view`）。
5. 发一句话（`session.send`）。
6. 照推过来的条目画时间线；遇到要确认、AI 提问的，问人，再用 `session.answer` 答。

## 最小的例子

下面这个头用 Python 标准库写，三个平台用同一份：开一个新会话，发一句话，把回答的字边收边印，遇到确认和提问在终端里问你。Linux、macOS 上跑通过；Windows 上走的是 `WINDOWS` 那几行，照 `miyu-client` 的做法写的，还没在 Windows 上实测。存成 `tiny-head`；Linux、macOS 上加上执行权限：

```python
#!/usr/bin/env python3
"""最小的头：开一个会话，发一句话，把回答的字印出来。"""
import io, json, os, socket, subprocess, sys, time, uuid
from pathlib import Path

ROOT = Path(os.path.expanduser(os.environ.get("MIYU_HOME") or "~/.miyu"))
WINDOWS = os.name == "nt"
PREFIX = uuid.uuid4().hex[:8]          # 每个连接一个随机前缀，id 才不和以前用过的撞
seq, entries, shown, answered, done = 0, {}, {}, set(), False

def connect():
    """连上了交回（写的函数，读的文件）；连不上交回 None。"""
    try:
        where = (ROOT / "run" / "socket").read_bytes().rstrip(b"\n")
        if WINDOWS:                    # 命名管道：当文件打开，读写同一个
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
token = (ROOT / "run" / "token").read_text().strip()   # 先连上再读令牌

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

```bash
./tiny-head 用一句话介绍你自己
```

```powershell
python tiny-head 用一句话介绍你自己
```

下面几节逐步讲它发了什么、收到什么。`→` 是头发出的，`←` 是核心发来的；`…` 是省掉的格。

### 连上

- 数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的是它）里的 `run/socket` 是一行路径：核心在哪个套接字上等连接。Linux、macOS 上是 Unix 域套接字，Windows 上是命名管道 `\\.\pipe\miyu-<指纹>`，只对你这个用户开放。管道照字节流读写，像文件一样打开就行；之后一行一条、第一条 `hello` 出示令牌，和 Unix 套接字上一样。
- 核对对面是不是你自己的核心，免得别的用户抢先占了这个位置冒充核心：Linux、macOS 上连之前看套接字所在的目录是你的、别人进不来；Windows 上连上以后看管道另一头的进程是你这个用户的。例子里省掉了这一步；`miyu-client`（见下面 [用 Rust 写](#用-rust-写miyu-client)）连的时候照平台做了。
- `run/token` 是本机令牌，64 位十六进制。核心每次起来都换一个，所以先连上再读。
- 连不上就拉起核心：跑 `miyu core`，工作目录设成数据根，标准输入、标准错误接空。让它和你的终端脱开，你按 `Ctrl+C` 打不到它：Linux、macOS 上让它自成一个进程组；Windows 上带 `DETACHED_PROCESS`、`CREATE_NEW_PROCESS_GROUP` 起，它不占你的控制台，也不开新窗口。核心起来以后往标准输出写一行：`ready`（好了）、`running`（已经有一个核心在跑，这一个退出，你去连那一个）、`error <原因>`（起不来）。读到 `running` 的，隔一会儿再连。
- 用 `miyu <名字>` 打开的头，环境里有 `MIYU_PROGRAM`，是主程序 `miyu` 的真实位置：照它跑 `<它> core`，别去 `PATH` 里找。直接敲 `miyu` 打开的默认界面，核心在打开你之前已经拉起了。
- 一个数据根一个核心。头照 `MIYU_HOME` 认数据根，只连那一个数据根的核心。

### 握手

连上以后第一条必须是 `hello`，10 秒内没握手成的断开。

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tiny","version":"0.1.0"},"locale":"zh_CN.UTF-8","caps":{"input":true},"token":"9c41…"}}
← {"id":"3f2a91c0-1","jsonrpc":"2.0","result":{"core":{"version":"…"},"host":{"home":"/home/alice","platform":"linux","workspace":"…","dirs":{…}},"language":"zh","protocol":1,"sandbox":{"usable":true},"view":1}}
```

| 报的 | 说明 |
|---|---|
| `protocol` | 你说得了的协议主版本 `[最低, 最高]`。核心只有 `1`，不在范围里的回 `protocol_mismatch` 并断开 |
| `head` | `kind`、`version`：你是什么、哪一版，只记进核心的运行日志 |
| `locale` | 系统语言（`zh-CN`、`en_US.UTF-8` 都认）。`ui.language` 是 `auto` 时照它定这个连接说哪种话 |
| `caps.input` | 能不能让人当场回答。不写是 `false`：你开的会话没人能确认，要确认的工具调用不跑，AI 的提问直接得到「没人能回答」 |
| `caps.prefix_ask` | 文件改了、下一轮的请求前缀要变时，你先问人（见下面「前缀要变」） |
| `token` | 本机令牌 |

回应里要用的几格：`language`（之后给人看的字是 `zh`、`en`、`ja` 哪种）、`host.workspace`（会话不写工作目录时用的默认工作区）、`sandbox.usable`。握手的每一格见 [连接](../reference/connection.md#hello)。

### 开会话、订视图流

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-2","method":"session.create","params":{}}
← {"id":"3f2a91c0-2","jsonrpc":"2.0","result":{"cwd":"…","events":[1],"session":"01a14aca-d2c4-768c-acc6-a67e35106d22"}}
→ {"jsonrpc":"2.0","id":"3f2a91c0-3","method":"subscribe","params":{"session":"01a14aca-…","stream":"view"}}
← {"id":"3f2a91c0-3","jsonrpc":"2.0","result":{"entries":[],"more":false,"status":{"state":"idle",…},"permission":…,"model":…,…}}
```

`session.create` 不写 `cwd` 的用这个会话自己的默认工作区；`persona`、`preset`、`model` 不写的照配置的默认。接一个已有的会话，跳过 `session.create`，直接订阅它；会话列表用 `session.list`。

订阅的回应带最新一页的条目和整份会话状态 `status`。之后这个会话的变化都推过来。

### 发一句话

```json
→ {"jsonrpc":"2.0","id":"3f2a91c0-4","method":"session.send","params":{"session":"01a14aca-…","text":"用一句话介绍你自己"}}
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":null,"entry":{"id":"m2","kind":"user","text":"用一句话介绍你自己",…}}}
← {"jsonrpc":"2.0","method":"view.status","params":{"session":"01a14aca-…","status":{"state":"running",…}}}
← {"id":"3f2a91c0-4","jsonrpc":"2.0","result":{"cwd":"…","events":[2]}}
```

这句话落了盘就回，不等 AI 答完。一个请求引起的推送，一定比它的回应先到，所以等回应的时候也要处理推送。AI 正忙时这一句排队，`urgent: true` 的插进下一步。

### 把回答画出来

```json
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":"m2","entry":{"id":"b3.0","kind":"reply","text":"我是","open":true,"persona":null,…}}}
← {"jsonrpc":"2.0","method":"view.append","params":{"session":"01a14aca-…","id":"b3.0","text":"一个在你电脑上干活的助手。"}}
← {"jsonrpc":"2.0","method":"view.update","params":{"session":"01a14aca-…","entry":{"id":"b3.0","kind":"reply","text":"我是一个在你电脑上干活的助手。",…}}}
← {"jsonrpc":"2.0","method":"view.add","params":{"session":"01a14aca-…","after":"b3.0","entry":{"id":"e5","kind":"end",…}}}
```

画的时候：

- 条目照 `id` 存。`view.add` 是新的一条，排在 `after` 那一条后面（`null` 是最前）；`view.update` 整条换掉，带了 `after` 的是挪了位置；`view.append` 把字接在那一条的 `text`（工具的是 `args`）后面；`view.hidden` 是撤销、恢复藏起或显示回几条；`view.remove` 拿掉一条。
- 同一条可能先 `add` 再 `update`，`update` 里的字可能已经包含没单独 `append` 过的部分。照整条的字画，别只拼增量。例子里记着每条印到了第几个字，只印多出来的。
- `kind` 有 `user`（人说的）、`reply`（回答正文，Markdown 原文，渲染是头的事）、`media`（AI 发出的图）、`group`（时间线的一段）、`thought`（一步思考）、`tool`（一步工具）、`end`（一轮的收尾）、`notice`（旁白：压缩、撤销、换模型这些，看 `what`）。
- `open: true` 的还在写。工具在跑时的 `progress`（最新一行进度）、`retitle_failed` 旁白只在视图流里推，翻页、重新订阅拿不到。
- 一轮结束时加一条 `kind` 是 `end` 的；会话状态 `status.state` 回到 `idle`。

每种条目的格、`notice` 的每种 `what`、会话状态的每一格见 [视图流](../reference/view.md)。

### 掉队和重连

读得太慢，核心给一个订阅攒到上限（会话 1024 条）就推 `resync {session, stream: "view"}`，这个订阅停了。会话停了也推 `resync`。重新订阅一次：回应里是最新一页和整份状态，照它整个换掉手里的。

断了重连：重新握手，重新订阅。往上翻历史用 `view.page`。

视图流是给界面画的；要原始事件的（例如自己存日志），订 `stream: "events"`，可带 `after` 补上漏掉的，见 [事件](../reference/events.md)。

## 确认和提问

AI 要做受保护的事（开放权限下的危险命令、只读下的写操作这类）时停下等确认；用 `ask_user` 提问时也等。两样都在会话状态里：`status.state` 是 `waiting`，`status.waiting` 每件一项 `{what, entry, call}`，`what` 是 `approve`（确认）或 `ask`（提问），`entry` 指向那一步工具的条目。

确认：

- 那一步的条目有 `approval`：`access`、`rule`、`detail`。带 `expires_at` 的到那一刻没人答就自动拒，头可以画倒计时。时长是配置 `permission.confirm.timeout`，出厂 300 秒，0 是一直等。
- 答：`session.answer {"session", "call", "decision"}`，`decision` 是 `once`（允许这一次）、`session`（这个会话都允许，要条目里有 `rule` 才能选）、`deny`（拒绝，可带 `reason` 给 AI 看）。

提问：

- 那一步的条目有 `questions`：每题 `question`、可选的 `header`、`options`（`label`、`description`、`preview`）、`multiple`。提问一直等，不超时。
- 答：`session.answer {"session", "call", "answers"}`，照题目先后一题一项 `{"picked": [选的选项的 label…], "text": <自己写的>}`。

`waiting` 里带 `nested` 的（AI 在命令里经 `miyu call` 调的工具要确认），回答时照写。几个头开着同一个会话时，谁先答算谁的，后答的回 `not_asking`。每个格和出错见 [会话](../reference/sessions.md#sessionanswer)。

没有人看着的头（定时跑的脚本、只推通知的程序），握手不写 `caps.input` 或写 `false`。它开的会话里要确认的不跑，提问直接得到「没人能回答」，AI 照这个接着干，不会卡住等人。

## 会话的其余操作

| 界面上的动作 | 方法 |
|---|---|
| 新建会话 | `session.create` |
| 列会话、跟着列表变 | `session.list`；订 `sessions` 流，推 `sessions.changed` |
| 告诉核心你正开着哪个会话 | `session.viewing`：开着的会话的后台通知由你出，别的头不再弹 |
| 往上翻历史 | `view.page` |
| 发一句 | `session.send` |
| 带附件发 | 先 `blob.put`（`path` 交本机文件，或 `data` 交 base64），把回应的 `blob`、`name`、`media_type` 放进 `session.send` 的 `attachments` |
| 打断 | `session.interrupt`，`queued` 必写：排着的话 `send`（打断后马上发）、`return`（退回来）、`keep`（留着） |
| 退回一条排着的话 | `session.withdraw` |
| 撤销、恢复 | `session.revert`、`session.unrevert` |
| 重做最后一轮 | `session.redo` |
| 压缩、清空上下文 | `session.compact`、`session.clear` |
| 换人格、预设、模型、看图模型、思考强度 | `session.configure` |
| 切只读、切回开放权限 | `session.set_permission_level`，写 `{"read_only": true}` 或 `false` |
| 换工作目录 | `session.set_workspace` |
| 改标题、置顶 | `session.set_meta`；让模型重起标题用 `session.retitle` |
| 分叉 | `session.fork` |
| 导出 | `session.export`：`format` 写 `bundle`（原始包）或 `markdown` |
| 删除 | `session.delete` |

多数方法只回 `{}` 或几条事件的序号，变化本身照推送到达。每个方法见 [会话](../reference/sessions.md)。

**前缀要变**：握手写了 `caps.prefix_ask: true` 的头，开新一轮的请求（`session.send`、`session.redo`、开轮的 `command.run`）可能回 `prefix_changing`：人格、预设、提示词这些文件改了，下一轮的缓存会失效。`data.files` 是改了的文件。问人「直接发送」「压缩上下文并发送」「取消」，选了的带 `"prefix": "keep"` 或 `"compact"` 再发一次。不声明的头照旧直接发。

## 斜杠命令

- 列：`command.catalog {"session"}`，回 `commands`（`name`、`aliases`、`summary`、`argument`、`usage`、`opens`）和 `skills`（输入框里 `$技能名` 点名的技能）。
- 执行：人敲的整行交 `command.run {"session", "text"}`，回 `said`（回执，原样给人看）。带 `opens` 的回应是打开一个扩展的整页（见 [往界面上放东西](ui-surfaces.md)）。
- 头自己的命令（`/help`、换主题这类）头自己认、自己办，拼进同一个菜单。

核心认的命令和出错见 [软件包与命令](../reference/packages.md#commandrun)。

## 头自己的设置

- 皮肤、配色这类跟着人走的设置，存进核心：`prefs.get {"package"}`、`prefs.set {"package", "values"}`，`package` 是你这个头的软件包编号，整份最多 64 KiB。别的头改了推 `prefs.changed`（订 `prefs` 流，带 `package`）。只有装成界面包（下面）的头能用，别的回 `unknown_package` 或 `not_a_ui_package`。见 [头自己的设置](../reference/prefs.md)。
- 只跟这台设备有关的（窗口大小、输入历史）存在头自己那里。
- 核心的配置用 `config.get` 读；订 `config` 流，变了推 `config.changed`。头该照着办的几项：`ui.language`（界面语言）、`ui.startup`（打开头时开新会话 `new`，还是接最近的 `recent`）。
- 你自己的配置项写进清单的 `[settings.<名字>]`，键是 `<前缀>.<名字>`，设置页照它画（见 [清单](../authors/manifest.md)）。

## 用 Rust 写：`miyu-client`

核心仓库里的 crate `miyu-client` 是给头用的门面。照钉住的提交依赖：

```toml
[dependencies]
miyu-client = { git = "https://github.com/miyu-agent/miyu", rev = "<核心的提交号>" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt", "io-util"] }
```

核心的 crate 要 Rust 1.97 以上。`miyu-client` 用 AGPL-3.0-or-later；只经协议和核心说话的程序不受它管。

它做的是连接、找数据根、拉起核心；握手、订阅、收发由你自己写，和上面一样：

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

`connect_or_start` 连上就交回连接和本机令牌；核心没在跑的照你给的命令拉起，等它那一行，几个头同时拉起时只起一个。它不替你重连：断了你再调一次、重新握手、重新订阅。

| 模块 | 给什么 |
|---|---|
| `connect` | `connect`（只连）、`connect_or_start`（没在跑就拉起）、`Connection`（异步读写的字节流）和错误类型 |
| `places` | `DataRoot`（`locate`、`prepare`）、`Env`、`locale`、缓存目录、读写配置文件 |
| `open` | `Core`：出示本机令牌握手、一问一答，不收推送。只要问几句就走的程序（命令行工具）用它 |
| `texts` | 核心给人看的字，照语言读 |
| `protocol`、`ids`、`time`、`events` | 会话编号、事件、时刻这些协议里的类型 |
| `settings` | 配置的种类和读法 |
| `widgets` | 界面块内容的查验和挑语言 |
| `manifest` | 读自己的软件包清单 |
| `log`、`heap`、`open_files` | 运行日志、进程的堆、打开文件数上限 |
| `seeds` | 从一张图挑主题色的候选 |

几样默认不开，要的打开 feature：`view`（视图流条目、推送、会话状态的类型）、`wallpaper`（找桌面壁纸、跟着换）、`http`、`tool`。

## 做成软件包

做成界面包以后，`miyu tiny` 打开你的头，人还能把它设成直接敲 `miyu` 时打开的那一个。

```text
tiny-head/
├── package.toml
└── bin/
    └── tiny-head      # 上面那个 Python 文件，Linux、macOS 上带执行权限
```

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

1. 装：`miyu pkg install ./tiny-head`。
2. 打开：`miyu tiny 你好`。人敲的参数原样交给你的程序，环境里有 `MIYU_PROGRAM`。Linux、macOS 上 `miyu` 换成你的程序；Windows 上 `miyu` 起你的程序、等它退出，退出码照你的，按 `Ctrl+C` 送到你的程序。
3. 设成默认：`miyu config set ui.head tiny-head`。之后在终端里直接敲 `miyu` 就打开它（标准输入、标准输出都是终端时才这样）。装了两个以上界面时，设置页「通用」里也有「默认界面」一项。

- `[ui]` 有它就是界面包。`opens` 写你认的页，例如 `opens = ["config"]`：人敲 `miyu config` 时核心打开你的头、带上 `--page config`；没写的，`miyu config` 不打开你。
- `[command]` 的每一格、`[settings]` 怎么写见 [清单](../authors/manifest.md)。`[ui]` 这张表只给界面包用，不在那一页列的冻结格式里，以后可能改：升级核心以后先对一遍。
- Windows 上核心先找 `bin/tiny-head.exe`；没有的照 `bin/tiny-head` 第一行的 `#!` 找解释器，`#!/usr/bin/env python3` 找的是 `PATH` 里的 `python`。所以这个包三个平台照用，文件名不加 `.py`，Windows 上要装好 Python、`python` 在 `PATH` 里找得到。见 [写一个 Miyu 软件包：三个平台](../authors/README.md#三个平台)。
- 带编出来的程序的，写 `targets`，一种系统打一个包；打包、发到软件源见 [写一个软件包](../authors/README.md)。

## 网页类的头

浏览器连不了本机套接字。出厂网页是这样接的：网页服务 `miyu-web` 在本机听一个端口，浏览器经 WebSocket 连它，它把每一行原样转给核心，握手的凭据由页面自己出示（`miyu-web` 不读本机令牌）。

要做网页类的头，有两条路：

- **自己起一个本机服务转发**，照上面那样。转的是别的机器来的连接时，在握手之前先替它写一行 `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":"<来源地址>"}}`：这个连接就按远程算，读本机文件、交本机路径的方法一律回 `local_machine_only`，凭据要用访问密码。见 [连接](../reference/connection.md#怎么连)。
- **往出厂网页里加东西**：见 [往界面上放东西](ui-surfaces.md)。

## 要注意的

- 协议 1 是冻结的：已有的方法、字段不删、不改名、不改意思，只加。新的方法老核心回 `unknown_method`，新的字段老核心不理；用新东西之前看握手回应里核心的版本，或者照 `unknown_method` 退回去。
- 判断出了什么错看 `error.data.reason`，别看 `error.message`：那是照连接的语言写给人看的一句。全部原因码见 [出错](../reference/errors.md)。
- 每条消息一行，最长 1 MiB。大的内容走 `blob.put`，超过上传上限的文件只交路径（见 [会话](../reference/sessions.md#sessionsend)）。
- 请求的 `id` 每个连接都要和以前用过的不一样：核心照 `id` 认断线重发，撞上旧的拿到的是上一次的回应、什么都不做。每次连接取一个随机前缀，后面接递增的数字。
- 你不是唯一的头。终端、网页、托盘可能同时开着同一个会话：别人发的话、别人答的确认、别人改的设置，都照推送更新，别假设只有你在改。
