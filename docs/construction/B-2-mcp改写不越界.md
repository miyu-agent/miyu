## 施工单 B-2：`mcp.set` 改写不越界

状态：做完，等合。漏洞排查会话 2026-10-22 的鲁棒性测试（补丁 `core-robust-tests.patch`）复现。

### 根因

同一个服务点号键写在前、子表写在后（合法的 TOML），`mcp.set` 换掉它时（`crates/miyu-endpoint/src/mcp/written.rs` 的 `put`），新的一张插在第一张表的位置，位置照原文算；前面点号写的那几行已经删了、字变短了，`&cut[..at]` 越界 panic。

### 做了什么

- 插入的位置照删过以后的字算：减去它前面删掉的那几段的长度。

### 验收

1. `mcp::written::tests::a_server_written_both_dotted_and_as_tables_does_not_panic`（补丁里的，修前红）。
2. 变异（去掉减去的那一步）报红。
3. `cargo xtask check` 八道全过。
