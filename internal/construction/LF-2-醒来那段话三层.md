## 施工 LF-2：人生循环醒来那段话分三层

状态：已完成（2026-11-14）；三份字照草稿登记，等 LF-5 下拿真模型走一遍再定稿。设计 `70-推演人生.md` 第三节第 3 条。接在 LF-1 后面。

### 目的

人生循环到点交进会话的那一句，从「一句话包循环的标签」换成三层：固定的框（讲机制，人改不掉）、推演方法（人格目录里的 `prompts/life.md`，没有的用出厂的一段）、她上次留给自己的话。定时表那一行的 `prompt` 从此只存她的留言。

### 蓝图改哪几节

- `schedules.md` 第六节（标签）：`by.loop` 是 `life` 的用人生自己的一对，`resources/software/life/common/message-open.txt`、`message-close.txt`，标签里的 `{id}` 照旧是任务编号；造会话时冻结进快照的 `core.schedule`。以前造的快照里没有这一对的，`life` 的那句照人的话原样渲染（同别的缺字的）；以前的快照照旧读得回、指纹不变。
- `schedules.md` 第三节第 4 条（跑在已有的会话）：`life` 的交进去的正文由核心当场拼：先是推演方法，空一行，再是那一行的 `prompt`（她的留言）；留言是空的只有推演方法。拼好的原样记进 `message.user`，重放一字不差。
- 推演方法从哪来：这个会话的人格（照它的快照）的 `prompts/life.md`，照人格三层的叠法找；去掉末尾空白是空的、没有这个文件、会话不带人格的，用出厂的 `resources/software/life/method.txt`。到点那一刻读，改了文件下一次醒来就用新的。`personas.md`「目录」那一段加上 `life.md`。
- `schedules.md` 第四节 `set_next`、第五节讲 `schedule_wakeup` 的那一条：落在 `life` 行上，这一轮没写 `prompt` 的把留言清成空的（不把旧的再念一遍）；`life` 行的 `prompt` 可以是空的。`dynamic` 照旧（没写的不动）。
- 给模型看的字三份（框的开头、框的收尾、出厂的推演方法）登记进 `26-提示词.md` 第十节，量出 token 数；`cargo xtask prompts` 重新生成。原文照设计 70 第三节第 3 条的草稿，收尾那一段用下面这一版（说清这里的 `prompt` 不是原样传回的）：

  ```text
  Set your next wakeup with schedule_wakeup. Here its prompt is not passed back: write a note to yourself for that wakeup, or leave it out. delay_seconds is how long until then; reason is what you are doing now. Without a call you wake again after the default interval.
  </life-wakeup>
  ```

### 要拍板的

- 三份字是草稿：等 LF-5 下能真的开一条人生会话以后，拿真模型走一遍再定稿（她写不写留言、还编不编真人的事、节奏对不对）。这一步先照草稿登记。

### 不做什么

- 不做编辑 `prompts/life.md` 的协议和界面（随 LF-8 头的那一步；人现在直接放文件）。
- 不改 `schedule_wakeup` 的工具说明：它和 `/loop` 共用，一个字不动。
- 不加出厂包 `base-life`、不做 `end_life`（LF-4）。
- 不动回合索引和抽取（LF-3）。

### 验收

1. `miyu-assemble`：`by.loop` 是 `life` 的那句包的是人生的一对标签、带任务编号；没有这一对字的照原样；`dynamic`、`fixed` 的一字不变。
2. `miyu-policy`：快照带上人生的一对；以前的快照（没有这两格）照旧读得回，指纹和改前一样（守着已有的冻结样本）。
3. `miyu-endpoint`（真核心、手拨的钟）：
   - 人格目录有 `prompts/life.md` 的：`message.user` 的正文是它的原文、空一行、留言；
   - 没有这个文件、文件是空的、会话不带人格的：用出厂的那一段；
   - 留言是空的：正文只有推演方法，末尾没有多出来的空行；
   - 她这一轮调 `schedule_wakeup` 带了 `prompt` 的，下一次的留言是它；没带的，留言清成空的；`dynamic` 的没带照旧不动；
   - 改了 `prompts/life.md`，下一次到点用新的。
4. 原有的定时、循环、人格测试一条不改、全过。
5. 门禁全过；Windows 版 clippy 过。

### 风险

- 快照多两格：以前的快照要读得回、指纹不变，不然全体老会话的快照对不上。照 `core.schedule` 现在「缺字的当没有」的做法加，冻结样本的测试守着。
- 到点时读人格文件是 I/O：只在端点这一层做，不进内核、策略、组装。
- 人格文件的路径：照人格存储现成的读法（三层叠），不自己拼路径。
