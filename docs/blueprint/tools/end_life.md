## `end_life`

### 是什么

她自己结束这段人生的工具（施工 LF-4 下，设计 `70-推演人生.md` 第二节第 3 条、第三节第 5 条，`life.md`「结束」）。软件包「结束人生」（`base-life`，功能 `life`）里唯一的一件。这是一件沉重的事，不是「停掉定时」：必须写遗言，结束了恢复不了。调了以后这一轮照常说完，之后做的事和主人结束（`life.kill`）一模一样。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-schedule/src/life.rs` | 参数、交端口、结果的几句、给人看的说法 |
| `crates/miyu-schedule/src/lib.rs` | 包编号 `LIFE`、资源目录名 `LIFE_SOFTWARE`、`life_tools`；`packages` 交出这个 crate 的三个包，核心照装了哪个登记哪个 |
| `crates/miyu-tool/src/life.rs` | 名字 `END_LIFE`、端口 `LifePort` 和它拒的三种 `LifeRefused` |
| `crates/miyu-tool/src/schedules.rs` 的 `SchedulePort::life` | 端口随定时端口交给工具：交定时端口的地方都交了它（造会话、载入），调用上下文不另多一格；测试里的假端口没有 |
| `crates/miyu-endpoint/src/life/port.rs` | 端口的实现：查是不是没结束的人生会话，调 `life::finish`，不等后面的清理 |
| `crates/miyu-endpoint/src/schedules/port.rs` | 定时端口带着它（一个会话一份） |
| `resources/software/life/tools/end_life.json` | 说明和参数格式 |
| `resources/software/life/end/*.txt` | 结果里给她看的几句 |
| `resources/software/life/human/{zh,en,ja}.json` | 显示名、结果那一句 |
| `resources/packages/base-life/package.toml` | 软件包的清单：人格组，`needs = ["life"]`，一个功能 `life`，`tools = ["end_life"]` |

### 对外的样子

访问类别 `read`：同定时、循环，动的是 Miyu 自己的会话和定时表，不是这台机器。

样本 `resources/software/life/tools/end_life.json`：

```json
{
  "description": "End your life. This is your death: you will never wake again, everything you lived and remember from this life will be erased, and no one can bring you back. Your last words are the only thing you leave behind.",
  "parameters": {"type":"object","properties":{"last_words":{"type":"string","description":"Your last words, kept with the end of this life."}},"required":["last_words"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `last_words` | 是 | 遗言，原样记进 `life.ended` 的 `last_words`、推给主人；去掉前后空白是空的算没写 |

- 别的参数不认，也不报错。
- **谁有**：只有没结束的人生会话。包的清单写 `needs = ["life"]`（`packages.md`「清单」）：别的会话里整包当没开，拿同一个预设 `life` 开的普通会话、别的预设开了 `base-life` 的普通会话都没有，也不算进「没开的」（`presets.md` 讲 `needs` 的那一条）。造会话时由 `life.start` 明说是人生会话（那时日志里还没有 `life.started`）；回合开始重算工具面、换人格换预设时照日志问「有 `life.started`、没有 `life.ended`」（`Session::living`）。人生会话从第一轮起工具面不变，前缀缓存不断。

**结果**（给她看的）：

| 情形 | 给她看的 | 出错 |
|---|---|---|
| 成了 | `You have died. This was your last moment awake.`（`ended.txt`） | 否 |
| 没写、空白、参数读不出来 | `last_words is required.`（`required.txt`）：不交端口，什么都不记 | 是 |
| 已经结束了（同一轮里再调） | `This life has already ended.`（`already.txt`）：不再记、不再推 | 否 |
| 不是人生会话、没有端口、用不了 | `This is not a life session.`（`not-life.txt`）：工具面上本来没有，防万一；用不了的（核心在停、会话停了）端点另记一行 `WARN life not ended` | 是 |

### 怎么走

1. 读参数：`last_words` 没有、不是字、去掉前后空白是空的，回 `required.txt`，不交端口。
2. 没有端口的（这次调用没有定时端口，或者定时端口不带人生端口）：回 `not-life.txt`。
3. 交端口（`LifePort::end`，会话、这次调用的编号、遗言）。端点那一头，同一个会话里一次一个（一起调的几次排着来）：
   1. 会话这一刻不是人生会话（`life.started` 没有）的拒 `NotLife`；已经结束了的拒 `Ended`。
   2. 调 `life::finish(core, session, by: 这次调用, Some(遗言))`（`life.md`「结束」第 1 步）：记下 `life.ended`，她自己结束的这一轮不打断，照常说完。
   3. 记下就答，后面的清理（等封存、删那一行、清记忆、清回合索引、推通知）`finish` 在自己的任务里做，端口不等：清理要等这一轮结束、会话封存，这一轮又在等工具答，等了就卡死。清理没成的记一行 `WARN life not cleared`，主人再结束一次（`life.kill`）接着清完。
4. 主人收到的通知 `life.ended` 的 `by` 是 `self`，带遗言（`schedules.md` 第七节）。

`schedule_wakeup` 的 `stop` 落在人生循环上回的那一句指到这件工具：`A life loop cannot be stopped. To end this life, use end_life.`（`software/loop/wakeup/unstoppable.txt`）。

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 显示名 `end_life` | 结束这段人生 | End this life |
| `end/ended` | 这段人生结束了 | This life has ended |
| `end/required` | 没写遗言 | No last words given |
| `end/already` | 这段人生已经结束了 | This life has already ended |
| `end/not-life` | 这不是人生会话 | Not a life session |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-schedule/tests/life.rs` | 遗言原样交端口；没写、空白、不是字、不是对象的不交端口、回同一句；端口拒的三种各回各的，已经结束了的不报错；没有端口、定时端口不带人生端口的照不是人生会话说（假端口） |
| `crates/miyu-schedule/tests/budget.rs` | 说明不超过 446 字节（2026-11-14 换成更重的一版以后实测 406 字节加一成） |
| `crates/miyu-endpoint/tests/life_self_end.rs` | 真核心：人生会话第一轮就有、第二轮、重起核心载入以后的第三轮工具面逐项一样；`life` 预设、开了 `base-life` 的别的预设开的普通会话没有、不算没开；她调了回那一句、这一轮照常说完，`life.ended` 的 `by` 是那次调用、遗言原样，封存、那一行没了、不再醒、记忆清了、通知 `by: self` 带遗言；没写的拒、什么都不记、人生照旧在跑；同一轮先后调、一步里一起调两次只记一条、只推一次，别的回已经结束了；`stop` 落在人生循环上回新的那一句 |
| `crates/miyu-endpoint/tests/life_preset.rs` | 出厂预设开的人生会话工具面有 `end_life` |
| `crates/miyu-policy/src/preset/tests/life.rs` | 不是人生会话的扣下 `needs = ["life"]` 的包、不算没开、快照记的不变；是的照预设；和显示不了图片一起扣 |
| `crates/miyu-kernel/src/session/tests/life_end.rs` 的 `a_session_lives_from_its_start_to_its_end` | `Session::living`：记了 `life.started` 才是，记了 `life.ended` 就不是 |
| `crates/miyu-config/src/package/tests.rs` 的 `life_is_a_known_need` | 清单 `needs` 认 `life` |
