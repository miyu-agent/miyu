# 清单 `package.toml`

一个包的清单，每一张表、每一格怎么写。只收下面这些，不认识的表、键报错（`miyu pkg install` 和 `miyu check` 都会说第几行哪里不对）。英文版：[manifest.en.md](manifest.en.md)。

**给人看的字**（`name`、`summary`、`about`、配置项的 `name`、`description`……）写成「语言到一句话」：`{ en = "Hello", zh = "打招呼" }`，只认 `zh`、`en`、`ja`，界面照人的语言挑，没有的退到 `en`、`zh`、`ja`。

**包是什么种类**不用写，照带了哪几张表认：有 `[process]` 的是扩展；没有程序、带了 `[[mcp]]` 的是 MCP 服务包；有 `[mascot]` 的是吉祥物包，有 `[theme]` 的是主题包（这两种见文末）；只有 `[package]` 的是只带技能、脚本、命令模板的包。还有界面、内置、小程序几种是 Miyu 自己用的，这里不写。

## 一份完整的扩展清单

```toml
[package]
version = "0.1.0"                                   # 可以不写，只给人看
protocol = [1, 1]                                   # 扩展必写：说得了的协议主版本 [最低, 最高]，现在只有 1
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # 可以不写
author = "Alice <alice@example.com>"               # 作者，可以不写
upstream = "https://github.com/alice/hello"         # 源码仓库，可以不写
icon = "hand"                                       # Lucide 的图标名，可以不写
logo = "art/logo.png"                               # 包里自带的一张图标，可以不写
needs = ["images"]                                  # 可以不写：显示不了图片的地方（语音）整包当没开
targets = ["linux-x86_64", "macos-aarch64"]         # 可以不写：带了本机程序的写它能跑的几种系统

[command]                                           # 扩展必写
name = "hello"                                      # 人敲 miyu hello … 时转给你的程序
program = "miyu-hello"                              # bin/ 下的程序名，不带路径
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]                                           # 有它的就是扩展
args = ["serve"]                                    # 核心拉起时带的参数
start = "manual"                                    # manual：开关打开才拉起（默认）；always：核心起来就拉起
capabilities = ["tools", "widgets"]                 # 要的能力，见下面「能力」
system_account = false                              # 是不是聊天平台的桥（只有桥才要），默认不是

[check]
args = ["check"]                                    # miyu check 时跑 <程序> check

[settings.greeting]                                 # 配置项，键是 hello.greeting
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }

[features.greet]                                    # 预设里一个开关
name = { en = "Greeting", zh = "打招呼" }
tools = ["hello_greet"]

[page]
dir = "page"                                        # 后台页：目录里要有 index.html

[export]
methods = ["greet"]                                 # 别的包能经 package.call 调的方法

[[slash]]
name = "hi"
summary = { en = "Say hi", zh = "打个招呼" }

[[widget]]
id = "status"
place = "status"
```


## 格式是冻结的

这一页列的表和格是稳定的（2026-10-23 冻结）：以后只加不改不删，已经写好的清单换了新版本的 Miyu 照样读。新加的格、表会写明从哪一版起有；用到了的，在 `[package] miyu` 写上要的最低版本，旧版本的 Miyu 读到会说清要更新。不认识的表、格一律报错，所以别写这一页以外的。
## `[package]`

| 格 | 说明 |
|---|---|
| `name` | 必写，给人看的名字 |
| `protocol` | 带程序的包必写：`[最低, 最高]`，现在只有 `1` |
| `version` | 版本，只给人看 |
| `summary` | 一句说明 |
| `author` | 作者：人名、组织名，可以带邮箱，一行，至多 100 个字。软件包信息页上露出来 |
| `upstream` | 上游：这个包的源码仓库，`https://` 开头。软件包信息页上露出来 |
| `members` | `true`：默认成员也能用。带进程的包（扩展、带 MCP 服务的）默认只有管理员组的人能用；管理员在系统那一层改了的照他的。成员自己装的包不认这一格。默认 `false` |
| `icon` | Lucide 的图标名。界面不一定认得 Lucide 的全部名字：终端界面只认它对照表里的那些，认不出的画通用的工具图标（扳手） |
| `logo` | 包里自带的一张图标：包目录里的相对路径，`.png` 或 `.webp`。不超过 256 KiB、每边不超过 512；`miyu pkg pack` 打包时超了的照原格式缩到以内（目录里的原图不动）。不在、不是图、超了的当没有图标，包照装，`miyu check` 报警告。界面有它先画它，没有画 `icon` |
| `needs` | 要这个地方有什么才有用：现在只认 `images`（显示不了图片的会话里整包当没开） |
| `miyu` | 最低要哪一版 Miyu：三段数字，例如 `"0.2.0"`。用了新加的格、新的方法时写上；旧版本的 Miyu 读到会说清要更新，不会报看不懂的键 |
| `groups` | 只给出厂的包：在哪几个组里（例如 `["basesystem"]`），预设写组名就开关组里的包。别的包写了报错。原来的 `required`（也只给出厂的包）2026-10-26 去掉了 |
| `targets` | 能装在哪几种系统上：`linux-x86_64`、`linux-aarch64`、`macos-aarch64`、`windows-x86_64`。不写是哪种都能装；写了的，别的系统上装不上，打包时用 `miyu pkg pack --target` 一种一个包文件 |

## `[command]`

扩展、界面要写。`name`：人在终端敲 `miyu <name> …` 时，核心把后面的参数原样交给你的程序，退出码照你的（`miyu help <name>` 转成 `--help`）：小写字母开头，只用小写字母、数字、`-`，最多 32 个；和内置子命令（`ask`、`config`、`pkg`、`web` 这些）撞了的内置的赢。`program`：`bin/` 下的程序名，Windows 上核心找 `<名字>.exe`。`about`：一句说明。`args`：可以不写，转交时加在人敲的参数前面。

所以你的程序除了 `serve` 这类给核心用的参数，也该认人敲的那些。

## `[process]`

有它的就是扩展，要有 `[command]`。

| 格 | 说明 |
|---|---|
| `args` | 核心拉起时带的参数 |
| `start` | `manual`：开关打开才拉起（默认）；`always`：核心起来就拉起 |
| `capabilities` | 要的能力，见下 |
| `system_account` | 带进程的包都有自己的账号，账号名就是包的编号，你上传的东西存在它名下，平时替装 Miyu 的人干活。`true`：这是接聊天平台的桥，替平台上的人干活，群会话归这个账号 |
| `scope` | `system`（默认）：起一份，用包自己的账号跑。`account`：每个用它的人一份，以那个人的身份跑，数据在他自己的家目录里，别人（包括管理员）看不到；用到时才起。不能和 `system_account = true` 一起写 |

**能力**：只认这十七个。管理员自己装的包，打开时要人批；升级以后多要的，只问多的那几个。**照实声明**：没声明的，调对应的方法回 `needs_approval`。

| 能力 | 能做什么 |
|---|---|
| `tools` | 往工具目录里登记工具 |
| `commands` | 提供斜杠命令（`[[slash]]`） |
| `search` | 当搜索服务供应商（`[[search]]`） |
| `widgets` | 在界面上显示自己的内容（`[[widget]]`） |
| `context.inject` | 回合开始时往她的上下文里加几句 |
| `context.system` | 往系统提示词末尾放一段 |
| `packages.call` | 调别的包导出的方法 |
| `act_for_external` | 代表聊天平台上的人说话：只给平台的桥 |
| `sessions.drive` | 驱动会话：开会话、说话、打断、撤销、压缩、分叉、发斜杠命令 |
| `events.read` | 读会话：订阅事件、读历史、读后台命令的输出 |
| `events.write` | 往会话里记自己的事件 |
| `memory.write` | 写记忆、知识库 |
| `fs.read`、`network` | 经核心读文件、抓链接 |
| `fs.write` | 写指定的路径：扩展进沙盒以后照它放行 |
| `tool.guard`、`tool.rewrite` | 工具执行前拦下、执行后改写结果：还没有 |

## `[settings.<名字>]`

一项配置一张表，键是 `<包的编号>.<名字>`。项多了可以分组，多写一层 `[settings.<组>.<名字>]`，键就是 `<包的编号>.<组>.<名字>`（例如 `im.qq.listen`），只能多这一层；组里的项不能叫 `type`、`name`。设置页的「软件包」那一页有你一组。**不要自己读配置文件**：值在握手时交给你，改了推给你（见 [写扩展](extensions.md)）。

| 格 | 说明 |
|---|---|
| `type` | 必写：`bool`、`int`（`min`、`max`）、`option`（`choices`，至少两个）、`text`（`max`，默认 200 个字）、`name`、`url`、`secret`（只能写成 `{ secret = … }` 那种引用）、`list`（`element` 是元素的类型） |
| `default` | 默认值，合这个类型；`secret` 不能写 |
| `layers` | `system`、`personal` 里的一个或两个，默认两个 |
| `applies` | 什么时候生效：`now`、`new_session`、`next_turn`、`program_start`（你的程序下次启动，默认） |
| `name`、`description` | 给人看的名字、说明 |
| `hidden` | `true` 的设置页不画，照样能写、能查 |

## `[features.<编号>]`

预设里一个开关管一组工具。不写的整个包算一个功能；写了几个的，你登记的每一件工具都要列在某个功能下面，不然登记不进来。

| 格 | 说明 |
|---|---|
| `name` | 必写，给人看的名字 |
| `summary` | 一句说明 |
| `tools` | 这个功能管的工具名 |

```toml
[features.greet]
name = { en = "Greeting", zh = "打招呼" }
summary = { en = "Greet people" }                  # 可以不写
tools = ["hello_greet"]
```

功能的编号全局不重，建议带包的编号做前缀。

## `[check]`

`args`：`miyu check` 时跑 `<程序> <args…>`。你在标准输出上一行一个 JSON 报问题（`kind`、`file`、`level`、`message` 必有；`line`、`column` 是正整数，`code`、`key`、`rule`、`source` 是字，可以有；别的格不收），只有警告时退出码 0，有错误 1。最多跑 30 秒。要有 `[command]`。

## `[page]`

`dir`：后台页的目录，里面要有 `index.html`，网页的「软件包」页里打开。页面经网页软件给的通道调你登记的方法（`package.methods`、`method.call`，见 [写扩展](extensions.md)）。

## `[[mcp]]`

带 MCP 服务，一段一个，细节见 [接 MCP 服务](mcp.md)。

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

## `[[slash]]`

扩展提供的斜杠命令，一段一条；`capabilities` 里要有 `commands`。

| 格 | 说明 |
|---|---|
| `name` | 命令名，写法同 `[command] name`；和 Miyu 自带的命令同名的不收 |
| `summary` | 命令菜单里那一句 |
| `argument` | 名字后面跟什么的提示，例如 `{ en = "<city>", zh = "<城市>" }` |
| `access` | 谁能用：`admin`（默认：本人、群里的主人和管理的人）、`anyone`、`off`（先停用）。人在系统配置里写的压过它 |

人打了这条命令，核心调你的程序 `command.call`，见 [写扩展](extensions.md)。只想展开成一句话发给 AI 的，用命令模板，不用写程序（见 [脚本、技能、命令模板](scripts-skills.md)）。

## `[[search]]`

把你的程序登记成一个搜索服务供应商，一段一个；`capabilities` 里要有 `search`。

| 格 | 说明 |
|---|---|
| `id` | 编号，写法同 `[command] name`，一个包里不重 |
| `name` | 给人看的名字 |

设置页「搜索服务供应商」的下拉里多一项，值是 `<包的编号>/<编号>`。人选了它才用，不进「自动」的顺序：你的程序不会在人不知道的时候拿到搜索词。AI 搜网页时核心调你的程序 `search.query`，你用不了的（出错、限额），照出厂的几家兜底。见 [写扩展](extensions.md)。

## `[[side]]`

登记你的程序在对话以外调模型的请求（旁路请求），一段一条，声明它要哪一档的模型。不用另声明能力。

| 格 | 说明 |
|---|---|
| `id` | 编号，写法同 `[command] name`，一个包里不重 |
| `tier` | 要哪一档：`lite`（便宜、快，量大的判断）、`normal`（要读懂一段对话的）、`flagship`（最好的） |
| `name` | 给人看的名字 |
| `summary` | 一句说明：什么时候调、做什么 |

```toml
[[side]]
id = "judge"
tier = "lite"
name = { en = "Reply judge", zh = "判断要不要回" }
summary = { en = "Decides whether to reply in a group.", zh = "群里有人说话时判断要不要回" }
```

调 `model.call` 时写 `"side": "judge"`（只写你自己包里的编号，不带包名），不写 `model`、`use`：核心照档位从同名的模型池（`lite`、`normal`、`flagship`，人在设置里往里放模型）取；池是空的、或者池里的模型都在冷却、都出了错，用会话的模型——你同时写了 `session` 的照那个会话，没写的照 `models.chat`。人不用为你的请求另配模型。写了你没登记的编号，回 `bad_params`。`model.list` 的 `side` 里列得出你登记的（编号是 `<包的编号>/<编号>`）和这一刻会落到哪。见 [写扩展](extensions.md)。

## `[[widget]]`

往终端、网页上放自己的一块内容；`capabilities` 里要有 `widgets`。一个包最多 16 块。

| 格 | 说明 |
|---|---|
| `id` | 包里唯一：小写字母开头，小写字母、数字、`-`，最多 32 个 |
| `place` | `home` 首页、`sidebar` 侧边栏、`status` 底栏的一小段、`page` 整页 |
| `title` | 侧边栏、整页必写 |
| `scope` | `global`（默认）；`session` 跟着会话、一个会话一份，只给侧边栏 |
| `command` | 只给整页：打了这个斜杠命令就打开这一页 |

内容由你的程序推，见 [写扩展](extensions.md)。

## `[export]`

`methods`：别的包能经 `package.call` 调到你的哪几个方法（照样要经 `package.methods` 登记）。

## `[content.<包的编号>]`

给另一个包带一个目录的内容（表情包的内容包就是这样）：`dir` 是你包文件夹里的相对目录。被带的那个包的扩展经 `content.list`、`content.read` 读。可以写几张。

## `[connection]`

`platform`：接的聊天平台，写法同包的编号（例如 `qq`）；接几个的写列表（`["qq", "tg"]`）。只有扩展能写；接聊天平台的桥写它。

接几个平台的，工具名照平台起头：`qq_send` 只在 `qq:` 的群、私聊里给，`tg_send` 只在 `tg:` 的里给；本机的会话都给。不带平台前缀的工具哪里都给。

## `[depends]`、`[recommends]`

`packages`：依赖的、推荐的包的编号（以前的写法 `workers` 照认，读进同一份）。依赖的缺了装不上、装着的不起；推荐的缺了照装、照起，少一部分本事。

## `[mascot]`

终端首页的吉祥物，做成一个包：

```toml
[package]
name = { zh = "布丁", en = "Pudding" }
icon = "cat"            # 可以不写

[mascot]
model = "mascot.json"   # 包文件夹里的相对路径
```

- `model` 必写：包文件夹里的相对路径，不能有 `..`、开头的 `/`。别的格不收。
- 只带吉祥物的包只写 `[package]`、`[mascot]`，另可写 `[depends]`、`[recommends]`；不说协议，写了 `protocol` 或别的表都报 `wrong_kind`。扩展包也能多带一张 `[mascot]`。
- `miyu check` 查模型文件：不在、超过 256 KiB、不是 JSON 的对象都报错。文件里面怎么写由终端定、终端查，见终端仓库的 [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md)（`mascot.json`）。
- 没有开关、配置项、程序；终端照它的配置项 `tui.mascot` 挑用哪一个。

## `[theme]`

终端的主题（框线、记号、字的修饰），做成一个包：

```toml
[package]
name = { zh = "纯字符", en = "ASCII" }

[theme]
file = "theme.json"     # 包文件夹里的相对路径
```

- `file` 必写，写法同 `[mascot] model`。
- 主题包只写 `[package]`、`[theme]`，另可写 `[depends]`、`[recommends]`；写了 `protocol`、`[command]`、`[mascot]` 或别的表都报 `wrong_kind`：主题包不带吉祥物，别的包也带不了主题。
- `miyu check` 查主题文件，规矩同吉祥物。文件里面怎么写见终端仓库的 [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md)（「主题」）。
- 没有开关、配置项、程序；终端照它的配置项 `tui.look` 挑用哪一个。
