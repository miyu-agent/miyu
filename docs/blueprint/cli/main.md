## 主程序 `miyu`

### 是什么

一个程序，像 busybox 那样按子命令分发：`ask`、`undo`（别名 `rewind`）、`restore`、`redo`、`compact`、`recap`、`rename`、`config`、`provider`、`sandbox` 是命令行的头，`core` 是核心进程。装了的软件包加的子命令照清单转交给包里的程序（施工 9-2，`packages.md`「转交」）。不认识的子命令就报错，绝不当成对话发给核心。给人看的话跟着界面语言。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令；换上帮助页；参数不对时交给 `misuse`；拉起核心用的命令 |
| `crates/miyu-cli/src/help.rs`、`help/{zh,en}/{miyu,ask,undo,restore,redo,compact,recap,rename,memory,pkg,kb,schedule,sandbox,config,provider,setup}.txt` | 帮助页（施工 CL-7 起没有 `web` 那一页，网页包自己印）（`pkg` 那一页施工 T-3，`kb` 那一页施工 K-7 下，`schedule` 那一页施工 J-3 下），编进程序（施工 4-11；`web` 那一页施工 W-9，`setup` 那一页施工 8-11，`sandbox` 那一页施工 5-8，`compact` 那一页施工 6-8，`redo` 那一页施工 4-7 再补，`recap` 那一页施工 3-8 四补，`rename` 那一页施工 3-8 五补，`config` 那一页施工 8-2，`provider` 那一页施工 8-5，原来是 `login`、`logout` 两页，Y-13 上并成一页） |
| `crates/miyu-cli/src/misuse.rs` | 参数写错时说的话，不认识的子命令也在这里（施工 4-11）；少了子命令、嵌着的子命令写错、成对的选项少了一个（施工 5-8） |
| `crates/miyu-cli/src/diagnostic.rs` | 报错、警告、提示的写法（设计 47，施工 CL-2 上）：`miyu: ` 打头、警告多 `warning:`（「警告：」）、建议另起一行 `hint:`（「提示：」）；别处交来的原话去掉句末句号、开头大写词改小写；命令行往标准错误上报错都经它 |
| `crates/miyu-cli/src/text_rules.rs` | 给人看的字的检查（施工 CL-3，设计 47 第三节第 10 条）：帮助页说明不带句号、中文命令总表不列「、」串、不写「她」「你」、报错不绕过 `diagnostic` |
| `crates/miyu-cli/src/lib.rs` | 命令行的头对外的几样：`Ask`、`ask`、`talk`、`Format`、`Plan`、`Screen`、`Target`、`exit`，`Undo`、`undo`、`undo_on`、`Direction`、`UndoPlan`，`Compact`、`compact`（施工 6-8），`Recap`、`recap`、`recap_on`、`RecapPlan`（施工 3-8 四补），`Rename`、`rename`、`rename_on`、`RenamePlan`（施工 3-8 五补），`Memory`、`MemoryCommand`、`MemoryClear`、`memory`、`memory_on`、`MemoryPlan`（施工 R-3 再补），`Pkg`、`PkgCommand`、`PkgPlan`、`pkg`、`pkg_on`（施工 T-3），`Kb`、`KbCommand`、`KbPlan`、`kb`、`kb_on`（施工 K-7 下），`Schedule`、`ScheduleCommand`、`ScheduleAdd`、`SchedulePlan`、`schedule`、`schedule_on`、`interval`、`moment`（施工 J-3 下），`Redo`、`redo`、`redo_on`、`RedoPlan`（施工 4-7 再补），`Setup`、`setup`、`setup_on`、`SetupPlan`、`HeadEnv`、`model_ready_on`（施工 8-11），`Sandbox`、`sandbox`，`Web`、`web`、`web_on`（施工 W-9），`help`、`misuse`、`language` |
| `crates/miyu-cli/src/language.rs` | 界面语言；这一页和 `miyu ask` 给人看的字 |
| `crates/miyu-cli/src/language/undo.rs` | `miyu undo`、`miyu restore` 给人看的字（`cli/undo.md`） |
| `crates/miyu-cli/src/sandbox.rs`、`sandbox/flow.rs`、`language/sandbox.rs` | `miyu sandbox setup`、`remove`（`sandbox/windows.md`，施工 5-8） |
| `crates/miyu-cli/src/ask.rs` 的 `exit` | 退出码 0、1、3、4、5；2 在 `main.rs` |

### 对外的样子

| 子命令 | 做什么 | 在哪一页 |
|---|---|---|
| `ask` | 说一句话，打印她的回答 | `cli/ask.md` |
| `undo`（别名 `rewind`） | 撤掉当前会话的最后一轮（文件不改回，施工 U-1） | `cli/undo.md` |
| `restore` | 发下一句之前，恢复最近一次撤销（原来叫 `redo`，施工 4-7 补改名，2026-09-29 项目主人定） | `cli/undo.md` |
| `redo` | 撤掉当前会话的最后一轮，把你那句话（或者换成写的话）再发一次，让她重新做（施工 4-7 再补，2026-09-30 项目主人定） | `cli/redo.md` |
| `compact` | 把当前会话的上下文压缩成摘要，可以附上要求（施工 6-8，命令名 2026-09-29 项目主人定） | `cli/compact.md` |
| `recap` | 一句话回顾当前会话：在做什么、做完了什么、卡在哪（施工 3-8 四补） | `cli/recap.md` |
| `mcp login`、`mcp logout` | 登录、登出要浏览器授权的网址 MCP 服务（施工 X-42 上、下） | `cli/mcp.md` |
| `rename` | 给当前会话起名（施工 3-8 五补） | `cli/rename.md` |
| `memory` | 看她记了你什么，搜、记、改、忘、清空（施工 R-3 再补） | `cli/memory.md` |
| `kb` | 管知识库：列、搜、读、建、改、加、删、挂、重建（施工 K-7 下） | `cli/kb.md` |
| `schedule` | 管定时任务：列、建、删、停、续（施工 J-3 下） | `cli/schedule.md` |
| `pkg` | 看装了哪些软件包，装、卸（施工 T-3） | `cli/pkg.md` |
| `check` | 查手写的文件有没有写错：配置、密钥文件、人格（施工 8-30） | `cli/check.md` |
| `config` | `get`、`explain`、`path`：看配置（施工 8-2，`check` 施工 8-30 挪成 `miyu check`）；`set`、`unset`、`edit`、`trust`：改配置、信任项目配置（施工 8-3） | `cli/config.md` |
| `provider` | `login`、`list`、`logout`：存、列、删供应商的 key（施工 8-5，Y-13 上从 `login`、`logout` 挪来） | `cli/login.md` |
| `setup` | 接上第一个模型：找现成的 key 和本机的服务，或者搜目录、贴 key，试通了写进系统配置（施工 8-11） | `cli/setup.md` |
| `service` | `enable`、`disable`、`status`：让核心开机、登录就起，替本人装、卸、看本人这一层的服务（施工 Z-1 下） | `cli/service.md` |
| `sandbox` | `setup`、`remove`：Windows 上装好、撤掉沙盒用户，要管理员权限；别的平台上说一句不用装 | `sandbox/windows.md` |
| `call` | 在 AI 跑的命令里调这个会话的工具（施工 M-3，设计 39）：不读清单、不经 clap，`main.rs` 当场交给 `miyu_cli::call`；不写进总帮助，`miyu call -h` 印它自己的那一页 | `cli/call.md` |
| `core` | 核心进程：由头拉起，平时不用人敲；不写进帮助 | `core.md` |
| `help` | clap 自带：印帮助，`miyu help <子命令>` 印那一条的 | |

| 选项 | 做什么 |
|---|---|
| `-h`、`--help` | 印帮助 |
| `-V`、`--version` | 印 `miyu <版本>` |

**界面语言**：握手以前照系统的语言（`miyu-store` 的 `locale`）：`LC_ALL`、`LC_MESSAGES`、`LANG` 照这个先后，取第一个设了、不是空的（不是 UTF-8 的当没设），都没设的看系统设置（`sys-locale`：macOS 的首选语言、Windows 的界面语言，Linux 上它看 `LANGUAGE`，施工 8-2）；`zh` 开头的说中文，别的说英文，都没有的也是英文。握手以后，每个子命令都照回应的 `language` 说（施工 8-2，2026-10-01 主会话定）：核心照这个人的 `ui.language` 算，`auto` 的照握手时报的语言；`ja` 的照英文（命令行自己的字只有中文、英文）；老的核心没回的照握手以前的。换了语言的，`miyu ask`、`miyu redo` 给人看的字（`human/<语言>.json`）也照新的那种读一份。

| 语言 | 握手时报给核心的 `locale` | 给人看的字读哪一份 |
|---|---|---|
| 中文 | `zh-CN` | `human/zh.json` |
| 英文 | `en` | `human/en.json` |

### 怎么走

0. **软件包加的子命令**（施工 9-2，`crates/miyu-cli/src/packages.rs`，`packages.md`「转交」）：第一个词不是内置的子命令（名字、别名都算，连 `help`），或者是 `help`、选项、没写，才照磁盘读两层的清单（不连核心）：
   1. 第一个词是某个包 `[command]` 的名字：照那份清单找程序（先找包文件夹的 `bin/`，再找 `miyu` 真实位置旁边的，不找 `PATH`；施工 F-8 上再补），参数（第一个词以后的全部）、环境、标准输入输出原样交给它，Unix 上换成它，Windows 上起它、等它、这期间不理 Ctrl+C；退出码照它的。`help <名字>` 转成 `<程序> --help`。程序没找到：标准错误上说没找到、哪一份清单说这个子命令由它跑，退出码 1。
   2. 主程序的帮助页在最后一组命令后面、「选项」那一节前面多一组「软件包加的命令」（`Commands from packages:`），空一行隔开，一个一行，说明和上面的命令从同一列（第 22 列）开始，说明照界面语言挑；没有的不写这一组（施工 AP-1：原来接在「命令」那一节后面）。撞了内置子命令的不转交、不列：内置的优先。`miyu web` 施工 CL-7 起不再内置，由网页包（`miyu-webui`）的 `[command]` 加，同 `miyu tui`（项目主人 2026-10-30 定：没装时内置的那一条只会报没装）。
   3. 别的照下面走。
1. 先照界面语言给主程序和 `ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename`、`memory`（连同它的子命令，施工 R-3 再补）、`pkg`（连同它的子命令，施工 T-3）、`kb`（连同它的子命令，施工 K-7 下）、`schedule`（连同它的子命令，施工 J-3 下）、`config`（连同它的八个子命令）、`provider`（连同它的 `login`、`list`、`logout`）、`setup`、`sandbox`（连同它的 `setup`、`remove`）换上帮助页（clap 的 `override_help`；`rewind` 是 `undo` 的别名，用同一页），再解析参数。
2. 解析参数，不对的：
   1. 不认识的子命令：标准错误上报 `miyu: 未知命令 '<名字>'`，提示运行 `miyu --help`（下面「给人看的字」），退出码 2。不连核心，不拉起，什么都不发。本机存着的软件源索引（`state/packages/.sync/`，再照名字找 `state/packages/.sources/<名字>/`，不验签名，只是提示）里有包提供这个命令（`provides.command`）的，提示换成「软件包 <编号> 提供这个命令，用 'miyu pkg install <编号>' 安装」（施工 CL-7，`crates/miyu-cli/src/packages/offered.rs`）。
   2. `-h`、`--help`、`help`、`help <子命令>`：把那一页原样印在标准输出上，退出码 0。`-V`、`--version`：印 `miyu <版本>`，退出码 0。
   3. 别的：标准错误上报错（下面「参数写错时」），退出码 2。
3. 没写子命令（施工 9-3，`crates/miyu-cli/src/head.rs`）：标准输入、标准输出都是终端的，连上核心（没在跑就拉起）问 `config.get` 拿 `ui.head`（出厂 `miyu-tui`），照两层清单找这个编号的界面包（`kind = "ui"`、有 `[command]`），程序照第 0 条找，换成它、不带参数，退出码照它的。没有这个界面（没清单、不是界面、没有子命令）：标准错误上报没装哪一个，提示用 `miyu pkg install <编号>` 装；装了别的界面的再提示一行换成哪几个（`miyu config set ui.head <编号>`），一个都没装的提示不用界面也能 `miyu ask`，退出码 1。有清单、程序不在旁边的（出厂带了终端的清单，程序随 M9；9-3 补）：报这个界面的程序找不到，提示重新安装，后一行同上，退出码 1（设计 47，施工 CL-2 上）。「装了的界面」只算程序在 `miyu` 旁边的：只有清单的列出来也打不开。不在终端里（被脚本调、接管道）：把主程序的帮助页印在标准输出上，退出码 2，不连核心。
   - `config` 不写它的子命令（原来的 8-24）：在终端里、`ui.head` 那个界面的清单 `[ui] opens` 里有 `config` 的，同上拉起它、带 `--page config`；没有这一页的、不在终端里的，把 `config` 的帮助页印在标准输出上，退出码 2。
4. `ask`、`undo`（`rewind`）、`restore`、`redo`、`compact`、`recap`、`rename`、`memory`、`pkg`、`kb`、`schedule`、`config`、`provider`、`setup`、`mcp`：交给命令行的头（`cli/ask.md`、`cli/undo.md`、`cli/redo.md`、`cli/compact.md`、`cli/recap.md`、`cli/rename.md`、`cli/memory.md`、`cli/pkg.md`、`cli/kb.md`、`cli/schedule.md`、`cli/config.md`、`cli/login.md`、`cli/setup.md`、`cli/mcp.md`），连同拉起核心用的命令。
5. 拉起核心用的命令：自己这个程序（`std::env::current_exe`，拿不到的用 `miyu`，照 `PATH` 找），加上 `core`。别的参数、环境变量不加；工作目录、标准输入输出、跟终端脱开，由拉起的那一边接（`ipc.md`）。
6. `core`：跑核心进程，`--idle-seconds <秒>` 是空闲多少秒退出，不写的照系统配置 `core.idle_exit`，默认不退（施工 Z-1 上，`core.md`）。
7. `sandbox setup`、`sandbox remove`：交给命令行的头（`sandbox/windows.md`）。

**帮助页**：自己写的，一种语言十三页（`miyu`、`ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename`、`sandbox`、`config`、`provider`、`setup`），编进程序，资源目录找不到也印得出；每页以一个换行结尾，最宽 80 列（中文字算两列）。`ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename` 的七页见 `cli/ask.md`、`cli/undo.md`、`cli/redo.md`、`cli/compact.md`、`cli/recap.md`、`cli/rename.md`，`sandbox` 那一页见 `sandbox/windows.md`，`provider` 那一页见 `cli/login.md`；`miyu sandbox setup -h`、`miyu sandbox remove -h` 印的也是它。`help` 子命令、`core` 不列；`miyu core --help` 照样印得出，是 clap 照代码注释生成的。

总表（`miyu`）照用途分组（对话、内容、软件包、设置、系统，软件包加的命令另一组，施工 AP-1），只列命令和全局选项 `-h`、`-V`，`ask` 和会话那几条命令的选项写在各自的页上；用法写 `miyu [<命令>] [<选项>]`，可以不写的整段用方括号，要填的写 `<名字>`，几个选一的写 `<text|json>`；命令那几组的说明从第 22 列开始，同一节里的选项、子命令说明也各自从同一列开始。末尾一句指向 `miyu help <命令>`。`ask` 的页多一节「退出状态」，列 0 到 5（`cli/ask.md`「退出码」）。

样本 `crates/miyu-cli/src/help/zh/miyu.txt`（帮助页，中文）：

```text
用法：miyu [<命令>] [<选项>]

不带命令时打开默认界面。

对话：
  ask <消息>          发送消息并打印回复
  undo                撤销上一轮（别名：rewind）
  restore             恢复上一次撤销的内容
  redo [<消息>]       重做上一轮，可换成新消息
  compact [<要点>]    压缩上下文
  recap               总结当前会话
  rename <标题>       重命名会话

内容：
  memory <命令>       管理记忆
  kb <命令>           管理知识库
  schedule <命令>     管理定时任务

软件包：
  pkg <命令>          管理软件包
  mcp <命令>          登录或登出 MCP 服务

设置：
  config <命令>       查看和修改设置
  check [<文件>]      检查配置文件
  provider <命令>     管理供应商密钥
  setup               配置模型供应商

系统：
  service <命令>      管理后台服务
  sandbox <命令>      配置或移除沙盒用户（Windows）

选项：
  -h, --help     显示帮助
  -V, --version  显示版本

例子：
  miyu ask "这个项目是做什么的"
  miyu ask -c "测试怎么跑"
  miyu undo

运行 'miyu help <命令>' 查看命令的说明。
```

样本 `crates/miyu-cli/src/help/en/miyu.txt`（帮助页，英文）：

```text
Usage: miyu [<command>] [<options>]

Without a command, open the default interface.

Conversation:
  ask <message>       Send a message and print the reply
  undo                Undo the last turn (alias: rewind)
  restore             Restore what the last undo removed
  redo [<message>]    Redo the last turn, optionally with a new message
  compact [<focus>]   Compact the context
  recap               Summarize the session
  rename <title>      Rename the session

Content:
  memory <command>    Manage memories
  kb <command>        Manage knowledge bases
  schedule <command>  Manage scheduled jobs

Packages:
  pkg <command>       Manage packages
  mcp <command>       Log in to or out of MCP servers

Settings:
  config <command>    View and change settings
  check [<file>]      Check configuration files for errors
  provider <command>  Manage provider keys
  setup               Set up a model provider

System:
  service <command>   Manage the background service
  sandbox <command>   Set up or remove the sandbox user (Windows)

Options:
  -h, --help     Print help
  -V, --version  Print version

Examples:
  miyu ask "what is this project"
  miyu ask -c "how do I run the tests"
  miyu undo

Run 'miyu help <command>' for more on a command.
```

**参数写错时**：标准错误上照设计 47 报错：`miyu: ` 打头一行，能照着改的另起 `hint:`（「提示：」），退出码 2（施工 CL-2 上）。照 clap 报的错分：

| clap 报的 | 中文 | 英文 |
|---|---|---|
| 缺了必写的：最外面那一层只有 `ask` 的要说的话是必写的 | `miyu: 缺少消息`，`提示：用法：miyu ask "<消息>"` | `miyu: missing message`，`hint: usage: miyu ask "<message>"` |
| 嵌着的子命令缺了必写的（`miyu kb read notes`、`miyu memory edit m5`，施工 K-7 修） | `miyu: 缺少参数`，`提示：用法：miyu <命令> <帮助页上这个子命令那一行的写法>`，例如 `miyu kb read <库> <文档>`；帮助页上没有那一行的说到命令为止 | `miyu: missing arguments`，`hint: usage: miyu kb read <kb> <document>` |
| 成对的选项少了一个：只给提升过的自己用的 `--owner-home`、`--owner-sid`（`sandbox/windows.md`） | `miyu: 缺少选项 '<选项>'` | `miyu: missing option '<选项>'` |
| 少了子命令：`miyu sandbox` 后面没写 | `miyu: '<命令>' 需要子命令`，`提示：可用的有：<子命令> 或 <子命令>` | `miyu: '<命令>' requires a subcommand`，`hint: one of: <子命令> or <子命令>` |
| 嵌着的子命令写错：`miyu sandbox frob` | `miyu: '<命令>' 没有子命令 '<名字>'`，`提示：运行 '<命令> --help' 查看可用命令` | `miyu: unknown command '<名字>' for '<命令>'`，`hint: see '<命令> --help'` |
| 不认识的参数，`-` 开头 | `miyu: 无法识别的选项 '<参数>'` | `miyu: unrecognized option '<参数>'` |
| 不认识的参数，别的 | `miyu: 多余的参数 '<参数>'` | `miyu: unexpected argument '<参数>'` |
| 两个选项不能一起写 | `miyu: '<选项>' 不能和 '<选项>' 同时使用` | `miyu: '<选项>' cannot be used with '<选项>'` |
| 选项后面没写值 | `miyu: 选项 '<选项>' 需要一个值` | `miyu: option '<选项>' requires a value` |
| 值不认识 | `miyu: '<选项>' 的值无效：'<值>'`，`提示：可选的值：<值> 或 <值>` | `miyu: invalid value '<值>' for '<选项>'`，`hint: possible values: <值> or <值>` |
| `--add-dir` 后面不是一个已经有的目录（施工 5-10 上） | `miyu: '--add-dir' 的值无效：'<值>' 不是已有的目录` | `miyu: invalid value for '--add-dir': '<值>' is not an existing directory` |
| `--timeout` 后面不是一个时长（施工 7-9，写法见 `cli/ask.md`） | `miyu: '--timeout' 的值无效：'<值>'`，`提示：写成 30s、10m、1h 这样的时长` | `miyu: invalid value for '--timeout': '<值>'`，`hint: use a duration such as 30s, 10m or 1h` |
| `--from` 后面是空的、只有空白（施工 7-10，`cli/ask.md`） | `miyu: 选项 '--from' 需要另一个 harness 的名字` | `miyu: option '--from' requires the name of the other harness` |
| 别的 | `miyu: 参数错误：<clap 的原话>` | `miyu: <clap 的原话>` |

- `<参数>` 照敲的原样；`<选项>` 照 clap 报的，是长的写法，去掉后面的值名（`--session <SESSION>` 写成 `--session`）。
- 少了子命令时，`<命令>` 是 clap 报的那一层的全名（`miyu sandbox`），`<子命令>` 是它报的能写的几个，不列 `help`（`sandbox` 关掉了 `help` 子命令）。
- 子命令写错时，看 clap 报的用法那一行（`Usage: miyu sandbox <COMMAND>`）：第一个 `<`、`[` 之前的几个词就是 `<命令>`。只有 `miyu` 一个词的，是最外面那一层写错，照「怎么走」第 2 条说。
- 能写的值两个的用「或」（`or`）连，三个以上的前面用顿号（逗号）隔开，最后一个前面用「或」（`or`）。
- clap 的原话取它报错的第一行，去掉开头的 `error: `。
- 控制字符换成 `�`：敲的参数可能混着终端的控制序列。
- 说的话、clap 报的用法里，主程序的名字一律是 `miyu`：clap 的 `bin_name` 定死了，不照可执行文件的名字（Windows 上是 `miyu.exe`，施工 5-8 查出来的）。
- 必写的不止一个了，第一种要跟着改：测试查全部子命令里必写的只有这一个。

### 退出码

各条命令共用（`22-命令行.md` 第二节）：

| 码 | 什么时候 |
|---|---|
| 0 | 成功；`--help`、`--version` |
| 1 | 出错了：核心、模型、工具出了问题 |
| 2 | 用法不对：不认识的子命令、参数不对、只敲了 `miyu` |
| 3 | 被打断了（`miyu ask`、`miyu redo`、`miyu compact`）；`miyu ask` 等子代理的时候不等了、到了 `--timeout`（施工 7-9） |
| 4 | 有几步要人确认，这里确认不了，没做（`miyu ask`、`miyu redo`） |
| 5 | 没有可用的模型（`miyu ask`、`miyu redo`、`miyu compact`） |

`miyu core` 的另见 `core.md`。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 不认识的子命令 | `miyu: 未知命令 '<名字>'`，`提示：运行 'miyu --help' 查看可用命令` | `miyu: unknown command '<name>'`，`hint: see 'miyu --help'` |
| 没配模型 | `miyu: 未配置模型`，`提示：运行 'miyu setup' 添加模型` | `miyu: no model configured`，`hint: run 'miyu setup' to add one` |
| 出错了（模型、核心） | `miyu: <哪一类>：<原话>` | `miyu: <kind>: <message>` |
| 核心拒绝了 | `miyu: <核心的原话，去掉句号>` | `miyu: <core's words, without the full stop>` |

- 写法照设计 47：程序名打头，中文、英文都不带句号，建议另起一行。
- 帮助页、参数写错时的话，见上面「怎么走」。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu/tests/commands.rs` | 不认识的子命令：中文、英文的那一句，退出码 2，不拉起核心、核心没起来过；只敲 `miyu`：退出码 2、说用 `miyu ask`；`-h`、`--help`、`help` 印那一页，中文、英文各和样本一样，退出码 0；`--version` 印 `miyu ` 开头；参数写错的几种，中文、英文各说那一句，退出码 2；主程序换了文件名，说的还是 `miyu`（施工 5-8） |
| `crates/miyu-cli/src/help/tests.rs` | 每一页列的选项和程序真有的一一对得上（长短写法、值名），最宽 80 列，以一个换行结尾；值名照表换成页里的写法（`<编号>`、`<目录>`）（施工 5-10 上）、`<时长>`（施工 7-9） |
| `crates/miyu-cli/src/misuse/tests.rs` | 七种错各说哪一句、两种语言；值的连法；控制字符换掉；全部子命令里必写的只有 `ask` 的要说的话；成对的少了一个、少了子命令、嵌着的子命令写错（施工 5-8）；`--add-dir` 后面不是已经有的目录（施工 5-10 上）；`--timeout` 后面不是时长（施工 7-9）；`--from` 后面是空的、只有空白（施工 7-10）；嵌着的子命令少了必写的照帮助页说怎么写，有必写的子命令在帮助页上都有那一行（施工 K-7 修） |
| `crates/miyu/tests/ask.rs` | 参数不对退出码 2（什么都不写、`--session` 和 `--continue` 一起写）；`miyu ask --help` 跟着界面语言；没有 key、核心没在跑的不拉起 |
| `crates/miyu/tests/undo.rs` | `miyu undo --help`、`miyu rewind -h`、`miyu restore --help` 跟着界面语言；`undo`、`restore` 各接各的 |
| `crates/miyu/tests/redo.rs` | `miyu redo --help`、`miyu help redo` 跟着界面语言，`redo` 印的是重做那一页、不是恢复那一页；没有 key、核心没在跑的不拉起（施工 4-7 再补） |
| `crates/miyu/tests/compact.rs` | `miyu compact --help` 跟着界面语言；没有 key、核心没在跑的不拉起（施工 6-8） |
| `crates/miyu/tests/recap.rs` | `miyu recap --help` 跟着界面语言；没有 key、核心没在跑的不拉起（施工 3-8 四补） |
| `crates/miyu/tests/rename.rs` | `miyu rename --help` 跟着界面语言；没写标题是参数不对；没有 key、核心没在跑的不拉起（施工 3-8 五补） |
| `crates/miyu/tests/config.rs` | `miyu config -h`、`miyu config get --help`、`miyu help config` 跟着界面语言；参数不对退出码 2；没有 key、核心没在跑的不拉起；握手以后照 `ui.language` 说，`miyu ask` 也是（施工 8-2，`cli/config.md`） |
| `crates/miyu/tests/login.rs` | `miyu provider -h`、`miyu provider login -h`、`miyu provider logout --help` 跟着界面语言；名字写错、不写名字又不在终端里、`--format` 不在 `list` 上、老写法 `miyu login` 退出码 2；没有 key、核心没在跑的不拉起（施工 8-5，`cli/login.md`） |
| `crates/miyu-cli/src/link/tests.rs` | 握手以后照回应的 `language`，`ja` 的照英文、没回的照旧；握手以前照系统的语言挑（施工 8-2） |
| `crates/miyu/tests/core.rs` | 拉起的是真的 `miyu core`（`core.md`、`ipc.md`） |

### 出处

- `12-进程形态与分发.md` 第三节、R2：一个主程序，按子命令分发；R4：对话必须显式，不认识的子命令直接报错。
- `22-命令行.md` 第二节：命令行的规矩、退出码、「没有 hello 这个子命令」那一句；第五节：命令的全表。
- `00-设计理念.md` 第六节：文字也是数据，住在代码之外（界面的字现在还写在代码里，见「还没有的」）。

### 还没有的

- 只敲 `miyu` 打开终端界面（`22-命令行.md` 第五节、`13-终端界面.md`）：现在只说还没做好。
- 会话怎么接：现在每次 `miyu ask` 开一个一次性会话，`--continue`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename` 管的都是上一次 `miyu ask` 开的那个，容易让人迷惑。做头的时候和终端里的会话一起重定（2026-09-28 项目主人定）。
- 第五节表里的其余命令：`stdio`、`web`、`status`、`doctor`、`logs`、`session`、`config`、`persona`、`preset`、`memory`、`venue`、`listen`、`stt`、`pkg`、`tools`、`account`、`service`、`upgrade`、`completions`（`shellhook` 归终端界面的包，`22-命令行.md` 第七节）。
- 界面语言是配置里跟着人走的一项（`14-配置.md` 第一节、`16-人格与预设.md` 第五节）；界面的字放在代码之外（`00-设计理念.md` 第六节）：现在照环境变量，字写在代码里。
