## 施工单 BUG-1：造会话的去重认属主，停核心不再误报 ERROR

状态：核心的分会话做。2026-10-26 miyu-call 用真核心（main 8ad41fd04）撞见：对同一个工作目录第二次 `session.create` 交回上一次的会话，接着 `session.send` 有回应、会话里却没有这句话，等不到 `turn.ended`；停核心时日志末尾有一行 `ERROR heap settle panicked panic=false`。主会话转来查，根因报过、主会话同意修法。

### 查下来的

1. 不是丢消息：复现脚本每次连接都用写死的请求 `id`（`"c"` 造会话、`"m"` 说话）。核心按 `id` 认断线重发是有意的：会话表同一个命令编号再造交回上次那个（施工 4-9 再补三上），内核同一个会话同一个命令编号照上一次回应答、不再产生事件（设计 02 不变量 9）。第二次连上用的还是 `"c"`、`"m"`，就拿回旧会话、旧回应。命令行（`miyu-cli/src/rpc.rs`）每次连接带随机前缀，不受影响。协议参考没写清「去重不分连接」。
2. 顺带查出的真 bug：会话表找「造过的」只比命令编号，不看属主。别的账号碰巧用了同一个编号，拿到这个账号的会话编号（编号透出去），自己的会话也造不出来。核心重启后补回编号只读机主的会话目录，成员、系统账号（平台桥）的会话重启以后认不出重发。
3. `heap settle panicked panic=false` 是误报：停核心时运行时在收尾，还内存的阻塞任务被取消（`JoinError`，不是 panic），代码不分取消和崩溃一律打 `ERROR`。

### 做了什么

- `miyu-endpoint/src/sessions.rs`：去重的键改成（属主, 命令编号）；`recall` 读每个账号的会话，合起来取最新的 1024 个，属主照会话所在的账号。
- `miyu-core/src/settle.rs`：还了一次以后怎么收抽成 `settled`：取消的记 `DEBUG heap settle cancelled`、不再盯，真 panic 才 `ERROR`。
- 协议参考 `connection.md`（中英）「先后」第 3 条写清：去重不分连接，每个连接的 `id` 要和以前的都不一样（随机前缀）；`session.create` 也照 `id` 认重发，同一个账号重启以后也认得，不同账号各造各的。
- 图纸 `protocol.md`「造会话」第 4 条和测试表、`heap.md`「还内存」。测试支持多 `member_with_token`（重启以后照令牌再登录）。

### 不做的

- 记录里「连跑五次的第一次在 `session.create` 之前就卡住」：没有回应原文推不出来；miyu-call 的脚本换成随机前缀以后还卡的，再打出原文查（主会话定）。

### 验收

1. `restart.rs`：两个账号用同一个编号各造各的、自己重发拿回自己那一个；成员的造会话重启以后照样认得出重发。
2. `settle/tests.rs`：取消的不打 `ERROR`、不再盯；panic 的打 `ERROR heap settle panicked panic=true`、接着盯。
3. 手改：去重只比编号、补回只读机主的、取消照旧打 `ERROR`，各至少红一条。
4. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
