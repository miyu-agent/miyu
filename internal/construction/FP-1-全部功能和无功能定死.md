# FP-1 全部功能和无功能定死

状态：待验收（2026-11-28）

## 为什么

2026-11-28 项目主人定：「全部功能」（`full`）和「无功能」（`none`）两个预设不许修改，并且在列表里置顶。终端、网页都照 `preset.list` 给的先后画，所以置顶和拒改都在核心做，头只管显示。原来 `none.toml` 写着「能编辑，改了写进家目录那一层」，机主的数据根里可能已经有这两个预设的改动：主会话定不再读、照出厂的算，文件是人的数据、不删，核心起来时记一行警告。

## 蓝图改哪几节

- `presets.md`：新加「定死的」一节；「无功能」去掉「能编辑」；「怎么走」第 7、8 条，「改」，「协议」，「守着它的」。
- `protocol.md`：`preset.list`、`preset.get` 多 `fixed`；`preset.set` 第 7 条、`preset.delete` 多 `preset_fixed`；出错表、给人看的话表各一行。
- `log.md`：`core` 那张表多 `WARN fixed preset override ignored`；日志门禁 `xtask/src/logs.rs` 的 `ALLOWED` 加一条：`start.rs` 的 `preset` 只会是出厂定死的编号，不是人起的名字。

## 改了什么

1. **名单是数据**：出厂的 `full.toml`、`none.toml` 的 `[preset]` 写 `fixed = true`。`miyu_policy::preset::read_shipped` 认它，`read`（包带的、你那一层）照旧当不认识的键（`unknown_key`），`miyu check` 也报；`fixed` 不进指纹。
2. **只认出厂的**：`miyu_store::presets` 叠定死的预设时跳过出厂以外的层；`Presets::fixed` 问一个编号定没定死，`Presets::ignored` 列出你那一层里不再读的那几份。
3. **核心起来时**（`crates/miyu-core/src/start.rs`）：`ignored` 里每一份记一行 `WARN fixed preset override ignored`，带 `preset`。
4. **协议**：`preset.list` 定死的排最前面（稳定排序，彼此照编号，`full` 在 `none` 前面），其余照编号；`preset.list`、`preset.get` 定死的多一格 `fixed: true`，别的不带。`preset.set`、`preset.delete` 拒 `preset_fixed`（带 `new` 的照旧先拒 `preset_exists`），给人看的话「这个预设不能修改或删除。」「This preset cannot be changed or deleted.」。
5. **别的写入口**：在会话的预设里打开装上的包（`packages/port.rs` 的 `open_in`）走 `preset.set`，同样拒，记一行就算了，装照样算成。终端、网页的预设编辑都走 `preset.set`、`preset.delete`；命令行只有 `ask`、`schedule` 的 `--preset`（选，不改）；她的工具里没有改预设的。
6. **文档**：`none.toml`、`full.toml` 开头的注释；用户手册「人格和预设」（出厂的预设、自己编写预设的例子换成常规开发、界面里改、三层、排查问题）；协议参考 `personas.md`、`errors.md`，中英两份。

## 不做什么

- 不删、不改你那一层已有的 `full.toml`、`none.toml`。
- 不在 `miyu check` 里另报一种警告：起来时那一行就是说明。
- 头那边不归这一步：网页、终端照 `fixed` 不给改、不给删，由各自的会话做。

## 验收

- `crates/miyu-endpoint/tests/preset_fixed.rs` 三个：列表顺序和 `fixed`；`preset.set`、`preset.delete` 拒、文件不动；你那一层以前的改动不读、文件留着。改前三个都红。
- `crates/miyu-endpoint/src/packages/port/tests.rs` 的 `opening_in_a_fixed_preset_writes_nothing`：改前红（写进了 `none.toml`）。
- `crates/miyu-store/src/presets/tests/fixed.rs` 三个：只叠出厂那一份、`ignored`；别的层写不得 `fixed`；出厂的只有 `full`、`none` 定死、不进指纹。
- 原有的 `preset_set.rs`（改出厂的、删回出厂的）照全部功能写的换成极简开发；`preset_none.rs` 无功能排第二、带 `fixed`。
- 门禁全过，三平台 CI。

## 风险

- 机主以前改过 `none.toml` 的会话，升级以后下一个回合照出厂的换一次快照、断一次缓存。这是定了的代价。
