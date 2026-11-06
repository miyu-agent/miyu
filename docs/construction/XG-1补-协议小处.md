## XG-1补 扩展工具协议的几处小出入和一处措辞

状态：施工完，待 core1 审（2026-11-06）。设计 `63-扩展工具的效果和声明.md`（XG-1 上、下已合）。开发包 `miyu-sdk` 对照协议时发现参考
和核心有几处出入，core1 定了改法；措辞那一处是 e0 复核提的。蓝图 `providers.md`（规格三格、扩展交的效果第 1 条、会跑命令的第 2 条、
出错、守着它的）、`protocol.md`（`bad_tool`）；协议参考、作者文档 `extensions`（中英）；设计 24、设计 63 第九节各补一句。

### 目的

1. `todo.written` 的 `done: null`：核心整条丢、记 `shape`，和别处「`null` 当没写」（`effects`、`state`、规格的格）不一样。改成当没写。
2. 规格的 `runs_commands` 写 `null`：代码照「没写」放行（serde 的 `Option` 把 `null` 读成没有），参考、蓝图写的却是「不是布尔的回
   `bad_tool`」。照代码写明；`paths`、`background_parameters` 写 `null` 一样，核对后一起写明。
3. `file.read` 的 `lines`：代码要求从 1 数起、从不大于到（不合的整条丢），参考只写了「从 1 数起」。补上。
4. 认「扩展进程的子孙」那几处（扩展进程自己不算），读起来像在挡扩展。写清楚这是防误用，不是安全边界：扩展起一个子进程就能调；
   对扩展真正的关口是能力 `tools.commands` 要人批。

### 做了什么

- **代码**（`crates/miyu-endpoint/src/provide/effects.rs`）：`todo.written` 先把 `done: null` 去掉再照 `TodoWritten` 读，写成别的
  样子的（字、对象）照旧整条丢。`TodoWritten` 本身（`miyu-kernel`）不动：日志、事件里的写法不变。
- **规格三格写 `null`**：代码不动。`ToolParams` 的三格都是 `Option<Value>`，`null` 读进来就是没有，和不写一样登记、缓存（缓存
  不写出 `null`）。加一条测试钉住。
- **文档**：
  - 协议参考 `extensions`（中英）：`paths`、`background_parameters`、`runs_commands` 三格写明 `null` 同不写；出错里
    `runs_commands` 改成「写了真假、`null` 以外的」；效果表 `file.read` 的 `lines` 补「从不大于到（写错的整条丢）」，
    `todo.written` 补「`done` 写 `null` 同不写」；「跑命令」认子孙那条补一句防误用、不是挡扩展、关口是 `tools.commands`。
  - 作者文档 `extensions`（中英）：`file.read` 的 `lines` 补「从不大于到」；「跑命令的工具」第 3 条补同一句。
  - 蓝图 `providers.md`：规格 `paths`、`background_parameters`、`runs_commands` 写 `null` 同不写；扩展交的效果第 1 条 `done`；
    会跑命令的第 2 条补「防误用，不是安全边界」；出错表；守着它的两行加这一步的测试。`protocol.md` 的 `bad_tool` 那一格同改。
  - 设计 24（接受的风险，扩展的通道那一行）：认子孙加括号说明防误用、不防扩展本身。设计 63 第九节「已知边界」加一条。
  - `crates/miyu-call/src/kin.rs` 模块说明补同一句（只改注释）。

### 定的技术题

1. **`done: null` 在端点去掉，不改 `TodoWritten`**：改 `miyu-kernel` 的反序列化会连日志、事件的读法一起放宽（内核那份是核心自己写
   的，不该有 `null`）；只在收扩展回应的这一处当没写，范围最小。
2. **`todos: null` 照旧丢**：`todos` 是必填的，写 `null` 等于没写，没写本来就是 `shape`，和「`null` 当没写」一致。
3. **措辞只写事实，不改行为**：认子孙、扩展进程自己不算照旧（XG-1 下技术题 3）；只把它的作用说准。

### 不做

- 开发包 `miyu-sdk`（另一个仓库）跟着改：另做。
- 给模型看的字：没加。
- `miyu-store` 的受限文件：没碰。

### 怎么验

- `provide/tests/effects_tests.rs` 的 `a_null_done_on_todos_is_unwritten`：`done: null` 收下（`done` 空），写了的照收，`done` 写成
  字、对象的丢，`todos: null` 丢。
- `provide/tests/declared_tests.rs` 的 `null_declarations_are_unwritten`：规格三格写 `null` 登记得上，不报路径、没有只在后台给的参数、
  不跑命令。
- `miyu-endpoint` 单测全过；门禁 11 项（`xg1d`）；三平台 CI。

### 变异

| 改坏 | 抓住的 |
|---|---|
| `done: null` 不去掉（照旧当 `shape`） | `a_null_done_on_todos_is_unwritten` |
| 不是数组的 `done` 都去掉（写错类型的也收） | `a_null_done_on_todos_is_unwritten` |
| `paths` 的 `null` 送去查写法（回 `bad_tool`） | `null_declarations_are_unwritten` |
| `runs_commands` 的 `null` 当写了别的（回 `bad_tool`） | `null_declarations_are_unwritten` |
