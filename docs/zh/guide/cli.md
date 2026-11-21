# 命令行全表

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页列出 `miyu` 的每个子命令、参数、输出和退出码，供你查阅。如果某个命令有更详细的用法说明，各节的末尾会链接到讲解它的那一页。

## 总表

下面是全部内置命令，按用途分类：

| 用途 | 命令 | 做什么 |
|---|---|---|
| 对话 | [`ask`](#miyu-ask) | 说一句话，输出 AI 的回答 |
| | [`undo`](#miyu-undo)（别名 `rewind`） | 撤销最后一轮 |
| | [`restore`](#miyu-restore) | 恢复刚刚撤销的那一轮 |
| | [`redo`](#miyu-redo) | 撤销最后一轮，用原来的话或者换一句话重新来一次 |
| | [`compact`](#miyu-compact) | 把上下文压缩成摘要 |
| | [`recap`](#miyu-recap) | 让 AI 写一段回顾 |
| 会话 | [`rename`](#miyu-rename) | 修改会话的名字 |
| | [`session`](#miyu-session) | 导出、导入会话 |
| 模型和配置 | [`setup`](#miyu-setup) | 接入第一个模型 |
| | [`provider`](#miyu-provider) | 存储、列出、删除供应商的密钥 |
| | [`config`](#miyu-config) | 查看、修改配置，信任项目配置 |
| 软件包 | [`pkg`](#miyu-pkg) | 安装、卸载、查询软件包 |
| | [`mcp`](#miyu-mcp) | 登录、登出需要浏览器授权的 MCP 服务 |
| | [`bus`](#miyu-bus) | 查看总线上哪个软件包提供了什么功能 |
| | [`dev`](#miyu-dev) | 供编写软件包的人使用 |
| 记忆和知识库 | [`memory`](#miyu-memory) | 查看、搜索、记下、修改、忘掉记忆 |
| | [`kb`](#miyu-kb) | 管理知识库 |
| 定时 | [`schedule`](#miyu-schedule) | 管理定时任务 |
| 服务和升级 | [`service`](#miyu-service) | 让核心在开机或登录时自动启动 |
| | [`upgrade`](#miyu-upgrade) | 升级 Miyu |
| | [`core`](#miyu-core) | 启动、停止核心，查看它是否在运行 |
| 排错 | [`check`](#miyu-check) | 检查手写的文件有没有写错 |
| | [`rescue`](#miyu-rescue) | 核心出问题时用 LiteMiyu 修复 |
| 供脚本使用 | [`call`](#miyu-call) | 在 AI 运行的命令里调用这个会话的工具 |

出厂自带的软件包还另外提供三个命令：

| 命令 | 哪个软件包 | 做什么 |
|---|---|---|
| [`tui`](#miyu-tui) | 终端界面（`tui`） | 打开终端界面 |
| [`shellhook`](#miyu-shellhook) | 终端界面（`tui`） | 在 shell 的提示符后面直接和 AI 对话 |
| [`web`](#miyu-web) | 网页界面（`webui`） | 打开网页界面 |

单独安装的软件包也会带来命令，安装之后才有：

| 命令 | 做什么 | 说明 |
|---|---|---|
| `im` | 开启、关闭、查看通讯平台的接入 | 安装 `接入通讯平台` 之后才有，见 [接入通讯平台](heads/im.md) |
| `migrate` | 把旧版 Miyu 的数据目录迁移到这一版 | 安装 `从旧版 Miyu 迁过来` 之后才有，见 [从 0.7 旧版迁过来](migrate.md) |
| `ocr` | 提取图片里的文字 | 安装 `OCR文字识别` 之后才有，见 [附加软件](addons.md) |

## 不带子命令运行

在终端里直接运行 `miyu`，会打开设置里 `默认界面`（`ui.head`）指定的界面，出厂设置是终端界面。如果核心没有在运行，会先启动核心。如果标准输入、标准输出中有一个不是终端（例如在脚本或管道里运行），只输出帮助，退出码是 `2`。

如果 `ui.head` 指定的界面没有安装，会报错 `界面 'tui' 没有安装`，并提示用 `miyu pkg install tui` 装回来，或者用 `miyu config set ui.head <编号>` 换成已经安装的界面。

## 软件包加的命令

如果 `miyu <名字>` 不是内置的子命令，Miyu 会在已经安装的软件包里寻找提供这个命令的软件包，把后面的参数原样交给它的程序，并使用那个程序的退出码。在 Linux、macOS 上，`miyu` 会被替换成那个程序；在 Windows 上，`miyu` 启动那个程序并等待它退出，在这期间按下的 `Ctrl+C` 由那个程序处理，`miyu` 自己不处理。如果和内置的命令重名，内置的命令优先。`miyu -h` 里会多出一节 `软件包加的命令：`，列出已经安装的这类命令。`miyu help <名字>` 输出的是那个程序自己的帮助。如果只运行 `miyu <名字> --version`（或 `-V`），`miyu` 会自己输出 `miyu <名字> <包的版本>`，不交给那个程序；如果后面还跟着其他参数，就照常交给那个程序。那个程序的环境变量里有 `MIYU_HOME`（当前这一份数据根）和 `MIYU_PROGRAM`。

如果运行了没有安装的命令，会报错 `未知命令 '<名字>'`。如果本机保存的软件源索引里有提供这个命令的软件包，会提示 `软件包 <编号> 提供这个命令，用 'miyu pkg install <编号>' 安装`。

## 所有命令通用的约定

下面这些约定对所有命令都适用。

### 帮助和版本

| 写法 | 做什么 |
|---|---|
| `miyu -h`、`miyu --help` | 输出总的帮助 |
| `miyu <命令> -h`、`miyu help <命令>` | 输出这个命令的帮助。如果命令有子命令，帮助是完整的一页，所有子命令都写在里面 |
| `miyu -V`、`miyu --version` | 输出版本，例如 `miyu 2026.11.0` |

帮助和版本输出在标准输出上，退出码是 `0`。

### 输出格式

多数用来查看信息的命令都有 `--format <text|json>` 选项：出厂设置 `text` 是给人看的格式，`json` 会输出一行 JSON 给脚本使用。有这个选项的命令是 `bus`、`check`、`config get`、`config explain`、`kb`、`memory`、`pkg`、`provider list`、`schedule`、`service`。`ask` 还多一种 `jsonl`，见 [`miyu ask`](#miyu-ask)。

只有标准输出是终端、并且没有设置环境变量 `NO_COLOR` 时，输出才会带颜色。

### 界面语言

命令行只有中文和英文两种语言。连接核心以前，命令行使用环境变量 `LC_ALL`、`LC_MESSAGES`、`LANG` 中第一个设置了的值；如果都没有设置，就使用系统设置（macOS 的首选语言、Windows 的界面语言）。值以 `zh` 开头时使用中文，其他情况使用英文。帮助页和参数写错时的报错都使用这里决定的语言。

连接核心以后，命令行使用设置页 `通用 → 显示 → 界面语言`（`ui.language`）的设置：写 `zh` 时使用中文，写 `en`、`ja` 时使用英文；`auto`（出厂设置）按照上面的系统语言决定。

### 报错的格式

报错输出在标准错误上，以 `miyu: ` 开头；如果有你可以照着做的操作，会另起一行，以 `提示：` 开头：

```text
miyu: 'miyu kb' 没有子命令 'ls'
提示：运行 'miyu kb --help' 查看可用命令
```

### 退出码

`miyu` 的退出码有下面几种：

| 退出码 | 意思 | 哪些命令会出 |
|---|---|---|
| `0` | 成功完成 | 全部 |
| `1` | 出错：连接不上核心、核心拒绝了请求、模型或工具出错 | 全部 |
| `2` | 参数不对，没有做任何事 | 全部 |
| `3` | 被打断（`Ctrl+C`），或者到了 `--timeout` 设定的时间 | `ask`、`redo`、`compact`、`memory dream`、`call` |
| `4` | 这一轮结束了，但有几步需要人确认，因为没有人能确认，这几步没有执行 | `ask`、`redo`、`call` |
| `5` | 没有可用的模型：没有配置模型，或者模型都在冷却 | `ask`、`redo`、`compact`、`recap` |
| `130` | 粘贴密钥时取消了：按了 `Ctrl+C`；在 Linux、macOS 上一个字都没有输入时按 `Ctrl+D` 也算取消 | `setup`、`provider login` |

因为在 `miyu ask` 里没有人能当场确认，所以需要确认的步骤一律不执行，退出码是 `4`。这类操作需要到终端界面或网页界面里完成，见 [权限和沙盒](permissions.md)。

### 从标准输入读取的命令

下面这些命令会从标准输入读取内容：

| 命令 | 读什么 |
|---|---|
| `miyu ask` | 如果没有写要说的话，就把读到的整段内容当作要说的话；如果话里有一个词正好是 `-`，就把读到的内容接在后面 |
| `miyu setup --provider <编号>` | 不在终端里运行时，从管道读取密钥 |
| `miyu provider login` | 从管道读取密钥 |
| `miyu call <工具> --json -` | 从管道读取参数的 JSON |
| `miyu pkg`、`miyu upgrade` | 需要确认时读取一行回答。如果标准输入是关闭的，就不执行操作，此时需要加上 `--yes` |

## miyu ask

```text
miyu ask [<选项>] [<话>]
```

`miyu ask` 用来说一句话，并输出 AI 的回答。如果没有写 `-c` 或 `-s`，会新开一个会话。如果 AI 派出了子代理，命令会等所有子代理都回报、回答结束之后才退出。

| 参数 | 做什么 |
|---|---|
| `-c`、`--continue` | 接着上一次 `miyu ask` 开始的会话继续说。不能和 `-s` 一起使用 |
| `-s`、`--session <编号>` | 接着这个会话继续说 |
| `-C`、`--cd <目录>` | 在这个目录里工作，这个目录必须已经存在。只对新会话有效 |
| `--persona <人格>` | 使用这个人格。只对新会话有效，不能和 `--no-persona` 一起使用 |
| `--no-persona` | 不使用人格。只对新会话有效 |
| `--preset <预设>` | 使用这个预设。只对新会话有效 |
| `--permission <read-only\|full>` | 开始时的权限级别：只读或开放权限。只对新会话有效 |
| `--format <text\|json\|jsonl>` | `text`（出厂设置）边接收边输出；`json` 在最后一行输出一个 JSON；`jsonl` 每行输出一个事件 |
| `--add-dir <目录>` | 把另一个目录也加入工作区，可以读也可以写；这个参数可以写多次 |
| `--file <文件>` | 附上一个文件（图片、PDF、文本）；这个参数可以写多次 |
| `--timeout <时长>` | 最多等待多长时间，例如 `30s`、`10m`、`1h`，不写单位时按秒计算 |
| `--from <名字>` | 由其他 agent 调用时，报上调用方自己的名字，例如 `claude-code` |
| `--model <模型>` | 使用这个模型或 `@池`。如果是接着说的会话，这个会话从此换成这个模型 |
| `--no-memory` | 不召回记忆，也不保存记忆。只对新会话有效 |

标着「只对新会话有效」的几个参数不能和 `-c`、`-s` 一起使用。

例如，让 AI 审查当前的改动：

```bash
git diff | miyu ask --format jsonl "审查这些改动" -
```

```powershell
git diff --output=改动.diff
miyu ask --format jsonl --file 改动.diff "审查这些改动"
```

退出码：`0` 到 `5` 都可能出现，见上面的退出码表。详细的用法见 [会话](sessions.md)。

## miyu undo

```text
miyu undo [-s <编号>]
```

`miyu undo` 撤销会话的最后一轮，只回退对话，不会把文件改回去。这个命令也可以写成 `miyu rewind`。

| 参数 | 做什么 |
|---|---|
| `-s`、`--session <编号>` | 指定哪个会话。不写时是上一次 `miyu ask` 开始的那个会话 |

例如：

```sh
miyu undo
```

如果请求被拒绝（例如 `没有可撤销的回合`），退出码是 `1`。见 [会话](sessions.md#撤销恢复重做)。

## miyu restore

```text
miyu restore [-s <编号>]
```

`miyu restore` 恢复刚刚撤销的那一轮，需要在发送下一句话之前使用。参数和 `miyu undo` 相同。

例如：

```sh
miyu restore
```

如果没有可以恢复的内容，会报错 `没有可恢复的撤销`，退出码是 `1`。

## miyu redo

```text
miyu redo [-s <编号>] [<话>]
```

`miyu redo` 撤销最后一轮，把你说的那句话重新发送一次；如果写了新的话，就换成新的话。参数和 `miyu undo` 相同，回答的输出方式和 `miyu ask` 一样。

例如：

```sh
miyu redo 只改 README，别动代码
```

退出码和 `miyu ask` 相同。如果最后一轮不是由你说的话开始的，会报错 `无法重做`，退出码是 `1`。

## miyu compact

```text
miyu compact [-s <编号>] [<要求>]
```

`miyu compact` 把会话的上下文压缩成摘要，最近的一段保留原文。命令后面写的话会交给生成摘要的过程，用来说明要保留什么。参数和 `miyu undo` 相同。

例如：

```sh
miyu compact 保留数据库迁移那部分的细节
```

退出码：`0` 表示压缩完成；`1` 表示请求被拒绝或者压缩失败（没有内容可以压缩时会报错 `没有可压缩的内容`）；`3` 表示被打断；`5` 表示没有可用的模型。见 [会话](sessions.md#上下文满了)。

## miyu recap

```text
miyu recap [-s <编号>]
```

`miyu recap` 让 AI 写一段回顾：正在做什么、做完了什么、卡在哪里。命令只把这一段回顾输出到标准输出，所以可以接管道。参数和 `miyu undo` 相同。

例如：

```sh
miyu recap -s 0198c2a4-7e1b-7c3d-9a2f-4b5c6d7e8f90
```

退出码：`0` 表示回顾写好了；`1` 表示请求被拒绝（没有可以回顾的内容、回顾没有写成、没有这个会话）；`5` 表示没有可用的模型。

## miyu rename

```text
miyu rename [-s <编号>] [<标题>]
```

`miyu rename` 修改会话的名字。标题里的几个词会用空格连起来；如果不写标题，会让 AI 根据对话重新起一个。修改成功时不输出任何内容。参数和 `miyu undo` 相同。

例如：

```sh
miyu rename 迁移数据库
```

如果请求被拒绝（标题是空白或者太长、没有这个会话），退出码是 `1`。

## miyu session

```text
miyu session export [<编号>] [-o <文件>] [--md [--for <用途>] [--last <轮数>]]
miyu session import <文件> [--archive]
```

`miyu session` 用来导出和导入会话：

| 子命令、参数 | 做什么 |
|---|---|
| `export [<编号>]` | 导出会话的原始包（ZIP）。不写编号时导出上一次 `miyu ask` 的会话。导出完成后输出文件写到了哪里 |
| `-o`、`--output <文件>` | 写到这个文件。不写时是当前目录下的 `miyu-session-<编号前 8 位>.zip`，导出 Markdown 时扩展名是 `.md`。导出 Markdown 时写 `-` 会输出到标准输出 |
| `--md` | 导出成 Markdown |
| `--for <read\|agent\|chat>` | Markdown 给谁看：`read` 给自己看（出厂设置），`agent` 给其他 AI，`chat` 用来贴进网页版的聊天。必须和 `--md` 一起使用 |
| `--last <轮数>` | Markdown 只包含最近几轮，至少 1 轮。必须和 `--md` 一起使用 |
| `import <文件>` | 导入原始包，每行输出一个新导入的会话编号；这个数据根里原来就有的会话不输出 |
| `--archive` | 导入成只读存档：可以查看、搜索，但不能接着聊 |

例如：

```sh
miyu session export --md --for agent --last 20 -o -
```

如果请求被拒绝（文件已经存在、要写进数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）、文件不是会话包），退出码是 `1`。见 [会话](sessions.md#导出导入)。

## miyu setup

```text
miyu setup [--provider <编号>] [--env <变量>] [--model <模型>]
```

`miyu setup` 用来接入第一个模型：选择一家供应商，粘贴密钥，测试连接，选择模型，然后写进系统配置。

| 参数 | 做什么 |
|---|---|
| `--provider <编号>` | 使用模型目录里的这一家供应商，不再让你选择 |
| `--env <变量>` | 从核心环境里的这个变量读取密钥，不再让你粘贴 |
| `--model <模型>` | 使用这个模型，不再让你选择 |

不在终端里运行时，必须写 `--provider`，密钥从管道读取：

```bash
echo "$KEY" | miyu setup --provider deepseek
```

```powershell
$key | miyu setup --provider deepseek
```

在终端里粘贴密钥时不会显示内容，按 `Ctrl+C` 可以取消；在 Linux、macOS 上，一个字都没有输入时按 `Ctrl+D` 也会取消。

退出码：`0` 表示已经写进配置；`1` 表示没有选择、没有收到密钥、测试连接失败，或者核心拒绝了请求；`2` 表示参数不对，或者不在终端里运行却没有写 `--provider`；`130` 表示粘贴密钥时取消了。见 [模型](models.md#快速配置miyu-setup)。

## miyu provider

```text
miyu provider login [<名字>]
miyu provider list [--format <text|json>]
miyu provider logout [<名字>]
```

`miyu provider` 用来存储、列出和删除密钥：

| 子命令 | 做什么 |
|---|---|
| `login [<名字>]` | 存储一个密钥。不写名字时，在终端里从配置用到的、已经存过的名字里选择。粘贴时不显示内容，也可以从管道读取 |
| `list` | 列出已经存储的密钥，以及配置里用到了但还没有存储的密钥（显示为灰色）。不显示密钥的内容 |
| `logout [<名字>]` | 删除一个密钥。不写名字时，在终端里从已经存过的名字里选择 |

例如：

```bash
echo "$KEY" | miyu provider login acme
```

```powershell
$key | miyu provider login acme
```

粘贴密钥时取消的方法和 `miyu setup` 相同。

退出码：`0` 表示成功；`1` 表示核心拒绝了请求、没有收到密钥、没有选择，或者要删除的密钥没有存过；`2` 表示名字不符合写法，或者没有写名字又不在终端里运行；`130` 表示粘贴密钥时取消了。见 [模型](models.md#存放密钥)。

## miyu config

```text
miyu config <子命令> [<选项>]
```

`miyu config` 用来查看和修改配置，以及信任项目配置：

| 子命令、参数 | 做什么 |
|---|---|
| `get [<键>…] [--format <text\|json>]` | 输出生效的值。不写键时输出全部；只给一个键时只输出值 |
| `set <键> <值>` | 修改一项设置，写进系统配置 `~/.miyu/etc/config.toml`。不接受 `--project`，因为项目配置只能手动修改 |
| `unset <键>` | 从系统配置里删除一项，让它回到默认值 |
| `edit [--project]` | 用编辑器打开系统配置（写 `--project` 时打开当前目录的项目配置），保存之前先检查。编辑器按照 `VISUAL`、`EDITOR` 的设置选择，都没有设置时，Linux、macOS 上使用 `vi`，Windows 上使用 `notepad` |
| `explain <键> [--format <text\|json>]` | 输出这一项在每一层写了什么、哪一层生效 |
| `path [--project]` | 输出系统配置的路径；写 `--project` 时输出当前目录的项目配置的路径 |
| `trust [--yes \| --no]` | 查看当前目录的项目配置会修改什么，然后决定是否信任。`--yes` 表示信任，`--no` 表示不信任，两者都不再询问 |

在终端里只运行 `miyu config`，会打开默认界面的设置页；如果界面没有设置页，或者不在终端里运行，会输出帮助，退出码是 `2`。

例如，查看 `models.chat` 在每一层的值：

```sh
miyu config explain models.chat
```

列表值按照 TOML 的数组写。在 bash 里，整个值用单引号括起来；在 PowerShell 里，外面用双引号，里面用 TOML 的单引号字符串（因为 Windows PowerShell 5.1 会吃掉参数里的双引号）：

```bash
miyu config set skills.external.dirs '["~/.agents/skills", "~/my-skills"]'
```

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/my-skills']"
```

退出码：`0` 表示成功；`1` 表示核心拒绝了请求（键不存在、值不对、文件有错误）；`2` 表示参数不对，或者不在终端里运行 `edit` 或不带 `--yes`、`--no` 的 `trust`。见 [配置](config.md)。

## miyu pkg

```text
miyu pkg [<子命令>] [<选项>]
```

`miyu pkg` 用来安装、卸载和查询软件包。不写子命令时运行 `list`。

| 子命令 | pacman 写法 | 做什么 |
|---|---|---|
| `list` | `-Q` | 列出已经安装的软件包 |
| `install <软件包>` | `-U` | 安装软件包。`<软件包>` 可以是包含 `package.toml` 的目录、`.miyu` 包文件，或者编号（从软件源安装，或者装回卸载掉的出厂软件包） |
| `remove <编号>` | `-R` | 卸载软件包，它的状态一起删除，配置目录和配置文件里它的设置项保留下来 |
| `info <软件包>` | | 查看一个软件包的信息。没有安装的软件包会去软件源查询；如果写的是目录或 `.miyu` 文件，只查看，不安装 |
| | `-Qi <编号>` | 只查询已经安装的软件包 |
| | `-Si <编号>` | 只查询软件源 |
| `files <编号>` | `-Ql` | 列出它安装了哪些文件 |
| `owns <路径>` | `-Qo` | 查询一个文件属于哪个软件包 |
| `check [<编号>]` | `-Qk` | 检查安装好的文件有没有被改动或者缺失 |
| `page <编号>` | | 用浏览器打开它的后台页 |
| `pack <目录>` | | 把软件包目录打包成 `.miyu` 文件，不连接核心 |
| `update` | `-Sy` | 从软件源获取新的索引 |
| `search [<词>…]` | `-Ss` | 在软件源里搜索，结果要包含所有的词；不写词时列出全部 |
| `upgrade` | `-Su` | 按照软件源升级已经安装的软件包。`-Syu` 会先获取索引再升级 |
| `clean` | `-Sc` | 清除缓存里不是当前安装版本的包文件 |
| | `-S <编号>…` | 从软件源安装这几个软件包 |

| 选项 | 做什么 |
|---|---|
| `--format <text\|json>` | 输出格式 |
| `--yes`、`--noconfirm` | 安装、升级、卸载之前不询问，软件包需要的能力一并批准 |
| `-n`、`--purge` | `remove` 时把配置目录和配置文件里它的设置项也一起删除 |
| `--target <系统>` | `pack` 为哪种系统打包；如果清单里写了 `targets`，必须选择其中一种 |
| `-o`、`--out <目录>` | `pack` 写到哪个目录，不写时是当前目录 |
| `--print` | `page` 只输出网址，不打开浏览器 |

例如，搜索并安装 OCR 软件包：

```sh
miyu pkg search ocr
miyu pkg install ocr
```

退出码：`0` 表示成功；`1` 表示核心拒绝了请求，或者 `search` 一个结果都没有搜到、`owns` 查不到文件属于哪个软件包、`check` 查出了被改动或缺失的文件；`2` 表示参数不对。见 [软件包](packages.md)。

## miyu mcp

```text
miyu mcp login <服务>
miyu mcp logout <服务>
```

`miyu mcp` 用来登录和登出需要浏览器授权的 MCP 服务：

| 子命令 | 做什么 |
|---|---|
| `login <服务>` | 登录只接受浏览器授权的网址型 MCP 服务。命令会输出授权页的地址，并尝试打开浏览器；你同意授权以后，令牌会存进密钥文件，服务会自动连接。如果浏览器在另一台电脑上，授权之后把浏览器地址栏里的完整地址粘贴回来，按回车即可。最多等待 10 分钟 |
| `logout <服务>` | 删除保存的登录信息 |

`<服务>` 是这个 MCP 服务在配置或软件包里的名字。

例如：

```sh
miyu mcp login acme-docs
```

退出码：`0` 表示已经连接上或者已经登出；`1` 表示核心拒绝了请求、等待时间到了还没有连接上，或者连接不上核心。见 [扩展 Miyu](extending.md)。

## miyu bus

```text
miyu bus [list [<名字>]] [--format <text|json>]
```

`miyu bus` 用来查看总线上哪个软件包提供了什么功能。不写子命令时运行 `list`。软件包在总线上按照名字提供功能，彼此之间不写死软件包的名字；`list` 列出每个名字由谁提供、能不能使用，写了名字时只列出这个名字。

例如：

```sh
miyu bus list render
```

详细的见 [软件包](packages.md)。

## miyu dev

```text
miyu dev driver-check <编号>
```

`miyu dev` 供编写软件包的人使用：`driver-check` 检查这个软件包提供的模型驱动的编码是否稳定。检查时会重启它的扩展，如果有会话正在使用，就不进行检查。全部通过时退出码是 `0`，有没通过的项时退出码是 `1`。见 [写一个 Miyu 软件包](../dev/packages.md)。

## miyu memory

```text
miyu memory [<子命令>] [<选项>]
```

`miyu memory` 用来管理记忆。不写子命令时运行 `list`。选项写在子命令的前面或后面都可以。

| 子命令 | 做什么 |
|---|---|
| `list` | 列出记忆，新的排在前面 |
| `search <词>…` | 搜索记忆 |
| `add <内容>…` | 记下一条记忆 |
| `edit <m序号> [<内容>…]` | 修改一条记忆，并输出新的序号；如果只修改 `--keywords`、`--who`，就不用写内容 |
| `forget <m序号>` | 作废一条记忆 |
| `clear session [<编号>]` | 清除一个会话记下的记忆。不写编号时使用 `-s` 指定的会话，如果也没有 `-s`，就是上一次 `miyu ask` 的会话 |
| `clear me` | 清空这一份记忆：全部类别、认识的人的卡、对你的了解。这个命令不会先询问，清空后找不回来 |
| `dream` | 立即整理记忆。按 `Ctrl+C` 可以停下，已经整理好的部分会保留 |
| `cards` | 列出人物卡 |

记忆写成 `m` 加序号的形式，例如 `m12`，序号按照 `list` 的输出填写。

| 选项 | 做什么 |
|---|---|
| `--persona <编号>` | 使用这个人格的记忆，不写时使用默认人格。不能和 `-s` 一起使用 |
| `-s`、`--session <编号>` | 使用这个会话所用的那一份记忆 |
| `--forgotten` | `list`、`search` 时把作废的记忆也一起列出 |
| `--class <user\|feedback\|episode\|reference\|person>` | `list` 时只列出这一类；`add` 时记成这一类，不写时是 `user` |
| `--why <原因>` | `forget` 时写明为什么忘掉 |
| `--keywords <词>` | `add`、`edit` 时使用：检索用的关键词，用逗号隔开；`edit` 时写空值表示去掉关键词 |
| `--who <是谁>` | `add`、`edit` 时使用：人物记忆说的是谁，可以写平台编号或名字。`add --class person` 时必须写 |
| `--format <text\|json>` | `list`、`search` 的输出格式 |

例如，记下一条带关键词的记忆：

```bash
miyu memory add --keywords 编辑器,vim 用户写代码用 Neovim
```

```powershell
miyu memory add --keywords '编辑器,vim' 用户写代码用 Neovim
```

退出码：`0` 表示成功；`1` 表示核心拒绝了请求；`2` 表示参数不对；`3` 表示 `dream` 被 `Ctrl+C` 停下。见 [记忆](memory.md)。

## miyu kb

```text
miyu kb [<子命令>] [<选项>]
```

`miyu kb` 用来管理知识库。不写子命令时运行 `list`。删除的文档和知识库会移到系统的回收站。选项写在子命令的前面或后面都可以。

| 子命令 | 做什么 |
|---|---|
| `list [<库>]` | 列出知识库；写了库时列出库里的文档 |
| `search <词>…` | 搜索知识库 |
| `read <库> <文档>` | 读取一篇文档中的一页 |
| `new <库>` | 新建一个知识库 |
| `edit <库>` | 修改名字、说明，开启或关闭自动想起 |
| `add <库> <文件>…` | 把文件加进知识库，每个文件最大 20 MiB |
| `remove <库> <文档>` | 删除一篇文档 |
| `mount <库> <目录>` | 把现有的目录挂载成只读的知识库 |
| `delete <库>` | 删除整个知识库；对于挂载进来的知识库，只取消登记，不改动目录 |
| `reindex <库>` | 重建索引 |

| 选项 | 做什么 |
|---|---|
| `--kb <库>` | `search` 时只搜索这一个知识库 |
| `--page <页>` | `read` 时读取第几页，不写时读第 1 页 |
| `--name <名字>` | `new`、`edit`、`mount` 时使用：显示的名字 |
| `--description <说明>` | `new`、`edit`、`mount` 时使用：一句话的说明 |
| `--auto` | `new`、`edit`、`mount` 时使用：开启自动想起，对话提到库里的某一段时，自动带上这一段的开头 |
| `--no-auto` | `edit` 时使用：关闭自动想起 |
| `--to <目录>` | `add` 时放进知识库里的这个目录 |
| `--replace` | `add` 时替换掉已有的同名文档 |
| `--format <text\|json>` | `list`、`search` 的输出格式 |

例如，把一个现有的目录挂载成知识库：

```sh
miyu kb mount handbook ~/Documents/handbook --name "团队手册" --auto
```

`<库>` 是知识库的编号：以小写字母开头，只包含小写字母、数字、`-` 和 `_`，最长 64 个字符，并且不能是 `con`、`nul`、`com1` 这类 Windows 保留名。写法不对时会报错 `知识库编号无效`。

退出码：`0` 表示成功；`1` 表示核心拒绝了请求；`2` 表示参数不对。见 [知识库](knowledge.md)。

## miyu schedule

```text
miyu schedule [<子命令>] [<选项>]
```

`miyu schedule` 用来管理定时任务。不写子命令时运行 `list`。到了设定的时间，定时任务会把一句话交给会话运行一轮；核心停止期间错过的任务不会补上。

| 子命令 | 做什么 |
|---|---|
| `list [--session <编号>]` | 列出定时任务；写了会话时只列出在这个会话上运行的任务 |
| `add <话>…` | 添加一个定时任务。时间用 `--cron`、`--at`、`--every` 其中之一指定 |
| `rm <编号>` | 删除定时任务 |
| `pause <编号>` | 暂停定时任务 |
| `resume <编号>` | 恢复定时任务，下一次运行的时间从现在开始重新计算 |

任务的编号是 `list` 输出的每一行开头的那一格，例如 `t1`。

`add` 的选项如下：

| 选项 | 做什么 |
|---|---|
| `--cron <五段>` | 按照 cron 表达式运行，例如 `'0 8 * * *'` |
| `--once` | cron 只运行一次 |
| `--zone <时区>` | cron 使用的时区，例如 `Asia/Tokyo`，不写时使用本机时区 |
| `--at <时刻>` | 在这个时刻运行一次：`'2026-10-28 08:00'`（本机时区）或 `2026-10-28T08:00:00+09:00` |
| `--every <间隔>` | 每隔一段时间运行一次：写秒数，或者 `90s`、`10m`、`2h`、`1d`，间隔至少 60 秒 |
| `--session <编号>` | 在这个已有的会话上运行。不能和下面四个选项一起使用 |
| `--persona <人格>` | 每次新开一个会话，使用这个人格 |
| `--no-persona` | 每次新开一个没有人格的会话 |
| `--preset <预设>` | 新开的会话使用这个预设 |
| `--cwd <目录>` | 新开的会话的工作目录，不写时是每个新会话自己的默认工作区 `~/.miyu/root/workspace/<会话编号>/` |

`--once`、`--zone` 只能和 `--cron` 一起使用。`list` 还接受 `--format <text|json>`。

例如，每天早上 8 点让 AI 检查一次 CI：

```sh
miyu schedule add --cron '0 8 * * *' --zone Asia/Shanghai 看一眼昨晚的 CI，有失败的列出来
```

退出码：`0` 表示成功；`1` 表示核心拒绝了请求；`2` 表示参数不对。见 [让 AI 自己接着干](automation.md)。

## miyu service

```text
miyu service <enable|disable|status> [--format <text|json>]
```

`miyu service` 用来让核心在开机或登录时自动启动：

| 子命令 | 做什么 |
|---|---|
| `enable` | 写好服务文件，交给系统的服务管理器，之后开机或登录时就会启动核心 |
| `disable` | 停止服务，从服务管理器里移除，并删除服务文件 |
| `status` | 查看服务有没有安装、有没有开启 |

Linux 上使用 systemd 用户服务，没有时使用桌面自启动；macOS 上使用 launchd；Windows 上使用登录时触发的任务计划。服务只对你自己这个用户有效。在 Windows 上，如果设置了 `MIYU_HOME`，就无法安装服务，会报错 `无法安装服务：Windows 任务计划无法携带 MIYU_HOME`。

例如：

```sh
miyu service enable
```

退出码：成功时是 `0`，服务本来就没有安装时、以及 `status` 也是 `0`；其他情况是 `1`。见 [常驻和服务器](service.md)。

## miyu upgrade

```text
miyu upgrade [--prerelease] [--yes]
```

`miyu upgrade` 用来升级 Miyu：

| 参数 | 做什么 |
|---|---|
| `--prerelease` | 把预发布版也算进来 |
| `--yes` | 不询问，直接升级 |

如果是用安装脚本安装的，命令会下载新版，核对签名和校验和之后替换旧版；如果是从 AUR、Homebrew 安装的，命令会告诉你应该使用的包管理器命令；如果是手动解压压缩包安装的，命令会告诉你下载地址。升级完成以后，已经在运行的核心仍然是旧版，需要运行 `miyu core stop`，下次打开时就是新版。在 Windows 上，正在运行的 `miyu.exe` 删不掉，会留在安装目录的 `.miyu-old-<旧版本>/` 里，在下一次运行 `miyu upgrade` 的开头删除。如果你的系统没有发行包（例如 Intel 芯片的 Mac、ARM 的 Windows），会报错 `无法升级：没有这种系统的发行包`，并提示有哪几种系统的发行包，以及其他系统需要到发行页下载源码自己编译。

例如：

```sh
miyu upgrade --yes
```

退出码：升级完成、已经是最新版、需要使用包管理器升级、只给出了下载地址，这几种情况都是 `0`；其他情况是 `1`。见 [安装、升级、卸载](install.md#升级)。

## miyu core

```text
miyu core start
miyu core status [--format <text|json>]
miyu core stop
```

`miyu core` 用来管理在后台运行的核心。平时界面和命令会在需要时自己启动核心，不需要你手动管理；如果想提前启动核心、查看它是否在运行，或者停止它，就使用这三条命令。

- `start`：如果核心没有在运行，就启动它，等核心准备好之后再返回，成功时不输出任何内容；如果核心已经在运行，不做任何事。这条命令只启动核心，不打开界面，也不打开浏览器。如果核心启动失败，会说明原因，退出码是 `1`。
- `status`：只查看，不启动核心。核心在运行时输出三行：状态、版本（核心自己报告的版本：升级以后还没有重启时，和 `miyu --version` 不一样）、数据根；核心没有在运行时，输出状态、数据根和一行提示。
- `stop`：让正在运行的核心停止，等它退出之后再返回，成功时不输出任何内容。如果核心没有在运行，不做任何事，退出码是 `0`。如果请求被拒绝、连接不上，或者核心在 60 秒内没有退出，退出码是 `1`。

例如，启动核心并查看它的状态：

```sh
miyu core start
miyu core status
```

核心在运行时，`status` 的输出类似这样：

```text
核心正在运行
版本：2026.11.0
数据根：/home/alice/.miyu
```

`status` 的退出码：`0` 表示在运行，`3` 表示没有在运行，`1` 表示出错（例如核心还在运行却连接不上）。加上 `--format json` 会输出一行 JSON 给脚本使用，退出码相同，字段如下：

| 字段 | 是什么 |
|---|---|
| `running` | 是否在运行 |
| `version` | 核心的版本；没有在运行时是 `null` |
| `data_root` | 数据根的绝对路径 |

## miyu check

```text
miyu check [<文件>] [--format <text|json>]
```

`miyu check` 检查你手写的、Miyu 要读取的文件有没有写错。不写文件时检查全部，写了文件时只检查这一个文件。

例如：

```sh
miyu check ~/.miyu/root/personas/miyu/prompts/examples.md
```

检查哪些文件、怎么阅读结果，见 [出了问题怎么查](troubleshooting.md#先运行-miyu-check)。有错误时退出码是 `1`，只有警告时是 `0`。

## miyu rescue

```text
miyu rescue [<参数>…]
```

`miyu rescue` 在核心出了问题时，用 LiteMiyu 的应急模式来修复。它不连接核心，而是根据 `models.chat` 填好 `LITEMIYU_RESCUE_*` 环境变量，运行 `litemiyu --rescue`，并把后面的参数原样交给它；已经设置了的变量不会被改动。它依次在 `miyu` 所在的目录、`miyu` 实际所在的目录、`PATH` 里寻找 `litemiyu`。交给 `litemiyu` 的方式和 [软件包加的命令](#软件包加的命令) 相同。

例如：

```sh
miyu rescue
```

LiteMiyu 只有 Linux 版。在 macOS、Windows 上找不到它，会输出 `未找到 litemiyu`，并提示 `目前只支持 Linux`。

退出码使用 `litemiyu` 的退出码；找不到 `litemiyu` 时是 `1`。见 [应急修复](rescue.md)。

## miyu call

```text
miyu call <工具> --<参数> <值>…
miyu call <工具> --json '<对象>'
miyu call <工具> --json -
miyu call --schema <工具>…
```

`miyu call` 在 AI 运行的命令里调用这个会话的工具，供 AI 写的脚本使用。它只能在 Miyu 运行的命令里、并且开启了 `命令行调用工具` 的会话里使用；如果在你自己的 shell 里运行，退出码是 `2`。

- `--<参数> <值>`：值以文字形式发送，再按照工具的参数格式转换成数字或真假值；同一个参数写多次，就是一个列表。
- `--json`：传入一个 JSON 对象，写 `-` 时从标准输入读取。具名的参数会覆盖对象里同名的参数。Windows PowerShell 5.1 会吃掉参数里的双引号，因此需要改用管道。
- `--schema <工具>…`：输出这几件工具的参数。

有结构化结果的工具会输出一行 JSON，其他工具输出文字。AI 运行的命令在 Linux 上是 bash，在 macOS 上是 zsh，在 Windows 上是 PowerShell（见 [AI 能用的工具](tools.md#shell)），脚本要按照对应的 shell 来写：

```bash
miyu call web_search --query niri | jq -r '.links[].site' | sort | uniq -c
```

```powershell
(miyu call web_search --query niri | ConvertFrom-Json).links.site
'{"query": "niri"}' | miyu call web_search --json -
```

退出码：`0` 表示成功；`1` 表示工具出错或者被拒绝；`2` 表示用法不对，或者不在 Miyu 运行的命令里；`3` 表示被打断，或者它所属的命令已经结束；`4` 表示需要确认，此时要改成直接调用这件工具。

## miyu tui

```text
miyu tui [--resume <编号> | --page <页> | config]
```

`miyu tui` 用来打开终端界面。

| 参数 | 做什么 |
|---|---|
| 不带参数 | 按照 `启动时打开`（`ui.startup`）的设置，开始新会话或者接着最近的会话 |
| `--resume <编号>` | 打开这个会话，必须写完整的编号；如果打不开，会报错并退出 |
| `--page config`、`config` | 只打开设置页，不进入会话；在设置页的菜单上按 `Esc` 退出 |
| `--page oobe` | 重新走一遍第一次打开时的引导 |
| `-h`、`--help` | 输出帮助 |

如果写了认不出的页名，会照常打开首页。设置环境变量 `MIYU_TUI_IME=0` 可以让终端界面不切换输入法（终端界面只在你使用 fcitx5 输入法时才会切换）。

例如：

```sh
miyu tui --resume 0198c2a4-7e1b-7c3d-9a2f-4b5c6d7e8f90
```

详细的见 [终端界面](heads/tui.md)。

## miyu shellhook

```text
miyu shellhook <子命令> [<选项>]
```

`miyu shellhook` 把 Miyu 装进 shell。装进 shell 以后，在提示符后面输入一句话、按回车，就是在和 AI 对话。

| 子命令 | 做什么 |
|---|---|
| `install [--shell <fish\|bash\|zsh\|pwsh\|powershell>] [--program <路径>]` | 装进 shell。`pwsh` 是 PowerShell 7，`powershell` 是 Windows 自带的 Windows PowerShell 5.1。不写 `--shell` 时会自动判断：Linux 上看上一层进程，认不出时看 `SHELL`；macOS 上看 `SHELL`；Windows 上看 `SHELL`，没有时根据 `PSModulePath` 区分 PowerShell 7 和 5.1。如果这种 shell 里还有旧版（0.7.x）装的终端集成，会先备份再清理，见 [终端界面](heads/tui.md#清理旧版的终端集成) |
| `uninstall [--shell <fish\|bash\|zsh\|pwsh\|powershell>]` | 从 shell 里移除 |
| `status` | 查看装进了哪几个 shell，以及旧版的终端集成还在不在 |
| `doctor` | 检查有没有装好、能不能连接核心、终端的颜色和剪贴板 |

`--program <路径>` 是 shell 里调用的程序，不写时是这个程序旁边的 `miyu`。

例如：

```sh
miyu shellhook install --shell zsh
```

退出码：参数不对时是 `2`；`doctor` 有一项检查不通过时是 `1`。见 [终端界面](heads/tui.md)。

## miyu web

```text
miyu web [<选项>]
miyu web stop
miyu web status
```

`miyu web` 用来打开网页界面：如果网页界面没有在运行，会先在后台启动它，再交给浏览器打开；命令会马上返回，不占用终端。关掉浏览器以后，网页界面仍然在运行。

| 参数 | 做什么 |
|---|---|
| `--port <端口>` | 这一次监听哪个端口（只在网页界面没有在运行时有效），出厂设置是 `8300` |
| `--print` | 不打开浏览器，只输出地址 |
| `--format <text\|json>` | 输出的格式；`json` 也不打开浏览器 |
| `--reset` | 带上一次性码打开，用来重新设置访问密码 |
| `--logout` | 让所有浏览器都需要重新登录 |
| `--package <编号>` | 直接打开 `软件后台` 里这个软件包的页面 |
| `--session <编号>` | 直接打开这个会话 |
| `--settings [<分页>]` | 直接打开设置；写 `access` 时打开 `通用` 里的 `访问` |
| `-h`、`--help` | 输出帮助 |

- `stop`：停止网页界面，核心照常运行。文字输出在标准错误上：停止成功时输出 `已停止网页界面`，本来就没有在运行时输出 `网页界面没有在运行`，这两种情况的退出码都是 `0`；没有停止成功时输出 `miyu: 无法停止网页界面：<原因>`，退出码是 `1`。
- `status`：查看网页界面是否在运行，文字输出在标准输出上：在运行时输出 `网页界面正在运行：<地址>`，退出码是 `0`；没有在运行时输出 `网页界面没有在运行`，退出码是 `3`（和 `miyu core status` 使用同一套退出码）。

这两条命令不连接核心，核心没有在运行时也不会被启动；它们后面不能再写其他参数。

例如：

```sh
miyu web --settings access
miyu web status
```

参数不对时退出码是 `2`。见 [网页界面](heads/web.md)、[常驻和服务器](service.md#从其他电脑和手机打开网页界面)。
