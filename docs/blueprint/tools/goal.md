## `goal`

### 是什么

长期目标的工具（施工 X-8 上，`goal.md`）：她看、设、结束这个会话的长期目标。软件包「长期目标」（`goal`）里唯一的一件；改、停、继续、清掉只走人的 `/goal`（`goal.md`「定了的」第 1 条）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-goal/src/goal.rs` | 参数、四个动作、结果的几句、给人看的说法 |
| `crates/miyu-goal/src/texts.rs` | 读结果的几句 |
| `crates/miyu-tool/src/goals.rs` | 名字 `GOAL`、三轮下限 `BLOCKED_AFTER`、端口 `GoalPort` 和它交回的样子 |
| `crates/miyu-session/src/tools/goals.rs` | 端口的实现：交给 actor，等内核回应（`session/tools.md`） |
| `crates/miyu-session/src/actor/goal/asked.rs` | actor 怎么办：看的当场答，改的交内核 `Goal` 命令，三轮下限（`session/actor.md`） |
| `resources/software/goal/tools/goal.json` | 说明和参数格式 |
| `resources/software/goal/goal/*.txt`、`common/*.txt` | 结果里给她看的几句，两段收尾 |
| `resources/software/goal/human/{zh,en,ja}.json` | 显示名、结果那一句 |
| `resources/packages/goal/package.toml` | 软件包的清单：名字、说明，没写功能，整个包算一个功能 `goal` |

### 对外的样子

访问类别 `read`：只改这个会话自己的目标，只读开着也能用。说明三句：是什么、什么时候设（要很多轮的活，设了会话闲着就自己开下一轮）、怎么结束。三轮下限、收尾都写在调用以后的结果里，不写进说明。

样本 `resources/software/goal/tools/goal.json`：

```json
{
  "description": "Read, set or end this session's long-running goal. Set one only for work that takes many rounds: while it runs, the session starts a new round by itself whenever it is idle. End it with complete once the result is verified, or with blocked and a reason when you cannot go on.",
  "parameters": {"type":"object","properties":{"action":{"type":"string","enum":["get","set","complete","blocked"]},"objective":{"type":"string","description":"With set."},"max_rounds":{"type":"integer","description":"With set. Default no limit."},"reason":{"type":"string","description":"With blocked."}},"required":["action"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `action` | 是 | `get`、`set`、`complete`、`blocked`；别的是参数不对 |
| `objective` | `set` 时 | 目标原文 |
| `max_rounds` | 否 | `set` 时的轮数上限，正整数；不写的不限 |
| `reason` | `blocked` 时 | 卡在哪 |

- 别的参数不认，也不报错；一条路径都不报，不报效果。
- **谁有**：长期目标开着的本机主会话；群里、私聊、子代理都没有（`session/tools.md`「工具面」），执行器也只给它们端口。

**结果**（给她看的）：

| 动作 | 成了 | 没成 |
|---|---|---|
| `get` | `Objective: <原文>`、`State: <running 这类> · Round 3 of 10`（不限的 `Round 3`），卡住的再一行 `Blocked: <原因>`（`now.txt`、`reason.txt`）；没有目标 `No goal is set.`（`none.txt`） | — |
| `set` | `Goal set. From now on a new round starts by itself whenever the session is idle.`（`set.txt`）：当场算授权过（`goal.md`「在新的架构里怎么放」第 3 条） | 已经有一个：说只有人能用 `/goal` 改、清掉（`exists.txt`） |
| `complete` | 续轮开的那一轮里：收尾（`wrapup-completed.txt`）；人开的那一轮：`Goal marked complete.`（`completed.txt`） | 没有目标：`none.txt` |
| `blocked` | 续轮开的那一轮里：收尾，带上原因（`wrapup-blocked.txt`）；人开的那一轮：`Goal marked blocked.`（`blocked.txt`） | 续轮开的那一轮里、目标开过的轮数不到 3：说这是第几轮、至少几轮（`too-early.txt`）；没有目标：`none.txt` |

- 原文、原因照模板的转义换进去（引号、换行写成转义，`08-上下文投影.md` 第五节）。
- 内核拒的别的原因（原文空白、会话封存了这类）、会话停了：`The goal was not changed: <原因码>.`（`failed.txt`，停了的写 `stopped`）。
- 收尾两段照搬旧版的措辞（`26-提示词.md` 第六节）：写收尾给人，做了什么、怎么验的、产出在哪；卡住的说做到哪、卡在哪、要人做什么；这一次自动运行里不再调工具。她接着在同一轮写，不另开一轮。

### 怎么走

1. 读参数：读不成的（少了格、动作不认识、上限是 0），交回参数不对的那一句，不交端口。
2. 没有端口的（这个会话没有长期目标）：说这个会话用不了。
3. 交端口：会话层的 actor 看这一次调用还在跑、没叫它停过，才办。`get` 照会话状态当场答（会话状态的 `goal`，`view.md`）；`blocked` 在续轮开的那一轮里、目标跑着、开过的轮数不到 3 的当场拒；别的交内核 `Goal` 命令（编号 `goal-<调用编号>`，`by` 是这次调用），内核照现在的目标判，拒的原因原样交回来。
4. 内核接受了的落了盘才交回；交回的是改之前的目标、这次调用在不在续轮开的那一轮里，结果照它写。

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 显示名 `goal` | 长期目标 | Goal |
| `goal/now` | 查看目标 | Read the goal |
| `goal/none` | 无目标 | No goal |
| `goal/set` | 已设定目标：{objective} | Goal set: {objective} |
| `goal/exists` | 已有目标 | A goal is already set |
| `goal/completed` | 目标已完成 | Goal complete |
| `goal/blocked` | 目标受阻：{reason} | Goal blocked: {reason} |
| `goal/too-early` | 第 {round} 轮，暂不能标记受阻 | Round {round}, too early to mark blocked |
| `goal/failed` | 目标未更改：{reason} | Goal not changed: {reason} |
| `common/bad-args` | 参数不对：{error} | Bad arguments: {error} |
| `common/off` | 此会话不支持长期目标 | Goals are not available in this session |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-goal/tests/tool.rs` | 四个动作交给端口的、结果逐字节比、续轮里的收尾、人开的那一轮不带、端口拒的五种、参数不对、没有端口、给人看的说法（假端口） |
| `crates/miyu-goal/tests/budget.rs` | 说明不超过 690 字节 |
| `crates/miyu-session/tests/goal_tool.rs` | 真 actor：她自己设的当场跑、续轮消息说是她设的；第 1 轮报卡住拒、第 3 轮成、带收尾、之后不再开；报完成同样；再设拒、看的照会话状态；只有本机的主会话有它 |
| `crates/miyu-core/tests/tools.rs` | 清单装了就登记，没装没有 |
