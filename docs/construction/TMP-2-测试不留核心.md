## 施工单 TMP-2：测试不留核心，数据根没了核心自己走

状态：写好了（2026-10-29）。主会话的机器上攒了十个 `<工作树>/target/debug/miyu core`：程序已经被重新编译掉（`(deleted)`），父进程是用户的 systemd，跑了二十个小时一直不退；数据根、工作目录都是已经删掉的 `/tmp/miyu-main-<进程号>-<时刻>-<n>`，几次门禁各留下 39、40、42、50 这四个。主会话定（2026-10-29）：两头都查，哪些测试拉起核心不收，核心的数据根没了为什么不走、要不要在产品上修。

### 根因

1. **哪些测试**：`crates/miyu/tests` 里让头自己拉起核心的那几条：`heads.rs` 的三条（在本机插桩跑一遍，对得上 39、40、42）、`login.rs` 的 `misuse_and_no_core_are_refused_before_reading_a_key`（50），`config.rs` 里同样形状的几处。头拉起的核心不带 `--idle-seconds`，施工 Z-1 上起默认不空闲退出，全靠测试最后一行 `kill_core` 收。中间哪个断言 panic 了（门禁满载时等核心说好了超过十秒就会），走不到这一行，核心就漏了；夹具丢掉时只等锁十秒，等不到照样删目录。本机不加负载反复跑都干净，复现不了，所以修法不靠运气。别的测试先用 `Home::core`（空闲一秒就走）拉起，不漏。
2. **为什么不走**：没有空闲时限的核心只等停的信号。数据根删了，锁还在它打开的那个文件上，它不知道；路径上已经没有了，下一个核心在新建的数据根上照样拿得到一把锁。本机还有一个别处（不是这个仓库的测试）留下的，数据根删了、跑了四天多，看线程还在 `serve` 里正常转，不是退出时卡住了。

### 做了什么

1. 锁记下锁文件是哪一个（`crates/miyu-ipc/src/lock.rs` 的 `Held`）：Unix 上拿锁时记下打开的文件的设备号和 inode，看时重新查路径比；Windows 上开着的文件删不掉、改不了名，只看在不在。`Listener::held` 交出同一份。路径照系统解析（macOS 的 `/var` 指着 `/private/var`，查的是同一个文件）；比的是拿着的那个文件，锁文件里写什么不相干。
2. 核心看到数据根没了就走（`crates/miyu-core/src/serve.rs`）：没有连接、每次看空不空闲的时候先看一眼；不是拿着的那一个了，记 `WARN data root gone, stopping`，有计划地停下会话，交回 `Stopped::Gone`，记 `stopped reason=gone`。没有空闲时限的每 30 秒看一次，所以最多三十秒走。
3. 测试的夹具收没人停的核心（`crates/miyu/tests/support/mod.rs`）：丢掉时先照旧等锁十秒；还被拿着的，照运行日志里「起来了」那一行的进程号请它停（Unix 上 `SIGTERM`，Windows 上强行结束），再等十秒，还不走的强行结束。`kill_core` 和它共用取进程号、结束进程那一段。多一个 `Home::core_staying`：同头拉起的一样不带空闲时限。
4. 蓝图：`core.md`「在哪」「停下」第 7 条、新的第 9 条、出错、守着它的；`ipc.md`「对外的样子」「核心起来」第 1 条、守着它的。

### 不做什么

- 不动主会话列出来、还在跑的那几个进程（主人定）。
- 不改 `crates/miyu-cli`（主会话在改全部命令行的字，施工 CL）。
- 连着头的核心不看数据根：有头连着说明有人在用；头走了以后照常看。
- 不改头拉起核心等「好了」的十秒：满载时超时是门禁的事，不是这一步的。

### 验收

1. `crates/miyu-core/tests/serve.rs` 的 `a_core_whose_data_root_is_gone_stops_by_itself`（只在 Unix 上）：没有空闲时限的核心，数据根在时不走，删了以后自己走、`Stopped::Gone`；修之前红（十秒没等到）。
2. `crates/miyu/tests/core.rs` 的 `a_core_nobody_stopped_goes_with_the_home`：头拉起那种不空闲退出的核心，测试不收，夹具丢掉以后十五秒内没了；夹具不收之前红（夹具丢掉以后核心还在）。这个漏掉的核心在数据根删了以后自己走了：真进程上验过第 2 条。
3. `crates/miyu-ipc/src/lock/tests.rs`：锁文件删了的、删了又建了的不是拿着的那一个（只在 Unix 上）。
4. `cargo xtask check`；Windows 版 clippy；三平台 CI（macOS 上锁文件在 `/var/folders` 下，看的路径照系统解析）。
