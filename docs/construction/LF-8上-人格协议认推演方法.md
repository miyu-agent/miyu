## 施工 LF-8 上：人格的协议认推演方法

状态：已完成（2026-11-14）。设计 `70-推演人生.md` 第二节第 8、9 条、第三节第 3、9 条。接在 LF-4 下后面。

### 目的

推演方法（人格目录里的 `prompts/life.md`）现在只能人手放文件（施工 LF-2 留的：「编辑它的协议、界面随后面的步子做」）。头的人格页要一个「推演方法」文本框（LF-8），得能经协议读、写这份文件。照人格别的提示词（人设、示范对话、角色扮演提示）已有的读写走，多认一个名字 `life`，不另起方法。

### 蓝图改哪几节

`personas.md`「目录」第 6 条（去掉「编辑它的协议、界面随后面的步子做，现在人直接放文件」，换成怎么读写）、讲 `persona.get`、`persona.read`、`persona.set` 的那几条；协议参考 `docs/reference/personas.md`、`personas.en.md` 三处跟着改：

| 方法 | 多了什么 |
|---|---|
| `persona.get`（`persona.set` 的回应同） | `prompts` 多一格 `life`：布尔，叠好以后推演方法有没有字（去掉末尾空白不是空的）。没有字的人格用出厂的那一份 |
| `persona.read` | `prompt` 多认 `life`。回应 `{"text", "version"}` 同别的：`text` 是叠好的那一份的原文（哪一层都没有的 `null`；上面一层的空文件盖住下面的，这时 `text` 是空的字），`version` 是你那一层这一份的版本。另带 `default`：出厂的推演方法原文（`resources/software/life/method.txt`，去掉末尾空白），头照它在文本框里显示「没写时用的是这一份」。读不出出厂那一份的 `default` 是 `null`，不算错 |
| `persona.set` | `prompts` 多认 `life`：`{"text": <整份>}` 或 `{"unset": true}`，可带 `expect`，同人设、角色扮演提示（不认 `pairs`）。写的是你那一层的 `prompts/life.md` |

- 别的三个名字的行为一点不变；`default` 只有 `life` 带。
- 写了以后什么时候生效：人生循环每次醒来那一刻读（施工 LF-2），下一次醒来就用新的；不动在跑的会话的快照。这一步不碰那一段。
- 新建人格（`persona.set` 不写编号）时也可以一起带 `life`。

### 不做什么

- 不做界面（头的会话）。
- 不把推演方法放进 `persona.list`。
- 不动人生循环读它的那一段（`schedules/life.rs`），不动出厂的那一份字。
- 不加新的拒绝原因。

### 验收

1. `persona.read {prompt: "life"}`：哪一层都没有的 `text`、`version` 都是 `null`，`default` 是出厂那一份的原文；你那一层写了的回原文和版本；包、出厂那一层带了的读得到它的原文、`version` 是 `null`。
2. `persona.set` 写 `life` 的 `text`：文件落在你那一层的 `prompts/life.md`，再读回来一样；带对的 `expect` 写得进，带错的拒 `persona_conflict`、文件没变；`unset` 删掉你那一层的，回到下面那一层的（没有就没有）。
3. `persona.get` 的 `prompts.life`：没有的、只有空白的是 `false`，有字的是 `true`；`persona.set` 的回应里也是改完的样子。
4. `life` 带 `pairs` 的拒 `bad_params`，什么都不写。
5. 经协议写的推演方法，人生循环下一次醒来用的就是它（接上 LF-2 已有的那条路：一条端到端的测试，写完到点、看交进会话的正文开头）。
6. 别的三个提示词名原有的测试一条不改、全过；`persona.read` 别的名字的回应里没有 `default`。
7. 门禁、Windows 版 clippy 过。

### 风险

- `Found` 要多记一格「推演方法来自哪一层」（`persona.read` 照它找原文）：照 `reminders_from` 的写法加，别动 `life` 那一格现有的语义（上面一层的空文件盖住下面的）。
- 协议参考中英两份、蓝图一起改，头的会话要照它做。
