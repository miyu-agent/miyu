# 清单 `package.toml`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页逐张表、逐格说明软件包的清单怎么写。清单只接受下面列出的表和键，遇到不认识的表或键会报错（`miyu pkg install` 和 `miyu check` 都会指出是第几行的哪一处不对）。

**给人看的文字**（`name`、`summary`、`about`、配置项的 `name`、`description`……）写成「语言到一句话」的形式：`{ en = "Hello", zh = "打招呼" }`。语言只接受 `zh`、`en`、`ja`；界面按照人使用的语言挑选，没有这种语言时依次退回 `en`、`zh`、`ja`。

**包的种类**不需要写，Miyu 根据清单里带了哪几张表来判断：有 `[process]` 的是扩展；没有程序、带了 `[[mcp]]` 的是 MCP 服务包；有 `[mascot]` 的是吉祥物包，有 `[theme]` 的是外观包（这两种见文末）；只有 `[package]` 的包只带技能、脚本、命令模板、知识库、人格、预设，这几样按照约定放在包文件夹的 `skills/`、`scripts/`、`commands/`、`kb/`、`personas/`、`presets/` 里，清单里不需要写（见 [放文件就能用的](scripts-skills.md)）。另外还有界面、内置、小程序几种包，它们是 Miyu 自己使用的，这里不介绍。

## 一份完整的扩展清单

```toml
[package]
version = "0.1.0"                                   # semver；安装时可以不写，打包、发布到软件源时必须写；其他包写的版本下限和它比较
protocol = [1, 1]                                   # 扩展必须写：能使用的协议主版本 [最低, 最高]，现在只有 1
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # 可以不写
author = "Alice <alice@example.com>"               # 作者，可以不写
upstream = "https://github.com/alice/hello"         # 源码仓库，可以不写
license = "MIT"                                     # 许可证，建议写 SPDX 表达式，可以不写
icon = "hand"                                       # Lucide 的图标名，可以不写
logo = "art/logo.png"                               # 包里自带的一张图标，可以不写
needs = ["images"]                                  # 可以不写：在显示不了图片的地方（语音），整个包视为没有打开
targets = ["linux-x86_64", "macos-aarch64"]         # 可以不写：带了本机程序时，写它能运行的几种系统
systems = ["linux", "macos"]                        # 可以不写：只支持这几种系统；不写表示支持所有系统

[command]                                           # 扩展必须写
name = "hello"                                      # 人运行 miyu hello … 时，核心把参数转给你的程序
program = "miyu-hello"                              # bin/ 下的程序名，不带路径
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]                                           # 有这张表的包就是扩展
args = ["serve"]                                    # 核心启动程序时带的参数
start = "manual"                                    # manual：开关打开时才启动（默认）；always：核心启动后就启动
capabilities = ["tools", "widgets", "commands"]     # 需要的能力，见下面的「能力」；[[widget]] 需要 widgets，[[slash]] 需要 commands

[check]
args = ["check"]                                    # 运行 miyu check 时，核心运行 <程序> check

[settings.greeting]                                 # 一项配置，键是 hello.greeting
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }

[features.greet]                                    # 预设里的一个开关
name = { en = "Greeting", zh = "打招呼" }
tools = ["hello_greet"]

[page]
dir = "page"                                        # 后台页：目录里要有 index.html

[tray]
name = { en = "Open Hello", zh = "打开打招呼" }      # 托盘菜单里的一行，点击后打开上面的后台页

[export]
provides = ["hello"]                                # 在总线上提供的名字：其他包按这个名字调用你，不是按你的包名
methods = ["hello.greet"]                           # 这个名字下面给其他包调用的方法：<名字>.<方法>

[[slash]]
name = "hi"
summary = { en = "Say hi", zh = "打个招呼" }

[[widget]]
id = "status"
place = "status"
```


## 格式是冻结的

这一页列出的表和格是稳定的：以后只会增加，不会修改或删除，所以已经写好的清单在新版本的 Miyu 里仍然可以读取。新加的格和表会写明从哪一版开始提供；如果用到了它们，要在 `[package] miyu` 里写上需要的最低版本，旧版本的 Miyu 读到这样的清单时会说明需要更新。不认识的表和格一律报错，所以不要写这一页以外的内容。
## `[package]`

| 格 | 说明 |
|---|---|
| `name` | 必须写，是给人看的名字。 |
| `protocol` | 带程序的包必须写，格式是 `[最低, 最高]`，现在只有 `1`。 |
| `version` | 包的版本，按照 semver 写（`1.2.0`、`1.2.0-rc.1`）。安装时可以不写；但是 `miyu pkg pack` 和软件源收录都需要它，没有写的包无法打包。其他包依赖你的包时写的版本下限和它比较，软件源也根据它判断有没有新版本。 |
| `summary` | 一句话的说明。 |
| `prefix` | 配置键和记录的事件种类使用的前缀：以小写字母开头，只能使用小写字母、数字、`-`、`_`，最多 32 个字符（比编号的 64 个短），并且不能是 `con`、`nul` 这类 Windows 保留名；不写时使用包的编号，这时不受 32 个字符的限制（编号最长可以到 64 个字符）。如果和其他包的前缀（没有写前缀的包使用编号）冲突，就无法安装。编号很长的包可以用它让键短一些：例如编号是 `qq-bridge`，写 `prefix = "qq"` 以后，配置键是 `qq.*`，记录的事件是 `ext.qq.*`。 |
| `author` | 作者：人名或组织名，可以带邮箱，只能写一行，最多 100 个字符。它会显示在软件包信息页上。 |
| `upstream` | 上游，也就是这个包的源码仓库：以 `https://` 开头，不能带空白，最多 300 个字符。它会显示在软件包信息页上。 |
| `license` | 许可证，建议填写：按照 [SPDX](https://spdx.org/licenses/) 的写法，例如 `MIT`、`CC-BY-SA-4.0`、`MIT OR Apache-2.0`。只能写一行，最多 100 个字符；核心不检查它是不是合法的 SPDX 表达式，会按原样显示给人看（软件包信息页、`miyu pkg info` 的 `许可证` 一行、软件源）。不写也能安装和发布，只是看到这个包的人不知道可以怎样使用它。 |
| `icon` | Lucide 的图标名。界面不一定能识别 Lucide 的全部名字：终端界面只识别它对照表里的名字，识别不了的会画成通用的工具图标（扳手）。 |
| `logo` | 包里自带的一张图标，写成包目录里的相对路径，格式是 `.png` 或 `.webp`。图片不能超过 256 KiB，每条边不能超过 512 像素；`miyu pkg pack` 打包时会把超出限制的图片按原格式缩小到限制以内（目录里的原图不会改动）。如果文件不存在、不是图片或者超出限制，就视为没有图标：包仍然可以安装，`miyu check` 会报警告。界面优先画这张图标，没有时画 `icon`。 |
| `needs` | 这个包需要所在的地方具备什么才有用：现在只接受 `images`（在显示不了图片的会话里，整个包视为没有打开）。 |
| `miyu` | 需要的最低 Miyu 版本：三段数字，例如 `"2026.11.0"`（年.月.这个月的第几版）；候选版带 `-rc.N`（例如 `"2026.11.0-rc.2.4"`，它比同号的正式版旧）。用到新加的格或新的方法时要写上它；旧版本的 Miyu 读到以后会说明需要更新，而不是报看不懂的键。如果 Miyu 的版本太旧，连这一格都无法比较，它在报不认识的表或键时，会在后面补一句 `这个包可能需要更新版本的 Miyu`。 |
| `groups` | 只供出厂的包使用：写这个包属于哪几个组（例如 `["basesystem"]`），预设里写组名就能开关组里的包。其他包写了会报错。 |
| `targets` | 包能安装在哪几种系统上：`linux-x86_64`、`linux-aarch64`、`macos-aarch64`、`windows-x86_64`。不写表示哪种系统都能安装；写了以后，在其他系统上无法安装，打包时用 `miyu pkg pack --target` 为每一种系统各打一个包文件。 |
| `systems` | 包支持哪几种系统：`linux`、`macos`、`windows`，不能重复。不写表示支持所有系统。它和 `targets` 不是一回事：`targets` 用于带了本机程序的包，每种系统一个包文件；`systems` 说明你支持哪几种系统，不带程序的脚本、扩展也可以写（例如还没有做 Windows 适配的包写 `["linux", "macos"]`）。两样都写时，`targets` 里每一项对应的系统都要在 `systems` 里。没有写 `systems` 时，软件源根据 `targets` 判断是否支持。在其他系统上，软件源的列表默认不列出这个包（在设置里打开 `其他系统的软件包` 以后才会列出，并标上 `不支持当前系统`），安装时会被拒绝。 |

## `[command]`

扩展和界面要写这张表。`name`：人在终端里运行 `miyu <name> …` 时，核心把后面的参数原样交给你的程序，退出码使用你的程序的退出码（`miyu help <name>` 会转成 `--help`；如果只运行 `miyu <name> --version` 或 `-V`，参数不会交给你，Miyu 自己输出 `miyu <name> <[package] version>`）。名字以小写字母开头，只能使用小写字母、数字、`-`，最多 32 个字符；如果和内置子命令（`ask`、`config`、`pkg`、`web` 等）冲突，以内置子命令为准。`program`：`bin/` 下的程序名，在 Windows 上核心会找 `<名字>.exe`。`about`：一句话的说明。`args`：可以不写，转交参数时会加在人输入的参数前面。如果一个包要添加几个子命令，就写几段 `[[command]]`，每一段的写法同上；第一段的 `program` 是这个包的程序（扩展启动的就是它）。

所以你的程序除了要识别 `serve` 这类给核心用的参数，也应该识别人输入的那些参数。

## `[process]`

有这张表的包就是扩展，并且要同时有 `[command]`。

| 格 | 说明 |
|---|---|
| `args` | 核心启动程序时带的参数。 |
| `start` | `manual`：开关打开时才启动（默认）；`always`：核心启动后就启动。 |
| `capabilities` | 需要的能力，见下面的说明。 |

**能力**：只接受下面这二十三项。安装时的那一屏会列出你的包需要的每一项能力（名字和一句话的说明由 Miyu 提供，包不能自己写），人回答「是」就批准了。升级以后，升级的那一屏只列出新增的能力；清单里不再需要的能力，批准会被撤销，以后再加回来要重新批准。从本机文件夹或包文件安装的包（包括开发时修改后重新安装），以前的批准不会沿用，每次安装都要重新批准。社区的包，在那一屏上另外显示 `这是个社区包，没有经过官方审核，可能存在风险`；从本机文件安装的包显示 `这个包来自本机文件，没有经过官方审核，可能存在风险`。**要如实声明**：没有声明对应的能力时，调用相应的方法会返回 `needs_approval`。

| 能力 | 能做什么 |
|---|---|
| `tools` | 往工具目录里登记工具。 |
| `tools.commands` | 登记的工具会运行命令（规格里写 `runs_commands`）：这些命令里可以用 `miyu call` 以会话的身份调用 AI 的工具，见 [写扩展](extensions.md)。 |
| `commands` | 提供斜杠命令（`[[slash]]`）。 |
| `search` | 作为搜索服务供应商（`[[search]]`）。 |
| `model.driver` | 提供模型驱动（`[[driver]]`）：使用它的模型时，发给模型的全部对话都要经过它。这项能力不附带网络访问，如果程序自己要上网，需要另外声明 `network`。 |
| `provider.key` | 获取主人在模型设置里配好的某一家供应商的密钥（`[provider_key]`）。出厂的包也需要人批准。 |
| `widgets` | 在界面上显示自己的内容（`[[widget]]`）。 |
| `context.inject` | 在回合开始时往 AI 的上下文里加几句话。 |
| `context.system` | 在系统提示词的末尾放一段内容。 |
| `packages.call` | 通过总线调用其他包导出的方法（`bus.call`）。 |
| `act_for_external` | 代表聊天平台上的人说话，只给平台的桥使用。 |
| `login.page` | 替这台电脑上打开的页面换取登录票（`login.ticket {"via": "page"}`），换到的票只能在本机的连接上换成设备令牌。出厂的包也需要人批准。只能为核实过来自本机的页面申请登录票，来自其他机器的页面一律使用访问密码，见 [扩展参考](../reference/extensions.md) 的「替本机页面换登录票」一节。 |
| `sessions.drive` | 驱动会话：打开会话、发送消息、打断、撤销、压缩、分叉、发送斜杠命令。 |
| `events.read` | 读取会话：订阅事件、读取历史、读取后台命令的输出；在工具调用之后接收通知（`[after]`）。 |
| `events.write` | 往会话里记录自己的事件。 |
| `memory.write` | 写入记忆和知识库。 |
| `fs.read`、`network` | 通过核心读取文件、抓取链接。 |
| `fs.write` | 写入指定的路径。扩展现在不在沙盒里运行，你的程序自己写文件不经过这一项；以后扩展放进沙盒时，会根据这一项放行。 |
| `audio.record` | 录制本机麦克风的声音（用于语音识别、语音助手）。 |
| `audio.play` | 通过本机的音箱、耳机播放声音（用于朗读、语音助手）。 |
| `tool.guard` | 在工具调用执行之前决定放行、拒绝或者询问人（`[guard]`）。 |
| `tool.rewrite` | 在工具执行之后改写结果：这项能力还没有实现。 |

## `[settings.<名字>]`

一项配置写成一张表，键是 `<前缀>.<名字>`（前缀见上面的 `prefix`，不写时使用包的编号）。配置项多的时候可以分组：多写一层 `[settings.<组>.<名字>]`，键就是 `<前缀>.<组>.<名字>`（例如 `im.qq.listen`），只能多这一层；组里的项不能叫 `type`、`name`、`shown_when`，分节的写法见下面。设置页的 `软件包` 那一页里有你的包的一组设置。**不要自己读取配置文件**：扩展的配置值在握手时交给你，修改以后会推送给你（见 [写扩展](extensions.md)）；只带脚本的包，脚本从环境变量 `MIYU_SETTINGS_JSON` 读取（见 [脚本和技能](scripts-skills.md)）。卸载包时，人写在配置里的值按照 pacman 的做法默认保留（这些值不会生效，`miyu check` 只会提示一句这是你的包留下的），重新安装以后按原样生效；只有 `miyu pkg remove --purge` 才会删除它们。

| 格 | 说明 |
|---|---|
| `type` | 必须写，可以是：`bool`、`int`（`min`、`max`）、`number`（小数，`min`、`max` 必须写）、`option`（`choices`，至少两个）、`text`（`max`，默认 200 个字符；`lines`）、`name`、`model`（选择模型：`<供应商>/<模型>` 或模型池 `@<池>`）、`persona`（选择人格，写人格的编号）、`knowledge`（选择知识库，写知识库的编号）、`preset`（选择预设，写预设的编号）、`url`、`secret`（只能写成 `{ secret = … }` 这种引用）、`list`（`element` 是元素的类型，`record` 表示每一项是一组字段，见下面）。 |
| `choices` | `option` 的几个选项：可以是文字的数组（`["auto", "opaque"]`），也可以是表的数组，每张表必须写 `value`，`name`、`description` 可以不写，写法和配置项的 `name`、`description` 相同（设置页用 `name` 画下拉菜单，`description` 用暗色写在下面）。两种写法不能混用；存进配置的是 `value`。 |
| `choices_from` | 只有 `text` 才能写（不能是带 `lines` 的那种，也不能是列表的元素或一组字段里的字段）：写你的程序登记的一个方法的名字，设置页会向这个方法要选项，画成下拉菜单，见下面的「选项由你的程序给」。 |
| `lines` | 在 `text` 上写 `lines = true`，显示成多行的输入框（用于提示词模板这类内容）；不写 `max` 时最多 2000 个字符。 |
| `min`、`max` | `int` 的取值范围，不写时是整数的全部范围；`number` 必须写，并且只接受整数（例如 `0` 到 `2`），更细的精度用 `step` 控制。 |
| `step` | `int`、`number` 每一格的步长（设置页使用）：是正数，`int` 的步长是整数。 |
| `slider` | 在 `int`、`number` 上写 `slider = true`，显示成滑块，此时要写 `min`、`max`；不写 `step` 时，`int` 的步长是 1，`number` 的步长是范围的百分之一；`number` 的两头相同时要写 `step`。 |
| `default` | 默认值，要符合这一项的类型；`secret` 不能写默认值。 |
| `layers` | 只能写 `system`（系统配置），不写时也是它。 |
| `fields`、`title` | 只有 `element = "record"` 的列表才有这两格：`fields` 下面每个字段写一张表 `[settings.<名字>.fields.<字段>]`，写法和一项配置相同（可以用 `type`、`default`、`name`、`description`、`hidden`，也可以用 `number` 类型、带 `name` 和 `description` 的 `choices`、`lines`、`step`、`slider`），但是不写 `layers`、`applies`，也不能再是 `list`；字段名不能叫 `fields`，没有 `default` 的字段每一项都要填写。`title` 指定用哪个 `text`、`name` 或 `option` 字段作为每一项的标题。整个列表不能写默认值，最多 100 项。 |
| `applies` | 修改以后什么时候生效：`now`、`new_session`、`next_turn`、`restart`（重启核心以后）、`program_start`（你的程序下次启动时，这是默认值）。 |
| `name`、`description` | 给人看的名字和说明。 |
| `hidden` | 写 `true` 时，设置页不显示这一项，但是仍然可以写入和查询。 |

如果一项配置是一组字段的列表（例如几个账号，每个账号有备注、地址、Key），可以这样写：

```toml
[settings.accounts]
type = "list"
element = "record"
title = "name"
name = { zh = "账号", en = "Accounts" }

[settings.accounts.fields.name]
type = "text"
name = { zh = "备注" }

[settings.accounts.fields.base_url]
type = "url"
default = "https://api.example.com/v1"
name = { zh = "接口地址" }

[settings.accounts.fields.key]
type = "secret"
name = { zh = "Key" }
```

人在设置页上一张一张地填写卡片，配置文件里写成表的数组 `[[<前缀>.accounts]]`；`secret` 字段仍然只写引用，Key 本身另外保存在密钥文件里。交给你的值是对象的数组：`[{"name": "家里", "base_url": "https://api.example.com/v1", "key": "sk-…"}]`，其中 Key 是真实的值；没有填写的字段使用默认值，没有默认值的字段不会出现。

组自己的那张表 `[settings.<组>]` 可以写两格，设置页根据它们把你的这一组分成几节：`name` 是这一节给人看的名字（写法和配置项的 `name` 相同，不写时显示组名）；`shown_when` 决定这一节什么时候显示，写成 `{ "<这个包的一项>" = "<值>" }`，值也可以是几个值的列表（`["mimo", "openai"]`），那一项当前的值是其中之一时才显示。`shown_when` 只能指向你自己清单里的一个 `option`，或者一个带 `choices_from` 的 `text`（选项在运行时才知道，见下面），写这一项的名字（`engine`；组里的项写 `<组>.<名字>`），不写前缀；它不能指向这一节里面的项；指向 `option` 时，值要是它的选项之一，指向 `text` 时不检查值、只按值比较。不符合这些规则的包无法安装（`bad_shown_when`）。不显示的节只是不画出来，里面的值仍然会保存、交给你，也同样会检查。

```toml
[settings.engine]
type = "option"
choices = ["minimax", "mimo", "openai"]
name = { zh = "引擎", en = "Engine" }

[settings.minimax]
name = { zh = "MiniMax", en = "MiniMax" }
shown_when = { engine = "minimax" }

[settings.minimax.key]
type = "secret"
name = { zh = "Key" }

[settings.cloud]
shown_when = { engine = ["mimo", "openai"] }

[settings.cloud.voice]
type = "text"
name = { zh = "音色" }
```

### 选项由你的程序给

如果选项要到运行时才知道（例如这台机器上的麦克风、引擎当前提供的音色），就写成 `text`，再多写一格 `choices_from = "<方法名>"`。这个方法是你的程序连接核心以后用 `package.methods` 登记的（见 [写扩展](extensions.md)），名字的写法和登记时相同：以小写字母开头，只能有小写字母、数字、`_`、`.`、`-`，最多 64 个字符。名字写错的，或者写在上面说的不能写的地方的，包无法安装（`bad_choices_from`）；其他类型不接受这一格（`unknown_key`）。

```toml
[settings.mic]
type = "text"
name = { zh = "麦克风", en = "Microphone" }
choices_from = "devices.list"
```

设置页显示这一项时，会不带参数地调用你的这个方法，你返回：

```json
{"choices": [{"value": "hw:1,0", "name": "USB 麦克风", "description": "48 kHz"}]}
```

`value` 必须写，是存进配置的文字；`name` 是给人看的名字，不写时显示 `value`；`description` 可以不写，会用暗色写在下面。名字和说明由你自己按照人使用的语言挑好再返回。如果调用失败（程序没有连接、没有登记这个方法、超时）或者返回的列表是空的，设置页会退回成文字输入框，让人手动填写；如果存着的值不在你返回的列表里（例如设备被拔掉了），仍然会显示出来。核心不会用你的列表检查值：这一项仍然是普通的 `text`，交给你的也是原样的文字，用不了的值由你自己处理。

## `[features.<编号>]`

预设里的一个开关管理一组工具。如果不写这张表，整个包算作一个功能；如果写了几个功能，你登记的每一件工具都要列在某个功能下面，否则无法登记。

| 格 | 说明 |
|---|---|
| `name` | 必须写，是给人看的名字。 |
| `summary` | 一句话的说明。 |
| `tools` | 这个功能管理的工具名。 |

```toml
[features.greet]
name = { en = "Greeting", zh = "打招呼" }
summary = { en = "Greet people" }                  # 可以不写
tools = ["hello_greet"]
```

功能的编号在全局范围内不能重复，建议用包的编号作为前缀。

对于接入聊天平台的包（写了 `[connection]` 的包）：预设只管理本机的会话，所以预设菜单里只列出登记时 `venues` 带 `local` 的工具，一件这样的工具都没有的功能整项不列出。只在群和私聊里使用的工具，由你自己的场所开关管理。

## `[check]`

`args`：运行 `miyu check` 时，核心会运行 `<程序> <args…>`。你的程序在标准输出上一行输出一个 JSON 来报告问题（`kind`、`file`、`level`、`message` 必须有；`line`、`column` 是正整数，`code`、`key`、`rule`、`source` 是文字，可以有；其他格不接受）。只有警告时退出码是 0，有错误时退出码是 1。程序最多运行 30 秒。这张表要求清单里有 `[command]`。

## `[page]`

`dir`：后台页的目录，里面要有 `index.html`，在网页界面的 `软件包` 页里打开。页面通过网页界面提供的通道调用你登记的方法（`package.methods`、`method.call`，见 [写扩展](extensions.md)）。

`server`：写 `true` 表示你的程序自己启动网页服务，后台页就是这个服务。它和 `dir` 只能写一个，并且只有扩展能写。你登记的 `status` 方法在回应里写 `web`，值是服务实际监听的端口（还没有开始监听时不写）。头（连接核心的界面程序，例如终端界面、网页界面）和 `miyu pkg page` 根据它打开 `http://127.0.0.1:<端口>/#miyu=<…>`，片段里带有一张登录票，你的页面用它换取设备令牌，再连接核心。

## `[tray]`

这张表在托盘菜单（Miyu 常驻的托盘图标，在 macOS 上是菜单栏图标）里添加一行，点击后用浏览器打开你的后台页（上一节的 `[page]`）。一个包只能添加一行；这一行只能打开你自己的后台页，不能写命令、网址或者其他包。

| 格 | 说明 |
|---|---|
| `name` | 那一行显示的文字，写法和 `[package] name` 相同，按照人使用的语言挑选。 |

```toml
[tray]
name = { en = "Open IM connections", zh = "打开通讯平台接入界面" }
```

只有写了 `[page]` 的扩展和内置包能写这张表；如果没有写 `[page]`，或者写了 `name` 以外的格，会报 `bad_tray`。包要已经安装、已经打开，并且后台页的 `index.html` 存在，托盘才会列出它。

## `[[mcp]]`

包可以带 MCP 服务，一段写一个，详细的见 [接 MCP 服务](mcp.md)。

| 格 | 说明 |
|---|---|
| `name` | 服务名（在配置里是表名），以小写字母开头，只能有小写字母、数字、`-`，长度是 1 到 32 个字符；在全局范围内不能重复。 |
| `command` | 命令和参数。第一项先在包的 `bin/` 里找，再到 `PATH` 里找（`npx`、`uvx`、`docker` 这类）。它和 `url` 只写一个。 |
| `url` | 服务的网址，以 `https://` 开头；`http://` 只能用于本机的服务（`localhost`、`127.0.0.1`），其他机器上的服务不会连接，以免令牌被明文发出去。它和 `command` 只写一个。 |
| `bearer` | 和 `url` 一起使用：令牌，会以 `Authorization: Bearer <值>` 的形式发送；写法和 `env` 的值相同，令牌请写成 `{ secret = … }`。 |
| `headers` | 和 `url` 一起使用：其他请求头，例如 `{ X-Api-Key = { secret = "key" } }`，写法和 `env` 相同。 |
| `env` | 给服务的环境变量。密钥写成 `{ secret = "<名字>" }`（用 `miyu provider login` 或设置页保存进去），不要把值写在清单里。 |
| `venues` | `local`、`private`、`group` 中的一个或几个，分别表示本机的会话、聊天平台的私聊和群；不写时只给本机的会话使用。 |
| `timeout_ms` | 一次调用最多等待多久，单位是毫秒：1000 到 3600000，不写时是 600000。 |
| `clicall` | AI 在命令里写脚本时，能不能用 `miyu call` 调用这个服务的工具，不写时可以。如果服务会写入或发送东西，就写 `false`。 |

## `[[slash]]`

扩展提供的斜杠命令，一段写一条；`capabilities` 里要有 `commands`。

| 格 | 说明 |
|---|---|
| `name` | 命令名，写法和 `[command] name` 相同；和 Miyu 自带的命令同名时不接受。 |
| `summary` | 命令菜单里显示的那一句说明。 |
| `argument` | 提示名字后面跟什么，例如 `{ en = "<city>", zh = "<城市>" }`。 |
| `access` | 谁能使用：`admin`（默认，指本人、群里的主人和管理员）、`anyone`、`off`（暂时停用）。你在配置里写的值会覆盖它。 |

人输入这条命令以后，核心调用你的程序的 `command.call`，见 [写扩展](extensions.md)。如果只想把命令展开成一句话发给 AI，用命令模板即可，不需要写程序（见 [脚本、技能、命令模板](scripts-skills.md)）。

## `[[search]]`

这张表把你的程序登记成一个搜索服务供应商，一段写一个；`capabilities` 里要有 `search`。

| 格 | 说明 |
|---|---|
| `id` | 编号，写法和 `[command] name` 相同，在一个包里不能重复。 |
| `name` | 给人看的名字。 |

登记以后，设置页 `联网服务` 的下拉菜单里会多出一项，值是 `<包的编号>/<编号>`。只有人选了它才会使用，它也不会加入 `自动` 的顺序，所以你的程序不会在人不知道的情况下拿到搜索词。AI 搜索网页时，核心调用你的程序的 `search.query`；如果你的程序无法处理（出错、超出限额），就由出厂的几家搜索服务兜底。见 [写扩展](extensions.md)。

## `[guard]`

这张表把你的程序登记成工具调用之前的守卫：AI 的调用执行之前，核心会问你是放行、拒绝还是询问人；`capabilities` 里要有 `tool.guard`。

| 格 | 说明 |
|---|---|
| `tools` | 要询问你的工具：工具名的数组，自带的、其他包的、MCP 服务的工具都按名字识别。 |
| `access` | 要询问你的访问类别：从 `read`、`write`、`execute`、`network`、`outbound`、`venue`、`install` 里选。 |
| `timeout_ms` | 等待你回答多久，单位是毫秒：100 到 5000，不写时是 1000。到时还没有回答的算作拒绝。 |

```toml
[guard]
tools = ["write", "edit"]
access = ["execute"]
```

只有调用的工具名在 `tools` 里，或者访问类别在 `access` 里，核心才会问你；两样至少要写一样，不许什么都问。你自己登记的工具不会问你。守卫已经打开但程序没有在运行、超时或者出错，都算作拒绝。核心调用你的程序的 `tool.guard`，见 [写扩展](extensions.md)。

## `[[driver]]`

这张表提供一种模型驱动，一段写一个；`capabilities` 里要有 `model.driver`，声明了 `model.driver` 也要至少写一段。

| 格 | 说明 |
|---|---|
| `name` | 驱动名，写法和 `[command] name` 相同，在一个包里不能重复。 |
| `cache` | 缓存类型：`contract`（只要前缀不变就能命中缓存，要实现 `driver.encode`，并用 `miyu dev driver-check` 检查）、`best_effort`（默认）、`none`。 |
| `key` | 是否需要 Miyu 交给它这一家供应商的密钥：`none`、`optional`（默认）、`required`。 |
| `base_url` | 是否需要这一家供应商的地址：`none`、`optional`（默认）、`required`。写 `none` 的驱动（例如自己登录、走本机程序的），设置界面不显示地址那一项；`key = "none"` 同样让界面不显示认证和密钥。 |
| `images` | 能看图片时写 `true`（默认是 `false`）。 |
| `hosted` | 这个驱动会让供应商自己运行的工具名，例如 `["search_web", "read_url_content"]`（默认为空）：安装包时，核心根据它向主人显示 `会让供应商自己运行：…`。每个名字 1 到 64 个字节，不能带控制字符，不能重复。如果驱动交上来的名字不在这个列表里，Miyu 仍然接受、仍然显示，只在运行日志里记一行警告；见 [写扩展](extensions.md#供应商自己跑的工具)。 |
| `quiet_secs` | 多久没有动静才算连接断开，单位是秒，1 到 900（默认使用 Miyu 的空闲时限，从 180 秒起）：等待第一个字、两段输出之间都按照它等待；比 Miyu 的时限短的值不会让等待变短。如果供应商不流式输出思考过程、第一个字常常要等一两分钟，驱动就写这一格，例如 `quiet_secs = 300`。 |

只有主人在供应商的配置里写了 `driver = "<包的编号>/<驱动名>"`，或者你的包用 `[providers.<编号>]` 自带一家指向它的供应商（见下一节），这个驱动才会被使用；模型档案、npm 表和默认值永远使用内置的几种驱动，所以装上你的包也不会接管主人原来配好的供应商。如果包没有安装、没有批准，或者驱动名写错了，`miyu check` 会报告出来，那一家供应商视为不可用，不会退回内置驱动。Miyu 怎样调用你的驱动（`driver.call`、`driver.delta`、`driver.cancel`），见 [协议参考](../reference/extensions.md#drivercalldriverdeltadrivercancel)。

## `[providers.<编号>]`

这张表带一家模型供应商，一张表一家：包已经安装并且打开的时候，这家供应商会出现在主人的供应商列表里，主人可以直接切换到它的模型（`/model`、默认模型），不用自己抄写配置。卸载或关闭包以后，这家供应商就不再出现。扩展包和只带脚本、技能的包都可以写这张表。

```toml
[providers.agy]                                     # 编号：以小写字母开头，只用小写字母、数字、-、_，最多 32 个；模型写成 agy/<模型>
name = { en = "Antigravity", zh = "Antigravity" }   # 可以不写：不写时使用目录里的名字，再没有就用编号
driver = "antigravity/antigravity"                  # 必须写：这个包 [[driver]] 里的一种（<包的编号>/<驱动名>），或者内置的 openai-chat、anthropic、openai-responses
local = true                                        # 可以不写：价格按 0 计算

[providers.agy.models."gemini-3-pro"]              # 可以不写：手写的模型资料，写法和配置里的相同
window = 1000000
```

| 格 | 说明 |
|---|---|
| `name` | 名字，可以写三种语言，按照主人的界面语言挑选；不写时使用目录里的名字，再没有就用编号。 |
| `driver` | 必须写。你自己的扩展驱动写 `<包的编号>/<驱动名>`（驱动名要在这份清单的 `[[driver]]` 里）；也可以是内置的 `openai-chat`、`anthropic`、`openai-responses`。指向其他包的驱动不接受。 |
| `base_url` | 地址，只能写固定的文字（不能写 `{ env = … }`）；如果你的驱动自己连接，可以不写。 |
| `catalog` | 对应 models.dev 里的哪一家供应商，和配置里的写法相同。 |
| `price_multiplier` | 价格的倍率，和配置里的写法相同。 |
| `local` | 表示这是本机的服务，价格按 0 计算。 |
| `cache` | 缓存类别：`contract`、`best_effort`、`per_request`，和配置里的写法相同。 |
| `models` | 手写的模型资料，写成 `[providers.<编号>.models."<模型>"]`，格子和配置里的相同（`window`、`max_output`、`inputs`、`tools`、`reasoning`、`effort`、`price` 等）。模型列表通过你的驱动的 `driver.models` 获取，不需要在这里列全。 |

`key` 不接受：密钥属于主人，由主人在设置里配置（如果你的驱动写了 `key = "required"`，主人没有配置时 `miyu check` 会报告出来）。其他格写错时，写错的那几格不会被接受，`miyu check` 会给出提示。

主人修改这一家供应商的设置时，修改写在主人自己的配置里，会覆盖你给的值（Miyu 会顺便写上 `package = "<你的包>"`）。如果主人的配置里原来就有同一个编号、并且不是覆盖在你这一家上的供应商，以主人的为准，你的这一家不会出现，`miyu check` 会给出提示；所以编号要挑一个不容易冲突的（例如带上你的包名）。如果两个包带了同一个编号，先安装的那个包得到这个编号。

## `[provider_key]`

这张表用来获取主人在模型设置里配好的某一家供应商的密钥，这样主人不用再填一遍；`capabilities` 里要有 `provider.key`，声明了 `provider.key` 也要写这张表。

| 格 | 说明 |
|---|---|
| `catalog` | 要哪一家供应商：写模型目录里的编号，和供应商配置里的 `catalog` 是同一套编号，例如 `deepseek`。 |

```toml
[process]
capabilities = ["provider.key"]

[provider_key]
catalog = "deepseek"
```

安装时的那一屏会显示 `这个软件包会拿到你 DeepSeek 的密钥`，人批准以后才会交给你。Miyu 根据主人的供应商配置判断哪一家是它：写了 `catalog` 的按照 `catalog` 判断；没有写的，看地址里有没有那一家的官方地址（现在只识别 DeepSeek 的 `api.deepseek.com`）；两样都没有写的，看供应商的编号。在识别出来、并且配置了密钥的供应商里，按编号排在第一的那一家的密钥交给你；其他供应商的密钥一律不交。获取密钥的方法见 [写扩展](extensions.md) 的「握手和配置」一节。

## `[after]`

AI 的工具调用得到结果、记录进会话以后，核心会通知你的程序（只读，不等你回答）；`capabilities` 里要有 `events.read`。

| 格 | 说明 |
|---|---|
| `tools` | 要通知你的工具：工具名的数组，自带的、其他包的、MCP 服务的、你自己的工具都按名字识别。 |
| `access` | 要通知你的访问类别：和 `[guard]` 的那几个相同。 |

```toml
[after]
tools = ["shell"]
access = ["write", "network"]
```

两样至少要写一样，不许什么都接收；这张表没有时限这一格（写了会导致清单读取失败）。核心发给你的程序 `tool.after`，见 [写扩展](extensions.md)。

## `[[side]]`

这张表登记你的程序在对话以外调用模型的请求（旁路请求），一段写一条，声明它需要哪一档的模型。不需要另外声明能力。

| 格 | 说明 |
|---|---|
| `id` | 编号，写法和 `[command] name` 相同，在一个包里不能重复。 |
| `tier` | 需要哪一档：`lite`（便宜、快，用于大量的判断）、`normal`（用于需要读懂一段对话的请求）、`flagship`（最好的模型）。 |
| `name` | 给人看的名字。 |
| `summary` | 一句话的说明：什么时候调用、做什么。 |

```toml
[[side]]
id = "judge"
tier = "lite"
name = { en = "Reply judge", zh = "判断要不要回" }
summary = { en = "Decides whether to reply in a group.", zh = "群里有人说话时判断要不要回" }
```

调用 `model.call` 时写 `"side": "judge"`（只写你自己包里的编号，不带包名），不写 `model`、`use`：核心按照档位从同名的模型池（`lite`、`normal`、`flagship`，人在设置里往池里放模型）取模型；如果池是空的，或者池里的模型都在冷却、都出了错，就使用会话的模型——你同时写了 `session` 时使用那个会话的模型，没有写时使用 `models.chat`。人不需要为你的请求另外配置模型。如果写了你没有登记的编号，会返回 `bad_params`。`model.list` 的 `side` 里会列出你登记的请求（编号是 `<包的编号>/<编号>`），以及它们此刻会落到哪个模型上。见 [写扩展](extensions.md)。

## `[[purpose]]`

这张表给你的程序调用 `model.call` 时写的用途（`purpose`）起一个给人看的名字，一段写一条。人查看用量时，按用途分开的那几行（`usage.query` 的 `purpose_name`、终端界面的 `/usage`、网页界面的用量页）会显示这个名字；没有声明的用途按 `purpose` 原样显示。不需要另外声明能力。

| 格 | 说明 |
|---|---|
| `id` | 用途，和 `model.call` 的 `purpose` 完全相同：1 到 32 个小写字母、数字、`-`，在一个包里不能重复。 |
| `name` | 给人看的名字，按语言分开写；写成软件里的说法、名词短语（写「入群审核」，不写「AI 判断要不要让人进群」）。 |

```toml
[[purpose]]
id = "join-approval"
name = { en = "Join review", zh = "入群审核" }
```

核心自己的用途（`title`、`compaction`、`vision` 等）的名字由核心提供，你声明了也不会替换。如果几个包声明了同一个用途，以包的编号排在第一的那个为准。

## `[[widget]]`

这张表往终端界面、网页界面上放一块自己的内容；`capabilities` 里要有 `widgets`。一个包最多放 16 块。

| 格 | 说明 |
|---|---|
| `id` | 在包里唯一：以小写字母开头，只用小写字母、数字、`-`，最多 32 个字符。 |
| `place` | 放在哪里：`home` 首页、`sidebar` 侧边栏、`status` 底栏的一小段、`page` 整页。 |
| `title` | 放在侧边栏和整页的块必须写。 |
| `scope` | `global`（默认）；`session` 表示跟着会话，一个会话一份，只能用于侧边栏。 |
| `command` | 只用于整页：输入这个斜杠命令就会打开这一页。 |
| `state` | 写 `true` 表示这一块的内容来自你的包在会话里的状态（工具结果的 `state`、`session.set_state`，见 [扩展](../reference/extensions.md)），你的程序没有在运行时也同样显示；只用于 `scope = "session"` 的块。 |

块的内容由你的程序推送，见 [写扩展](extensions.md)。

## `[export]`

其他包只能通过总线按名字调用你（`bus.call`），按包的编号调用不到。所以要先声明名字，再把名字下面的方法列出来；只给你自己的后台页、界面块调用的方法不需要写进来，用 `package.methods` 登记就可以了。

`provides`：你在总线上提供的名字（例如 `["stt"]`）；一个名字在已经安装的包里只能有一家提供，安装第二家会报冲突。`plugs`：你接入的名字（例如 `["tts.engine"]`），可以有多家并存，由使用它的一方挑选。名字由点分隔的几段组成，每一段以小写字母开头，只能有小写字母、数字和 `-`；同一个名字不能两样都写。`methods`：这些名字下面给其他包调用的方法（仍然要通过 `package.methods` 登记），方法名以名字为前缀，后面的短名只有一段：例如 `stt` 的 `stt.transcribe`、`tts.engine` 的 `tts.engine.synthesize`。声明的每个名字至少要有一个方法；`methods` 里的每一个方法都要在你声明的某个名字下面，没有声明名字就不能写 `methods`。写错的清单无法读取（`bad_export`）。

## `[content.<包的编号>]`

这张表给另一个包带一个目录的内容（表情包的内容包就是这样做的）：`dir` 是你的包文件夹里的相对目录。被带内容的那个包的扩展通过 `content.list`、`content.read` 读取。这张表可以写几张。

## `[connection]`

`platform`：接入的聊天平台，写法和包的编号相同（例如 `qq`）；接入几个平台时写成列表（`["qq", "tg"]`）。只有扩展能写这一格，接入聊天平台的桥写它。

如果接入了几个平台，工具名以平台开头：`qq_send` 只在 `qq:` 的群和私聊里提供，`tg_send` 只在 `tg:` 的群和私聊里提供；本机的会话里都提供。不带平台前缀的工具在所有地方都提供。

## `[depends]`、`[recommends]`

`packages`：依赖的包、推荐的包的编号（以前的写法 `workers` 仍然可以识别，会读进同一个列表）。缺少依赖的包时无法安装，已经安装的也无法启动；缺少推荐的包时仍然可以安装和启动，只是少一部分功能。

如果要求其他包至少是某一个版本，写成 `<编号>>=<版本>`：

```toml
[depends]
packages = ["onebot>=1.2.0", "embed"]
```

- 只接受 `>=`，中间不加空格；版本按照 semver 写（`1.2.0`、`1.2.0-rc.1`），两段的 `1.2`、带 `v` 的版本、`=`、`<` 等写法都会报 `bad_dependency`。`workers` 里只写编号。
- 版本按照 semver 比较：`1.10.0` 比 `1.2.0` 新，`1.2.0-rc.1` 达不到 `1.2.0`。如果已经安装的那一份没有写 `version`，或者写的不是 semver，算作达不到。
- 已经安装的版本达不到下限，和没有安装一样：无法安装（`depends_missing`），已经安装的无法启动；从软件源安装时，Miyu 会按照下限从软件源里挑选满足要求的版本，一起安装、一起升级。
- 旧版本的 Miyu 读不了这种写法，所以用了这种写法的包要在 `[package]` 里写上 `miyu` 需要的最低版本（见上面的表格）。

## `[mascot]`

这张表把终端界面首页的吉祥物做成一个包：

```toml
[package]
name = { zh = "布丁", en = "Pudding" }
icon = "cat"            # 可以不写

[mascot]
model = "mascot.json"   # 包文件夹里的相对路径
```

- `model` 必须写：包文件夹里的相对路径，不能有 `..`，也不能以 `/` 开头。其他格不接受。
- 只带吉祥物的包只写 `[package]`、`[mascot]`，另外可以写 `[depends]`、`[recommends]`；这种包不使用协议，写了 `protocol` 或其他表都会报 `wrong_kind`。扩展包也可以多带一张 `[mascot]`。
- `miyu check` 会检查模型文件：文件不存在、超过 256 KiB、不是 JSON 对象都会报错。文件里面怎么写由终端界面决定、由终端界面检查，见终端界面仓库的 [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md)（`mascot.json`）。
- 吉祥物包没有开关、配置项和程序；终端界面根据它的配置项 `tui.mascot` 选择使用哪一个吉祥物。

## `[theme]`

这张表把外观（框线、记号、文字的修饰、配色等）做成一个包：可以带终端主题、网页主题，或者两样都带。

```toml
[package]
name = { zh = "夜", en = "Night" }

[theme.tui]
file = "theme.json"     # 终端主题：包文件夹里的相对路径

[theme.web]
dir = "web"             # 网页主题：包文件夹里的相对目录
```

- `[theme]` 下面有 `tui`、`web` 两张子表，至少要写一张，两张都没有时报 `missing_key`。
- `[theme.tui]` 只有 `file` 一格，必须写，写法和 `[mascot] model` 相同，写错时报 `bad_theme_file`。文件里面怎么写见终端界面仓库的 [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md)（「主题」一节）。
- `[theme.web]` 只有 `dir` 一格，必须写，写法和 `[page] dir` 相同，写错时报 `bad_theme_dir`。这个目录是一个普通的网页包（`manifest.json`、`index.js` 和其他文件，可以带脚本），里面怎么写由网页界面决定。
- 以前的写法是在 `[theme]` 下面直接写 `file`，这种写法仍然可以识别，视为 `[theme.tui] file`；如果和 `[theme.tui]` 一起写，会报 `unknown_key`。
- 外观包只写 `[package]`、`[theme]`，另外可以写 `[depends]`、`[recommends]`；写了 `protocol`、`[command]`、`[mascot]` 或其他表都会报 `wrong_kind`：外观包不能带吉祥物，其他包也不能带外观。
- `miyu check` 检查终端主题文件的规则和吉祥物相同；网页主题目录不存在时也会报错。
- 外观包没有开关、配置项和程序；终端界面根据它的配置项 `tui.look` 选择使用哪一个外观，网页界面根据它自己的设置选择。
