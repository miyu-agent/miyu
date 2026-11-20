## 施工单 FX-2：`@` 文件列表大目录里先收浅处的、时间往回拨时循环不再丢、一个测试的竞态

状态：做完，等合（分支 `step/FX-1` 上）。来源：LiteMiyu 会话 2026-11-12 转来（LiteMiyu 的这几处代码是从 Miyu 原样搬过去的），三处先在 Miyu 上证实再修。编号 FX-1 已被「太长的一行只拒这一行」占用，顺延成 FX-2。

### 目的

1. `@` 文件列表在大目录里找得到浅处的文件。
2. 系统时间往回拨（双系统、虚拟机回快照、NTP 校正）以后，定时任务、长期目标的续轮不再被内核当成收过的、悄悄丢掉。
3. `miyu-sandbox` 的 `the_watcher_holds_nothing_but_the_lifeline` 不再偶发红。

### 蓝图改哪几节

- `fs.md` 第七节「模糊找的清单」第 1 条（收满了按层重走）、第 4 条（同一句只报一次）、「守着它的」一行。
- `schedules.md` 第三节第 4、5 条（命令编号的写法）、第八节测试表一行。
- `goal.md`「在新的架构里怎么放」第 7 条（续轮编号的写法）、「守着它的」一行。

### 一、`@` 文件列表漏掉浅处的文件

**证实**：`crates/miyu-fs/src/find.rs` 的 `walk` 深度优先、照 readdir 的先后收，收满 `cap`（出厂 20000）就停：先走到的深子目录把清单占满，浅处的一个没收，框上只说「仅显示部分结果」。新测试 `find::tests::a_full_index_keeps_the_shallow_entries_before_the_deep_ones`：30 个子目录各自第四层 90 个文件（单拿哪一个都比上限多）、浅处只有 `src/main.rs`，上限照第二层以内的条数加第三层的一半。改前红（收到的全是一个深目录里的，`src/main.rs` 不在；只有 `src` 恰好排在 readdir 最前面才过，约 1/31）。

**修法**：没收满的和以前一样，深度优先一遍走完。收满了的清掉，按层加深重走：第 1 层、第 2 层……第 `DEPTH` 层，每一遍 `max_depth` 到那一层、只收正好在那一层的，收满就停；某一层一个目录都没有就不再往下。上限、内存不变，只在收满时多花几遍读浅处目录的工夫。重走时清单先空再长，`done` 还是 `false`，读的一方看到的照旧是「没建完」的那部分。按层走时读不了的目录每一遍都会再碰到，`on_error` 同一句只收一次（新测试 `a_rewalk_reports_each_unreadable_directory_once`）。没选广度优先：`ignore` 库只有深度优先的走法，自己写队列要重做 `.gitignore` 的继承。

### 二、时间往回拨时的命令编号

**证实**：

- 定时任务交进会话的命令编号是 `schedule/<任务>/<到点的毫秒>/say`。Miyu 里这个毫秒是表里记的到点时刻，不是会话时钟：固定间隔的循环照上一次往后数，严格往后走，所以 LiteMiyu 那种「固定间隔的循环第二次起全丢」在 Miyu 上不成立（Miyu 没有 `loop/stopped/<毫秒>` 这一行）。但到点时刻会重复：cron 照现在算下一次，系统时间往回拨以后人停了再续（或者改了时间），下一次落在拨回前跑过的那一刻，编号和那一次一样，内核照重发回上一次（换了话的拒 `id_reused`），不开轮。新测试 `schedule_clock_back::the_same_moment_due_again_after_the_clock_went_back_still_speaks`（手拨的钟、`*/10 * * * *`、拨回十分钟、停了再续）改前红：第二轮六十秒内等不到。
- 长期目标续轮是 `goal-<第几轮>-<会话时钟的毫秒>`。会话时钟不往回走，系统时间比日志里最后一条早时一直停在那一毫秒；清掉一个目标再设一个，新目标的第一轮和上一个的第一轮编号一样。新测试 `goal_rounds::a_new_goal_runs_its_first_round_while_the_clock_stands_still`（照 `a_loaded_session_never_goes_back_in_time` 把日志挪到 2100 年再载入）改前红：新目标的第一轮六十秒内没开。

**修法**：编号里的毫秒换成每次新造的 UUIDv7，用会话编号的造法 `miyu_session::new_id`（一个核心进程共用一份计数器：同一毫秒、系统时间往回拨都接着往上数；进程之间靠随机位），同 `projects.rs` 造项目编号。

- 定时：`schedule/<任务>/<UUIDv7>/say`，每次新开会话的 `…/open` 和它同一个 UUIDv7。原来想的「核心重启后再交一遍，会话认得是重发」用不上：先写下一次再交，重启以后不会再交同一次。
- 续轮：`goal-<第几轮>-<UUIDv7>`。`round_command(round, unique)` 造，`round_of` 认的时候后一段照会话编号的写法查（不认的写法照旧没有）。头只看前缀 `goal-` 和第几轮，形状不变。
- 跨会话的「等它闲下来」通知 `…/idle/<等的会话>/<起算毫秒>` 也带会话时钟的毫秒，但它是故意的：同一次订再交一遍要认得是重的（`cross-session.md` 第 53 条），这一步不动，另行再看。

### 三、lifeline 测试的竞态

**证实**：测试先列 `/proc/<pid>/fd`，再逐个 `read_link`；看门的刚起来那一阵自己开了又关 3 号描述符，读到的时候已经没了，`expect("读得了")` 当场红。LiteMiyu 在 .91 上加压单跑 3200 遍红 131 遍。Miyu 这台机器上改前没复现：`taskset` 挤一个核（同核另起一个空转进程）单跑 600 遍、八个核各单跑 400 遍、十二个核各配一个空转进程各跑 300 遍，一共 7400 遍 0 红。代码一样、读法一样，空当是真的，照 LiteMiyu 的证据修。

**修法**：这一遍有一个读不到（`NotFound`）就整遍不算，过 20 毫秒重看一遍，和原来等它起好的那一圈共用五秒的上限；最后的断言不变。

### 不做什么

- `@` 文件列表的上限、层数、跳过名单不动；不改协议端点记几份清单。
- 不改跨会话通知的编号（见上）。
- 不写旧版兼容：以前写进日志的 `goal-<轮>-<毫秒>` 不再认作续轮（没发版）。

### 验收

1. `cargo test -p miyu-fs --lib find::`：两条新测试过；旧的照过。
2. `cargo test -p miyu-endpoint --test all schedule_clock_back`、`cargo test -p miyu-session --test all goal_rounds`、`cargo test -p miyu-kernel --lib event::goal`：拨回去以后两次都交进去、编号不同；新目标第一轮照开；编号读写一致、别的写法认不出。
3. lifeline：`taskset -c 0` 挤一个核循环单跑 `the_watcher_holds_nothing_but_the_lifeline`，改后不红。
4. `miyu-fs`、`miyu-kernel`、`miyu-session`、`miyu-endpoint`、`miyu-sandbox`、`miyu-basesystem`、`miyu` 全套测试；clippy（含 Windows 目标）、`fmt --check`、`doc -D warnings`、`xtask prompts`。

### 风险

- 收满的大目录要多走几遍浅处：只在收满时、最多 `DEPTH` 遍，都是读目录，不读文件内容。
- 续轮编号变长（36 位）：命令编号上限 128 字节，`goal-<轮>-` 加 36 位远在上限内；定时的任务编号是 `t<n>` 一类的短名。
