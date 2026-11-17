## PL-2 软件包声明支持的系统

状态：施工完，待验收（2026-11-17）。项目主人 2026-11-17 本机验收第 21 条提的，定「默认藏，可打开看」。

### 目的

- 作者能在清单里写支持哪几种系统（例如 Antigravity 包不做 Windows 适配，写只支持 Linux、macOS）；不写的照旧哪种都支持。
- 不支持本机的包，软件源的列表默认不列；设置里打开「其他系统的软件包」才列出来、标「不支持该系统」；装的时候拒，说「不支持该系统」。

### 蓝图改哪几节

- `packages.md`：「格式」例子多 `systems` 一行；「依赖」末尾多第 11 条「软件源索引里的 `systems`」；新加一节「支持的系统」（和 `targets` 的分别、本机支不支持怎么算、列表、装、配置项的名字）。
- `protocol.md`：`package.search` 多一条「不支持本机的默认不列，开着的带 `unsupported`」。
- `cli/pkg.md`：`search` 不支持本机的接「（不支持该系统）」。
- 设计 `37-软件源.md`：第二节第 3 条补「支持的系统」，第三节第 2 条补索引的 `systems`。
- 协议参考 `packages`、`errors`（中英）：`package.search` 每一项、`package.info` 软件源那一份多 `unsupported`，`wrong_target` 改说法。
- 作者文档 `manifest`（中英）：例子清单、`[package]` 表多 `systems`。

### 定了的

- 键名 `systems`，值 `linux`、`macos`、`windows`：清单里「平台」说的是通讯平台（`[connection] platform`），系统这一格照 `targets` 叫「系统」；只说操作系统，处理器归 `targets`。
- 和 `targets` 分开：`targets` 是带了本机程序的包「有哪几种包文件」，`systems` 是作者「支持哪几种」。两样都写的，`targets` 里每一种的系统都要在 `systems` 里（不然 `bad_systems`，免得写出一份装不上的包文件）。没写 `systems` 的照 `targets` 推：有本机这一种包文件才算支持（比只看系统更准，Linux ARM 上不列只有 x86-64 包文件的）。
- 索引原样带 `systems`，客户端照它和 `files[].target` 算，不下载包文件；以前的索引没有这一格，读成哪种都支持。
- 配置项 `packages.other_systems`，名字「其他系统的软件包」/「Packages for other systems」/「他のシステム向けのパッケージ」：UX-1 的规矩里开关的名字不许带「不」「隐藏」，「显示不支持本机的软件包」「隐藏不支持的软件包」都不合；说明里写清是不支持本机系统的那些。
- 拒绝照旧用 `wrong_target`（同一件事：不支持本机），话改成「不支持该系统。」/「Not supported on this system.」。从软件源装的照索引就拒，不下载。

### 不做什么

- 不改软件源仓库：收录（`miyu-index`）在核心仓库的 `miyu-source` 里，这一步已经写进去；软件源仓库换 `miyu-rev` 到这一步合进 main 的提交以后，新生成的索引才带 `systems`。
- 不改终端、网页：头列软件源照 `package.search`，默认就不列了；打开设置后画「不支持该系统」标记是头的事（交回的清单）。
- 装着的包后来写了不支持本机：照旧能用，升级时拒（`wrong_target`）。

### 验收

1. 改前红：`miyu-config` `package::system::tests` 三条（main 上清单不认 `systems`，`unknown_key`）；`miyu-endpoint` `packages_systems` 两条（main 上 `away` 照列、照装）。
2. 跟着改的测试：`package::fields::tests::every_table_and_key_is_in_the_authors_docs`（作者文档写上 `systems`）；配置样本 `MIYU_SAMPLE_WRITE=1` 重写。
3. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
4. 实测（沙箱数据根）：本地软件源两个包，`win-only` 写 `systems = ["windows"]`、`anywhere` 不写；`miyu pkg search` 只列 `anywhere`；`miyu config set packages.other_systems true` 以后两个都列，`win-only` 接「（不支持该系统）」；`miyu pkg install win-only` 被拒「不支持该系统」。
   - 2026-11-17 实测过（调试版核心，`miyu-index folder` 收的本地软件源）：索引里 `win-only` 带 `systems: ["windows"]`；`miyu pkg search only` 关着时什么都不印、退出码 1，打开后印「win-only  只支持 Windows  只支持 Windows 的包（社区）（不支持该系统）」；装被拒「miyu: 不支持该系统」（英文「miyu: not supported on this system」），本地服务器的日志里没有下过 `win-only` 的包文件；`anywhere` 照装。

### 风险

- 写了 `systems` 的包，旧版核心读清单报不认识的键：发版以前不管（没发版前不做旧兼容）。
- 头要跟着画 `unsupported`：没画的头打开设置后会把不支持的包当普通包列出来，装的时候核心照样拒。
