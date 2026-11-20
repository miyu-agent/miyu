## BS-1 下下：斜杠命令照背后那件归属

状态：主会话做。设计 44（基础系统拆包）第三节第 3 条，项目主人 2026-10-26 指出：内置的斜杠命令只是方法的另一种写法，照背后那件归属，`/clear`、`/stop`、`/workspace` 归核心，`/todo` 跟着 `todowrite`，没装的就没有这条。2026-10-27 又定：别的跟着包的命令一样（`/goal`、`/remember`、`/dream`、`/clear-memory`、`/loop`），没装就没有这条命令。

### 一、做了什么

1. **拥有它的包可以没有**：`Slash::package` 改成 `Option`，没有的是核心自带的（`/clear`、`/stop`、`/workspace`）；`/todo` 写 `todowrite`，`/goal`、记忆三条、`/loop` 照旧。
2. **包没装就没有这条**（`registry::present`、`Commands::find_present`）：`command.run`、在后台答不答、`command.catalog`、模板和扩展声明的命令「被内置的盖住」那几处，都只认包装着的；没装的照「没有这条命令」，模板照平常认。
3. **名字空出来**：装包查斜杠命令撞名（`Commands::reserved`）只比包装着的；撞了核心自带的记成 `Holder::Core`，`data.conflicts` 写 `{"kind", "name", "core": true}`，命令行说「是核心自带的」，她的 `package` 工具照 `core` 写。
4. **删掉走不到的**：「人格记忆未安装」「长期目标未安装」两句（`commands/unavailable/not-installed`、`goal-not-installed`，三种语言）和 `why.rs` 里判没装的两段：包没装的命令到不了那里。装着但预设关了、群里用不了的照旧说为什么。
5. **文档**：蓝图 `slash.md`、`packages.md`「冲突」、`memory.md`。

### 二、怎么验

1. 端点 `slash_owners.rs`：卸掉 `todowrite` 以后菜单不列 `/todo`、核心自带的三条在，敲 `/todo` 是 `unknown_command`；别的包带的 `/todo` 模板装得上；带 `/stop` 模板的包装不上，冲突写 `core: true`。
2. 跟着改：`memory_installed.rs`（没装记忆敲 `/remember` 是 `unknown_command`）、`packages_conflicts.rs`（`/reset` 撞了写 `core: true`）、`miyu-store` 冲突的测试。
3. 手写变异 7 个全抓到：找命令不看包装没装、撞名照旧比没装的包、「装着」恒真、菜单不滤、`/todo` 不归 `todowrite`、核心自带的记成包、冲突不写 `core`。
4. 本机门禁。
