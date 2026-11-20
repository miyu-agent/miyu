## 施工单 DOC-5：手册写上软件包带的模型驱动（antigravity）

状态：已完成（2026-11-16 合进 main 4addbfd91）。

### 目的

社区的包 `antigravity` 0.1.0 进了软件源，它带一个模型驱动，把请求交给本机的 Antigravity CLI。手册原来只列了核心自带的三种驱动，迁移页还写着旧版的 `antigravity` 中转「这一版没有对应的驱动」。中英两份跟着改。

### 改了什么

- `guide/models`：驱动表后面加一段：软件包也能带驱动，`driver` 写 `<包编号>/<驱动名>`，举 `antigravity` 为例（只有 Linux、macOS），装法配法指到包自己的说明，自己写驱动指到作者教程。
- `guide/migrate`：「没迁的」表里 CLI 中转那一行：`antigravity` 有社区的包，别的三家照旧换成接 API 的。
- `guide/packages`：「会让供应商自己跑」那句的例子换成这个包真的会印的一行；「社区收录过、占了编号的」加 `antigravity`。

### 没写的

- 这个包怎么装、怎么配、往 Antigravity CLI 的配置里写什么：是社区包自己的事，写在它的 README 里，手册不抄。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过。
2. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
3. 包编号、驱动名、支持的系统、会让供应商跑的工具对过 `miyu-antigravity` 的 `package/antigravity/package.toml`；「上游」那一行的名字对过 `crates/miyu-cli/src/language/pkg/info.rs`。

### 风险

- 社区包以后改了驱动名、支持的系统，手册这几句要跟着改。
