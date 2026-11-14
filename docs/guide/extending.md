# 扩展 Miyu

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

不写常驻的程序，往自己的 Miyu 里加四样东西：技能、脚本、斜杠命令模板、MCP 服务。前三样放文件就行，MCP 服务写进配置。

| | 是什么 | 放在哪 |
|---|---|---|
| 技能 | 一份 `SKILL.md`，教 AI 一套做事的办法 | `~/.miyu/root/skills/<名字>/SKILL.md` |
| 脚本 | 一个文件就是 AI 的一件工具 | `~/.miyu/root/scripts/<文件>` |
| 斜杠命令模板 | 一份 `.md`，你敲 `/<名字>` 时把正文当你说的一句发给 AI | `~/.miyu/root/commands/<名字>.md` |
| MCP 服务 | 接一个现成的 MCP 服务，它的工具给 AI 用 | `~/.miyu/etc/config.toml` 里一张 `[mcp.<名字>]` |

路径里的 `~/.miyu` 是数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的照它）。上面三个文件夹没有的自己建。

四样在预设里各是一个功能，能单独开关：技能是 `skill-<名字>`，脚本是 `script-<工具名>`，模板是 `command-<名字>`，MCP 服务是 `mcp-<服务名>`。见 [人格和预设](personas-presets.md)。

写错了的、和别的东西重名的不会出现，`miyu check` 说哪一个、为什么。

## 技能

技能是一个文件夹，里面一份 `SKILL.md`。格式是公开的 Agent Skills 格式，Claude Code、Codex 这些认的技能放进来就能用。

### 写一个

`~/.miyu/root/skills/weekly-report/SKILL.md`：

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

开头两行 `---` 之间是 YAML，后面是正文。

| 格 | 说明 |
|---|---|
| `name` | 1 到 64 个字，只用小写字母、数字、`-`，`-` 不在头尾、不连着两个。和文件夹同名。不同名的照写的用，没写、写错的照文件夹名用，`miyu check` 各报一条警告；两个都不合的不收 |
| `description` | 给 AI 看的：说清是什么、什么时候用。必写。超过 1024 个字的截掉。用双引号包起来：值里有「冒号加空格」又没加引号的不合 YAML，Miyu 能照一整句重读，但会报警告 |
| `metadata` 的 `display_name`、`summary` | 给你看的名字和一句说明。中文、日文另写 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja` |
| `disable-model-invocation` | 写 `true` 的 AI 不会自己用，只能你点名 |
| `user-invocable` | 写 `false` 的你点不了名，只给 AI 自己用 |

别的格（`license`、`compatibility`、`allowed-tools` 这些）认、不用。文件夹里另放的文件（参考资料、带的脚本）AI 读得到、改不了；带的脚本不是工具，正文里写清怎么跑。

### AI 什么时候读它

AI 平时只看到每个技能的名字、`description` 和 `SKILL.md` 的路径，觉得用得上时自己去读正文。所以 `description` 要写清什么时候用。AI 要有读文件的工具（出厂的「读文件」）才列技能。

你也能点名：在终端、网页的输入框里敲 `$`，弹出技能列表，选一个，或者直接敲 `$weekly-report`。一句话能点几个。点了的技能正文排在这句话前面，一起交给 AI。会话的预设没开这个技能的，点名报「预设「<预设>」未启用技能 $<名字>。」。

### 加了以后

- 放进去，开着的会话从下一轮起也看得到。
- 网页设置页（`/settings`）的「技能」页列着全部技能，标着「自制」「随软件包来的」「共用」，每个一个开关；「打开目录」打开 `~/.miyu/root/skills/`。
- 输入框敲 `$` 看列表里有没有它。

同名的，`~/.miyu/root/skills/` 里的赢，其次软件包带的，最后是别的目录里的。

### 读别的目录里的技能

别的应用（Codex、opencode 这些）常把技能装在 `~/.agents/skills`。打开以后 Miyu 也认：

```sh
miyu config set skills.external.enabled true
```

或者在网页「技能」页顶上打开「读取其他目录的 Skills」。读哪些目录是 `skills.external.dirs`，出厂是 `["~/.agents/skills"]`，写 `~/…` 或绝对路径，照先后，同名的先列的赢。`~` 后面用 `/`，Windows 上也是，`~\…` 不认：

```bash
miyu config set skills.external.dirs '["~/.agents/skills", "~/src/team-skills"]'
```

PowerShell 里外面用双引号，里面用 TOML 的单引号字符串，Windows PowerShell 5.1 和 PowerShell 7 都能照抄：

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/src/team-skills']"
```

两项都从下一轮起生效。这些目录里的技能 Miyu 不删，要删到那个目录里删。

### 写错了

`miyu check` 报跳过的技能，说为什么：开头没有两行 `---` 包着的一段、YAML 读不懂、没有 `description`、名字和文件夹名都不合写法。照收了、但不合格式的另报警告：

| 警告 | 意思 |
|---|---|
| 写的名字和文件夹不同名，照写的名字用 | 改成同名 |
| 名字没写或者不合写法，照文件夹的名字用 | 补上 `name` |
| 开头不是合法的 YAML | 值里有冒号的加上引号 |
| 说明超过 1024 个字，后面截掉了 | 写短 |
| `compatibility` 应是不超过 500 个字的一句话 | 写短 |
| `metadata` 应是字到字的表 | 值不能是数字、列表或者嵌套的表 |
| 和 <别的> 同名，被它盖住了 | 换个名字 |

## 脚本

一个文件一件工具。文件开头的注释写说明和参数，AI 和调别的工具一样调它。

### 写一个

Shell，没有参数，`~/.miyu/root/scripts/disk-free.sh`：

```sh
#!/bin/sh
# Description: Show used and free space of every mounted disk.
# 描述：看每块盘用了多少、还剩多少
# Display name: Disk free
# 显示名称：磁盘空间
# Timeout: 10

df -h
```

它照 `#!/bin/sh` 起，Windows 上要 `PATH` 里找得到 `sh` 才起得来。Windows 上同一件事写成 PowerShell，`~/.miyu/root/scripts/disk-free.ps1`，只在 Windows 上用：

```powershell
# Description: Show used and free space of every mounted disk.
# 描述：看每块盘用了多少、还剩多少
# Display name: Disk free
# 显示名称：磁盘空间
# Timeout: 10

Get-PSDrive -PSProvider FileSystem
```

Python，带参数，`~/.miyu/root/scripts/c_to_f.py`：

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

工具名照文件名：去掉最后一个扩展名，不是字母、数字、`_`、`-` 的换成 `_`，最多 64 个字。上面的是 `disk-free`（`.sh`、`.ps1` 两份同名，放一份）、`c_to_f`。子文件夹、点开头的文件不算。

### 开头的注释

开头是文件第一行起连着的那一段 `#` 或 `//` 注释，`#!` 那一行也算在里面，遇到第一行不是注释的就完。开头写完空一行再写别的注释，不然后面注释里的「Timeout: …」也会被读进去。每一行「键: 值」，冒号全角、半角都行。

| 键 | 说明 |
|---|---|
| `Description` | 给 AI 看的一句。必写 |
| `描述` | 给你看的一句说明 |
| `Display name`、`显示名称` | 给你看的名字，英文、中文 |
| `Parameters` | 参数的 JSON Schema，顶层要是 `"type": "object"`。可以写在同一行，也可以从下一行起分几行写，拼成一份完整的 JSON 为止。不写是没有参数 |
| `Timeout` | 最多跑几秒，正整数，默认 `60`。到时整组进程杀掉 |
| `Clicall` | `yes`（或 `true`）：AI 在命令里写脚本时，也能用 `miyu call` 调它，结果交给那段命令，不进对话。不写是 `no`。只读、只查的脚本适合写上，会改东西、发东西、花钱的别写 |
| `Venues` | 给哪种会话：`local`（本机的终端、网页）、`private`（通讯平台的私聊）、`group`（群）里的几个，逗号隔开，例如 `Venues: local, private`。不写是哪种都给 |
| `Scope` | 在 `Venues` 之内再收窄：`any`（默认）、`local`（本机的会话）、`local_main`（本机的主会话，不给子代理）、`local_main_attended`（本机的主会话，而且有人看着）、`spawn`（本机、子代理层数没到上限） |
| `Role` | 只认 `read_file`：说明这件工具是读文本文件的 |

没写 `Description` 的，`Parameters`、`Timeout`、`Clicall`、`Venues`、`Scope`、`Role` 写错了值的，整个脚本不收。`Role`、`Scope` 的键名写成 `role`、`Scpoe` 这种差一个字的也算写错。别的键认、不用。

### 怎么跑

- 起法看第一行有没有 `#!`：

  | | 写了 `#!` 的 | 没写 `#!` 的 |
  |---|---|---|
  | Linux、macOS | 照写的起：解释器、它后面的几个参数、脚本 | 直接跑这个文件，要有可执行权限：`chmod +x ~/.miyu/root/scripts/<文件>` |
  | Windows | 解释器写的是这台机器上在的绝对路径的照它；别的取解释器的名字照 `PATH` 找，写 `env` 的取 `env` 后面的那个，`python3` 换成 `python`：`#!/usr/bin/env python3` 找 `python`，`#!/bin/sh` 找 `sh` | 照扩展名：`.py` 用 `python`，`.ps1` 用 Windows PowerShell（`powershell`，不读配置文件、不管执行策略），`.js` 用 `node`，`.cmd`、`.bat` 用 `cmd /C`；别的直接跑 |

  Miyu 不带解释器，用到的自己装。Windows 上 Python、Node 装好以后，`python`、`node` 要在 `PATH` 里找得到。
- 参数：AI 给的一份 JSON，从标准输入整份读得到；不超过 30 KiB 的，环境变量 `MIYU_ARGS_JSON` 里也有一份。长的只在标准输入里，读标准输入最稳。
- 结果：标准输出、标准错误照先后交给 AI，最多 30000 个字，多了留头尾各一半。退出码不是 `0` 的算失败，原因写进输出里。
- 进度：印一整行以 `miyu-progress: ` 开头的，你在界面上看得到，AI 看不到。写到标准错误、马上冲出去，例如 Python 写 `print("miyu-progress: 3/10", file=sys.stderr, flush=True)`。
- 工作目录是这个会话的工作目录。Python 照 UTF-8 读写标准输入输出（`PYTHONUTF8=1`）。
- 脚本里调不了 `miyu call`。

给脚本的几个目录：

| 环境变量 | 是什么 |
|---|---|
| `MIYU_CACHE_DIR` | 能写的缓存目录，在 `~/.miyu/var/cache/scripts/` 下每个脚本一格。30 天没碰的会被清掉，只放丢了能再生成的 |
| `MIYU_STATE_DIR` | 一直留着的状态目录（登录态、上次跑到哪），每个脚本一格。删了脚本，状态目录还在，`miyu check` 提醒你在哪 |
| `MIYU_IMAGES_DIR` | 交图用的空目录，每次调用一个新的 |
| `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR` | `~/.miyu/root/` 下的 `pictures`、`videos`、`documents`、`downloads`，给你的文件放这里。只有本机的会话有，用之前先看在不在 |
| `MIYU_CALLER_VENUE` | 在哪种会话里调的：本机的是 `local`，通讯平台的是场所编号 |
| `MIYU_CALLER_OWNER` | 这一轮开口的是不是你本人：`1`、`0`。只是告诉脚本，挡不住谁；要挡用 `Venues` |
| `MIYU_MODEL`、`MIYU_EFFORT` | 这一轮的模型、思考强度。还没定下模型的没有 |

### 交图

要 AI 直接看到图，把图拷进 `MIYU_IMAGES_DIR`：

```python
import os, shutil
shutil.copy("chart.png", os.environ["MIYU_IMAGES_DIR"])
print("Drew 1 chart.")
```

- 照文件名排，最多 4 张；认 PNG、JPEG、GIF、WebP，看文件内容认，不看扩展名。
- 超过 5 MiB 或者一边超过 8000 像素的先缩小，缩不了的不交。
- 只看这一层的普通文件，不跟链接。
- 不交的那张 AI 不知道有它，运行日志里有一行 `script image dropped`。
- 到时被杀的那一次不交图；退出码不是 `0` 的照样交。

### 在什么权限下跑

照 AI 跑命令的规矩（见 [权限和沙盒](permissions.md)）：

- 开放权限：不进沙盒，脚本有你这个系统用户的全部权限。
- 只读：进沙盒，只写得了它自己的缓存、状态、交图目录和上面那四个文件夹，别处都写不了。
- 通讯平台的群、陌生人私聊：在沙盒里跑，只写得了那个会话的工作区和它自己的缓存、状态、交图目录。Windows 上这两种会话里不跑脚本，Linux 上沙盒管不住 Unix 套接字的也不跑，见 [权限和沙盒](permissions.md#通讯平台上的会话)。

脚本文件本身在数据根里，进沙盒时单独放开它只读。碰硬件、要 `sudo` 的脚本在沙盒里跑不通。Windows 上用 WMI 的（`Get-CimInstance`、`Get-WmiObject`、`wmic` 这类）在沙盒里也连不上，要用就切到开放权限。各系统的沙盒还有哪些不同，见 [权限和沙盒](permissions.md#三个平台)。

### 加了以后

- 放进去、改了、删了，核心当场知道，下一轮工具就跟着变，不用重启。
- 终端敲 `/tool-list` 看这个会话有哪些工具。网页设置页的「脚本」页列着全部脚本，每个一个开关，「打开目录」打开 `~/.miyu/root/scripts/`。
- 同工具名的，`~/.miyu/root/scripts/` 里的赢过软件包带的。和别的工具重名的不登记。

脚本也可以是链接，指到你别处放的文件，照它指到的认；断了的跳过。指到数据根里别处的不认。

### 写错了

`miyu check` 报：

| 看到的 | 怎么办 |
|---|---|
| 跳过了这个脚本：`Description is missing or empty` | 补上 `Description` |
| 跳过了这个脚本：`Parameters is not valid JSON: …` | JSON 没写完或者写错了 |
| 跳过了这个脚本：`Parameters must be a JSON object schema` | 顶层写 `"type": "object"` |
| 跳过了这个脚本：`Timeout "…" is not a positive number of seconds` | 写正整数 |
| 跳过了这个脚本：`Clicall "…" is not yes or no` | 写 `yes` 或 `no` |
| 跳过了这个脚本：`Venues "…" must list some of local, private, group, each once` | 只写这三种，不重复 |
| 跳过了这个脚本：`Role …`、`Scope …` | 键名、值照上表写 |
| 脚本 <名字> 和别的工具重名，没登记上 | 改文件名 |
| 核心还没读到脚本 <名字> | 刚放进去，过一会儿再查；一直这样就重启核心 |
| 有这个脚本的状态目录（…），可现在没有这个脚本了 | 用不上就删掉那个目录 |

## 斜杠命令模板

一份 Markdown，文件名就是命令名。你敲 `/<名字> 参数`，正文照参数展开，当你说的一句发给 AI，时间线上显示你敲的那一行。

### 写一个

`~/.miyu/root/commands/commit-msg.md`：

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

敲 `/commit-msg 简短`，AI 收到的是正文，`$ARGUMENTS` 换成「简短」。

- 名字：小写字母开头，只用小写字母、数字、`-`，最多 32 个。子文件夹、点开头的、不是 `.md` 的不算。
- 开头那一段可以不写，整份就是正文。

| 格 | 说明 |
|---|---|
| `description` | 命令列表里那一句 |
| `argument-hint` | 名字后面跟什么的提示 |
| `metadata` 的 `summary_zh`、`summary_ja` | 中文、日文的那一句 |

别的格（`allowed-tools`、`model` 这些）认、不用。

### 参数

- `$ARGUMENTS` 换成名字后面的全部字，去掉前后空白。
- `$ARGUMENTS[0]` 是第一个词，`$ARGUMENTS[1]` 是第二个，照空白切。带空格的值用英文双引号括起来：`/commit-msg "very short"` 的 `$ARGUMENTS[0]` 是 `very short`。没有那一个的换成空的。
- 正文里一个 `$ARGUMENTS` 都没写、你又跟了字的，空一行接在正文末尾。

### 加了以后

- 每次敲都现找，改了不用重启。
- 在终端、网页敲 `/` 看命令列表里有没有它。
- 同名的，你的赢过软件包带的；和 Miyu 自带的命令同名的不算，自带的赢。
- 谁能用和 Miyu 自带的命令一样，见 [斜杠命令](commands.md)。

`miyu check` 报「跳过了这个斜杠命令：…」（开头的 YAML 读不懂、不是一张表）、「和 <别的> 同名，被它盖住了」、「和 Miyu 自带的 /<名字> 同名，这一份不用」。

## MCP 服务

MCP（Model Context Protocol）服务有两种接法：Miyu 拉起一个本机程序，经标准输入输出说话；或者连一个网址（Streamable HTTP）。

### 写进配置

一个服务一张 `[mcp.<名字>]`，写进 `~/.miyu/etc/config.toml`。用 `miyu config edit` 打开，存之前会检查；`miyu config set` 改不了这几张表。存了就生效，不用重启。

拉起本机程序的：

```toml
[mcp.notes]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/home/me/notes"]
env = { LOG_LEVEL = "warn" }
```

Windows 上路径照 Windows 的写，用 TOML 的单引号字符串，里面的 `\` 照原样：

```toml
[mcp.notes]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", 'C:\Users\me\notes']
env = { LOG_LEVEL = "warn" }
```

连网址的：

```toml
[mcp.acme-docs]
url = "https://mcp.example.com/mcp"
bearer = { secret = "acme-docs" }
headers = { X-Workspace = "team-a" }
venues = ["local", "private"]
timeout_ms = 120000
clicall = false
```

`bearer` 引用的密钥先存进密钥文件：

```sh
miyu provider login acme-docs
```

贴的时候不显示。

| 格 | 说明 |
|---|---|
| 表名 `<名字>` | 服务名：小写字母开头，只用小写字母、数字、`-`，最多 32 个 |
| `command` | 命令和参数，一个词一项，不经 shell。第一项写绝对路径、`~/…`（Windows 上也用 `/`，`~\…` 不认），或者只写名字照 `PATH` 找（`npx`、`uvx`、`docker` 这类）。Windows 上只写名字的照 `PATH` 找 `.exe`、`.cmd`、`.bat`，`npx`、`uvx` 是 `.cmd`，照样找得到。`~` 只换第一项的，后面的参数照原样交给程序。和 `url` 只写一个 |
| `env` | 跟着 `command`：给服务的环境变量 |
| `url` | 服务的网址。`https://` 开头；`http://` 只连本机的服务。和 `command` 只写一个 |
| `bearer` | 跟着 `url`：令牌，发成 `Authorization: Bearer <值>` |
| `headers` | 跟着 `url`：别的请求头。名字只用字母、数字、`-`；`Authorization`、`Mcp-Session-Id`、`MCP-Protocol-Version`、`Mcp-Method`、`Mcp-Name` 不能写 |
| `venues` | 给哪种会话：`local`、`private`、`group` 里的几个。默认 `["local"]`，只给本机的会话 |
| `timeout_ms` | 一次调用最多等多久，毫秒，`1000` 到 `3600000`，默认 `600000` |
| `clicall` | AI 在命令里能不能用 `miyu call` 调这个服务的工具。默认能；`true`、`false`，或者 `"yes"`、`"no"`。会写东西、发东西的服务写 `false` |

`env`、`bearer`、`headers` 的值写字，或者引用：`{ secret = "<密钥名>" }` 照密钥文件取，`{ env = "<变量名>" }` 照核心的环境变量取。令牌写成引用，别把值写进配置。

也可以在网页设置页（`/settings`）的「MCP」页按「添加服务」，选「本地」或「网址」照表填。

配置里和软件包带的同名的，配置里的赢，包里那个不起。

### 要浏览器登录的服务

只认浏览器授权、给不了个人令牌的服务：只写 `url`，不写 `bearer`。它连上以后停在「需要登录」，然后：

1. 敲 `miyu mcp login <服务名>`。
2. 在印出来、自动打开的那个地址里点同意。浏览器没开成的，照印出来的地址自己打开。
3. 看到「已登录，<服务名> 已连接」就好了。

浏览器在另一台机器上的：授权完，把浏览器地址栏里的整段地址贴回命令行，回车。最多等 10 分钟。令牌存进密钥文件，快过期时核心自己续。不用了敲 `miyu mcp logout <服务名>`。

网页「MCP」页、终端设置页的「MCP」里也有「登录」。写了 `bearer` 的服务不走登录。

### AI 看到什么

| 服务给的 | 在 Miyu 里变成 |
|---|---|
| 工具 | AI 的工具，名字 `mcp__<服务名>__<工具名>`。不是字母、数字、`_`、`-` 的字换成 `_`，最长 64 个字；说明最多 2048 个字 |
| 资源 | 多两件工具：`mcp__<服务名>__list_resources` 一页页列，`mcp__<服务名>__read_resource` 照地址读一份。服务自己有同名工具的，用它的 |
| 提示词 | 斜杠命令 `/<服务名>:<提示词名>`，在命令列表里。后面跟的字照空白切开，照提示词声明的参数先后对上，最后一个参数拿剩下的整段；带空格的值用英文双引号。取回来的内容当你说的一句发给 AI。只在本机的会话里能用 |
| 说明（`instructions`） | 放进 AI 的系统提示词最后，一个服务一段，最多 4000 个字 |

只读标记：服务给工具标了 `readOnlyHint: true` 的，哪种权限下都不问你；没标的照联网的一类，开放权限下不问，只读下每次问你。见 [权限和沙盒](permissions.md)。

### 加了以后

- 终端设置页的「MCP」列着每个服务现在怎样：「关着」「在启动」「在跑」「等重连」「停了」「要登录」，和有几件工具。网页「MCP」页也列着，能「重启」「编辑」「删除」。
- 终端敲 `/tool-list` 看工具在不在。
- 服务列出来、却没登记上的工具和提示词，网页上写「服务列出来了，但核心没登记这几个」，`miyu check` 也报。

### 写错了、起不来

- `miyu check` 报「MCP 服务的写法不对：…」，带第几行。写错的那一个不起，别的照常。
- 本机程序的工作目录是 `~/.miyu/var/lib/mcp/<服务名>/`（也在环境变量 `MIYU_STATE_DIR` 里），不进沙盒。标准错误在 `~/.miyu/var/log/mcp/<服务名>.stderr`，起不来先看它。
- 起不来、崩了的隔一会儿重试，连续 5 次停下。改好以后在「MCP」页按「重启」。
- 用 `npx`、`uvx` 起的服务，要先自己装 Node、Python 这些。

## 想做得更多

要一个一直跑着的程序、自己的界面或者后台页，或者要把这些发给别人用，就做成软件包：见 [写一个 Miyu 软件包](../authors/README.md) 和 [写扩展](../authors/extensions.md)。技能、脚本、模板放进包文件夹的 `skills/`、`scripts/`、`commands/`，格式和这一页一样。

也可以让 AI 帮你写。出厂的「写技能和包」带两个技能：

- `skill-creator`（写技能）：照包的样子写一个技能，检查后装上。
- `package-creator`（写脚本和包）：把脚本做成工具放进包里，检查后装上。

在输入框里敲 `$skill-creator` 或 `$package-creator` 接上你要什么，或者直接说「写个技能」「写个工具」，AI 自己会去读。两个技能写出来的都是软件包，装上以后在软件包页里管，见 [软件包](packages.md)。
