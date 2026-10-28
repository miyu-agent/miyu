## TW-1：包的会话状态，待办移出核心

状态：主会话做，分上、中上、中下、下四步。项目主人 10-27 定（通用化、`todowrite` 进官方源的可选包、排在接入通讯平台「按人接入」之后），10-28 答还没发版、不做升级和老日志迁移。设计 `46-包的会话状态.md`。

### 一、为什么

待办是核心里的特例：内核有专门的效果、事件、命令、表、推送、压缩分支，会话、端点、两个头各有一份。扩展想做同样的「计划」「进度」交不了效果，界面块的内容只在内存里、跟不上撤销和压缩。项目主人要通用化：包在会话里记一份自己的状态，`todowrite` 当第一个用它的包，核心的待办代码删掉。

### 二、上：定下的（技术题，主会话定）

1. **一份状态**：`{"widgets"?, "note"?, "data"?}`，空的是清掉。原稿一个 `view` 只画得了一块，`todowrite` 要侧边栏、底栏两块内容不同，改成 `widgets` 按块的编号各给 `{"content", "title"?}`，照每块声明的位置查写法。
2. **内核不解读**：`PackageState {package, state}`，`State` 三格都是原样的 JSON（`note` 是字），内核只认空的是清掉。效果和程序直接写的事件同名 `package.state`、同一种 `body`。
3. **有效历史的表**照待办的表做（`history/states.rs`）：每个包最近一份没撤掉的；清空上下文的检查点清掉全部；平常的压缩不动。
4. **推送**只比给头看的 `widgets`：`note`、`data` 改了不推。
5. **压缩**：每个包的 `note` 照包的编号原样接在取回指路（和待办）后面，不另加模板（给模型的字非必要不加）。没有包状态的会话检查点字节不变。
6. **查写法**在协议端点（`sessions/state.rs`）：`note` 照给模型看的字的规矩（复用 `miyu_policy::overrides::model_text`，最多 2000 字）；`data` 写出来最多 16 KiB；`widgets` 只收清单写了 `state = true` 的块。工具结果带的写坏了整份丢掉、结果照常、记一行；程序直接写的拒（`bad_widget_content` 或 `bad_params` 带 `field`）。
7. **两个方法**只给扩展，要 `sessions.drive`（同 J-4 复用，不加新能力名）：`session.set_state` 不开回合，回合进行中写的跟着那一轮；`session.state` 读回自己的（含 `note`、`data`）。会话属主那一道照通用的查。
8. 工具结果里 `state` 写 `null` 当没写，清掉写 `{}`（serde 分不出没写和 `null`，写法照 `detail`、`data`）。
9. 会话给头看的那一份多 `states`（每个包的完整状态），只在落了盘推出去的事件动了状态时照内核的换。

### 三、上：做了什么

1. `miyu-kernel`：`event/package.rs`（`PackageState`、`State`）、效果和事件 `package.state`、瞬时事件 `package.state_changed`；`history/states.rs`；`session/states.rs`（`Command::SetState`、变了才推、载入不推）；`rebuild.rs` 的 `package_notes`；样本两份；测具 `Play::States`。
2. `miyu-tool`、`miyu-session`：效果原样换；`Shown.states`、`Handle::package_state`。
3. `miyu-config`：`[[widget]] state = true`，只给 `scope = "session"`。
4. `miyu-endpoint`：`sessions/state.rs`（查写法、两个方法）；`tool.call` 回应的 `state`（`provide/remote.rs`，包声明的状态块在造工具时取好）；扩展能调的表；方法清单。测试扩展多一步 `session:<路径>`、两件工具 `state`、`bad_state`。
5. 蓝图 `kernel/events-bodies.md`、`kernel/session.md`、`compaction.md`、`providers.md`、`extensions.md`、`widgets.md`；参考 `extensions`、`events` 中英两份。

### 四、上：怎么验

1. `miyu-kernel` 单测：每个包最近一份没撤掉的；空的是清掉；清空上下文清掉全部、平常的压缩不动；撤销退回、恢复回来；压缩以后的有效历史照样记得；只有写、清空、撤销、恢复算动了状态。
2. `miyu-kernel` 会话测试：落了盘才推、只比给头看的；程序直接写的不开回合、落了盘回应；撤销、恢复跟着推；载入不推；还没落盘的不推；压缩时包的 `note` 照包的编号接在取回指路后面。样本 `events/package.state.jsonl`、`transient/package.state_changed.jsonl` 逐字节比。
3. `miyu-config` 单测：`state = true` 只给跟着会话的块，不是布尔的报错。
4. 端点单测 `sessions/state/tests.rs`：清掉的几种写法；三格照写的记；每一种写坏报哪一格；内容照块的位置查（底栏不收列表）。
5. 端点 `package_state.rs`：测试扩展 `session.set_state` 写、`session.state` 读回（含 `data`）、写没标 `state = true` 的块拒 `bad_params`（`state.widgets.plain`）；工具结果带的记进效果，写了没声明的块的整份丢掉、结果照常。
6. 手写变异 16 个全杀。「还没落盘的不推」头一轮活了，补了会话测试才杀掉。
7. 本机门禁全过。
