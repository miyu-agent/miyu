# 写扩展

扩展是一个独立的程序，什么语言写都行。核心拉起它，经标准输入输出说 JSON-RPC 2.0，一行一条。扩展先握手，再把自己的工具登记进来；AI 调到这些工具时，核心反过来请扩展去做。英文版：[extensions.en.md](extensions.en.md)。每个方法的完整说明在 [协议参考：扩展](../reference/extensions.md)。

## 一个最小的扩展

```text
hello/
├── package.toml
└── bin/
    └── miyu-hello
```

`package.toml`：

```toml
[package]
protocol = [1, 1]
name = { en = "Hello", zh = "打招呼" }

[command]
name = "hello"
program = "miyu-hello"
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]
args = ["serve"]
capabilities = ["tools"]

[settings.greeting]
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }
```

`bin/miyu-hello`（Python；Linux、macOS 上 `chmod +x`）：

```python
#!/usr/bin/env python3
import json
import sys

TOOLS = [{
    "name": "hello_greet",
    "description": "Greet someone by name.",
    "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
    "access": "read",
    "venues": ["local"],
    "display_name": {"en": "Greet", "zh": "打招呼"},
    "subject": "name",
}]


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def serve():
    send({"jsonrpc": "2.0", "id": "h1", "method": "hello",
          "params": {"protocol": [1, 1], "head": {"kind": "hello", "version": "0.1.0"}}})
    greeting = "Hello"
    for line in sys.stdin:  # 读到头就是核心请你退出
        message = json.loads(line)
        if message.get("id") == "h1":
            if "error" in message:
                sys.exit(1)  # 握手被拒：重启也没用
            greeting = message["result"].get("config", {}).get("hello.greeting", greeting)
            send({"jsonrpc": "2.0", "id": "p1", "method": "provide", "params": {"tools": TOOLS}})
        elif message.get("method") == "extension.config":
            greeting = message["params"]["keys"].get("hello.greeting") or "Hello"
        elif message.get("method") == "tool.call":
            name = message["params"]["args"].get("name", "there")
            send({"jsonrpc": "2.0", "id": message["id"],
                  "result": {"blocks": [{"type": "text", "text": f"{greeting}, {name}!"}]}})


if __name__ == "__main__":
    if sys.argv[1:] == ["serve"]:
        serve()
    else:
        print("usage: miyu hello serve (started by the Miyu core)", file=sys.stderr)
        sys.exit(2)
```

装上试：`miyu pkg install ./hello`，答「现在打开？」是；然后对 AI 说「跟 Ann 打个招呼」。Python 写的照这样直接用，三个平台都行：`bin/miyu-hello` 第一行写 `#!/usr/bin/env python3`，Linux、macOS 上给它可执行权限，Windows 上核心照这一行找 `python`。要求装了 Python 3 的，写进包的说明。

## 用开发包写

上面是手写的：协议不挑语言，什么语言都能照这样写。用 Python、Rust 的，有现成的开发包 [miyu-agent/miyu-sdk](https://github.com/miyu-agent/miyu-sdk)（MIT）：握手、一行一条的收发、几个调用同时来时照编号对着回、取消、配置推送、标准输入读到头就退、退出码 1 的规矩都包好了。它不多给本事，扩展能做什么照旧由协议定；另带一个假核心，不起 Miyu 也能测。

**Python**：一个文件 `miyu_ext.py`，只用标准库，Python 3.9 起，不用 pip。拷进包的 `bin/`，和程序放在一起；清单同上面那份：

```text
hello/
├── package.toml
└── bin/
    ├── miyu-hello        # 程序，第一行 #!/usr/bin/env python3
    └── miyu_ext.py       # 拷过来的开发包
```

```python
#!/usr/bin/env python3
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from miyu_ext import Extension

ext = Extension("hello", "0.1.0")

@ext.tool("hello_greet", "Greet someone by name.",
          {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
          access="read")
def greet(args, call):
    return f"{ext.config.get('hello.greeting', 'Hello')}, {args['name']}!"

if __name__ == "__main__":
    ext.run()
```

三个平台都照这一个文件跑，同上面手写的那份。配置在 `ext.config`，调核心的方法用 `ext.call(method, params)`。

**Rust**：crate `miyu-ext`，同步、只依赖 `serde_json`。它没发到 crates.io，照仓库的版本标签依赖：

```toml
[dependencies]
miyu-ext = { git = "https://github.com/miyu-agent/miyu-sdk", tag = "v0.1.0" }
```

有哪些版本见开发包的说明。编出来的程序放进包的 `bin/`，Windows 上是 `miyu-hello.exe`。

**测**：不起 Miyu，假核心 `FakeCore` 在进程里演核心：

```python
from miyu_ext import FakeCore
core = FakeCore(ext, config={"hello.greeting": "Hi"})
assert core.call_tool("hello_greet", {"name": "Ann"})["blocks"][0]["text"] == "Hi, Ann!"
core.close()
```

斜杠命令、方法、推送怎么写，Rust 的例子，完整的例子包，见开发包的说明，这里不抄。

## 被拉起

- 程序是 `bin/` 下那一个，参数是 `[process] args`。
- 环境照核心的，另有 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`、`MIYU_PACKAGE_DIR`（你这个包的文件夹：包里带的文件照它找，别自己拼路径）、`MIYU_PACKAGE_STATE`（你这个包的状态目录，已经建好）。人在终端敲 `miyu <你的命令>` 时也设 `MIYU_PACKAGE_DIR`、`MIYU_PACKAGE_STATE`。
- 你的数据库、缓存放在 `MIYU_PACKAGE_STATE` 指的目录里，卸载时一起删。一律照这个变量找，别靠工作目录（被拉起时工作目录也是它，可人敲命令时工作目录是人当时所在的目录），也别自己拼 `MIYU_HOME` 下的路径。
- 这个变量只给扩展本身和人敲的命令用：她在沙盒里跑的脚本读不到状态目录（它在数据根里，沙盒整个藏着）。
- 标准错误接到 `~/.miyu/state/logs/<编号>.stderr`，出了问题先看它。

## 握手和配置

第一条发 `hello`，不用出示凭据。回应里：

- `config`：你这个包的配置项的最终值（`{"hello.greeting": "Hello"}`，密钥是真值，没设的键不放）。**不要自己读配置文件**。
- `language`：给人看的字该用哪种（`zh`、`en`、`ja`）。
- `account`：你是谁（声明了系统账号的是那个账号）。

之后配置变了，核心推通知 `extension.config {"keys": {"hello.greeting": "Hi"}}`，只放变了的键，变成没设的是 `null`。连上 10 秒还没握手成的会被断开。

## 登记工具：`provide`

握手以后发 `provide`，`tools` 是你现在提供的**全部**工具，每发一次整份换掉。一件工具：

| 格 | 说明 |
|---|---|
| `name` | 英文字母、数字、`_`、`-`，1 到 64 个；不能和别的撞，建议带包的编号做前缀 |
| `description`、`input_schema` | 给模型看的：英文短句，一件最多三句 |
| `access` | 会碰什么：`read`（只看，不出 Miyu）、`write`（改文件）、`execute`（跑命令）、`network`（上网）、`outbound`（往 Miyu 外面发东西）、`venue`（在聊天平台上做动作：撤回、禁言）。权限策略照它决定问不问人，**照实写**：改你自己数据的工具（记账、删一条记录）也写 `write`，只读级别才拦得住 |
| `venues` | 给哪种会话：`local`（本机的）、`private`（平台私聊）、`group`（群） |
| `timeout_ms` | 1000 到 600000，默认 60000 |
| `display_name` | 给人看的名字，照语言：`{"zh": "打招呼", "en": "Greet"}` |
| `subject` | 时间线上名字后面跟哪一个参数的值，显示成「打招呼 · Ann」。搜索的、对着某个文件或编号的工具应当写：不写的时间线上只有名字，看不出在搜什么、动的是哪个 |
| `need_load` | `true`：AI 平时只看到名字和一行说明，要用时先加载 |
| `clicall` | `true`：AI 在命令里写脚本时也能用 `miyu call` 调它，结果交给脚本。只读、联网、执行类的才调得成，不写是不能。要读、要查，并且 AI 在命令里自己不方便做到的工具，都该写上：AI 能在脚本里批量调，省下一轮轮来回；会改东西、发东西的别写。有结构化结果的写进回答的 `data`，脚本拿到一行 JSON |

写错的整份不收，回应的 `data` 说哪一件、哪里不对（`{"tool": "hello_greet", "problem": "name"}`）。

## 被调：`tool.call`

```json
{"jsonrpc":"2.0","id":"core-1","method":"tool.call","params":{"session":"…","call_id":"…","tool":"hello_greet","args":{"name":"Ann"},"cwd":"/home/me/src","owner":true}}
```

回应：

```json
{"jsonrpc":"2.0","id":"core-1","result":{"blocks":[{"type":"text","text":"Hello, Ann!"}]}}
```

- `args` 已经照你的 `input_schema` 修正过。`by` 是谁让她做的，`owner` 是核心认不认这个人是主人本人：在群里做动作的工具，谁能用要你自己照它们挡。
- `blocks` 是给模型看的内容：文字 `{"type": "text", "text"}`；图片先经 `blob.put` 传上去，再放 `{"type": "image", "blob", "media_type", "width", "height"}`。包里自己带的图用 `{"path": "<MIYU_PACKAGE_DIR>/files/cat.png"}` 传就行（数据目录里别处的文件不收）。
- 画出来、生成的图存到哪：会话里的调用多一格 `pictures`，是会话属主家目录里放图的目录（真实位置，可能还没建，写之前自己建好），存在这里人在终端、网页里找得到；没有这一格的（会话外面的调用、老的核心）照旧存工作区。
- 失败的照样回结果、`error` 写 `true`，把原因写给模型看；回 JSON-RPC 的错误也算失败。
- 想让她「发出去」一张图（表情包这类，画在正文里，聊天平台当图发）：回答里加 `"sent": [{"blob", "media_type", "width", "height", "alt"?, "sticker"?}]`，至多 4 张。`sent` 不给 AI 看。
- 想让头把结果画成卡片：回答里加 `detail`，照三种通用形状写一种：链接列表 `{"kind": "links", "links": [{"title", "url", "snippet"?}]}`、表格 `{"kind": "table", "columns": [...], "rows": [[...]]}`、图片 `{"kind": "images", "images": [{"blob", "media_type", "caption"?}]}`。`detail` 不给 AI 看；形状不对的核心丢掉这一格，运行日志里记一行 `tool detail dropped`。写法和上限见 [协议参考](../reference/extensions.md#toolcalltoolcancel)。
- 写了 `clicall` 的工具，想让脚本拿到结构化的结果：回答里加 `data`，任意 JSON（比如 `{"items": [...]}`；搜不到的也交同一个样子，列表是空的，脚本照 JSON 解析不会崩）。AI 在命令里 `miyu call` 调到时，脚本拿到它的一行 JSON，没有 `data` 的拿到 `blocks` 里的字。`data` 不给 AI 看，`error` 是真的不交。
- 同一时间可能来好几个 `tool.call`，照 `id` 对着回。
- 过了 `timeout_ms`、人打断了，核心发通知 `tool.cancel {"session", "call_id"}`：放下那一件。

## 退出、崩了

- 标准输入读到头就是「请退出」，三个平台一样；5 秒内不退会被杀掉。
- 你自己退出（退出码 0 也算）、握手不成、标准输出关了，都算一次失败，核心隔 1、2、4、8、16 秒再拉起，连续 5 次停下。
- **退出码 1 留给「配置错、端口被占」这类重启也没用的**：核心直接停下、不重试，人改好再开。
- 连着跑满 60 秒的，下次失败从头数。

## 更多本事

每一样都要在清单里声明对应的能力（见 [清单](manifest.md)）。

**自己的斜杠命令**（`commands`）：清单写 `[[slash]]`。人敲了，核心发 `command.call {"command", "argument", "text", "session", "by", "owner", "language"}`，你答 `{"said": "<回执，可以多行>"}`，这段话原样给人看。你的程序起来以后要 `provide` 一次（没有工具的写空的），核心才认得你的连接。

**当搜索服务供应商**（`search`）：清单写 `[[search]]`。人在设置里选了你，AI 搜网页时核心发 `search.query {"source", "query", "want", "allowed_domains", "blocked_domains"}`，你答 `{"hits": [{"title", "url", "snippet"?, "date"?}]}`；限额了回错误、`data.reason` 写 `limited`，核心让你歇一阵。同样要先 `provide` 一次。

**回合开始往她的上下文里加几句**（`context.inject`）：每一轮开始核心发 `user.append {"session", "venue", "text"?}`，300 毫秒内答 `{"content": [{"kind", "text"}]}`（至多 3 块）。没得加就答 `{"content": []}`。只问这个会话的工具面上有你的工具的扩展。字是给模型看的：英文短句，外面套一层 XML 标签。

**往系统提示词末尾放一段**（`context.system`）：调 `system.append {"content", "session"?}`。不写 `session` 管全部会话；`content` 写 `null` 去掉。改了下一轮整段重算一次，别频繁改；只想放固定几句的，起来时调一次就行。

**按意思搜**：调 `model.embed {"texts": [...]}` 拿向量（用人在设置里选的语义模型）。没配的回 `embedding_off`，退回关键词搜就好。不用另批能力。

**叫一次模型**：调 `model.call`（例如让 AI 给一张图写描述，`"use": "vision"`）。会花人的额度，别滥用。

**后台页和界面块的方法**：发 `package.methods {"methods": [{"name": "status"}]}` 登记；页面、按钮调到时核心发 `method.call {"method", "params"}`，你回什么对面就拿到什么，不认识的方法回 JSON-RPC 的 `-32601`。

**界面块**（`widgets`）：清单写 `[[widget]]`。有人在看你的一块时核心发 `widget.watch {"widget", "session"?}`，最后一个人不看了发 `widget.unwatch`；只在有人看时取数据，别定时刷。推内容用 `widget.set {"widget", "session"?, "title"?, "content"}`：

```json
[{"type":"heading","text":"本周"},{"type":"text","text":{"zh":"晴 23°C","en":"Sunny 23°C"},"tone":"accent"},{"type":"pairs","items":[{"label":"湿度","value":"40%"}]},{"type":"progress","value":0.4,"text":"索引 40%"},{"type":"list","items":[{"title":"构建 #128","right":"3m","state":"doing","action":{"call":"build.open","params":{"id":128}}}]},{"type":"buttons","items":[{"label":"刷新","action":{"call":"refresh"}}]}]
```

只有字：不认 Markdown、HTML、颜色码。整页（`place = "page"`）另能放可以改的表单（`sections`、`form`）。段、表单的每一格见 [协议参考：界面块](../reference/extensions.md#widgetsetwidgetclearwidgetwatchwidgetunwatch)。

**让别的包调你、你调别的包**：给别人调的方法写进清单 `[export] methods`（照样要 `package.methods` 登记）；别人调到时 `method.call` 多一格 `from`（调的包的编号，冒充不了）。要调别人的，声明 `packages.call`，发 `package.call {"package", "method", "params"}`。

**读别的包给你带的内容**：别的包在清单里写 `[content.<你的编号>] dir = "…"` 就是给你带了一个目录。调 `content.list {}` 看谁带了，`content.read {"package", "path", "offset"?}` 分块读；装、卸、升级以后核心推 `content.changed`。

**往会话里记自己的事件**：`events.append {"session", "kind": "ext.<你的编号>.<名字>", "body"}`。不开回合、不给模型看，界面和你以后读日志时看得到。

## 别做的

- 别读写数据根里别的地方（会话、别人的包、配置文件、密钥文件）：要信息走协议。
- 只给人用的方法（替人回答确认、改权限级别、删会话、改配置、装卸软件包、`extension.*` 这些）扩展调了会被拒（`local_only`）。
- 能力**照实声明**：没声明的，调对应的方法回 `needs_approval`，`data.capabilities` 写着缺哪一项。你不带 `as` 说的话记成你自己（名字是包的编号），不是本人。
