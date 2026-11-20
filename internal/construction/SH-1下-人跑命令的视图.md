## 施工单 SH-1 下：人跑的命令在视图流和参考文档里

状态：已完成（2026-11-04）。照设计 57 第四节第 4 条、第五节第 2 条；核心那一半是 SH-1 上。

### 目的

终端要画得出人用 `!` 打的命令（网页不做 `!`，设计 57 第六节第 4 条，2026-11-04 改；视图流的格照旧给所有头）：那条人话下面挂它的输出、退出码，还没跑完的画「在跑」。头照视图流画，不读 `shell.ran` 事件。

### 改了什么

- 视图流 `user` 多一格 `shell`（`miyu-view` 的 `entry/shell.rs`）：写了 `shell` 的 `message.user` 先是 `{"running": true}`；`shell.ran` 到了换成 `exit` 或 `signal`、`text`、`omitted`、`duration_ms`，异常结束的另有 `reason`。对不上一条画着的话的不理。排着的照 `queued` 分，退回的照 `withdrawn`，都照普通消息。
- 蓝图 `view.md`（`user` 那一行、守着它的）。
- 参考文档中英两份：`sessions.md` 的 `session.send`（`shell` 参数、第 4 条、两个原因码）、`errors.md`、`events.md`（`message.user` 的 `shell`、`shell.ran`）、`view.md`（`user` 的 `shell`）。`methods-v1.txt` 不变（还是 `session.send`）。

### 不做什么

- 不改核心的走法、事件（SH-1 上）；头怎么画归终端；协议、视图流不分头，不跟着收窄。

### 验收

1. `miyu-view` 三条：跑完的带结果、在跑的是 `running` 后换成结果、普通的话没有这一格；视图测试全过。
2. 手写变异 4 个都逮住（没跑完不画在跑、跑完不挂上、退出码丢、原因丢）。
3. 门禁 11 项、Windows clippy、分支 CI 六项。
4. 合完告诉终端形状（网页不做 `!`，不另发）。
