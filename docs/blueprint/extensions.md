## 扩展进程

### 是什么

核心拉起带 `[process]` 的软件包（施工 F-8 上补起清单不写种类）（第一个是通讯平台的桥），经标准输入输出说同一套协议（`05-内核接口.md` 第四节、I2）。开关存在核心里：开着的常驻，核心重启时照开关拉起；崩了退避重启，连续失败停下、说明原因；随核心退出（第九节）。

状态：图纸，施工 9-4（上）（2026-10-08 主会话；方向是施工方案第三节 9-4 那一行和 `05-内核接口.md` 第四、九节；桥那一头的形状和通讯平台的会话对过）。能力的声明和审批施工 9-4（下上）做了（2026-10-08，项目主人照推荐定：在开的那一下批）；把包自己的配置交给它随 9-4（下下）。状态的推送随 9-4（补）（2026-10-08 主会话，两个头的设置页要的）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/extensions.rs` | 开关：`system/extensions.json`，照只给自己看的 JSON 小文件的规矩读写 |
| `crates/miyu-endpoint/src/extensions.rs` | 一张表：每个 `process` 包一个看管的任务；开、关、重启、列状态；核心空闲、退出时问它 |
| `crates/miyu-endpoint/src/extensions/supervise.rs` | 看管一个包：拉起、交给连接、等它退出、退避、停下 |
| `crates/miyu-endpoint/src/extensions/stderr.rs` | 标准错误的文件：拉起前太大的挪成 `.old`，状态里给最后几行 |
| `crates/miyu-endpoint/src/appends.rs` | `system.append`：照包、照会话记下扩展放的，交给端口；`miyu-session` 的 `appended.rs` 照工具面挑、钉进快照 |
| `crates/miyu-endpoint/src/hooks.rs` | 回合开始的挂接点（施工 X-16）：会话的 `HooksPort` 照清单、提供者挑要问的扩展，并行发 `user.append`、各自限时，照写法收；`miyu-session` 的 `actor/injects.rs` 拼问的话、换成注入 |
| `crates/miyu-endpoint/src/extensions/methods.rs` | `extension.enable`、`extension.disable`、`extension.restart`、`extension.status`；推送用的一项（`entry`，施工 9-4 补） |
| `crates/miyu-endpoint/src/subscriptions/listed.rs` | 扩展的状态的订阅（施工 9-4 补；F-8 三补起和软件包列表的订阅共用这一份）：先写回应，收到哪个包变了照这一刻算那一项推 `extension.changed`，掉了队推 `resync` |
| `crates/miyu-endpoint/src/connection.rs`、`hello.rs`、`login.rs` | 核心亲手给的连接：握手不看凭据（`Via::Spawned`） |
| `crates/miyu-endpoint/src/connection/rights.rs` | 扩展能调什么的表（施工 X-30，「谁能调」）：只给人、要哪项能力、都能调；连接起来时照清单算一次声明了哪些；门禁 `xtask/src/rights.rs` 查代码里分发的方法都在表里 |
| `crates/miyu-core/src/lib.rs`、`serve.rs` | 写了 `ready` 以后照开关拉起，停的时候请扩展退出 |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展：照参数一步步做（握手、调方法、带参数调方法（参数里的 `{session}` 换成最近回应里的会话编号，施工 O-4 下）、等标准输入读到头、不理它、退出码、往标准错误写、记下一个环境变量（施工 O-18）），不随发行带出去；`inject:<怎么答>` 定 `serve` 怎么答 `user.append`（施工 X-16、X-16 补） |

### 对外的样子

**开关的文件** `system/extensions.json`：

```json
{"version":1,"on":{"echo":true}}
```

没写的照清单的 `start`：`always` 的开着，`manual` 的关着。写了的照写的。读不成的当没有，记一行 `WARN`，照样起来；写的时候盖掉。

成员能不能用（施工 IM-1 上，`package.members`）记在同一份里的 `members`：`{"version":1,"on":{},"members":{"im":true}}`。没写的照清单的 `[package] members`；一个都没改过的不写这一格。

批过的能力（施工 Y-5 下上，设计 06 U16「能力即组」）记在组表 `system/groups.json` 里：批了就是把包的账号加进它声明的每一个能力的组 `ext.<能力>`（同一张表上人的组是 `admin` 这类不带点的，施工 Y-12 中下，设计 06 U16「落地」）：

```json
{"version":1,"groups":{"admin":["bob"],"ext.events.read":["echo"],"ext.network":["echo"]}}
```

以前开关文件里记批过的那一格 `approved`（施工 9-4 下上）读的时候认得、直接扔掉，写回时不写：整份读不进的话开关会一起丢。换核心以后要重新批一次，开关不用重开（主会话 2026-10-23 定）。

**协议**（只有本机连接能调，现在都是管理员）：

| 方法 | 参数 | 回应 |
|---|---|---|
| `extension.status` | 无 | `{"extensions": [<一个>…]}`，照编号排，只列读成了的 `process` 包。每人一份的包照看的人那一份报，带 `scope: "account"`；管理员另带 `copies`，拉起过的每一份一项（施工 IM-1 中下四） |
| `extension.enable` | `{"package", "approve"?}` | `<一个>`：要的能力还有没批的，`approve`（能力名的列表）要盖住它们，批了记下（见「能力」）；开着、记下来、没在跑的拉起 |
| `extension.disable` | `{"package"}` | `<一个>`：关了、记下来、在跑的请它退出 |
| `extension.restart` | `{"package"}` | `<一个>`：请它退出、重新拉起，连续失败从零数；关着的拒绝 `extension_off`，还有没批的能力拒绝 `needs_approval` |
| `process.retire` | `{}` | 扩展自己调（施工 IM-1 下）：`{"ok": true}` 是核心答应了、可以退了，`{"ok": false}` 是还有核心发给它、没回的请求，过一会儿再问（「怎么走」第 9 条） |
| `port.claim`、`port.release` | `{"port"}` | 扩展自己调（施工 IM-5）：登记、放开要听的端口，回 `{}`；别的扩展登记着的 `port_taken`（「怎么走」第 10 条） |
| `feature.offer` | `{"features"}` | 扩展自己调（施工 IM-6）：替这个人交预设菜单里的功能，回 `{}`（「怎么走」第 11 条） |
| `port.list` | 无 | 只给管理员（施工 IM-5）：`{"ports": [{"port", "package", "account"?}]}` |
| `process.keep` | `{"on"}` | 扩展自己调（施工 IM-1 下）：每人一份的那一份要不要常驻，回 `{"on": <现在的值>}`；一份的本来就常驻，回 `{"on": true}`、不记 |

一个：

| 格 | 有没有 | 是什么 |
|---|---|---|
| `package`、`name` | 都有 | 编号；名字照连接的语言挑（同 `package.list`） |
| `start` | 都有 | `manual`、`always` |
| `on` | 都有 | 开关 |
| `state` | 都有 | `off`（关着）、`starting`（拉起了、还没握手）、`running`（握了手）、`waiting`（退避中，等着下一次）、`stopped`（不再拉起，看 `reason`） |
| `pid` | `running`，起来了的 `starting` | 进程号 |
| `failures` | 都有 | 连续失败了几次 |
| `retry_in` | `waiting` | 还有几毫秒拉起 |
| `reason` | `stopped` | `needs_approval`（开着，要的能力还有没批的，施工 9-4 下上）、`config_error`（退出码 1）、`failed_repeatedly`（连续失败 5 次）、`not_installed`（程序不在包文件夹的 `bin/`、也不在 `miyu` 旁边）、`cannot_start`（起不来：系统不让起、建不了它的目录和标准错误的文件）、`protocol_mismatch`（清单说的协议版本不包含核心的，不拉起）、`retired`（问过 `process.retire`、核心答应了、退出码 0：自己退的，不算失败，施工 IM-1 下） |
| `stderr` | `stopped` | 标准错误的最后 20 行，最多 4 KiB |
| `capabilities` | 声明了能力的 | 声明的每一个 `{"id", "name", "summary"}`，照能力表的先后；名字、一句说明照连接的语言挑（内核那份 `human` 的 `extensions/capabilities/<名字>`、`…/summary`），没有字的名字是编号、说明是 `null`（施工 9-4 下上） |
| `unapproved` | 还有没批的 | 没批的能力名，照能力表的先后（施工 9-4 下上）。出厂的包没有 |

拒绝：编号不是读成了的包 `unknown_package`；不是 `process` 包 `not_an_extension`；参数不对 `bad_params`（`approve` 里有不是这个包声明的名字的也是）；要的能力还有没批的 `needs_approval`，`data.capabilities` 是没盖住的那几个（施工 9-4 下上）。

**能力**（施工 9-4 下上，`05-内核接口.md` 第三节、I5；项目主人 2026-10-08 照推荐定）：

1. 清单 `[process] capabilities` 声明要哪些（`packages.md`；声明斜杠命令 `[[slash]]` 的要 `commands`，施工 X-14 下，`slash.md`），只认那张表里的十七个名字（`crates/miyu-config/src/package/capability.rs` 的 `ALL`；施工 X-15 上加了 `widgets`，`widgets.md`；X-20 加了 `context.system`，X-18 加了 `packages.call`，N-5 加了 `search`，X-30 加了 `memory.write`）：`tools`、`commands`、`widgets`、`context.inject`、`context.system`、`tool.guard`、`tool.rewrite`、`events.read`、`events.write`、`sessions.drive`、`packages.call`、`search`、`act_for_external`、`network`、`fs.read`、`fs.write`、`memory.write`。
2. 管理员装的（系统那一层 `system/packages/`，施工 U23 上上）要批：在 `extension.enable` 那一下带上 `approve`。批的时候把包的账号加进它这时声明的每一个能力的组；还没批的是「声明的减组里有的」，升级以后多了的只问多的；关掉不清批准，卸掉移出它在的 `ext.*` 组（施工 Y-5 下上）。
3. 出厂的（资源目录里随 Miyu 一起装的，通讯平台的桥）算批过的：和编译进核心的模块一样本来就被信任（05 第三节「内置模块」）。
4. 开着、还有没批的（开关文件里开着的，`start = "always"` 没写开关的）不拉起：`stopped`、`needs_approval`，不拦核心空闲退出；批了就拉起。
5. 照包手里有的能力挡方法（施工 X-30，下面「谁能调」；Y-5 下上起照组表算：`ext.<能力>` 组里有包的账号，出厂的算全有），连接起来时算一次，批、升级都会重新拉起。包替人干活的（设计 06 U17，`Caller::serving`），那个人自己也得有这项权限、取交集：这一版人这一侧照「是不是管理员」（替的只有机主，恒是），成员也能用扩展时照名单换，判法不变。`context.inject` 是挂接点不是方法，照旧在 `hooks.rs` 查。沙盒里跑另做（「还没有的」）。

**谁能调**（施工 X-30，`05-内核接口.md` 第三节、I5，`04-核心协议.md` 第四节；2026-10-20 项目主人定从多用户那一段提前做）：核心拉起的扩展的每一条请求，先照 `connection/rights.rs` 的表判，参数都不看就拒；头的连接不经这里。

| 种 | 回 | 方法 |
|---|---|---|
| 只给人 | `local_only` | `session.answer`、`session.set_permission_level`、`session.set_workspace`、`session.terminal`、`session.set_terminal`；`account.*`、`login.ticket`；`preset.set`、`preset.delete`、`persona.set`、`persona.delete`；`package.install`、`remove`、`enable`、`disable`、`restart`、`sync`、`search`、`clean`、`upgrades`、`upgrade`、`file`；`extension.*`、`mcp.*`、`project.*`；`skill.remove`、`tool.set_loading`、`feature.set`；`config.trust`、`secret.set`、`secret.delete`；`subscribe` 的 `projects` 流；核心发出去的（`tool.call`、`view.*` 这些），扩展调了没意义 |
| `session.delete`（施工 Y-2 上） | 要 `sessions.drive` | 只删得掉自己名下的（属主照「会话只给属主」那一道查）：平台的桥删它的场所会话，它的后台照这条给管理员删（2026-10-23 项目主人定） |
| 要 `sessions.drive` | `needs_approval` | `session.create`、`send`（不带 `as`）、`interrupt`、`revert`、`unrevert`、`redo`、`compact`、`clear`、`configure`、`fork`、`set_meta`、`respond`，`command.run`（不带 `as`）、`job.stop`，包的会话状态 `session.set_state`、`session.state`（施工 TW-1） |
| 要 `events.read` | 同上 | `subscribe` 的 `events`、`view`、`sessions` 流，`session.list`、`session.recap`、`session.tools`、`view.page`、`view.detail`、`job.output` |
| 要 `events.write` | 同上 | `events.append`（`ext.<包>.*`） |
| 要 `act_for_external` | 同上 | 带 `as` 的 `session.send`、`command.run`，`session.note`，`venue.*`，`events.append` 的 `venue.*` |
| 要 `memory.write` | 同上 | `memory.remember`、`update`、`forget`、`restore`、`dream`、`dream_stop`、`card_update`、`card_forget`（施工 E-1 下），`kb.add`、`create`、`update`、`mount`、`remove`、`delete`、`reindex` |
| 要 `fs.read`、`network` | 同上 | `fs.*`；`link.preview` |
| 要 `tools`、`widgets`、`context.system`、`packages.call` | 同上 | 有工具的 `provide`（空的照收：斜杠命令、搜索要先 `provide` 一次）；`widget.set`、`widget.clear`；`system.append`；`package.call`（照旧另查对方的 `[export]`） |
| 都能调 | 照常 | 别的：读配置和密钥的名字、`blob.*`、`model.*`、`human.get`、读软件包的、`content.*`、人格和预设读的、技能、脚本、记忆和知识库读的、`subscribe` 的别的流、`login.redeem` |

1. 缺能力的 `data.capabilities` 是缺的那一项，同开扩展时没批的那一种回法；`system.append`、`package.call` 原来各自查声明的那几行去掉了，统一在这里查。
2. 表里没有的方法当只给人：新方法默认不对扩展开。门禁的「文档」一项查代码里分发的每个方法都在表里、表里写的代码里都有（`xtask/src/rights.rs`，照协议参考找方法的那一套，不算核心发出去的推送）。
3. 扩展的身份是它自己：不带 `as` 的 `session.send` 记成 `harness`、名字是包的编号，它自己写 `from` 回 `bad_params`（`protocol.md` 的 `session.send`）。
4. 带 `as` 的只看 `act_for_external`，不另要系统账号：系统账号管的是会话归谁，不是能不能代表人（主会话照推荐定）。`venue.session` 这些另照旧只给系统账号的连接（`venues.md`）。
5. 记忆、知识库的写一项能力管两样（2026-10-20 项目主人定；原来推荐只给人）。

**推送**（施工 9-4 补，两个头的设置页要的，照会话列表的推送那一路，`protocol.md`「会话列表的推送」）：

1. `subscribe {"stream": "extensions"}`：回应 `{"extensions": [<一个>…]}`，和 `extension.status` 一样。不带 `session`、`after`，带了 `bad_params`。一个连接至多一个，再订阅换一个新的；`unsubscribe` 停掉，没订阅过的也回 `{}`。
2. 之后一个包的 `state`、`on`、`failures`、`reason` 变了推 `extension.changed`：`{"entry": <一个>}`，头照 `package` 整项替换。推的那一项照这一刻的状态、这个连接这一刻的语言算，晚到的、重复的都对，不编号。
3. 读得太慢、掉了队：推 `resync`（`{"stream": "extensions"}`），这个订阅停了，头重新订阅。

**配置**（施工 9-4 下下；项目主人 2026-10-08 照推荐定，形状和通讯平台的会话对过）：核心拉起的扩展不自己读系统配置、密钥文件，要的值由核心交给它。只在核心亲手拉起、走标准输入输出的连接上，只给这个包自己的键：`config.md` 第九条「密钥不进协议的回应」的例外。

1. **哪些键**：「包的编号加点」开头的配置项（核心替它声明的、它清单里 `[settings]` 声明的都算；带 `<…>` 的模板键不算）。
2. **握手**：`hello` 的回应多 `config`：`{"onebot.listen": 8301, "onebot.token": "<真值>", …}`。值是最终值（没写的照默认值，系统配置、个人设置合出来的，不看项目配置），密钥引用（`{ secret }`、`{ env }`）解成真值的字；没设、没默认值、引用取不到的那一键不放。头的连接没有这一格。
3. **变了推** `extension.config`：`{"keys": {"onebot.token": "<新真值>"}}`。握手的回应写出去以后，核心盯着配置服务交给核心的那一份（配置文件改了、`secret.set` 换了值、手改了密钥文件，都会换上新的一份），把这个包自己的那份重算一遍，跟上一次交给它的比，变了推，只放变了的键；变成没设、取不到的写 `null`。不用订阅。不叫 `config.changed`：那是头订阅的推送，形状带 `origin`、`applies`（通讯平台的会话提的，主会话定）。
4. **不留痕**：值不进运行日志、不进事件；运行日志只记 `DEBUG extension config handed package=… keys=<几个>`、`extension config pushed`，不带值。

**回合开始的挂接点**（施工 X-16，设计 `34-表情包.md` 第五节第 1 条）：

1. **谁被问**：清单 `[process] capabilities` 写了 `context.inject`、批过的（出厂的算批过）、在跑、登记过工具的扩展（`provide` 过的连接），而且这个会话这一刻的工具面上有它登记的工具（预设关了它就没有，就不问）。照包的编号排。
2. **问**：回合开始、记忆和知识库的挂接点之后，核心向每一个发 `user.append {"session", "venue", "text"?}`（施工 X-16 补以前叫 `turn.start`）：`venue` 是 `local`（本机）、`private`（平台私聊）、`group`（群）；`text` 是触发这一轮的人话，不是人开的一轮（回报叫醒的、续轮）不带。几个并行问，各自限时 300 毫秒（扩展的 `Timing::answer`，测试里放宽），过了的当这一轮没交，记一行 `DEBUG`。
3. **答**：`{"content": [{"kind", "text"}]}`（X-16 补以前这一格叫 `inject`），至多 3 块：`kind` 照事实类别的写法（小写字母、数字、`-`、`_`），`text` 1 到 4000 个字符，原样进上下文（外壳、转义扩展自己做）。空的、写法不对的整份当没交，记一行 `WARN`；JSON-RPC 的错误也当没交。
4. **记**：交回的一块记成 `context.injected`，`by` 是这个包（模块编号就是包的编号），`kind` 照交的，`after` 是真：排在触发的那句话**后面**（施工 X-16 补，项目主人定「插到用户消息尾部」；记忆、知识库的照旧排前面）。写进日志就不变：下一轮照日志原样回放，前缀稳定。
5. **不问的**：没有这种扩展的会话一个字节不变；子会话照问（它的工具面上有那件工具才问）。

**放在 system 尾部的**（施工 X-20，设计 `34-表情包.md` 第五节第 5 条）：

1. **谁能放**：清单 `[process] capabilities` 写了 `context.system`、在跑的扩展（`appends.rs`），经 `system.append {content, session?}`。
2. **管哪些会话**：不写 `session` 的管全部，写了的只管那一个、优先；`content` 写 `null` 去掉那一层（单给会话的去掉了退回全部的那一份）。只记在核心的内存里。
3. **怎么进 system**：工具面上有这个包的工具的会话才带；原文照放、不套包的编号，每个包一段、照包的编号排，接在整段 system 的最后（`26-提示词.md` 第四节第 8 块）。造会话时照这一刻的钉进快照（`appended`）；回合开始经端口（`HooksPort::appended`）问一声，和钉的不一样就换一份快照，整段重算一次。核心重启以后照钉的，扩展再设一样的不再换；包卸了、预设关了它的下一轮去掉。

### 怎么走

1. **拉起**（施工 F-6 上：程序不在的当没装，`package-pages.md`「程序不在就当没装」：`extension.enable` 拒绝 `program_missing`、什么都不写；开着的在核心起来时不照登记缓存登记工具、记成停下 `not_installed`）：程序照 9-2 的找法（`miyu_store::packages::locate`：先找包文件夹的 `bin/`，再找 `miyu` 真实位置旁边的，不走 `PATH`，施工 F-8 上再补；Windows 上都没有 `.exe` 的，照 `bin/<名字>` 第一行的 `#!` 找解释器起，施工 X-34，`packages.md`「转交」），参数是清单的 `[process] args`，环境照核心的，另把 `MIYU_HOME`、`MIYU_RESOURCES` 设成核心手上正在用的数据根、资源目录的路径（施工 O-18：扩展和核心认同一份，不管核心是怎么找到它们的；测试里的核心跑在测试程序里，环境里没有临时的数据根，靠它把扩展指过去），`MIYU_PACKAGE_DIR` 设成这个包的文件夹（施工 S-6 上：清单所在的那一层，出厂的在资源目录、自己装的在家目录；扩展照它读自己包里的文件，不自己拼家目录的布局），工作目录是这个包放状态的目录（`state/packages/<编号>/`，没有的建）。标准输入、输出接管道，交给协议的连接；标准错误直接接到 `state/logs/<编号>.stderr` 上（起不来的，比如 Windows 上解释器没装，起之前往这个文件里写一行原话，施工 X-34：停下的带最后几行照它说）（追加，拉起前超过 1 MiB 的先挪成 `<编号>.stderr.old`）。核心丢下它时杀掉（`kill_on_drop`），兜底。Windows 上不开新的控制台窗口。拉起时系统说程序正被开着写（Linux 的 `ETXTBSY`：装包刚拷进来的程序，拷的那一刻核心起的别的子进程在换成自己的程序以前也攥着写它的句柄），隔 50 毫秒再试，最多 20 次，还不成才算起不来（施工 F-8 上再补）。
2. **握手**：照头的样子（`hello`），不出示凭据：标准输入输出是核心亲手给的（`Via::Spawned`，写了的凭据不看）。握手的期限照头的。包声明了系统账号的（`packages.md`「系统账号」），这个连接就是它：握手回应的 `account` 写它，通讯平台的场所会话归它（`venues.md`）。没声明的也是包自己的账号（施工 Y-5 中上，设计 06 U16）：握手回应的 `account` 写包的编号，替机主干活（`Caller::acting_for`），它开的会话、读写的人格、预设、软件包都照机主的。上传的 blob 存在这个连接的账号名下（施工 Y-2 中下），经 `package.call` 转给别的包的拷进那个包名下（施工 Y-5 中上）。能力批过的才拉起（「能力」）；照能力挡方法随多用户、沙盒那一段。
3. **开关**：核心写了 `ready` 以后读一次开关的文件，照开关拉起开着的（`Core::start_extensions`）。`enable`、`disable` 先写文件再动进程，写不进的拒绝、不动进程（`internal_error`）；开、关、重启一件件办，两个连接同时开同一个不拉起两个。核心空闲的判断多一条：有要拉起、在跑、在等着再拉起的扩展，不算空闲；停下了的不拦着。
4. **退出**：核心停的时候、`disable`、`restart` 时关它的标准输入（读到头就是「请退出」，三个平台一样），等 5 秒，没退的杀掉。核心停的时候各个扩展一起等，不一个个排队。
5. **崩了**：它自己退出（退出码 1 以外的，包括 0；问过 `process.retire`、核心答应了的 0 除外，见第 9 条）、被信号杀掉、标准输出关了、握手不成，都算一次失败：退避 1、2、4、8、16 秒以后再拉起（最多 60 秒），连续 5 次停下（`failed_repeatedly`）。退出码 1 是「配置错、端口被占」这类重启也没用的：不退避，直接停下（`config_error`）。核心等满了杀掉的不看退出码，照一次失败退避（施工 9-4 修：Windows 上杀掉的进程退出码是 1，原来被当成配置错，第一次就停）。握了手、连着跑满 60 秒的，下一次失败从 1 数。进程活着、连接连着就算活，不另做健康检查。停下的照旧开着：`restart` 再试，核心下次起来也再试。
6. **日志**：拉起、握了手、退出（退出码或信号）、退避、停下，各记一行运行日志，带包的编号。
7. **装卸**（施工 F-5 下，`packages.md`「装卸」）：装、装回来做成了以后，照装卸以前、以后两份清单对一遍（`Core::follow_packages`）；卸是先照「去掉这个包以后的清单」对一遍再删文件，删不成的照原来的清单换回来（施工 F-5 补：Windows 上开着的文件删不掉）。卸掉的照「退出」那一条停下，它经提供者登记的工具拿掉、记下随包卸掉了：用过它的会话照旧留着、调到时报「已卸载」；装回来、又登记了的不再算卸掉（施工 F-5 补）。新装上的照开关拉起，和核心起来时一条路（没批的能力照旧记成停下）。清单变了的算升级：先停下再照开关拉起。清单没变的不动。声明了系统账号的照这时的清单先建好账号（`packages.md`「系统账号」）。
8. **每人一份**（施工 IM-1 中上，接入通讯平台设计 06 第 4.6 节第 1 样）：清单 `[process] scope = "account"` 的包，一份进程是（包，账号）。
   - 以那个人的身份连进来：`Caller` 的账号和替谁干活都是他；能力照包批过的，再和他取交集（`extensions::held`：不在管理员组的，包没勾「成员也能用」就一样都没有）。
   - 工作目录、`MIYU_PACKAGE_STATE` 是他家里的 `home/<账号>/state/packages/<编号>/`，管理员看不到；标准错误接到 `state/logs/<编号>.<账号>.stderr`。
   - 握手交的配置是他自己那一份（系统配置、他的个人设置合出来的），之后照他那一份推变化。
   - 核心起来、打开它时不起：工具照登记缓存先登记，她替谁调到工具时才替那个人起（`Core::start_for`：开关开着、依赖都在、程序在、能力都批了，他在管理员组或者包勾了「成员也能用」），等它连上、登记了工具再转（`provide/reach.rs`，最多等 60 秒）。头一回装上、还没有登记缓存的，先替机主起一份把工具登记上，不然谁都调不到。
   - 关掉、卸掉、重启：每一份都停下；重启以后每人一份的照旧用到时再起。
9. **自己退、常驻**（施工 IM-1 下，接入通讯平台设计 06 第 4.6 节第 1 样，`extensions/retire.rs`）：
   - `process.retire {}`：核心发给这个连接、还没回的请求一条都没有的（`reverse::Peer::retire`，查和记在同一把锁里），答 `{"ok": true}`，以后不再往它发请求、通知，找连接的都跳过它（当它断了）；有的答 `{"ok": false}`。
   - 答应了、退出码 0 的记成停下（`retired`），连续失败记 0，不重启。没问过就退的（包括 0）、答应了又以别的码退的照「崩了」算。「答应了」只算这一次拉起的进程，再拉起时清掉。
   - 答应以后这一份新来的调用（她调工具、`package.call`、挂接点、搜索、斜杠命令、到点推给它）照「用到时拉起」等着：等到它记成 `retired`，替那个人再起一份（`provide/reach.rs`），交给新的那一份，人看不到失败。看管的任务记完 `retired` 就交回，拉起时记成停下的当没在跑，不用等它交回完。
   - 一份的包也能 `retire`：照样停在 `retired`，不会因为用到再起，`extension.restart`、开关、核心再起来时才起。
   - `process.keep {"on"}`：照（包，账号）记在 `system/kept.json`（`miyu_store::kept`，`{"version":1,"kept":{"im":["alice"]}}`，一个都没有的包不写），核心重启也在。放 `system/`、不放 `state/`：删了的话没起的那一份没机会再说。进程退出不清，只由扩展自己改。核心起来、装上、升级这个包时，每人一份的包替名单里的人拉起（`Core::start_kept`），照 `start_for` 那几道查。
   - `start_for` 多一道（施工 IM-1 下）：这个人还在（管理员，或者登记过、没停用的成员，`login::members::active`）。常驻名单里留着删掉、停用的人，不替他们起。
   - 不是核心拉起的扩展的连接调这两个回 `not_an_extension`；谁都能调（`Anyone`），不另要能力。
10. **端口登记**（施工 IM-5，接入通讯平台设计 06 第 4.6 节第 5 样，`extensions/ports.rs`）：只是一张表，不替扩展听，真正抢端口的是操作系统。表在扩展表里（`Extensions.ports`），只在内存里，核心重启就空了。
   - `port.claim {"port"}`：记成（这一份、这个连接）。别的还连着的连接登记着的回 `port_taken`，不说是谁；同一个连接再登记照收；`0` 回 `bad_params`（`field` 是 `port`）。
   - `port.release {"port"}`：是这个连接登记的拿掉，不是的不动，都回 `{}`。
   - 连接断了算放开：查的时候登记它的连接 `Peer::closed()` 的当没有，列表时顺手拿掉，不另做收尾。`closed()` 施工 IM-5 起写的那一头没了也算：核心停扩展时整个连接被丢下，`reverse.close` 没机会跑。答应了 `process.retire` 的同样算断了。
   - `port.list`：只给人（权限表），不是管理员的回 `not_permitted`；照端口排，每人一份的带 `account`。
11. **按账号交预设功能**（施工 IM-6，接入通讯平台设计 06 第 4.6 节第 4 样，`extensions/offers.rs`）：`feature.offer {"features": [{"id", "name", "summary"?, "tools"}]}`。设计里叫 `feature.set`，和给人改全局开关的 `feature.set`（施工 X-27）撞名，改叫 `offer`。
   - 照（包，替谁干活的那个人）记在扩展表里（`Extensions.offers`），只在内存里，整份替换，空的是全撤；进程退了（包括 retire）不清，免得她在终端里用不到这些工具、又没东西把扩展拉起来。
   - 查：`id` 写法同包的编号，不能和这个人装了的功能（`scripts::features`，去掉这个包上一次交的）撞、一份里不能重；工具必须是这个包登记过的（照这个人的工具目录）。不对的 `bad_params`（`field` 是 `features`），什么都不换。
   - 怎么生效见 `presets.md`「扩展替这个人交的功能」；`preset.get` 照交了的列（名字、说明照连接的语言挑）。
   - 只有核心拉起的扩展能调（`Anyone`，别的连接 `not_an_extension`），不另要能力。

### 出错

| 代码 | 什么时候 |
|---|---|
| `unknown_package` | 没有这个编号的包，或者它的清单读不成 |
| `not_an_extension` | 是 `ui` 包 |
| `extension_off` | `restart` 一个关着的 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/src/extensions/tests.rs` | 没有文件的是空的、写进去读得回来一字不差；坏了的、版本不认识的说是坏了；这一瞬间有人改了的不写 |
| `crates/miyu-endpoint/src/extensions/supervise/tests.rs` | 退避 1、2、4、8 秒、最多 60 秒、不溢出；连续 5 次停下；退出码 1 直接停下，0、别的、被杀掉的退避；跑满了的这一次从 1 数；核心杀掉的不看退出码（施工 9-4 修） |
| `crates/miyu-endpoint/src/extensions/stderr/tests.rs` | 追加；太大的挪成 `.old`、盖掉上一份；最后 20 行、最多 4 KiB、截了半截的那一行不要 |
| `crates/miyu-endpoint/tests/extensions.rs` | 真核心、真进程（测试用的小程序 `miyu-test-extension`，`crates/miyu-endpoint/src/bin/`，三个平台一样，拷在测试程序旁边）：`manual` 的开了才拉起、工作目录、握手不出示凭据、扩展调 `extension.*` 被拒、标准错误进日志、开关的文件；`always` 的不用开、新核心照开关拉起、随核心退出；有开着的核心不空闲；退出码 1 停下带标准错误的最后几行；别的退出连续 5 次停下；不握手也算失败；关了标准输入不退的到时杀掉；`restart` 换一个新进程、关着的拒绝；程序没找到、协议版本对不上、界面包、没有的包、参数不对；拉起的环境里 `MIYU_HOME`、`MIYU_RESOURCES` 是核心的数据根、资源目录（施工 O-18），`MIYU_PACKAGE_DIR` 是这个包的文件夹，自己装的指家目录里的那一份（施工 S-6 上）。夹具（测试用的扩展、装清单、造核心、等状态）在 `tests/support/extensions.rs` |
| `crates/miyu-endpoint/tests/per_account_retire.rs`、`src/reverse/tests.rs`、`crates/miyu-store/src/kept/tests.rs`（施工 IM-1 下） | 每人一份的那一份问 `process.retire` 得 `ok: true`、退出码 0：停在 `retired`、失败 0、过了退避也不重启；再 `package.call` 替他起一份、答得上。说了 `process.keep` 的，核心再起来时只替他拉起，用过、没说的成员那一份不起。有在等回应的不答应，答应了以后请求、通知都不发、找连接的跳过它。常驻名单记上、拿掉、最后一个拿掉的包不写 |
| `crates/miyu-endpoint/tests/feature_offer.rs`、`crates/miyu-policy/src/features/tests.rs`（施工 IM-6） | 交以前工具归包的功能、开着；交了以后归交的功能、预设没列它的关着，开着的会话下一个回合工具面换掉；工具不是自己的、编号撞了包的功能回 `bad_params`；预设菜单列它、归这个包、名字照语言挑，全部功能开着；整份换掉、空的全撤 |
| `crates/miyu-endpoint/tests/ports.rs`、`src/reverse/tests.rs`（施工 IM-5） | 端口登记：同一个连接登记两次照收，别人登记着的 `port_taken` 不说是谁，放开了的别人能登记，`0` 是 `bad_params`，放别人的不动；管理员看整张表、头的连接登记回 `not_an_extension`；扩展停了它登记的放开、没人接手的也不列；写的那一头没了的连接算断 |
| `crates/miyu-endpoint/tests/packages_programs.rs`（施工 F-8 上再补） | 程序在包文件夹的 `bin/` 里、不在测试程序旁边的扩展装上就拉起；程序被开着写的（只在 Linux 上测）不记成起不来，放开以后在跑 |
| `crates/miyu-endpoint/tests/extension_stream.rs`（施工 9-4 补） | 订阅回整份、名字照语言；开了推到在跑、关了推到关着；带 `after` 和会话的参数不对；取消了不推 |
| `crates/miyu-endpoint/tests/extension_rights.rs`、`src/connection/tests.rs`、`xtask/src/rights/tests.rs`（施工 X-30） | 真核心拉起只声明 `tools` 的测试扩展：只给人的、表里没有的回 `local_only`，缺能力的回 `needs_approval`、`data.capabilities` 是缺的那一项（参数都不看），读模型目录照常；声明了 `sessions.drive`、`events.read` 的造会话、说话、订阅照常，说的话 `by` 是 `harness`、名字是包编号，带 `from` 回 `bad_params`。表：原来只给人的照旧、新挡的几类、照参数分的（`as`、`venue.*` 事件、空的 `provide`、订阅的流）。门禁只认方法样子的字面量 |
| `crates/miyu-endpoint/tests/extension_approval.rs`、`crates/miyu-config/src/package/tests.rs`、`crates/miyu-store/src/extensions/tests.rs`（施工 9-4 下上） | 清单的能力照表的先后、不认识的和重复的报 `bad_capability`；开关文件的 `approved` 读写、以前的文件当没批过、没批的不写这一格；管理员装的：不带 `approve` 拒绝 `needs_approval`、只批一部分说剩下的、不是声明的名字 `bad_params`、批了开起来、记下、关了不清；`always` 的没批不拉起、不拦空闲、`restart` 拒绝、批了拉起；升级多了的只问多的；出厂的扩展不用批（施工 S-6 下起照测试自己放的出厂扩展 `courier`）；名字照连接的语言 |
| `crates/miyu-endpoint/tests/extension_config.rs`、`src/extensions/config/tests.rs`（施工 9-4 下下） | 握手交自己的键（默认值、密钥真值、没设的不放），头的连接没有；改了自己的项推、`secret.set` 换了值推新真值、去掉了推 `null`、别的项变了不推；只放变了的键 |
| `crates/miyu-endpoint/tests/audit_extension_core_key.rs`（施工 5-19） | 包编号和核心模块同名（`ui`）、没声明配置项的扩展改 `ui.language` 回 `local_only`，系统配置里什么都没写 |
| `crates/miyu-endpoint/tests/packages_extensions.rs`（施工 F-5 下） | 真核心装上一个 `always` 的扩展当场拉起、它登记的工具进工具面、调得到；卸掉当场停下、`extension.status` 不列、用过它的会话照旧留着、调到报「已卸载」；升级了的停下再拉起；装别的包时清单没变的进程不换。卸包先停再删（停下时清单还在）；升级成撞了别的包的撤回、原来的重新拉起；换不成、删不成的（Unix 上目录只读）照原来的重新拉起（施工 F-5 补） |
| `crates/miyu-endpoint/tests/turn_inject.rs`、`src/hooks/tests.rs`（施工 X-16） | 真核心拉起八个测试扩展：声明了、工具面上有的交的两块照包的编号进日志（`by` 是包）、进请求排在人说的话前面；没声明的、工具只在群里的不问；超时、回错误、写法不对、交了四块的当没交、这一轮照常走完；回应的写法（至多 3 块、1 到 4000 字、类别和包的编号合写法、一块不对整份不要） |
| `crates/miyu-endpoint/tests/system_account.rs`、`crates/miyu-store/src/packages/tests.rs`、`crates/miyu-store/src/personas/tests.rs`（施工 O-4 下） | 声明了系统账号的包：起来时建它的工作区；起来以后才装上的、装回来的当场建（施工 F-5 下），会话写进它自己的索引；拉起的扩展握手的 `account` 是它；群的场所会话归它（目录、工作目录、回应的 `account`）、找回同一个；对应表里的主人的私聊照旧归主人、这个连接照样能对它说话；群里主人说的记成外部身份带 `account`；附件拷进它名下；用量记在它名下；记忆归管理员、删了照管理员埋墓碑；没声明的扩展照旧 `no_system_account`、不建账号；撞了管理员的 `account_taken` |

### 起草时定的

- 开关放 `system/`、不放 `state/`：`state/` 是派生的、能删掉重建，开关是人做的决定（2026-10-08 主会话）。
- 退出码 0 也算失败：扩展该常驻，自己退了就是没在干活（2026-10-08 主会话，照施工方案「退出码 1 以外的退避重启」）。施工 IM-1 下起，先问过 `process.retire`、核心答应了的除外：每人一份的 im 没配连接的空闲了要退，省资源（im 设计 06 第 4.6 节第 1 样，核心审过）。
- 「连续」照跑满 60 秒断开：握了手就从零数的话，一握手就崩的扩展会一直重启下去（2026-10-08 主会话）。
- 标准错误直接接文件、不经核心转：核心不读它、不拷它，不怕它写得多（2026-10-08 主会话）。
- 测试用的扩展是端点 crate 里的一个小程序（`miyu-test-extension`），不是 `sh` 脚本：包的程序在 Windows 上找的是 `.exe`，脚本只管得了 Unix（2026-10-08 主会话）。
- 协议版本对不上的不拉起：说不通的话起了也白起（2026-10-08 主会话）。
- 拉起时明着设 `MIYU_HOME`、`MIYU_RESOURCES`，值取核心手上的数据根、资源目录，不从核心自己的环境变量转抄；只加这两个，别的环境照旧继承，三个平台一条路（2026-10-08 主会话，施工 O-18 要的：真核心拉起真的桥的测试）。施工 S-6 上多一个 `MIYU_PACKAGE_DIR`：这个包的文件夹由核心挑是哪一层，扩展不自己拼家目录的布局（家目录怎么排是核心的事；2026-10-16 主会话，设计 32 第九节第 2 条）。施工 F-15 再多一个 `MIYU_PACKAGE_STATE`：这个包的状态目录（工作目录那一个）的真实位置，拉起前建好；作者说明改成一律照它找状态目录（2026-10-20 项目主人定：人敲命令时工作目录是人的目录，靠工作目录找不到）。

- 装卸时照清单前后对：卸掉的停、新的起、变了的重起、没变的不动（2026-10-09 主会话，施工 F-5 下）。比较整份清单，不另记版本号：清单里哪一格变了，起来的进程都可能不一样（参数、能力、配置项）。卸掉的那一格状态留着、记成关着：装回来从零数，不留着没用的进程。

### 还没有的

- 装包时批能力：有了装包那一步，审批挪到装的时候。
- 沙盒里跑：另做（`network`、`fs.*` 现在管的是经核心做的，扩展自己联网、读写文件不挡）。照能力挡方法、扩展说的话记成它自己：施工 X-30 做了（「谁能调」）。
- 扩展提供工具、命令：O-2；回合开始的挂接点：X-16；放在 system 尾部的：X-20。`tool.guard`、`tool.rewrite` 还没有。
