## `miyu schedule`

### 是什么

在 shell 里管定时任务（施工 J-3 下，设计 45 第三节第 3 条、第五节第 3 条，`schedules.md`）：列、建、删、停、续。连上核心（没在跑就拉起来），走 `schedule.*`（`docs/reference/schedules.md`），不另起一套。建 `cron` 的自动交本机的时区（2026-10-27 项目主人定：建时自动记下，可以手写覆盖）；时刻照本机时区印。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `schedule`；换上帮助页 |
| `crates/miyu-cli/src/schedule.rs` | 参数、找数据根、连核心、握手、照子命令发请求、`add` 交的参数、退出码 |
| `crates/miyu-cli/src/schedule/shown.rs` | 一项一行印成什么样，`--at`、`--every` 怎么读（纯的，时区由调的一方交） |
| `crates/miyu-cli/src/language/schedule.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/schedule.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出全部 | `schedule.list`，`--session` 只列跑在它上面的 |
| `add <话>...` | 建一个；几个词用一个空格连起来 | `schedule.add` |
| `rm <编号>` | 删掉 | `schedule.cancel` |
| `pause <编号>` | 停 | `schedule.pause` |
| `resume <编号>` | 续，照现在重算下一次 | `schedule.resume` |

| 选项 | 做什么 |
|---|---|
| `--cron <五段>` | `add`：`when.cron`；不写 `--zone` 的交本机的 IANA 时区名（`jiff` 的 `TimeZone::try_system()`），本机说不出名字的不交，照核心的默认 |
| `--once` | `add`：`--cron` 的 `recurring` 是 `false` |
| `--zone <时区>` | `add`：`--cron` 照这个时区读 |
| `--at <时刻>` | `add`：一次。带偏移的 RFC 3339 照它；不带偏移的（`2026-10-28 08:00`、`2026-10-28T08:00`）照本机时区读；换成核心认的 `2026-10-27T23:00:00.000Z` 再交。已经过了的不另查，核心回 `past` |
| `--every <间隔>` | `add`：秒数，或者数字接 `s`、`m`、`h`、`d` |
| `--session <编号>` | `add`：`target.session`；`list`：只列跑在它上面的 |
| `--persona <人格>`、`--no-persona` | `add`：每次新开，`target.new.persona` 写它、写 `null`（无人格）；都不写的不交，照核心的默认 |
| `--preset <预设>` | `add`：`target.new.preset` |
| `--cwd <目录>` | `add`：`target.new.cwd`：相对的照当前目录接，在的换成真实的位置，`~` 开头的照原样 |
| `--format text\|json` | `list`：`json` 原样印 `schedules` 那一组 |

时间三选一、`--once`、`--zone` 只跟 `--cron`、`--session` 不和新开的几样一起写、`--persona` 不和 `--no-persona` 一起写，都由 clap 管，写错的退出码 2。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu memory`（`cli/memory.md` 第 1 步）。请求的编号是 `schedule-<16 位十六进制>-<序号>`。
2. **读时刻、间隔**：`--at`、`--every` 读不懂的不发，报「无法识别时间 '<写的>'」「无法识别间隔 '<写的>'」，另起一行提示写法，退出码 2。
3. **发**：照上面的表；没给的选项不写那一格。
4. **印**（标准输出）：
   - `list`：一项一行，两个空格隔开：编号、什么时候（`cron <五段>`，记了时区、只跑一次的跟在括号里；`at <时刻>`；`every <间隔>`，间隔说成最大的整单位）、循环的接「循环」（自己定节奏的「循环，节奏自定」）、停着的接「停着」、「下一次 <时刻>」、「上一次 <时刻> <结果>」、跑在哪（「会话 <编号>」，扩展建的推给它自己的「发给软件包 <编号>」（施工 J-4），或者「新会话」接写了的人格、预设、目录）、那句话（「」括起来，换行换成空格，多于 40 个字的截掉接 `…`）。时刻照本机时区印到分钟。没有的不写。一个都没有的：「没有定时任务」
   - `add`：「已添加 <那一项>」；`pause`、`resume`：「已暂停 <那一项>」「已恢复 <那一项>」；`rm`：「已删除 <编号>」。
5. **被拒绝**：核心的原话照原样印在标准错误上，退出码 1；`bad_params` 带了 `data.problem` 的，原话（去掉句号）后面接「：」和是哪一样（下面的表）。核心断开、请求写不出去照 `miyu memory`。远程登录连上的核心拒 `cwd` 的（`local_machine_only`）照原话。

### 样子

```text
t1  cron 0 8 * * *（Asia/Tokyo）  下一次 2026-10-28 08:00  上一次 2026-10-27 08:00 已运行  新会话 人格 miyu 目录 /home/me  「早报：天气和日程」
t2  every 10m  停着  会话 s-1  「看一眼服务器」
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了，列一个都没有的也是 |
| 1 | 找不到数据根、连不上核心；被拒绝；核心断开 |
| 2 | 参数不对：子命令写错、少了编号、话，时间没写或写了两种，`--once`、`--zone` 跟了别的，`--format` 写错（`cli/main.md`）；`--at`、`--every` 读不懂 |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 一个都没有 | 没有定时任务 | no schedules |
| 建好了 | 已添加 {项} | added {item} |
| 删掉了 | 已删除 {id} | removed {id} |
| 停了 | 已暂停 {项} | paused {item} |
| 续上了 | 已恢复 {项} | resumed {item} |
| `--at` 读不懂（报错） | 无法识别时间 '{text}'；提示：写成 '2026-10-28 08:00'（本机时区）或 2026-10-28T08:00:00+09:00 | invalid time '{text}'; hint: use '2026-10-28 08:00' (local time) or 2026-10-28T08:00:00+09:00 |
| `--every` 读不懂（报错） | 无法识别间隔 '{text}'；提示：写秒数，或 90s、10m、2h、1d | invalid interval '{text}'; hint: use seconds, or 90s, 10m, 2h, 1d |
| 是哪一样（`data.problem`） | `when` 时间格式不对、`cron` cron 表达式无效，需要标准五段、`past` 时间已经过去、`short` 间隔至少 60 秒、`prompt` 消息为空、`session` 会话不存在、`target` 会话和新建会话的选项只能用一种、`zone` 未知时区、`cwd` 目录不存在或不可用 | invalid time、invalid cron expression, use the standard five fields、time already passed、interval must be at least 60 seconds、empty message、no such session、use either a session or new-session options, not both、unknown time zone、directory missing or unusable |
| 循环 | 循环、循环，节奏自定 | loop、loop, self-paced |
| 停着 | 停着 | paused |
| 下一次、上一次 | 下一次 {时刻}、上一次 {时刻} {结果} | next {time}、last {time} {result} |
| 上一次的结果 | `ran` 已运行、`queued` 已排队、`sent` 已发给软件包、`skipped:session_gone` 跳过：会话已不存在、`skipped:preset_off` 跳过：预设已关闭、`skipped:not_opened` 跳过：无法新建会话、`skipped:package_unavailable` 跳过：软件包未运行，别的照原样 | ran、queued、sent to package、skipped: session gone、skipped: disabled by preset、skipped: new session failed to open、skipped: package not running |
| 只跑一次（`cron`） | 只运行一次 | once |
| 跑在哪 | 会话 {id}、发给软件包 {id}、新会话 人格 {p} / 无人格 预设 {p} 目录 {dir} | session {id}、to package {id}、new persona {p} / no persona preset {p} in {dir} |

报错、警告照设计 47 印在标准错误上：`miyu: <事>`（警告是 `miyu: 警告：`，英文 `miyu: warning: `），建议另起一行 `提示：`（英文 `hint: `）；下表报错那几行写的是 `miyu: ` 后面那一截。

**帮助页**：`-h`、`--help`、`miyu help schedule`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/schedule.txt`（中文）：

```text
用法：miyu schedule <命令> [<选项>]

不写命令时运行 list。到点时把一条消息交给会话运行一轮；核心停止期间错过的不补。

命令：
  list           列出定时任务
  add <消息>...  添加定时任务（时间用 --cron --at --every 之一）
  rm <编号>      删除定时任务
  pause <编号>   暂停定时任务
  resume <编号>  恢复定时任务，从现在起重新计算下一次

选项：
      --cron <五段>         按 cron 表达式运行，例如 '0 8 * * *'
      --once                cron 只运行一次
      --zone <时区>         cron 使用的时区，例如 Asia/Tokyo（默认本机时区）
      --at <时刻>           在指定时刻运行一次，例如 '2026-10-28 08:00'
                            （本机时区）或 2026-10-28T08:00:00+09:00
      --every <间隔>        每隔一段时间运行，秒数或 90s 10m 2h 1d，至少 60 秒
      --session <编号>      在这个会话上运行；list 时只列出它的
      --persona <人格>      每次新建会话，使用这个人格
      --no-persona          每次新建无人格的会话
      --preset <预设>       新建的会话使用这个预设
      --cwd <目录>          新建的会话的工作目录（默认家目录）
      --format <text|json>  输出格式（默认 text）
  -h, --help                显示帮助
```

样本 `crates/miyu-cli/src/help/en/schedule.txt`（英文）：

```text
Usage: miyu schedule <command> [<options>]

Runs list when no command is given. At the set time, a message goes to a
session for one turn. Times missed while the core is stopped are skipped.

Commands:
  list              List schedules
  add <message>...  Add a schedule (time: one of --cron, --at, --every)
  rm <id>           Remove a schedule
  pause <id>        Pause a schedule
  resume <id>       Resume a schedule; the next run counts from now

Options:
      --cron <fields>       Run on a five-field cron, e.g. '0 8 * * *'
      --once                Run the cron only once
      --zone <zone>         Time zone for the cron, e.g. Asia/Tokyo (default:
                            this machine's)
      --at <time>           Run once at a time, e.g. '2026-10-28 08:00' (local
                            time) or 2026-10-28T08:00:00+09:00
      --every <interval>    Repeat every interval: seconds or 90s, 10m, 2h, 1d;
                            at least 60s
      --session <id>        Run in this existing session; with list, show only
                            its schedules
      --persona <persona>
                            Open a new session each time with this persona
      --no-persona          Open a new session each time with no persona
      --preset <preset>     Use this preset for the new session
      --cwd <dir>           Working directory for the new session (default:
                            home)
      --format <text|json>  Output format (default: text)
  -h, --help                Print help
```

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/schedule/tests.rs` | 一项一行的样子（时区照测试交的）、长的话截掉；`--every` 的几种写法和读不懂的；`--at` 不带偏移照本机时区、带偏移照它、换成核心认的写法；`cron` 交本机的时区、`--zone` 手写的照它、本机说不出名字的不交；`--at`、`--every`、目标交的参数；不能一起写的几样 clap 拦下 |
| `crates/miyu-cli/tests/schedule.rs` | 真核心：建、列（和建好时印的一样）、`json`、停（没有下一次）、续、删；`cron` 记下东京、纽约；`--cwd` 相对的换成真实的位置；隔太短、已经过了、不认识的时区照 `problem` 说清，没有的编号照原话，读不懂的间隔退出码 2，拒了的都没建 |
| `crates/miyu-cli/src/help/tests.rs` | 帮助页列的选项和程序真有的一一对上、80 列以内 |

### 出处

- 设计 `45-定时和循环.md` 第三节第 3 条、第五节第 3 条；施工 J-3（`docs/construction/J-3-定时的时区上限和通知.md` 第五节）。

### 还没有的

- 改一项（换时间、换话）：协议没有 `schedule.update`，先删了重建。
