## 预设

### 是什么

预设管「她以什么方式工作」：哪些软件开着、哪几件工具关掉、不指定人格时用哪个人格（`16-人格与预设.md`）。一个预设是一份 TOML 文件，和人格一样三层叠起来：出厂的、装着的软件包带的（施工 P-11 上，`<包文件夹>/presets/<编号>.toml`，一个编号只认一份，同 `personas.md`「软件包带的」）、你的（数据根的 `presets/`）。开会话必须有预设，没指定的照默认（Y12），钉在会话上（Y5）。

状态：图纸，施工 P-2（上）做了（2026-10-08 主会话；方向照人格与预设走查的 C 组、D2 和 Y12，项目主人 2026-10-07 定；出厂编号和默认预设和通讯平台的会话对过）。照预设挑工具面、装了没开的那一行（Y8）、开不开记忆和角色扮演施工 P-2（中）做了（2026-10-08，走查 C1、C4、E2）。改了预设的文件开着的会话下一个回合换上施工 P-2（下）做了（2026-10-08，K3）。预设里写配置值（D1）、子代理选人格和预设（C5）随后面两步，新建、改、删、`base` 随 P-3。 预设照功能开关施工 F-3（上）做了（2026-10-09 主会话，设计 `30-插件框架.md` 第四节）：`[features]`、工具照功能留不留、没装的功能就是没有；`preset.get` 照功能列、`preset.set` 改功能施工 F-3（下）做了（同一天）。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-policy/src/preset.rs` | 一份预设文件读成样子：每一格的写法，写错的报第一处，带代码、第几行；逐格叠（纯逻辑） |
| `crates/miyu-policy/src/preset/switches.rs` | 全局开关（施工 X-27）：预设明写的、全局开关、`unlisted` 的先后（`PresetFile::decided`），只认技能、脚本、MCP 三类功能 |
| `crates/miyu-session/src/switches.rs` | 配置项 `features.<name>`，照这一轮的配置读出全局开关（施工 X-27） |
| `crates/miyu-session/src/packages_off.rs` | 配置项 `packages.<id>.off`（施工 U23 末上）：把某个包关掉，照这一轮的配置读出关了哪些，并进叠好的预设文件（`PresetFile::switching_off`、`switched_off`） |
| `crates/miyu-store/src/presets.rs` | 三层在哪、怎么叠、列出编号、`miyu check` 查每一份 |
| `crates/miyu-store/src/layers.rs` | 人格、预设共用的四层、包那一层认哪一份（`covered`，施工 P-11 上）和编号的写法 |
| `crates/miyu-endpoint/src/presets.rs` | 造会话时照默认找、算装了哪些软件和没开的、`preset.list`、`preset.get` |
| `crates/miyu-tool/src/catalog.rs` | 工具目录按包登记，记下每件工具归哪个包（施工 P-2 中） |
| `crates/miyu-session/src/agents.rs`、`open.rs` | 造会话时照预设筛工具面、定记忆的范围（施工 P-2 中） |
| `crates/miyu-policy/src/compose.rs` | `with_preset`：快照记预设、去掉角色扮演提示、写装了没开的那一行（施工 P-2 中） |
| `crates/miyu-endpoint/src/settings.rs` | 配置项 `preset.default` |
| `resources/presets/full.toml`、`dev.toml` | 出厂的两个 |

### 对外的样子

**在哪**：

```text
<资源目录>/presets/<编号>.toml        出厂的，只读
<包文件夹>/presets/<编号>.toml         装着的软件包带的，只读（施工 P-11 上）；被出厂的、排在前面的包盖住的 `miyu check` 报 `preset_covered`
<数据根>/presets/<编号>.toml          你的
```

1. 编号就是文件名，写法同人格的编号（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个）。不合写法的、不是 `.toml` 的、是目录的不算。
2. 同名的后面的叠在前面的上面，逐格盖：`[preset]` 逐格（`name`、`summary` 逐种语言），`[software]` 逐个键，关掉的工具叠在一起。空文件也算有这一层。

**格式**（只收下面这些，不认识的表、键报错，写明第几行）：

```toml
[preset]
name = "开发"                   # 一句字（施工 P-3 补）；以前的语言表照样认
summary = "只开写代码必需的"
unlisted = "off"               # 没列在 [software] 里的软件开不开：on、off

[features]                     # 功能的编号 = 开不开（施工 F-3 上，设计 30 第三节）
commands = false

[software]                     # 以前的写法：软件包的编号 = 开不开，照认
basesystem = true
net = true
goal = true

[tools]                        # 在开着的包里关掉单件工具：只能写 false
shell = false
```

1. `name`、`summary` 照人格的写法：一句字，名字去掉前后空白不能是空的，说明可以是空的字、就是没有说明（施工 P-3 补、再补）；以前写成语言表（`zh`、`en`、`ja`）的照样认，出厂的照旧这么写，界面照连接的语言显示。上一层写了的整格换掉。叠好的文件的指纹照以前的写法算（没写的是空表、语言表照原样）：以前造的快照照旧对得上。
2. 没有 `default_persona`（施工 P-4 上撤了，2026-10-08 项目主人：「人格、记忆、预设相互之间耦合了」，只去掉预设的默认人格）：以前写了的认出来当没写，指纹照没写算，下一次 `preset.set` 写这份文件时去掉；`preset.set` 写这个键是 `bad_params`。
3. `unlisted`：几层都没写的照 `on`。功能全开是默认，`[tools]` 里只关一两件的写法也是建在「其余都开」上的。
4. `[features]`（施工 F-3 上）的键是功能的编号（设计 30 第三节第 6 条那十五个，加上装了的包自己声明的），写法同包的编号，值是开关。一个功能开不开的先后：`[features]` 写了的；没写的照 `[software]`，先认功能的编号、再认它所在的包的编号、再认包所在的组（施工 BS-1 上中：`basesystem = true` 开组里没单独写的包；组照出厂清单的 `groups`，找预设时放进 `PresetFile::groups`，不进指纹）；都没写的照全局开关（下面「全局开关」，只管技能、脚本、MCP 服务，只在 `unlisted` 开着的预设里算，施工 X-27）；再没有的照 `unlisted`（`PresetFile::opens_in`、`decided`）。写了没装的不报错。关掉的包（施工 U23 末上，配置 `packages.<编号>.off`，`switched_off`）整包不开，同场所扣下的包；不同的是它的功能算进没开的那几个，快照记的跟着变，开着的会话下一个回合换上工具面、技能（施工 BS-1 上中起没有必需的包）。
   以前的 `[software]`（施工 F-3 上起照认，新写的用 `[features]`）：键是软件的编号，写法同包的编号。现在装了的：基础系统组 `basesystem`（施工 BS-1 上中起是组，写它就是写组里的十八个包）、记忆 `memory`（三件工具和回合开始的召回）、知识库 `knowledge`（两件工具，施工 K-1 下）、角色扮演 `roleplay`（人格的角色扮演提示和风格锁），和清单装的 `process` 包（桥）；联网 `net`、长期目标 `goal` 等它们做出来。写了没装的不报错（这台机器上以后可能装），`preset.get` 的 `software` 里标 `installed: false`（施工 P-3 补）。
5. `[tools]` 的键是工具名：字母、数字、`-`、`_`，最多 64 个。单件打开某个包里的一件先不做（走查 C1），写 `true` 报错。
6. 权限级别、压缩模板、强调色这些配置值随 P-2（下）（D1、C3）。

**出厂的三个**（Y6、Y7，`10-自带软件.md` 第四节；极简开发是施工 P-16 上加的）：

| 编号 | 名字 | 内容 |
|---|---|---|
| `full` | 全部功能 | `unlisted = "on"`：装了的全开，以后新装的也开。`preset.default` 出厂是它 |
| `dev` | 常规开发 | `unlisted = "off"`，开 `basesystem`、`env`（注入环境信息，施工 ENV-1）、`net`、`goal`、`memory`（人格记忆，2026-10-20 项目主人定，施工 P-15）、`authoring`（写技能、写包的两个技能，施工 P-8，2026-10-12 项目主人照推荐定：不开她要写技能时看不到，只能在本机目录里翻） |
| `minimal` | 极简开发 | `unlisted = "off"`，`[software]` 只开 `read`、`write`、`edit`、`shell` 四个包（施工 BS-1 上中起一件一个包，不用再一件件关）。基础系统组以后新加的包照 `unlisted` 关着（「脚本调工具」`clicall` 就是，施工 M-6）。环境那七个（施工 C-11，下面「环境那几块」）和「用户身份」（施工 ENV-1）照旧开着，`[features]` 写明了 |
| `none` | 无功能 | `unlisted = "off"`，什么都不写：菜单里能开关的全关，以后新装的也关（施工 P-17，下面「无功能」） |

- 名字三种语言：全部功能、Everything、すべての機能；常规开发、Development、開発（2026-10-20 项目主人改名，原来是基础功能、Basics、基本機能，施工 P-15）；极简开发、Minimal、ミニマル開発；无功能、Nothing、機能なし（施工 P-17）。图标 `boxes`、`box`、`square-terminal`、`circle-off`。编号照旧 `full`、`dev`、`minimal`，加 `none`，只在核心和配置 `preset.default` 里用，界面上不露。
- 说明三种语言（施工 P-16 上，2026-10-23 项目主人定；原来出厂的不写说明，施工 P-4 下）：全部功能「开放全部工具。」，常规开发「在极简开发的基础上增加记忆、网络搜索、子代理、Agent间交流等开发常用工具。」，极简开发「只提供Shell和读写文件的工具，最小化工具集以最大程度发挥 AI 原有的能力。」，无功能「不开启任何功能。」（施工 P-17）。给人看的，不发给模型。

**环境那几块**（施工 C-11，2026-10-23 项目主人定：拆成能开关的功能，群聊里用不上的能关掉；设计 30 第三节第 6 条）：七个功能，都没有工具，照功能的规矩开关（`unlisted = "off"`、又没写它们的预设里是关的）。关了的回合开始不再注入那一块事实，system 里跟着它的那一行不拼（`miyu_policy::shown`）：

| 功能 | 回合开始的事实 | system 里那一行 |
|---|---|---|
| `env-time` 时间 | 环境那一块的时间、时区 | |
| `env-workdir` 工作目录 | 环境那一块的工作目录 | 本机文件写绝对路径 |
| `env-permission` 权限级别 | 权限级别（平常的、切了级别的两份） | 每一级能做什么 |
| `env-session` 会话编号 | 会话编号 | |
| `env-model` 模型信息 | 模型 | |
| `env-harness` Harness 版本 | | 跑在哪一版 miyu-agent 里 |
| `env-machine` 本机系统 | | 本机是什么系统 |
| `env-profile` 用户身份 | | 「关于你」那一块（称呼、更多描述、回复语言，造会话时钉下） |

- 包（施工 ENV-1，2026-10-26 项目主人定）：这八个是内置包「注入环境信息」（`env`；Environment info、環境情報）的功能，前七个原来在基础系统里，编号不改，预设、配置里写的 `env-*` 照认。这个包不是必需的：整包关掉（`package.disable env`、配置 `packages.env.off`）就是一点环境信息都不注入，请求和八个都关一字不差。
- 开不开照「装着、并且没被关」：快照记两样，预设没开的（`off`）和没装的（`absent`，只记这八个里没装的，空的不写）；两样里都没有才算开。
- 自己写的预设（施工 ENV-1）：`unlisted = "off"`、`[software]` 只写了 `basesystem = true` 的，环境那几块原来跟着基础系统开，挪包以后要另写 `env = true` 才开。没发版，不做迁移。
- 「用户身份」只在造会话时看：关着的快照里就没有「关于你」，会话中途改预设不补、不撤（同改称呼）。

- 环境那一块只关一样的换成拆开的写法（`core/facts/env-time.txt`、`env-cwd.txt`），类别照旧是 `env`；两样都关的不注入。
- 关了的只是之后不再注入，已经注入过的留在历史里。改了预设的开着的会话下一个回合换快照（第 6 条）：只关事实的 system 不变；关了 system 那几行的断一次缓存。
- 以前造的快照里记的没开的功能没有它们、也没有 `absent`：都开着，前缀一字不变。

**无功能**（施工 P-17，2026-10-26 项目主人定）：出厂一份真预设 `resources/presets/none.toml`，名字「无功能」（Nothing、機能なし），说明「不开启任何功能。」，图标 `circle-off`，`unlisted = "off"`、别的什么都不写：预设菜单里能开关的全关着（工具、环境那几块、「用户身份」、记忆、技能、脚本、MCP、命令），以后新装的也关。它就是一份普通预设：能编辑，改了写你那一层，同别的出厂预设。

- 换掉下面的「无工具」：`preset.list` 不再拼编号是 `null` 的那一项。原来怕 `none` 和人自己写的 `none.toml` 撞，现在出厂就叫 `none`，人写的照分层盖住出厂那份，就是编辑。
- 协议上 `preset: null`、快照的 `no_tools` 照旧认：以前的会话照旧，交 `null` 的照旧开无工具（下面四条）。
- 项目主人在无人格、无工具的会话里问出来的八样（关于你、本机路径那一句、harness 版本、环境那一块、权限级别、会话编号、模型、记忆）每样在菜单里都有开关，选「无功能」的一样都没有（`preset_nothing.rs`）。

**无工具**（施工 P-16 下，2026-10-23 项目主人列的四种之一，主会话审过形状；施工 P-17 起不再列，协议照旧认）：不是一份预设文件，是核心给的一项。

1. （施工 P-17 起不列）`preset.list` 末尾固定一项 `{"preset": null, "name", "summary", "icon": "circle-off"}`，名字、说明（「无工具」「不提供任何工具。」，三语在核心给人看的字 `preset-none/*` 里）照连接的语言。编号是 `null` 不是字：预设的编号是文件名，写成 `none` 会和人自己写的 `none.toml` 撞。
2. `session.create`、`session.configure` 的 `preset` 交 `null` 选它；不写照旧照默认。`preset.default` 不能是它。空字照旧是参数不对。
3. 这样的会话一件工具都不给：工具面是空的，跟着工具面走的几块（技能目录、知识库那一块、`shell` 末尾那一段）也就都没有。回合开始重新筛照它，换回别的预设的下一轮照那份筛。
4. 记法：`session.created` 多 `no_tools: true`（`preset` 不写），快照多 `no_tools`；换预设的 `session.policy_changed` 照人格的写法，`preset` 是空的编号；内核、会话索引里也是空的编号。以前没记预设的（测试里造的、以前的快照）照旧不筛，和它分开。头那边会话信息里看到的是 `preset: null`、`no_tools: true`。

**配置**：`preset.default`，名字，出厂 `full`，以后开的会话照它（`config.md`）。设置页在「通用」那一页的「预设」一组。

**全局开关**（施工 X-27，2026-10-20 项目主人定）：每个技能、脚本、MCP 服务在头的三页上有一个开关，记在配置项 `features.<功能编号>`（开关，设置页不画，下一个回合生效；`<name>` 占位，`config.md`「人起的名字」），预设照它（默认关的除外，见下）。预设里对这个功能明写了开或关的（`[features]` 写了它，或者以前的 `[software]` 写了它、它所在的包）照预设，三页只是方便管理；预设没写的照全局开关；全局开关也没写的照 `unlisted`。全局开关只管 `unlisted = "on"` 的预设（2026-10-20 项目主人定）：`unlisted = "off"` 的（出厂的 `dev`）没写的照旧关着，全局开关打开的也不开，`by` 记 `default`，预设明写的照旧照预设；在默认开的预设里全局开关关掉的就没有。只接技能 `skill-`、脚本 `script-`、MCP 服务 `mcp-` 三类功能编号，包自己的功能不接。开关不是预设文件里的：核心造会话、回合开始找预设时照这一轮的配置放进叠好的文件（`PresetFile::following`），不进指纹、不叠层；改了以后快照里记的没开的功能变了，开着的会话下一个回合换上（第 6 条）。头经 `feature.set` 改（`protocol.md`），`preset.get` 每个功能的 `by` 说是谁定的。

**协议**（`protocol.md`）：

- `session.create`、`venue.session` 多 `preset`：不写的照默认找；`venue.session` 找回已有的会话时不看。
- `preset.list`、`preset.get`；原因码 `unknown_preset`、`preset_invalid`。
- `session.created` 多 `preset`（`kernel/events-bodies.md`）；会话列表、`sessions.changed`、`subscribe` 的回应照它写 `preset`，以前的日志没有的不写。

### 怎么走

1. **新会话用哪个预设**：开会话时指定的（`session.create` 的 `preset`；场所会话是桥照场所规则算好交来的 `venue.session` 的 `preset`），没有就照这一刻的 `preset.default`（没写是 `full`）。同一个命令编号再来，照上一次造的那个，不再找。
2. **找**：在三层里照编号找、叠好。编号不合写法：`bad_params`；哪一层都没有：`unknown_preset`，默认预设指着没有的也一样，不悄悄换成别的（Y12）；文件写错：`preset_invalid`，`data.problem` 写明哪一层、哪个文件第几行（例如 `home dev.toml:3: preset.unlisted must be on or off`）；读不了：内部出错，记一行运行日志。找好了才造会话，什么都没找成的什么都不造。
3. **再找人格**：开会话时指定的人格 → `persona.default` → 无人格（`personas.md`「怎么走」第 1 条）。预设不再参与（施工 P-4 上）。
4. **钉在会话上**：`session.created` 记下预设的编号；子会话照父会话的，她派子代理时挑不了预设（只挑得了池和人格，2026-10-08 项目主人定，`tools/subagent.md`）：照编号重新找，找不到、写错了的不派，同人格；父会话是以前造的、没有预设的，子会话也没有。
5. **照预设挑**（施工 P-2 中）：
   - 装了的功能（施工 F-3 上）：读成了的内置包、扩展包带的功能（`miyu_endpoint::packages::features`，没写功能的包整个算一个），加上每个登记上的脚本一个（施工 X-12 中，`script-<工具名>`，端点的 `scripts::features`），随预设的几层交给会话。工具归哪个功能：MCP 服务的、脚本的照它们自己的功能，清单写明了的照写的，包只有一个功能的归它，都不是的照它的包的编号（`Features::of_tool`）。施工 F-3 上以前是：工具目录里有工具的包、`roleplay`、清单装的 `process` 包。
   - 一个功能开着：照「格式」第 4 条的先后（`PresetFile::opens_in`），全局开关照这一刻的配置（施工 X-27）。
   - 工具面：归的功能开着的工具，减去 `[tools]` 关掉的（`PresetFile::keeps`），再照原来的几道筛（场所、子会话、能不能确认）。关了「后台运行」（`basesystem` 的 `background`）的：快照记 `foreground`，`shell` 拿掉 `run_in_background`（`ToolEntry::without`，别的字节不动），执行器不给任务端口（施工 T-1 上，设计 30 第三节第 7 条）；换预设时照新的预设定，以前就有的 `shell` 照旧快照里的原样，调到放到后台的照「这里不能放到后台」说。
   - 脚本调工具（`basesystem` 的 `clicall`，施工 M-6，2026-10-23 项目主人定，设计 `39-脚本调工具.md` MC15）：关着的会话里工具面上每一件的 `clicall` 都当假（`agents/clicall.rs` 的 `on`），`shell` 的说明不接怎么用的那一段、system 没有那一句，和一件都不能在脚本里调的会话一字不差；内核不开通道，`miyu call` 退出码 2。出厂的全部功能、常规开发开着（整个基础系统开着），极简开发关着（没写、`unlisted` 关）。换预设时以前就有的工具照旧原样，只有 `clicall` 照新的预设（这一格不发给模型，工具说明一字不变）：关了的内核当场不开通道，以前接上的那一段照旧留着；打开的要新会话才接上那一段。
   - 记忆：人格记忆 `memory` 没开的、没装的（施工 F-3 上），范围一律 `off`，不管 `session.create` 的 `memory`（走查 E2：开不开归预设，范围归开会话时）。
   - 人设遵循提醒：`roleplay` 没开的、没装的（施工 F-3 上），快照里没有提醒短语、system 没有风格锁。
   - 装了没开的那一行：2026-10-23 删掉（设计 16 Y8，施工 P-OFF）。新会话的 system 不提预设关掉的软件；以前造的快照里拼好的照旧发。快照里的 `preset.off` 照旧记全部没开的功能（`preset.get`、设置页照它）。以前造的快照记的是包的编号，换快照时照现在的功能读（是装了的包、又不是功能编号的换成它的全部功能），一样的算没改，不换（`PresetPin::means_the_same`）。
   - 场所显示不了图片的会话（施工 X-25，`venues.md`「`venue.session`」的 `images`）：清单写了 `needs = ["images"]` 的包整包扣下，和预设关了一样不进工具面、回合开始不问、`system.append` 那一段不拼；不列进上面那一行（那一行是说换个预设就有，这里换了也没有），不进预设的指纹、快照的 `preset.off`。换快照时照旧扣着（`PresetFile::withheld`，施工 IM-4 起照 `Chosen::situated` 定）。
   - 接平台的包在自己平台的场所会话里（施工 IM-4，接入通讯平台设计 06 第 4.6 节第 3 样）：清单 `[connection] platform` 里有场所平台（场所编号第一个 `:` 前面那段）的包整包开着，`[features]`、`[software]`、`[tools]`、全局开关、`unlisted` 都不管它，回合开始照问、`system.append` 照拼、技能照放开；不算进没开的，不进指纹。别的几道筛照旧：显示不了图片扣下的、关掉的包（`packages.<编号>.off`）照旧不开，场所、子会话、能不能确认那几条照工具面自己的规矩。本机的会话、别的平台的场所照预设。平台包在自己平台的群里替她干活，开关在平台自己的后台（im 的功能开关、O-60 的场所插件），预设管的是基础那一层。记在 `PresetFile::own`，同 `withheld`，不是文件里写的、不叠层（`Features::connected_to` 照清单算）。
   - 扩展交的功能（施工 IM-6，`feature.offer`，`extensions.md`「怎么走」第 11 条）：算装了的功能，接在装了的后面（`Features::offering`），列出来的工具归它、不归包自己的功能（`Features::of_tool` 先认它）；开不开同别的功能，照 `[features]`、`[software]`、`unlisted`（出厂「全部功能」开，别的出厂预设关）。会话的 `PresetPlaces::offers` 每回合开始取一次，开着的会话下一个回合照新的算；没交的照原样借出去、不拷。`preset.get` 照交了的列，写法同包自己的功能。
   - 快照记 `preset`：编号、没开的软件和文件的指纹（`policy.md`「拼」第 6 条）。
6. **改了文件**（施工 P-2 下，K3）：每个回合开始，执行器照快照里预设的编号把几层重新找一遍、放进这一轮配置里的全局开关（施工 X-27），指纹或者没开的那几个变了的照新的重拼，下一轮用上，日志里一条内核记的 `session.policy_changed`（`session/actor.md`「换快照」，人格、预设一起改了的换一次）：
   - 工具面照新的预设重新筛；以前就有的那几件照旧快照里的原样（描述、`subagent` 能选的池都不跟着变），新打开的照现在的目录拿。
   - 记忆不跟着换：开不开记忆在开会话时定（L3），新预设里 `memory` 改了也照开会话时的。
   - 角色扮演、装了没开的那一行照新的重新算。
   - 找不着、写错了的照旧用原来的，记一行运行日志；P-2（中）造的快照没有指纹，不换。撤掉换快照那一轮不换回去。
7. **`preset.list`**：几层里所有的编号，照编号排，一个一个找；名字、说明照这个连接的语言挑（挑法同 `persona.list`）；写错了的只带 `problem`（照这个连接的语言说）和知道第几行的 `line`（施工 P-3 再补）。
8. **`preset.get {preset}`**：叠好的各格（施工 P-3 补：只给人要看的）；名字、说明一句字；`unlisted` 是叠好以后的（几层都没写的是 `on`）；`features` 是一个个功能（施工 F-3 下，原来的 `software`、`tools` 换成它）：装了的照清单读的先后，各带开不开和归它的工具（每件的显示名、开不开）；接着是登记上的脚本，一个脚本一个功能 `script-<工具名>`（施工 X-12 中）：名字、说明照脚本开头给人看的几格（`显示名称`、`Display name`、`描述`）挑这个连接的语言，没写名字的照工具名，`tools` 是它那一件；再接着是用得上的技能，一个技能一个功能 `skill-<名字>`（施工 X-11 上，`skills.md`）：名字、说明照技能写在 `metadata` 里给人看的 `display_name`、`summary`，没写名字的照技能的名字，`tools` 是空的；再接着是用得上的命令模板，一条一个功能 `command-<名字>`（施工 X-14 上，`slash.md`「找、盖住、开关」第 3 条）：名字是 `/<名字>`，说明照模板的、照连接的语言挑，`tools` 是空的；写了没装的接在后面（`installed: false`，界面写「没安装」，16 第五节）；每个功能带 `by`：`preset`（预设里写了）、`global`（照全局开关）、`default`（都没写，照 `unlisted`）（施工 X-27），格子见 `protocol.md`「`preset.get`」；`remove` 是删了会怎样（`restore`、`delete`、`null`）。来自哪几层不给。头照它一个功能一个开关画，展开能逐件关（2026-10-09 项目主人定：一个开关下面还能逐件关工具）。
9. **`miyu check`**（`cli/check.md`）：每一层里每一份预设各查各的，上面一层盖住了照样报，种类 `preset`；给人看的那一句照 `preset-problems/<code>`。写了文件的，某一层 `presets/` 下的 `<编号>.toml` 认作预设（两边换成真的路径比），还没有的报读不了。

### 改（施工 P-3 中）

- `preset.set {"preset", "changes"}`、`preset.delete {"preset"}`（`protocol.md`）：只写你那一层（数据根的 `presets/`）的 `<编号>.toml`。不写编号的是新建，编号由核心起 `preset-<n>`（施工 P-3 补，2026-10-08 项目主人定：新建不填编号；新建的什么开关都不写就是全开），改出厂的、包带的就是建同名覆盖、只写改了的项（16 第四节 Y3）。删掉你那一层，下面几层还有的回到它们的样子。
- 一项项在原来的字上改（`miyu_config::edit::apply`），注释、顺序照原样，P-3 上写进去的 `[preset] base` 顺手去掉（施工 P-3 再补：读的时候认出来当没写）；新开的一组在文件末尾另起一张表（`[preset.name]`）。`expect` 照 `miyu_config::plain::json_at` 读的你那一层现在的值比。
- 写之前查：改完的一份照规矩读一遍，再照它叠一遍（`Presets::find_with`：你那一层照改完的字算，不读盘），写错的整条不收，拒绝里带照连接语言的一句（`data.message`，施工 P-3 补）。
- 写盘照配置文件的规矩（`miyu_store::config_file`）：顺着链接写、先写临时文件再替换、替换之前再读一次本体，撞上手改的从头再来，三次还不行的 `preset_conflict`。写成了记一行运行日志 `INFO preset saved`，删了 `INFO preset deleted`；不进配置的留痕日志。
- 只给本机的连接：扩展进程调回 `local_only`。不推 `preset.changed`：两个头都是每次打开重新读（2026-10-08 和两个头对过）。

### 图标（Y14，施工 P-5 做了，2026-10-10；网页、终端对过）

1. **在哪**：预设文件 `[preset]` 里的 `icon`：Lucide 的图标名，小写字母开头，只有小写字母、数字、`-`，最多 64 个。核心只查写法，不查 Lucide 里有没有；头认不出的照没写画一个通用的。不合写法的报 `preset.icon must be a lucide icon name`（照预设别的错的写法，带行号）。
2. **叠**：同 `name`、`summary`，上一层写了的盖下面的。
3. **改**：`preset.set` 的键多 `preset.icon`，值是名字，`unset` 回到下面那一层的。
4. **列出来**：`preset.list` 的每一格、`preset.get` 多一格 `icon`，没写的是 `null`。
6. 图标不进预设的指纹（`PresetFile::digest`）：界面上的事，换了图标开着的会话不换快照、不断缓存。
5. 出厂的两个预设各写一个：常规开发 `box`、全部功能 `boxes`，一对，后一个多一些（网页的会话 2026-10-10 挑的，项目主人验收时不喜欢再换）。

### 出错

| 代码 | 什么时候 |
|---|---|
| `syntax` | 读不成 TOML |
| `unknown_table`、`not_a_table`、`unknown_key` | `[preset]`、`[software]`、`[tools]` 以外的表；这三样不是表；`[preset]` 里别的键 |
| `not_phrases`、`unknown_language`、`empty_phrase` | 「语言到一句话」那一格写错 |
| `bad_unlisted` | `unlisted` 不是 `on`、`off`（`bad_persona` 随 `default_persona` 施工 P-4 上撤了） |
| `bad_feature`、`bad_software`、`not_bool` | `[features]` 的键不合功能编号的写法（施工 F-3 上）；`[software]` 的键不合包编号的写法；值不是开关 |
| `bad_tool`、`not_false` | `[tools]` 的键不是工具名的写法；值不是 `false` |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-policy/src/preset/tests.rs` 的 `the_icon_is_read_layered_and_left_out_of_the_digest`、`crates/miyu-endpoint/tests/persona_avatar.rs` 的 `presets_carry_their_icons_and_one_can_be_changed`（施工 P-5） | 图标的读法、层叠、不进指纹、写错报错；列出来、改、回到出厂的 |
| `crates/miyu-policy/src/preset/tests.rs` | 每一格读对；空文件什么都没有、`unlisted` 照 `on`；逐格叠；每一种写错报对代码和行；工具名的写法；P-3 上写的 `base` 当没写（施工 P-3 再补） |
| `crates/miyu-store/src/presets/tests.rs` | 三层逐格叠；包那一层在出厂和你的中间、一个编号只认一份（施工 P-11 上）；一层也行、空文件也算；没有的、编号不合写法的、写错的写明哪一层；编号照文件名、不重复、照编号排；`check` 每一层各查、写了文件的认得出；出厂两份读得出、零问题、三种语言齐；每一种代码三种语言都有给人看的一句 |
| `crates/miyu-store/src/index/tests.rs` | 索引里存得下、读得回人格和预设 |
| `crates/miyu-endpoint/tests/preset_set.rs`（施工 P-3 中、补、再补） | 不写编号新建、`remove`、改出厂的只写改了的、保留格式、删一项、`expect`、整条不收还带一句、参数不对、删你那一层、一样的值不写（`protocol.md` 测试表）；旧的 `base` 照常列出、下一次写时去掉，写错的一项列表里照连接的语言说、带行 |
| `crates/miyu-config/src/plain/tests.rs`（施工 P-3 中） | 表、行内表、点号连着的键里读一项，没写的、是表的、读不懂的是空的；只有字、开关、数换得成要写的值 |
| `crates/miyu-endpoint/src/connection/tests.rs`（施工 P-3 中） | 扩展进程调了回 `local_only` 的名单 |
| `crates/miyu-kernel/src/facts/tests/gates.rs`、`crates/miyu-policy/src/shown/tests.rs`、`crates/miyu-policy/src/compose/tests.rs` 的 `a_preset_can_leave_out_each_core_line`（施工 C-11） | 环境那几块：都开着的逐字节一样；只关时间、只关工作目录照拆开的写法，都关了不注入；权限、编号、模型各自关得掉；拆开的写法要不了另一半；照预设没开的功能编号读；system 那几行跟着开关 |
| `crates/miyu-endpoint/tests/env_package.rs`、`crates/miyu-policy/src/shown/tests.rs` 的 `absent_*`（施工 ENV-1） | 真核心：全部功能的快照 system 有「关于你」；只关「用户身份」的快照没有「关于你」、别的一字不变；八个都关的连环境那几行也不拼；`package.disable env` 以后新会话的 system 和开关（`Shown`）跟八个都关的一字不差；没装的功能和关了的一样算关，记成 `absent` |
| `crates/miyu-endpoint/tests/presets.rs` | 不写预设照默认、指定的压着默认；找人格的先后；没有的、编号不对的、写错的拒绝、什么都不造，默认预设指着没有的也拒；会话列表、`subscribe` 写 `preset`，以前的日志不写；`venue.session` 带预设造、找回时不看；`preset.list`、`preset.get`；`check` 查预设 |
| `crates/miyu-session/tests/spawn.rs` | 子会话照父会话的预设，父会话载入以后也照 |
| `crates/miyu-endpoint/tests/preset_face.rs`（施工 P-2 中） | 功能全开的工具面全有、没有那一行；开发预设没有记忆三件、范围 `off`（要了也没用）、那一行只写 `memory`；单件关掉的没有、不写那一行；整包关掉的工具全没、写进那一行；角色扮演开着的有提示和风格锁、那一行在风格锁前面，关着的都没有；`preset.get` 的 `software`：装了的、写了没装的，内置的照固定的先后（施工 P-3 补）；没有工具的 `process` 包关着：快照记它、那一行不列，有工具的照列（施工 O-18） |
| `crates/miyu-tool/src/catalog/tests.rs` | 按包登记、以前的名字照现在的包、老写法算基础系统、两个包里同名的照样拒 |
| `crates/miyu-policy/src/preset/tests.rs` | 软件开不开、单件留不留；没开的照编号、不重复；功能照 `[features]`、`[software]`、`unlisted` 的先后开不开，上层盖下层；写了 `[features]` 才进指纹；以前记包编号的快照算同一回事（施工 F-3 上） |
| `crates/miyu-policy/src/features/tests.rs`（施工 F-3 上） | 工具归写明它的功能、只有一个功能的包工具都归它；装了的就是表里的；以前的包编号读成它的功能 |
| `crates/miyu-session/src/actor/persona/preset_tests.rs`（施工 F-3 上加的两条） | 以前的快照记包编号的不换、真改了的照换；`[features]` 关掉一个功能，工具没了、那一行写功能编号 |
| `crates/miyu-endpoint/tests/preset_skills.rs`（施工 X-11 上） | `preset.get` 一个技能一个功能：名字、说明照技能给人看的两格，没写的照名字；常规开发里关着、全部功能里开着、写了关的关着；写过的不再当没装的另列；照预设造的会话只列、只放开开着的，载入以后照样 |
| `crates/miyu-endpoint/tests/preset_face.rs` 的 `preset_get_lists_features_with_their_tools`、`preset_set.rs` 的 `a_feature_is_switched_by_its_id`（施工 F-3 下） | `preset.get` 照功能列、写了没装的接在后面、每件工具的显示名和开不开、单件关掉的；`preset.set` 写 `features.<编号>` |
| `crates/miyu-endpoint/tests/preset_face.rs`（施工 F-3 上加的三条） | 真核心：`[features]` 关掉运行命令；没装人格记忆的范围 `off`、不进那一行；没装人设遵循提醒的没有提醒和风格锁 |
| `crates/miyu-core/tests/tools.rs`（施工 F-3 上） | 出厂的每一件工具都归上一个功能 |
| `crates/miyu-session/src/actor/persona/tests.rs` | 角色扮演没开的换人格以后照旧没有提示和风格锁，换过以后不再换 |
| `crates/miyu-session/src/actor/persona/preset_tests.rs`（施工 P-2 下） | 改了预设重新筛工具面、留着的那一件是快照里的原样、换过以后不再换、整包关掉的写进那一行、重新打开的照现在的目录拿；记忆照开会话时的（开着的照开，关着的照关）；角色扮演打开了提示回来；写错了的、没有指纹的不换 |
| `crates/miyu-endpoint/tests/preset_background.rs`、`crates/miyu-session/src/actor/persona/preset_tests.rs`、`crates/miyu-policy/src/snapshot/tests.rs`（施工 T-1 上） | 关了后台运行的预设：工具面上 `shell` 没有 `run_in_background`、别的参数不动，没有 `jobs`，快照记 `foreground`；硬写放到后台的回「这里不能放到后台」、不派任务；开着的照旧；换成关着的预设记下、以前就有的 `shell` 原样，改回来不记；`foreground` 写在快照最后、读得回来，开着的不写 |
| `crates/miyu-policy/src/preset/switches/tests.rs`、`crates/miyu-session/src/switches/tests.rs`（施工 X-27） | 预设明写的（`[features]`、`[software]` 的包）先，全局开关其次，`unlisted` 最后；`unlisted = "off"` 的预设不照全局开关；只认三类功能；不进指纹；配置项只读三类 |
| `crates/miyu-endpoint/tests/script_pages.rs` 的 `a_global_switch_leaves_an_off_preset_alone`（施工 X-27） | 真核心：全局开关打开的在 `unlisted = "off"` 的预设里照旧没有、`by` 是 `default`；默认开的预设里全局关掉的没有 |
| `crates/miyu-endpoint/tests/script_pages.rs` 的 `a_global_switch_rules_where_the_preset_says_nothing`（施工 X-27） | 真核心：`feature.set` 关掉以后 `preset.get` 的 `by` 从 `default` 变 `global`、明写的照旧 `preset`；开着的会话下一个回合没有那件，新会话没有、明写开的预设照样有；删掉以后回到默认；`skill.list`、`script.list` 的 `on` 跟着变 |
| `crates/miyu-session/src/agents/tests.rs`、`crates/miyu-session/src/actor/persona/preset_tests.rs`（施工 M-6） | 「脚本调工具」：没有预设的、整个基础系统开着的、`unlisted` 开着的能调，极简开发那样没写的、明着关的一件都不能调、工具照旧在；关了的工具面和一件都不能调的目录拼出来的一字不差、没有 system 那一句；换成关着的预设以前就有的说明原样、一件都不能调，改回来又能调 |
| `crates/miyu-endpoint/tests/preset_swap.rs`（施工 P-2 下） | 真核心：改了预设下一轮的工具面变、日志一条内核记的换快照；没改的不换；写错了的不换；重启以后照换上的那一份，载入的会话改了也换 |

### 起草时定的

- 一个预设一份文件，不像人格是目录（16 第三节）：它只是一张开关表（2026-10-08 主会话）。
- `unlisted` 几层都没写的照 `on`（2026-10-08 主会话）：理由见「格式」第 3 条。
- `[tools]` 只收 `false`（走查 C1）；关掉的工具几层叠在一起，上面一层不能把下面关掉的再打开：打开要等单件打开那一步一起定（2026-10-08 主会话）。
- 预设先于人格找：人格要看预设的默认人格（2026-10-08 主会话）。
- P-2（上）预设不进快照：它还不改变请求；P-2（中）让它改工具面时一起进（2026-10-08 主会话）。
- 角色扮演算一个软件、开发预设不开（16 第八节），但不进装了没开的那一行：它没有工具，她不会去用它，列出来反倒像是不许演（2026-10-08 主会话）。施工 O-18 一般化成「只列有工具的」：那一行是让她知道「有这个能力、这次没开」，好说换哪个预设；没有工具的包她用不上，桥这种核心拉起的又不随会话的预设，写「这次的预设里关着」对她是假话（2026-10-08 主会话定）。
- 记忆没开的，范围一律 `off`：开不开归预设、范围归开会话时（走查 E2）；快照里的范围就是照它算好的，载入不用再看预设（2026-10-08 主会话）。
- 装了没开的那一行只陈述是什么、不写该怎么做（26 第十节的规矩）：没开的原因摆在那里，她自己会说换预设（2026-10-08 主会话）。
- 预设是全部功能的开关：`preset.get` 列出装了的每一个软件开不开，功能都做成软件，预设就管得到每一样（2026-10-08 项目主人定）。
- 预设里单独关掉的工具哪里都不出现：不在工具列表里，也不写进装了没开的那一行（2026-10-08 项目主人定）。真模型实测时关了 `shell`，她把跑不了命令猜成了权限问题，主会话问过要不要也写进那一行，项目主人定不写。
- 人格、预设的三层抽成 `layers.rs` 共用（2026-10-08 主会话）。
- 施工 F-3 下（2026-10-09 主会话）：`preset.get` 去掉 `software`、`tools`，只给 `features`，不另带「这个功能除了工具还有别的作用」那种标记：每个功能本来就有自己的开关，头一个功能一行画，展开是它的工具（以前答应终端界面的 `software[i].tools`、`extra` 作废，头恢复时告诉它们）。工具的显示名照各个包给人看的字里已经有的 `tools.<名字>.name`，不另写。写了没装的名字照以前的 `software/<编号>`，现在只剩长期目标用得上。
- 施工 F-3 上（2026-10-09 主会话）：功能表放在 `miyu-policy`（纯逻辑，只认编号），由核心照清单拼、随预设的几层交给会话，不改工具目录：工具归哪个功能在筛的时候查，目录照旧只记包。认不出功能的工具照它的包的编号，和以前按包开关的写法对得上。`[software]` 照认、先认功能编号再认包编号：人格记忆、人设遵循提醒的功能编号和包一样，以前的预设写 `memory = false` 照旧管用。指纹只在写了 `[features]` 时把它算进去，以前的预设文件指纹不变。以前的快照记包编号，比的时候照现在的功能读，常规开发的会话升级以后不为 `onebot` 换成 `qq` 断一次缓存。
- 换预设时以前就有的工具照旧快照里的原样，新打开的照现在的目录拿：留着的不跟着程序升级变，`swappable` 就不用比工具面；新打开的本来就没有旧的一份（2026-10-08 主会话，施工 P-2 下）。
- 换预设时记忆照开会话时的：范围钉在会话上（L3），记忆开着的会话中途关掉，回合库、召回都不好收场（2026-10-08 主会话，施工 P-2 下）。

### 还没有的

- 预设里写配置值（D1：配置清单每一项标跟着人格、预设还是人）：先放着。2026-10-08 项目主人说「预设需要有所有功能的开关，这就是把功能设计成软件的意义」，预设先只做开关；哪一项真要跟着预设走时再定。
- 强调色（`[preset]` 的 `color`）：随 D1 以后。
