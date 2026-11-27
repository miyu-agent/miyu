## 施工单 CS-7：核心停了，扩展、MCP 服务起的子进程不再留下

状态：做完，等验收（分支 `step/CS-7-测试不留扩展进程`）。2026-11-27 core1 派：每跑一趟 `cargo xtask check` 留下 5 个 `target/debug/miyu-test-extension hang`，父进程是 systemd，几个小时不退。

### 目的

核心有计划地停下（信号、空闲、头叫停、数据根没了），或者最后一个扩展、MCP 服务退出、被重启时，它们起的、还留在共用组里的子进程跟着走，不再成为一直留着的孤儿。门禁跑完一个都不留。

### 根因

- 生产代码（施工 XH-1 起，施工 LW-28 照样）：长住的子进程共用一个组、一个看门的（`crates/miyu-sandbox/src/lifeline/unix/shared.rs`）。有计划地停时只杀进程本身（`crates/miyu-endpoint/src/extensions/supervise.rs` 的 `finish`），孙进程留在组里；最后一个拿着组的丢了，原来只收看门的（`Watcher` 的 `Drop`，`lifeline/unix.rs`），孙进程从此没人看着，核心停了、没了都不走。
- 不是施工 CS-6 的生产改动带进来的：CS-6 没动 `miyu-sandbox`、`supervise.rs`。它新加的 `crates/miyu/tests/stopping.rs` 是第一份「有计划地停一个带孙进程的扩展」的测试，把这条老毛病照了出来：`a_stopping_core_takes_no_new_connections` 留 1 个；`starting_while_the_core_stops_brings_up_a_new_one` 留 4 个（旧核心 1 个，新核心的扩展不握手、被重启两次各 1 个，数据根没了停下时 1 个）。`crash_kept.rs` 硬杀核心，看门的整组杀，一个不留。
- 测试夹具（同时修）：`Home::core_pid` 取整份日志最后一个 `pid=`，装了扩展的拿到 `extension started pid=…`，`Home` 丢掉时的 `SIGTERM` 打到扩展身上，新核心一直活到「数据根没了」的那一次看（最多 30 秒），中间把扩展重启了两次。

### 蓝图改哪几节

- `core.md`「子进程随核心退出」第 5 条前两款：最后一个拿着的丢了先整组 `SIGKILL`，再收看门的；测试表加 `stop_kept.rs`。
- `extensions.md`「怎么走」第 1 条、`mcp.md` 第二条第 6 条：各补一句。

### 不做什么

- 不改有计划地停的顺序和 5 秒：还有别的长住的子进程拿着组时，重启留下的孙进程照旧留着，最后一个丢掉或者核心没了时收。
- 看门的起不来、自成一组的那种不管（照旧只杀进程本身）。
- Windows 不动：核心退出时作业对象关掉，里面的都结束。

### 验收

1. 改前红：`miyu-sandbox` 的 `lifeline_kept.rs` 新加 `the_last_holder_takes_the_leftovers_with_it`（只杀长住的子进程本身，最后一个丢了，孙进程要没了）红在头一种看门的（`miyu-watchdog`）上；`miyu` 的 `stop_kept.rs`（新）扩展、MCP 服务两条都红（「核心停了，扩展……还剩 [孙进程]」）。改后全绿。
2. `stopping::`、`crash_kept::`、`stop_kept::` 跑完，`ps -eo pid,ppid,args | grep '[m]iyu-test-extension'` 这棵树下一个都没有（改前单跑这四条留 5 个）。
3. 门禁全过，跑完照样数，一个都没有。

### 风险

- 整组杀打错组：只在看门的这一刻还没被收时杀（它在组里，组号不会被别人占）；已经没了、收掉了的不杀。
- 留下的进程拿着编译锁：测试的进程是 `flock … cargo test` 的子孙，继承锁的描述符。2026-11-27 实测留下的 `hang` 拿着本机的 `miyu-cargo.lock` 卡住了排在后面的编译十几分钟；这一步以后不再留。
