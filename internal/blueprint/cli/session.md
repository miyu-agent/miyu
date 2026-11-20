## `miyu session`

### 是什么

在 shell 里导出、导入会话（施工 SE-1 中上、中下下，设计 `58-会话导出和导入.md`）：原始包换机器、换数据根时搬会话，旧版迁移程序之外的人工搬运也用它；Markdown 自己看、贴给别的 agent、贴进网页 chat（`view.md`「导出成 Markdown」）。连上核心（没在跑就拉起来），握手，发 `session.export` / `session.import`（`protocol.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `session`，下面 `export`、`import`；换上帮助页 |
| `crates/miyu-cli/src/session.rs` | 参数、找数据根、连核心、握手、相对路径接成绝对的、发、印、退出码 |
| `crates/miyu-cli/src/help/{zh,en}/session.txt` | 帮助页，`export`、`import` 的 `-h` 印同一页 |
| `crates/miyu-cli/src/language.rs` | `-o -` 超了上限时那条建议（`export_to_file`） |
| `crates/miyu-cli/tests/session.rs` | 两个核心之间导出再导入；`--md` 写到默认的 `.md`、`-o -` 带 `--for`、`--last` 打到标准输出；太大的报错带建议 |

### 用法

```text
miyu session export [<会话>] [-o <文件>]
miyu session export [<会话>] --md [--for read|agent|chat] [--last N] [-o <文件>|-]
miyu session import <文件> [--archive]
```

### 怎么走

1. `export`：不写会话的是上一次 `miyu ask` 开的那个（同 `miyu rename`，`link::latest_oneshot`）；不写 `-o` 的写到当前目录下的 `miyu-session-<编号前 8 位>.zip`。相对路径照当前目录接成绝对的（核心只认绝对路径）。做成了在标准输出印一行写到哪（绝对路径），退出码 0。
2. `export --md`（施工 SE-1 中下下，调研第五节第 5 条）：交 `session.export {format: "markdown", for}`，`--for` 不写的是 `read`；`--last N`（N ≥ 1）是 `range: {"turns": N}`；`--for`、`--last` 不带 `--md` 的照用错了参数报（clap 的 `requires`）。不写 `-o` 的写到 `miyu-session-<编号前 8 位>.md`，写好了印路径同上。`-o -` 走 `to: "inline"`，Markdown 原样写到标准输出（不另加换行），好接管道、`wl-copy`、`pbcopy`；不做剪贴板。超了 512 KiB 的核心拒 `export_too_big`：照原话报错，另起一行提示「用 -o <文件> 存成文件」，退出码 1，标准输出上什么都没有。`children`、`assets` 命令行不收，要的走协议。
3. `import`：文件照当前目录接成绝对路径，交 `session.import {path, archive}`。新导进来的会话一行一个印在标准输出；早就在的（重导）不印，退出码照样 0。
4. 被拒的（文件已经有了、写到数据根里、包坏了、同编号内容不一样……）照核心的原话印在标准错误上，退出码 1；核心没在跑、又没设 key 的 5（同别的命令）。
