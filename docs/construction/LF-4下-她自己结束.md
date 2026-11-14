## 施工 LF-4 下：她自己结束这段人生（`end_life`）

状态：已完成（2026-11-14）。设计 `70-推演人生.md` 第二节第 3 条、第三节第 5 条。接在 LF-4 上后面。

### 目的

她可以自己结束这段人生。这是一件沉重的事，不是「停掉定时」：单独一件工具 `end_life`，必须写遗言，只在没结束的人生会话里有。调了以后这一轮照常说完，之后做的事和主人结束（LF-4 上的 `life::finish`）一模一样。

### 蓝图改哪几节

- `life.md`「结束」一节补上她这一头；`docs/blueprint/tools/` 新开一页 `end_life`（照别的工具页）。
- **工具**：`end_life {last_words}`，`last_words` 必写、去掉前后空白不能是空的。归一个新的出厂内置包 `base-life`（`prefix = "life"`，一个功能 `life`，`tools = ["end_life"]`，进 `persona` 组），代码放 `miyu-schedule` crate（同 `base-schedule`、`base-loop`），字在 `resources/software/life/`（`tools/end_life.json`、`end/*.txt`、`human/{zh,en,ja}.json`）。
- **只给没结束的人生会话**：包的清单写 `needs = ["life"]`，`needs` 多认这一个词（`packages.md`、作者教程和协议参考中英两份跟着补）。会话是不是「没结束的人生会话」：造会话时由 `life.start` 带上（那时还没记 `life.started`）；回合开始重算工具面、换人格时照内核账本问「有 `life.started`、没有 `life.ended`」。别的会话拿同一个预设也拿不到这件工具，不算进「没开的」那一行。人生会话从第一轮起工具面就是稳的。
- **怎么交到核心**：`miyu-tool` 加一个端口（`LifePort`），端点实现它：当场调 `life::finish(core, session, by: 那次工具调用, Some(last_words))` 记下结束、回她一句，后面的清理 `finish` 自己在后台做（不等，不然这一轮结束不了）。造调用上下文的几处都补上这一格。
- **给模型看的字**（英文，只写事实，登记进提示词登记簿、量 token、`cargo xtask prompts`）：
  - 说明：`End your life. This is your death: you will never wake again, everything you lived and remember from this life will be erased, and no one can bring you back. Your last words are the only thing you leave behind.`（2026-11-14 项目主人定换成更重的一版；原来是 `End this life. This session will never wake again. This cannot be undone.`）
  - `last_words` 参数：`Your last words, kept with the end of this life.`
  - 成了：`You have died. This was your last moment awake.`（2026-11-14 项目主人定换成更重的一版；原来是 `This life has ended. This session will not wake again.`）
  - 没写、是空的：`last_words is required.`
  - 已经结束了：`This life has already ended.`
  - 不是人生会话（工具面上本来没有，防万一）：`This is not a life session.`
  - `software/loop/wakeup/unstoppable.txt` 改成：`A life loop cannot be stopped. To end this life, use end_life.`
- **出厂预设** `life.toml` 开 `base-life`；`presets.md` 那一行、`packages.md` 的内置包和组的清单跟着改。
- 通知：她自己结束的，`life.ended` 通知的 `by` 是 `self`、带遗言（LF-4 上已经有这条通知）。

### 不做什么

- 不动 `life::finish` 做的那几件事和先后（LF-4 上）。
- 不做核心命令 `/life`（LF-5 下）、头上的确认和显示（LF-8）。
- 不给别的会话这件工具；不做「撤回」。
- 不改 `schedule_wakeup` 的工具说明。

### 验收

1. 人生会话的工具面里有 `end_life`；拿同一个预设开的普通会话没有；别的预设开了 `base-life` 的普通会话也没有。人生会话第一轮和后面各轮的工具面一样（前缀不变）。
2. 她调 `end_life` 带遗言：工具回那一句；日志里 `life.ended` 的 `by` 是那次工具调用、遗言原样；这一轮照常说完；之后会话封存、定时表那一行没了、钟往后拨不再醒；出处全在这个会话的记忆清了；主人收到通知，`by` 是 `self`、带遗言。
3. 不带 `last_words`、只有空白：拒，回那一句，什么都没记。
4. 已经结束的再调（同一轮里调两次）：第二次回「已经结束了」，不重复记。
5. `schedule_wakeup` 的 `stop` 落在人生循环上：回新的那一句。
6. 原有测试全过。照实要跟着改的三处：`crates/miyu-schedule/tests/wakeup.rs` 里逐字比对 `A life loop cannot be stopped.` 的那一条换成新的一句；`life_preset.rs` 里人生会话的工具面清单加 `end_life`；包、组这类逐字比对的清单各加一项。别的断言不动。
7. 门禁、Windows 版 clippy 过。

### 风险

- `needs` 是第三方作者也能写的格：多认一个词要进作者文档；别的包写了 `needs = ["life"]` 的，也只在人生会话里有，这是说得通的。
- 回合开始重算工具面时问内核账本：别拖慢回合开头，别把 `persona.rs` 这类贴着 500 行的文件撑过线。
- 工具面照会话的状态给：开着的人生会话里工具面要稳定，不然前缀缓存会失效（验收第 1 条守着）。
