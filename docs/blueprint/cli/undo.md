## `miyu undo`、`miyu restore`

### 是什么

在 shell 里撤掉当前会话的最后一轮（只撤对话，文件不动：施工 U-1，2026-10-15 项目主人定，改回文件交给 git 这类程序）；发下一句之前，`miyu restore` 恢复最近一次撤销。`miyu undo` 也可以写成 `miyu rewind`（2026-09-29 项目主人定：恢复原来叫 `miyu redo`，施工 4-7 补改名）。连上核心（没在跑就拉起来，施工 8-6 起一律拉起），找会话，发 `session.revert` 或者 `session.unrevert`，照核心交回的几样印出撤了哪一轮。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `undo`（别名 `rewind`）、`restore`；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/undo.rs` | 参数、找数据根、连核心、握手、找会话、发、退出码 |
| `crates/miyu-cli/src/undo/print.rs` | 核心交回的几样写成一行行 |
| `crates/miyu-cli/src/language/undo.rs` | 这两条命令给人看的字 |
| `crates/miyu-cli/src/help/{zh,en}/{undo,restore}.txt` | 帮助页（`cli/main.md`「帮助页」） |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、找最新的一次性会话、请求的编号、一行怎么上色、路径怎么写短、截断；和 `miyu ask` 共用 |
| `crates/miyu-cli/src/language.rs` | 界面语言；「出错」、没有一次性会话、核心断开的那几句 |
| `crates/miyu-endpoint/src/undo.rs` | 核心那边算回应里的几样（`protocol/undo.md`） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `-s`、`--session <编号>` | 撤（恢复）这个会话；不写的是上一次 `miyu ask` 开的那个，就是最新的那个一次性会话 |

- 没有别的参数。`miyu undo` 一次撤最后一轮，要往前就再撤一次。
- 界面语言照 `cli/main.md`，帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`NO_COLOR`；拉起的核心照它自己的一套（`core.md`）。

### 怎么走

1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**（`ipc.md`），照 `miyu ask` 的规矩：没在跑就拉起来（施工 8-6 起 key 来自配置，一律拉起；以前没设 `DEEPSEEK_API_KEY` 的不拉起、退出码 5）。连不上、拉不起：原因写在标准错误上，退出码 1。
3. **握手** `hello`：和 `miyu ask` 一样（`cli/ask.md` 第 3 步），`caps.input` 是 `false`。
4. **找会话**：
   1. 写了 `--session`：照写的，头这边不查写法，交给核心查。
   2. 没写：`session.list`，带 `oneshot: true`、`limit: 1`，取第一个。一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
5. **发**：`miyu undo`（`miyu rewind`）发 `session.revert`，`miyu restore` 发 `session.unrevert`，参数都只有 `{"session": …}`。不写 `turn`：核心撤还在有效历史里的最后一轮（`protocol/undo.md`）。
6. 请求的编号是 `undo-<16 位十六进制>-<序号>`，`miyu restore` 也是 `undo-`：前缀每个进程随机一次（取不到随机数的，用进程号和此刻的纳秒，各写成十六进制接在一起），序号从 1 数起。
7. **被接受**：照下面「样子」印在标准输出上，退出码 0。
8. **被拒绝**：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。
9. **核心断开**：说「核心断开了」，退出码 1。请求写不出去：系统的原话写在标准错误上，退出码 1。
10. 不订阅，不收推送。没有另外接 Ctrl+C。

### 样子

被接受的：标准输出上一行行印，标准错误上什么都不印。

样本 `docs/designs/samples/cli/undo-text.txt`（撤销，中文）：

```text
· 撤销「把 README 改成中文」这一轮
发下一句之前，可以用 miyu restore 恢复。
```

| 行 | 是什么 | 颜色 |
|---|---|---|
| 1 | 撤的（恢复的）是哪一轮 | 灰 |
| — | 撤掉了压缩、清空、停掉了几个任务（这个例子里没有）：各一句，在第一行下面 | 灰 |
| 2 | 怎么恢复：只有撤销有，前面没有 `· ` | 灰 |

- 不印文件（施工 U-1）：撤销不再改回文件，回执也不提文件（2026-10-15 项目主人定）。以前的核心回应里的 `files`、`commands` 照样不印。

**第一行**：`said` 是回应里人说的那句话（`protocol/undo.md`），控制字符换成 `�`，超过 40 个字的留前面 40 个，后面加 `…`。`turns` 是撤了几轮，回应里没有的当 1。

| | 有 `said`，`turns` 是 0 或 1 | 有 `said`，几轮 | 没有 `said` |
|---|---|---|---|
| 撤销 | `· 撤销「<said>」这一轮` | `· 撤销「<said>」起的 <几> 轮` | `· 撤销最后一轮` |
| 恢复 | `· 恢复「<said>」这一轮` | `· 恢复「<said>」起的 <几> 轮` | `· 恢复撤销的那一轮` |

**撤掉了压缩的那一句**（施工 6-9，`compaction.md` 第十一条）：回应里 `compactions` 大于 0 才印（核心只在撤销时交），印在第一行下面，灰，前面是 `· `。写 `· 撤掉了压缩，上下文回到了压缩前`：撤掉一次、几次（一轮里压过几次的）都是这一句（2026-09-29 项目主人定）。`miyu restore` 不印。

例子（撤销，这一轮里压过一次）：

```text
· 撤销「接着把测试补完」这一轮
· 撤掉了压缩，上下文回到了压缩前
发下一句之前，可以用 miyu restore 恢复。
```

**撤掉了清空的那一句**（施工 6-8 补，`compaction.md` 第十四条，2026-09-30 项目主人定）：回应里 `clears` 大于 0 才印（核心只在撤销时交），照撤掉了压缩那一句的规矩：灰，前面是 `· `，撤掉一次、几次都是 `· 撤掉了清空，上下文回到了清空以前`，`miyu restore` 不印。两样都有的，先压缩后清空：回应里只有次数，没有先后，定一个顺序。

样本 `docs/designs/samples/cli/undo-clear-text.txt`（撤销，这几轮里压过一次、清空过两次）：

```text
· 撤销「接着把测试补完」起的 3 轮
· 撤掉了压缩，上下文回到了压缩前
· 撤掉了清空，上下文回到了清空以前
发下一句之前，可以用 miyu restore 恢复。
```

**停掉了几个任务的那一句**（施工 7-8，`agents.md` 第七条第 1 条）：回应里 `jobs` 不空才印（核心只在撤销、重做时交），灰，前面是 `· `，只说几个：`· 停掉了 <几> 个任务`，是哪几个在回应里。印在撤掉了压缩、清空的那两句下面。`miyu restore` 不印。

样本 `docs/designs/samples/cli/undo-jobs-text.txt`（撤销，这一轮放了一条后台命令、派了一个子代理，都还在跑）：

```text
· 撤销「后台跑测试，再派一个去查 CI」这一轮
· 停掉了 2 个任务
发下一句之前，可以用 miyu restore 恢复。
```

**最后一行**：只有 `miyu undo` 印：`发下一句之前，可以用 miyu restore 恢复。`

**上色**：标准输出是终端、`NO_COLOR` 没设或者设成空的才上色（no-color.org；施工 4-9 再补四上：原来设成空的也不上色）。灰是 `ESC[90m`。一行从灰开始，换颜色时写新颜色，行尾写 `ESC[0m`；不上色的只有字。例如：

```text
ESC[90m· 撤销「把 README 改成中文」这一轮ESC[0m
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 被接受 |
| 1 | 找不到数据根、建不了骨架；连不上、拉不起核心；被拒绝；核心断开；请求写不出去；一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 撤销，一轮 | `· 撤销「<said>」这一轮` | `· Undid the turn “<said>”` |
| 撤销，几轮 | `· 撤销「<said>」起的 <几> 轮` | `· Undid <几> turns from “<said>”` |
| 撤销，没有 `said` | `· 撤销最后一轮` | `· Undid the last turn` |
| 撤掉了压缩（一次、几次同一句） | `· 撤掉了压缩，上下文回到了压缩前` | `· Undid the compaction; the context is back to how it was before` |
| 撤掉了清空（一次、几次同一句，施工 6-8 补） | `· 撤掉了清空，上下文回到了清空以前` | `· Undid the clear; the context is back to before it.` |
| 停掉了任务（施工 7-8） | `· 停掉了 <几> 个任务` | 一个：`· Stopped 1 job`；几个：`· Stopped <几> jobs` |
| 恢复，一轮 | `· 恢复「<said>」这一轮` | `· Restored the turn “<said>”` |
| 恢复，几轮 | `· 恢复「<said>」起的 <几> 轮` | `· Restored <几> turns from “<said>”` |
| 恢复，没有 `said` | `· 恢复撤销的那一轮` | `· Restored the undone turn` |
| 最后一行 | 发下一句之前，可以用 miyu restore 恢复。 | Until you say something else, miyu restore brings it back. |
| 没有一次性会话 | 还没有 miyu ask 开过的会话 | No session opened by miyu ask yet |
| 核心断开 | 核心断开了 | The core went away |

- 英文的引号是弯引号 `“` `”`。
- 核心拒绝时说的话，照核心写的原样印（它照握手时的语言写，`protocol.md`）。
- 连上核心之前的出错（找不到数据根、拉不起核心……），照出错的原话印，只有中文（`ipc.md`、`store.md`）。

**帮助页**：`-h`、`--help`、`miyu help undo` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/undo.txt`（`miyu undo`，中文）：

```text
用法：miyu undo [选项]

撤掉最后一轮。也可以写成 miyu rewind。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/undo.txt`（`miyu undo`，英文）：

```text
Usage: miyu undo [options]

Undo the last turn. Also: miyu rewind.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

样本 `crates/miyu-cli/src/help/zh/restore.txt`（`miyu restore`，中文）：

```text
用法：miyu restore [选项]

发下一句之前，恢复最近一次撤销。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/restore.txt`（`miyu restore`，英文）：

```text
Usage: miyu restore [options]

Bring back what the latest undo took, until you say something else.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/undo/tests.rs` | 定的样子和 `docs/designs/samples/cli/undo-text.txt` 逐字节一样，蓝图里的样本块由门禁和同一份比（施工 4-9 三补）；恢复的第一行、不说怎么恢复；英文；人说的话截断；几轮；撤掉了压缩的那一句：一次、几次同一句、两种语言、在第一行下面，没有的、恢复的不印（施工 6-9）；撤掉了清空的那一句：只有它、和压缩那一句都有的（先压缩后清空），两样都有的和 `undo-clear-text.txt` 逐字节一样（施工 6-8 补）；停掉了几个任务那一句，和 `undo-jobs-text.txt` 逐字节一样（施工 7-8）；以前的核心多交的 `files`、`commands` 不印（施工 U-1） |
| `crates/miyu-cli/tests/undo.rs` | 真的核心、真的工具：撤销、恢复照样子印在标准输出上，文件一直是她改完的样子（施工 U-1）；撤完了再撤说「没有可撤销的回合。」、退出码 1；一个会话都没有、退出码 1；`--session` 撤的是指定的那个 |
| `crates/miyu/tests/undo.rs` | 真跑主程序：`undo`、`rewind`、`restore` 的 `-h` 印帮助页，跟着界面语言；`-s` 和 `--session` 一样；撤掉上一次 `miyu ask` 的那一轮、再恢复，两条命令各接各的 |
| `crates/miyu-endpoint/tests/undo.rs` | 核心交回的几样（`protocol/undo.md`） |

### 出处

- `22-命令行.md` 第三节「`miyu undo`、`miyu restore`」；第二节（结果走标准输出、不是终端不上色、退出码）。
- `04-核心协议.md` 第九节：`session.revert` 不写回合编号的写法、回应多出的几样。
- `10-自带软件.md` 第七节：撤销只撤对话，文件不改回、回执不提文件（B7，2026-10-15 项目主人改定）。第十节：路径怎么写。
- `02-内核.md` 第六节「撤销与恢复」。
- `01-架构.md` D1：该显示什么由核心算好，头只照着印。

### 还没有的

- 终端界面里的 `/undo`、`Ctrl+R`，和从某一轮起撤（`22-命令行.md` 第三节，M8）；到那时这两条命令留给脚本用。
- 印出停掉的是哪几个（编号、标题）：回应里有，现在只说几个（施工 7-8）；终端界面（M8）再定要不要列。
