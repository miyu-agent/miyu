# 命令行全表

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

`miyu` 的每个子命令、参数、输出和退出码都在这一页，查的时候用。怎么用讲得更细的，各节末尾链到讲透它的那一页。

## 总表

| 用途 | 命令 | 做什么 |
|---|---|---|
| 对话 | [`ask`](#miyu-ask) | 说一句话，印出回答 |
| | [`undo`](#miyu-undo)（别名 `rewind`） | 撤掉最后一轮 |
| | [`restore`](#miyu-restore) | 恢复刚撤掉的那一轮 |
| | [`redo`](#miyu-redo) | 撤掉最后一轮，原话或换一句话重来 |
| | [`compact`](#miyu-compact) | 把上下文压成摘要 |
| | [`recap`](#miyu-recap) | 让 AI 写一段回顾 |
| 会话 | [`rename`](#miyu-rename) | 给会话改名 |
| | [`session`](#miyu-session) | 导出、导入会话 |
| 模型和配置 | [`setup`](#miyu-setup) | 接上第一个模型 |
| | [`provider`](#miyu-provider) | 存、列、删供应商的 key |
| | [`config`](#miyu-config) | 看、改配置，信任项目配置 |
| 软件包 | [`pkg`](#miyu-pkg) | 装、卸、查软件包 |
| | [`mcp`](#miyu-mcp) | 登录、登出要浏览器授权的 MCP 服务 |
| | [`bus`](#miyu-bus) | 看总线上哪个软件包提供了什么 |
| | [`dev`](#miyu-dev) | 给写软件包的人用 |
| 记忆和知识库 | [`memory`](#miyu-memory) | 看、搜、记、改、忘记忆 |
| | [`kb`](#miyu-kb) | 管知识库 |
| 定时 | [`schedule`](#miyu-schedule) | 管定时任务 |
| 服务和升级 | [`service`](#miyu-service) | 让核心开机或登录就起来 |
| | [`upgrade`](#miyu-upgrade) | 升级 Miyu |
| | [`core stop`](#miyu-core-stop) | 停掉在跑的核心 |
| 排错 | [`check`](#miyu-check) | 查手写的文件有没有写错 |
| | [`rescue`](#miyu-rescue) | 核心坏了时用 LiteMiyu 修 |
| 给脚本用的 | [`call`](#miyu-call) | 在 AI 跑的命令里调这个会话的工具 |

出厂就装着的软件包另带三个命令：

| 命令 | 哪个软件包 | 做什么 |
|---|---|---|
| [`tui`](#miyu-tui) | 终端（`tui`） | 打开终端界面 |
| [`shellhook`](#miyu-shellhook) | 终端（`tui`） | 在 shell 的提示符后面直接和 AI 说话 |
| [`web`](#miyu-web) | 网页（`webui`） | 打开网页界面 |

单独装的软件包带的命令，装了才有：

| 命令 | 做什么 | 说明 |
|---|---|---|
| `im` | 开、关、查看接入通讯平台 | 装了「接入通讯平台」才有，见 [接入通讯平台](heads/im.md) |
| `migrate` | 把旧版 Miyu 的数据目录迁进这一版 | 装了「从旧版 Miyu 迁过来」才有，见 [从 0.7 旧版迁过来](migrate.md) |
| `ocr` | 提取图片里的文字 | 装了「OCR文字识别」才有，见 [附加软件](addons.md) |

## 不带子命令

在终端里直接敲 `miyu`，打开设置里「默认界面」（`ui.head`）指的那个界面，出厂是终端。核心没在跑的先拉起来。标准输入、标准输出有一个不是终端的（脚本、管道里），只印帮助，退出码 `2`。

`ui.head` 指的界面没装时报「界面 'tui' 没有安装」，提示用 `miyu pkg install tui` 装回来，或者用 `miyu config set ui.head <编号>` 换成装了的。

## 软件包加的命令

`miyu <名字>` 不是内置的子命令时，Miyu 在装了的软件包里找带这个命令的，把后面的参数原样交给它的程序，退出码照它的。Linux、macOS 上 `miyu` 换成那个程序；Windows 上 `miyu` 起它、等它退出，这期间按 `Ctrl+C` 交给那个程序处理，`miyu` 自己不理。和内置的撞名的，内置的优先。`miyu -h` 里多一节「软件包加的命令：」，列出装了的。`miyu help <名字>` 印的是那个程序自己的帮助。

敲了没装的命令，报「未知命令 '<名字>'」。本机存着的软件源索引里有带它的软件包的，提示「软件包 <编号> 提供这个命令，用 'miyu pkg install <编号>' 安装」。

## 通用的

### 帮助和版本

| 写法 | 做什么 |
|---|---|
| `miyu -h`、`miyu --help` | 印总的帮助 |
| `miyu <命令> -h`、`miyu help <命令>` | 印这个命令的帮助。有子命令的，帮助是一整页，子命令都写在里面 |
| `miyu -V`、`miyu --version` | 印版本，例如 `miyu 2026.11.0` |

帮助、版本印在标准输出上，退出码 `0`。

### 输出格式

多数看东西的命令有 `--format <text|json>`，出厂 `text` 给人看，`json` 印一行 JSON 给脚本用：`bus`、`check`、`config get`、`config explain`、`kb`、`memory`、`pkg`、`provider list`、`schedule`、`service`。`ask` 多一种 `jsonl`，见 [`miyu ask`](#miyu-ask)。

标准输出是终端、没设环境变量 `NO_COLOR` 时才上色。

### 界面语言

命令行只有中文、英文两种。连上核心以前，照环境变量 `LC_ALL`、`LC_MESSAGES`、`LANG` 里第一个设了的；都没设的照系统设置（macOS 的首选语言、Windows 的界面语言）。`zh` 开头的用中文，别的用英文。帮助页、参数写错的报错都照它。

连上核心以后，照设置页「通用 → 显示 → 界面语言」（`ui.language`）：写了 `zh` 用中文，`en`、`ja` 用英文；`auto`（出厂）照上面的系统语言。

### 报错

报错印在标准错误上，`miyu: ` 打头，能照着做的另起一行「提示：」：

```text
miyu: 'kb' 没有子命令 'ls'
提示：运行 'miyu kb --help' 查看可用命令
```

### 退出码

| 退出码 | 意思 | 哪些命令会出 |
|---|---|---|
| `0` | 做成了 | 全部 |
| `1` | 出错：连不上核心、核心拒绝了、模型或工具出错 | 全部 |
| `2` | 参数不对，什么都没做 | 全部 |
| `3` | 被打断（`Ctrl+C`）、到了 `--timeout` | `ask`、`redo`、`compact`、`memory dream`、`call` |
| `4` | 这一轮结束了，但有几步要人确认，没人能确认，没有做 | `ask`、`redo`、`call` |
| `5` | 没有可用的模型：没配，或者都在冷却 | `ask`、`redo`、`compact`、`recap` |
| `130` | 贴 key 时取消了：按了 `Ctrl+C`；Linux、macOS 上一个字都没敲时按 `Ctrl+D` 也算 | `setup`、`provider login` |

`miyu ask` 里没人能当场确认，要确认的那几步一律不做，退出码 `4`。这类活到终端或网页里做，见 [权限和沙盒](permissions.md)。

### 从标准输入读的

| 命令 | 读什么 |
|---|---|
| `miyu ask` | 不写话的，读整段当话；话里有一个词正好是 `-` 的，读进来接在后面 |
| `miyu setup --provider <编号>` | 不在终端里时，从管道读 key |
| `miyu provider login` | 从管道读 key |
| `miyu call <工具> --json -` | 从管道读参数的 JSON |
| `miyu pkg`、`miyu upgrade` | 要确认时读一行回答。标准输入关着的不做，加 `--yes` |

## miyu ask

```text
miyu ask [<选项>] [<话>]
```

说一句话，印出回答。不写 `-c`、`-s` 的新开一个会话。AI 派出了子代理的，等它们都回报、回答结束才退出。

| 参数 | 做什么 |
|---|---|
| `-c`、`--continue` | 接着上一次 `miyu ask` 开的会话说。不能和 `-s` 一起写 |
| `-s`、`--session <编号>` | 接着这个会话说 |
| `-C`、`--cd <目录>` | 在这个目录里干活，目录要已经有。只管新会话 |
| `--persona <人格>` | 用这个人格。只管新会话，不能和 `--no-persona` 一起写 |
| `--no-persona` | 不用人格。只管新会话 |
| `--preset <预设>` | 用这个预设。只管新会话 |
| `--permission <read-only\|full>` | 开局的权限级别：只读或开放权限。只管新会话 |
| `--format <text\|json\|jsonl>` | `text`（出厂）边收边打；`json` 最后一行印一个 JSON；`jsonl` 一行一个事件 |
| `--add-dir <目录>` | 把另一个目录也算进工作区，能读能写，可以写好几次 |
| `--file <文件>` | 附上一个文件（图片、PDF、文本），可以写好几次 |
| `--timeout <时长>` | 最多等多久，例如 `30s`、`10m`、`1h`，不写单位是秒 |
| `--from <名字>` | 别的 agent 调用时报自己的名字，例如 `claude-code` |
| `--model <模型>` | 用这个模型或 `@池`。接着说的会话从此换成它 |
| `--no-memory` | 不召回也不保存记忆。只管新会话 |

「只管新会话」的几个不能和 `-c`、`-s` 一起写。

```bash
git diff | miyu ask --format jsonl "审查这些改动" -
```

```powershell
git diff --output=改动.diff
miyu ask --format jsonl --file 改动.diff "审查这些改动"
```

退出码：`0` 到 `5` 全会出，见上面的退出码表。详细见 [会话](sessions.md)。

## miyu undo

```text
miyu undo [-s <编号>]
```

撤掉会话的最后一轮，只退对话，不把文件改回去。也能写成 `miyu rewind`。

| 参数 | 做什么 |
|---|---|
| `-s`、`--session <编号>` | 哪个会话。不写的是上一次 `miyu ask` 开的那个 |

```sh
miyu undo
```

被拒绝（例如「没有可撤销的回合」）的退出码 `1`。见 [会话](sessions.md#撤销恢复重做)。

## miyu restore

```text
miyu restore [-s <编号>]
```

恢复刚撤掉的那一轮，要在发下一句之前。参数同 `miyu undo`。

```sh
miyu restore
```

没得恢复的报「没有可恢复的撤销」，退出码 `1`。

## miyu redo

```text
miyu redo [-s <编号>] [<话>]
```

撤掉最后一轮，把你那句话再发一次；写了话的换成新的话。参数同 `miyu undo`，回答照 `miyu ask` 的样子印。

```sh
miyu redo 只改 README，别动代码
```

退出码同 `miyu ask`。最后一轮不是你的话开的，报「无法重做」，退出码 `1`。

## miyu compact

```text
miyu compact [-s <编号>] [<要求>]
```

把会话的上下文压成摘要，最近的一段保留原文。后面写的话交给摘要，说要留住什么。参数同 `miyu undo`。

```sh
miyu compact 保留数据库迁移那部分的细节
```

退出码：`0` 压好了；`1` 被拒绝或压缩失败；`3` 被打断；`5` 没有可用的模型。见 [会话](sessions.md#上下文满了)。

## miyu recap

```text
miyu recap [-s <编号>]
```

让 AI 写一段回顾：在做什么、做完了什么、卡在哪。只把这一段印到标准输出，能接管道。参数同 `miyu undo`。

```sh
miyu recap -s 0198c2a4-7e1b-7c3d-9a2f-4b5c6d7e8f90
```

退出码：`0` 写好了；`1` 被拒绝（没什么可回顾的、回顾没写成、没有这个会话）；`5` 没有可用的模型。

## miyu rename

```text
miyu rename [-s <编号>] [<标题>]
```

给会话改名。几个词用空格连起来；不写标题的，让 AI 照对话重起一个。改好了什么都不印。参数同 `miyu undo`。

```sh
miyu rename 迁移数据库
```

被拒绝（标题是空白、太长，没有这个会话）的退出码 `1`。

## miyu session

```text
miyu session export [<编号>] [-o <文件>] [--md [--for <用途>] [--last <轮数>]]
miyu session import <文件> [--archive]
```

| 子命令、参数 | 做什么 |
|---|---|
| `export [<编号>]` | 导出会话的原始包（ZIP）。不写编号的是上一次 `miyu ask` 的会话。写好了印出写到哪 |
| `-o`、`--output <文件>` | 写到这个文件。不写的是当前目录下的 `miyu-session-<编号前 8 位>.zip`，Markdown 是 `.md`。Markdown 写 `-` 打到标准输出 |
| `--md` | 导成 Markdown |
| `--for <read\|agent\|chat>` | Markdown 给谁看：`read` 自己看（出厂）、`agent` 给别的 AI、`chat` 贴进网页版聊天。要和 `--md` 一起写 |
| `--last <轮数>` | Markdown 只要最近几轮，至少 1。要和 `--md` 一起写 |
| `import <文件>` | 导入原始包，一行一个印出新导进来的会话编号 |
| `--archive` | 导成只读存档：能看、能搜，不能接着聊 |

```sh
miyu session export --md --for agent --last 20 -o -
```

被拒绝（文件已存在、要写进数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）、不是会话包）的退出码 `1`。见 [会话](sessions.md#导出导入)。

## miyu setup

```text
miyu setup [--provider <编号>] [--env <变量>] [--model <模型>]
```

接上第一个模型：选一家供应商，贴 key，试通，选模型，写进系统配置。

| 参数 | 做什么 |
|---|---|
| `--provider <编号>` | 用模型目录里的这一家，不再选 |
| `--env <变量>` | 照核心环境里的这个变量取 key，不再贴 |
| `--model <模型>` | 用这个模型，不再选 |

不在终端里时必须写 `--provider`，key 从管道读：

```bash
echo "$KEY" | miyu setup --provider deepseek
```

```powershell
$key | miyu setup --provider deepseek
```

在终端里贴 key 时不显示，按 `Ctrl+C` 取消；Linux、macOS 上一个字都没敲时按 `Ctrl+D` 也是取消。

退出码：`0` 写好了；`1` 没选、没收到 key、试不通、核心拒绝了；`2` 参数不对，或者不在终端里又没写 `--provider`；`130` 贴 key 时取消了。见 [模型](models.md#最快miyu-setup)。

## miyu provider

```text
miyu provider login [<名字>]
miyu provider list [--format <text|json>]
miyu provider logout [<名字>]
```

| 子命令 | 做什么 |
|---|---|
| `login [<名字>]` | 存一个 key。不写名字的，在终端里从配置用到的、存过的里面选。贴的时候不显示，也能从管道读 |
| `list` | 列出存了的，和配置里用到了、还没存的（灰字）。不显示 key |
| `logout [<名字>]` | 删一个。不写名字的，在终端里从存过的里面选 |

```bash
echo "$KEY" | miyu provider login acme
```

```powershell
$key | miyu provider login acme
```

贴 key 怎么取消同 `miyu setup`。

退出码：`0` 成了；`1` 核心拒绝了、没收到 key、没选、要删的没存过；`2` 名字不合写法，或者不写名字又不在终端里；`130` 贴 key 时取消了。见 [模型](models.md#key-放在哪)。

## miyu config

```text
miyu config <子命令> [<选项>]
```

| 子命令、参数 | 做什么 |
|---|---|
| `get [<键>…] [--format <text\|json>]` | 印生效的值。不写键的印全部；只给一个键的只印值 |
| `set <键> <值>` | 改一项，写进系统配置 `~/.miyu/etc/config.toml`。写 `--project` 的不收：项目配置只能手改 |
| `unset <键>` | 从系统配置里删掉一项，回到默认值 |
| `edit [--project]` | 用编辑器打开系统配置（写 `--project` 打开当前目录的项目配置），存之前先检查。编辑器照 `VISUAL`、`EDITOR`，都没有的 Linux、macOS 上是 `vi`，Windows 上是 `notepad` |
| `explain <键> [--format <text\|json>]` | 印这一项在每一层写的什么、哪一层生效 |
| `path [--project]` | 印系统配置的路径；写 `--project` 印当前目录的项目配置的路径 |
| `trust [--yes \| --no]` | 看当前目录的项目配置会改什么，决定信不信任。`--yes` 信任、`--no` 不信任，都不问 |

在终端里只敲 `miyu config`，打开默认界面的设置页；界面没有设置页的、不在终端里的，印帮助，退出码 `2`。

```sh
miyu config explain models.chat
```

列表值照 TOML 的数组写。bash 里整个用单引号括起来；PowerShell 里外面用双引号、里面用 TOML 的单引号字符串（Windows PowerShell 5.1 会吃掉参数里的双引号）：

```bash
miyu config set skills.external.dirs '["~/.agents/skills", "~/my-skills"]'
```

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/my-skills']"
```

退出码：`0` 成了；`1` 核心拒绝了（键不存在、值不对、文件有误）；`2` 参数不对，或者不在终端里敲 `edit`、不带 `--yes`、`--no` 的 `trust`。见 [配置](config.md)。

## miyu pkg

```text
miyu pkg [<子命令>] [<选项>]
```

不写子命令的是 `list`。

| 子命令 | pacman 写法 | 做什么 |
|---|---|---|
| `list` | `-Q` | 列出装了的软件包 |
| `install <软件包>` | `-U` | 装。`<软件包>` 是含 `package.toml` 的目录、`.miyu` 包文件，或者编号（从软件源装，或装回卸掉的出厂软件包） |
| `remove <编号>` | `-R` | 卸掉，连同它的设置和状态，配置留着 |
| `info <软件包>` | | 看一个软件包的信息。没装的查软件源；写目录或 `.miyu` 文件的只看不装 |
| | `-Qi <编号>` | 只查装了的 |
| | `-Si <编号>` | 只查软件源 |
| `files <编号>` | `-Ql` | 列出它装了哪些文件 |
| `owns <路径>` | `-Qo` | 查一个文件属于哪个软件包 |
| `check [<编号>]` | `-Qk` | 查装好的文件有没有被改过、少了 |
| `page <编号>` | | 用浏览器打开它的后台页 |
| `pack <目录>` | | 把软件包目录打成 `.miyu` 文件，不连核心 |
| `update` | `-Sy` | 从软件源拿新的索引 |
| `search [<词>…]` | `-Ss` | 在软件源里搜，几个词都要有；不写词的列出全部 |
| `upgrade` | `-Su` | 照软件源升级装了的软件包。`-Syu` 先拿索引再升 |
| `clean` | `-Sc` | 清掉缓存里不是装着那一版的包文件 |
| | `-S <编号>…` | 从软件源装这几个 |

| 选项 | 做什么 |
|---|---|
| `--format <text\|json>` | 输出格式 |
| `--yes`、`--noconfirm` | 装、升级、卸载前不问，要的能力一并批准 |
| `-n`、`--purge` | `remove` 时连配置目录一起删 |
| `--target <系统>` | `pack` 打给哪种系统；清单写了 `targets` 的必须挑一种 |
| `-o`、`--out <目录>` | `pack` 写到哪个目录，不写是当前目录 |
| `--print` | `page` 只印网址，不开浏览器 |

```sh
miyu pkg search ocr
miyu pkg install ocr
```

退出码：`0` 成了；`1` 核心拒绝了，或者 `search` 一个都没搜到、`owns` 哪个包都不认、`check` 查出改过少了的；`2` 参数不对。见 [软件包](packages.md)。

## miyu mcp

```text
miyu mcp login <服务>
miyu mcp logout <服务>
```

| 子命令 | 做什么 |
|---|---|
| `login <服务>` | 登录只收浏览器授权的网址 MCP 服务。印出授权页地址并试着打开浏览器；同意以后令牌存进密钥文件，服务自动连上。浏览器在另一台机器上的，授权后把地址栏里的完整地址贴回来回车。最多等 10 分钟 |
| `logout <服务>` | 删掉存着的登录 |

`<服务>` 是配置或软件包里这个 MCP 服务的名字。

```sh
miyu mcp login acme-docs
```

退出码：`0` 连上了、登出了；`1` 核心拒绝了、等到时还没连上、连不上核心。见 [扩展 Miyu](extending.md)。

## miyu bus

```text
miyu bus [list [<名字>]] [--format <text|json>]
```

不写子命令的是 `list`。软件包在总线上照名字提供功能，彼此不写死包名；`list` 列出每个名字由谁提供、能不能用，写了名字的只列它。

```sh
miyu bus list render
```

见 [软件包](packages.md)。

## miyu dev

```text
miyu dev driver-check <编号>
```

给写软件包的人用：查这个软件包提供的模型驱动编码稳不稳。会重启它的扩展，有会话正在用时不查。都过了的退出码 `0`，有不过的 `1`。见 [写一个 Miyu 软件包](../authors/README.md)。

## miyu memory

```text
miyu memory [<子命令>] [<选项>]
```

不写子命令的是 `list`。选项写在子命令前后都行。

| 子命令 | 做什么 |
|---|---|
| `list` | 列出记忆，新的在前 |
| `search <词>…` | 搜记忆 |
| `add <内容>…` | 记一条 |
| `edit <m序号> [<内容>…]` | 改一条，印出新的序号；只改 `--keywords`、`--who` 的不写内容 |
| `forget <m序号>` | 作废一条 |
| `clear session [<编号>]` | 清掉一个会话记下的。不写的照 `-s`，再没有的是上一次 `miyu ask` 的会话 |
| `clear me` | 清空这一份记忆：全部类别、认识的人的卡、对你的了解。不先问，找不回来 |
| `dream` | 现在就整理记忆。`Ctrl+C` 停下，整理好的留着 |
| `cards` | 列出人物卡 |

记忆写成 `m` 加序号，例如 `m12`，照 `list` 印的写。

| 选项 | 做什么 |
|---|---|
| `--persona <编号>` | 用这个人格的记忆，不写是默认人格。不能和 `-s` 一起写 |
| `-s`、`--session <编号>` | 用这个会话用的那一份记忆 |
| `--forgotten` | `list`、`search` 连作废的一起列 |
| `--class <user\|feedback\|episode\|reference\|person>` | `list` 只列这一类；`add` 记成这一类，不写是 `user` |
| `--why <原因>` | `forget` 写为什么忘 |
| `--keywords <词>` | `add`、`edit`：检索用的关键词，逗号隔开；`edit` 写空的是去掉 |
| `--who <是谁>` | `add`、`edit`：人物记忆说的是谁，平台编号或名字。`add --class person` 必须写 |
| `--format <text\|json>` | `list`、`search` 的输出格式 |

```bash
miyu memory add --keywords 编辑器,vim 用户写代码用 Neovim
```

```powershell
miyu memory add --keywords '编辑器,vim' 用户写代码用 Neovim
```

退出码：`0` 成了；`1` 核心拒绝了；`2` 参数不对；`3` `dream` 被 `Ctrl+C` 停下。见 [记忆](memory.md)。

## miyu kb

```text
miyu kb [<子命令>] [<选项>]
```

不写子命令的是 `list`。删掉的文档和知识库挪进系统的回收站。选项写在子命令前后都行。

| 子命令 | 做什么 |
|---|---|
| `list [<库>]` | 列出知识库；写了库的列出库里的文档 |
| `search <词>…` | 搜知识库 |
| `read <库> <文档>` | 读一篇文档的一页 |
| `new <库>` | 新建一个知识库 |
| `edit <库>` | 改名字、说明，开关自动想起 |
| `add <库> <文件>…` | 把文件加进库，每个最多 20 MiB |
| `remove <库> <文档>` | 删一篇 |
| `mount <库> <目录>` | 把现有的目录挂成只读的知识库 |
| `delete <库>` | 删整个库；挂进来的只取消登记，目录不动 |
| `reindex <库>` | 重建索引 |

| 选项 | 做什么 |
|---|---|
| `--kb <库>` | `search` 只搜这一个库 |
| `--page <页>` | `read` 读第几页，不写是第 1 页 |
| `--name <名字>` | `new`、`edit`、`mount`：显示的名字 |
| `--description <说明>` | `new`、`edit`、`mount`：一句话说明 |
| `--auto` | `new`、`edit`、`mount`：开自动想起，对话提到库里某一段时自动带上它的开头 |
| `--no-auto` | `edit`：关自动想起 |
| `--to <目录>` | `add` 放进库里的这个目录 |
| `--replace` | `add` 换掉已有的同名文档 |
| `--format <text\|json>` | `list`、`search` 的输出格式 |

```sh
miyu kb mount handbook ~/Documents/handbook --name "团队手册" --auto
```

`<库>` 是知识库的编号：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。写法不对的报「知识库编号无效」。

退出码：`0` 成了；`1` 核心拒绝了；`2` 参数不对。见 [知识库](knowledge.md)。

## miyu schedule

```text
miyu schedule [<子命令>] [<选项>]
```

不写子命令的是 `list`。到点把一句话交给会话跑一轮；核心停着时错过的不补。

| 子命令 | 做什么 |
|---|---|
| `list [--session <编号>]` | 列出定时任务；写了会话的只列跑在它上面的 |
| `add <话>…` | 加一个。时间用 `--cron`、`--at`、`--every` 之一 |
| `rm <编号>` | 删掉 |
| `pause <编号>` | 暂停 |
| `resume <编号>` | 恢复，从现在起重新算下一次 |

任务的编号是 `list` 每行开头那一格，例如 `t1`。

`add` 的选项：

| 选项 | 做什么 |
|---|---|
| `--cron <五段>` | 照 cron 表达式跑，例如 `'0 8 * * *'` |
| `--once` | cron 只跑一次 |
| `--zone <时区>` | cron 用的时区，例如 `Asia/Tokyo`，不写是本机时区 |
| `--at <时刻>` | 在这个时刻跑一次：`'2026-10-28 08:00'`（本机时区）或 `2026-10-28T08:00:00+09:00` |
| `--every <间隔>` | 每隔一段跑一次：秒数，或 `90s`、`10m`、`2h`、`1d`，至少 60 秒 |
| `--session <编号>` | 跑在这个已有的会话上。不能和下面四个一起写 |
| `--persona <人格>` | 每次新开会话，用这个人格 |
| `--no-persona` | 每次新开无人格的会话 |
| `--preset <预设>` | 新开的会话用这个预设 |
| `--cwd <目录>` | 新开的会话的工作目录，不写是每个新会话自己的默认工作区 `~/.miyu/root/workspace/<会话编号>/` |

`--once`、`--zone` 只配 `--cron`。`list` 另收 `--format <text|json>`。

```sh
miyu schedule add --cron '0 8 * * *' --zone Asia/Shanghai 看一眼昨晚的 CI，有失败的列出来
```

退出码：`0` 成了；`1` 核心拒绝了；`2` 参数不对。见 [让 AI 自己接着干](automation.md)。

## miyu service

```text
miyu service <enable|disable|status> [--format <text|json>]
```

| 子命令 | 做什么 |
|---|---|
| `enable` | 写好服务文件、交给系统的服务管理器，开机或登录就起核心 |
| `disable` | 停掉、从服务管理器里拿掉、删掉服务文件 |
| `status` | 看装没装、开没开着 |

Linux 用 systemd 用户服务，没有的用桌面自启动；macOS 用 launchd；Windows 用登录时的任务计划。只管你自己这个用户。Windows 上设了 `MIYU_HOME` 的装不了，报「无法安装服务：Windows 任务计划无法携带 MIYU_HOME」。

```sh
miyu service enable
```

退出码：`0` 成了，本来就没装的、`status` 也是 `0`；别的 `1`。见 [常驻和服务器](service.md)。

## miyu upgrade

```text
miyu upgrade [--prerelease] [--yes]
```

| 参数 | 做什么 |
|---|---|
| `--prerelease` | 预发布版也算 |
| `--yes` | 不问，直接升 |

安装脚本装的，下载新版、核对签名和校验和后替换；AUR、Homebrew 装的，说该用的包管理器命令；手工解开压缩包的，说下载地址。升完以后已经在跑的核心还是旧版，敲 `miyu core stop`，下次打开就是新版。Windows 上正在跑的 `miyu.exe` 删不掉，留在安装目录的 `.miyu-old-<旧版本>/` 里，下一次 `miyu upgrade` 开头再删。这种系统没有发行包的（Intel 芯片的 Mac、ARM 的 Windows 这类），报「无法升级：没有这种系统的发行包」。

```sh
miyu upgrade --yes
```

退出码：`0` 升好了、已是最新、该用包管理器、只说了下载地址；别的 `1`。见 [安装、升级、卸载](install.md#升级)。

## miyu core stop

```text
miyu core stop
```

叫在跑的核心停下，等它退了再回来，成了什么都不印。核心没在跑的什么都不做，退出码 `0`。被拒绝、连不上、60 秒内没退的退出码 `1`。

```sh
miyu core stop
```

## miyu check

```text
miyu check [<文件>] [--format <text|json>]
```

查手写的、Miyu 读的文件有没有写错。不写文件的查全部，写了的只查这一份。

```sh
miyu check ~/.miyu/root/personas/miyu/prompts/examples.md
```

查哪些、结果怎么读，见 [出了问题怎么查](troubleshooting.md#先跑-miyu-check)。有错误的退出码 `1`，只有警告的 `0`。

## miyu rescue

```text
miyu rescue [<参数>…]
```

核心出了问题时，用 LiteMiyu 的应急模式修。不连核心，照 `models.chat` 填好 `LITEMIYU_RESCUE_*` 环境变量，跑 `litemiyu --rescue`，后面的参数原样交给它；已经设了的变量不动。依次在 `miyu` 所在的目录、`miyu` 实际所在的目录、`PATH` 里找 `litemiyu`。交给 `litemiyu` 的办法同 [软件包加的命令](#软件包加的命令)。

```sh
miyu rescue
```

LiteMiyu 只有 Linux 的。macOS、Windows 上找不到它，印「未找到 litemiyu」，提示「暂只支持 Linux」。

退出码照 `litemiyu` 的；找不到 `litemiyu` 的 `1`。见 [应急修复](rescue.md)。

## miyu call

```text
miyu call <工具> --<参数> <值>…
miyu call <工具> --json '<对象>'
miyu call <工具> --json -
miyu call --schema <工具>…
```

在 AI 跑的命令里调这个会话的工具，给 AI 写的脚本用。只在 Miyu 跑的命令里、开着「命令行调用工具」的会话里能用；在你自己的 shell 里敲，退出码 `2`。

- `--<参数> <值>`：值当文字发，照工具的参数格式换成数字或真假；同一个参数写几次是列表。
- `--json`：传一个 JSON 对象，写 `-` 从标准输入读。具名的参数压过对象里同名的。Windows PowerShell 5.1 会吃掉参数里的双引号，改用管道。
- `--schema <工具>…`：印这几件工具的参数。

有结构化结果的工具印一行 JSON，别的印文字。AI 跑的命令在 Linux 上是 bash、macOS 上是 zsh、Windows 上是 PowerShell（见 [AI 能用的工具](tools.md#shell)），脚本照那个 shell 写：

```bash
miyu call web_search --query niri | jq -r '.links[].site' | sort | uniq -c
```

```powershell
(miyu call web_search --query niri | ConvertFrom-Json).links.site
'{"query": "niri"}' | miyu call web_search --json -
```

退出码：`0` 成了；`1` 工具出错或被拒绝；`2` 用法不对、不在 Miyu 跑的命令里；`3` 被打断，或者它所属的命令已经结束；`4` 要确认，改成直接调这件工具。

## miyu tui

```text
miyu tui [--resume <编号> | --page <页> | config]
```

打开终端界面。

| 参数 | 做什么 |
|---|---|
| 不带 | 照「启动时打开」（`ui.startup`）开新会话或接着最近的 |
| `--resume <编号>` | 打开这个会话，要写完整的编号；打不开就报错退出 |
| `--page config`、`config` | 只打开设置页，不进会话；在设置页的菜单上按 `Esc` 退出 |
| `--page oobe` | 重走一遍第一次打开的引导 |
| `-h`、`--help` | 印帮助 |

认不出的页名照常打开首页。环境变量 `MIYU_TUI_IME=0` 让终端不切换输入法（只在用 fcitx5 输入法时才切换）。

```sh
miyu tui --resume 0198c2a4-7e1b-7c3d-9a2f-4b5c6d7e8f90
```

见 [终端](heads/tui.md)。

## miyu shellhook

```text
miyu shellhook <子命令> [<选项>]
```

装进 shell 以后，在提示符后面敲一句话、回车，就是和 AI 说话。

| 子命令 | 做什么 |
|---|---|
| `install [--shell <fish\|bash\|zsh\|pwsh\|powershell>] [--program <路径>]` | 装进 shell。`pwsh` 是 PowerShell 7，`powershell` 是 Windows 自带的 Windows PowerShell 5.1。不写 `--shell` 的自己认：Linux 上看上一层进程，认不出的看 `SHELL`；macOS 上看 `SHELL`；Windows 上看 `SHELL`，没有的看 `PSModulePath` 分出 PowerShell 7 和 5.1 |
| `uninstall [--shell <fish\|bash\|zsh\|pwsh\|powershell>]` | 从 shell 里拿掉 |
| `status` | 看装进了哪几个 shell |
| `doctor` | 查装好了没有、连不连得上核心、终端的颜色、剪贴板 |

`--program <路径>` 是 shell 里调的程序，不写是这个程序旁边的 `miyu`。

```sh
miyu shellhook install --shell zsh
```

退出码：参数不对的 `2`；`doctor` 有一条不好的 `1`。见 [终端](heads/tui.md)。

## miyu web

```text
miyu web [<选项>]
```

打开网页界面：网页没在跑的先起来，再交给浏览器。

| 参数 | 做什么 |
|---|---|
| `--port <端口>` | 这一次听哪个端口（网页没在跑时才算），出厂 `8300` |
| `--print` | 不开浏览器，印出地址 |
| `--format <text\|json>` | 印的格式；`json` 也不开浏览器 |
| `--reset` | 带一次性码打开，重设访问密码 |
| `--logout` | 让所有浏览器都要重新登录 |
| `--package <编号>` | 直接打开「软件后台」里这个软件包的页 |
| `--session <编号>` | 直接打开这个会话 |
| `--settings [<分页>]` | 直接打开设置；写 `access` 打开「通用」里的「访问」 |
| `--stop` | 停掉网页，核心照常跑。只能单独写 |
| `-h`、`--help` | 印帮助 |

```sh
miyu web --settings access
```

参数不对的退出码 `2`。见 [网页](heads/web.md)、[常驻和服务器](service.md#从别的机器手机打开网页)。
