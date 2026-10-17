## 施工单 W-6 三补：扩展只读得到它拿得到的 blob

状态：2026-10-18 合了（86c6442e）。接入QQ 的会话报的；图纸 `docs/blueprint/protocol.md` 的 `blob.get` 第 1 条。

### 起因

`blob.get` 不写 `session` 的照管理员名下给，写了的照会话的属主名下给，属主、管理员直接放行。扩展的连接也走这两条：没声明系统账号的包的扩展就是管理员，知道一个哈希就能读走主人名下任何一个 blob（不写 `session`，或者写上主人随便一个会话）。哈希会从事件、别的包的回应里漏出去，不能当口令用。

### 做了什么

1. `crates/miyu-endpoint/src/attach/grants.rs`（新）：核心内存里一张表，记每个包拿得到哪些 blob。自己 `blob.put`、分块上传 `blob.close` 存的记上；`package.call` 的回应里出现的哈希，答的那个包拿得到的转给调的包（`backstage/calls.rs`）；包卸了扔掉它那一份（`packages/manage.rs`）。核心重起从空的开始。
2. `attach.rs` 的 `get`：扩展的连接不写 `session` 的，表里没有就 `unknown_blob`。
3. `attach/reach.rs`：扩展的连接写了 `session` 的，照账号放行那一条不放它，blob 也得是那个会话结构里引用过的。
4. 人的连接照旧。

### 测试

- `crates/miyu-endpoint/tests/blob_grants.rs`（新）两个：不写 `session` 的几种（自己存的、主人存的、别的包存的、经 `package.call` 拿回来的、答的包写进回应但自己拿不到的）；写了 `session` 的管理员扩展（引用过的、没引用过的）。修之前两个都报红（主人存的读到了、没引用过的读到了）。
- `src/attach/grants/tests.rs`：表的三样。
- 手动变异：`pass` 不看答的包拿不拿得到，测试报红。
- 测试用的扩展多一步 `give:`（`method.call` 的 `give` 交回这几段字）。
