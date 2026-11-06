## HK-1 补 被拒的那一步，标题也配上给人看的那一句

状态：施工完，待 core1 审（2026-11-07）。core1 网页实测 HK-1 时报的。蓝图 `view.md`「标题那一句」；参考 `view`（中英）。

### 目的

写入、编辑、命令被守卫、权限策略、只读拦下（`tool.result` 的 `status` 是 `denied`）时，时间线上的标题也接上为什么：
「acme-guard 拦下了：周五不许写」「这里不能执行命令」「只读模式，未执行」。原来视图只给没分类的工具、记住、搜索、抓取（还有没送到的
留言）配结果那一句，命令、编辑、写入被拒了人只能点开看给模型的英文 `Blocked by acme-guard: …`。

### 做了什么

- `miyu-view` 的 `title.rs`：结果那一句先看状态，被拒的不分工具都照结果的 `human` 换成字；没带 `human` 的、换不成字的照旧不写。
  别的照旧：命令、编辑、子代理成了的、出错的不写（有预览、加减的行数、任务编号），留言送到了的不写。
- 拒的说法都是现成的，不加字：守卫拒的 `core/guard/…`（HK-1 起，`miyu-endpoint` 的 `extensions/guards/texts.rs`）；权限策略拒的
  一直带 `human`（`miyu-session` 的 `guard.rs` 的 `deny`：`core/permissions/forbidden`、`forbidden-kb`、`unresolvable`、
  `no-commands` 这些，只读时的写照 `core/tool-results/read-only`；看一眼卡片当场答不了的照工具交的 `human`，可能没有）；内核拦下的
  （只读、没人确认）、人拒的照 `core/tool-results/…`。
- 导出给人看的 Markdown（`read`）照 `title.said` 写那一行，被拒的跟着多结果那一句（「acme-guard 拦下了：理由 · 被拒」）；
  `agent`、`chat` 不看它，不变。

### 定了的（技术题）

- 人拒的（确认时点了拒绝）也算被拒，一样写「已拒绝：理由」：同一个状态，不分谁拒的；要人确认的那一次照旧另记在 `approval` 上。

### 不做什么

- 不加给人看的字、不改给模型的字：视图只给头看，不进请求。网页、终端照 `title.said` 画，不用改。
- 出错的命令、编辑不配那一句（原样）。

### 验收

- `miyu-view` 的 `tests/denied.rs`：守卫拒的写入、编辑，权限策略拒的命令，只读时内核拦下的写入，中文、英文的 `title.said` 都是那一句，
  对象照旧；被拒的没带 `human` 的不写。`tests/lines.rs`：手写的日志里命令被拒的写、出错的不写。没改 `title.rs` 时这六条里五条红（没带
  `human` 那条是对照，本来就绿）。
- 变异：拿掉被拒那一格、被拒的照写 `None`，各五条红；被拒那一格改成「出错、被拒都算」，`lines` 那条红。
- 请求不变：`miyu-assemble --test all probe` 38 条、`miyu-kernel --test all request_sample` 2 条照旧绿。
- 门禁 11 项、CI 六项。

### 风险

- 头若另外按状态写了「被拒」，标题后面再接一句，人会看到两处：网页照 `title.said` 画，状态是图标和颜色，不重；终端同。
