## 施工单 CD-1：等人确认有倒计时，到时自动拒

状态：做完，等合。

### 目的

高危命令、装包这类要人确认的调用会一直等人。自己跑的时候（长期目标、定时任务、人走开了）一个确认弹出来没人管，她就卡在那里。项目主人 2026-11-12 定（todolist 第 49 条）：

1. 所有「允许/拒绝」的确认都倒计时：权限策略问的（高危命令、装包这些）、扩展守卫问的，脚本里调的也算。她的提问（`question.asked`、`ask_user`）不算，照旧一直等。
2. 出厂 5 分钟，到时按拒绝。拒绝要让她知道是没人确认、不是人拒的，好换别的办法往下走。
3. 只在系统设置里改：一项系统配置。
4. 头要画倒计时：确认里要给出到什么时候自动拒。

### 蓝图改哪几节

- `kernel/asking.md`：「是什么」「在哪」；「对外的样子」的动作、等多久和到时了的两种输入、事件表（`expires_at`、内核替人拒的决定）；「执行前的链」第 5 条之 3、「人确认」第 7 条；新加一节「到时没人确认」；「在等的怎么了结」多一行；给模型看的、给人看的表各多 `approval-timed-out`；「守着它的」「出处」。
- `session/guard.md`：「在哪」`ConfirmSettings` 那一行，问人那张表「要问人，别的」一行。
- `view.md`：`tool` 条目的 `approval` 多 `expires_at`；`waited_ms` 那一格写明到时被拒也算等完。
- `protocol.md`：`session.answer` 第 2 条，到时被拒以后再答是 `not_asking`。
- `kernel/events-bodies.md`：`tool.approval_requested` 多 `expires_at`，`tool.approval_decided` 写明内核替人拒的样子；样本 `tool.approval_requested.jsonl` 带上 `expires_at`。
- `config.md`：键表、键的说明、给人看的字多 `permission.confirm.timeout`；嵌着的参考文件样本（生成的）。
- 参考文档中英：`events`、`view`、`sessions`（`session.answer` 的开头和 `not_asking`）。

### 定了的

**配置**：`permission.confirm.timeout`，秒，整数 0 到 86400，出厂 300，`0` 是一直等；只有系统一层，`next_turn`；在 `ConfirmSettings` 声明（和受保护的命令同一个前缀），设置页「隐私与安全」那一页「权限」一组的数字框（那一组别的项不显示）。

| | 中文 | 英文 | 日文 |
|---|---|---|---|
| 名字 | 确认超时 | Confirmation timeout | 確認のタイムアウト |
| 说明 | 等你确认超过这么多秒就自动拒绝。0 为一直等。 | Anything waiting for your confirmation is denied after this many seconds. 0 waits forever. | 確認待ちがこの秒数を過ぎると自動で拒否します。0 はずっと待ちます。 |

**计时在哪起**：内核不读时钟（`kernel/session.md`）。

1. 执行器回合开始冻结这一轮的配置时读它，换成毫秒，跟着 `TurnStartHooksDone` 交给内核（新格 `confirm_ms`，没有的一直等），内核记在这一轮上。所以生效时机是下一轮。
2. 内核记 `tool.approval_requested` 时照那一条的时刻加等多久算截止的时刻，写进请求的 `expires_at`，接着出新动作 `ExpireApproval { at, call_id, nested }`。脚本里调的同样（`nested` 是序号）。没人能确认、只读拦下的照旧当场拒，不记请求、不计时。
3. 执行器照它起一个定时的任务（和到点叫醒 `Wake` 同一处，`actor/back.rs`），到时交回新输入 `ApprovalExpired { at, call_id, nested, text, human }`。不收回：人先定了、请求作废了的照样交回，内核不理。重启不接着计：在等的确认照原来的规矩了结。

**到时内核怎么记**：这件还在等确认的，记一条 `tool.approval_decided`（`deny`，`by` 是内核，`cause` 是回合的，没有 `reason`），直接调用的再记一条 `denied` 的结果，`by` 是内核，这一步因此齐了的往下走；脚本里调的那件了结成 `denied` 交给脚本，排在后面的接着过链。不在等的（定了、有了结果、作废、打断、收紧、重启、对不上的）不理。

为什么也记一条决定：脚本里调的请求只有决定能在账本里了结（父级还在跑，没有结果来了结它），不记的话同一个父级下面的下一次请求会被账本当成「已经有一个在等」拒掉；直接调用的照同一个样子记，头看 `approval.decision` 就知道定了、看 `by` 知道是内核替人拒的。

**给她看的那一句**：`Not run: no one answered the approval request in time.`（`resources/core/tool-results/approval-timed-out.txt`，登记簿第十节）。说法 `core/tool-results/approval-timed-out`：「无人确认，已自动拒绝」「No one confirmed in time, denied automatically」「確認がなかったため、自動で拒否しました」。这一句不进策略快照：执行器编进程序（同扩展守卫的 `core/guard/*.txt`），随 `ApprovalExpired` 交给内核。进了快照，以前造的会话的快照缺这一格就载入不了；不写旧版兼容，所以不放进去。

**头画倒计时**：照请求的 `expires_at`。事件流是 `tool.approval_requested` 的 `expires_at`，视图流是那一步 `tool` 条目的 `approval.expires_at`（照请求原样，定了以后照旧留着）。`view.status` 不加格。

**用时**：到时被拒也算等完了，`waited_ms`、`took_ms` 照旧刨掉等人的那一段（`miyu_kernel::waits` 认决定）。

### 不做什么

- 不给提问计时。
- 不跨重启接着倒计时。
- 不在会话、预设、协议上另给等多久：只有系统配置一项。
- 头怎么画倒计时归 UI 的会话。
- 施工图不画。

### 验收

1. `cargo test -p miyu-kernel --lib approval::expiring`：请求带截止的时刻（记请求的时刻加 300 秒）、出 `ExpireApproval`；没配的不带、不出；到时记内核替人拒的决定和 `denied` 的结果、`cause` 是回合的、说法对、她接着请求下一次、之后的回答 `not_asking`；允许、拒绝以后到时不理；来了一句话作废了的到时不理；对不上的不理。
2. `cargo test -p miyu-kernel --lib scenario::approval_expiry`：执行器替身记下截止时刻，直接调用的到时拒、她看到那一句说下一句、这一轮结束；人先答的照常跑、到时不理；脚本里调的到时拒、交给脚本、排在后面的那一件接着过链放行。
3. `cargo test -p miyu-kernel random:: -- --include-ignored`：三个种子里一个配一分钟，随机送到时了（在等的、定过的、对不上的），看守查还在等的才记内核替人拒的决定、`by` 写对；八百例里走到「交出了截止时刻」「到时没人确认被拒」，喂过「到时了」这一种输入。
4. `cargo test -p miyu-session --lib settings`：出厂 300、0 到 86400、只在系统层、下一轮生效、「权限」组的数字框；交给内核的毫秒数，0 没有。
5. `cargo test -p miyu-session --test all confirm_timeout`：真的 actor 配 1 秒，不理，一秒多以后 `denied`、决定和结果的 `by` 都是内核、说法对、工具没跑、她拿到那一句又问了一次；到时前允许的照常跑，过了截止的时刻不多记；配 0 不带截止、1.5 秒后照旧在等；没配的截止是 5 分钟。
6. `cargo test -p miyu-view`、`miyu-endpoint`、`miyu-core`、`miyu`：视图条目带 `expires_at`；`config.schema`、`config get` 多这一项。
7. `cargo xtask prompts`：只多 `approval-timed-out` 一句，登记过。

### 风险

- 定时的任务不收回：人先答了的，任务照样睡到截止的时刻再交一封，内核不理。出厂 5 分钟，一个会话同时在等的确认很少，多睡几个任务不要紧。
- 只读、没人能确认的照旧当场拒，不受影响。改了配置下一轮才生效，这一轮已经在等的照记下的截止的时刻。
