## 施工单 test-ext 三补：测试扩展整行写标准错误

状态：施工中（2026-10-12 主会话）。记忆的会话报的偶发红：CI run 1873，Windows 上 `extensions::one_that_never_says_hello_fails_too` 读到 `up\nup\nupup\nup\n`。

### 目的

测试用的扩展 `err:<字>` 一行一次写完：`eprintln!` 把字和换行分两次写（`strace` 看得到两次 `write`），核心退避时杀进程正好落在两次之间，就少一个换行，下一次拉起的接在后面。改成先拼好整行、一次 `write_all`（`strace` 看到一次 `write`）。

### 不做什么

- 测试照旧逐字节比，不放宽。

### 验收

- `strace -e trace=write` 跑 `miyu-test-extension err:up`：改前两次 `write`，改后一次。
- `cargo xtask check` 八项；三台 CI。
