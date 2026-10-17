## 施工单 X-8 再补：再发 `/goal` 就是改

状态：施工中（2026-10-17）。2026-10-17 项目主人定；图纸 `docs/blueprint/goal.md` 第 6 条、`protocol.md` 的 `command.run` 第 10 条。

### 为什么

以前已经有一个没完成的目标时，`/goal <目标>` 拒 `goal_exists`，改要写 `/goal edit <目标>`，两个头都没提这个子命令，项目主人以为只能设和清。项目主人定：头里以前的 `/goal` 在输入历史里，按上方向键调出来改了再发就是改，不用 `edit`；要换新的先 `/goal clear`。

### 做了什么

1. `miyu-endpoint` 的 `commands/goal.rs`：`/goal <目标>` 先看现在有没有没完成的（跑着、等人、停着、卡住）：有的照 `edit` 办（`goal.changed`，没写 `--rounds` 的上限照原来的），停着、卡住的再发一条 `Resume`（编号接 `/resume`，记 `goal.resumed`），回执「已修改长期目标：…」；没有的、完成了的照旧新设。`--rounds 0`（写了不限）和没写分得开。
2. 内核的规矩不动：她的 `goal` 工具有一个没完成的再 `set` 照旧拒 `goal_exists`。
3. `/goal edit` 照旧认；头的帮助、补全不再提（已告诉两个头）。

### 测试

- `commands/goal/tests.rs`：`--rounds 0` 和没写分得开。
- `tests/goal_command.rs` 的 `a_second_goal_edits_the_current_one`：停着的改完接着跑、轮数接着数、上限照原来的；等人的只改；清掉以后是新设；日志里 `goal.*` 的先后。原来两条测试里「再设拒 `goal_exists`」的两步拿掉。
- 手写变异 2 处全报红（不接着跑、上限照写的）。
