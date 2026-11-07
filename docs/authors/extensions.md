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
- 环境照核心的，另有 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`、`MIYU_PROGRAM`（主程序 `miyu` 的真实位置：要拉起核心的跑 `$MIYU_PROGRAM core`，别照自己旁边找，装进数据根的包旁边没有主程序），和你这个包在数据根里各自的那一格，都已经建好、是真实位置：
  - `MIYU_APP_DIR`：你这个包的文件夹（包里带的文件照它找，别自己拼路径）；
  - `MIYU_CONFIG_DIR`：给人手改的配置（`etc/<编号>/`；卸包默认留下，重装照旧读，`miyu pkg remove --purge` 才删）；
  - `MIYU_STATE_DIR`：数据库、登录态这些要留着的（`var/lib/<编号>/`，被拉起时也是工作目录）；
  - `MIYU_CACHE_DIR`：删了能重新生成的（`var/cache/<编号>/`）；
  - `MIYU_LOG_DIR`：你自己的运行日志（`var/log/<编号>/`；核心记的标准错误也在这里，叫 `stderr`、`stderr.old`，这两个名字别用）；
  - `MIYU_TMP_DIR`：短命的暂存（`tmp/<编号>/`，每次拉起前清空）；
  - `MIYU_RUNTIME_DIR`：锁、套接字、地址这类跑着才有意义的（`run/<编号>/`）；
  - `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR`：给人的文件存这里。

  人在终端敲 `miyu <你的命令>` 时也设这一套。旧名字 `MIYU_PACKAGE_DIR`（同 `MIYU_APP_DIR`）、`MIYU_PACKAGE_STATE`（同 `MIYU_STATE_DIR`）过渡期间照样给，下一版去掉。
- 你的东西照上面分开放，卸载时这几格一起删（给人的四处不删；配置目录照 pacman 默认留下，卸之前那一屏写「配置留在 <路径>」，`--purge` 的才删，写「会删掉 <路径>」）。给人改的东西放 `MIYU_CONFIG_DIR`，卸了重装还在；你自己用的状态放 `MIYU_STATE_DIR`，卸包会删。人的数据别放 `MIYU_STATE_DIR`：卸之前那一屏写「会删掉 <路径>（<大小>）」，删了就没了；要留给人的放 `MIYU_DOCUMENTS_DIR`（`root/documents/`）这些人看得见的地方。一律照这些变量找，别靠工作目录（被拉起时工作目录是 `MIYU_STATE_DIR`，可人敲命令时是人当时所在的目录），也别自己拼 `MIYU_HOME` 下的路径。
- 这个变量只给扩展本身和人敲的命令用：她在沙盒里跑的脚本读不到状态目录（它在数据根里，沙盒整个藏着）。
- 标准错误接到 `~/.miyu/var/log/<编号>/stderr`（就在 `MIYU_LOG_DIR` 里），出了问题先看它。

## 握手和配置

第一条发 `hello`，不用出示凭据。回应里：

- `config`：你这个包的配置项的最终值（`{"hello.greeting": "Hello"}`，密钥是真值，没设的键不放）。**不要自己读配置文件**。
- `language`：给人看的字该用哪种（`zh`、`en`、`ja`）。

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
| `role` | 职能：`read_file` 表示这件是读文本文件的（参数里给路径）。核心照职能认、不看名字：你替换了读文件的工具，AI 照样看得到技能列表。不写是没有 |
| `scope` | 给哪种会话，在 `venues` 之内再收窄：`local`（本机的会话）、`local_main`（本机的主会话，子代理没有）、`local_main_attended`（本机的主会话、有人看着，比如要问人的工具）、`spawn`（派子会话的工具）。不写是 `any` |
| `paths` | 哪几个参数是文件路径、是读是写：`[{"parameter": "file_path", "write": true}]`。读写文件的工具都该写：核心照它做权限检查（工作区外的写要问人），你回报的文件效果才收得下。见下面「改文件的工具」 |
| `background_parameters` | 只在会话能放到后台时才有意义的参数名（比如 `["run_in_background"]`）：后台运行关着的会话里，AI 看到的参数格式没有它们。要是不必填的 |
| `runs_commands` | `true`：这件工具会跑命令（替换内置 `shell` 的、在容器里跑命令的这类）。清单要声明能力 `tools.commands`、人批过，不然整份 `provide` 回 `needs_approval`。见下面「跑命令的工具」 |

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
- `blocks` 是给模型看的内容：文字 `{"type": "text", "text"}`；图片先经 `blob.put` 传上去，再放 `{"type": "image", "blob", "media_type", "width", "height"}`。包里自己带的图用 `{"path": "<MIYU_APP_DIR>/files/cat.png"}` 传就行（数据目录里别处的文件不收）。
- 画出来、生成的图存到哪：会话里的调用多一格 `pictures`，是数据根里放图的目录（真实位置，可能还没建，写之前自己建好），存在这里人在终端、网页里找得到；没有这一格的（会话外面的调用、群和私聊这类场所会话、老的核心）照旧存工作区：场所会话只碰得到自己的工作区，存到别处她就读不到、发不出去。
- 失败的照样回结果、`error` 写 `true`，把原因写给模型看；回 JSON-RPC 的错误也算失败。
- 想让她「发出去」一张图（表情包这类，画在正文里，聊天平台当图发）：回答里加 `"sent": [{"blob", "media_type", "width", "height", "alt"?, "sticker"?}]`，至多 4 张。`sent` 不给 AI 看。
- 想让头把结果画成卡片：回答里加 `detail`，照三种通用形状写一种：链接列表 `{"kind": "links", "links": [{"title", "url", "snippet"?}]}`、表格 `{"kind": "table", "columns": [...], "rows": [[...]]}`、图片 `{"kind": "images", "images": [{"blob", "media_type", "caption"?}]}`。`detail` 不给 AI 看；形状不对的核心丢掉这一格，运行日志里记一行 `tool detail dropped`。写法和上限见 [协议参考](../reference/extensions.md#toolcalltoolcancel)。
- 写了 `clicall` 的工具，想让脚本拿到结构化的结果：回答里加 `data`，任意 JSON（比如 `{"items": [...]}`；搜不到的也交同一个样子，列表是空的，脚本照 JSON 解析不会崩）。AI 在命令里 `miyu call` 调到时，脚本拿到它的一行 JSON，没有 `data` 的拿到 `blocks` 里的字。`data` 不给 AI 看，`error` 是真的不交。
- 同一时间可能来好几个 `tool.call`，照 `id` 对着回。
- 过了 `timeout_ms`、人打断了，核心发通知 `tool.cancel {"session", "call_id"}`：放下那一件。

## 改文件的工具

自己写一件编辑器、格式化、改名这类工具，想让它和内置的 `edit` 一样：时间线上有增删行数、能看差异，AI 读过的文件被你改了不用重读。

1. 规格里写 `paths`，说哪个参数是路径、要不要写，`access` 写 `write`：

   ```json
   {"name": "my_edit", "access": "write", "venues": ["local"],
    "input_schema": {"type": "object", "properties": {"file_path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, "required": ["file_path", "old", "new"]},
    "paths": [{"parameter": "file_path", "write": true}]}
   ```

   参数要是 `input_schema.properties` 顶层的一个，类型是字或字的数组（数组的每一项各算一条）。核心照它做权限检查，和内置的 `edit` 一样：工作区一级下写工作区外的文件先问人，人点头以前你收不到 `tool.call`；Miyu 自己的数据目录、私密目录照规矩拒。没写 `paths` 的工具核心不知道你要碰哪个文件，你报的文件效果一条都不收。
2. `tool.call` 多一格 `checked`：检查判过的真实位置，照参数里路径的先后。写这几个：你自己把参数里的路径换成真实位置（相对的照 `cwd`，`~` 照家目录，跟着链接走），不在 `checked` 里的别碰：检查以后路径上的链接可能被换过，写下去就不是人同意的那个。
3. 回答里加 `effects`，只说「哪个文件怎么了」，内容核心自己读：

   ```json
   {"blocks": [{"type": "text", "text": "Edited a.txt."}], "effects": [{"kind": "file.changed", "path": "a.txt"}]}
   ```

   - `file.changed {"path"}`：改了、新建了。改前是发 `tool.call` 之前核心读的那份，改后是你回答以后核心现读的；前后一样的不收。想多一层保险可以带 `before`、`after`（`sha256:` 开头的哈希）或 `before_text`、`after_text`，对不上的不收。
   - `file.read {"path", "lines"?}`：读了（`lines` 是 `[从, 到]`，从 1 数起，从不大于到）。AI 改文件之前照它核对看没看过。
   - `file.trashed {"path", "trash"}`：移进了回收站，`trash` 是回收站里的位置，只给人看。
   - `todo.written {"todos", "done"?}`：换上一整份待办，写法同内置的 `todowrite`（替换它的工具用）。
   - 路径要是 `checked` 里的：改、删的要判了写。别的种类、没判过的路径、写法不对的那一条丢掉，结果照常；运行日志记一行 `tool effect dropped`，排查时看 `why`。
   - 改了一半出了错，照样回 `error: true` 并报改了的文件：改了的是真的。
4. 撤销不改回文件（内置的也不改），只退回 AI 看过的：撤掉一轮以后再改那个文件，AI 要先重读。

## 跑命令的工具

自己写一件跑命令的工具（换掉内置的 `shell`、在容器或远程机器上跑），想让 AI 在它跑的命令里也能 `miyu call` 调工具，和内置的 `shell` 一样：

1. 清单 `[process] capabilities` 加上 `tools.commands`（装的时候人会看到「工具会跑命令」这一项），规格里写 `"runs_commands": true`，`access` 照实写（一般是 `execute`）。AI 看到的说明后面会自动接上 `miyu call` 怎么用那一段，你不用自己写。
2. 会话里有 AI 能在脚本里调的工具时，`tool.call` 多一格 `channel`：`{"sock", "token", "bin"}`。给这次调用起命令时设上环境变量：

   ```python
   env = dict(os.environ)
   if "channel" in params:
       ch = params["channel"]
       env["MIYU_CALL_SOCK"] = ch["sock"]
       env["MIYU_CALL_TOKEN"] = ch["token"]
       if "bin" in ch:
           env["PATH"] = ch["bin"] + os.pathsep + env.get("PATH", "")
   subprocess.run(command, shell=True, env=env, cwd=params["cwd"])
   ```

3. 命令要是你这个扩展进程起的（子进程、孙进程都行）：核心照父进程一层层往上认到你的进程，光有令牌不够。`setsid`、两次 `fork` 甩出去的，或者交给别的常驻进程去跑的，调不成（退出码 2）。这一条是防误用的（令牌漏给了别的进程、命令甩出去以后还在调），不是防你的扩展：你起一个子进程就能调。真正的关口是上面第 1 条，`tools.commands` 要人批。
4. 这次调用一回应，通道就关了：命令里的 `miyu call` 要在你回应之前跑完。放到后台、回应以后还在跑的命令调不成（退出码 3）。
5. 命令里调的工具照这个会话的权限、预设走，和 AI 直接调一样，会记在这次调用下面。人在会话里打的 `!命令` 不会交给你，只给内置的 `shell`。

## 写一个模型驱动

借订阅登录的、走 WebSocket 的、自己签名的模型服务，Miyu 内置的三种驱动接不上，可以由你的扩展来接：Miyu 交统一的请求，你编成线上的格式、登录、发出去、解响应，照增量交回来；重试、冷却、换池、计价都在 Miyu。

清单声明能力 `model.driver`，写一段 `[[driver]]`：

```toml
[process]
capabilities = ["model.driver", "network"]

[[driver]]
name = "relay"          # 驱动名
cache = "contract"      # 前缀不变就命中缓存：要实现 driver.encode
key = "required"        # 要 Miyu 交这一家的 key
images = false          # 看得了图的写 true
```

主人在供应商的配置里写 `driver = "<你的包编号>/relay"` 才用它（地址可以不写）。用到它的模型时，Miyu 发 `driver.call`：

```json
{"jsonrpc":"2.0","id":"core-7","method":"driver.call","params":{"call":"d3","driver":"pi-relay/relay","request":{"tools":[],"system":"…","messages":[{"role":"user","blocks":[{"type":"text","text":"你好"}]}],"stable":0},"fingerprint":{"tools":"…","system":"…","messages":["…"]},"params":{"model":"relay-pro","max_output":4096},"key":"sk-…"}}
```

你照模型回来的一点点交 `driver.delta`（通知，不带 `id`），最后回答：

```json
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"start":{"index":0,"kind":"text"}},{"text":{"index":0,"text":"你好！"}}]}}
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"end":{"index":0}}]}}
{"jsonrpc":"2.0","id":"core-7","result":{"usage":{"uncached":12,"cache_read":0,"cache_write":0,"output":3},"sent":"sha256:…"}}
```

- **编码只看参数**：不放时间、随机数，字段的先后固定，`call` 不编进去。同样的请求编出同样的字节，前缀缓存才命中；`sent` 报真发出去的字节的哈希，同一个请求两次报得不一样的，Miyu 记一行警告。
- **出错照类报**：`{"error": {"class", "message", "retry_after_ms"?}}`，`class` 是 `retryable`、`rate_limited`、`context_too_long`、`auth`、`quota`、`content_policy`、`unclassified` 之一。交了增量的别自己重试（回复会重），Miyu 照分类重试、换池；还没交增量时连不上、要刷新令牌，你自己处理。
- **私有数据**（思考的签名、供应商自己的调用编号）照 `{"private": {"index", "data"}}` 交，以后的请求里原样交回给你，别的驱动看不到。
- **时限**：两条增量之间、最后一条到回答都不能超过空闲时限（180 秒起，照思考强度放大）；超了、人打断了，Miyu 发 `driver.cancel {"call"}`，那一次别再发。
- **key**：只有这一家配置里的那一把；`key = "none"` 的不交。自己登录拿的令牌存在你自己的数据目录里。

**查编码稳不稳**：实现 `driver.encode`（参数同 `driver.call`，回 `{"body": "<你会发的字节>"}`，不真发），跑 `miyu dev driver-check <你的包编号>`：每份样本编两次、重启你再编一次，末尾加一条消息再编一次，有一处字节对不上就红。声明 `contract` 的必须过。

**列模型**（可选）：实现 `driver.models`（参数 `{"driver", "key"?, "base_url"?}`），回 `{"models": [{"id", "window"?, "max_output"?, "tools"?, "vision"?}]}`，主人的模型列表里就有了，窗口、最大输出、能不能调工具、看不看得了图照你报的算（主人手写的压过你的）；不实现的，主人手写模型名。

每个参数、增量、出错的细节见 [协议参考](../reference/extensions.md#drivercalldriverdeltadrivercancel)。

## 退出、崩了

- 标准输入读到头就是「请退出」，三个平台一样；5 秒内不退会被杀掉。
- 核心崩了、被强行结束时，你和你起的子进程跟着没：Unix 上整个进程组一起结束（`setsid` 出去、自己换了进程组的不算），Windows 上同一个作业对象里的一起结束。要收尾的事别指望这时候做得了。
- 你自己退出（退出码 0 也算）、握手不成、标准输出关了，都算一次失败，核心隔 1、2、4、8、16 秒再拉起，连续 5 次停下。
- **退出码 1 留给「配置错、端口被占」这类重启也没用的**：核心直接停下、不重试，人改好再开。
- 连着跑满 60 秒的，下次失败从头数。

## 更多本事

每一样都要在清单里声明对应的能力（见 [清单](manifest.md)）。

**自己的斜杠命令**（`commands`）：清单写 `[[slash]]`。人敲了，核心发 `command.call {"command", "argument", "text", "session", "by", "owner", "language"}`，你答 `{"said": "<回执，可以多行>"}`，这段话原样给人看。你的程序起来以后要 `provide` 一次（没有工具的写空的），核心才认得你的连接。

**当搜索服务供应商**（`search`）：清单写 `[[search]]`。人在设置里选了你，AI 搜网页时核心发 `search.query {"source", "query", "want", "allowed_domains", "blocked_domains"}`，你答 `{"hits": [{"title", "url", "snippet"?, "date"?}]}`；限额了回错误、`data.reason` 写 `limited`，核心让你歇一阵。同样要先 `provide` 一次。

**回合开始往她的上下文里加几句**（`context.inject`）：每一轮开始核心发 `user.append {"session", "venue", "text"?}`，300 毫秒内答 `{"content": [{"kind", "text"}]}`（至多 3 块）。没得加就答 `{"content": []}`。只问这个会话的工具面上有你的工具的扩展。字是给模型看的：英文短句，外面套一层 XML 标签。

**工具调用前拦一道**（`tool.guard`）：清单写 `[guard]`，列出要问的工具名、访问类别（见 [清单](manifest.md)）。AI 的调用过了权限策略、执行之前，核心发 `tool.guard {"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`，你答 `{"verdict": "allow"}`、`{"verdict": "deny", "reason": "<为什么>"}` 或者 `{"verdict": "ask", "reason": "<为什么>"}`（要人确认）。拒绝时 AI 看到 `Blocked by <你的包编号>: <理由>`，理由截到 300 个字、转义成一行：写一句英文，说清为什么、该怎么改（守卫不改参数，要改的让她重调）。放行不进上下文。不用 `provide`。

- 答得快：人一直等着这一步，p95 最好在 20 毫秒以内；到了 `timeout_ms`（不写是 1 秒）没答的算拒绝。
- 你没在跑（正在起、崩了）的时候，挂在你身上的调用一律拒绝：守卫失效往严里走。别在握手以后才去慢慢加载规则。
- 人打断了、你超时了，核心发 `guard.cancel {"session", "call_id"}`，那一次你不用再答。
- 几个守卫都问，有一个拒绝就拒绝；你只能收紧权限策略，放不开它。

**工具调用以后记一笔**（`events.read`）：清单写 `[after]`，列出要收的工具名、访问类别（见 [清单](manifest.md)）。调用的结果记进会话以后，核心发通知 `tool.after`（不带 `id`，别回）。只读：改不了结果，也不进她的上下文。直接调的和脚本里调的（`miyu call`）形状不一样，照有没有 `blocks` 分：

```python
def on_tool_after(params):
    if "blocks" in params:
        # 直接调的：call_id 像 call_12_1，blocks 是 AI 看到的内容
        log(params["session"], params["call_id"], params["tool"], params["status"], params["blocks"])
    else:
        # 脚本里调的：call_id 像 call_12_1#3，结果只给 blob 的引用（result、bytes），要内容自己 blob.get
        log(params["session"], params["call_id"], params["tool"], params["status"], params.get("result"))
    # args 两种都是一段 JSON 字（不是对象）：要看参数自己 json.loads(params["args"])
```

- 不等你：每个扩展一条至多 256 条的队列，你读得慢、满了丢新来的；你崩了、重启了，中间的不补。**`tool.after` 不保证送达**，要完整的审计订事件流（`subscribe` 的 `events`）。
- 只收得到你碰得到的会话：不是平台桥的只收本机的会话和主人本人没关起来的私聊；平台桥只收自己平台的场所会话。

**往系统提示词末尾放一段**（`context.system`）：调 `system.append {"content", "session"?}`。不写 `session` 管全部会话；`content` 写 `null` 去掉。改了下一轮整段重算一次，别频繁改；只想放固定几句的，起来时调一次就行。

**按意思搜**：调 `model.embed {"texts": [...]}` 拿向量（用人在设置里选的语义模型）。没配的回 `embedding_off`，退回关键词搜就好。不用另批能力。

**叫一次模型**：调 `model.call`（例如让 AI 给一张图写描述，`"use": "vision"`）。会花人的额度，别滥用。在对话以外、会反复调的（判断要不要回、打分），在清单里用 `[[side]]` 登记、调的时候写 `"side": "<编号>"`：照登记的档位用人放进 `lite`、`normal`、`flagship` 池的模型，人不用为你另配（见 [清单](manifest.md)）。

**后台页和界面块的方法**：发 `package.methods {"methods": [{"name": "status"}]}` 登记；页面、按钮调到时核心发 `method.call {"method", "params"}`，你回什么对面就拿到什么，不认识的方法回 JSON-RPC 的 `-32601`。

**界面块**（`widgets`）：清单写 `[[widget]]`。有人在看你的一块时核心发 `widget.watch {"widget", "session"?}`，最后一个人不看了发 `widget.unwatch`；只在有人看时取数据，别定时刷。推内容用 `widget.set {"widget", "session"?, "title"?, "content"}`：

```json
[{"type":"heading","text":"本周"},{"type":"text","text":{"zh":"晴 23°C","en":"Sunny 23°C"},"tone":"accent"},{"type":"pairs","items":[{"label":"湿度","value":"40%"}]},{"type":"progress","value":0.4,"text":"索引 40%"},{"type":"list","items":[{"title":"构建 #128","right":"3m","state":"doing","action":{"call":"build.open","params":{"id":128}}}]},{"type":"buttons","items":[{"label":"刷新","action":{"call":"refresh"}}]}]
```

只有字：不认 Markdown、HTML、颜色码。整页（`place = "page"`）另能放可以改的表单（`sections`、`form`）。段、表单的每一格见 [协议参考：界面块](../reference/extensions.md#widgetsetwidgetclearwidgetwatchwidgetunwatch)。

**让别的包调你、你调别的包**：给别人调的方法写进清单 `[export] methods`（照样要 `package.methods` 登记）；别人调到时 `method.call` 多一格 `from`（调的包的编号，冒充不了）。要调别人的，声明 `packages.call`，发 `package.call {"package", "method", "params"}`。

**读别的包给你带的内容**：别的包在清单里写 `[content.<你的编号>] dir = "…"` 就是给你带了一个目录。调 `content.list {}` 看谁带了，`content.read {"package", "path", "offset"?}` 分块读；装、卸、升级以后核心推 `content.changed`。

**往会话里记自己的事件**：`events.append {"session", "kind": "ext.<你的前缀>.<名字>", "body"}`。不开回合、不给模型看，界面和你以后读日志时看得到。前缀是清单里的 `prefix`，没写的是包的编号。

## 别做的

- 别读写数据根里别的地方（会话、别人的包、配置文件、密钥文件）：要信息走协议。
- 只给人用的方法（替人回答确认、改权限级别、删会话、改配置、装卸软件包、`extension.*` 这些）扩展调了会被拒（`local_only`）。
- 能力**照实声明**：没声明的，调对应的方法回 `needs_approval`，`data.capabilities` 写着缺哪一项。你不带 `as` 说的话记成你自己（名字是包的编号），不是本人。
