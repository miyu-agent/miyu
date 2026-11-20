## 施工 LF-4 上：结束一段人生

状态：已完成（2026-11-14）。设计 `70-推演人生.md` 第二节第 2、3 条，第三节第 5、6 条。接在 LF-5 中后面。LF-4 拆成上下两张（主会话 2026-11-14 定）：上是结束本身和主人的两个方法，下是她自己的工具 `end_life` 和带它的内置包。

### 目的

一段人生只会被有意结束。结束是一件沉重的事：这个会话不再醒、归档只读，这段人生产生的记忆和回合索引清掉，日志照常能看；结束了不能接上。这一张做结束本身（交出一个口子 `life::finish`，「下」的 `end_life` 直接调）和主人的 `life.kill`、`life.new`；`life.new` 是有就先结束、再开一条。

### 蓝图改哪几节

`life.md` 加「结束」一节，别的页跟着改：

1. **内核事件 `life.ended`**：`body` 是 `{"last_words"?}`，`by` 是结束它的（人，或者她的那次工具调用），不带回合；同 `life.started` 只由核心经内核命令 `Append` 记，`events.append` 记不了；一个会话只记一次。记了以后同封存：改对话的命令拒 `session_sealed`，这一轮结束时（空闲的当场）记 `session.sealed`，排着的话不再开轮（`session/fork.md`「封存」多一个来源）。
2. **会话索引那一格**多一种取值：结束了的存 2，照 `life.ended` 派生，不升版本（`store/index.md`）。会话列表那一项结束了的多写 `life_ended: true`。
3. **`life::finish(core, session, by, last_words)`**：两条路（主人的 `life.kill`、`life.new`，「下」的 `end_life`）做同样的几件事，照这个先后，每一步先查后做、能重做，重做不重复清、不重复推通知：
   - 记 `life.ended`（已经记过的跳过）；正在跑一轮的，主人结束的打断它；
   - 等会话封存（清理一律放在封存以后：那一轮里记下的、进索引的不漏）；
   - 删掉定时表那一行；
   - 清记忆：出处全在这个会话里、还算数的有才清（现成的按会话清，`memory.forget {clear: session}` 那条路；它本来就让摘要作废、下次整理重写；人卡不动）；
   - 回合索引：`forget_session`；载入、后台补齐共用的 `Turns::connect` 见到日志里有 `life.ended` 的不接；
   - 给主人推通知 `life.ended {persona, session, by: "self"|"owner", last_words?}`，只在这一次新记上 `life.ended` 时推。
4. **抽取拦两处**（`memory.md` 第六条）：结束了的不上闹钟；抽完落地之前再查一次，结束了的整批丢掉。不然封存当场响的闹钟会把这段人生抽回记忆（按会话清只清「那以前」的）。
5. **定时那一行的自愈**（`schedules.md`）：人生那一行到点、会话已经结束的，删掉这一行、记 `skipped:life_ended`，不叫醒她。
6. **协议**：找「这个人格的人生会话」取最新的一个，开着的、结束了的都算。`life.kill {persona}`：在跑、暂停着的照第 3 条结束；结束了的把清理剩下的几步再走一遍、原样回成功；没有的 `no_life`。回 `{"state": "ended", "session"}`。`life.new {persona}`：在跑、暂停着的先结束，结束了的先补齐清理，再照 `life.start` 开；回 `{"session", "ended"?}`。`life.status` 多一种 `ended`，带那个会话。`life.start` 碰上结束了的：先补齐清理，开一条新的。`life.pause`、`life.resume` 碰上结束了的：拒 `life_ended`，带 `data.session`。只给人调。

协议参考 `docs/reference/life.md`、`life.en.md`、`methods-v1.txt`、`errors.*`、`pushes.*`、`events.*` 跟着改。

### 定了（主会话 2026-11-14）

1. 归档只读用支线的封存（不另做一套）；会话列表那一项结束了的多写 `life_ended: true`。
2. 回合索引照日志判：结束时 `forget_session`，`Turns::connect` 开头见到 `life.ended` 的不接。
3. 摘要不另做（现成的按会话清已经让它作废重写）。抽取两处拦：结束了的不上闹钟；抽完落地之前再查一次，结束了的整批丢掉。
4. 清理一律放在封存以后；每一步先查后做、能重做；定时那一行到点发现会话已经结束的，删掉这一行、记 `skipped:life_ended`、不叫醒她。
5. `life.kill` 碰上已经结束的：把清理剩下的几步再走一遍，原样回成功（不回 `no_life`）。`life.pause`、`life.resume` 碰上结束了的都拒 `life_ended`，带 `data.session`。
6. 索引那一格存 2，不升版本。
7. 通知 `life.ended {persona, session, by: "self"|"owner", last_words?}`，只在这一次新记上的时候推。

### 不做什么

- 不做 `end_life` 工具、内置包 `base-life`、`needs = life`、改 `schedule_wakeup` 的 `stop` 那句拒绝、预设 `life.toml` 开它、提示词登记：归「下」。
- 不做核心命令 `/life`（LF-5 下）、头上的确认框（LF-8）。
- 不做复活：没有把结束了的接上的入口。
- 不删会话日志、不进回收处。
- 不另加「确认」的协议往返：确认是头的事。

### 验收

1. `life.kill`：日志里有 `life.ended`（`by` 是人、没有遗言），会话封存；定时表那一行没了；钟往后拨多久都不再醒；会话只读（发话拒 `session_sealed`）；主人收到一条通知。
2. 记忆：出处全在这个会话的条目清掉了，`memory.list`、搜、常驻那一块、联想都不再出来；别处也有出处的、别的会话记的照旧在。
3. 抽取抽不回来：结束以后封存当场响的抽取闹钟不抽，记忆里没有这段人生的新条目。
4. 抽取正在半路上时结束：那一次交回来的整批丢掉，不进记忆。
5. 回合索引：这个会话的回合搜不到了；删了回合库重建、重启载入以后还是搜不到。
6. `life.status` 回 `ended`；`life.pause`、`life.resume` 拒 `life_ended`；`life.start`、`life.new` 开出一条新的，新的想不起旧的那一段。`life.new` 在有一条在跑的时候：旧的结束、新的开出来，是两个会话。
7. 会话索引那一格：结束了的是 2；删了索引照日志重建的一样；列表带 `life_ended: true`。
8. 重做：已经结束的再 `kill` 原样成功，不再推通知、不再清；清到一半断了的（定时那一行还在）到点删掉、不叫醒她，`kill` 接着清完。
9. 原有测试一条不改、全过（逐字比对清单的照实加一项）；门禁、Windows 版 clippy 过。

### 手写变异（做完以后补证，2026-11-14）

测试和代码是一起写完才编的，没有先看到红，所以改坏七处各跑一遍新测试。逮住五个：封存的条件里去掉「人生结束了」；`finish` 不调 `forget_session`；`Turns::connect` 不看 `life.ended`；抽取落地前不查结束；每落一批不关抽取。没逮住两个：

1. **上闹钟前那一道不查**：`crates/miyu-session/src/actor/extract.rs` 里上抽取闹钟以前的 `if extractor.closed() { return; }` 去掉。逮不住，因为后面还有三道一样的：同一个文件里闹钟响了以后的两处，和 `memory/extract/job.rs` 落地前的一处。单独去掉它，结果是人生结束以后多上一次闹钟、响了照样不抽，不会把这段人生抽回记忆；只有后面三道也一起坏了才会出事。留着它是少上一次没用的闹钟。
2. **写下以后再清一次的那一步关掉**：`crates/miyu-session/src/memory/extract/job.rs` 里 `record_extraction` 以后的 `if closed.closed() { keeper.clear(...) }`。它只防一种交错：落地前那一道看过是没结束，写下以前人生结束了，而且结束那边的清记忆抢在抽取写下以前做完。测试里结束和抽取落地是先后做的，造不出这个交错，所以关掉它测试照过。真碰上的话，这一批抽出来的条目（出处在已经结束的人生会话里）会留在记忆里，到再 `life.kill` 一次，或者下次 `life.new`、`life.start` 补齐清理时才清掉。

### 风险

- 清记忆、清索引不可恢复：先记 `life.ended` 再清，清到一半断了的，重做能接着清完。
- 主人结束时正好在跑一轮：先记、再打断，等封存了再清。
