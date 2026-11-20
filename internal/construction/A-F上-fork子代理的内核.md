## 施工单 A-F 上：fork 子代理的内核

状态：待核心主会话审。经过：项目主人要子代理能「从当前上下文分出去」；核心主会话 2026-10-24 排成 A-F，定了六点（分叉点从派它的那一步、快照全照父会话、算子代理、不封存、记忆照子代理、fork 出来的不能再 fork），拆上、中、下三步。图纸 `session/fork.md`「fork 子代理」、`kernel/events-bodies.md`（`forked.within`、结束原因 `forked`）。

### 为什么

C-8 的分叉点在一轮之外：父会话在跑一轮的，退到这一轮的 `turn.started` 之前，内核也明确拒绝底子停在一轮当中。fork 子代理是她这一轮当中、调 `subagent` 那一步派出去的，照这个分叉点，这一轮她读过、查到的子代理全看不到，交代里写「刚才那个文件」就对不上。照 Claude Code 的 fork：从派它的那一步分叉，那一步的调用都给一句一样的占位结果，再接交代。

### 做了什么

- `forked` 多一格 `within`：分叉点在哪一轮当中。普通分叉不写，字节不变。
- 结束原因多一种 `forked`：fork 子代理收住接过来的那一轮。组装不说什么（不是没走完），向上回报不看它（那时还不欠回报）。
- `fork_plan_at(父会话, 日志, 调用)`：那次调用得是正在进行的那一轮里还等结果的；分叉点是派它的那条 `message.assistant`（账本记着的最近一条回复：等结果的调用都在它里面）；权限、模型、快照照到那一条为止的。`fork_plan`（C-8）不变。
- 底子（`Base::of`）：写了 `within` 的，要停在那一轮里、一步刚派出去的调用上；没写的照旧拒停在一轮当中。
- 账本 `continuing`：子会话接着那一轮，开着的回合、等结果的调用、最近一条回复照父会话的；那一轮不进能撤的几轮（父会话开的）。造和载入走同一个（`continued`）。
- `Session::fork_within`：同 `Session::fork`，多一句占位结果。头几条同一批：`session.created`；那一步每个调用一条占位结果（照调用编号的先后，编号就是那一步派出的先后，几个兄弟因此一致；父会话里后来先回来了结果的也记占位，前缀才一样；`ok`，`by` 内核，`turn` 是父会话那一轮，给人看的 `core/tool-results/forked`）；`turn.ended`（`forked`）。`Session::fork` 遇到 `within` 拒，`fork_within` 没写 `within` 拒。
- 占位那一句 `resources/core/tool-results/forked.txt`（`Forked; the parent handles this call.`，11 个 token），进登记簿；给人看的三种语言：已分叉、Forked、フォークしました。字由执行器读了交进来（施工 A-F 中），不进快照：子会话的快照要和父会话的同一个哈希。
- 文档：`session/fork.md`、`kernel/events-bodies.md`、`kernel/tools.md`、`26-提示词.md`、`prompts.md`。

不在这一步：`subagent` 的 `fork` 参数、交代前面的外壳、递归闸、执行器读占位的字（A-F 中）；端点、真模型量两轮请求的缓存（A-F 下）。

### 验收

1. `crates/miyu-kernel/src/session/tests/fork_within.rs` 六条：
   - 计划：分叉点是派它的那条回复、写那一轮；普通分叉照旧退到这一轮之前；已经有结果的调用、闲着的会话不认。
   - 头几条：接着分叉点往下数，两条占位结果（照调用的先后、字、`ok`、内核、带那一轮、命令编号），`turn.ended`（`forked`）。
   - 接着说：交代开的那一轮里，底子那一段是父会话到派它的那一步为止的，后面是占位结果、收住、交代；同一步的兄弟头几条一字不差、请求一样。
   - 载入的和活着的一样，载入时什么都不补。
   - 拒：普通分叉停在一轮当中、`within` 走错入口、没写 `within`（一轮当中、一轮之外的都拒）、不是那一轮、没停在一步刚派出去的调用上。
   - 子会话自己的回复看到的在派它的那条回复之前的，载入拒（账本接着父会话的最近一条回复查）。
2. `crates/miyu-assemble/src/texts/tests.rs` 的 `a_turn_settled_by_a_fork_says_nothing`：`forked` 同走完了的一样什么都不说（`render/tests.rs` 到了 500 行，不往里加）。
3. C-8 的分叉测试照旧过。
4. 变异 13 个都报红。头一遍活下来两个，是测试没盯着：账本接着父会话的最近一条回复、`fork_within` 用在一轮之外的分叉点上；补了上面两句，原样测试先过，再跑都红。
5. 本机门禁、Windows 版 clippy；碰了内核，推 `step/` 过三台 CI。
