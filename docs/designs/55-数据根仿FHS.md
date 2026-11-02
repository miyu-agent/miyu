## 数据根仿 FHS

数据根里照 Linux 的 FHS 分块：`etc/` 配置和凭据，`opt/` 装上的软件包，`root/` 你的东西，`var/` 机器的状态、缓存、日志，`tmp/` 短命的暂存，`run/` 运行时。改掉现在顶层二十几项摊在一起、`state/` 里真数据和派生数据混放、一个软件包的东西散在七八处的样子。

状态：已定（2026-11-02 项目主人和 sub1 讨论定；同一天经一轮独立审查改过）。改掉 `07-存储.md` 第二节「目录布局」（数据根一层、状态区像 `/var`）。施工单 `docs/construction/FH-数据根仿FHS.md`；盘点见同目录 `FH-盘点-核心.md`、`FH-盘点-别的仓库.md`。

### 一、为什么改

1. 顶层太挤：配置和凭据七八个文件，你写的七个目录，她给你存的四个目录，还有会话、blob、知识库、模块、索引、缓存、状态、运行时，二十几项摊在一层，看不出哪些是你的、哪些是程序的。
2. `state/` 名义上是派生的，实际混着三类：能重建的（运行日志、用量汇总、配置参考、模型列表）、丢了就没了的（唤醒表、各包的数据、脚本的登录态）、凭据（Windows 沙盒用户的加密密码）。派生数据又散在 `index/`、`state/`、`cache/`、机器缓存目录四处，「哪些能删」说不清。
3. 一个软件包的东西散在 `packages/`、`state/packages/`、`state/packages/.local/`、`packages/<编号>.removed`、`state/packages/.leftovers.json`、`extensions.json`、`config.toml`、`state/scripts/*/<编号>/`、`state/mcp/`。包改一次名，搬家工具要动将近二十处（2026-10-30 改编号）。
4. 外面的程序自己拼数据根里的路径：im 自己拼 `state/packages/miyu-im/`（三处）、`state/logs/`，网页自己拼 `run/web*`。布局一改它们就坏。

### 二、布局

```text
<数据根>/                         默认 ~/.miyu，MIYU_HOME 能改（07 第二节，不变）
├── .miyu-layout                  标记：This directory is a Miyu data root (layout 3).（不再叫 .miyu-root，第六节第 1 条）
├── etc/                          机器级的配置。建数据根时建好，Windows 沙盒里整条禁
│   ├── config.toml               配置
│   ├── trust.toml                信任过的项目配置
│   ├── extensions.json           扩展开没开、批过的能力
│   ├── kb.toml                   包带的知识库开不开「自动想起」
│   ├── private/                  凭据，0700，建数据根时建好
│   │   ├── secrets.toml          密钥，0600
│   │   ├── access.json           访问密码的哈希，0600
│   │   ├── logins.json           设备令牌的哈希，0600
│   │   └── sandbox-windows.json  Windows 沙盒用户的记录（含 DPAPI 加密的密码）
│   ├── removed/<编号>            卸掉的出厂包的记号（空文件，像 systemd 的 mask）
│   └── <编号>/                   包自己的配置文件（例如 miyu-im/venues.d/、moderation.txt）
├── opt/
│   ├── <编号>/                   装上的软件包：清单、bin/、包带的人格技能知识库模型
│   └── .<编号>.new/ .<编号>.old/  装、升级时的暂存和备份（同一个目录里才能改名换上）
├── root/                         你的东西。只给一个人用，这个人就是 root；以后多用户另开 home/<用户名>/
│   ├── personas/ presets/        你的人格、预设，盖过出厂的、包带的
│   ├── prompts/ tools/           提示词、工具说明的覆盖
│   ├── skills/ scripts/ commands/  你的技能、脚本、斜杠命令
│   ├── avatar.<png|jpg|webp>     你的头像
│   ├── sessions/<会话>/          会话日志（段）、attachments/、jobs/
│   ├── memory/<人格>/            跟着人格的记忆日志；没选人格的是 _none/
│   ├── kb/<编号>/                你的知识库：文档和这个库的 kb.toml
│   ├── blobs/<前两位>/<哈希>     附件、大内容；存的时候的临时文件、分块上传在 blobs/tmp/（像 git 的 objects/tmp）
│   ├── trash/                    回收处：sessions/、personas/
│   ├── projects.json             项目
│   ├── schedules.db              定时任务（唤醒表）
│   ├── state/
│   │   ├── scripts/[@<场所>/]<来处>/<工具>/  脚本的状态（登录态这些）
│   │   └── terminal.json         各终端进哪个会话
│   ├── workspace/<会话>/         工作区
│   └── pictures/ videos/ documents/ downloads/  她给你存的文件
├── var/
│   ├── lib/                      丢了就没了，但不是你写的：机器和软件包的状态
│   │   ├── journal.jsonl         配置、密钥、信任的改动；删掉的会话和一次性调用的用量
│   │   ├── log-salt              运行日志里记忆短号的盐，0600
│   │   ├── models/               学到的上下文窗口 learned.json、池的指针 pools.json、供应商改名 renamed.json
│   │   ├── packages/<编号>/      本地库：desc、files（照 pacman 的 local/）；leftovers.json 卸包留下的改动
│   │   ├── mcp/<服务>/           MCP 服务（命令型）的工作目录
│   │   └── <编号>/               每个包自己的数据
│   ├── cache/                    能删（先停核心）：核心会重建或重新下载
│   │   ├── index/                sessions.db、recall/、kb/（向量要重算，删了慢一阵）
│   │   ├── usage.db rates.db     用量汇总（照会话日志、回收处、journal 重建）、汇率
│   │   ├── config/               生成的 config.schema.json、reference.toml
│   │   ├── models/               供应商的模型列表 providers/、目录 catalog.json
│   │   ├── packages/             软件源索引 sync/、sources/<源>/，下载的包文件 downloads/
│   │   ├── mcp/<服务>.json       MCP 工具目录缓存
│   │   ├── provide/<包>.json     包登记的工具原文缓存
│   │   ├── scripts/[@<场所>/]<来处>/<工具>/  脚本的缓存，30 天没碰的清掉；交图的 images-<hex>/ 在 <来处>/ 下
│   │   ├── personas/ mermaid/    人格图片的缓存、mermaid 字体
│   │   ├── sandbox-probe.json    探到的沙盒能力
│   │   ├── perf/                 性能测试工具自己的缓存
│   │   └── <编号>/               包自己的缓存
│   ├── log/                      运行日志，能删
│   │   ├── core.log core.log.1…  核心的运行日志
│   │   ├── memory.jsonl          核心的内存账
│   │   ├── mcp/<服务>.stderr     MCP 服务的错误输出
│   │   ├── sandbox/windows-error.json  Windows 沙盒安装失败的原因
│   │   └── <编号>/               包的日志；stderr、stderr.old 是核心记的它的错误输出，包自己不用这两个名字
│   └── tmp/                      跨重启留着的暂存，按时限清；现在没有住户，先留着（例如以后能续传的下载）
├── tmp/                          短命的暂存
│   ├── unpack/                   装 .miyu 时解包，核心起来清
│   └── <编号>/                   包自己的短命暂存，核心拉起这个包的扩展之前清
└── run/                          运行时：core.lock spawn.lock token socket（core.sock）、写 etc/ root/ 里文件的临时文件
    └── <编号>/                   包自己的锁、套接字、地址（例如网页的 web.lock、web、open/）
```

机器缓存目录（`~/.cache/miyu/` 这些，`07-存储.md` 第二节）不变：沙盒里的工具链缓存、models.dev 目录，跟机器走，几个数据根共用。套接字的位置（Linux 上 `$XDG_RUNTIME_DIR/miyu-<指纹>/`，指纹照数据根的路径算）、项目里的 `.miyu/config.toml`、资源目录不变。

### 三、规则

1. **备份** `etc/`、`root/`、`var/lib/`；**能删** `var/cache/`、`var/log/`、`var/tmp/`、`tmp/`（删 `var/cache/` 先停核心，第五节第 2 条）；`opt/` 能重装；`run/` 核心在跑时别动。
2. **归谁**：你产生的、你写的放 `root/`；机器和软件包的放 `etc/`、`opt/`、`var/`。以后多用户时，每人一个 `home/<用户名>/`，排法同 `root/`，`root/` 不挪；以后只是多出一份个人版的（个人设置、个人密钥），现在放机器级，到时候在家里加一层盖过它，现在的不挪。
3. **核心的和包的**：照 Linux 的 `/etc/passwd` 和 `/etc/<软件>/` 并存：核心自己的放在 `etc/`、`var/lib/`、`var/cache/`、`var/log/`、`tmp/`、`run/` 这一层，每个软件包各占一个 `<编号>/`。核心在这几层用到的名字保留，软件包不能用作编号：`private`、`removed`、`models`、`packages`、`mcp`、`index`、`config`、`provide`、`scripts`、`personas`、`mermaid`、`sandbox`、`perf`、`unpack`、`token`、`socket`、`log-salt`。保留表由测试照各层的常量推出来、和这份对；装包时拒，`miyu check` 报，软件源收录时拒。包编号不带点，和 `config.toml` 这类文件名撞不上。
4. **一个软件包的东西**：`opt/<编号>/`、`etc/<编号>/`、`var/lib/<编号>/`、`var/cache/<编号>/`、`var/log/<编号>/`、`tmp/<编号>/`、`run/<编号>/`；`var/lib/packages/<编号>/` 本地库、`etc/removed/<编号>`；`etc/extensions.json`、`etc/config.toml`、`etc/kb.toml` 里它的那几项；它带的脚本的状态和缓存 `root/state/scripts/<编号>/`、`var/cache/scripts/<编号>/`（还有各 `@场所` 下的）；它带的 MCP 服务的 `var/lib/mcp/<服务>/`、`var/cache/mcp/<服务>.json`、`var/log/mcp/<服务>.stderr`；`var/cache/provide/<编号>.json`、`var/cache/index/kb/packages/<编号>/`。卸包就是清这些（照现在「卸就是清干净」）。
5. **两个 tmp**：`tmp/` 短命，像 `/tmp`：核心起来只清自己的 `tmp/unpack/`；`tmp/<编号>/` 在核心拉起这个包的扩展之前清，包自己拉起的常驻程序（例如 `miyu web` 分离起的服务）自己管。`var/tmp/` 跨重启留着，像 `/var/tmp`，按时限清（初值 7 天没碰），现在没有住户。沙盒里命令和脚本用的临时目录照旧用系统的，不放进数据根。blob 的临时文件留在 `root/blobs/tmp/`：要和 blob 在同一个目录树里才能改名换上，`tmp/` 可能被人单独挂成 tmpfs。
6. **人格和记忆**：人格三层（出厂的、包带的、你的 `root/personas/`），记忆照人格编号只有一份，不管用的是哪一层（`personas.md`「怎么走」第 5 条），放 `root/memory/<人格>/`，不放进人格目录：出厂的、包带的人格在只读的地方；在 `root/personas/` 下为出厂人格建一个目录会被当成你那一层的覆盖；没选人格的 `_none` 没有人格目录。
   - 删人格时，三层里都没有这个编号了（删的是你那一层、下面没有出厂的、包带的同编号），它的记忆跟着进 `root/trash/personas/`；删的只是盖在出厂人格上的那一层（回到出厂的那份，`protocol.md` 的 `persona.delete`），记忆不动。
   - 从回收处恢复人格，记忆一起回来；这个编号这时已经有了记忆（同编号又建过、又说过话），人格照恢复，回收处里那份记忆留着不并，说一句「这个人格已经有新的记忆，旧的留在回收处」。清回收处时一起删。
7. **会话和工作区分开**：会话日志她碰不到，工作区她能写；一个工作区不只属于一个会话（子会话沿用、项目指定）。
8. **回忆库是派生的**，放 `var/cache/index/recall/`：撤销的回合的墓碑照会话日志补得回来；删会话时埋的整个会话的墓碑只用来挡后台补的一批，出处活不活照会话目录在不在判（`memory.md`「怎么走」第三条第 4 款，施工 R-3 三补），库删了重建，删掉的会话里记的不会冒出来。
9. **写文件的临时文件**：`etc/`、`etc/private/`、`root/` 里整体替换的文件（配置、密钥、信任、开关、项目、终端指针），临时文件都建在 `run/`（今天只认「上一层是数据根」，改成照数据根找），同一个数据根里改名照样是原子的；`miyu config edit` 给编辑器的副本照旧放在本体旁边（`etc/.config.toml.edit-<随机>.toml`，0600），`#:schema` 的相对路径照样对得上，`etc/` 在 Windows 沙盒里整条禁。

### 四、交给程序的环境变量

Miyu 拉起来的程序（扩展、包带的命令和程序、脚本、MCP 服务）都拿同一组变量，每个变量已经指到这个程序自己的那一格；程序不用知道自己是包还是脚本，不拿 `MIYU_HOME` 拼路径。写法照 XDG 的 `XDG_*_HOME`，统一 `_DIR` 结尾。

| 变量 | 包（扩展、命令、程序） | 脚本 | MCP 服务 |
|---|---|---|---|
| `MIYU_APP_DIR` | `opt/<编号>/`；出厂包是资源目录里它的文件夹 | 脚本所在的包的文件夹（你的脚本没有） | 不设 |
| `MIYU_CONFIG_DIR` | `etc/<编号>/` | 不设 | 不设 |
| `MIYU_STATE_DIR` | `var/lib/<编号>/`（也是工作目录） | `root/state/scripts/[@场所/]<来处>/<工具>/` | `var/lib/mcp/<服务>/`（也是工作目录） |
| `MIYU_CACHE_DIR` | `var/cache/<编号>/` | `var/cache/scripts/[@场所/]<来处>/<工具>/` | 不设 |
| `MIYU_LOG_DIR` | `var/log/<编号>/`（核心记的 stderr 在这里的 `stderr`） | 不设（输出进工具结果） | 不设（stderr 核心记） |
| `MIYU_TMP_DIR` | `tmp/<编号>/` | 不设（用沙盒的 `TMPDIR`） | 不设 |
| `MIYU_RUNTIME_DIR` | `run/<编号>/` | 不设 | 不设 |
| `MIYU_IMAGES_DIR` | 不设 | 交图的目录（原 `MIYU_SCRIPT_IMAGES_DIR`） | 不设 |
| `MIYU_PICTURES_DIR` `MIYU_VIDEOS_DIR` `MIYU_DOCUMENTS_DIR` `MIYU_DOWNLOADS_DIR` | `root/` 下四处 | 同左 | 不设 |
| `MIYU_HOME` `MIYU_PROGRAM` `MIYU_RESOURCES` | 数据根、主程序、资源目录 | 同左 | 不设 |

- 不设的就是这一种程序用不着。
- 核心拉起程序之前把给出去的目录都建好（Windows 沙盒要求，第五节），程序拿到的一定已经存在。
- 旧名字 `MIYU_PACKAGE_STATE`、`MIYU_PACKAGE_DIR`、`MIYU_SCRIPT_STATE_DIR`、`MIYU_SCRIPT_CACHE_DIR`、`MIYU_SCRIPT_IMAGES_DIR` 去掉。施工期间（FH-3 到 FH-6）核心新旧两套一起给、旧名字指到新位置，等各包发了新版再删（FH-6），不留到发版。
- 以后多用户，个人那一格另加变量（例如 `MIYU_USER_STATE_DIR`），现有的不改名、不改义。
- 给包作者的文档（`docs/authors/`）写这一张表，替掉 im 文档里「`<数据根>/state/logs/<你的包>.stderr`」那种写法。

### 五、Windows 沙盒

Windows 的沙盒只禁命令起来那一刻已有的，嵌在禁止里的放开压不过禁止，所以数据根里有要透出的（工作区、技能文件夹、脚本的状态和缓存、附件）时，是一层层禁旁边那几项（`docs/blueprint/sandbox/windows-run.md` 第 4 步）。新布局照这条排：

1. **凭据都在 `etc/private/`**，`etc/`、`etc/private/` 建数据根时就建好，里面从不透出东西，沙盒里整条禁：密钥、访问密码、设备令牌第一次新建时也在禁止的目录里，读不到（解决 `windows-run.md` 记的「第一次新建的那一下」）。写它们的临时文件在 `run/`，也整条禁（第三节第 9 条）。
2. **事先建好**（核心起来时）：`etc/`、`etc/private/`、`etc/removed/`、`opt/`、`root/` 和它下面的 `sessions/`、`memory/`、`kb/`、`blobs/`、`trash/`、`state/`、`workspace/`、`var/lib/`、`var/lib/packages/`、`var/lib/mcp/`、`var/cache/`、`var/cache/index/`、`var/log/`、`var/tmp/`、`tmp/`、`run/`。每条命令前照旧补 `run/`、本会话的 `jobs/`、本会话的工作区，脚本前补它的状态和缓存目录。`var/cache/` 在跑脚本时是透出路径上的一层，`index/` 在命令跑着时重建出来的话读得到回忆库里的原文，所以删 `var/cache/` 要先停核心（第三节第 1 条），起来时先建好 `index/`。
3. **透出路径上的几层**（`root/`、`root/state/`、`root/sessions/<本会话>/`、`var/`、`var/cache/`、`var/cache/scripts/` 这些）里命令跑着时第一次建的东西读得到。凭据不在这些层里；`root/` 下第一次建的 `projects.json`、`schedules.db`、`avatar.*` 和今天顶层的一样，记作已知、不处理。
4. **沙盒的安装程序**（提权的那一个，`miyu-sandbox/src/install/`）写两处：安装记录 `etc/private/sandbox-windows.json`、失败原因 `var/log/sandbox/windows-error.json`，两条路都逐层查过不是链接再往下走。

### 六、不做的、不认的

1. **不认旧布局**（2026-11-02 项目主人定：还没发版，不做旧版兼容，代码整个改掉）：标记换名 `.miyu-layout`，新程序只认它、读里面的版本，不是 `layout 3` 的不动、报错停下：「这是旧版 Miyu 的数据根（layout 2），新版不认；设 `MIYU_HOME` 指到一个空目录，旧数据照施工单 FH-7 自己搬」。旧标记 `.miyu-root` 的数据根新程序不认（目录不空、没有新标记，照「认不出的不动」）；反过来旧的核心、头、沙盒助手只认 `.miyu-root`，碰到新数据根也认不出、不往里写（`run/` 下几样的位置没变，旧头连得上新核心，没有换名的话旧程序会照旧布局往新数据根里建 `state/` 这些）。产品里不写搬家。
2. **主人那一份部署**（数据根 `/home/shorin/.cache/miyu-tui-home`，包装脚本 `~/.local/bin/miyu-remake` 写死；`~/.miyu` 是旧版 Miyu 的活数据，不碰）：现有的数据可以不要，只留配置和 im 的赞助账本（2026-11-02 项目主人定）。同一个路径新开数据根（旧的整个改名留着），拷配置类的文件（`config.toml`、`secrets.toml`、`access.json`、`logins.json`、`extensions.json`）、`miyu-im-sponsor` 的 `sponsors.json`、人格 `persona-1`、预设 `dev.toml`、`minimal.toml`（人格和预设 2026-11-02 主人定留）。
   - 她的记忆也留（2026-11-03 主人定）：记忆日志 `modules/memory/`（每个人格一份，连 `_none`）搬进 `root/memory/`；她记下的每一条带出处（哪个会话第几轮），出处全在不在了的会话里的召回、列出、搜都当它不在（`memory.md`「怎么走」第三条第 3、4 款），所以会话 `sessions/` 一起搬进 `root/sessions/`，会话引用的 `blobs/` 搬进 `root/blobs/`。会话日志里写着的数据根里的绝对路径（附件、默认工作区）照前缀整体换成新位置，换完逐字节核对；回忆库、会话列表、用量这些派生的不搬，新核心照日志重建。回收处里的会话不搬（出处在那里的本来就不算数）。
   - 知识库、im 自己的记录（`state/packages/miyu-im/` 下的记录库、统计）不搬。
3. **「跟随会话」的记忆范围去掉**（2026-11-02 项目主人定：记忆就是为了跨会话，跟随会话的范围不合理；`17-记忆.md` L2 记作 2026-09-25 项目主人提出，主人没有这个印象，以这次为准）：范围只剩跟随人格和关掉；「清掉这个会话记下的」留着。会话目录里不再有 `memory/`。单独一步（FH-1）。
4. **附件、默认工作区在会话日志里的绝对路径**这次不动：数据不搬，新数据根里写的就是新位置；改成相对路径牵扯回放、分叉、导出、拼提示词几处读者，块里的 `path` 还装着头交来的本机路径，另议。

### 七、已定

**FHS1. 数据根是一个目录还是照 XDG 分散**

- **已定：一个目录，里面仿 FHS（2026-11-02 项目主人）。** 跟这份数据走的缓存放 `var/cache/`，跟机器走的（沙盒工具链、models.dev）留在机器缓存目录。
- 未选：配置进 `~/.config/miyu`、数据进 `~/.local/share/miyu`……备份要记几处，Windows、macOS 另定一套。

**FHS2. 目录名**

- **已定：用 Linux 原词 `etc/ opt/ var/lib var/cache var/log tmp/ run/`（2026-11-02 项目主人）。** 先例：Termux 私有目录里的 `usr/`、`home/`，MSYS2、Cygwin 的 `etc/ home/ usr/ var/ tmp/`，Homebrew 前缀的 `etc/ var/ opt/`。
- 未选：`config/ data/ cache/ logs/ packages/ files/` 这种直白的词。

**FHS3. 你的东西放哪**

- **已定：`root/`，单用户就是 root；以后多用户另开 `home/<用户名>/`（2026-11-02 项目主人）。** 你写的人格、预设、提示词和工具说明的覆盖、技能、脚本、命令，你和她的会话、记忆、知识库、附件、回收处、项目、定时，脚本的状态，她给你的文件，都在里面，平铺；派生数据都在 `var/cache/`，`root/` 里全是要备份的，不再分 `.config`、`.local`。
- 未选：`home/admin/`；人格这些「她是谁」的放 `etc/`（`etc/` 是给所有人的系统级默认，人格属于它的主人；以后真要给所有人的默认人格，在 `etc/personas/` 加一层，不挪现在的）。

**FHS4. 核心的和包的怎么分**

- **已定：核心的放各块这一层，包各占一个 `<编号>/`，核心用到的名字保留（2026-11-02 项目主人）。**
- 未选：核心的再套一层 `miyu/`（多一层）；包统一收进 `packages/<编号>/`（包的路径多一段）。

**FHS5. 两个 tmp**

- **已定：`tmp/` 和 `var/tmp/` 都要（2026-11-02 项目主人）。**

**FHS6. 环境变量**

- **已定：一组通用的 `MIYU_*_DIR`，以后所有程序都能用（2026-11-02 项目主人定：以后所有程序都照这一组用）。** 见第四节。
