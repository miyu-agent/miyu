# 扩展 Miyu

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

不需要写常驻的程序，也可以往自己的 Miyu 里添加四样东西：技能、脚本、斜杠命令模板和 MCP 服务。前三样只要把文件放进对应的文件夹即可，MCP 服务需要写进配置文件。

| | 是什么 | 放在哪里 |
|---|---|---|
| 技能 | 一份 `SKILL.md`，教 AI 一套做事的方法。 | `~/.miyu/root/skills/<名字>/SKILL.md` |
| 脚本 | 一个文件就是 AI 的一件工具。 | `~/.miyu/root/scripts/<文件>` |
| 斜杠命令模板 | 一份 `.md` 文件，你输入 `/<名字>` 时，它的正文会当作你说的一句话发给 AI。 | `~/.miyu/root/commands/<名字>.md` |
| MCP 服务 | 接入一个现成的 MCP 服务，把它的工具交给 AI 使用。 | `~/.miyu/etc/config.toml` 里的一张 `[mcp.<名字>]` 表 |

下文路径里的 `~/.miyu` 指数据根：默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，数据根就是它指向的目录。如果上面三个文件夹不存在，需要你自己创建。

这四样东西在预设里各是一个功能，可以单独开关：技能是 `skill-<名字>`，脚本是 `script-<工具名>`，模板是 `command-<名字>`，MCP 服务是 `mcp-<服务名>`。详细的见 [人格和预设](personas-presets.md)。

如果写错了，或者和其他东西重名，它就不会出现，`miyu check` 会告诉你是哪一个、为什么。

## 技能

技能是一个文件夹，里面有一份 `SKILL.md`。它的格式是公开的 Agent Skills 格式，所以 Claude Code、Codex 这些工具能识别的技能，放进来就能直接使用。

### 编写技能

例如，创建 `~/.miyu/root/skills/weekly-report/SKILL.md`：

```markdown
---
name: weekly-report
description: "Write the weekly report from this week's git log and notes/. Use when the user asks for a weekly report, 周报."
metadata:
  display_name: Weekly report
  display_name_zh: 写周报
  summary: Weekly report from git log and notes
  summary_zh: 照 git 记录和笔记写周报
---

# Weekly report

1. Run `git log --since="1 week ago" --oneline` in the workspace.
2. Read every file under `notes/` changed this week.
3. Write `reports/YYYY-WW.md` with three sections: Done, In progress, Blocked.
```

文件开头两行 `---` 之间的部分是 YAML，后面是正文。

| 字段 | 说明 |
|---|---|
| `name` | 1 到 64 个字符，只能使用小写字母、数字和 `-`；`-` 不能在开头或结尾，也不能连续出现两个。它要和文件夹同名。如果不同名，按写的名字使用；如果没写或者写错了，按文件夹名使用，这两种情况 `miyu check` 都会各报一条警告；如果两个名字都不合规则，这个技能不会被收录。 |
| `description` | 写给 AI 看的：说清楚这个技能是什么、什么时候用。必须填写。超过 1024 个字符的部分会被截掉。要用双引号包起来：如果值里有「冒号加空格」又没加引号，就不是合法的 YAML；Miyu 能把它当作一整句重新读取，但会报警告。 |
| `metadata` 的 `display_name`、`summary` | 写给你看的名字和一句说明。中文、日文另外写在 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja` 里。 |
| `disable-model-invocation` | 写 `true` 时，AI 不会自己使用这个技能，只能由你点名。 |
| `user-invocable` | 写 `false` 时，你不能点名这个技能，它只给 AI 自己使用。 |

其他字段（`license`、`compatibility`、`allowed-tools` 等）Miyu 能识别，但不会使用。文件夹里另外放的文件（参考资料、附带的脚本）AI 可以读取，但不能修改；附带的脚本不是工具，所以需要在正文里写清楚怎么运行它。

### AI 什么时候读取技能

平时 AI 只能看到每个技能的名字、`description` 和 `SKILL.md` 的路径，觉得用得上时才会自己去读正文。所以 `description` 必须写清楚什么时候用。另外，AI 要有读文件的工具（出厂的「读文件」），技能才会列给 AI。

你也可以点名使用技能：在终端界面或网页界面的输入框里输入 `$`，会弹出技能列表，从中选一个，或者直接输入 `$weekly-report`。一句话里可以点名几个技能。点名的技能正文会排在这句话前面，一起交给 AI。如果会话的预设没有开启这个技能，点名时会报 `预设「<预设>」未启用技能 $<名字>。`。

### 添加之后

- 技能放进文件夹以后，正在进行的会话从下一轮开始也能看到它。
- 网页界面设置页（`/settings`）的 `技能` 页面列出了全部技能，分别标着 `自制`、`随软件包来的`、`共用`，每个技能有一个开关；点击 `打开目录` 会打开 `~/.miyu/root/skills/`。
- 在输入框里输入 `$`，可以看看列表里有没有它。

如果技能重名，`~/.miyu/root/skills/` 里的优先，其次是软件包带的，最后是其他目录里的。

### 读取其他目录里的技能

其他应用（Codex、opencode 等）常把技能安装在 `~/.agents/skills`。打开下面这个设置以后，Miyu 也会识别这些技能：

```sh
miyu config set skills.external.enabled true
```

也可以在网页界面 `技能` 页面的顶部打开 `读取其他目录的 Skills`。读取哪些目录由 `skills.external.dirs` 决定，出厂值是 `["~/.agents/skills"]`；可以写 `~/…` 或绝对路径，按先后顺序读取，重名时排在前面的优先。在 Windows 上也可以写 `~\…`：

```bash
miyu config set skills.external.dirs '["~/.agents/skills", "~/src/team-skills"]'
```

在 PowerShell 里，外面用双引号，里面用 TOML 的单引号字符串，Windows PowerShell 5.1 和 PowerShell 7 都可以直接复制使用：

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/src/team-skills']"
```

这两项设置都从下一轮开始生效。Miyu 不会删除这些目录里的技能，要删除的话需要到那个目录里删除。

### 检查写法错误

`miyu check` 会报出被跳过的技能，并说明原因：开头没有用两行 `---` 包起来的一段、YAML 无法解析、没有 `description`、名字和文件夹名都不合规则。对于已经收录、但格式不合规范的技能，会另外报警告：

| 警告 | 意思 |
|---|---|
| 写的名字和文件夹不同名，照写的名字用 | 把两者改成同名。 |
| 名字没写或者不合写法，照文件夹的名字用 | 补上 `name`。 |
| 开头不是合法的 YAML | 给带冒号的值加上引号。 |
| 说明超过 1024 个字，后面截掉了 | 把说明写短。 |
| `compatibility` 应是不超过 500 个字的一句话 | 把它写短。 |
| `metadata` 应是字到字的表 | 值不能是数字、列表或者嵌套的表。 |
| 和 <其他> 同名，被它盖住了 | 换一个名字。 |

## 脚本

一个脚本文件就是一件工具。在文件开头的注释里写说明和参数，AI 就可以像调用其他工具一样调用它。

### 编写脚本

一个不带参数的 Shell 脚本，`~/.miyu/root/scripts/disk-free.sh`：

```sh
#!/bin/sh
# Description: Show used and free space of every mounted disk.
# 描述：看每块盘用了多少、还剩多少
# Display name: Disk free
# 显示名称：磁盘空间
# Timeout: 10

df -h
```

这个脚本按 `#!/bin/sh` 启动，所以在 Windows 上，`PATH` 里要能找到 `sh` 才能运行。在 Windows 上，可以把同样的功能写成 PowerShell 脚本 `~/.miyu/root/scripts/disk-free.ps1`，它只在 Windows 上使用：

```powershell
# Description: Show used and free space of every mounted disk.
# 描述：看每块盘用了多少、还剩多少
# Display name: Disk free
# 显示名称：磁盘空间
# Timeout: 10

Get-PSDrive -PSProvider FileSystem
```

一个带参数的 Python 脚本，`~/.miyu/root/scripts/c_to_f.py`：

```python
#!/usr/bin/env python3
# Description: Convert a temperature between Celsius and Fahrenheit.
# 描述：摄氏度和华氏度互换
# 显示名称：温度换算
# Clicall: yes
# Parameters:
# {"type": "object",
#  "properties": {"value": {"type": "number"}, "to": {"type": "string", "enum": ["C", "F"]}},
#  "required": ["value", "to"]}

import json
import sys

args = json.load(sys.stdin)
v = float(args["value"])
if args["to"] == "F":
    print(f"{v} °C = {v * 9 / 5 + 32:.1f} °F")
else:
    print(f"{v} °F = {(v - 32) * 5 / 9:.1f} °C")
```

工具名由文件名决定：去掉最后一个扩展名，不是字母、数字、`_`、`-` 的字符换成 `_`，最多 64 个字符。上面几个脚本的工具名是 `disk-free`（`.sh`、`.ps1` 两份同名，只放其中一份）和 `c_to_f`。子文件夹和以点开头的文件不算作脚本。

### 开头的注释

文件开头是指从第一行起连续的一段 `#` 或 `//` 注释，`#!` 那一行也算在里面，遇到第一个不是注释的行就结束。写完开头以后要先空一行，再写其他注释，否则后面注释里的 `Timeout: …` 这类内容也会被读进去。每一行的格式是 `键: 值`，冒号用全角或半角都可以。

| 键 | 说明 |
|---|---|
| `Description` | 写给 AI 看的一句说明。必须填写。 |
| `描述` | 写给你看的一句说明。 |
| `Display name`、`显示名称` | 写给你看的名字，分别是英文和中文。 |
| `Parameters` | 参数的 JSON Schema，顶层必须是 `"type": "object"`。可以写在同一行，也可以从下一行开始分几行写，直到拼成一份完整的 JSON 为止。不写表示没有参数。 |
| `Timeout` | 最多运行几秒，必须是正整数，默认是 `60`。到时间以后，整组进程都会被杀掉。 |
| `Clicall` | 写 `yes`（或 `true`）时，AI 在命令里写脚本，也可以用 `miyu call` 调用这个脚本，结果交给那段命令，不进入对话。不写时是 `no`。只读取、只查询的脚本适合写上；会修改东西、发送东西、花钱的脚本不要写。 |
| `Venues` | 给哪几种会话使用：从 `local`（本机的终端界面、网页界面）、`private`（通讯平台上的私聊）、`group`（群）里选几个，用逗号隔开，例如 `Venues: local, private`。不写时给所有会话使用。 |
| `Scope` | 在 `Venues` 的范围内再收窄：`any`（默认）、`local`（本机的会话）、`local_main`（本机的主会话，不给子代理）、`local_main_attended`（本机的主会话，而且有人在看着）、`spawn`（本机的会话，并且子代理的层数没有到上限）。 |
| `Role` | 只能写 `read_file`，表示这件工具是用来读取文本文件的。 |

如果没写 `Description`，或者 `Parameters`、`Timeout`、`Clicall`、`Venues`、`Scope`、`Role` 的值写错了，整个脚本都不会被收录。`Role`、`Scope` 的键名写成 `role`、`Scpoe` 这种差一个字的写法，也算写错。其他键 Miyu 能识别，但不会使用。

### 怎么跑

- 脚本怎样启动，取决于第一行有没有 `#!`：

  | | 写了 `#!` | 没写 `#!` |
  |---|---|---|
  | Linux、macOS | 按写的内容启动：解释器、它后面的几个参数，再加上脚本。 | 直接运行这个文件，文件需要有可执行权限：`chmod +x ~/.miyu/root/scripts/<文件>` |
  | Windows | 如果解释器写的是这台电脑上存在的绝对路径，就使用它；否则取解释器的名字在 `PATH` 里查找：写 `env` 的取 `env` 后面的那个名字，`python3` 换成 `python`。例如 `#!/usr/bin/env python3` 会查找 `python`，`#!/bin/sh` 会查找 `sh`。 | 按扩展名决定：`.py` 用 `python`，`.ps1` 用 Windows PowerShell（`powershell`，不读取配置文件，也不受执行策略限制），`.js` 用 `node`，`.cmd`、`.bat` 用 `cmd /C`；其他扩展名直接运行。 |

  Miyu 不自带解释器，用到的解释器需要你自己安装。在 Windows 上安装好 Python、Node 以后，`python`、`node` 必须能在 `PATH` 里找到。
- 参数：AI 给出的参数是一份 JSON，可以从标准输入完整读取；如果不超过 30 KiB，环境变量 `MIYU_ARGS_JSON` 里也有一份。长的参数只在标准输入里，所以从标准输入读取最稳妥。
- 结果：标准输出和标准错误按输出的先后顺序交给 AI，最多 30000 个字符，超出时保留开头和结尾各一半。退出码不是 `0` 时算作失败，原因要写进输出里。
- 进度：输出一整行以 `miyu-progress: ` 开头的内容，你在界面上能看到，AI 看不到。这一行要写到标准错误并立即刷新，例如 Python 里写 `print("miyu-progress: 3/10", file=sys.stderr, flush=True)`。
- 工作目录是这个会话的工作目录。Python 按 UTF-8 读写标准输入和标准输出（`PYTHONUTF8=1`）。
- 脚本里不能调用 `miyu call`。

Miyu 会通过下面这些环境变量把几个目录交给脚本：

| 环境变量 | 是什么 |
|---|---|
| `MIYU_CACHE_DIR` | 可以写入的缓存目录，位于 `~/.miyu/var/cache/scripts/` 下，每个脚本一个。30 天没有用过的内容会被清掉，所以这里只放丢了还能重新生成的东西。 |
| `MIYU_STATE_DIR` | 一直保留的状态目录（存登录状态、上次运行到哪里等），每个脚本一个。删除脚本以后，状态目录仍然存在，`miyu check` 会提醒你它在哪里。 |
| `MIYU_IMAGES_DIR` | 用来交图片的空目录，每次调用都是一个新的。 |
| `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR` | `~/.miyu/root/` 下的 `pictures`、`videos`、`documents`、`downloads`，要交给你的文件放在这里。只有本机的会话有这几个变量，所以使用之前要先检查它们是否存在。 |
| `MIYU_CALLER_VENUE` | 脚本是在哪种会话里被调用的：本机的会话是 `local`，通讯平台上的会话是场所编号。 |
| `MIYU_CALLER_OWNER` | 这一轮说话的是不是你本人：`1` 或 `0`。它只是告诉脚本，并不能挡住任何人；要限制使用者，需要用 `Venues`。 |
| `MIYU_MODEL`、`MIYU_EFFORT` | 这一轮使用的模型和思考强度。还没有确定模型时，没有这两个变量。 |

### 把图片交给 AI

如果要让 AI 直接看到图片，把图片复制到 `MIYU_IMAGES_DIR` 里即可：

```python
import os, shutil
shutil.copy("chart.png", os.environ["MIYU_IMAGES_DIR"])
print("Drew 1 chart.")
```

- 图片按文件名排序，最多 4 张；支持 PNG、JPEG、GIF、WebP，根据文件内容识别，不看扩展名。
- 超过 5 MiB 或者某一边超过 8000 像素的图片会先缩小，缩小不了的不会交给 AI。
- 只看这个目录这一层的普通文件，不跟随链接。
- 没有交出去的图片，AI 不会知道它的存在，运行日志里会有一行 `script image dropped`。
- 如果这次调用因为超时被杀掉，就不会交图片；退出码不是 `0` 时照样会交。

### 在什么权限下跑

脚本和 AI 运行命令遵循同样的规则（见 [权限和沙盒](permissions.md)）：

- 开放权限：不在沙盒里运行，脚本拥有你这个系统用户的全部权限。
- 只读：在沙盒里运行，只能写入它自己的缓存目录、状态目录、交图目录和上面那四个文件夹，其他地方都不能写入。
- 通讯平台上的群、和陌生人的私聊：在沙盒里运行，只能写入那个会话的工作区，以及它自己的缓存目录、状态目录、交图目录。在 Windows 上，这两种会话里不运行脚本；在 Linux 上，如果沙盒管不住 Unix 套接字，也不运行脚本，详细的见 [权限和沙盒](permissions.md#通讯平台上的会话)。

脚本文件本身在数据根里，所以在沙盒里照样能读取。需要访问硬件或者需要 `sudo` 的脚本在沙盒里无法正常运行。在 Windows 上，使用 WMI 的脚本（`Get-CimInstance`、`Get-WmiObject`、`wmic` 这类）在沙盒里也连接不上 WMI，要使用这类脚本，需要切换到开放权限。各个系统的沙盒还有哪些不同，详细的见 [权限和沙盒](permissions.md#三个平台)。

### 添加之后

- 脚本放进去、修改或者删除以后，核心会马上知道，下一轮的工具就会跟着变化，不需要重启。
- 在终端界面里输入 `/tool-list`，可以查看这个会话有哪些工具。网页界面设置页的 `脚本` 页面列出了全部脚本，每个脚本有一个开关；点击 `打开目录` 会打开 `~/.miyu/root/scripts/`。
- 如果工具名相同，`~/.miyu/root/scripts/` 里的脚本优先于软件包带的脚本。和其他工具重名的脚本不会被登记。

脚本也可以是一个链接，指向你放在别处的文件，此时按它指向的文件来识别；断开的链接会被跳过。指向数据根里其他位置的链接不会被识别。

### 检查写法错误

`miyu check` 会报出下面这些问题：

| 看到的提示 | 怎么解决 |
|---|---|
| `跳过了这个脚本`：`Description is missing or empty` | 补上 `Description`。 |
| `跳过了这个脚本`：`Parameters is not valid JSON: …` | JSON 没有写完，或者写错了。 |
| `跳过了这个脚本`：`Parameters must be a JSON object schema` | 在顶层写上 `"type": "object"`。 |
| `跳过了这个脚本`：`Timeout "…" is not a positive number of seconds` | 写一个正整数。 |
| `跳过了这个脚本`：`Clicall "…" is not yes or no` | 写 `yes` 或 `no`。 |
| `跳过了这个脚本`：`Venues "…" must list some of local, private, group, each once` | 只写这三种，不要重复。 |
| `跳过了这个脚本`：`Role …`、`Scope …` | 按照上面的表格写键名和值。 |
| `脚本 <名字> 和别的工具重名，没登记上` | 修改文件名。 |
| `核心还没读到脚本 <名字>` | 脚本刚放进去，过一会儿再查；如果一直是这样，就重启核心。 |
| `有这个脚本的状态目录（…），可现在没有这个脚本了` | 如果用不上了，就删掉那个目录。 |

## 斜杠命令模板

斜杠命令模板是一份 Markdown 文件，文件名就是命令名。你输入 `/<名字> 参数` 时，正文会按参数展开，当作你说的一句话发给 AI，时间线上显示的是你输入的那一行。

### 编写模板

例如，创建 `~/.miyu/root/commands/commit-msg.md`：

```markdown
---
description: Draft a commit message for the staged changes
argument-hint: "[style]"
metadata:
  summary_zh: 给暂存的改动写提交说明
---

Read `git diff --cached` and draft a one-line commit message.
Style: $ARGUMENTS
```

输入 `/commit-msg 简短` 时，AI 收到的是正文，其中的 `$ARGUMENTS` 换成了 `简短`。

- 名字：以小写字母开头，只能使用小写字母、数字和 `-`，最多 32 个字符。子文件夹、以点开头的文件、不是 `.md` 的文件都不算作模板。
- 开头的那一段可以不写，这时整份文件都是正文。

| 字段 | 说明 |
|---|---|
| `description` | 命令列表里显示的那一句说明。 |
| `argument-hint` | 提示名字后面应该跟什么。 |
| `metadata` 的 `summary_zh`、`summary_ja` | 中文、日文的那一句说明。 |

其他字段（`allowed-tools`、`model` 等）Miyu 能识别，但不会使用。

### 参数

- `$ARGUMENTS` 会换成名字后面的全部文字，并去掉前后的空白。
- `$ARGUMENTS[0]` 是第一个词，`$ARGUMENTS[1]` 是第二个词，按空白切分。带空格的值要用英文双引号括起来：`/commit-msg "very short"` 的 `$ARGUMENTS[0]` 是 `very short`。如果没有对应的词，就换成空的。
- 如果正文里一个 `$ARGUMENTS` 都没写，而你在命令后面又跟了文字，这些文字会空一行接在正文末尾。

### 添加之后

- 每次输入命令时都会重新查找模板，所以修改以后不需要重启。
- 在终端界面或网页界面里输入 `/`，可以看看命令列表里有没有它。
- 如果重名，你的模板优先于软件包带的模板；和 Miyu 自带的命令重名时，你的模板不算数，自带的命令优先。
- 谁能使用模板，和 Miyu 自带的命令一样，详细的见 [斜杠命令](commands.md)。

`miyu check` 会报 `跳过了这个斜杠命令：…`（开头的 YAML 无法解析，或者不是一张表）、`和 <别的> 同名，被它盖住了`、`和 Miyu 自带的 /<名字> 同名，这一份不用`。

## MCP 服务

MCP（Model Context Protocol）服务有两种接入方式：由 Miyu 启动一个本机程序，通过标准输入输出和它通信；或者连接一个网址（Streamable HTTP）。

### 写进配置文件

每个服务写成一张 `[mcp.<名字>]` 表，写进 `~/.miyu/etc/config.toml`。可以用 `miyu config edit` 打开配置文件，保存之前会自动检查；`miyu config set` 不能修改这几张表。保存以后就会生效，不需要重启。

启动本机程序的写法：

```toml
[mcp.notes]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/home/me/notes"]
env = { LOG_LEVEL = "warn" }
```

在 Windows 上，路径按 Windows 的格式写，使用 TOML 的单引号字符串，里面的 `\` 保持原样：

```toml
[mcp.notes]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", 'C:\Users\me\notes']
env = { LOG_LEVEL = "warn" }
```

连接网址的写法：

```toml
[mcp.acme-docs]
url = "https://mcp.example.com/mcp"
bearer = { secret = "acme-docs" }
headers = { X-Workspace = "team-a" }
venues = ["local", "private"]
timeout_ms = 120000
clicall = false
```

`bearer` 引用的密钥要先存进密钥文件，运行：

```sh
miyu provider login acme-docs
```

粘贴密钥时不会显示出来。

| 字段 | 说明 |
|---|---|
| 表名 `<名字>` | 服务名：以小写字母开头，只能使用小写字母、数字和 `-`，最多 32 个字符。 |
| `command` | 命令和参数，一个词一项，不经过 shell。第一项可以写绝对路径、`~/…`（在 Windows 上也可以写 `~\…`），或者只写名字，在 `PATH` 里查找（`npx`、`uvx`、`docker` 这类）。在 Windows 上，只写名字时会在 `PATH` 里查找 `.exe`、`.cmd`、`.bat`；`npx`、`uvx` 是 `.cmd`，照样能找到。`~` 只在第一项里替换，后面的参数按原样交给程序。`command` 和 `url` 只能写其中一个。 |
| `env` | 和 `command` 一起使用：给服务的环境变量。 |
| `url` | 服务的网址，以 `https://` 开头；`http://` 只能连接本机的服务。`url` 和 `command` 只能写其中一个。 |
| `bearer` | 和 `url` 一起使用：令牌，会以 `Authorization: Bearer <值>` 的形式发送。 |
| `headers` | 和 `url` 一起使用：其他请求头。名字只能使用字母、数字和 `-`；不能写 `Authorization`、`Mcp-Session-Id`、`MCP-Protocol-Version`、`Mcp-Method`、`Mcp-Name`。 |
| `venues` | 给哪几种会话使用：从 `local`、`private`、`group` 里选几个。默认是 `["local"]`，只给本机的会话使用。 |
| `timeout_ms` | 一次调用最多等多久，单位是毫秒，范围是 `1000` 到 `3600000`，默认是 `600000`。 |
| `clicall` | AI 在命令里能不能用 `miyu call` 调用这个服务的工具。默认可以；可以写 `true`、`false`，或者 `"yes"`、`"no"`。会写入东西、发送东西的服务要写 `false`。 |

`env`、`bearer`、`headers` 的值可以直接写文字，也可以写引用：`{ secret = "<密钥名>" }` 从密钥文件里取值，`{ env = "<变量名>" }` 从核心的环境变量里取值。令牌要写成引用，不要把令牌的值写进配置文件。

也可以在网页界面设置页（`/settings`）的 `MCP` 页面点击 `添加服务`，选择 `本地` 或 `网址`，然后按表格填写。

如果配置文件里的服务和软件包带的服务同名，配置文件里的优先，软件包里的那个不会启动。

### 需要浏览器登录的服务

有些服务只支持浏览器授权，没办法给你个人令牌。这种服务只写 `url`，不写 `bearer`。它连上以后会停在 `需要登录` 状态，接下来按下面的步骤登录：

1. 运行 `miyu mcp login <服务名>`。
2. 命令会输出一个地址并自动在浏览器里打开，在这个页面里点击同意。如果浏览器没有自动打开，就自己打开输出的地址。
3. 看到 `已登录，<服务名> 已连接` 就完成了。

如果浏览器在另一台电脑上：授权完成以后，把浏览器地址栏里的整段地址粘贴回命令行，按回车即可。命令最多等待 10 分钟。令牌会存进密钥文件，快过期时核心会自动续期。不再使用时，运行 `miyu mcp logout <服务名>`。

网页界面的 `MCP` 页面、终端界面设置页的 `MCP` 里也有 `登录`。写了 `bearer` 的服务不走登录流程。

### AI 看到什么

| 服务提供的 | 在 Miyu 里变成 |
|---|---|
| 工具 | AI 的工具，名字是 `mcp__<服务名>__<工具名>`。不是字母、数字、`_`、`-` 的字符会换成 `_`，名字最长 64 个字符；说明最多 2048 个字符。 |
| 资源 | 多出两件工具：`mcp__<服务名>__list_resources` 一页一页地列出资源，`mcp__<服务名>__read_resource` 按地址读取一份资源。如果服务自己有同名的工具，就使用服务自己的。 |
| 提示词 | 斜杠命令 `/<服务名>:<提示词名>`，出现在命令列表里。命令后面跟的文字按空白切开，按提示词声明的参数顺序一一对应，最后一个参数拿走剩下的全部文字；带空格的值要用英文双引号括起来。取回来的内容会当作你说的一句话发给 AI。只能在本机的会话里使用。 |
| 说明（`instructions`） | 放在 AI 的系统提示词末尾，每个服务一段，最多 4000 个字符。 |

只读标记：如果服务给工具标了 `readOnlyHint: true`，在任何权限下都不会问你；没有标的工具按联网的一类处理，开放权限下不问，只读下每次都会问你。详细的见 [权限和沙盒](permissions.md)。

### 添加之后

- 终端界面设置页的 `MCP` 里列出了每个服务现在的状态：`关着`、`在启动`、`在跑`、`等重连`、`停了`、`要登录`，以及它有几件工具。网页界面的 `MCP` 页面也列出了这些服务，可以 `重启`、`编辑`、`删除`。
- 在终端界面里输入 `/tool-list`，可以查看工具在不在。
- 如果服务列出了某些工具和提示词，但没有登记上，网页界面会显示 `服务列出来了，但核心没登记这几个`，`miyu check` 也会报出来。

### 写法错误和启动失败

- `miyu check` 会报 `MCP 服务的写法不对：…`，并带上行号。写错的那个服务不会启动，其他服务照常运行。
- 本机程序的工作目录是 `~/.miyu/var/lib/mcp/<服务名>/`（也在环境变量 `MIYU_STATE_DIR` 里），不在沙盒里运行。它的标准错误在 `~/.miyu/var/log/mcp/<服务名>.stderr`，启动失败时先查看这个文件。
- 启动失败或者崩溃的服务会隔一会儿重试，连续失败 5 次后停止。修好以后，在 `MCP` 页面点击 `重启`。
- 用 `npx`、`uvx` 启动的服务，需要你先自己安装 Node、Python 这些环境。

## 想做得更多

如果你需要一个一直运行的程序、自己的界面或者后台页，或者想把这些东西发给别人用，就把它们做成软件包：可以看 [写一个 Miyu 软件包](../../authors/README.md) 和 [写扩展](../../authors/extensions.md)。技能、脚本、模板分别放进软件包文件夹的 `skills/`、`scripts/`、`commands/`，格式和这一页讲的一样。

也可以让 AI 帮你写。出厂的「写技能和包」带了两个技能：

- `skill-creator`（写技能）：按软件包的结构写一个技能，检查之后安装上。
- `package-creator`（写脚本和包）：把脚本做成工具放进软件包里，检查之后安装上。

在输入框里输入 `$skill-creator` 或 `$package-creator`，后面接上你想要什么；或者直接说「写个技能」「写个工具」，AI 会自己去读这两个技能。两个技能写出来的都是软件包，安装以后在软件包页里管理，详细的见 [软件包](packages.md)。
