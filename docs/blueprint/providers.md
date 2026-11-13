## 提供者：扩展登记工具、核心反向调用

### 是什么

核心自带的工具编译在核心里（`tools/`）。通讯平台的桥要给她「发群消息」「撤回」这类只有它做得到的工具，以后头也会有（终端集成）。提供者经协议的 `provide` 把自己的工具登记进核心的工具目录；她调到这件工具时，核心反向调用 `tool.call` 交给那个连接去跑，等它回结果（`05-内核接口.md` 第四节「内核空间模块与外部扩展」、第六节「工具的规格」、第八节「启用、注册与目录快照」，`04-核心协议.md` 第九节「方法一览」的「提供者」「反向调用」）。

状态：图纸，施工 O-2 上起（2026-10-09 主会话写；O-2 由主会话拆成上、中、下，补另起）。上、中、下做了。扩展的工具的效果和声明（设计 `63-扩展工具的效果和声明.md`）：XG-1 上做了（`paths`、`effects`、`background_parameters`），XG-1 下做了（`runs_commands`、能力 `tools.commands`、`tool.call` 的 `channel`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/wire.rs` | 读进来的一行多一种：对核心发出去的请求的回应 |
| `crates/miyu-endpoint/src/reverse.rs` | 反向调用：核心发给一个连接的请求（编号 `core-<n>`），等它的回应；连接断了，在等的都了结（施工 O-2 上）；对方答的那一行太长、没读的，等它的当场了结成一条错误（施工 YC-5补） |
| `crates/miyu-endpoint/src/provide.rs` | `provide`：查规格、登记进目录；提供者表：哪个包现在由哪个连接提供（施工 O-2 上） |
| `crates/miyu-endpoint/src/provide/remote.rs` | 提供者的一件工具（`Tool` 的实现）：执行时经提供者表找到那个连接、反向调用 `tool.call`（施工 O-2 上）；照 `paths` 报路径、带 `checked`，照 `background_parameters` 交后台参数（施工 XG-1 上） |
| `crates/miyu-endpoint/src/provide/declared.rs` | 规格里的 `paths`、`background_parameters` 照 `input_schema` 查；照 `paths` 从参数取要碰的路径（施工 XG-1 上，设计 63 第二、三节） |
| `crates/miyu-endpoint/src/provide/commands.rs` | 声明了 `runs_commands` 的：说明末尾 `miyu call` 那一段、一次调用开一个认扩展子孙的端点（施工 XG-1 下，设计 63 第六节第 2 条；端点在 `miyu-call`，`tools/shell.md`「脚本调工具」） |
| `crates/miyu-endpoint/src/provide/effects.rs` | 扩展交的效果：发之前读改前，回应到了查 `effects`、读改后，丢的记 `WARN`（施工 XG-1 上，设计 63 第三、四节） |
| `crates/miyu-tool/src/catalog.rs` | 目录多一样：换掉一个包的工具，交回新的目录（施工 O-2 上）；记着哪几个包是提供者登记的（施工 O-2 中） |
| `crates/miyu-tool/src/shelf.rs` | 目录架子：核心现在的目录，换一次是一代；核心、执行器、权限策略、换快照的那一段拿着同一个（施工 O-2 中） |
| `crates/miyu-endpoint/src/provide/cache.rs` | 登记缓存：`provide` 成了写下，起来时、开扩展时照它先登记（施工 O-2 中） |
| `crates/miyu-session/src/actor/persona.rs` | 回合开头换快照：目录换了代，照现在的登记重新筛提供者的工具（施工 O-2 中） |
| `crates/miyu-kernel/src/session/asked.rs` | 是谁要她做的：回合记下她这时在回应的那一条的 `by`，派工具时带上（施工 O-2 下） |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展多一步 `serve`：登记以后答 `tool.call`（施工 O-2 上）；`silent` 不答（施工 O-2 下）；`skip`、`skip_error` 带 `end_turn`（施工 4-14） |

### 对外的样子

**`provide {tools}`**（施工 O-2 上）：

1. 只收核心拉起的扩展的连接（`hello` 时认出是哪个包的）；别的连接回 `not_a_provider`。头扮演提供者随 O-2（下）。
2. `tools` 是这个包现在提供的全部工具，每件 `{name, description, input_schema, access, venues}`：
   - `name` 只用英文字母、数字、`_`、`-`，1 到 64 个字符；
   - `input_schema` 是 `{"type":"object",…}`；
   - `access` 是 `read`、`write`、`execute`、`network`、`outbound`、`venue`、`install` 之一（`venue` 施工 O-31 前，见下面「在场所里做的事」；`install` 施工 F-10 下，改这台机器上装了什么，`tools/package.md`）；
   - `need_load` 退役（施工 L-1 登记、FL-1 拆掉：工具一律全量）：写了的照旧登记得上，写什么都不看；
   - `clicall` 可以不写：`true` 是脚本里能经 `miyu call` 调（施工 M-2，设计 `39-脚本调工具.md` 第三节），不写、`false` 的不能；不是布尔的 `bad_tool`，`problem` 是 `clicall`；
   - `timeout_ms` 可以不写：等它答多久，1000 到 600000 毫秒，不写是 60000（施工 O-2 下）；
   - `display_name` 可以不写：给人看的名字，语言到名字（`{"zh": "发到群里", "en": "Send to group"}`），时间线上照连接的语言写它，没写的照工具名（施工 9-8 六补）；语言、名字是空白的拒；
   - `subject` 可以不写：给人看的名字后面跟哪一个参数的值（例如 `reason`），要是 `input_schema` 的 `properties` 里的一个名字，不是的（参数格式里没有 `properties` 的也是）拒，`problem` 是 `subject`（施工 S-6 上）；核心照它放进 `human.get` 那件工具的 `subject`（`protocol.md` 的 `human.get` 第 4 条）。调用那一刻记进 `tool.result` 的只有 `display_name`；
   - `plugin` 可以不写：场所插件的名字（施工 O-60，`venues.md`「场所插件」），1 到 64 个字，写法同包的编号；写了的，场所会话里只有那个场所开着它（`venue.plugins`）才进工具面，本机会话照旧。写法不对的 `bad_tool`，`problem` 是 `plugin`；
   - `role`、`scope` 可以不写（施工 XG-2 下，设计 `64-工具职能.md`，写法同出厂的说明文件，`tools/interface.md`）：职能、范围；范围只在 `venues` 之内再收窄。写法不对的 `bad_tool`，`problem` 是 `role`、`scope`；
   - `paths` 可以不写（施工 XG-1 上，设计 63 第二、三节）：哪几个参数是路径，`[{"parameter": "file_path", "write": true}]`。`parameter` 是 `input_schema.properties` 顶层的一个，类型是字（`"type": "string"`）或字的数组（`"items": {"type": "string"}`）；`write` 不写是假（读），写了真的，`access` 要是 `write`；同一个参数不能写两次；别的格不收。写 `null` 同不写；写法不对的 `bad_tool`，`problem` 是 `paths`。核心照它报这次调用要碰的路径（下面「声明了 `paths` 的」）；
   - `background_parameters` 可以不写（施工 XG-1 上，设计 63 第六节第 1 条）：只在会话有后台时才给的几个参数名，`["run_in_background"]`。要是 `input_schema.properties` 顶层的、不在 `required` 里的（去掉了请求就对不上格式）、不重复。写 `null` 同不写；写法不对的 `bad_tool`，`problem` 是 `background_parameters`。后台运行关着的会话（`presets.md`），组工具面时去掉它们（`Tool::background_parameters`，同出厂的 `shell`）；
   - `runs_commands` 可以不写（施工 XG-1 下，设计 63 第二节、第六节第 2 条）：`true` 是这件工具会跑命令，下面「会跑命令的」；不写、写 `null`（同不写，施工 XG-1 补核对过：规格的格都是 serde 的 `Option`，`null` 读成没有）、`false` 的不是。写了真假、`null` 以外的 `bad_tool`，`problem` 是 `runs_commands`；写了真的、包没批能力 `tools.commands` 的（清单没声明的也是）整个不收，`needs_approval`，`data.capabilities` 是 `["tools.commands"]`（同 `connection/rights.rs` 挡方法的写法）。出厂的包声明了就算批过（`extensions.md`「能力」第 3 条）；
   - 上面三格都不写的工具，规格、工具面、登记缓存逐字节和以前一样；
   - `venues` 是给哪种会话：`local`（本机的）、`private`（通讯平台的私聊）、`group`（群）里的一个或几个，不能是空的（05 第六节的 `venues`；2026-10-09 和通讯平台的会话定：「发给任意好友或群」只给本机，`skip_reply` 只给场所）。
3. 登记进工具目录，归这个包：换掉它上一次登记的那几件。名字撞上核心自带的、别的包的、写法不对的：整个不收，`bad_tool`，`data` 是 `{"tool": 名字, "problem": "duplicate" | "name" | "parameters" | "access" | "venues" | "timeout" | "clicall" | "result" | "display_name" | "subject" | "feature" | "plugin" | "role" | "scope" | "paths" | "background_parameters" | "runs_commands"}`。`feature`（施工 T-2）：清单写了几个功能、这一件哪个功能都没列，或者写了空的 `[features]`；归法同预设认工具归哪个功能（`Features::of_tool`：列了的照列的，只有一个功能的都归它，没写 `[features]` 的整个包算一个）。归不上的预设开关不了它。回应 `{"tools": 件数}`。
4. 记下这个包现在由这个连接提供。连接断了、扩展崩了，工具照旧留在目录里（`05-内核接口.md` 第九节：工具从目录里消失会改变请求字节），被调到时回「暂时不可用」。
5. 这一次登记的原文写进登记缓存 `var/cache/provide/<包>.json`（施工 O-2 中，下面「登记缓存」）。

**在场所里做的事**（访问类别 `venue`，施工 O-31 前，2026-10-10 核心定；`kernel/tools.md`、`session/guard.md` 第四条）：通讯平台上的动作（撤回、禁言、戳一戳这些）登记成 `venue`。权限策略只看会话在不在场所里：场所会话里放行、不问人，本机的会话里拒绝（`not_in_venue`）。也给本机用的（工具的 `venues` 里有 `local`，施工 O-31 补，2026-10-17 接入QQ 的会话要的「主动发消息」）本机的会话里不拒，照对外发消息（`outbound`）判：完全放开放行，只读时问人，没人能确认的拒（本机的会话就是主人本人）。场所会话没人能确认：`network`、`outbound` 在那里照级别判，平时照完全放开放行，只读开着时要问人、等于被拒（A16 起场所会话照完全放开开局，施工 LV-1），所以要在群里用的动作登记成 `venue`，只读开着也照样放行。

1. **谁能叫、能动谁是提供者的义务**：登记成 `venue` 的工具，核心不替它挡人。提供者照 `tool.call` 的 `by`（`by.role` 是 `manager` 的是管理的人）、`owner`、`venue.binding`（这个平台身份是不是主人，`venues.md`「问对应表」：动谁的时候要认目标）自己挡；挡下的照工具结果回一句，`error` 是真。
2. **出去的动作不许记成 `read`**：`read` 是不碰这台机器、不出 Miyu 的。`send_message` 用 `read` 是因为它只发给她能看到的会话，不出 Miyu（`tools/send_message.md`）；`skip_reply` 只让桥这一轮不发，也不出去。撤回、禁言、戳一戳会在平台上留下后果，是 `venue`。

**`extension.disable`**（施工 O-2 中）：关掉的扩展的工具出目录，开着的会话下一个回合换掉；缓存留着。关是人的决定，和重启以后关着的包不读缓存一致；崩了、断了是一时的，不出目录。

**造会话时的工具面**：照这时的目录，提供者的工具照它的 `venues` 挑这个会话是哪种（本机的、私聊、群）。场所会话里写了 `plugin` 的再照那个场所开着的场所插件挑（施工 O-60）。提供者的工具归它的包，预设照包开关，和核心自带的一样。

**包的工具变了，开着的会话下一个回合换上**（施工 O-2 中，`kernel/session.md`「换策略快照」）：会话记着它的快照照的是目录的第几代；回合开头目录换了代，照现在的目录、快照里的预设重新筛工具面：

1. 提供者的包里的那几件照现在的登记：新登记的加上，改了说明、参数的照新的，不再给这个会话的拿掉。
2. 核心自带的照旧快照里的原样（说明、`subagent` 能选的池、改过名的旧名字都不变），新装的自带工具不加：它们只随程序升级变，前缀稳定第一；预设改了时才照现在的目录加（施工 P-2 下）。
3. 目录里没有了的：会话上一次对过的那一代里是提供者的拿掉（扩展关掉了、不再登记它）；别的照旧留着，调到时暂时不可用（施工 4-2：程序升级拿掉的；载入的会话不知道上一次对的是哪一代，认不出来的都留着）。
4. 拼出来和原来一样的（只给群的工具、预设关着的包）什么都不记；不一样的照换人格的办法换上，记 `session.policy_changed`。
5. 载入的会话不知道快照照的是哪一代，第一个回合照现在的目录对一次。以前造的快照找不回预设的（没有指纹）不跟。
6. 程序升级过的老会话（核心的字变了）照样跟（施工 P-1 三补）：工具面真的变了才换，换的时候新的核心的字一起换上；光是升级、只有装了没开的那一行变了的不换。

**登记缓存**（施工 O-2 中）：`provide` 成了，这个包的参数原样写进 `var/cache/provide/<包>.json`（派生的数据：一样的不写，先写临时文件再改名）。核心起来拉起扩展以前、`extension.enable` 拉起以前，开着的、批过的包照缓存先登记进目录，连接还没来，调到的暂时不可用；扩展连上来再 `provide`，照它的换掉。不缓存的话，核心重启以后、扩展重新登记以前跑的那一轮会把它的工具换掉，登记了又换回来，每次重启断两次缓存。缓存读不成、写错了、照它登记不上（和别的撞名）的不用，记一行 `WARN`；包卸了的不读，文件留着不管。

**给模型看的字**：出厂的包（随 Miyu 装的，通讯平台的桥）的工具说明放在资源目录 `software/<包>/tools/<名字>.json`，和核心自带的一个写法（`{description, parameters}`），扩展启动时经 `miyu_tool::load::spec` 读它们再 `provide`；登记进 `26-提示词.md` 第十节（登记簿门禁照资源目录查，不用另做），包自己的工具面预算照记忆的办法在它自己的 crate 里守（`crates/miyu-memory/tests/budget.rs`，量法记进 `10-自带软件.md` 第九节）（2026-10-09 和通讯平台的会话定）。第三方的扩展不在仓库里，不管。

**后台页的方法**（施工 F-6 中）：扩展另用 `package.methods` 登记它的后台页要调的方法，核心反向发 `method.call`，和登记工具的 `provide` 分开、不缓存，见 `package-pages.md`。

**`tool.call`**（反向调用，核心发给提供者，施工 O-2 上）：`{"jsonrpc":"2.0","id":"core-<n>","method":"tool.call","params":{…}}`，`params`：

| 格 | 是什么 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数，修正过的 JSON 对象 |
| `cwd` | 这一轮的工作目录 |
| `by` | 是谁要她做的（施工 O-2 下，下面「是谁要的」）：那一条的 `by` 原样，例如 `{"kind":"external","venue":…,"id":"qq:…","role":"manager"}`；没有触发的回合不写 |
| `owner` | 核心认不认 `by` 是主人本人（施工 O-2 下）：本机的人、私聊里对应表认出的本人、群里对应表里有的外部身份是；没有 `by` 的不是 |
| `checked` | 只给声明了 `paths` 的（施工 XG-1 上）：权限链判过的真实位置，照参数报的先后（字的数组每项一条），就是 `Call::checked`；交出去之前去掉 Windows 的 `\\?\`（`miyu_fs::usable`）。扩展应当写这几个位置 |
| `channel` | 只给声明了 `runs_commands` 的、这次调用开了通道的（施工 XG-1 下，下面「会跑命令的」）：`{"sock", "token", "bin"}`。`sock` 是端点（Unix 上套接字的路径，Windows 上 `\\.\pipe\miyu-call-…`），`token` 是 32 个十六进制字的令牌，`bin` 是只放着 `miyu` 的目录（建不成的不写）。扩展把前两个设成 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` 进它起的命令，`bin` 放 `PATH` 最前面 |

回应 `{"result": {"blocks": [内容块…], "error": 布尔, "end_turn": 布尔}}`：内容块照内核的写法（`text`、`image`），`error` 不写是假。`end_turn` 是真的（施工 4-14，通讯平台的 `skip_reply` 这类「这一轮不说了」）：这一步几件工具的结果都到了，核心不再请求模型，这一轮照走完了结束（同一步别的照常跑完；这一轮里排着、并进来的照常接着开下一轮；分叉照常封存），结果上记着 `end_turn`；`error` 是真的不认；不写是假。`detail` 是给头看的结构化结果（施工 W-11 补，2026-10-18 项目主人定）：照 `miyu_tool::detail` 的三种通用形状（`links`、`table`、`images`）读，多一格少一格、超了上限、`error` 是真的，整格丢掉、结果照常，运行日志记 `WARN tool detail dropped tool=… why=…`；读成了的照形状重写一遍记进结果（不是原样）。`images` 引用的 blob 和图片块一样，要在存处里。`data` 是给脚本的结构化结果（施工 X-33，设计 `39-脚本调工具.md` 第六节第 3 条、MC7）：任意 JSON，可以不写；照收、交进 `Done.data`，只给经 `miyu call` 调到它的脚本（印成一行 JSON），不进上下文、不进 `tool.result`；`error` 是真的不交，同 MCP 的 `structuredContent`。不另截断，整条回应照旧受管道一行 1 MiB 的上限。不拿 `detail` 顶它：`detail` 跟着头的画法走，给脚本什么扩展自己写。`state` 是这个包在这个会话里的一份状态（施工 TW-1，设计 46，`kernel/session.md`「包的会话状态」）：照 `session.set_state` 的写法查（`sessions/state.rs` 的 `read`，包声明的状态块在造这件工具时取好），合的记成结果的效果 `package.state`，写坏了整份丢掉、结果照常、运行日志记 `WARN package state dropped package=… why=…`；`null` 当没写，清掉写 `{}`；`error` 是真的不收。`effects` 是这次调用的效果（施工 XG-1 上，下面「扩展交的效果」），查过才收。回应是 JSON-RPC 的错误、写法不对的：算这次调用出错，原话写进结果。图片、文件块引用的 blob 先经 `blob.put` 传上来（扩展和会话共用一个存处），块原样不动（施工 O-2 三补，同 `session.send` 的附件）。存处里没有这个 blob 的，这次调用照暂时不可用。

她发出去的图（施工 X-19，设计 `34-表情包.md` 第五节第 4 条）：回应多一格 `sent`：`[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`，至多 4 张，多了的整份当写法不对。图同上先经 `blob.put` 传；`path` 写了也不收（扩展说的本机位置不可信）。记进 `tool.result.sent`，**不发给模型**（她看到的照旧只有 `blocks`），头画在正文里，通讯平台照它发（`sticker` 的当表情）。

**声明了 `paths` 的**（施工 XG-1 上，设计 63 第三节，2026-11-05 core1 定 4A）：

1. `RemoteTool::targets` 照 `paths` 从参数里取路径：字的一条，字的数组每项一条，照声明的先后；参数缺的、不是字的不出（同内置工具「参数不对交回空的」，执行时扩展自己报错）。交给权限链，和内置 `edit` 走同一张判法表（`session/guard.md` 第三条）：数据根拒、扩展收不到 `tool.call`，只读时写一律拒，私密目录照表（A16 起完全放开下写工作区外不问人，施工 LV-1）；不声明的照「不报路径的调用」判（读写放行），以前第三方的写工具改工作区外的文件不问人就是这样。
2. `tool.call` 带 `checked`。判过以后路径上的链接被换掉的，扩展写的就不是判的那个：照内置 `write` 的规矩，扩展写之前自己换真实位置核对是 `checked` 里的一条（作者文档写明）。
3. 发 `tool.call` 之前，核心在阻塞线程里把要写的那几条各读一份留着（改前）：照「安全地打开」读（`miyu_fs::open_file`，不跟最后一层链接、不是普通文件的不读），不在的记成新建，超过 20 MiB 的不读（它的 `file.changed` 照 `too_big` 丢）。没有要写的不读，不多花一次磁盘。

**扩展交的效果**（施工 XG-1 上，设计 63 第四、五节）：回应的 `effects` 照 `tool.result` 里效果的写法（`kernel/events-bodies.md`「效果」），核心不信扩展说的内容，只信它说「哪条改了」：

1. 收四种：`{"kind": "file.changed", "path", "before"?, "after"?, "before_text"?, "after_text"?}`（哈希、原文带了的要和核心读的对得上）；`{"kind": "file.read", "path", "lines"?}`（`lines` 是 `[从, 到]`，从 1 数起、从不大于到；哈希由核心读盘算，扩展写了也不看）；`{"kind": "file.trashed", "path", "trash"}`（`trash` 只给人看，至多 4096 个字；施工 U-1 起不移回来）；`{"kind": "todo.written", "todos", "done"?}`（只查写法，`done` 写 `null` 同不写（施工 XG-1 补），换掉 `todowrite` 的包要它）。别的种类（`job.*`、`peer.watch`、`file.shown`、`package.state` 这些要核心的账本、端口，或者另有一格）整条丢，不原样留：不可信的东西不进日志。
2. 文件效果的 `path` 照这次调用的工作目录、家目录换成真实位置（同 `Call::real`），要是这次调用链判过的一条：`file.changed`、`file.trashed` 要是判了写的，`file.read` 读写都行。没声明 `paths` 的工具一条文件效果都不收（同 `shell`：命令改的文件没有差异）。核心照效果读盘的只有这里的改后和压后重读（`compaction.md` 第九条），两处的路径都只能是过了链的，扩展报 `~/.ssh/id_rsa` 这类进不了她的上下文。
3. `file.changed`：改前用发之前留的那份，改后由核心现读（同一个文件在一次回应里只读一次）；两份一样的丢（`unchanged`）。`file.trashed`：路径现在要不在了（链接本身也不算在）。`file.read`：哈希照现读的整份。读成的照内置工具的样子做成 `miyu_tool::Effect`，接在 `state` 那一条前面交给执行器：存 blob、小的写原文（施工 V-18）、记她看过的、看差异（`view.detail`、条目的增删行数）、压后重读，从这一步起和内置 `edit` 交的分不出来。撤销照有效历史重建她看过的，不改回文件（施工 U-1），和内置的一样。
4. 丢的记一行 `WARN tool effect dropped tool=… kind=… why=…`，`why` 是 `not_checked`（没判过、要写却只判了读、换不成真实位置）、`unchanged`、`mismatch`（哈希、原文对不上；说改了、读了却不在、不是普通文件、读不了；说删了却还在）、`too_big`、`kind`、`shape`（写法不对、`effects` 不是数组、同一种同一个文件报了第二遍）之一；不写路径。结果别的格照常，不算出错，不告诉她。
5. `error` 是真的照样收文件效果（改了一半出错，改了的是真的）；`todo.written` 不收（同 `state`），不记 `WARN`。超时、被掐的没有回应，没有效果。
6. 报假效果拿不到什么（设计 63 第五节）：撤销、恢复、重做不写文件，写不坏别的文件；改前改后都是核心自己读的，骗不了看差异的人，最多少报。扩展现在没进沙盒，本来就能自己读写这些文件；这一套守的是「核心不替扩展多做一步」，以后扩展进沙盒照样成立，过了链的写路径就是那时要按调用放开写的几条（能力 `fs.write`）。

**会跑命令的**（施工 XG-1 下，设计 63 第六节第 2 条、第七节第 3 条，2026-11-05 core1 定 3A；照内置 `shell` 的几处，一处不按名字）：

1. `RemoteTool::runs_commands` 照规格交。核心照它认跑命令的工具的几处都接上：说明后面接 `miyu call` 那一段（`Tool::clicall_section`，字是核心的一份 `core/call/`，`shell` 也读它，字节不变；例子照这台机器挑：Windows 上 PowerShell 的，别处有 `jq` 用它、没有有 `python3` 用 Python、都没有不带）；会话照现在的条件开端口（`session/tools.md` 第 6 条：工具面上有脚本里能调的、或者核心自带的命令能调的）；后台运行关着的会话造工具面、拼 system 时算它一件（`agents.rs`）；`miyu check` 放行出沙盒的跑命令工具照旧提醒 `sandbox_shell`（`session/guard.md`）。
2. 通道：这次调用拿到了端口、知道扩展进程的进程号的（提供者表在 `provide` 时照扩展的状态记下，`Starting`、`Running` 的 `pid`），开一个端点（`miyu-call`，`tools/shell.md`「脚本调工具」），`tool.call` 带 `channel`。认人照「连上来的是这个扩展进程的子孙」加令牌：Linux 照 `/proc/<pid>/stat` 的父进程链，macOS 照 `proc_pidinfo`，Windows 照进程快照的父进程号（`miyu_pipe::process_parents`），每一层还要父进程起得不比它晚（`miyu_pipe::process_created`，施工 XG-1补2：Windows 不过继，被重用的编号是后起的，问不出时间的当链断）；扩展进程自己不算，`setsid`、两次 `fork` 甩出去的链断了，调不成；过不了的退出码 2。这一层防误用（令牌漏给别的进程、命令甩出去以后还在调），不是安全边界、不防扩展本身：扩展起一个子进程就能调。对扩展的关口是能力 `tools.commands` 要人批（设计 63 第七节第 3 条）。命令里调的照会话的链判，记 `tool.nested`，挂在这次调用下面（同 `shell`）。
3. 端点跟着这次调用：答了、超时、被掐（这次调用的等待被丢掉），丢掉就关，套接字删掉、令牌作废，还在等的、之后连的 `miyu call` 退出码 3。扩展起不了会话的后台任务（不收 `job.started`），没有 `shell` 那种「交给任务表、跟着任务走」。
4. 开不成的（建不了目录、套接字）记一行 `WARN miyu call endpoint not opened`，调用照旧发，只是没有这一格；不知道进程号的（不是核心拉起的连接）不开，记 `DEBUG`。
5. 人在会话里跑命令（`session.send {shell: true}`，设计 57）不交给扩展：认的是 `Tool::serves_person`（内置 `shell` 是），扩展的跑命令工具不算，只有它们的会话照「没有能跑命令的工具」拒（`shell_unavailable`）。

**是谁要的**（施工 O-2 下，`asked.rs`）：回合记下她这时在回应的那一条的 `by`。开回合时是触发的那一条（照记下的几条开的是最后一条）；之后每次请求前，请求新看到的排队消息、并进来的群消息（`turn.joined` 的最后一条）、别的 harness 和别的会话发来的话里最新的那一条换上它；后台命令、子代理的回报是她自己派出去的事，不换。多条触发、并进来的几条照最后一条（2026-10-09 和通讯平台的会话定）：她这时看到的最新那一条才是这次调用要回应的。提供者照 `owner`、`by.role` 自己挡（群里的禁言、撤回、踢人只给主人、管理的人）。

**超时、`tool.cancel`**（施工 O-2 下）：到了登记时写的 `timeout_ms` 没回的，这次调用交回「没在 N 秒内答完、可能做了一部分」（`core/tool-results/timed-out.txt`，字段 `name`、`seconds`，秒数往上取整；提供者的工具自己读，不进快照的核心字：进了以后以前造的会话换不了快照）。核心发通知 `{"jsonrpc":"2.0","method":"tool.cancel","params":{"session":…,"call_id":…}}`（不带 `id`，不等回应）：超时、叫它停（打断改东西的调用；照样等它回，内核到点掐掉）、掐掉（打断别的、关核心，这次调用的等待被丢掉）时各发一次，回应先到的不发。提供者收到了就放下那次调用；已经做了的收不回，回的时候照实说。

### 怎么走

1. **读回应**：读进来的一行有 `result` 或 `error`、没有 `method` 的，是回应：照 `id` 找到核心发出去的那一条，交给等它的；找不到的不理。
2. **发反向调用**：每个连接一张「发出去还没回」的表；编号照连接从 `core-1` 数起。连接断了，表里在等的都了结成「连接断了」，往这个连接写的那一头也放掉：提供者表里存着这个连接，它要是还攥着写的那一头，写的任务就一直不结束，核心察觉不到扩展退出，崩了不重新拉起、端口被占退出了还显示在跑（施工 O-2 再补，通讯平台的会话 O-26 查出来的）。
3. **执行提供者的工具**：执行器照目录找到这件（`RemoteTool`）；它照包查提供者表：没有连接的，交回「暂时不可用」（`core/tool-results/unavailable.txt`）；有的，发 `tool.call`、等回应、到点算超时，超时和叫停时发 `tool.cancel`（施工 O-2 下）。不等了的调用从「发出去还没回」的表里拿掉。
4. **目录换代**：核心的目录放在架子上（`Shelf`），`provide`、读缓存、关扩展、装卸内置包（施工 F-5 中）各换一代，两处同时换的一个接一个；以后造的会话、载入的会话照新的，开着的会话下一个回合照上面「包的工具变了」换上；执行、判权限照现在的那一份找工具。会话一律拿核心那一个架子：会话的执行器、权限策略、放行出沙盒认来处（`Shelf::package_of`）、回合开头换快照都照它。

### 出错

| 什么时候 | 原因码 | 说明 |
|---|---|---|
| 不是核心拉起的扩展 | `not_a_provider` | 头扮演提供者随 O-2（下） |
| 规格不对、撞名、时限出了范围、归不上功能（施工 T-2）、`paths`、`background_parameters` 写错（施工 XG-1 上）、`runs_commands` 写了真假、`null` 以外的（施工 XG-1 下） | `bad_tool` | `data.tool`、`data.problem` |
| 写了 `runs_commands`、包没批 `tools.commands`（施工 XG-1 下） | `needs_approval` | `data.capabilities` 是 `["tools.commands"]`；照缓存登记的不收、记 `WARN` |
| 参数写错 | `bad_params` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 对核心发出去的请求的回应认得出来：`result`、`error` 原样交出，`id` 要是字符串、要是 2.0 |
| `crates/miyu-endpoint/tests/provide_exit.rs`（施工 O-2 再补） | 登记过工具的扩展一直退出：每次都察觉、照退避重新拉起，五次停下；端口被占退出的记成 `config_error` |
| `crates/miyu-endpoint/src/reverse/tests.rs` | 反向调用发出去的一行带 `core-<n>`、方法、参数；对上编号的回应交给等它的，对不上的不理；连接断了，在等的和以后发的都了结；对方答的一行太长的，对得上编号的当场了结成一条错误、对不上的不理（施工 YC-5补；真核心拉起真进程的那一层在 `tests/extension_long_line.rs`） |
| `crates/miyu-endpoint/src/provide/tests.rs` | 访问类别（施工 O-31 前起认 `venue`）、给哪种会话不认识的、空的拒；提供者表照包记，重新登记的换掉旧的；没有连接的、发不出去的暂时不可用，说法同执行器的 |
| `crates/miyu-tool/src/catalog/tests.rs` 的 `replacing_a_package_keeps_the_others_and_checks_the_new_ones` | 换掉一个包的工具：别的包的照留，原来那份不动，撞名、写法不对的整个不收 |
| `crates/miyu-session/src/agents/tests.rs` | 工具面照会话在哪挑提供者的工具：本机的、私聊、群 |
| `crates/miyu-endpoint/tests/provide_venue.rs`（施工 O-31 前） | 契约：登记成 `venue` 的工具收；本机的会话调到它，权限策略当场拒绝、写 `not-in-venue` 那一句，扩展收不到 `tool.call` |
| `crates/miyu-endpoint/tests/provide_features.rs`（施工 T-2） | 写了几个功能的，登记没列的工具整个不收（`feature`）、只登记列了的照收；空的 `[features]` 一件都不收；只写了一个功能的都收 |
| `crates/miyu-endpoint/tests/provide_end_turn.rs`（施工 4-14） | 契约：回应带 `end_turn` 的，这一轮结束、不再请求；出错的照样带上的不认，接着请求 |
| `crates/miyu-endpoint/tests/provide_detail.rs`（施工 W-11 补） | 契约：合形状的 `detail` 记进结果；网址不是 http 的、出错时带的丢掉 |
| `crates/miyu-endpoint/tests/provide_data.rs`（施工 X-33） | 契约：脚本里调到的拿到原样的 `data`；出错时带的不交；直接调用的 `tool.result` 里没有它 |
| `crates/miyu-tool/src/detail/tests.rs`（施工 W-11 补） | 三种形状读进来写回去一样；只有图引用 blob；多一格、少一格、超上限、坏网址、坏格子、坏哈希、不是图、超 64 KiB 各报原因 |
| `crates/miyu-endpoint/tests/provide.rs` | 契约：扩展登记、撞名的整个不收、头不是提供者；新造的会话工具面里有给本机的、没有只给群的；`tool.call` 带会话、调用编号、参数、是谁要的、是不是主人；结果、错误、写法不对的各自交回；不答的到点超时、扩展收到 `tool.cancel`；扩展关掉了，下一个回合没有它的工具 |
| `crates/miyu-kernel/src/session/tests/respond/asking.rs`、`origin/tests.rs` 的 `the_owner_is_a_person_or_someone_on_the_owner_table` | 是谁要的：开回合的触发、照记下的几条开的最后一条、并进来的和排着队的被请求看到以后换上，回报不换、别的 harness 换；主人的判法 |
| `crates/miyu-endpoint/src/provide/tests/remote_tests.rs`、`reverse/tests.rs` | `tool.call` 带 `by`、`owner`；超时交回那一句、发 `tool.cancel`；叫它停发、照样等它回；掐掉发；回应先到的不发；不等了的调用从表里拿掉，通知不带编号、断了不发 |
| `crates/miyu-tool/src/shelf/tests.rs`、`catalog/tests.rs` 的 `the_catalog_knows_which_tools_came_from_a_provider` | 架子换一次是一代、拿着同一个的都看到，换不成的不动；目录记着提供者的包的工具 |
| `crates/miyu-session/src/actor/persona/shelf_tests.rs` | 目录换代：提供者的工具加上、改掉、去掉；自带的照旧、不新加（包里多了一件的）、改过名的照旧名字，目录里没有了的照旧留着；新装上的内置包的工具加进来、卸掉的照旧留着（施工 F-5 中）；只给群的不动本机的会话；载入的对一次；找不回预设的不跟 |
| `crates/miyu-endpoint/src/provide/tests/declared_tests.rs`（施工 XG-1 上） | `paths` 要是顶层的字或字的数组、写的要 `access` 是写、不重复、不多格；`background_parameters` 要是顶层的、不在 `required` 里、不重复；照 `paths` 报路径（数组每项一条，缺的、不是字的不出）；不写两格的登记缓存逐字节和以前一样；三格写 `null` 同不写（施工 XG-1 补） |
| `crates/miyu-endpoint/src/provide/tests/effects_tests.rs`（施工 XG-1 上） | 改前改后核心读、带的哈希原文对不上的丢、没改的丢、新建的、太大的、改后不在的；只收判过的（只判了读的不收改、删，没声明的一条不收）；删了的要真不在、`trash` 至多 4096 字；不收的种类、写法不对的、重复的丢；出错时待办不收、文件照收；待办的 `done` 写 `null` 的照收、写成别的样子的丢（施工 XG-1 补） |
| `crates/miyu-endpoint/tests/provide_effects.rs`（施工 XG-1 上，e0 写的红测试） | 契约：第三方 `edit` 和内置 `edit` 改两个一样的文件，`tool.result` 的 `file.changed`、`view.detail` 的差异和增删行数一样；`tool.call` 带 `checked`；接着用内置 `edit` 两边都不用重读；撤掉那一轮两边一样被拦、文件不动 |
| `crates/miyu-endpoint/tests/provide_paths.rs`（施工 XG-1 上） | 契约：声明的写进数据根当场拒、不问人，扩展收不到 `tool.call`；工作区里的照常发、不问，`checked` 是链判过的真实位置；后台运行关着的会话工具面上没有声明的后台参数，开着的照旧，不写新格的参数格式逐字节照登记的 |
| `crates/miyu-endpoint/tests/provide_effects_log.rs`（施工 XG-1 上，单独一个测试程序） | 契约：报工作区别的文件、工作区外的私密文件、没改说改了、还在说删了、不收的种类、写法不对的各自丢、记 `WARN tool effect dropped`、结果照常；私密文件的路径不进日志、不进运行日志；没声明 `paths` 的一条文件效果不收 |
| `crates/miyu-endpoint/src/provide/tests/declared_tests.rs` 的 `runs_commands_is_a_boolean_and_only_true_runs_commands`（施工 XG-1 下） | `runs_commands` 是布尔、不写是假，别的 `bad_tool`；只有真的跑命令，人跑命令不交给它（`serves_person` 是假）；登记缓存原样存 |
| `crates/miyu-endpoint/tests/provide_commands.rs`（施工 XG-1 下） | 契约：扩展起的命令里经通道调 `probe` 成了，记一条 `tool.nested`、挂在那次调用下面；`tool.call` 带 `channel`（令牌 32 个字、有 `bin`）；核心进程（扩展的父进程，不是子孙）拿着令牌连上来退出码 2、那一次没调；答完端点关了、连不上；没批 `tools.commands` 的 `needs_approval`、不是布尔的 `bad_tool`；说明后面接 `miyu call` 那一段、不跑命令的照登记的；`!命令` 只有扩展的跑命令工具时 `shell_unavailable`；放行出沙盒的跑命令扩展工具 `miyu check` 报 `sandbox_shell` |
| `crates/miyu-call/src/kin/tests.rs`、`unix/tests.rs`、`windows/tests.rs`（施工 XG-1 下） | 子孙照父进程链认：子、孙是，自己、别人家的、问不出的、绕回来的、太长的不是；`/proc/<pid>/stat` 的父进程照最后一个 `)` 往后数；Windows 上父进程起得比子进程晚的（重用了的编号）、问不出时间的链断，同一刻起的算数（`elder_parent`，施工 XG-1补2，到处跑）；照快照找的那一层接线用假的快照、假的时间测（`descends_in_snapshot`，施工 XG-1补3：链上有重用了的编号不是子孙）；真的子进程照这台机器问得出；扩展的端点答它父进程（测试进程是子孙）、拒另一个进程的子孙；交给扩展的那一格和设进命令的环境变量一样，端点名不带令牌的任何一段（施工 XG-1补2） |
| `crates/miyu-endpoint/tests/provide_later.rs` | 契约：先造的会话下一个回合有扩展的工具、调得通、记 `session.policy_changed`，缓存照登记的原样；关掉以后下一个回合拿掉；重启以后扩展没登记，照缓存在目录里，载入的第一个回合对上、调到的暂时不可用，新造的带上；关着的包不读缓存 |

### 还没有的

- O-2（补）：执行中的进度 `tool.progress`；头扮演提供者；提供者登记命令、挂接点。
