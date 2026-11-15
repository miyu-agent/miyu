## `note_change`

### 是什么

她在人生会话里记下、改、去掉自己的一处变化的工具（施工 LF-11，设计 `70-推演人生.md` 第六节，`life.md`「后天」，`memory.md` 的类 `change`）。建人格时写的提示词是天生的，推演过程里的变化是后天的：后天走记忆，人设一字不动。软件包「结束人生」（`base-life`）里另算一个功能「后天」（`change`），同 `end_life` 只在没结束的人生会话的工具面上，预设里能单独关。记下的这一类这个人格每个会话的常驻那一块里都带着（`memory.md` 第三条第 10 款），别处谁也改不了。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-schedule/src/change.rs` | 参数（`text`、`drop` 只能有一个）、长度、编号，交记忆的端口，结果的几句、给人看的说法 |
| `crates/miyu-schedule/src/lib.rs` | `life_tools` 交出 `end_life`、`note_change` 两件 |
| `crates/miyu-tool/src/memory.rs` | 名字 `NOTE_CHANGE`；端口 `MemoryPort::note`、`MemoryPort::drop_note`，要的 `Note`，拒的 `NoteRefused`（照记忆的几种，另加 `NotANote`、`Full`） |
| `crates/miyu-session/src/memory/port.rs`、`memory/keeper/change.rs` | 端口的实现：出处是这一轮、`by` 是这次调用；`Keeper::note`、`Keeper::drop_note` 查类、查算不算数、查上限（`memory.md`「工具」） |
| `crates/miyu-recall/src/memory.rs` | 类 `CHANGE`、上限 `CHANGES`（10） |
| `resources/software/life/tools/note_change.json` | 说明和参数格式 |
| `resources/software/life/change/*.txt` | 结果里给她看的自己那几句；没有这一条、已经不算了、记忆没开、写不进、参数读不懂的照记忆包的 `resources/software/memory/common/` |
| `resources/software/life/human/{zh,en,ja}.json` | 显示名、结果那几句（借记忆包的那几句给人看的照 `software/memory/common/…`） |
| `resources/packages/base-life/package.toml` | 功能 `change`（「后天」，`tools = ["note_change"]`） |

### 对外的样子

访问类别 `read`：同 `remember`、`end_life`，动的是 Miyu 自己的记忆，不是这台机器。

样本 `resources/software/life/tools/note_change.json`：

```json
{
  "description": "Keep a note of a lasting change in yourself over this life: a habit, a taste, a way of talking, how you feel about someone or something. A note is for a change that has held for a week or more and shows across many days; a new activity of the last few days, a single day's event, a plan or a wish is not one yet. Notes stay with you in every conversation, next to who you were to begin with, until you update or drop them; at most 10 are kept. Update a note only when the change itself has deepened or turned, not to add what happened today. Notes describe how you have changed; they do not lift anything you were told to always or never do.",
  "parameters": {"type":"object","properties":{"text":{"type":"string","description":"One sentence, at most 120 characters: how you were before and how you are now. Not a log of what you did."},"replaces":{"type":"string","description":"Id of an earlier note this one updates, like m12."},"drop":{"type":"string","description":"Id of a note that no longer holds. Give only this, to drop it."},"why":{"type":"string","description":"What brought the change about; with drop, why the note no longer holds."}}}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `text` | 和 `drop` 二选一 | 一处变化，不超过 120 字（`miyu_tool::TEXT_CHARS`，数 Unicode 字符） |
| `replaces` | 否 | 只跟 `text`：改的是哪一条（`m12`），得是现在算数的一条 `change` |
| `drop` | 和 `text` 二选一 | 去掉的是哪一条，得是现在算数的一条 `change` |
| `why` | 否 | 跟 `text` 的是为什么变的，记在那一条上（`ext.memory.saved` 的 `why`），给人看，不给模型看；跟 `drop` 的记进作废的原因。没写的没有 |

- 四格都可以不写，但 `text` 和 `drop` 得有一个、只能有一个；`drop` 带 `replaces` 的同两个都写。「没传」的几种写法（去掉前后空白是空的、`null`、`undefined`）当没传，同 `remember`。别的参数不认，也不报错。
- **谁有**：同 `end_life`（`end_life.md`「谁有」）：包的清单写 `needs = ["life"]`，只有没结束的人生会话。预设里关功能 `change` 的人生会话里没有它、`end_life` 还在；出厂预设 `life` 开着整包。
- **规矩在哪**：只碰 `change`、上限、出处这些在记忆那一层（端口后面的 `Keeper`），工具不判自己在哪种会话里（那由工具面管）。

**结果**（给她看的）：

| 情形 | 给她看的 | 出错 |
|---|---|---|
| 记下了 | `Noted as {id}.`（`change/saved.txt`） | 否 |
| 改了一条 | `Noted as {id}, in place of {old}.`（`change/replaced.txt`） | 否 |
| 去掉了 | `Dropped {id}.`（`change/dropped.txt`） | 否 |
| `text`、`drop` 都没给、都给了，`drop` 带 `replaces` | `Give text to note a change, or drop alone to drop one.`（`change/one-of.txt`）：不交端口 | 是 |
| 超过 120 字 | `That is {chars} characters; a note is at most {limit}.`（`change/too-long.txt`）：不交端口 | 是 |
| 已经有 10 条，新记一条 | `You already keep {limit} notes. Update one with replaces, or drop one first.`（`change/full.txt`） | 是 |
| `replaces`、`drop` 指着不是 `change` 的 | `{id} is not one of your notes of change.`（`change/not-a-note.txt`） | 是 |
| 编号没有、写法不对 | `There is no memory {id}.`（`memory/common/no-such.txt`） | 是 |
| 那一条已经改掉、作废、清掉了 | `{id} was already replaced or retired.`（`memory/common/not-current.txt`） | 是 |
| 会话没开记忆（没有端口） | `Memory is off in this session.`（`memory/common/off.txt`） | 是 |
| 写不进、读不了 | `Memory is not available right now: {error}.`（`memory/common/failed.txt`） | 是 |
| 参数读不懂（不是对象、格子不是字） | `Bad arguments: {error}.`（`memory/common/bad-args.txt`） | 是 |

拒了的什么都不记。

### 怎么走

1. 读参数：读不懂的回 `bad-args`；照上面挑出要做的那一件，挑不出的回 `one-of`，不交端口。
2. 没有记忆端口的回 `off`。
3. 记一条：超过 120 字的回 `too-long`；`replaces` 写法不对的回 `no-such`；交 `MemoryPort::note`（正文、改哪一条、为什么）。端口那一头（`Keeper::note`）：`replaces` 那一条没有的 `NoSuch`、不是 `change` 的 `NotANote`、不算了的 `NotCurrent`；新记的数这一间现在算数的 `change`（没改掉、没作废、没清掉、出处活着），到了 10 条的 `Full`；否则记一条 `ext.memory.saved`：类 `change`、出处是这一轮、听众照谁在听、`by` 是这次调用，没有 `who`、`people`、`keywords`、`place`，她写了 `why` 的带着（改一条不带上旧的那一版的）；记下了在后台补它的向量。
4. 去掉一条：`drop` 写法不对的回 `no-such`；交 `MemoryPort::drop_note`（编号、`why`）。端口那一头（`Keeper::drop_note`）同上查，记一条 `ext.memory.retired`。
5. 改、去掉不让摘要作废（`memory.md` 第七条第 16 款）。主人恢复作废的一条不看上限（`memory.restore`）。

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 显示名 `note_change` | 记下自己的变化 | Note a change |
| `change/saved` | 记成 {id} | Noted as {id} |
| `change/replaced` | 记成 {id}，替换 {old} | Noted as {id}, replacing {old} |
| `change/dropped` | 去掉了 {id} | Dropped {id} |
| `change/full` | 已经记了 {limit} 条 | Already {limit} notes |
| `change/not-a-note` | {id} 不是记下的变化 | {id} is not a note of change |
| `change/one-of` | text 和 drop 只写一个 | Give either text or drop |
| `change/too-long` | 太长，{chars} 个字 | Too long, {chars} characters |

借记忆包的那几句照 `software/memory/common/…` 的（`memory/human/`）。日文在 `ja.json`。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-schedule/tests/change.rs` | 假端口：`text` 交 `note`、`replaces` 照编号、`drop` 交 `drop_note`、`why` 跟 `text` 的交给 `note`、跟 `drop` 的原样没写的是空的，没传的几种写法当没传；都没给、都给了、`drop` 带 `replaces`、只有 `replaces`、超长、编号写错、参数读不懂的不交端口、各回各的；端口拒的五种各回各的；没有端口回记忆没开；访问类别是读、四格都可以不写 |
| `crates/miyu-schedule/tests/budget.rs` | 这个包的两份说明合计不超过 1737 字节 |
| `crates/miyu-session/tests/memory_change.rs` | 真记忆日志：记、改、去掉，出处、`by`；没有的、不算了的、不是 `change` 的各拒各的、什么都不写；满 10 条新记的拒、改一条照常、去掉一条以后又记得下 |
| `crates/miyu-endpoint/tests/life_change.rs` | 真核心：人生会话第一轮就有、普通会话拿同一个预设没有、不算没开；预设单关 `change` 的没有、`end_life` 还在；记、改、去掉回的几句，`memory.list` 列得出、出处是这个会话，`memory_search` 行里的类是 `change` |
| `crates/miyu-policy/src/preset/tests/life.rs` 的 `note_change_lives_with_end_life_and_can_be_turned_off_alone` | 结束了的、普通会话（`living` 是假）整包扣下；单关 `change` 的扣它、`end_life` 还在、算进没开的 |
| `crates/miyu-core/tests/tools.rs` | 核心的工具目录里有它，没装 `base-life` 的没有 |
