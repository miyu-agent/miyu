## 施工单 FX-14：撤回没处理时敲 `/loop` 算人开口，循环马上开始

状态：做完了，等合并（分支 `step/FX-14-敲loop算人开口`，从 main 的 ebe63021d 开；主会话 2026-11-22 派工）。蓝图 `schedules.md` 第三节第 4、9 条、第五节第 9 条，`kernel/session.md` 命令表和「定时交进来的话」第 3 条；手册 `automation.md`（中英）。

### 目的

项目主人 2026-11-22 定：「我都 /loop 了，我还要撤回干嘛？这消息是用户发的！」人亲手敲 `/loop …`（终端、网页、私聊走 `command.run` 的那一路）是人的一次操作，和人发一句话一样，表示接受了这次撤回。

FX-12、FX-13 以后，撤回没处理时敲 `/loop`，回执说「有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始」，循环搁着不交。人敲了命令却什么都没发生，还要再发一句话才开始。

### 改了什么

- 内核加命令 `Kickoff { blocks }`（`session/input/command.rs`，`session/messages.rs` 的 `kickoff`）：`by` 是 `schedule` 的，照「定时交进来的话」收，记下的事件和 `Send` 交的一模一样；只是叫醒她时（`Waker::Schedule` 的 `typed`）`can_wake` 不看撤销，还能恢复撤销也由它开一轮，开的 `turn.started` 照账本清掉撤销。`by` 不是 `schedule` 的照 `Send`。日志、协议的形状都不变。
- 核心（`schedules/fire.rs`）：人建的、跑在已有会话上、还没跑过（`last` 是空的）的循环，就是人刚敲的 `/loop` 的第一次（`kickoff`）。到点时不因撤销等（`waits` 只看忙不忙），交 `Kickoff`。之后每一次照旧：撤回没处理时搁着（FX-13）。
- `/loop` 的回执不再分撤回没处理的情况，照常说「已开始循环……」。删掉 `commands/loop-fixed-held`、`commands/loop-dynamic-held`、`commands/loop-auto-held` 三句（中、英、日三份字表）和回执里分 held 的分支。
- 蓝图、手册同步。

### 不做什么

- 先建好循环、之后人才撤回、然后到点：人没有新操作，照 FX-13 搁着，发一句话或 `/restore` 以后再交。
- 定时任务、她自己建的循环、后台命令、别的会话发来的话的规矩都不变。
- 没人看着的一次性会话里循环照旧只记下（`kernel/session.md`「定时交进来的话」第 1 条）。

### 验收

- `crates/miyu-endpoint/tests/loop_held.rs`：
  - `a_loop_typed_while_a_revert_is_pending_starts_right_away`：撤回后敲 `/loop 5m 看看 CI`，回执是「已开始循环：每 5m · 看看 CI」，马上开一轮，请求里有「看看 CI」、没有撤掉的「在吗」，之后 `session.unrevert` 拒 `nothing_to_unrevert`。改之前回执带着「循环暂不运行」，这条是红的。
  - `an_autonomous_loop_typed_while_a_revert_is_pending_starts_right_away`：同上，光打 `/loop`。改之前也是红的。
  - `a_loop_due_while_a_revert_is_pending_waits_for_restore`：没撤回时敲 `/loop 10m 看一眼时间`（回执照原来的说）、跑了第一次，撤掉这一轮，到了第二次：不开轮、不留话；恢复以后交一句、开一轮。
  - FX-13 的两条改成同样的起法（先建循环、再撤回、到点），再 `/loop off`：恢复以后不开轮；人说一句，请求里「看一眼时间」只有一次（第一次交进去的那句撤销不带走，「定时交进来的话」的老规矩），搁着的那一次没有多出来。
- 内核 `session/tests/scenario/kickoff.rs`：撤销没处理时 `Kickoff` 开一轮、之后恢复不了；同一句用 `Send` 交只记下；没人看着的一次性会话里照旧只记下。
- 黑盒：沙箱数据根（`MIYU_HOME` 指临时目录）起 `target/debug/miyu core`，模型指 `bench/rivals/toolfake.py` 的假模型，走协议说一句、`session.revert`、`command.run /loop 5m 看看 CI`：回执「已开始循环：每 5m · 看看 CI」，日志接着 `command.ran`、`by: schedule` 的 `message.user`、`turn.started`，假模型收到一次请求，`session.unrevert` 回 `nothing_to_unrevert`。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过，`miyu-kernel`、`miyu-endpoint`、`miyu-session` 的测试通过，本机门禁 12 项全过。
