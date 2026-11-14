# 从 0.7 旧版迁过来

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

用过旧版 Miyu（0.7.x）、装了这一版的，照这一页把以前的人格、会话、记忆、设置和文件搬过来。

路径里的 `~/.miyu` 是数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的照它）。

## 先要知道的

- 这一版不认旧版的数据目录，也不会自己搬。搬靠软件包 `migrate` 带的命令 `miyu migrate`，要先装。
- 旧版的数据目录默认也在 `~/.miyu`，和这一版的数据根撞在一处。旧目录还在那里时，这一版的核心起不来。所以第一步是把旧目录挪开。
- 迁移不改、不删旧目录。它先把旧目录整个打成一个压缩包 `miyu-0.7.tar.zst`，逐个文件核对，之后只从解出来的副本读。压缩包放在 `~/.miyu/root/documents/旧版 Miyu 迁移/<时刻>/`，卸了 `migrate` 也不删。
- 没有「先看看、不动手」的模式。动手前会查旧版停了没有、盘够不够，不满足的什么都不写。
- 能重跑。已经迁过的不会多出一份，见下面「重跑和中途失败」。

支持 Linux x86_64、macOS（M 系列）、Windows x86_64。Linux aarch64 上没有这个包，`miyu pkg install migrate` 报「不支持本机的系统」。

## 步骤

1. 关掉旧版：它的窗口和后台进程都退出。旧版还在跑的，迁移会拒绝开始。

2. 挪开旧目录：

   ```bash
   mv ~/.miyu ~/.miyu-0.7
   ```

   ```powershell
   Rename-Item $HOME\.miyu .miyu-0.7
   ```

3. 装这一版，见 [安装、升级、卸载](install.md)。AUR 上从旧的 `miyu` 包升级上来的已经装好了。

4. 接上模型：

   ```sh
   miyu setup
   ```

   不接也能迁：旧版配的供应商、key、默认模型会补进来。但迁记忆那一步要让 AI 整理一次，整理要用模型；那时还没有模型的，这一步跳过，报告里写「整理没成」，迁完再跑 `miyu memory dream`。怎么接见 [模型](models.md)。

5. 想一起迁过来的，先装对应的软件包（不装的以后也能补，见下面的表）：

   ```sh
   miyu pkg install im          # QQ 消息记录
   miyu pkg install memes       # 表情包
   miyu pkg install im-sponsor  # 赞助记录
   miyu pkg install goofish     # 闲鱼的登录态
   ```

6. 装迁移的包：

   ```sh
   miyu pkg install migrate
   ```

7. 迁：

   ```bash
   miyu migrate ~/.miyu-0.7
   ```

   ```powershell
   miyu migrate $HOME\.miyu-0.7
   ```

   核心没在跑的先拉起来。成了印两行：

   ```text
   报告：/home/me/.miyu/root/documents/旧版 Miyu 迁移/20261114T083000Z/报告.md
   给 agent 的：/home/me/.miyu/root/documents/旧版 Miyu 迁移/20261114T083000Z/给agent.json
   ```

   印的是全路径，上面是 Linux 上的样子；macOS、Windows 上印的是那个系统上数据根的全路径。

8. 读 `报告.md`，照下面「看报告」一节核对。

`miyu migrate` 只收一个参数：

| 写法 | 做什么 |
|---|---|
| `miyu migrate <旧目录>` | 把 `<旧目录>` 迁进这一版的数据根 |
| `miyu migrate -h`、`--help` | 印用法 |

参数个数不对的印用法，退出码 `2`。迁进哪个数据根照 `MIYU_HOME`，没设的是 `~/.miyu`。

## 迁什么、变成什么

| 东西 | 从旧版的哪里来 | 到这一版变成 | 要先装的包 |
|---|---|---|---|
| 人格 | 内置的 Miyu、开发模式，`prompts/` 里自己写的人设 | 内置的 Miyu 新建成名字叫「Miyu」的人格，用旧版的人设原文，你改过的角色扮演提示、示范对话盖在上面；开发模式对上出厂的「软件工程师」（`software-engineer`）；自己写的每个新建一个。这一版已经有同名人格的不建，直接用它 | |
| 会话 | 会话库 `conversation.db` | 一个会话一个只读存档：能看、能搜，不能接着聊，在会话列表的「存档」一组里（见 [会话](sessions.md)）。字、工具调用和结果、附件、AI 问你的问题、你中途插的话都在；子代理的会话作子会话一起导；旧版压缩时挤出去的轮插回原来的会话，原会话不在了的归进「旧版被挤出的对话」 | |
| QQ 上的会话 | 会话库里绑了 QQ 群、私聊的会话 | 那个群、私聊的只读存档。群里的一轮照每行开头的号拆成各人说的话 | |
| QQ 消息记录 | `history.sqlite3` | 转成 `qq-records.jsonl` 放进这一次的目录，再交 `miyu im records import` 导进 im 的聊天记录 | `im`：没装的，报告写装上以后跑哪一行 |
| 记忆 | 每个人格的 `memory.db` 里的知识点 | 导进对上的那个新人格，原来记下的时刻留着。说的是别人的记成那个人的，说的是你的记成你的。导完整理一次，合掉和已经记着的重复的 | |
| 配置和密钥 | `config.jsonc`、关于你的那份档案 | 供应商（地址、key、模型的窗口、温度、价格），默认模型（`models.chat`）、看图的模型（`models.vision`），远程的语义模型（`models.embedding`），三个挡位的模型进 `lite`、`normal`、`flagship` 三个池，Tavily、Exa 的 key 和 SearXNG 的地址，关于你（`profile.about`），旧版的管理员 QQ 号认成你本人。只补这一版没有的，已经配过的不动。key 存进密钥文件 | |
| 表情包 | 自己的表情包库，每个一个文件夹 | 每个库做成一个图包 `legacy-memes-<库名>`，装上 | `memes`：没装的，图包留在这一次的目录里，报告写装上以后跑哪一行 |
| 技能 | 自己写的技能 | 拷进 `~/.miyu/root/skills/<名字>/`。同名、内容不一样的改名 `<名字>-legacy`。原来停用的，这一版是开着的 | |
| 知识库 | 知识库里自己放的文档 | 拷进一个新库「旧版知识库」（`~/.miyu/root/kb/legacy/`），这一版自己建索引。同名、内容不一样的改名 `<名>-legacy.<扩展名>` | |
| 文件 | 图片、文档、工具的产物、工作目录 | 图片进 `~/.miyu/root/pictures/`；文档进 `~/.miyu/root/documents/`；产物照会话分好进 `~/.miyu/root/documents/旧版产物/`；工作目录进 `~/.miyu/root/workspace/旧版/`。同名、内容不一样的改名 `<名> (旧版 N).<后缀>` | |
| 闲鱼的登录态 | 闲鱼搜索的登录文件 | 拷进 `goofish` 包的状态目录。新包认不认旧格式没保证，不认的照新包的说明重新登录 | `goofish`：没装的不拷 |
| 赞助记录 | 会话库里的赞助记录 | 导进「赞助记录」插件的账本，原来的时刻、金额、汇率照旧 | `im-sponsor`：没装的，装上以后重跑迁移 |

拷过去的文件逐个照 SHA-256 核对。

## 不迁的

都留在压缩包里，报告逐样写在压缩包的哪个路径、是什么格式。要的时候，让 AI 读同一目录里的 `给agent.json` 接着迁。

| 东西 | 为什么、怎么办 |
|---|---|
| 自己写的脚本 | 这一版的脚本格式不一样。报告逐个列出；官方软件源里有同名新包的，报告提示装哪个 |
| 旧版包管理器装的技能、脚本 | 不拷。报告写软件源里对应的新包，用 `miyu pkg install` 装 |
| 人格的子系统、插件开关，开发模式的提示词，没选人格时的系统提示词 | 这一版没有对应。人格、预设见 [人格和预设](personas-presets.md) |
| 配置里的 CLI 中转供应商（`claude-code`、`codex`、`antigravity`、`codebuddy`） | 这一版没有对应的驱动。换成接 API 的供应商，见 [模型](models.md) |
| 配置里的 `context`、`tools`、`cache`、`display`、`notifications`、`voice`、`skills` 段，看图、搜索以外的插件设置 | 这一版没有对应的格 |
| 配置里的 `mcp` 段 | MCP 服务在这一版重新加，见 [扩展 Miyu](extending.md#mcp-服务) |
| QQ 的接入设置（管理员号以外的） | 在 im 里重新接 QQ，见 [接入通讯平台](heads/im.md#接-qq) |
| 用量记录 | 这一版的用量账本还收不进来 |
| 终端的上键历史 | |
| QQ 收到的图片、文件、头像 | 这一版的 im 只记占位，不存内容 |
| 好感度、情绪这类 QQ 插件数据 | 这一版的 im 里要重做 |
| 记忆里的日记、逐轮经历、改动记录、旧向量 | 只导知识点。向量这一版自己算 |
| AI 自己的、不在用的、超过 120 字的记忆 | 超长的写进 `给agent.json`，改短了以后能接着导 |
| 一次性的 `ask` 会话、成员的会话、QQ 以外平台的会话 | |
| 成员的家目录 | 这一版只给一个人用 |
| 定时提醒 | 要的照报告里的内容重新定，见 [定时任务](automation.md#定时任务) |
| 旧网页的登录密码、自定义样式 | 这一版的网页另有登录和外观 |
| 知乎、小红书、B 站搜索的登录态 | 这一版还没有对应的包。B 站开播装 `bilibili-live-stream`，在里面重新登录 |
| 旧版内置的默认知识库、默认表情包 | 不迁 |
| 以前给别人的分享 | 不迁 |
| 日志、缓存、索引 | 这一版自己重建 |

旧版在数据根外面装过的东西不进压缩包。迁移只看家目录下这六处：`~/.local/bin/miyu`、`~/.config/fish/conf.d/miyu.fish`、`~/.gemini/config/agents/miyu`、`~/.gemini/antigravity-cli`、`~/.cache/miyu/voice-models`、`~/.local/state/miyu`，报告的「数据根外面」一节列出哪几处在。macOS、Windows 上也只看家目录下这几处；旧版装在别处的东西报告里不列，要自己找、自己清。

## 看报告

每跑一次一个目录 `~/.miyu/root/documents/旧版 Miyu 迁移/<时刻>/`，时刻是 UTC，写成 `20261114T083000Z`。里面有：

| 文件 | 是什么 |
|---|---|
| `报告.md` | 给你看的报告 |
| `给agent.json` | 同一份报告的 JSON，给 AI 读，接着迁没迁的 |
| `miyu-0.7.tar.zst` | 旧目录整个的压缩包 |
| `miyu-0.7.manifest.json` | 压缩包里每个文件的 SHA-256 |
| `qq-records.jsonl` | 有 QQ 消息记录的才有 |
| `legacy-memes-<库名>/` | 有自己的表情包库的才有 |

`报告.md` 从上往下：

- 开头：迁移时刻、旧目录在哪、压缩包在哪。
- 「人格」：一张表，旧版的每个人格对上了这一版的哪个，「新建」「已经有同名的」「出厂的软件工程师」「没映射上」。
- 「每一类」：一类一节，节名是英文的类名（`sessions`、`memories`、`config` 这样）。每节先写「迁了」「迁了一部分」「没迁，在压缩包里」「不用迁」，迁了几条、跳过几条，迁到哪，再一句补充。没装对应的包的，补充里写装上以后跑哪一行。
- 「数据根外面」：旧版在数据根外面装过的东西。
- 「没迁的」：一张表，每样在压缩包里的路径、旧格式。
- 「旧数据根」：压缩包的大小和 SHA-256，什么时候可以删旧目录。

报告只写名字和条数，不写 key、cookie、聊天内容。

## 核对迁过来了

- 人格：设置页的人格列表里有「Miyu」和你自己的那几个。
- 会话：会话列表的「存档」一组。
- 记忆：`miyu memory list --persona <编号>`。
- 知识库：`miyu kb list legacy`。
- 设置：`miyu config get`，`miyu provider list`。
- 技能：`~/.miyu/root/skills/`。

## 重跑和中途失败

再跑一次 `miyu migrate ~/.miyu-0.7` 是安全的，适合装了漏装的包以后补迁：

- 会话：编号照旧编号算，已经导过的不再导。
- 记忆：每条记着来自旧版哪一条，导过的、你后来删掉的都不再导。
- 配置：只补还没有的。
- 人格：同名的用已有的，不重建。
- 技能、知识库、文件：内容一样的算迁过；同名不一样的改名并列。
- QQ 消息记录、赞助记录、表情包：对方认得出已经有的，不会多。

每跑一次都重新打一个压缩包、多一个目录，要占旧目录两倍的盘。同一秒里跑两次的，后一次报 `… already exists; wait a second and run again`。

中途失败的，标准错误上印 `miyu migrate: <原因>`，退出码 `1`。失败前已经写进去的留着，照原因改好重跑，迁过的会跳过。失败的那一次留下：

- `~/.miyu/root/documents/旧版 Miyu 迁移/<时刻>/`：压缩包，没有报告。确认后可以删。
- `~/.miyu/var/lib/migrate/<时刻>/`：解出来核对的副本。成了的那一次用完就删，失败的留着，可以删。

## 迁完以后

- 旧程序：AUR 上旧的 `miyu` 包升级以后就换成了过渡包 `miyu`，它自己不带文件，只依赖 `miyu-agent`；旧版的程序已经没了。新版的 `miyu-core` 和 0.7 那一代的 `miyu` 互斥，见 [安装、升级、卸载](install.md#用-aur-装arch-linux)。别的装法装的，照报告「数据根外面」那一节，确认是旧版的再删。报告里说 `~/.local/bin/miyu` 是新版包装脚本的，别删。Linux、macOS 上用安装脚本装的这一版，`~/.local/bin/miyu` 是指向 `~/.local/lib/miyu/miyu` 的链接，也是新版的，别删。
- 旧目录 `~/.miyu-0.7`：迁移不删。确认都迁过来了、用了一阵没问题，自己删。
- 压缩包：留着。以后想让 AI 接着迁没迁的东西，要用它和 `给agent.json`。不要了再删整个 `<时刻>/` 目录。
- `migrate` 这个包：用完可以 `miyu pkg remove migrate`。迁移的目录在 `~/.miyu/root/documents/` 里，卸包不删。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| 这一版起不来：「… 认不出是 Miyu 的数据根 …」或「… 是旧版 Miyu 的数据根，新版不认 …」 | 旧目录还在 `~/.miyu`。照上面第 2 步挪开 |
| `old Miyu is still running (pid <进程号>); stop it first` | 旧版还在跑。关掉它，或者 `kill <进程号>`（Linux、macOS） |
| `old Miyu is still running; stop it first` | 旧版还在跑，说不出进程号。Windows 上报的都是这一句。关掉旧版 Miyu 的窗口和后台程序再迁；Windows 上可以在任务管理器里找到它结束掉 |
| `<路径> is not an old Miyu data folder` | 这个目录不是旧版的数据目录。看看是不是写错了路径 |
| `old Miyu did not finish moving its files (…); start the old version once to let it recover` | 旧版上次搬自己的文件没搬完。用旧版启动一次，让它收尾，关掉再迁 |
| `not enough disk space for the archive and its check under <目录>: need <N> MiB, have <M> MiB` | 盘不够：压缩包那一处要旧目录两倍大。腾出地方再跑 |
| `cannot reach the core: …` | 连不上这一版的核心。见 [出了问题怎么查](troubleshooting.md) |
| ``run this as `miyu migrate <old folder>`: MIYU_STATE_DIR is not set`` | 直接跑了包里的程序。用 `miyu migrate` |
| 报告里「整理没成」 | 迁记忆时还没有能用的模型。接上模型以后跑 `miyu memory dream --persona <编号>` |
| 报告里某一类「没迁，在压缩包里」，补充说某个包没装 | 装上那个包，照补充里那一行跑，或者重跑迁移 |
