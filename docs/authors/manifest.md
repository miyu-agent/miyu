# 清单 `package.toml`

一个包的清单，每一张表、每一格怎么写。只收下面这些，不认识的表、键报错（`miyu pkg install` 和 `miyu check` 都会说第几行哪里不对）。英文版：[manifest.en.md](manifest.en.md)。

**给人看的字**（`name`、`summary`、`about`、配置项的 `name`、`description`……）写成「语言到一句话」：`{ en = "Hello", zh = "打招呼" }`，只认 `zh`、`en`、`ja`，界面照人的语言挑，没有的退到 `en`、`zh`、`ja`。

**包是什么种类**不用写，照带了哪几张表认：有 `[process]` 的是扩展；没有程序、带了 `[[mcp]]` 的是 MCP 服务包；只有 `[package]` 的是只带技能、脚本、命令模板的包。

## 一份完整的扩展清单

```toml
[package]
version = "0.1.0"                                   # 可以不写，只给人看
protocol = [1, 1]                                   # 扩展必写：说得了的协议主版本 [最低, 最高]，现在只有 1
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # 可以不写
icon = "hand"                                       # Lucide 的图标名，可以不写
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
system_account = false                              # 有没有自己的系统账号（接聊天平台的才要），默认没有

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

## `[package]`

| 格 | 说明 |
|---|---|
| `name` | 必写，给人看的名字 |
| `protocol` | 带程序的包必写：`[最低, 最高]`，现在只有 `1` |
| `version` | 版本，只给人看 |
| `summary` | 一句说明 |
| `icon` | Lucide 的图标名 |
| `needs` | 要这个地方有什么才有用：现在只认 `images`（显示不了图片的会话里整包当没开） |
| `targets` | 能装在哪几种系统上：`linux-x86_64`、`linux-aarch64`、`macos-aarch64`、`windows-x86_64`。不写是哪种都能装；写了的，别的系统上装不上，打包时用 `miyu pkg pack --target` 一种一个包文件 |

## `[command]`

扩展、界面要写。`name`：人在终端敲 `miyu <name> …` 时，核心把后面的参数原样交给你的程序，退出码照你的（`miyu help <name>` 转成 `--help`）：小写字母开头，只用小写字母、数字、`-`，最多 32 个；和内置子命令（`ask`、`config`、`pkg`、`web` 这些）撞了的内置的赢。`program`：`bin/` 下的程序名，Windows 上核心找 `<名字>.exe`。`about`：一句说明。

所以你的程序除了 `serve` 这类给核心用的参数，也该认人敲的那些。

## `[process]`

有它的就是扩展，要有 `[command]`。

| 格 | 说明 |
|---|---|
| `args` | 核心拉起时带的参数 |
| `start` | `manual`：开关打开才拉起（默认）；`always`：核心起来就拉起 |
| `capabilities` | 要的能力，见下 |
| `system_account` | `true`：有自己的系统账号，账号名就是包的编号。接聊天平台的桥才要：群会话归这个账号 |

**能力**：只认这十五个。管理员自己装的包，打开时要人批；升级以后多要的，只问多的那几个。**照实声明**。

| 能力 | 能做什么 |
|---|---|
| `tools` | 往工具目录里登记工具 |
| `commands` | 提供斜杠命令（`[[slash]]`） |
| `widgets` | 在界面上显示自己的内容（`[[widget]]`） |
| `context.inject` | 回合开始时往她的上下文里加几句 |
| `context.system` | 往系统提示词末尾放一段 |
| `packages.call` | 调别的包导出的方法 |
| `act_for_external` | 代表聊天平台上的人说话：只给平台的桥 |
| `tool.guard`、`tool.rewrite`、`events.read`、`events.write`、`sessions.drive`、`network`、`fs.read`、`fs.write` | 先声明、先批，具体放开随以后的版本 |

## `[settings.<名字>]`

一项配置一张表，键是 `<包的编号>.<名字>`。设置页的「软件包」那一页有你一组。**不要自己读配置文件**：值在握手时交给你，改了推给你（见 [写扩展](extensions.md)）。

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

```toml
[features.greet]
name = { en = "Greeting", zh = "打招呼" }
summary = { en = "Greet people" }                  # 可以不写
tools = ["hello_greet"]
```

功能的编号全局不重，建议带包的编号做前缀。

## `[check]`

`miyu check` 时跑 `<程序> <args…>`。你在标准输出上一行一个 JSON 报问题（`kind`、`file`、`level`、`message` 必有，`line`、`column`、`code`、`key` 可以有），只有警告时退出码 0，有错误 1。最多跑 30 秒。要有 `[command]`。

## `[page]`

`dir`：后台页的目录，里面要有 `index.html`，网页的「软件包」页里打开。页面经网页软件给的通道调你登记的方法（`package.methods`、`method.call`，见 [写扩展](extensions.md)）。

## `[[mcp]]`

带 MCP 服务，一段一个，见 [接 MCP 服务](mcp.md)。

## `[[slash]]`

扩展提供的斜杠命令，一段一条；`capabilities` 里要有 `commands`。

| 格 | 说明 |
|---|---|
| `name` | 命令名，写法同 `[command] name`；和 Miyu 自带的命令同名的不收 |
| `summary` | 命令菜单里那一句 |
| `argument` | 名字后面跟什么的提示，例如 `{ en = "<city>", zh = "<城市>" }` |
| `access` | 谁能用：`admin`（默认：本人、群里的主人和管理的人）、`anyone`、`off`（先停用）。人在系统配置里写的压过它 |

人打了这条命令，核心调你的程序 `command.call`，见 [写扩展](extensions.md)。只想展开成一句话发给 AI 的，用命令模板，不用写程序（见 [脚本、技能、命令模板](scripts-skills.md)）。

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

`platform`：接的聊天平台，写法同包的编号（例如 `qq`）。只有扩展能写；接聊天平台的桥写它。

## `[depends]`、`[recommends]`

`packages`：依赖的、推荐的包的编号。依赖的缺了装不上、装着的不起；推荐的缺了照装、照起，少一部分本事。
