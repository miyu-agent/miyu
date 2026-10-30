## 软件包清单

### 是什么

一个软件包一份清单：它是什么、哪个程序、给 `miyu` 加哪个子命令、界面认哪几页、核心怎么拉起它、`miyu check` 怎么查它自己的文件（`05-内核接口.md` 第二节、I9）。核心起来时读一次，头经 `package.list` 列得出装了哪些。之后的几步照它走：包的配置项并进配置清单（9-1 下），`miyu <名字>` 转交给包的程序（9-2），界面照清单找（9-3），核心拉起 `process` 包（9-4），预设按包开关（P-2）。

状态：图纸，施工 9-1（上）做了（2026-10-07 主会话；方向是施工方案第三节 9-1 那一行和 I9，项目主人 2026-10-07 定；清单的形状和终端界面、通讯平台的会话对过）。包的配置项施工 9-1（下）做了（2026-10-07 主会话；设置页的挂法和终端界面的会话对过），列表随 9-1（补）（2026-10-07 主会话，通讯平台的会话要的）。转交子命令、跑包的检查随 9-2，照清单找界面随 9-3，拉起 `process` 随 9-4（上）（另见 `extensions.md`）。种类多内置、小程序，功能、平台接入、依赖施工 F-1 做了（2026-10-09 主会话；方向是设计 `30-插件框架.md`，项目主人同一天定）：这一步只读、只列；内置的包照清单启用施工 F-2 做了（同一天）；预设照功能开关随 F-3。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-config/src/package.rs`、`package/reader.rs` | 一份清单读成样子：每一格的写法，写错的报第一处，带代码、第几行（纯逻辑）；`reader.rs` 一张表一张表地读 |
| `crates/miyu-config/src/package/settings.rs` | `[settings]` 的读法；拼成配置清单的项（施工 9-1 下） |
| `crates/miyu-config/src/package/features.rs` | `[features]` 的读法；没写的照包算一个（`Manifest::features_of`，施工 F-1） |
| `crates/miyu-config/src/package/fields.rs` | 清单格式的真相源（施工 Y-5 末上，冻结）：认的 22 张表、每张表认的格；读的各处照它挡，`fields/tests.rs` 拿中英作者文档 `docs/authors/manifest*` 对它。以后只加不改不删，新格写进这里和两份作者文档，用到的包写 `[package] miyu` |
| `crates/miyu-config/src/package/links.rs` | `[connection]`、`[depends]`、`[recommends]`、`[worker]` 的读法（施工 F-1） |
| `crates/miyu-config/src/package/side.rs` | `[[side]]` 的读法：扩展登记的旁路请求（施工 Q-3），档位只认三个固定池名（`SIDE_TIERS`） |
| `crates/miyu-config/src/package/code.rs` | 读不成时的代码（施工 F-1 从 `package.rs` 挪出来） |
| `crates/miyu-config/src/phrases.rs` | 「语言到一句话」那一格的读法，和人格的名字、说明共用 |
| `crates/miyu-store/src/packages.rs` | 两层在哪、读出所有清单、同编号、子命令名撞了、功能的编号撞了（施工 F-1）；包放状态的目录；包目录（`Found::files_dir`，施工 R-5 三补） |
| `crates/miyu-store/src/packages/conflicts.rs` | 装之前查冲突（施工 F-8 六补，下面「冲突」）：一个包占的名字，和装着的别的包重了的那几处 |
| `crates/miyu-store/src/packages/install.rs` | 装卸时拷进、换下、删掉 `packages/` 里的包目录（施工 F-5 上） |
| `crates/miyu-config/src/package/kinds.rs`、`upgrade.rs` | 照带的表认种类、哪种包能写哪几张表；写了 `kind` 的清单怎么改（施工 F-8 上补） |
| `crates/miyu-endpoint/src/packages.rs` | 核心起来时读一次、记运行日志；`package.list`；照核心自己的模块认撞没撞、拼包的配置项（`settle`，施工 9-1 下） |
| `crates/miyu-core/src/settings.rs` | 起来时照清单拼好包的配置项（`Packaged`），读配置、生成 Schema 和参考文件时并进去（施工 9-1 下） |
| `crates/miyu-store/src/human.rs` | 给人看的字并进包的配置项的名字、说明和组名（`Human::with_packages`，施工 9-1 下） |
| `crates/miyu-endpoint/src/check.rs` | `miyu check` 照磁盘查清单 |
| `heads.toml` | 两个界面（终端 `tui`、网页 `web`）的清单不在核心仓库里，在各自的仓库（`miyu-agent/miyu-tui`、`miyu-agent/miyu-web`）里和编号同名的文件夹 `tui/`、`web/` 下（2026-10-13 起，以前叫 `package/`）；核心仓库只记发版时取哪一份（施工 S-5，设计 `32-仓库拆分.md` 第六节）。发版时清单放进资源目录的 `packages/tui/`、`packages/web/`，程序放在 `miyu` 旁边；核心仓库自己编出来的没带它们 |
| `resources/packages/onebot/`（搬走了） | 接入QQ 的清单原来在这里（施工 O-18 起）；施工 S-6 下随桥搬进 `miyu-agent/miyu-onebot` 仓库，单独发、自己装，核心的发布包不带它，也不进 `heads.toml`（设计 `32-仓库拆分.md` 第九节） |
| `resources/packages/<编号>/package.toml`、`resources/packages/groups.toml` | 出厂的内置包的清单（施工 F-2）。基础系统（施工 BS-1 上中起，设计 44）拆成十八个包：`miyu-base-read`、`miyu-base-write`、`miyu-base-edit`、`miyu-base-glob`、`miyu-base-grep`、`miyu-base-trash`、`miyu-base-shell`、`miyu-base-jobs`、`miyu-base-subagent`、`miyu-base-ask-user`、`miyu-base-todowrite`、`miyu-base-history`、`miyu-base-session-usage`、`miyu-base-sessions`（带 `send_message`）、`miyu-base-packages`（带 `package`）、`miyu-base-show`、`miyu-base-project-docs`、`miyu-base-clicall`，每个一个功能、写明带的工具，功能编号是改名前的短名（`read`、`ask_user`、`session_usage`……）。出厂五个组（施工 FG-1，设计 52 第一节），组表 `groups.toml` 的先后就是预设页上组的先后（`preset.get` 的 `sections`，`presets.md`「功能分组」）：基础系统 `basesystem`（读、写、改、找文件、搜内容、回收站、运行命令、后台、命令行调用工具、子代理、提问、展示、跨会话消息、翻查本会话、用量、软件包十六个包）、任务和定时 `tasks`（`miyu-base-todowrite`、`miyu-base-goal`、`miyu-base-schedule`、`miyu-base-loop`；待办、定时、循环施工 FG-1 从基础系统挪来）、注入环境信息 `environment`（`miyu-base-env`、`miyu-base-project-docs`）、人格 `persona`（`miyu-base-memory`、`miyu-base-roleplay`）、查资料 `research`（`miyu-base-net`、`miyu-base-knowledge`）。包在哪个组照清单的 `groups` 写；组表给名字、说明（中英日）和 `order`：组里的包照用途的先后（施工 PG-1，`package.list` 顶上的 `groups` 照它给 `packages`，`sections` 照它排组里的功能）。写技能和包（`miyu-base-authoring`，只带技能）、画 mermaid（没有功能）不进组。组名也是预设 `[software]` 里能写的词，定了不轻易改；出厂资源的测试守着清单写的组都在组表里、`order` 不多不漏、组名不和包编号撞（`tests/package_groups.rs`）。没有哪个包是必需的。注入环境信息（`miyu-base-env`，施工 ENV-1）不是必需的，八个功能：环境那七块加「用户身份」（`presets.md`「环境那几块」）；人格记忆、知识库（施工 K-1 下）、人设遵循提醒各算一个功能；画 mermaid 不带功能，联网两个功能「网络搜索」「抓取网页」（施工 N-2 上、N-1）；人格记忆推荐小程序 `embed`（施工 R-5 三补）。出厂的包施工 PN-3 起都叫 `miyu-base-<名字>`（设计 48），清单写 `prefix = "<改名前的编号>"`，配置键、功能编号、资源目录照旧是短名；基础系统的工具归哪个包照 `miyu_basesystem::TOOLS` 对照，不再靠工具名等于包编号 |
| `miyu-agent/miyu-embed` 仓库的 `package/embed/` | 「内置语义模型」这个小程序包（施工 R-5 三补，`recall.md` 第四条；S-7 下拆进自己的仓库）：出厂不装，从软件源装（`miyu pkg install miyu-embed`，施工 PN-4 起的编号） |
| `crates/miyu-core/src/lib.rs`、`packages.rs` | 编进来的内置包那张表（`built_in`）；起来时照清单登记工具（`tools`）、查询（`packages::register`）（施工 F-2） |

### 对外的样子

**在哪**：一个文件夹就是一个包（施工 F-8 上，设计 `31-软件包.md` 第二节）：出厂的放资源目录的 `packages/<编号>/`，你装的放数据根的 `packages/<编号>/`（`07-存储.md` 第二节；协议上这一层叫 `home`），清单是文件夹里的 `package.toml`，包自己的文件放在同一个文件夹里（照 `miyu_store::packages::Found::files_dir` 算，用的一方不自己拼）。包带的程序（扩展、小程序、界面）放在文件夹里的 `bin/` 下（施工 F-8 上再补，设计 `31-软件包.md` 第二节），名字照清单写的程序名，Windows 上加 `.exe`；装的时候随包拷进来，本地库记它的哈希。编号就是文件夹名，写法同人格的编号（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个），不合写法的、文件夹里没有 `package.toml` 的不算。包自己在这台机器上的状态放 `<数据根>/state/packages/<编号>/`，包自己建、自己用。

**格式**（TOML；只收下面这些，不认识的表、键报错）。不写种类（施工 F-8 上补，设计 `31-软件包.md` 第二节）：包带了什么看有哪几张表。程序表 `[ui]`（界面）、`[process]`（核心拉起的扩展，9-4，通讯平台的接入也是这一种）、`[builtin]`（内置，代码编在核心里，一张空表）、`[worker]`（小程序，核心按需拉起，施工 F-1）至多一张，写了两张报 `two_programs`；没有程序表、带了 MCP 服务（`[[mcp]]`）的是 `bundle`（施工 X-2，下面「MCP 服务」）；再没有、有 `[mascot]` 的是吉祥物包，有 `[theme]` 的是主题包（施工 F-16）；都没有的也是 `bundle`：只带技能、脚本的包（施工 X-13 上，带的是包文件夹里的 `skills/`、`scripts/`，`skills.md`；人格、预设照约定带在 `personas/`、`presets/` 里，施工 P-11 上，`personas.md`「软件包带的」；知识库照约定带在 `kb/<编号>/` 里，同 `skills/`，施工 K-8，`knowledge.md`「对外的样子」软件包带来的那一节），只写一张 `[package]` 就行；写了 `[command]` 或协议版本、却没有程序表的报 `missing_key`（少了程序表）。吉祥物哪一种包都能带。`package.list` 的 `kind` 照这个推出来（程序表的那一种，不带程序的是 `bundle`、`mascot` 或 `theme`）。

```toml
[package]
version = "0.0.1"                # 可以不写
protocol = [1, 1]                # 说得了的协议主版本 [最低, 最高]，和握手一样
name = { en = "Terminal interface", zh = "终端界面", ja = "ターミナル画面" }
summary = { en = "…", zh = "…" } # 可以不写
needs = ["images"]               # 要这个地方有什么才有用（施工 X-25），可以不写；现在只认 images：显示不了图片的会话里整包当没开
miyu = "0.2.0"                   # 最低要哪一版 Miyu（施工 X-31）：三段数字，可以不写；读清单时先比它，见「出错」的 `needs_newer_miyu`
targets = ["linux-x86_64"]       # 能装在哪几种系统上（施工 G-1，设计 37 第二节第 3 条），可以不写（哪种都能装）；只认 linux-x86_64、linux-aarch64、macos-aarch64、windows-x86_64，带了本机程序的写它能跑的几种
author = "miyu-agent"              # 作者（施工 F-12，项目主人 2026-10-19 定每个包都写），可以不写；一行，1 到 100 个字
upstream = "https://github.com/miyu-agent/miyu"   # 上游源码仓库（施工 F-12），可以不写；https:// 开头，至多 300 个字。`package.list`、`package.info` 原样交出去，另交 `from`（shipped、official、local）

[[command]]                      # 可以没有；一个包可以加几个（施工 F-13），只有一个的照旧写 [command]
name = "tui"                     # miyu 后面敲的那个词：小写字母开头，小写字母、数字、-，最多 32 个
program = "miyu-tui"             # 程序名，不带路径分隔符；第一个子命令的程序是这个包的程序
about = { en = "Open the terminal interface", zh = "打开终端界面" }

[[command]]                      # 第二个：同一个程序，转交时 args 加在人敲的参数前面
name = "shellhook"
program = "miyu-tui"
args = ["shellhook"]             # 可以不写；miyu shellhook install 转交成 miyu-tui shellhook install
about = { en = "Talk to Miyu from your shell", zh = "在 shell 里和她说话" }

[ui]                             # 有它的是界面
opens = ["config"]               # 界面认的页（--page），照配置清单的页名；可以是空的
pages_dir = "pages"              # 页面文件的目录，相对包文件夹（施工 G-15）；网页那种才写

[process]                        # 有它的是核心拉起的扩展，要有 [command]
args = ["serve"]                 # 拉起时带的参数，没写的是空的
start = "manual"                 # manual：开关打开才拉起（默认）；always：核心起来就拉起
capabilities = ["network"]       # 要的扩展能力（施工 9-4 下上，extensions.md「能力」）：只认 extensions.md「能力」那十七个名字，可以不写

[check]                          # 要有 [command]
args = ["check"]                 # miyu check 跑 <program> <args…>（9-2）

[settings.port]                  # 一项一张表，见下面「配置项」
type = "int"
min = 1
max = 65535
default = 8300
layers = ["system"]
name = { en = "Web port", zh = "网页的端口" }
```

施工 F-1 加的几格（设计 `30-插件框架.md`）：

```toml
[package]
groups = ["basesystem"]          # 在哪几个组里（施工 BS-1 上中）：只有出厂的内置包能写；预设写组名就开关组里没单独写的包

[builtin]                        # 有它的是内置包：一张空表

[features.files]                 # 包带的功能，编号写法同包的编号，全局不重
name = { en = "Files", zh = "文件读写" }
summary = { en = "Read, write and search files" }   # 可以不写
tools = ["read", "write", "edit"]                    # 下面的工具，可以不写；同一个包里一件只列一次

[connection]                     # 平台接入：只有扩展包能写
platform = "qq"                  # 写法同包的编号；接几个的写列表 ["qq", "tg"]（施工 IM-3）

[[side]]                         # 旁路请求（施工 Q-3，设计 43）：只有扩展包能写，一段一条，不另要能力
id = "judge"                     # 包里不重，写法同命令名；model.call 写 side = "judge"，model.list 里是 <包>/judge
tier = "lite"                    # 要哪一档：lite、normal、flagship（三个固定池）
name = { en = "Reply judge", zh = "判断要不要回" }
summary = { en = "Decides whether to reply in a group.", zh = "群里有人说话时判断要不要回" }

[depends]                        # 依赖的包：缺了装不上、装着的不起；包编号，什么种类都行，不重复（施工 F-11）
packages = ["onebot"]            # 以前只写小程序，叫 workers，照样认

[recommends]                     # 推荐的包：缺了照装、照起，少一部分本事
packages = ["miyu-embed"]

[worker]                         # 小程序必写，别的种类不能写
program = "miyu-embed"           # 程序名，不带路径分隔符
args = ["serve"]                 # 可以不写
```

一个小程序包的样子：内置语义模型（施工 R-5 三补，`recall.md` 第四条；2026-10-09 项目主人定做成可选的包、只放一个模型、出厂不装；放的模型 2026-10-18 换成 granite-embedding-97m-multilingual-r2，施工 R-5 七补）。人格记忆的清单写 `[recommends] workers = ["miyu-embed"]`，核心照它拉：

```toml
# packages/miyu-embed/package.toml
[package]
protocol = [1, 1]
name = { zh = "内置语义模型", en = "Built-in semantic model", ja = "内蔵の意味モデル" }

[worker]
program = "miyu-embed"
```

模型清单、模型文件放在包目录 `packages/miyu-embed/` 里：`model.toml`（这个小程序认的名字，小程序清单里不另写）、`model_quantized.onnx`、`vocab.txt`。

每种包能写的表（别的写了报 `wrong_kind`）：

| 种类 | 能写的表 |
|---|---|
| `ui` | `[package]`、`[command]`、`[ui]`、`[check]`、`[settings]`、`[depends]`、`[recommends]` |
| `process` | `[package]`、`[command]`、`[process]`、`[check]`、`[settings]`、`[features]`、`[connection]`、`[depends]`、`[recommends]`、`[page]`、`[[mcp]]`、`[[slash]]`（施工 X-14 下）、`[[widget]]`（施工 X-15 上，`widgets.md`）、`[[side]]`（施工 Q-3，`models.md`「旁路请求」）（施工 X-2） |
| `builtin` | `[package]`、`[features]`、`[depends]`、`[recommends]`、`[page]` |
| `worker` | `[package]`、`[worker]`、`[depends]`、`[recommends]` |
| `mascot` | `[package]`、`[mascot]`（施工 F-7）、`[depends]`、`[recommends]` |
| `bundle` | `[package]`、`[settings]`（施工 PS-1）、`[[mcp]]`、`[mascot]`（施工 X-2）、`[depends]`、`[recommends]`；只带技能、脚本的只有 `[package]`（施工 X-13 上） |

施工 F-11 起，什么种类都能写 `[depends]`、`[recommends]`（照 pacman：依赖是包和包之间的事，不管种类）。

施工 F-6 上加的两格（`package-pages.md`「清单多的几格」）：`[package] icon` 是 Lucide 的图标名，哪种包都能写；`[page] dir` 是软件后台页在包目录里的子目录，入口是里面的 `index.html`，只有扩展、内置包能写。

**功能**（施工 F-1）：写了 `[features]` 的照写的先后；写了空的 `[features]` 的一个都没有；没写的内置包、扩展包整个算一个，编号、名字、说明照包的，下面的工具不列（都归它，F-3 照这一条挂）。界面、小程序没有功能。两个包的功能编号撞了：照读的先后（出厂的先于装上的，同一层照编号）先到先得，后到的那一份报 `feature_taken`、整份不收，报在那个功能那一行（照包算的那一个没有行号）。依赖的小程序装没装，这一步只读不查（F-2）。

1. `name`、`summary`、`about` 照人格的写法：只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。
2. `[check]` 的程序在标准输出上一行一处、每处一个 JSON，格子照 `check` 的回应（`kind`、`file`、`line`、`column`、`level`、`message`），可以多带 `key`、`rule`、`source`；`file` 照核心的写法：数据根里的写相对数据根的，资源目录里的写真的路径。只有警告时退出码 0，有错误 1（和通讯平台的会话 2026-10-07 对过）。

**配置项**（`[settings.<名字>]`，施工 9-1 下；`ui`、`process` 包，施工 PS-1 起只带脚本的 `bundle` 也能写，同一张表、同一套画法）：

| 格 | 是什么 |
|---|---|
| `type` | 必写：`bool`、`int`（可以带 `min`、`max`，不写是整数的全部范围）、`number`（小数，施工 PC-1：`min`、`max` 必写、只收整数，进配置清单是 `float`）、`option`（带 `choices`，至少两个不重复的）、`text`（可以带 `max`，最多几个字符，不写是 200；`lines = true` 是几行字，进配置清单是 `lines`、控件 `textarea`，不写 `max` 是 2000，施工 PC-1）、`name`、`url`、`secret`（`{ secret = … }` 那种引用）、`list`（带 `element`，施工 9-1 补） |
| `choices` | 字的数组；或者表的数组（施工 PC-1，设计 53 第一节）：`value` 必写（字），`name`、`description` 照项的写法、可以不写，别的格不收；两种不能混着写，`bad_choices`。存的是 `value`；名字、说明并进字表（`ItemWords` 的 `options`、`option_descriptions`），`config.schema` 的 `options` 每一个照连接的语言带 `name`（没写的是值）、`description`（没写的不带） |
| `step`、`slider` | `int`、`number` 才有（施工 PC-1）：`step` 正数、`int` 的是整数，核心不拿它查值；`slider = true` 要写了 `min`、`max`，控件 `slider`（`miyu_config::Control::Slider`）带一格多少，`step` 不写的 `int` 是 1、`number` 是范围的百分之一；`config.schema` 带 `step`。不画滑块的不带 |
| `element` | 只有列表写，必写：元素的类型，上面除 `list` 以外的一种；元素带的 `min`、`max`、`choices`、`max` 写在同一张表里，管每一个元素 |
| `default` | 合这个类型的值；选项要在 `choices` 里；列表写数组、每一个照元素查，空数组也算；`secret`、元素是 `secret` 的列表不能写。没写的没有默认值 |
| `layers` | 只能写 `system`，不写也是它。项目配置不给包用 |
| `applies` | `now`、`new_session`、`next_turn`、`program_start`（这个程序下次启动时，协议上写 `head_start`）；不写是 `program_start` |
| `name`、`description` | 语言到一句话，`name` 必写 |
| `hidden` | `true` 的设置页不画：照样能写、能查、进 Schema |

1. 名字：小写字母开头，只有小写字母、数字、`_`，最多 64 个。键是 `<前缀>.<名字>`，例如 `web.port`；前缀是 `[package] prefix`，没写的是包的编号（施工 PN-1，设计 48 第二节）。可以多一层组（施工 9-1 再补，接入通讯平台要的）：`[settings.<组>.<名字>]`，键是 `<前缀>.<组>.<名字>`，例如 `im.qq.listen`；组名照名字的写法，只多一层。一张表里有 `type` 或 `name` 的（空的也算）是一项，别的是一组，所以组里的项不能叫 `type`、`name`。设置页照旧在这个包那一组。
2. 核心起来时读清单那一次，读成了的清单的配置项接在核心自己的配置项后面，进配置清单：读配置、`miyu check`、`config.schema`、`config.get`、`config.set`、生成的 Schema 和参考文件都照它。经 `package.install`、`package.remove` 装卸以后照新的清单再拼一次（`Builtins::settings`，`miyu-core` 装上），配置服务换上（`Config::refit`）：配置照手里的字重新认，认得的项、问题变了的推 `config.changed`（`via: package`）；生成的三份照新的清单重写（施工 F-5 补）。卸掉的包，配置里它的项一并删掉（「装卸」第 7 条）；项目目录里还写着的照旧报不认识。
3. 包的前缀是核心自己某一段配置的第一段（`ui`、`persona`、`permission`、`models`、`providers`、`log`、`usage`、`external` 这些，照核心起来时的配置清单认）、又声明了配置项的，这一份报 `settings_taken`，当写错了的列出，配置项一项都不收。
4. 设置页：都在一页 `packages`（「软件包」），一个包一组，组的编号是包的编号、名字是包的名字；每一项的名字、说明用清单里的，照连接的语言挑（这种语言、`en`、`zh`、`ja`），不进 `core/human`。控件照类型：开关 `toggle`、整数 `number`、选项 `select`、列表 `list`，别的 `text`。`config.schema` 里列表照核心自己的列表写：多 `element`，元素是选项的多 `options`。 平台接入的包也在这一页（施工 F-6 上：「接入」页去掉了，设计 30 第十三节）。`config.schema` 里这一页的项多一格 `package`，就是组的编号，头照它把项画在那个软件的信息页上。程序不在 `miyu` 旁边的扩展、小程序当没装，配置项不进（`package-pages.md`「程序不在就当没装」）；界面不算。核心替内置包声明的配置项（现在是人格记忆的几项、联网的几组、基础系统的 `show.system_hint`）也挂在这一页、这个包那一组；包没装的照样认、照样有最终值，设置页不画（`hidden`，施工 F-4，设计 30 第七节）。
5. 脚本包（施工 PS-1）：`bundle` 的配置项照上面登记、画；它自己 `scripts/` 下的脚本跑的时候拿到这几项这一刻的最终值（`MIYU_SETTINGS_JSON`，`skills.md`「脚本」第 3 条），扩展握手交的同一份（`extensions/config.rs` 的 `own`）。

**两层怎么认**：一个包只有一份清单，不像人格那样一层层叠。同一个编号两层都有的，认出厂的，装上的那一份报 `duplicate`。两个包要同一个子命令名的，出厂的先于装上的、同一层照编号，先读到的得，后读到的那一份报 `command_taken`（报在子命令名那一行）；一个包加了几个的，每个都算，有一个被占就整份不收（施工 F-13）。同一个包里写了两个同名的也报 `command_taken`，报在后一个。和内置子命令撞的，由 9-2 在命令行那一头拦。别的包占了的名字装的时候先查过（下面「冲突」），读的时候撞得上的只剩核心升级后出厂的新带了同名的：出厂的先读到。

**用得到的那一份**：核心手里的 `Core::system_packages` 是出厂的、装上的全部，进程、扩展、MCP 服务、装卸照它；会话看得到、用得到的（工具面里预设的功能、技能、脚本列表、命令模板、包带的人格预设、投影里技能的显示名）照 `Core::packages_of`：全部去掉关了的（下面「关掉某个包」），用到时造一份留着，装卸了清掉。装、卸、升级人的连接、不关的会话做得了（`packages/may_manage.rs`），关起来的会话回 `not_permitted`。

**关掉某个包**（施工 U23 末上，项目主人 2026-10-24）：设置 `packages.<编号>.off`（扩展进程的总闸照旧是 `package.disable`）。出厂的卸不掉，照它关。设置页「软件包」页每个装着的包那一组底下一个开关（核心给每个包现拼一项，`miyu_session::packages_off::item_for`）；带占位的那一项不上设置页，接住没装的编号，先关后装就是关着的。关了的包从 `packages_of()` 里去掉（技能、人格预设、命令模板、交给会话的「装了的包」、预设页都跟着没有），会话每一轮照活的配置把它并进预设文件（工具面、技能下一个回合就没有，开着的会话也一样，工具面变了断一次缓存）；看得到的脚本也去掉它带的。施工 BS-1 上中起没有必需的包，哪个都关得掉（原来必需的照 `config_invalid` 拒）。带进程的扩展照跑。它往 system 尾部放的那一段（`system.append`）、它带的 MCP 服务的说明、`user.append` 都跟着工具面走（只给工具在这个会话工具面上的包），关了的包工具不在面上，下一个回合就没有（施工 U23 末下核过，有测试守着）；（施工 U23 末下）它声明的斜杠命令、整页命令、它带的服务的提示词菜单里不列、敲了回 `unknown_command`；设置页（`config.schema`、`config.get`）不给看它的配置项，值照旧留在文件里，它的关包开关照给。扩展按会话推的界面块这一版不筛（扩展不知道谁关了它）。

**包的身份**：核心拉起的扩展连进来替主人干活，是哪个包另记在连接上，能力照批过的挡。清单写了 `[connection] platform` 的是平台桥（`venues/bridge.rs`），场所会话归主人，群和陌生人的私聊关在沙盒里（`venues.md`）。

**协议**（`protocol.md`）：`package.list`，不带参数，交回 `{"packages": [...]}`，照编号排（同编号出厂的在前）。列表里变了的经订阅 `packages` 推 `package.changed`（施工 F-8 三补，`protocol.md`「软件包列表的推送」，端点的 `packages/changes.rs`、`subscriptions/listed.rs`）。

| 格 | 什么时候有 | 是什么 |
|---|---|---|
| `package`、`layer` | 每一项 | 编号；`shipped`、`home` |
| `kind`、`protocol`、`name`、`state` | 读成了的 | `ui`、`process`、`builtin`、`worker`、`mascot`、`bundle`（施工 X-2），照带的表推出来（施工 F-8 上补）；`[最低, 最高]`；照连接的语言挑的名字（挑法同 `persona.list`：这种语言、`en`、`zh`、`ja`）；放状态的目录的真路径 |
| `version`、`summary` | 写了的 | 原样；照语言挑 |
| `features` | `builtin`、`process` 包（施工 F-1） | `[{"id", "name", "summary"}]`：照包算的那一个也列；名字、说明照语言挑，没说明的没有 `summary`；不列工具 |
| `connection` | 写了的（施工 F-1） | `{"platform"}`；接几个平台的 `platform` 是第一个，另带 `platforms`（全部，照写的先后，施工 IM-3） |
| `depends`、`recommends` | 写了、不空的（施工 F-1） | `{"packages": [...]}`，照写的先后（施工 F-11；以前是 `{"workers": [...]}`） |
| `missing` | 依赖的包缺了的（施工 F-11） | 缺的那几个，照写的先后；这时 `status` 是 `depends_missing` |
| `export` | 写了 `[export]` 的（施工 F-11） | `{"methods": [...]}`，照写的先后：别的软件经 `package.call` 调得到的方法 |
| `worker` | `worker` 包（施工 F-1） | `{"program", "args"}` |
| `mcp` | 带 MCP 服务的（施工 X-2） | `[{"name", "command", "venues"}]`，照写的先后；环境变量不列（可能引用密钥） |
| `command` | 有 `[command]` 的 | `{"name", "program", "about"}`，`about` 照语言挑；加了几个的是第一个 |
| `commands` | 有 `[command]` 的（施工 F-13） | 每个子命令一项，同 `command`，照写的先后 |
| `opens`、`pages_dir` | `ui` 包；`pages_dir` 写了的 | |
| `process` | `process` 包 | `{"args", "start"}` |
| `check` | 有 `[check]` 的 | `{"args"}` |
| `icon` | 写了的（施工 F-6 上） | 原样 |
| `page` | 有后台页的（施工 F-6 上） | `true` |
| `status` | 读成了的（施工 F-6 上） | `running`、`starting`、`stopped`、`off`、`ready`、`program_missing` 之一，算法见 `package-pages.md`；依赖的包缺了的（卸掉的出厂的不算）是 `depends_missing`（施工 F-11） |
| `enabled` | 有开关的（施工 F-6 上） | 开关现在开没开：扩展照它的开关，出厂的内置包照有没有卸掉（施工 BS-1 上中起没有必需的包，都有开关） |
| `contents` | 给别的包带了内容的（施工 G-9） | 给哪几个包带了内容：清单里 `[content.<包>]` 的编号，照编号排。im 的商城照它挑 im 的插件 |
| `removed` | 卸掉了的出厂包（施工 F-5 上） | `true`，接在后面 |
| `code`、`problem`、`line` | 写错的、撞了的、读不了的、协议版本对不上的 | 代码；给人看的一句（照连接的语言，`core/human` 的 `package-problems/<code>`，读不了的是 `config/unreadable`）；第几行，有的才有。写错的、撞了的、读不了的只有 `package`、`layer` 和这几格；协议版本对不上的（`code` 是 `protocol_mismatch`）照样带全，头自己决定用不用 |

**`miyu check`**（`cli/check.md`）：多查两层里每一份清单，种类 `package`，写法同人格；协议版本对不上的是警告，写了后台页、目录里没有 `index.html` 的也是警告 `page_missing`（施工 F-6 上）。写了文件的，某一层 `packages/` 下的 `<编号>/package.toml` 认作清单（两边换成真的路径比，施工 F-8 上），还没有的报读不了；包目录里别的文件不认。

### 怎么走

1. **核心起来时读一次**（`Core::new`）：两层的目录读不了的当没有。写错的、撞了的各记一行运行日志 `WARN package invalid package=… file=… error=…`，读不了的 `WARN package unreadable`；照样起来。读完照编进来的内置包那张表标一遍（施工 F-2）：清单是内置包、这一份核心没编进它的代码的，照读坏了的报 `not_built_in`。
   - **内置包照清单启用**（施工 F-2，设计 30 第二节第 3 条）：有读成了的清单、种类是内置的才算装了。装了的才登记：基础系统组的各个包（施工 BS-1 上中起一包一包登记，`miyu_basesystem::packages`）、人格记忆的工具进工具目录，画 mermaid、联网的查询进查询表；没装的工具、查询都没有（查询照 `unknown_method`）。经 `package.install`、`package.remove` 装卸的当场换（「装卸」）；手改了磁盘上的清单的，照旧要重启核心才认。
2. **读一份**：照「格式」查，报第一处；必写的少了报在表头那一行（缺 `[package]` 的整份，没有行号）。
3. **`package.list`**：照起来时读到的答，名字照这个连接的语言挑。
4. **`miyu check`**：照磁盘上现在的读，改了马上查得出。
5. **转交**（施工 9-2，`cli/main.md`「怎么走」第 0 条）：`miyu <名字> …` 不是内置的子命令，照磁盘读两层的清单，`[command]`（几个里的任一个，施工 F-13）的名字是它的那一个包：程序先找这个包文件夹的 `bin/`，再找 `miyu` 真实位置旁边的（`miyu_store::packages::locate`，施工 F-8 上再补：别人做的包自己带程序，随 Miyu 装的那几个在 `miyu` 旁边；包文件夹是相对路径的不找 `bin/`；不找 `PATH`，别的程序冒充不了，2026-10-01 项目主人定；Windows 上这两处都没有 `<名字>.exe` 的，再找包文件夹的 `bin/<名字>`（没有后缀的文件），照它第一行的 `#!` 起：解释器写的是这台机器上在的绝对路径的照它，别的照名字在 `PATH` 里找（`/usr/bin/env python3` 是 `python`），同脚本工具，没有 `#!` 的当没找到；Linux、macOS 照旧直接起，系统认 `#!`、文件要有可执行权限，施工 X-34。找法交回程序和接在前面的几个参数，扩展、转交、头、`miyu web`、小程序、包自己的检查都照它起，脚本工具也用它找解释器），参数、环境、标准输入输出原样（环境只多一个 `MIYU_PACKAGE_DIR`，是这个包的文件夹，同核心拉起扩展，施工 S-6 上；`MIYU_PACKAGE_STATE`，是这个包的状态目录 `state/packages/<编号>/` 的真实位置，跑之前建好，建不成的说一句、退出码 1、不跑，施工 F-15），Unix 上换成它，Windows 上起它、等它，退出码照它的；清单写了 `args` 的，加在人敲的参数前面（施工 F-13）；`miyu help <名字>` 转成 `args` 加 `--help`（`miyu help shellhook` 是 `miyu-tui shellhook --help`）。没找到程序说没装、退出码 1；找到了起不来的（Windows 上解释器没装）说跑不起来、照它的原话，退出码 1（施工 X-34）。撞了内置子命令的：内置的优先，不转交，帮助页不列；装包时拦随装包那一步。`miyu -h` 多一节「软件包加的命令」。 另设 `MIYU_PROGRAM`：主程序 `miyu` 的真实位置，装进数据根的包程序旁边没有主程序，要拉起核心的照它跑 `<它> core`（施工 G-15）。
6. **跑包的检查**（施工 9-2，`crates/miyu-endpoint/src/check/run.rs`）：核心的 `check` 不写文件时，照起来时读到的清单，有 `[check]` 的每个包跑 `<程序> <args…>`（程序同第 5 条找），标准输入是空的、标准错误不要、环境照核心的，最多等 30 秒、收 1 MiB。标准输出一行一个 JSON：`kind`、`file`、`level`（`error`、`warning`）、`message` 必有，`line`、`column`（正整数）、`code`、`key`、`rule`、`source` 有的才收，别的格不收，接在核心自己查的后面，照包的编号的先后。退出码 0、1 是正常的；别的、被信号杀掉的、跑不起来的、到时没完的报一条警告 `check_failed`；程序没找到的报 `check_unavailable`；有看不懂的行的报 `check_output`（几行），都写清单的位置。写了文件的照旧只认核心自己认得出的。
7. **入口**（施工 9-3，`cli/main.md`「怎么走」第 3 条）：不带子命令的 `miyu`、不带子命令的 `miyu config` 在终端里时照配置 `ui.head` 找界面包拉起，`miyu config` 带 `--page config`，要清单的 `[ui] opens` 认 `config` 这一页；`miyu web` 照子命令是 `web` 的那一份找网页软件。程序照第 5 条找。

### MCP 服务（施工 X-2，`mcp.md`「对外的样子」）

```toml
[[mcp]]
name = "github"                                   # 服务名：小写字母开头，只有小写字母、数字、-，最多 32 个
command = ["npx", "-y", "@modelcontextprotocol/server-github"]   # 第一项是程序，后面是参数，不经 shell
env = { GITHUB_TOKEN = { secret = "github" } }    # 可以不写：值是字，或者 { secret = … }、{ env = … }
venues = ["local"]                                # 可以不写：local、private、group 里的几个，不写是只给本机的会话
timeout_ms = 600000                               # 可以不写：一次调用等多久，1000 到 3600000，不写是 600000
```

1. 一段一个服务，照写的先后；同一个包里服务名不重（重了 `bad_mcp`）。扩展包和 `bundle` 能写，别的种类写了 `wrong_kind`。`bundle` 只能写 `[package]`、`[[mcp]]`、`[mascot]`，不说协议（写了 `protocol` 报 `wrong_kind`：带的服务说的是 MCP，不是 Miyu 的协议）。
2. 一个服务一个功能（2026-10-11 项目主人照推荐定，`mcp.md`）：编号 `mcp-<服务名>`，名字是服务名，接在包自己的功能后面（`Manifest::features_of`）；`bundle` 自己不另算一个。两个包带了同名的服务，功能编号撞了，后读到的那一份报 `feature_taken`、整份不收、报在那一段。这个服务的工具名是 `mcp__<服务名>__<工具名>`（X-3 登记），照前缀归它的功能；包自己的工具照包自己的功能算，MCP 服务的功能不算在「只有一个功能」里（`miyu_policy::features::Features::of_tool`）。
3. 列出来：`package.list` 的一项多 `mcp`，`features` 里有每个服务的那一个；看一眼（`preview: true`）多 `mcp`（服务名），`bundle` 没有 `program`；`miyu pkg install` 照它印「MCP 服务 <名字>」。包文件夹里带的技能、脚本（施工 X-13 上）也列：看一眼多 `skills`（技能名）、`scripts`（工具名）、`slash`（斜杠命令模板的名字，施工 X-14 上补）、`personas`、`presets`（编号，施工 P-11 下），开头读得成的才列；读不成的另列在 `skipped`（施工 X-13 补：`file` 是包文件夹里的相对路径，`why` 英文一句）；`miyu pkg install` 照它印「技能 <名字>」「脚本 <名字>」，读不成的印「读不成：<路径>（为什么）」。
4. 起服务、登记工具、开关随施工 X-3：带了服务的 `bundle` 照扩展的办法开关（只带技能、脚本的没有开关，施工 X-13 上），开关记在同一份 `extensions.json`，没写的关着；`status` 照它带的服务合起来算，`enabled` 照开关（施工 X-3 上，`mcp.md`、`package-pages.md`「开关」）。扩展包带的服务跟着扩展起停（施工 X-3 下）；卸包时服务的缓存、工作目录一起删。

### 吉祥物包（施工 F-7，2026-10-11 项目主人定，终端界面的会话转来）

终端界面的吉祥物做成专门的一种包，在配置页「通用 › 终端界面 › 吉祥物」里换；长相、颜色、小动作都写在模型文件里，格式由终端定（终端的图纸 `tui.md`「吉祥物包」），核心只认它在哪。

```toml
# packages/pudding/package.toml
[package]
name = { zh = "布丁", en = "Pudding" }

[mascot]
model = "mascot.json"   # 包目录 packages/pudding/ 里的相对路径
```

1. **清单**：只带吉祥物的包只能写 `[package]`、`[mascot]`（另可写依赖），`[package]` 里可以写 `icon`；不说协议，写了 `protocol` 报 `wrong_kind`，写了别的表照样 `wrong_kind`。`[mascot]` 只有 `model`、必写，写法同 `[page] dir`：包目录里的相对路径，不许 `..`、开头的 `/`，写错 `bad_mascot_model`。施工 F-8 上补起带程序的包也能多带一张 `[mascot]`：`kind` 照程序那一种，多一格 `mascot`。
2. **列出来**：`package.list` 的一项多 `mascot: {"model"}`，没有 `protocol`。
3. **读模型**：`package.file {"package", "path"}` 对吉祥物包只给 `model` 那一份（`path` 照原样写），照真实路径找，链接出了包目录的不认；别的路径 `file_not_found`。分块、512 KiB 一块照旧。
4. **`miyu check`**：模型文件不在（照真实路径，出了包目录的也算不在）`mascot_missing`，超过 256 KiB `mascot_too_big`，不是 JSON 的对象 `mascot_not_json`，都是错误，写清哪个包、哪个文件。模型里面写得对不对由终端查。
5. **装卸照常**：`package.install`、`package.remove`；没有开关（`package.enable`、`package.disable` 回 `not_switchable`）、没有配置项、不拉起程序。终端照配置项 `tui.mascot`（终端的清单 `[settings.mascot]`，填包的编号）挑用哪一个。

### 主题包（施工 F-16，2026-10-21 项目主人定，终端界面的会话转来）

终端界面照网页的「皮肤」做一层变量化，叫「主题」：框线、记号、修饰、留白、动效一张表，做成专门的一种包，默认主题也是一个包（终端的图纸 `tui.md`「主题」）。格式由终端定，核心只认它在哪，同吉祥物包。

```toml
# packages/miyu-tui-theme-ascii/package.toml
[package]
name = { zh = "纯字符", en = "ASCII" }

[theme]
file = "theme.json"   # 包目录里的相对路径
```

1. **清单**：主题包只能写 `[package]`、`[theme]`（另可写依赖），`[package]` 里可以写 `icon`；不说协议，写了 `protocol` 报 `wrong_kind`；写了别的表（`[command]`、`[mascot]` 都算）照样 `wrong_kind`：主题包不带吉祥物，别的包也带不了主题。`[theme]` 只有 `file`、必写，写法同 `[mascot] model`，写错 `bad_theme_file`。
2. **列出来**：`package.list` 的一项 `kind` 是 `theme`，多 `theme: {"file"}`，没有 `protocol`。装之前的看一眼多 `theme: true`。
3. **读主题**：`package.file {"package", "path"}` 对主题包只给 `file` 那一份，照真实路径找，链接出了包目录的不认；别的路径 `file_not_found`。
4. **`miyu check`**：主题文件不在 `theme_missing`、超过 256 KiB `theme_too_big`、不是 JSON 的对象 `theme_not_json`，都是错误（同吉祥物，一个函数查，`packages/status.rs` 的 `data_problem`）。主题里面写得对不对由终端查。
5. **装卸照常**，没有开关、配置项、程序。终端照自己的配置项 `tui.look`（填主题包的编号）挑用哪一个。

### 给别的包带的内容（施工 X-22，设计 `34-表情包.md` 第三节「内容包」）

一个包给另一个包带一个目录的东西（表情包合集、人设卡、主题）：核心不认里面是什么，只管谁给谁带了哪个目录、让被带的那个读得到。

```toml
[content.miyu-memes]      # 给哪个包带：键是它的编号
dir = "memes"             # 包目录里的相对目录，写法同 [page] dir
```

1. 只有 `bundle`、扩展包能写；可以写几张，给几个包带。键不是包的编号、不是表、`dir` 不是包目录里的相对目录、少了 `dir` 的整份清单不收（`bad_content`、`missing_key`、`unknown_key`）。被带的包没装的不报错：装上以后它就读得到。
2. 被带的那个包的扩展（核心拉起的）经 `content.list`、`content.read` 读（`protocol.md`），只读得到写明给它的那个目录；不要能力。包自己给自己带的照样列。
3. 核心每重读一次清单（装、卸、升级、装回出厂的包），给它带内容的包前后有一个就给它推 `content.changed`（`{}`）。同版本重装也推（挂在重读上，不挂在列表的推送上）；前后都没有的不推。扩展照 `content.list` 的 `version`、自己记的摘要比，变了的重读。

### 依赖（施工 F-11，设计 `31-软件包.md` 第三节，2026-10-17 接入QQ 的会话要的：附加包靠接入QQ 的 `[export]` 干活）

1. **写法**：`[depends] packages`、`[recommends] packages` 写别的包的编号，什么种类都行；以前的 `workers` 照样认，和 `packages` 读进同一份（同一个编号两边都写了报 `bad_dependency`）。不做版本约束（pacman 的 `onebot>=1.2`），写了报 `bad_dependency`。
2. **装**：`[depends]` 里有没装的（读成了的才算装了，升级时它自己那一份不算）拒绝 `depends_missing`，`data.missing` 是缺的那几个，什么都不留；看一眼（`preview`）、装回卸掉的出厂的同样查。软件源做好以后再定要不要自动装依赖。`[recommends]` 缺了照装。
3. **卸**：别的装着的包依赖它的拒绝 `package_depended`，`data.by` 是依赖它的那几个，照编号（照 pacman）；`package.disable` 关出厂的内置包也走这里。
4. **装着的缺了依赖**（手删了文件、出厂的被卸了、升级以后多依赖了一个）：扩展不拉起（核心起来时、装卸以后照新的清单拉起的那一批都不算它）；`package.list` 的 `status` 是 `depends_missing`、多一格 `missing`；`miyu check` 报 `depends_missing`（错误，说缺谁）。
5. `package.list` 的每一项另带 `export`（写了 `[export]` 的方法名）：接入QQ 这类扩展照它找挂在自己身上的附加包，不用另外登记。
6. **软件源索引里的四格**（施工 IN-1，设计 `49-装包照pacman.md` 第三节第 1 条）：软件源 `index.json` 的每个包多 `recommends`（清单 `[recommends]` 的编号，照写的先后）、`author`、`upstream`（清单 `[package]` 的原样）、`installed`（包文件解开以后所有普通文件的字节数之和；分系统有几份文件的取最大的那份）。收录（`miyu-index`）读清单、解包时算出；没有的不写（`recommends` 空、其余三格 `None` 都不出现在 JSON 里）。以前生成的索引没有这四格，照旧读得进来，读成空；旧的头读新索引时不认识的格子照略过。签名、`index.json` 的写法不变。

### 本地库（施工 F-8 中上，设计 `31-软件包.md` 第四节）

照 pacman 的 `local/`（`miyu_store::packages::local`，端点的 `packages/local.rs`）：

1. **在哪**：`<数据根>/state/packages/.local/<编号>/`。点开头：和包自己放状态的 `state/packages/<编号>/` 在同一层，编号不会撞上（设计 31 原来写的 `local/` 会撞上编号叫 `local` 的包）。
2. **记什么**：`desc` 是一个 JSON 对象（`id`、`version` 写了的才有、`installed` 装的时刻、`source` 从哪装的、`size` 总字节数、从软件源装的另有 `origin` 来源仓库（施工 G-4 上，升级照它判断））；`files` 一个文件一行 `<SHA-256> <字节数> <相对包目录的路径>`，路径用 `/` 分开、照路径排、放最后（带空格也认得）。写的时候整份先写进 `.<编号>.new` 再换上。
3. **什么时候写**：装成、升级成的照装好的包目录现算一份，记不成的记一行 `WARN package not recorded`、装照算成；卸掉的删掉那一份。核心起来读清单时，装上的读成了、还没记的补一份（`source` 是空的），记 `INFO package recorded`。出厂的不进本地库（资源目录归系统的包管理）。
4. **答什么**：`package.info`、`package.files`（`protocol.md`），装上的照本地库（没记的现算），出厂的照现在的文件现算。`package.info` 另照清单答名字、说明、依赖、推荐、带了什么（同装之前看一眼的「包含」）、要的能力，照装着的包答「被依赖」（`required_by`），从软件源装的答来源仓库（施工 IN-2，设计 `49-装包照pacman.md`）；没装的照软件源答（端点的 `packages/source/look.rs`：同从软件源装的找法，先拿索引、照先后找各个源，找到第一个就用，文件挑本机能用的那一份），照 pacman 的 `-Qi`、`-Si`。
5. **查**（施工 F-8 中下，端点的 `packages/verify.rs`）：`package.owns` 照真实的位置看路径在哪个包的目录里、本地库里记没记；`package.check` 照本地库比：改了的（哈希不一样）、少了的、多出来的（包自己运行时写的也算，只是提示）。`miyu check` 不写文件时也报：改了的 `files_modified`、少了的 `files_missing`，各一条警告，`detail` 是几个，写在那个包的清单上。

### 装卸（施工 F-5 上，设计 `30-插件框架.md` 第九节）

1. **只动装上的那一层**（`miyu_store::packages::install`，`packages/`）：装是把整个包目录拷成 `<编号>/`（施工 F-8 上）；先拷到点开头的暂存处（`.<编号>.new`）再换进去，原来就有的先挪到点开头的备份处（`.<编号>.old`），装成了删备份、装不成放回去；换进去那一下改名失败的，当场把备份放回去（施工 AE-7）。卸装上的是删掉这个包目录。卸出厂的是在 `packages/` 记一笔 `<编号>.removed`（空文件，像 systemd 的 mask），资源目录不动；装回来是删掉这一笔。
   - **起来时收拾**（施工 AE-7，发布前第二轮审查 R2-4）：两次改名之间核心被杀，`<编号>/` 没了、原来那一份还在 `.<编号>.old`。核心起来时、读清单之前（`core.md`「起来的先后」第 5 步）照 `install::recover` 收拾 `packages/`：这时没有在装的，名字是 `.<编号>.old`、`.<编号>.new`（编号照人格、预设的写法）的都是上一次剩下的。`<编号>/` 不在、`.<编号>.old` 在的放回去，记一行 `WARN half-done package upgrade put back package=<编号>`；别的删掉；放不回去、删不掉的记一行 `WARN package leftover kept`，不耽误起来。别的名字一个都不碰。
2. **当场生效**：装、卸以后照两层重读、标没编进来的内置包、认配置项撞没撞，换掉核心手里的那一份（`Core::reload_packages`）。`package.list`、预设的功能、新开的会话、开着的会话下一个回合都照新的。内置包的工具照 `Builtins` 端口（`miyu-core` 装上）重新要，新装上的换进工具目录、卸掉的拿掉并记下随包卸掉了（施工 F-5 中，`Catalog::placing`、`removing`）：用过它的会话工具面不变、调到时报「已卸载」，开着的会话下一个回合拿到新装上的包的工具。查询记着属于哪个包，包没装的当没有（`unknown_method`）。扩展进程照装卸前后的清单停下、拉起、升级了的重起，经提供者登记的工具随包卸掉的同样报「已卸载」（施工 F-5 下，`extensions.md`「怎么走」第 7 条）；卸是先停用着它的再删文件（施工 F-5 补）。配置项照新的清单当场换（「配置项」第 2 条，施工 F-5 补）。人格记忆装没装最先照新的清单设（`Memory::set_installed`，施工 R-10，`memory.md` 第十一条）：开着的会话照它交不交摘要、抽不抽。本机的向量模型照新的清单当场换（`Builtins::embed` 拼、`Vectors::replace_local` 换，一样的不动，施工 F-5 再补）。这几样走同一个入口（`Core::switch_packages`）：卸包、升级在动文件以前照去掉它的清单换一遍，删不成、换不成的照原来的换回来。
3. **装之前查**：路径要是绝对的包目录（文件夹名就是编号），或者包目录里的 `package.toml`（施工 F-8 上）；照规矩读得成；编号不和出厂的撞；关起来的会话装不了（`packages/may_manage.rs`）；带的名字不和装着的别的包撞，撞了的报 `package_conflict`、什么都不动（施工 F-8 六补，下面「冲突」）。拷进去以后照两层重读一遍，这一份还是读坏了的、是核心没编进来的内置包的，撤回（原来那一份放回去）、报 `package_invalid`。
4. **卸之前查**：没装的 `unknown_package`（施工 BS-1 上中起没有必需的包，`package_required` 去掉了）。
5. **卸掉的出厂的**：读两层时不算装了（`Packages::read`）；`package.list` 照样列它，带 `removed: true`（`Packages::read_removed`），好让头给人装回来。
6. 装、卸一次只做一件；只给本机的人用，扩展进程调回 `local_only`；做成了记一行运行日志 `INFO package installed`、`package removed`、`package restored`。
7. **卸就是清干净**（施工 F-8 中下补，设计 31 第三节第 3 条）：换下清单以后，配置里这个包清单列的项照原文删掉（`miyu_config::edit` 的 `Unset`，空了的表头一起删，注释、别的项不动），走 `config.set` 那一条写盘、记日志、推 `config.changed`，改的人记成主人（端点的 `config/forget.rs`）；包的状态目录 `state/packages/<编号>/` 整个删掉；它带的 MCP 服务的目录缓存 `state/mcp/<服务名>.json`、工作目录 `state/mcp/<服务名>/` 一起删（施工 X-3 下）；它带的脚本的状态目录 `state/scripts/*/<编号>/` 一起删（施工 X-44）。装上的、出厂的都一样，重装回来从头设。不碰的：密钥文件（一条密钥可能别处也在引用）、项目目录里的 `.miyu/config.toml`、群里的会话（是人的数据，施工 SU-4 中起在主人名下）。写不进、删不掉的记一行 `WARN`，卸照算成。 开关的文件 `system/extensions.json` 里它开没开、批过的能力也删掉（施工 F-8 五补）：装回来从头开、从头批，不会一装上就照以前的开关起来。
8. **先看一眼**（施工 F-8 下补，端点的 `packages/preview.rs`）：`package.install`、`package.remove` 带 `preview: true` 的照真做之前查的那几样查一遍（装、看一眼共用 `manage.rs` 的 `source`，卸、看一眼共用 `removable`），交回要做什么（`protocol.md`），文件、配置、清单一样都不动；和装卸一样排队（拿同一把锁），看到的是前一件做完的样子。命令行照它印一份、问一句（`cli/pkg.md`）。

### 冲突（施工 F-8 六补，设计 `31-软件包.md` 第四节最后两条、定了的 I、J）

1. **查什么**：一个包占的名字（`miyu_store::packages::conflicts`）：`[command]` 的子命令名（加了几个的每个都算，施工 F-13）；功能（`Manifest::features_of`）的编号，MCP 服务的那一个算服务名；功能下列的工具名、`scripts/` 里脚本的工具名；`skills/` 里的技能名；`kb/` 里库的编号；`commands/` 里斜杠命令模板的名字（施工 X-14 上补，`slash.md`）、清单 `[[slash]]` 声明的命令名（施工 X-14 下，和模板一个名字空间）、`[[widget]]` 整页的命令名（施工 X-15 上，同一个名字空间）；`personas/` 里人格的编号、`presets/` 里预设的编号（施工 P-11 下）；前缀（`Manifest::prefix_of`，没写的照包编号，施工 PN-1）。十样各算各的：技能 `notes` 和库 `notes` 不算撞。
   人格、预设还和出厂、你那一层有的比（施工 P-11 下，设计 31 第十节第 4 条）：撞了的写 `layer` 不写 `package`；这个包以前卸掉时留下的改动（`state/packages/.leftovers.json`）、升级时装着的这一份已经带着的不算。卸包时它带的人格、预设在你那一层有改动的记进 `.leftovers.json`，重装它划掉；`miyu check` 对包没装着、改动还在的报 `persona_left`、`preset_left` 警告（`file` 是那一份改动）。
2. **跟谁比**：两层里清单读成了的、编号不是它的包；斜杠命令模板另和核心内置的命令名、别名比（`Commands::reserved`，记在拥有它的包上：记忆的 `miyu-base-memory`，长期目标的 `miyu-base-goal`，待办的 `miyu-base-todowrite`，循环的 `miyu-base-loop`；`/clear`、`/stop`、`/workspace` 是核心自带的，撞了写 `core: true`，施工 BS-1 下下；拥有它的包自己不算，装回人格记忆不和 `/remember` 撞；包没装的命令不比，名字空出来）。升级同一个包不和自己比；读坏了的、卸掉的出厂的不比；你自己放的技能、脚本、MCP 服务不比（自己的赢，设计 31 第五节第 3 条）。
3. **什么时候**：`package.install` 从路径装、装回出厂的，在读清单、比编号以后，动文件以前；`preview: true` 一样查。撞了的报 `package_conflict`，`data.conflicts` 一处一条 `{"kind", "name", "package"}`（`kind` 是 `command`、`feature`、`tool`、`skill`、`mcp`、`knowledge`、`slash`（斜杠命令模板，施工 X-14 上补）、`persona`、`preset`、`prefix`（施工 PN-1）），照哪一样、名字、包排（`protocol.md`）。查要读包文件夹里的技能、脚本、库，放在阻塞线程里。
4. **以前**：装完重读清单才认，谁先到照编号算：编号排在前的新包装得上，被判撞了的是装着的那个，悄悄失效。现在新来的让。
5. **读的时候**：撞得上的只剩核心升级后出厂的新带了同名的（定了的 J）：出厂的先于装上的，同一层照编号。子命令、功能、前缀撞了的装上的那一份整份不收（`command_taken`、`feature_taken`、`prefix_taken`）；技能、脚本、知识库跳过装上的那一份，技能、脚本 `miyu check` 报被谁盖住（`skills.md`）。

### 出错

| 代码 | 什么时候 |
|---|---|
| `syntax` | 读不成 TOML |
| `unknown_table`、`not_a_table`、`unknown_key` | 不认识的表、该是表的不是表、表里不认识的键 |
| `missing_key` | 少了 `[package]`、程序表和 `[mascot]` 都没有（施工 F-8 上补）、`protocol`、`name`、`[command]` 的三格；功能的 `name`、`[connection] platform`、小程序的 `[worker]`、`program`（施工 F-1；少了 `[worker]` 的整份，没有行号） |
| `wrong_kind` | 写了这种包不能写的表（「格式」那张表）；`groups` 写在不是内置包的清单里（施工 BS-1 上中；原来的 `required` 同理，现在写了是 `unknown_key`） |
| `bad_group` | `[package] groups` 里的组名写法不对、重复了（施工 BS-1 上中） |
| `needs_command` | `[process]`、`[check]` 没有 `[command]` |
| `two_programs`、`bad_protocol`、`bad_start` | 程序表 `[ui]`、`[process]`、`[builtin]`、`[worker]` 写了不止一张（施工 F-8 上补，原来的 `bad_kind` 去掉）；`protocol` 不是两个非负整数、最低不大于最高；`start` 不是 `manual`、`always` |
| `not_text`、`not_texts` | `version` 不是字；`args` 不是字的数组 |
| `not_phrases`、`unknown_language`、`empty_phrase` | 「语言到一句话」那一格写错 |
| `bad_command_name`、`bad_program`、`bad_page`、`bad_pages_dir` | 子命令名、程序名、页名、页面目录的写法不对（`pages_dir` 不能是绝对路径、带 `..`、`\`、`:`） |
| `duplicate`、`command_taken` | 两层同编号的装上的那一份；子命令名被先读到的占了 |
| `bad_setting_name`、`bad_type`、`bad_element`、`bad_default`、`bad_choices`、`bad_range`、`bad_layers`、`bad_applies`、`not_bool` | 配置项写错：名字、类型、列表的元素（写错、写成 `list`、不是字，施工 9-1 补）、默认值、选项、范围、几层、什么时候生效、`hidden`（施工 9-1 下） |
| `settings_taken` | 包的前缀和核心自己的配置撞了、又声明了配置项（施工 9-1 下；PN-1 起照前缀） |
| `bad_prefix`、`prefix_taken` | `[package] prefix` 写法不像包编号；前缀被先读到的包占了（出厂的先于装上的，同一层照编号；没写的照包编号算）（施工 PN-1） |
| `bad_capability` | `[process] capabilities` 里有不认识的、重复的名字（施工 9-4 下上） |
| `bad_feature`、`bad_tool`、`bad_platform`、`bad_dependency` | 功能的编号写法不对；功能下的工具名写法不对（英文字母、数字、`_`、`-`，1 到 64 个）、同一个包里列了两次；平台名写法不对；依赖的包编号写法不对、重复（施工 F-1） |
| `feature_taken` | 功能的编号被先读到的包占了（施工 F-1） |
| `not_built_in` | 清单是内置包，这一份核心没编进它的代码（施工 F-2，核心起来时标；`miyu check` 不查） |
| `bad_icon`、`bad_page_dir` | `[package] icon` 不是 Lucide 图标名的写法，`[page] dir` 不是包目录里的相对目录（施工 F-6 上） |
| `bad_logo` | `[package] logo` 不是包目录里一个 `.png`、`.webp` 的相对路径（施工 F-18 上，`package-pages.md`「清单多的几格」）；图本身不合规矩的在 `miyu check` 报警告 `logo_missing`、`logo_not_image`、`logo_too_big`，包照装 |
| `bad_export` | `[export] methods` 里有不是方法名的、重复的（施工 X-18） |
| `bad_mcp` | `[[mcp]]` 写错了（施工 X-2）：服务名、命令、环境变量、给哪种会话、时限，同一个包里服务名重了；`detail` 是哪一格（`mcp.<服务名>.<格>`）、报在那一段 |
| `bad_slash` | `[[slash]]` 写错了（施工 X-14 下，`slash.md`「扩展的命令」）：命令名、说明、提示，同一个包里重了；声明了却没在 `[process] capabilities` 写 `commands`（`detail` 是 `slash`，报在第一段）；`detail` 是哪一格（`slash.<名字>.<格>`）、报在那一段 |
| `bad_widget` | `[[widget]]` 写错了（施工 X-15 上，`widgets.md`「清单」）：`detail` 是哪一格、报在那一段 |
| `bad_content` | `[content.<包>]` 写错了（施工 X-22，「给别的包带的内容」）：键不是包的编号、不是表、`dir` 不是包目录里的相对目录 |
| `bad_needs` | `[package] needs` 里有不认识的、重复的词（施工 X-25）：现在只认 `images` |
| `bad_targets` | `[package] targets` 里有不认识的、重复的系统（施工 G-1） |
| `bad_miyu` | `[package] miyu` 不是三段数字的版本（施工 X-31） |
| `needs_newer_miyu` | 要比正在跑的这一版新的 Miyu（施工 X-31）：`detail` 是清单写的版本。装包、读装了的包时（`miyu_config::package::read_for`）第一件事比它，比不过的整份不再往下读，不报新版本才有的格是不认识的键；正在跑的是 `0.0.0`（开发时编的，没发过版）的不比；打包（`miyu pkg pack`）、软件源收录照 `read` 读，不比，只查写法 |
| `bad_search` | `[[search]]` 写错了：编号、名字，重了，没声明 `search` 能力（施工 N-5） |
| `bad_side` | `[[side]]` 写错了（施工 Q-3）：编号不合写法、少了，档位不是 `lite`、`normal`、`flagship`，少了名字、说明，同一个包里重了；`detail` 是哪一格（`side.<编号>.<格>`） |
| `bad_author` | `[package] author` 不是一行 1 到 100 个字（施工 F-12） |
| `bad_upstream` | `[package] upstream` 不是 `https://` 开头、不带空白、至多 300 个字的网址（施工 F-12） |
| `depends_missing` | 装着的包依赖的包缺了（施工 F-11，`miyu check` 是错误）：`detail` 是缺的那几个，逗号分开 |
| `protocol_mismatch` | 读成了，说的协议版本不包含 1（列表里照样带全；`miyu check` 是警告） |
| `check_failed`、`check_unavailable`、`check_output` | 跑包的检查（施工 9-2，都是警告）：跑坏了、到时没完；程序没找到；印了看不懂的行 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/src/packages/tests.rs` 的 `an_upgrade_cut_off_between_the_renames_is_put_back`、`recovering_leaves_what_it_does_not_recognize`（施工 AE-7） | 两次改名之间被杀，收拾时放回原来那一份、暂存的删掉，再收拾一遍什么都不做；本体在的备份删掉；认不出的名字不碰 |
| `crates/miyu-config/src/package/tests.rs` | 两份样例（终端界面会话给的草稿、桥那种）每一格读对；最小的清单；每一种写错报对代码和行 |
| `crates/miyu-config/src/phrases.rs` 的测试 | 语言到一句话的读法、第一处错 |
| `crates/miyu-store/src/packages/tests.rs` | 两层照编号排、不是 `.toml` 的和编号不合写法的不算；空的；同编号认出厂的；子命令名先到先得、报在那一行；写错的照样列出；状态目录；功能的编号先到先得、照包算的那一个也算（施工 F-1） |
| `crates/miyu-config/src/package/features/tests.rs`（施工 F-1） | 功能照写的先后读对、编号在第几行；没写的照包算一个；空表一个都没有；界面、小程序没有；编号、表、名字、工具的每一种写错；只有内置包、扩展包能写 |
| `crates/miyu-config/src/package/links/tests.rs`（施工 F-1） | 小程序的程序和参数；依赖、推荐；平台接入；组只给内置包、写法不对和重复的报 `bad_group`、`required` 不认了（施工 BS-1 上中）；每种包只能写自己的表；种类四选一 |
| `crates/miyu-config/src/package/side/tests.rs`（施工 Q-3） | `[[side]]` 读成编号、档位、名字、说明，全编号是 `<包>/<编号>`，三档都认；编号写错、少了、重了，档位不认识、少了，少了名字、说明报 `bad_side`、`detail` 对；不认识的格、写成一张表的、不是扩展包的照各自的代码 |
| `crates/miyu-config/src/package/settings/tests.rs` | 每一种类型、默认值、几层、什么时候生效、隐藏；每一种写错；拼成配置项的键、类型、界面提示（施工 9-1 下）；列表的每一种元素、默认值一个个查、写错（施工 9-1 补） |
| `crates/miyu-endpoint/tests/package_settings.rs` | 包的配置项进 `config.schema`（「软件包」那一页、这个包那一组、名字说明照语言、隐藏的带标记）；最终值；写错的、写错层的 `check` 报；撞了核心的模块整份不收（施工 9-1 下）；列表进 Schema、读得到、写错 `check` 报（施工 9-1 补） |
| `crates/miyu-cli/src/packages/tests.rs`（施工 9-2） | 只有包的子命令转交（内置的、选项、`help`、不认识的不转）；没装的程序说哪份清单；帮助页多的那一节、接在「命令」后面、照语言；跑包的程序时环境多 `MIYU_PACKAGE_DIR`（施工 S-6 上）、`MIYU_PACKAGE_STATE`（施工 F-15） |
| `crates/miyu/tests/packages.rs` 的 `a_package_command_is_told_its_package_folder`（施工 S-6 上，只在 Unix 上） | 真二进制转交：包带的程序（指向 `printenv` 的链接）看到的 `MIYU_PACKAGE_DIR` 是 `packages/` 里这个包的文件夹；`MIYU_PACKAGE_STATE` 是建好了的状态目录的真实位置（施工 F-15） |
| `crates/miyu/tests/packages.rs`（施工 9-2） | 真二进制：参数原样交过去、退出码照它的、`help <名字>` 转成 `--help`、不拉起核心；帮助页列出包的子命令、撞了内置的不列、内置的照旧，只有出厂的包（都不带命令）不写这一节；没装的程序退出码 1；不认识的照旧退出码 2 |
| `crates/miyu-endpoint/src/check/run/tests.rs`、`tests/package_check.rs`（施工 9-2） | 一行输出收哪几格、哪些不收；0、1 以外的退出码、信号、到时、跑不起来；真的跑 `sh`、到时杀掉；真核心：包报的接在后面，看不懂的行、跑坏了的、程序没找到的各一条警告 |
| `crates/miyu-cli/src/head/tests.rs`、`crates/miyu/tests/heads.rs`（施工 9-3） | 照清单定怎么开（不带参数、带 `--page config`、不认这一页、没装、不是界面、有清单程序不在）、没装的列出装了的（只算程序在旁边的，9-3 补）；真二进制在伪终端里：`miyu`、`miyu config` 拉起清单里的界面、退出码照它的，不认设置页的印帮助，`ui.head` 指着没装的退出码 1；出厂的终端只有清单的说程序不在旁边（9-3 补） |
| `crates/miyu-core/tests/embed_package.rs`、`src/embed/tests.rs`（施工 R-5 三补） | 仓库里的内置语义模型的清单读得成小程序包；出厂的人格记忆推荐它、不依赖它，出厂的资源里没有它；照装了的包拼本机 embedding（`recall.md` 第四条第 1 款） |
| `crates/miyu-core/tests/tools.rs`、`tests/packages.rs`（施工 F-2） | 没装记忆、基础系统组的包的工具目录里没有它们的工具（卸掉 `miyu-base-shell` 只少它，`miyu-base-sessions` 带着 `send_message`）；读坏了的清单不算装了；出厂的内置包清单和编进来的一一对得上；没装画 mermaid、联网的查询是 `unknown_method` |
| `crates/miyu-endpoint/tests/packages_embed.rs`、`crates/miyu-core/src/embed/tests.rs`、`crates/miyu-core/tests/embed_log.rs`（施工 F-5 再补） | 真核心装上内置语义模型的包，`config.schema` 里「内置模型」当场写它的模型名，升级成另一个模型的换成新的，卸掉又没了；装卸时拼的和起来时一样、说得出缺的是哪一样，只有起来时记「没装」那一行 |
| `crates/miyu-endpoint/tests/packages_config.rs`、`crates/miyu/tests/packages_live.rs`、`crates/miyu-core/tests/tools.rs`、`crates/miyu-tool/src/catalog/tests.rs`（施工 F-5 补） | 装卸以后配置项当场换（见 `config.md`「守着它的」）；端口交的整份配置清单和起来时读配置用的一样、没装的人格记忆的几项不画；卸掉的提供者装回来登记了不再算卸掉 |
| `crates/miyu-endpoint/tests/packages_extensions.rs`（施工 F-5 下） | 真核心装上的扩展当场拉起、卸掉的当场停下、旧会话调到它的工具报「已卸载」、升级了的重起、没变的不动 |
| `crates/miyu-endpoint/tests/packages_live.rs`（施工 F-5 中） | 真核心卸掉一个内置包：查询当没有、用过它的会话工具面不变、调到报「已卸载」、新开的会话没有；装回来工具、查询都回来 |
| `crates/miyu-endpoint/tests/packages_preview.rs`（施工 F-8 下补） | 看一眼：装的说带了什么、要什么能力、几个文件多大，升级的说换下哪个版本，写错的、撞了出厂的照真装一样拒；卸的说哪一层、写了的设置键、有没有状态目录；装回出厂的；都什么都不动 |
| `crates/miyu-endpoint/tests/packages_push.rs`（施工 F-8 三补） | 订阅的回应是整份列表；别的连接装上、卸掉装上的、卸掉和装回出厂的，各推变了的那一项，照这个连接的语言，卸没了的是 `null`；取消了不推；没握手、带 `after` 的拒 |
| `crates/miyu-endpoint/tests/packages_purge.rs`（施工 F-8 中下补） | 真核心卸掉装上的、出厂的包（出厂的照测试自己放的出厂扩展 `courier`，施工 S-6 下）：配置里它的项删了、空了的表头也删了，别的项和注释不动；状态目录删了 |
| `crates/miyu-endpoint/tests/packages_install.rs`（施工 F-5 上） | 装一份清单、同名目录一起拷、列表和预设的功能当场有；升级换掉、升级撞了放回原来的、不留暂存；写错的、和出厂撞了的、和别的包撞了的不装；卸装上的删掉；卸出厂的记一笔、列表里标卸掉、装得回来；必需的、没装的不能卸 |
| `crates/miyu-store/src/packages/tests.rs` 的 `a_program_is_found_in_the_package_bin_then_beside_main`、`crates/miyu-cli/src/head/tests.rs` 的 `a_head_runs_the_program_its_package_brings`、`crates/miyu/tests/packages.rs` 的 `a_package_command_runs_the_program_in_its_bin`、`crates/miyu-endpoint/tests/packages_programs.rs`（施工 F-8 上再补） | 程序先找包文件夹的 `bin/`、再找主程序旁边的，相对的包文件夹不找 `bin/`；界面、转交的子命令照包里带的跑；真核心装上一个自己带程序的扩展：拉起、列表里在跑、预设里它的功能算装了、装上的文件里有它的程序；程序还被开着写的（Linux）等放开再拉起 |
| `crates/miyu-config/src/package/mcp/tests.rs`、`crates/miyu-policy/src/features/tests.rs` 的 `an_mcp_tool_belongs_to_its_server_and_the_package_keeps_its_own`、`crates/miyu-store/src/packages/tests.rs` 的 `an_mcp_server_name_is_taken_by_whoever_is_read_first`、`crates/miyu-endpoint/tests/packages.rs` 的 `a_bundle_lists_its_mcp_servers_as_features`、`packages_preview.rs` 的 `a_bundle_preview_names_its_mcp_servers`、`crates/miyu-cli/src/pkg/confirm/tests.rs`（施工 X-2） | `[[mcp]]` 每一格、没写的照什么；每一种写错报 `bad_mcp`、报在那一段；只带 MCP 服务的是 `bundle`、不说协议、能带吉祥物；界面、小程序、内置包写了 `wrong_kind`；一个服务一个功能、扩展包照旧先算自己的；MCP 的工具照前缀归服务、包自己的工具照旧；两个包同名的服务后到的 `feature_taken`；列表、看一眼、命令行那一份都列出服务 |
| `crates/miyu-store/src/packages/conflicts/tests.rs`、`crates/miyu-endpoint/tests/packages_conflicts.rs`、`crates/miyu-store/src/skills/tests.rs` 和 `scripts/tests.rs` 的 `a_shipped_package_wins_over_a_home_one_whatever_the_ids`、`crates/miyu-store/tests/knowledge_package.rs` 的 `the_package_given_first_wins_whatever_its_id`、`crates/miyu-endpoint/tests/knowledge_package.rs` 的 `a_shipped_package_base_wins_over_a_home_one_whatever_the_ids`、`crates/miyu-cli/src/pkg/tests.rs` 的 `a_conflict_says_cannot_install_and_one_line_each`（施工 F-8 六补） | 「冲突」：六样各找得到、各算各的，同一个包、读坏了的不比；真核心：撞了技能的装不上、看一眼也拒、什么都不动、装着的照旧好；编号排在前的新包挤不掉装着的（以前挤得掉）；脚本撞了出厂的工具、MCP 服务、知识库也算；升级自己、和家目录里自己放的同名照装；装回出厂的也查；读的时候技能、脚本、库出厂的先于家目录；命令行第一行说装不上、一处一行 |
| `crates/miyu-endpoint/tests/packages.rs` | `package.list` 的每一格、照语言挑；施工 F-1 的几格（功能、平台接入、依赖、小程序）；没编进来的内置包报 `not_built_in`、只认读成了的内置包算装了（施工 F-2）；写错的、同编号、撞名、协议版本对不上、`process` 和 `check`；起来时读一次；`check` 查清单、写了文件的认得出、别的文件认不出。只断言出厂的网页和测试自己放的几份，装上的编号、子命令名避开出厂会有的（施工 9-1 补：终端界面要出厂 `tui`） |

| `crates/miyu-endpoint/src/packages/depends/tests.rs`、`crates/miyu-endpoint/tests/package_depends.rs`（施工 F-11） | 缺了的装不上、`data.missing` 照写的先后；被依赖的卸不掉、`data.by`；装着的缺了依赖的列表写 `depends_missing`、`miyu check` 报；以前的 `workers` 照认；推荐的缺了不算；读坏了的不算装着；依赖缺了的扩展不拉起；列表带 `export` |
| `crates/miyu-store/src/personas/tests.rs`、`presets/tests.rs` 的包那一层、`crates/miyu-endpoint/tests/package_personas.rs`（施工 P-11 上） | 包带的人格、预设在出厂和你的中间；出厂的、排在前面的包盖住别的包；真核心：装上列得出、开得了会话，改了写你那一层、`remove` 是 `restore`，卸了包里的没了、你那一层的改动留着；和出厂同名的不认、`miyu check` 报 `preset_covered` |
| `crates/miyu-store/src/packages/conflicts/tests.rs` 的 `personas_and_presets_conflict_with_packages_and_layers`、`leftovers/tests.rs`、`crates/miyu-endpoint/tests/package_personas.rs` 后两条、`crates/miyu-cli/src/pkg/tests.rs`（施工 P-11 下） | 人格、预设和别的包、几层撞了说清是谁；留下的改动记、划掉、空的不留文件；真核心：看一眼列出来，撞了别的包、你那一层的不装，卸了 `miyu check` 报 `persona_left`，重装不算撞、提示没了，升级不和自己的改动比；命令行那一层写成人话 |

### 起草时定的

- 清单的读法放在 `miyu-config`：和配置一样是带行号的 TOML，下一步 `[settings]` 也从这里接；「语言到一句话」那一格挪到 `miyu-config` 和人格共用（2026-10-07 主会话）。
- 写错的照样列出、只有编号和问题：头看得到装了一个写错的包（2026-10-07 主会话，照 `persona.list`）。
- 协议版本对不上的照样带全、多 `code`：不一样的头也许还能用一部分，核心不替它拿主意（2026-10-07 主会话）。
- 桥算 `process`，`[process]` 的 `args`、`start` 照通讯平台的会话要的定（`18-通讯平台.md` Q17，2026-10-07）。
- 包的配置项读的时候存自己的一份，只在核心起来时拼成配置项那一次把字留在进程里：配置清单的项是编译期常量的样子，`miyu check` 每次读盘，不能每次都留（2026-10-07 主会话，施工 9-1 下）。
- 设置页一页「软件包」、一个包一组：主菜单不会随装的包越来越长（2026-10-07 和终端界面的会话对过）。
- 列表元素的那一格叫 `element`，和 `config.schema` 里列表那一格一个词；元素带的几格不另开表，写在同一张表里（2026-10-07 主会话，施工 9-1 补）。
- 施工 F-1（2026-10-09 主会话）：每种包能写哪几张表列成一张表（`PackageKind::tables`），写错了统一报 `wrong_kind`，比一张张写判断好查；内置包、小程序先收得紧，要用了再放开。没写 `[features]` 的整个包算一个功能，写了空表的一个都没有：别人的扩展不写也有一个开关，只接平台、不带工具的写空表就不列。`package.list` 的功能不列工具：头画预设页用 `preset.get`（F-3），这里只给人看装了什么。依赖分两种照 Debian 的 Depends、Recommends。功能编号、平台名、依赖的包编号都照包编号的写法（`miyu_config::secret::valid_name`）。
- 施工 F-4（2026-10-09 主会话）：没装的内置包，核心替它声明的配置项照样登记、只是不画：从配置清单里拿掉会让写过它们的配置文件多出「不认识的键」的警告，包装回来又得重新写。别人做的包卸了以后它的键照旧报不认识：核心不知道它曾经有过。
- 施工 X-2（2026-10-11 主会话）：只带 MCP 服务的包叫 `bundle`（一组东西，以后技能、脚本也算进来），不新造「功能包」这类给人看的叫法：种类只给程序用，界面上不印。`[[mcp]]` 只给扩展包和 `bundle`：界面、小程序没有功能，内置包随 Miyu 发、要带服务写进代码。服务各算一个功能、照工具名前缀归，MCP 的工具名到 X-3 才有，归法先定在这里，X-3 不用再动预设。
- 施工 F-8 上再补（2026-10-11 主会话）：包带的程序放包文件夹的 `bin/`，先找它、再找 `miyu` 旁边的：设计 31 第二节的图里就是这样放的，原来只找 `miyu` 旁边的，别人做的扩展用 `miyu pkg install` 装上也起不来。两层一个找法：出厂的包文件夹里一般没有 `bin/`（程序归系统的包管理，装在 `miyu` 旁边），照旧找到旁边的。没有改成「装上的只找 `bin/`」：在 `miyu` 旁边放程序、`packages/` 里放清单的开发办法照旧能用。
- 施工 F-2（2026-10-09 主会话）：编进来的内置包那张表放在 `miyu-core`：只有它知道 cargo 开关开了哪几个。`miyu check` 不查 `not_built_in`：查清单时不知道是哪一份核心在跑，起来时的运行日志和 `package.list` 已经说了。基础系统组的包都没装也照样起来：工具全没有也能聊天，比起不来好查。联网的清单先不写功能：网络搜索、抓取网页两件工具还没有，写了功能预设里就多两个空开关。

### 还没有的

- 能力的审批（清单多 `capabilities`）、握手时把包自己的配置交给扩展：9-4（下），见 `extensions.md`「还没有的」。
- 写了文件的 `check` 交给包自己的检查：包怎么认自己的文件、怎么交给它，和通讯平台的会话对好再做。
- 锁文件（`07-存储.md` 第二节）；从软件源装（12 第四节）；命令行 `miyu package …`；装卸以后工具、查询、扩展、配置项当场换（F-5 中、下）。
