## `miyu session`

### 是什么

在 shell 里导出、导入会话的原始包（施工 SE-1 中上，设计 `58-会话导出和导入.md`）：换机器、换数据根时搬会话，旧版迁移程序之外的人工搬运也用它。连上核心（没在跑就拉起来），握手，发 `session.export` / `session.import`（`protocol.md`）。Markdown 的三种随 SE-1 中下。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `session`，下面 `export`、`import`；换上帮助页 |
| `crates/miyu-cli/src/session.rs` | 参数、找数据根、连核心、握手、相对路径接成绝对的、发、印、退出码 |
| `crates/miyu-cli/src/help/{zh,en}/session.txt` | 帮助页，`export`、`import` 的 `-h` 印同一页 |
| `crates/miyu-cli/tests/session.rs` | 两个核心之间导出再导入 |

### 用法

```text
miyu session export [<会话>] [-o <文件>]
miyu session import <文件> [--archive]
```

### 怎么走

1. `export`：不写会话的是上一次 `miyu ask` 开的那个（同 `miyu rename`，`link::latest_oneshot`）；不写 `-o` 的写到当前目录下的 `miyu-session-<编号前 8 位>.zip`。相对路径照当前目录接成绝对的（核心只认绝对路径）。做成了在标准输出印一行写到哪（绝对路径），退出码 0。
2. `import`：文件照当前目录接成绝对路径，交 `session.import {path, archive}`。新导进来的会话一行一个印在标准输出；早就在的（重导）不印，退出码照样 0。
3. 被拒的（文件已经有了、写到数据根里、包坏了、同编号内容不一样……）照核心的原话印在标准错误上，退出码 1；核心没在跑、又没设 key 的 5（同别的命令）。
