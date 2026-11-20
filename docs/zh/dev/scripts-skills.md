# 放文件就能用的：脚本、技能、命令模板、知识库、人格

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这几样都不用写常驻的程序，放文件就能用。这一页是完整的格式，写给要把它们做进软件包的人；只给自己用的，可以先看用户手册的 [扩展 Miyu](../guide/extending.md)，那里按步骤讲。

| | 是什么 | AI 怎么用 |
|---|---|---|
| **脚本** | 一个可执行文件，就是一件工具 | 和其他工具一样，由 AI 直接调用 |
| **技能** | 一份 `SKILL.md`，是一套做事的办法 | AI 平时只看到名字和一句说明，用得上时自己读取正文 |
| **命令模板** | 一份 `<名字>.md`，对应一条斜杠命令 | 人输入 `/<名字>` 时，正文被当作这个人说的一句话发给 AI |
| **知识库** | `kb/<编号>/`：一份 `kb.toml` 和一些文档 | AI 用知识库的工具查询；打开了自动想起的知识库，相关的段落每一轮会自动带上 |
| **人格、预设** | `personas/<编号>/`、`presets/<编号>.toml` | 人在列表里挑选 |

放在哪里：如果做成软件包，就放在软件包文件夹的 `scripts/`、`skills/`、`commands/` 下面，清单里只写一张 `[package]`；如果只给自己用，就直接放进 `~/.miyu/root/scripts/`、`skills/`、`commands/`（同名时以这里的为准）。放进去以后，下一个会话就能看到；每个脚本、技能、命令模板都可以在预设里单独开关。写错了的脚本、和其他工具重名的脚本不会登记，`miyu check` 会指出是哪一个、为什么；如果刚放进去就检查，`miyu check` 提示核心还没有读到，过一会儿再检查即可。

也可以放链接，指向其他位置的文件或文件夹（例如统一放在你自己的仓库里管理）：Miyu 以链接指向的文件为准，AI 看到的是真实的路径；断了的链接会被跳过。指向 Miyu 数据目录里其他位置的链接（例如本机令牌、密钥文件）不被接受，`miyu check` 会指出它指向了哪里。

```text
dice/
├── package.toml          # 清单里只有 [package]
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
version = "0.1.0"           # 安装时可以不写，打包、发布到软件源时必须写
name = { en = "Dice and notes", zh = "骰子和笔记" }
```

## 脚本

一个文件就是一件工具。工具名取自文件名（去掉扩展名，不符合 `[A-Za-z0-9_-]` 的字符换成 `_`），文件开头的注释写说明和参数。下面是一个可以直接使用的例子 `dice.py`：

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

第一行写了 `#!` 的脚本不需要 `chmod +x`：Miyu 启动的是解释器，并把脚本文件作为参数交给它。没有写 `#!` 的脚本在 Linux、macOS 上直接启动，需要先 `chmod +x`。

**开头的注释**

| 键 | 说明 |
|---|---|
| `Description` | 给模型看的说明，用英文短句写，必写。 |
| `描述` | 给人看的说明。 |
| `Display name`、`显示名称` | 给人看的名字（分别是英文、中文），时间线上显示的就是它。 |
| `Parameters` | 参数的 JSON Schema，从下一行开始写，可以写好几行；不写表示没有参数。 |
| `Timeout` | 超时的秒数，默认 60。 |
| `Clicall` | 写 `yes` 或 `true`（不区分大小写）时，AI 在命令里写脚本时也能用 `miyu call` 调用它，结果交给那个脚本。写 `no`、`false` 或者不写时不能这样调用，只能直接调用；写其他值时，整个脚本都不登记。需要读取、查询，并且 AI 在命令里自己不方便做到的脚本，都应该写上：这样 AI 能在脚本里批量调用，省下一轮又一轮的来回；会修改东西、发送东西、花费额度的脚本不要写。 |
| `Venues` | 这件工具给哪几种会话使用：从 `local`（本机的终端、网页等）、`private`（通讯平台的私聊）、`group`（群聊）里选几个，用逗号隔开，例如 `Venues: local, private`。不写表示所有会话都有；写错时整个脚本都不登记，`miyu check` 会报告出来。 |
| `Role` | 工具的职能，写法和扩展的 `role` 相同：`read_file` 表示这件工具用来读取文本文件。写错时整个脚本都不收录：值写错算写错，键名写成 `role`、`Rloe` 这种和它很像的（大小写不对、差一个字）也算写错。 |
| `Scope` | 给哪种会话使用，写法和扩展的 `scope` 相同（`local`、`local_main`、`local_main_attended`、`spawn`），在 `Venues` 的范围之内再收窄；不写时是 `any`，写错时整个脚本都不收录（键名写成 `scope`、`Scpoe` 这种和它很像的也算写错）。 |

开头指的是文件最开头连在一起的那一段注释（以 `#` 或 `//` 开头的行），遇到第一行不是注释的就结束。开头写完以后，要先空一行再写其他注释，否则说明里的一句「Timeout: …」也会被当成开头来读。

**怎么运行**

- 有 `#!` 的脚本按那一行启动：启动解释器，并把脚本文件作为参数（在 Windows 上，取 `#!` 里解释器的名字在 `PATH` 里查找，遇到 `env` 时取它后面的那个名字，`python3` 换成 `python`）。没有 `#!` 的脚本，在 Linux、macOS 上直接启动文件；在 Windows 上根据扩展名查找解释器：`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`，`.cmd`、`.bat` 用 `cmd /C`，其他的直接启动。
- 参数是一份 JSON，标准输入里有完整的一份；不超过 30 KiB 时，环境变量 `MIYU_ARGS_JSON` 里也有一份。长的参数只在标准输入里，所以稳妥的写法是读取标准输入。
- 标准输出、标准错误都按写出的先后顺序交给 AI；退出码不是 0 的算作失败，失败时要把原因写进输出。
- 运行时间长的脚本如果想报告进度，可以输出一整行以 `miyu-progress: ` 开头的文字：人在界面上能看到它，AI 看不到，它也不进入结果。这一行要写到标准错误，并且立即刷新输出（因为标准输出接管道时 Python 会整块缓冲，进度会攒到最后才出来）：

  ```python
  import sys
  print("miyu-progress: 3/10 downloaded", file=sys.stderr, flush=True)
  ```

- 脚本工具里无法调用 `miyu call`：调用工具的通道只对 AI 直接运行的 `shell` 命令开放，脚本工具运行时不开放，以免一层套一层。如果要先调用工具、再处理结果，不要做成脚本工具，而是让 AI 在 `shell` 里运行它（例如放进技能里，在正文里写清楚怎么运行）。
- 工作目录是会话的工作区；可以写缓存的目录在 `MIYU_CACHE_DIR`（位于数据根的 `var/cache/scripts/` 里，每个脚本一个目录，重启以后仍然在；30 天没有动过的会被清掉，所以只放丢了也能重新生成的东西）。
- 给人的文件（画的图、下载的视频、生成的文档）存进你家目录里的那几个目录，人在终端、网页里能找到它们：`MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR`，这些目录在脚本运行之前已经建好、可以写入。只有你自己（本机）的会话才有这些变量；群聊、陌生人私聊的会话没有，在那里仍然写缓存目录或者工作区，所以使用之前先检查变量是否存在。
- 需要一直保留的数据（登录状态、上一次运行到了哪里）写进 `MIYU_STATE_DIR`：每个脚本一个目录，下一次运行时还在；本机的会话共用一个目录，群聊、陌生人的私聊按场所各用各的目录（同一个群开了新会话，用的还是那一个目录）。做成软件包安装的脚本，卸载软件包时状态目录跟着删除；自己放在 `scripts/` 里的脚本，删除脚本以后状态目录仍然保留，`miyu check` 会提醒，并输出它的位置。
- 做成软件包的脚本另外有 `MIYU_APP_DIR`：它指向脚本所在的软件包文件夹（清单和软件包带的其他文件都在里面），是只读的；自己放在 `scripts/` 里的脚本没有这个变量。
- 旧的名字 `MIYU_SCRIPT_CACHE_DIR`、`MIYU_SCRIPT_STATE_DIR`、`MIYU_SCRIPT_IMAGES_DIR`、`MIYU_PACKAGE_DIR` 在过渡期间仍然提供，指向同样的位置，下一个版本会去掉；新写的脚本要使用上面的名字。
- 调用者是谁：`MIYU_CALLER_VENUE` 表示在哪种会话里调用（本机的是 `local`，群聊、私聊是场所编号，例如 `qq:group:123`）；`MIYU_CALLER_OWNER` 表示这一轮说话的是不是机器的主人（值是 `1` 或 `0`；本机的会话里是主人；群里几个人中有一个不是主人的，或者无法确定的，都是 `0`）。
- 这一轮使用的模型：`MIYU_MODEL` 是模型名，`MIYU_EFFORT` 是思考强度（配置里没有写时是 `default`）；`MIYU_SESSION_LOG` 是这个会话的日志目录，只有机器主人自己的会话才有（因为其他人看不到运行 Miyu 的那台电脑）。还没有确定模型的调用、会话之外的调用都没有这几个变量，所以使用之前先检查它们是否存在。
- 这两个变量只是事实，不是门禁：AI 自己在命令里写 `MIYU_CALLER_OWNER=1 ./脚本` 时，它们挡不住。如果要让脚本只在某几种会话里出现，就在开头写 `Venues`；环境变量只用来调整脚本的行为，例如在群里只读、不修改东西：

  ```python
  import os
  if os.environ.get("MIYU_CALLER_OWNER") != "1":
      print("Read-only here: only the owner can change the title.")
  ```

- 脚本软件包的配置项：做成软件包的脚本，可以在清单里写 `[settings.<名字>]`，规则和扩展包相同（类型、`default`、`applies`、`name`、`description`、`hidden`；`secret` 不能写默认值），人在设置的「软件包」页里这个软件包的那一组中填写。软件包里 `scripts/` 下的脚本运行时，环境变量 `MIYU_SETTINGS_JSON` 里是这几项当前的值，是一个 JSON 对象：键去掉了这个软件包的配置前缀（清单里的 `[package] prefix`，没有写时就是软件包编号），有分组的写成 `组.名字`；`secret` 是解出来的真实值；每一项是一组字段的列表（`element = "record"`）时，值是对象的数组，字段里的 `secret` 同样是真实值，没有写的字段使用默认值；没有填写、也没有默认值的键不会放进去，所以使用之前先检查键是否存在。每次调用都会重新读取，人修改以后，下一次调用拿到的就是新值。只有这个软件包自己的脚本才有这个变量：其他软件包的脚本、自己放在 `scripts/` 里的脚本、AI 运行的命令都没有。

  ```python
  import json, os
  settings = json.loads(os.environ.get("MIYU_SETTINGS_JSON", "{}"))
  key = settings.get("api_key")
  if not key:
      print("Set the API key in Settings → Packages first.")
  ```

- 运行的规则和 AI 运行命令相同：是否进入沙盒，见下面的「什么时候在沙盒里运行」；进入了沙盒的脚本读不到 Miyu 的数据目录，只有脚本自己那个文件单独开放为只读。

**交图**

如果要让 AI 直接看到图片（截图、从视频里抽出的帧），把图片拷贝进 `MIYU_IMAGES_DIR`：每次调用都是一个新的空目录，运行完以后 Miyu 收走里面的图片，然后删除这个目录。

```python
import os, shutil
shutil.copy("frame-01.png", os.environ["MIYU_IMAGES_DIR"])
print("Grabbed 1 frame.")
```

- 图片按文件名排序，最多 4 张；格式可以是 PNG、JPEG、GIF、WebP。一张图片超过 5 MiB，或者有一边超过 8000 像素时，会先缩小到上限以内再交给 AI；无法缩小的、文件超过 64 MiB 的不交。
- 只看这一层目录里的普通文件，不跟随链接：要交的图片请拷贝进去，不要放链接。
- 不符合要求的图片不交，AI 也不知道有这张图；运行日志里会有一行 `script image dropped`，写明是哪个脚本、为什么（不写文件名）。
- 因为超时被停掉的那一次调用不交图片；退出码不是 0 的仍然照常交图片。

**什么时候在沙盒里运行**

- 你自己的会话（终端、网页、你的 QQ 私聊、桌面语音）在完全放开（默认）的权限下不进入沙盒，脚本以你本人的权限运行。
- 打开只读以后，脚本在只读的沙盒里运行：只能写入它自己的状态目录、缓存目录、交图目录（`MIYU_STATE_DIR`、`MIYU_CACHE_DIR`、`MIYU_IMAGES_DIR`），本机的会话另外可以写入数据根 `root/` 下的 `pictures`、`videos`、`documents`、`downloads`，其他位置都不能写入；在群聊、陌生人的私聊里，脚本在沙盒里运行，只能写入那个场所自己的工作区。需要访问硬件、需要 root（`sudo`）的脚本在那里无法正常运行，也没有办法单独放行某一个脚本。

## 技能

技能使用公开的 Agent Skills 格式（Claude Code、Codex 等都支持），网上现成的技能放进来就能用。在设置里打开「读取其他目录的 Skills」（`skills.external.enabled`）以后，其他客户端安装到 `~/.agents/skills/` 的技能 Miyu 也会识别（目录列表 `skills.external.dirs` 可以增删），它们排在你自己放的、软件包带的技能后面，同名时以 Miyu 自己的为准；Miyu 不会删除那里的技能。技能放在 `skills/<名字>/SKILL.md`：

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
| `name` | 只用小写字母、数字、`-`，和文件夹同名（不同名时按写的名字使用，没有写或者写错时按文件夹的名字使用，这两种情况 `miyu check` 都会各报一条警告）。 |
| `description` | 给模型看的说明：用英文写，说清楚是什么、什么时候用；可以带几个人真正会说的中文触发词。要用双引号包起来（因为里面有「冒号加空格」时，不加引号就不符合 YAML：Miyu 会把它当作一整句文字重新读取，能读出来，但会报警告，其他软件不一定读得出来）；最多 1024 个字，超出的部分截掉。 |
| `metadata` 的 `display_name`、`summary` | 给人看的名字和一句说明（英文）；中文、日文另外写在 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja` 里。 |
| `disable-model-invocation` | 写 `true` 时，只有人点名才能使用：AI 看不到它，也不会自己挑选（适合发版、删除东西这类不想让 AI 自己发起的事）。 |
| `user-invocable` | 写 `false` 时，人无法点名使用，它也不出现在命令菜单里，只给 AI 自己使用（适合背景知识这类内容）。 |

AI 平时只看到技能的名字、说明和 `SKILL.md` 的路径，用得上时自己读取正文。人也可以在输入框里用 `$技能名` 点名，一句话里可以点名好几个：被点名的技能正文排在这句话前面，直接交给 AI。文件夹里的其他文件（参考资料、附带的脚本）AI 也读得到，但是改不了；技能里附带的脚本不是工具，要在正文里写清楚怎么运行。

## 命令模板

命令模板使用 Claude Code 的写法：`commands/<名字>.md`。人输入 `/<名字> 参数` 时，正文被当作这个人说的一句话发给 AI。

```markdown
---
description: Review the changes in the working tree
argument-hint: "[focus]"
metadata:
  summary_zh: 检查工作区里的改动
---

Review the uncommitted changes in this repository. Focus on: $ARGUMENTS
```

- `$ARGUMENTS` 会换成名字后面跟着的全部文字，`$ARGUMENTS[0]` 是第一个参数（按空白切分；带空格的值用英文双引号括起来，例如 `/review "my file.rs"` 的 `$ARGUMENTS[0]` 是 `my file.rs`）；如果正文里没有写 `$ARGUMENTS`，参数接在正文末尾。
- `description` 是命令菜单里显示的那一句（英文），中文、日文写在 `metadata` 的 `summary_zh`、`summary_ja` 里；`argument-hint` 用来提示名字后面该跟什么。
- 名字的写法：以小写字母开头，只用小写字母、数字、`-`。和 Miyu 自带的命令同名的模板不生效（以自带的命令为准）。
- 时间线上显示人输入的那一行。谁能使用命令模板，和 Miyu 自带的命令一样。

如果斜杠命令需要由程序来处理（查询东西、修改状态、回复一段话），就使用扩展的 `[[slash]]`，详细的见 [写扩展](extensions.md)。

## 知识库

知识库是一套给 AI 查询的资料（设定集、产品手册、笔记）。软件包文件夹里的 `kb/<编号>/` 就是一个知识库，不需要写进清单；编号的写法和软件包的编号相同。

```text
lighthouse/
├── package.toml          # 清单里只有 [package]
└── kb/
    └── lighthouse/
        ├── kb.toml
        └── people.md
```

```toml
# kb/lighthouse/kb.toml，这个文件可以没有
name = "Lighthouse lore"                               # 名字，没有写时使用编号
description = "People, places and rules of the lighthouse story"   # 用一句话说明这个库是什么
auto = true                                            # 自动想起，不写时是关闭的
```

- 知识库的名字和说明会列进 AI 的系统提示词，让 AI 知道有这个库、什么时候去查询，所以要写得简短。`kb.toml` 只识别这三个字段，写错的字段按没有写处理，`miyu kb list` 里这个库会带着错误信息。
- `auto = true` 表示每一轮根据人说的话，从这个库里带上相关的段落，不需要 AI 自己去查询。人在知识库页面里可以修改这个开关，修改记录在人那一边。
- 文档：支持 `.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`，子目录里的文件同样算数；以点开头的文件、链接不算。文字文件不超过 4 MiB，并且要求是 UTF-8；PDF、网页文件不超过 64 MiB。读取失败的文件会被跳过，运行日志里会有一行记录。
- 关键词：在 Markdown 一节标题下面的第一行写 `关键词：守灯人、灯塔的姑娘、Eileen`（也接受 `keys:`，`、`、`，`、`,` 都可以用来分隔）；在打开了自动想起的库里，人提到其中任何一个关键词，就会带上这一节。
- 只读：AI 无法写入软件包里的知识库，人也不能修改它的名字、说明和文档（重建索引仍然可以）。卸载软件包时，知识库和它的索引一起删除。
- 如果人自己有同编号的知识库，以人的那一个为准；和其他软件包带的知识库编号相同时，软件包无法安装（`package_conflict`）。
- 安装以后，`miyu kb list` 里能看到这个知识库（写着 `来自 <包名>`），可以用 `miyu kb search <词>` 试着搜索。

## 人格和预设

- **人格**：`personas/<编号>/`，和人在 Miyu 里创建的人格结构一样：`persona.toml`（`[persona]` 里的 `name`、`summary` 等）、`prompts/persona.md`（人设）、`prompts/examples.md`（示范对话）、`prompts/reminders.md`（角色扮演提示），这些文件都可以没有。最省事的做法是：先在终端或网页里把人格创建好、调好，再把数据根里的 `root/personas/<编号>/` 整个拷贝进软件包的 `personas/`。人设、示范对话是人格自己的文字，用什么语言写都可以。
- **预设**：`presets/<编号>.toml`，同样先在 Miyu 里创建好，再从 `root/presets/` 拷贝过来。
- 只读：人修改的内容写进人自己那一层，覆盖在软件包的上面；卸载软件包时，软件包里的人格、预设会被删除，人修改过的内容保留下来，`miyu check` 会提示一句。
- 编号和出厂自带的、其他软件包带的、人自己已有的人格或预设冲突时，软件包无法安装（`package_conflict`）。
