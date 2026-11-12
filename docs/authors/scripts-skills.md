# 放文件就能用的：脚本、技能、命令模板、知识库、人格

这几样都不用写常驻的程序，放文件就能用。英文版：[scripts-skills.en.md](scripts-skills.en.md)。

| | 是什么 | AI 怎么用 |
|---|---|---|
| **脚本** | 一个可执行文件，一件工具 | 和别的工具一样直接调 |
| **技能** | 一份 `SKILL.md`，一套做事的办法 | 平时只看到名字和一句说明，用得上时自己读正文 |
| **命令模板** | 一份 `<名字>.md`，一条斜杠命令 | 人敲 `/<名字>` 时把正文当这个人说的一句发给 AI |
| **知识库** | `kb/<编号>/`：一份 `kb.toml` 和一些文档 | 用知识库的工具查；开了自动想起的，相关的段落每一轮自己带上 |
| **人格、预设** | `personas/<编号>/`、`presets/<编号>.toml` | 人在列表里挑 |

放在哪：做成包的放在包文件夹的 `scripts/`、`skills/`、`commands/` 下面，清单只写一张 `[package]`；只给自己用的直接放进 `~/.miyu/root/scripts/`、`skills/`、`commands/`（同名的这里赢）。放进去下一个会话就看得到；每个脚本、技能、命令模板在预设里各自开关。写错了的、和别的工具重名的脚本不登记，`miyu check` 说哪一个、为什么；刚放进去就查、说核心还没读到的，过一会儿再查。

也可以放链接，指到别处的文件、文件夹（例如放在你自己的仓库里统一管）：照链接指到的认，AI 看到的是真实的路径；断了的跳过。指到 Miyu 数据目录里别处的（比如本机令牌、密钥文件）不认，`miyu check` 说指到了哪里。

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
version = "0.1.0"           # 装可以不写，打包、发到软件源必写
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
| `Need load` | 以前的写法，照认、不起作用：工具一律一开始就给完整的参数 |
| `Clicall` | `yes`：AI 在命令里写脚本时也能用 `miyu call` 调它，结果交给脚本。不写是不能，只能直接调。要读、要查，并且 AI 在命令里自己不方便做到的脚本，都该写上：AI 能在脚本里批量调，省下一轮轮来回；会改东西、发东西、花额度的别写 |
| `Venues` | 给哪种会话：`local`（本机的终端、网页这些）、`private`（通讯平台的私聊）、`group`（群）里的几个，逗号隔开，例如 `Venues: local, private`。不写是哪种会话都有；写错的整个脚本不登记，`miyu check` 报出来 |
| `Role` | 职能，写法同扩展的 `role`：`read_file` 表示这件是读文本文件的。写错的整个不收：值写错的，键名写成 `role`、`Rloe` 这样像它的（大小写不对、差一个字）也算 |
| `Scope` | 给哪种会话，写法同扩展的 `scope`（`local`、`local_main`、`local_main_attended`、`spawn`），在 `Venues` 之内再收窄；不写是 `any`，写错的整个不收（键名写成 `scope`、`Scpoe` 这样像它的也算） |

开头是文件开头连着的那一段注释（`#` 或 `//` 开头的行），遇到第一行不是注释的就完。开头写完空一行再写别的注释，不然说明里一句「Timeout: …」也会被当成开头读。

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
- 工作目录是会话的工作区；能写缓存的目录在 `MIYU_CACHE_DIR`（在数据根的 `var/cache/scripts/` 里，每个脚本一格，重启也在；30 天没碰的会被清掉，所以只放丢了也能再生成的东西）。
- 给人的文件（画的图、下载的视频、生成的文档）存进你家目录的那几处，人在终端、网页里找得到：`MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR`，跑之前建好、能写。只有你自己（本机）的会话给；群里、陌生人私聊的会话没有，那里照旧写缓存目录或工作区，所以用之前先看变量在不在。
- 要一直留着的（登录态、上一次跑到哪）写进 `MIYU_STATE_DIR`：每个脚本一格，下一次还在；本机的会话共用一格，群、陌生人的私聊照场所各是各的一格（同一个群开了新会话还是那一格）。装成包的，卸包时跟着删；自己放在 `scripts/` 里的，删了脚本状态目录还留着，`miyu check` 会提醒、印出在哪。
- 做成包的脚本另有 `MIYU_APP_DIR`：它所在的包文件夹（清单、包带的别的文件都在里面），只读；`scripts/` 里自己放的没有。
- 旧名字 `MIYU_SCRIPT_CACHE_DIR`、`MIYU_SCRIPT_STATE_DIR`、`MIYU_SCRIPT_IMAGES_DIR`、`MIYU_PACKAGE_DIR` 过渡期间照样给、指到同一处，下一版去掉，新写的照上面的名字。
- 谁调的：`MIYU_CALLER_VENUE` 是在哪种会话里调的（本机的是 `local`，群、私聊是场所编号，例如 `qq:group:123`），`MIYU_CALLER_OWNER` 是这一轮开口的是不是机器主人（`1`、`0`；本机的会话是；群里几个人有一个不是主人的、说不清的都是 `0`）。
- 这一轮用的模型：`MIYU_MODEL` 是模型名，`MIYU_EFFORT` 是思考强度（配置里没写的是 `default`）；`MIYU_SESSION_LOG` 是这个会话日志的目录，只有机主自己的会话有（别人看不到运行 Miyu 的那台电脑）。还没定下模型的、会话外面的调用都没有这几个，用之前先看在不在。
- 这两个只是事实，不是门：AI 自己在命令里写 `MIYU_CALLER_OWNER=1 ./脚本` 挡不了。要让脚本只在某几种会话里有，写开头的 `Venues`；环境变量只用来调整行为，比如群里只读、不改东西：

  ```python
  import os
  if os.environ.get("MIYU_CALLER_OWNER") != "1":
      print("Read-only here: only the owner can change the title.")
  ```

- 脚本包的配置项：做成包的，清单里可以写 `[settings.<名字>]`，规矩同扩展包（类型、`default`、`applies`、`name`、`description`、`hidden`，`secret` 不能写默认值），人在设置的「软件包」页这个包那一组里填。包里 `scripts/` 下的脚本跑的时候，环境变量 `MIYU_SETTINGS_JSON` 里是这几项现在的值，一个 JSON 对象：键去掉这个包的配置前缀（清单 `[package] prefix`，没写就是包编号），有组的写 `组.名字`；`secret` 是解出来的真值；每一项一组字段的列表（`element = "record"`）是对象的数组，字段里的 `secret` 同样是真值，没写的字段照默认值；没填、也没默认值的键不放，所以用之前先看在不在。每次调用都现读，人改了下一次调用就是新的。只给这个包自己的脚本：别的包的、`scripts/` 里自己放的、AI 跑的命令都没有。

  ```python
  import json, os
  settings = json.loads(os.environ.get("MIYU_SETTINGS_JSON", "{}"))
  key = settings.get("api_key")
  if not key:
      print("Set the API key in Settings → Packages first.")
  ```

- 跑的规矩同 AI 跑命令：进不进沙盒见下面「什么时候在沙盒里跑」；进了沙盒的，Miyu 的数据目录读不到，脚本自己那个文件单独放开只读。

**交图**

要让 AI 直接看到图（截图、视频抽的帧），把图拷进 `MIYU_IMAGES_DIR`：每次调用一个新的空目录，跑完 Miyu 收走，再删掉。

```python
import os, shutil
shutil.copy("frame-01.png", os.environ["MIYU_IMAGES_DIR"])
print("Grabbed 1 frame.")
```

- 照文件名排，至多 4 张；PNG、JPEG、GIF、WebP，一张不超过 5 MiB、每边不超过 8000 像素。
- 只看这一层的普通文件，不跟链接：要交的图请拷进去，别放链接。
- 不合的不交，AI 也不知道有它；运行日志里有一行 `script image dropped`，写着哪个脚本、为什么（不写文件名）。
- 到时被停掉的那一次不交图；退出码不是 0 的照样交。

**什么时候在沙盒里跑**

- 你自己的会话（终端、网页、你的 QQ 私聊、桌面语音）在完全放开（默认）下不进沙盒，脚本以你本人的权限跑。
- 打开只读以后，脚本在只读的沙盒里跑：哪里都写不了；群聊、陌生人的私聊里，脚本在沙盒里跑，只写得了那个场所自己的工作区。碰硬件、要 root（`sudo`）的脚本在那里跑不通，没有办法单独放行某个脚本。

## 技能

照公开的 Agent Skills 格式（Claude Code、Codex 这些都认），网上现成的技能放进来就能用。设置里打开「读取其他目录的 Skills」（`skills.external.enabled`），别的客户端装进 `~/.agents/skills/` 的技能 Miyu 也认（目录表 `skills.external.dirs` 能增删），排在自己放的、包带的后面，同名的照 Miyu 自己的；那里的不经 Miyu 删。`skills/<名字>/SKILL.md`：

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
| `name` | 小写字母、数字、`-`，和文件夹同名（不同名的照写的名字用，没写、写错的照文件夹的名字用，`miyu check` 各报一条警告） |
| `description` | 给模型看：英文，说清是什么、什么时候用；可以带几个人真会说的中文触发词。用双引号包起来（里面有「冒号加空格」的不加引号不合 YAML：Miyu 照一整句字重读能读成，但会报警告，别的软件不一定读得成）；最多 1024 个字，多的截掉 |
| `metadata` 的 `display_name`、`summary` | 给人看的名字、一句说明（英文）；中文、日文另写 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja` |
| `disable-model-invocation` | 写 `true` 的只能人点名用：AI 看不到它，不会自己挑（发版、删东西这种不想让她自己起头的） |
| `user-invocable` | 写 `false` 的人点名用不了、不进命令菜单，只给 AI 自己用（背景知识这种） |

AI 平时只看到名字、说明和 `SKILL.md` 的路径，用得上时自己读正文。人也能在输入框用 `$技能名` 点名，一句话里能点几个：正文排在这句话前面直接交给她。文件夹里别的文件（参考资料、带的脚本）她也读得到，改不了；技能里带的脚本不是工具，正文里写清楚怎么跑。

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

## 知识库

一套给她查的资料（设定集、产品手册、笔记）。包文件夹里 `kb/<编号>/` 是一个知识库，清单里不写；编号写法同包的编号。

```text
lighthouse/
├── package.toml          # 只有 [package]
└── kb/
    └── lighthouse/
        ├── kb.toml
        └── people.md
```

```toml
# kb/lighthouse/kb.toml，可以没有
name = "Lighthouse lore"                               # 名字，没写的照编号
description = "People, places and rules of the lighthouse story"   # 一句话说这个库是什么
auto = true                                            # 自动想起，不写是关
```

- 名字、说明会列进她的系统提示词，让她知道有这个库、什么时候去查：写短。`kb.toml` 只认这三格，写错的照没写用，`miyu kb list` 里这个库带着错。
- `auto = true`：每一轮照人说的话从这个库里带上相关的段落，不用她自己去查。人在知识库页里能改开关，改的记在人那边。
- 文档：`.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`，子目录里的照样算；点开头的、链接不算。文字的不超过 4 MiB、要是 UTF-8；PDF、网页的不超过 64 MiB。读不成的跳过，运行日志里有一行。
- 关键词：Markdown 一节标题下面的头一行写 `关键词：守灯人、灯塔的姑娘、Eileen`（`keys:` 也认，`、`、`，`、`,` 都能分），开了自动想起的库里，人提到其中哪一个就带上这一节。
- 只读：她写不进包里的库，人也改不了名字、说明、文档（重建索引照做）。卸包时库和它的索引一起没了。
- 人自己有同编号的库，人的那一个赢；和别的包带的同编号的装不上（`package_conflict`）。
- 装上以后 `miyu kb list` 看得到（写着「来自 <包名>」），`miyu kb search <词>` 试搜。

## 人格和预设

- **人格**：`personas/<编号>/`，和人在 Miyu 里建的人格一个样：`persona.toml`（`[persona]` 的 `name`、`summary` 这些）、`prompts/persona.md`（人设）、`prompts/examples.md`（示范对话）、`prompts/reminders.md`（角色扮演提示），都可以没有。最省事的做法：先在终端或网页里把人格建好、调好，再把数据根里的 `root/personas/<编号>/` 整个拷进包的 `personas/`。人设、示范对话是人格自己的字，用什么语言都行。
- **预设**：`presets/<编号>.toml`，同样先在 Miyu 里建好，从 `root/presets/` 拷过来。
- 只读：人改了写进人自己那一层、盖在包的上面；卸包时包里的没了，人改过的留着，`miyu check` 提一句。
- 编号和出厂的、别的包带的、人自己已有的撞了的装不上（`package_conflict`）。
