## 施工 PL-1：换权限级别的事件带上换以前的级别

状态：已完成（2026-11-17）。项目主人 2026-11-17 定（界面会话转来）。

### 为什么

终端时间线左边的竖线照当时的权限级别上色。读老会话时，一页里第一次换级别以前的那几句没法知道当时是哪一级：终端只能先画暗色，读进更早一页再补。换级别的 `session.policy_changed` 多一格换以前的级别，头一打开就画得对。

### 改了什么

1. `PolicyChanged` 加最后一格 `from`（权限，`{level, read_only}` 两格都写）：内核切权限时写切以前那一刻人设的权限（`Session::permission`，`session.created` 的或者上一次切成的）。只换模型、思考强度、人格、预设、上下文上限、看图模型、压缩线、快照的不写；切成和现在一样的什么都不记，也就不写。
2. 会话造的时候就有权限（`session.created` 的 `permission` 必写），没有「之前是默认」的情况：头一次切的写造会话时的那一份，所以内核切的总带着 `from`，头拿到哪一条都画得对。
3. 账本多一条：`from` 只和 `permission` 一起出现（`from comes only with permission`），照 `replaced` 只和 `model` 一起的写法。
4. 以前的日志没有这一格，照没有读，写回去一个字节不变，重放、载入照旧（载入只看 `permission`）；头照以前的办法往前找。
5. 样本：`internal/designs/samples/events/session.policy_changed.jsonl` 的 52、63 号带上 `from`；探针存档 `probe/permission/log.jsonl`（5 条）、`probe/terminal/log.jsonl`（1 条）照 `MIYU_PROBE_WRITE=1` 重写，只有日志里换级别的那几行多了 `from`，请求一个字节没变。
6. 蓝图 `kernel/events-bodies.md`（`session.policy_changed` 表、测试表）、`kernel/session.md`「切权限级别」第 3 条和测试表、`kernel/history.md` 账本表和测试表、`protocol.md` 的 `session.set_permission_level` 第 4 条和测试表；协议参考中英两份 `events`（落盘的事件表）、`sessions`（`session.set_permission_level` 的回应）。

### 验收

1. 改前红的测试（先写、修之前跑过是红的）：
   - `miyu-kernel` `session::tests::permission::a_switch_records_the_permission_it_switched_from`：从造会话时的完全放开切只读、再切回来，两条的 `from` 各是上一份。
   - `miyu-kernel` `ledger::tests::model::from_comes_only_with_a_permission`：只有 `from` 的拒，两格都有的、只有 `permission` 的（以前的日志）收。
   - `miyu-endpoint` `permission::switching_over_the_protocol_records_it_and_pushes_it`：推送的 `body` 是 `{"permission", "from"}`。
2. 守着的：`event::session::tests::a_policy_change_records_the_permission_it_switched_from`（读写一字不差、以前的日志照读写回不变、`null` 当没有、少一格读不进来），`session::tests::configure::a_change_is_recorded_and_answered_once_stored` 加一句：只换模型的不写 `from`。
3. 门禁十一项全过；`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`（给模型的字不变）、`cargo xtask logs` 全过。
4. 实测（沙箱数据根起调试版核心，Python 直接连套接字）：
   - 造会话、订阅 `events`，切只读、切回开放、再切开放：推送两条，`body` 依次是 `{"permission":{"level":"full","read_only":true},"from":{"level":"full","read_only":false}}`、`{"permission":{"level":"full","read_only":false},"from":{"level":"full","read_only":true}}`；第三次一样的回 `{}`、不推不记。会话日志里同样两行，`from` 写在 `permission` 后面。
   - 老日志：停核心，把另一个会话日志里的 `from` 删掉（当成老日志），重起核心订阅 `after: 0`：两条照原样补回来、没有 `from`，订阅回应的 `permission` 是 `{"level":"full","read_only":false}`；接着切只读，新的一条 `from` 是 `{"level":"full","read_only":false}`，照老日志算回来的。

### 风险

- 名字 `from` 写在 `session.policy_changed` 的顶层，只和 `permission` 一起出现；以后别的格也要「换以前的」时不能共用它，得另起名字。
