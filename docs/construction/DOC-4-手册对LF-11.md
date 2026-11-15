## 施工单 DOC-4：手册写上「后天」，关机那一行改宽

状态：待验收（2026-11-16）。

### 目的

LF-11 给推演人生加了「后天」：AI 在人生会话里记下自己变了什么，这个人格的每个会话都带着。手册中英两份写上它。另改 DOC-3 留下的一处：权限页「Windows 上的写法」里关机那一行比代码窄。

### 改了什么

中文页和英文页同一处一起改。

- `guide/life`：加一节「后天」（记什么、一条多长、最多几条、每个会话都带着、别的入口动不了、你能作废和找回但改不了、多久记一次、人生结束时清掉、怎么单独关）；开头的包表和工具那一条补 `note_change`。
- `guide/tools`：工具表加 `note_change` 一行。
- `guide/packages`：`base-life` 一行补上。
- `guide/personas-presets`：`life` 预设开着的功能、功能编号（`note_change` 是 `change`）、`persona` 组里有什么。
- `guide/memory`：五类后面加一段，说还有 AI 自己记的一类 `change`。
- `guide/packages`：「会让供应商自己跑」那一行 `miyu pkg info` 里也有（RB-1 起从软件源装的包也说得出，手册那一句原来就没限定装法）。
- `guide/permissions`：关机那一行改成「`shutdown` 带开关的都算，只有 `/a`、`/?` 不算」。

### 没写的

- 记的时候写的原因（`why`）：给人看的，但终端、网页的记忆页还没显示它，等头做了再写。
- 命令行 `miyu memory list --class` 不收 `change`，手册不写怎么单独列这一类。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过。
2. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
3. 写进去的界面上的字对过 `resources/packages/base-life/package.toml`、`resources/software/life/human/`、`crates/miyu-endpoint/src/refusal/message.rs`；单独关的写法对过 `crates/miyu-endpoint/tests/life_change.rs`。

### 风险

- 「后天」照 LF-11 的施工单和测试写，没有在真的人生会话里看过它记下来的样子。
