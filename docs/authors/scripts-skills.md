# 脚本、技能、命令模板

三样都不用写常驻的程序，放一个文件就能用。英文版：[scripts-skills.en.md](scripts-skills.en.md)。

| | 是什么 | AI 怎么用 |
|---|---|---|
| **脚本** | 一个可执行文件，一件工具 | 和别的工具一样直接调 |
| **技能** | 一份 `SKILL.md`，一套做事的办法 | 平时只看到名字和一句说明，用得上时自己读正文 |
| **命令模板** | 一份 `<名字>.md`，一条斜杠命令 | 人敲 `/<名字>` 时把正文当这个人说的一句发给 AI |

放在哪：做成包的放在包文件夹的 `scripts/`、`skills/`、`commands/` 下面，清单只写一张 `[package]`；只给自己用的直接放进 `~/.miyu/home/admin/scripts/`、`skills/`、`commands/`（同名的这里赢）。放进去下一个会话就看得到；每个脚本、技能、命令模板在预设里各自开关。写错了的不登记，`miyu check` 说哪一个、为什么。

也可以放链接，指到别处的文件、文件夹（例如放在你自己的仓库里统一管）：照链接指到的认，AI 看到的是真实的路径；断了的跳过。指到 Miyu 数据目录里别处的（`system/`、别的账号下）不认，`miyu check` 说指到了哪里。

```text
dice/
├── package.toml          # 只有 [package]
├── scripts/
│   └── dice.py
├── skills/
│   └── take-notes/
│       └── SKILL.md
└── commands/
    └── review.md
```

```toml
[package]
name = { en = "Dice and notes", zh = "骰子和笔记" }
```

## 脚本

一个文件一件工具。工具名照文件名（去掉扩展名，不合 `[A-Za-z0-9_-]` 的换成 `_`），开头的注释写说明和参数。一个能直接用的例子，`dice.py`：

```python
#!/usr/bin/env python3
# Display name: Dice
# 显示名称：掷骰子
# Description: Roll dice and return each face and the total.
# 描述：掷骰子，交回每一颗和总数。
# Timeout: 10
# Parameters:
# {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
import json
import random
import sys

args = json.load(sys.stdin)
count = max(1, min(int(args["count"]), 100))
sides = max(2, min(int(args["sides"]), 1000))
faces = [random.randint(1, sides) for _ in range(count)]
print(f"{count}d{sides}: {faces} = {sum(faces)}")
```

Linux、macOS 上 `chmod +x dice.py`。

**开头的注释**

| 键 | 说明 |
|---|---|
| `Description` | 给模型看的说明，英文短句，必写 |
| `描述` | 给人看的说明 |
| `Display name`、`显示名称` | 给人看的名字（英文、中文），时间线上照它写 |
| `Parameters` | 参数的 JSON Schema，从下一行起，可以写几行；不写是没有参数 |
| `Timeout` | 秒，默认 60 |
| `Need load` | 不写是先加载：AI 平时只看到名字和一行说明，要用时先加载完整的参数，省钱。`no`：一直给完整的参数，常用的写上 |
| `Clicall` | `yes`：AI 在命令里写脚本时也能用 `miyu call` 调它，结果交给脚本。不写是不能，只能直接调。要读、要查，并且 AI 在命令里自己不方便做到的脚本，都该写上：AI 能在脚本里批量调，省下一轮轮来回；会改东西、发东西、花额度的别写 |
| `Venues` | 给哪种会话：`local`（本机的终端、网页这些）、`private`（通讯平台的私聊）、`group`（群）里的几个，逗号隔开，例如 `Venues: local, private`。不写是哪种会话都有；写错的整个脚本不登记，`miyu check` 报出来 |

开头是文件开头连着的那一段注释，遇到第一行不是注释的就完。开头写完空一行再写别的注释，不然说明里一句「Timeout: …」也会被当成开头读。

**怎么跑**

- 照 `#!` 那一行起（Windows 上照扩展名找解释器：`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`）。
- 参数是一份 JSON，标准输入里有完整的一份；不超过 30 KiB 的，环境变量 `MIYU_ARGS_JSON` 里也有一份。长的参数只在标准输入里，所以稳妥的写法是读标准输入。
- 标准输出、标准错误都交给 AI，照写出来的先后；退出码不是 0 的算失败，把原因写进输出。
- 跑得久的想说到哪了：印一整行以 `miyu-progress: ` 开头的，人在界面上看得到，AI 看不到、不进结果。写到标准错误、马上冲出去（标准输出接管道时 Python 整块缓冲，进度会攒到最后才出来）：

  ```python
  import sys
  print("miyu-progress: 3/10 downloaded", file=sys.stderr, flush=True)
  ```

- 脚本工具里调不了 `miyu call`：调工具的通道只开给 AI 直接跑的 `shell` 命令，脚本工具跑起来不开，免得一层套一层。要先调工具、再处理结果的，别做成脚本工具，让 AI 在 `shell` 里跑它（比如放进技能里，正文写清楚怎么跑）。
- 工作目录是会话的工作区；能写缓存的目录在 `MIYU_SCRIPT_CACHE_DIR`（在你的家目录的 `cache/` 里，每个脚本一格，重启也在；30 天没碰的会被清掉，所以只放丢了也能再生成的东西）。
- 要一直留着的（登录态、上一次跑到哪）写进 `MIYU_SCRIPT_STATE_DIR`：每个账号、每个脚本一格，下一次还在；群里、陌生人私聊的会话和你自己的会话不是一个账号，各是各的一格；几个群之间、几个陌生人之间也各是各的（照场所分，同一个群开了新会话还是那一格）。装成包的，卸包时跟着删；自己放在 `scripts/` 里的，删了脚本状态目录还留着，`miyu check` 会提醒、印出在哪。
- 谁调的：`MIYU_CALLER_VENUE` 是在哪种会话里调的（本机的是 `local`，群、私聊是场所编号，例如 `qq:group:123`），`MIYU_CALLER_OWNER` 是这一轮开口的是不是机器主人（`1`、`0`；本机的会话属主是管理员的才是，成员自己的会话不是；群里几个人有一个不是主人的、说不清的都是 `0`）。
- 这两个只是事实，不是门：AI 自己在命令里写 `MIYU_CALLER_OWNER=1 ./脚本` 挡不了。要让脚本只在某几种会话里有，写开头的 `Venues`；环境变量只用来调整行为，比如群里只读、不改东西：

  ```python
  import os
  if os.environ.get("MIYU_CALLER_OWNER") != "1":
      print("Read-only here: only the owner can change the title.")
  ```

- 跑的规矩同 AI 跑命令：在沙盒里跑（只写得了工作区，Miyu 的数据目录读不到，脚本自己那个文件单独放开只读）；沙盒用不了的要人确认。

**交图**

要让 AI 直接看到图（截图、视频抽的帧），把图拷进 `MIYU_SCRIPT_IMAGES_DIR`：每次调用一个新的空目录，跑完 Miyu 收走，再删掉。

```python
import os, shutil
shutil.copy("frame-01.png", os.environ["MIYU_SCRIPT_IMAGES_DIR"])
print("Grabbed 1 frame.")
```

- 照文件名排，至多 4 张；PNG、JPEG、GIF、WebP，一张不超过 5 MiB、每边不超过 8000 像素。
- 只看这一层的普通文件，不跟链接：要交的图请拷进去，别放链接。
- 不合的不交，AI 也不知道有它；运行日志里有一行 `script image dropped`，写着哪个脚本、为什么（不写文件名）。
- 到时被停掉的那一次不交图；退出码不是 0 的照样交。

**要在沙盒外跑的**

碰硬件、要 root（`sudo`）的脚本在沙盒里跑不通。脚本自己要不了，开头写什么都不算：请管理员在系统配置 `system/config.toml` 里放行它，值写它的来处。

```toml
[sandbox.tools]
ceiling-light = "~home"   # 自己放在 scripts/ 里的脚本
lights = "smart-home"     # 包带的，写包的编号
```

- 放行了的，在管理员自己的会话里（终端、网页、主人的 QQ 私聊、桌面语音）直接在沙盒外跑，不问。群聊、陌生人的私聊照旧拒绝，只读的时候也拒绝；高危的命令照样问。
- 来处对不上的不算放行：脚本删了、换成别的包带的同名工具，放行自动失效。`miyu check` 对工具没有了的、来处对不上的各报一条警告，对不上的印出现在的来处；放行了 `shell` 的也提醒一声。
- 放行了就是你本人的全部权限：只放行你看过代码、参数安全的脚本。
- 放行只认放在家目录 `scripts/` 里（或装成包）的真文件：文件是链接、`scripts/` 本身是链接、或者文件落在 AI 这一次能写的地方（比如工作区）的，都不算放行，照旧在沙盒里跑（这些地方 AI 改得了）。`miyu check` 会提醒，并印出文件真正在哪。

## 技能

照公开的 Agent Skills 格式（Claude Code、Codex 这些都认），网上现成的技能放进来就能用。`skills/<名字>/SKILL.md`：

```markdown
---
name: take-notes
description: "Keep meeting notes in notes/YYYY-MM-DD.md: decisions, owners, open questions. Use when the user asks to take or tidy notes, 记笔记."
metadata:
  display_name: Take notes
  summary: Meeting notes in a fixed format
  display_name_zh: 记笔记
  summary_zh: 照固定格式记会议笔记
---

# Taking notes

1. Create `notes/` in the workspace if it doesn't exist.
2. …
```

| 格 | 说明 |
|---|---|
| `name` | 小写字母、数字、`-`，和文件夹同名 |
| `description` | 给模型看：英文，说清是什么、什么时候用；可以带几个人真会说的中文触发词。用双引号包起来（里面有「冒号加空格」的不加引号读不成） |
| `metadata` 的 `display_name`、`summary` | 给人看的名字、一句说明（英文）；中文、日文另写 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja` |

AI 平时只看到名字、说明和 `SKILL.md` 的路径，用得上时自己读正文。文件夹里别的文件（参考资料、带的脚本）她也读得到，改不了；技能里带的脚本不是工具，正文里写清楚怎么跑。

## 命令模板

照 Claude Code 的写法：`commands/<名字>.md`。人敲 `/<名字> 参数` 时，正文当这个人说的一句发给 AI。

```markdown
---
description: Review the changes in the working tree
argument-hint: "[focus]"
metadata:
  summary_zh: 检查工作区里的改动
---

Review the uncommitted changes in this repository. Focus on: $ARGUMENTS
```

- `$ARGUMENTS` 换成名字后面跟的全部字，`$ARGUMENTS[0]` 是第一个（照空白切；带空格的值用英文双引号括起来，`/review "my file.rs"` 的 `$ARGUMENTS[0]` 是 `my file.rs`）；正文里没写 `$ARGUMENTS` 的，参数接在末尾。
- `description` 是命令菜单里那一句（英文），中文、日文写在 `metadata` 的 `summary_zh`、`summary_ja`；`argument-hint` 是名字后面跟什么的提示。
- 名字的写法：小写字母开头，只用小写字母、数字、`-`。和 Miyu 自带的命令同名的不算（自带的赢）。
- 时间线上显示人敲的那一行。谁能用和 Miyu 自带的命令一样。

要程序来办的斜杠命令（查东西、改状态、回一段话），用扩展的 `[[slash]]`，见 [写扩展](extensions.md)。
