## 施工单 LIC-1：许可证分层，核心换 AGPL-3.0-or-later

状态：主会话做。项目主人 2026-10-26 定：分层——核心和第一方的程序 AGPL-3.0-or-later，给别人照着写东西用的 Apache-2.0。

### 为什么

Miyu 以后主要是在服务器上给人连着用（远程是主入口），GPL 管不到「改了只做成网上服务、不分发」，正是设计 12 R15 写的重开条件。分层是为了不吓着写扩展、软件包、另写头的人。

### 做了什么（这个仓库）

- `LICENSE` 换成自由软件基金会发的 AGPL-3.0 原文（34523 字节，sha256 `0d96a4ff…`）；新加 `LICENSES/Apache-2.0.txt`（Apache 软件基金会原文，11358 字节，sha256 `cfc7749b…`）。
- `Cargo.toml` 的 `license = "AGPL-3.0-or-later"`（每个 crate 照工作区）。
- Apache-2.0 的几处：`docs/authors/`、`docs/reference/`、`resources/packages/authoring/`；各自开头写明。README「许可证」一节写清分层，并写明只经协议和 Miyu 说话的程序（扩展、软件包、脚本、MCP 服务器、另写的头）是独立的作品。
- `AGENTS.md` 依赖那条、蓝图 `licenses.md`、设计 12 R15、门禁的「许可证」那一项的说法（能用的许可证那张表不变：能和 GPL-3.0 合的宽松许可证也都能和 AGPL-3.0 合，MPL-2.0 把 AGPL 列为次要许可证）。

### 别的仓库（各自的会话改，主会话 10-26 通知）

- 终端 `miyu-tui`、网页 `miyu-web`、接入通讯平台 `miyu-im`、渲染 `miyu-render`、本机语义模型 `miyu-embed`、表情包 `miyu-memes`：GPL-3.0-or-later → AGPL-3.0-or-later。
- 开发包 `miyu-sdk`：MIT → Apache-2.0。
- 打包：AUR 三份 PKGBUILD 的 `license`、Homebrew formula 的 `license`，下一次发版时跟着改（发版链手册）。

### 不做的

- `miyu-client` 不放宽：它带着核心的一大半，放宽了等于把核心放宽。以后要让第三方的 Rust 头不受 AGPL 约束，另拆一个只有协议类型和连接的薄 crate 用 Apache-2.0。
- 以前按 MIT、GPL 公开的提交照旧是那时的许可证。

### 验收

1. `cargo xtask check` 全过（「许可证」一项照新说法报）；`xtask` 的许可证单元测试照新说法过。
2. `LICENSE`、`LICENSES/Apache-2.0.txt` 的字节数、sha256 和蓝图 `licenses.md` 写的一样。
