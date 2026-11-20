# 写扩展

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

扩展是一个独立的程序，可以用任何语言来写。核心启动它，通过标准输入输出和它用 JSON-RPC 2.0 通信，一行一条消息。扩展先握手，再把自己的工具登记进来；AI 调用这些工具时，核心反过来请扩展去执行。每个方法的完整说明见 [协议参考：扩展](../reference/extensions.md)。

## 一个最小的扩展

一个最小的扩展只有两个文件：一份清单和一个程序。

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

`bin/miyu-hello`（用 Python 写；在 Linux、macOS 上要运行 `chmod +x` 给它可执行权限）：

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
    for line in sys.stdin:  # 标准输入读到结尾，说明核心请你退出
        message = json.loads(line)
        if message.get("id") == "h1":
            if "error" in message:
                sys.exit(1)  # 握手被拒绝，重启也没有用
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

安装并试用：运行 `miyu pkg install ./hello`，问到「现在打开？」时回答是；然后对 AI 说「跟 Ann 打个招呼」。用 Python 写的扩展可以像这样直接使用，三个平台都可以：`bin/miyu-hello` 的第一行写 `#!/usr/bin/env python3`，在 Linux、macOS 上给它可执行权限，在 Windows 上核心会根据这一行找到 `python`。如果扩展要求安装 Python 3，请写进包的说明。

## 用开发包写

上面的扩展是手写的：协议不限定语言，任何语言都可以按这种方式来写。如果你用 Python 或 Rust，可以使用现成的开发包 [miyu-agent/miyu-sdk](https://github.com/miyu-agent/miyu-sdk)（Apache-2.0）：握手、一行一条消息的收发、多个调用同时到达时按编号分别回答、取消、配置推送、标准输入读到结尾就退出、退出码 1 的规矩，这些开发包都已经处理好了。开发包不会增加额外的能力，扩展能做什么仍然由协议决定；开发包还另带一个假核心，不启动 Miyu 也能测试。

**Python**：开发包是一个文件 `miyu_ext.py`，只用标准库，适用于 Python 3.9 及以上版本，不需要 pip。把它拷贝进包的 `bin/`，和程序放在一起；清单和上面那份相同：

```text
hello/
├── package.toml
└── bin/
    ├── miyu-hello        # 你的程序，第一行是 #!/usr/bin/env python3
    └── miyu_ext.py       # 从开发包拷贝过来的文件
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

这一个文件在三个平台上都能运行，和上面手写的那份一样。配置在 `ext.config` 里，调用核心的方法用 `ext.call(method, params)`。

**Rust**：开发包是 crate `miyu-ext`，同步的，只依赖 `serde_json`。它没有发布到 crates.io，所以要按照仓库的版本标签来依赖：

```toml
[dependencies]
miyu-ext = { git = "https://github.com/miyu-agent/miyu-sdk", tag = "v0.1.0" }
```

有哪些版本可以看开发包的说明。编译出来的程序放进包的 `bin/`，在 Windows 上文件名是 `miyu-hello.exe`。

**测试**：假核心 `FakeCore` 在你的进程里扮演核心，不需要启动 Miyu：

```python
from miyu_ext import FakeCore
core = FakeCore(ext, config={"hello.greeting": "Hi"})
assert core.call_tool("hello_greet", {"name": "Ann"})["blocks"][0]["text"] == "Hi, Ann!"
core.close()
```

斜杠命令、方法、推送怎么写，Rust 的例子，以及完整的示例包，都在开发包的说明里，这里不再重复。

## 被拉起

这一节说明核心启动扩展时会给它什么：程序、参数、环境变量和目录。

- 核心启动的程序是 `bin/` 下的那一个，参数是 `[process] args`。
- 环境变量继承自核心，另外还有 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`、`MIYU_PROGRAM`（主程序 `miyu` 的真实位置：如果要启动核心，就运行 `$MIYU_PROGRAM core`，不要在自己的程序旁边找，因为装进数据根的包旁边没有主程序），以及你这个包在数据根各个部分里的目录。这些目录都已经建好，给出的都是真实位置：
  - `MIYU_APP_DIR`：你这个包的文件夹（包里带的文件要从这里找，不要自己拼路径）；
  - `MIYU_CONFIG_DIR`：给人手动修改的配置（`etc/<编号>/`；卸载包时默认保留，重新安装后仍然读取，运行 `miyu pkg remove --purge` 才会删除）；
  - `MIYU_STATE_DIR`：数据库、登录状态这类需要保留的数据（`var/lib/<编号>/`，核心启动你时这也是工作目录）；
  - `MIYU_CACHE_DIR`：删除以后可以重新生成的数据（`var/cache/<编号>/`）；
  - `MIYU_LOG_DIR`：你自己的运行日志（`var/log/<编号>/`；核心记录的标准错误也在这里，文件名是 `stderr`、`stderr.old`，不要使用这两个名字）；
  - `MIYU_TMP_DIR`：短期使用的临时文件（`tmp/<编号>/`，每次启动前清空）；
  - `MIYU_RUNTIME_DIR`：锁、套接字、地址这类只在运行期间有意义的文件（`run/<编号>/`）；
  - `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR`：给人的文件存放在这里。

  人在终端运行 `miyu <你的命令>` 时，也会设置同样的这一套变量。旧的变量名 `MIYU_PACKAGE_DIR`（等同 `MIYU_APP_DIR`）、`MIYU_PACKAGE_STATE`（等同 `MIYU_STATE_DIR`）在过渡期间仍然会设置，下一个版本去掉。
- 你的文件要按照上面的分类分开存放，卸载时这几个目录会一起删除（给人的四个目录不删除；配置目录按照 pacman 的默认做法保留，卸载前的确认界面会写 `配置留在 <路径>`，只有加了 `--purge` 才删除，界面会写 `会删掉 <路径>`）。给人修改的东西放在 `MIYU_CONFIG_DIR`，卸载以后重新安装还在；你自己使用的状态放在 `MIYU_STATE_DIR`，卸载包时会删除。不要把人的数据放进 `MIYU_STATE_DIR`：卸载前的确认界面会写 `会删掉 <路径>（<大小>）`，删除以后就没有了；要留给人的东西放在 `MIYU_DOCUMENTS_DIR`（`root/documents/`）这类人看得见的地方。一律通过这些变量来找目录，不要依赖工作目录（核心启动你时工作目录是 `MIYU_STATE_DIR`，但是人运行命令时，工作目录是人当时所在的目录），也不要自己拼 `MIYU_HOME` 下的路径。
- 这几个变量只提供给扩展本身和人运行的命令：AI 运行的命令和脚本工具拿不到这些变量（脚本工具有自己的一套目录，见 [脚本](scripts-skills.md)），AI 的文件工具也访问不到数据根里的这几个目录。
- 标准错误会写到 `~/.miyu/var/log/<编号>/stderr`（就在 `MIYU_LOG_DIR` 里），出了问题先看这个文件。

## 握手和配置

扩展发出的第一条消息是 `hello`，不需要出示凭据。核心的回应里有：

- `config`：你这个包的配置项的最终值（例如 `{"hello.greeting": "Hello"}`；密钥给出的是真实的值，没有设置的键不会出现）。**不要自己读配置文件**。
- `language`：给人看的文字应该使用哪种语言（`zh`、`en`、`ja`）。

之后如果配置变了，核心会推送通知 `extension.config {"keys": {"hello.greeting": "Hi"}}`，里面只放变化了的键，变成没有设置的键的值是 `null`。连接以后 10 秒内还没有完成握手的扩展会被断开。

**使用主人配置好的模型 key**：在清单里声明 `provider.key`，并写上 `[provider_key] catalog = "deepseek"`（见 [清单](manifest.md) 的 `[provider_key]`），主人批准以后，`config` 里会多一格 `"provider_key": "<那一家的 key>"`。如果主人没有配置那一家，或者那一家没有填写 key，就没有这一格；主人更换了 key 时，核心会推送 `extension.config {"keys": {"provider_key": "<新的>"}}`，主人删除了那一家或者去掉了 key 时推送 `null`。你拿到的是真实的 key：只放在内存里，不要写进日志或文件，不要交给其他人。

## 登记工具：`provide`

握手以后发送 `provide`，其中 `tools` 是你现在提供的**全部**工具，每发送一次就整份替换。一个工具有下面这些格：

| 格 | 说明 |
|---|---|
| `name` | 由英文字母、数字、`_`、`-` 组成，长度 1 到 64 个；不能和其他工具重名，建议用包的编号做前缀。 |
| `description`、`input_schema` | 给模型看的内容：用英文短句，一个工具最多三句。`enum` 只写在 `"type": "string"` 上：Gemini 只接受字符串的 `enum`，写在整数、数字、真假上时整个请求都会被拒绝，所以数字的选项要写成字符串，或者去掉 `enum`、写进说明。包里放在 `tools/<名字>.json` 的工具说明，`miyu pkg pack` 打包时会检查这一点，不合要求时无法打包；安装时和 `miyu check` 会报警告 `tool_not_portable`。 |
| `access` | 必须填写，表示工具会碰到什么：`read`（只查看，不出 Miyu）、`write`（修改文件）、`execute`（运行命令）、`network`（上网）、`outbound`（往 Miyu 外面发送东西）、`venue`（在聊天平台上做动作，例如撤回、禁言）。权限策略根据它决定要不要问人，所以要**如实填写**：修改你自己数据的工具（记账、删除一条记录）也要写 `write`，这样只读级别才能拦住它。 |
| `venues` | 必须填写，表示给哪种会话使用：从 `local`（本机的会话）、`private`（平台私聊）、`group`（群）里选一个或几个。 |
| `plugin` | 场所插件的名字，写法和包的编号相同：在群和私聊里，只有那个场所打开了这个插件时才提供这个工具，本机的会话仍然照常提供。不写时一直提供。 |
| `timeout_ms` | 范围 1000 到 600000，默认 60000。 |
| `display_name` | 给人看的名字，按语言分别填写：`{"zh": "打招呼", "en": "Greet"}`。 |
| `subject` | 时间线上名字后面显示哪一个参数的值，例如显示成 `打招呼 · Ann`。搜索的工具、针对某个文件或编号的工具应当填写：不填写时时间线上只有名字，看不出在搜索什么、动的是哪一个。 |
| `clicall` | `true` 表示 AI 在命令里写脚本时，也能用 `miyu call` 调用这个工具，结果交给脚本。只有只读、联网、执行类的工具才能这样调用，不写时不能。需要读取、查询，并且 AI 在命令里自己不方便做到的工具，都应该写上：这样 AI 可以在脚本里批量调用，省下一轮又一轮的来回；会修改东西、发送东西的工具不要写。有结构化结果的工具把结果写进回答的 `data`，脚本拿到的是一行 JSON。 |
| `result` | 有结构化结果时写出结果的形状，一行文字，例如 `{"items":[{"id","title"}]}`：它告诉 AI 在脚本里可以直接取哪几格（`miyu call --schema` 也会输出它）。不写时表示没有。 |
| `role` | 工具的职能：`read_file` 表示这个工具读取文本文件（参数里给出路径）。核心按照职能识别工具、不看名字：如果你替换了读文件的工具，AI 仍然看得到技能列表。不写时表示没有。 |
| `scope` | 在 `venues` 的范围之内，再收窄给哪种会话使用：`local`（本机的会话）、`local_main`（本机的主会话，子代理没有）、`local_main_attended`（本机的主会话，并且有人看着，例如需要问人的工具）、`spawn`（派生子会话的工具）。不写时是 `any`。 |
| `paths` | 说明哪几个参数是文件路径，是读还是写：`[{"parameter": "file_path", "write": true}]`。读写文件的工具都应该写上：核心根据它做权限检查（在工作区外写入要问人），你回报的文件效果也才会被接收。见下面的「改文件的工具」。 |
| `background_parameters` | 只在会话可以放到后台运行时才有意义的参数名（例如 `["run_in_background"]`）：在关闭了后台运行的会话里，AI 看到的参数格式里没有这些参数。这些参数必须是非必填的。 |
| `runs_commands` | `true` 表示这个工具会运行命令（例如替换内置 `shell` 的工具、在容器里运行命令的工具）。清单要声明能力 `tools.commands`，并且经过人批准，否则整份 `provide` 会返回 `needs_approval`。见下面的「跑命令的工具」。 |

如果有写错的地方，整份都不接收，回应的 `data` 会说明是哪一个工具、哪里不对（`{"tool": "hello_greet", "problem": "name"}`）。

## 被调：`tool.call`

AI 调用你的工具时，核心会发送 `tool.call`：

```json
{"jsonrpc":"2.0","id":"core-1","method":"tool.call","params":{"session":"…","call_id":"…","tool":"hello_greet","args":{"name":"Ann"},"cwd":"/home/me/src","owner":true}}
```

你的回应写成这样：

```json
{"jsonrpc":"2.0","id":"core-1","result":{"blocks":[{"type":"text","text":"Hello, Ann!"}]}}
```

- `args` 已经按照你的 `input_schema` 修正过。`by` 表示是谁让 AI 做的，`owner` 表示核心是否认定这个人是主人本人：如果工具在群里做动作，谁能使用要由你自己根据这两格来把关。
- `blocks` 是给模型看的内容：文字写成 `{"type": "text", "text"}`；图片要先通过 `blob.put` 上传，再放 `{"type": "image", "blob", "media_type", "width", "height"}`。包里自带的图片用 `{"path": "<MIYU_APP_DIR>/files/cat.png"}` 上传即可（数据目录里其他位置的文件不接收）。
- 画出来、生成的图片存到哪里：会话里的调用会多一格 `pictures`，它是数据根里存放图片的目录（真实位置，可能还没有建立，写入之前要自己建好），存在这里的图片，人在终端界面、网页界面里找得到。如果没有这一格（会话外面的调用、群和私聊这类场所会话、旧版的核心），仍然存到工作区：因为场所会话只能访问自己的工作区，存到其他地方 AI 就读不到、发不出去。
- 失败时同样返回结果，把 `error` 写成 `true`，并把原因写给模型看；返回 JSON-RPC 的错误也算失败。
- 如果想让 AI「发出去」一张图片（表情包这类，画在正文里，在聊天平台上作为图片发送）：在回答里加上 `"sent": [{"blob", "media_type", "width", "height", "alt"?, "sticker"?}]`，最多 4 张。`sent` 不给 AI 看。
- 如果想让头（连接核心的界面程序，例如终端界面、网页界面）把结果画成卡片：在回答里加上 `detail`，按照三种通用形状之一来写：链接列表 `{"kind": "links", "links": [{"title", "url", "snippet"?}]}`、表格 `{"kind": "table", "columns": [...], "rows": [[...]]}`、图片 `{"kind": "images", "images": [{"blob", "media_type", "caption"?}]}`。`detail` 不给 AI 看；如果形状不对，核心会丢掉这一格，并在运行日志里记录一行 `tool detail dropped`。写法和上限见 [协议参考](../reference/extensions.md#toolcalltoolcancel)。
- 如果工具写了 `clicall`，想让脚本拿到结构化的结果：在回答里加上 `data`，内容是任意 JSON（例如 `{"items": [...]}`；搜不到结果时也返回同一种形状，列表为空，这样脚本按 JSON 解析时不会出错）。AI 在命令里用 `miyu call` 调用到这个工具时，脚本拿到的是 `data` 的一行 JSON，没有 `data` 时拿到 `blocks` 里的文字。`data` 不给 AI 看，`error` 为真时也不会交给脚本。
- 同一时间可能有多个 `tool.call` 到达，要按 `id` 分别回答。
- 如果超过了 `timeout_ms`，或者人打断了，核心会发送通知 `tool.cancel {"session", "call_id"}`：此时放下那一个调用。

## 改文件的工具

如果你自己写一个编辑器、格式化、改名这类工具，想让它和内置的 `edit` 一样：时间线上显示增删的行数、可以查看差异，AI 读过的文件被你修改以后不需要重新读取，就按照下面几步来做：

1. 在工具规格里写上 `paths`，说明哪个参数是路径、会不会写入，并把 `access` 写成 `write`：

   ```json
   {"name": "my_edit", "access": "write", "venues": ["local"],
    "input_schema": {"type": "object", "properties": {"file_path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, "required": ["file_path", "old", "new"]},
    "paths": [{"parameter": "file_path", "write": true}]}
   ```

   这个参数必须是 `input_schema.properties` 顶层的一个参数，类型是字符串或字符串数组（数组的每一项各算一条路径）。核心根据它做权限检查，和内置的 `edit` 一样：打开只读以后，写入一律拒绝；读取在两种级别下都放行；数据根不另外保护，和其他目录一样按权限级别判断；关在自己工作区里的场所会话，真实位置在工作区和加进来的目录以外的，读写一律拒绝（数据根也在其中）。被拒绝的调用你收不到 `tool.call`。没有写 `paths` 的工具，核心不知道你要访问哪个文件，你报告的文件效果一条都不接收。
2. `tool.call` 会多一格 `checked`：它是检查判定过的真实位置，顺序和参数里路径的先后相同。写入时写这几个位置：你自己把参数里的路径换成真实位置（相对路径按照 `cwd` 解析，`~` 解析为家目录，并跟随链接），不在 `checked` 里的位置不要碰：因为检查以后路径上的链接可能被换过，写下去的就不是人同意的那个位置了。
3. 在回答里加上 `effects`，只说明「哪个文件怎么了」，文件内容由核心自己读取：

   ```json
   {"blocks": [{"type": "text", "text": "Edited a.txt."}], "effects": [{"kind": "file.changed", "path": "a.txt"}]}
   ```

   - `file.changed {"path"}`：文件被修改或新建了。修改前的内容是核心在发送 `tool.call` 之前读取的那份，修改后的内容是你回答以后核心重新读取的；前后相同的不接收。如果想多一层保险，可以带上 `before`、`after`（以 `sha256:` 开头的哈希）或者 `before_text`、`after_text`，对不上的不接收。
   - `file.read {"path", "lines"?}`：读取了文件（`lines` 是 `[从, 到]`，从 1 开始数，从不大于到）。AI 修改文件之前会根据它核对有没有读过这个文件。
   - `file.trashed {"path", "trash"}`：文件移进了回收站，`trash` 是它在回收站里的位置，只给人看。
   - `todo.written {"todos", "done"?}`：换上一整份待办，写法和内置的 `todowrite` 相同（供替换它的工具使用）。
   - 路径要是 `checked` 里的：修改、删除的路径要是按写入判定过的。其他种类的效果、没有判定过的路径、写法不对的那一条会被丢掉，结果仍然正常处理；运行日志会记录一行 `tool effect dropped`，排查时看其中的 `why`。
   - 如果改到一半出了错，仍然要返回 `error: true`，并报告已经修改的文件：因为已经改了的部分是真实发生的。
4. 撤销不会把文件改回去（内置的工具也不会），只会退回 AI 看过的内容：撤销一轮以后，如果 AI 要再改那个文件，就要先重新读取。

## 跑命令的工具

如果你自己写一个运行命令的工具（换掉内置的 `shell`，或者在容器、远程机器上运行命令），想让 AI 在它运行的命令里也能用 `miyu call` 调用工具，和内置的 `shell` 一样，就按照下面几步来做：

1. 在清单的 `[process] capabilities` 里加上 `tools.commands`（安装时人会看到「工具会运行命令」这一项），在工具规格里写 `"runs_commands": true`，`access` 如实填写（一般是 `execute`）。AI 看到的工具说明后面会自动接上 `miyu call` 用法的那一段，你不需要自己写。
2. 如果会话里有 AI 能在脚本里调用的工具，`tool.call` 会多一格 `channel`：`{"sock", "token", "bin"}`。为这次调用启动命令时，把它们设置成环境变量：

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

3. 命令必须由你这个扩展进程启动（子进程、孙进程都可以）：核心会沿着父进程一层层往上找到你的进程，光有令牌不够。用 `setsid`、两次 `fork` 脱离出去的命令，或者交给其他常驻进程去运行的命令，都无法调用（退出码 2）。这一条是为了防止误用（令牌泄露给了其他进程、命令脱离出去以后还在调用），不是为了防你的扩展：你启动一个子进程就能调用。真正的关口是上面的第 1 步，`tools.commands` 需要人批准。
4. 你回应这次调用以后，通道就会关闭：命令里的 `miyu call` 必须在你回应之前运行完。放到后台、在你回应以后还在运行的命令无法调用（退出码 3）。
5. 命令里调用的工具按照这个会话的权限和预设处理，和 AI 直接调用一样，并且会记录在这次调用下面。人在会话里输入的 `!命令` 不会交给你，只交给内置的 `shell`。

## 写一个模型驱动

有些模型服务需要借订阅登录、走 WebSocket 或者自己签名，Miyu 内置的三种驱动接不上它们，这时可以由你的扩展来接：Miyu 交给你统一格式的请求，你把它编码成线上的格式、登录、发送出去、解析响应，再按增量返回；重试、冷却、换池、计价都由 Miyu 负责。

在清单里声明能力 `model.driver`，并写一段 `[[driver]]`：

```toml
[process]
capabilities = ["model.driver", "network"]

[[driver]]
name = "relay"          # 驱动名
cache = "contract"      # 前缀不变就命中缓存：需要实现 driver.encode
key = "required"        # 需要 Miyu 交给你这一家的 key
images = false          # 能看图片的驱动写 true
hosted = ["search_web"] # 会让供应商自己运行的工具（见下面「供应商自己跑的工具」），没有就不写
```

主人在供应商的配置里写上 `driver = "<你的包编号>/relay"` 才会使用它（地址可以不写）。用到这个驱动的模型时，Miyu 会发送 `driver.call`：

```json
{"jsonrpc":"2.0","id":"core-7","method":"driver.call","params":{"call":"d3","driver":"pi-relay/relay","request":{"tools":[],"system":"…","messages":[{"role":"user","blocks":[{"type":"text","text":"你好"}]}],"stable":0},"fingerprint":{"tools":"…","system":"…","messages":["…"]},"params":{"model":"relay-pro","max_output":4096},"key":"sk-…"}}
```

模型的回复每到一部分，你就发送一条 `driver.delta`（通知，不带 `id`），最后返回回答：

```json
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"start":{"index":0,"kind":"text"}},{"text":{"index":0,"text":"你好！"}}]}}
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"end":{"index":0}}]}}
{"jsonrpc":"2.0","id":"core-7","result":{"usage":{"uncached":12,"cache_read":0,"cache_write":0,"output":3},"sent":"sha256:…"}}
```

- **编码只看参数**：不放时间、随机数，字段的先后顺序固定，`call` 不编码进去。同样的请求要编码出同样的字节，前缀缓存才会命中；`sent` 报告真正发出去的字节的哈希，如果同一个请求两次报告的不一样，Miyu 会记录一行警告。
- **出错按类别报告**：`{"error": {"class", "message", "retry_after_ms"?}}`，`class` 是 `retryable`、`rate_limited`、`context_too_long`、`auth`、`quota`、`content_policy`、`unclassified` 之一。已经发送过增量的调用不要自己重试（回复会重复），Miyu 会按分类重试、换池；还没有发送增量时连接不上、需要刷新令牌，由你自己处理。
- **私有数据**（思考的签名、供应商自己的调用编号）按照 `{"private": {"index", "data"}}` 的格式发送，以后的请求里会原样交还给你，其他驱动看不到。请求里的 `hosted_tool` 块（模型在供应商那边自己运行了的工具）也按这个方式区分：带着你的私有数据的块原样交给你，其他的换成一块 `text`，内容是 Miyu 写给模型看的那一段文字。
- **时限**：从发送出去到第一条增量、两条增量之间、从最后一条增量到回答，都不能超过空闲时限（从 180 秒起，按思考强度放大；如果第一个字常常要等很久，就在 `[[driver]]` 里写 `quiet_secs`，最多 900 秒）；超时或者人打断时，Miyu 会发送 `driver.cancel {"call"}`，那一次调用不要再发送任何东西。
- **key**：你只会拿到这一家配置里的那一把 key；写了 `key = "none"` 的驱动拿不到 key。你自己登录拿到的令牌要存在你自己的数据目录里。

**检查编码是否稳定**：实现 `driver.encode`（参数和 `driver.call` 相同，返回 `{"body": "<你会发的字节>"}`，不真正发送），然后运行 `miyu dev driver-check <你的包编号>`：每份样本编码两次，重启你的程序后再编码一次，在末尾加一条消息再编码一次，只要有一处字节对不上就判为不通过。声明了 `contract` 的驱动必须通过这项检查。

**列出模型**（可选）：实现 `driver.models`（参数是 `{"driver", "key"?, "base_url"?}`），返回 `{"models": [{"id", "window"?, "max_output"?, "tools"?, "vision"?}]}`，主人的模型列表里就会出现这些模型，窗口、最大输出、能不能调用工具、能不能看图片按照你报告的计算（主人手写的值优先于你报告的值）；如果不实现，主人就要手写模型名。

每个参数、增量、出错的细节见 [协议参考](../reference/extensions.md#drivercalldriverdeltadrivercancel)。

### 供应商自己跑的工具

有的供应商、agent CLI 会在它们那边自己运行工具（搜索、读网页），运行完接着写回复，Miyu 不需要执行，也拦不住。你要如实报告上来，这样人在时间线上能看到模型搜索过什么，换了模型也能接得上：

```json
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"start":{"index":0,"kind":{"hosted_tool":{"name":"search_web"}}}},{"text":{"index":0,"text":"{\"query\":\"rust\"}"}}]}}
{"jsonrpc":"2.0","method":"driver.delta","params":{"call":"d3","deltas":[{"private":{"index":0,"data":{"step":7}}},{"outcome":{"index":0,"text":"1. Rust https://www.rust-lang.org"}},{"end":{"index":0}}]}}
```

- **怎么报告**：一次调用对应一块，`start` 的 `kind` 写成 `{"hosted_tool": {"name"}}`，名字使用供应商自己的原样；这一块的 `text` 是参数原文；结果用 `{"outcome": {"index", "text", "failed"?}}`，每一块最多一次，并且在 `end` 之前，运行失败的写 `"failed": true`。结果是模型和人都看得懂的文字（标题、网址、摘录），Miyu 会截断到 64 KiB，你先截断更好；被打断、没有结果的不发送 `outcome`。
- **想原样收回**：发送时带上一份 `private`（内容由你决定，例如供应商的步骤编号），以后的请求里这一块会原样交还给你；没有带的，Miyu 会换成一段 `<hosted-tool name="…">` 的文字返回给你，和交给其他驱动的那一份相同。
- **会修改东西的原生工具不许报告成供应商侧工具**：运行命令、写文件这类会动到主人机器、动到外部世界的工具，在供应商那边运行时，Miyu 的权限、确认、撤销都管不到。要在 CLI、供应商那边关掉它们，改用 Miyu 的工具（按普通的工具调用发送 `tool_call`，由 Miyu 来执行）。只有只读、不碰本机的工具（搜索、读网页）才报告成这一种。
- **清单的 `hosted` 要写全**：会发送的名字都要写进 `[[driver]]` 的 `hosted`，主人在安装包时根据它知道供应商会自己运行什么。如果发送了没有写进去的名字，Miyu 仍然接收、仍然显示，并在运行日志里记录一行警告。

## 退出、崩了

- 标准输入读到结尾就表示「请退出」，三个平台都一样；5 秒内不退出的扩展会被强行结束。
- 核心崩溃或者被强行结束时，你和你启动的子进程会跟着一起结束：在 Unix 上整个进程组一起结束（用 `setsid` 脱离出去、自己换了进程组的进程除外），在 Windows 上同一个作业对象里的进程一起结束。需要收尾的事情不要指望在这个时候完成。
- 你自己退出（退出码 0 也算）、握手失败、标准输出关闭，都算一次失败，核心会间隔 1、2、4、8、16 秒重新启动你，连续失败 5 次后停止。
- **退出码 1 留给「配置错误、端口被占用」这类重启也没有用的情况**：核心会直接停止、不再重试，等人改好以后再打开。
- 连续运行满 60 秒以后，下次失败时重新从头计数。

## 更多本事

下面每一种功能都要在清单里声明对应的能力（见 [清单](manifest.md)）。

**自己的斜杠命令**（`commands`）：在清单里写 `[[slash]]`。人输入这个命令以后，核心会发送 `command.call {"command", "argument", "text", "session", "by", "owner", "language"}`，你回答 `{"said": "<回执，可以多行>"}`，这段话会原样显示给人看。你的程序启动以后要 `provide` 一次（没有工具时发送空的列表），核心才能识别你的连接。

**当搜索服务供应商**（`search`）：在清单里写 `[[search]]`。人在设置里选择了你以后，AI 搜索网页时核心会发送 `search.query {"source", "query", "want", "allowed_domains", "blocked_domains"}`，你回答 `{"hits": [{"title", "url", "snippet"?, "date"?}]}`；如果被限额了，就返回错误并把 `data.reason` 写成 `limited`，核心会让你休息一段时间。同样要先 `provide` 一次。

**回合开始往 AI 的上下文里加几句**（`context.inject`）：每一轮开始时，核心会发送 `user.append {"session", "venue", "text"?}`，你要在 300 毫秒内回答 `{"content": [{"kind", "text"}]}`（最多 3 块）。没有要加的内容就回答 `{"content": []}`。核心只询问在这个会话的工具集里有你的工具的扩展。这些文字是给模型看的：用英文短句，外面套一层 XML 标签。

**工具调用前拦一道**（`tool.guard`）：在清单里写 `[guard]`，列出要询问的工具名、访问类别（见 [清单](manifest.md)）。AI 的调用通过了权限策略、执行之前，核心会发送 `tool.guard {"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`，你回答 `{"verdict": "allow"}`、`{"verdict": "deny", "reason": "<为什么>"}` 或者 `{"verdict": "ask", "reason": "<为什么>"}`（要人确认）。拒绝时 AI 会看到 `Blocked by <你的包编号>: <理由>`，理由截断到 300 个字，并转义成一行：理由写一句英文，说清楚为什么拒绝、应该怎么改（守卫不修改参数，需要改参数的让 AI 重新调用）。放行不会进入上下文。这一项不需要 `provide`。

- 要回答得快：人一直在等这一步，p95 最好在 20 毫秒以内；到了 `timeout_ms`（不写时是 1 秒）还没有回答的算作拒绝。
- 你没有在运行（正在启动、已经崩溃）的时候，所有交给你把关的调用一律拒绝：守卫失效时往严格的方向处理。不要在握手以后才慢慢加载规则。
- 人打断了或者你超时了，核心会发送 `guard.cancel {"session", "call_id"}`，那一次你不需要再回答。
- 如果有多个守卫，每个都会被询问，只要有一个拒绝就拒绝；你只能收紧权限策略，不能放宽它。

**工具调用以后记一笔**（`events.read`）：在清单里写 `[after]`，列出要接收的工具名、访问类别（见 [清单](manifest.md)）。调用的结果记进会话以后，核心会发送通知 `tool.after`（不带 `id`，不要回复）。这是只读的：你改不了结果，它也不会进入 AI 的上下文。直接调用和在脚本里调用（`miyu call`）的形状不一样，根据有没有 `blocks` 来区分：

```python
def on_tool_after(params):
    if "blocks" in params:
        # 直接调用：call_id 形如 call_12_1，blocks 是 AI 看到的内容
        log(params["session"], params["call_id"], params["tool"], params["status"], params["blocks"])
    else:
        # 在脚本里调用：call_id 形如 call_12_1#3，结果只给出 blob 的引用（result、bytes），需要内容就自己调用 blob.get
        log(params["session"], params["call_id"], params["tool"], params["status"], params.get("result"))
    # 两种情况下 args 都是一段 JSON 字符串（不是对象）：要看参数就自己 json.loads(params["args"])
```

- 核心不会等你：每个扩展有一条最多 256 条的队列，如果你读得慢、队列满了，新来的会被丢掉；你崩溃、重启期间的通知不会补发。**`tool.after` 不保证送达**，如果需要完整的审计，就订阅事件流（`subscribe` 的 `events`）。
- 你只能收到你能访问的会话：不是平台桥的扩展只收到本机的会话，以及主人本人没有被关起来的私聊；平台桥只收到自己平台的场所会话。

**往系统提示词末尾放一段**（`context.system`）：调用 `system.append {"content", "session"?}`。不写 `session` 时作用于全部会话；`content` 写 `null` 时去掉这一段。修改以后下一轮会把整段重新计算一次，所以不要频繁修改；如果只想放固定的几句，启动时调用一次就可以了。

**按意思搜索**：调用 `model.embed {"texts": [...]}` 获取向量（使用人在设置里选择的语义模型）。如果没有配置语义模型，会返回 `embedding_off`，此时退回关键词搜索就好。这一项不需要另外批准能力。

**调用一次模型**：调用 `model.call`（例如让 AI 给一张图片写描述，`"use": "vision"`）。这会花费人的额度，不要滥用。在对话以外、会反复调用的场合（判断要不要回复、打分），在清单里用 `[[side]]` 登记，调用时写 `"side": "<编号>"`：这样会按照登记的档位，使用人放进 `lite`、`normal`、`flagship` 池的模型，人不需要为你另外配置（见 [清单](manifest.md)）。

**后台页和界面块的方法**：发送 `package.methods {"methods": [{"name": "status"}]}` 登记方法；页面、按钮调用到这些方法时，核心会发送 `method.call {"method", "params"}`，你返回什么，对方就拿到什么；无法识别的方法返回 JSON-RPC 的 `-32601`。

**界面块**（`widgets`）：在清单里写 `[[widget]]`。有人开始看你的某一块时，核心会发送 `widget.watch {"widget", "session"?}`，最后一个人不看了时发送 `widget.unwatch`；只在有人看的时候取数据，不要定时刷新。推送内容用 `widget.set {"widget", "session"?, "title"?, "content"}`：

```json
[{"type":"heading","text":"本周"},{"type":"text","text":{"zh":"晴 23°C","en":"Sunny 23°C"},"tone":"accent"},{"type":"pairs","items":[{"label":"湿度","value":"40%"}]},{"type":"progress","value":0.4,"text":"索引 40%"},{"type":"list","items":[{"title":"构建 #128","right":"3m","state":"doing","action":{"call":"build.open","params":{"id":128}}}]},{"type":"buttons","items":[{"label":"刷新","action":{"call":"refresh"}}]}]
```

界面块里只有文字：不识别 Markdown、HTML、颜色码。整页的界面块（`place = "page"`）还可以放能修改的表单（`sections`、`form`）。段和表单的每一格见 [协议参考：界面块](../reference/extensions.md#widgetsetwidgetclearwidgetwatchwidgetunwatch)。

**让其他包调用你、你调用其他包**：通过总线按照名字调用，不写对方的包名（见 [总线](../reference/bus.md)）。给其他包调用的方法写进清单的 `[export] methods`，再在 `[export]` 里声明名字：`provides = ["stt"]` 表示由你提供（一个名字只有一家提供），`plugs = ["tts.engine"]` 表示你接入进去（可以有多家，由使用的一方挑选）；方法名带上名字作为前缀（`stt.transcribe`），同样要用 `package.methods` 登记，全部登记完以后 `bus.list` 才会报告你 `ready`。其他包调用到你时，`method.call` 会多一格 `from`（调用方的包编号，无法冒充）。如果要调用其他包，就声明 `packages.call`，发送 `bus.call {"name", "method", "params"}`（`method` 写短名，例如 `transcribe`；`plugs` 的名字还要另写 `package` 来挑选哪一家）。按照包编号调用不到其他包：扩展发送 `package.call` 时写了其他包的编号，会返回 `own_package_only`；如果清单的 `methods` 里有不在你声明的名字下面的方法，清单就读不成功（`bad_export`）。公认的名字（`stt`、`tts` 这些）的形状写在参考文档里，要按照它来回答、按照它来调用；运行 `miyu bus list` 可以查看这台机器上谁提供了什么。

**读取其他包给你带的内容**：其他包在清单里写 `[content.<你的编号>] dir = "…"`，就是给你带了一个目录。调用 `content.list {}` 查看谁带了内容，用 `content.read {"package", "path", "offset"?}` 分块读取；安装、卸载、升级以后，核心会推送 `content.changed`。

**往会话里记录自己的事件**：调用 `events.append {"session", "kind": "ext.<你的前缀>.<名字>", "body"}`。这不会开启回合，也不给模型看，界面和你以后读日志时能看到。前缀是清单里的 `prefix`，没有写时是包的编号。

## 别做的

- 不要读写数据根里的其他地方（会话、其他包、配置文件、密钥文件）：需要信息时通过协议获取。
- 只给人使用的方法（替人回答确认、修改权限级别、删除会话、修改配置、安装卸载软件包、`extension.*` 这些）被扩展调用时会被拒绝（`local_only`）。
- 能力要**如实声明**：没有声明的能力，调用对应的方法会返回 `needs_approval`，`data.capabilities` 里写着缺少哪一项。你不带 `as` 说的话会记成你自己说的（名字是包的编号），不是记成本人说的。
