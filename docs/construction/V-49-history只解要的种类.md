## 施工单 V-49：history 只解要的种类

状态：已完成（2026-11-05）。照 V-45 施工单「不做什么」第二条、V-39 上补的做法做。**2026-11-05 项目主人批：history 也只解要的种类，同 V-39 上补口径**——不要的种类正文合 JSON、只是不合那一种写法的，不报坏、照读；外壳坏、要的种类坏，照旧报一样的错。

### 为什么

V-45 以后，`history` 经 `ReadLog::read_range` 流式读、交给 `Sweep`、边挑边扔，内存不再随日志长；可指令几乎没动：V-40 的大私聊会话（101.7k 条、88.6 MB）一次「读」那一轮 28.7 亿指令，剩下的几乎都是把整份日志每一行完整 JSON 解一遍。这份日志里 `model.called` 占两成字节（29,290 条、18.0 MB），`history` 不列它，只有有效历史撤销时看它的 `seen`、`error` 几格；注入的事实、会话的标题这些 `history` 也不看。

### 项目主人拍板的

1. 2026-11-05：`history` 也只解要的种类，接受和 V-39 上补同一口径的行为差别。
2. 除这一处外交回给她的字一律不变。

### 照的图纸

- V-45 施工单「不做什么」第二条；V-39 上补施工单（「要解、不解的种类，和为什么」、内核 `event/only.rs`、存储 `read_range_only`）；V-43 施工单「风险」第二条（工具结果里带 `job.started` 这类会影响有效历史的效果要全解）。
- `docs/blueprint/tools/history.md`「哪些算一条」「怎么走」第 3 条；`kernel/events.md` 的 `Event::from_line_only`；`store.md` 第 7 条 `read_range_only`；`tools/interface.md` 的 `log`。

### 要解、不解的种类，和为什么

逐个从代码里推（`history/collect.rs`、`entry.rs`，内核 `history.rs` 的 `note`、`append`、`is_peer`、`history/jobs.rs`、`history/undo.rs`，`miyu-store` 的 `sweep.rs`、`sweep/bones.rs`）：

| 种类 | 解多深 | 谁看、看什么 |
|---|---|---|
| `message.assistant` | 全解 | 算一条、写原文（正文、工具调用、图、文件的占位，`entry::text_of`）；她自己翻记录的调用照回复里 `history` 调用的编号认（`not_a_lookup`、整份读的 `own_calls`）；有效历史撤销时看 `seen`、`interrupted` |
| `tool.result` | 全解 | 算一条、写原文；翻记录的那次结果照调用编号认、不算；**派出去的任务（效果 `job.started`）照它记**，认别的会话（`History::note` → `jobs.rs`，`is_peer`）——工具结果本来就要显示，所以派任务的效果跟着全解，V-43 风险提示的那一条在这里自动满足 |
| `session.created` | 全解 | 父会话（`jobs.rs` 的 `parent`）：父会话发来的话「谁」写 `user`，不当别的会话（`is_peer`） |
| `session.policy_changed` | 全解 | 人切权限级别算一条（`entry::switched`，看 `permission`）；不经有效历史 |
| `message.user`、`context.compacted` | 全解（内核 `HISTORY` 本来就全解） | 算一条（人话、以前的摘要）、「谁」那一格（`venue`、`by`）、显示的时刻（`venue.sent`）；有效历史撤回、撤销、压缩照它们 |
| `turn.started`、`turn.ended`、`turn.reverted`、`turn.unreverted`、`message.withdrawn`、`job.reported`、`job.lines`、`child.reported`、`peer.idle` | 全解（同上） | 有效历史照种类认的（撤销、恢复、撤回、回合的开头结尾、撤销认上一轮时看触发是哪一种）；`Sweep` 的骨架（`bones.rs`）看的也只有这几种；`jobs.rs` 照撤销、恢复标派的任务撤没撤（`is_peer` 不看这一格） |
| `model.called` | 只解 `seen`、`messages`、`result`、`error`（内核 `SEEN`） | 有效历史撤销查上一轮排着的话：看到了第几条，暂停着没发出去的（`error.class` 是 `compaction_paused`）不算看到；`history` 不列它 |
| 别的（`context.injected`、`session.meta_changed`、`command.ran`、审批、提问、目标、`todos.cleared`、`package.state`、`files.restored`、`shell.ran`、不认识的 `ext.*`……） | 不解 | `history` 不列、不看（「哪些算一条」）；有效历史只认种类、回合、序号（撤销按 `turn` 拿走一整轮，外壳全解了）；待办、包的状态（`History::note` 也记）`history` 用不上；`body` 只过一遍（是合法的 JSON 就行），读成不认识的种类、照先后留在有效历史里，挑的时候本来就跳过 |

前四种写在 `collect.rs` 的 `ONLY`；人话这些有效历史照种类认的、`model.called` 只解几格，照 `Event::from_line_only` 自己的规矩（内核 `event/only.rs` 的 `HISTORY`、`SEEN`），这里不另写。挨着的规矩写在 `ONLY` 的注释里：

- `entry.rs` 多列了别的种类、多看了别的格，`History::note` 认别的会话多看了别的种类、别的效果（现在是 `session.created` 和工具结果的 `job.started`），`ONLY` 要跟着加；
- 有效历史多认了种类、多看了 `model.called`、`message.assistant` 的别的格，改内核的 `HISTORY`、`SEEN`（同判官、撤销的回应、记忆抽取），`history` 跟着就对了，不用动 `ONLY`。

### 做什么

1. **只读入口**（`miyu-tool` 的 `log.rs`）：`ReadLog::read_range_only(从第几条, 到第几条, 要的种类, 每一块)`，`Log` 跟着包一个。默认照 `read_range` 全解（测试里的假日志）：全解读得成的，只解要的种类也读得成，交出去的要的种类一样。
2. **照会话目录读的两份**：这个会话自己的（`miyu-session` 的 `store.rs` `LogDir`）、读别的会话的（`miyu-endpoint` 的 `spawn.rs` `Dir`，`history` 带 `session` 走它）都照存储的 `read_range_only` 读，自检、报的错同 `read_range`。读别的会话那条路一起改了。
3. **`history`**（`collect.rs`）：头一遍、放早了再读一遍、骨架认不得或调用编号对不上的整份读，三处都改成 `read_range_only(…, ONLY, …)`。挑、筛、写一页的代码一行没动。
4. 测试、蓝图（下面）。

### 不做什么

- 不碰内核的撤销逻辑、`event/only.rs` 的表、存储的读法（V-39 上补的原样用），不另造一套解法。
- 回复、工具结果照旧全解：要显示，又要认翻记录的调用和派出去的任务。「读」满了以后后面的回复、工具结果其实只要调用编号和任务效果，可以只解那几格，再省回复、工具结果正文的解析；要另写一种解深，这一步不做（见「实测」）。
- 不落盘、不加缓存、不常驻；不碰 im、`miyu-sandbox`、`root.rs`、`config_file.rs`、`cache.rs`。

### 行为差别

交回给她的字（`Done` 的每一格：正文、出没出错、给人看的那一句）逐字节一样：读、找、翻页、范围、时刻、谁说的、一页几条、撤销撤回过的、分叉的、子会话、别的会话、空会话、越界。

只有一处，就是 V-39 上补的口径（2026-11-05 项目主人批）：不要的种类（上表最后两行）的正文合 JSON、只是不合那一种的写法的（外人改过的日志才有，内核写不出来），以前报 `Could not read the log: <段> line <行> is broken`，现在照读，交回的同那一行没坏时。`model.called` 只解的那几格坏了的照报。外壳坏了、正文不是合法 JSON、要的种类（上表前七行）坏了的照报，交回的一字不差，存储报的为什么也一字不差（读不成的照全解再读一遍、报它的错）。下次打开这个会话照样整份自检、报出来。

### 验收

1. **对照测试**（`miyu-basesystem` 的 `history/tests/same.rs`，V-45 的准绳 `same/old.rs` 照旧全解）：新的读法每一行只解要的种类。切成几段交的那一样改成原文行（`same/only.rs` 的 `Lines`：以前的读法照它全解，新的照 `Event::from_line_only` 解），写成真段文件的经存储的 `read_range_only`。生成器造的不认识的种类（`ext.note`）四回里三回换成 `history` 不要的各种（`same/only.rs` 的 `vary`：会话的标题、命令、审批、提问、目标、清空待办、包的状态、压缩暂停、`shell.ran`、回顾，暂停着没发出去的请求），合写法的，对照测试照它比。
2. **坏行**（`same/only.rs` 的 `only_unwanted_bodies_that_are_still_json_read_as_if_unbroken`）：照种子造会话（同上的生成器、加 `vary`），写成一到四段真文件，各写坏一行三样：
   - 不要的种类：整个正文写成 `7`，`model.called` 坏在不解的 `endpoint`：以前的读法报读不了，新的交回的同那一行没坏时以前的读法交的；
   - 外壳：序号写成字、时刻不对、种类写成数、没有 `body`、两个 `body`、写到一半：交回的一字不差，存储报的错（哪一段、第几行、为什么）`read_range_only` 和 `read_segments` 一字不差；
   - 要的种类：人话、回复、工具结果、造会话、切权限、摘要、回合、撤销、恢复、撤回、回报的正文写成 `7`，回复的 `seen` 写成 0，`model.called` 撤销看的 `seen` 写成字（三回里一回专挑请求）：同上。
   每样三种参数（`{}`、`{"query":"omega"}`、从坏行前面几条读 7 条）、两种放法（调小的、出厂的）。
3. **只读入口**：`miyu-session` 的 `store/tests/only.rs`、`miyu-endpoint` 的 `spawn/tests.rs`：注入的事实正文写成 `7` 的，`read_range_only` 照读、读成不认识的种类，`read_range` 报坏了；人话正文坏了、两个 `body` 的，两样报的一字不差。
4. **种子数和时长**：平时对照测试 12 份、坏行 12 份，Linux debug 3.8 + 1.8 秒（对照测试原来照预先解好的事件拷，改成原文行以后两种解法各解一遍记着、照记着的拷，同 V-45 的 4 秒上下）；Windows debug 照慢三倍估二十秒以内（Windows 的 check 一趟已近 50 分钟、上限 60）。`MIYU_HISTORY_FULL=1`：对照测试 3,000 份、坏行 1,500 份，见下面「全量」。
5. **手写变异** 11 个都被逮住（`scratchpad/v49/mutants.py`）：

| 变异 | 逮住它的 |
|---|---|
| `ONLY` 少解工具结果 | 对照、坏行 |
| `ONLY` 少解造会话（父会话发来的话当别的会话） | 对照、坏行 |
| `ONLY` 少解切权限 | 对照、坏行 |
| `ONLY` 少解回复 | 对照、坏行 |
| 头一遍改回全解 | 坏行 |
| 放早了再读一遍改回全解 | 坏行 |
| 整份读改回全解 | 坏行 |
| 内核读不成的不照全解再读（报的为什么不一样） | 坏行（存储报的错比；交回的那一句只写哪一段第几行，看不出来） |
| 内核 `model.called` 不解 `seen`（多放过请求撤销看的那格坏了的） | 坏行 |
| 这个会话自己那份（`LogDir`）照全解读 | `store/tests/only.rs` |
| 读别的会话那份（`Dir`）照全解读 | `spawn/tests.rs` |

6. **真数据**：V-45 的量尺和数据，改前改后交回的字的 sha 一样（下面「实测」）。
7. 原有的测试照过（`miyu-tool`、`miyu-basesystem`、`miyu-session`、`miyu-endpoint`、`miyu-store`、`miyu-kernel`）；`cargo xtask check`（远程门禁），Windows 版 clippy，三平台 CI。

### 全量

2026-11-05，debug，`MIYU_HISTORY_FULL=1`（`scratchpad/v49/full-debug.txt`、`full-broken.txt`）：

- 对照测试 3,000 份：比了 81,138 次都一样；两种放法各 3,000 份里收齐 3,902、再读一遍 1,684、调用编号对不上 390、骨架认不得 24，四条路都走到。1,022 秒。
- 坏行 1,500 份：13,485 样（每样两种放法），不要的种类 4,485 样（以前的读法都报读不了，新的都同没坏时）、外壳 4,500 样、要的种类 4,500 样（都报读不了，交回的、存储报的一字不差）。302 秒。

### 实测

2026-11-05，本机（Ryzen 9 7940H），release 核心（改前 origin/main `d8e4d51b7`，V-47；改后这一步，同一个工作树先后编），断网命名空间（`unshare -rn`），核心绑 12–15 号核。量法、数据同 V-45（`scratchpad/v49/` 的 `hist.sh`、`batch.sh`、`summary.py`，照 `v45/` 只换了核心和结果目录）：V-40 的大私聊会话（102,372 条、88.6 MB、两段，FH-3 以后的布局）每遍新拷数据根，像网页那样打开、等 3 秒，说一句，假模型调一次 `history`，量到 `turn.ended` 为止的那一轮（含两次请求）。四样参数：第一页 `{}`、很靠前的一页 `{from: 3000}`、快到最后的一页 `{from: 101500}`、关键词 `{query: omega}`。每样每边 4 遍，第 0 遍预热不算，写 3 遍的中位，括号里是范围。指令、task-clock 是 `perf stat` 套那一轮（用户态）；匿名内存每 2 ms 读一次 `RssAnon`。每一遍量完从会话日志里取这次 `history` 的结果算 sha：四样改前改后各 4 遍都一样，也和 V-45 的一样（第一页 `2422e5c2`、很靠前 `72c5ba42`、快到最后 `fb0f5638`、关键词 `ebfa4b53`，给人看的那一句也一样）。原始数据在 `scratchpad/v49/m-*/`。

| | 改前 | 改后 |
|---|---|---|
| 第一页：用户态指令 | 28.7 亿 | 26.5 亿（−7.7%） |
| 　墙钟 | 121.9 ms（121～285） | 115.6 ms（114～133） |
| 　task-clock | 301 ms（299～491） | 271 ms（269～282） |
| 　匿名多 | 18.6 MB | 18.4 MB |
| 很靠前的一页：指令 | 28.7 亿 | 26.5 亿（−7.7%） |
| 　墙钟 | 126.8 ms（122～131） | 121.6 ms（115～126） |
| 　task-clock | 297 ms | 276 ms |
| 　匿名多 | 19.4 MB | 18.8 MB |
| 快到最后的一页：指令 | 28.7 亿 | 26.5 亿（−7.7%） |
| 　墙钟 | 133.0 ms（129～135） | 124.7 ms（109～125） |
| 　task-clock | 289 ms | 264 ms |
| 　匿名多 | 19.4 MB | 18.6 MB |
| 关键词：指令 | 48.3 亿 | 46.1 亿（−4.6%） |
| 　墙钟 | 257.2 ms（251～258） | 251.3 ms（250～252） |
| 　task-clock | 504 ms | 473 ms |
| 　匿名多 | 19.3 MB | 18.9 MB |

（改前的墙钟比 V-45 量的 168 ms 短，机器这次闲；只看改前改后的比。）

每一样都少 2.2 亿指令，差不多全是 `model.called` 那两成不再建 `usage`、`cost`、块的起止、端点、模型这些（分配、释放、时刻和数的解析）；不要的正文照旧要过一遍分词（`IgnoredAny`，同 V-39 上补「和设计不一样的」那一条），所以省的不到两成。task-clock 少 7～10%（解析在存储那几个线程里，省下的照样省）。匿名内存不变（一窗原文、几个线程解的块、有效历史留着的最后一截，同 V-45）。挪出去的总账：没有挪到别的线程、进程，不落盘、不加常驻。

剩下的大头是回复（36%）、工具结果（39%）的全解：「读」的那一页满了以后，后面的回复、工具结果只要调用编号和派任务的效果；只解那几格还能再省，可要给 `from_line_only` 另加一种解深、改内核，这一步不做（「不做什么」第二条）。

### 风险

- `history` 以后多列了别的种类、多看了别的格（`entry.rs`），或者认别的会话多看了别的种类、别的效果（`History::note`、`jobs.rs`），`ONLY` 不跟着加的话那一种读成不认识的种类、显示不出来或者认错：`ONLY` 的注释写了，对照测试、手写变异逮得到生成器造得出的那几种。
- 有效历史以后多认了别的种类、多看了别的格：改内核 `event/only.rs` 的 `HISTORY`、`SEEN`（同判官、撤销的回应、记忆抽取）。工具结果在 `history` 里本来就全解，以后工具结果多了别的影响有效历史的效果也不用另认字（V-43 那条风险在这里不成立）。
- 不要的种类的正文被改得不合写法、仍是合法 JSON 的，`history` 不报（主人接受的代价），下次打开会话整份自检照报。

### 蓝图改哪几节

- `tools/history.md`：状态一行；「在哪」`collect.rs`、`log.rs`、`log/open.rs`、`spawn.rs` 几行，加内核 `event/only.rs` 一行；「交给工具的」`log`；「怎么走」第 3 条加只解要的种类和那一处行为差别；「守着它的」`same.rs` 一行，加 `same/only.rs`、`store/tests/only.rs`、`spawn/tests.rs` 两行。
- `store.md`：第 7 条 `read_segments`（V-49 起 `history` 用 `read_range_only`）、`read_range_only` 用它的几处加 `history`。
- `kernel/events.md`：`Event::from_line_only` 加 `history` 也用。
- `tools/interface.md`：`log` 那一格加 `read_range_only`。
