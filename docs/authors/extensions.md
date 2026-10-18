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

装上试：`miyu pkg install ./hello`，答「现在打开？」是；然后对 AI 说「跟 Ann 打个招呼」。Windows 上 `bin/` 里要放 `.exe`：用 PyInstaller 打包，或者换一种编得出可执行文件的语言。

## 被拉起

- 程序是 `bin/` 下那一个，参数是 `[process] args`。
- 环境照核心的，另有 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`、`MIYU_PACKAGE_DIR`（你这个包的文件夹：包里带的文件照它找，别自己拼路径）。人在终端敲 `miyu <你的命令>` 时也设 `MIYU_PACKAGE_DIR`。
- 工作目录是这个包放状态的目录 `~/.miyu/state/packages/<编号>/`：你的数据库、缓存放这里，卸载时一起删。
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
| `access` | 会碰什么：`read`（只看，不出 Miyu）、`write`（改文件）、`execute`（跑命令）、`network`（上网）、`outbound`（往 Miyu 外面发东西）、`venue`（在聊天平台上做动作：撤回、禁言）。权限策略照它决定问不问人，**照实写** |
| `venues` | 给哪种会话：`local`（本机的）、`private`（平台私聊）、`group`（群） |
| `timeout_ms` | 1000 到 600000，默认 60000 |
| `display_name` | 给人看的名字，照语言：`{"zh": "打招呼", "en": "Greet"}` |
| `subject` | 时间线上名字后面跟哪一个参数的值，显示成「打招呼 · Ann」。搜索的、对着某个文件或编号的工具应当写：不写的时间线上只有名字，看不出在搜什么、动的是哪个 |
| `need_load` | `true`：AI 平时只看到名字和一行说明，要用时先加载 |

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
- 失败的照样回结果、`error` 写 `true`，把原因写给模型看；回 JSON-RPC 的错误也算失败。
- 想让她「发出去」一张图（表情包这类，画在正文里，聊天平台当图发）：回答里加 `"sent": [{"blob", "media_type", "width", "height", "alt"?, "sticker"?}]`，至多 4 张。`sent` 不给 AI 看。
- 想让头把结果画成卡片：回答里加 `detail`，照三种通用形状写一种：链接列表 `{"kind": "links", "links": [{"title", "url", "snippet"?}]}`、表格 `{"kind": "table", "columns": [...], "rows": [[...]]}`、图片 `{"kind": "images", "images": [{"blob", "media_type", "caption"?}]}`。`detail` 不给 AI 看；形状不对的核心丢掉这一格，运行日志里记一行 `tool detail dropped`。写法和上限见 [协议参考](../reference/extensions.md#toolcalltoolcancel)。
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
- 只给本机的人用的方法（改配置、装卸软件包、`extension.*` 这些）扩展调了会被拒（`local_only`）。
- 能力**照实声明**：现在还没全强制，以后没声明的会被挡掉。
