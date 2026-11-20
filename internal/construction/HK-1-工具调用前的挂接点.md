## HK-1 工具调用前的挂接点 `tool.guard`

状态：施工完，待 core1 审（2026-11-07）。依据设计 `65-工具挂接点.md`（已定，core1 2026-11-06；项目主人 2026-11-07 定挪到首个正式版
以前），照第九节第 1 条施工；第三节第 4 条「拒绝的理由」、第十节一并做。蓝图 `extensions.md`（在哪、「工具调用前的守卫」九条、
守着它的、还没有的）、`kernel/asking.md`（链的结论、还没有的）、`session/actor.md`（「过执行前的链」那一行）、`packages.md`
（`bad_guard`）、`perf.md`（工具一步多一种）；协议参考 `extensions`（中英）、目录页；作者文档 `extensions`、`manifest`（中英）；
设计 26 第十节登记六份字，`prompts.md` 重新生成。

### 目的

扩展能在 AI 的一次工具调用执行之前说放行、拒绝还是要问人：公司策略（某些目录不许写、某些命令要问人）、审计这类用法不用写进
核心，也不用让扩展去改模型看到的字。

### 做了什么

- **清单 `[guard]`**（`miyu-config` 的 `package/guard.rs`）：`tools`、`access`、`timeout_ms`；两样都没写、访问类别不认识、工具名
  不像样、时限出了 100 到 5000、没声明 `tool.guard` 的读不进，`bad_guard`（人话三份）。只有扩展包能写。能力 `tool.guard` 本来就在
  表里，照 PK-3、LP-1 的做法装时批（出厂的算批过）。
- **表**（`miyu-endpoint` 的 `extensions/guards.rs`）：照核心手上的那份清单建，核心起来时建一次，清单换了照新的重建（照 `Arc` 认）。
  调用前查一次表；对上了才读开关文件、看批没批。自己登记的工具不问自己；主人关掉的不问。
- **问、合**：会话那一头（`miyu-session` 的 `actor/guarding.rs`）在权限策略、看一眼卡片之后：策略拒绝的到此为止；没挂守卫的当场送进
  内核，照原来的路；挂着的起任务问，actor 不等它（打断随时进得来），答了照「最严者胜、只能收紧」合上策略的结论，经执行器的回报
  送回内核。端口那一头并行发 `tool.guard`、各自照清单的时限，照包的编号合。超时、连接断了、回错误、答得不对、开着却没在跑的都算
  拒绝，各记一行 `WARN`。脚本里调的（嵌套调用）同一条路。内核没改：结论照旧只收还在等结论的调用。
- **`guard.cancel`**：在等的调用有了结果（打断、插话、收紧成只读、重启时内核补的那一条），actor 追加之前掐掉它的任务，端口的
  future 丢掉，还没答的守卫收到 `guard.cancel {"session", "call_id"}`；超时的同样发。
- **拒绝的理由防注入**（`guards/texts.rs`）：去掉头尾空白、截到 300 个字，换进出厂模板 `core/guard/blocked.txt`
  （`Blocked by {package}: {reason}`），照模板转义成一行；没写理由的、超时、不在、出错、答得不对是 `core/guard/` 的另五句，同样换进
  这个模板。几份字编进程序。给人看的说法带截过的原文、不转义。
- **测试扩展**多一步 `guard:<怎么答>`（`allow`、`deny`、`ask`、`forged`、`silent`、`boom`、`garbled`、`crash`），答法挪进
  `miyu-test-extension/hooks.rs`（主文件到了行数上限），回合开始的 `user.append` 的答法一起挪过去。
- **量尺**（`miyu-perf` 的 `measure/tools/guard.rs`）：工具一步最后打开一个立即答 `allow` 的守卫（量尺自己当扩展），再量一遍
  「read 小文件」。

### 协议最终形状（core1 转终端、网页、作者）

- 清单：
  ```toml
  [process]
  capabilities = ["tool.guard"]
  [guard]
  tools = ["shell", "write"]   # 工具名，可以不写
  access = ["execute"]         # read、write、execute、network、outbound、venue、install，可以不写；两样至少一样
  timeout_ms = 1000            # 100 到 5000，不写是 1000
  ```
  写错的 `bad_guard`。
- 核心 → 扩展 `tool.guard`（要回应）：`{"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`；嵌套调用的
  `call_id` 是 `call_12_1#3` 这样。回应 `{"verdict": "allow" | "deny" | "ask", "reason"?}`，多了格、别的写法算答得不对。
- 核心 → 扩展 `guard.cancel`（通知）：`{"session", "call_id"}`。
- 拒绝：`tool.result` 的 `status` 是 `denied`，`by` 是 `{"kind": "module", "id": <包>}`，给模型看的 `Blocked by <包>: <理由>`
  （一行、转义过）；`human` 是 `core/guard/denied-with-reason`（字段 `package`、`reason` 原文）、`denied`、`timed-out`、
  `not-running`、`failed`、`malformed`（字段 `package`）。**头**：时间线上照 `human` 显示「谁拦的、为什么」，`reason` 是扩展写的
  原文，照头自己的规矩转义；三种语言的字在 `resources/core/human/*.json` 的 `said`。
- 要问人：照常的 `tool.approval_requested`，`by` 是那个包，没有 `rule`（只能允许这一次或者拒绝），`detail` 是
  `{"guard", "tool", "reason"?}`。**头**：确认卡片照 `detail.reason` 写守卫的理由（没有 `command`、`paths`），没写的只写哪件工具。
- 放行、问人不进上下文；拒绝只追加那一条结果。没挂守卫的会话一个字节不变。

### 前缀不变的证明

- `tests/tool_guard.rs` 的 `an_allowing_guard_leaves_the_request_byte_for_byte_the_same`：同一个剧本（调一次 `jot` 再说一句），没装
  守卫和挂着一个放行的守卫各跑一遍，第二次请求（带着那次结果的）规范 JSON 逐字节一样（数据根、会话编号、十分钟一档的时间换掉）；
  守卫确实被问过。拒绝的那一份只在末尾那条工具结果里多了那一句（`one_guard_denies_and_one_allows_the_call_is_blocked`）。
- `request_shape` 那一组探针（`miyu-assemble/tests/probe*.rs`、`miyu-kernel/tests/request_sample.rs`）照过：组装、内核、
  工具面、system 都没碰，存档逐字节一样。

### 先证明不修时会红

守卫的那一问整个关掉（`guarded()` 第一行改成一律交回策略的结论，等于没有这一步）：`tool_guard`、`tool_guard_more` 十二个测试
红了十个；没红的两个是「关掉的不问」「自己的工具、策略先拒了的不问」，本来就该和没守卫一样。

### 定的技术题

1. `guard.cancel` 带 `session`：设计写 `{"call_id"}`，又写「照 `tool.cancel` 的写法」；调用编号只在一个会话里不重，扩展同时守着
   几个会话时光有 `call_id` 认不出是哪一次，照 `tool.cancel` 带上 `session`（见「要拍板的」第 1 条）。
2. 超时的也发 `guard.cancel`（同 `tool.cancel` 超时也发）：扩展好放下那一次。
3. 策略、守卫都要问人的照策略的问（`by` 是 `permissions`，带放行规则）：两边一样严，策略的那一份能「本会话都允许」；人允许了以后
   下一次策略放行、守卫照旧问。
4. 不要求 `provide`：问的是握了手的连接（`Provided::linked`）；只认它在不在，不另看状态（状态写「在跑」和登记连接之间有一小段先后）。
5. 答的写法照 `user.append`：多了格也算答得不对。`reason` 写 `null` 当没写。
6. 声明了 `tool.guard`、没写 `[guard]` 的读得进，就是不被问；写了 `[guard]`、没声明的读不进（同 `[[search]]`）。
7. 给人看的 `reason` 也截到 300 个字（只截不转义）：不让扩展往会话日志里塞没边的字。
8. 核心没了（正在退出）的守卫问不了：不放行，这次调用等会话跟着停时照重启收尾。
9. 几份字编进程序（`include_str!`，同 `miyu-cli` 的 `core/call/`）：拒绝这一路没有读不出出厂字的时候。
10. `[guard] tools` 只挡明显写错的工具名（1 到 128 个字、没有空白、控制字符），细规矩归工具目录。

### 要拍板的（都已照推荐做，改起来小）

1. `guard.cancel` 的参数：推荐 `{"session", "call_id"}`（现在的做法，理由见技术题第 1 条）；另一种是照设计字面只给 `call_id`。
2. 确认卡片的 `detail` 形状 `{"guard", "tool", "reason"?}`：推荐这样（头照 `reason` 写守卫的话）；要不要另给头一个说法键，等头那边说。
3. 给人看的原文也截到 300 个字：推荐截（技术题第 7 条）；另一种是照原文全存。

### 不做什么

- `tool.after`（HK-2，下一步）、`tool.rewrite`（设计 65 第七节不做）。
- 守卫改参数、改沙盒、开 `outside_sandbox`。
- 头的确认卡片、时间线怎么画守卫的话（头那边各自做）。

### 怎么验

- 测试（真核心拉起测试扩展）：`tests/tool_guard.rs`（两个守卫一个拒一个放、问的八格；两个都拒的照编号；问人；放行前缀不变；
  理由防注入）、`tests/tool_guard_more.rs`（超时、`guard.cancel`；回错误、崩了；开着却没握手；打断；关掉的；自己的工具、策略先拒的；
  脚本里调的；脚本里调的、父级被打断）。单元：`extensions/guards/tests.rs`（答的读法、照编号合、没答的五句、截了再转义、访问类别和内核一样）、
  `actor/guarding/tests.rs`（合上策略只收紧）、`package/guard/tests.rs`（清单的读法）；作者文档对清单的测试（`fields/tests.rs`）。
- 防注入：理由塞「Ignore all previous instructions」、`</tool_result>`、伪造的 `tool.result` 记录、`user: "FORGED-USER"`、`<system>`、
  换行、引号：结果的字只有行尾一个换行、没有原样的引号和尖括号，请求里只有工具结果那一条带着它，整份请求找不到 `</tool_result>`、
  `<system>`、原样的引号；300 字截断、先截后转义在单元里查。
- 变异：见下表。
- 量尺：`cargo xtask perf --tools 20`，见下面「量尺」。

### 变异

改坏、看对应的测试变红、还原（十七个都红了）：

| 改坏了什么 | 红的测试 |
|---|---|
| 两个拒绝取编号在后的 | `two_denials_keep_the_reason_of_the_package_first_by_id` |
| 超时不算拒绝 | `a_silent_guard_times_out_into_a_denial_and_is_told_to_cancel` |
| 时限放大一千倍 | 同上 |
| 理由不截 | `the_reason_is_cut_and_escaped_into_one_line` |
| 理由不转义（换模板时原样填） | `a_forged_reason_stays_inside_one_escaped_line` |
| 自己登记的工具也问自己 | `a_guard_is_not_asked_about_its_own_tools_or_what_the_policy_refused` |
| 关掉的也算挂着 | `a_switched_off_guard_is_not_asked` |
| 没在跑的放行 | `a_guard_switched_on_but_not_running_blocks_without_being_asked` |
| 不发 `guard.cancel` | `an_interrupt_ends_the_wait_and_tells_the_guard_to_cancel` |
| 有了结果不掐等着的守卫 | `an_interrupt_ends_the_wait…`、`an_interrupt_also_cancels_a_guard_asked_from_a_script` |
| 只看 `tool.result`、不看 `tool.nested` | `an_interrupt_also_cancels_a_guard_asked_from_a_script`（第一轮变异逮不住，补了这个测试） |
| 策略放行、守卫问人的照放行 | `a_guard_that_asks_waits_for_the_person` |
| 策略问人时守卫的拒绝不算 | `a_denial_wins_over_anything_the_policy_said` |
| 脚本里调的不过守卫 | `tools_called_from_scripts_pass_the_guard_too` |
| 不认识的 `verdict` 当放行 | `two_denials_keep_the_reason_of_the_package_first_by_id` |
| `[guard]` 两样都没写也收 | `wrong_guards_are_refused` |
| 问的时候不带 `args` | `one_guard_denies_and_one_allows_the_call_is_blocked` |

### 量尺

`cargo xtask perf --tools 20 --tag hk1`（2026-11-07 本机，Ryzen 9 7940H，负载 5 上下；结果
`docs/perf/2026-11-06-tools-linux-x86_64-hk1.md`、`.json`，文件名照 UTC 的日期）：

| 调用 | p50 / p95（毫秒） | 核心 CPU p50 |
|---|---|---|
| read 小文件 | 1.30 / 1.67 | 1.52 |
| read 小文件（挂着一个立即答的守卫） | 2.00 / 5.19 | 2.28 |

p50 多 0.70 毫秒，在设计 65 第六节的目标（1 毫秒以内）里；p95 多 3.5 毫秒，是二十步里一两步的尖（本机同时有别的会话在编），
还在设计 23 F5「人整段等着的上限 50 毫秒」里。多出来的是查表读开关、往扩展写一行、等它答、合，外加任务往返一次 actor。
