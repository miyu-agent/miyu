# 接 MCP 服务

现成的 MCP 服务（Model Context Protocol）能直接接进 Miyu：核心拉起服务的进程，经标准输入输出说 MCP；只给一个网址的服务（Streamable HTTP），核心直接连那个网址。服务的工具登记给 AI 用。两种接法：写进个人设置（只给自己用，最快），或者做成一个包（发给别人）。英文版：[mcp.en.md](mcp.en.md)。

## 最快：写进个人设置

打开 `~/.miyu/config.toml`（或者 `miyu config edit`），一个服务一张表：

```toml
[mcp.files]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/home/me/notes"]
```

只给网址的服务（GitHub 这类）写 `url`，令牌写成 `bearer`：

```toml
[mcp.github]
url = "https://api.githubcopilot.com/mcp/"
bearer = { secret = "github" }       # 用 miyu provider login 或设置页存进去；发成 Authorization: Bearer …
```

存了就生效，不用重启。终端、网页的「软件包」页里看得到它在跑没有；`miyu check` 查写没写错。

只认浏览器授权、发不了个人令牌的服务（Notion、Linear 这类）：只写 `url`，不写 `bearer`。它会停在「要登录」（`needs_login`），打 `miyu mcp login <服务>`，在浏览器里点同意就连上了；令牌存进密钥文件，你不用经手，快过期时核心自己续。浏览器在别的机器上的，授权完把地址栏里的整段地址贴回命令行。不用了打 `miyu mcp logout <服务>`。

## 做成一个包

```text
notes-mcp/
└── package.toml
```

```toml
[package]
name = { en = "Notes", zh = "笔记" }

[[mcp]]
name = "notes"                                    # 服务名：小写字母、数字、-，1 到 32 个
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/srv/notes"]
env = { TOKEN = { secret = "notes" } }            # 可以不写；值能写字，也能引用密钥 { secret = … }、环境变量 { env = … }
venues = ["local"]                                # 可以不写：给哪种会话，不写是只给本机的
timeout_ms = 600000                               # 可以不写：一次调用最多等多久，1000 到 3600000
need_load = false                                 # 可以不写：不写是先加载，写 false 一直给完整的参数
clicall = false                                   # 可以不写：AI 在命令里用 miyu call 不能调这个服务的工具
```

只带服务、不带程序的包不用写 `protocol`、`[command]`、`[process]`。一个包能带几个服务；扩展包也能带，服务跟着扩展起停。

装：`miyu pkg install ./notes-mcp`。只带服务的包装好是**关着的**，在「软件包」页里打开。

## 每一格

| 格 | 说明 |
|---|---|
| `name` | 服务名（个人设置里是表名），小写字母、数字、`-`，1 到 32 个；全局不重 |
| `command` | 命令和参数。第一项先在包的 `bin/` 里找，再在 `miyu` 旁边找，最后找 `PATH`（`npx`、`uvx`、`docker` 这类）。和 `url` 写一个 |
| `url` | 服务的网址：`https://` 开头；`http://` 只给本机的服务（`localhost`、`127.0.0.1`），别的机器的不连，免得令牌明文发出去。和 `command` 写一个 |
| `bearer` | 跟着 `url`：令牌，发成 `Authorization: Bearer <值>`；写法同 `env` 的值，令牌请写成 `{ secret = … }` |
| `headers` | 跟着 `url`：别的请求头，`{ X-Api-Key = { secret = "key" } }` 这样，写法同 `env` |
| `env` | 给服务的环境变量。密钥写成 `{ secret = "<名字>" }`（用 `miyu provider login` 或设置页存进去），别把值写在清单里 |
| `venues` | `local`、`private`、`group` 里的一个或几个：本机的会话、聊天平台的私聊、群 |
| `timeout_ms` | 一次调用最多等多久 |
| `need_load` | 不写是先加载：AI 平时只看到工具的名字和一行说明，要用时先加载完整的参数，工具多的服务省钱。`false`：一直给完整的参数，工具少、天天用的服务写上 |
| `clicall` | AI 在命令里写脚本时能不能用 `miyu call` 调这个服务的工具，不写是能。服务会写东西、发东西的，写 `false` |

## AI 看到的样子

- 工具名是 `mcp__<服务名>__<工具名>`，说明、参数照服务给的原样（说明超过 2048 个字的截掉）。
- 预设里一个服务算一个功能（`mcp-<服务名>`），能整个关，也能一件件关。
- **问不问人**：工具的注解里写了 `readOnlyHint: true` 的哪一级都不问；别的照联网的一类，「工作区」那一级要人确认。你写服务的话，只读的工具请标上它。

## 服务的说明、提示词和资源

- **说明**（`instructions`）：服务交的那一段放进 AI 的 system 的最后，一个服务一段，带服务名，最多 4000 个字符。只有工具在这个会话里的服务才带。说明写清它的工具什么时候用：工具多半先只给名字和一句说明，AI 照你的说明决定什么时候去拿完整的工具。会话半路才换的说明，等工具有变化的那一轮才换上（免得每轮都打断缓存）。
- **提示词**（`prompts`）：变成斜杠命令 `/<服务名>:<提示词名>`，在命令菜单里看得到。人跟的字照空白切开（带空格的值用英文双引号括起来，例如 `/weather:forecast "San Francisco" CA`），照你声明的参数先后对上，最后一个参数拿剩下的整段；取回来的文字、图片当这个人说的一句发给 AI。名字里不合 `[A-Za-z0-9_-]` 的字换成 `_`，换完撞名的不登记，`miyu check` 会说。只在本机的会话里用得上。
- **资源**（`resources`）：服务开放出来的资料（文件、文档、表结构……），一份一个地址，也可以给地址模板（`file:///logs/{day}.txt`）。声明了 `resources` 的服务，AI 多两件工具：`mcp__<服务名>__list_resources` 一页一页列出资源和模板，`mcp__<服务名>__read_resource` 照地址读一份。读回来的文字原样给 AI，图照工具交的图收，别的二进制只说一句类型和大小。跟着这个服务的开关、场所、先加载走；你的服务自己有同名工具的，用你的。只开放资源、没有工具的服务也接得上。

## 跑起来的样子

- 工作目录是 `~/.miyu/state/mcp/<服务名>/`，不进沙盒；标准错误在 `~/.miyu/state/logs/mcp-<服务名>.stderr`，起不来先看它。
- 起不来、崩了的退避着重试，连续 5 次停下；「软件包」页上看得到原因和标准错误的最后几行，点重启再试。
- 新旧两版 MCP 都认，服务说哪一版都行。接工具、服务的说明、提示词；资源、`sampling` 这些不接。
- Miyu 不替人装 Node、Python：用 `npx`、`uvx` 起的服务，说明里写清楚要先装什么。

## 个人设置和包里都有同名的

个人设置里的赢，包里的那个被盖住（「软件包」页上看得到）。
