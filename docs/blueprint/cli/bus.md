## `miyu bus`

### 是什么

印总线上谁提供了什么、可不可用（施工 BUS-3，设计 `67-总线.md` 第四节「命令行」），照 `busctl list`。查「为什么终端里没有麦克风」「长消息为什么没转图」用。连上核心（没在跑就拉起来），发 `bus.list`，不另起一套。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `bus`；换上帮助页 |
| `crates/miyu-cli/src/bus.rs` | 参数、找数据根、连核心、握手、发 `bus.list`、印、退出码；一行的样子（`lines`，纯的） |
| `crates/miyu-cli/src/bus/tests.rs` | 一行的样子 |
| `crates/miyu-cli/src/language/bus.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/bus.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出总线上的名字 | `bus.list` |
| `list <名字>` | 只看这一个名字 | `bus.list {"name"}` |

`--format text|json`（默认 `text`）：`json` 原样印回应。

### 样子

一行一个：名字、种类、包、可不可用，名字、包照最长的对齐，行尾不留空格；种类、可不可用照语言（中文「提供」「接进」「可用」「不可用」，英文 `provides`、`plugs`、`ready`、`not ready`）。一个都没有的印一句「总线上还没有名字」。

```
stt         提供  miyu-voice-input  可用
tts.engine  接进  gptsovits         不可用
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 印出来了，一个名字都没有的也是 |
| 1 | 找不到数据根、连不上核心；被拒绝；核心断开 |
| 2 | 参数不对：子命令写错、`--format` 写错（`cli/main.md`） |

### 守着它的

`crates/miyu-cli/src/bus/tests.rs`：两行的对齐、中英文的种类和可不可用、一个都没有的那一句。`bus.list` 本身由端点的 `tests/bus.rs` 守着（施工 BUS-2）。
