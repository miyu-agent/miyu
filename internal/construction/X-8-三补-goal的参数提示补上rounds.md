## 施工单 X-8 三补：`/goal` 的参数提示补上 `--rounds`

状态：已完成（2026-11-18）。

### 为什么

`/goal` 设、改的时候能在目标前面写 `--rounds N` 定最多跑几轮（施工 X-8 再补）。但核心给头的那两格里都没有它：提要 `commands/argument/goal` 是 `[<目标> | pause | resume | clear]`，几种写法（`usage`）里也没有。终端、网页的参数提示都照这两格画（终端核实过），人看不出能写 `--rounds`。手册的命令表早就写着 `[[--rounds <轮数>] <目标> | …]`，和程序给的对不上。核心主会话 2026-11-18 交的。

### 改成什么样

- 提要：`[[--rounds <轮数>] <目标> | pause | resume | clear]`（英文 `<rounds>`、`<goal>`，日文 `<回数>`、`<目標>`），和手册一样。
- 几种写法多一条，排在「设定」后面：`/goal --rounds <轮数> <目标>`，说明「设定或改的时候定最多跑几轮；0 是不限」。占位的那一段是新的 `commands/placeholder/goal-rounds`。
- 命令怎么读一个字没改，只是把已经有的写法说出来。

### 验收

- 改前红：`command.catalog` 里 `/goal` 那一项的两条测试（`command_catalog.rs`、`goal_command.rs`）照新的写，在没改的字上红。
- 门禁（三种语言的键一样多由 `human_languages` 守着）。
