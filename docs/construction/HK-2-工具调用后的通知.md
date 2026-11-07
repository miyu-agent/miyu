## HK-2 工具调用后的通知 `tool.after`

状态：施工完，待 core1 审（2026-11-07）。依据设计 `65-工具挂接点.md`（已定），照第九节第 2 条施工；施工前问的三条（场所会话、脚本里调的、
断开时）core1 2026-11-07 照推荐定，连同 HK-1 那三条写进设计 65 第三节、第四节、第十节第 4、5 条。蓝图 `extensions.md`（在哪、
「工具调用后的通知」七条、守着它的、还没有的）、`session/actor.md`（第 5 条第 13 款）、`packages.md`（`bad_after`）、`perf.md`（工具
一步多一种）；协议参考 `extensions`（中英）、目录页、`methods-v1.txt`；作者文档 `extensions`、`manifest`（中英）。

### 目的

扩展在 AI 的一次工具调用有了结果、记进会话以后收到一条只读的通知：审计（每次调用记到外面的日志里）、调用统计这类用法，不用为了
看工具调用去订整条事件流，也不碰模型看到的字。

### 做了什么

- **清单 `[after]`**（`miyu-config` 的 `package/after.rs`）：`tools`、`access`，读法、报错同 `[guard]`（`guard.rs` 的读法抽成
  `covering` 共用）；两样都没写、访问类别不认识、工具名不像样、没声明 `events.read` 的读不进，`bad_after`（人话三份）；没有时限，
  写了 `timeout_ms` 的是 `unknown_key`。只有扩展包能写。能力用 `events.read`，不另加（设计 65 第十节）。
- **表**（`miyu-endpoint` 的 `extensions/afters.rs`）：照核心手上的清单建，和守卫的表共用 `extensions/table.rs`（从 `guards.rs`
  抽出来），核心起来时建一次，清单换了重建。会话那一头查表只拿一次锁、不读盘（`HooksPort::afters`）。
- **写好**（`miyu-session` 的 `actor/after.rs`）：追加一批之前，她的回复里表里有收件人的调用记下工具名、参数原文（`tool_call` 块）；
  这一批里那几次的 `tool.result` 写成一条，`tool.nested`（脚本里调的）照它自己带的写。写进去了才交给端口（`store.rs`），写不进去
  的不发。
- **发给谁**（`afters::send`）：表对上了的，再读开关文件看开着、批了 `events.read`，看碰不碰得到这个会话：`sessions::reaches`，
  从 `reachable` 抽出来按包判，和订事件流是同一道（不是平台桥的只收本机的会话、主人本人没关起来的私聊；平台桥只收自己平台的场所
  会话）。会话那一头交来场所编号和关没关着（`Tools::venue`、`Guard::confined`），端口不读盘。握过手的连接在才发。
- **队列**：每个扩展一条 256 条的队列和一个发送任务。会话那一头只 `try_send`：满了丢新的，一阵连着丢的头一条记 `WARN`（带累计
  丢了几条），有了空位记一行 `INFO`。发送任务往连接的写队列送、等得起空位（`Peer::notify_waiting`）。连接断了、崩了，队列和任务
  一起丢掉，不补发；连接换了的新开一条空的；没在跑的丢掉它的队列。
- **量尺**（`miyu-perf` 的 `measure/tools/after.rs`）：守卫那一种前面多一种「read 小文件（挂着一个慢的 tool.after 收件人）」：
  量尺自己当收件人（参数 `after-extension`，握手以后每读一行睡 200 毫秒），量完关掉再量守卫。

### 协议最终形状（core1 转终端、网页、作者）

- 清单：
  ```toml
  [process]
  capabilities = ["events.read"]
  [after]
  tools = ["shell", "write"]   # 工具名，可以不写
  access = ["network"]         # 同 [guard] 的七个访问类别，可以不写；两样至少一样；没有 timeout_ms
  ```
  写错的 `bad_after`。
- 核心 → 扩展 `tool.after`（通知，不带 `id`，不要回应）：
  - 直接调的：`{"session", "call_id", "tool", "access"?, "status", "duration_ms"?, "by", "args", "blocks"}`。`call_id` 是
    `call_12_1`；`status`、`duration_ms`、`by`、`blocks` 同那条 `tool.result`（`by` 照事件流的写法，被拒的是拒它的一方）；`args`
    是模型写的参数原文，一段字（同事件流里 `tool_call` 块的 `args`），不是对象。
  - 脚本里调的：`{"session", "call_id", "tool", "access"?, "status", "duration_ms"?, "by", "args", "result"?, "bytes"?}`。
    `call_id` 是 `call_12_1#3`；`status`（`ok`、`error`、`denied`、`needs_approval`、`cancelled`）、`args`（修正过的原文）、
    `result`（blob 的哈希）、`bytes` 同那条 `tool.nested`；不带 `blocks`。
  - 目录里没有的工具不写 `access`（只照 `tools` 认）；没真执行过的不写 `duration_ms`。
- 谁收：声明并批了 `events.read`、写了 `[after]`、开着、握过手、碰得到这个会话的扩展（同订事件流）。自己登记的工具也发。
- 不保证送达：每个扩展至多攒 256 条，满了丢新的；断了、崩了丢队列，不补发；要完整的审计订事件流。

### 前缀不变的证明

- `tests/tool_after.rs` 的 `listening_leaves_the_request_byte_for_byte_the_same`：同一个剧本（调一次 `jot` 再说一句），没装和挂着
  一个收 `jot` 的扩展各跑一遍，第二次请求（带着那次结果的）规范 JSON 逐字节一样（借 HK-1 的 `second_request`，改成可以指定看哪个
  方法，并等到扩展收到再停）；扩展确实收到过一条。
- 组装、内核、工具面、system 都没碰（改的是会话追加以后、端点、清单），`request_shape` 探针那一组不受影响。

### 先证明不修时会红

会话那一头写好的通知不交给端口（`actor/after.rs` 的 `send_afters` 改成什么都不做，等于没有这一步）：`tool_after`、
`tool_after_more` 七个真核心测试全红（收到、照访问类别、慢的崩的、没批关掉的（照收的那一个也收不到）、群会话、请求逐字节不变
（收到过一条）、脚本里调的）。端点那一头的单元测试不经会话，不在里面。

### 定的技术题

1. 直接调的参数原文从她的回复（`message.assistant` 的 `tool_call` 块）里记：`tool.result` 里没有工具名、参数；只在表里有收件人的调用
   才记，结果落盘时拿掉。载入以前派的调用（重启时内核补的结果）没记，不发（不保证送达，完整的订事件流）。
2. 会话那一头交场所编号和关没关着，端口照它判（`reaches`），不再为每一条读会话日志的第一条。
3. 开关、批没批在表对上以后才读开关文件（同 HK-1）；没有收件人的会话每次调用只查一次内存里的表。
4. 队列满了的 `WARN` 一阵只记头一条、带累计数，恢复记 `INFO`：设计写「记一行 `WARN`」，满的时候每条都记会刷屏。累计数只在内存、
   只进日志，不加协议格。
5. 发送任务等写队列的空位（`notify_waiting`），不用 `notify` 的 `try_send`：写队列一满就丢，256 条的队列就形同虚设。
6. 自己登记的工具也发给它（守卫不问自己是因为自己判自己没意义，通知没有这层）。
7. `[after]` 不要求 `provide`，同守卫。
8. `[after]` 没有 `timeout_ms`：通知不等回答。

### 不做什么

- `tool.rewrite`（设计 65 第七节不做）。
- 丢了几条不进协议（`extension.status` 不加格）。
- 重启以后补发、按扩展持久化队列。

### 怎么验

1. 不修时会红：会话那一头写好的通知不交给端口（`send_afters` 改成什么都不做，等于没有这一步）。
2. 测试（真核心拉起测试扩展）：`tests/tool_after.rs`（收到、九格对得上日志、`args` 原文、表里没有的不发；照访问类别、守卫拒的
   `by`；一个不读、一个握了手就崩，回合照常、照收的那一个四十条照落盘的先后；没批 `events.read`、关掉的收不到；群会话只给它平台的
   桥、本机的只给别的扩展；请求逐字节不变）、`tests/tool_after_more.rs`（脚本里调的：嵌套调用的编号、`result`、`bytes`、不带
   `blocks`；脚本工具本身不在表里不发）。单元：`extensions/afters/tests.rs`（两种参数的形状；队列满了丢新的、照扩展累计、读走的照
   先后、有了空位接着收；没在跑的丢队列，重新握手、连接换了的新开空队列）、`package/after/tests.rs`（清单的读法、报错）。
3. 变异：见下表。
4. 量尺：`cargo xtask perf --tools 20`，见下面「量尺」。

### 变异

改坏、看对应的测试变红、还原（十五个：十四个红了，一个逮不住）：

| 改坏了什么 | 红的测试 |
|---|---|
| 满了不累计 | `a_full_queue_drops_new_ones_and_counts_them`、`a_stopped_extension_loses_its_queue_and_a_restart_starts_empty` |
| 队列放到 512 条 | 同上 |
| 不看碰不碰得到会话 | `a_venue_session_is_told_only_to_its_bridge` |
| 不看开关、批没批 | `listeners_without_events_read_or_switched_off_hear_nothing`（第一轮逮不住：开关一开始就关着的根本没拉起；补成跑着的扩展在开关文件里关掉） |
| 没在跑的不丢队列 | `a_stopped_extension_loses_its_queue_and_a_restart_starts_empty` |
| 连接换了照旧用老队列 | 同上 |
| 直接调的不带 `blocks` | `a_direct_call_carries_blocks_and_args_as_written`、`a_notice_follows_the_written_result_with_its_fields`、`access_picks_calls_and_a_blocked_call_is_told_who_blocked_it` |
| 脚本里调的不发 | `tools_called_from_scripts_are_told_too` |
| `args` 不是模型写的原文 | `a_notice_follows_the_written_result_with_its_fields`、`a_venue_session_is_told_only_to_its_bridge` |
| `by` 写成内核 | `a_notice_follows_the_written_result_with_its_fields`、`access_picks_calls_and_a_blocked_call_is_told_who_blocked_it` |
| 记调用时不看访问类别 | `access_picks_calls_and_a_blocked_call_is_told_who_blocked_it` |
| 直接调的不写访问类别 | `a_notice_follows_the_written_result_with_its_fields`、`access_picks_calls_and_a_blocked_call_is_told_who_blocked_it` |
| 没声明 `events.read` 也收 | `wrong_afters_are_refused` |
| 不认工具名 | `after_reads_tools_and_access` |
| 落盘以前就交给端口（写不进去也发） | 逮不住：写盘在阻塞线程里一下就完，测试看不出先后；要造写不进去的存储才看得见，代价大。照结构保证：交给端口在 `append` 写成功的那一支里 |

### 量尺

`cargo xtask perf --tools 20 --tag hk2`（2026-11-07 本机，Ryzen 9 7940H，负载 25 上下：同时有别的会话在编；结果
`docs/perf/2026-11-06-tools-linux-x86_64-hk2.md`、`.json`，文件名照 UTC 的日期）：

| 调用 | p50 / p95（毫秒） | 核心 CPU p50 |
|---|---|---|
| read 小文件 | 3.66 / 6.40 | 3.01 |
| read 小文件（挂着一个慢的 tool.after 收件人） | 2.57 / 3.43 | 2.49 |
| read 小文件（挂着一个立即答的守卫） | 2.77 / 3.49 | 2.68 |

挂着一个每读一行睡 200 毫秒的收件人，一步的时长没有变长（比头一种还短：头一种量在负载最高的那一阵，三种的差在噪声里），
设计 65 第六节「慢的收件人不拖回合」成立。拆段看，结果落盘到下一个请求头（`tool.result` → `request`）0.55 毫秒，和守卫那一种的
0.45 毫秒一个量级：会话那一头多的只是查表、写好一条、入队。负载高，绝对数比 HK-1 那次（1.30 毫秒）大，比的是同一次里的几行。

