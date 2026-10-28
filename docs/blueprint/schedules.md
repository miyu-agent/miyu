## 定时和循环

设计 `45-定时和循环.md`。核心里只有一样东西：到点把一句话交给一个会话跑一轮的「唤醒」，记在一张表里，核心重启不丢。定时任务、固定间隔的循环、她自己定节奏的循环都是它。

状态：J-1 上做完（唤醒表、到点、交进会话，2026-10-26），J-1 下做完（协议 `schedule.*`、`schedules` 流）；她的工具、`/loop`、标签的字是 J-2；命令行、系统通知是 J-3；J-4 做完（2026-10-27）：扩展能建，到点推给建它的扩展（第五节第 7 条），场所会话的循环由场所自己拿它拼。

### 一、唤醒表

一个账号一份 `home/<账号>/state/schedules.db`（SQLite，`miyu_store::schedules`）。它是真相，不是派生的，和会话列表的索引、用量汇总不一样：

1. **开**：没有就建，`user_version` 是 1。只加表、加索引的改动不升版本。
2. **坏了**：读不了、坏了的连同 `-wal`、`-shm` 改名成 `schedules.db.broken-<毫秒>` 留着，开一份空的，记 `WARN schedules set aside`。人的定时没了要告诉他：施工 J-3 中起另推 `notice {"kind": "schedules.reset"}`（第七节「给人的通知」）。
3. **比这一版新**：新的程序写过、又换回旧的，不动它，这一回不用，记 `WARN schedules newer`。回到新的程序照样在。
4. **写盘**：`synchronous = FULL`，答应了的改动断电也在。一个账号一个连接，开了一直开着。
5. **用到时开**：人、她建第一个任务时建；核心起来时只开数据根里已经有表的账号。

一行一个任务：

| 格 | 是什么 |
|---|---|
| `id` | `t<n>`，账号里递增，删了的号不再用（`counter` 表） |
| `created_at` | 建的时刻，毫秒 |
| `by` | 谁建的，JSON：`{"kind":"person","account"}`（人：命令行、协议）、`{"kind":"model","session"}`（她：在这个会话里用工具建的，施工 J-2）、`{"kind":"package","package"}`（包，施工 J-4） |
| `when_json` | 什么时候跑，JSON：`{"cron":{"cron":"0 8 * * *","recurring":true}}`、`{"at":{"at":"<时刻>"}}`、`{"every":{"every":600}}` |
| `prompt` | 交进会话的那句话，原样存；标签组装时包，不在这里 |
| `target` | 跑在哪，JSON：`{"session":{"session":"<编号>"}}`，或 `{"new":{"persona"?,"preset","cwd"}}`（每次新开，人格、预设、工作目录建的时候就定下来） |
| `target_session` | 跑在哪个已有会话，好查；每次新开的没有 |
| `loop` | `fixed`、`dynamic`，没有的是定时任务（施工 J-2 的 `/loop`、`schedule_wakeup`） |
| `paused` | 停着：不到点 |
| `next_at` | 下一次；停着的、她自己定节奏的循环这一轮还没定的没有 |
| `last_at`、`last_result`、`last_session` | 上一次：时刻、`ran` \| `queued` \| `skipped:<原因>`、交给了哪个会话 |

### 二、什么时候跑

1. **cron**：只认标准五段（分、时、日、月、星期），不认秒和年，照这一个记下的时区算（施工 J-3 上：`when.zone`，IANA 名；建的时候人交的，不交的记这台机器那一刻的；以前建的没有，照本机时区；记下的认不出了也照本机），读不出本机时区的当 UTC。日、星期两段照 Vixie cron 的「或」。夏令时照 OCPS 规范：跳过的那一小时里的时点挪到跳过以后，重复的那一小时只算一次。库是 croner 4.0.1（`jiff` 特性，不带 chrono）。`recurring` 是假的跑一次就删。
2. **某个时刻一次**：建的时候已经过了的不收（核心的入口另有 `first`，见第四节）。跑过就删。
3. **每隔一阵**：最短 60 秒。下一次照上一次往后数整数个间隔，错过的几个跳过，正好是现在这一刻的也算过去了。
4. **错过的不补**（同旧版定时消息）：核心起来时已经过了的一律跳过，记 `INFO schedule missed`。一次的删掉，她自己定节奏的停着等，别的照现在往后找下一次。核心开着时醒晚了的（机器睡过一觉）照常跑一次。

### 三、到点

1. 核心起来时（`Core::start_schedules`）开上已有的表、跳过错过的，然后一个后台任务只睡到各账号里最早的那一次。表一改（建、停、续、删、改下一次）就叫醒它重算；不轮询。真的钟一次最多睡一小时再看一眼：改了系统时间、机器睡过一觉，最多晚这么久发现。测试换手拨的钟（`Clock`，`Core::with_clock`）。
2. 到点的一个：先把下一次写进表（一次的删掉，她自己定节奏的清掉下一次），再交进会话，最后只记上一次那几格。顺序不能反：她在那一轮里定下一次（`schedule_wakeup`，施工 J-2）不会被这里盖掉。写不进下一次的不交，记 `WARN schedule not advanced`：交了又记不住，下一次醒来会再交一遍。
3. **写不进的先放一放**（2026-10-26 主会话审出来的）：表里还是过去那个时刻，不放的话马上又醒、又写不进，空转吃满一个核。只在内存里给它记一个「不早于」（`schedules/hold.rs`）：头一次 1 秒，再写不进翻倍，最多 5 分钟；算最早一次、挑到点的都照它往后挪，写进了就清掉。不碰表，核心重启就忘。表的连接锁住了不等（`busy_timeout = 0`），当场报错交给这里退避；睡到点的那个每一轮让一下线程，不霸着。
4. **跑在已有的会话**：交一条 `Send`，`by` 是 `{"kind":"schedule","schedule":"t3","account":"alice","loop"?:"fixed"|"dynamic"}`（`kernel/ids.md`「谁」）。照「别处来的话」的规矩收（`kernel/session.md`「定时交进来的话」）：闲着开一轮，正在跑一轮的排在这一轮后面。`last_result` 照交的那一刻会话忙不忙记 `queued` 或 `ran`。命令编号 `schedule/<任务>/<这一次的毫秒>/say`：核心重启后再交一遍，会话认得是重发。
5. **每次新开**：照建时定下的人格、预设、工作目录开一个会话（命令编号 `…/open`），没人在场（要问人的照规矩拒），再照第 3 条交。`last_session` 记开出来的那个。开不成的记 `skipped:not_opened`。跑完给人推 `notice {"kind": "schedule.ran"}`（施工 J-3 中，第七节）。
6. **会话没了**、不是这个账号的：记 `skipped:session_gone`，任务停掉，留给人看、给人删。
7. **她建的照预设**（项目主人 2026-10-26 定）：`by` 是 `model` 的，到点时看那个会话现在的预设，关了建它的那个包的这一次不跑，记 `skipped:preset_off`，下一次照算。定时任务看 `schedule` 包，循环（`loop` 是 `fixed`、`dynamic`）看 `loop` 包；包的功能里装着、没被关掉的有一个就算开（`presets::package_on`）。没装的包算关；没有预设的老会话都开着；无工具（空的编号）算关。人建的、包建的不看预设。
8. 每一次记 `INFO schedule fired`，带账号、任务编号、结果；不记那句话。

### 四、核心的入口

`Core::schedules()` 交回 `Schedules`，都照账号：

| 入口 | 做什么 |
|---|---|
| `add(account, Wanted)` | 建一个：cron 读不成、算不出第一次、隔得少于 60 秒的不收，什么都不建。`Wanted.first` 写了的照它（`/loop` 马上先跑一次写现在，施工 J-2），不写的照 `when` 从现在往后算 |
| `list(account)` | 全部：有下一次的照时刻在前，停着的、没定下一次的在后，照编号；还没有表的是空的 |
| `get(account, id)` | 一个；没有的是 `Unknown` |
| `cancel(account, id)` | 删掉 |
| `pause(account, id)` | 停：清掉下一次 |
| `resume(account, id)` | 续：照现在重算下一次，错过的不补；一次的已经过了的报 `NoNext` |
| `set_next(account, id, next, prompt)` | 改下一次，`prompt` 写了的换掉那句话（她自己定节奏的循环每轮定一次，施工 J-2）；`next` 没有的停着等 |

协议 `schedule.*` 和推送照第五节（施工 J-1 下）。

### 五、协议（施工 J-1 下，形状 2026-10-26 主会话审过；协议参考 `docs/reference/schedules.md`）

都照调用者的账号那一份表，别人的编号一律 `unknown_schedule`。

1. `schedule.add {when: {"cron", "recurring"?, "zone"?} | {"at"} | {"every"}, prompt, target?: {"session"} | {"new": {persona?, preset?, cwd?}}}`：不写 `target` 的是每次新开（人建的默认），人格、预设照这时的默认定下来（`personas::resolve`、`presets::resolve`，找不到的照开会话回 `unknown_persona`、`unknown_preset`），`cwd` 不写照家目录。回 `{"schedule": <一项>}`。写错的什么都不建，回 `bad_params`，`data.problem` 是 `when`（三种没写对一种、时刻写法不对）、`cron`、`past`、`short`、`prompt`（空的）、`session`（没有、不是他的）、`target`（两种都写了）、`zone`（不认识，施工 J-3 上）、`cwd`（每次新开的工作目录建时就查：照开会话的写法换成真实位置，不在、不是目录、落在数据根里又不在他自己工作区里的，施工 J-3 上）。这个账号已有 `PER_ACCOUNT`（200）个的回 `too_many_schedules`：人建、她建、包建合计，熔断性质的安全上限（项目主人 2026-10-27 定），已有的照跑；她经工具建的照 `software/schedule/cron/account-full.txt` 告诉她叫人删。时刻照核心的写法（带毫秒）。带 `target.new.cwd` 的只给本机的连接（`reach.rs`，同项目的 `workspace`）。
2. `schedule.list {session?}`：写了 `session` 的只列跑在它上面的，回 `{"schedules": [<一项>…]}`。
3. `schedule.cancel {id}` 回 `{}`；`schedule.pause {id}`、`schedule.resume {id}` 回 `{"schedule": <一项>}`。
4. 一项：`{"id","when","prompt","target","by","recurring","paused","loop"?,"next"?,"last"?: {"at","result","session"?},"created"}`。
5. 推送：`subscribe {"stream":"schedules"}` 回 `{"schedules":[…]}`；之后推 `schedules.changed {"id","entry"}`，删了的 `{"id","removed":true}`；掉队推 `resync {"stream":"schedules"}`。建、停、续、删、改下一次、到点交过都推（`schedules/changes.rs`，一条是「账号、换行、编号」，同项目的）；只推自己账号的。
6. 谁能调：人，成员也能（`connection/gate/members.rs`）；扩展要批过 `sessions.drive`（施工 J-4，`connection/rights.rs`），建的 `by` 记成包，只看得到、改得了自己建的（`methods.rs` 的 `mine`、`own`），别的编号一样 `unknown_schedule`。`schedules` 流只给人，扩展回 `local_only`。
7. 推给扩展（施工 J-4，形状主会话 2026-10-27 和 onebot 定）：`target` 多一种 `{"package": {}}`，只有扩展能写（人写回 `bad_params`），不带 `loop`。到点不进会话：`fire.rs` 的 `pushed` 照 `by` 的包找那个包连着的连接（`Provided::linked`，不替它拉起），推通知 `schedule.fired {"id","at","prompt","missed"?}`，结果记 `sent`，推不到的记 `skipped:package_unavailable`。`missed` 是核心停着时错过的次数：起来跳过时（`run::skip`、`next::missed`，最多数到 1000）累加进表里 `target` 那一格，推过就清零。扩展握手成了另记一份连着的（`Provided` 的 `linked`），推通知照它找，不登记工具的扩展也推得到；工具调用照旧只认登记了工具的。

### 六、标签

交进会话的那句话进请求时包一层标签（`miyu-assemble` 的 `schedule.rs`，写法照「别的 harness 发来的话」）：她分得清这是到点在叫她，不是人说的。定时任务、固定间隔的循环用 `software/schedule/common/` 的一对，她自己定节奏的循环用 `software/loop/common/` 的一对，标签里的 `{id}` 是任务编号。字在造会话时冻结进快照（`core.schedule`），四份缺一份的当没有，那种话照人的话原样渲染：以前造的快照、这一版还没写字的都是这样。字由施工 J-2 写、登记（设计 26）。

### 七、给人的通知（施工 J-3 中，形状两个头 2026-10-27 都推荐）

- `subscribe {"stream": "notices"}`：回应 `{}`，不补以前的；成员也能订，只收自己账号的；扩展订不了（`local_only`）；掉队推 `resync {"stream": "notices"}`。
- `notice {"kind", "at", …}`：`schedule.ran {schedule, session, ok}`：每次新开会话的定时，交进去之前先订阅那个会话，交进去的那句开的那一轮结束时推（`schedules/ran.rs`），`ok` 是答完了；跑在已有会话上的不推（头照会话状态弹）。`schedules.reset {}`：唤醒表坏了改名留着、换了一份空的。
- 不存盘（`schedules/notices.rs`，一条是「账号、换行、推送的那一行」，同知识库的进度）。头照 `kind` 弹系统通知。

### 八、代码和测试

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/schedules.rs`、`schedules/{open,row}.rs` | 表：建、读、改、删、最早的、到点的；开、坏了改名、比这一版新的不动 |
| `crates/miyu-endpoint/src/schedules.rs` | 入口、核心起来时开表和跳过错过的 |
| `crates/miyu-endpoint/src/schedules/{methods,changes}.rs`、`crates/miyu-endpoint/src/refusal/schedule.rs` | 协议 `schedule.*`、一项的写法、推送的广播、拒绝（施工 J-1 下） |
| `crates/miyu-endpoint/src/schedules/{clock,next,run,fire,hold}.rs` | 钟、下一次、睡到点、交进会话（扩展建的推给它，施工 J-4）、写不进的先放一放 |
| `crates/miyu-kernel/src/origin.rs` | `by` 的 `schedule` |
| `crates/miyu-assemble/src/schedule.rs`、`crates/miyu-policy/src/schedule.rs`、`crates/miyu-store/src/resources/schedules.rs` | 标签：组装、快照、读资源 |

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/src/schedules/tests.rs` | 建、读、改、删，编号不复用；到点的、最早的照时刻，停着的不算；列表的先后；重开读得回原样；坏了的改名留着开空的，比这一版新的不动 |
| `crates/miyu-endpoint/src/schedules/next/tests.rs` | cron 只认五段、照时区算、不含现在；每隔一阵的错过的跳过；一次的晚于现在才有 |
| `crates/miyu-endpoint/tests/schedules.rs` | 真核心、手拨的钟：没到点不交、到点交进去记 `by: schedule`、原话原样、照上一次往后；每次新开照建时的预设开一个、一次的跑完删；核心起来时错过的跳过；会话没了停掉；她建的照预设、人建的照跑；她自己定节奏的交过以后停着等、改了下一次照新的那句再交；写错的不收，停、续、删 |
| `crates/miyu-endpoint/src/schedules/hold/tests.rs`、`crates/miyu-endpoint/tests/schedules_log.rs` | 写不进的放 1 秒起翻倍、最多 5 分钟、写进了清掉；表被锁住时拨过十秒只试几次、一次都不交，放开以后照常交 |
| `crates/miyu-endpoint/tests/schedule_methods.rs`（施工 J-1 下） | 建、列（照会话筛）、停、续、删，一项的写法；写错的照哪一样拒、没有的 `unknown_schedule`；每次新开照这时的默认定下人格、预设、工作目录；订阅的回应是整张表，改了的、到点交过的推 `schedules.changed`，删了的 `removed` |
| `crates/miyu-assemble/src/schedule/tests.rs` | 两种标签照 `loop` 挑、带任务编号；没有字的原样；`by` 的 JSON 写法、读得回 |
| `crates/miyu-store/src/resources/tests.rs` 的 `schedule_texts_need_all_four_files` | 四份都在才读得出 |
