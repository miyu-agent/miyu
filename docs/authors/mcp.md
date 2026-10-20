# 接 MCP 服务

现成的 MCP 服务（Model Context Protocol）能直接接进 Miyu：核心拉起服务的进程，经标准输入输出说 MCP，把服务的工具登记给 AI 用。两种接法：写进个人设置（只给自己用，最快），或者做成一个包（发给别人）。英文版：[mcp.en.md](mcp.en.md)。

## 最快：写进个人设置

打开 `~/.miyu/home/admin/settings.toml`（或者 `miyu config edit`），一个服务一张表：

```toml
[mcp.files]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/home/me/notes"]
```

存了就生效，不用重启。终端、网页的「软件包」页里看得到它在跑没有；`miyu check` 查写没写错。

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
| `command` | 命令和参数。第一项先在包的 `bin/` 里找，再在 `miyu` 旁边找，最后找 `PATH`（`npx`、`uvx`、`docker` 这类） |
| `env` | 给服务的环境变量。密钥写成 `{ secret = "<名字>" }`（用 `miyu login` 或设置页存进去），别把值写在清单里 |
| `venues` | `local`、`private`、`group` 里的一个或几个：本机的会话、聊天平台的私聊、群 |
| `timeout_ms` | 一次调用最多等多久 |
| `need_load` | 不写是先加载：AI 平时只看到工具的名字和一行说明，要用时先加载完整的参数，工具多的服务省钱。`false`：一直给完整的参数，工具少、天天用的服务写上 |
| `clicall` | AI 在命令里写脚本时能不能用 `miyu call` 调这个服务的工具，不写是能。服务会写东西、发东西的，写 `false` |

## AI 看到的样子

- 工具名是 `mcp__<服务名>__<工具名>`，说明、参数照服务给的原样（说明超过 2048 个字的截掉）。
- 预设里一个服务算一个功能（`mcp-<服务名>`），能整个关，也能一件件关。
- **问不问人**：工具的注解里写了 `readOnlyHint: true` 的哪一级都不问；别的照联网的一类，「工作区」那一级要人确认。你写服务的话，只读的工具请标上它。

## 跑起来的样子

- 工作目录是 `~/.miyu/state/mcp/<服务名>/`，不进沙盒；标准错误在 `~/.miyu/state/logs/mcp-<服务名>.stderr`，起不来先看它。
- 起不来、崩了的退避着重试，连续 5 次停下；「软件包」页上看得到原因和标准错误的最后几行，点重启再试。
- 新旧两版 MCP 都认，服务说哪一版都行。只接工具：资源、提示词、`sampling` 这些不接。
- Miyu 不替人装 Node、Python：用 `npx`、`uvx` 起的服务，说明里写清楚要先装什么。

## 个人设置和包里都有同名的

个人设置里的赢，包里的那个被盖住（「软件包」页上看得到）。
