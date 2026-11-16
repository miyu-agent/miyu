## 施工单 DOC-7：手册去掉私密目录，只读照实写

状态：待验收（2026-11-16）。跟 PV-RM：项目主人 2026-11-16 定，只读就是开放权限减去写，Miyu 不替人藏东西；自己的数据根照旧碰不到。

### 改了什么

中文页、英文页同一处一起改。

- `guide/permissions`：「私密目录」一节整节删掉。两种权限的对照表：读那一行只读同开放权限；扩展里联网的工具单列一行，只读下也直接用；MCP 里没标只读的、扩展里对外发消息的照旧每次问。加一段：只读不替你藏东西，`~/.ssh`、浏览器的数据这些照样读得到、读了会发给模型供应商，不想让它读到的自己另布沙盒或换系统用户；数据根哪一种权限下都碰不到。确认框从三个选择改成两个（去掉「此目录不再询问」）。「出了问题」表里私密目录的两行、项目配置一节和三个平台表里提到它的地方去掉。
- `guide/security`：「挡的」表去掉「钥匙、登录、浏览器数据」一行；「不挡的」表里「你的文件发给模型供应商」一行照实改写。
- `guide/config-keys`：删 `sandbox.private.enabled`、`.add`、`.remove` 三行。
- `guide/config`（示例配置和生效时机表）、`guide/data`、`guide/heads/web`（确认框、设置页一行）、`guide/heads/tui`（设置页一行）、`guide/README`：去掉提到它的地方。

### 实测

沙箱数据根，当前 main（含 PV-RM）编的 `miyu`，Linux，测试端点的模型：

- `miyu config set sandbox.private.add '["~/notes/keys"]'`：「没有 sandbox.private.add 这一项」，退出码 1。配置文件里手写的：`miyu check` 警告「没有 sandbox.private.add 这一项。已忽略此行。」。
- 只读下让 AI 用命令读沙箱里一个假的 `~/.ssh/id_test`：读到了，没有问。（模型自己不肯把整份「私钥」贴出来，只印了头一行，那是模型的事，不是 Miyu 拦的。）
- 只读下 `touch ~/.ssh/x`：「权限不够」，文件没建出来。
- 读数据根里的 `etc/private/secrets.toml`：只读、开放权限下文件工具都记「没做：这是 Miyu 自己的数据，谁都不能碰」；只读下用命令 `cat` 数据根里的配置：「权限不够」。

没实测：确认框只剩两个选择（要在终端、网页里看，开发数据根里没装这两个包），照 PV-RM 的施工单写；扩展里联网的工具只读下直接用，照 `crates/miyu-session/src/guard/untargeted.rs` 写；Windows、macOS。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. `docs/guide`、`docs/dev` 里搜不到「私密目录」「sandbox.private」「此目录不再询问」和它们的英文。
3. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
