## 施工单 7-4 修：`job.stop` 在后台答

状态：主会话做（网页 2026-10-26 报）。

### 为什么

网页「全部停止」改成一下子发上百个 `job.stop`（一个会话约 200 个后台任务：子代理加孙代理）以后，大半不回、没回的也真没停：发 103 个回 1 个，再发 99 个回 33 个。不是死锁：同一条连接上的请求一条条办，`job.stop` 要等子代理连它派的停下、回报落了盘才回，一个一两秒，上百个排成一队，网页等 15 秒只看得到前几个，后面的还没轮到。

### 做了什么

- `job.stop` 加进在后台答的那几样（`methods/later.rs` 的 `answered_later`、`call_later`），同 `model.call`、`job.output {wait}`：交给这个连接的后台任务，接着读下一行；回应照 `id` 对上，照办完那一刻的订阅走「先见结果，后见回应」（`subscriptions/later.rs`），回报的推送照旧排在回应前面。`methods.rs` 里按顺序答的那一条拿掉（走不到了）。
- 图纸 `protocol.md`「一个连接」第 1 条，协议参考 `connection` 中英。

### 不做的

- 整会话停全部的方法（`job.stop {all: true}`）：后台答以后网页照发几百个也不排队，先不加；真要再加，只加不改。

### 验收

1. `methods/later/tests.rs`：`job.stop` 在后台答，`session.list` 照旧按顺序。变异「拿掉 job.stop」红。
2. `tests/job_stop.rs` 新一条：同一条连接上不等回应连着发 6 个 `job.stop`，6 个都回 `{}`，父会话记了 6 条 `stopped` 回报。原来两条照过。
3. `cargo xtask check` 全过。
