## 施工单 RS-1：重启接着跑最多三次

状态：做完了（2026-11-12）。macOS 上有人报的循环；项目主人 2026-11-12 定「同一轮最多自己重启 3 次」，core1 2026-11-12 照推荐定细节。

### 为什么

macOS 上有人报：会话里她换了 miyu 的程序、用 `launchctl` 重启核心；核心照「有计划的重启」把这一轮记成 `restarted`，再起来接着干；接着干的一轮里她又换程序、又重启，一直转下去。

接着干的上限 2026-09-28 就有：`resumes`（3，策略快照里的数，`miyu-policy` 的 `compose.rs`），蓝图 `kernel/session.md`「载入和崩溃」第 6 条。漏在清零的口径上：原来是「别的回合一开，从 0 数」。推断的循环（照代码推的，等报告者的日志佐证：两次 `restarted` 之间有没有回报叫醒的 `turn.started`）：

1. 接着干的那一轮放一条后台命令去重启核心，自己结束。
2. 命令在核心收到停的信号以前退出了，记 `exited`，叫醒她，开一轮：不是接着干的那一轮，计数清零。
3. 这一轮被重启打断，再起来接着干，计数是 1；回到第 1 步，永远数不到 3。

### 改了什么

1. 载入的数法（`crates/miyu-kernel/src/session/load.rs` 的 `Replay`）：数从人最后说的那句话起，接着干开过几轮（`turn.started` 的 `trigger` 是被重启打断时排着的最后一条、没有就是那条 `turn.ended`）。到了 `resumes` 不接，和崩了一样等人开口，排着的留在日志里，什么都不另记。
2. 清零只认人开口：`message.user` 的 `by` 是人（`By::is_person`），回合中途插进来的也算，和「回报」第 11 条那一串（`chain.rs`，R3E-2）同一个口径。回报、子代理的留言、别的会话、别的 harness 发来的话、定时任务、长期目标的续轮开的回合都不清零。
3. 不动的：上限照旧是快照里的 `resumes`，不做配置项；只照日志往回数，不加事件、不加原因、不加存状态的地方；崩了（日志停在没结束的回合）照旧不接；手动压缩那一轮（没有 `trigger`）照旧不接；子会话没接着干的照旧当场报 `aborted`。
4. 蓝图：`kernel/session.md`「载入和崩溃」第 6 条、策略那张表的 `resumes`；`policy.md` 的 `resumes`。随机测试的看守照同样的数法对（`tests/random/watch/load.rs`）。

### 怎么验

1. 场景测试 `crates/miyu-kernel/src/session/tests/scenario/resume_limit.rs`：重启一次接着干；接够三次第四次不接、什么都不另记；主力：回报叫醒的一轮夹在中间，第四次照样不接；人说一句从头数，又能接三次；回合中途插进来的一句也从头数；接着干以后崩了照旧不接。手动压缩那一轮不接由 `manual.rs` 的 `after_a_restart_she_does_not_go_on_compacting` 守着。
2. 修之前红：载入换回原来的数法，主力那条报红；上限临时改得很大，接够三次的几条报红。
3. 随机测试：`cargo test -p miyu-kernel random:: -- --include-ignored`。
