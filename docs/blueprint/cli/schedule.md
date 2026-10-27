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
| `list`（不写子命令也是它） | 列出这个账号的 | `schedule.list`，`--session` 只列跑在它上面的 |
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
2. **读时刻、间隔**：`--at`、`--every` 读不懂的不连核心以后发，说清写法，退出码 2。
3. **发**：照上面的表；没给的选项不写那一格。
4. **印**（标准输出）：
   - `list`：一项一行，两个空格隔开：编号、什么时候（`cron <五段>`，记了时区、只跑一次的跟在括号里；`at <时刻>`；`every <间隔>`，间隔说成最大的整单位）、循环的接「循环」（她定节奏的「循环，她定节奏」）、停着的接「停着」、「下一次 <时刻>」、「上一次 <时刻> <结果>」、跑在哪（「会话 <编号>」，扩展建的推给它自己的「推给包 <编号>」（施工 J-4），或者「新开」接写了的人格、预设、目录）、那句话（「」括起来，换行换成空格，多于 40 个字的截掉接 `…`）。时刻照本机时区印到分钟。没有的不写。一个都没有的：「还没有定时任务。」
   - `add`：「建好了：<那一项>」；`pause`、`resume`：「停了：<那一项>」「续上了：<那一项>」；`rm`：「删掉了：<编号>」。
5. **被拒绝**：核心的原话照原样印在标准错误上，退出码 1；`bad_params` 带了 `data.problem` 的，原话（去掉句号）后面接「：」和是哪一样（下面的表）。核心断开、请求写不出去照 `miyu memory`。远程登录连上的核心拒 `cwd` 的（`local_machine_only`）照原话。

### 样子

```text
t1  cron 0 8 * * *（Asia/Tokyo）  下一次 2026-10-28 08:00  上一次 2026-10-27 08:00 跑了  新开 人格 miyu 目录 /home/me  「早报：天气和日程」
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
| 一个都没有 | 还没有定时任务。 | No schedules yet. |
| 建好了 | 建好了：{项} | Added: {item} |
| 删掉了 | 删掉了：{id} | Removed: {id} |
| 停了 | 停了：{项} | Paused: {item} |
| 续上了 | 续上了：{项} | Resumed: {item} |
| `--at` 读不懂 | 读不懂 --at {text}：写成 2026-10-28 08:00（照本机时区），或者带偏移的 2026-10-28T08:00:00+09:00。 | Can't read --at {text}: write 2026-10-28 08:00 (this machine's time zone), or with an offset like 2026-10-28T08:00:00+09:00. |
| `--every` 读不懂 | 读不懂 --every {text}：写秒数，或者 90s、10m、2h、1d。 | Can't read --every {text}: write seconds, or 90s, 10m, 2h, 1d. |
| 是哪一样（`data.problem`） | `when` 时间没写对、`cron` cron 读不懂，要标准五段、`past` 那个时刻已经过了、`short` 间隔至少 60 秒、`prompt` 话是空的、`session` 没有这个会话、`target` 会话和新开只能写一种、`zone` 不认识这个时区、`cwd` 目录不在、不是目录，或者不能用 | the time isn't written right、the cron can't be read; use the standard five fields、that time has already passed、the interval must be at least 60 seconds、the message is empty、no such session、give either a session or new-session options, not both、unknown time zone、the folder doesn't exist, isn't a folder, or can't be used |
| 循环 | 循环、循环，她定节奏 | loop、loop, her pace |
| 停着 | 停着 | paused |
| 下一次、上一次 | 下一次 {时刻}、上一次 {时刻} {结果} | next {time}、last {time} {result} |
| 上一次的结果 | `ran` 跑了、`queued` 排在后面、`sent` 推给了包、`skipped:session_gone` 跳过：会话没了、`skipped:preset_off` 跳过：预设关了它、`skipped:not_opened` 跳过：新会话开不成、`skipped:package_unavailable` 跳过：包没在跑，别的照原样 | ran、queued、sent to package、skipped: session gone、skipped: preset turned it off、skipped: new session didn't open、skipped: package not running |
| 只跑一次（`cron`） | 只跑一次 | once |
| 跑在哪 | 会话 {id}、推给包 {id}、新开 人格 {p} / 无人格 预设 {p} 目录 {dir} | session {id}、to package {id}、new persona {p} / no persona preset {p} in {dir} |

**帮助页**：`-h`、`--help`、`miyu help schedule`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/schedule.txt`（中文）：

```text
用法：miyu schedule [命令] [选项]

管定时任务：到点把一句话交给一个会话跑一轮。不写命令就是 list。
核心停着时错过的不补。

命令：
  list                    列出这个账号的定时任务
  add <话>...             建一个；时间三选一：--cron、--at、--every
  rm <编号>               删掉一个
  pause <编号>            停一个
  resume <编号>           续一个，照现在重算下一次，错过的不补

选项：
      --cron <五段>       add：标准五段的 cron，例如 '0 8 * * *'；照本机的时区读
      --once              add：--cron 的只跑一次
      --zone <时区>       add：--cron 照这个时区读，例如 Asia/Tokyo
      --at <时刻>         add：只跑一次，例如 '2026-10-28 08:00'（照本机时区），
                          或者带偏移的 2026-10-28T08:00:00+09:00
      --every <间隔>      add：每隔一阵，秒数或者 90s、10m、2h、1d，至少 60 秒
      --session <编号>    add：跑在这个已有的会话上；list：只列跑在它上面的
      --persona <人格>    add：每次新开一个会话，用这个人格
      --no-persona        add：每次新开一个无人格的会话
      --preset <预设>     add：每次新开的会话用这个预设
      --cwd <目录>        add：每次新开的会话在这个目录里开，不写是家目录
      --format text|json  list：json 给脚本
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/schedule.txt`（英文）：

```text
Usage: miyu schedule [command] [options]

Manage schedules: at the set time, a message goes to a session for one turn.
No command means list. Times missed while the core was stopped are skipped.

Commands:
  list                    List this account's schedules
  add <message>...        Add one; pick one of --cron, --at, --every
  rm <id>                 Remove one
  pause <id>              Pause one
  resume <id>             Resume one; the next time counts from now

Options:
      --cron <fields>     add: a five-field cron such as '0 8 * * *', read in
                          this machine's time zone
      --once              add: the --cron one runs only once
      --zone <zone>       add: read --cron in this time zone, e.g. Asia/Tokyo
      --at <time>         add: run once, e.g. '2026-10-28 08:00' (this
                          machine's time zone) or 2026-10-28T08:00:00+09:00
      --every <interval>  add: repeat; seconds or 90s, 10m, 2h, 1d; at least 60s
      --session <id>      add: run in this existing session; list: only those
      --persona <persona>
                          add: open a new session each time with this persona
      --no-persona        add: open a new session each time with no persona
      --preset <preset>   add: the new session uses this preset
      --cwd <dir>         add: open the new session in this folder (home if
                          not given)
      --format text|json  list: json for scripts
  -h, --help              Print help
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
