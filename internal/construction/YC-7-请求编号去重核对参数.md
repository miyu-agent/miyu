## 施工单 YC-7：请求编号去重核对参数

状态：施工中（2026-11-10）。压测报告 O1。

### 为什么

协议照命令编号去重：同一个 `id` 再来，交回上一次的结果，不再生效（断线重发用得上）。可是只比编号、不比参数：头重用了编号、换了参数，`session.send` 换了一句话被悄悄吞掉，`session.create` 换了人格、`memory` 交回的是上一次那个会话，头看到的是「成功」。

### 做什么

- 内核 `recent.rs`：每个记着的编号另记一份命令的指纹（照 `Debug` 写法算的标准库哈希，只在内存里）。判一个新命令前 `expect` 记下编号和指纹，收下时 `insert` 跟着编号记，满了一起丢；十几处 `insert` 不用改。接受过的编号再来、指纹不一样：拒绝，新原因码 `id_reused`（`Reason::IdReused`），什么都不记；一样的照旧照上一次回应。载入时从日志认回来的编号没有指纹，再来照老规矩回（重启以后不比参数）。回顾不记编号，也不比。
- 协议 `sessions.rs`：造会话的编号另记一份参数的指纹，`cwd`、`dirs` 不算（重发的 `cwd` 照这一次报的算，原来的规矩）；别的参数换了的拒 `id_reused`（`Refusal::ID_REUSED`，`refusal/reused.rs`）。重启以后从 `session.created` 补回来的编号没有指纹，照旧交回原来那一个。
- 给人看的话：「同一个请求编号换了参数，没有收。请换一个新的编号。」，英文「This request id was already used with different parameters. Use a new id.」。
- 蓝图 `protocol.md`「`session.create`」第 4 条、「同一个命令编号」（多一条第 2 条，后面的往后挪）、错误码表、给人看的字、测试表；`kernel/session.md`「命令和回应」第 4 条、原因码表、测试表；参考 `errors.md`、`errors.en.md` 各一行，`connection.md`、`connection.en.md`「先后」第 3 条。
- 原来照「换成别的也照上一次回应」写的几条测试改成新规矩：`tests/configure.rs`、`tests/meta.rs`、随机测试的换模型（看守不再把 `id_reused` 当成没接受过）、`miyu-endpoint` 的 `respond.rs`；`loop_command.rs` 每次换一个请求编号。

不做：指纹不落盘（落盘要改日志格式，重启以后的重发本来就少）；不比 `by`、`at`。`command.run` 先跑命令、再把 `{id}/ran` 交给内核记：换了参数重发的，命令照样跑了一遍，回的才是 `id_reused`（原来是跑了一遍、记的那一条被悄悄吞掉）；协议那一层先比要另排。

### 验收

- `crates/miyu-kernel/src/session/tests/again.rs`：原样再来照上一次回应、不记；换了一句的落盘前后都 `id_reused`、什么都不记；同一个编号换成打断也拒；日志里认回来的编号换了一句照上一次回应。
- `crates/miyu-endpoint/tests/reused_ids.rs`：造会话只换 `cwd` 的交回原来那一个、`cwd` 照这一次；换了 `memory` 的 `id_reused`、只造了一个；说话原样再来照上一次回应，换了一句的 `id_reused`、只记了第一句；重启以后补回来的造会话编号换了参数照旧交回。
- 先红：`reused()` 总回假、协议那一层不比指纹时，内核两条、协议两条报红。
- 本机门禁全过。改协议（多一个原因码），合了告诉 UI 和 im。
