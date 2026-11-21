## 定时和循环

设计 `45-定时和循环.md`。核心里只有一样东西：到点把一句话交给一个会话跑一轮的「唤醒」，记在一张表里，核心重启不丢。定时任务、固定间隔的循环、她自己定节奏的循环都是它。

状态：J-1 上做完（唤醒表、到点、交进会话，2026-10-26），J-1 下做完（协议 `schedule.*`、`schedules` 流）；她的工具、`/loop`、标签的字是 J-2；命令行、系统通知是 J-3；J-4 做完（2026-10-27）：扩展能建，到点推给建它的扩展（第五节第 7 条），场所会话的循环由场所自己拿它拼；J-5 做完（2026-11-02）：`schedule.update` 改时间、改那句话，编号、上一次不变（第五节第 3a 条）。LO-1 做完（2026-11-14）：光打 `/loop` 开自主循环（第五节第 9 条）。LO-3 做完（2026-11-16）：她自己定节奏的循环那一轮没定下一次就删掉那一行（第三节第 10 条），`/loop clear`、`/loop stop` 同 `off`（第五节第 9 条）。LO-3补 做完（2026-11-16）：有计划地停下时被打断的那一轮不删、起来接着盯，核心停着时留下的没定下一次的删掉（第三节第 11 条）。

### 一、唤醒表

一份 `root/schedules.db`（SQLite，`miyu_store::schedules`）。它是真相，不是派生的，和会话列表的索引、用量汇总不一样：

1. **开**：没有就建，`user_version` 是 1。只加表、加索引的改动不升版本。
2. **坏了**：读不了、坏了的连同 `-wal`、`-shm` 改名成 `schedules.db.broken-<毫秒>` 留着，开一份空的，记 `WARN schedules set aside`。人的定时没了要告诉他：施工 J-3 中起另推 `notice {"kind": "schedules.reset"}`（第七节「给人的通知」）。
3. **一行读不懂**（施工 AE-4，发布前测试 2026-10-30）：某一格被改坏了、换了这一版不认的写法（类型不对也算）的，`list`、`due` 跳过这一行，别的照常到点、照常列；`unreadable()` 交出它们（编号、哪一格），核心起来时一行记一条 `WARN schedule unreadable, skipped`，`check` 报 `schedule_unreadable`（`kind` 是 `schedule`）。照编号读它（`get`）的照旧报错，删（`cancel`）得掉。原来一行坏了 `due`、`list` 整份交错，睡着的那个当成一个都没有，全机的定时悄悄停掉。跳过它也不空转：最早的一次照 `list` 算，坏的那一行不在里面。SQLite 自己出错（读不了库）的照旧整份交错。
4. **比这一版新**：新的程序写过、又换回旧的，不动它，这一回不用，记 `WARN schedules newer`。回到新的程序照样在。
5. **写盘**：`synchronous = FULL`，答应了的改动断电也在。一个连接，开了一直开着。
6. **用到时开**：人、她建第一个任务时建；核心起来时只在已经有表时开。

一行一个任务：

| 格 | 是什么 |
|---|---|
| `id` | `t<n>`，递增，删了的号不再用（`counter` 表） |
| `created_at` | 建的时刻，毫秒 |
| `by` | 谁建的，JSON：`{"kind":"person"}`（人：命令行、协议）、`{"kind":"model","session"}`（她：在这个会话里用工具建的，施工 J-2）、`{"kind":"package","package"}`（包，施工 J-4） |
| `when_json` | 什么时候跑，JSON：`{"cron":{"cron":"0 8 * * *","recurring":true}}`、`{"at":{"at":"<时刻>"}}`、`{"every":{"every":600}}` |
| `prompt` | 交进会话的那句话，原样存；标签组装时包，不在这里 |
| `target` | 跑在哪，JSON：`{"session":{"session":"<编号>"}}`，或 `{"new":{"persona"?,"preset","cwd"}}`（每次新开，人格、预设、工作目录建的时候就定下来） |
| `target_session` | 跑在哪个已有会话，好查；每次新开的没有 |
| `loop` | `fixed`、`dynamic`，没有的是定时任务（施工 J-2 的 `/loop`、`schedule_next`） |
| `paused` | 停着：不到点 |
| `next_at` | 下一次；停着的、她自己定节奏的循环这一轮还没定的没有（这一轮结束时还没有的删掉这一行，施工 LO-3，第三节第 10 条） |
| `last_at`、`last_result`、`last_session` | 上一次：时刻、`ran` \| `queued` \| `held` \| `skipped:<原因>`、交给了哪个会话 |

### 二、什么时候跑

1. **cron**：只认标准五段（分、时、日、月、星期），不认秒和年，照这一个记下的时区算（施工 J-3 上：`when.zone`，IANA 名；建的时候人交的，不交的记这台机器那一刻的；以前建的没有，照本机时区；记下的认不出了也照本机），读不出本机时区的当 UTC。日、星期两段照 Vixie cron 的「或」。夏令时照 OCPS 规范：跳过的那一小时里的时点挪到跳过以后，重复的那一小时只算一次。库是 croner 4.0.1（`jiff` 特性，不带 chrono）。`recurring` 是假的跑一次就删。
2. **某个时刻一次**：建的时候已经过了的不收（核心的入口另有 `first`，见第四节）。跑过就删。
3. **每隔一阵**：最短 60 秒。下一次照上一次往后数整数个间隔，错过的几个跳过，正好是现在这一刻的也算过去了。
4. **错过的不补**（同旧版定时消息）：核心起来时已经过了的一律跳过，记 `INFO schedule missed`。一次的删掉，她自己定节奏的清掉下一次（跳完以后照第三节第 11 条删掉），别的照现在往后找下一次。核心开着时醒晚了的（机器睡过一觉）照常跑一次。

### 三、到点

1. 核心起来时（`Core::start_schedules`）开上已有的表、跳过错过的，然后一个后台任务只睡到最早的那一次。表一改（建、停、续、删、改、改下一次）就叫醒它重算；不轮询。真的钟一次最多睡一小时再看一眼：改了系统时间、机器睡过一觉，最多晚这么久发现。测试换手拨的钟（`Clock`，`Core::with_clock`）。
2. 到点的一个：先把下一次写进表（一次的删掉，她自己定节奏的清掉下一次、这一轮结束时还没定的删掉（第 10 条）），再交进会话，最后只记上一次那几格（`record`：不带着读出来的那一行写回去，下一次那一格不动，除非会话没了要停掉）。顺序不能反：她在那一轮里定下一次（`schedule_next`，施工 J-2）不会被这里盖掉。写不进下一次的不交，记 `WARN schedule not advanced`：交了又记不住，下一次醒来会再交一遍。
3. **写不进的先放一放**（2026-10-26 主会话审出来的）：表里还是过去那个时刻，不放的话马上又醒、又写不进，空转吃满一个核。只在内存里给它记一个「不早于」（`schedules/hold.rs`）：头一次 1 秒，再写不进翻倍，最多 5 分钟；算最早一次、挑到点的都照它往后挪，写进了就清掉。不碰表，核心重启就忘。表的连接锁住了不等（`busy_timeout = 0`），当场报错交给这里退避；睡到点的那个每一轮让一下线程，不霸着。
4. **跑在已有的会话**：交一条 `Send`，`by` 是 `{"kind":"schedule","schedule":"t3","loop"?:"fixed"|"dynamic","person"?:true}`（`kernel/ids.md`「谁」；`person` 是这一行的 `by` 是人的才写，施工 SC-1）。照「别处来的话」的规矩收（`kernel/session.md`「定时交进来的话」）：闲着开一轮，正在跑一轮的排在这一轮后面（定时任务照这一条；循环等这一轮结束再交，第 9 条）。`last_result` 照内核回的记（施工 SC-1）：由它开了一轮的（回应里附了 `turn.started`）记 `ran`；没开、交的那一刻会话正在跑一轮的记 `queued`；没开也没在跑的记 `held`，那句话记进了日志、没由它开轮，等人开口一起听到——没人看着的一次性会话（`miyu ask` 开的、这时没有头订阅着）里她自己建的和循环，还能恢复撤销这些开不了的时候（跑在已有会话上的循环除外：还能恢复撤销时不交，照第 9 条等，施工 FX-13）。人建的定时任务在没人看着的一次性会话里照样开轮（2026-11-18 项目主人定；以前一律只记下、还记成 `ran`，人看着是跑了、其实没跑）。命令编号 `schedule/<任务>/<UUIDv7>/say`，每一次到点新造一个（会话编号的造法 `miyu_session::new_id`，一个核心进程共用一份计数器：同一毫秒、系统时间往回拨都接着往上数，不重）。施工 FX-2 以前是 `schedule/<任务>/<这一次的毫秒>/say`：系统时间往回拨以后人停了再续、改了时间，cron 照现在算出的下一次会落在拨回前跑过的那一刻，编号和那一次一样，内核照重发回上一次（换了话的拒 `id_reused`），这一次不开轮、白白丢掉；原来想的「核心重启后再交一遍，会话认得是重发」用不上：先写下一次再交，重启以后不会再交同一次。
5. **每次新开**：照建时定下的人格、预设、工作目录开一个会话（命令编号 `schedule/<任务>/<UUIDv7>/open`，和交那句话的 `…/say` 同一个 UUIDv7），没人在场（要问人的照规矩拒），再照第 3 条交。`last_session` 记开出来的那个。开不成的记 `skipped:not_opened`。跑完给人推 `notice {"kind": "schedule.ran"}`（施工 J-3 中，第七节）。
6. **会话没了**：记 `skipped:session_gone`，任务停掉，留给人看、给人删。
7. **她建的照预设**（项目主人 2026-10-26 定）：`by` 是 `model` 的，到点时看那个会话现在的预设，关了建它的那个包的这一次不跑，记 `skipped:preset_off`，下一次照算。定时任务看 `base-schedule` 包，循环（`loop` 是 `fixed`、`dynamic`）看 `base-loop` 包（施工 PN-5 起的包编号）；包的功能里装着、没被关掉的有一个就算开（`presets::package_on`）。没装的包算关；没有预设的老会话都开着；无工具（空的编号）算关。人建的、包建的不看预设。
8. 每一次记 `INFO schedule fired`，带任务编号、结果；不记那句话。
9. **循环等这一轮结束**（施工 LO-2，2026-11-16 项目主人报的：输出中敲的 `/loop` 图标在、循环不在）：跑在已有会话上的循环（`loop` 是 `fixed`、`dynamic`）到点时那个会话正在跑一轮（`Handle::busy`），或者还有能恢复的撤销（`Handle::restorable`，施工 FX-13），这一次先不交、表不动，只在内存里记下等哪个会话（`schedules/busy.rs`）；算最早一次、挑到点的都跳过它。会话忙不忙变了（会话表端口的 `listing`，`spawn.rs`）、有没有能恢复的撤销变了（端口的 `restorable`）就放掉等它的、叫醒睡着的那个再挑：空下来的照常交、由这句开一轮，还忙的再记一次。撤销没处理时也等（施工 FX-13，2026-11-21 主人本机实测报的）：以前照第 4 条交进去，内核只记下（`can_wake`），日志里多一条没人回答的 `message.user`，头画成一行「循环 · 继续」；`/loop off` 以后它还记在内核一边，`/restore` 时由它开一轮，人发一句话时她也看得到这几句陈旧的话。现在撤回处理了（`/restore`，或者人说了一句、那一轮结束）才交，交的就是那一刻的一句，几次到点也只交一次。先记再看忙不忙：看完到记下之间空下来的也叫得醒。为什么不照第 4 条交进去：交进去的话这一轮的下一步就带着它，她顺着手上的活看到一句「继续」，不定下一次（实测：她答完手上的问题就结束了，一次 `schedule_next` 都没调），循环就停了、那一行停着等她定。人打断这一轮的，循环照样在它结束以后跑（以前交进去的那句被打断吞掉，不叫醒她）；`/stop` 连循环一起删（第五节第 9 条）。不碰表：核心重启以后那一行照错过的跳过。定时任务不等，照第 4 条。
10. **她不定下一次循环就结束**（施工 LO-3，2026-11-16 项目主人定，照 Claude Code「不调就停」，推翻施工 J-2 的「停着等她定」）：`loop` 是 `dynamic` 的那一行交进去、由这句开的那一轮（`turn.started` 的 `trigger` 是交进去的那一条）结束时，那一行的下一次还是空的（她这一轮里没调 `schedule_next`），删掉这一行，记 `INFO loop ended without a next wakeup`，同 `/loop off` 叫醒睡着的那个、推 `removed`，头上的「◷」跟着没了。以前那一行留着、`next` 是空的：图标在、循环其实停了。交之前先订阅那个会话（同第 5 条的 `schedule.ran`，`schedules/ended.rs`），交的那一刻会话正忙的不盯（排进了别人的一轮，第 9 条以后只在极少的竞争里有）；交进去的那一条并进了人、别的会话开的一轮的，那一轮结束就不再盯。只看它自己那一轮：人插的一轮、别的会话发来的开的一轮里她没调的不删。那一轮怎么结束的都算（打断、出错的也删：留着的那一行再也不会到点），只有照「重启了」（`restarted`）结束的不算，见第 11 条。删在存储那一层一条语句里查（`end_unpaced`：`loop` 是 `dynamic`、`next_at` 是空的、没停着），她这一轮里定了下一次的、人这时停了它的、再敲 `/loop` 换掉了的都不删。只在内存里盯：核心在这一轮里停了的照第 11 条。固定间隔的、定时任务不碰。
11. **核心停过以后的循环**（施工 LO-3补，2026-11-16 核心主会话定，修 LO-3 带进来的）：
    - 有计划地停下（`stop_sessions`）时，循环那一轮照「重启了」结束（`turn.ended` 的 `reason` 是 `restarted`），再载入时那一轮接着干，她在接着的一轮里还会定下一次。第 10 条盯着的看到这种结束不删，那一行留着、下一次还空着。LO-3 原来当场删掉：接着的一轮里她调 `schedule_next` 报找不到循环，循环断了（实测：沙箱里正常停核心，起来以后她定下一次失败）。
    - 核心起来时（`start_schedules`，跳过错过的以后，`ended.rs` 的 `boot`），`loop` 是 `dynamic`、没定下一次、没停着的每一行，载入它跑在的会话，先订阅再看忙不忙。正在跑一轮的就是接着干的那一轮（载入时内核接着开，设计 `02-内核.md` 第六节「载入、崩溃、重启」），它由「重启了」那一条触发、`trigger` 不是交进去的那一条，只能照命令编号认：`cause` 是 `schedule/<任务>/` 开头的。看到的头一个 `turn.ended` 是它的：是循环的照第 10 条办（又是重启了的照旧留着），不是的说明循环那一轮没接着干，删掉。空着的（被强杀、断电的那一轮不接着干，错过的没有轮）、会话没了的，当场照 `end_unpaced` 删掉、推 `removed`：它们再也不会到点，留着只剩头上一个「◷」。人停着的留着给人续。

### 四、核心的入口

`Core::schedules()` 交回 `Schedules`：

| 入口 | 做什么 |
|---|---|
| `add(Wanted)` | 建一个：cron 读不成、算不出第一次、隔得少于 60 秒的不收，什么都不建。`Wanted.first` 写了的照它（`/loop` 马上先跑一次写现在，施工 J-2），不写的照 `when` 从现在往后算 |
| `list()` | 全部：有下一次的照时刻在前，停着的、没定下一次的在后，照编号；还没有表的是空的；读不懂的行跳过（第一节第 3 条） |
| `get(id)` | 一个；没有的是 `Unknown` |
| `cancel(id)` | 删掉 |
| `pause(id)` | 停：清掉下一次 |
| `resume(id)` | 续：照现在重算下一次，错过的不补；一次的已经过了的报 `NoNext` |
| `update(id, when, prompt)` | 改（施工 J-5）：`when` 写了的照 `add` 一样查、换掉，没停的照现在重算下一次，停着的还停着；循环换不了种类（她自己定节奏的只认 `At`，固定间隔的只认 `Every`，别的报 `LoopKind`）；`prompt` 写了的换掉。编号、建的时刻、`by`、`target`、上一次都不动，记 `INFO schedule updated` |
| `set_next(id, next, prompt)` | 改下一次，`prompt` 写了的换掉那句话（她自己定节奏的循环每轮定一次，施工 J-2）；`next` 没有的停着等（她自己定节奏的那一轮结束时删掉，第三节第 10 条）；`prompt` 没写的不动 |

协议 `schedule.*` 和推送照第五节（施工 J-1 下）。

### 五、协议（施工 J-1 下，形状 2026-10-26 主会话审过；协议参考 `docs/zh/reference/schedules.md`）

只有一份表，没有的编号 `unknown_schedule`。

1. `schedule.add {when: {"cron", "recurring"?, "zone"?} | {"at"} | {"every"}, prompt, target?: {"session"} | {"new": {persona?, preset?, cwd?}}}`：不写 `target` 的是每次新开（人建的默认），人格、预设照这时的默认定下来（`personas::resolve`、`presets::resolve`，找不到的照开会话回 `unknown_persona`、`unknown_preset`），`cwd` 不写照家目录。回 `{"schedule": <一项>}`。写错的什么都不建，回 `bad_params`，`data.problem` 是 `when`（三种没写对一种、时刻写法不对）、`cron`、`past`、`short`、`prompt`（空的）、`session`（没有；这个连接碰不到的同样说，看不到的不露：平台桥碰不到本机的会话，人的头碰不到关着的场所会话，同 `protocol.md`「关着的场所会话只给桥」，施工 AS-3，发布前测试 2026-10-30 发现原来只查有没有）、`target`（两种都写了）、`zone`（不认识，施工 J-3 上）、`cwd`（每次新开的工作目录建时就查：照开会话的写法换成真实位置，不在、不是目录、落在数据根里又不在工作区 `workspace/` 里的，施工 J-3 上）。已有 `IN_ALL`（200）个的回 `too_many_schedules`：人建、她建、包建合计，熔断性质的安全上限（项目主人 2026-10-27 定），已有的照跑；她经工具建的照 `software/schedule/cron/all-full.txt` 告诉她叫人删。时刻照核心的写法（带毫秒）。带 `target.new.cwd` 的只给本机的连接（`reach.rs`，同项目的 `workspace`）。
2. `schedule.list {session?}`：写了 `session` 的只列跑在它上面的，回 `{"schedules": [<一项>…]}`。
3. `schedule.cancel {id}` 回 `{}`；`schedule.pause {id}`、`schedule.resume {id}` 回 `{"schedule": <一项>}`。
3a. `schedule.update {id, when?, prompt?}`（施工 J-5，项目主人 2026-11-02 报定时、循环改不了时间，形状主会话当天定）：`when`、`prompt` 写法、查法同 `schedule.add`（`methods.rs` 的 `when_of`、`Schedules::update` 共用 `checked`），至少写一样，都不写的回 `bad_params`（不带 `data.problem`）。回 `{"schedule": <一项>}`，照常推 `schedules.changed`。改了 `when` 的照现在重算 `next`，停着的改完还停着；编号不变，`last`、建的时刻、`target`、`by` 都留着。循环换不了种类：`loop` 是 `dynamic` 的 `when` 只收 `at`（就是改下一次醒的时刻），`fixed` 的只收 `every`，别的回 `bad_params`（`problem` 是 `when`）：换了写法她那边的 `by.loop` 对不上。`prompt` 都能改。以前只能删了重建：编号换了、上一次没了，循环的 `by.loop` 断了。
4. 一项：`{"id","when","prompt","target","by","recurring","paused","loop"?,"next"?,"last"?: {"at","result","session"?},"created"}`。
5. 推送：`subscribe {"stream":"schedules"}` 回 `{"schedules":[…]}`；之后推 `schedules.changed {"id","entry"}`，删了的 `{"id","removed":true}`；掉队推 `resync {"stream":"schedules"}`。建、停、续、删、改、改下一次、到点交过都推（`schedules/changes.rs`，一条是任务编号，同项目的）；订着的连接都推。
6. 谁能调：人；扩展要批过 `sessions.drive`（施工 J-4，`connection/rights.rs`），建的 `by` 记成包，只看得到、改得了自己建的（`methods.rs` 的 `mine`、`own`，`schedule.update` 同样），别的编号一样 `unknown_schedule`。`schedules` 流只给人，扩展回 `local_only`。
7. 推给扩展（施工 J-4，形状主会话 2026-10-27 和 onebot 定）：`target` 多一种 `{"package": {}}`，只有扩展能写（人写回 `bad_params`），不带 `loop`。到点不进会话：`fire.rs` 的 `pushed` 照 `by` 的包找那个包连着的连接（`Provided::linked`，不替它拉起），推通知 `schedule.fired {"id","at","prompt","missed"?}`，结果记 `sent`，推不到的记 `skipped:package_unavailable`。`missed` 是核心停着时错过的次数：起来跳过时（`run::skip`、`next::missed`，最多数到 1000）累加进表里 `target` 那一格，推过就清零。扩展握手成了另记一份连着的（`Provided` 的 `linked`），推通知照它找，不登记工具的扩展也推得到；工具调用照旧只认登记了工具的。
8. （原第 8 条讲推演人生，施工 RM-1 拆掉了。）
9. **`/loop`**（施工 J-2，斜杠命令走 `command.run`，`protocol.md`「`command.run`」第 16 条；`crates/miyu-endpoint/src/commands/looping.rs`）：人敲的记成 `by: person`、跑在这个会话上，一个会话一个循环，再敲一次换掉原来的。有能恢复的撤销时（内核 `Session::restorable`），循环先不交那一句、不开一轮（第三节第 9 条，施工 FX-13；FX-12 时是交进去只记下，`jobs.rs` 的 `can_wake`），建循环的回执换成同一句接上「有一处撤回还没处理，循环先搁着：发一句话或 /restore 以后开始」（`commands/loop-<fixed|dynamic|auto>-held`，三种语言；施工 FX-12，2026-11-21 主会话定：原来照常说「已开始循环」，人等着却什么都没发生）。会话照内核每收下一条输入记下的看（`Handle::restorable`，`Shown.restorable`，同忙不忙一起写）。`/loop <间隔> <话>`（`/loop every <间隔> <话>`、`/loop <话> every <间隔>` 也认）建 `fixed`，`/loop <话>` 建 `dynamic`，两种的第一次都是现在（`Wanted.first`）；`/loop off` 删掉这个会话的循环，单独一个 `clear`、`stop` 同它（施工 LO-3，2026-11-16 项目主人定：大小写不论、前后空白不算，`/loop clear the cache` 照旧是要说的话；用法那一行三语补上「也可以写 clear、stop」，是给人看的字，不登记）；`/stop` 也删（施工 LO-2，先删再打断：打断以后会话空下来，等着的循环不再交进来）。光打 `/loop` 开自主循环（施工 LO-1，项目主人 2026-11-14 定，照 Claude Code）：同 `/loop <话>` 建一行 `dynamic`、马上跑一次、之后她每轮自己调 `schedule_next`、不调就停（那一轮结束时删掉这一行，第三节第 10 条），只是 `prompt` 是出厂的一句英文 `software/loop/autonomous.txt`（没有新的指示，照自己判断接着做，定下一次或者停下；去掉行尾空白），敲命令那一刻读、原样存进那一行。不另加「自主」的标记：表的形状、到点交话、组装、`schedule_next` 都不用改，交进会话的就是存着的原文，记进 `message.user`，重放一字不差；读不出这份文件的回 `internal_error`，原来的循环不动。写错了的（只写了间隔、没写话）回 `bad_params`，`data.why` 是照连接的语言写的用法（「参数不对。用法：」，下面一行一种写法，同 `command.catalog` 的 `usage`）。回执照连接的语言：光打的「已开始循环：由 AI 自己定做什么、下一次什么时候」（`commands/loop-auto`）。群里的 `/loop` 由 im 自己截下，不到核心；私聊和本机的头一样走这一路。

### 六、标签

交进会话的那句话进请求时包一层标签（`miyu-assemble` 的 `schedule.rs`，写法照「别的 harness 发来的话」）：她分得清这是到点在叫她，不是人说的。定时任务、固定间隔的循环用 `software/schedule/common/` 的一对，她自己定节奏的循环用 `software/loop/common/` 的一对，标签里的 `{id}` 是任务编号。字在造会话时冻结进快照（`core.schedule`），四份缺一份的当没有，那种话照人的话原样渲染：以前造的快照、这一版还没写字的都是这样。字由施工 J-2 写、登记（设计 26）。

### 七、给人的通知（施工 J-3 中，形状两个头 2026-10-27 都推荐）

- `subscribe {"stream": "notices"}`：回应 `{}`，有连接认领了发后台通知的写 `{"claimed": true}`（施工 TR-1，下面最后一条）；不补以前的；扩展订不了（`local_only`）；掉队推 `resync {"stream": "notices"}`。
- `notice {"kind", "at", …}`：`schedule.ran {schedule, session, ok}`：每次新开会话的定时，交进去之前先订阅那个会话，交进去的那句开的那一轮结束时推（`schedules/ran.rs`），`ok` 是答完了；跑在已有会话上的不推（头照会话状态弹）。`schedules.reset {}`：唤醒表坏了改名留着、换了一份空的。
- 不存盘（`schedules/notices.rs`，一条就是推送的那一行，同知识库的进度）。头照 `kind` 弹系统通知。
- 谁来发后台通知（施工 TR-1，托盘 `miyu-applet` 的蓝图 `app.md` 第 5 条）：托盘这样浏览器关了也要弹的头连上时调 `notices.claim`（只给人的头），核心照连接记（`viewing.rs` 的座位），断了就放。「有没有人认领」变了往这条流推 `notices.claimed {"claimed"}`，几个认领的走了一个不推。有人认领时，别的头不再为没人开着的会话（列表那一项没有 `viewed`）、`notice` 弹系统通知，免得和托盘弹两遍；开着某个会话的头照常弹它那一个。

### 八、代码和测试

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/schedules.rs`、`schedules/{open,row}.rs` | 表：建、读、改、删、最早的、到点的；开、坏了改名、比这一版新的不动 |
| `crates/miyu-endpoint/src/schedules.rs` | 入口、核心起来时开表和跳过错过的 |
| `crates/miyu-endpoint/src/schedules/update.rs` | 改一项：换时间、换那句话，循环不改种类（施工 J-5） |
| `crates/miyu-endpoint/src/schedules/{methods,changes}.rs`、`crates/miyu-endpoint/src/refusal/schedule.rs` | 协议 `schedule.*`、一项的写法、推送的广播、拒绝（施工 J-1 下） |
| `crates/miyu-endpoint/src/schedules/{clock,next,run,fire,hold}.rs` | 钟、下一次、睡到点、交进会话（扩展建的推给它，施工 J-4）、写不进的先放一放 |
| `crates/miyu-endpoint/src/commands/looping.rs`、`commands/usage.rs`（施工 J-2、LO-1） | `/loop` 的几种写法、写错了带用法；光打的读出厂的那句 |
| `crates/miyu-endpoint/src/schedules/busy.rs`（施工 LO-2） | 等会话这一轮结束的循环：记下、会话状态变了放掉 |
| `crates/miyu-endpoint/src/schedules/ended.rs`（施工 LO-3、LO-3补） | 她自己定节奏的那一轮结束时没定下一次：删掉那一行、推 `removed`；重启了的不删；核心起来时接着干的接着盯、停在那儿的删掉 |
| `crates/miyu-kernel/src/origin.rs` | `by` 的 `schedule` |
| `crates/miyu-assemble/src/schedule.rs`、`crates/miyu-policy/src/schedule.rs`、`crates/miyu-store/src/resources/schedules.rs` | 标签：组装、快照、读资源 |

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/src/schedules/tests.rs` | 建、读、改、删，编号不复用；到点的、最早的照时刻，停着的不算；列表的先后；重开读得回原样；坏了的改名留着开空的，比这一版新的不动；一行读不懂（写法认不出、类型不对）的跳过、交在 `unreadable` 里，别的照常到点、照常列，删得掉（施工 AE-4）；`end_unpaced` 只删没定下一次、没停着的 `dynamic`（施工 LO-3） |
| `crates/miyu-endpoint/src/schedules/next/tests.rs` | cron 只认五段、照时区算、不含现在；每隔一阵的错过的跳过；一次的晚于现在才有 |
| `crates/miyu-endpoint/tests/schedules.rs` | 真核心、手拨的钟：没到点不交、到点交进去记 `by: schedule`、原话原样、照上一次往后；每次新开照建时的预设开一个、一次的跑完删；核心起来时错过的跳过；会话没了停掉；她建的照预设、人建的照跑；她自己定节奏的交进去的那一轮没定下一次，结束时删掉（施工 LO-3）；写错的不收，停、续、删；一行读不懂的跳过、`check` 报、删得掉，好的照常到点（施工 AE-4） |
| `crates/miyu-endpoint/tests/schedule_clock_back.rs`（施工 FX-2） | 系统时间往回拨了十分钟、人停了再续，cron 的下一次又是刚跑过的那一刻：照样交进去开一轮，两次的命令编号不一样 |
| `crates/miyu-endpoint/src/schedules/hold/tests.rs`、`crates/miyu-endpoint/tests/schedules_log.rs` | 写不进的放 1 秒起翻倍、最多 5 分钟、写进了清掉；表被锁住时拨过十秒只试几次、一次都不交，放开以后照常交 |
| `crates/miyu-endpoint/tests/schedule_methods.rs`（施工 J-1 下） | 建、列（照会话筛）、停、续、删，一项的写法；写错的照哪一样拒、没有的 `unknown_schedule`；每次新开照这时的默认定下人格、预设、工作目录；订阅的回应是整张表，改了的、到点交过的推 `schedules.changed`，删了的 `removed` |
| `crates/miyu-endpoint/tests/schedule_update.rs`（施工 J-5） | 改时间照现在重算下一次、列表和推送是改过的、写进了表、睡着的照新的下一次醒；改话编号和上一次留着、下一次不动；停着的改完还停着，续上照新的时间算；循环换不了种类、话能改；写错的照 `problem` 拒、什么都不改，都不写的、没有的编号照拒。扩展改不了别人建的、改得了自己的在 `schedule_package.rs` |
| `crates/miyu-endpoint/tests/loop_command.rs`（施工 J-2、LO-1） | `/loop` 固定的马上跑、之后照间隔；动态的跑一次、照她定的下一次等着、再敲换掉、`off` 删掉；没有循环包的没有这条命令；光打的建一行动态、马上交进一轮、交的和存的都是出厂的那句，她定了下一次、到点再交，第二轮的请求前缀和第一轮逐字节相同；只写了间隔的 `bad_params`、`data.why` 是用法、什么都不建 |
| `crates/miyu-endpoint/tests/loop_busy.rs`（施工 LO-2） | 她这一步停在工具上时敲的 `/loop`：这一轮里不交，结束以后交、由它开第二轮，第一轮的请求里没有它；人打断这一轮，循环照样接着跑；`/loop` 后 `/stop`：那一行删掉、这一轮结束以后也不交；循环自己那一轮里 `/stop` 同样删掉；第二轮里她没定下一次，结束时那一行删了（施工 LO-3） |
| `crates/miyu-endpoint/tests/loop_ends.rs`（施工 LO-3、LO-3补） | 动态循环自己那一轮没调 `schedule_next`：那一行删了、推了 `removed`；调了的留着、下一次是她定的、到点再跑；循环等着时人插的一轮没调的不删；`/loop clear`、`/loop STOP`、`/loop stop ` 关掉，`/loop clear the cache` 开循环。`commands/looping/tests.rs` 守读法；有计划地停下时那一行留着，接着干的一轮里她定了下一次的留着、没定的删掉；核心停着时留下的没定下一次的起来时删掉，停着的、定了的留着 |
| `crates/miyu-assemble/src/schedule/tests.rs` | 两种标签照 `loop` 挑、带任务编号；没有字的原样；`by` 的 JSON 写法、读得回 |
| `crates/miyu-store/src/resources/schedules/tests.rs` | 到点交进来的话的标签四份都在才读得出；光打 `/loop` 交进去的那句读得出出厂的原文（施工 LO-1） |
