## 人生会话

状态：LF-5 中施工中（2026-11-14）：开一条、问状态、暂停、继续，出厂预设「推演人生」。LF-4 上施工中（2026-11-14）：结束（`life.ended`、`life.kill`、`life.new`、清理）。LF-7 施工中（2026-11-14）：别的会话知道她现在在干嘛（下面「现在在干嘛」）。LF-4 下施工中（2026-11-14）：她自己的工具 `end_life` 和带它的内置包 `base-life`（`tools/end_life.md`）。LF-5 下施工中（2026-11-14）：核心命令 `/life`（「命令」）。头的开关是 LF-8。

### 是什么

给一个人格开一条一直跑循环的会话（设计 `70-推演人生.md`）：她自己定下次几点醒，醒来过一段自己的日子，记下的进人格记忆。一个人格一条。它就是一个普通会话：用出厂的预设 `life` 开，醒来靠定时表里人生那一行（`loop` 是 `life`，`schedules.md`），醒来那段话是 LF-2 的三层（`schedules.md` 第三节第 4 条）。这一页写怎么开、怎么认、怎么停和续。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/life.rs` | `life.start`、`life.status`、`life.pause`、`life.resume`、`life.kill`、`life.new`：查人格、照索引找这个人格的人生会话、开会话、设上下文上限、记身份、建和停续人生那一行 |
| `crates/miyu-endpoint/src/commands/life.rs`（施工 LF-5 下） | 命令 `/life`：认参数、照会话的人格调 `life.rs` 的 `act`、拼回执、回应带 `session` |
| `crates/miyu-endpoint/src/life/finish.rs`（施工 LF-4 上） | `finish`：结束一段人生的六步（「结束」），`life.kill`、`life.new` 和她的 `end_life`（LF-4 下）共用 |
| `crates/miyu-endpoint/src/life/port.rs`（施工 LF-4 下） | 交给 `end_life` 的端口 `LifePort`：查是不是没结束的人生会话、调 `finish`、不等清理；随定时端口交进会话（`schedules/port.rs`） |
| `crates/miyu-schedule/src/life.rs`、`resources/software/life/`、`resources/packages/base-life/`（施工 LF-4 下） | 工具 `end_life` 和带它的内置包（`tools/end_life.md`） |
| `crates/miyu-kernel/src/ledger/seal.rs` 的 `life_started`、`session/seal.rs` 的 `Session::living`（施工 LF-4 下） | 是不是没结束的人生会话：只给人生会话的包（`needs = ["life"]`）照它给不给（`presets.md`） |
| `crates/miyu-kernel/src/ledger/seal.rs`、`session/seal.rs`（施工 LF-4 上） | `life.ended` 只记一次；记了以后同封存，这一轮结束时（闲着的当场）记 `session.sealed` |
| `crates/miyu-session/src/actor/extract.rs`、`memory/extract/{extractor,job}.rs`（施工 LF-4 上） | 结束了的不再抽取：不上闹钟，在路上的那一次落地前丢掉 |
| `crates/miyu-session/src/memory/turns.rs`（施工 LF-4 上） | 载入、补齐见到 `life.ended` 的不进回合索引 |
| `crates/miyu-endpoint/src/schedules/fire.rs`（施工 LF-4 上） | 人生那一行到点、人生已经结束了的：删掉、不叫醒她 |
| `crates/miyu-endpoint/src/refusal/life.rs` | `life_exists`（带 `session`）、`no_life`、`life_ended`（带 `session`，施工 LF-4 上），拒绝带上会话编号 |
| `crates/miyu-endpoint/src/lib.rs` 的 `Core::life` | 一把锁：改的三个方法排着来 |
| `crates/miyu-kernel/src/event/life.rs` | 事件 `life.started`、`life.ended`（`kernel/events-bodies.md`）；内核命令 `Append` 的 `Life`、`LifeEnded`（`input/command.rs`）记它们 |
| `crates/miyu-store/src/index/row.rs` 的 `Life` | 会话索引的 `life` 一列（`store/index.md`） |
| `crates/miyu-endpoint/src/list.rs` | 会话列表那一项写 `life: true` |
| `crates/miyu-endpoint/src/schedules.rs` 的 `resume` | 人生那一行续上，下一次写成现在（`schedules.md` 第四节） |
| `resources/presets/life.toml` | 出厂预设「推演人生」（`presets.md`「出厂的几份」）；施工 LF-4 下起开 `base-life` |
| `crates/miyu-endpoint/src/schedules/doing.rs` | 「现在在干嘛」：记她写的那一句（`Schedules::set_doing`）、内存里的一份（`Lives`，只在 `Schedules::notify` 跟着表变）、核心起来时铺回、写好那一行、`life.status` 的 `doing`（施工 LF-7） |
| `crates/miyu-store/src/schedules/doing.rs` | 定时表的 `doing` 那张小表（`schedules.md` 第一节） |
| `crates/miyu-session/src/actor/model/doing.rs` | 回合开头哪些会话问、交过的不交（施工 LF-7） |
| `resources/software/life/doing.txt` | 那一行的外壳（登记簿） |

### 对外的样子

1. **身份记在会话日志里**：`life.start` 在新会话的日志里记一条 `life.started`（`body` 是 `{}`，`by` 是开它的人，不带回合）。真相只有日志（同长期目标，`goal.md`）：定时表那一行只管下次几点醒，表坏了换成空的以后照这一条接上。记成内核的事件而不是包的 `ext.*`：`ext.<前缀>` 是给包用的，前缀是 `life` 的包、本机的头经 `events.append` 都能往别的会话里记一条，冒充人生会话、占住一个人格（施工 LF-5 中，主会话 2026-11-14 定）。`events.append` 记不了 `life.started`：只有 `life.start` 经内核命令记。
2. **会话索引一格**：`life`，照日志里有没有 `life.started`、`life.ended` 派生（`store/index.md`：0 不是、1 开着、2 结束了），索引删了重建照样派生得出。会话列表（`session.list`、`sessions` 流）里人生会话那一项写 `life: true`，结束了的再写 `life_ended: true`（施工 LF-4 上，头照它显示成只读的档案），别的不写。
3. **方法**（只给人，本机、远程的头都行；核心拉起的扩展回 `local_only`，`connection/rights.rs`）。参数都是 `{persona}`；没有这个人格的 `unknown_persona`（照 `personas::resolve`，写法不对的 `bad_params`）。
   - `life.start {persona}` → `{"session"}`。
   - `life.status {persona}` → `{"state": "none" | "running" | "paused", "session"?, "next"?, "doing"?}`：没有人生会话的 `none`；人生那一行在、没停的 `running`，带会话和下一次；停着的、那一行不在的 `paused`，带会话。`doing` 是 `{"text", "at"}`：那一行上她写过「现在在干嘛」的才带（停着的也带），见下面「现在在干嘛」（施工 LF-7）。
   - `life.pause {persona}`、`life.resume {persona}` → 同 `life.status` 的回应。没有人生会话的 `no_life`，结束了的 `life_ended`（施工 LF-4 上）。
   - `life.status` 多一种 `ended`（施工 LF-4 上）：这个人格最近的那一条已经结束、还没开新的，带那个会话。
   - `life.kill {persona}`（施工 LF-4 上）→ `{"state": "ended", "session"}`：照「结束」做完才回；结束了的把清理剩下的几步再走一遍、原样回成功；没有的 `no_life`。
   - `life.new {persona}`（施工 LF-4 上）→ `{"session", "ended"?}`：在跑、暂停着的先结束（`ended` 是它），结束了的先补齐清理，再照 `life.start` 开一条新的。`life.start` 碰上结束了的同样先补齐清理、开新的。
4. **出错**：`life_exists`（`data.session` 是那一个）、`no_life`、`life_ended`（`data.session` 是那一个，施工 LF-4 上）；清理哪一步没成的照那一步的原因，带 `data.session`；定时表的照 `schedule.*` 的说法（`too_many_schedules`、`internal_error`），`life.start` 记了身份以后才出的多带 `data.session`。给人看的话在 `refusal/message.rs`（中、英）。
5. **出厂预设** `life`：见 `presets.md`「出厂的几份」。

### 怎么走

**1. 找这个人格的人生会话**：照会话列表的读法（`list::scan`，索引落后的照日志补到末尾），在本机的主会话里找 `life` 是开着的或者结束了的（施工 LF-4 上）、人格（这一刻的，中途换了人格的照换了的）是它的，取最新的一个。只照不会变的格（父会话、场所）先挑：`life`、人格在造好以后才变，落后的那一行照它们挑会漏掉。

**2. 排着来**：`life.start`、`life.pause`、`life.resume`、`life.kill`、`life.new` 在 `Core::life` 那一把锁里做完（同终端会话、场所会话找回的写法）：两个头同时开同一个人格的，后一个等前一个记完身份再找，找到了回 `life_exists`。`life.status` 只读，不拿锁。一把锁管全部人格：开、停、续很少，不同的人格排着队、结果互不相干。

**3. `life.start` 的四步**：

1. 找（第 1 条），有的回 `life_exists`。
2. 开会话：照 `session.create` 的那一条路（`Sessions::create`，命令编号是这条请求的编号），人格是参数、预设 `life`、模型照这一刻的默认（`model` 不写：照这时的 `models.chat` 记下，以后改配置这个会话照旧）、`cwd` 是 `~`、没人看着（`attended` 假，同定时任务每次新开的）、标题照常自动起。开不成的照原因回，什么都没留下。
3. 设上下文上限（设计 68 的 `context`，同 `session.configure`，命令编号接 `/context`，记成人发的）：照人格的 `[life] context`，没写的照出厂的 100000 token，这一刻的模型最少要的比它大的照最少要的（下面「上下文上限」）。人格在 `core.life` 的锁里读。再记身份（命令 `Append` 的 `Life`，编号接 `/life`）。这两样哪一样没成，删掉刚开的会话（照 `session.delete`，删不掉的记 `WARN life session not removed`），照原因回：不留一个还不是人生会话的空会话。
4. 建人生那一行：人建的、跑在这个会话上、留言是空的、第一次是现在（`Schedules::add`），于是马上醒第一次。没建成的（表挂满了、用不了）照原因回，带 `data.session`：身份已经记上了，它就是这个人格的人生会话，`life.status` 说 `paused`，`life.resume` 照第 5 条重建。

**4. `life.pause`**：找，没有的 `no_life`；那一行在、没停的停掉（`Schedules::pause`：下一次清掉）；停着的、不在的不动。

**5. `life.resume`**：找，没有的 `no_life`；那一行停着的续上（`Schedules::resume`：人生那一行的下一次写成现在，马上醒，她的留言不动）；在跑的不动；不在的照第 3 条第 4 步重建（表坏了换成空的以后，设计 70 第三节第 1 条）。

**6. 结束**（施工 LF-4 上，设计 70 第三节第 5 条）：只有有意的结束，入口是主人的 `life.kill`、`life.new` 和她自己的 `end_life`（施工 LF-4 下，见下面第 7 条）。都调 `life::finish(core, session, by, last_words)`，`by` 是人或者她的那次工具调用，遗言只有她自己结束的才有。照这个先后，每一步先查后做、能重做，重做不重复清、不重复推：

1. 记 `life.ended`（`kernel/events-bodies.md`）：已经记过的跳过。命令编号照会话固定（`life/<会话>/end`），两处同时结束的内核照编号只记一次。主人结束、这时正跑一轮的，打断它（`/stop` 的打断，排着的留着）；她自己结束的那一轮照常说完。记了以后同封存（`session/fork.md`「封存」）：改对话的命令拒 `session_sealed`，排着的不再开轮，这一轮结束时（闲着的当场同一批）记 `session.sealed`。
2. 等封存了再清（订阅这个会话等 `session.sealed`）：那一轮里她记下的、进回合索引的不漏。第 1 步当场做完交回，第 2 到 6 步在自己的任务里做：她调工具结束的，那一轮要等工具答完才结束、才封存。
3. 删掉定时表里人生那一行（没有的算成）。
4. 清记忆：同 `memory.forget {clear: session}`（`memory.md` 第二条第 5 款），出处全在这个会话里的清掉，别处也有出处的、人记的不动；命令编号照会话固定（`life/<会话>/memory`），重做照编号只算一次。现成的清法本来就让摘要作废、下次整理从头写（`memory.md` 第七条第 8 款）；人卡不动（只清一个会话不碰卡）。这个会话没有记忆的跳过。抽取另拦两处（`memory.md` 第六条）：结束了的不上闹钟，在路上的那一次落地前再看一次、整批丢掉；看过以后才结束的，写下以后再清一次这个会话的。不拦的话封存当场响的闹钟会把这段人生抽回记忆：按会话清只清「那以前」的。
5. 回合索引：`forget_session`（`memory.md` 第一条第 7 款），埋整个会话的墓碑、拿掉全部；回合库删了重建以后墓碑没了，载入、补齐照日志判：日志里有 `life.ended` 的不接（`memory/turns.rs`）。
6. 给主人推一条通知 `life.ended {persona, session, by: "self" | "owner", last_words?}`（`schedules.md` 第七节）：只在第 1 步是这一次新记上的时候推。

哪一步没成的照那一步的原因回（带 `data.session`），再结束一次（`life.kill`，或者 `life.start`、`life.new` 碰上它）接着清完。第 1 步成了、第 3 步以前断了的，人生那一行到点时看到会话已经结束，删掉这一行、记 `skipped:life_ended`，不叫醒她（`schedules/fire.rs`）。会话日志不删、不进回收处，照常能读、能翻历史；没有接上的入口。确认是头的事，协议上不另加往返。

**7. 她自己结束**（施工 LF-4 下，设计 70 第二节第 3 条，`tools/end_life.md`）：

1. 工具 `end_life {last_words}`，归出厂内置包 `base-life`（人格组，功能 `life`），清单写 `needs = ["life"]`：只在没结束的人生会话的工具面上。造会话时 `life.start` 明说是人生会话（`Opening` 的 `life`，那时还没记 `life.started`）；之后回合开始重算、换人格换预设时照日志问「有 `life.started`、没有 `life.ended`」（`Session::living`）。从第一轮起工具面不变，前缀缓存不断。别的会话拿同一个预设也没有，不算进「没开的」。出厂预设 `life` 开它。
2. 遗言必写，去掉前后空白不能是空的；没写的拒，什么都不记。
3. 工具经 `LifePort`（随定时端口交进会话，`SchedulePort::life`）交给端点：同一个会话一次一个；不是人生会话的拒、结束了的回「已经结束了」；否则调 `finish(core, session, by: 那次工具调用, Some(遗言))`，记下 `life.ended` 就答她，后面几步 `finish` 自己在后台做，不等（等了这一轮结束不了：清理要等封存，封存要等这一轮说完）。没清完的记 `WARN life not cleared`，主人再结束一次接着清完。
4. 这一轮照常说完，结束时封存；之后和主人结束的一样（第 6 条第 2 到 6 步），通知的 `by` 是 `self`、带遗言。
5. `schedule_wakeup` 的 `stop` 落在人生循环上回的那一句指到它：`A life loop cannot be stopped. To end this life, use end_life.`

**8. 现在在干嘛**（施工 LF-7，设计 70 第三节第 8 条，主会话 2026-11-14 定）：她在人生会话里调 `schedule_wakeup` 写的 `reason`，同一个人格别的会话的回合开头变了才追加一块。

1. **存在哪**：`schedule_wakeup` 落在人生那一行上时（`schedules/port.rs`），连同写下的时刻（现在）、她刚定的下次醒的时刻，记在定时表的 `doing` 表那一行（`schedules.md` 第一节）。存之前换行（`\r\n`、`\n`、`\r`、U+2028、U+2029）换成空格、去掉两头空白、最多留 200 个字（照字符，`DOING_MOST`），多的截掉；她没写、清理完是空的，清掉。照默认间隔醒的那一次（她没调工具）不动它：到点写下一次碰不到这张表。她自己定节奏的循环（`dynamic`）不记。记不下的记 `WARN life doing not saved`，她定的下一次照旧。
2. **内存里一份**：端点的 `Schedules` 里一份 `Lives`：人生那一行对它的会话、在不在跑（没停）、她写的那一句；人生会话对它的人格。只在 `Schedules::notify` 这一个口跟着表变：建、停、续、删、改下一次、到点、记这一句都报一声，照编号重读那一行（主键查，只在表变时读）；行没了的拿掉。读不了的记 `WARN life doing not read`，内存里的不动。人生会话是哪个人格的：核心起来时在后台只查表里人生那几行指着的会话（`list::one`：索引的那一行补到日志末尾；一个个查，查不到的、不是开着的人生的记 `WARN life session not listed`），`life.start` 开出来时记一次。表里没有人生的什么都不读：核心起来时不为它扫全部会话（全扫会和重启后头一次打开会话抢读盘，性能闸门「热启动到能提交第一轮」量得出来）。已知：人生会话中途换了人格的，下一次核心起来才认得出。
3. **交给谁**：带人格、不是派出去的子代理的会话，本机的、场所的（群、私聊）、人手分叉出来的支线都算（支线的 `lineage` 是空的，子代理才有，`miyu-session` 的 `open/load.rs`）。人格要和人生会话的一样；人生会话自己、人生停着的、那一行不在的（结束了、表坏了换成空的）、她还没写过的不交。会话那一层判子代理、人格（`actor/model/doing.rs`），端口判别的（`Schedules::doing_of`）。
4. **什么时候交**：回合开始的挂接点（`session/actor.md`「跑回合开始的挂接点」），排在记忆后面。只查内存，不读盘，不另限时（每个会话每一轮都问）。这一段上下文里最近一块模块 `life`、类别 `doing` 的 `refs` 和这一句写下的时刻（毫秒）一样的不交；压缩、撤掉以后不在了的重交一次。重启以后各会话的日志里有交过的那一块，照样认得出，不重交；重放一字不差。
5. **写法**：一块 `context.injected`，模块 `life`、类别 `doing`，`refs` 是写下的时刻（毫秒），排在触发这一轮的那句前面。原文照 `resources/software/life/doing.txt`：`<life-doing since="{since}" until="{until}">{text}</life-doing>`，以换行结尾。`since` 是写下的时刻、`until` 是那时定的下次醒的时刻，照会话的时区写到分钟（`2026-11-14 19:31`，`Timestamp::local_minute`）；`text` 是她写的那一句。三个字段都照模板的规矩转义（`kernel/request.md`「模板」第 3 条）：`<`、`>`、`"`、`&` 写成 `\u` 的样子，伪造不了外壳。外壳读不成、写法不对的记 `WARN life doing shell not read`，不交。
6. **结束**（LF-4）：删人生那一行走 `Schedules::cancel`，`doing` 表那一行跟着删，内存里那一份跟着拿掉。

**9. 上下文上限**（施工 LF-10，设计 70 第四节，2026-11-14 项目主人定：32000 太小，出厂至少 100000，放在人格的编辑页能改）：

1. **要用的数** = 人格叠好的 `[life] context`（`personas.md`「`persona.toml`」），没写的照出厂的数（`life.rs` 的 `CONTEXT`，100000，不是配置项）；比会话这时最少要的（`least_context`，同 `session.configure` 的 `context` 拒的那条线）小的抬到最少要的。生效的窗口照旧取它和模型窗口里小的（`Limits::capped`）。不做「不限」的写法：想放开的写一个很大的数。不设的话一周后每次醒来要带二十多万 token：她隔一两个小时才醒一次，供应商的缓存多半已经失效，每次醒来整份上下文按原价算。
2. **什么时候设**：
   - `life.start`、`life.new` 开会话时（第 3 条第 3 步）。
   - `persona.set` 改了 `life.context`（写了新的数，或者 `unset`）、你那一层的这一格这一次确实变了、几份都写成了的：回应以前，在 `core.life` 的锁里照第 1 条找这个人格的人生会话，没结束的（在跑的、停着的；停着的没载入的载入一次）当场设成要用的数（`life::follow`）。命令记成人发的，编号每次新造 `life/<UUIDv7>/context`：请求编号是各个连接自己编的，接它的话别的连接、过几天的另一次重用同一个编号会撞成 `id_reused`，或者被内核当成重发吞掉。和会话这时一样的内核不记事件（端点不另比）。没有人生会话的、结束了的什么都不做；设不成的（会话这时停了、人生刚好在这一刻结束封存了）记一行 `WARN life context not set`（`session`、`reason`），`persona.set` 照样算成：数已经写进人格，下一段人生照它。
3. **到点不同步**（2026-11-14 项目主人定）：上下文上限是会话自己的一格。人生会话里打 `/context`（或者 `session.configure` 的 `context`）改了的一直管用，她醒几次都是这个数，和别的会话一样；人格里的数是「开一段人生时用多少」和「在人格页上改的那一刻改成多少」。两个入口改的是会话的同一格，后改的算数。
4. **手改 `persona.toml`** 的不碰跑着的人生会话（没有人告诉核心），只管下一段人生；要马上生效的走 `persona.set`（人格的编辑页），或者在人生会话里打 `/context`。

### 命令（施工 LF-5 下，设计 70 第二节第 11 条）

人在本机的会话里打 `/life`，看、开、停、续、结束这个会话的人格的人生会话，不用先找到那个会话。它只是 `life.*` 的另一个入口：照这个会话这一刻的人格调同一个 `life::act`（`life.rs`，六个方法也走它），命令只认参数、拼回执。跳到人生会话是头的事，核心在回应里带上它的编号。

| 打的 | 做什么 | 回执（中文） | 带 `session` |
|---|---|---|---|
| `/life` | 同 `life.status`。在跑的说下次几点醒（核心这台机器的时区，到分钟），她这会儿正跑一轮的说醒着；停着的说停着。没有的、上一条结束了的**不开**，只回说明 | 「人生会话进行中，下次醒来：2026-11-14 15:00。」「人生会话进行中，她这会儿醒着。」「人生会话已暂停，用 /life resume 继续。」；说明见下 | 有的带，没有的不带 |
| `/life new` | 同 `life.new`：有在跑、停着的先结束（它留下的记忆清掉），再开一条，她马上醒第一次 | 「人生会话已开始，她马上醒来。」，结束了旧的「上一段人生会话已结束，新的一段已开始，她马上醒来。」 | 新的那一条 |
| `/life pause`、`/life resume` | 同 `life.pause`、`life.resume` | 停的回「已暂停」那一句，续的回状态那一句 | 带 |
| `/life kill` | 同 `life.kill`：有在跑、停着的结束 | 「人生会话已结束，留下的记忆已清掉，会话保留为只读。」 | 结束的那一条 |

1. **没有的**：停、续、结束碰上没有的、结束了的（`no_life`、`life_ended`）回「这个人格没有人生会话。」，不带 `session`；结束了的不再走一遍清理（要补齐的走 `life.kill` 或者 `/life new`）。
2. **说明**（光打 `/life` 而没有的）：说清这是什么、她会自己醒、每次醒是一次请求会多花 token、她会自己记事、怎么开：「人生会话让这个人格一直过着自己的日子：她到点自己醒来，做点自己的事，记下的会进她的记忆。每次醒来都是一次模型请求，会多花 token。用 /life new 开始。」英文、日文照意思写（`core/human/<语言>.json` 的 `commands/life-*`）。
3. **不做确认**：打了就做，没有 `confirm` 这种第二个词（2026-11-14 项目主人定）；有界面的头要不要自己画确认框是头的事（LF-8）。
4. **谁能用**：同 `/workspace`，只给主人本人（管理的人 `owner_only`）；场所会话里照没有这条命令（`unknown_command`），菜单本来就不收场所会话。这个会话不带人格的回「这个会话没有人格，没有人生会话。」；人格已经删了的照 `unknown_persona`。认不出的参数回 `bad_params`，`data.why` 是「用法：/life [new | pause | resume | kill]」。都什么都不动。
5. **预设**：核心自带的命令（同 `/clear`、`/context`），不归哪个包、不跟预设的功能开关：核心命令现在都这样（`Slash::package` 是空的就一直在，`commands/registry.rs`），菜单在每个本机会话里都列它（施工 LF-5 下查的现状，照它做）。
6. **在人生会话自己里面打**：照它自己的人格，同样管用。`/life kill`、`/life new` 结束的就是这个会话：结束等封存了才回，回执照样回得出来；之后 `command.ran` 记不进封存了的会话（`protocol.md`「`command.run`」第 6 条），不算错。
7. **回执不进她的上下文**：回执只在 `command.run` 的回应里，日志里只记一条 `command.ran`（原文和命令名，不带回执），渲染不给模型看（`miyu-assemble` 的 `render.rs`）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-endpoint/tests/life.rs` 的 `start_opens_a_life_session_and_she_wakes_at_once` | 开出的会话人格、预设、上下文上限 100000（人格没写，施工 LF-10）都对；日志里一条不带回合、人记的 `life.started`；第一次醒来的正文是出厂的推演方法、`by.loop` 是 `life`；人生那一行人建的、醒过以后照默认间隔 |
| 同上 `one_life_per_persona_even_when_two_heads_start_at_once` | 两个连接同时开同一个人格只开出一条，另一个 `life_exists` 带那一个；再开同样；表里只有一行人生；别的人格互不相干 |
| 同上 `status_says_none_running_or_paused` | 没开的 `none`；在跑的带会话和下一次；停了的 `paused` |
| 同上 `a_paused_life_sleeps_and_a_resumed_one_wakes_from_now` | 停着到点不醒；续上照现在醒、再照默认间隔；那一行删了以后状态是停着，续上照日志重建、接着醒 |
| 同上 `resuming_a_running_life_changes_nothing_and_no_life_is_refused` | 没有人生会话的停、续回 `no_life`；在跑的续上不重算 |
| 同上 `an_unknown_persona_is_refused_by_all_four` | 没有的人格四个方法都回 `unknown_persona` |
| 同上 `the_list_marks_a_life_session_and_a_rebuilt_index_still_does` | 会话列表人生会话带 `life: true`、别的不带；索引的行都没了照日志补回来一样，`life.start` 照样认得出来 |
| 同上 `resume_puts_a_life_row_at_now_and_leaves_other_loops_alone` | `Schedules::resume` 落在人生那一行上是现在、留言不动；`dynamic` 照旧 |
| `crates/miyu-endpoint/tests/persona_life_context.rs` 的 `life_start_caps_by_the_persona`（施工 LF-10） | 人格没写的开出来 100000，写了 50000 的是 50000，写得比最少要的还小的抬到最少要的 |
| 同上 `persona_set_moves_a_running_or_paused_life_once` | `persona.set` 改了 `life.context` 当场换在跑的人生会话的上限、只多一条；同一个数再写不多记；停着的也换，`unset` 换回出厂的数 |
| 同上 `persona_set_leaves_no_life_and_ended_lives_alone` | 没有人生会话的照样写成；开的时候照人格写的；结束了的会话日志一条都不多 |
| 同上 `context_typed_in_life_stays_across_wakeups` | 人生会话里 `/context` 改的，改人格别的格不动它、再醒两次还是它（到点不同步）；再在人格里改换成新写的 |
| 同上 `life_context_is_set_and_unset_through_changes` | `persona.set` 的 `changes` 写、删 `life.context`，`persona.get` 的两格跟着变；0、负数、小数、字拒 `persona_invalid`、什么都不写 |
| `crates/miyu-endpoint/tests/life_preset.rs` | 出厂预设列出来、`hidden`、图标、步数上限；开出的会话工具面 |
| `crates/miyu-endpoint/tests/extension_rights.rs` 的 `life_methods_are_people_only` | 批了驱动会话、读写事件的扩展调四个方法回 `local_only` |
| `crates/miyu-kernel/src/session/tests/venue.rs` 的 `the_life_identity_is_appended_like_the_rest` | 内核记身份：不带回合、不开回合、`by` 照发的 |
| `crates/miyu-store/src/index/tests/life.rs` | 索引那一格照日志派生、存了读回、认不得的数读不懂 |
| `crates/miyu-kernel/tests/samples.rs` | `life.started`、`life.ended` 的样本读写一字不差 |
| `crates/miyu-endpoint/tests/life_end.rs` 的 `kill_ends_a_life_and_clears_what_it_left`（施工 LF-4 上） | `life.kill`：人记的、没有遗言、不带回合的 `life.ended`，封存；人生那一行没了；推了通知；这段人生记下的清掉、别的会话记的还在；回合索引拿掉、别的还在；往后拨不再醒；发话拒 `session_sealed`；状态 `ended`；列表带 `life_ended`，索引删了照日志一样 |
| 同上 `an_ended_life_refuses_pause_and_resume_and_new_ones_start_fresh` | 停、续拒 `life_ended` 带会话；`life.start` 开出新的一条；在跑的时候 `life.new` 结束旧的、开出第三条 |
| 同上 `ending_again_redoes_nothing_and_a_leftover_row_is_dropped_unwoken` | 没有的 `no_life`；再结束一次原样成功、不记第二条、不再推；留下的人生那一行到点删掉、不叫醒她 |
| `crates/miyu-kernel/src/session/tests/life_end.rs`（施工 LF-4 上） | 闲着的同一批封存、只记一次；在跑一轮的这一轮说完才封存，中间来的话拒、不再开轮 |
| `crates/miyu-session/tests/memory_extract_life.rs`（施工 LF-4 上） | 结束以后封存当场响的闹钟不抽；抽取正在半路上时结束的整批丢掉、不记抽到了哪 |
| `crates/miyu-session/tests/memory_life_ended.rs`（施工 LF-4 上） | 回合库重建以后补齐不补结束了的人生，别的会话照补 |
| `crates/miyu-endpoint/tests/life_doing.rs` 的 `another_session_sees_it_once_and_again_when_she_writes_a_new_one` | 她写了一句：换行换成空格，`life.status` 带 `doing`；同人格的另一个本机会话下一轮开头一块，带这一句、写下的时刻、下次醒的时刻，请求里有；再下一轮不重交；她写了新的交新的；没写 `reason` 的清掉、`doing` 不带；人生会话自己没有 |
| 同上 `a_dynamic_loop_keeps_no_reason` | 她自己定节奏的循环带 `reason`：下一次照旧，不记、别的会话没有 |
| `crates/miyu-endpoint/tests/life_doing_more.rs` 的 `venues_and_branches_see_it_but_not_her_life_other_personas_or_subagents` | 人手分叉的支线、场所会话（主人的私聊；会话这一层不分群和私聊）有；人生会话自己、别的人格、派出去的子代理（同一个人格）没有 |
| 同上 `nothing_before_she_writes_while_paused_or_after_the_row_is_gone` | 她还没写过、人生停着的没有（停着的 `life.status` 照样带 `doing`），续上了有；那一行删了跟着删、没有 |
| 同上 `compaction_brings_it_back_once` | 压缩以后的第一轮重交一次，再下一轮不交 |
| 同上 `her_words_cannot_close_the_shell_and_long_ones_are_cut` | 带收尾标签、换行、引号的伪造不了外壳、只有一行；超过 200 字截掉 |
| 同上 `after_a_restart_it_is_laid_back_from_the_table_and_replays_byte_for_byte` | 核心重启以后照表铺回、别的会话拿得到；再重启交过的不重交、同样的日志投影出逐字节相同的前缀 |
| `crates/miyu-endpoint/src/schedules/doing/tests.rs` | 换行换成空格、两头空白去掉、最多 200 个字 |
| `crates/miyu-session/src/actor/model/doing/tests.rs` | 模块 `life`、类别 `doing`、记号；只比最近那一块、别的模块别的类别不算；子代理、没人格的连问都不问；没端口、端口没有的不交 |
| `crates/miyu-store/src/schedules/doing/tests.rs` | `doing` 表写、换、清、重开读回，到点写下一次碰不到它；删任务那一行跟着删；以前建的库开的时候补上，原来的行照读 |
| `crates/miyu-schedule/tests/wakeup.rs` 的 `a_blank_reason_is_not_handed_on` 等 | `reason` 交给端口，空白的当没写 |
| `crates/miyu-endpoint/tests/life_self_end.rs`（施工 LF-4 下） | 她自己结束：工具面只在人生会话里有、第一轮起每一轮一样（重起核心以后也一样）、别的会话没有也不算没开；调了回那一句、这一轮说完，`by` 是那次调用、遗言原样，之后和主人结束一样清理，通知 `by: self`；没写遗言的拒；同一轮调两次只记一条；`stop` 回新的那一句 |
| `crates/miyu-schedule/tests/life.rs`（施工 LF-4 下） | `end_life` 的几种回话（假端口） |
| `crates/miyu-endpoint/tests/life_command.rs`（施工 LF-5 下） | `/life`：没有的只回说明、不开、不带 `session`，结束了的同没有；在跑的说下次几点醒、停着的说停着，带它；`new` 开出来马上醒，再 `new` 旧的人记结束、新的是另一个会话、回应带新的；`pause` 到点不醒、`resume` 接着醒，没有的两个都回没有；`kill` 结束、封存，没有的回没有；在人生会话自己里面看、结束、再开都管用；不带人格、场所会话、认不出的参数各回各的、什么都不动；英文、日文回执，命令目录里有它和几种写法 |
