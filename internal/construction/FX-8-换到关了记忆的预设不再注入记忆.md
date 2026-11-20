## 施工单 FX-8：会话中途换到关了记忆的预设，记忆不再注入；请求那一行记下工具名

状态：做完了，等合并（分支 `step/FX-8`，从 main 开；主会话 2026-11-21 派工）。蓝图 `presets.md`「改了文件」的记忆一条、「守着它的」、「起草时定的」，`memory.md` 第二节范围一条，`session/actor.md` 日志表的 `request`；设计 `16-人格与预设.md` Y5 第 8 条。

### 目的

项目主人的群会话 2026-11-20 中途把预设换成「无功能」（`none`）：技能跟着关了，记忆却没关。换了以后核心照样注入常驻的摘要（`context.injected` 的 `memory`），之后每轮还有联想（`recall`）、名片（`card`），记忆的三件工具也还在工具面上。

根因：设计 16 Y5 第 8 条定的是「换预设时记忆开不开照开会话时的」，代码照它做（`Chosen::keeping_memory` 把预设里记忆那一格改成开会话时的；回合开始只要接着记忆的端口就问）。开会话时开了记忆的会话，换到哪个预设记忆都开着。直接用「无功能」开的会话没有这个问题：开会话时范围就是 `off`，不接记忆的端口。

另一件：核心的运行日志里，每一次请求那一行（`request`）记下这次请求的工具名，以后核对工具面不用翻会话日志。

### 改成什么样

- 记忆的规矩（主会话 2026-11-21 改 Y5 第 8 条）：开会话时没开记忆的照旧不开；开了的照现在的预设开不开。中途换预设、改了预设文件同一条规矩。
  - 新预设关了记忆：从下一个回合起工具面上没有记忆的三件，回合开始不问记忆（常驻的摘要、名片、联想都不再注入）。已经注入过的照旧留在历史里，逐字节回放。
  - 范围、回合索引、抽取不动，所以换回开着记忆的预设接着用，不用重接。
  - 代码：`miyu-policy` 的 `Chosen::keeping_memory` 改成「开会话时开了 且 预设开着」；中途换预设（`actor/persona/recast.rs`）照快照的范围算开会话时开没开，改了预设文件（`preset.rs`）照上一份快照里的预设记忆开没开（照范围算的话，无人格的会话范围是 `off`、预设里记忆却记着开，升级以后会白换一次快照，门禁里 `without_a_persona_a_changed_preset_still_swaps` 照出来了；代价是预设文件把记忆关了又打开的，要换一次预设才回来）；回合开始问记忆之前先看快照里的预设记没记着记忆没开（`actor/model.rs` 的 `remembering`）。
- 运行日志 `request` 那一行多一格 `tools`：这一次请求的工具名，照请求里的先后用逗号连起来；没有工具的不写这一格（`lines.rs` 的 `tool_names`）。

### 不做什么

- 不停回合索引和抽取：要求只是之后不再注入；停了索引，换回来时那几轮就缺了。
- 不把工具名记进会话日志的 `model.called`：会话日志是化石化的格式，多一格要改事件的写法；运行日志够核对用。
- 工具名每一次请求都记，不只在变了的时候记：只记变了的要在 actor 上多存一份状态，`actor.rs`、`tools.rs` 都在 500 行上限上。

### 验收

- 改之前是红的（`keeping_memory` 照旧只认开会话时的、回合开始照旧只看端口，跑同样的测试）：`crates/miyu-endpoint/tests/preset_switch_memory.rs` 的 `switching_to_a_preset_without_memory_stops_memory_from_the_next_turn` 失败；`miyu-policy` 的 `chosen_lists_installed_features_that_are_off_in_order`、`miyu-session` 的 `memory_follows_the_preset_only_if_the_session_was_made_with_it` 按新规矩改了期望，旧代码下也是红的。
- 直接用「无功能」开的会话（同一个文件的 `a_session_opened_with_no_features_has_no_memory_at_all`）改之前就是绿的，留着守住。
- `crates/miyu-session/tests/log.rs` 的会话带两件工具，`request` 那一行末尾是 `tools=read,shell`。
- 本机门禁 12 项全过，三平台 CI 全绿。

### 日志增长

出厂的工具一共 34 件，名字连起来 293 个字节；装了接入通讯平台这类扩展的再多几十件，一行多三五百字节。`request` 原来一行一百多字节，加上 `ended` 那一行，每一次请求的运行日志从约三百字节变成约六七百字节。
