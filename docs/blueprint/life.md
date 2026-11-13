## 人生会话

状态：LF-5 中施工中（2026-11-14）：开一条、问状态、暂停、继续，出厂预设「推演人生」。结束（`life.ended`、`end_life`、`life.kill`、`life.new`）是 LF-4，命令 `/life` 是 LF-5 下，头的开关是 LF-8。

### 是什么

给一个人格开一条一直跑循环的会话（设计 `70-推演人生.md`）：她自己定下次几点醒，醒来过一段自己的日子，记下的进人格记忆。一个人格一条。它就是一个普通会话：用出厂的预设 `life` 开，醒来靠定时表里人生那一行（`loop` 是 `life`，`schedules.md`），醒来那段话是 LF-2 的三层（`schedules.md` 第三节第 4 条）。这一页写怎么开、怎么认、怎么停和续。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/life.rs` | `life.start`、`life.status`、`life.pause`、`life.resume`：查人格、照索引找这个人格的人生会话、开会话、设上下文上限、记身份、建和停续人生那一行 |
| `crates/miyu-endpoint/src/refusal/life.rs` | `life_exists`（带 `session`）、`no_life`，拒绝带上会话编号 |
| `crates/miyu-endpoint/src/lib.rs` 的 `Core::life` | 一把锁：改的三个方法排着来 |
| `crates/miyu-kernel/src/event/life.rs` | 事件 `life.started`（`kernel/events-bodies.md`）；内核命令 `Append` 的 `Life`（`input/command.rs`）记它 |
| `crates/miyu-store/src/index/row.rs` 的 `Life` | 会话索引的 `life` 一列（`store/index.md`） |
| `crates/miyu-endpoint/src/list.rs` | 会话列表那一项写 `life: true` |
| `crates/miyu-endpoint/src/schedules.rs` 的 `resume` | 人生那一行续上，下一次写成现在（`schedules.md` 第四节） |
| `resources/presets/life.toml` | 出厂预设「推演人生」（`presets.md`「出厂的几份」） |

### 对外的样子

1. **身份记在会话日志里**：`life.start` 在新会话的日志里记一条 `life.started`（`body` 是 `{}`，`by` 是开它的人，不带回合）。真相只有日志（同长期目标，`goal.md`）：定时表那一行只管下次几点醒，表坏了换成空的以后照这一条接上。记成内核的事件而不是包的 `ext.*`：`ext.<前缀>` 是给包用的，前缀是 `life` 的包、本机的头经 `events.append` 都能往别的会话里记一条，冒充人生会话、占住一个人格（施工 LF-5 中，主会话 2026-11-14 定）。`events.append` 记不了 `life.started`：只有 `life.start` 经内核命令记。
2. **会话索引一格**：`life`，照日志里有没有 `life.started` 派生（`store/index.md`），索引删了重建照样派生得出。会话列表（`session.list`、`sessions` 流）里人生会话那一项写 `life: true`，别的不写。
3. **方法**（只给人，本机、远程的头都行；核心拉起的扩展回 `local_only`，`connection/rights.rs`）。参数都是 `{persona}`；没有这个人格的 `unknown_persona`（照 `personas::resolve`，写法不对的 `bad_params`）。
   - `life.start {persona}` → `{"session"}`。
   - `life.status {persona}` → `{"state": "none" | "running" | "paused", "session"?, "next"?}`：没有人生会话的 `none`；人生那一行在、没停的 `running`，带会话和下一次；停着的、那一行不在的 `paused`，带会话。
   - `life.pause {persona}`、`life.resume {persona}` → 同 `life.status` 的回应。没有人生会话的 `no_life`。
4. **出错**：`life_exists`（`data.session` 是那一个）、`no_life`；定时表的照 `schedule.*` 的说法（`too_many_schedules`、`internal_error`），`life.start` 记了身份以后才出的多带 `data.session`。给人看的话在 `refusal/message.rs`（中、英）。
5. **出厂预设** `life`：见 `presets.md`「出厂的几份」。

### 怎么走

**1. 找这个人格的人生会话**：照会话列表的读法（`list::scan`，索引落后的照日志补到末尾），在本机的主会话里找 `life` 是开着的、人格（这一刻的，中途换了人格的照换了的）是它的，取最新的一个。只照不会变的格（父会话、场所）先挑：`life`、人格在造好以后才变，落后的那一行照它们挑会漏掉。

**2. 排着来**：`life.start`、`life.pause`、`life.resume` 在 `Core::life` 那一把锁里做完（同终端会话、场所会话找回的写法）：两个头同时开同一个人格的，后一个等前一个记完身份再找，找到了回 `life_exists`。`life.status` 只读，不拿锁。一把锁管全部人格：开、停、续很少，不同的人格排着队、结果互不相干。

**3. `life.start` 的四步**：

1. 找（第 1 条），有的回 `life_exists`。
2. 开会话：照 `session.create` 的那一条路（`Sessions::create`，命令编号是这条请求的编号），人格是参数、预设 `life`、模型照这一刻的默认（`model` 不写：照这时的 `models.chat` 记下，以后改配置这个会话照旧）、`cwd` 是 `~`、没人看着（`attended` 假，同定时任务每次新开的）、标题照常自动起。开不成的照原因回，什么都没留下。
3. 设上下文上限（设计 68 的 `context`，同 `session.configure`，命令编号接 `/context`）：32000 token（设计 70 第四节：不设的话一周后每次醒来要带二十多万 token），这一刻的模型最少要的比它大的照最少要的；是写在 `life.rs` 的数，不是配置项。再记身份（命令 `Append` 的 `Life`，编号接 `/life`）。这两样哪一样没成，删掉刚开的会话（照 `session.delete`，删不掉的记 `WARN life session not removed`），照原因回：不留一个还不是人生会话的空会话。
4. 建人生那一行：人建的、跑在这个会话上、留言是空的、第一次是现在（`Schedules::add`），于是马上醒第一次。没建成的（表挂满了、用不了）照原因回，带 `data.session`：身份已经记上了，它就是这个人格的人生会话，`life.status` 说 `paused`，`life.resume` 照第 5 条重建。

**4. `life.pause`**：找，没有的 `no_life`；那一行在、没停的停掉（`Schedules::pause`：下一次清掉）；停着的、不在的不动。

**5. `life.resume`**：找，没有的 `no_life`；那一行停着的续上（`Schedules::resume`：人生那一行的下一次写成现在，马上醒，她的留言不动）；在跑的不动；不在的照第 3 条第 4 步重建（表坏了换成空的以后，设计 70 第三节第 1 条）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-endpoint/tests/life.rs` 的 `start_opens_a_life_session_and_she_wakes_at_once` | 开出的会话人格、预设、上下文上限 32000 都对；日志里一条不带回合、人记的 `life.started`；第一次醒来的正文是出厂的推演方法、`by.loop` 是 `life`；人生那一行人建的、醒过以后照默认间隔 |
| 同上 `one_life_per_persona_even_when_two_heads_start_at_once` | 两个连接同时开同一个人格只开出一条，另一个 `life_exists` 带那一个；再开同样；表里只有一行人生；别的人格互不相干 |
| 同上 `status_says_none_running_or_paused` | 没开的 `none`；在跑的带会话和下一次；停了的 `paused` |
| 同上 `a_paused_life_sleeps_and_a_resumed_one_wakes_from_now` | 停着到点不醒；续上照现在醒、再照默认间隔；那一行删了以后状态是停着，续上照日志重建、接着醒 |
| 同上 `resuming_a_running_life_changes_nothing_and_no_life_is_refused` | 没有人生会话的停、续回 `no_life`；在跑的续上不重算 |
| 同上 `an_unknown_persona_is_refused_by_all_four` | 没有的人格四个方法都回 `unknown_persona` |
| 同上 `the_list_marks_a_life_session_and_a_rebuilt_index_still_does` | 会话列表人生会话带 `life: true`、别的不带；索引的行都没了照日志补回来一样，`life.start` 照样认得出来 |
| 同上 `resume_puts_a_life_row_at_now_and_leaves_other_loops_alone` | `Schedules::resume` 落在人生那一行上是现在、留言不动；`dynamic` 照旧 |
| `crates/miyu-endpoint/tests/life_preset.rs` | 出厂预设列出来、`hidden`、图标、步数上限；开出的会话工具面 |
| `crates/miyu-endpoint/tests/extension_rights.rs` 的 `life_methods_are_people_only` | 批了驱动会话、读写事件的扩展调四个方法回 `local_only` |
| `crates/miyu-kernel/src/session/tests/venue.rs` 的 `the_life_identity_is_appended_like_the_rest` | 内核记身份：不带回合、不开回合、`by` 照发的 |
| `crates/miyu-store/src/index/tests/life.rs` | 索引那一格照日志派生、存了读回、认不得的数读不懂 |
| `crates/miyu-kernel/tests/samples.rs` | `life.started` 的样本读写一字不差 |
